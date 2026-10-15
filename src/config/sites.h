#ifndef CLAK_CONFIG_SITES_H
#define CLAK_CONFIG_SITES_H

#include "apps/terminal.h"
#include "apps/ide.h"
#include "apps/browser.h"
#include "apps/steam.h"
#include "apps/wps.h"
#include "apps/sites.h"

namespace clak {
namespace config {

// backwards compatibility aliases
using apps::isTerminalApp;
using apps::isVSCodeApp;
using apps::isJetBrainsApp;
using apps::isBrowserApp;
using apps::isGeckoApp;
using apps::isChromiumApp;
using apps::isSteamApp;
using apps::isWpsOfficeApp;
using apps::isMetaSite;
using apps::isDraftJsSite;
using apps::isForceUinputSite;
using apps::extractDomain;

} // namespace config
} // namespace clak

#endif
