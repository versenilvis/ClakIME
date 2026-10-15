#pragma once

#include <functional>
#include <memory>
#include <fcitx-utils/event.h>

struct libinput;
struct udev;

namespace clak {
namespace mouse {

// theo doi su kien click chuot qua libinput de reset composition
class MouseTracker {
public:
    using ClickCallback = std::function<void()>;

    explicit MouseTracker(fcitx::EventLoop& eventLoop, ClickCallback callback);
    ~MouseTracker();

    bool isAvailable() const { return available_; }

private:
    fcitx::EventLoop& event_loop_;
    ClickCallback callback_;
    struct udev* udev_{nullptr};
    struct libinput* li_{nullptr};
    int li_fd_{-1};
    std::unique_ptr<fcitx::EventSourceIO> io_event_;
    bool available_{false};

    void processEvents();
};

} // namespace mouse
} // namespace clak
