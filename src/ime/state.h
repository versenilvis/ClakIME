#ifndef CLAK_IME_STATE_H
#define CLAK_IME_STATE_H

#include <fcitx/inputcontext.h>
#include <fcitx/inputcontextproperty.h>
#include <fcitx-utils/event.h>
#include <string>
#include <memory>
#include "core.h"
#include "repeat_handler.h"
#include "modal_handler.h"
#include "surrounding_verifier.h"
#include "key_buffer.h"
#include "steam_pipeline.h"

namespace clak {

class ClakEngine;

namespace ime {

class ClakState : public fcitx::InputContextProperty {
public:
  ClakState(ClakEngine* engine, fcitx::InputContext* ic);
  ~ClakState() override;

  void keyEvent(fcitx::KeyEvent& keyEvent);
  void reset(bool force = false);
  bool isBrowser() const;
  bool isGecko() const;
  bool isSteam() const;
  std::string appKey();
  void syncConfig();
  bool shouldUseUinput(bool use_surrounding, uint32_t action_type, const fcitx::SurroundingText& surr);
  bool isAutofillCertain(const fcitx::SurroundingText& surr);

  bool isDeleting() const { return is_deleting_; }
  bool isSelectionDeletion() const { return is_selection_deletion_; }
  uint64_t safetyTimerTime() const { return safety_timer_ ? safety_timer_->time() : 0; }
  bool isRichTextEditor() const { return verifier_.isRichTextEditor(); }
  bool isDraftJsEditor() const;
  int mismatchCount() const { return verifier_.mismatchCount(); }
  size_t expectedBackspaces() const { return expected_backspaces_; }
  size_t bufferedKeysCount() const { return key_buffer_.size(); }
  size_t currentBackspaceCount() const { return current_backspace_count_; }
  bool allRealBackspacesReceived() const {
    return is_deleting_ && expected_backspaces_ > 0 && current_backspace_count_ >= (expected_backspaces_ - 1);
  }
  void finishUinputDeletion();
  const std::string& pendingCommitString() const { return pending_commit_string_; }
  uint64_t adaptiveExtraWaitUs() const { return adaptive_extra_us_; }
  void observeTransactionLatency(uint64_t elapsed_us);
  void onRepeatTimer() { repeat_handler_.onTimer(); }
  bool isRepeating() const { return repeat_handler_.isRepeating(); }
  fcitx::Key heldKey() const { return repeat_handler_.heldKey(); }
  void setEnableRepeatForMock(bool enable) { repeat_handler_.setEnableRepeatForMock(enable); }

private:
  void arm_safety_timer();
  bool handleKey(const fcitx::Key& key);
  void replayBufferedKeys();
  std::string activeSite();
  void doCommitString(const std::string& text);
  std::string classifyGroup(const std::string& app, const std::string& site, bool is_autofill, bool used_uinput);
  void logLatency(const std::string& group, uint64_t start_us, const std::string& action_type);

  ClakEngine* engine_;
  fcitx::InputContext* ic_;
  ClakContext* rust_ctx_{nullptr};
  uint64_t applied_config_version_{0};
  uint64_t last_commit_time_us_{0};

  bool is_deleting_{false};
  bool is_address_bar_fix_{false};
  bool is_selection_deletion_{false};
  uint64_t adaptive_extra_us_{0};
  uint32_t stable_transactions_count_{0};
  size_t expected_backspaces_{0};
  size_t current_backspace_count_{0};
  size_t in_flight_sentinel_count_{0};
  uint64_t sentinel_grace_until_us_{0};
  size_t last_text_len_{0};
  uint64_t op_start_us_{0};
  std::string op_group_;

  std::string pending_commit_string_;
  std::unique_ptr<fcitx::EventSourceTime> safety_timer_;

  uint64_t last_site_check_us_{0};
  std::string cached_site_;
  bool ctrl_pressed_first_{false};
  bool ctrl_shift_armed_{false};
  uint64_t last_selection_time_us_{0};

  RepeatHandler repeat_handler_;
  ModalHandler modal_handler_;
  SurroundingVerifier verifier_;
  KeyBuffer key_buffer_;
  SteamPipeline steam_pipeline_;
};

} // namespace ime
} // namespace clak

#endif
