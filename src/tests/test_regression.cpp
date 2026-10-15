#include <gtest/gtest.h>
#include <fcitx/instance.h>
#include <fcitx-utils/event.h>
#include "mock_input_context.h"
#include "ime/state.h"
#include "engine.h"
#include "core.h"
#include "uinput/uinput.h"
#include "platform/window_info.h"
#include "config/config.h"

namespace clak {
namespace test {

class RegressionCorpusTest : public ::testing::Test {
protected:
    void SetUp() override {
        int argc = 1;
        char arg0[] = "clak_regression_test";
        char* argv[] = {arg0, nullptr};
        instance_ = std::make_unique<fcitx::Instance>(argc, argv);
        engine_ = std::make_unique<ClakEngine>(instance_.get());
        engine_->setConfigForTest(clak_config_default());
        uinput::UinputTool::instance().setMockHandler([](size_t, uint32_t, uint32_t, uint32_t) {
            return true;
        });
        platform::setMockActiveWindow(platform::WindowInfo{"google-chrome", "Standard Page", 1234});
    }

    void TearDown() override {
        uinput::UinputTool::instance().clearMockHandler();
        platform::clearMockActiveWindow();
        engine_.reset();
        instance_.reset();
    }

    std::unique_ptr<fcitx::Instance> instance_;
    std::unique_ptr<ClakEngine> engine_;
};

TEST_F(RegressionCorpusTest, test_regression_address_bar_dd_to_d) {
    // address bar autocomplete highlight requires extra backspace and uinput routing
    MockInputContext ic(instance_->inputContextManager(), "google-chrome");
    ic.setUrlCapability(true);
    ime::ClakState state(engine_.get(), &ic);

    size_t uinput_bs_sent = 0;
    uinput::UinputTool::instance().setMockHandler([&](size_t count, uint32_t, uint32_t, uint32_t) {
        uinput_bs_sent = count;
        return true;
    });

    ic.setSurrounding("", 0, 0);
    ic.typeChar('d', &state);

    // simulate address bar autocomplete: single line text with selection extending past cursor
    ic.setSurrounding("ddos.com", 1, 8);
    EXPECT_TRUE(state.isAutofillCertain(ic.surroundingText()));

    ic.typeChar('d', &state);

    // expect 3 backspaces: 1 real + 1 autofill clear + 1 sentinel
    EXPECT_EQ(uinput_bs_sent, 3);
    EXPECT_TRUE(state.isDeleting());

    ic.sendCleanBackspace(&state);
    ic.sendCleanBackspace(&state);
    ic.sendCleanBackspace(&state);

    EXPECT_FALSE(state.isDeleting());
    ASSERT_FALSE(ic.commits.empty());
    EXPECT_EQ(ic.commits.back(), "đ");
}

TEST_F(RegressionCorpusTest, test_regression_hyprland_stale_selection_during_retoning) {
    // hyprland/niri temporary mid-word selection must not be detected as address bar autocomplete
    MockInputContext ic(instance_->inputContextManager(), "google-chrome");
    ic.setUrlCapability(true);
    ime::ClakState state(engine_.get(), &ic);

    // cursor and anchor inside "dựng" (length 6) should not trigger autofill
    ic.setSurrounding("dựng", 3, 4);
    EXPECT_FALSE(state.isAutofillCertain(ic.surroundingText()));
    ic.setSurrounding("dựng", 4, 3);
    EXPECT_FALSE(state.isAutofillCertain(ic.surroundingText()));

    // selection not extending to line end is not autocomplete
    ic.setSurrounding("google more", 2, 6);
    EXPECT_FALSE(state.isAutofillCertain(ic.surroundingText()));

    // reverse direction autocomplete extending to end is valid
    ic.setSurrounding("google", 6, 2);
    EXPECT_TRUE(state.isAutofillCertain(ic.surroundingText()));

    // autocomplete extending to end with spaces in query suggestion is valid
    ic.setSurrounding("cach check hang", 2, 15);
    EXPECT_TRUE(state.isAutofillCertain(ic.surroundingText()));
}

TEST_F(RegressionCorpusTest, test_regression_docs_cascading_chars) {
    // google docs must route to uinput to avoid cascading duplicate characters
    MockInputContext ic(instance_->inputContextManager(), "google-chrome");
    ime::ClakState state(engine_.get(), &ic);

    platform::setMockActiveWindow(platform::WindowInfo{
        "google-chrome",
        "Google Docs - Document",
        1234
    });

    ic.setSurrounding("nguoi", 5, 5);
    EXPECT_TRUE(state.shouldUseUinput(true, CLAK_ACTION_REPLACE, ic.surroundingText()));
}

TEST_F(RegressionCorpusTest, test_regression_laf_sentinel_timeout) {
    // when loopback sentinel is lost during standard typing, safety timer recovers fast (~50ms)
    platform::setMockActiveWindow(platform::WindowInfo{"zen", "Zen Browser", 1234});
    MockInputContext ic(instance_->inputContextManager(), "zen");
    ime::ClakState state(engine_.get(), &ic);

    uinput::UinputTool::instance().setMockHandler([](size_t, uint32_t, uint32_t, uint32_t) {
        return true;
    });

    ic.setSurrounding("", 0, 0);
    ic.typeChar('d', &state);
    ic.typeChar('d', &state);

    EXPECT_TRUE(state.isDeleting());
    EXPECT_FALSE(state.isSelectionDeletion());

    auto exit_timer = instance_->eventLoop().addTimeEvent(
        CLOCK_MONOTONIC,
        fcitx::now(CLOCK_MONOTONIC) + 300000,
        0,
        [this](fcitx::EventSourceTime*, uint64_t) {
            instance_->eventLoop().exit();
            return true;
        }
    );
    instance_->eventLoop().exec();

    EXPECT_FALSE(state.isDeleting());
    ASSERT_FALSE(ic.commits.empty());
    EXPECT_EQ(ic.commits.back(), "đ");
}

TEST_F(RegressionCorpusTest, test_regression_rapid_selection_deletion) {
    // rapid selection deletion uses the 250ms timeout scenario to prevent race conditions
    platform::setMockActiveWindow(platform::WindowInfo{"zen", "Zen Browser", 1234});
    MockInputContext ic(instance_->inputContextManager(), "zen");
    ime::ClakState state(engine_.get(), &ic);

    uinput::UinputTool::instance().setMockHandler([](size_t, uint32_t, uint32_t, uint32_t) {
        return true;
    });

    // simulate text selection shortcut (shift + left arrow)
    fcitx::Key shift_left(FcitxKey_Left, fcitx::KeyState::Shift);
    fcitx::KeyEvent key_event(&ic, shift_left, false);
    state.keyEvent(key_event);

    ic.setSurrounding("text", 0, 4);
    uint64_t before_type_us = fcitx::now(CLOCK_MONOTONIC);
    ic.typeChar('d', &state);
    ic.typeChar('d', &state);

    EXPECT_TRUE(state.isDeleting());
    EXPECT_TRUE(state.isSelectionDeletion());
    // verify safety timer was armed with 250ms selection timeout instead of 50ms
    EXPECT_GE(state.safetyTimerTime(), before_type_us + config::kSelectionDeletionTimeoutUs);

    // at 600ms, the 250ms selection timer has fired and recovered state cleanly
    auto exit_timer = instance_->eventLoop().addTimeEvent(
        CLOCK_MONOTONIC,
        fcitx::now(CLOCK_MONOTONIC) + 600000,
        0,
        [this](fcitx::EventSourceTime*, uint64_t) {
            instance_->eventLoop().exit();
            return true;
        }
    );
    instance_->eventLoop().exec();

    EXPECT_FALSE(state.isDeleting());
    ASSERT_FALSE(ic.commits.empty());
    EXPECT_EQ(ic.commits.back(), "đ");
}

TEST_F(RegressionCorpusTest, test_regression_safety_timer_self_reset_crash) {
    // safety timer callback must not call reset() on itself, avoiding sd-event use-after-free crash
    platform::setMockActiveWindow(platform::WindowInfo{"zen", "Zen Browser", 1234});
    MockInputContext ic(instance_->inputContextManager(), "zen");
    ime::ClakState state(engine_.get(), &ic);

    uinput::UinputTool::instance().setMockHandler([](size_t, uint32_t, uint32_t, uint32_t) {
        return true;
    });

    ic.setSurrounding("", 0, 0);
    ic.typeChar('d', &state);
    ic.typeChar('d', &state);

    EXPECT_TRUE(state.isDeleting());

    // run event loop past the 50ms safety timer timeout without throwing EventLoopException
    EXPECT_NO_THROW({
        auto exit_timer = instance_->eventLoop().addTimeEvent(
            CLOCK_MONOTONIC,
            fcitx::now(CLOCK_MONOTONIC) + 300000,
            0,
            [this](fcitx::EventSourceTime*, uint64_t) {
                instance_->eventLoop().exit();
                return true;
            }
        );
        instance_->eventLoop().exec();
    });

    EXPECT_FALSE(state.isDeleting());
    ASSERT_FALSE(ic.commits.empty());
    EXPECT_EQ(ic.commits.back(), "đ");
}

TEST_F(RegressionCorpusTest, test_regression_gecko_stale_surrounding) {
    // gecko apps must always use uinput and never call deleteSurroundingText
    MockInputContext ic(instance_->inputContextManager(), "firefox");
    ime::ClakState state(engine_.get(), &ic);

    EXPECT_TRUE(state.isGecko());
    ic.setSurrounding("o", 1, 1);
    EXPECT_TRUE(state.shouldUseUinput(true, CLAK_ACTION_REPLACE, ic.surroundingText()));
}

TEST_F(RegressionCorpusTest, test_regression_twitter_draftjs_dd) {
    // draftjs rich text editors switch to uinput after 3 surrounding text mismatches
    MockInputContext ic(instance_->inputContextManager(), "google-chrome");
    ic.auto_update_surrounding = true;
    ime::ClakState state(engine_.get(), &ic);

    size_t uinput_calls = 0;
    uinput::UinputTool::instance().setMockHandler([&](size_t, uint32_t, uint32_t, uint32_t) {
        uinput_calls++;
        return true;
    });

    // simulate 3 consecutive stale reports from draftjs
    for (int i = 0; i < 3; ++i) {
        state.reset();
        ic.clearHistory();
        ic.setSurrounding("", 0, 0);
        ic.typeChar('d', &state);
        ic.typeChar('d', &state);
        ic.setSurrounding("stale", 5, 5);
        ic.typeChar(' ', &state);
    }

    EXPECT_TRUE(state.isRichTextEditor());

    // 4th replacement must use uinput
    state.reset();
    ic.clearHistory();
    ic.setSurrounding("", 0, 0);
    ic.typeChar('d', &state);
    ic.typeChar('d', &state);

    EXPECT_GT(uinput_calls, 0);
    EXPECT_TRUE(ic.deletions.empty());
}

TEST_F(RegressionCorpusTest, test_regression_terminal_forward_key_dup) {
    // terminal apps must always use uinput to avoid key event duplication
    MockInputContext ic(instance_->inputContextManager(), "kitty");
    ime::ClakState state(engine_.get(), &ic);

    ic.setSurrounding("d", 1, 1);
    EXPECT_TRUE(state.shouldUseUinput(true, CLAK_ACTION_REPLACE, ic.surroundingText()));
}

TEST_F(RegressionCorpusTest, test_regression_adaptive_wait_scales_and_decays) {
    // adaptive latency wait scales up under heavy lag and decays back down when stable
    MockInputContext ic(instance_->inputContextManager(), "zen");
    ime::ClakState state(engine_.get(), &ic);

    EXPECT_EQ(state.adaptiveExtraWaitUs(), 0);

    // fast roundtrip (15ms) maintains 0 extra wait
    state.observeTransactionLatency(15000);
    EXPECT_EQ(state.adaptiveExtraWaitUs(), 0);

    // laggy roundtrip (40ms) scales up by 10ms
    state.observeTransactionLatency(40000);
    EXPECT_EQ(state.adaptiveExtraWaitUs(), 10000);

    // repeated lag scales up further
    state.observeTransactionLatency(45000);
    EXPECT_EQ(state.adaptiveExtraWaitUs(), 20000);

    // 4 consecutive stable transactions trigger a decay step (-10ms)
    for (int i = 0; i < 4; ++i) {
        state.observeTransactionLatency(12000);
    }
    EXPECT_EQ(state.adaptiveExtraWaitUs(), 10000);

    // 4 more stable transactions decay back to 0
    for (int i = 0; i < 4; ++i) {
        state.observeTransactionLatency(12000);
    }
    EXPECT_EQ(state.adaptiveExtraWaitUs(), 0);
}

TEST_F(RegressionCorpusTest, test_regression_newline_does_not_trigger_rich_text_editor) {
    // an empty line with "\n" and cursor 0 must not switch to rich text / uinput mode
    MockInputContext ic(instance_->inputContextManager(), "google-chrome");
    ime::ClakState state(engine_.get(), &ic);

    ic.setSurrounding("\n", 0, 0);
    ic.typeChar('t', &state);
    EXPECT_FALSE(state.isRichTextEditor());

    // continue typing 'h', 'e', 'e', 's' ("thees" -> "thế")
    ic.setSurrounding("t\n", 1, 1);
    ic.typeChar('h', &state);
    ic.setSurrounding("th\n", 2, 2);
    ic.typeChar('e', &state);

    // 2nd 'e' turns "the" into "thê"
    ic.setSurrounding("the\n", 3, 3);
    ic.typeChar('e', &state);
    EXPECT_FALSE(state.isRichTextEditor());
    EXPECT_FALSE(ic.deletions.empty());
    EXPECT_EQ(ic.deletions.back().first, -1);
    EXPECT_EQ(ic.deletions.back().second, 1);
    ASSERT_FALSE(ic.commits.empty());
    EXPECT_EQ(ic.commits.back(), "ê");

    // 's' turns "thê" into "thế"
    ic.setSurrounding("thê\n", 3, 3);
    ic.typeChar('s', &state);
    EXPECT_FALSE(state.isRichTextEditor());
    EXPECT_EQ(ic.deletions.back().first, -1);
    EXPECT_EQ(ic.deletions.back().second, 1);
    EXPECT_EQ(ic.commits.back(), "ế");
}

TEST_F(RegressionCorpusTest, test_regression_twitter_draftjs_inss) {
    // typing "instantly" on twitter: i-n-s -> "ín", next 's' cancels tone to "ins", not swallowed
    platform::setMockActiveWindow(platform::WindowInfo{"google-chrome", "Home / X", 1234});
    MockInputContext ic(instance_->inputContextManager(), "google-chrome");
    auto* state = ic.propertyFor(&engine_->factory());

    fcitx::InputMethodEntry entry("clak", "Vietnamese", "vi", "clak");
    fcitx::InputContextEvent deact_event(&ic, fcitx::EventType::InputContextInputMethodDeactivated);
    fcitx::InputContextEvent act_event(&ic, fcitx::EventType::InputContextInputMethodActivated);

    size_t uinput_calls = 0;
    size_t uinput_bs_sent = 0;
    uinput::UinputTool::instance().setMockHandler([&](size_t count, uint32_t, uint32_t, uint32_t) {
        uinput_calls++;
        uinput_bs_sent = count;
        return true;
    });

    ic.setSurrounding("", 0, 0);
    ic.typeChar('i', state);
    // chromium ozone wayland text-input pulses deactivate/activate on dom mutation
    engine_->deactivate(entry, deact_event);
    engine_->activate(entry, act_event);

    ic.typeChar('n', state);
    engine_->deactivate(entry, deact_event);
    engine_->activate(entry, act_event);

    ic.typeChar('s', state);

    // expect 3 backspaces (2 to delete "in" + 1 sentinel) and state->isDeleting() == true
    EXPECT_EQ(uinput_calls, 1);
    EXPECT_EQ(uinput_bs_sent, 3);
    EXPECT_TRUE(state->isDeleting());

    // 2 deletion backspaces arrive
    ic.sendCleanBackspace(state);
    ic.sendCleanBackspace(state);

    // simulate chromium deactivate/activate during in-flight deletion
    engine_->deactivate(entry, deact_event);
    engine_->activate(entry, act_event);
    // all real backspaces arrived so activate completes deletion early without waiting 150ms timeout
    EXPECT_FALSE(state->isDeleting());
    ASSERT_FALSE(ic.commits.empty());
    EXPECT_EQ(ic.commits.back(), "ín");

    // late sentinel backspace is safely absorbed by grace window without deleting committed character
    ic.sendCleanBackspace(state);
    EXPECT_FALSE(state->isDeleting());
    EXPECT_EQ(ic.commits.back(), "ín");

    // simulate post-commit deactivate/activate pulse
    engine_->deactivate(entry, deact_event);
    engine_->activate(entry, act_event);

    // user types 2nd 's' to revert tone to "ins" (deletes "ín", 2 chars + 1 sentinel = 3 bs)
    ic.typeChar('s', state);
    EXPECT_EQ(uinput_calls, 2);
    EXPECT_EQ(uinput_bs_sent, 3);
    EXPECT_TRUE(state->isDeleting());

    ic.sendCleanBackspace(state);
    ic.sendCleanBackspace(state);
    ic.sendCleanBackspace(state);
    EXPECT_FALSE(state->isDeleting());
    EXPECT_EQ(ic.commits.back(), "ins");

    // user continues typing "tantly" to complete "instantly"
    ic.typeChar('t', state);
    engine_->deactivate(entry, deact_event);
    engine_->activate(entry, act_event);

    ic.typeChar('a', state);
    ic.typeChar('n', state);
    ic.typeChar('t', state);
    ic.typeChar('l', state);
    ic.typeChar('y', state);
    EXPECT_FALSE(state->isDeleting());
}

TEST_F(RegressionCorpusTest, test_regression_twitter_draftjs_is_n_to_in) {
    // typing "i" then "s" -> "í", then "n" -> "ín", then "s" -> reverts to "ins"
    platform::setMockActiveWindow(platform::WindowInfo{"google-chrome", "Home / X", 1234});
    MockInputContext ic(instance_->inputContextManager(), "google-chrome");
    auto* state = ic.propertyFor(&engine_->factory());

    fcitx::InputMethodEntry entry("clak", "Vietnamese", "vi", "clak");
    fcitx::InputContextEvent deact_event(&ic, fcitx::EventType::InputContextInputMethodDeactivated);
    fcitx::InputContextEvent act_event(&ic, fcitx::EventType::InputContextInputMethodActivated);

    size_t uinput_calls = 0;
    size_t uinput_bs_sent = 0;
    uinput::UinputTool::instance().setMockHandler([&](size_t count, uint32_t, uint32_t, uint32_t) {
        uinput_calls++;
        uinput_bs_sent = count;
        return true;
    });

    ic.setSurrounding("", 0, 0);
    ic.typeChar('i', state);
    engine_->deactivate(entry, deact_event);
    engine_->activate(entry, act_event);

    ic.typeChar('s', state);
    // expect uinput replacement of "i" with "í"
    EXPECT_EQ(uinput_calls, 1);
    EXPECT_EQ(uinput_bs_sent, 2);
    EXPECT_TRUE(state->isDeleting());

    ic.sendCleanBackspace(state);
    ic.sendCleanBackspace(state);
    EXPECT_FALSE(state->isDeleting());
    ASSERT_FALSE(ic.commits.empty());
    EXPECT_EQ(ic.commits.back(), "í");

    engine_->deactivate(entry, deact_event);
    engine_->activate(entry, act_event);

    // type "n" -> forwards 'n' directly to app (unfiltered), resulting in "ín"
    EXPECT_FALSE(ic.typeChar('n', state));
    EXPECT_EQ(uinput_calls, 1);
    EXPECT_FALSE(state->isDeleting());

    engine_->deactivate(entry, deact_event);
    engine_->activate(entry, act_event);

    // type "s" again -> reverts tone to "ins" (deletes "ín", 2 chars + 1 sentinel = 3 bs)
    ic.typeChar('s', state);
    EXPECT_EQ(uinput_calls, 2);
    EXPECT_EQ(uinput_bs_sent, 3);
    EXPECT_TRUE(state->isDeleting());

    ic.sendCleanBackspace(state);
    ic.sendCleanBackspace(state);
    ic.sendCleanBackspace(state);
    EXPECT_FALSE(state->isDeleting());
    EXPECT_EQ(ic.commits.back(), "ins");
}

TEST_F(RegressionCorpusTest, test_regression_twitter_delete_all_and_retype) {
    // deleting all text in twitter and typing again resets composition cleanly
    platform::setMockActiveWindow(platform::WindowInfo{"google-chrome", "Home / X", 1234});
    MockInputContext ic(instance_->inputContextManager(), "google-chrome");
    auto* state = ic.propertyFor(&engine_->factory());

    fcitx::InputMethodEntry entry("clak", "Vietnamese", "vi", "clak");
    fcitx::InputContextEvent deact_event(&ic, fcitx::EventType::InputContextInputMethodDeactivated);
    fcitx::InputContextEvent act_event(&ic, fcitx::EventType::InputContextInputMethodActivated);

    size_t uinput_calls = 0;
    uinput::UinputTool::instance().setMockHandler([&](size_t, uint32_t, uint32_t, uint32_t) {
        uinput_calls++;
        return true;
    });

    // type 'i'
    ic.setSurrounding("\n", 0, 0);
    ic.typeChar('i', state);
    engine_->deactivate(entry, deact_event);
    engine_->activate(entry, act_event);

    // type 'n'
    ic.setSurrounding("i\n", 1, 1);
    ic.typeChar('n', state);
    engine_->deactivate(entry, deact_event);
    engine_->activate(entry, act_event);

    // type 's' -> triggers replace to "ín"
    ic.setSurrounding("in\n", 2, 2);
    ic.typeChar('s', state);
    EXPECT_EQ(uinput_calls, 1);
    EXPECT_TRUE(state->isDeleting());

    ic.sendCleanBackspace(state);
    ic.sendCleanBackspace(state);
    ic.sendCleanBackspace(state);
    EXPECT_FALSE(state->isDeleting());
    EXPECT_EQ(ic.commits.back(), "ín");

    engine_->deactivate(entry, deact_event);
    engine_->activate(entry, act_event);

    // simulate user clearing all text on twitter (dom cleared to empty line)
    ic.setSurrounding("\n", 0, 0);

    // user types 'i' again -> must forward cleanly with fresh buffer
    EXPECT_FALSE(ic.typeChar('i', state));
    engine_->deactivate(entry, deact_event);
    engine_->activate(entry, act_event);

    // user types 'n'
    ic.setSurrounding("i\n", 1, 1);
    EXPECT_FALSE(ic.typeChar('n', state));
    engine_->deactivate(entry, deact_event);
    engine_->activate(entry, act_event);

    // user types 's' -> must produce "ín", not swallowed or forwarded
    ic.setSurrounding("in\n", 2, 2);
    ic.typeChar('s', state);
    EXPECT_EQ(uinput_calls, 2);
    EXPECT_TRUE(state->isDeleting());

    ic.sendCleanBackspace(state);
    ic.sendCleanBackspace(state);
    ic.sendCleanBackspace(state);
    EXPECT_FALSE(state->isDeleting());
    EXPECT_EQ(ic.commits.back(), "ín");
}

TEST_F(RegressionCorpusTest, test_regression_unknown_site_draftjs_auto_detect) {
    // unknown domain not in hardcoded list
    platform::setMockActiveWindow(platform::WindowInfo{"google-chrome", "Some React App - random-site.org", 1234});
    MockInputContext ic(instance_->inputContextManager(), "google-chrome");
    auto* state = ic.propertyFor(&engine_->factory());

    size_t uinput_calls = 0;
    uint32_t last_post_delay = 0;
    uint32_t last_gap_ms = 0;
    uinput::UinputTool::instance().setMockHandler([&](size_t, uint32_t post_delay, uint32_t, uint32_t gap) {
        uinput_calls++;
        last_post_delay = post_delay;
        last_gap_ms = gap;
        return true;
    });

    // initially not draftjs
    EXPECT_FALSE(state->isDraftJsEditor());

    // type 'd' then 'd'
    ic.setSurrounding("", 0, 0);
    ic.typeChar('d', state);

    // 2nd 'd' tries surrounding delete
    ic.setSurrounding("d", 1, 1);
    ic.typeChar('d', state);
    EXPECT_FALSE(ic.deletions.empty());

    // react dom ignored the delete, so surrounding text became "dđ" instead of "đ"
    ic.setSurrounding("dđ", 2, 2);
    // next keystroke verifies surrounding and detects ignored delete
    ic.typeChar('d', state);

    // verify that draftjs was immediately auto-detected
    EXPECT_TRUE(state->isDraftJsEditor());
    EXPECT_TRUE(state->isRichTextEditor());

    // next deletion must use uinput with draftjs pacing (kDraftJsPostDelayMs post_delay, kDraftJsGapMs gap)
    EXPECT_GT(uinput_calls, 0);
    EXPECT_EQ(last_post_delay, config::kDraftJsPostDelayMs);
    EXPECT_EQ(last_gap_ms, config::kDraftJsGapMs);
}

TEST_F(RegressionCorpusTest, test_regression_backspace_hold_continues_deleting_when_composing_empties) {
    platform::setMockActiveWindow(platform::WindowInfo{"gedit", "Untitled Document", 4567});
    MockInputContext ic(instance_->inputContextManager(), "gedit");
    auto* state = ic.propertyFor(&engine_->factory());

    // type "tieng"
    ic.typeString("tieng", state);

    // initial backspace down press while composing (deletes 'g')
    bool filtered = ic.sendKey(FcitxKey_BackSpace, fcitx::KeyStates(), false, state);
    EXPECT_FALSE(filtered);

    // key-repeats delete 'n', 'e', 'i'
    for (int i = 0; i < 3; ++i) {
        bool rep_filtered = ic.sendKey(FcitxKey_BackSpace, fcitx::KeyStates(), false, state);
        EXPECT_FALSE(rep_filtered);
    }

    // next repeat deletes final character 't' and empties composing buffer
    bool empty_filtered = ic.sendKey(FcitxKey_BackSpace, fcitx::KeyStates(), false, state);
    EXPECT_FALSE(empty_filtered);

    // subsequent key-repeats while still held down must continue to forward freely
    bool repeat_filtered1 = ic.sendKey(FcitxKey_BackSpace, fcitx::KeyStates(), false, state);
    EXPECT_FALSE(repeat_filtered1);
    bool repeat_filtered2 = ic.sendKey(FcitxKey_BackSpace, fcitx::KeyStates(), false, state);
    EXPECT_FALSE(repeat_filtered2);

    // key release
    ic.sendKey(FcitxKey_BackSpace, fcitx::KeyStates(), true, state);

    // subsequent new backspace press is forwarded normally
    bool new_press_filtered = ic.sendKey(FcitxKey_BackSpace, fcitx::KeyStates(), false, state);
    EXPECT_FALSE(new_press_filtered);
}

TEST_F(RegressionCorpusTest, test_regression_backspace_hold_does_not_suppress_non_composing_text) {
    platform::setMockActiveWindow(platform::WindowInfo{"gedit", "Untitled Document", 4567});
    MockInputContext ic(instance_->inputContextManager(), "gedit");
    auto* state = ic.propertyFor(&engine_->factory());

    state->reset(true);

    // hold backspace without composing
    bool filtered1 = ic.sendKey(FcitxKey_BackSpace, fcitx::KeyStates(), false, state);
    EXPECT_FALSE(filtered1);

    // repeat events must not be suppressed
    for (int i = 0; i < 5; ++i) {
        bool repeat_filtered = ic.sendKey(FcitxKey_BackSpace, fcitx::KeyStates(), false, state);
        EXPECT_FALSE(repeat_filtered);
    }
}

TEST_F(RegressionCorpusTest, test_regression_zero_cursor_surrounding_never_crashes_compositor) {
    // When an editor (e.g. Twitter/DraftJS on GNOME Wayland) reports cursor 0 in surrounding text
    // while replacing 'dd' -> 'đ', calling deleteSurroundingText(-1, 1) violates Wayland protocol
    // and causes Mutter (GNOME Shell) to crash with SIGABRT (assertion before >= start failed).
    // Clak must detect cursor < real_bs, never invoke deleteSurroundingText, and route safely to uinput.
    MockInputContext ic(instance_->inputContextManager(), "google-chrome");
    auto* state = ic.propertyFor(&engine_->factory());

    size_t uinput_calls = 0;
    size_t uinput_bs_sent = 0;
    uinput::UinputTool::instance().setMockHandler([&](size_t count, uint32_t, uint32_t, uint32_t) {
        uinput_calls++;
        uinput_bs_sent = count;
        return true;
    });

    // Twitter Tweet composer on Wayland reports newline with cursor 0
    ic.setSurrounding("\n", 0, 0);
    ic.typeChar('d', state);

    // DraftJS React DOM re-renders without updating surrounding text cursor (cursor remains 0)
    ic.setSurrounding("\n", 0, 0);
    ic.typeChar('d', state);

    // Must NEVER call deleteSurroundingText (which crashes Mutter)
    EXPECT_TRUE(ic.deletions.empty());

    // Must route to uinput with 1 deletion + 1 sentinel = 2 backspaces
    EXPECT_EQ(uinput_calls, 1);
    EXPECT_EQ(uinput_bs_sent, 2);
    EXPECT_TRUE(state->isDeleting());

    // Must automatically flag editor as DraftJS/RichText
    EXPECT_TRUE(state->isDraftJsEditor());
    EXPECT_TRUE(state->isRichTextEditor());

    // Send backspaces to finish deletion and commit 'đ'
    ic.sendCleanBackspace(state);
    ic.sendCleanBackspace(state);

    EXPECT_FALSE(state->isDeleting());
    ASSERT_FALSE(ic.commits.empty());
    EXPECT_EQ(ic.commits.back(), "đ");
}

TEST_F(RegressionCorpusTest, test_regression_twitter_draftjs_newline_proactive_detection) {
    // Twitter/DraftJS composer often exposes surrounding text ending with double newlines "\n\n"
    // or placeholder text. Clak must proactively identify it as DraftJS/RichText and route to uinput.
    MockInputContext ic(instance_->inputContextManager(), "google-chrome");
    auto* state = ic.propertyFor(&engine_->factory());

    size_t uinput_calls = 0;
    uinput::UinputTool::instance().setMockHandler([&](size_t, uint32_t, uint32_t, uint32_t) {
        uinput_calls++;
        return true;
    });

    ic.setSurrounding("What is happening?!\n\n", 21, 21);
    ic.typeChar('d', state);
    ic.typeChar('d', state);

    // Must NEVER call deleteSurroundingText
    EXPECT_TRUE(ic.deletions.empty());
    // Must use uinput
    EXPECT_GT(uinput_calls, 0);
    EXPECT_TRUE(state->isDraftJsEditor());
    EXPECT_TRUE(state->isRichTextEditor());

    ic.sendCleanBackspace(state);
    ic.sendCleanBackspace(state);
    EXPECT_FALSE(state->isDeleting());
    ASSERT_FALSE(ic.commits.empty());
    EXPECT_EQ(ic.commits.back(), "đ");
}

TEST_F(RegressionCorpusTest, test_regression_twitter_space_does_not_clear_draftjs_mode) {
    // When typing consecutive words in Twitter composer separated by space,
    // the DraftJS editor detection must persist and NOT be cleared by space or word reset.
    platform::setMockActiveWindow(platform::WindowInfo{"google-chrome", "Home / X", 12345});
    MockInputContext ic(instance_->inputContextManager(), "google-chrome");
    auto* state = ic.propertyFor(&engine_->factory());

    size_t uinput_calls = 0;
    uinput::UinputTool::instance().setMockHandler([&](size_t, uint32_t, uint32_t, uint32_t) {
        uinput_calls++;
        return true;
    });

    // 1st word: "dd" -> "đ" in DraftJS
    ic.setSurrounding("\n\n", 2, 2);
    ic.typeChar('d', state);
    ic.typeChar('d', state);
    EXPECT_TRUE(state->isDraftJsEditor());
    ic.sendCleanBackspace(state);
    ic.sendCleanBackspace(state);
    EXPECT_EQ(ic.commits.back(), "đ");

    // Type space
    ic.typeChar(' ', state);
    // DraftJS mode MUST NOT be cleared by typing space!
    EXPECT_TRUE(state->isDraftJsEditor());

    // 2nd word: "dd" -> "đ"
    size_t prev_uinput = uinput_calls;
    ic.typeChar('d', state);
    ic.typeChar('d', state);

    // 2nd word must also route to uinput and never call deleteSurroundingText
    EXPECT_TRUE(ic.deletions.empty());
    EXPECT_GT(uinput_calls, prev_uinput);

    ic.sendCleanBackspace(state);
    ic.sendCleanBackspace(state);
    EXPECT_EQ(ic.commits.back(), "đ");
}

TEST_F(RegressionCorpusTest, test_regression_terminal_kitty_tone_undo_ass_to_as) {
    // In terminal emulators (e.g. Kitty running zsh), surrounding text is ignored or unsupported by PTY.
    // Typing 'a' + 's' -> 'á', then typing 's' again must undo tone to 'as' using uinput backspace,
    // never deleteSurroundingText (which causes 'aá' or 'aáas').
    platform::setMockActiveWindow(platform::WindowInfo{"kitty", "kitty", 1234});
    MockInputContext ic(instance_->inputContextManager(), "kitty");
    auto* state = ic.propertyFor(&engine_->factory());

    size_t uinput_calls = 0;
    size_t uinput_bs_sent = 0;
    uinput::UinputTool::instance().setMockHandler([&](size_t count, uint32_t, uint32_t, uint32_t) {
        uinput_calls++;
        uinput_bs_sent += count;
        return true;
    });

    // 1. Type 'a' -> forward key (del=0)
    ic.typeChar('a', state);
    EXPECT_TRUE(ic.commits.empty());
    EXPECT_TRUE(ic.deletions.empty());
    EXPECT_EQ(uinput_calls, 0);

    // 2. Type 's' -> replace 'a' with 'á' via uinput (1 bs + 1 sentinel = 2 bs)
    ic.typeChar('s', state);
    EXPECT_TRUE(ic.deletions.empty());
    EXPECT_EQ(uinput_calls, 1);
    EXPECT_EQ(uinput_bs_sent, 2);
    EXPECT_TRUE(state->isDeleting());

    ic.sendCleanBackspace(state);
    ic.sendCleanBackspace(state);
    EXPECT_FALSE(state->isDeleting());
    ASSERT_FALSE(ic.commits.empty());
    EXPECT_EQ(ic.commits.back(), "á");

    // 3. Type 's' -> tone undo: replace 'á' with 'as' via uinput (1 bs + 1 sentinel = 2 bs)
    ic.typeChar('s', state);
    EXPECT_TRUE(ic.deletions.empty());
    EXPECT_EQ(uinput_calls, 2);
    EXPECT_EQ(uinput_bs_sent, 4);
    EXPECT_TRUE(state->isDeleting());

    ic.sendCleanBackspace(state);
    ic.sendCleanBackspace(state);
    EXPECT_FALSE(state->isDeleting());
    ASSERT_FALSE(ic.commits.empty());
    EXPECT_EQ(ic.commits.back(), "as");
}

TEST_F(RegressionCorpusTest, test_regression_antigravity_vscode_uses_native_surrounding_text_not_uinput) {
    // Antigravity / VSCode is an Electron application. It natively and reliably supports
    // Wayland deleteSurroundingText. It must NOT be treated as a Linux terminal,
    // avoiding synthetic uinput backspaces that race with Electron's DOM and eat characters.
    platform::setMockActiveWindow(platform::WindowInfo{"antigravity", "antigravity", 1234});
    MockInputContext ic(instance_->inputContextManager(), "antigravity");
    auto* state = ic.propertyFor(&engine_->factory());

    size_t uinput_calls = 0;
    uinput::UinputTool::instance().setMockHandler([&](size_t, uint32_t, uint32_t, uint32_t) {
        uinput_calls++;
        return true;
    });

    ic.setSurrounding("", 0, 0);
    ic.typeChar('a', state);

    ic.setSurrounding("a", 1, 1);
    ic.typeChar('s', state);

    // Must use native deleteSurroundingText, NOT uinput!
    EXPECT_EQ(uinput_calls, 0);
    EXPECT_FALSE(ic.deletions.empty());
    EXPECT_EQ(ic.deletions.back().first, -1);
    EXPECT_EQ(ic.deletions.back().second, 1);
    ASSERT_FALSE(ic.commits.empty());
    EXPECT_EQ(ic.commits.back(), "á");
}

TEST_F(RegressionCorpusTest, test_regression_antigravity_double_newline_chat_never_routes_to_uinput) {
    // In Antigravity / Electron chat prompt, surrounding text ends with "\n\n".
    // Clak must NEVER classify Antigravity as DraftJS or route to Uinput on double newline.
    platform::setMockActiveWindow(platform::WindowInfo{"antigravity", "Antigravity", 1234});
    MockInputContext ic(instance_->inputContextManager(), "antigravity");
    auto* state = ic.propertyFor(&engine_->factory());

    size_t uinput_calls = 0;
    uinput::UinputTool::instance().setMockHandler([&](size_t, uint32_t, uint32_t, uint32_t) {
        uinput_calls++;
        return true;
    });

    ic.setSurrounding("\n\n", 0, 0);
    ic.typeChar('a', state);

    ic.setSurrounding("a\n\n", 1, 1);
    ic.typeChar('s', state);

    EXPECT_FALSE(state->isDraftJsEditor());
    EXPECT_FALSE(state->isRichTextEditor());
    EXPECT_EQ(uinput_calls, 0);
    EXPECT_FALSE(ic.deletions.empty());
    EXPECT_EQ(ic.deletions.back().first, -1);
    EXPECT_EQ(ic.deletions.back().second, 1);
    ASSERT_FALSE(ic.commits.empty());
    EXPECT_EQ(ic.commits.back(), "á");
}

TEST_F(RegressionCorpusTest, test_regression_brave_fast_typing_stale_surrounding_never_falls_back_to_uinput) {
    // When typing fast in Brave on normal web pages, surrounding text may lag behind (actualWord == preWord).
    // SurroundingVerifier must recognize this as normal IPC latency and NOT increment mismatch count
    // or auto-switch to Uinput.
    platform::setMockActiveWindow(platform::WindowInfo{"brave", "Google - Brave", 5678});
    MockInputContext ic(instance_->inputContextManager(), "brave");
    auto* state = ic.propertyFor(&engine_->factory());

    size_t uinput_calls = 0;
    uinput::UinputTool::instance().setMockHandler([&](size_t, uint32_t, uint32_t, uint32_t) {
        uinput_calls++;
        return true;
    });

    for (int i = 0; i < 5; ++i) {
        state->reset();
        ic.clearHistory();

        // Type 'a'
        ic.setSurrounding("hello ", 6, 6);
        ic.typeChar('a', state);

        // Type 's' -> replaces 'a' with 'á'
        ic.setSurrounding("hello a", 7, 7);
        ic.typeChar('s', state);

        // Next keystroke 't' arrives while surrounding text is still lagging at "hello a" (preWord)
        ic.setSurrounding("hello a", 7, 7);
        ic.typeChar('t', state);

        // Must NOT switch to uinput!
        EXPECT_FALSE(state->isDraftJsEditor());
        EXPECT_FALSE(state->isRichTextEditor());
        EXPECT_EQ(uinput_calls, 0);
    }
}

} // namespace test
} // namespace clak




