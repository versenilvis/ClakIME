#include "apps/browser.h"
#include "apps/ide.h"
#include <vector>
#include <cctype>
#include <chrono>
#include <cstring>
#include <dirent.h>
#include <sys/stat.h>

namespace clak {
namespace apps {

bool isBrowserApp(const std::string& app) {
    if (isVSCodeApp(app) || isJetBrainsApp(app)) return false;
    static const std::vector<std::string> browsers = {
        "chromium", "chrome", "google-chrome", "brave", "firefox", "zen",
        "vivaldi", "opera", "microsoft-edge", "edge", "waterfox", "librewolf",
        "helium", "thorium", "floorp", "cavalry", "qutebrowser", "electron"
    };
    for (const auto& b : browsers) {
        if (app.find(b) != std::string::npos) return true;
    }
    return false;
}

// gecko family: firefox, zen, librewolf, floorp, waterfox
bool isGeckoApp(const std::string& app) {
    static const std::vector<std::string> gecko = {
        "firefox", "zen", "librewolf", "floorp", "waterfox", "firefox-esr"
    };
    for (const auto& g : gecko) {
        if (app.find(g) != std::string::npos) return true;
    }
    return false;
}

// chromium family: chrome, brave, helium, edge, vivaldi, opera, thorium
bool isChromiumApp(const std::string& app) {
    static const std::vector<std::string> chromium = {
        "chromium", "chrome", "google-chrome", "brave", "helium", "thorium",
        "vivaldi", "opera", "microsoft-edge", "edge", "cavalry", "electron"
    };
    for (const auto& c : chromium) {
        if (app.find(c) != std::string::npos) return true;
    }
    return false;
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
    for (const auto& suffix : browser_suffixes) {
        if (clean_title.size() >= suffix.size() &&
            clean_title.compare(clean_title.size() - suffix.size(), suffix.size(), suffix) == 0) {
            clean_title = clean_title.substr(0, clean_title.size() - suffix.size());
            break;
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
