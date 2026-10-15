#include "de/gnome.h"
#include <cstdio>
#include <dlfcn.h>
#include <cctype>
#include <string>

namespace clak {
namespace de {

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

bool queryAtspiActiveWindow(window::WindowInfo& info) {
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

std::optional<uint64_t> queryGnomeRepeatDelayUs() {
    FILE* fp = popen("gsettings get org.gnome.desktop.peripherals.keyboard delay 2>/dev/null", "r");
    if (!fp) return std::nullopt;

    char buf[128];
    std::optional<uint64_t> result;
    while (fgets(buf, sizeof(buf), fp)) {
        unsigned int val = 0;
        if (sscanf(buf, "uint32 %u", &val) == 1 && val > 0) {
            result = static_cast<uint64_t>(val) * 1000;
            break;
        } else if (sscanf(buf, "%u", &val) == 1 && val > 0) {
            result = static_cast<uint64_t>(val) * 1000;
            break;
        }
    }
    pclose(fp);
    return result;
}

std::optional<uint64_t> queryGnomeRepeatIntervalUs() {
    FILE* fp = popen("gsettings get org.gnome.desktop.peripherals.keyboard repeat-interval 2>/dev/null", "r");
    if (!fp) return std::nullopt;

    char buf[128];
    std::optional<uint64_t> result;
    while (fgets(buf, sizeof(buf), fp)) {
        unsigned int val = 0;
        if (sscanf(buf, "uint32 %u", &val) == 1 && val > 0) {
            result = static_cast<uint64_t>(val) * 1000;
            break;
        } else if (sscanf(buf, "%u", &val) == 1 && val > 0) {
            result = static_cast<uint64_t>(val) * 1000;
            break;
        }
    }
    pclose(fp);
    return result;
}

} // namespace de
} // namespace clak
