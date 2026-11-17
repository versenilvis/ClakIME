#include "editor_context.h"
#include "window/window_info.h"
#include "apps/apps.h"
#include "apps/browser.h"
#include "apps/steam.h"
#include "apps/sites.h"
#include "apps/terminal.h"
#include "de/kde.h"
#include "config/config.h"
#include "config/sites.h"
#include "core.h"
#include <string_view>
#include <algorithm>
#include <cctype>

namespace clak {
namespace ime {

EditorContextDetector::EditorContextDetector(fcitx::InputContext* ic, SurroundingVerifier& verifier)
    : ic_(ic), verifier_(verifier) {}

std::string EditorContextDetector::appKey() const {
    if (ic_ && !ic_->program().empty() && !de::isDesktopShell(ic_->program())) {
        return ic_->program();
    }
    bool has_term_cap = ic_ && ic_->capabilityFlags().test(fcitx::CapabilityFlag::Terminal);
    platform::WindowInfo win = platform::getActiveWindow();
    if (!win.win_class.empty() && !de::isDesktopShell(win.win_class)) {
        if (!has_term_cap || apps::isTerminalApp(win.win_class)) {
            return win.win_class;
        }
    }
    if (has_term_cap) {
        return "terminal";
    }
    if (ic_ && !ic_->program().empty() && !de::isDesktopShell(ic_->program())) {
        return ic_->program();
    }
    return "default";
}

std::string EditorContextDetector::activeSite() {
    uint64_t now_us = fcitx::now(CLOCK_MONOTONIC);
    if (now_us - last_site_check_us_ < config::kCacheCheckIntervalUs && !cached_site_.empty()) {
        return cached_site_;
    }
    last_site_check_us_ = now_us;
    std::string fallback = (ic_ && ic_->capabilityFlags().test(fcitx::CapabilityFlag::Terminal))
                                ? "terminal"
                                : ((ic_ && !ic_->program().empty() && !de::isDesktopShell(ic_->program())) ? ic_->program() : "");
    platform::WindowInfo win = platform::getActiveWindow(fallback);
    std::string new_site = config::extractDomain(win.win_class, win.win_title);
    if (!new_site.empty() && !cached_site_.empty() && new_site != cached_site_) {
        // clear rich text and draftjs editor flags when switching sites
        verifier_.setDraftJsEditor(false);
        verifier_.setRichTextEditor(false);
        verifier_.setCanvasEditor(false);
    }
    cached_site_ = new_site;
    return cached_site_;
}

bool EditorContextDetector::isBrowser() const {
    std::string app = appKey();
    return apps::isBrowserApp(app);
}

bool EditorContextDetector::isGecko() const {
    std::string app = appKey();
    return apps::isGeckoApp(app);
}

bool EditorContextDetector::isSteam() const {
    std::string app = appKey();
    return apps::isSteamApp(app);
}

bool EditorContextDetector::isDraftJsEditor() {
    std::string site = activeSite();
    if (apps::isClaudeSite(site)) {
        bool is_multiline = (ic_ && ic_->capabilityFlags().test(fcitx::CapabilityFlag::Multiline));
        if (!is_multiline && ic_ && ic_->surroundingText().isValid() &&
            ic_->surroundingText().text().find('\n') != std::string::npos) {
            is_multiline = true;
        }
        return !is_multiline;
    }
    return verifier_.isDraftJsEditor(site);
}

bool EditorContextDetector::looksLikeUrlAutofill(const fcitx::SurroundingText& surr) {
    if (!surr.isValid()) return false;
    const std::string& text = surr.text();
    if (text.empty()) return false;

    unsigned int cursor = surr.cursor();
    unsigned int anchor = surr.anchor();
    if (cursor == anchor) return false;

    unsigned int sel_start = std::min(anchor, cursor);
    unsigned int sel_end = std::max(anchor, cursor);
    if (sel_start == 0 || sel_end != text.size()) return false;

    for (size_t i = 0; i < text.size(); ++i) {
        unsigned char c = static_cast<unsigned char>(text[i]);
        if (c < 0x21 || c > 0x7e) return false;
    }

    if (text.find(":/") != std::string::npos || text.rfind("about:", 0) == 0) {
        return true;
    }
    if (text.find("localhost") != std::string::npos) {
        return true;
    }
    if (text.rfind("www.", 0) == 0) {
        return true;
    }

    size_t host_end = text.find_first_of("/:?#");
    std::string host = (host_end == std::string::npos) ? text : text.substr(0, host_end);
    if (host.empty() || host.front() == '.' || host.back() == '.') {
        return false;
    }

    static const std::string_view tlds[] = {
        ".com", ".org", ".net", ".edu", ".gov", ".mil", ".io", ".dev", ".app",
        ".ai", ".vn", ".me", ".co", ".info", ".xyz", ".biz", ".tv", ".cc",
        ".to", ".so", ".online", ".site", ".top", ".pro", ".uk", ".us",
        ".de", ".fr", ".jp", ".cn", ".ru", ".ca", ".au", ".in"
    };

    for (const auto& tld : tlds) {
        if (host.size() > tld.size() &&
            host.compare(host.size() - tld.size(), tld.size(), tld) == 0) {
            char before = host[host.size() - tld.size() - 1];
            if (isalnum(static_cast<unsigned char>(before)) || before == '-') {
                return true;
            }
        }
    }

    return false;
}

void EditorContextDetector::reset() {
    cached_site_.clear();
    last_site_check_us_ = 0;
}

} // namespace ime
} // namespace clak
