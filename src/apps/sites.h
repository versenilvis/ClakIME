#ifndef CLAK_APPS_SITES_H
#define CLAK_APPS_SITES_H

#include "apps/app_profile.h"
#include <string>
#include <memory>

namespace clak {
namespace apps {

bool isMetaSite(const std::string& site);
bool isDraftJsSite(const std::string& site);
bool isGoogleDocsSite(const std::string& site);
bool isForceUinputSite(const std::string& site);

std::unique_ptr<AppProfile> createMetaSiteProfile();
std::unique_ptr<AppProfile> createDraftJsProfile();
std::unique_ptr<AppProfile> createGoogleDocsProfile();
std::unique_ptr<AppProfile> createAddressBarProfile();

} // namespace apps
} // namespace clak

#endif
