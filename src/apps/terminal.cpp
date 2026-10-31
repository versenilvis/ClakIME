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

bool isVteTerminalApp(const std::string& app) {
    static const std::vector<std::string> vte_terms = {
        "ptyxis", "kgx", "gnome-console", "blackbox",
        "gnome-terminal", "gnome-terminal-server",
        "x-terminal-emulator", "x-terminal-emul",
        "tilix", "terminator", "guake", "xfce4-terminal", "mate-terminal",
        "terminal"
    };
    for (const auto& t : vte_terms) {
        if (app.find(t) != std::string::npos) return true;
    }
    return false;
}

bool isTerminalApp(const std::string& app) {
    if (isJetBrainsApp(app)) return true;
    if (isVteTerminalApp(app)) return true;
    static const std::vector<std::string> terms = {
        "kitty", "ghostty", "alacritty", "foot", "wezterm", "xterm",
        "konsole", "yakuake", "urxvt", "terminal"
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
        "kitty", "alacritty", "ghostty", "foot",
        "wezterm", "gnome-terminal", "gnome-terminal-server", "konsole", "yakuake", "xterm",
        "terminator", "tilix", "ptyxis", "kgx", "gnome-console", "blackbox",
        "x-terminal-emulator", "x-terminal-emul"
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
                    found = comm;
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

class VteTerminalProfile : public AppProfile {
public:
    std::string name() const override { return "VTE-Terminal"; }
    bool allowsSurroundingText() const override { return false; }
    bool forcesUinput() const override { return false; }
    bool erasesWithCommittedDel() const override { return true; }
};

} // namespace

std::unique_ptr<AppProfile> createTerminalProfile() {
    return std::make_unique<TerminalProfile>();
}

std::unique_ptr<AppProfile> createVteTerminalProfile() {
    return std::make_unique<VteTerminalProfile>();
}

} // namespace apps
} // namespace clak
