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
#include "adaptive_pacing.h"
#include "apps/app_profile.h"

namespace clak {

class ClakEngine;

namespace ime {

class ClakState : public fcitx::InputContextProperty {
public:
  ClakState(ClakEngine* engine, fcitx::InputContext* ic);
  ~ClakState() override;

  void keyEvent(fcitx::KeyEvent& keyEvent);
  void reset(bool force = false);
  void resetHard();
  bool isBrowser() const;
  bool isGecko() const;
  bool isSteam() const;
  std::string appKey();
  void syncConfig();
  bool shouldUseUinput(bool use_surrounding, uint32_t action_type, const fcitx::SurroundingText& surr);
  bool isAutofillCertain(const fcitx::SurroundingText& surr);
  static bool looksLikeUrlAutofill(const fcitx::SurroundingText& surr);

  bool isDeleting() const { return rust_session_ ? clak_session_is_deleting(rust_session_) : false; }
  bool isSelectionDeletion() const { return is_selection_deletion_; }
  uint64_t safetyTimerTime() const { return safety_timer_ ? safety_timer_->time() : 0; }
  bool isRichTextEditor() const { return verifier_.isRichTextEditor(); }
  bool isDraftJsEditor() const;
  int mismatchCount() const { return verifier_.mismatchCount(); }
  size_t expectedBackspaces() const { return rust_session_ ? clak_session_expected_backspaces(rust_session_) : 0; }
  size_t bufferedKeysCount() const {
    size_t count = rust_session_ ? clak_session_buffered_keys_count(rust_session_) : 0;
    count += key_buffer_.size();
    return count;
  }
  size_t currentBackspaceCount() const { return rust_session_ ? clak_session_current_backspace_count(rust_session_) : 0; }
  bool allRealBackspacesReceived() const {
    size_t exp = expectedBackspaces();
    return isDeleting() && exp > 0 && currentBackspaceCount() >= (exp - 1);
  }
  void finishUinputDeletion(uint64_t rtt_us = 0);
  std::string pendingCommitString() const {
    const char* s = rust_session_ ? clak_session_pending_commit(rust_session_) : "";
    return s ? s : "";
  }
  uint64_t adaptiveExtraWaitUs() const { return adaptive_extra_us_; }
  void observeTransactionLatency(uint64_t elapsed_us);
  void onRepeatTimer() { repeat_handler_.onTimer(); }
  bool isRepeating() const { return repeat_handler_.isRepeating(); }
  fcitx::Key heldKey() const { return repeat_handler_.heldKey(); }
  void cancelRepeat() { repeat_handler_.cancel(); }
  void flushCollisionStats();
  void onDeactivate();
  void setEnableRepeatForMock(bool enable) { repeat_handler_.setEnableRepeatForMock(enable); }
  apps::AppProfile* currentProfile();
  pacing::AdaptivePacingEngine& pacingEngine() { return pacing_engine_; }
  const pacing::AdaptivePacingEngine& pacingEngine() const { return pacing_engine_; }

private:
  void executeSessionCommands(const ClakSessionCommandList& list);
  void handleSafetyTimeout();
  bool handleKey(const fcitx::Key& key);
  bool triggerShiftAutoFix();
  void replayBufferedKeys(const std::string& initial_batch = "");
  std::string activeSite();
  void doCommitString(const std::string& text);
  void doCommitDirect(const std::string& text);
  void forwardBackspacePressRelease(size_t count);
  bool scheduleForwardBackspaceFallback(size_t count, const std::string& commit_str, const std::string& app);
  std::string classifyGroup(const std::string& app, const std::string& site, bool is_autofill, bool used_uinput);
  void logLatency(const std::string& group, uint64_t start_us, const std::string& action_type, const std::string& layer = "");
  void logCsv(uint64_t rtt_us, const std::string& action);

  ClakEngine* engine_;
  fcitx::InputContext* ic_;
  ClakSession* rust_session_{nullptr};
  uint64_t applied_config_version_{0};
  uint64_t last_commit_time_us_{0};
  size_t uncommitted_forward_count_{0};

  struct CollisionStats {
    uint32_t total_replaces{0};
    uint32_t total_collisions{0};
    uint32_t missing_rtt_count{0};
    uint32_t buffered_key_interruptions{0};
    uint32_t batch_absorbed_ops{0};
  } collision_stats_;

  uint32_t surr_cursor_underflow_count_{0};

  bool is_address_bar_fix_{false};
  bool is_selection_deletion_{false};
  uint64_t adaptive_extra_us_{0};
  uint32_t stable_transactions_count_{0};
  size_t in_flight_sentinel_count_{0};
  uint64_t sentinel_grace_until_us_{0};
  size_t last_text_len_{0};
  size_t last_surr_cursor_{0};
  size_t last_surr_text_len_{0};
  uint32_t surr_keystroke_count_{0};
  size_t vscode_invalid_surr_count_{0};
  bool word_used_uinput_{false};
  std::string last_vscode_uinput_commit_;
  uint64_t op_start_us_{0};
  std::string op_group_;

  std::unique_ptr<fcitx::EventSourceTime> safety_timer_;
  std::unique_ptr<fcitx::EventSourceTime> commit_timer_;
  std::unique_ptr<fcitx::EventSourceTime> retired_commit_timer_;
  uint64_t fallback_seq_{0};
  std::unique_ptr<fcitx::EventSourceTime> surr_lag_timer_;
  bool is_surr_lag_replaying_{false};
  uint64_t last_key_time_us_{0};
  uint64_t last_key_interval_us_{0};

  uint64_t last_site_check_us_{0};
  std::string cached_site_;
  bool ctrl_pressed_first_{false};
  bool ctrl_shift_armed_{false};
  bool alt_pressed_first_{false};
  bool alt_shift_armed_{false};
  uint64_t last_selection_time_us_{0};
  bool address_bar_has_space_{false};

  uint64_t shift_press_time_us_{0};
  bool shift_other_key_pressed_{false};
  bool shift_mouse_clicked_{false};
  uint64_t last_autofix_time_us_{0};
  std::string last_autofix_original_;
  size_t last_autofix_converted_chars_{0};

  RepeatHandler repeat_handler_;
  ModalHandler modal_handler_;
  SurroundingVerifier verifier_;
  KeyBuffer key_buffer_;
  pacing::AdaptivePacingEngine pacing_engine_;

  std::unique_ptr<apps::AppProfile> profile_;
  std::string profile_app_;
  std::string profile_site_;
  bool profile_autofill_{false};
  bool profile_draftjs_{false};
  bool profile_terminal_{false};
  int profile_app_mode_{0};
};

} // namespace ime
} // namespace clak

#endif
