#include "state.h"
#include "engine.h"
#include "config/config.h"
#include "config/sites.h"
#include "platform/window_info.h"
#include "uinput/uinput.h"
#include "utils/log.h"
#include "utils/text_utils.h"

#include <fcitx/inputcontext.h>
#include <fcitx-utils/keysym.h>
#include <fcitx-utils/utf8.h>

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

ClakState::~ClakState() {
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
    if (!force) {
        // guard against false reset from web apps within 50ms post-commit window
        uint64_t now_us = fcitx::now(CLOCK_MONOTONIC);
        if (last_commit_time_us_ > 0 && (now_us - last_commit_time_us_) < 50000) {
            utils::clakLog("reset: ignored false reset within 50ms post-commit window (delta=" +
                           std::to_string(now_us - last_commit_time_us_) + "us) app=" + app + " site='" + site + "'");
            return;
        }
        if (!key_buffer_.empty()) {
            utils::clakLog("reset: ignored reset while keys buffered (count=" +
                           std::to_string(key_buffer_.size()) + ") app=" + app + " site='" + site + "'");
            return;
        }
    }
    last_text_len_ = 0;
    if (safety_timer_) {
        safety_timer_.reset();
    }
    repeat_handler_.cancel();
    modal_handler_.reset();
    steam_pipeline_.reset();

    if (!key_buffer_.empty() || !pending_commit_string_.empty()) {
        utils::clakLog("reset: clearing state (buf_len=" + std::to_string(key_buffer_.size()) + ") app=" + app + " site='" + site + "'");
    }
    is_deleting_ = false;
    is_address_bar_fix_ = false;
    is_selection_deletion_ = false;
    expected_backspaces_ = 0;
    current_backspace_count_ = 0;
    in_flight_sentinel_count_ = 0;
    sentinel_grace_until_us_ = 0;
    pending_commit_string_.clear();
    key_buffer_.clear();
    verifier_.reset();
    cached_site_.clear();
    last_site_check_us_ = 0;
    if (rust_ctx_) {
        clak_context_reset(rust_ctx_);
    }
}

std::string ClakState::appKey() {
    if (ic_ && !ic_->program().empty()) {
        return ic_->program();
    }
    if (ic_ && ic_->capabilityFlags().test(fcitx::CapabilityFlag::Terminal)) {
        return "terminal";
    }
    platform::WindowInfo win = platform::getActiveWindow();
    if (!win.win_class.empty()) {
        return win.win_class;
    }
    return "default";
}

std::string ClakState::activeSite() {
    uint64_t now_us = fcitx::now(CLOCK_MONOTONIC);
    if (now_us - last_site_check_us_ < config::kCacheCheckIntervalUs && !cached_site_.empty()) {
        return cached_site_;
    }
    last_site_check_us_ = now_us;
    std::string fallback = (ic_ && !ic_->program().empty()) ? ic_->program() : "";
    platform::WindowInfo win = platform::getActiveWindow(fallback);
    cached_site_ = config::extractDomain(win.win_class, win.win_title);
    return cached_site_;
}

bool ClakState::isBrowser() const {
    std::string app = const_cast<ClakState*>(this)->appKey();
    return config::isBrowserApp(app);
}

bool ClakState::isGecko() const {
    std::string app = const_cast<ClakState*>(this)->appKey();
    return config::isGeckoApp(app);
}

bool ClakState::isSteam() const {
    std::string app = const_cast<ClakState*>(this)->appKey();
    return config::isSteamApp(app);
}

bool ClakState::isDraftJsEditor() const {
    std::string site = const_cast<ClakState*>(this)->activeSite();
    return verifier_.isDraftJsEditor(site);
}



static bool isWpsFontSizeText(const std::string& text) {
    if (text.empty() || text.size() > 4) return false;
    for (char c : text) {
        if (!isdigit(static_cast<unsigned char>(c)) && c != '.') return false;
    }
    return true;
}

bool ClakState::shouldUseUinput(bool use_surrounding, uint32_t action_type, const fcitx::SurroundingText& /*surr*/) {
    if (modal_handler_.isModalEditor()) {
        return true;
    }

    std::string app = appKey();
    std::string site = activeSite();

    if (config::isWpsOfficeApp(app)) {
        return true;
    }
    if (config::isTerminalApp(app)) {
        return true;
    }
    if (config::isGeckoApp(app)) {
        return true;
    }
    if (config::isMetaSite(site) || config::isMetaSite(app)) {
        return false;
    }
    if (config::isSteamApp(app)) {
        return false;
    }
    if (action_type == CLAK_ACTION_ADDRESS_BAR_FIX) {
        return true;
    }
    if (config::isForceUinputSite(site) || isDraftJsEditor()) {
        return true;
    }
    if (verifier_.isCanvasEditor() || verifier_.isRichTextEditor()) {
        return true;
    }
    return !use_surrounding;
}

bool ClakState::isAutofillCertain(const fcitx::SurroundingText& surr) {
    if (!surr.isValid()) return false;
    std::string app = appKey();
    std::string site = activeSite();
    if (config::isMetaSite(site) || config::isMetaSite(app)) {
        return false;
    }
    const std::string& text = surr.text();
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
    last_commit_time_us_ = fcitx::now(CLOCK_MONOTONIC);
    ic_->commitString(text);
}

std::string ClakState::classifyGroup(const std::string& app, const std::string& site, bool is_autofill, bool used_uinput) {
    bool has_url_cap = ic_ && ic_->capabilityFlags().test(fcitx::CapabilityFlag::Url);
    if (is_autofill || has_url_cap) return "address-bar";
    if (config::isWpsOfficeApp(app)) return "WPS-Office";
    if (config::isJetBrainsApp(app)) return "JetBrains-Uinput";
    if (config::isTerminalApp(app)) return "Terminal-Uinput";
    if (site == "docs.google.com" || site.find("docs.google.com") != std::string::npos) return "Docs-Uinput";
    if (isDraftJsEditor()) return "DraftJS-Uinput";
    if (config::isGeckoApp(app)) return "Gecko-Uinput";
    if (config::isSteamApp(app)) return "Steam-ForwardKey";
    if (used_uinput) return "Chromium-Uinput";
    return "Chromium-SurroundingText";
}

void ClakState::logLatency(const std::string& group, uint64_t start_us, const std::string& action_type) {
    if (start_us == 0) return;
    uint64_t now_us = fcitx::now(CLOCK_MONOTONIC);
    double delta_ms = (now_us >= start_us) ? static_cast<double>(now_us - start_us) / 1000.0 : 0.0;
    utils::clakLog("[LATENCY] group=" + group + " delta_ms=" + std::to_string(delta_ms) + " action=" + action_type);
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
    safety_timer_ = engine_->instance()->eventLoop().addTimeEvent(
        CLOCK_MONOTONIC,
        now_us + timeout_us,
        1000,
        [this, timeout_us](fcitx::EventSourceTime*, uint64_t) {
            if (is_deleting_) {
                if (adaptive_extra_us_ < 100000) {
                    adaptive_extra_us_ = std::min<uint64_t>(100000, adaptive_extra_us_ + 20000);
                }
                stable_transactions_count_ = 0;

                std::string app = appKey();
                std::string site = activeSite();
                utils::clakLog("SENTINEL TIMEOUT (" + std::to_string(timeout_us / 1000) + "ms expired): giving up waiting, only " +
                               std::to_string(current_backspace_count_) + "/" + std::to_string(expected_backspaces_) +
                                " BS returned! force committing '" + pending_commit_string_ + "' app=" + app +
                               " site='" + site + "'");
                is_deleting_ = false;
                is_address_bar_fix_ = false;
                is_selection_deletion_ = false;
                expected_backspaces_ = 0;
                current_backspace_count_ = 0;
                in_flight_sentinel_count_ = 0;
                sentinel_grace_until_us_ = 0;
                if (!pending_commit_string_.empty()) {
                    doCommitString(pending_commit_string_);
                    pending_commit_string_.clear();
                }
                logLatency(op_group_, op_start_us_, "REPLACE_TIMEOUT");
                op_start_us_ = 0;
                replayBufferedKeys();
            }
            return true;
        }
    );
}

void ClakState::finishUinputDeletion() {
    if (!is_deleting_) return;
    if (safety_timer_) {
        safety_timer_.reset();
    }
    if (expected_backspaces_ > current_backspace_count_) {
        in_flight_sentinel_count_ = expected_backspaces_ - current_backspace_count_;
        sentinel_grace_until_us_ = fcitx::now(CLOCK_MONOTONIC) + 50000;
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
    if (op_start_us_ > 0) {
        uint64_t now_us = fcitx::now(CLOCK_MONOTONIC);
        uint64_t roundtrip_us = (now_us > op_start_us_) ? (now_us - op_start_us_) : 0;
        observeTransactionLatency(roundtrip_us);
    }
    logLatency(op_group_, op_start_us_, "REPLACE");
    op_start_us_ = 0;
    replayBufferedKeys();
}

void ClakState::replayBufferedKeys() {
    key_buffer_.replay(
        [this](const fcitx::Key& k) { return handleKey(k); },
        [this](const std::string& str) { doCommitString(str); },
        [this](const fcitx::Key& k) { ic_->forwardKey(k); },
        [this]() { return is_deleting_; }
    );
}

bool ClakState::handleKey(const fcitx::Key& key) {
    if (key.isModifier()) {
        return false;
    }

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
        reset(/*force=*/true);
        return false;
    }

    const char* surr_text = nullptr;
    size_t cursor = 0;
    size_t anchor = 0;

    std::string app = appKey();
    std::string site = activeSite();
    bool is_term = config::isTerminalApp(app);
    bool is_jb = config::isJetBrainsApp(app);
    bool is_meta = config::isMetaSite(site) || config::isMetaSite(app);
    bool is_draftjs = isDraftJsEditor();
    bool is_force_uinput = config::isForceUinputSite(site) || is_draftjs;
    bool is_gecko = config::isGeckoApp(app);

    bool has_surrounding = ic_->capabilityFlags().test(fcitx::CapabilityFlag::SurroundingText);
    const auto& surr = ic_->surroundingText();

    if (sym == FcitxKey_BackSpace && has_surrounding && surr.isValid()) {
        if (surr.cursor() == 0 || surr.cursor() != surr.anchor()) {
            reset(/*force=*/true);
            return false;
        }
    }

    if (is_meta) {
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
    }

    bool is_steam = isSteam();
    bool is_office = config::isWpsOfficeApp(app);
    bool is_wps_toolbar = is_office && isWpsFontSizeText(surr.text());
    bool valid_surr = !is_steam && has_surrounding && surr.isValid() && !verifier_.isCanvasEditor() && !is_term && !is_jb && !is_wps_toolbar;
    if (valid_surr) {
        surr_text = surr.text().c_str();
        cursor = surr.cursor();
        anchor = surr.anchor();
    }
    bool use_surrounding = !is_steam && valid_surr && !verifier_.isRichTextEditor() && !is_force_uinput && !is_office;

    bool skip_verify = is_gecko || is_meta;
    if (verifier_.isPending() && use_surrounding && !skip_verify) {
        verifier_.verify(surr);
        is_draftjs = isDraftJsEditor();
        use_surrounding = !is_steam && valid_surr && !verifier_.isRichTextEditor() && !is_force_uinput && !is_draftjs && !is_office;
    } else if (verifier_.isPending()) {
        verifier_.clearPending();
    }

    if (is_gecko && verifier_.isRichTextEditor()) {
        verifier_.setRichTextEditor(false);
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

    switch (action.action_type) {
        case CLAK_ACTION_FORWARD:
            if (valid_surr) {
                last_text_len_ = surr.cursor() + 1;
            } else {
                last_text_len_ = 1;
            }
            if (is_steam && !has_ctrl_alt && !key_str.empty() && sym < 0xff00) {
                doCommitString(key_str);
                logLatency("Steam-ForwardKey", op_start_us_, "COMMIT");
                op_start_us_ = 0;
                return true;
            }
            logLatency(classifyGroup(app, site, false, false), op_start_us_, "FORWARD");
            op_start_us_ = 0;
            return false;

        case CLAK_ACTION_COMMIT:
            if (action.commit_str && action.commit_str[0] != '\0') {
                doCommitString(action.commit_str);
            }
            logLatency(classifyGroup(app, site, false, false), op_start_us_, "COMMIT");
            op_start_us_ = 0;
            last_text_len_ = 0;
            return true;

        case CLAK_ACTION_ADDRESS_BAR_FIX:
        case CLAK_ACTION_REPLACE:
        case CLAK_ACTION_REPLACE_SURROUNDING: {
            size_t real_bs = action.delete_count;

            bool is_autofill = (action.action_type == CLAK_ACTION_ADDRESS_BAR_FIX) ||
                               (isBrowser() && isAutofillCertain(surr));
            bool use_uinput = is_autofill || shouldUseUinput(use_surrounding, action.action_type, surr);

            if (use_uinput && real_bs > 0) {
                size_t autofill_extra = is_autofill ? 1 : 0;
                size_t bs_to_send = real_bs + autofill_extra + 1;
                is_address_bar_fix_ = is_autofill;
                uint64_t now_us = fcitx::now(CLOCK_MONOTONIC);
                bool recent_selection = (last_selection_time_us_ > 0) && (now_us - last_selection_time_us_ < 1500000);
                bool has_selection = has_surrounding && surr.isValid() && (surr.cursor() != surr.anchor());
                is_selection_deletion_ = recent_selection || has_selection;
                op_group_ = classifyGroup(app, site, is_autofill, true);

                bool is_wps = config::isWpsOfficeApp(app);
                bool use_term_pacing = is_term || is_jb || modal_handler_.isModalEditor();
                uint32_t post_delay = is_autofill ? config::kAddressBarPostDelayMs :
                                      (use_term_pacing ? config::kTerminalPostDelayMs :
                                      (is_draftjs ? 1 :
                                      (is_wps ? 3 : 2)));
                uint32_t gap_ms = use_term_pacing ? config::kTerminalGapMs :
                                  (is_draftjs ? 1 :
                                  (is_wps ? 2 : 2));
                uint32_t pre_delay = 0;

                utils::clakLog("uinput waiting for sentinel: expected=" + std::to_string(bs_to_send) +
                               " (real=" + std::to_string(real_bs) + (is_autofill ? " + 1 autofill" : "") +
                               " + 1 sentinel) post_delay=" + std::to_string(post_delay) + "ms gap=" +
                               std::to_string(gap_ms) + "ms commit='" +
                               (action.commit_str ? action.commit_str : "") + "' app=" + app + " site='" + site + "'");

                if (uinput::UinputTool::instance().send_backspace(bs_to_send, post_delay, pre_delay, gap_ms)) {
                    is_deleting_ = true;
                    expected_backspaces_ = bs_to_send;
                    current_backspace_count_ = 0;
                    pending_commit_string_ = (action.commit_str ? action.commit_str : "");
                    arm_safety_timer();
                    last_text_len_ = 0;
                    return true;
                } else {
                    utils::clakLog("ERROR: uinput send_backspace failed! app=" + app + " site='" + site + "'");
                }
            }

            if (real_bs > 0) {
                if (use_surrounding) {
                    std::string wordBefore = utils::extractWordBeforeCursor(surr.text(), surr.cursor());
                    verifier_.setExpectation(wordBefore, real_bs, action.commit_str ? action.commit_str : "");
                    utils::clakLog("surrounding delete: -" + std::to_string(real_bs) + " commit='" +
                                   (action.commit_str ? action.commit_str : "") + "' app=" + app + " site='" + site + "'");
                    ic_->deleteSurroundingText(-static_cast<int>(real_bs),
                                               static_cast<unsigned int>(real_bs));
                } else if (is_steam) {
                    utils::clakLog("steam forward backspace: count=" + std::to_string(real_bs) +
                                   " commit='" + (action.commit_str ? action.commit_str : "") + "' app=" + app);
                    if (engine_ && engine_->instance()) {
                        is_deleting_ = true;
                        expected_backspaces_ = real_bs;
                        current_backspace_count_ = 0;
                        pending_commit_string_ = (action.commit_str ? action.commit_str : "");
                        steam_pipeline_.startStaggeredDeletion(
                            engine_->instance()->eventLoop(),
                            ic_,
                            real_bs,
                            pending_commit_string_,
                            [this](const std::string& str) { doCommitString(str); },
                            [this]() {
                                pending_commit_string_.clear();
                                is_deleting_ = false;
                                logLatency("Steam-ForwardKey", op_start_us_, "REPLACE");
                                op_start_us_ = 0;
                                replayBufferedKeys();
                            }
                        );
                        last_text_len_ = 0;
                        return true;
                    } else {
                        for (size_t i = 0; i < real_bs; ++i) {
                            ic_->forwardKey(fcitx::Key(FcitxKey_BackSpace), false);
                            ic_->forwardKey(fcitx::Key(FcitxKey_BackSpace), true);
                        }
                        if (action.commit_str && action.commit_str[0] != '\0') {
                            doCommitString(action.commit_str);
                        }
                    }
                    logLatency("Steam-ForwardKey", op_start_us_, "REPLACE");
                    op_start_us_ = 0;
                    last_text_len_ = 0;
                    return true;
                } else {
                    utils::clakLog("forward backspace: count=" + std::to_string(real_bs) +
                                   " commit='" + (action.commit_str ? action.commit_str : "") + "' app=" + app);
                    for (size_t i = 0; i < real_bs; ++i) {
                        ic_->forwardKey(fcitx::Key(FcitxKey_BackSpace));
                    }
                }
            }
            if ((!is_steam || real_bs == 0) && action.commit_str && action.commit_str[0] != '\0') {
                doCommitString(action.commit_str);
            }
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

    bool is_alt = key.states().test(fcitx::KeyState::Alt);
    bool is_ctrl = key.states().test(fcitx::KeyState::Ctrl);
    bool is_shift = key.states().test(fcitx::KeyState::Shift);
    bool is_super = key.states().test(fcitx::KeyState::Super);
    bool is_ctrl_sym = (key.sym() == FcitxKey_Control_L || key.sym() == FcitxKey_Control_R);
    bool is_shift_sym = (key.sym() == FcitxKey_Shift_L || key.sym() == FcitxKey_Shift_R);
    char* sc_c = engine_->config() ? clak_config_get_toggle_shortcut(engine_->config()) : nullptr;
    std::string shortcut = sc_c ? sc_c : "ctrl_shift";
    if (sc_c) clak_free_string(sc_c);

    if (keyEvent.isRelease()) {
        if (repeat_handler_.heldKey().sym() != 0 && (key.sym() == repeat_handler_.heldKey().sym() || key.isModifier())) {
            repeat_handler_.cancel();
        }
        if (shortcut == "ctrl_shift") {
            if (ctrl_shift_armed_ && (is_shift_sym || is_ctrl_sym)) {
                engine_->toggleAppEnabled(app);
                reset(/*force=*/true);
                ic_->updateUserInterface(fcitx::UserInterfaceComponent::StatusArea, true);
                if (engine_->instance() && std::string(ic_->frontend()) != "mock") {
#ifdef FCITX5_HAVE_SHOW_CUSTOM_IM_INFO
                    engine_->instance()->showCustomInputMethodInformation(ic_, engine_->isAppEnabled(app) ? "VI" : "EN");
#else
                    engine_->instance()->showInputMethodInformation(ic_);
#endif
                }
                ctrl_shift_armed_ = false;
                ctrl_pressed_first_ = false;
            } else if (is_ctrl_sym) {
                ctrl_pressed_first_ = false;
            }
        }
        return;
    }

    syncConfig();

    utils::clakLog("keyEvent: sym=" + std::to_string(key.sym()) + " ('" + key_str + "') app='" + app +
                   "' enabled=" + std::to_string(engine_->isAppEnabled(app)) +
                   " ctrl=" + std::to_string(is_ctrl) + " shift=" + std::to_string(is_shift) +
                   " alt=" + std::to_string(is_alt) + " shortcut='" + shortcut + "'");

    if (shortcut == "alt_z" && is_alt && !is_ctrl && (key.sym() == FcitxKey_z || key.sym() == FcitxKey_Z)) {
        engine_->toggleAppEnabled(app);
        reset(/*force=*/true);
        ic_->updateUserInterface(fcitx::UserInterfaceComponent::StatusArea, true);
        if (engine_->instance() && std::string(ic_->frontend()) != "mock") {
#ifdef FCITX5_HAVE_SHOW_CUSTOM_IM_INFO
            engine_->instance()->showCustomInputMethodInformation(ic_, engine_->isAppEnabled(app) ? "VI" : "EN");
#else
            engine_->instance()->showInputMethodInformation(ic_);
#endif
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

    if (isSteam() && !key.isModifier()) {
        if (steam_pipeline_.isDuplicateKey(sym, fcitx::now(CLOCK_MONOTONIC))) {
            keyEvent.filterAndAccept();
            return;
        }
    }

    bool is_cursor_move = key.isCursorMove() || (sym >= FcitxKey_Home && sym <= FcitxKey_End);
    bool is_special_nav = sym == FcitxKey_Escape ||
                          sym == FcitxKey_Delete || sym == FcitxKey_KP_Delete ||
                          sym == FcitxKey_Tab || sym == FcitxKey_KP_Tab || sym == FcitxKey_ISO_Left_Tab;

    if ((is_ctrl && !is_alt && (sym == FcitxKey_a || sym == FcitxKey_A)) ||
        (is_shift && is_cursor_move)) {
        last_selection_time_us_ = fcitx::now(CLOCK_MONOTONIC);
    }

    bool is_clean_backspace = (sym == FcitxKey_BackSpace) && !has_ctrl_alt && !is_shift;
    if (is_clean_backspace && in_flight_sentinel_count_ > 0 &&
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
        finishUinputDeletion();
        return;
    }

    if (has_ctrl_alt || is_cursor_move || is_special_nav) {
        repeat_handler_.cancel();
        steam_pipeline_.cancelTimer();
        if (is_deleting_) {
            if (safety_timer_) {
                safety_timer_.reset();
            }
            is_deleting_ = false;
            is_address_bar_fix_ = false;
            is_selection_deletion_ = false;
            expected_backspaces_ = 0;
            current_backspace_count_ = 0;
            in_flight_sentinel_count_ = 0;
            sentinel_grace_until_us_ = 0;
            pending_commit_string_.clear();
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
        return;
    }

    if (key.isModifier()) {
        repeat_handler_.cancel();
        return;
    }

    if (is_deleting_) {
        key_buffer_.push(key);
        keyEvent.filterAndAccept();
        return;
    }

    if (sym == FcitxKey_BackSpace) {
        last_selection_time_us_ = 0;
    }

    if (repeat_handler_.heldKey().sym() != 0 && key.sym() != repeat_handler_.heldKey().sym()) {
        repeat_handler_.cancel();
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

} // namespace ime
} // namespace clak
