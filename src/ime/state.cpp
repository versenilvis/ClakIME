#include "state.h"
#include "engine.h"
#include "config/config.h"
#include "config/sites.h"
#include "platform/window_info.h"
#include "platform/modal_editor.h"
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
        // guard against false reset doi nguoc tu web app trong 50ms post-commit
        uint64_t now_us = fcitx::now(CLOCK_MONOTONIC);
        if (last_commit_time_us_ > 0 && (now_us - last_commit_time_us_) < 50000) {
            utils::clakLog("reset: ignored false reset within 50ms post-commit window (delta=" +
                           std::to_string(now_us - last_commit_time_us_) + "us) app=" + app + " site='" + site + "'");
            return;
        }
        if (!buffered_keys_.empty()) {
            utils::clakLog("reset: ignored reset while keys buffered (count=" +
                           std::to_string(buffered_keys_.size()) + ") app=" + app + " site='" + site + "'");
            return;
        }
    }
    last_editor_check_us_ = 0;
    last_text_len_ = 0;
    if (safety_timer_) {
        safety_timer_.reset();
    }
    cancelRepeatTimer();
    if (!buffered_keys_.empty() || !pending_commit_string_.empty()) {
        utils::clakLog("reset: clearing state (buf_len=" + std::to_string(buffered_keys_.size()) + ") app=" + app + " site='" + site + "'");
    }
    is_deleting_ = false;
    is_address_bar_fix_ = false;
    is_selection_deletion_ = false;
    expected_backspaces_ = 0;
    current_backspace_count_ = 0;
    in_flight_sentinel_count_ = 0;
    sentinel_grace_until_us_ = 0;
    pending_commit_string_.clear();
    buffered_keys_.clear();
    verify_.pending = false;
    is_canvas_editor_ = false;
    is_rich_text_editor_ = false;
    is_draftjs_editor_ = false;
    mismatch_count_ = 0;
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

bool ClakState::isDraftJsEditor() const {
    std::string site = const_cast<ClakState*>(this)->activeSite();
    return is_draftjs_editor_ || config::isDraftJsSite(site);
}

bool ClakState::isCursorNearWord(const fcitx::SurroundingText& surr) {
    if (!surr.isValid()) return false;
    const std::string& text = surr.text();
    if (text.empty()) return false;
    unsigned int cursor = surr.cursor();
    size_t utf8_len = fcitx::utf8::length(text);
    if (cursor >= utf8_len) return false;

    // skip to character at cursor offset
    auto it = text.begin();
    for (unsigned int i = 0; i < cursor && it != text.end(); ++i) {
        uint32_t chr = 0;
        it = fcitx::utf8::getNextChar(it, text.end(), &chr);
    }
    // check remaining characters after cursor
    while (it != text.end()) {
        uint32_t chr = 0;
        it = fcitx::utf8::getNextChar(it, text.end(), &chr);
        if (chr != ' ' && chr != '\t' && chr != '\n' && chr != '\r') {
            return true;
        }
    }
    return false;
}

bool isWpsOfficeApp(const std::string& app) {
    return app == "wps" || app == "wpp" || app == "et" || app == "wpspdf" || app == "wpsoffice" ||
           app.find("wps") != std::string::npos;
}

static bool isWpsFontSizeText(const std::string& text) {
    if (text.empty() || text.size() > 4) return false;
    for (char c : text) {
        if (!isdigit(static_cast<unsigned char>(c)) && c != '.') return false;
    }
    return true;
}

bool ClakState::shouldUseUinput(bool use_surrounding, uint32_t action_type, const fcitx::SurroundingText& /*surr*/) {
    if (is_modal_editor_) {
        return true;
    }

    std::string app = appKey();
    std::string site = activeSite();

    // wps office on xwayland ignores dbus forwardKey, require reliable uinput
    if (isWpsOfficeApp(app)) {
        return true;
    }

    if (config::isTerminalApp(app)) {
        return true;
    }
    // gecko apps (zen, firefox) require paced uinput to avoid wayland delete_surrounding_text bugs
    if (config::isGeckoApp(app)) {
        return true;
    }
    // meta sites (facebook, messenger, instagram...) rely strictly on surrounding text
    if (config::isMetaSite(site) || config::isMetaSite(app)) {
        return false;
    }
    if (action_type == CLAK_ACTION_ADDRESS_BAR_FIX) {
        return true;
    }
    if (config::isForceUinputSite(site) || isDraftJsEditor()) {
        return true;
    }
    if (is_canvas_editor_ || is_rich_text_editor_) {
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
    // address bar autofill happens in single-line context without newlines
    if (text.empty() || text.find('\n') != std::string::npos) return false;

    unsigned int cursor = surr.cursor();
    unsigned int anchor = surr.anchor();

    // selection extends past cursor through to line end in single-line context (chromium address bar autocomplete)
    if (cursor != anchor) {
        unsigned int sel_start = std::min(anchor, cursor);
        unsigned int sel_end = std::max(anchor, cursor);
        if (sel_end == text.length() && sel_start > 0) {
            return true;
        }
    }

    return false;
}

void ClakState::setVerifyExpectation(const std::string& wordBefore, size_t delChars, const std::string& added) {
    // do not verify multi-word or boundary-terminated strings
    if (added.find(' ') != std::string::npos || added.find('\n') != std::string::npos || added.find('\t') != std::string::npos) {
        verify_.pending = false;
        return;
    }
    verify_.pending = true;
    verify_.preWord = wordBefore;
    verify_.del = delChars;
    verify_.added = added;

    std::string base = wordBefore;
    utils::popUtf8Chars(base, delChars);
    verify_.expectWord = base + added;
}

void ClakState::verifySurrounding(const fcitx::SurroundingText& surr) {
    if (!verify_.pending) return;
    verify_.pending = false;

    std::string actualWord = utils::extractWordBeforeCursor(surr.text(), surr.cursor());

    if (actualWord == verify_.expectWord) {
        mismatch_count_ = 0;
        is_rich_text_editor_ = false;
        is_draftjs_editor_ = false;
    } else {
        mismatch_count_++;
        utils::clakLog("verifySurrounding mismatch #" + std::to_string(mismatch_count_) +
                       ": expected='" + verify_.expectWord + "' actual='" + actualWord + "'");
        // if delete was completely ignored in rich text dom, switch immediately
        bool delete_ignored = (!verify_.preWord.empty() && actualWord.find(verify_.preWord) != std::string::npos);
        if (delete_ignored) {
            is_draftjs_editor_ = true;
            is_rich_text_editor_ = true;
            utils::clakLog("verifySurrounding: detected draftjs / react contenteditable editor, switched to uinput");
        } else if (mismatch_count_ >= config::kMismatchThreshold) {
            is_rich_text_editor_ = true;
            utils::clakLog("verifySurrounding: auto-switched to uinput for this editor");
        }
    }
}

void ClakState::doCommitString(const std::string& text) {
    if (text.empty()) return;
    last_commit_time_us_ = fcitx::now(CLOCK_MONOTONIC);
    ic_->commitString(text);
}

std::string ClakState::classifyGroup(const std::string& app, const std::string& site, bool is_autofill, bool used_uinput) {
    bool has_url_cap = ic_ && ic_->capabilityFlags().test(fcitx::CapabilityFlag::Url);
    if (is_autofill || has_url_cap) return "address-bar";
    if (isWpsOfficeApp(app)) return "WPS-Office";
    if (config::isJetBrainsApp(app)) return "JetBrains-Uinput";
    if (config::isTerminalApp(app)) return "Terminal-Uinput";
    if (site == "docs.google.com" || site.find("docs.google.com") != std::string::npos) return "Docs-Uinput";
    if (isDraftJsEditor()) return "DraftJS-Uinput";
    if (config::isGeckoApp(app)) return "Gecko-Uinput";
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
    // scale up extra wait when roundtrip latency approaches or exceeds base threshold (35ms)
    if (elapsed_us >= 35000) {
        if (adaptive_extra_us_ < 100000) {
            adaptive_extra_us_ = std::min<uint64_t>(100000, adaptive_extra_us_ + 10000);
        }
        stable_transactions_count_ = 0;
    } else if (elapsed_us <= 20000) {
        // decay extra wait back towards 0 after 4 consecutive fast transactions
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
                // system lag caused timeout, increase adaptive wait
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

uint64_t ClakState::repeatDelayUs() {
    static uint64_t cached_delay = 0;
    if (cached_delay > 0) return cached_delay;
    if (getenv("HYPRLAND_INSTANCE_SIGNATURE")) {
        FILE* fp = popen("hyprctl getoption input:repeat_delay 2>/dev/null", "r");
        if (fp) {
            char buf[128];
            while (fgets(buf, sizeof(buf), fp)) {
                int val = 0;
                if (sscanf(buf, "int: %d", &val) == 1 && val > 0) {
                    cached_delay = static_cast<uint64_t>(val) * 1000;
                    break;
                }
            }
            pclose(fp);
        }
    }
    if (cached_delay == 0) {
        cached_delay = 200000;
    }
    utils::clakLog("repeat delay initialized: " + std::to_string(cached_delay) + "us");
    return cached_delay;
}

uint64_t ClakState::repeatIntervalUs() {
    static uint64_t cached_interval = 0;
    if (cached_interval > 0) return cached_interval;
    if (getenv("HYPRLAND_INSTANCE_SIGNATURE")) {
        FILE* fp = popen("hyprctl getoption input:repeat_rate 2>/dev/null", "r");
        if (fp) {
            char buf[128];
            while (fgets(buf, sizeof(buf), fp)) {
                int val = 0;
                if (sscanf(buf, "int: %d", &val) == 1 && val > 0) {
                    cached_interval = 1000000 / val;
                    break;
                }
            }
            pclose(fp);
        }
    }
    if (cached_interval == 0) {
        cached_interval = 25000;
    }
    utils::clakLog("repeat interval initialized: " + std::to_string(cached_interval) + "us");
    return cached_interval;
}

void ClakState::armRepeatTimer(const fcitx::Key& key) {
    if (is_repeating_ && held_key_.sym() == key.sym()) {
        return;
    }
    held_key_ = key;
    is_repeating_ = false;
    uint64_t now_us = fcitx::now(CLOCK_MONOTONIC);
    uint64_t delay_us = repeatDelayUs();
    // use 1000us accuracy to prevent sd-event default 250ms slack coalescing
    repeat_timer_ = engine_->instance()->eventLoop().addTimeEvent(
        CLOCK_MONOTONIC,
        now_us + delay_us,
        1000,
        [this](fcitx::EventSourceTime*, uint64_t) {
            onRepeatTimer();
            return true;
        }
    );
}

void ClakState::cancelRepeatTimer() {
    if (repeat_timer_) {
        repeat_timer_.reset();
    }
    held_key_ = fcitx::Key();
    is_repeating_ = false;
}

void ClakState::onRepeatTimer() {
    if (held_key_.sym() == 0) return;
    if (is_deleting_) {
        // defer if deletion is in flight
        if (repeat_timer_) {
            uint64_t now_us = fcitx::now(CLOCK_MONOTONIC);
            repeat_timer_->setTime(now_us + 10000);
            repeat_timer_->setAccuracy(1000);
            repeat_timer_->setOneShot();
        }
        return;
    }

    if (!is_repeating_) {
        utils::clakLog("onRepeatTimer: start repeat sym=" + std::to_string(held_key_.sym()));
        is_repeating_ = true;
    }

    if (!handleKey(held_key_)) {
        // engine forwarded raw key, commit directly in timer repeat
        std::string str = fcitx::Key::keySymToUTF8(held_key_.sym());
        if (!str.empty()) {
            doCommitString(str);
        }
    }

    if (repeat_timer_ && held_key_.sym() != 0) {
        uint64_t now_us = fcitx::now(CLOCK_MONOTONIC);
        repeat_timer_->setTime(now_us + repeatIntervalUs());
        repeat_timer_->setAccuracy(1000);
        repeat_timer_->setOneShot();
    }
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
    if (buffered_keys_.empty()) {
        return;
    }
    auto keys = std::move(buffered_keys_);
    buffered_keys_.clear();
    utils::clakLog("replayBufferedKeys: " + std::to_string(keys.size()) + " keys");

    std::string batch_commit;
    auto flush_batch = [this, &batch_commit]() {
        if (!batch_commit.empty()) {
            utils::clakLog("replay batch commit: '" + batch_commit + "'");
            doCommitString(batch_commit);
            batch_commit.clear();
        }
    };

    for (size_t i = 0; i < keys.size(); ++i) {
        const auto& k = keys[i];
        if (is_deleting_) {
            flush_batch();
            buffered_keys_.push_back(k);
            continue;
        }
        if (!handleKey(k)) {
            bool has_ctrl_alt = k.states().test(fcitx::KeyState::Ctrl) ||
                                k.states().test(fcitx::KeyState::Alt) ||
                                k.states().test(fcitx::KeyState::Super);
            std::string str = fcitx::Key::keySymToUTF8(k.sym());
            if (!has_ctrl_alt && !str.empty() && k.sym() < 0xff00) {
                // batch consecutive raw printable characters to minimize wayland round-trips
                batch_commit += str;
            } else {
                flush_batch();
                utils::clakLog("replay forward raw: sym=" + std::to_string(k.sym()));
                ic_->forwardKey(k);
            }
        } else {
            flush_batch();
        }
    }
    flush_batch();
}

void ClakState::updateModalEditorStatus() {
    uint64_t now_us = fcitx::now(CLOCK_MONOTONIC);
    if (now_us - last_editor_check_us_ < config::kCacheCheckIntervalUs) {
        return;
    }
    last_editor_check_us_ = now_us;

    std::string fallback = (ic_ && !ic_->program().empty()) ? ic_->program() : "";
    platform::WindowInfo win = platform::getActiveWindow(fallback);
    bool was_editor = is_modal_editor_;
    is_modal_editor_ = platform::isEditorActive(win);
    if (!was_editor && is_modal_editor_) {
        editor_mode_ = EditorMode::NORMAL;
        utils::clakLog("modal editor activated: class='" + win.win_class + "' title='" + win.win_title + "' pid=" + std::to_string(win.pid) + " -> mode: NORMAL");
    } else if (was_editor && !is_modal_editor_) {
        utils::clakLog("modal editor deactivated: class='" + win.win_class + "'");
    }
}

bool ClakState::handleKey(const fcitx::Key& key) {
    if (key.isModifier()) {
        return false;
    }

    bool has_ctrl_alt = key.states().test(fcitx::KeyState::Ctrl) ||
                        key.states().test(fcitx::KeyState::Alt) ||
                        key.states().test(fcitx::KeyState::Super);
    bool has_ctrl = key.states().test(fcitx::KeyState::Ctrl);

    std::string key_str = fcitx::Key::keySymToUTF8(key.sym());
    uint32_t sym = key.sym();

    updateModalEditorStatus();

    if (is_modal_editor_) {
        if (editor_mode_ == EditorMode::INSERT) {
            if (sym == FcitxKey_Escape ||
                (has_ctrl && (sym == FcitxKey_bracketleft || sym == FcitxKey_c || sym == FcitxKey_C))) {
                editor_mode_ = EditorMode::NORMAL;
                reset();
                utils::clakLog("editor mode -> NORMAL (via " + key_str + ")");
                return false;
            }
        } else if (editor_mode_ == EditorMode::COMMAND) {
            if (sym == FcitxKey_Return || sym == FcitxKey_KP_Enter ||
                sym == FcitxKey_Escape ||
                (has_ctrl && (sym == FcitxKey_bracketleft || sym == FcitxKey_c || sym == FcitxKey_C))) {
                editor_mode_ = EditorMode::NORMAL;
                reset();
                utils::clakLog("editor mode -> NORMAL (via " + key_str + ")");
                return false;
            }
            reset();
            utils::clakLog("editor COMMAND: forward raw '" + key_str + "'");
            return false;
        } else {
            // normal mode
            if (!has_ctrl_alt) {
                if (sym == FcitxKey_colon || sym == FcitxKey_slash || sym == FcitxKey_question) {
                    editor_mode_ = EditorMode::COMMAND;
                    reset();
                    utils::clakLog("editor mode -> COMMAND (via " + key_str + ")");
                    return false;
                }
                if (sym == FcitxKey_i || sym == FcitxKey_I ||
                    sym == FcitxKey_a || sym == FcitxKey_A ||
                    sym == FcitxKey_o || sym == FcitxKey_O ||
                    sym == FcitxKey_c || sym == FcitxKey_C ||
                    sym == FcitxKey_s || sym == FcitxKey_S ||
                    sym == FcitxKey_R ||
                    sym == FcitxKey_Insert || sym == FcitxKey_KP_Insert) {
                    editor_mode_ = EditorMode::INSERT;
                    reset();
                    utils::clakLog("editor mode -> INSERT (via " + key_str + ")");
                    return false;
                }
            }
            reset();
            utils::clakLog("editor NORMAL: forward raw '" + key_str + "'");
            return false;
        }
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
        is_canvas_editor_ = false;
        is_rich_text_editor_ = false;
        is_draftjs_editor_ = false;
    } else if (!is_gecko && has_surrounding && surr.isValid()) {
        const std::string& text = surr.text();
        if (text == "  " || text == "\xc2\xa0\xc2\xa0") {
            is_canvas_editor_ = true;
        } else if (is_canvas_editor_ && text.size() > 2 && text != "  " && text != "\xc2\xa0\xc2\xa0") {
            is_canvas_editor_ = false;
        }
    }

    bool is_office = isWpsOfficeApp(app);
    bool is_wps_toolbar = is_office && isWpsFontSizeText(surr.text());
    // valid surrounding text to pass to rust engine (filter bogus wps font-size toolbar)
    bool valid_surr = has_surrounding && surr.isValid() && !is_canvas_editor_ && !is_term && !is_jb && !is_wps_toolbar;
    if (valid_surr) {
        surr_text = surr.text().c_str();
        cursor = surr.cursor();
        anchor = surr.anchor();
    }
    bool use_surrounding = valid_surr && !is_rich_text_editor_ && !is_force_uinput && !is_office;

    // gecko surrounding text is async so skip verify to avoid false mismatch
    bool skip_verify = is_gecko || is_meta;
    if (verify_.pending && use_surrounding && !skip_verify) {
        verifySurrounding(surr);
        is_draftjs = isDraftJsEditor();
        use_surrounding = valid_surr && !is_rich_text_editor_ && !is_force_uinput && !is_draftjs && !is_office;
    } else if (verify_.pending) {
        verify_.pending = false;
    }

    if (is_gecko && is_rich_text_editor_) {
        is_rich_text_editor_ = false;
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
            std::string del_str = (action.delete_str && action.delete_str[0] != '\0') ? action.delete_str : "";

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

                bool is_wps = isWpsOfficeApp(app);
                bool use_term_pacing = is_term || is_jb || is_modal_editor_;
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

            // surrounding text path
            if (real_bs > 0) {
                if (use_surrounding) {
                    std::string wordBefore = utils::extractWordBeforeCursor(surr.text(), surr.cursor());
                    setVerifyExpectation(wordBefore, real_bs, action.commit_str ? action.commit_str : "");
                    utils::clakLog("surrounding delete: -" + std::to_string(real_bs) + " commit='" +
                                   (action.commit_str ? action.commit_str : "") + "' app=" + app + " site='" + site + "'");
                    ic_->deleteSurroundingText(-static_cast<int>(real_bs),
                                               static_cast<unsigned int>(real_bs));
                } else {
                    utils::clakLog("forward backspace: count=" + std::to_string(real_bs) +
                                   " commit='" + (action.commit_str ? action.commit_str : "") + "' app=" + app);
                    for (size_t i = 0; i < real_bs; ++i) {
                        ic_->forwardKey(fcitx::Key(FcitxKey_BackSpace));
                    }
                }
            }
            if (action.commit_str && action.commit_str[0] != '\0') {
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
        if (held_key_.sym() != 0 && (key.sym() == held_key_.sym() || key.isModifier())) {
            cancelRepeatTimer();
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
            // any key other than ctrl or shift cancels armed toggle
            if (!is_ctrl_sym && !is_shift_sym) {
                ctrl_shift_armed_ = false;
            }
        } else if (is_ctrl_sym && !is_shift && !is_alt && !is_super) {
            // phase 1: ctrl pressed first cleanly without other modifiers held
            ctrl_pressed_first_ = true;
        } else if (is_shift_sym && ctrl_pressed_first_ && is_ctrl) {
            // phase 2: shift pressed strictly after clean ctrl
            ctrl_shift_armed_ = true;
            ctrl_pressed_first_ = false;
        } else {
            // any intervening key cancels phase 1
            ctrl_pressed_first_ = false;
        }
    }

    bool has_ctrl_alt = is_ctrl || is_alt || is_super;
    uint32_t sym = key.sym();

    if (!engine_->isAppEnabled(app)) {
        reset(/*force=*/true);
        return;
    }

    bool is_cursor_move = key.isCursorMove() || (sym >= FcitxKey_Home && sym <= FcitxKey_End);
    bool is_special_nav = sym == FcitxKey_Escape ||
                          sym == FcitxKey_Delete || sym == FcitxKey_KP_Delete ||
                          sym == FcitxKey_Tab || sym == FcitxKey_KP_Tab || sym == FcitxKey_ISO_Left_Tab;

    // track text selection shortcuts (ctrl+a, shift+arrows)
    if ((is_ctrl && !is_alt && (sym == FcitxKey_a || sym == FcitxKey_A)) ||
        (is_shift && is_cursor_move)) {
        last_selection_time_us_ = fcitx::now(CLOCK_MONOTONIC);
    }

    // uinput deletion sentinel check: only clean backspace from uinput loopback counts
    bool is_clean_backspace = (sym == FcitxKey_BackSpace) && !has_ctrl_alt && !is_shift;
    if (is_clean_backspace && in_flight_sentinel_count_ > 0 &&
        fcitx::now(CLOCK_MONOTONIC) <= sentinel_grace_until_us_) {
        in_flight_sentinel_count_--;
        keyEvent.filterAndAccept();
        return;
    }
    if (is_deleting_ && is_clean_backspace) {
        current_backspace_count_++;
        if (current_backspace_count_ < expected_backspaces_) {
            return;
        }
        keyEvent.filterAndAccept();
        finishUinputDeletion();
        return;
    }

    // abort deletion and reset state if user pressed a modifier shortcut or navigation key
    if (has_ctrl_alt || is_cursor_move || is_special_nav) {
        cancelRepeatTimer();
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
            buffered_keys_.clear();
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
        cancelRepeatTimer();
        return;
    }

    if (is_deleting_) {
        if (buffered_keys_.size() < config::kMaxBufferedKeys) {
            buffered_keys_.push_back(key);
        }
        keyEvent.filterAndAccept();
        return;
    }

    if (sym == FcitxKey_BackSpace) {
        last_selection_time_us_ = 0;
    }

    if (held_key_.sym() != 0 && key.sym() != held_key_.sym()) {
        cancelRepeatTimer();
    }

    if (!is_deleting_) {
        op_start_us_ = fcitx::now(CLOCK_MONOTONIC);
    }

    if (handleKey(key)) {
        keyEvent.filterAndAccept();
        // arm repeat timer when printable key is swallowed by ime
        if (sym >= 0x20 && sym < 0xff00 && !has_ctrl_alt && sym != FcitxKey_BackSpace &&
            ((!ic_ || std::string(ic_->frontend()) != "mock") || enable_repeat_for_mock_)) {
            armRepeatTimer(key);
        } else {
            cancelRepeatTimer();
        }
    } else {
        cancelRepeatTimer();
    }
}

} // namespace ime
} // namespace clak
