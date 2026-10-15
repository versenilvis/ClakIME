#ifndef CLAK_DE_GNOME_H
#define CLAK_DE_GNOME_H

#include "window/window_info.h"
#include <optional>
#include <cstdint>

namespace clak {
namespace de {

bool queryAtspiActiveWindow(window::WindowInfo& info);
std::optional<uint64_t> queryGnomeRepeatDelayUs();
std::optional<uint64_t> queryGnomeRepeatIntervalUs();

} // namespace de
} // namespace clak

#endif
