#include "window/x11_tracker.h"
#include "utils/log.h"
#include <xcb/xcb.h>
#include <mutex>
#include <memory>
#include <fstream>
#include <cstdlib>
#include <cstring>

namespace clak {
namespace window {

namespace {

struct XcbAtoms {
    xcb_atom_t net_active_window{XCB_ATOM_NONE};
    xcb_atom_t net_wm_pid{XCB_ATOM_NONE};
    xcb_atom_t wm_class{XCB_ATOM_NONE};
    xcb_atom_t net_wm_name{XCB_ATOM_NONE};
    xcb_atom_t wm_name{XCB_ATOM_NONE};
};

std::mutex s_xcb_mutex;
xcb_connection_t* s_xcb_conn = nullptr;
XcbAtoms s_atoms;

xcb_atom_t internAtom(xcb_connection_t* conn, const char* name) {
    if (!conn || !name) return XCB_ATOM_NONE;
    auto cookie = xcb_intern_atom(conn, 0, static_cast<uint16_t>(std::strlen(name)), name);
    std::unique_ptr<xcb_intern_atom_reply_t, decltype(&free)> reply(
        xcb_intern_atom_reply(conn, cookie, nullptr), &free);
    return reply ? reply->atom : XCB_ATOM_NONE;
}

void initAtoms(xcb_connection_t* conn, XcbAtoms& atoms) {
    atoms.net_active_window = internAtom(conn, "_NET_ACTIVE_WINDOW");
    atoms.net_wm_pid = internAtom(conn, "_NET_WM_PID");
    atoms.wm_class = internAtom(conn, "WM_CLASS");
    atoms.net_wm_name = internAtom(conn, "_NET_WM_NAME");
    atoms.wm_name = internAtom(conn, "WM_NAME");
}

uint32_t firstCardinal(xcb_connection_t* conn, xcb_window_t window, xcb_atom_t atom) {
    if (!conn || window == XCB_WINDOW_NONE || atom == XCB_ATOM_NONE) return 0;
    auto cookie = xcb_get_property(conn, 0, window, atom, XCB_GET_PROPERTY_TYPE_ANY, 0, 1);
    std::unique_ptr<xcb_get_property_reply_t, decltype(&free)> reply(
        xcb_get_property_reply(conn, cookie, nullptr), &free);
    if (!reply || reply->type == XCB_ATOM_NONE || reply->format != 32 || reply->value_len == 0) {
        return 0;
    }
    return *static_cast<uint32_t*>(xcb_get_property_value(reply.get()));
}

std::string programForPid(pid_t pid) {
    if (pid <= 0) return {};
    std::string path = "/proc/" + std::to_string(pid) + "/comm";
    std::ifstream file(path);
    if (!file.is_open()) return {};
    std::string comm;
    if (std::getline(file, comm)) {
        while (!comm.empty() && (comm.back() == '\n' || comm.back() == '\r')) {
            comm.pop_back();
        }
        return comm;
    }
    return {};
}

std::string wmClassName(xcb_connection_t* conn, xcb_window_t window, xcb_atom_t atom_wm_class) {
    if (!conn || window == XCB_WINDOW_NONE || atom_wm_class == XCB_ATOM_NONE) return {};
    auto cookie = xcb_get_property(conn, 0, window, atom_wm_class, XCB_GET_PROPERTY_TYPE_ANY, 0, 256);
    std::unique_ptr<xcb_get_property_reply_t, decltype(&free)> reply(
        xcb_get_property_reply(conn, cookie, nullptr), &free);
    if (!reply || reply->type == XCB_ATOM_NONE || reply->value_len == 0) {
        return {};
    }
    const char* bytes = static_cast<const char*>(xcb_get_property_value(reply.get()));
    size_t size = static_cast<size_t>(reply->value_len);
    std::string val(bytes, size);
    size_t first = val.find('\0');
    if (first == std::string::npos) return val;
    size_t second = val.find('\0', first + 1);
    if (second == std::string::npos) return val.substr(0, first);
    // return class name rather than instance name
    return val.substr(first + 1, second - first - 1);
}

std::string windowTitle(xcb_connection_t* conn, xcb_window_t window, xcb_atom_t net_wm_name, xcb_atom_t wm_name) {
    if (!conn || window == XCB_WINDOW_NONE) return {};
    if (net_wm_name != XCB_ATOM_NONE) {
        auto cookie = xcb_get_property(conn, 0, window, net_wm_name, XCB_GET_PROPERTY_TYPE_ANY, 0, 512);
        std::unique_ptr<xcb_get_property_reply_t, decltype(&free)> reply(
            xcb_get_property_reply(conn, cookie, nullptr), &free);
        if (reply && reply->type != XCB_ATOM_NONE && reply->value_len > 0) {
            const char* bytes = static_cast<const char*>(xcb_get_property_value(reply.get()));
            return std::string(bytes, static_cast<size_t>(reply->value_len));
        }
    }
    if (wm_name != XCB_ATOM_NONE) {
        auto cookie = xcb_get_property(conn, 0, window, wm_name, XCB_GET_PROPERTY_TYPE_ANY, 0, 512);
        std::unique_ptr<xcb_get_property_reply_t, decltype(&free)> reply(
            xcb_get_property_reply(conn, cookie, nullptr), &free);
        if (reply && reply->type != XCB_ATOM_NONE && reply->value_len > 0) {
            const char* bytes = static_cast<const char*>(xcb_get_property_value(reply.get()));
            return std::string(bytes, static_cast<size_t>(reply->value_len));
        }
    }
    return {};
}

xcb_window_t rootWindow(xcb_connection_t* conn) {
    if (!conn) return XCB_WINDOW_NONE;
    const xcb_setup_t* setup = xcb_get_setup(conn);
    if (!setup) return XCB_WINDOW_NONE;
    xcb_screen_iterator_t iter = xcb_setup_roots_iterator(setup);
    return iter.rem > 0 ? iter.data->root : XCB_WINDOW_NONE;
}

xcb_window_t focusedWindow(xcb_connection_t* conn) {
    if (!conn) return XCB_WINDOW_NONE;
    auto cookie = xcb_get_input_focus(conn);
    std::unique_ptr<xcb_get_input_focus_reply_t, decltype(&free)> reply(
        xcb_get_input_focus_reply(conn, cookie, nullptr), &free);
    if (!reply) return XCB_WINDOW_NONE;
    if (reply->focus == XCB_WINDOW_NONE || reply->focus == XCB_INPUT_FOCUS_POINTER_ROOT) {
        return XCB_WINDOW_NONE;
    }
    return reply->focus;
}

// traverse ancestors to find top-level window publishing class or pid
bool queryWindowChain(xcb_connection_t* conn, xcb_window_t start, const XcbAtoms& atoms, WindowInfo& out) {
    if (!conn || start == XCB_WINDOW_NONE) return false;
    xcb_window_t root = rootWindow(conn);
    if (root == XCB_WINDOW_NONE) return false;

    xcb_window_t curr = start;
    while (curr != XCB_WINDOW_NONE && curr != root) {
        uint32_t pid_val = firstCardinal(conn, curr, atoms.net_wm_pid);
        std::string prog;
        if (pid_val > 0) {
            prog = programForPid(static_cast<pid_t>(pid_val));
            out.pid = static_cast<pid_t>(pid_val);
        }
        std::string cls = wmClassName(conn, curr, atoms.wm_class);
        std::string title = windowTitle(conn, curr, atoms.net_wm_name, atoms.wm_name);

        if (!cls.empty() || !prog.empty()) {
            out.win_class = !cls.empty() ? cls : prog;
            out.win_title = title;
            return true;
        }

        auto tree_cookie = xcb_query_tree(conn, curr);
        std::unique_ptr<xcb_query_tree_reply_t, decltype(&free)> tree_reply(
            xcb_query_tree_reply(conn, tree_cookie, nullptr), &free);
        if (!tree_reply || tree_reply->parent == curr) {
            break;
        }
        curr = tree_reply->parent;
    }
    return false;
}

} // namespace

bool queryX11ActiveWindow(WindowInfo& out_info) {
    std::scoped_lock lock(s_xcb_mutex);
    if (!s_xcb_conn || xcb_connection_has_error(s_xcb_conn) != 0) {
        if (s_xcb_conn) {
            xcb_disconnect(s_xcb_conn);
            s_xcb_conn = nullptr;
        }
        const char* dpy = std::getenv("DISPLAY");
        if (!dpy || dpy[0] == '\0') {
            return false;
        }
        s_xcb_conn = xcb_connect(nullptr, nullptr);
        if (!s_xcb_conn || xcb_connection_has_error(s_xcb_conn) != 0) {
            if (s_xcb_conn) {
                xcb_disconnect(s_xcb_conn);
                s_xcb_conn = nullptr;
            }
            return false;
        }
        initAtoms(s_xcb_conn, s_atoms);
    }

    xcb_window_t root = rootWindow(s_xcb_conn);
    if (root == XCB_WINDOW_NONE) return false;

    // query focused window first
    xcb_window_t focus = focusedWindow(s_xcb_conn);
    if (focus != XCB_WINDOW_NONE && queryWindowChain(s_xcb_conn, focus, s_atoms, out_info)) {
        return true;
    }

    // fallback to _NET_ACTIVE_WINDOW on root
    uint32_t active_win = firstCardinal(s_xcb_conn, root, s_atoms.net_active_window);
    if (active_win != 0 && queryWindowChain(s_xcb_conn, static_cast<xcb_window_t>(active_win), s_atoms, out_info)) {
        return true;
    }

    return false;
}

} // namespace window
} // namespace clak
