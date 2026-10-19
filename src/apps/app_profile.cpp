#include "apps/app_profile.h"
#include "apps/steam.h"
#include "apps/wps.h"
#include "apps/terminal.h"
#include "apps/ide.h"
#include "apps/sites.h"
#include "apps/browser.h"

namespace clak {
namespace apps {

std::unique_ptr<AppProfile> createAppProfile(
    const std::string& app,
    const std::string& site,
    bool is_autofill,
    bool is_draftjs
) {
    if (is_autofill) {
        return createAddressBarProfile();
    }
    if (isSteamApp(app)) {
        return createSteamProfile();
    }
    if (isWpsOfficeApp(app)) {
        return createWpsProfile();
    }
    if (isJetBrainsApp(app)) {
        return createJetBrainsProfile();
    }
    if (isVteTerminalApp(app)) {
        return createVteTerminalProfile();
    }
    if (isTerminalApp(app)) {
        return createTerminalProfile();
    }
    if (isMetaSite(site) || isMetaSite(app)) {
        return createMetaSiteProfile();
    }
    if (isGoogleDocsSite(site)) {
        return createGoogleDocsProfile();
    }
    if (isTikTokSite(site)) {
        return createTikTokProfile();
    }
    if (is_draftjs || isDraftJsSite(site)) {
        return createDraftJsProfile();
    }
    if (isGeckoApp(app)) {
        return createGeckoProfile();
    }
    if (isVSCodeApp(app)) {
        return createVSCodeProfile();
    }
    return createDefaultProfile();
}

} // namespace apps
} // namespace clak
