#ifndef CLAK_APPS_APP_PROFILE_H
#define CLAK_APPS_APP_PROFILE_H

#include <string>
#include <memory>
#include <functional>
#include <cstdint>
#include <fcitx-utils/event.h>
#include <fcitx/inputcontext.h>

namespace clak {
namespace apps {

struct UinputPacing {
    uint32_t post_delay_ms{6};
    uint32_t gap_ms{2};
};

class AppProfile {
public:
    virtual ~AppProfile() = default;

    virtual std::string name() const = 0;

    // deletion and input method routing
    virtual bool allowsSurroundingText() const { return true; }
    virtual bool forcesUinput() const { return false; }
    virtual bool preventsUinput() const { return false; }
    virtual bool allowsAutofillHeuristic() const { return true; }
    virtual bool directCommitPrintableKey() const { return false; }

    // uinput delay settings
    virtual UinputPacing uinputPacing() const { return {6, 2}; }

    // key event filter hook (returns true to drop/accept key)
    virtual bool filterIncomingKey(uint32_t sym, uint64_t now_us) {
        (void)sym;
        (void)now_us;
        return false;
    }

    // custom deletion pipeline (e.g. steam staggered timers)
    virtual bool hasCustomDeletion() const { return false; }
    virtual void startCustomDeletion(
        fcitx::EventLoop& loop,
        fcitx::InputContext* ic,
        size_t count,
        const std::string& commit_str,
        std::function<void(const std::string&)> commit_fn,
        std::function<void()> on_completed
    ) {
        (void)loop;
        (void)ic;
        (void)count;
        (void)commit_str;
        (void)commit_fn;
        (void)on_completed;
    }
    virtual void cancelCustomDeletion() {}

    virtual void reset() {}
};

std::unique_ptr<AppProfile> createAppProfile(
    const std::string& app,
    const std::string& site,
    bool is_autofill,
    bool is_draftjs
);

} // namespace apps
} // namespace clak

#endif
