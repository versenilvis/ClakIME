#include "mouse/mouse_tracker.h"
#include "utils/log.h"
#include <libinput.h>
#include <libudev.h>
#include <fcntl.h>
#include <unistd.h>
#include <errno.h>

namespace clak {
namespace mouse {

namespace {
int open_restricted(const char* path, int flags, void* user_data) {
    (void)user_data;
    int fd = open(path, flags | O_CLOEXEC);
    return fd < 0 ? -errno : fd;
}

void close_restricted(int fd, void* user_data) {
    (void)user_data;
    close(fd);
}

const struct libinput_interface s_interface = {
    .open_restricted = open_restricted,
    .close_restricted = close_restricted,
};
} // namespace

MouseTracker::MouseTracker(fcitx::EventLoop& eventLoop, ClickCallback callback)
    : event_loop_(eventLoop), callback_(std::move(callback)) {
    udev_ = udev_new();
    if (!udev_) {
        utils::clakLog("mouse_tracker: failed to create udev context");
        return;
    }

    li_ = libinput_udev_create_context(&s_interface, nullptr, udev_);
    if (!li_) {
        utils::clakLog("mouse_tracker: failed to create libinput udev context");
        udev_unref(udev_);
        udev_ = nullptr;
        return;
    }

    if (libinput_udev_assign_seat(li_, "seat0") != 0) {
        utils::clakLog("mouse_tracker: failed to assign seat0 (check permissions on /dev/input)");
        libinput_unref(li_);
        udev_unref(udev_);
        li_ = nullptr;
        udev_ = nullptr;
        return;
    }

    li_fd_ = libinput_get_fd(li_);
    if (li_fd_ < 0) {
        utils::clakLog("mouse_tracker: failed to get libinput fd");
        libinput_unref(li_);
        udev_unref(udev_);
        li_ = nullptr;
        udev_ = nullptr;
        return;
    }

    io_event_ = event_loop_.addIOEvent(
        li_fd_,
        fcitx::IOEventFlag::In,
        [this](fcitx::EventSourceIO*, int, fcitx::IOEventFlags) {
            processEvents();
            return true;
        }
    );

    available_ = true;
    utils::clakLog("mouse_tracker: initialized successfully on seat0");
}

MouseTracker::~MouseTracker() {
    if (io_event_) {
        io_event_.reset();
    }
    if (li_) {
        libinput_unref(li_);
    }
    if (udev_) {
        udev_unref(udev_);
    }
}

void MouseTracker::processEvents() {
    if (!li_) return;
    if (libinput_dispatch(li_) != 0) return;

    struct libinput_event* ev = nullptr;
    bool clicked = false;

    while ((ev = libinput_get_event(li_)) != nullptr) {
        enum libinput_event_type type = libinput_event_get_type(ev);
        if (type == LIBINPUT_EVENT_POINTER_BUTTON) {
            auto* p_ev = libinput_event_get_pointer_event(ev);
            if (libinput_event_pointer_get_button_state(p_ev) == LIBINPUT_BUTTON_STATE_PRESSED) {
                clicked = true;
            }
        }
        libinput_event_destroy(ev);
    }

    if (clicked && callback_) {
        callback_();
    }
}

} // namespace mouse
} // namespace clak
