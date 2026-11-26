#include "apps/browser.h"
#include "apps/ide.h"
#include <algorithm>
#include <cctype>
#include <chrono>
#include <cstring>
#include <dirent.h>
#include <fstream>
#include <mutex>
#include <sys/stat.h>
#include <unordered_map>
#include <vector>

namespace clak {
namespace apps {

namespace {

enum class BrowserFamily {
    None,
    Chromium,
    Gecko,
    OtherBrowser,
};

std::string toLowerAscii(std::string s) {
    for (char& c : s) {
        c = static_cast<char>(std::tolower(static_cast<unsigned char>(c)));
    }
    return s;
}

bool dirExists(const std::string& path) {
    struct stat st{};
    return stat(path.c_str(), &st) == 0 && S_ISDIR(st.st_mode);
}

bool isKnownGecko(const std::string& lower_app) {
    static const std::vector<std::string> gecko = {
        "firefox", "zen", "librewolf", "floorp", "waterfox", "firefox-esr"
    };
    for (const auto& g : gecko) {
        if (lower_app.find(g) != std::string::npos) return true;
    }
    return false;
}

bool isKnownChromium(const std::string& lower_app) {
    static const std::vector<std::string> chromium = {
        "chromium", "chrome", "google-chrome", "brave", "helium", "thorium",
        "vivaldi", "opera", "microsoft-edge", "edge", "cavalry", "electron"
    };
    for (const auto& c : chromium) {
        if (lower_app.find(c) != std::string::npos) return true;
    }
    return false;
}

bool isKnownOtherBrowser(const std::string& lower_app) {
    static const std::vector<std::string> others = { "qutebrowser" };
    for (const auto& b : others) {
        if (lower_app.find(b) != std::string::npos) return true;
    }
    return false;
}

BrowserFamily classifyDesktopEntry(const std::string& exec_val, const std::string& name_val) {
    std::string combined = toLowerAscii(exec_val + " " + name_val);
    if (isKnownGecko(combined) || combined.find("gecko") != std::string::npos) {
        return BrowserFamily::Gecko;
    }
    if (isKnownChromium(combined) || combined.find("blink") != std::string::npos) {
        return BrowserFamily::Chromium;
    }
    return BrowserFamily::OtherBrowser;
}

std::string extractExecBinary(const std::string& exec_val) {
    std::string trimmed = exec_val;
    size_t first = trimmed.find_first_not_of(" \t\"'");
    if (first == std::string::npos) return "";
    trimmed = trimmed.substr(first);
    size_t end = trimmed.find_first_of(" \t\"'");
    std::string token = (end == std::string::npos) ? trimmed : trimmed.substr(0, end);
    if (token == "env" && end != std::string::npos) {
        return extractExecBinary(trimmed.substr(end + 1));
    }
    size_t slash = token.rfind('/');
    if (slash != std::string::npos) {
        token = token.substr(slash + 1);
    }
    return toLowerAscii(token);
}

const std::unordered_map<std::string, BrowserFamily>& getDesktopBrowsers() {
    static std::unordered_map<std::string, BrowserFamily> desktop_map;
    static std::once_flag once;
    std::call_once(once, []() {
        std::vector<std::string> dirs = { "/usr/share/applications", "/usr/local/share/applications" };
        if (const char* home = getenv("HOME")) {
            dirs.push_back(std::string(home) + "/.local/share/applications");
        }
        for (const auto& dir : dirs) {
            DIR* d = opendir(dir.c_str());
            if (!d) continue;
            struct dirent* ent;
            while ((ent = readdir(d)) != nullptr) {
                std::string fname = ent->d_name;
                if (fname.size() <= 8 || fname.compare(fname.size() - 8, 8, ".desktop") != 0) continue;
                std::ifstream in(dir + "/" + fname);
                if (!in) continue;
                std::string line, categories, exec_val, wm_class, name_val;
                bool in_entry = false;
                while (std::getline(in, line)) {
                    if (!line.empty() && line.front() == '[') {
                        in_entry = (line == "[Desktop Entry]");
                        continue;
                    }
                    if (!in_entry) continue;
                    if (line.rfind("Categories=", 0) == 0) categories = line.substr(11);
                    else if (line.rfind("Exec=", 0) == 0) exec_val = line.substr(5);
                    else if (line.rfind("StartupWMClass=", 0) == 0) wm_class = line.substr(15);
                    else if (line.rfind("Name=", 0) == 0) name_val = line.substr(5);
                }
                if (categories.find("WebBrowser") == std::string::npos) continue;
                BrowserFamily fam = classifyDesktopEntry(exec_val, name_val);
                std::string stem = toLowerAscii(fname.substr(0, fname.size() - 8));
                if (!stem.empty()) desktop_map.emplace(stem, fam);
                std::string bin = extractExecBinary(exec_val);
                if (!bin.empty()) desktop_map.emplace(bin, fam);
                std::string wm = toLowerAscii(wm_class);
                if (!wm.empty()) desktop_map.emplace(wm, fam);
            }
            closedir(d);
        }
    });
    return desktop_map;
}

BrowserFamily detectFromProc(const std::string& lower_app) {
    DIR* proc_dir = opendir("/proc");
    if (!proc_dir) return BrowserFamily::None;
    struct dirent* ent;
    BrowserFamily found = BrowserFamily::None;
    while ((ent = readdir(proc_dir)) != nullptr) {
        if (ent->d_name[0] < '0' || ent->d_name[0] > '9') continue;
        std::string base = std::string("/proc/") + ent->d_name;
        std::ifstream cmd_in(base + "/cmdline", std::ios::binary);
        if (!cmd_in) continue;
        std::string cmdline((std::istreambuf_iterator<char>(cmd_in)), std::istreambuf_iterator<char>());
        if (cmdline.empty()) continue;
        for (char& c : cmdline) {
            if (c == '\0') c = ' ';
        }
        std::string lower_cmd = toLowerAscii(cmdline);
        if (lower_cmd.find(lower_app) == std::string::npos) continue;
        if (lower_cmd.find("/chrome") != std::string::npos ||
            lower_cmd.find("chromium") != std::string::npos ||
            lower_cmd.find("--type=renderer") != std::string::npos) {
            found = BrowserFamily::Chromium;
            break;
        }
        if (lower_cmd.find("firefox") != std::string::npos ||
            lower_cmd.find("libxul") != std::string::npos) {
            found = BrowserFamily::Gecko;
            break;
        }
    }
    closedir(proc_dir);
    return found;
}

BrowserFamily detectBrowserFamily(const std::string& app) {
    if (app.empty()) return BrowserFamily::None;
    std::string lower_app = toLowerAscii(app);
    if (isVSCodeApp(lower_app) || isJetBrainsApp(lower_app)) {
        return BrowserFamily::None;
    }
    if (isKnownGecko(lower_app)) return BrowserFamily::Gecko;
    if (isKnownChromium(lower_app)) return BrowserFamily::Chromium;
    if (isKnownOtherBrowser(lower_app)) return BrowserFamily::OtherBrowser;

    static std::mutex cache_mu;
    static std::unordered_map<std::string, BrowserFamily> cache;
    {
        std::lock_guard<std::mutex> lock(cache_mu);
        auto it = cache.find(lower_app);
        if (it != cache.end()) {
            return it->second;
        }
    }

    BrowserFamily detected = BrowserFamily::None;
    if (const char* home = getenv("HOME")) {
        if (dirExists(std::string(home) + "/.config/" + lower_app + "/Default/Sessions") ||
            dirExists(std::string(home) + "/.config/" + app + "/Default/Sessions")) {
            detected = BrowserFamily::Chromium;
        }
    }
    if (detected == BrowserFamily::None) {
        const auto& desktop_map = getDesktopBrowsers();
        auto it = desktop_map.find(lower_app);
        if (it != desktop_map.end()) {
            detected = it->second;
        }
    }
    if (detected == BrowserFamily::None) {
        detected = detectFromProc(lower_app);
    }

    std::lock_guard<std::mutex> lock(cache_mu);
    cache[lower_app] = detected;
    return detected;
}

} // namespace

bool isBrowserApp(const std::string& app) {
    return detectBrowserFamily(app) != BrowserFamily::None;
}

// gecko family: firefox, zen, librewolf, floorp, waterfox
bool isGeckoApp(const std::string& app) {
    return detectBrowserFamily(app) == BrowserFamily::Gecko;
}

// chromium family: chrome, brave, helium, edge, vivaldi, opera, thorium, and auto-detected chromium forks
bool isChromiumApp(const std::string& app) {
    return detectBrowserFamily(app) == BrowserFamily::Chromium;
}

std::string detectRunningBrowserApp() {
    static int64_t s_last_browser_check_ms = 0;
    static std::string s_last_browser_app;

    auto now_ms = std::chrono::duration_cast<std::chrono::milliseconds>(
        std::chrono::steady_clock::now().time_since_epoch()).count();
    if (now_ms - s_last_browser_check_ms < 2000 && !s_last_browser_app.empty()) {
        return s_last_browser_app;
    }
    s_last_browser_check_ms = now_ms;

    static const char* const kBrowsers[] = {
        "helium", "zen", "zen-bin", "firefox", "google-chrome", "chrome",
        "chromium", "brave", "vivaldi", "opera", "microsoft-edge", "edge",
        "waterfox", "librewolf", "thorium", "floorp"
    };

    DIR* proc_dir = opendir("/proc");
    if (!proc_dir) return s_last_browser_app;

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
            if (strstr(comm, "crashpad") != nullptr) {
                fclose(f);
                continue;
            }
            for (const char* b : kBrowsers) {
                if (strcmp(comm, b) == 0) {
                    found = comm;
                    fclose(f);
                    closedir(proc_dir);
                    s_last_browser_app = found;
                    return found;
                }
            }
        }
        fclose(f);
    }
    closedir(proc_dir);
    s_last_browser_app = found;
    return found;
}

static std::string extractSessionDomain(const std::string& app) {
    const char* home = getenv("HOME");
    if (!home) return "";

    std::vector<std::string> session_dirs;
    if (app == "helium") {
        session_dirs.push_back(std::string(home) + "/.config/net.imput.helium/Default/Sessions");
    } else if (app == "google-chrome" || app == "chrome") {
        session_dirs.push_back(std::string(home) + "/.config/google-chrome/Default/Sessions");
    } else if (app == "chromium") {
        session_dirs.push_back(std::string(home) + "/.config/chromium/Default/Sessions");
    } else if (app == "brave") {
        session_dirs.push_back(std::string(home) + "/.config/BraveSoftware/Brave-Browser/Default/Sessions");
    } else {
        if (!app.empty()) {
            session_dirs.push_back(std::string(home) + "/.config/" + app + "/Default/Sessions");
        }
        session_dirs.push_back(std::string(home) + "/.config/net.imput.helium/Default/Sessions");
        session_dirs.push_back(std::string(home) + "/.config/google-chrome/Default/Sessions");
        session_dirs.push_back(std::string(home) + "/.config/chromium/Default/Sessions");
    }

    std::string newest_file;
    time_t newest_mtime = 0;

    for (const auto& sdir : session_dirs) {
        DIR* d = opendir(sdir.c_str());
        if (!d) continue;
        struct dirent* ent;
        while ((ent = readdir(d)) != nullptr) {
            if (strncmp(ent->d_name, "Session_", 8) == 0) {
                std::string full_path = sdir + "/" + ent->d_name;
                struct stat st{};
                if (stat(full_path.c_str(), &st) == 0 && st.st_mtime > newest_mtime) {
                    newest_mtime = st.st_mtime;
                    newest_file = full_path;
                }
            }
        }
        closedir(d);
    }

    if (newest_file.empty()) return "";

    FILE* f = fopen(newest_file.c_str(), "rb");
    if (!f) return "";

    if (fseek(f, 0, SEEK_END) != 0) {
        fclose(f);
        return "";
    }
    long size = ftell(f);
    long read_len = std::min(4096L, size);
    if (read_len <= 0 || fseek(f, size - read_len, SEEK_SET) != 0) {
        fclose(f);
        return "";
    }

    std::vector<char> buf(read_len + 1, 0);
    size_t n = fread(buf.data(), 1, read_len, f);
    fclose(f);
    buf[n] = '\0';

    std::string chunk(buf.data(), n);
    size_t pos_https = chunk.rfind("https://");
    size_t pos_http = chunk.rfind("http://");
    size_t pos = std::string::npos;
    if (pos_https != std::string::npos && pos_http != std::string::npos) {
        pos = std::max(pos_https, pos_http);
    } else if (pos_https != std::string::npos) {
        pos = pos_https;
    } else {
        pos = pos_http;
    }

    if (pos == std::string::npos) return "";

    size_t start = (chunk.substr(pos, 8) == "https://") ? pos + 8 : pos + 7;
    size_t end = chunk.find_first_of("/ :?#\x00\r\n\t", start);
    std::string host = (end == std::string::npos) ? chunk.substr(start) : chunk.substr(start, end - start);
    if (host.rfind("www.", 0) == 0) host = host.substr(4);
    return host;
}

std::string extractDomain(const std::string& app, const std::string& title) {
    std::string lower_app = app;
    for (char& c : lower_app) c = tolower(c);
    if (!isBrowserApp(lower_app)) {
        return app.empty() ? "unknown" : app;
    }

    if (title.empty()) {
        std::string session_domain = extractSessionDomain(lower_app);
        if (!session_domain.empty()) {
            return session_domain;
        }
        return app;
    }

    std::string clean_title = title;
    static const std::vector<std::string> browser_suffixes = {
        " - Helium", " - Google Chrome", " - Chromium", " - Brave",
        " - Mozilla Firefox", " - Firefox", " - Vivaldi", " - Opera",
        " - Microsoft Edge", " - Zen Browser", " - Thorium", " - Floorp",
        " - Arc", " - Waterfox", " - Librewolf",
        " \xe2\x80\x94 Zen Browser", " \xe2\x80\x94 Mozilla Firefox",
        " \xe2\x80\x94 Firefox", " \xe2\x80\x94 Floorp", " \xe2\x80\x94 Waterfox",
        " \xe2\x80\x94 Librewolf"
    };
    bool stripped_suffix = false;
    for (const auto& suffix : browser_suffixes) {
        if (clean_title.size() >= suffix.size() &&
            clean_title.compare(clean_title.size() - suffix.size(), suffix.size(), suffix) == 0) {
            clean_title = clean_title.substr(0, clean_title.size() - suffix.size());
            stripped_suffix = true;
            break;
        }
    }
    if (!stripped_suffix && !lower_app.empty()) {
        size_t sep = clean_title.rfind(" - ");
        if (sep != std::string::npos && toLowerAscii(clean_title.substr(sep + 3)) == lower_app) {
            clean_title = clean_title.substr(0, sep);
        }
    }

    size_t http_pos = clean_title.find("://");
    if (http_pos != std::string::npos) {
        size_t start = http_pos + 3;
        size_t end = clean_title.find_first_of("/ :?#", start);
        std::string host = (end == std::string::npos) ? clean_title.substr(start) : clean_title.substr(start, end - start);
        if (host.rfind("www.", 0) == 0) host = host.substr(4);
        if (!host.empty()) return host;
    }

    static const std::vector<std::string> tlds = {
        ".com", ".ai", ".org", ".net", ".vn", ".io", ".dev", ".app", ".so",
        ".me", ".edu", ".gov", ".co", ".tv", ".xyz", ".cc", ".to", ".info"
    };
    for (const auto& tld : tlds) {
        size_t pos = clean_title.find(tld);
        if (pos != std::string::npos) {
            size_t start = pos;
            while (start > 0 && (isalnum(clean_title[start - 1]) || clean_title[start - 1] == '.' || clean_title[start - 1] == '-')) {
                start--;
            }
            size_t end = pos + tld.size();
            std::string domain = clean_title.substr(start, end - start);
            if (domain.rfind("www.", 0) == 0) domain = domain.substr(4);
            if (!domain.empty() && domain != tld) return domain;
        }
    }

    std::string lower = clean_title;
    for (char& c : lower) c = tolower(c);

    // social and chat
    if (lower.find(" / x") != std::string::npos ||
        lower.find("on x:") != std::string::npos ||
        lower.find("on x：") != std::string::npos ||
        lower == "x" ||
        lower.find("twitter") != std::string::npos ||
        lower.find("tweet") != std::string::npos) {
        return "x.com";
    }
    if (lower.find("messenger") != std::string::npos) return "messenger.com";
    if (lower.find("facebook") != std::string::npos) return "facebook.com";
    if (lower.find("instagram") != std::string::npos) return "instagram.com";
    if (lower.find("threads") != std::string::npos) return "threads.net";
    if (lower.find("whatsapp") != std::string::npos) return "web.whatsapp.com";

    // ai assistants
    if (lower.find("claude") != std::string::npos) return "claude.ai";
    if (lower.find("chatgpt") != std::string::npos || lower.find("openai") != std::string::npos) return "chatgpt.com";
    if (lower.find("gemini") != std::string::npos) return "gemini.google.com";
    if (lower.find("perplexity") != std::string::npos) return "perplexity.ai";
    if (lower.find("deepseek") != std::string::npos) return "chat.deepseek.com";
    if (lower.find("copilot") != std::string::npos) return "copilot.microsoft.com";
    if (lower.find("grok") != std::string::npos) return "grok.com";

    // google and video
    if (lower.find("youtube") != std::string::npos) return "youtube.com";
    if (lower.find("google docs") != std::string::npos ||
        lower.find("google tài liệu") != std::string::npos ||
        lower.find("tài liệu không có tiêu đề") != std::string::npos) return "docs.google.com";
    if (lower.find("google sheets") != std::string::npos ||
        lower.find("google trang tính") != std::string::npos ||
        lower.find("bảng tính không có tiêu đề") != std::string::npos) return "sheets.google.com";
    if (lower.find("google slides") != std::string::npos ||
        lower.find("google trang trình bày") != std::string::npos ||
        lower.find("bản trình bày không có tiêu đề") != std::string::npos) return "slides.google.com";
    if (lower.find("google drive") != std::string::npos) return "drive.google.com";
    if (lower.find("gmail") != std::string::npos) return "mail.google.com";
    if (lower.find("google search") != std::string::npos || lower == "google") return "google.com";

    // developer tools
    if (lower.find("github") != std::string::npos) return "github.com";
    if (lower.find("gitlab") != std::string::npos) return "gitlab.com";
    if (lower.find("stackoverflow") != std::string::npos || lower.find("stack overflow") != std::string::npos) return "stackoverflow.com";
    if (lower.find("hacker news") != std::string::npos) return "news.ycombinator.com";
    if (lower.find("leetcode") != std::string::npos) return "leetcode.com";
    if (lower.find("vercel") != std::string::npos) return "vercel.com";
    if (lower.find("supabase") != std::string::npos) return "supabase.com";

    // productivity and others
    if (lower.find("monkeytype") != std::string::npos) return "monkeytype.com";
    if (lower.find("discord") != std::string::npos) return "discord.com";
    if (lower.find("telegram") != std::string::npos) return "web.telegram.org";
    if (lower.find("zalo") != std::string::npos) return "chat.zalo.me";
    if (lower.find("slack") != std::string::npos) return "slack.com";
    if (lower.find("notion") != std::string::npos) return "notion.so";
    if (lower.find("figma") != std::string::npos) return "figma.com";
    if (lower.find("reddit") != std::string::npos) return "reddit.com";
    if (lower.find("linkedin") != std::string::npos) return "linkedin.com";
    if (lower.find("tiktok") != std::string::npos) return "tiktok.com";
    if (lower.find("twitch") != std::string::npos) return "twitch.tv";
    if (lower.find("wikipedia") != std::string::npos) return "wikipedia.org";
    if (lower.find("medium") != std::string::npos) return "medium.com";
    if (lower.find("substack") != std::string::npos) return "substack.com";

    size_t last_sep = clean_title.rfind(" - ");
    if (last_sep == std::string::npos) last_sep = clean_title.rfind(" | ");
    if (last_sep != std::string::npos && last_sep + 3 < clean_title.size()) {
        std::string last_seg = clean_title.substr(last_sep + 3);
        while (!last_seg.empty() && (last_seg.front() == ' ' || last_seg.front() == '\t')) last_seg.erase(0, 1);
        while (!last_seg.empty() && (last_seg.back() == ' ' || last_seg.back() == '\t')) last_seg.pop_back();
        if (!last_seg.empty()) {
            std::string seg_lower = last_seg;
            for (char& c : seg_lower) c = tolower(c);
            return seg_lower;
        }
    }

    return clean_title;
}

namespace {

class GeckoProfile : public AppProfile {
public:
    std::string name() const override { return "Gecko-Uinput"; }
    bool allowsSurroundingText() const override { return false; }
    bool forcesUinput() const override { return true; }
    UinputPacing uinputPacing() const override { return {6, 2}; }
};

class DefaultProfile : public AppProfile {
public:
    std::string name() const override { return "Chromium-SurroundingText"; }
    bool allowsSurroundingText() const override { return true; }
    UinputPacing uinputPacing() const override { return {6, 2}; }
};

} // namespace

std::unique_ptr<AppProfile> createGeckoProfile() {
    return std::make_unique<GeckoProfile>();
}

std::unique_ptr<AppProfile> createDefaultProfile() {
    return std::make_unique<DefaultProfile>();
}

} // namespace apps
} // namespace clak
