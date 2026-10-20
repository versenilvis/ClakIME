#include "de/keyboard_settings.h"
#include "de/hyprland.h"
#include "de/gnome.h"
#include "de/kde.h"
#include <chrono>
#include <cstdlib>

namespace clak {
namespace de {

static std::optional<uint64_t> s_mock_repeat_delay_us;
static std::optional<uint64_t> s_mock_repeat_interval_us;

void setMockRepeatSettings(std::optional<uint64_t> delay_us, std::optional<uint64_t> interval_us) {
    s_mock_repeat_delay_us = delay_us;
    s_mock_repeat_interval_us = interval_us;
}

void clearMockRepeatSettings() {
    s_mock_repeat_delay_us = std::nullopt;
    s_mock_repeat_interval_us = std::nullopt;
}

uint64_t getRepeatDelayUs() {
    if (s_mock_repeat_delay_us.has_value()) {
        return *s_mock_repeat_delay_us;
    }

    static uint64_t s_cached_delay = 0;
    static int64_t s_last_check_ms = 0;

    auto now_ms = std::chrono::duration_cast<std::chrono::milliseconds>(
        std::chrono::steady_clock::now().time_since_epoch()).count();

    // refresh setting every 2000ms so user setting changes in desktop take effect
    if (s_cached_delay > 0 && now_ms - s_last_check_ms < 2000) {
        return s_cached_delay;
    }
    s_last_check_ms = now_ms;

    if (getenv("HYPRLAND_INSTANCE_SIGNATURE")) {
        auto val = queryHyprlandRepeatDelayUs();
        if (val.has_value()) {
            s_cached_delay = *val;
            return s_cached_delay;
        }
        if (s_cached_delay == 0) s_cached_delay = 200000; // hyprland default 200ms
    } else if (isKdeSession()) {
        auto val = queryKdeRepeatDelayUs();
        if (val.has_value()) {
            s_cached_delay = *val;
            return s_cached_delay;
        }
        if (s_cached_delay == 0) s_cached_delay = 600000; // kde default 600ms
    } else {
        auto val = queryGnomeRepeatDelayUs();
        if (val.has_value()) {
            s_cached_delay = *val;
            return s_cached_delay;
        }
        if (s_cached_delay == 0) s_cached_delay = 500000; // gnome default 500ms
    }

    return s_cached_delay;
}

uint64_t getRepeatIntervalUs() {
    if (s_mock_repeat_interval_us.has_value()) {
        return *s_mock_repeat_interval_us;
    }

    static uint64_t s_cached_interval = 0;
    static int64_t s_last_check_ms = 0;

    auto now_ms = std::chrono::duration_cast<std::chrono::milliseconds>(
        std::chrono::steady_clock::now().time_since_epoch()).count();

    if (s_cached_interval > 0 && now_ms - s_last_check_ms < 2000) {
        return s_cached_interval;
    }
    s_last_check_ms = now_ms;

    if (getenv("HYPRLAND_INSTANCE_SIGNATURE")) {
        auto val = queryHyprlandRepeatIntervalUs();
        if (val.has_value()) {
            s_cached_interval = *val;
            return s_cached_interval;
        }
        if (s_cached_interval == 0) s_cached_interval = 25000; // hyprland default 25ms
    } else if (isKdeSession()) {
        auto val = queryKdeRepeatIntervalUs();
        if (val.has_value()) {
            s_cached_interval = *val;
            return s_cached_interval;
        }
        if (s_cached_interval == 0) s_cached_interval = 40000; // kde default 40ms
    } else {
        auto val = queryGnomeRepeatIntervalUs();
        if (val.has_value()) {
            s_cached_interval = *val;
            return s_cached_interval;
        }
        if (s_cached_interval == 0) s_cached_interval = 30000; // gnome default 30ms
    }

    return s_cached_interval;
}

} // namespace de
} // namespace clak
