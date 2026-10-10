#ifndef CLAK_IME_KEY_BUFFER_H
#define CLAK_IME_KEY_BUFFER_H

#include <fcitx-utils/key.h>
#include <vector>
#include <functional>
#include <string>
#include <cstddef>

namespace clak {
namespace ime {

class KeyBuffer {
public:
  KeyBuffer() = default;

  bool push(const fcitx::Key& key);
  void replay(const std::function<bool(const fcitx::Key&)>& handle_key_fn,
              const std::function<void(const std::string&)>& commit_fn,
              const std::function<void(const fcitx::Key&)>& forward_fn,
              const std::function<bool()>& is_deleting_fn);

  size_t size() const { return keys_.size(); }
  bool empty() const { return keys_.empty(); }
  void clear() { keys_.clear(); }
  const std::vector<fcitx::Key>& keys() const { return keys_; }

private:
  std::vector<fcitx::Key> keys_;
};

} // namespace ime
} // namespace clak

#endif
