#ifndef CLAK_WINDOW_WINDOW_INFO_H
#define CLAK_WINDOW_WINDOW_INFO_H

#include <string>
#include <optional>
#include <sys/types.h>

namespace clak {
namespace window {

struct WindowInfo {
    std::string win_class;
    std::string win_title;
    pid_t pid{0};
};

WindowInfo getActiveWindow(const std::string& fallback_app = "");
void setMockActiveWindow(std::optional<WindowInfo> info);
void clearMockActiveWindow();
void clearWindowCache();

} // namespace window

// backward compatibility alias
namespace platform {
using WindowInfo = window::WindowInfo;
using window::getActiveWindow;
using window::setMockActiveWindow;
using window::clearMockActiveWindow;
using window::clearWindowCache;
} // namespace platform

} // namespace clak

#endif
