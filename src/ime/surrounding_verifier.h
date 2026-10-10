#ifndef CLAK_IME_SURROUNDING_VERIFIER_H
#define CLAK_IME_SURROUNDING_VERIFIER_H

#include <fcitx/inputcontext.h>
#include <string>
#include <cstddef>

namespace clak {
namespace ime {

struct PendingVerify {
  bool pending{false};
  std::string preWord;
  size_t del{0};
  std::string added;
  std::string expectWord;
};

class SurroundingVerifier {
public:
  SurroundingVerifier() = default;

  void setExpectation(const std::string& wordBefore, size_t delChars, const std::string& added);
  void verify(const fcitx::SurroundingText& surr);
  void reset();

  bool isPending() const { return verify_.pending; }
  void clearPending() { verify_.pending = false; }
  int mismatchCount() const { return mismatch_count_; }
  void clearMismatches() { mismatch_count_ = 0; }

  bool isCanvasEditor() const { return is_canvas_editor_; }
  void setCanvasEditor(bool val) { is_canvas_editor_ = val; }

  bool isRichTextEditor() const { return is_rich_text_editor_; }
  void setRichTextEditor(bool val) { is_rich_text_editor_ = val; }

  bool isDraftJsEditor(const std::string& site) const;
  bool isDraftJsEditorDirect() const { return is_draftjs_editor_; }
  void setDraftJsEditor(bool val) { is_draftjs_editor_ = val; }

private:
  PendingVerify verify_;
  int mismatch_count_{0};
  bool is_canvas_editor_{false};
  bool is_rich_text_editor_{false};
  bool is_draftjs_editor_{false};
};

} // namespace ime
} // namespace clak

#endif
