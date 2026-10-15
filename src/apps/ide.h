#ifndef CLAK_APPS_IDE_H
#define CLAK_APPS_IDE_H

#include "apps/app_profile.h"
#include <string>
#include <memory>

namespace clak {
namespace apps {

bool isVSCodeApp(const std::string& app);
bool isJetBrainsApp(const std::string& app);

std::unique_ptr<AppProfile> createJetBrainsProfile();
std::unique_ptr<AppProfile> createVSCodeProfile();

} // namespace apps
} // namespace clak

#endif
