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
  void replay(const std::string& initial_batch,
              const std::function<bool(const fcitx::Key&)>& handle_key_fn,
              const std::function<void(const std::string&)>& commit_fn,
              const std::function<void(const fcitx::Key&)>& forward_fn,
              const std::function<bool()>& is_deleting_fn,
              const std::function<void(std::function<void()>)>& set_flush_hook = nullptr);

  size_t size() const { return keys_.size(); }
  bool empty() const { return keys_.empty(); }
  void clear() { keys_.clear(); }
  size_t peak_depth() const { return peak_depth_; }
  size_t overflow_count() const { return overflow_count_; }
  void reset_metrics() { peak_depth_ = keys_.size(); overflow_count_ = 0; }
  const std::vector<fcitx::Key>& keys() const { return keys_; }
  std::vector<fcitx::Key> take_keys() {
    auto ret = std::move(keys_);
    keys_.clear();
    return ret;
  }

private:
  std::vector<fcitx::Key> keys_;
  size_t peak_depth_{0};
  size_t overflow_count_{0};
};

} // namespace ime
} // namespace clak

#endif
