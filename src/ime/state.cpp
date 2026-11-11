#include "state.h"
#include "engine.h"
#include "config/config.h"
#include "config/sites.h"
#include "window/window_info.h"
#include "de/kde.h"
#include "uinput/uinput.h"
#include "utils/log.h"
#include "utils/text_utils.h"

#include <fcitx/inputcontext.h>
#include <fcitx/instance.h>
#include <fcitx/addonmanager.h>
#if __has_include(<fcitx-module/clipboard/clipboard_public.h>)
#define CLAK_HAVE_FCITX5_CLIPBOARD 1
#include <fcitx-module/clipboard/clipboard_public.h>
#endif
#include <fcitx-utils/keysym.h>
#include <fcitx-utils/utf8.h>
#include <dlfcn.h>

namespace {
// dynamically resolve custom im info if supported by host fcitx5 version
void showInputMethodInfo(fcitx::Instance* instance, fcitx::InputContext* ic, const std::string& info) {
    if (!instance || !ic) return;
    using ShowCustomFn = void (*)(fcitx::Instance*, fcitx::InputContext*, const std::string&);
    static auto showCustom = reinterpret_cast<ShowCustomFn>(
        dlsym(RTLD_DEFAULT, "_ZN5fcitx8Instance32showCustomInputMethodInformationEPNS_12InputContextERKNSt7__cxx1112basic_stringIcSt11char_traitsIcESaIcEEE")
    );
    if (showCustom) {
        showCustom(instance, ic, info);
    } else {
        instance->showInputMethodInformation(ic);
    }
}
} // namespace

namespace clak {
namespace ime {

ClakState::ClakState(ClakEngine* engine, fcitx::InputContext* ic)
    : engine_(engine), ic_(ic) {
    rust_ctx_ = clak_context_new(CLAK_METHOD_TELEX);
    syncConfig();
}

void ClakState::syncConfig() {
    if (!engine_ || !rust_ctx_) return;
    const ClakConfig* cfg = engine_->config();
    if (cfg && applied_config_version_ != engine_->configVersion()) {
        clak_context_apply_config(rust_ctx_, cfg);
        applied_config_version_ = engine_->configVersion();
    }
}

void ClakState::flushCollisionStats() {
    if (collision_stats_.total_replaces > 0 || collision_stats_.buffered_key_interruptions > 0 || collision_stats_.batch_absorbed_ops > 0) {
        utils::clakLog("[COLLISION_STATS] replaces=" + std::to_string(collision_stats_.total_replaces) +
                       " collisions=" + std::to_string(collision_stats_.total_collisions) +
                       " missing_rtt=" + std::to_string(collision_stats_.missing_rtt_count) +
                       " key_interrupts=" + std::to_string(collision_stats_.buffered_key_interruptions) +
                       " batch_absorbed=" + std::to_string(collision_stats_.batch_absorbed_ops));
        collision_stats_ = {};
    }
}

void ClakState::onDeactivate() {
    flushCollisionStats();
    surr_cursor_underflow_count_ = 0;
    if (!is_deleting_) {
        verifier_.reset(/*hard_reset=*/true);
        bool is_terminal = (ic_ && ic_->capabilityFlags().test(fcitx::CapabilityFlag::Terminal)) ||
                           config::isTerminalApp(appKey());
        if (is_terminal) {
            resetHard();
        }
    }
}

ClakState::~ClakState() {
    flushCollisionStats();
    if (rust_ctx_) {
        clak_context_free(rust_ctx_);
        rust_ctx_ = nullptr;
    }
}

void ClakState::reset(bool force) {
    std::string app = appKey();
    std::string site = activeSite();
    if (is_deleting_) {
        utils::clakLog("reset: ignored mid-flight uinput deletion (sentinel in flight: " +
                       std::to_string(current_backspace_count_) + "/" + std::to_string(expected_backspaces_) +
                       " pending='" + pending_commit_string_ + "' app=" + app + " site='" + site + "')");
        return;
    }
    if (shift_press_time_us_ > 0) {
        shift_mouse_clicked_ = true;
    }
    uint64_t now_us = fcitx::now(CLOCK_MONOTONIC);
    if (!force) {
        // guard against false reset from web apps within 50ms post-commit window
        if (last_commit_time_us_ > 0 && (now_us - last_commit_time_us_) < 50000) {
            utils::clakLog("reset: ignored false reset within 50ms post-commit window (delta=" +
                           std::to_string(now_us - last_commit_time_us_) + "us) app=" + app + " site='" + site + "'");
            return;
        }
        if (in_flight_sentinel_count_ > 0 && now_us <= sentinel_grace_until_us_) {
            utils::clakLog("reset: ignored false reset while sentinel in flight app=" + app + " site='" + site + "'");
            return;
        }
        if (!key_buffer_.empty()) {
            utils::clakLog("reset: ignored reset while keys buffered (count=" +
                           std::to_string(key_buffer_.size()) + ") app=" + app + " site='" + site + "'");
            return;
        }
    }
    if (force) {
        if (collision_stats_.total_replaces > 0) {
            utils::clakLog("[COLLISION_STATS] replaces=" + std::to_string(collision_stats_.total_replaces) +
                           " collisions=" + std::to_string(collision_stats_.total_collisions) +
                           " missing_rtt=" + std::to_string(collision_stats_.missing_rtt_count));
            collision_stats_ = {};
        }
        last_autofix_original_.clear();
        last_autofix_converted_chars_ = 0;
        last_autofix_time_us_ = 0;
        last_key_time_us_ = 0;
        last_key_interval_us_ = 0;
    }
    uncommitted_forward_count_ = 0;
    last_text_len_ = 0;
    if (safety_timer_) {
        safety_timer_.reset();
    }
    if (commit_timer_) {
        commit_timer_.reset();
    }
    if (retired_commit_timer_) {
        retired_commit_timer_.reset();
    }
    ++fallback_seq_;
    if (surr_lag_timer_) {
        surr_lag_timer_.reset();
    }
    is_surr_lag_replaying_ = false;
    repeat_handler_.cancel();
    modal_handler_.reset();
    if (profile_) {
        profile_->reset();
    }

    if (!key_buffer_.empty() || !pending_commit_string_.empty()) {
        utils::clakLog("reset: clearing state (buf_len=" + std::to_string(key_buffer_.size()) + ") app=" + app + " site='" + site + "'");
    }
    is_deleting_ = false;
    is_address_bar_fix_ = false;
    is_selection_deletion_ = false;
    expected_backspaces_ = 0;
    current_backspace_count_ = 0;
    if (now_us > sentinel_grace_until_us_) {
        in_flight_sentinel_count_ = 0;
        sentinel_grace_until_us_ = 0;
    }
    pending_commit_string_.clear();
    batch_commit_.clear();
    is_replaying_ = false;
    key_buffer_.clear();
    last_surr_cursor_ = 0;
    last_surr_text_len_ = 0;
    surr_keystroke_count_ = 0;
    word_used_uinput_ = false;
    if (force) {
        verifier_.reset(/*hard_reset=*/false);
        if (config::isVSCodeApp(appKey())) {
            verifier_.setRichTextEditor(false);
        }
        vscode_invalid_surr_count_ = 0;
        last_vscode_uinput_commit_.clear();
        address_bar_has_space_ = false;
    } else {
        verifier_.clearPending();
    }
    cached_site_.clear();
    last_site_check_us_ = 0;
    if (rust_ctx_) {
        clak_context_reset(rust_ctx_);
    }
}

void ClakState::resetHard() {
    verifier_.reset(/*hard_reset=*/true);
    reset(/*force=*/true);
}

std::string ClakState::appKey() {
    if (ic_ && !ic_->program().empty() && !de::isDesktopShell(ic_->program())) {
        return ic_->program();
    }
    bool has_term_cap = ic_ && ic_->capabilityFlags().test(fcitx::CapabilityFlag::Terminal);
    platform::WindowInfo win = platform::getActiveWindow();
    if (!win.win_class.empty() && !de::isDesktopShell(win.win_class)) {
        if (!has_term_cap || apps::isTerminalApp(win.win_class)) {
            return win.win_class;
        }
    }
    if (has_term_cap) {
        return "terminal";
    }
    if (ic_ && !ic_->program().empty() && !de::isDesktopShell(ic_->program())) {
        return ic_->program();
    }
    return "default";
}

std::string ClakState::activeSite() {
    uint64_t now_us = fcitx::now(CLOCK_MONOTONIC);
    if (now_us - last_site_check_us_ < config::kCacheCheckIntervalUs && !cached_site_.empty()) {
        return cached_site_;
    }
    last_site_check_us_ = now_us;
    std::string fallback = (ic_ && ic_->capabilityFlags().test(fcitx::CapabilityFlag::Terminal))
                                ? "terminal"
                                : ((ic_ && !ic_->program().empty() && !de::isDesktopShell(ic_->program())) ? ic_->program() : "");
    platform::WindowInfo win = platform::getActiveWindow(fallback);
    std::string new_site = config::extractDomain(win.win_class, win.win_title);
    if (!new_site.empty() && !cached_site_.empty() && new_site != cached_site_) {
        // clear rich text and draftjs editor flags when switching sites
        verifier_.setDraftJsEditor(false);
        verifier_.setRichTextEditor(false);
        verifier_.setCanvasEditor(false);
    }
    cached_site_ = new_site;
    return cached_site_;
}

bool ClakState::isBrowser() const {
    std::string app = const_cast<ClakState*>(this)->appKey();
    return apps::isBrowserApp(app);
}

bool ClakState::isGecko() const {
    std::string app = const_cast<ClakState*>(this)->appKey();
    return apps::isGeckoApp(app);
}

bool ClakState::isSteam() const {
    std::string app = const_cast<ClakState*>(this)->appKey();
    return apps::isSteamApp(app);
}

bool ClakState::isDraftJsEditor() const {
    std::string site = const_cast<ClakState*>(this)->activeSite();
    if (apps::isClaudeSite(site)) {
        bool is_multiline = (ic_ && ic_->capabilityFlags().test(fcitx::CapabilityFlag::Multiline));
        if (!is_multiline && ic_ && ic_->surroundingText().isValid() &&
            ic_->surroundingText().text().find('\n') != std::string::npos) {
            is_multiline = true;
        }
        // claude chat composer is multiline and works reliably with surrounding text
        // only the search bar (single-line input) exhibits draftjs controlled behavior
        return !is_multiline;
    }
    return verifier_.isDraftJsEditor(site);
}

bool ClakState::looksLikeUrlAutofill(const fcitx::SurroundingText& surr) {
    if (!surr.isValid()) return false;
    const std::string& text = surr.text();
    if (text.empty()) return false;

    unsigned int cursor = surr.cursor();
    unsigned int anchor = surr.anchor();
    if (cursor == anchor) return false;

    unsigned int sel_start = std::min(anchor, cursor);
    unsigned int sel_end = std::max(anchor, cursor);
    if (sel_start == 0 || sel_end != text.size()) return false;

    for (size_t i = 0; i < text.size(); ++i) {
        unsigned char c = static_cast<unsigned char>(text[i]);
        // url bar autofill contains printable ascii without whitespace
        if (c < 0x21 || c > 0x7e) return false;
    }

    if (text.find(":/") != std::string::npos || text.rfind("about:", 0) == 0) {
        return true;
    }
    if (text.find("localhost") != std::string::npos) {
        return true;
    }
    if (text.rfind("www.", 0) == 0) {
        return true;
    }

    // require established address-bar evidence (recognized tld before path or port)
    size_t host_end = text.find_first_of("/:?#");
    std::string host = (host_end == std::string::npos) ? text : text.substr(0, host_end);
    if (host.empty() || host.front() == '.' || host.back() == '.') {
        return false;
    }

    static const std::vector<std::string> tlds = {
        ".com", ".org", ".net", ".edu", ".gov", ".mil", ".io", ".dev", ".app",
        ".ai", ".vn", ".me", ".co", ".info", ".xyz", ".biz", ".tv", ".cc",
        ".to", ".so", ".online", ".site", ".top", ".pro", ".uk", ".us",
        ".de", ".fr", ".jp", ".cn", ".ru", ".ca", ".au", ".in"
    };

    for (const auto& tld : tlds) {
        if (host.size() > tld.size() &&
            host.compare(host.size() - tld.size(), tld.size(), tld) == 0) {
            char before = host[host.size() - tld.size() - 1];
            if (isalnum(before) || before == '-') {
                return true;
            }
        }
    }

    return false;
}

apps::AppProfile* ClakState::currentProfile() {
    std::string app = appKey();
    std::string site = activeSite();
    int app_mode = 0;
    if (engine_ && engine_->config()) {
        app_mode = clak_config_get_app_mode(engine_->config(), app.c_str());
    }
    bool has_url_cap = ic_ && ic_->capabilityFlags().test(fcitx::CapabilityFlag::Url);
    bool is_browser = apps::isBrowserApp(app);
    bool is_url_site = is_browser && (site == "New Tab" || site == "about:blank" || site == "address-bar");
    bool is_url_shape = is_browser && app_mode == 0 && !apps::isMetaSite(site) && !apps::isMetaSite(app) &&
                        ic_ && looksLikeUrlAutofill(ic_->surroundingText());
    bool is_address_bar = has_url_cap || is_url_site || is_url_shape;
    bool is_draftjs = isDraftJsEditor();
    bool is_terminal = ic_ && ic_->capabilityFlags().test(fcitx::CapabilityFlag::Terminal);

    if (!profile_ || profile_app_ != app || profile_site_ != site ||
        profile_autofill_ != is_address_bar || profile_draftjs_ != is_draftjs ||
        profile_terminal_ != is_terminal || profile_app_mode_ != app_mode) {
        profile_ = apps::createAppProfile(app, site, is_address_bar, is_draftjs, is_terminal, app_mode);
        profile_app_ = app;
        profile_site_ = site;
        profile_autofill_ = is_address_bar;
        profile_draftjs_ = is_draftjs;
        profile_terminal_ = is_terminal;
        profile_app_mode_ = app_mode;
    }
    return profile_.get();
}

bool ClakState::shouldUseUinput(bool use_surrounding, uint32_t action_type, const fcitx::SurroundingText& /*surr*/) {
    if (modal_handler_.isModalEditor()) {
        return true;
    }

    auto* profile = currentProfile();
    if (profile->forcesUinput()) {
        return true;
    }
    if (profile->preventsUinput()) {
        return false;
    }
    if (profile->hasCustomDeletion()) {
        return false;
    }
    if (action_type == CLAK_ACTION_ADDRESS_BAR_FIX) {
        return true;
    }
    if (isBrowser() && (verifier_.isCanvasEditor() || verifier_.isRichTextEditor())) {
        return true;
    }
    return !use_surrounding;
}

bool ClakState::isAutofillCertain(const fcitx::SurroundingText& surr) {
    if (!surr.isValid()) return false;
    auto* profile = currentProfile();
    if (!profile->allowsAutofillHeuristic()) {
        return false;
    }
    const std::string& text = surr.text();
    // multiline editors (including ai inline ghost text in code editors) contain newlines
    if (text.empty() || text.find('\n') != std::string::npos) return false;

    unsigned int cursor = surr.cursor();
    unsigned int anchor = surr.anchor();

    if (cursor != anchor) {
        unsigned int sel_start = std::min(anchor, cursor);
        unsigned int sel_end = std::max(anchor, cursor);
        if (sel_end == text.length() && sel_start > 0) {
            return true;
        }
    }

    return false;
}

void ClakState::doCommitString(const std::string& text) {
    if (text.empty()) return;
    if (is_replaying_) {
        batch_commit_ += text;
        return;
    }
    doCommitDirect(text);
}

void ClakState::doCommitDirect(const std::string& text) {
    if (text.empty()) return;
    last_commit_time_us_ = fcitx::now(CLOCK_MONOTONIC);
    uncommitted_forward_count_ = 0;
    if (ic_) {
        ic_->commitString(text);
    }
}

void ClakState::forwardBackspacePressRelease(size_t count) {
    if (!ic_ || count == 0) return;
    // x11/wayland keycode 22 matches evdev KEY_BACKSPACE (14 + 8)
    const fcitx::Key bs_key(FcitxKey_BackSpace, fcitx::KeyStates(), 22);
    for (size_t i = 0; i < count; ++i) {
        ic_->forwardKey(bs_key, false);
        ic_->forwardKey(bs_key, true);
    }
}

bool ClakState::scheduleForwardBackspaceFallback(size_t count, const std::string& commit_str, const std::string& app) {
    forwardBackspacePressRelease(count);
    if (commit_str.empty() || !engine_ || !engine_->instance()) {
        return false;
    }

    repeat_handler_.cancel();
    is_deleting_ = true;
    expected_backspaces_ = 0;
    current_backspace_count_ = 0;
    pending_commit_string_ = commit_str;

    uint64_t delay_us = std::min<uint64_t>(20000ULL, 8000ULL + count * 2000ULL);
    uint64_t seq = ++fallback_seq_;
    if (commit_timer_) {
        retired_commit_timer_ = std::move(commit_timer_);
    }
    commit_timer_ = engine_->instance()->eventLoop().addTimeEvent(
        CLOCK_MONOTONIC,
        fcitx::now(CLOCK_MONOTONIC) + delay_us,
        1000,
        [this, seq, commit_str](fcitx::EventSourceTime*, uint64_t) {
            if (is_deleting_ && fallback_seq_ == seq) {
                is_deleting_ = false;
                pending_commit_string_.clear();
                if (commit_timer_) {
                    commit_timer_.reset();
                }
                replayBufferedKeys(commit_str);
            }
            return false;
        }
    );
    if (!commit_timer_) {
        is_deleting_ = false;
        pending_commit_string_.clear();
        return false;
    }
    commit_timer_->setOneShot();
    utils::clakLog("forward backspace fallback scheduled commit in " +
                   std::to_string(delay_us / 1000) + "ms app=" + app);
    return true;
}

std::string ClakState::classifyGroup(const std::string& app, const std::string& site, bool is_address_bar, bool used_uinput) {
    (void)app;
    (void)site;
    auto* profile = currentProfile();
    if (is_address_bar || (profile && profile->name() == "address-bar")) return "address-bar";
    if (used_uinput && profile->name() == "Chromium-SurroundingText") return "Chromium-Uinput";
    return profile->name();
}

void ClakState::logLatency(const std::string& group, uint64_t start_us, const std::string& action_type, const std::string& layer) {
    if (start_us == 0) return;
    uint64_t now_us = fcitx::now(CLOCK_MONOTONIC);
    double delta_ms = (now_us >= start_us) ? static_cast<double>(now_us - start_us) / 1000.0 : 0.0;
    std::string line = "[LATENCY] group=" + group + " delta_ms=" + std::to_string(delta_ms) + " action=" + action_type;
    if (!layer.empty()) {
        line += " layer=" + layer;
    }
    utils::clakLog(line);
}

void ClakState::logCsv(uint64_t rtt_us, const std::string& action) {
    if (op_start_us_ == 0) return;
    uint64_t now_us = fcitx::now(CLOCK_MONOTONIC);
    double delta_ms = (now_us >= op_start_us_) ? static_cast<double>(now_us - op_start_us_) / 1000.0 : 0.0;
    std::string frontend = (ic_ && ic_->frontend()) ? ic_->frontend() : "unknown";
    std::string app = appKey();
    std::string group = op_group_.empty() ? (currentProfile() ? currentProfile()->name() : "unknown") : op_group_;
    utils::logRttCsv(rtt_us, delta_ms, frontend, group, app, action);
}

void ClakState::observeTransactionLatency(uint64_t elapsed_us) {
    if (elapsed_us >= 35000) {
        if (adaptive_extra_us_ < 100000) {
            adaptive_extra_us_ = std::min<uint64_t>(100000, adaptive_extra_us_ + 10000);
        }
        stable_transactions_count_ = 0;
    } else if (elapsed_us <= 20000) {
        stable_transactions_count_++;
        if (stable_transactions_count_ >= 4) {
            if (adaptive_extra_us_ >= 10000) {
                adaptive_extra_us_ -= 10000;
            } else {
                adaptive_extra_us_ = 0;
            }
            stable_transactions_count_ = 0;
        }
    }
}

void ClakState::arm_safety_timer() {
    uint64_t now_us = fcitx::now(CLOCK_MONOTONIC);
    uint64_t timeout_us = is_selection_deletion_ ? config::kSelectionDeletionTimeoutUs :
                          ((is_address_bar_fix_ ? (config::kSafetyTimeoutUs * 2) : config::kSafetyTimeoutUs) + adaptive_extra_us_);
    uint64_t deadline_us = now_us + timeout_us;
    safety_timer_ = engine_->instance()->eventLoop().addTimeEvent(
        CLOCK_MONOTONIC,
        deadline_us,
        1000,
        [this, timeout_us, deadline_us](fcitx::EventSourceTime*, uint64_t) {
            uint64_t actual_fired_us = fcitx::now(CLOCK_MONOTONIC);
            if (is_deleting_) {
                if (adaptive_extra_us_ < 100000) {
                    adaptive_extra_us_ = std::min<uint64_t>(100000, adaptive_extra_us_ + 20000);
                }
                stable_transactions_count_ = 0;

                std::string app = appKey();
                std::string site = activeSite();
                bool is_draftjs = isDraftJsEditor() || config::isDraftJsSite(site) || config::isTikTokSite(site);
                bool is_terminal = ic_ && ic_->capabilityFlags().test(fcitx::CapabilityFlag::Terminal);
                pacing::PacingGroup pgroup = pacing::AdaptivePacingEngine::classify(app, site, is_address_bar_fix_, is_draftjs, is_terminal);
                pacing_engine_.observeTimerDrift(pgroup, deadline_us, actual_fired_us);
                uint64_t incident_now_us = actual_fired_us;
                if (is_selection_deletion_) {
                    utils::clakLog("[INCIDENT_TYPE] kind=SelectionTimeout buffered=" + std::to_string(key_buffer_.size()));
                    pacing_engine_.onIncident(pgroup, pacing::IncidentType::SelectionTimeout, incident_now_us);
                } else if (is_address_bar_fix_) {
                    utils::clakLog("[INCIDENT_TYPE] kind=AddressBarDelay buffered=" + std::to_string(key_buffer_.size()));
                    pacing_engine_.onIncident(pgroup, pacing::IncidentType::AddressBarDelay, incident_now_us);
                } else if (!key_buffer_.empty()) {
                    utils::clakLog("[INCIDENT_TYPE] kind=PacingStarvationWithBuffer(was_KeyInterruption) buffered=" + std::to_string(key_buffer_.size()));
                    pacing_engine_.onIncident(pgroup, pacing::IncidentType::PacingStarvation, incident_now_us);
                } else {
                    utils::clakLog("[INCIDENT_TYPE] kind=PacingStarvationIdle buffered=0");
                    pacing_engine_.onIncident(pgroup, pacing::IncidentType::PacingStarvation, incident_now_us);
                }
                utils::clakLog("SENTINEL TIMEOUT (" + std::to_string(timeout_us / 1000) + "ms expired): giving up waiting, only " +
                               std::to_string(current_backspace_count_) + "/" + std::to_string(expected_backspaces_) +
                                " BS returned! force committing '" + pending_commit_string_ + "' app=" + app +
                               " site='" + site + "'");
                size_t emitted = uinput::UinputTool::instance().cancel_pending();
                size_t expected_in_flight = (emitted > 0) ? emitted : expected_backspaces_;
                if (expected_in_flight > current_backspace_count_) {
                    in_flight_sentinel_count_ = expected_in_flight - current_backspace_count_;
                    sentinel_grace_until_us_ = fcitx::now(CLOCK_MONOTONIC) + 500000;
                }
                is_deleting_ = false;
                is_address_bar_fix_ = false;
                is_selection_deletion_ = false;
                expected_backspaces_ = 0;
                current_backspace_count_ = 0;
                if (surr_lag_timer_) {
                    surr_lag_timer_.reset();
                }
                is_surr_lag_replaying_ = false;

                std::string flush_str = pending_commit_string_;
                pending_commit_string_.clear();
                auto pending = key_buffer_.take_keys();
                for (const auto& k : pending) {
                    bool has_ctrl_alt = k.states().test(fcitx::KeyState::Ctrl) ||
                                        k.states().test(fcitx::KeyState::Alt) ||
                                        k.states().test(fcitx::KeyState::Super);
                    std::string str = fcitx::Key::keySymToUTF8(k.sym());
                    if (!has_ctrl_alt && !str.empty() && k.sym() < 0xff00) {
                        flush_str += str;
                    } else if (k.sym() == FcitxKey_BackSpace && !flush_str.empty()) {
                        utils::popUtf8Chars(flush_str, 1);
                    } else {
                        if (!flush_str.empty()) {
                            doCommitDirect(flush_str);
                            flush_str.clear();
                        }
                        if (ic_) {
                            ic_->forwardKey(k);
                        }
                    }
                }
                if (!flush_str.empty()) {
                    doCommitDirect(flush_str);
                }
                verifier_.clearPending();
                if (rust_ctx_) {
                    clak_context_reset(rust_ctx_);
                }
                uinput::UinputTool::instance().consume_sentinel_emit_time_us();
                logCsv(0, "TIMEOUT");
                logLatency(op_group_, op_start_us_, "REPLACE_TIMEOUT");
                op_start_us_ = 0;
            }
            return true;
        }
    );
}

void ClakState::finishUinputDeletion(uint64_t rtt_us) {
    if (!is_deleting_) return;
    if (safety_timer_) {
        safety_timer_.reset();
    }
    if (commit_timer_) {
        commit_timer_.reset();
    }
    if (retired_commit_timer_) {
        retired_commit_timer_.reset();
    }
    ++fallback_seq_;
    if (surr_lag_timer_) {
        surr_lag_timer_.reset();
    }
    is_surr_lag_replaying_ = false;
    if (expected_backspaces_ > current_backspace_count_) {
        in_flight_sentinel_count_ = expected_backspaces_ - current_backspace_count_;
        sentinel_grace_until_us_ = fcitx::now(CLOCK_MONOTONIC) + 500000;
    }
    is_address_bar_fix_ = false;
    is_selection_deletion_ = false;
    expected_backspaces_ = 0;
    current_backspace_count_ = 0;

    std::string app = appKey();
    std::string site = activeSite();
    bool is_draftjs = isDraftJsEditor() || config::isDraftJsSite(site) || config::isTikTokSite(site);
    bool is_terminal = ic_ && ic_->capabilityFlags().test(fcitx::CapabilityFlag::Terminal);
    pacing::PacingGroup pgroup = pacing::AdaptivePacingEngine::classify(app, site, is_address_bar_fix_, is_draftjs, is_terminal);
    pacing_engine_.recordRtt(pgroup, rtt_us);
    pacing_engine_.onTransactionSuccess(pgroup);
    auto pm = pacing_engine_.metrics(pgroup);
    uint32_t active_post_delay = pm.post_delay_ms;
    const char* env_fixed_delay = utils::clakGetEnv("CLAK_FIXED_DELAY_MS");
    if (env_fixed_delay) {
        try { active_post_delay = std::stoul(env_fixed_delay); } catch (...) {}
    }
    utils::clakLog("[PACING] group=" + std::to_string(static_cast<int>(pgroup)) +
                   " post_delay=" + std::to_string(active_post_delay) +
                   "ms rtt=" + std::to_string((rtt_us + 500) / 1000) +
                   "ms ssthresh=" + std::to_string(pm.ssthresh_post_ms) +
                   "ms clean=" + std::to_string(pm.consecutive_clean_words));

    if (op_start_us_ > 0) {
        uint64_t now_us = fcitx::now(CLOCK_MONOTONIC);
        uint64_t roundtrip_us = (now_us > op_start_us_) ? (now_us - op_start_us_) : 0;
        observeTransactionLatency(roundtrip_us);
    }
    logCsv(rtt_us, "UINPUT");
    logLatency(op_group_, op_start_us_, "REPLACE", "uinput");
    op_start_us_ = 0;
    std::string to_commit = pending_commit_string_;
    pending_commit_string_.clear();

    uint32_t compensation_ms = pacing_engine_.commitCompensation(pgroup);
    if ((apps::isKonsoleApp(app) || apps::isKonsoleApp(site)) && compensation_ms < 8) {
        compensation_ms = 8;
    }
    if (compensation_ms > 0 && engine_ && engine_->instance()) {
        utils::clakLog("compensatory wait: delaying commit by " + std::to_string(compensation_ms) + "ms");
        commit_timer_ = engine_->instance()->eventLoop().addTimeEvent(
            CLOCK_MONOTONIC,
            fcitx::now(CLOCK_MONOTONIC) + compensation_ms * 1000ULL,
            1000,
            [this, to_commit](fcitx::EventSourceTime*, uint64_t) {
                is_deleting_ = false;
                replayBufferedKeys(to_commit);
                commit_timer_.reset();
                return true;
            }
        );
    } else {
        is_deleting_ = false;
        replayBufferedKeys(to_commit);
    }
}

void ClakState::replayBufferedKeys(const std::string& initial_batch) {
    if (is_replaying_) return;
    is_replaying_ = true;
    batch_commit_ = initial_batch;

    auto pending = key_buffer_.take_keys();
    utils::clakLog("replayBufferedKeys: " + std::to_string(pending.size()) +
                   " keys (initial_batch='" + initial_batch + "')");

    for (size_t i = 0; i < pending.size(); ++i) {
        const auto& k = pending[i];
        if (is_deleting_) {
            // new deletion in flight, buffer remaining keys
            for (size_t j = i; j < pending.size(); ++j) {
                key_buffer_.push(pending[j]);
            }
            break;
        }

        if (!handleKey(k)) {
            bool has_ctrl_alt = k.states().test(fcitx::KeyState::Ctrl) ||
                                k.states().test(fcitx::KeyState::Alt) ||
                                k.states().test(fcitx::KeyState::Super);
            std::string str = fcitx::Key::keySymToUTF8(k.sym());
            if (!has_ctrl_alt && !str.empty() && k.sym() < 0xff00) {
                batch_commit_ += str;
            } else if (k.sym() == FcitxKey_BackSpace && !batch_commit_.empty()) {
                utils::popUtf8Chars(batch_commit_, 1);
            } else {
                if (!batch_commit_.empty()) {
                    utils::clakLog("replay batch commit before control key: '" + batch_commit_ + "'");
                    doCommitDirect(batch_commit_);
                    batch_commit_.clear();
                }
                utils::clakLog("replay forward raw: sym=" + std::to_string(k.sym()));
                if (ic_) {
                    ic_->forwardKey(k);
                }
            }
        } else if (surr_lag_timer_ && !is_surr_lag_replaying_) {
            if (!batch_commit_.empty()) {
                doCommitDirect(batch_commit_);
                batch_commit_.clear();
            }
            for (size_t j = i + 1; j < pending.size(); ++j) {
                key_buffer_.push(pending[j]);
            }
            break;
        }
    }

    if (!is_deleting_ && !batch_commit_.empty()) {
        utils::clakLog("replay final batch commit: '" + batch_commit_ + "'");
        doCommitDirect(batch_commit_);
        batch_commit_.clear();
    }
    is_replaying_ = false;
}

bool ClakState::handleKey(const fcitx::Key& key) {
    if (key.isModifier()) {
        return false;
    }

    uint64_t now_us = fcitx::now(CLOCK_MONOTONIC);
    last_key_interval_us_ = (last_key_time_us_ > 0 && now_us > last_key_time_us_) ? (now_us - last_key_time_us_) : 0;
    last_key_time_us_ = now_us;

    bool has_ctrl_alt = key.states().test(fcitx::KeyState::Ctrl) ||
                        key.states().test(fcitx::KeyState::Alt) ||
                        key.states().test(fcitx::KeyState::Super);

    std::string key_str = fcitx::Key::keySymToUTF8(key.sym());
    uint32_t sym = key.sym();

    std::string fallback = (ic_ && !ic_->program().empty()) ? ic_->program() : "";
    modal_handler_.updateStatus(fallback);
    if (modal_handler_.handleKey(key, [this]() { reset(); })) {
        return false;
    }

    bool is_cursor_move = key.isCursorMove() || (sym >= FcitxKey_Home && sym <= FcitxKey_End);
    bool is_reset_key = is_cursor_move || sym == FcitxKey_Escape ||
                        sym == FcitxKey_Delete || sym == FcitxKey_KP_Delete ||
                        has_ctrl_alt;

    if (is_reset_key) {
        if (!pending_commit_string_.empty()) {
            doCommitString(pending_commit_string_);
            pending_commit_string_.clear();
        }
        reset(/*force=*/false);
        return false;
    }

    const char* surr_text = nullptr;
    size_t cursor = 0;
    size_t anchor = 0;

    std::string app = appKey();
    std::string site = activeSite();
    bool is_meta = config::isMetaSite(site) || config::isMetaSite(app);
    bool is_draftjs = isDraftJsEditor();
    bool is_gecko = config::isGeckoApp(app);

    bool has_surrounding = currentProfile()->allowsSurroundingText() &&
                           ic_->capabilityFlags().test(fcitx::CapabilityFlag::SurroundingText);
    const auto& surr = ic_->surroundingText();

    if (sym == FcitxKey_BackSpace && has_surrounding && surr.isValid()) {
        if (surr.cursor() == 0 || surr.cursor() != surr.anchor()) {
            reset(/*force=*/true);
            return false;
        }
    }

    if (is_meta || currentProfile()->preventsUinput()) {
        verifier_.setCanvasEditor(false);
        verifier_.setRichTextEditor(false);
        verifier_.setDraftJsEditor(false);
    } else if (!is_gecko && has_surrounding && surr.isValid()) {
        const std::string& text = surr.text();
        if (text == "  " || text == "\xc2\xa0\xc2\xa0") {
            verifier_.setCanvasEditor(true);
        } else if (verifier_.isCanvasEditor() && text.size() > 2 && text != "  " && text != "\xc2\xa0\xc2\xa0") {
            verifier_.setCanvasEditor(false);
        }
        bool is_twitter_placeholder = (text.find("happening") != std::string::npos || text.find("diễn ra") != std::string::npos);
        if (!config::isVSCodeApp(app) && (config::isDraftJsSite(site) || is_twitter_placeholder) &&
            text.size() >= 2 && text.compare(text.size() - 2, 2, "\n\n") == 0) {
            verifier_.setDraftJsEditor(true);
            verifier_.setRichTextEditor(true);
            is_draftjs = true;
        }
    }

    bool valid_surr = has_surrounding && surr.isValid() &&
                      surr.cursor() <= surr.text().size() &&
                      !verifier_.isCanvasEditor() &&
                      !verifier_.isRichTextEditor();
    bool use_surrounding = valid_surr && currentProfile()->allowsSurroundingText();

    // detect xterm.js in vscode / electron ides (invalid or frozen surrounding cursor)
    bool is_printable = (!key_str.empty() && sym < 0xff00);
    bool is_bs_on_text = (sym == FcitxKey_BackSpace && surr.cursor() > 0);
    if (!currentProfile()->preventsUinput() && config::isVSCodeApp(app) && !has_ctrl_alt) {
        if ((!has_surrounding || !surr.isValid()) && is_printable) {
            vscode_invalid_surr_count_++;
            if (vscode_invalid_surr_count_ >= 2) {
                verifier_.setRichTextEditor(true);
                valid_surr = false;
                use_surrounding = false;
            }
        } else if (has_surrounding && surr.isValid() && (is_printable || is_bs_on_text)) {
            vscode_invalid_surr_count_ = 0;
            bool surr_matches_uinput_commit =
                !last_vscode_uinput_commit_.empty() &&
                surr.text().size() >= last_vscode_uinput_commit_.size() &&
                surr.text().compare(
                    surr.text().size() - last_vscode_uinput_commit_.size(),
                    last_vscode_uinput_commit_.size(),
                    last_vscode_uinput_commit_) == 0;
            bool in_flight_commit = (now_us - last_commit_time_us_ < 60000);
            bool is_terminal_dummy_text = (surr.text().size() <= 3 && surr.text().find('\n') == std::string::npos);

            if (surr_matches_uinput_commit) {
                // ignore helper textarea echoing our own uinput commitString (e.g. xterm.js)
                if (word_used_uinput_) {
                    verifier_.setRichTextEditor(true);
                }
                if (verifier_.isRichTextEditor()) {
                    valid_surr = false;
                    use_surrounding = false;
                }
            } else if ((is_terminal_dummy_text || verifier_.isRichTextEditor()) &&
                       !in_flight_commit &&
                       last_surr_cursor_ == surr.cursor() &&
                       last_surr_text_len_ == surr.text().size()) {
                surr_keystroke_count_++;
                if (surr_keystroke_count_ >= 2) {
                    verifier_.setRichTextEditor(true);
                    valid_surr = false;
                    use_surrounding = false;
                    utils::clakLog("handleKey: detected frozen surrounding cursor in app=" + app + " (cursor=" +
                                   std::to_string(surr.cursor()) + " len=" + std::to_string(surr.text().size()) + "), auto-switched to uinput");
                }
            } else {
                surr_keystroke_count_ = 0;
                // only restore surrounding text when cursor genuinely moved to real multi-char editor text
                if (verifier_.isRichTextEditor() && !word_used_uinput_ && surr.text().size() > 4) {
                    last_vscode_uinput_commit_.clear();
                    verifier_.setRichTextEditor(false);
                    verifier_.setDraftJsEditor(false);
                    valid_surr = true;
                    use_surrounding = currentProfile()->allowsSurroundingText();
                    utils::clakLog("handleKey: surrounding cursor resumed moving in app=" + app + ", restored surrounding text");
                }
            }
            last_surr_cursor_ = surr.cursor();
            last_surr_text_len_ = surr.text().size();
        }
    }

    bool skip_verify = is_gecko || is_meta || currentProfile()->preventsUinput();
    if (verifier_.isPending() && use_surrounding && !skip_verify) {
        std::string actualWord = utils::extractWordBeforeCursor(surr.text(), surr.cursor());
        // delay key if previous surrounding replacement is still in flight on wayland ipc
        if (actualWord == verifier_.preWord() &&
            (now_us - last_commit_time_us_ < 20000) &&
            !is_surr_lag_replaying_ &&
            engine_ && engine_->instance() && std::string(ic_->frontend()) != "mock") {
            key_buffer_.push(key);
            uint64_t elapsed_us = now_us - last_commit_time_us_;
            uint64_t delay_us = (elapsed_us < 16000) ? (16000 - elapsed_us) : 8000;
            utils::clakLog("handleKey: surrounding replacement lagging in app=" + app +
                           " (" + std::to_string(elapsed_us / 1000) + "ms since commit), buffering key for " +
                           std::to_string(delay_us / 1000) + "ms");
            surr_lag_timer_ = engine_->instance()->eventLoop().addTimeEvent(
                CLOCK_MONOTONIC,
                now_us + delay_us,
                1000,
                [this](fcitx::EventSourceTime*, uint64_t) {
                    surr_lag_timer_.reset();
                    is_surr_lag_replaying_ = true;
                    replayBufferedKeys();
                    is_surr_lag_replaying_ = false;
                    return true;
                }
            );
            return true;
        }
        bool was_draft = verifier_.isDraftJsEditorDirect();
        verifier_.verify(surr);
        if (!was_draft && verifier_.isDraftJsEditorDirect()) {
            pacing_engine_.onIncident(pacing::PacingGroup::WebDraftJs,
                                      pacing::IncidentType::SurroundingMismatch,
                                      fcitx::now(CLOCK_MONOTONIC),
                                      last_key_interval_us_);
        }
        is_draftjs = isDraftJsEditor();
        use_surrounding = currentProfile()->allowsSurroundingText() && valid_surr && !verifier_.isRichTextEditor() && !is_draftjs;
    } else if (verifier_.isPending()) {
        verifier_.clearPending();
    }

    if (is_gecko && verifier_.isRichTextEditor()) {
        verifier_.setRichTextEditor(false);
    }

    if (valid_surr) {
        surr_text = surr.text().c_str();
        cursor = surr.cursor();
        anchor = surr.anchor();
    } else {
        surr_text = nullptr;
        cursor = 0;
        anchor = 0;
    }

    ClakAction action = clak_process_key(rust_ctx_,
                                         sym,
                                         key_str.c_str(),
                                         has_ctrl_alt,
                                         surr_text,
                                         cursor,
                                         anchor);

    std::string action_name = (action.action_type == CLAK_ACTION_FORWARD ? "FORWARD" :
                              (action.action_type == CLAK_ACTION_COMMIT ? "COMMIT" : "REPLACE"));
    utils::clakLog("handleKey: sym=" + std::to_string(sym) + " ('" + key_str + "') app=" + app +
                   " site='" + site + "' -> " + action_name + " del=" + std::to_string(action.delete_count) +
                   " commit='" + std::string(action.commit_str ? action.commit_str : "") + "'");

    if (!pending_commit_string_.empty()) {
        doCommitString(pending_commit_string_);
        pending_commit_string_.clear();
    }

    switch (action.action_type) {
        case CLAK_ACTION_FORWARD:
            last_autofix_original_.clear();
            last_autofix_time_us_ = 0;
            if (valid_surr) {
                last_text_len_ = surr.cursor() + 1;
            } else {
                last_text_len_ = 1;
            }
            if (currentProfile()->directCommitPrintableKey() && !has_ctrl_alt && !key_str.empty() && sym < 0xff00) {
                doCommitString(key_str);
                logLatency(currentProfile()->name(), op_start_us_, "COMMIT");
                op_start_us_ = 0;
                return true;
            }
            uncommitted_forward_count_++;
            logLatency(classifyGroup(app, site, false, false), op_start_us_, "FORWARD", "forwardKey");
            op_start_us_ = 0;
            return false;

        case CLAK_ACTION_COMMIT:
            last_autofix_original_.clear();
            last_autofix_time_us_ = 0;
            if (action.commit_str && action.commit_str[0] != '\0') {
                doCommitString(action.commit_str);
            }
            logLatency(classifyGroup(app, site, false, false), op_start_us_, "COMMIT");
            op_start_us_ = 0;
            last_text_len_ = 0;
            return true;

        case CLAK_ACTION_AUTOCORRECT: {
            if (action.delete_str && action.commit_str) {
                last_autofix_original_ = action.delete_str;
                last_autofix_converted_chars_ = utils::utf8CharCount(action.commit_str);
                last_autofix_time_us_ = fcitx::now(CLOCK_MONOTONIC);
                utils::clakLog("handleKey: autocorrect applied: '" + std::string(action.delete_str) +
                               "' -> '" + std::string(action.commit_str) + "' (tap Shift to undo)");
            }
            [[fallthrough]];
        }
        case CLAK_ACTION_ADDRESS_BAR_FIX:
        case CLAK_ACTION_REPLACE:
        case CLAK_ACTION_REPLACE_SURROUNDING: {
            size_t real_bs = action.delete_count;

            if (is_replaying_ && !utils::clakGetEnv("CLAK_DISABLE_BATCH_ABSORB")) {
                size_t avail = utils::utf8CharCount(batch_commit_);
                if (avail >= real_bs) {
                    utils::popUtf8Chars(batch_commit_, real_bs);
                    if (action.commit_str && action.commit_str[0] != '\0') {
                        batch_commit_ += action.commit_str;
                    }
                    collision_stats_.batch_absorbed_ops++;
                    utils::clakLog("handleKey: absorbed " + std::to_string(real_bs) +
                                   " deletions in batch_commit (batch='" + batch_commit_ + "')");
                    logLatency(classifyGroup(app, site, false, false), op_start_us_, "BATCH_ABSORB");
                    op_start_us_ = 0;
                    last_text_len_ = 0;
                    return true;
                } else if (avail > 0) {
                    utils::popUtf8Chars(batch_commit_, avail);
                    real_bs -= avail;
                    collision_stats_.batch_absorbed_ops++;
                    utils::clakLog("handleKey: partially absorbed " + std::to_string(avail) +
                                   " deletions in batch_commit, remaining " + std::to_string(real_bs) + " to send to OS");
                }
            }

            int app_mode = 0;
            if (engine_ && engine_->config()) {
                app_mode = clak_config_get_app_mode(engine_->config(), app.c_str());
            }
            bool is_address_bar = (currentProfile() && currentProfile()->name() == "address-bar") ||
                                  (ic_ && ic_->capabilityFlags().test(fcitx::CapabilityFlag::Url)) ||
                                  (isBrowser() && (site == "New Tab" || site == "about:blank" || site == "address-bar" ||
                                                   (app_mode == 0 && !apps::isMetaSite(site) && !apps::isMetaSite(app) &&
                                                    looksLikeUrlAutofill(surr))));
            bool has_autofill_selection = (action.action_type == CLAK_ACTION_ADDRESS_BAR_FIX) ||
                                          (isAutofillCertain(surr));
            if (real_bs > 0 && surr.isValid() && surr.cursor() < real_bs) {
                if (!config::isVSCodeApp(app) && !currentProfile()->preventsUinput()) {
                    use_surrounding = false;
                    surr_cursor_underflow_count_++;
                    if (surr_cursor_underflow_count_ >= 3) {
                        verifier_.setRichTextEditor(true);
                        if (isBrowser()) {
                            verifier_.setDraftJsEditor(true);
                        }
                    }
                    is_draftjs = isDraftJsEditor();
                }
            } else if (real_bs > 0 && surr.isValid() && surr.cursor() >= real_bs) {
                surr_cursor_underflow_count_ = 0;
            }

            if (currentProfile()->erasesWithCommittedDel() && real_bs > 0) {
                std::string del_str(real_bs, '\x7f');
                if (action.commit_str && action.commit_str[0] != '\0') {
                    del_str += action.commit_str;
                }
                doCommitString(del_str);
                utils::clakLog("vte del commit: count=" + std::to_string(real_bs) +
                               " commit='" + (action.commit_str ? action.commit_str : "") + "' app=" + app);
                logLatency(currentProfile()->name(), op_start_us_, "COMMIT_DEL");
                op_start_us_ = 0;
                last_text_len_ = 0;
                return true;
            }

            bool use_uinput = is_address_bar || has_autofill_selection || shouldUseUinput(use_surrounding, action.action_type, surr);

            if (use_uinput && real_bs > 0) {
                size_t autofill_extra = has_autofill_selection ? 1 : 0;
                size_t bs_to_send = real_bs + autofill_extra + 1;
                is_address_bar_fix_ = is_address_bar;
                uint64_t now_us = fcitx::now(CLOCK_MONOTONIC);
                bool recent_selection = (last_selection_time_us_ > 0) && (now_us - last_selection_time_us_ < 1500000);
                bool has_selection = has_surrounding && surr.isValid() && (surr.cursor() != surr.anchor());
                is_selection_deletion_ = recent_selection || has_selection;
                op_group_ = classifyGroup(app, site, is_address_bar, true);

                apps::UinputPacing pacing = currentProfile()->uinputPacing();
                const ClakConfig* cfg = engine_ ? engine_->config() : nullptr;
                bool adaptive_enabled = cfg ? clak_config_get_adaptive_pacing(cfg) : true;

                const char* env_fixed_delay = utils::clakGetEnv("CLAK_FIXED_DELAY_MS");
                const char* env_fixed_gap = utils::clakGetEnv("CLAK_FIXED_GAP_MS");
                bool fixed_override = (env_fixed_delay != nullptr);
                if (fixed_override) {
                    adaptive_enabled = false;
                }

                pacing_engine_.setEnabled(adaptive_enabled);

                bool is_terminal = ic_ && ic_->capabilityFlags().test(fcitx::CapabilityFlag::Terminal);
                pacing::PacingGroup pgroup = pacing::AdaptivePacingEngine::classify(app, site, is_address_bar, is_draftjs, is_terminal);
                if (cfg) {
                    pacing_engine_.configureGroup(pacing::PacingGroup::TerminalUinput,
                        clak_config_get_terminal_delay_ms(cfg), clak_config_get_terminal_gap_ms(cfg), 30, 10);
                    uint32_t draft_delay = clak_config_get_draftjs_delay_ms(cfg);
                    uint32_t draft_gap = clak_config_get_draftjs_gap_ms(cfg);
                    uint32_t draft_ceiling = std::max(60u, draft_delay + 20u);
                    uint32_t draft_gap_ceiling = std::max(20u, draft_gap + 10u);
                    pacing_engine_.configureGroup(pacing::PacingGroup::WebDraftJs,
                        draft_delay, draft_gap, draft_ceiling, draft_gap_ceiling);
                    pacing_engine_.configureGroup(pacing::PacingGroup::HeavyApp,
                        clak_config_get_heavy_app_delay_ms(cfg), clak_config_get_heavy_app_gap_ms(cfg), 35, 12);
                    pacing_engine_.configureGroup(pacing::PacingGroup::Default,
                        8, 2, 30, 10);
                }

                if (fixed_override) {
                    uint32_t f_delay = 10;
                    try { f_delay = std::stoul(env_fixed_delay); } catch (...) {}
                    auto base_pacing = pacing_engine_.uinputPacing(pgroup);
                    uint32_t f_gap = base_pacing.gap_ms;
                    if (env_fixed_gap) {
                        try { f_gap = std::stoul(env_fixed_gap); } catch (...) {}
                    }
                    pacing.post_delay_ms = f_delay;
                    pacing.gap_ms = f_gap;
                    pacing.hold_us = base_pacing.hold_us;
                    pacing_engine_.configureGroup(pgroup, f_delay, f_gap, f_delay, f_gap);
                } else if (is_address_bar || has_autofill_selection) {
                    pacing = {config::kAddressBarPostDelayMs, 2};
                } else if (adaptive_enabled) {
                    pacing = pacing_engine_.uinputPacing(pgroup);
                } else if (modal_handler_.isModalEditor() || config::isTerminalApp(app)) {
                    pacing = {cfg ? clak_config_get_terminal_delay_ms(cfg) : config::kTerminalPostDelayMs,
                              cfg ? clak_config_get_terminal_gap_ms(cfg) : config::kTerminalGapMs};
                } else if (is_draftjs || isDraftJsEditor() || config::isTikTokSite(site)) {
                    pacing = {cfg ? clak_config_get_draftjs_delay_ms(cfg) : config::kDraftJsPostDelayMs,
                              cfg ? clak_config_get_draftjs_gap_ms(cfg) : config::kDraftJsGapMs};
                } else if (apps::isWpsOfficeApp(app) || apps::isSteamApp(app) || apps::isJetBrainsApp(app)) {
                    pacing = {cfg ? clak_config_get_heavy_app_delay_ms(cfg) : 15,
                              cfg ? clak_config_get_heavy_app_gap_ms(cfg) : 4};
                }

                if (!fixed_override) {
                    if (surr.isValid() && ime::SurroundingVerifier::looksLikeMessengerComposer(surr)) {
                        uint32_t settle_delay = ime::SurroundingVerifier::isFirstWordOfMessage(surr) ? 60u : 40u;
                        pacing.post_delay_ms = std::max(pacing.post_delay_ms, settle_delay);
                    } else if (de::isKdeSession()) {
                        pacing.post_delay_ms = std::max(pacing.post_delay_ms, 12u);
                    }
                }
                uint32_t pre_delay = pacing_engine_.calculatePreDelay(pgroup, last_commit_time_us_, now_us);

                collision_stats_.total_replaces++;
                auto pmetrics = pacing_engine_.metrics(pgroup);
                uint64_t elapsed_us = (last_commit_time_us_ > 0 && now_us >= last_commit_time_us_) ? (now_us - last_commit_time_us_) : UINT64_MAX;
                bool has_rtt = (pmetrics.smoothed_rtt_ms > 0);
                if (!has_rtt) {
                    collision_stats_.missing_rtt_count++;
                }
                uint64_t effective_rtt_window_us = has_rtt ? (static_cast<uint64_t>(pmetrics.smoothed_rtt_ms) * 2000UL) : 50000UL;
                uint64_t dynamic_window_us = std::max<uint64_t>(static_cast<uint64_t>(pmetrics.post_commit_window_ms) * 1000UL, effective_rtt_window_us);
                if (real_bs > uncommitted_forward_count_ && elapsed_us < dynamic_window_us) {
                    collision_stats_.total_collisions++;
                }
                if (real_bs <= uncommitted_forward_count_) {
                    uncommitted_forward_count_ -= real_bs;
                } else {
                    uncommitted_forward_count_ = 0;
                }
                if (action.commit_str && action.commit_str[0] != '\0') {
                    uncommitted_forward_count_ += utils::utf8CharCount(action.commit_str);
                }

                bool is_adaptive_env = (utils::clakGetEnv("CLAK_ADAPTIVE") &&
                                        std::string(utils::clakGetEnv("CLAK_ADAPTIVE")) == "1" &&
                                        !utils::clakGetEnv("CLAK_SIMPLE_BACKOFF"));
                std::string pacing_mode = fixed_override ? "fixed" :
                                          (!is_adaptive_env ? "fixed_13_backoff" :
                                          (adaptive_enabled ? "adaptive" : "profile"));
                utils::clakLog("uinput waiting for sentinel: expected=" + std::to_string(bs_to_send) +
                               " (real=" + std::to_string(real_bs) + (has_autofill_selection ? " + 1 autofill" : "") +
                               " + 1 sentinel) mode=" + pacing_mode + " post_delay=" + std::to_string(pacing.post_delay_ms) +
                               "ms pre_delay=" + std::to_string(pre_delay) + "ms gap=" +
                               std::to_string(pacing.gap_ms) + "ms hold=" + std::to_string(pacing.hold_us) +
                               "us commit='" + (action.commit_str ? action.commit_str : "") + "' app=" + app + " site='" + site + "'");

                if (uinput::UinputTool::instance().send_backspace(bs_to_send, pacing.post_delay_ms, pre_delay, pacing.gap_ms, pacing.hold_us)) {
                    word_used_uinput_ = true;
                    if (config::isVSCodeApp(app)) {
                        last_vscode_uinput_commit_ = (action.commit_str ? action.commit_str : "");
                        if (!valid_surr) {
                            verifier_.setRichTextEditor(true);
                        }
                    }
                    repeat_handler_.cancel();
                    if (utils::clakGetEnv("CLAK_DISABLE_SENTINEL_GATE")) {
                        in_flight_sentinel_count_ = 1;
                        sentinel_grace_until_us_ = fcitx::now(CLOCK_MONOTONIC) + 500000;
                        if (action.commit_str && action.commit_str[0] != '\0') {
                            doCommitString(action.commit_str);
                        }
                        last_text_len_ = 0;
                        return true;
                    }
                    is_deleting_ = true;
                    expected_backspaces_ = bs_to_send;
                    current_backspace_count_ = 0;
                    in_flight_sentinel_count_ = 0;
                    sentinel_grace_until_us_ = 0;
                    pending_commit_string_ = (action.commit_str ? action.commit_str : "");
                    arm_safety_timer();
                    last_text_len_ = 0;
                    return true;
                } else {
                    utils::clakLog("ERROR: uinput send_backspace failed! app=" + app + " site='" + site + "'");
                }
            }

            bool deferred_fallback_commit = false;
            if (real_bs > 0) {
                if (use_surrounding) {
                    if (surr.cursor() < real_bs) {
                        utils::clakLog("WARNING: surr.cursor()=" + std::to_string(surr.cursor()) +
                                       " < real_bs=" + std::to_string(real_bs) +
                                       ", skipping deleteSurroundingText to prevent compositor crash!");
                        std::string to_commit = action.commit_str ? action.commit_str : "";
                        deferred_fallback_commit = scheduleForwardBackspaceFallback(real_bs, to_commit, app);
                    } else {
                        std::string wordBefore = utils::extractWordBeforeCursor(surr.text(), surr.cursor());
                        verifier_.setExpectation(wordBefore, real_bs, action.commit_str ? action.commit_str : "");
                        utils::clakLog("surrounding delete: -" + std::to_string(real_bs) + " commit='" +
                                       (action.commit_str ? action.commit_str : "") + "' app=" + app + " site='" + site + "'");
                        ic_->deleteSurroundingText(-static_cast<int>(real_bs),
                                                   static_cast<unsigned int>(real_bs));
                    }
                } else if (currentProfile()->hasCustomDeletion()) {
                    utils::clakLog("custom deletion: count=" + std::to_string(real_bs) +
                                   " commit='" + (action.commit_str ? action.commit_str : "") + "' app=" + app);
                    if (engine_ && engine_->instance()) {
                        repeat_handler_.cancel();
                        is_deleting_ = true;
                        expected_backspaces_ = real_bs;
                        current_backspace_count_ = 0;
                        pending_commit_string_ = (action.commit_str ? action.commit_str : "");
                        currentProfile()->startCustomDeletion(
                            engine_->instance()->eventLoop(),
                            ic_,
                            real_bs,
                            pending_commit_string_,
                            [this](const std::string& str) { doCommitString(str); },
                            [this]() {
                                pending_commit_string_.clear();
                                is_deleting_ = false;
                                logCsv(0, "SURROUNDING");
                                logLatency(currentProfile()->name(), op_start_us_, "REPLACE", "surrounding");
                                op_start_us_ = 0;
                                replayBufferedKeys();
                            }
                        );
                        last_text_len_ = 0;
                        return true;
                    } else {
                        forwardBackspacePressRelease(real_bs);
                        if (action.commit_str && action.commit_str[0] != '\0') {
                            doCommitString(action.commit_str);
                        }
                    }
                    logCsv(0, "SURROUNDING");
                    logLatency(currentProfile()->name(), op_start_us_, "REPLACE", "surrounding");
                    op_start_us_ = 0;
                    last_text_len_ = 0;
                    return true;
                } else {
                    utils::clakLog("forward backspace: count=" + std::to_string(real_bs) +
                                   " commit='" + (action.commit_str ? action.commit_str : "") + "' app=" + app);
                    std::string to_commit = action.commit_str ? action.commit_str : "";
                    deferred_fallback_commit = scheduleForwardBackspaceFallback(real_bs, to_commit, app);
                }
            }
            if (deferred_fallback_commit) {
                logCsv(0, "FORWARD_FALLBACK");
                logLatency(classifyGroup(app, site, false, false), op_start_us_, "REPLACE", "forwardKey");
                op_start_us_ = 0;
                last_text_len_ = 0;
                return true;
            }
            if ((!currentProfile()->hasCustomDeletion() || real_bs == 0) && action.commit_str && action.commit_str[0] != '\0') {
                doCommitString(action.commit_str);
            }
            logCsv(0, "COMMIT");
            logLatency(classifyGroup(app, site, false, false), op_start_us_, "REPLACE");
            op_start_us_ = 0;
            last_text_len_ = 0;
            return true;
        }

        default:
            return false;
    }
}

void ClakState::keyEvent(fcitx::KeyEvent& keyEvent) {
    std::string app = appKey();
    const auto& key = keyEvent.key();
    std::string key_str = fcitx::Key::keySymToUTF8(key.sym());

    bool is_ctrl = key.states().test(fcitx::KeyState::Ctrl);
    bool is_shift = key.states().test(fcitx::KeyState::Shift);
    bool is_alt = key.states().test(fcitx::KeyState::Alt);
    bool is_super = key.states().test(fcitx::KeyState::Super);
    bool is_ctrl_sym = (key.sym() == FcitxKey_Control_L || key.sym() == FcitxKey_Control_R);
    bool is_shift_sym = (key.sym() == FcitxKey_Shift_L || key.sym() == FcitxKey_Shift_R);
    bool is_alt_sym = (key.sym() == FcitxKey_Alt_L || key.sym() == FcitxKey_Alt_R);
    char* sc_c = engine_->config() ? clak_config_get_toggle_shortcut(engine_->config()) : nullptr;
    std::string raw_shortcut = sc_c ? sc_c : "ctrl_shift";
    if (sc_c) clak_free_string(sc_c);

    std::string shortcut = "";
    for (char c : raw_shortcut) {
        if (c == '+' || c == ' ' || c == '-') {
            if (!shortcut.empty() && shortcut.back() != '_') shortcut.push_back('_');
        } else {
            shortcut.push_back(std::tolower(static_cast<unsigned char>(c)));
        }
    }
    if (shortcut.empty()) shortcut = "ctrl_shift";

    if (keyEvent.isRelease()) {
        if (repeat_handler_.heldKey().sym() != 0) {
            repeat_handler_.cancel();
        }
        if (is_shift_sym) {
            uint64_t now_us = fcitx::now(CLOCK_MONOTONIC);
            uint64_t hold_time_us = (shift_press_time_us_ > 0) ? (now_us - shift_press_time_us_) : 0;
            bool single_tap = (shift_press_time_us_ > 0) && (hold_time_us < 350000) && !shift_other_key_pressed_ && !shift_mouse_clicked_;
            utils::clakLog("shift release: hold_ms=" + std::to_string(hold_time_us / 1000) +
                           " single_tap=" + std::to_string(single_tap) +
                           " other_key=" + std::to_string(shift_other_key_pressed_) +
                           " mouse=" + std::to_string(shift_mouse_clicked_) +
                           " armed=" + std::to_string(ctrl_shift_armed_ || alt_shift_armed_));
            shift_press_time_us_ = 0;
            shift_other_key_pressed_ = false;
            shift_mouse_clicked_ = false;

            if (single_tap && !ctrl_shift_armed_ && !alt_shift_armed_) {
                if (triggerShiftAutoFix()) {
                    return;
                }
            }
        }
        if (shortcut == "ctrl_shift") {
            if (ctrl_shift_armed_ && (is_shift_sym || is_ctrl_sym)) {
                engine_->toggleAppEnabled(app);
                reset(/*force=*/true);
                ic_->updateUserInterface(fcitx::UserInterfaceComponent::StatusArea, true);
                if (engine_->instance() && std::string(ic_->frontend()) != "mock") {
                    showInputMethodInfo(engine_->instance(), ic_, engine_->isAppEnabled(app) ? "VI" : "EN");
                }
                ctrl_shift_armed_ = false;
                ctrl_pressed_first_ = false;
            } else if (is_ctrl_sym) {
                ctrl_pressed_first_ = false;
            }
        } else if (shortcut == "alt_shift") {
            if (alt_shift_armed_ && (is_shift_sym || is_alt_sym)) {
                engine_->toggleAppEnabled(app);
                reset(/*force=*/true);
                ic_->updateUserInterface(fcitx::UserInterfaceComponent::StatusArea, true);
                if (engine_->instance() && std::string(ic_->frontend()) != "mock") {
                    showInputMethodInfo(engine_->instance(), ic_, engine_->isAppEnabled(app) ? "VI" : "EN");
                }
                alt_shift_armed_ = false;
                alt_pressed_first_ = false;
            } else if (is_alt_sym) {
                alt_pressed_first_ = false;
            }
        }
        return;
    }

    syncConfig();

    if (is_shift_sym) {
        if (shift_press_time_us_ == 0) {
            shift_press_time_us_ = fcitx::now(CLOCK_MONOTONIC);
            shift_other_key_pressed_ = false;
            shift_mouse_clicked_ = false;
        }
    } else {
        shift_other_key_pressed_ = true;
    }

    utils::clakLog("keyEvent: sym=" + std::to_string(key.sym()) + " ('" + key_str + "') app='" + app +
                   "' term_cap=" + std::to_string(ic_->capabilityFlags().test(fcitx::CapabilityFlag::Terminal)) +
                   " enabled=" + std::to_string(engine_->isAppEnabled(app)) +
                   " ctrl=" + std::to_string(is_ctrl) + " shift=" + std::to_string(is_shift) +
                   " alt=" + std::to_string(is_alt) + " super=" + std::to_string(is_super) +
                   " shortcut='" + shortcut + "'");

    bool match_toggle = false;
    if (shortcut == "alt_z" && is_alt && !is_ctrl && (key.sym() == FcitxKey_z || key.sym() == FcitxKey_Z)) {
        match_toggle = true;
    } else if (shortcut == "super_space" && is_super && (key.sym() == FcitxKey_space || key.sym() == FcitxKey_KP_Space)) {
        match_toggle = true;
    } else if (shortcut == "ctrl_space" && is_ctrl && !is_shift && !is_alt && (key.sym() == FcitxKey_space || key.sym() == FcitxKey_KP_Space)) {
        match_toggle = true;
    } else if (shortcut == "shift_space" && is_shift && !is_ctrl && !is_alt && (key.sym() == FcitxKey_space || key.sym() == FcitxKey_KP_Space)) {
        match_toggle = true;
    } else if (shortcut == "alt_space" && is_alt && !is_ctrl && (key.sym() == FcitxKey_space || key.sym() == FcitxKey_KP_Space)) {
        match_toggle = true;
    }

    if (match_toggle) {
        engine_->toggleAppEnabled(app);
        reset(/*force=*/true);
        ic_->updateUserInterface(fcitx::UserInterfaceComponent::StatusArea, true);
        if (engine_->instance() && std::string(ic_->frontend()) != "mock") {
            showInputMethodInfo(engine_->instance(), ic_, engine_->isAppEnabled(app) ? "VI" : "EN");
        }
        keyEvent.filterAndAccept();
        return;
    }

    if (shortcut == "ctrl_shift") {
        if (ctrl_shift_armed_) {
            if (!is_ctrl_sym && !is_shift_sym) {
                ctrl_shift_armed_ = false;
            }
        } else if (is_ctrl_sym && !is_shift && !is_alt && !is_super) {
            ctrl_pressed_first_ = true;
        } else if (is_shift_sym && ctrl_pressed_first_ && is_ctrl) {
            ctrl_shift_armed_ = true;
            ctrl_pressed_first_ = false;
        } else {
            ctrl_pressed_first_ = false;
        }
    } else if (shortcut == "alt_shift") {
        if (alt_shift_armed_) {
            if (!is_alt_sym && !is_shift_sym) {
                alt_shift_armed_ = false;
            }
        } else if (is_alt_sym && !is_shift && !is_ctrl && !is_super) {
            alt_pressed_first_ = true;
        } else if (is_shift_sym && alt_pressed_first_ && is_alt) {
            alt_shift_armed_ = true;
            alt_pressed_first_ = false;
        } else {
            alt_pressed_first_ = false;
        }
    }

    bool has_ctrl_alt = is_ctrl || is_alt || is_super;
    uint32_t sym = key.sym();
    if (sym == 0 || sym == FcitxKey_None) {
        return;
    }

    if (!engine_->isAppEnabled(app)) {
        reset(/*force=*/true);
        return;
    }

    auto* profile = currentProfile();
    if (!key.isModifier() && profile && profile->filterIncomingKey(sym, fcitx::now(CLOCK_MONOTONIC))) {
        keyEvent.filterAndAccept();
        return;
    }

    std::string fallback = (ic_ && ic_->capabilityFlags().test(fcitx::CapabilityFlag::Terminal))
                                ? "terminal"
                                : ((ic_ && !ic_->program().empty() && !de::isDesktopShell(ic_->program())) ? ic_->program() : "");
    modal_handler_.updateStatus(fallback);
    if (modal_handler_.handleKey(key, [this]() { reset(); })) {
        return;
    }

    bool is_cursor_move = key.isCursorMove() || (sym >= FcitxKey_Home && sym <= FcitxKey_End);
    bool is_special_nav = sym == FcitxKey_Escape ||
                          sym == FcitxKey_Delete || sym == FcitxKey_KP_Delete ||
                          sym == FcitxKey_Tab || sym == FcitxKey_KP_Tab || sym == FcitxKey_ISO_Left_Tab;

    if ((is_ctrl && !is_alt && (sym == FcitxKey_a || sym == FcitxKey_A)) ||
        (is_shift && is_cursor_move)) {
        last_selection_time_us_ = fcitx::now(CLOCK_MONOTONIC);
    }

    // ctrl+z undoes recent autocorrect or autofix
    bool is_ctrl_z = is_ctrl && !is_alt && !is_super && !key.states().test(fcitx::KeyState::Shift) &&
                     (sym == FcitxKey_z || sym == FcitxKey_Z);
    if (is_ctrl_z && !last_autofix_original_.empty() && last_autofix_time_us_ > 0 &&
        (fcitx::now(CLOCK_MONOTONIC) - last_autofix_time_us_ < 5000000)) {
        size_t converted_chars = last_autofix_converted_chars_;
        std::string restore_text = last_autofix_original_;
        last_autofix_original_.clear();
        last_autofix_converted_chars_ = 0;
        last_autofix_time_us_ = 0;

        repeat_handler_.cancel();
        reset(/*force=*/true);

        bool has_surrounding = ic_->capabilityFlags().test(fcitx::CapabilityFlag::SurroundingText);
        const auto& surr = ic_->surroundingText();
        bool valid_surr = has_surrounding && surr.isValid() &&
                          surr.cursor() <= surr.text().size() &&
                          !verifier_.isCanvasEditor() &&
                          !verifier_.isRichTextEditor();
        bool use_surrounding = valid_surr && currentProfile()->allowsSurroundingText();

        if (converted_chars > 0) {
            if (currentProfile()->erasesWithCommittedDel()) {
                std::string del_str(converted_chars, '\x7f');
                del_str += restore_text;
                doCommitString(del_str);
            } else if (use_surrounding && surr.cursor() >= converted_chars) {
                ic_->deleteSurroundingText(-static_cast<int>(converted_chars),
                                           static_cast<unsigned int>(converted_chars));
                doCommitString(restore_text);
            } else {
                for (size_t i = 0; i < converted_chars; ++i) {
                    ic_->forwardKey(fcitx::Key(FcitxKey_BackSpace));
                }
                doCommitString(restore_text);
            }
            utils::clakLog("keyEvent: Ctrl+Z undo autocorrect restored '" + restore_text + "'");
        }

        keyEvent.filterAndAccept();
        return;
    }

    bool is_clean_backspace = (sym == FcitxKey_BackSpace) && !has_ctrl_alt && !is_shift;
    if (!is_deleting_ && is_clean_backspace && in_flight_sentinel_count_ > 0 &&
        fcitx::now(CLOCK_MONOTONIC) <= sentinel_grace_until_us_) {
        in_flight_sentinel_count_--;
        keyEvent.filterAndAccept();
        return;
    }
    if (is_deleting_ && expected_backspaces_ > 0 && is_clean_backspace) {
        current_backspace_count_++;
        if (current_backspace_count_ < expected_backspaces_) {
            return;
        }
        keyEvent.filterAndAccept();
        uint64_t t0 = uinput::UinputTool::instance().consume_sentinel_emit_time_us();
        uint64_t rtt_us = 0;
        if (t0 > 0) {
            uint64_t t1 = uinput::monotonic_now_us();
            if (t1 >= t0) {
                rtt_us = t1 - t0;
            }
        }
        finishUinputDeletion(rtt_us);
        return;
    }

    if (has_ctrl_alt || is_cursor_move || is_special_nav) {
        repeat_handler_.cancel();
        if (profile) {
            profile->cancelCustomDeletion();
        }
        if (is_deleting_) {
            if (safety_timer_) {
                safety_timer_.reset();
            }
            if (commit_timer_) {
                commit_timer_.reset();
            }
            if (expected_backspaces_ > current_backspace_count_) {
                in_flight_sentinel_count_ = expected_backspaces_ - current_backspace_count_;
                sentinel_grace_until_us_ = fcitx::now(CLOCK_MONOTONIC) + 500000;
            }
            is_deleting_ = false;
            is_address_bar_fix_ = false;
            is_selection_deletion_ = false;
            expected_backspaces_ = 0;
            current_backspace_count_ = 0;
            if (!pending_commit_string_.empty()) {
                doCommitString(pending_commit_string_);
                pending_commit_string_.clear();
            }
            key_buffer_.clear();
        }
        reset(/*force=*/true);

        if (sym == FcitxKey_BackSpace) {
            uint64_t now_us = fcitx::now(CLOCK_MONOTONIC);
            bool has_surrounding = ic_->capabilityFlags().test(fcitx::CapabilityFlag::SurroundingText);
            const auto& surr = ic_->surroundingText();
            bool has_selection = has_surrounding && surr.isValid() && (surr.cursor() != surr.anchor());
            bool recent_selection = (last_selection_time_us_ > 0) && (now_us - last_selection_time_us_ < 1500000);

            if (recent_selection || has_selection) {
                last_selection_time_us_ = 0;
                utils::clakLog("rapid selection backspace: forward clean BackSpace without modifiers");
                ic_->forwardKey(fcitx::Key(FcitxKey_BackSpace));
                keyEvent.filterAndAccept();
                return;
            }
        }

        if (is_cursor_move) {
            return;
        }

        bool is_delete = (sym == FcitxKey_Delete || sym == FcitxKey_KP_Delete);
        if (is_delete) {
            uint64_t now_us = fcitx::now(CLOCK_MONOTONIC);
            bool has_surrounding = ic_->capabilityFlags().test(fcitx::CapabilityFlag::SurroundingText);
            const auto& surr = ic_->surroundingText();
            bool has_selection = has_surrounding && surr.isValid() && (surr.cursor() != surr.anchor());
            bool recent_selection = (last_selection_time_us_ > 0) && (now_us - last_selection_time_us_ < 1500000);

            if (recent_selection || has_selection) {
                last_selection_time_us_ = 0;
                utils::clakLog("rapid selection delete: forward clean Delete without modifiers");
                ic_->forwardKey(fcitx::Key(static_cast<fcitx::KeySym>(sym)));
                keyEvent.filterAndAccept();
                return;
            }
        }
        return;
    }

    if (key.isModifier()) {
        repeat_handler_.cancel();
        return;
    }

    if (is_deleting_ || (surr_lag_timer_ && !is_surr_lag_replaying_)) {
        repeat_handler_.cancel();
        collision_stats_.buffered_key_interruptions++;
        if (!key_buffer_.push(key)) {
            // finalize pending deferred fallback transaction before forwarding keys so replacement is not lost
            if (!pending_commit_string_.empty()) {
                doCommitString(pending_commit_string_);
                pending_commit_string_.clear();
            }
            if (commit_timer_) {
                commit_timer_.reset();
            }
            if (retired_commit_timer_) {
                retired_commit_timer_.reset();
            }
            ++fallback_seq_;

            // flush queued keys first to preserve strict order on buffer overflow
            auto pending = key_buffer_.take_keys();
            if (ic_) {
                for (const auto& k : pending) {
                    ic_->forwardKey(k);
                }
                ic_->forwardKey(key);
            }
            if (expected_backspaces_ > current_backspace_count_) {
                in_flight_sentinel_count_ = expected_backspaces_ - current_backspace_count_;
                sentinel_grace_until_us_ = fcitx::now(CLOCK_MONOTONIC) + 500000;
            }
            is_deleting_ = false;
            is_address_bar_fix_ = false;
            is_selection_deletion_ = false;
            expected_backspaces_ = 0;
            if (surr_lag_timer_) {
                surr_lag_timer_.reset();
            }
            if (rust_ctx_) {
                clak_context_reset(rust_ctx_);
            }
        }
        keyEvent.filterAndAccept();
        return;
    }

    if (sym == FcitxKey_BackSpace) {
        last_selection_time_us_ = 0;
    }

    repeat_handler_.cancel();

    if (sym == FcitxKey_space || sym == FcitxKey_KP_Space) {
        address_bar_has_space_ = true;
    } else if (sym == FcitxKey_Return || sym == FcitxKey_KP_Enter || sym == FcitxKey_Escape) {
        address_bar_has_space_ = false;
    }

    if (!is_deleting_) {
        op_start_us_ = fcitx::now(CLOCK_MONOTONIC);
    }

    if (handleKey(key)) {
        keyEvent.filterAndAccept();
        if (sym >= 0x20 && sym < 0xff00 && !has_ctrl_alt && sym != FcitxKey_BackSpace &&
            ((!ic_ || std::string(ic_->frontend()) != "mock") || repeat_handler_.enableRepeatForMock())) {
            repeat_handler_.arm(
                engine_->instance()->eventLoop(),
                key,
                [this](const fcitx::Key& k) {
                    if (is_deleting_) {
                        repeat_handler_.defer(10000);
                        return;
                    }
                    if (!handleKey(k)) {
                        std::string str = fcitx::Key::keySymToUTF8(k.sym());
                        if (!str.empty()) {
                            doCommitString(str);
                        }
                    }
                }
            );
        } else {
            repeat_handler_.cancel();
        }
    } else {
        repeat_handler_.cancel();
    }
}

bool ClakState::triggerShiftAutoFix() {
    if (!engine_->config() || !clak_config_get_shift_autofix(engine_->config())) {
        utils::clakLog("triggerShiftAutoFix: shift_autofix is disabled in config");
        return false;
    }
    if (!ic_) {
        return false;
    }
    if (ic_->capabilityFlags().test(fcitx::CapabilityFlag::Password)) {
        utils::clakLog("triggerShiftAutoFix: password field, skipping");
        return false;
    }

    const auto& surr = ic_->surroundingText();
    uint64_t now_us = fcitx::now(CLOCK_MONOTONIC);
    unsigned int cur = surr.isValid() ? surr.cursor() : 0;
    unsigned int anch = surr.isValid() ? surr.anchor() : 0;
    bool is_address_bar = (currentProfile() && currentProfile()->name() == "address-bar") ||
                          (ic_->capabilityFlags().test(fcitx::CapabilityFlag::Url));
    utils::clakLog("triggerShiftAutoFix: cur=" + std::to_string(cur) + " anch=" + std::to_string(anch) +
                   " text_len=" + std::to_string(surr.isValid() ? surr.text().size() : 0) +
                   " is_addr_bar=" + std::to_string(is_address_bar));

    // quick undo check within 2s of previous autofix
    if (!last_autofix_original_.empty() && last_autofix_time_us_ > 0 &&
        (now_us - last_autofix_time_us_ < 2000000)) {
        utils::clakLog("triggerShiftAutoFix: undo requested to restore '" + last_autofix_original_ + "'");
        size_t converted_chars = last_autofix_converted_chars_;
        std::string restore_text = last_autofix_original_;
        last_autofix_original_.clear();
        last_autofix_converted_chars_ = 0;
        last_autofix_time_us_ = 0;

        if (converted_chars > 0) {
            if (cur >= converted_chars) {
                ic_->deleteSurroundingText(-static_cast<int>(converted_chars), converted_chars);
            }
            doCommitString(restore_text);
            return true;
        }
    }

    int method = clak_config_get_method(engine_->config());

    // selection mode: surrounding selection or primary selection fallback
    std::string selected;
    if (surr.isValid() && cur != anch) {
        selected = utils::extractSelection(surr.text(), cur, anch);
    } else if (is_address_bar || !surr.isValid()) {
#if CLAK_HAVE_FCITX5_CLIPBOARD
        if (engine_ && engine_->instance()) {
            auto* cb = engine_->instance()->addonManager().addon("clipboard");
            if (cb) {
                try {
                    selected = cb->call<fcitx::IClipboard::primary>(ic_);
                } catch (...) {
                    selected.clear();
                }
            }
        }
#endif
        if (!selected.empty()) {
            utils::clakLog("triggerShiftAutoFix: retrieved primary selection: '" + selected + "'");
        }
    }

    if (!selected.empty()) {
        int changed = 0;
        char* fixed = clak_autofix_text(selected.c_str(), method, 1, &changed);
        if (fixed) {
            std::string fixed_str = fixed;
            clak_free_string(fixed);

            if (changed != 0 && fixed_str != selected) {
                last_autofix_original_ = selected;
                last_autofix_converted_chars_ = utils::utf8CharCount(fixed_str);
                last_autofix_time_us_ = now_us;

                utils::clakLog("triggerShiftAutoFix (selection): '" + selected + "' -> '" + fixed_str + "'");
                // commit string replaces selection directly in wayland and x11
                doCommitString(fixed_str);
                return true;
            }
        }
        return false;
    }

    // word-before-cursor mode: cursor == anchor and cur > 0
    if (surr.isValid() && cur > 0) {
        std::string wordBefore = utils::extractWordBeforeCursor(surr.text(), cur);
        if (wordBefore.empty()) {
            return false;
        }
        int changed = 0;
        char* fixed = clak_autofix_text(wordBefore.c_str(), method, 1, &changed);
        if (!fixed) {
            return false;
        }
        std::string fixed_str = fixed;
        clak_free_string(fixed);

        if (changed != 0 && fixed_str != wordBefore) {
            size_t word_chars = utils::utf8CharCount(wordBefore);
            last_autofix_original_ = wordBefore;
            last_autofix_converted_chars_ = utils::utf8CharCount(fixed_str);
            last_autofix_time_us_ = now_us;

            utils::clakLog("triggerShiftAutoFix (word-before): '" + wordBefore + "' -> '" + fixed_str + "'");
            ic_->deleteSurroundingText(-static_cast<int>(word_chars), word_chars);
            doCommitString(fixed_str);
            return true;
        }
    }

    return false;
}

} // namespace ime
} // namespace clak
