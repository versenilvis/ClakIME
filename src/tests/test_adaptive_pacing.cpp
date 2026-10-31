#include <gtest/gtest.h>
#include "ime/adaptive_pacing.h"
#include <filesystem>
#include <fstream>

using namespace clak::pacing;

TEST(AdaptivePacingTest, InitialMetricsAndFloors) {
    AdaptivePacingEngine engine;
    EXPECT_TRUE(engine.isEnabled());

    auto web = engine.metrics(PacingGroup::WebDraftJs);
    EXPECT_EQ(web.post_delay_ms, 16u);
    EXPECT_EQ(web.gap_ms, 8u);
    EXPECT_EQ(web.floor_post_ms, 10u);
    EXPECT_EQ(web.ceiling_post_ms, 40u);

    auto term = engine.metrics(PacingGroup::TerminalUinput);
    EXPECT_EQ(term.post_delay_ms, 12u);
    EXPECT_EQ(term.gap_ms, 3u);

    auto heavy = engine.metrics(PacingGroup::HeavyApp);
    EXPECT_EQ(heavy.post_delay_ms, 15u);
    EXPECT_EQ(heavy.gap_ms, 4u);
}

TEST(AdaptivePacingTest, AdditiveIncreaseOnStarvation) {
    AdaptivePacingEngine engine;
    uint64_t t0 = 1000000ULL;

    engine.onIncident(PacingGroup::WebDraftJs, IncidentType::PacingStarvation, t0);
    auto m = engine.metrics(PacingGroup::WebDraftJs);
    EXPECT_EQ(m.post_delay_ms, 20u); // 16 + 4
    EXPECT_EQ(m.gap_ms, 10u);        // 8 + 2
    EXPECT_EQ(m.consecutive_clean_words, 0u);

    // second incident
    engine.onIncident(PacingGroup::WebDraftJs, IncidentType::PacingStarvation, t0 + 100000ULL);
    m = engine.metrics(PacingGroup::WebDraftJs);
    EXPECT_EQ(m.post_delay_ms, 24u);
    EXPECT_EQ(m.gap_ms, 12u);
}

TEST(AdaptivePacingTest, FilterKeyCollisionAndDeliberateTimeouts) {
    AdaptivePacingEngine engine;
    uint64_t t0 = 1000000ULL;

    // key interruption should NOT increase delay
    engine.onIncident(PacingGroup::WebDraftJs, IncidentType::KeyInterruption, t0);
    auto m = engine.metrics(PacingGroup::WebDraftJs);
    EXPECT_EQ(m.post_delay_ms, 16u);
    EXPECT_EQ(m.gap_ms, 8u);

    // selection and address bar delays should also be ignored
    engine.onIncident(PacingGroup::WebDraftJs, IncidentType::SelectionTimeout, t0 + 10000ULL);
    engine.onIncident(PacingGroup::WebDraftJs, IncidentType::AddressBarDelay, t0 + 20000ULL);
    m = engine.metrics(PacingGroup::WebDraftJs);
    EXPECT_EQ(m.post_delay_ms, 16u);
    EXPECT_EQ(m.gap_ms, 8u);
}

TEST(AdaptivePacingTest, SlowDecayTowardsFloor) {
    AdaptivePacingEngine engine;
    uint64_t t0 = 1000000ULL;

    // bump up delay first
    engine.onIncident(PacingGroup::TerminalUinput, IncidentType::PacingStarvation, t0);
    auto m = engine.metrics(PacingGroup::TerminalUinput);
    EXPECT_EQ(m.post_delay_ms, 16u); // 12 + 4
    EXPECT_EQ(m.gap_ms, 5u);         // 3 + 2

    // simulate 39 successful transactions -> no decay yet
    for (int i = 0; i < 39; ++i) {
        engine.onTransactionSuccess(PacingGroup::TerminalUinput);
    }
    m = engine.metrics(PacingGroup::TerminalUinput);
    EXPECT_EQ(m.post_delay_ms, 16u);
    EXPECT_EQ(m.gap_ms, 5u);

    // 40th transaction -> 1ms decay
    engine.onTransactionSuccess(PacingGroup::TerminalUinput);
    m = engine.metrics(PacingGroup::TerminalUinput);
    EXPECT_EQ(m.post_delay_ms, 15u);
    EXPECT_EQ(m.gap_ms, 4u);

    // decay repeatedly towards floor, never drops below floor (6ms, gap 2ms)
    for (int i = 0; i < 40 * 20; ++i) {
        engine.onTransactionSuccess(PacingGroup::TerminalUinput);
    }
    m = engine.metrics(PacingGroup::TerminalUinput);
    EXPECT_EQ(m.post_delay_ms, 6u); // reached floor
    EXPECT_EQ(m.gap_ms, 2u);        // reached floor
}

TEST(AdaptivePacingTest, CeilingClamp) {
    AdaptivePacingEngine engine;
    uint64_t t0 = 1000000ULL;

    // trigger starvation 20 times -> should be capped at ceiling (40ms, gap 20ms)
    for (int i = 0; i < 20; ++i) {
        engine.onIncident(PacingGroup::WebDraftJs, IncidentType::PacingStarvation, t0 + i * 10000ULL);
    }
    auto m = engine.metrics(PacingGroup::WebDraftJs);
    EXPECT_EQ(m.post_delay_ms, 40u);
    EXPECT_EQ(m.gap_ms, 20u);
}

TEST(AdaptivePacingTest, Classification) {
    EXPECT_EQ(AdaptivePacingEngine::classify("x-terminal-emulator", "", false), PacingGroup::TerminalUinput);
    EXPECT_EQ(AdaptivePacingEngine::classify("kitty", "", false), PacingGroup::TerminalUinput);
    EXPECT_EQ(AdaptivePacingEngine::classify("alacritty", "", false), PacingGroup::TerminalUinput);

    EXPECT_EQ(AdaptivePacingEngine::classify("google-chrome", "x.com", false), PacingGroup::WebDraftJs);
    EXPECT_EQ(AdaptivePacingEngine::classify("firefox", "twitter.com", false), PacingGroup::WebDraftJs);
    EXPECT_EQ(AdaptivePacingEngine::classify("brave", "tiktok.com", false), PacingGroup::WebDraftJs);

    EXPECT_EQ(AdaptivePacingEngine::classify("wps", "", false), PacingGroup::HeavyApp);
    EXPECT_EQ(AdaptivePacingEngine::classify("steam", "", false), PacingGroup::HeavyApp);
    EXPECT_EQ(AdaptivePacingEngine::classify("idea.sh", "", false), PacingGroup::HeavyApp);

    EXPECT_EQ(AdaptivePacingEngine::classify("gedit", "", false), PacingGroup::Default);
}

TEST(AdaptivePacingTest, SerializationRoundtrip) {
    AdaptivePacingEngine engine1;
    engine1.onIncident(PacingGroup::WebDraftJs, IncidentType::PacingStarvation, 1000ULL);
    engine1.onIncident(PacingGroup::TerminalUinput, IncidentType::PacingStarvation, 2000ULL);

    std::string json = engine1.serializeState();
    EXPECT_FALSE(json.empty());

    AdaptivePacingEngine engine2;
    EXPECT_TRUE(engine2.deserializeState(json));

    EXPECT_EQ(engine2.metrics(PacingGroup::WebDraftJs).post_delay_ms, 20u);
    EXPECT_EQ(engine2.metrics(PacingGroup::TerminalUinput).post_delay_ms, 16u);
}

TEST(AdaptivePacingTest, FilePersistence) {
    std::string test_path = "/tmp/test_clak_pacing_state.json";
    std::filesystem::remove(test_path);

    AdaptivePacingEngine engine1;
    engine1.onIncident(PacingGroup::HeavyApp, IncidentType::PacingStarvation, 1000ULL);
    engine1.saveStateToFile(test_path);

    EXPECT_TRUE(std::filesystem::exists(test_path));

    AdaptivePacingEngine engine2;
    engine2.loadStateFromFile(test_path);
    EXPECT_EQ(engine2.metrics(PacingGroup::HeavyApp).post_delay_ms, 19u); // 15 + 4

    std::filesystem::remove(test_path);
}

TEST(AdaptivePacingTest, SubFrameMismatchIgnored) {
    AdaptivePacingEngine engine;
    uint64_t t0 = 1000000ULL;

    // sub-frame ipc latency (< 16ms) ignored
    engine.onIncident(PacingGroup::WebDraftJs, IncidentType::SurroundingMismatch, t0, 8000ULL);
    auto m = engine.metrics(PacingGroup::WebDraftJs);
    EXPECT_EQ(m.post_delay_ms, 16u);
    EXPECT_EQ(m.gap_ms, 8u);

    // intervals >= 16ms accounted for
    engine.onIncident(PacingGroup::WebDraftJs, IncidentType::SurroundingMismatch, t0 + 100000ULL, 20000ULL);
    m = engine.metrics(PacingGroup::WebDraftJs);
    EXPECT_EQ(m.post_delay_ms, 20u);
    EXPECT_EQ(m.gap_ms, 10u);
}

TEST(AdaptivePacingTest, EventLoopTimerDriftCompensation) {
    AdaptivePacingEngine engine;

    // drift < 5ms ignored
    engine.observeTimerDrift(PacingGroup::WebDraftJs, 1000000ULL, 1003000ULL);
    EXPECT_EQ(engine.driftCompensation(PacingGroup::WebDraftJs), 0u);

    // drift >= 5ms increases compensation
    engine.observeTimerDrift(PacingGroup::WebDraftJs, 1000000ULL, 1008000ULL);
    EXPECT_EQ(engine.driftCompensation(PacingGroup::WebDraftJs), 8u);
    auto pacing = engine.uinputPacing(PacingGroup::WebDraftJs);
    EXPECT_EQ(pacing.post_delay_ms, 24u);

    // drift capped at 15ms
    engine.observeTimerDrift(PacingGroup::WebDraftJs, 1000000ULL, 1030000ULL);
    EXPECT_EQ(engine.driftCompensation(PacingGroup::WebDraftJs), 15u);

    // success decays drift compensation
    engine.onTransactionSuccess(PacingGroup::WebDraftJs);
    EXPECT_EQ(engine.driftCompensation(PacingGroup::WebDraftJs), 14u);
}

TEST(AdaptivePacingTest, DynamicRttAutoPacing) {
    AdaptivePacingEngine engine;

    // fast rtt keeps default delay and zero commit settle window
    engine.recordRtt(PacingGroup::WebDraftJs, 8000ULL);
    EXPECT_EQ(engine.metrics(PacingGroup::WebDraftJs).smoothed_rtt_ms, 8u);
    EXPECT_EQ(engine.commitCompensation(PacingGroup::WebDraftJs), 0u);

    // high rtt raises post delay and sets commit settle window
    for (int i = 0; i < 5; ++i) {
        engine.recordRtt(PacingGroup::WebDraftJs, 34000ULL);
    }
    auto m = engine.metrics(PacingGroup::WebDraftJs);
    EXPECT_GE(m.smoothed_rtt_ms, 25u);
    EXPECT_GE(m.post_delay_ms, 25u);
    EXPECT_GE(engine.commitCompensation(PacingGroup::WebDraftJs), 10u);
}
