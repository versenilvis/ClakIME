#include "apps/ide.h"
#include "config/config.h"
#include <vector>

namespace clak {
namespace apps {

bool isVSCodeApp(const std::string& app) {
    static const std::vector<std::string> vsc = {
        "antigravity", "antigravity-ide", "cursor", "windsurf", "vscodium", "codium",
        "code-oss", "positron", "trae"
    };
    if (app == "code" || app == "vscode") return true;
    for (const auto& v : vsc) {
        if (app.find(v) != std::string::npos) return true;
    }
    return false;
}

bool isJetBrainsApp(const std::string& app) {
    static const std::vector<std::string> jb = {
        "idea", "jetbrains", "pycharm", "clion", "webstorm", "goland",
        "rider", "rubymine", "phpstorm", "datagrip", "android-studio", "studio",
        "rustrover", "fleet", "aqua", "dataspell", "gateway", "mps"
    };
    for (const auto& j : jb) {
        if (app.find(j) != std::string::npos) return true;
    }
    return false;
}

namespace {

class JetBrainsProfile : public AppProfile {
public:
    std::string name() const override { return "JetBrains-Uinput"; }
    bool allowsSurroundingText() const override { return false; }
    bool forcesUinput() const override { return true; }
    UinputPacing uinputPacing() const override {
        return {config::kTerminalPostDelayMs, config::kTerminalGapMs};
    }
};

class VSCodeProfile : public AppProfile {
public:
    std::string name() const override { return "VSCode-SurroundingText"; }
    bool allowsSurroundingText() const override { return true; }
    UinputPacing uinputPacing() const override { return {6, 2}; }
};

} // namespace

std::unique_ptr<AppProfile> createJetBrainsProfile() {
    return std::make_unique<JetBrainsProfile>();
}

std::unique_ptr<AppProfile> createVSCodeProfile() {
    return std::make_unique<VSCodeProfile>();
}

} // namespace apps
} // namespace clak
