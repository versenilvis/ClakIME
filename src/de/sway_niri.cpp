#include "de/sway_niri.h"

#include <cctype>
#include <cerrno>
#include <cstdint>
#include <cstdlib>
#include <cstring>
#include <poll.h>
#include <sys/socket.h>
#include <sys/un.h>
#include <unistd.h>

namespace clak {
namespace de {

namespace {

constexpr int kIpcTimeoutMs = 25;
constexpr uint32_t kSwayMsgGetTree = 4;
constexpr uint32_t kMaxSwayPayloadBytes = 4 * 1024 * 1024;
constexpr size_t kMaxNiriResponseBytes = 256 * 1024;
constexpr int kMaxJsonDepth = 64;

int connectUnixSocketNonBlocking(const char* path) {
    if (!path || path[0] == '\0') return -1;
    if (std::strlen(path) >= sizeof(sockaddr_un::sun_path)) return -1;

    int fd = socket(AF_UNIX, SOCK_STREAM | SOCK_NONBLOCK | SOCK_CLOEXEC, 0);
    if (fd < 0) return -1;

    sockaddr_un addr{};
    addr.sun_family = AF_UNIX;
    std::strncpy(addr.sun_path, path, sizeof(addr.sun_path) - 1);

    if (connect(fd, reinterpret_cast<sockaddr*>(&addr), sizeof(addr)) < 0) {
        if (errno != EINPROGRESS) {
            close(fd);
            return -1;
        }
        pollfd pfd{fd, POLLOUT, 0};
        if (poll(&pfd, 1, kIpcTimeoutMs) <= 0 || (pfd.revents & (POLLERR | POLLHUP | POLLNVAL))) {
            close(fd);
            return -1;
        }
    }
    return fd;
}

bool writeAll(int fd, const char* data, size_t len) {
    size_t sent = 0;
    while (sent < len) {
        ssize_t n = write(fd, data + sent, len - sent);
        if (n > 0) {
            sent += static_cast<size_t>(n);
            continue;
        }
        if (n < 0 && errno == EINTR) continue;
        if (n < 0 && (errno == EAGAIN || errno == EWOULDBLOCK)) {
            pollfd pfd{fd, POLLOUT, 0};
            if (poll(&pfd, 1, kIpcTimeoutMs) <= 0) return false;
            continue;
        }
        return false;
    }
    return true;
}

bool readAvailable(int fd, std::string& out, size_t max_bytes, bool stop_on_newline, size_t target_bytes) {
    char buf[4096];
    while (out.size() < max_bytes) {
        ssize_t n = read(fd, buf, sizeof(buf));
        if (n > 0) {
            out.append(buf, static_cast<size_t>(n));
            if (stop_on_newline && out.find('\n') != std::string::npos) return true;
            if (target_bytes > 0 && out.size() >= target_bytes) return true;
            continue;
        }
        if (n == 0) return true;
        if (errno == EINTR) continue;
        if (errno == EAGAIN || errno == EWOULDBLOCK) {
            pollfd pfd{fd, POLLIN, 0};
            int pr = poll(&pfd, 1, kIpcTimeoutMs);
            if (pr > 0 && (pfd.revents & POLLIN)) continue;
            if (pr > 0 && (pfd.revents & POLLHUP)) return !out.empty();
            return false;
        }
        return false;
    }
    return false;
}

void appendUtf8(std::string& out, uint32_t cp) {
    if (cp < 0x80) {
        out.push_back(static_cast<char>(cp));
    } else if (cp < 0x800) {
        out.push_back(static_cast<char>(0xC0 | (cp >> 6)));
        out.push_back(static_cast<char>(0x80 | (cp & 0x3F)));
    } else {
        out.push_back(static_cast<char>(0xE0 | (cp >> 12)));
        out.push_back(static_cast<char>(0x80 | ((cp >> 6) & 0x3F)));
        out.push_back(static_cast<char>(0x80 | (cp & 0x3F)));
    }
}

struct JsonCursor {
    const std::string& src;
    size_t pos{0};

    void skipWs() {
        while (pos < src.size() && std::isspace(static_cast<unsigned char>(src[pos]))) {
            ++pos;
        }
    }

    char peek() {
        skipWs();
        return pos < src.size() ? src[pos] : '\0';
    }

    bool consume(char expected) {
        if (peek() != expected) return false;
        ++pos;
        return true;
    }

    bool parseString(std::string& out) {
        if (!consume('"')) return false;
        out.clear();
        while (pos < src.size()) {
            char c = src[pos++];
            if (c == '"') return true;
            if (c == '\\') {
                if (pos >= src.size()) return false;
                char esc = src[pos++];
                switch (esc) {
                    case '"': case '\\': case '/': out.push_back(esc); break;
                    case 'b': out.push_back('\b'); break;
                    case 'f': out.push_back('\f'); break;
                    case 'n': out.push_back('\n'); break;
                    case 'r': out.push_back('\r'); break;
                    case 't': out.push_back('\t'); break;
                    case 'u': {
                        if (pos + 4 > src.size()) return false;
                        uint32_t cp = 0;
                        for (int i = 0; i < 4; ++i) {
                            char h = src[pos++];
                            cp <<= 4;
                            if (h >= '0' && h <= '9') cp |= static_cast<uint32_t>(h - '0');
                            else if (h >= 'a' && h <= 'f') cp |= static_cast<uint32_t>(h - 'a' + 10);
                            else if (h >= 'A' && h <= 'F') cp |= static_cast<uint32_t>(h - 'A' + 10);
                            else return false;
                        }
                        appendUtf8(out, cp);
                        break;
                    }
                    default: out.push_back(esc); break;
                }
            } else {
                out.push_back(c);
            }
        }
        return false;
    }

    bool parseBool(bool& out) {
        skipWs();
        if (src.compare(pos, 4, "true") == 0) {
            pos += 4;
            out = true;
            return true;
        }
        if (src.compare(pos, 5, "false") == 0) {
            pos += 5;
            out = false;
            return true;
        }
        return false;
    }

    bool parseInt(int64_t& out) {
        skipWs();
        if (pos >= src.size()) return false;
        size_t start = pos;
        bool neg = false;
        if (src[pos] == '-') {
            neg = true;
            ++pos;
        }
        if (pos >= src.size() || !std::isdigit(static_cast<unsigned char>(src[pos]))) {
            pos = start;
            return false;
        }
        int64_t val = 0;
        while (pos < src.size() && std::isdigit(static_cast<unsigned char>(src[pos]))) {
            val = val * 10 + (src[pos++] - '0');
        }
        out = neg ? -val : val;
        return true;
    }

    bool skipValue(int depth) {
        if (depth > kMaxJsonDepth) return false;
        char c = peek();
        if (c == '\0') return false;
        if (c == '"') {
            std::string dummy;
            return parseString(dummy);
        }
        if (c == '{') {
            ++pos;
            if (consume('}')) return true;
            std::string key;
            while (pos < src.size()) {
                if (!parseString(key) || !consume(':') || !skipValue(depth + 1)) return false;
                if (consume(',')) continue;
                return consume('}');
            }
            return false;
        }
        if (c == '[') {
            ++pos;
            if (consume(']')) return true;
            while (pos < src.size()) {
                if (!skipValue(depth + 1)) return false;
                if (consume(',')) continue;
                return consume(']');
            }
            return false;
        }
        while (pos < src.size()) {
            char ch = src[pos];
            if (std::isspace(static_cast<unsigned char>(ch)) || ch == ',' || ch == '}' || ch == ']') {
                return true;
            }
            ++pos;
        }
        return true;
    }
};

void parseWindowProperties(JsonCursor& cur, std::string& win_class, std::string& win_title, int depth) {
    if (depth > kMaxJsonDepth || !cur.consume('{')) {
        cur.skipValue(depth);
        return;
    }
    if (cur.consume('}')) return;

    std::string key;
    while (cur.pos < cur.src.size()) {
        if (!cur.parseString(key) || !cur.consume(':')) break;
        if (key == "class" && cur.peek() == '"') {
            cur.parseString(win_class);
        } else if (key == "title" && cur.peek() == '"') {
            cur.parseString(win_title);
        } else {
            cur.skipValue(depth + 1);
        }
        if (cur.consume(',')) continue;
        cur.consume('}');
        break;
    }
}

bool parseSwayNode(JsonCursor& cur, window::WindowInfo& info, int depth) {
    if (depth > kMaxJsonDepth || !cur.consume('{')) return false;
    if (cur.consume('}')) return false;

    bool focused = false;
    std::string app_id;
    std::string name;
    std::string prop_class;
    std::string prop_title;
    int64_t pid = 0;

    std::string key;
    while (cur.pos < cur.src.size()) {
        if (!cur.parseString(key) || !cur.consume(':')) return false;
        if (key == "focused") {
            bool b = false;
            if (cur.parseBool(b)) focused = b;
            else if (!cur.skipValue(depth + 1)) return false;
        } else if (key == "app_id" && cur.peek() == '"') {
            if (!cur.parseString(app_id)) return false;
        } else if (key == "name" && cur.peek() == '"') {
            if (!cur.parseString(name)) return false;
        } else if (key == "pid" && (std::isdigit(static_cast<unsigned char>(cur.peek())) || cur.peek() == '-')) {
            if (!cur.parseInt(pid)) return false;
        } else if (key == "window_properties" && cur.peek() == '{') {
            parseWindowProperties(cur, prop_class, prop_title, depth + 1);
        } else if ((key == "nodes" || key == "floating_nodes") && cur.peek() == '[') {
            cur.consume('[');
            if (!cur.consume(']')) {
                while (cur.pos < cur.src.size()) {
                    if (cur.peek() == '{') {
                        if (parseSwayNode(cur, info, depth + 1)) return true;
                    } else if (!cur.skipValue(depth + 1)) {
                        return false;
                    }
                    if (cur.consume(',')) continue;
                    if (!cur.consume(']')) return false;
                    break;
                }
            }
        } else {
            if (!cur.skipValue(depth + 1)) return false;
        }

        if (cur.consume(',')) continue;
        if (!cur.consume('}')) return false;
        break;
    }

    // ignore non-window containers like empty workspaces
    if (focused && (pid > 0 || !app_id.empty() || !prop_class.empty())) {
        info.win_class = !app_id.empty() ? app_id : prop_class;
        info.win_title = !name.empty() ? name : prop_title;
        info.pid = pid > 0 ? static_cast<pid_t>(pid) : 0;
        return true;
    }
    return false;
}

bool parseNiriWindowObject(JsonCursor& cur, window::WindowInfo& info, int depth) {
    if (depth > kMaxJsonDepth || !cur.consume('{')) return false;
    if (cur.consume('}')) return false;

    std::string app_id;
    std::string title;
    int64_t pid = 0;

    std::string key;
    while (cur.pos < cur.src.size()) {
        if (!cur.parseString(key) || !cur.consume(':')) return false;
        if (key == "app_id" && cur.peek() == '"') {
            if (!cur.parseString(app_id)) return false;
        } else if (key == "title" && cur.peek() == '"') {
            if (!cur.parseString(title)) return false;
        } else if (key == "pid" && (std::isdigit(static_cast<unsigned char>(cur.peek())) || cur.peek() == '-')) {
            if (!cur.parseInt(pid)) return false;
        } else {
            if (!cur.skipValue(depth + 1)) return false;
        }
        if (cur.consume(',')) continue;
        if (!cur.consume('}')) return false;
        break;
    }

    if (app_id.empty() && pid <= 0) return false;
    info.win_class = app_id;
    info.win_title = title;
    info.pid = pid > 0 ? static_cast<pid_t>(pid) : 0;
    return true;
}

} // namespace

bool parseSwayTreeJson(const std::string& json, window::WindowInfo& info) {
    JsonCursor cur{json, 0};
    return parseSwayNode(cur, info, 0);
}

bool parseNiriFocusedWindowJson(const std::string& json, window::WindowInfo& info) {
    JsonCursor cur{json, 0};
    if (!cur.consume('{') || cur.consume('}')) return false;

    std::string key;
    while (cur.pos < cur.src.size()) {
        if (!cur.parseString(key) || !cur.consume(':')) return false;
        if (key == "Ok" && cur.peek() == '{') {
            cur.consume('{');
            if (cur.consume('}')) return false;
            std::string ok_key;
            while (cur.pos < cur.src.size()) {
                if (!cur.parseString(ok_key) || !cur.consume(':')) return false;
                if (ok_key == "FocusedWindow") {
                    if (cur.peek() != '{') return false;
                    return parseNiriWindowObject(cur, info, 1);
                }
                if (!cur.skipValue(2)) return false;
                if (cur.consume(',')) continue;
                cur.consume('}');
                break;
            }
        } else {
            if (!cur.skipValue(1)) return false;
        }
        if (cur.consume(',')) continue;
        cur.consume('}');
        break;
    }
    return false;
}

bool querySwayActiveWindow(window::WindowInfo& info) {
    const char* sock = std::getenv("SWAYSOCK");
    if (!sock || sock[0] == '\0') return false;

    int fd = connectUnixSocketNonBlocking(sock);
    if (fd < 0) return false;

    char req[14];
    std::memcpy(req, "i3-ipc", 6);
    uint32_t payload_len = 0;
    uint32_t msg_type = kSwayMsgGetTree;
    std::memcpy(req + 6, &payload_len, 4);
    std::memcpy(req + 10, &msg_type, 4);

    if (!writeAll(fd, req, sizeof(req))) {
        close(fd);
        return false;
    }

    std::string raw;
    readAvailable(fd, raw, 14, false, 14);
    if (raw.size() < 14 || std::memcmp(raw.data(), "i3-ipc", 6) != 0) {
        close(fd);
        return false;
    }

    uint32_t resp_len = 0;
    uint32_t resp_type = 0;
    std::memcpy(&resp_len, raw.data() + 6, 4);
    std::memcpy(&resp_type, raw.data() + 10, 4);
    if (resp_type != kSwayMsgGetTree || resp_len == 0 || resp_len > kMaxSwayPayloadBytes) {
        close(fd);
        return false;
    }

    size_t total_expected = 14 + static_cast<size_t>(resp_len);
    if (raw.size() < total_expected) {
        readAvailable(fd, raw, total_expected, false, total_expected);
    }
    close(fd);

    if (raw.size() < total_expected) return false;
    std::string json = raw.substr(14, resp_len);
    return parseSwayTreeJson(json, info);
}

bool queryNiriActiveWindow(window::WindowInfo& info) {
    const char* sock = std::getenv("NIRI_SOCKET");
    if (!sock || sock[0] == '\0') return false;

    int fd = connectUnixSocketNonBlocking(sock);
    if (fd < 0) return false;

    static constexpr char kReq[] = "\"FocusedWindow\"\n";
    if (!writeAll(fd, kReq, sizeof(kReq) - 1)) {
        close(fd);
        return false;
    }

    std::string raw;
    bool got = readAvailable(fd, raw, kMaxNiriResponseBytes, true, 0);
    close(fd);
    if (!got && raw.empty()) return false;

    return parseNiriFocusedWindowJson(raw, info);
}

} // namespace de
} // namespace clak
