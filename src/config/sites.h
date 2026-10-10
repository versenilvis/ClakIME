#ifndef CLAK_CONFIG_SITES_H
#define CLAK_CONFIG_SITES_H

#include <string>

namespace clak {
namespace config {

bool isTerminalApp(const std::string& app);
bool isVSCodeApp(const std::string& app);
bool isJetBrainsApp(const std::string& app);
bool isBrowserApp(const std::string& app);
bool isGeckoApp(const std::string& app);
bool isChromiumApp(const std::string& app);
bool isSteamApp(const std::string& app);
bool isWpsOfficeApp(const std::string& app);
bool isMetaSite(const std::string& site);
bool isDraftJsSite(const std::string& site);
bool isForceUinputSite(const std::string& site);
std::string extractDomain(const std::string& app, const std::string& title);

} // namespace config
} // namespace clak

#endif
