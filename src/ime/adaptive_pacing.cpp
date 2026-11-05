#include "ime/adaptive_pacing.h"
#include "config/sites.h"
#include "apps/terminal.h"
#include "apps/wps.h"
#include "apps/steam.h"
#include "apps/ide.h"
#include "utils/log.h"
#include <algorithm>
#include <cstdlib>

namespace clak {
namespace pacing {

AdaptivePacingEngine::AdaptivePacingEngine(bool enabled)
    : enabled_(enabled) {
    web_draftjs_.metrics = {16, 8, 10, 40, 4, 20, 0, 0, 0, 0, 8, 30, config::kDefaultUinputHoldUs, 10, 0};
    terminal_uinput_.metrics = {12, 3, 0, 30, 0, 10, 0, 0, 0, 0, 0, 0, config::kDefaultUinputHoldUs, 0, 0};
    heavy_app_.metrics = {15, 4, 8, 35, 3, 12, 0, 0, 0, 0, 8, 30, config::kDefaultUinputHoldUs, 8, 0};
    default_group_.metrics = {8, 2, 4, 30, 2, 10, 0, 0, 0, 0, 8, 30, config::kDefaultUinputHoldUs, 4, 0};

    const char* env_hold = std::getenv("CLAK_UINPUT_HOLD_US");
    if (env_hold) {
        try {
            uint32_t h = static_cast<uint32_t>(std::stoul(env_hold));
            terminal_uinput_.metrics.hold_us = h;
            default_group_.metrics.hold_us = h;
            heavy_app_.metrics.hold_us = h;
            web_draftjs_.metrics.hold_us = h;
        } catch (...) {}
    }

    const char* env_pinned = std::getenv("CLAK_PINNED_DELAY_MS");
    if (env_pinned) {
        try {
            uint32_t p = static_cast<uint32_t>(std::stoul(env_pinned));
            for (auto* g : {&web_draftjs_, &terminal_uinput_, &heavy_app_, &default_group_}) {
                g->metrics.post_delay_ms = p;
                g->metrics.floor_post_ms = p;
                g->metrics.ceiling_post_ms = p;
                g->metrics.ssthresh_post_ms = p;
            }
        } catch (...) {}
    }
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
    const char* env_pinned = std::getenv("CLAK_PINNED_DELAY_MS");
    if (env_pinned) {
        try {
            uint32_t p = static_cast<uint32_t>(std::stoul(env_pinned));
            floor_delay_ms = p;
            ceiling_delay_ms = p;
        } catch (...) {}
    }

    auto& g = getGroup(group);
    g.metrics.floor_post_ms = floor_delay_ms;
    g.metrics.floor_gap_ms = floor_gap_ms;
    g.metrics.ceiling_post_ms = ceiling_delay_ms;
    g.metrics.ceiling_gap_ms = ceiling_gap_ms;

    if (g.incident_timestamps_us.empty()) {
        g.metrics.post_delay_ms = floor_delay_ms;
        g.metrics.gap_ms = floor_gap_ms;
        g.metrics.ssthresh_post_ms = floor_delay_ms;
    } else {
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

        if (g.metrics.ssthresh_post_ms < floor_delay_ms) {
            g.metrics.ssthresh_post_ms = floor_delay_ms;
        } else if (g.metrics.ssthresh_post_ms > ceiling_delay_ms) {
            g.metrics.ssthresh_post_ms = ceiling_delay_ms;
        }
    }
}

void AdaptivePacingEngine::onIncident(PacingGroup group, IncidentType incident, uint64_t now_us, uint64_t key_interval_us) {
    if (!enabled_) {
        return;
    }

    // ignore key collisions or deliberate timeouts
    if (incident == IncidentType::KeyInterruption ||
        incident == IncidentType::SelectionTimeout ||
        incident == IncidentType::AddressBarDelay) {
        return;
    }

    // ignore sub-frame surrounding mismatches caused by in-flight wayland ipc
    if (incident == IncidentType::SurroundingMismatch && key_interval_us > 0 && key_interval_us < 16000) {
        utils::clakLog("adaptive pacing: ignored sub-frame surrounding mismatch (interval=" +
                       std::to_string(key_interval_us / 1000) + "ms < 16ms)");
        return;
    }

    auto& g = getGroup(group);

    // purge incidents outside sliding window
    while (!g.incident_timestamps_us.empty() &&
           now_us - g.incident_timestamps_us.front() > kSlidingWindowUs) {
        g.incident_timestamps_us.pop_front();
    }
    g.incident_timestamps_us.push_back(now_us);

    // update ssthresh threshold to current delay + 2ms margin
    uint32_t last_incident_delay = g.metrics.post_delay_ms;
    g.metrics.ssthresh_post_ms = std::min(g.metrics.ceiling_post_ms, last_incident_delay + 2);
    if (std::getenv("CLAK_DISABLE_SSTHRESH")) {
        g.metrics.ssthresh_post_ms = g.metrics.floor_post_ms;
    }

    // additive increase
    g.metrics.post_delay_ms = std::min(g.metrics.ceiling_post_ms,
                                       g.metrics.post_delay_ms + kAdditiveDelayStepMs);
    g.metrics.gap_ms = std::min(g.metrics.ceiling_gap_ms,
                                g.metrics.gap_ms + kAdditiveGapStepMs);
    g.metrics.consecutive_clean_words = 0;
    utils::clakLog("[PACING_INCIDENT] group=" + std::to_string(static_cast<int>(group)) +
                   " post_delay=" + std::to_string(g.metrics.post_delay_ms) +
                   "ms ssthresh=" + std::to_string(g.metrics.ssthresh_post_ms) + "ms");
}

void AdaptivePacingEngine::observeTimerDrift(PacingGroup group, uint64_t deadline_us, uint64_t fired_at_us) {
    if (!enabled_ || fired_at_us <= deadline_us) {
        return;
    }
    uint64_t drift_us = fired_at_us - deadline_us;
    if (drift_us >= 5000) {
        auto& g = getGroup(group);
        uint32_t drift_ms = static_cast<uint32_t>((drift_us + 999) / 1000);
        g.metrics.drift_compensation_ms = std::min(15u, std::max(g.metrics.drift_compensation_ms, drift_ms));
        utils::clakLog("adaptive pacing: event loop drift " + std::to_string(drift_ms) + "ms detected, compensating");
    }
}

void AdaptivePacingEngine::onTransactionSuccess(PacingGroup group) {
    if (!enabled_) {
        return;
    }

    auto& g = getGroup(group);
    if (g.metrics.drift_compensation_ms > 0) {
        g.metrics.drift_compensation_ms -= 1;
    }
    g.metrics.consecutive_clean_words++;

    // determine if rtt is healthy relative to observed p10 baseline
    // proportional threshold of baseline * 1.5 with 2ms floor for fast systems
    uint32_t rtt_threshold = std::max(2u, g.metrics.min_rtt_ms * 3 / 2);
    bool rtt_healthy = (g.metrics.smoothed_rtt_ms == 0) ||
                       (g.metrics.min_rtt_ms == 0) ||
                       (g.metrics.smoothed_rtt_ms <= rtt_threshold);

    // tcp-style ssthresh: fast decay when comfortably above incident threshold
    bool above_ssthresh = (g.metrics.post_delay_ms > g.metrics.ssthresh_post_ms);
    bool use_fast_decay = above_ssthresh && rtt_healthy;

    uint32_t decay_threshold = use_fast_decay ? kFastDecayWordThreshold : kConservativeDecayWordThreshold;
    uint32_t step = use_fast_decay ? kFastDecayStepMs : kConservativeDecayStepMs;

    if (g.metrics.consecutive_clean_words >= decay_threshold) {
        g.metrics.consecutive_clean_words = 0;
        if (g.metrics.post_delay_ms > g.metrics.floor_post_ms) {
            uint32_t delta = g.metrics.post_delay_ms - g.metrics.floor_post_ms;
            g.metrics.post_delay_ms -= std::min(step, delta);
        }
        if (g.metrics.gap_ms > g.metrics.floor_gap_ms) {
            uint32_t delta = g.metrics.gap_ms - g.metrics.floor_gap_ms;
            g.metrics.gap_ms -= std::min(step, delta);
        }
        // relax ssthresh downwards during sustained clean periods
        if (!use_fast_decay && g.metrics.ssthresh_post_ms > g.metrics.post_delay_ms) {
            g.metrics.ssthresh_post_ms = g.metrics.post_delay_ms;
        }
        if (g.metrics.commit_delay_ms > 0 && g.metrics.smoothed_rtt_ms < 20) {
            g.metrics.commit_delay_ms -= 1;
        }
    }
}

void AdaptivePacingEngine::recordRtt(PacingGroup group, uint64_t rtt_us) {
    if (!enabled_ || rtt_us == 0) {
        return;
    }

    auto& g = getGroup(group);
    uint32_t rtt_ms = static_cast<uint32_t>((rtt_us + 500) / 1000);

    // track baseline rtt using asymmetric filter with p10 buffer to resist outliers
    if (rtt_ms > 0) {
        g.rtt_samples.push_back(rtt_ms);
        if (g.rtt_samples.size() > 50) {
            g.rtt_samples.pop_front();
        }
        std::vector<uint32_t> sorted(g.rtt_samples.begin(), g.rtt_samples.end());
        std::sort(sorted.begin(), sorted.end());
        size_t p10_idx = sorted.size() / 10;
        uint32_t current_p10 = sorted[p10_idx];

        if (g.metrics.min_rtt_ms == 0) {
            g.metrics.min_rtt_ms = current_p10;
        } else if (current_p10 < g.metrics.min_rtt_ms) {
            // fast drop when system returns to idle or a lower baseline is confirmed
            g.metrics.min_rtt_ms = current_p10;
        } else if (current_p10 > g.metrics.min_rtt_ms) {
            // asymmetric: baseline can only creep up during prolonged clean periods with zero incidents
            if (g.incident_timestamps_us.empty() && g.metrics.consecutive_clean_words >= 50) {
                g.metrics.min_rtt_ms += 1;
            }
        }
    }

    // smoothed rtt using exponential moving average
    if (g.metrics.smoothed_rtt_ms == 0) {
        g.metrics.smoothed_rtt_ms = rtt_ms;
    } else {
        g.metrics.smoothed_rtt_ms = (g.metrics.smoothed_rtt_ms * 7 + rtt_ms * 3) / 10;
    }

    // dynamic auto pacing based on observed rtt for web and heavy apps
    if (group == PacingGroup::WebDraftJs || group == PacingGroup::HeavyApp) {
        if (g.metrics.smoothed_rtt_ms > g.metrics.floor_post_ms) {
            uint32_t target_post = std::min(g.metrics.ceiling_post_ms, g.metrics.smoothed_rtt_ms);
            if (target_post > g.metrics.post_delay_ms) {
                g.metrics.post_delay_ms = target_post;
            }
        }
        if (g.metrics.smoothed_rtt_ms >= 20) {
            g.metrics.commit_delay_ms = std::min(20u, std::max(10u, g.metrics.smoothed_rtt_ms / 2));
        } else {
            g.metrics.commit_delay_ms = 0;
        }
    }
}

PacingMetrics AdaptivePacingEngine::metrics(PacingGroup group) const {
    return getGroup(group).metrics;
}

apps::UinputPacing AdaptivePacingEngine::uinputPacing(PacingGroup group) const {
    const auto& m = getGroup(group).metrics;
    uint32_t eff_post = std::min(m.ceiling_post_ms, m.post_delay_ms + m.drift_compensation_ms);
    return {eff_post, m.gap_ms, m.hold_us};
}

void AdaptivePacingEngine::setHoldUs(PacingGroup group, uint32_t hold_us) {
    getGroup(group).metrics.hold_us = hold_us;
}

uint32_t AdaptivePacingEngine::driftCompensation(PacingGroup group) const {
    return getGroup(group).metrics.drift_compensation_ms;
}

uint32_t AdaptivePacingEngine::commitCompensation(PacingGroup group) const {
    const auto& m = getGroup(group).metrics;
    return std::max(m.drift_compensation_ms, m.commit_delay_ms);
}

uint32_t AdaptivePacingEngine::calculatePreDelay(PacingGroup group, uint64_t last_commit_time_us, uint64_t now_us) const {
    if (group == PacingGroup::TerminalUinput || last_commit_time_us == 0) {
        return 0;
    }
    const auto& m = getGroup(group).metrics;
    uint64_t window_us = static_cast<uint64_t>(m.post_commit_window_ms) * 1000ULL;
    if (now_us <= last_commit_time_us || (now_us - last_commit_time_us) < window_us) {
        uint32_t eff_pre = std::max(m.post_delay_ms, m.pre_delay_ms);
        // adapt pre-delay to target application latency under load
        if (m.smoothed_rtt_ms > eff_pre) {
            eff_pre = std::min(m.ceiling_post_ms, std::max(eff_pre, m.smoothed_rtt_ms / 2));
        }
        return eff_pre;
    }
    return 0;
}

PacingGroup AdaptivePacingEngine::classify(const std::string& app, const std::string& site, bool is_autofill, bool is_draftjs, bool is_terminal) {
    if (is_terminal) {
        return PacingGroup::TerminalUinput;
    }
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

} // namespace pacing
} // namespace clak
