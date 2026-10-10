#ifndef CLAK_IME_MODAL_HANDLER_H
#define CLAK_IME_MODAL_HANDLER_H

#include <fcitx-utils/key.h>
#include <string>
#include <functional>
#include <cstdint>

namespace clak {
namespace ime {

enum class EditorMode {
  NORMAL,
  INSERT,
  COMMAND
};

class ModalHandler {
public:
  ModalHandler() = default;

  void updateStatus(const std::string& fallback_program);
  bool isModalEditor() const { return is_modal_editor_; }
  EditorMode mode() const { return editor_mode_; }

  // returns true if key was handled/intercepted by modal editor mode logic
  bool handleKey(const fcitx::Key& key, const std::function<void()>& on_reset);
  void reset();

private:
  EditorMode editor_mode_{EditorMode::NORMAL};
  bool is_modal_editor_{false};
  uint64_t last_editor_check_us_{0};
};

} // namespace ime
} // namespace clak

#endif
