#include "caret_tracker.h"
#include "utils/text_utils.h"
#include <fcitx-utils/keysym.h>

namespace clak {
namespace ime {

void CaretTracker::onKeyEvent(
    uint32_t sym,
    bool is_cursor_move,
    bool is_shift,
    bool has_ctrl_alt,
    bool is_special_nav,
    const fcitx::SurroundingText& surr
) {
    if (is_cursor_move && !is_shift && !has_ctrl_alt) {
        if (surr.isValid()) {
            size_t total_chars = utils::utf8CharCount(surr.text());
            if (!cursor_tracked_ || surr.text() != last_tracked_text_) {
                tracked_cursor_ = surr.cursor();
                last_tracked_text_ = surr.text();
                last_surr_cursor_ = surr.cursor();
                cursor_tracked_ = true;
            }
            if (sym == FcitxKey_Home) {
                tracked_cursor_ = 0;
            } else if (sym == FcitxKey_End) {
                tracked_cursor_ = total_chars;
            } else if (sym == FcitxKey_Left) {
                if (tracked_cursor_ > 0) {
                    tracked_cursor_--;
                }
            } else if (sym == FcitxKey_Right) {
                if (tracked_cursor_ < total_chars) {
                    tracked_cursor_++;
                }
            }
        }
    } else if (has_ctrl_alt || is_special_nav) {
        cursor_tracked_ = false;
        last_tracked_text_.clear();
    }
}

void CaretTracker::resolveEffectiveOffsets(const fcitx::SurroundingText& surr, size_t& out_cursor, size_t& out_anchor) {
    if (surr.isValid()) {
        size_t total_chars = utils::utf8CharCount(surr.text());
        if (!cursor_tracked_ || surr.text() != last_tracked_text_) {
            tracked_cursor_ = surr.cursor();
            last_tracked_text_ = surr.text();
            last_surr_cursor_ = surr.cursor();
            cursor_tracked_ = true;
        }
        if (tracked_cursor_ > total_chars) {
            tracked_cursor_ = total_chars;
        }
        if (surr.cursor() != surr.anchor()) {
            out_cursor = surr.cursor();
            out_anchor = surr.anchor();
            tracked_cursor_ = surr.cursor();
            last_surr_cursor_ = surr.cursor();
        } else {
            out_cursor = tracked_cursor_;
            out_anchor = tracked_cursor_;
        }
    } else {
        cursor_tracked_ = false;
        last_tracked_text_.clear();
        out_cursor = 0;
        out_anchor = 0;
    }
}

void CaretTracker::reset() {
    cursor_tracked_ = false;
    last_tracked_text_.clear();
}

} // namespace ime
} // namespace clak
