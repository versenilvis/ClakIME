#include "hotkey_handler.h"
#include "engine.h"
#include "utils/log.h"
#include "utils/text_utils.h"
#include "core.h"
#include <fcitx-utils/keysym.h>
#include <fcitx-utils/event.h>
#include <dlfcn.h>
#ifdef CLAK_HAVE_FCITX5_CLIPBOARD
#include <fcitx/module/clipboard/clipboard.h>
#endif

namespace {
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

HotkeyHandler::HotkeyHandler(
    ClakEngine* engine,
    fcitx::InputContext* ic,
    std::function<void(const std::string&)> do_commit_string,
    std::function<void(bool)> reset_state,
    std::function<apps::AppProfile*()> get_profile
) : engine_(engine),
    ic_(ic),
    do_commit_string_(std::move(do_commit_string)),
    reset_state_(std::move(reset_state)),
    get_profile_(std::move(get_profile)) {}

bool HotkeyHandler::handleKeyEvent(fcitx::KeyEvent& keyEvent, const std::string& app) {
    const auto& key = keyEvent.key();
    bool is_ctrl = key.states().test(fcitx::KeyState::Ctrl);
    bool is_shift = key.states().test(fcitx::KeyState::Shift);
    bool is_alt = key.states().test(fcitx::KeyState::Alt);
    bool is_super = key.states().test(fcitx::KeyState::Super);

    bool is_shift_sym = (key.sym() == FcitxKey_Shift_L || key.sym() == FcitxKey_Shift_R);
    bool is_ctrl_sym = (key.sym() == FcitxKey_Control_L || key.sym() == FcitxKey_Control_R);
    bool is_alt_sym = (key.sym() == FcitxKey_Alt_L || key.sym() == FcitxKey_Alt_R);

    std::string shortcut = "ctrl_shift";
    if (engine_->config()) {
        char* sc_c = clak_config_get_toggle_shortcut(engine_->config());
        if (sc_c) {
            std::string raw = sc_c;
            clak_free_string(sc_c);
            shortcut.clear();
            for (char c : raw) {
                if (c == '+' || c == ' ' || c == '-') {
                    if (!shortcut.empty() && shortcut.back() != '_') {
                        shortcut.push_back('_');
                    }
                } else {
                    shortcut.push_back(std::tolower(static_cast<unsigned char>(c)));
                }
            }
        }
    }
    if (shortcut.empty()) shortcut = "ctrl_shift";

    if (keyEvent.isRelease()) {
        if (is_shift_sym) {
            uint64_t now_us = fcitx::now(CLOCK_MONOTONIC);
            uint64_t hold_time_us = (shift_press_time_us_ > 0) ? (now_us - shift_press_time_us_) : 0;
            bool single_tap = (shift_press_time_us_ > 0) && (hold_time_us < 350000) && !shift_other_key_pressed_ && !shift_mouse_clicked_;
            shift_press_time_us_ = 0;
            shift_other_key_pressed_ = false;
            shift_mouse_clicked_ = false;

            if (single_tap && !ctrl_shift_armed_ && !alt_shift_armed_) {
                if (triggerShiftAutoFix()) {
                    return true;
                }
            }
        }
        if (shortcut == "ctrl_shift") {
            if (ctrl_shift_armed_ && (is_shift_sym || is_ctrl_sym)) {
                engine_->toggleAppEnabled(app);
                reset_state_(/*force=*/true);
                ic_->updateUserInterface(fcitx::UserInterfaceComponent::StatusArea, true);
                if (engine_->instance() && std::string(ic_->frontend()) != "mock") {
                    showInputMethodInfo(engine_->instance(), ic_, engine_->isAppEnabled(app) ? "VI" : "EN");
                }
                ctrl_shift_armed_ = false;
                ctrl_pressed_first_ = false;
                return true;
            } else if (is_ctrl_sym) {
                ctrl_pressed_first_ = false;
            }
        } else if (shortcut == "alt_shift") {
            if (alt_shift_armed_ && (is_shift_sym || is_alt_sym)) {
                engine_->toggleAppEnabled(app);
                reset_state_(/*force=*/true);
                ic_->updateUserInterface(fcitx::UserInterfaceComponent::StatusArea, true);
                if (engine_->instance() && std::string(ic_->frontend()) != "mock") {
                    showInputMethodInfo(engine_->instance(), ic_, engine_->isAppEnabled(app) ? "VI" : "EN");
                }
                alt_shift_armed_ = false;
                alt_pressed_first_ = false;
                return true;
            } else if (is_alt_sym) {
                alt_pressed_first_ = false;
            }
        }
        return false;
    }

    if (is_shift_sym) {
        if (shift_press_time_us_ == 0) {
            shift_press_time_us_ = fcitx::now(CLOCK_MONOTONIC);
            shift_other_key_pressed_ = false;
            shift_mouse_clicked_ = false;
        }
    } else {
        shift_other_key_pressed_ = true;
    }

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
        reset_state_(/*force=*/true);
        ic_->updateUserInterface(fcitx::UserInterfaceComponent::StatusArea, true);
        if (engine_->instance() && std::string(ic_->frontend()) != "mock") {
            showInputMethodInfo(engine_->instance(), ic_, engine_->isAppEnabled(app) ? "VI" : "EN");
        }
        keyEvent.filterAndAccept();
        return true;
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

    return false;
}

void HotkeyHandler::clearAutofix() {
    last_autofix_original_.clear();
    last_autofix_converted_chars_ = 0;
    last_autofix_time_us_ = 0;
}

void HotkeyHandler::recordAutofix(const std::string& original, size_t converted_chars, uint64_t time_us) {
    last_autofix_original_ = original;
    last_autofix_converted_chars_ = converted_chars;
    last_autofix_time_us_ = time_us;
}

bool HotkeyHandler::tryUndo(fcitx::InputContext* ic) {
    if (last_autofix_original_.empty() || last_autofix_time_us_ == 0) {
        return false;
    }
    uint64_t now_us = fcitx::now(CLOCK_MONOTONIC);
    if (now_us - last_autofix_time_us_ >= 5000000) {
        return false;
    }
    size_t converted_chars = last_autofix_converted_chars_;
    std::string restore_text = last_autofix_original_;
    clearAutofix();

    if (converted_chars > 0 && ic) {
        auto* profile = get_profile_();
        bool has_surrounding = ic->capabilityFlags().test(fcitx::CapabilityFlag::SurroundingText);
        const auto& surr = ic->surroundingText();
        bool valid_surr = has_surrounding && surr.isValid() && surr.cursor() <= surr.text().size();
        bool use_surrounding = valid_surr && (!profile || profile->allowsSurroundingText());

        if (profile && profile->erasesWithCommittedDel()) {
            std::string del_str(converted_chars, '\x7f');
            del_str += restore_text;
            do_commit_string_(del_str);
        } else if (use_surrounding && surr.cursor() >= converted_chars) {
            ic->deleteSurroundingText(-static_cast<int>(converted_chars), static_cast<unsigned int>(converted_chars));
            do_commit_string_(restore_text);
        } else {
            for (size_t i = 0; i < converted_chars; ++i) {
                ic->forwardKey(fcitx::Key(FcitxKey_BackSpace));
            }
            do_commit_string_(restore_text);
        }
        return true;
    }
    return false;
}

bool HotkeyHandler::triggerShiftAutoFix() {
    if (!engine_->config() || !clak_config_get_shift_autofix(engine_->config())) {
        return false;
    }
    if (!ic_) {
        return false;
    }
    if (ic_->capabilityFlags().test(fcitx::CapabilityFlag::Password)) {
        return false;
    }

    const auto& surr = ic_->surroundingText();
    uint64_t now_us = fcitx::now(CLOCK_MONOTONIC);
    unsigned int cur = surr.isValid() ? surr.cursor() : 0;
    unsigned int anch = surr.isValid() ? surr.anchor() : 0;
    auto* profile = get_profile_();
    bool is_address_bar = (profile && profile->name() == "address-bar") ||
                          (ic_->capabilityFlags().test(fcitx::CapabilityFlag::Url));

    // quick undo check within 2s of previous autofix
    if (!last_autofix_original_.empty() && last_autofix_time_us_ > 0 &&
        (now_us - last_autofix_time_us_ < 2000000)) {
        return tryUndo(ic_);
    }

    int method = clak_config_get_method(engine_->config());

    // selection mode
    std::string selected;
    if (surr.isValid() && cur != anch) {
        selected = utils::extractSelection(surr.text(), cur, anch);
    } else if (is_address_bar || !surr.isValid()) {
#ifdef CLAK_HAVE_FCITX5_CLIPBOARD
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
                do_commit_string_(fixed_str);
                return true;
            }
        }
        return false;
    }

    // word-before-cursor mode
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
            ic_->deleteSurroundingText(-static_cast<int>(word_chars), word_chars);
            do_commit_string_(fixed_str);
            return true;
        }
    }

    return false;
}

void HotkeyHandler::onMouseClicked() {
    shift_mouse_clicked_ = true;
}

void HotkeyHandler::onReset() {
    if (shift_press_time_us_ > 0) {
        shift_mouse_clicked_ = true;
    }
}

void HotkeyHandler::reset() {
    ctrl_pressed_first_ = false;
    ctrl_shift_armed_ = false;
    alt_pressed_first_ = false;
    alt_shift_armed_ = false;
    shift_press_time_us_ = 0;
    shift_other_key_pressed_ = false;
    shift_mouse_clicked_ = false;
}

} // namespace ime
} // namespace clak
