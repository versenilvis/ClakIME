#include "apps/wps.h"

namespace clak {
namespace apps {

bool isWpsOfficeApp(const std::string& app) {
    if (app.empty()) return false;
    return app == "wps" || app == "wpp" || app == "et" || app == "wpspdf" || app == "wpsoffice" ||
           app.find("wps") != std::string::npos;
}

namespace {

class WpsProfile : public AppProfile {
public:
    std::string name() const override { return "WPS-Office"; }
    bool allowsSurroundingText() const override { return false; }
    bool forcesUinput() const override { return true; }
    UinputPacing uinputPacing() const override { return {15, 4}; }
};

} // namespace

std::unique_ptr<AppProfile> createWpsProfile() {
    return std::make_unique<WpsProfile>();
}

} // namespace apps
} // namespace clak
