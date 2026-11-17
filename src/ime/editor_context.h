#ifndef CLAK_IME_EDITOR_CONTEXT_H
#define CLAK_IME_EDITOR_CONTEXT_H

#include <fcitx/inputcontext.h>
#include <string>
#include <cstdint>
#include "apps/app_profile.h"
#include "surrounding_verifier.h"

namespace clak {

class ClakEngine;

namespace ime {

class EditorContextDetector {
public:
    EditorContextDetector(fcitx::InputContext* ic, SurroundingVerifier& verifier);

    std::string appKey() const;
    std::string activeSite();
    bool isBrowser() const;
    bool isGecko() const;
    bool isSteam() const;
    bool isDraftJsEditor();
    static bool looksLikeUrlAutofill(const fcitx::SurroundingText& surr);

    void reset();

private:
    fcitx::InputContext* ic_;
    SurroundingVerifier& verifier_;

    uint64_t last_site_check_us_{0};
    std::string cached_site_;
};

} // namespace ime
} // namespace clak

#endif
