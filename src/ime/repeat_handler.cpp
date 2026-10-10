#include "repeat_handler.h"
#include "utils/log.h"
#include <cstdio>
#include <cstdlib>

namespace clak {
namespace ime {

RepeatHandler::~RepeatHandler() {
    cancel();
}

uint64_t RepeatHandler::repeatDelayUs() {
    static uint64_t cached_delay = 0;
    if (cached_delay > 0) return cached_delay;
    if (getenv("HYPRLAND_INSTANCE_SIGNATURE")) {
        FILE* fp = popen("hyprctl getoption input:repeat_delay 2>/dev/null", "r");
        if (fp) {
            char buf[128];
            while (fgets(buf, sizeof(buf), fp)) {
                int val = 0;
                if (sscanf(buf, "int: %d", &val) == 1 && val > 0) {
                    cached_delay = static_cast<uint64_t>(val) * 1000;
                    break;
                }
            }
            pclose(fp);
        }
    }
    if (cached_delay == 0) {
        cached_delay = 200000;
    }
    utils::clakLog("repeat delay initialized: " + std::to_string(cached_delay) + "us");
    return cached_delay;
}

uint64_t RepeatHandler::repeatIntervalUs() {
    static uint64_t cached_interval = 0;
    if (cached_interval > 0) return cached_interval;
    if (getenv("HYPRLAND_INSTANCE_SIGNATURE")) {
        FILE* fp = popen("hyprctl getoption input:repeat_rate 2>/dev/null", "r");
        if (fp) {
            char buf[128];
            while (fgets(buf, sizeof(buf), fp)) {
                int val = 0;
                if (sscanf(buf, "int: %d", &val) == 1 && val > 0) {
                    cached_interval = 1000000 / val;
                    break;
                }
            }
            pclose(fp);
        }
    }
    if (cached_interval == 0) {
        cached_interval = 25000;
    }
    utils::clakLog("repeat interval initialized: " + std::to_string(cached_interval) + "us");
    return cached_interval;
}

void RepeatHandler::arm(fcitx::EventLoop& event_loop, const fcitx::Key& key, std::function<void(const fcitx::Key&)> callback) {
    if (is_repeating_ && held_key_.sym() == key.sym()) {
        return;
    }
    held_key_ = key;
    is_repeating_ = false;
    callback_ = std::move(callback);
    uint64_t now_us = fcitx::now(CLOCK_MONOTONIC);
    uint64_t delay_us = repeatDelayUs();
    repeat_timer_ = event_loop.addTimeEvent(
        CLOCK_MONOTONIC,
        now_us + delay_us,
        1000,
        [this](fcitx::EventSourceTime*, uint64_t) {
            onTimer();
            return true;
        }
    );
}

void RepeatHandler::cancel() {
    if (repeat_timer_) {
        repeat_timer_.reset();
    }
    held_key_ = fcitx::Key();
    is_repeating_ = false;
    callback_ = nullptr;
}

void RepeatHandler::defer(uint64_t delay_us) {
    if (repeat_timer_) {
        uint64_t now_us = fcitx::now(CLOCK_MONOTONIC);
        repeat_timer_->setTime(now_us + delay_us);
        repeat_timer_->setAccuracy(1000);
        repeat_timer_->setOneShot();
    }
}

void RepeatHandler::onTimer() {
    if (held_key_.sym() == 0) return;

    if (!is_repeating_) {
        utils::clakLog("onRepeatTimer: start repeat sym=" + std::to_string(held_key_.sym()));
        is_repeating_ = true;
    }

    if (callback_) {
        callback_(held_key_);
    }

    if (repeat_timer_ && held_key_.sym() != 0) {
        uint64_t now_us = fcitx::now(CLOCK_MONOTONIC);
        repeat_timer_->setTime(now_us + repeatIntervalUs());
        repeat_timer_->setAccuracy(1000);
        repeat_timer_->setOneShot();
    }
}

} // namespace ime
} // namespace clak
