#include "mock_input_context.h"
#include "ime/state.h"
#include <fcitx-utils/utf8.h>

namespace clak {
namespace test {

MockInputContext::MockInputContext(fcitx::InputContextManager& mgr, const std::string& program)
    : fcitx::InputContext(mgr, program) {
    created();
    setSurroundingEnabled(true);
}

MockInputContext::~MockInputContext() {
    destroy();
}

void MockInputContext::commitStringImpl(const std::string& text) {
    commits.push_back(text);
    if (auto_update_surrounding && surroundingText().isValid()) {
        const std::string& cur_text = surroundingText().text();
        unsigned int cur = surroundingText().cursor();
        std::string new_text;
        auto it = cur_text.begin();
        for (unsigned int i = 0; i < cur && it != cur_text.end(); ++i) {
            uint32_t c = 0;
            it = fcitx::utf8::getNextChar(it, cur_text.end(), &c);
        }
        size_t byte_idx = std::distance(cur_text.begin(), it);
        new_text = cur_text.substr(0, byte_idx) + text + cur_text.substr(byte_idx);
        size_t added_chars = fcitx::utf8::length(text);
        surroundingText().setText(new_text, cur + added_chars, cur + added_chars);
    }
    if (on_commit) {
        on_commit(text);
    }
}

void MockInputContext::deleteSurroundingTextImpl(int offset, unsigned int size) {
    if (surroundingText().isValid() && offset < 0 && static_cast<unsigned int>(-offset) > surroundingText().cursor()) {
        throw std::runtime_error("Mutter assertion crash in meta_wayland_text_input_focus_delete_surrounding: offset (" +
                                 std::to_string(offset) + ") exceeds cursor (" +
                                 std::to_string(surroundingText().cursor()) + ")!");
    }
    deletions.push_back({offset, size});
    if (auto_update_surrounding && surroundingText().isValid()) {
        surroundingText().deleteText(offset, size);
    }
    if (on_delete) {
        on_delete(offset, size);
    }
}

void MockInputContext::forwardKeyImpl(const fcitx::ForwardKeyEvent& key) {
    forwarded_keys.push_back(key.key());
    if (auto_update_surrounding && surroundingText().isValid()) {
        std::string str = fcitx::Key::keySymToUTF8(key.key().sym());
        if (!str.empty() && key.key().sym() < 0xff00) {
            const std::string& cur_text = surroundingText().text();
            unsigned int cur = surroundingText().cursor();
            std::string new_text;
            auto it = cur_text.begin();
            for (unsigned int i = 0; i < cur && it != cur_text.end(); ++i) {
                uint32_t c = 0;
                it = fcitx::utf8::getNextChar(it, cur_text.end(), &c);
            }
            size_t byte_idx = std::distance(cur_text.begin(), it);
            new_text = cur_text.substr(0, byte_idx) + str + cur_text.substr(byte_idx);
            size_t added_chars = fcitx::utf8::length(str);
            surroundingText().setText(new_text, cur + added_chars, cur + added_chars);
        }
    }
    if (on_forward_key) {
        on_forward_key(key);
    }
}

void MockInputContext::updatePreeditImpl() {
}

void MockInputContext::setSurrounding(const std::string& text, unsigned int cursor, unsigned int anchor) {
    surroundingText().setText(text, cursor, anchor);
}

void MockInputContext::invalidateSurrounding() {
    surroundingText().invalidate();
}

void MockInputContext::setSurroundingEnabled(bool enabled) {
    auto flags = capabilityFlags();
    if (enabled) {
        flags |= fcitx::CapabilityFlag::SurroundingText;
    } else {
        flags &= ~fcitx::CapabilityFlags(fcitx::CapabilityFlag::SurroundingText);
    }
    setCapabilityFlags(flags);
}

void MockInputContext::setUrlCapability(bool enabled) {
    auto flags = capabilityFlags();
    if (enabled) {
        flags |= fcitx::CapabilityFlag::Url;
    } else {
        flags &= ~fcitx::CapabilityFlags(fcitx::CapabilityFlag::Url);
    }
    setCapabilityFlags(flags);
}

bool MockInputContext::sendKey(fcitx::KeySym sym, fcitx::KeyStates states, bool isRelease, ime::ClakState* state) {
    fcitx::KeyEvent event(this, fcitx::Key(sym, states), isRelease);
    if (state) {
        state->keyEvent(event);
    } else {
        keyEvent(event);
    }
    if (!event.filtered() && !isRelease && auto_update_surrounding && surroundingText().isValid()) {
        std::string str = fcitx::Key::keySymToUTF8(sym);
        if (!str.empty() && sym < 0xff00) {
            const std::string& cur_text = surroundingText().text();
            unsigned int cur = surroundingText().cursor();
            std::string new_text;
            auto it = cur_text.begin();
            for (unsigned int i = 0; i < cur && it != cur_text.end(); ++i) {
                uint32_t c = 0;
                it = fcitx::utf8::getNextChar(it, cur_text.end(), &c);
            }
            size_t byte_idx = std::distance(cur_text.begin(), it);
            new_text = cur_text.substr(0, byte_idx) + str + cur_text.substr(byte_idx);
            size_t added_chars = fcitx::utf8::length(str);
            surroundingText().setText(new_text, cur + added_chars, cur + added_chars);
        }
    }
    return event.filtered();
}

bool MockInputContext::typeChar(char c, ime::ClakState* state) {
    fcitx::KeySym sym = static_cast<fcitx::KeySym>(static_cast<unsigned char>(c));
    return sendKey(sym, fcitx::KeyStates(), false, state);
}

bool MockInputContext::typeString(const std::string& str, ime::ClakState* state) {
    bool any_filtered = false;
    for (char c : str) {
        if (typeChar(c, state)) {
            any_filtered = true;
        }
    }
    return any_filtered;
}

bool MockInputContext::sendCleanBackspace(ime::ClakState* state) {
    return sendKey(FcitxKey_BackSpace, fcitx::KeyStates(), false, state);
}

void MockInputContext::clearHistory() {
    commits.clear();
    deletions.clear();
    forwarded_keys.clear();
}

} // namespace test
} // namespace clak
