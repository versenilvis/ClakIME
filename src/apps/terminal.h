#ifndef CLAK_APPS_TERMINAL_H
#define CLAK_APPS_TERMINAL_H

#include "apps/app_profile.h"
#include <string>
#include <memory>

namespace clak {
namespace apps {

bool isTerminalApp(const std::string& app);
std::string detectRunningTerminalApp();
std::unique_ptr<AppProfile> createTerminalProfile();

} // namespace apps
} // namespace clak

#endif
