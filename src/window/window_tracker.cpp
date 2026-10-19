#include "window/window_info.h"
#include "de/hyprland.h"
#include "de/gnome.h"
#include "apps/terminal.h"
#include "apps/browser.h"
#include <chrono>

namespace clak {
namespace window {

static std::optional<WindowInfo> s_mock_window_info;

void setMockActiveWindow(std::optional<WindowInfo> info) {
    s_mock_window_info = std::move(info);
}

void clearMockActiveWindow() {
    s_mock_window_info = std::nullopt;
}

static WindowInfo s_cached_info;
static int64_t s_last_ms = 0;

void clearWindowCache() {
    s_last_ms = 0;
    s_cached_info = WindowInfo{};
}

WindowInfo getActiveWindow(const std::string& fallback_app) {
    if (s_mock_window_info.has_value()) {
        WindowInfo info = *s_mock_window_info;
        if (info.win_class.empty() && !fallback_app.empty()) {
            info.win_class = fallback_app;
        }
        return info;
    }

    auto now_ms = std::chrono::duration_cast<std::chrono::milliseconds>(
        std::chrono::steady_clock::now().time_since_epoch()).count();

    // cache window info for 500ms to avoid querying compositor on every keystroke
    if (now_ms - s_last_ms < 500 && !s_cached_info.win_class.empty()) {
        if (fallback_app.empty() || s_cached_info.win_class == fallback_app) {
            return s_cached_info;
        }
    }
    s_last_ms = now_ms;

    WindowInfo info;

    // 1. first try hyprland if available
    bool hypr_ok = de::queryHyprlandActiveWindow(info);

    // 2. if not on hyprland (e.g. gnome wayland), query at-spi
    if (!hypr_ok || info.win_class.empty()) {
        if (de::queryAtspiActiveWindow(info)) {
            // found active window via at-spi
        } else if (!fallback_app.empty() && fallback_app != "gnome-shell") {
            info.win_class = fallback_app;
            info.win_title = fallback_app;
        } else {
            std::string term = apps::detectRunningTerminalApp();
            if (!term.empty()) {
                info.win_class = term;
                info.win_title = term;
            } else {
                std::string browser = apps::detectRunningBrowserApp();
                if (!browser.empty()) {
                    info.win_class = browser;
                    info.win_title = browser;
                }
            }
        }
    }

    if (info.win_class == "gnome-shell") {
        info.win_class.clear();
        info.win_title.clear();
    }

    if (!info.win_class.empty()) {
        s_cached_info = info;
    }
    return s_cached_info.win_class.empty() ? info : s_cached_info;
}

} // namespace window
} // namespace clak
