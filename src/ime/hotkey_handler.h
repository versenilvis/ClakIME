#ifndef CLAK_IME_HOTKEY_HANDLER_H
#define CLAK_IME_HOTKEY_HANDLER_H

#include <fcitx/inputcontext.h>
#include <fcitx/instance.h>
#include <string>
#include <cstdint>
#include <functional>
#include "apps/app_profile.h"

namespace clak {

class ClakEngine;

namespace ime {

class HotkeyHandler {
public:
    HotkeyHandler(
        ClakEngine* engine,
        fcitx::InputContext* ic,
        std::function<void(const std::string&)> do_commit_string,
        std::function<void(bool)> reset_state,
        std::function<apps::AppProfile*()> get_profile
    );

    bool handleKeyEvent(fcitx::KeyEvent& keyEvent, const std::string& app);
    bool triggerShiftAutoFix();
    void onMouseClicked();
    void onReset();
    void reset();

    void clearAutofix();
    void recordAutofix(const std::string& original, size_t converted_chars, uint64_t time_us);
    bool tryUndo(fcitx::InputContext* ic);

private:
    ClakEngine* engine_;
    fcitx::InputContext* ic_;
    std::function<void(const std::string&)> do_commit_string_;
    std::function<void(bool)> reset_state_;
    std::function<apps::AppProfile*()> get_profile_;

    bool ctrl_pressed_first_{false};
    bool ctrl_shift_armed_{false};
    bool alt_pressed_first_{false};
    bool alt_shift_armed_{false};

    uint64_t shift_press_time_us_{0};
    bool shift_other_key_pressed_{false};
    bool shift_mouse_clicked_{false};
    uint64_t last_autofix_time_us_{0};
    std::string last_autofix_original_;
    size_t last_autofix_converted_chars_{0};
};

} // namespace ime
} // namespace clak

#endif
