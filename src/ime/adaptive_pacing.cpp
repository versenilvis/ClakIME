#include "ime/adaptive_pacing.h"
#include "config/sites.h"
#include "apps/terminal.h"
#include "apps/wps.h"
#include "apps/steam.h"
#include "apps/ide.h"
#include "utils/log.h"
#include <algorithm>
#include <cstdlib>
#include <filesystem>
#include <fstream>
#include <sstream>

namespace clak {
namespace pacing {

AdaptivePacingEngine::AdaptivePacingEngine(bool enabled)
    : enabled_(enabled) {
    web_draftjs_.metrics = {16, 8, 10, 40, 4, 20, 0};
    terminal_uinput_.metrics = {12, 3, 6, 30, 2, 10, 0};
    heavy_app_.metrics = {15, 4, 8, 35, 3, 12, 0};
    default_group_.metrics = {8, 2, 4, 30, 2, 10, 0};
}

void AdaptivePacingEngine::setEnabled(bool enabled) {
    enabled_ = enabled;
}

bool AdaptivePacingEngine::isEnabled() const {
    return enabled_;
}

AdaptivePacingEngine::GroupState& AdaptivePacingEngine::getGroup(PacingGroup group) {
    switch (group) {
        case PacingGroup::WebDraftJs:
            return web_draftjs_;
        case PacingGroup::TerminalUinput:
            return terminal_uinput_;
        case PacingGroup::HeavyApp:
            return heavy_app_;
        case PacingGroup::Default:
        default:
            return default_group_;
    }
}

const AdaptivePacingEngine::GroupState& AdaptivePacingEngine::getGroup(PacingGroup group) const {
    switch (group) {
        case PacingGroup::WebDraftJs:
            return web_draftjs_;
        case PacingGroup::TerminalUinput:
            return terminal_uinput_;
        case PacingGroup::HeavyApp:
            return heavy_app_;
        case PacingGroup::Default:
        default:
            return default_group_;
    }
}

void AdaptivePacingEngine::configureGroup(PacingGroup group, uint32_t floor_delay_ms, uint32_t floor_gap_ms,
                                         uint32_t ceiling_delay_ms, uint32_t ceiling_gap_ms) {
    auto& g = getGroup(group);
    g.metrics.floor_post_ms = floor_delay_ms;
    g.metrics.floor_gap_ms = floor_gap_ms;
    g.metrics.ceiling_post_ms = ceiling_delay_ms;
    g.metrics.ceiling_gap_ms = ceiling_gap_ms;

    if (g.metrics.post_delay_ms < floor_delay_ms) {
        g.metrics.post_delay_ms = floor_delay_ms;
    } else if (g.metrics.post_delay_ms > ceiling_delay_ms) {
        g.metrics.post_delay_ms = ceiling_delay_ms;
    }

    if (g.metrics.gap_ms < floor_gap_ms) {
        g.metrics.gap_ms = floor_gap_ms;
    } else if (g.metrics.gap_ms > ceiling_gap_ms) {
        g.metrics.gap_ms = ceiling_gap_ms;
    }
}

void AdaptivePacingEngine::onIncident(PacingGroup group, IncidentType incident, uint64_t now_us) {
    if (!enabled_) {
        return;
    }

    // ignore key collisions or deliberate timeouts
    if (incident == IncidentType::KeyInterruption ||
        incident == IncidentType::SelectionTimeout ||
        incident == IncidentType::AddressBarDelay) {
        return;
    }

    auto& g = getGroup(group);

    // purge incidents outside sliding window
    while (!g.incident_timestamps_us.empty() &&
           now_us - g.incident_timestamps_us.front() > kSlidingWindowUs) {
        g.incident_timestamps_us.pop_front();
    }
    g.incident_timestamps_us.push_back(now_us);

    // additive increase
    g.metrics.post_delay_ms = std::min(g.metrics.ceiling_post_ms,
                                       g.metrics.post_delay_ms + kAdditiveDelayStepMs);
    g.metrics.gap_ms = std::min(g.metrics.ceiling_gap_ms,
                                g.metrics.gap_ms + kAdditiveGapStepMs);
    g.metrics.consecutive_clean_words = 0;
}

void AdaptivePacingEngine::onTransactionSuccess(PacingGroup group) {
    if (!enabled_) {
        return;
    }

    auto& g = getGroup(group);
    g.metrics.consecutive_clean_words++;

    // slow decay towards floor
    if (g.metrics.consecutive_clean_words >= kDecayWordThreshold) {
        g.metrics.consecutive_clean_words = 0;
        if (g.metrics.post_delay_ms > g.metrics.floor_post_ms) {
            g.metrics.post_delay_ms -= 1;
        }
        if (g.metrics.gap_ms > g.metrics.floor_gap_ms) {
            g.metrics.gap_ms -= 1;
        }
    }
}

PacingMetrics AdaptivePacingEngine::metrics(PacingGroup group) const {
    return getGroup(group).metrics;
}

apps::UinputPacing AdaptivePacingEngine::uinputPacing(PacingGroup group) const {
    const auto& m = getGroup(group).metrics;
    return {m.post_delay_ms, m.gap_ms};
}

PacingGroup AdaptivePacingEngine::classify(const std::string& app, const std::string& site, bool is_autofill, bool is_draftjs) {
    if (is_draftjs || config::isDraftJsSite(site) || config::isTikTokSite(site)) {
        return PacingGroup::WebDraftJs;
    }
    if (apps::isWpsOfficeApp(app) || apps::isSteamApp(app) || apps::isJetBrainsApp(app)) {
        return PacingGroup::HeavyApp;
    }
    if (config::isTerminalApp(app)) {
        return PacingGroup::TerminalUinput;
    }
    if (is_autofill) {
        return PacingGroup::WebDraftJs;
    }
    return PacingGroup::Default;
}

std::string AdaptivePacingEngine::serializeState() const {
    std::ostringstream ss;
    ss << "{\n";
    ss << "  \"web\": {\"delay\": " << web_draftjs_.metrics.post_delay_ms
       << ", \"gap\": " << web_draftjs_.metrics.gap_ms << "},\n";
    ss << "  \"terminal\": {\"delay\": " << terminal_uinput_.metrics.post_delay_ms
       << ", \"gap\": " << terminal_uinput_.metrics.gap_ms << "},\n";
    ss << "  \"heavy\": {\"delay\": " << heavy_app_.metrics.post_delay_ms
       << ", \"gap\": " << heavy_app_.metrics.gap_ms << "},\n";
    ss << "  \"default\": {\"delay\": " << default_group_.metrics.post_delay_ms
       << ", \"gap\": " << default_group_.metrics.gap_ms << "}\n";
    ss << "}\n";
    return ss.str();
}

static uint32_t parseUintAfterKey(const std::string& text, size_t pos, const std::string& key) {
    size_t kpos = text.find(key, pos);
    if (kpos == std::string::npos) return 0;
    size_t colon = text.find(':', kpos);
    if (colon == std::string::npos) return 0;
    size_t start = text.find_first_of("0123456789", colon);
    if (start == std::string::npos) return 0;
    size_t end = text.find_first_not_of("0123456789", start);
    std::string num_str = (end == std::string::npos) ? text.substr(start) : text.substr(start, end - start);
    try {
        return static_cast<uint32_t>(std::stoul(num_str));
    } catch (...) {
        return 0;
    }
}

bool AdaptivePacingEngine::deserializeState(const std::string& json) {
    if (json.empty()) return false;

    auto parseBlock = [&](const std::string& section, GroupState& g) {
        size_t sec_pos = json.find("\"" + section + "\"");
        if (sec_pos != std::string::npos) {
            uint32_t delay = parseUintAfterKey(json, sec_pos, "\"delay\"");
            uint32_t gap = parseUintAfterKey(json, sec_pos, "\"gap\"");
            if (delay > 0 && delay >= g.metrics.floor_post_ms && delay <= g.metrics.ceiling_post_ms) {
                g.metrics.post_delay_ms = delay;
            }
            if (gap > 0 && gap >= g.metrics.floor_gap_ms && gap <= g.metrics.ceiling_gap_ms) {
                g.metrics.gap_ms = gap;
            }
        }
    };

    parseBlock("web", web_draftjs_);
    parseBlock("terminal", terminal_uinput_);
    parseBlock("heavy", heavy_app_);
    parseBlock("default", default_group_);
    return true;
}

std::string defaultStateFilePath() {
    const char* xdg_state = std::getenv("XDG_STATE_HOME");
    std::filesystem::path state_dir;
    if (xdg_state && *xdg_state) {
        state_dir = std::filesystem::path(xdg_state) / "clak";
    } else {
        const char* home = std::getenv("HOME");
        if (!home) return "";
        state_dir = std::filesystem::path(home) / ".local" / "state" / "clak";
    }
    return (state_dir / "pacing.json").string();
}

void AdaptivePacingEngine::loadStateFromFile(const std::string& path) {
    std::string file_path = path.empty() ? defaultStateFilePath() : path;
    if (file_path.empty()) return;

    std::ifstream ifs(file_path);
    if (!ifs.is_open()) return;

    std::string content((std::istreambuf_iterator<char>(ifs)),
                        (std::istreambuf_iterator<char>()));
    deserializeState(content);
}

void AdaptivePacingEngine::saveStateToFile(const std::string& path) const {
    std::string file_path = path.empty() ? defaultStateFilePath() : path;
    if (file_path.empty()) return;

    try {
        std::filesystem::path p(file_path);
        std::filesystem::create_directories(p.parent_path());
        std::ofstream ofs(p);
        if (ofs.is_open()) {
            ofs << serializeState();
        }
    } catch (...) {
        // non-fatal
    }
}

} // namespace pacing
} // namespace clak
