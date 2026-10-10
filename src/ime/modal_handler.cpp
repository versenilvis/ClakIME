#include "modal_handler.h"
#include "config/config.h"
#include "platform/window_info.h"
#include "platform/modal_editor.h"
#include "utils/log.h"
#include <fcitx-utils/event.h>
#include <fcitx-utils/keysym.h>

namespace clak {
namespace ime {

void ModalHandler::updateStatus(const std::string& fallback_program) {
    uint64_t now_us = fcitx::now(CLOCK_MONOTONIC);
    if (now_us - last_editor_check_us_ < config::kCacheCheckIntervalUs) {
        return;
    }
    last_editor_check_us_ = now_us;

    platform::WindowInfo win = platform::getActiveWindow(fallback_program);
    bool was_editor = is_modal_editor_;
    is_modal_editor_ = platform::isEditorActive(win);
    if (!was_editor && is_modal_editor_) {
        editor_mode_ = EditorMode::NORMAL;
        utils::clakLog("modal editor activated: class='" + win.win_class + "' title='" + win.win_title + "' pid=" + std::to_string(win.pid) + " -> mode: NORMAL");
    } else if (was_editor && !is_modal_editor_) {
        utils::clakLog("modal editor deactivated: class='" + win.win_class + "'");
    }
}

void ModalHandler::reset() {
    last_editor_check_us_ = 0;
}

bool ModalHandler::handleKey(const fcitx::Key& key, const std::function<void()>& on_reset) {
    if (!is_modal_editor_) {
        return false;
    }

    bool has_ctrl_alt = key.states().test(fcitx::KeyState::Ctrl) ||
                        key.states().test(fcitx::KeyState::Alt) ||
                        key.states().test(fcitx::KeyState::Super);
    bool has_ctrl = key.states().test(fcitx::KeyState::Ctrl);
    fcitx::KeySym sym = key.sym();
    std::string key_str = fcitx::Key::keySymToUTF8(sym);

    if (editor_mode_ == EditorMode::INSERT) {
        if (sym == FcitxKey_Escape ||
            (has_ctrl && (sym == FcitxKey_bracketleft || sym == FcitxKey_c || sym == FcitxKey_C))) {
            editor_mode_ = EditorMode::NORMAL;
            if (on_reset) on_reset();
            utils::clakLog("editor mode -> NORMAL (via " + key_str + ")");
            return true;
        }
        return false;
    }

    if (editor_mode_ == EditorMode::COMMAND) {
        if (sym == FcitxKey_Return || sym == FcitxKey_KP_Enter ||
            sym == FcitxKey_Escape ||
            (has_ctrl && (sym == FcitxKey_bracketleft || sym == FcitxKey_c || sym == FcitxKey_C))) {
            editor_mode_ = EditorMode::NORMAL;
            if (on_reset) on_reset();
            utils::clakLog("editor mode -> NORMAL (via " + key_str + ")");
            return true;
        }
        if (on_reset) on_reset();
        utils::clakLog("editor COMMAND: forward raw '" + key_str + "'");
        return true;
    }

    // normal mode
    if (!has_ctrl_alt) {
        if (sym == FcitxKey_colon || sym == FcitxKey_slash || sym == FcitxKey_question) {
            editor_mode_ = EditorMode::COMMAND;
            if (on_reset) on_reset();
            utils::clakLog("editor mode -> COMMAND (via " + key_str + ")");
            return true;
        }
        if (sym == FcitxKey_i || sym == FcitxKey_I ||
            sym == FcitxKey_a || sym == FcitxKey_A ||
            sym == FcitxKey_o || sym == FcitxKey_O ||
            sym == FcitxKey_c || sym == FcitxKey_C ||
            sym == FcitxKey_s || sym == FcitxKey_S ||
            sym == FcitxKey_R ||
            sym == FcitxKey_Insert || sym == FcitxKey_KP_Insert) {
            editor_mode_ = EditorMode::INSERT;
            if (on_reset) on_reset();
            utils::clakLog("editor mode -> INSERT (via " + key_str + ")");
            return true;
        }
    }
    if (on_reset) on_reset();
    utils::clakLog("editor NORMAL: forward raw '" + key_str + "'");
    return true;
}

} // namespace ime
} // namespace clak
