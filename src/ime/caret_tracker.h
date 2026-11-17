#ifndef CLAK_IME_CARET_TRACKER_H
#define CLAK_IME_CARET_TRACKER_H

#include <fcitx/inputcontext.h>
#include <string>
#include <cstddef>

namespace clak {
namespace ime {

class CaretTracker {
public:
    CaretTracker() = default;

    void onKeyEvent(uint32_t sym, bool is_cursor_move, bool is_shift, bool has_ctrl_alt, bool is_special_nav, const fcitx::SurroundingText& surr);
    void resolveEffectiveOffsets(const fcitx::SurroundingText& surr, size_t& out_cursor, size_t& out_anchor);
    void reset();

    size_t trackedCursor() const { return tracked_cursor_; }
    bool isTracked() const { return cursor_tracked_; }

private:
    size_t tracked_cursor_{0};
    bool cursor_tracked_{false};
    std::string last_tracked_text_;
    size_t last_surr_cursor_{0};
};

} // namespace ime
} // namespace clak

#endif
