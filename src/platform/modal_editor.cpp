#include "modal_editor.h"
#include "config/sites.h"
#include <vector>
#include <unordered_set>
#include <fstream>
#include <cstring>
#include <cstdlib>
#include <unistd.h>
#include <dirent.h>

namespace clak {
namespace platform {

namespace {

pid_t getProcessPpid(pid_t pid) {
    std::string path = "/proc/" + std::to_string(pid) + "/stat";
    std::ifstream f(path);
    if (!f.is_open()) return 0;
    std::string s;
    if (!std::getline(f, s)) return 0;
    size_t rp = s.rfind(')');
    if (rp == std::string::npos || rp + 2 >= s.size()) return 0;
    const char* ptr = s.c_str() + rp + 2;
    while (*ptr && *ptr != ' ') ptr++;
    while (*ptr == ' ') ptr++;
    return static_cast<pid_t>(atoi(ptr));
}

bool isProcessDescendantOf(pid_t child, pid_t target_parent) {
    if (target_parent <= 1 || child <= 1) return false;
    pid_t cur = child;
    int limit = 16;
    while (cur > 1 && limit-- > 0) {
        if (cur == target_parent) return true;
        pid_t ppid = getProcessPpid(cur);
        if (ppid <= 1 || ppid == cur) break;
        cur = ppid;
    }
    return false;
}

bool hasAncestorMatching(pid_t child, const std::string& target_class) {
    if (child <= 1 || target_class.empty()) return false;
    pid_t cur = child;
    int limit = 16;
    while (cur > 1 && limit-- > 0) {
        std::string comm_path = "/proc/" + std::to_string(cur) + "/comm";
        std::ifstream f(comm_path);
        if (f.is_open()) {
            std::string comm;
            if (f >> comm) {
                for (char& c : comm) c = tolower(c);
                if (target_class.find(comm) != std::string::npos || comm.find(target_class) != std::string::npos ||
                    (config::isTerminalApp(target_class) && config::isTerminalApp(comm))) {
                    return true;
                }
            }
        }
        pid_t ppid = getProcessPpid(cur);
        if (ppid <= 1 || ppid == cur) break;
        cur = ppid;
    }
    return false;
}

bool isForegroundTerminalProcess(pid_t pid) {
    std::string path = "/proc/" + std::to_string(pid) + "/stat";
    std::ifstream f(path);
    if (!f.is_open()) return false;
    std::string s;
    if (!std::getline(f, s)) return false;
    size_t rp = s.rfind(')');
    if (rp == std::string::npos || rp + 2 >= s.size()) return false;
    const char* ptr = s.c_str() + rp + 2;

    while (*ptr && *ptr != ' ') ptr++;
    while (*ptr == ' ') ptr++;
    while (*ptr && *ptr != ' ') ptr++;
    while (*ptr == ' ') ptr++;
    char* end = nullptr;
    long pgrp = strtol(ptr, &end, 10);
    ptr = end;
    while (*ptr == ' ') ptr++;
    while (*ptr && *ptr != ' ') ptr++;
    while (*ptr == ' ') ptr++;
    long tty_nr = strtol(ptr, &end, 10);
    ptr = end;
    while (*ptr == ' ') ptr++;
    long tpgid = strtol(ptr, &end, 10);

    return (tty_nr != 0 && pgrp > 0 && pgrp == tpgid);
}

} // namespace

bool isEditorActive(const WindowInfo& win) {
    std::string lower_class = win.win_class;
    for (char& c : lower_class) c = tolower(c);

    bool is_term = config::isTerminalApp(lower_class);
    bool is_gui_editor = (lower_class.find("neovide") != std::string::npos ||
                          lower_class.find("gvim") != std::string::npos);
    if (!is_term && !is_gui_editor) {
        return false;
    }

    if (is_gui_editor) {
        return true;
    }

    static const std::vector<std::string> keywords = {
        "nvim", "neovim", "vim", "helix", "hx"
    };

    std::string lower_title = win.win_title;
    for (char& c : lower_title) c = tolower(c);

    for (const auto& kw : keywords) {
        if (lower_title.find(kw) != std::string::npos) {
            return true;
        }
    }

    // check if tmux is running an editor in active pane
    pid_t tmux_pane_pid = 0;
    FILE* fp = popen("tmux display-message -p '#{pane_pid} #{pane_current_command}' 2>/dev/null", "r");
    if (fp) {
        char buf[128];
        if (fgets(buf, sizeof(buf), fp)) {
            char* p = buf;
            while (*p == ' ' || *p == '\t') p++;
            tmux_pane_pid = static_cast<pid_t>(atoi(p));
            while (*p && *p != ' ' && *p != '\t') p++;
            while (*p == ' ' || *p == '\t') p++;
            char* end = p;
            while (*end && *end != '\n' && *end != '\r') end++;
            *end = '\0';
            std::string t_cmd = p;
            for (char& c : t_cmd) c = tolower(c);
            for (const auto& kw : keywords) {
                if (t_cmd == kw) {
                    pclose(fp);
                    return true;
                }
            }
        }
        pclose(fp);
    }

    DIR* dir = opendir("/proc");
    if (!dir) return false;

    static const std::unordered_set<std::string> editor_comms = {
        "nvim", "vim", "vi", "helix", "hx", "emacs"
    };

    struct dirent* ent;
    bool found = false;
    while ((ent = readdir(dir)) != nullptr) {
        if (ent->d_name[0] < '0' || ent->d_name[0] > '9') continue;
        pid_t p = static_cast<pid_t>(atoi(ent->d_name));
        std::string comm_path = std::string("/proc/") + ent->d_name + "/comm";
        std::ifstream f(comm_path);
        if (!f.is_open()) continue;
        std::string comm;
        f >> comm;
        if (editor_comms.count(comm) > 0) {
            if (win.pid > 1 && isProcessDescendantOf(p, win.pid)) {
                found = true;
                break;
            }
            if (tmux_pane_pid > 1 && isProcessDescendantOf(p, tmux_pane_pid)) {
                found = true;
                break;
            }
            // fallback for desktop environments without window pid (e.g. gnome)
            if (win.pid <= 1 && isForegroundTerminalProcess(p) && hasAncestorMatching(p, lower_class)) {
                found = true;
                break;
            }
        }
    }
    closedir(dir);
    return found;
}

} // namespace platform
} // namespace clak
