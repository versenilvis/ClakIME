#ifndef CLAK_DE_HYPRLAND_H
#define CLAK_DE_HYPRLAND_H

#include "window/window_info.h"
#include <optional>
#include <cstdint>

namespace clak {
namespace de {

bool queryHyprlandActiveWindow(window::WindowInfo& info);
std::optional<uint64_t> queryHyprlandRepeatDelayUs();
std::optional<uint64_t> queryHyprlandRepeatIntervalUs();

} // namespace de
} // namespace clak

#endif
