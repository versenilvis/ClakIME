#include "apps/steam.h"
#include "utils/log.h"
#include <fcitx-utils/keysym.h>

namespace clak {
namespace apps {

bool isSteamApp(const std::string& app) {
    if (app.empty()) return false;
    if (app == "steam" || app == "steamwebhelper" || app == "com.valvesoftware.Steam") return true;
    if (app.rfind("steamwebhelper", 0) == 0 || app.rfind("steam", 0) == 0) return true;
    return false;
}

namespace {

class SteamProfile : public AppProfile {
public:
    std::string name() const override { return "Steam-ForwardKey"; }
    bool allowsSurroundingText() const override { return false; }
    bool directCommitPrintableKey() const override { return true; }

    bool filterIncomingKey(uint32_t sym, uint64_t now_us) override {
        if (sym == last_sym_ && (now_us - last_time_us_) < 2000) {
            utils::clakLog("steam duplicate key dropped: sym=" + std::to_string(sym) +
                           " delta=" + std::to_string(now_us - last_time_us_) + "us");
            return true;
        }
        last_sym_ = sym;
        last_time_us_ = now_us;
        return false;
    }

    bool hasCustomDeletion() const override { return true; }

    void startCustomDeletion(
        fcitx::EventLoop& loop,
        fcitx::InputContext* ic,
        size_t count,
        const std::string& commit_str,
        std::function<void(const std::string&)> commit_fn,
        std::function<void()> on_completed
    ) override {
        cancelCustomDeletion();
        scheduleStep(loop, ic, count, 0, commit_str, std::move(commit_fn), std::move(on_completed));
    }

    void cancelCustomDeletion() override {
        if (timer_) {
            timer_.reset();
        }
    }

    void reset() override {
        last_sym_ = 0;
        last_time_us_ = 0;
        cancelCustomDeletion();
    }

private:
    void scheduleStep(
        fcitx::EventLoop& loop,
        fcitx::InputContext* ic,
        size_t total_backspaces,
        size_t current_backspaces,
        std::string commit_str,
        std::function<void(const std::string&)> commit_fn,
        std::function<void()> on_completed
    ) {
        if (!ic) return;

        if (current_backspaces < total_backspaces) {
            ic->forwardKey(fcitx::Key(FcitxKey_BackSpace), false);
            ic->forwardKey(fcitx::Key(FcitxKey_BackSpace), true);
            size_t next_count = current_backspaces + 1;

            uint64_t now_us = fcitx::now(CLOCK_MONOTONIC);
            if (next_count < total_backspaces) {
                timer_ = loop.addTimeEvent(
                    CLOCK_MONOTONIC,
                    now_us + 2000,
                    1000,
                    [this, &loop, ic, total_backspaces, next_count,
                     commit_str = std::move(commit_str),
                     commit_fn = std::move(commit_fn),
                     on_completed = std::move(on_completed)](fcitx::EventSourceTime*, uint64_t) mutable {
                        scheduleStep(loop, ic, total_backspaces, next_count, std::move(commit_str), std::move(commit_fn), std::move(on_completed));
                        return false;
                    });
                if (timer_) {
                    timer_->setOneShot();
                }
            } else {
                timer_ = loop.addTimeEvent(
                    CLOCK_MONOTONIC,
                    now_us + 8000,
                    1000,
                    [this, commit_str = std::move(commit_str),
                     commit_fn = std::move(commit_fn),
                     on_completed = std::move(on_completed)](fcitx::EventSourceTime*, uint64_t) {
                        if (!commit_str.empty() && commit_fn) {
                            commit_fn(commit_str);
                        }
                        cancelCustomDeletion();
                        if (on_completed) {
                            on_completed();
                        }
                        return false;
                    });
                if (timer_) {
                    timer_->setOneShot();
                }
            }
        }
    }

    uint32_t last_sym_{0};
    uint64_t last_time_us_{0};
    std::unique_ptr<fcitx::EventSourceTime> timer_;
};

} // namespace

std::unique_ptr<AppProfile> createSteamProfile() {
    return std::make_unique<SteamProfile>();
}

} // namespace apps
} // namespace clak
