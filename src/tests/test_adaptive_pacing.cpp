#include <gtest/gtest.h>
#include <cstdlib>
#include "ime/adaptive_pacing.h"

using namespace clak::pacing;

class AdaptivePacingTest : public ::testing::Test {
protected:
    void SetUp() override {
        setenv("CLAK_ADAPTIVE", "1", 1);
        unsetenv("CLAK_SIMPLE_BACKOFF");
    }
    void TearDown() override {
        unsetenv("CLAK_ADAPTIVE");
    }
};

TEST_F(AdaptivePacingTest, InitialMetricsAndFloors) {
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

TEST_F(AdaptivePacingTest, AdditiveIncreaseOnStarvation) {
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

TEST_F(AdaptivePacingTest, FilterKeyCollisionAndDeliberateTimeouts) {
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

TEST_F(AdaptivePacingTest, SlowDecayTowardsFloor) {
    AdaptivePacingEngine engine;
    uint64_t t0 = 1000000ULL;

    // bump up delay first
    engine.onIncident(PacingGroup::TerminalUinput, IncidentType::PacingStarvation, t0);
    auto m = engine.metrics(PacingGroup::TerminalUinput);
    EXPECT_EQ(m.post_delay_ms, 16u); // 12 + 4
    EXPECT_EQ(m.gap_ms, 5u);         // 3 + 2

    // record baseline idle rtt (2ms) then elevated rtt (25ms) so conservative decay is used
    engine.recordRtt(PacingGroup::TerminalUinput, 2000ULL);
    engine.recordRtt(PacingGroup::TerminalUinput, 25000ULL);

    // simulate 19 successful transactions -> no decay yet
    for (int i = 0; i < 19; ++i) {
        engine.onTransactionSuccess(PacingGroup::TerminalUinput);
    }
    m = engine.metrics(PacingGroup::TerminalUinput);
    EXPECT_EQ(m.post_delay_ms, 16u);
    EXPECT_EQ(m.gap_ms, 5u);

    // 20th transaction -> 1ms decay
    engine.onTransactionSuccess(PacingGroup::TerminalUinput);
    m = engine.metrics(PacingGroup::TerminalUinput);
    EXPECT_EQ(m.post_delay_ms, 15u);
    EXPECT_EQ(m.gap_ms, 4u);

    // decay repeatedly towards floor, never drops below floor
    for (int i = 0; i < 20 * 20; ++i) {
        engine.onTransactionSuccess(PacingGroup::TerminalUinput);
    }
    m = engine.metrics(PacingGroup::TerminalUinput);
    EXPECT_EQ(m.post_delay_ms, 0u);
    EXPECT_EQ(m.gap_ms, 0u);
}

TEST_F(AdaptivePacingTest, CeilingClamp) {
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

TEST_F(AdaptivePacingTest, Classification) {
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
    EXPECT_EQ(AdaptivePacingEngine::classify("electron-xterm", "clak_electron_xterm", false), PacingGroup::TerminalUinput);
}

TEST_F(AdaptivePacingTest, SubFrameMismatchIgnored) {
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

TEST_F(AdaptivePacingTest, EventLoopTimerDriftCompensation) {
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

TEST_F(AdaptivePacingTest, DynamicRttAutoPacing) {
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

TEST_F(AdaptivePacingTest, CalculatePreDelayWithinPostCommitWindow) {
    AdaptivePacingEngine engine;
    uint64_t t_commit = 5000000ULL;

    EXPECT_EQ(engine.calculatePreDelay(PacingGroup::WebDraftJs, 0, t_commit), 0u);
    EXPECT_EQ(engine.calculatePreDelay(PacingGroup::WebDraftJs, t_commit, t_commit), 16u);
    EXPECT_EQ(engine.calculatePreDelay(PacingGroup::WebDraftJs, t_commit, t_commit + 15000ULL), 16u);
    EXPECT_EQ(engine.calculatePreDelay(PacingGroup::WebDraftJs, t_commit, t_commit + 40000ULL), 0u);

    // elevated rtt expands pre-delay buffer up to ceiling
    engine.recordRtt(PacingGroup::WebDraftJs, 60000ULL);
    EXPECT_EQ(engine.calculatePreDelay(PacingGroup::WebDraftJs, t_commit, t_commit), 30u);

    // terminal uinput always skips pre-delay to avoid scheduler jitter
    EXPECT_EQ(engine.calculatePreDelay(PacingGroup::TerminalUinput, t_commit, t_commit), 0u);
    EXPECT_EQ(engine.calculatePreDelay(PacingGroup::TerminalUinput, t_commit, t_commit + 15000ULL), 0u);
}

TEST_F(AdaptivePacingTest, ConfigureGroupZeroFloorAppliesImmediately) {
    AdaptivePacingEngine engine;
    EXPECT_EQ(engine.metrics(PacingGroup::TerminalUinput).post_delay_ms, 12u);
    EXPECT_EQ(engine.metrics(PacingGroup::TerminalUinput).gap_ms, 3u);

    engine.configureGroup(PacingGroup::TerminalUinput, 0, 0, 30, 10);
    EXPECT_EQ(engine.metrics(PacingGroup::TerminalUinput).post_delay_ms, 0u);
    EXPECT_EQ(engine.metrics(PacingGroup::TerminalUinput).gap_ms, 0u);
    EXPECT_EQ(engine.metrics(PacingGroup::TerminalUinput).floor_post_ms, 0u);
    EXPECT_EQ(engine.metrics(PacingGroup::TerminalUinput).floor_gap_ms, 0u);
}

TEST_F(AdaptivePacingTest, FastDecayWhenRttLow) {
    AdaptivePacingEngine engine;
    uint64_t t0 = 1000000ULL;
    // baseline idle rtt
    engine.recordRtt(PacingGroup::TerminalUinput, 2000ULL);
    // raise delay via starvation incidents
    engine.onIncident(PacingGroup::TerminalUinput, IncidentType::PacingStarvation, t0);
    engine.onIncident(PacingGroup::TerminalUinput, IncidentType::PacingStarvation, t0 + 10000ULL);
    EXPECT_EQ(engine.metrics(PacingGroup::TerminalUinput).post_delay_ms, 20u);
    EXPECT_EQ(engine.metrics(PacingGroup::TerminalUinput).ssthresh_post_ms, 18u);

    // low rtt indicates clean idle ipc, above ssthresh allows fast decay
    engine.recordRtt(PacingGroup::TerminalUinput, 2000ULL);

    // 5 clean transactions should decay by 2ms down to ssthresh
    for (int i = 0; i < 5; ++i) {
        engine.onTransactionSuccess(PacingGroup::TerminalUinput);
    }
    EXPECT_EQ(engine.metrics(PacingGroup::TerminalUinput).post_delay_ms, 18u);

    // at ssthresh, fast decay halts and switches to conservative decay
    for (int i = 0; i < 5; ++i) {
        engine.onTransactionSuccess(PacingGroup::TerminalUinput);
    }
    EXPECT_EQ(engine.metrics(PacingGroup::TerminalUinput).post_delay_ms, 18u);
}

TEST_F(AdaptivePacingTest, ConservativeDecayWhenRttHigh) {
    AdaptivePacingEngine engine;
    uint64_t t0 = 1000000ULL;
    // record baseline idle rtt (2ms) across multiple samples
    for (int i = 0; i < 10; ++i) {
        engine.recordRtt(PacingGroup::TerminalUinput, 2000ULL);
    }
    engine.onIncident(PacingGroup::TerminalUinput, IncidentType::PacingStarvation, t0);
    engine.onIncident(PacingGroup::TerminalUinput, IncidentType::PacingStarvation, t0 + 10000ULL);
    EXPECT_EQ(engine.metrics(PacingGroup::TerminalUinput).post_delay_ms, 20u);

    // high rtt (25ms) indicates ongoing system load
    engine.recordRtt(PacingGroup::TerminalUinput, 25000ULL);

    // 5 transactions must not decay under high rtt
    for (int i = 0; i < 5; ++i) {
        engine.onTransactionSuccess(PacingGroup::TerminalUinput);
    }
    EXPECT_EQ(engine.metrics(PacingGroup::TerminalUinput).post_delay_ms, 20u);

    // only after 20 clean transactions does it decay by 1ms
    for (int i = 0; i < 15; ++i) {
        engine.onTransactionSuccess(PacingGroup::TerminalUinput);
    }
    EXPECT_EQ(engine.metrics(PacingGroup::TerminalUinput).post_delay_ms, 19u);
}

TEST_F(AdaptivePacingTest, FastOutlierDoesNotPolluteP10Baseline) {
    AdaptivePacingEngine engine;
    // normal samples around 10ms
    for (int i = 0; i < 20; ++i) {
        engine.recordRtt(PacingGroup::TerminalUinput, 10000ULL);
    }
    // a single anomalously fast sample (100us = 0ms)
    engine.recordRtt(PacingGroup::TerminalUinput, 100ULL);

    // min_rtt_ms (p10) should still be 10ms, not 0ms
    EXPECT_EQ(engine.metrics(PacingGroup::TerminalUinput).min_rtt_ms, 10u);
}

TEST_F(AdaptivePacingTest, ProlongedHighRttDoesNotInflateBaselineOrTriggerPrematureDecay) {
    AdaptivePacingEngine engine;
    uint64_t t0 = 1000000ULL;
    // initial idle baseline of 2ms
    for (int i = 0; i < 20; ++i) {
        engine.recordRtt(PacingGroup::TerminalUinput, 2000ULL);
    }
    EXPECT_EQ(engine.metrics(PacingGroup::TerminalUinput).min_rtt_ms, 2u);

    // simulate a heavy load spike that triggers starvation
    engine.onIncident(PacingGroup::TerminalUinput, IncidentType::PacingStarvation, t0);
    EXPECT_EQ(engine.metrics(PacingGroup::TerminalUinput).post_delay_ms, 16u);

    // simulate 100 transactions under prolonged high RTT (30ms) during a 5-minute build
    for (int i = 0; i < 100; ++i) {
        engine.recordRtt(PacingGroup::TerminalUinput, 30000ULL);
        engine.onTransactionSuccess(PacingGroup::TerminalUinput);
    }

    // asymmetric filter ensures baseline does NOT jump to 30ms
    EXPECT_LE(engine.metrics(PacingGroup::TerminalUinput).min_rtt_ms, 4u);

    // while under high RTT load, fast decay must NOT have triggered, preserving safe delay
    EXPECT_GT(engine.metrics(PacingGroup::TerminalUinput).post_delay_ms, engine.metrics(PacingGroup::TerminalUinput).floor_post_ms);
}

TEST_F(AdaptivePacingTest, SsthreshPreventsPrematureDecayAtHighDelay) {
    AdaptivePacingEngine engine;
    uint64_t t0 = 1000000ULL;
    for (int i = 0; i < 10; ++i) {
        engine.recordRtt(PacingGroup::WebDraftJs, 2000ULL);
    }
    // force delay up to 28ms via multiple incidents
    for (int i = 0; i < 3; ++i) {
        engine.onIncident(PacingGroup::WebDraftJs, IncidentType::PacingStarvation, t0 + i * 10000ULL);
    }
    // current delay is 28ms, ssthresh is 26ms
    auto m = engine.metrics(PacingGroup::WebDraftJs);
    EXPECT_EQ(m.post_delay_ms, 28u);
    EXPECT_EQ(m.ssthresh_post_ms, 26u);

    // fast decay drops delay from 28ms to 26ms in 5 words
    for (int i = 0; i < 5; ++i) {
        engine.onTransactionSuccess(PacingGroup::WebDraftJs);
    }
    EXPECT_EQ(engine.metrics(PacingGroup::WebDraftJs).post_delay_ms, 26u);

    // in the next 14 words of brief idle, ssthresh holds delay at 26ms
    for (int i = 0; i < 14; ++i) {
        engine.onTransactionSuccess(PacingGroup::WebDraftJs);
    }
    EXPECT_EQ(engine.metrics(PacingGroup::WebDraftJs).post_delay_ms, 26u);
}

TEST_F(AdaptivePacingTest, TcpStyleSsthreshDecayAndRelaxation) {
    AdaptivePacingEngine engine;
    uint64_t t0 = 1000000ULL;
    // baseline idle rtt (2ms)
    engine.recordRtt(PacingGroup::WebDraftJs, 2000ULL);

    // initial draftjs post delay is 16ms
    EXPECT_EQ(engine.metrics(PacingGroup::WebDraftJs).post_delay_ms, 16u);

    // incident at 16ms establishes ssthresh at 18ms (16 + 2) and raises delay to 20ms
    engine.onIncident(PacingGroup::WebDraftJs, IncidentType::PacingStarvation, t0);
    EXPECT_EQ(engine.metrics(PacingGroup::WebDraftJs).post_delay_ms, 20u);
    EXPECT_EQ(engine.metrics(PacingGroup::WebDraftJs).ssthresh_post_ms, 18u);

    // fast decay above ssthresh: 5 clean words drop delay by 2ms to 18ms
    for (int i = 0; i < 5; ++i) {
        engine.onTransactionSuccess(PacingGroup::WebDraftJs);
    }
    EXPECT_EQ(engine.metrics(PacingGroup::WebDraftJs).post_delay_ms, 18u);

    // at ssthresh, fast decay halts to prevent dropping into the danger zone
    for (int i = 0; i < 5; ++i) {
        engine.onTransactionSuccess(PacingGroup::WebDraftJs);
    }
    EXPECT_EQ(engine.metrics(PacingGroup::WebDraftJs).post_delay_ms, 18u);

    // conservative decay: after 20 clean words total, decays 1ms and relaxes ssthresh
    for (int i = 0; i < 15; ++i) {
        engine.onTransactionSuccess(PacingGroup::WebDraftJs);
    }
    EXPECT_EQ(engine.metrics(PacingGroup::WebDraftJs).post_delay_ms, 17u);
    EXPECT_EQ(engine.metrics(PacingGroup::WebDraftJs).ssthresh_post_ms, 17u);

    // recurring incident at 17ms raises ssthresh to 19ms and delay to 21ms
    engine.onIncident(PacingGroup::WebDraftJs, IncidentType::PacingStarvation, t0 + 5000000ULL);
    EXPECT_EQ(engine.metrics(PacingGroup::WebDraftJs).post_delay_ms, 21u);
    EXPECT_EQ(engine.metrics(PacingGroup::WebDraftJs).ssthresh_post_ms, 19u);
}

TEST(SimpleBackoffDefaultTest, DefaultModeUsesFixed13WithBackoffAndDecay) {
    unsetenv("CLAK_ADAPTIVE");
    unsetenv("CLAK_SIMPLE_BACKOFF");
    AdaptivePacingEngine engine;

    for (auto grp : {PacingGroup::WebDraftJs, PacingGroup::TerminalUinput, PacingGroup::HeavyApp, PacingGroup::Default}) {
        auto m = engine.metrics(grp);
        EXPECT_EQ(m.post_delay_ms, 13u);
        EXPECT_EQ(m.gap_ms, 2u);
        EXPECT_EQ(m.floor_post_ms, 13u);
        EXPECT_EQ(m.ceiling_post_ms, 30u);
        EXPECT_EQ(engine.calculatePreDelay(grp, 1000000ULL, 1010000ULL), 13u);
    }

    // incident jumps to 30ms / 4ms
    engine.onIncident(PacingGroup::Default, IncidentType::PacingStarvation, 1000000ULL);
    EXPECT_EQ(engine.metrics(PacingGroup::Default).post_delay_ms, 30u);
    EXPECT_EQ(engine.metrics(PacingGroup::Default).gap_ms, 4u);
    EXPECT_EQ(engine.calculatePreDelay(PacingGroup::Default, 1000000ULL, 1010000ULL), 30u);

    // 20 clean words decay by 1ms
    for (int i = 0; i < 20; ++i) {
        engine.onTransactionSuccess(PacingGroup::Default);
    }
    EXPECT_EQ(engine.metrics(PacingGroup::Default).post_delay_ms, 29u);
    EXPECT_EQ(engine.metrics(PacingGroup::Default).gap_ms, 3u);
}
