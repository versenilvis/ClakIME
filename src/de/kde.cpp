#include "de/kde.h"
#include <cstdlib>
#include <cstring>
#include <fstream>
#include <sstream>
#include <string>

namespace clak {
namespace de {

// detect if current session is kde plasma
bool isKdeSession() {
    const char* de = getenv("XDG_CURRENT_DESKTOP");
    if (de && (strstr(de, "KDE") || strstr(de, "Plasma") || strstr(de, "plasma"))) {
        return true;
    }
    const char* full_session = getenv("KDE_FULL_SESSION");
    if (full_session && strcmp(full_session, "true") == 0) {
        return true;
    }
    return false;
}

// check if app is a desktop shell
bool isDesktopShell(const std::string& app) {
    if (app.empty()) return false;
    return app == "gnome-shell" ||
           app == "plasmashell" ||
           app == "org.kde.plasmashell" ||
           app == "plasma-desktop" ||
           app == "krunner" ||
           app == "org.kde.krunner" ||
           app == "kscreenlocker_greet" ||
           app == "org.kde.kscreenlocker" ||
           app.find("portal") != std::string::npos ||
           app.find("polkit-kde") != std::string::npos;
}

// parse kcminputrc keyboard section value
static std::optional<int> readKcminputKey(const std::string& target_key) {
    const char* home = getenv("HOME");
    if (!home) return std::nullopt;

    std::string path = std::string(home) + "/.config/kcminputrc";
    std::ifstream file(path);
    if (!file.is_open()) return std::nullopt;

    std::string line;
    bool in_keyboard_section = false;
    while (std::getline(file, line)) {
        if (!line.empty() && line.back() == '\r') {
            line.pop_back();
        }
        if (line.empty() || line[0] == '#') continue;

        if (line[0] == '[') {
            in_keyboard_section = (line == "[Keyboard]");
            continue;
        }

        if (in_keyboard_section) {
            auto pos = line.find('=');
            if (pos != std::string::npos) {
                std::string key = line.substr(0, pos);
                if (key == target_key) {
                    try {
                        int val = std::stoi(line.substr(pos + 1));
                        return val;
                    } catch (...) {
                        return std::nullopt;
                    }
                }
            }
        }
    }
    return std::nullopt;
}

// query keyboard repeat delay in microseconds from kcminputrc
std::optional<uint64_t> queryKdeRepeatDelayUs() {
    if (!isKdeSession()) return std::nullopt;

    auto val = readKcminputKey("RepeatDelay");
    if (val.has_value() && *val > 0) {
        return static_cast<uint64_t>(*val) * 1000;
    }

    // fallback to kreadconfig6 or kreadconfig5
    FILE* fp = popen("kreadconfig6 --file kcminputrc --group Keyboard --key RepeatDelay 2>/dev/null || kreadconfig5 --file kcminputrc --group Keyboard --key RepeatDelay 2>/dev/null", "r");
    if (fp) {
        char buf[64];
        std::optional<uint64_t> res;
        if (fgets(buf, sizeof(buf), fp)) {
            int d = 0;
            if (sscanf(buf, "%d", &d) == 1 && d > 0) {
                res = static_cast<uint64_t>(d) * 1000;
            }
        }
        pclose(fp);
        if (res.has_value()) return res;
    }

    return 600000; // default kde delay 600ms
}

// query keyboard repeat interval in microseconds from kcminputrc
std::optional<uint64_t> queryKdeRepeatIntervalUs() {
    if (!isKdeSession()) return std::nullopt;

    auto val = readKcminputKey("RepeatRate");
    if (val.has_value() && *val > 0) {
        return 1000000 / static_cast<uint64_t>(*val);
    }

    // fallback to kreadconfig6 or kreadconfig5
    FILE* fp = popen("kreadconfig6 --file kcminputrc --group Keyboard --key RepeatRate 2>/dev/null || kreadconfig5 --file kcminputrc --group Keyboard --key RepeatRate 2>/dev/null", "r");
    if (fp) {
        char buf[64];
        std::optional<uint64_t> res;
        if (fgets(buf, sizeof(buf), fp)) {
            int r = 0;
            if (sscanf(buf, "%d", &r) == 1 && r > 0) {
                res = 1000000 / static_cast<uint64_t>(r);
            }
        }
        pclose(fp);
        if (res.has_value()) return res;
    }

    return 40000; // default kde rate 25hz -> 40ms interval
}

} // namespace de
} // namespace clak
