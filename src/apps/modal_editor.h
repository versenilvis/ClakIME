#ifndef CLAK_APPS_MODAL_EDITOR_H
#define CLAK_APPS_MODAL_EDITOR_H

#include "window/window_info.h"

namespace clak {
namespace apps {

bool isEditorActive(const window::WindowInfo& win);

} // namespace apps

// backward compatibility alias
namespace platform {
using apps::isEditorActive;
} // namespace platform

} // namespace clak

#endif
