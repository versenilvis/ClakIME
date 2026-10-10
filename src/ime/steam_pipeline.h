#ifndef CLAK_IME_STEAM_PIPELINE_H
#define CLAK_IME_STEAM_PIPELINE_H

#include <fcitx/inputcontext.h>
#include <fcitx-utils/event.h>
#include <functional>
#include <memory>
#include <string>
#include <cstdint>

namespace clak {
namespace ime {

class SteamPipeline {
public:
  SteamPipeline() = default;
  ~SteamPipeline();

  bool isDuplicateKey(uint32_t sym, uint64_t now_us);
  void reset();

  void startStaggeredDeletion(
      fcitx::EventLoop& loop,
      fcitx::InputContext* ic,
      size_t count,
      std::string commit_str,
      std::function<void(const std::string&)> commit_fn,
      std::function<void()> on_completed
  );

  bool isTimerActive() const { return timer_ != nullptr; }
  void cancelTimer();

private:
  void scheduleStep(
      fcitx::EventLoop& loop,
      fcitx::InputContext* ic,
      size_t total_backspaces,
      size_t current_backspaces,
      std::string commit_str,
      std::function<void(const std::string&)> commit_fn,
      std::function<void()> on_completed
  );

  uint32_t last_sym_{0};
  uint64_t last_time_us_{0};
  std::unique_ptr<fcitx::EventSourceTime> timer_;
};

} // namespace ime
} // namespace clak

#endif
