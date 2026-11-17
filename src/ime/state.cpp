#include "state.h"
#include "engine.h"
#include "config/config.h"
#include "config/sites.h"
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

namespace clak {
namespace ime {

ClakState::ClakState(ClakEngine* engine, fcitx::InputContext* ic)
    : engine_(engine),
      ic_(ic),
      editor_context_(ic, verifier_),
      hotkey_handler_(
          engine,
          ic,
          [this](const std::string& text) { doCommitString(text); },
          [this](bool force) { reset(force); },
          [this]() { return currentProfile(); }
      ) {
    int method = (engine_ && engine_->config()) ? clak_config_get_method(engine_->config()) : CLAK_METHOD_TELEX;
    rust_session_ = clak_session_new(method);
    syncConfig();
}

void ClakState::syncConfig() {
    if (!engine_ || !rust_session_) return;
    const ClakConfig* cfg = engine_->config();
    if (cfg && applied_config_version_ != engine_->configVersion()) {
        clak_session_apply_config(rust_session_, cfg);
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
    caret_tracker_.reset();
    hotkey_handler_.reset();
    if (!isDeleting()) {
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
    if (rust_session_) {
        clak_session_free(rust_session_);
        rust_session_ = nullptr;
    }
}

void ClakState::reset(bool force) {
    std::string app = appKey();
    std::string site = activeSite();
    if (isDeleting()) {
        utils::clakLog("reset: ignored mid-flight uinput deletion (sentinel in flight: " +
                       std::to_string(currentBackspaceCount()) + "/" + std::to_string(expectedBackspaces()) +
                       " pending='" + pendingCommitString() + "' app=" + app + " site='" + site + "')");
        return;
    }
    hotkey_handler_.onReset();
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
        if (bufferedKeysCount() > 0) {
            utils::clakLog("reset: ignored reset while keys buffered (count=" +
                           std::to_string(bufferedKeysCount()) + ") app=" + app + " site='" + site + "'");
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
        hotkey_handler_.clearAutofix();
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

    if (bufferedKeysCount() > 0 || !pendingCommitString().empty()) {
        utils::clakLog("reset: clearing state (buf_len=" + std::to_string(bufferedKeysCount()) + ") app=" + app + " site='" + site + "'");
    }
    is_address_bar_fix_ = false;
    is_selection_deletion_ = false;
    if (force) {
        last_uinput_finish_time_us_ = 0;
    }
    if (now_us > sentinel_grace_until_us_) {
        in_flight_sentinel_count_ = 0;
        sentinel_grace_until_us_ = 0;
    }
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
    surr_lag_retries_ = 0;
    if (surr_lag_timer_) {
        surr_lag_timer_.reset();
    }
    editor_context_.reset();
    if (rust_session_) {
        clak_session_reset(rust_session_, force);
    }
}

void ClakState::resetHard() {
    caret_tracker_.reset();
    hotkey_handler_.onMouseClicked();
    verifier_.reset(/*hard_reset=*/true);
    reset(/*force=*/true);
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
    (void)commit_str;
    (void)app;
    repeat_handler_.cancel();
    uint64_t delay_us = std::min<uint64_t>(20000ULL, 8000ULL + count * 2000ULL);
    auto cmds = clak_session_switch_to_forward_fallback(rust_session_, delay_us);
    executeSessionCommands(cmds);
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

void ClakState::handleSafetyTimeout() {
    uint64_t actual_fired_us = fcitx::now(CLOCK_MONOTONIC);
    std::string app = appKey();
    std::string site = activeSite();
    bool is_draftjs = isDraftJsEditor() || config::isDraftJsSite(site) || config::isTikTokSite(site);
    bool is_terminal = ic_ && ic_->capabilityFlags().test(fcitx::CapabilityFlag::Terminal);
    pacing::PacingGroup pgroup = pacing::AdaptivePacingEngine::classify(app, site, is_address_bar_fix_, is_draftjs, is_terminal);
    pacing_engine_.observeTimerDrift(pgroup, 0, actual_fired_us);
    pacing_engine_.onIncident(pgroup, pacing::IncidentType::PacingStarvation, actual_fired_us);

    size_t emitted = uinput::UinputTool::instance().cancel_pending();
    size_t exp = expectedBackspaces();
    size_t cur = currentBackspaceCount();
    size_t expected_in_flight = (emitted > 0) ? emitted : exp;
    if (expected_in_flight > cur) {
        in_flight_sentinel_count_ = expected_in_flight - cur;
        sentinel_grace_until_us_ = fcitx::now(CLOCK_MONOTONIC) + 500000;
    }
    is_address_bar_fix_ = false;
    is_selection_deletion_ = false;
    last_uinput_finish_time_us_ = fcitx::now(CLOCK_MONOTONIC);
    if (surr_lag_timer_) {
        surr_lag_timer_.reset();
    }
    is_surr_lag_replaying_ = false;
    verifier_.clearPending();
    uinput::UinputTool::instance().consume_sentinel_emit_time_us();

    logCsv(0, "TIMEOUT");
    logLatency(op_group_, op_start_us_, "REPLACE_TIMEOUT");
    op_start_us_ = 0;

    auto cmds = clak_session_on_timer_fired(rust_session_, CLAK_TIMER_SAFETY, actual_fired_us);
    executeSessionCommands(cmds);
}

void ClakState::finishUinputDeletion(uint64_t rtt_us) {
    if (!isDeleting()) return;
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

    size_t exp = expectedBackspaces();
    size_t cur = currentBackspaceCount();
    if (exp > cur) {
        in_flight_sentinel_count_ = exp - cur;
        sentinel_grace_until_us_ = fcitx::now(CLOCK_MONOTONIC) + 500000;
    }
    is_address_bar_fix_ = false;
    is_selection_deletion_ = false;
    last_uinput_finish_time_us_ = fcitx::now(CLOCK_MONOTONIC);

    std::string app = appKey();
    std::string site = activeSite();
    bool is_draftjs = isDraftJsEditor() || config::isDraftJsSite(site) || config::isTikTokSite(site);
    bool is_terminal = ic_ && ic_->capabilityFlags().test(fcitx::CapabilityFlag::Terminal);
    pacing::PacingGroup pgroup = pacing::AdaptivePacingEngine::classify(app, site, is_address_bar_fix_, is_draftjs, is_terminal);
    pacing_engine_.recordRtt(pgroup, rtt_us, last_key_interval_us_);
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

    uint32_t compensation_ms = pacing_engine_.commitCompensation(pgroup);
    if ((apps::isKonsoleApp(app) || apps::isKonsoleApp(site)) && compensation_ms < 8) {
        compensation_ms = 8;
    }
    if (compensation_ms > 0 && engine_ && engine_->instance()) {
        commit_timer_ = engine_->instance()->eventLoop().addTimeEvent(
            CLOCK_MONOTONIC,
            fcitx::now(CLOCK_MONOTONIC) + compensation_ms * 1000ULL,
            1000,
            [this](fcitx::EventSourceTime*, uint64_t) {
                commit_timer_.reset();
                executeSessionCommands(clak_session_on_sentinel_received(rust_session_, fcitx::now(CLOCK_MONOTONIC)));
                return true;
            }
        );
    } else {
        executeSessionCommands(clak_session_on_sentinel_received(rust_session_, fcitx::now(CLOCK_MONOTONIC)));
    }
}

void ClakState::replayBufferedKeys(const std::string& initial_batch) {
    key_buffer_.replay(
        initial_batch,
        [this](const fcitx::Key& k) { return handleKey(k); },
        [this](const std::string& s) { doCommitDirect(s); },
        [this](const fcitx::Key& k) { if (ic_) ic_->forwardKey(k); },
        [this]() { return isDeleting(); },
        nullptr
    );
}

void ClakState::executeSessionCommands(const ClakSessionCommandList& list) {
    if (!list.commands || list.count == 0) return;
    for (size_t i = 0; i < list.count; ++i) {
        const auto& cmd = list.commands[i];
        switch (cmd.cmd_type) {
            case CLAK_CMD_FORWARD_KEY: {
                if (ic_) {
                    auto* profile = currentProfile();
                    bool is_printable = (cmd.sym >= 0x20 && cmd.sym < 0xff00);
                    if (profile && profile->directCommitPrintableKey() && is_printable) {
                        std::string s = fcitx::Key::keySymToUTF8(static_cast<fcitx::KeySym>(cmd.sym));
                        if (!s.empty()) {
                            doCommitDirect(s);
                            break;
                        }
                    }
                    ic_->forwardKey(fcitx::Key(static_cast<fcitx::KeySym>(cmd.sym)));
                }
                break;
            }
            case CLAK_CMD_COMMIT_STRING: {
                if (cmd.text && cmd.text[0] != '\0') {
                    doCommitDirect(cmd.text);
                }
                break;
            }
            case CLAK_CMD_DELETE_SURROUNDING: {
                if (ic_) {
                    const auto& surr = ic_->surroundingText();
                    if (surr.isValid() && surr.cursor() < cmd.count) {
                        utils::clakLog("WARNING: surr.cursor()=" + std::to_string(surr.cursor()) +
                                       " < count=" + std::to_string(cmd.count) +
                                       ", skipping deleteSurroundingText to prevent compositor crash!");
                        std::string to_commit = "";
                        if (i + 1 < list.count && list.commands[i + 1].cmd_type == CLAK_CMD_COMMIT_STRING) {
                            to_commit = list.commands[i + 1].text ? list.commands[i + 1].text : "";
                        }
                        scheduleForwardBackspaceFallback(cmd.count, to_commit, appKey());
                    } else {
                        if (surr.isValid() && surr.cursor() >= cmd.count) {
                            std::string wordBefore = utils::extractWordBeforeCursor(surr.text(), surr.cursor());
                            std::string expectedCommit = "";
                            if (i + 1 < list.count && list.commands[i + 1].cmd_type == CLAK_CMD_COMMIT_STRING) {
                                expectedCommit = list.commands[i + 1].text ? list.commands[i + 1].text : "";
                            }
                            verifier_.setExpectation(wordBefore, cmd.count, expectedCommit);
                        }
                        ic_->deleteSurroundingText(cmd.offset, cmd.count);
                    }
                }
                break;
            }
            case CLAK_CMD_EMIT_UINPUT_BACKSPACE: {
                if (!uinput::UinputTool::instance().send_backspace(
                        cmd.count, cmd.post_delay_ms, cmd.pre_delay_ms, cmd.gap_ms, cmd.hold_us)) {
                    utils::clakLog("ERROR: uinput send_backspace failed! app=" + appKey());
                    uint64_t delay_us = std::min<uint64_t>(20000ULL, 8000ULL + cmd.count * 2000ULL);
                    auto fallback_cmds = clak_session_switch_to_forward_fallback(rust_session_, delay_us);
                    executeSessionCommands(fallback_cmds);
                    return;
                }
                word_used_uinput_ = true;
                break;
            }
            case CLAK_CMD_START_TIMER: {
                if (!engine_ || !engine_->instance()) break;
                if (cmd.timer_id == CLAK_TIMER_SAFETY) {
                    safety_timer_ = engine_->instance()->eventLoop().addTimeEvent(
                        CLOCK_MONOTONIC,
                        fcitx::now(CLOCK_MONOTONIC) + cmd.delay_us,
                        1000,
                        [this](fcitx::EventSourceTime*, uint64_t) {
                            if (safety_timer_) safety_timer_.reset();
                            if (isDeleting()) {
                                handleSafetyTimeout();
                            }
                            return true;
                        }
                    );
                } else if (cmd.timer_id == CLAK_TIMER_COMMIT) {
                    commit_timer_ = engine_->instance()->eventLoop().addTimeEvent(
                        CLOCK_MONOTONIC,
                        fcitx::now(CLOCK_MONOTONIC) + cmd.delay_us,
                        1000,
                        [this](fcitx::EventSourceTime*, uint64_t) {
                            if (commit_timer_) commit_timer_.reset();
                            executeSessionCommands(clak_session_on_timer_fired(
                                rust_session_, CLAK_TIMER_COMMIT, fcitx::now(CLOCK_MONOTONIC)));
                            return true;
                        }
                    );
                }
                break;
            }
            case CLAK_CMD_CANCEL_TIMER: {
                if (cmd.timer_id == CLAK_TIMER_SAFETY && safety_timer_) {
                    safety_timer_.reset();
                } else if (cmd.timer_id == CLAK_TIMER_COMMIT && commit_timer_) {
                    commit_timer_.reset();
                }
                break;
            }
            case CLAK_CMD_FORWARD_BACKSPACE: {
                forwardBackspacePressRelease(cmd.count);
                break;
            }
            case CLAK_CMD_START_CUSTOM_DELETION: {
                if (currentProfile() && engine_ && engine_->instance()) {
                    currentProfile()->startCustomDeletion(
                        engine_->instance()->eventLoop(),
                        ic_,
                        cmd.count,
                        cmd.text ? cmd.text : "",
                        [this](const std::string& str) { doCommitDirect(str); },
                        [this]() {
                            executeSessionCommands(clak_session_on_sentinel_received(
                                rust_session_, fcitx::now(CLOCK_MONOTONIC)));
                        }
                    );
                }
                break;
            }
        }
    }
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
        std::string pending = pendingCommitString();
        if (!pending.empty()) {
            doCommitString(pending);
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
            (now_us - last_commit_time_us_ < 60000) &&
            !is_surr_lag_replaying_ &&
            surr_lag_retries_ < 4 &&
            engine_ && engine_->instance() && std::string(ic_->frontend()) != "mock") {
            key_buffer_.push(key);
            surr_lag_retries_++;
            uint64_t elapsed_us = (now_us >= last_commit_time_us_) ? (now_us - last_commit_time_us_) : 0;
            uint64_t delay_us = (elapsed_us < 10000) ? (10000 - elapsed_us) : 4000;
            utils::clakLog("handleKey: surrounding replacement lagging in app=" + app +
                           " (" + std::to_string(elapsed_us / 1000) + "ms since commit, retry " +
                           std::to_string(surr_lag_retries_) + "/4), buffering key for " +
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
        surr_lag_retries_ = 0;
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
        surr_lag_retries_ = 0;
    }

    if (is_gecko && verifier_.isRichTextEditor()) {
        verifier_.setRichTextEditor(false);
    }

    if (valid_surr) {
        surr_text = surr.text().c_str();
        caret_tracker_.resolveEffectiveOffsets(surr, cursor, anchor);
    } else {
        caret_tracker_.reset();
        surr_text = nullptr;
        cursor = 0;
        anchor = 0;
    }

    bool is_address_bar = (currentProfile() && currentProfile()->name() == "address-bar") ||
                          (ic_ && ic_->capabilityFlags().test(fcitx::CapabilityFlag::Url)) ||
                          (isBrowser() && (site == "New Tab" || site == "about:blank" || site == "address-bar"));
    bool has_autofill_selection = isAutofillCertain(surr);

    int32_t deletion_mode = CLAK_DELETION_SURROUNDING;
    if (currentProfile()->erasesWithCommittedDel()) {
        deletion_mode = CLAK_DELETION_COMMITTED_DEL;
    } else if (is_address_bar || has_autofill_selection || shouldUseUinput(use_surrounding, 0, surr)) {
        deletion_mode = CLAK_DELETION_UINPUT;
    } else if (use_surrounding) {
        deletion_mode = CLAK_DELETION_SURROUNDING;
    } else if (currentProfile()->hasCustomDeletion()) {
        deletion_mode = CLAK_DELETION_CUSTOM;
    } else {
        deletion_mode = CLAK_DELETION_FORWARD_BS;
    }

    uint32_t autofill_extra = has_autofill_selection ? 1 : 0;
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

    bool is_custom_preset = (cfg && clak_config_get_pacing_preset(cfg) == 3);
    pacing_engine_.setCustomMode(is_custom_preset);

    bool is_terminal = ic_ && ic_->capabilityFlags().test(fcitx::CapabilityFlag::Terminal);
    pacing::PacingGroup pgroup = pacing::AdaptivePacingEngine::classify(app, site, is_address_bar, is_draftjs, is_terminal);
    if (cfg) {
        uint32_t term_delay = clak_config_get_terminal_delay_ms(cfg);
        uint32_t term_gap = clak_config_get_terminal_gap_ms(cfg);
        uint32_t term_ceiling = is_custom_preset ? std::min(24u, term_delay + 4u) : 30u;
        pacing_engine_.configureGroup(pacing::PacingGroup::TerminalUinput,
            term_delay, term_gap, term_ceiling, 10);
        uint32_t draft_delay = clak_config_get_draftjs_delay_ms(cfg);
        uint32_t draft_gap = clak_config_get_draftjs_gap_ms(cfg);
        uint32_t draft_ceiling = is_custom_preset ? std::min(32u, draft_delay + 6u) : std::max(60u, draft_delay + 20u);
        uint32_t draft_gap_ceiling = is_custom_preset ? std::min(12u, draft_gap + 4u) : std::max(20u, draft_gap + 10u);
        pacing_engine_.configureGroup(pacing::PacingGroup::WebDraftJs,
            draft_delay, draft_gap, draft_ceiling, draft_gap_ceiling);
        uint32_t heavy_delay = clak_config_get_heavy_app_delay_ms(cfg);
        uint32_t heavy_gap = clak_config_get_heavy_app_gap_ms(cfg);
        uint32_t heavy_ceiling = is_custom_preset ? std::min(28u, heavy_delay + 4u) : 35u;
        pacing_engine_.configureGroup(pacing::PacingGroup::HeavyApp,
            heavy_delay, heavy_gap, heavy_ceiling, 12);
        pacing_engine_.configureGroup(pacing::PacingGroup::SteamCEF,
            4, 3, 18, 4);
        pacing_engine_.configureGroup(pacing::PacingGroup::Default,
            4, 1, 25, 3);
    }

    if (fixed_override) {
        uint32_t f_delay = 10;
        try { f_delay = std::stoul(env_fixed_delay); } catch (...) {}
        auto base_pacing = pacing_engine_.uinputPacing(pgroup, last_key_interval_us_);
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
        pacing = pacing_engine_.uinputPacing(pgroup, last_key_interval_us_);
    } else if (modal_handler_.isModalEditor() || config::isTerminalApp(app)) {
        pacing = {cfg ? clak_config_get_terminal_delay_ms(cfg) : config::kTerminalPostDelayMs,
                  cfg ? clak_config_get_terminal_gap_ms(cfg) : config::kTerminalGapMs};
    } else if (is_draftjs || isDraftJsEditor() || config::isTikTokSite(site)) {
        pacing = {cfg ? clak_config_get_draftjs_delay_ms(cfg) : config::kDraftJsPostDelayMs,
                  cfg ? clak_config_get_draftjs_gap_ms(cfg) : config::kDraftJsGapMs};
    } else if (apps::isSteamApp(app)) {
        pacing = {4, 3};
    } else if (apps::isWpsOfficeApp(app) || apps::isJetBrainsApp(app)) {
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

    bool recent_selection = (last_selection_time_us_ > 0) && (now_us - last_selection_time_us_ < 1500000);
    bool has_selection = has_surrounding && surr.isValid() && (surr.cursor() != surr.anchor());
    is_selection_deletion_ = recent_selection || has_selection;
    is_address_bar_fix_ = is_address_bar;
    op_group_ = classifyGroup(app, site, is_address_bar, deletion_mode == CLAK_DELETION_UINPUT);

    uint64_t safety_timeout_us = is_selection_deletion_ ? config::kSelectionDeletionTimeoutUs : config::kSafetyTimeoutUs;

    auto cmds = clak_session_process_key(
        rust_session_,
        sym,
        key_str.c_str(),
        has_ctrl_alt,
        surr_text,
        cursor,
        anchor,
        deletion_mode,
        autofill_extra,
        pacing.post_delay_ms,
        pre_delay,
        pacing.gap_ms,
        pacing.hold_us,
        safety_timeout_us,
        now_us);

    bool emitted_uinput = false;
    for (size_t i = 0; i < cmds.count; ++i) {
        if (cmds.commands[i].cmd_type == CLAK_CMD_EMIT_UINPUT_BACKSPACE) {
            emitted_uinput = true;
            break;
        }
    }
    if (emitted_uinput) {
        if (valid_surr && surr.cursor() < 1 && !config::isVSCodeApp(app) && !currentProfile()->preventsUinput()) {
            surr_cursor_underflow_count_++;
            if (surr_cursor_underflow_count_ >= 3) {
                verifier_.setRichTextEditor(true);
                if (isBrowser()) {
                    verifier_.setDraftJsEditor(true);
                }
            }
        } else if (surr.isValid() && surr.cursor() >= 1) {
            surr_cursor_underflow_count_ = 0;
        }
    }

    if (cmds.count == 1 && cmds.commands[0].cmd_type == CLAK_CMD_FORWARD_KEY) {
        hotkey_handler_.clearAutofix();
        verifier_.clearPending();
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
    }

    const char* af_del = clak_session_last_autofix_delete(rust_session_);
    const char* af_com = clak_session_last_autofix_commit(rust_session_);
    if (af_del && af_del[0] != '\0' && af_com) {
        hotkey_handler_.recordAutofix(af_del, utils::utf8CharCount(af_com), fcitx::now(CLOCK_MONOTONIC));
        utils::clakLog("handleKey: autocorrect applied: '" + std::string(af_del) +
                       "' -> '" + std::string(af_com) + "' (tap Shift to undo)");
    } else {
        hotkey_handler_.clearAutofix();
    }
    executeSessionCommands(cmds);
    last_text_len_ = 0;
    return true;
}

void ClakState::keyEvent(fcitx::KeyEvent& keyEvent) {
    if (ic_ && ic_->capabilityFlags().test(fcitx::CapabilityFlag::Password)) {
        reset(/*force=*/true);
        return;
    }

    std::string app = appKey();
    const auto& key = keyEvent.key();

    if (repeat_handler_.heldKey().sym() != 0 && keyEvent.isRelease()) {
        repeat_handler_.cancel();
    }
    syncConfig();

    if (hotkey_handler_.handleKeyEvent(keyEvent, app)) {
        return;
    }
    if (keyEvent.isRelease()) {
        return;
    }

    bool is_ctrl = key.states().test(fcitx::KeyState::Ctrl);
    bool is_shift = key.states().test(fcitx::KeyState::Shift);
    bool is_alt = key.states().test(fcitx::KeyState::Alt);
    bool is_super = key.states().test(fcitx::KeyState::Super);
    bool has_ctrl_alt = is_ctrl || is_alt || is_super;
    uint32_t sym = key.sym();
    if (sym == 0 || sym == FcitxKey_None) {
        return;
    }
    std::string key_str = fcitx::Key::keySymToUTF8(key.sym());

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
    if (is_ctrl_z && hotkey_handler_.tryUndo(ic_)) {
        repeat_handler_.cancel();
        reset(/*force=*/true);
        keyEvent.filterAndAccept();
        return;
    }

    bool is_clean_backspace = (sym == FcitxKey_BackSpace) && !has_ctrl_alt && !is_shift;
    if (is_clean_backspace && last_uinput_finish_time_us_ > 0 &&
        (fcitx::now(CLOCK_MONOTONIC) - last_uinput_finish_time_us_) < 25000 &&
        apps::isSteamApp(app)) {
        utils::clakLog("keyEvent: steam post-deletion uinput bounce dropped delta=" +
                       std::to_string(fcitx::now(CLOCK_MONOTONIC) - last_uinput_finish_time_us_) + "us");
        keyEvent.filterAndAccept();
        return;
    }
    if (!isDeleting() && is_clean_backspace && in_flight_sentinel_count_ > 0 &&
        fcitx::now(CLOCK_MONOTONIC) <= sentinel_grace_until_us_) {
        in_flight_sentinel_count_--;
        keyEvent.filterAndAccept();
        return;
    }
    if (isDeleting() && expectedBackspaces() > 0 && is_clean_backspace) {
        if (!clak_session_on_backspace_received(rust_session_)) {
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

    if (ic_) {
        caret_tracker_.onKeyEvent(sym, is_cursor_move, is_shift, has_ctrl_alt, is_special_nav, ic_->surroundingText());
    }

    if (has_ctrl_alt || is_cursor_move || is_special_nav) {
        repeat_handler_.cancel();
        if (profile) {
            profile->cancelCustomDeletion();
        }
        if (isDeleting()) {
            if (safety_timer_) {
                safety_timer_.reset();
            }
            if (commit_timer_) {
                commit_timer_.reset();
            }
            size_t exp = expectedBackspaces();
            size_t cur = currentBackspaceCount();
            if (exp > cur) {
                in_flight_sentinel_count_ = exp - cur;
                sentinel_grace_until_us_ = fcitx::now(CLOCK_MONOTONIC) + 500000;
            }
            is_address_bar_fix_ = false;
            is_selection_deletion_ = false;
            std::string p = pendingCommitString();
            if (!p.empty()) {
                doCommitString(p);
            }
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

    if (isDeleting() || (surr_lag_timer_ && !is_surr_lag_replaying_)) {
        repeat_handler_.cancel();
        collision_stats_.buffered_key_interruptions++;
        if (surr_lag_timer_ && !is_surr_lag_replaying_) {
            key_buffer_.push(key);
            keyEvent.filterAndAccept();
            return;
        }
        size_t exp = expectedBackspaces();
        size_t cur = currentBackspaceCount();
        auto cmds = clak_session_process_key(
            rust_session_,
            sym,
            key_str.c_str(),
            has_ctrl_alt,
            nullptr,
            0,
            0,
            CLAK_DELETION_UINPUT,
            0,
            0,
            0,
            0,
            0,
            0,
            fcitx::now(CLOCK_MONOTONIC));
        executeSessionCommands(cmds);
        if (!isDeleting() && exp > cur) {
            in_flight_sentinel_count_ = exp - cur;
            sentinel_grace_until_us_ = fcitx::now(CLOCK_MONOTONIC) + 500000;
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

    if (!isDeleting()) {
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
                    if (isDeleting()) {
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

} // namespace ime
} // namespace clak
