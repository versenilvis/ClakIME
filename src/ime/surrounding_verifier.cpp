#include "surrounding_verifier.h"
#include "config/config.h"
#include "config/sites.h"
#include "utils/log.h"
#include "utils/text_utils.h"

namespace clak {
namespace ime {

void SurroundingVerifier::reset() {
    verify_.pending = false;
    mismatch_count_ = 0;
    is_canvas_editor_ = false;
    is_rich_text_editor_ = false;
    is_draftjs_editor_ = false;
}

bool SurroundingVerifier::isDraftJsEditor(const std::string& site) const {
    return is_draftjs_editor_ || config::isDraftJsSite(site);
}

void SurroundingVerifier::setExpectation(const std::string& wordBefore, size_t delChars, const std::string& added) {
    if (added.find(' ') != std::string::npos || added.find('\n') != std::string::npos || added.find('\t') != std::string::npos) {
        verify_.pending = false;
        return;
    }
    verify_.pending = true;
    verify_.preWord = wordBefore;
    verify_.del = delChars;
    verify_.added = added;

    std::string base = wordBefore;
    utils::popUtf8Chars(base, delChars);
    verify_.expectWord = base + added;
}

void SurroundingVerifier::verify(const fcitx::SurroundingText& surr) {
    if (!verify_.pending) return;
    verify_.pending = false;

    std::string actualWord = utils::extractWordBeforeCursor(surr.text(), surr.cursor());

    if (actualWord == verify_.expectWord) {
        mismatch_count_ = 0;
        is_rich_text_editor_ = false;
        is_draftjs_editor_ = false;
    } else {
        mismatch_count_++;
        utils::clakLog("verifySurrounding mismatch #" + std::to_string(mismatch_count_) +
                       ": expected='" + verify_.expectWord + "' actual='" + actualWord + "'");
        bool delete_ignored = (!verify_.preWord.empty() && actualWord.find(verify_.preWord) != std::string::npos);
        if (delete_ignored) {
            is_draftjs_editor_ = true;
            is_rich_text_editor_ = true;
            utils::clakLog("verifySurrounding: detected draftjs / react contenteditable editor, switched to uinput");
        } else if (mismatch_count_ >= config::kMismatchThreshold) {
            is_rich_text_editor_ = true;
            utils::clakLog("verifySurrounding: auto-switched to uinput for this editor");
        }
    }
}

} // namespace ime
} // namespace clak
