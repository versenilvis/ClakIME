#ifndef CLAK_IME_ADAPTIVE_PACING_H
#define CLAK_IME_ADAPTIVE_PACING_H

#include "apps/app_profile.h"
#include <string>
#include <deque>
#include <cstdint>

namespace clak {
namespace pacing {

enum class PacingGroup {
    WebDraftJs,
    TerminalUinput,
    HeavyApp,
    Default
};

enum class IncidentType {
    PacingStarvation,      // sentinel lost without intervening key collision
    KeyInterruption,       // user key collided while uinput was in flight
    SelectionTimeout,      // deliberate 250ms selection timeout
    AddressBarDelay,       // deliberate 20ms autofill delay
    SurroundingMismatch    // surrounding text verification detected mismatch
};

struct PacingMetrics {
    uint32_t post_delay_ms{16};
    uint32_t gap_ms{8};
    uint32_t floor_post_ms{16};
    uint32_t ceiling_post_ms{40};
    uint32_t floor_gap_ms{8};
    uint32_t ceiling_gap_ms{20};
    uint32_t consecutive_clean_words{0};
    uint32_t drift_compensation_ms{0};
    uint32_t smoothed_rtt_ms{0};
    uint32_t commit_delay_ms{0};
};

class AdaptivePacingEngine {
public:
    explicit AdaptivePacingEngine(bool enabled = true);

    void setEnabled(bool enabled);
    bool isEnabled() const;

    // configure base floor from preset or user settings
    void configureGroup(PacingGroup group, uint32_t floor_delay_ms, uint32_t floor_gap_ms,
                        uint32_t ceiling_delay_ms, uint32_t ceiling_gap_ms);

    // pure event handlers
    void onIncident(PacingGroup group, IncidentType incident, uint64_t now_us, uint64_t key_interval_us = 0);
    void observeTimerDrift(PacingGroup group, uint64_t deadline_us, uint64_t fired_at_us);
    void onTransactionSuccess(PacingGroup group);
    void recordRtt(PacingGroup group, uint64_t rtt_us);

    // retrieval
    PacingMetrics metrics(PacingGroup group) const;
    apps::UinputPacing uinputPacing(PacingGroup group) const;
    uint32_t driftCompensation(PacingGroup group) const;
    uint32_t commitCompensation(PacingGroup group) const;

    // classify app and site into pacing group
    static PacingGroup classify(const std::string& app, const std::string& site, bool is_autofill, bool is_draftjs = false, bool is_terminal = false);

    // persistence: serialization to/from simple json string or state file
    std::string serializeState() const;
    bool deserializeState(const std::string& json);

    void loadStateFromFile(const std::string& path = "");
    void saveStateToFile(const std::string& path = "") const;

    // constants
    static constexpr uint64_t kSlidingWindowUs = 10000000ULL; // 10 seconds
    static constexpr uint32_t kDecayWordThreshold = 40;       // words before 1ms decay
    static constexpr uint32_t kAdditiveDelayStepMs = 4;       // additive increase
    static constexpr uint32_t kAdditiveGapStepMs = 2;

private:
    struct GroupState {
        PacingMetrics metrics;
        std::deque<uint64_t> incident_timestamps_us;
    };

    GroupState& getGroup(PacingGroup group);
    const GroupState& getGroup(PacingGroup group) const;

    bool enabled_{true};
    GroupState web_draftjs_;
    GroupState terminal_uinput_;
    GroupState heavy_app_;
    GroupState default_group_;
};

// resolve xdg state path (~/.local/state/clak/pacing.json)
std::string defaultStateFilePath();

} // namespace pacing
} // namespace clak

#endif
