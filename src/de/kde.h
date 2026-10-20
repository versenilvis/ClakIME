#ifndef CLAK_DE_KDE_H
#define CLAK_DE_KDE_H

#include <optional>
#include <cstdint>
#include <string>

namespace clak {
namespace de {

// detect if current session is kde plasma
bool isKdeSession();

// check if app is a desktop shell (gnome-shell, plasmashell, krunner)
bool isDesktopShell(const std::string& app);

// query keyboard repeat delay in microseconds from kcminputrc
std::optional<uint64_t> queryKdeRepeatDelayUs();

// query keyboard repeat interval in microseconds from kcminputrc
std::optional<uint64_t> queryKdeRepeatIntervalUs();

} // namespace de
} // namespace clak

#endif
