#include "window_info.h"
#include <cstdlib>
#include <cstring>
#include <unistd.h>
#include <dirent.h>
#include <sys/socket.h>
#include <sys/un.h>
#include <sys/poll.h>
#include <chrono>
#include <dlfcn.h>
#include <cctype>

namespace clak {
namespace platform {

static std::optional<WindowInfo> s_mock_window_info;

void setMockActiveWindow(std::optional<WindowInfo> info) {
    s_mock_window_info = std::move(info);
}

void clearMockActiveWindow() {
    s_mock_window_info = std::nullopt;
}

static WindowInfo s_cached_info;
static int64_t s_last_ms = 0;

void clearWindowCache() {
    s_last_ms = 0;
    s_cached_info = WindowInfo{};
}

// -----------------------------------------------------------------------------
// Hyprland UNIX domain socket query
// -----------------------------------------------------------------------------
static std::string s_hypr_sock_path;

static bool queryHyprlandActiveWindow(WindowInfo& info) {
    const char* xdg = getenv("XDG_RUNTIME_DIR");
    if (!xdg) return false;

    if (s_hypr_sock_path.empty()) {
        std::string sig;
        const char* env_sig = getenv("HYPRLAND_INSTANCE_SIGNATURE");
        if (env_sig && env_sig[0] != '\0') {
            sig = env_sig;
        } else {
            std::string hypr_dir = std::string(xdg) + "/hypr";
            DIR* d = opendir(hypr_dir.c_str());
            if (d) {
                struct dirent* ent;
                while ((ent = readdir(d)) != nullptr) {
                    if (ent->d_name[0] != '.') {
                        sig = ent->d_name;
                        break;
                    }
                }
                closedir(d);
            }
        }
        if (sig.empty() || sig.find('/') != std::string::npos || sig.find("..") != std::string::npos) {
            return false;
        }
        s_hypr_sock_path = std::string(xdg) + "/hypr/" + sig + "/.socket.sock";
    }

    int fd = socket(AF_UNIX, SOCK_STREAM | SOCK_NONBLOCK | SOCK_CLOEXEC, 0);
    if (fd < 0) return false;

    struct sockaddr_un addr{};
    addr.sun_family = AF_UNIX;
    if (s_hypr_sock_path.length() >= sizeof(addr.sun_path)) {
        close(fd);
        s_hypr_sock_path.clear();
        return false;
    }
    strncpy(addr.sun_path, s_hypr_sock_path.c_str(), sizeof(addr.sun_path) - 1);
    addr.sun_path[sizeof(addr.sun_path) - 1] = '\0';
    socklen_t len = offsetof(struct sockaddr_un, sun_path) + strlen(addr.sun_path) + 1;

    if (connect(fd, reinterpret_cast<struct sockaddr*>(&addr), len) != 0) {
        close(fd);
        s_hypr_sock_path.clear();
        return false;
    }

    const char* cmd = "j/activewindow";
    send(fd, cmd, strlen(cmd), MSG_NOSIGNAL);

    std::string response;
    char buf[4096];
    struct pollfd pfd{fd, POLLIN, 0};
    if (poll(&pfd, 1, 15) > 0) {
        ssize_t n = recv(fd, buf, sizeof(buf) - 1, 0);
        if (n > 0) {
            buf[n] = '\0';
            response.assign(buf, n);
        }
    }
    close(fd);

    const char* p = strstr(response.c_str(), "\"pid\":");
    if (p) {
        p += 6;
        while (*p == ' ' || *p == '\t') p++;
        info.pid = static_cast<pid_t>(atoi(p));
    }

    p = strstr(response.c_str(), "\"class\":");
    if (p) {
        p += 8;
        while (*p == ' ' || *p == '\t' || *p == '\"') p++;
        const char* end = strchr(p, '\"');
        if (end) info.win_class = std::string(p, end - p);
    }

    p = strstr(response.c_str(), "\"title\":");
    if (p) {
        p += 8;
        while (*p == ' ' || *p == '\t' || *p == '\"') p++;
        const char* end = strchr(p, '\"');
        if (end) info.win_title = std::string(p, end - p);
    }

    return !info.win_class.empty();
}

// -----------------------------------------------------------------------------
// AT-SPI2 dynamic query for GNOME / generic Wayland
// -----------------------------------------------------------------------------
typedef void* (*fn_get_desktop)(int);
typedef int (*fn_get_child_count)(void*);
typedef void* (*fn_get_child_at_index)(void*, int);
typedef char* (*fn_get_name)(void*);
typedef void* (*fn_get_state_set)(void*);
typedef int (*fn_state_contains)(void*, int);
typedef void (*fn_unref)(void*);
typedef void (*fn_free)(void*);

#define ATSPI_STATE_ACTIVE 1

static void* s_atspi_handle = nullptr;
static fn_get_desktop p_get_desktop = nullptr;
static fn_get_child_count p_count = nullptr;
static fn_get_child_at_index p_child = nullptr;
static fn_get_name p_name = nullptr;
static fn_get_state_set p_state_set = nullptr;
static fn_state_contains p_contains = nullptr;
static fn_unref p_unref = nullptr;
static fn_free p_free = nullptr;
static bool s_atspi_inited = false;

static bool initAtspi() {
    if (s_atspi_inited) return (s_atspi_handle != nullptr);
    s_atspi_inited = true;
    s_atspi_handle = dlopen("libatspi.so.0", RTLD_LAZY);
    if (!s_atspi_handle) return false;
    p_get_desktop = (fn_get_desktop)dlsym(s_atspi_handle, "atspi_get_desktop");
    p_count = (fn_get_child_count)dlsym(s_atspi_handle, "atspi_accessible_get_child_count");
    p_child = (fn_get_child_at_index)dlsym(s_atspi_handle, "atspi_accessible_get_child_at_index");
    p_name = (fn_get_name)dlsym(s_atspi_handle, "atspi_accessible_get_name");
    p_state_set = (fn_get_state_set)dlsym(s_atspi_handle, "atspi_accessible_get_state_set");
    p_contains = (fn_state_contains)dlsym(s_atspi_handle, "atspi_state_set_contains");
    p_unref = (fn_unref)dlsym(s_atspi_handle, "g_object_unref");
    p_free = (fn_free)dlsym(s_atspi_handle, "g_free");
    return (p_get_desktop && p_count && p_child && p_name && p_state_set && p_contains);
}

static std::string normalizeAppName(const std::string& raw_name) {
    std::string lower = raw_name;
    for (char& c : lower) c = tolower(static_cast<unsigned char>(c));
    if (lower.find("visual studio code") != std::string::npos || lower == "code") return "code";
    if (lower.find("brave") != std::string::npos) return "brave";
    if (lower.find("chrome") != std::string::npos || lower.find("chromium") != std::string::npos) return "google-chrome";
    if (lower.find("firefox") != std::string::npos) return "firefox";
    if (lower.find("zen") != std::string::npos) return "zen";
    return lower;
}

static bool queryAtspiActiveWindow(WindowInfo& info) {
    if (!initAtspi()) return false;
    void* desktop = p_get_desktop(0);
    if (!desktop) return false;
    int app_count = p_count(desktop);
    bool found = false;
    for (int i = 0; i < app_count; ++i) {
        void* app = p_child(desktop, i);
        if (!app) continue;
        int wc = p_count(app);
        for (int j = 0; j < wc; ++j) {
            void* win = p_child(app, j);
            if (!win) continue;
            void* st = p_state_set(win);
            if (st && p_contains(st, ATSPI_STATE_ACTIVE)) {
                char* an = p_name(app);
                char* wn = p_name(win);
                if (an) {
                    info.win_class = normalizeAppName(an);
                    if (p_free) p_free(an);
                }
                if (wn) {
                    info.win_title = wn;
                    if (p_free) p_free(wn);
                }
                found = true;
            }
            if (st && p_unref) p_unref(st);
            if (p_unref) p_unref(win);
            if (found) break;
        }
        if (p_unref) p_unref(app);
        if (found) break;
    }
    return found;
}

// -----------------------------------------------------------------------------
// Terminal fallback (for non-AT-SPI terminal emulators like kitty, alacritty, foot)
// -----------------------------------------------------------------------------
static std::string detectRunningTerminalApp() {
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

WindowInfo getActiveWindow(const std::string& fallback_app) {
    if (s_mock_window_info.has_value()) {
        WindowInfo info = *s_mock_window_info;
        if (info.win_class.empty() && !fallback_app.empty()) {
            info.win_class = fallback_app;
        }
        return info;
    }

    auto now_ms = std::chrono::duration_cast<std::chrono::milliseconds>(
        std::chrono::steady_clock::now().time_since_epoch()).count();

    // cache window info for 500ms to avoid querying compositor on every keystroke
    if (now_ms - s_last_ms < 500 && !s_cached_info.win_class.empty()) {
        if (fallback_app.empty() || s_cached_info.win_class == fallback_app) {
            return s_cached_info;
        }
    }
    s_last_ms = now_ms;

    WindowInfo info;

    // 1. First try Hyprland if available
    bool hypr_ok = queryHyprlandActiveWindow(info);

    // 2. If not on Hyprland (e.g. GNOME Wayland), query AT-SPI
    if (!hypr_ok || info.win_class.empty()) {
        if (queryAtspiActiveWindow(info)) {
            // Found active window via AT-SPI (Brave, Chrome, Antigravity, etc.)
        } else {
            // AT-SPI has no active window. Check if a non-AT-SPI terminal is running
            std::string term = detectRunningTerminalApp();
            if (!term.empty()) {
                info.win_class = term;
                info.win_title = term;
            }
        }
    }

    if (info.win_class.empty() && !fallback_app.empty()) {
        info.win_class = fallback_app;
    }

    if (!info.win_class.empty()) {
        s_cached_info = info;
    }
    return s_cached_info.win_class.empty() ? info : s_cached_info;
}

} // namespace platform
} // namespace clak
