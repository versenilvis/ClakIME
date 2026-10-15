#ifndef CLAK_APPS_BROWSER_H
#define CLAK_APPS_BROWSER_H

#include "apps/app_profile.h"
#include <string>
#include <memory>

namespace clak {
namespace apps {

bool isBrowserApp(const std::string& app);
bool isGeckoApp(const std::string& app);
bool isChromiumApp(const std::string& app);
std::string extractDomain(const std::string& app, const std::string& title);

std::unique_ptr<AppProfile> createGeckoProfile();
std::unique_ptr<AppProfile> createDefaultProfile();

} // namespace apps
} // namespace clak

#endif
