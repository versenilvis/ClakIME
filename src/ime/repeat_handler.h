#ifndef CLAK_IME_REPEAT_HANDLER_H
#define CLAK_IME_REPEAT_HANDLER_H

#include <fcitx-utils/event.h>
#include <fcitx-utils/key.h>
#include <functional>
#include <memory>
#include <cstdint>

namespace clak {
namespace ime {

class RepeatHandler {
public:
  RepeatHandler() = default;
  ~RepeatHandler();

  void arm(fcitx::EventLoop& event_loop, const fcitx::Key& key, std::function<void(const fcitx::Key&)> callback);
  void cancel();
  void onTimer();
  void defer(uint64_t delay_us);

  bool isRepeating() const { return is_repeating_; }
  fcitx::Key heldKey() const { return held_key_; }
  void setEnableRepeatForMock(bool enable) { enable_repeat_for_mock_ = enable; }
  bool enableRepeatForMock() const { return enable_repeat_for_mock_; }

  static uint64_t repeatDelayUs();
  static uint64_t repeatIntervalUs();

private:
  std::unique_ptr<fcitx::EventSourceTime> repeat_timer_;
  fcitx::Key held_key_;
  bool is_repeating_{false};
  bool enable_repeat_for_mock_{false};
  std::function<void(const fcitx::Key&)> callback_;
};

} // namespace ime
} // namespace clak

#endif
