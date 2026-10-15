#ifndef CLAK_APPS_WPS_H
#define CLAK_APPS_WPS_H

#include "apps/app_profile.h"
#include <string>
#include <memory>

namespace clak {
namespace apps {

bool isWpsOfficeApp(const std::string& app);
std::unique_ptr<AppProfile> createWpsProfile();

} // namespace apps
} // namespace clak

#endif
