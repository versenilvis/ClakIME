#include "apps/terminal.h"
#include "apps/ide.h"
#include "config/config.h"
#include <cstdio>
#include <cstring>
#include <dirent.h>
#include <chrono>
#include <vector>

namespace clak {
namespace apps {

bool isTerminalApp(const std::string& app) {
    if (isJetBrainsApp(app)) return true;
    static const std::vector<std::string> terms = {
        "kitty", "ghostty", "alacritty", "foot", "wezterm", "xterm",
        "gnome-terminal", "konsole", "tilix", "terminator", "urxvt",
        "terminal", "x-terminal-emulator", "x-terminal-emul"
    };
    for (const auto& t : terms) {
        if (app.find(t) != std::string::npos) return true;
    }
    return false;
}

std::string detectRunningTerminalApp() {
    static int64_t s_last_term_check_ms = 0;
    static std::string s_last_term_app;

    auto now_ms = std::chrono::duration_cast<std::chrono::milliseconds>(
        std::chrono::steady_clock::now().time_since_epoch()).count();
    if (now_ms - s_last_term_check_ms < 2000 && !s_last_term_app.empty()) {
        return s_last_term_app;
    }
    s_last_term_check_ms = now_ms;

    static const char* const kTerminals[] = {
        "kitty", "x-terminal-emulator", "x-terminal-emul", "alacritty", "ghostty", "foot",
        "wezterm", "gnome-terminal", "konsole", "xterm", "terminator", "tilix"
    };

    DIR* proc_dir = opendir("/proc");
    if (!proc_dir) return s_last_term_app;

    struct dirent* ent;
    std::string found;
    while ((ent = readdir(proc_dir)) != nullptr) {
        if (ent->d_name[0] < '0' || ent->d_name[0] > '9') continue;
        char comm_path[64];
        snprintf(comm_path, sizeof(comm_path), "/proc/%s/comm", ent->d_name);
        FILE* f = fopen(comm_path, "r");
        if (!f) continue;
        char comm[64];
        if (fgets(comm, sizeof(comm), f)) {
            size_t len = strlen(comm);
            while (len > 0 && (comm[len - 1] == '\n' || comm[len - 1] == ' ' || comm[len - 1] == '\r')) {
                comm[--len] = '\0';
            }
            for (const char* term : kTerminals) {
                if (strcmp(comm, term) == 0 || strstr(comm, term) != nullptr) {
                    found = (strcmp(comm, "x-terminal-emul") == 0 || strcmp(comm, "x-terminal-emulator") == 0) ? "kitty" : comm;
                    fclose(f);
                    closedir(proc_dir);
                    s_last_term_app = found;
                    return found;
                }
            }
        }
        fclose(f);
    }
    closedir(proc_dir);
    s_last_term_app = found;
    return found;
}

namespace {

class TerminalProfile : public AppProfile {
public:
    std::string name() const override { return "Terminal-Uinput"; }
    bool allowsSurroundingText() const override { return false; }
    bool forcesUinput() const override { return true; }
    UinputPacing uinputPacing() const override {
        return {config::kTerminalPostDelayMs, config::kTerminalGapMs};
    }
};

} // namespace

std::unique_ptr<AppProfile> createTerminalProfile() {
    return std::make_unique<TerminalProfile>();
}

} // namespace apps
} // namespace clak
