#include "key_buffer.h"
#include "config/config.h"
#include "utils/log.h"
#include <fcitx-utils/keysym.h>

namespace clak {
namespace ime {

bool KeyBuffer::push(const fcitx::Key& key) {
    if (keys_.size() < config::kMaxBufferedKeys) {
        keys_.push_back(key);
        return true;
    }
    return false;
}

void KeyBuffer::replay(const std::function<bool(const fcitx::Key&)>& handle_key_fn,
                       const std::function<void(const std::string&)>& commit_fn,
                       const std::function<void(const fcitx::Key&)>& forward_fn,
                       const std::function<bool()>& is_deleting_fn) {
    if (keys_.empty()) {
        return;
    }
    auto pending = std::move(keys_);
    keys_.clear();
    utils::clakLog("replayBufferedKeys: " + std::to_string(pending.size()) + " keys");

    std::string batch_commit;
    auto flush_batch = [&batch_commit, &commit_fn]() {
        if (!batch_commit.empty()) {
            utils::clakLog("replay batch commit: '" + batch_commit + "'");
            if (commit_fn) {
                commit_fn(batch_commit);
            }
            batch_commit.clear();
        }
    };

    for (size_t i = 0; i < pending.size(); ++i) {
        const auto& k = pending[i];
        if (is_deleting_fn && is_deleting_fn()) {
            flush_batch();
            keys_.push_back(k);
            continue;
        }
        if (!handle_key_fn(k)) {
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
                if (forward_fn) {
                    forward_fn(k);
                }
            }
        } else {
            flush_batch();
        }
    }
    flush_batch();
}

} // namespace ime
} // namespace clak
