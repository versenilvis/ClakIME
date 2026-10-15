#include "de/hyprland.h"
#include <cstdlib>
#include <cstring>
#include <unistd.h>
#include <dirent.h>
#include <sys/socket.h>
#include <sys/un.h>
#include <sys/poll.h>
#include <cstdio>
#include <string>

namespace clak {
namespace de {

static std::string s_hypr_sock_path;

bool queryHyprlandActiveWindow(window::WindowInfo& info) {
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

std::optional<uint64_t> queryHyprlandRepeatDelayUs() {
    if (!getenv("HYPRLAND_INSTANCE_SIGNATURE")) return std::nullopt;
    FILE* fp = popen("hyprctl getoption input:repeat_delay 2>/dev/null", "r");
    if (!fp) return std::nullopt;

    char buf[128];
    std::optional<uint64_t> result;
    while (fgets(buf, sizeof(buf), fp)) {
        int val = 0;
        if (sscanf(buf, "int: %d", &val) == 1 && val > 0) {
            result = static_cast<uint64_t>(val) * 1000;
            break;
        }
    }
    pclose(fp);
    return result;
}

std::optional<uint64_t> queryHyprlandRepeatIntervalUs() {
    if (!getenv("HYPRLAND_INSTANCE_SIGNATURE")) return std::nullopt;
    FILE* fp = popen("hyprctl getoption input:repeat_rate 2>/dev/null", "r");
    if (!fp) return std::nullopt;

    char buf[128];
    std::optional<uint64_t> result;
    while (fgets(buf, sizeof(buf), fp)) {
        int val = 0;
        if (sscanf(buf, "int: %d", &val) == 1 && val > 0) {
            result = 1000000 / val;
            break;
        }
    }
    pclose(fp);
    return result;
}

} // namespace de
} // namespace clak
