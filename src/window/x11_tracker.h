#ifndef CLAK_WINDOW_X11_TRACKER_H
#define CLAK_WINDOW_X11_TRACKER_H

#include "window/window_info.h"

namespace clak {
namespace window {

// query active x11 window info by traversing parent windows up to root
bool queryX11ActiveWindow(WindowInfo& out_info);

} // namespace window
} // namespace clak

#endif
