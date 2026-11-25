#include <gtest/gtest.h>

#include "de/sway_niri.h"
#include "window/window_info.h"

#include <atomic>
#include <cstdint>
#include <cstdlib>
#include <cstring>
#include <poll.h>
#include <string>
#include <sys/socket.h>
#include <sys/un.h>
#include <thread>
#include <unistd.h>

namespace {

class ScopedEnvVar {
public:
    ScopedEnvVar(const char* name, const std::string& val) : name_(name) {
        const char* prev = std::getenv(name);
        if (prev) {
            had_prev_ = true;
            prev_ = prev;
        }
        setenv(name_.c_str(), val.c_str(), 1);
    }

    ~ScopedEnvVar() {
        if (had_prev_) {
            setenv(name_.c_str(), prev_.c_str(), 1);
        } else {
            unsetenv(name_.c_str());
        }
    }

private:
    std::string name_;
    bool had_prev_{false};
    std::string prev_;
};

class UnixMockServer {
public:
    UnixMockServer(const std::string& sock_path, std::string response_bytes)
        : path_(sock_path), response_(std::move(response_bytes)) {
        unlink(path_.c_str());
        listen_fd_ = socket(AF_UNIX, SOCK_STREAM | SOCK_CLOEXEC, 0);
        if (listen_fd_ < 0) return;

        sockaddr_un addr{};
        addr.sun_family = AF_UNIX;
        std::strncpy(addr.sun_path, path_.c_str(), sizeof(addr.sun_path) - 1);
        if (bind(listen_fd_, reinterpret_cast<sockaddr*>(&addr), sizeof(addr)) < 0 ||
            listen(listen_fd_, 4) < 0) {
            close(listen_fd_);
            listen_fd_ = -1;
            return;
        }

        worker_ = std::thread([this]() {
            pollfd pfd{listen_fd_, POLLIN, 0};
            while (!stop_.load()) {
                int pr = poll(&pfd, 1, 50);
                if (pr <= 0) continue;
                int client = accept(listen_fd_, nullptr, nullptr);
                if (client < 0) continue;
                char req_buf[256];
                read(client, req_buf, sizeof(req_buf));
                size_t sent = 0;
                while (sent < response_.size()) {
                    ssize_t n = write(client, response_.data() + sent, response_.size() - sent);
                    if (n <= 0) break;
                    sent += static_cast<size_t>(n);
                }
                close(client);
            }
        });
    }

    ~UnixMockServer() {
        stop_.store(true);
        if (worker_.joinable()) worker_.join();
        if (listen_fd_ >= 0) close(listen_fd_);
        unlink(path_.c_str());
    }

    bool valid() const { return listen_fd_ >= 0; }

private:
    std::string path_;
    std::string response_;
    int listen_fd_{-1};
    std::atomic<bool> stop_{false};
    std::thread worker_;
};

std::string makeSwayIpcFrame(uint32_t type, const std::string& json) {
    std::string frame(14 + json.size(), '\0');
    std::memcpy(frame.data(), "i3-ipc", 6);
    uint32_t len = static_cast<uint32_t>(json.size());
    std::memcpy(frame.data() + 6, &len, 4);
    std::memcpy(frame.data() + 10, &type, 4);
    std::memcpy(frame.data() + 14, json.data(), json.size());
    return frame;
}

TEST(WindowTrackerTest, ParseSwayTree_WaylandFocusedWindow) {
    const std::string tree = R"({
        "type": "root",
        "focused": false,
        "nodes": [
            {
                "type": "workspace",
                "focused": false,
                "nodes": [
                    {"type": "con", "focused": false, "app_id": "firefox", "name": "Other", "pid": 111},
                    {"type": "con", "focused": false, "app_id": null, "nodes": []}
                ],
                "floating_nodes": [
                    {"type": "floating_con", "focused": true, "app_id": "kitty", "name": "nvim README.md", "pid": 4242}
                ]
            }
        ]
    })";

    clak::window::WindowInfo info;
    EXPECT_TRUE(clak::de::parseSwayTreeJson(tree, info));
    EXPECT_EQ(info.win_class, "kitty");
    EXPECT_EQ(info.win_title, "nvim README.md");
    EXPECT_EQ(info.pid, 4242);
}

TEST(WindowTrackerTest, ParseSwayTree_XWaylandWindowProperties) {
    const std::string tree = R"({
        "type": "root",
        "focused": false,
        "nodes": [{
            "type": "con",
            "focused": true,
            "app_id": null,
            "name": "Threads \u2014 Chromium",
            "pid": 9001,
            "window_properties": {
                "class": "Google-chrome",
                "instance": "google-chrome",
                "title": "Fallback Title"
            }
        }]
    })";

    clak::window::WindowInfo info;
    EXPECT_TRUE(clak::de::parseSwayTreeJson(tree, info));
    EXPECT_EQ(info.win_class, "Google-chrome");
    EXPECT_NE(info.win_title.find("Chromium"), std::string::npos);
    EXPECT_EQ(info.pid, 9001);
}

TEST(WindowTrackerTest, ParseSwayTree_EmptyWorkspaceAndMalformedJson) {
    const std::string empty_ws = R"({"type":"workspace","focused":true,"nodes":[],"floating_nodes":[]})";
    clak::window::WindowInfo info;
    EXPECT_FALSE(clak::de::parseSwayTreeJson(empty_ws, info));

    const std::string truncated = R"({"type":"root","nodes":[{"focused":true,"app_id":"alacritty")";
    EXPECT_FALSE(clak::de::parseSwayTreeJson(truncated, info));
}

TEST(WindowTrackerTest, ParseNiriFocusedWindow_ValidNullAndError) {
    const std::string ok_json = R"({"Ok":{"FocusedWindow":{"id":12,"title":"Clak \u00e1","app_id":"Alacritty","pid":5075,"is_focused":true}}})";
    clak::window::WindowInfo info;
    EXPECT_TRUE(clak::de::parseNiriFocusedWindowJson(ok_json, info));
    EXPECT_EQ(info.win_class, "Alacritty");
    EXPECT_EQ(info.win_title, "Clak \xC3\xA1");
    EXPECT_EQ(info.pid, 5075);

    clak::window::WindowInfo null_info;
    EXPECT_FALSE(clak::de::parseNiriFocusedWindowJson(R"({"Ok":{"FocusedWindow":null}})", null_info));
    EXPECT_FALSE(clak::de::parseNiriFocusedWindowJson(R"({"Err":"unknown request"})", null_info));
}

TEST(WindowTrackerTest, QuerySwayActiveWindow_OverUnixSocket) {
    const std::string sock = "/tmp/clak_test_sway_" + std::to_string(getpid()) + ".sock";
    const std::string payload = R"({"type":"root","nodes":[{"type":"con","focused":true,"app_id":"org.wezfurlong.wezterm","name":"zsh","pid":7777}]})";
    UnixMockServer server(sock, makeSwayIpcFrame(4, payload));
    ASSERT_TRUE(server.valid());

    ScopedEnvVar env_sway("SWAYSOCK", sock);
    clak::window::WindowInfo info;
    EXPECT_TRUE(clak::de::querySwayActiveWindow(info));
    EXPECT_EQ(info.win_class, "org.wezfurlong.wezterm");
    EXPECT_EQ(info.win_title, "zsh");
    EXPECT_EQ(info.pid, 7777);
}

TEST(WindowTrackerTest, QueryNiriActiveWindow_OverUnixSocket) {
    const std::string sock = "/tmp/clak_test_niri_" + std::to_string(getpid()) + ".sock";
    const std::string payload = "{\"Ok\":{\"FocusedWindow\":{\"id\":3,\"title\":\"Ghostty\",\"app_id\":\"com.mitchellh.ghostty\",\"pid\":8888}}}\n";
    UnixMockServer server(sock, payload);
    ASSERT_TRUE(server.valid());

    ScopedEnvVar env_niri("NIRI_SOCKET", sock);
    clak::window::WindowInfo info;
    EXPECT_TRUE(clak::de::queryNiriActiveWindow(info));
    EXPECT_EQ(info.win_class, "com.mitchellh.ghostty");
    EXPECT_EQ(info.win_title, "Ghostty");
    EXPECT_EQ(info.pid, 8888);
}

TEST(WindowTrackerTest, QuerySwayActiveWindow_LargeTreeOverUnixSocket) {
    const std::string sock = "/tmp/clak_test_sway_large_" + std::to_string(getpid()) + ".sock";
    std::string filler(6000, 'x');
    const std::string payload =
        R"({"type":"root","nodes":[{"type":"con","focused":false,"app_id":"dummy","name":")" +
        filler +
        R"("},{"type":"con","focused":true,"app_id":"foot","name":"bash","pid":6123}]})";
    UnixMockServer server(sock, makeSwayIpcFrame(4, payload));
    ASSERT_TRUE(server.valid());

    ScopedEnvVar env_sway("SWAYSOCK", sock);
    clak::window::WindowInfo info;
    EXPECT_TRUE(clak::de::querySwayActiveWindow(info));
    EXPECT_EQ(info.win_class, "foot");
    EXPECT_EQ(info.win_title, "bash");
    EXPECT_EQ(info.pid, 6123);
}

TEST(WindowTrackerTest, GetActiveWindow_MockOverridesCompositorsAndClears) {
    clak::window::WindowInfo mock_info{"sway-app", "Sway Title", 3210};
    clak::window::setMockActiveWindow(mock_info);
    clak::window::clearWindowCache();

    clak::window::WindowInfo actual = clak::window::getActiveWindow("fallback");
    EXPECT_EQ(actual.win_class, "sway-app");
    EXPECT_EQ(actual.win_title, "Sway Title");
    EXPECT_EQ(actual.pid, 3210);

    clak::window::clearMockActiveWindow();
    clak::window::clearWindowCache();
}

} // namespace
