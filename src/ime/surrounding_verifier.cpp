#include "surrounding_verifier.h"
#include "config/config.h"
#include "config/sites.h"
#include "utils/log.h"
#include "utils/text_utils.h"

namespace clak {
namespace ime {

void SurroundingVerifier::reset(bool hard_reset) {
    verify_.pending = false;
    if (hard_reset) {
        mismatch_count_ = 0;
        is_canvas_editor_ = false;
        is_rich_text_editor_ = false;
        is_draftjs_editor_ = false;
    }
}

bool SurroundingVerifier::isDraftJsEditor(const std::string& site) const {
    return is_draftjs_editor_ || config::isDraftJsSite(site);
}

bool SurroundingVerifier::looksLikeMessengerComposer(const fcitx::SurroundingText& s) {
    if (!s.isValid()) return false;
    const std::string& t = s.text();
    if (t == "\n") return true;
    if (s.cursor() <= t.size()) {
        std::string after = t.substr(s.cursor());
        if (after == "\n\n" || after == "\n") return true;
    }
    return false;
}

bool SurroundingVerifier::looksLikeFacebookComposer(const fcitx::SurroundingText& s) {
    if (!s.isValid()) return false;
    const std::string& t = s.text();
    return t == "\n" || (t.size() >= 2 && t.compare(t.size() - 2, 2, "\n\n") == 0);
}

bool SurroundingVerifier::isFirstWordOfMessage(const fcitx::SurroundingText& s) {
    if (!s.isValid() || s.cursor() == 0) return true;
    const std::string& t = s.text();
    size_t limit = std::min(static_cast<size_t>(s.cursor()), t.size());
    for (size_t i = 0; i < limit; ++i) {
        if (t[i] == ' ' || t[i] == '\n') return false;
    }
    return true;
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
    } else if (actualWord == verify_.preWord) {
        // Asynchronous Wayland roundtrip: the client application has not yet processed
        // the in-flight deleteSurroundingText/commitString. This is normal IPC latency,
        // NOT an editor mismatch. Do not increment mismatch_count_ or trigger uinput.
    } else {
        bool delete_ignored = (!verify_.preWord.empty() &&
                               !verify_.added.empty() &&
                               actualWord.find(verify_.preWord) != std::string::npos &&
                               actualWord.find(verify_.added) != std::string::npos &&
                               actualWord.length() > verify_.preWord.length());
        mismatch_count_++;
        utils::clakLog("verifySurrounding mismatch #" + std::to_string(mismatch_count_) +
                       ": expected='" + verify_.expectWord + "' actual='" + actualWord + "'");
        if (mismatch_count_ >= config::kMismatchThreshold) {
            is_rich_text_editor_ = true;
            if (delete_ignored) {
                is_draftjs_editor_ = true;
            }
            utils::clakLog("verifySurrounding: auto-switched to uinput for this editor after 3 consecutive mismatches");
        }
    }
}

} // namespace ime
} // namespace clak
