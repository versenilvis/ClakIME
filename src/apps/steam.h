#ifndef CLAK_APPS_STEAM_H
#define CLAK_APPS_STEAM_H

#include "apps/app_profile.h"
#include <string>
#include <memory>

namespace clak {
namespace apps {

bool isSteamApp(const std::string& app);
std::unique_ptr<AppProfile> createSteamProfile();

} // namespace apps
} // namespace clak

#endif
