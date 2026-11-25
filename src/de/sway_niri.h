#ifndef CLAK_DE_SWAY_NIRI_H
#define CLAK_DE_SWAY_NIRI_H

#include "window/window_info.h"
#include <string>

namespace clak {
namespace de {

// query active window from sway ipc via $SWAYSOCK
bool querySwayActiveWindow(window::WindowInfo& info);

// query active window from niri ipc via $NIRI_SOCKET
bool queryNiriActiveWindow(window::WindowInfo& info);

// parse sway GET_TREE json response for the focused window node
bool parseSwayTreeJson(const std::string& json, window::WindowInfo& info);

// parse niri FocusedWindow json response
bool parseNiriFocusedWindowJson(const std::string& json, window::WindowInfo& info);

} // namespace de
} // namespace clak

#endif
