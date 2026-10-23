#include "apps/sites.h"
#include "config/config.h"
#include "de/kde.h"
#include <vector>

namespace clak {
namespace apps {

bool isMetaSite(const std::string& site) {
    static const std::vector<std::string> base_meta_sites = {
        "instagram.com", "web.whatsapp.com"
    };
    static const std::vector<std::string> hypr_meta_sites = {
        "facebook.com", "messenger.com", "facebook", "messenger"
    };
    for (const auto& s : base_meta_sites) {
        if (site == s || site.find(s) != std::string::npos) return true;
    }
    // on hyprland and non-kde sessions, keep facebook and messenger on surrounding text
    if (!de::isKdeSession()) {
        for (const auto& s : hypr_meta_sites) {
            if (site == s || site.find(s) != std::string::npos) return true;
        }
    }
    return false;
}

bool isTikTokSite(const std::string& site) {
    static const std::vector<std::string> tiktok_sites = {
        "tiktok.com", "tiktok"
    };
    for (const auto& s : tiktok_sites) {
        if (site == s || site.find(s) != std::string::npos) return true;
    }
    return false;
}

bool isDraftJsSite(const std::string& site) {
    static const std::vector<std::string> draftjs_sites = {
        "x.com", "twitter.com", "threads.net"
    };
    static const std::vector<std::string> kde_meta_sites = {
        "facebook.com", "messenger.com", "facebook", "messenger"
    };
    for (const auto& s : draftjs_sites) {
        if (site == s || site.find(s) != std::string::npos) return true;
    }
    // on kde plasma sessions, route facebook and messenger to uinput
    if (de::isKdeSession()) {
        for (const auto& s : kde_meta_sites) {
            if (site == s || site.find(s) != std::string::npos) return true;
        }
    }
    return false;
}

bool isGoogleDocsSite(const std::string& site) {
    static const std::vector<std::string> docs_sites = {
        "docs.google.com", "sheets.google.com", "slides.google.com"
    };
    for (const auto& s : docs_sites) {
        if (site == s || site.find(s) != std::string::npos) return true;
    }
    return false;
}

bool isForceUinputSite(const std::string& site) {
    return isGoogleDocsSite(site) || isDraftJsSite(site) || isTikTokSite(site);
}

namespace {

class MetaSiteProfile : public AppProfile {
public:
    std::string name() const override { return "Meta-SurroundingText"; }
    bool allowsSurroundingText() const override { return true; }
    bool preventsUinput() const override { return true; }
    bool allowsAutofillHeuristic() const override { return false; }
};

class TikTokProfile : public AppProfile {
public:
    std::string name() const override { return "TikTok-Uinput"; }
    bool allowsSurroundingText() const override { return true; }
    bool forcesUinput() const override { return true; }
    bool allowsAutofillHeuristic() const override { return false; }
    UinputPacing uinputPacing() const override {
        return {config::kDraftJsPostDelayMs, config::kDraftJsGapMs};
    }
};

class DraftJsProfile : public AppProfile {
public:
    std::string name() const override { return "DraftJS-Uinput"; }
    bool allowsSurroundingText() const override { return true; }
    bool forcesUinput() const override { return true; }
    UinputPacing uinputPacing() const override {
        return {config::kDraftJsPostDelayMs, config::kDraftJsGapMs};
    }
};

class GoogleDocsProfile : public AppProfile {
public:
    std::string name() const override { return "Docs-Uinput"; }
    bool allowsSurroundingText() const override { return false; }
    bool forcesUinput() const override { return true; }
    UinputPacing uinputPacing() const override { return {6, 2}; }
};

class AddressBarProfile : public AppProfile {
public:
    std::string name() const override { return "address-bar"; }
    bool allowsSurroundingText() const override { return false; }
    bool forcesUinput() const override { return true; }
    UinputPacing uinputPacing() const override {
        return {config::kAddressBarPostDelayMs, 2};
    }
};

} // namespace

std::unique_ptr<AppProfile> createMetaSiteProfile() {
    return std::make_unique<MetaSiteProfile>();
}

std::unique_ptr<AppProfile> createTikTokProfile() {
    return std::make_unique<TikTokProfile>();
}

std::unique_ptr<AppProfile> createDraftJsProfile() {
    return std::make_unique<DraftJsProfile>();
}

std::unique_ptr<AppProfile> createGoogleDocsProfile() {
    return std::make_unique<GoogleDocsProfile>();
}

std::unique_ptr<AppProfile> createAddressBarProfile() {
    return std::make_unique<AddressBarProfile>();
}

} // namespace apps
} // namespace clak
