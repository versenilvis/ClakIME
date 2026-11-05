#include <gtest/gtest.h>
#include <fcitx/instance.h>
#include <fcitx-utils/event.h>
#include "mock_input_context.h"
#include "ime/state.h"
#include "engine.h"
#include "core.h"
#include "uinput/uinput.h"
#include "window/window_info.h"
#include "apps/modal_editor.h"
#include "config/sites.h"
#include "config/config.h"
#include "utils/text_utils.h"

namespace clak {
namespace test {

class ClakStateTest : public ::testing::Test {
protected:
    void SetUp() override {
        int argc = 1;
        char arg0[] = "clak_test";
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

TEST_F(ClakStateTest, GivenGeckoApp_AlwaysRoutesToUinput) {
    // gecko apps like zen must always route deletions to uinput
    platform::setMockActiveWindow(platform::WindowInfo{"zen", "Zen Browser", 1234});
    MockInputContext ic(instance_->inputContextManager(), "zen");
    ime::ClakState state(engine_.get(), &ic);

    EXPECT_TRUE(state.isGecko());

    size_t uinput_bs_sent = 0;
    uinput::UinputTool::instance().setMockHandler([&](size_t count, uint32_t, uint32_t, uint32_t) {
        uinput_bs_sent = count;
        return true;
    });

    ic.setSurrounding("", 0, 0);
    ic.typeChar('d', &state);
    ic.typeChar('d', &state);

    // expect 2 backspaces: 1 real + 1 sentinel loopback
    EXPECT_EQ(uinput_bs_sent, 2);
    EXPECT_TRUE(ic.deletions.empty());
    EXPECT_TRUE(state.isDeleting());

    // feed 2 sentinel backspaces to complete the uinput deletion
    ic.sendCleanBackspace(&state);
    ic.sendCleanBackspace(&state);

    EXPECT_FALSE(state.isDeleting());
    ASSERT_FALSE(ic.commits.empty());
    EXPECT_EQ(ic.commits.back(), "đ");
}

TEST_F(ClakStateTest, GivenStaleSurroundingText3TimesInRow_FallsBackToUinput) {
    // rich text editors with stale surrounding text must switch to uinput after 3 mismatches
    MockInputContext ic(instance_->inputContextManager(), "google-chrome");
    ime::ClakState state(engine_.get(), &ic);

    EXPECT_FALSE(state.isGecko());
    EXPECT_FALSE(state.isRichTextEditor());
    EXPECT_EQ(state.mismatchCount(), 0);

    size_t uinput_calls = 0;
    uinput::UinputTool::instance().setMockHandler([&](size_t, uint32_t, uint32_t, uint32_t) {
        uinput_calls++;
        return true;
    });

    ic.auto_update_surrounding = true;

    for (int i = 0; i < 3; ++i) {
        state.reset();
        ic.clearHistory();
        ic.setSurrounding("", 0, 0);
        ic.typeChar('d', &state);
        ic.typeChar('d', &state);
        // editor provides stale text instead of composed đ
        ic.setSurrounding("stale", 5, 5);
        ic.typeChar(' ', &state);
        EXPECT_EQ(state.mismatchCount(), i + 1);
    }

    EXPECT_TRUE(state.isRichTextEditor());

    // 4th replacement should automatically use uinput instead of surrounding text
    state.reset();
    ic.clearHistory();
    ic.setSurrounding("", 0, 0);
    ic.typeChar('d', &state);
    ic.typeChar('d', &state);

    EXPECT_GT(uinput_calls, 0);
    EXPECT_TRUE(ic.deletions.empty());
}

TEST_F(ClakStateTest, GivenSentinelNeverArrives_SafetyTimerFires_RecoversAndCommits) {
    // lost sentinel backspace should recover via safety timer without deadlock
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
    EXPECT_EQ(state.pendingCommitString(), "đ");

    // run event loop on the same thread with an exit timer after safety timer
    auto exit_timer = instance_->eventLoop().addTimeEvent(
        CLOCK_MONOTONIC,
        fcitx::now(CLOCK_MONOTONIC) + config::kSafetyTimeoutUs + 100000,
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

TEST_F(ClakStateTest, GivenKeysBufferedDuringDelete_ReplaysInOriginalOrder) {
    // keystrokes typed while uinput is deleting must buffer and replay in order
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

    // type while deleting is in progress
    ic.typeChar(' ', &state);
    ic.typeChar('b', &state);
    ic.typeChar('a', &state);

    EXPECT_EQ(state.bufferedKeysCount(), 3);
    EXPECT_TRUE(ic.commits.empty());

    // sentinels arrive, completing deletion and triggering replay
    ic.sendCleanBackspace(&state);
    ic.sendCleanBackspace(&state);

    EXPECT_FALSE(state.isDeleting());
    // Pending commit and buffered forward keys are batched into a single commit to prevent Wayland overwrites
    ASSERT_EQ(ic.commits.size(), 1);
    EXPECT_EQ(ic.commits[0], "đ ba");
}

TEST_F(ClakStateTest, GivenTieengs_WhenKeysArriveDuringUinputDeletion) {
    platform::setMockActiveWindow(platform::WindowInfo{"kitty", "kitty", 1234});
    MockInputContext ic(instance_->inputContextManager(), "kitty");
    ime::ClakState state(engine_.get(), &ic);
    ic.setSurroundingEnabled(false);

    std::vector<std::string> event_order;
    uinput::UinputTool::instance().setMockHandler([&](size_t count, uint32_t, uint32_t, uint32_t) {
        event_order.push_back("uinput_bs_" + std::to_string(count));
        return true;
    });
    ic.on_commit = [&](const std::string& text) {
        event_order.push_back("commit_" + text);
    };

    ic.typeChar('t', &state);
    ic.typeChar('i', &state);
    ic.typeChar('e', &state);
    ic.typeChar('e', &state);

    EXPECT_TRUE(state.isDeleting());
    EXPECT_EQ(event_order.size(), 1u);
    EXPECT_EQ(event_order[0], "uinput_bs_2");

    ic.typeChar('n', &state);
    ic.typeChar('g', &state);
    ic.typeChar('s', &state);

    EXPECT_EQ(state.bufferedKeysCount(), 3u);

    ic.sendCleanBackspace(&state);
    ic.sendCleanBackspace(&state);

    // in-memory absorption: deletions absorbed without sending new backspaces over uinput
    ASSERT_EQ(event_order.size(), 2u);
    EXPECT_EQ(event_order[1], "commit_ếng");
    EXPECT_FALSE(state.isDeleting());
}

TEST_F(ClakStateTest, GivenStaleSurroundingTextAfterReplacement_DoesNotResetOnToneKey) {
    platform::setMockActiveWindow(platform::WindowInfo{"google-chrome", "Chrome", 1234});
    MockInputContext ic(instance_->inputContextManager(), "google-chrome");
    ime::ClakState state(engine_.get(), &ic);

    ic.setSurrounding("te", 2, 2);
    ic.typeChar('t', &state);
    ic.typeChar('e', &state);
    ic.typeChar('e', &state);

    ASSERT_FALSE(ic.commits.empty());
    EXPECT_EQ(ic.commits.back(), "ê");

    // stale surrounding text simulation
    ic.setSurrounding("te", 2, 2);
    ic.clearHistory();
    ic.typeChar('s', &state);

    EXPECT_FALSE(ic.deletions.empty());
    ASSERT_FALSE(ic.commits.empty());
    EXPECT_EQ(ic.commits[0], "ế");
}

TEST_F(ClakStateTest, GivenStaleSurroundingTextLaggingTwoKeys_DoesNotResetOnTieengs) {
    platform::setMockActiveWindow(platform::WindowInfo{"google-chrome", "Chrome", 1234});
    MockInputContext ic(instance_->inputContextManager(), "google-chrome");
    ime::ClakState state(engine_.get(), &ic);

    ic.setSurrounding("tie", 3, 3);
    ic.typeChar('t', &state);
    ic.typeChar('i', &state);
    ic.typeChar('e', &state);
    ic.typeChar('e', &state);

    // stale surrounding text simulation
    ic.setSurrounding("tie", 3, 3);
    ic.typeChar('n', &state);
    ic.setSurrounding("tie", 3, 3);
    ic.typeChar('g', &state);
    ic.setSurrounding("tie", 3, 3);
    ic.clearHistory();
    ic.typeChar('s', &state);

    ASSERT_FALSE(ic.commits.empty());
    EXPECT_NE(ic.commits[0], "s");
}

TEST_F(ClakStateTest, GivenChromiumNormalPage_UsesSurroundingText) {
    // normal browser page with valid surrounding text should use surrounding text deletion
    MockInputContext ic(instance_->inputContextManager(), "google-chrome");
    ic.auto_update_surrounding = true;
    ime::ClakState state(engine_.get(), &ic);

    ic.setSurrounding("", 0, 0);
    ic.typeChar('d', &state);
    ic.typeChar('d', &state);

    EXPECT_FALSE(ic.deletions.empty());
    EXPECT_EQ(ic.deletions[0].first, -1);
    EXPECT_EQ(ic.deletions[0].second, 1);
    ASSERT_FALSE(ic.commits.empty());
    EXPECT_EQ(ic.commits[0], "đ");
}

TEST_F(ClakStateTest, GivenGnomeFallbackApp_DetectsGuiEditorWithoutCompositorPid) {
    // gui editors like neovide or gvim on gnome should be detected even without window pid
    platform::setMockActiveWindow(platform::WindowInfo{"", "", 0});
    platform::WindowInfo win = platform::getActiveWindow("neovide");
    EXPECT_EQ(win.win_class, "neovide");
    EXPECT_EQ(win.pid, 0);
    EXPECT_TRUE(platform::isEditorActive(win));
}

TEST_F(ClakStateTest, GivenGnomeFallbackApp_PopulatesTerminalClass) {
    // fallback app should populate terminal class when hyprland socket is absent
    platform::setMockActiveWindow(platform::WindowInfo{"", "", 0});
    platform::WindowInfo win = platform::getActiveWindow("gnome-terminal-server");
    EXPECT_EQ(win.win_class, "gnome-terminal-server");
}

TEST_F(ClakStateTest, GivenX11TemporaryContextFallback_DoesNotAcceptTemporaryName) {
    // temporary context names like ctx_... on x11 must not be treated as real app class
    platform::clearWindowCache();
    platform::clearMockActiveWindow();
    platform::WindowInfo win = platform::getActiveWindow("ctx_12345");
    EXPECT_NE(win.win_class, "ctx_12345");
}

TEST_F(ClakStateTest, GivenAppModeRules_AppliesCorrectProfile) {
    // app mode 1 (uinput) must force uinput
    auto p1 = apps::createAppProfile("myapp", "", false, false, false, 1);
    EXPECT_TRUE(p1->forcesUinput());

    // app mode 2 (surrounding) must allow surrounding and prevent uinput
    auto p2 = apps::createAppProfile("myapp", "", false, false, false, 2);
    EXPECT_TRUE(p2->allowsSurroundingText());
    EXPECT_TRUE(p2->preventsUinput());

    // app mode 0 (auto) uses standard default profile
    auto p0 = apps::createAppProfile("myapp", "", false, false, false, 0);
    EXPECT_FALSE(p0->forcesUinput());
}

TEST_F(ClakStateTest, GivenXTerminalEmul_RecognizedAsTerminalAndEditorWhenTitleMatches) {
    EXPECT_TRUE(config::isTerminalApp("x-terminal-emul"));
    EXPECT_TRUE(config::isTerminalApp("x-terminal-emulator"));
    EXPECT_TRUE(apps::isVteTerminalApp("gnome-terminal"));
    EXPECT_TRUE(apps::isVteTerminalApp("gnome-terminal-server"));
    EXPECT_TRUE(apps::isVteTerminalApp("x-terminal-emulator"));
    EXPECT_TRUE(apps::isVteTerminalApp("tilix"));
    EXPECT_TRUE(apps::isVteTerminalApp("terminator"));
    platform::WindowInfo win{"x-terminal-emul", "nvim main.rs", 1234};
    EXPECT_TRUE(platform::isEditorActive(win));
}

TEST_F(ClakStateTest, GivenCtrlShift_TogglesEnabledState) {
    MockInputContext ic(instance_->inputContextManager(), "test-app");
    ime::ClakState state(engine_.get(), &ic);
    EXPECT_TRUE(engine_->isAppEnabled("test-app"));

    // strict ctrl then shift toggles off
    ic.sendKey(FcitxKey_Control_L, fcitx::KeyStates(), false, &state);
    ic.sendKey(FcitxKey_Shift_L, fcitx::KeyState::Ctrl, false, &state);
    ic.sendKey(FcitxKey_Shift_L, fcitx::KeyState::Ctrl, true, &state);
    EXPECT_FALSE(engine_->isAppEnabled("test-app"));
    fcitx::InputMethodEntry dummy_entry("clak", "Clak", "vi", "clak");
    EXPECT_EQ(engine_->subModeLabelImpl(dummy_entry, ic), "EN");

    // reverse order shift then ctrl must not toggle
    ic.sendKey(FcitxKey_Shift_L, fcitx::KeyStates(), false, &state);
    ic.sendKey(FcitxKey_Control_L, fcitx::KeyState::Shift, false, &state);
    ic.sendKey(FcitxKey_Shift_L, fcitx::KeyState::Ctrl, true, &state);
    ic.sendKey(FcitxKey_Control_L, fcitx::KeyStates(), true, &state);
    EXPECT_FALSE(engine_->isAppEnabled("test-app"));

    // shift+enter followed by ctrl+v must not toggle
    ic.sendKey(FcitxKey_Shift_L, fcitx::KeyStates(), false, &state);
    ic.sendKey(FcitxKey_Return, fcitx::KeyState::Shift, false, &state);
    ic.sendKey(FcitxKey_Return, fcitx::KeyState::Shift, true, &state);
    ic.sendKey(FcitxKey_Control_L, fcitx::KeyState::Shift, false, &state);
    ic.sendKey(FcitxKey_Shift_L, fcitx::KeyState::Ctrl, true, &state);
    ic.sendKey(FcitxKey_v, fcitx::KeyState::Ctrl, false, &state);
    ic.sendKey(FcitxKey_v, fcitx::KeyState::Ctrl, true, &state);
    ic.sendKey(FcitxKey_Control_L, fcitx::KeyStates(), true, &state);
    EXPECT_FALSE(engine_->isAppEnabled("test-app"));

    // ctrl then shift toggles back on
    ic.sendKey(FcitxKey_Control_L, fcitx::KeyStates(), false, &state);
    ic.sendKey(FcitxKey_Shift_L, fcitx::KeyState::Ctrl, false, &state);
    ic.sendKey(FcitxKey_Shift_L, fcitx::KeyState::Ctrl, true, &state);
    EXPECT_TRUE(engine_->isAppEnabled("test-app"));
    EXPECT_EQ(engine_->subModeLabelImpl(dummy_entry, ic), "VI");

    // combo with other key (ctrl+shift+t) must not toggle
    ic.sendKey(FcitxKey_Control_L, fcitx::KeyStates(), false, &state);
    ic.sendKey(FcitxKey_Shift_L, fcitx::KeyState::Ctrl, false, &state);
    ic.sendKey(FcitxKey_t, fcitx::KeyStates(fcitx::KeyState::Ctrl) | fcitx::KeyState::Shift, false, &state);
    ic.sendKey(FcitxKey_Shift_L, fcitx::KeyState::Ctrl, true, &state);
    ic.sendKey(FcitxKey_Control_L, fcitx::KeyStates(), true, &state);
    EXPECT_TRUE(engine_->isAppEnabled("test-app"));
}

TEST_F(ClakStateTest, GivenVowelAndHoldingToneKey_RepeatsCharacter) {
    MockInputContext ic(instance_->inputContextManager(), "test-app");
    ic.setSurrounding("", 0, 0);
    ic.auto_update_surrounding = true;
    ime::ClakState state(engine_.get(), &ic);
    state.setEnableRepeatForMock(true);

    // type a then press s
    ic.sendKey(FcitxKey_a, fcitx::KeyStates(), false, &state);
    ic.sendKey(FcitxKey_a, fcitx::KeyStates(), true, &state);
    ic.sendKey(FcitxKey_s, fcitx::KeyStates(), false, &state);

    // initial press of s produces á and arms repeat timer
    EXPECT_FALSE(ic.commits.empty());
    EXPECT_EQ(ic.commits.back(), "á");
    EXPECT_EQ(state.heldKey().sym(), FcitxKey_s);

    // first repeat tick untoggles á to as
    state.onRepeatTimer();
    EXPECT_EQ(ic.commits.back(), "as");
    EXPECT_TRUE(state.isRepeating());

    // subsequent repeat ticks append repeating s
    state.onRepeatTimer();
    EXPECT_EQ(ic.commits.back(), "s");

    state.onRepeatTimer();
    EXPECT_EQ(ic.commits.back(), "s");

    // release s cancels repeat
    ic.sendKey(FcitxKey_s, fcitx::KeyStates(), true, &state);
    EXPECT_EQ(state.heldKey().sym(), 0);
    EXPECT_FALSE(state.isRepeating());
}

TEST_F(ClakStateTest, GivenSteamApp_UsesForwardBackspaceWithTimer) {
    // steam app uses forward backspace and delayed commit instead of uinput
    platform::setMockActiveWindow(platform::WindowInfo{"steamwebhelper", "Steam", 1234});
    MockInputContext ic(instance_->inputContextManager(), "steamwebhelper");
    ime::ClakState state(engine_.get(), &ic);

    EXPECT_TRUE(state.isSteam());
    EXPECT_TRUE(config::isSteamApp("steam"));
    EXPECT_TRUE(config::isSteamApp("steamwebhelper"));
    EXPECT_TRUE(config::isSteamApp("com.valvesoftware.Steam"));
    EXPECT_FALSE(config::isSteamApp("upstream"));
    EXPECT_FALSE(config::isSteamApp("google-chrome"));
    EXPECT_FALSE(config::isSteamApp("zen"));
    EXPECT_FALSE(config::isSteamApp("kitty"));

    size_t uinput_bs_sent = 0;
    uinput::UinputTool::instance().setMockHandler([&](size_t count, uint32_t, uint32_t, uint32_t) {
        uinput_bs_sent = count;
        return true;
    });

    ic.setSurrounding("", 0, 0);
    ic.typeChar('d', &state);
    usleep(15000);
    ic.typeChar('d', &state);

    // uinput must not be used for steam
    EXPECT_EQ(uinput_bs_sent, 0);

    // expect 2 forwarded backspaces (press and release)
    EXPECT_EQ(ic.forwarded_keys.size(), 2);
    EXPECT_EQ(ic.forwarded_keys[0].sym(), FcitxKey_BackSpace);
    EXPECT_EQ(ic.forwarded_keys[1].sym(), FcitxKey_BackSpace);

    EXPECT_TRUE(state.isDeleting());

    auto exit_timer = instance_->eventLoop().addTimeEvent(
        CLOCK_MONOTONIC,
        fcitx::now(CLOCK_MONOTONIC) + 30000,
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

TEST_F(ClakStateTest, GivenSteamApp_CommitsPrintableKeyDirectly) {
    // steam app commits printable characters directly to prevent xim bounce reflections
    platform::setMockActiveWindow(platform::WindowInfo{"steamwebhelper", "Steam", 1234});
    MockInputContext ic(instance_->inputContextManager(), "steamwebhelper");
    ime::ClakState state(engine_.get(), &ic);

    // printable key is filtered and committed directly
    fcitx::Key key_n(FcitxKey_n);
    fcitx::KeyEvent event1(&ic, key_n, false);
    state.keyEvent(event1);
    EXPECT_TRUE(event1.filtered());
    ASSERT_FALSE(ic.commits.empty());
    EXPECT_EQ(ic.commits.back(), "n");

    // immediate duplicate reflection within 2ms is filtered and dropped without committing again
    fcitx::KeyEvent event2(&ic, key_n, false);
    state.keyEvent(event2);
    EXPECT_TRUE(event2.filtered());
    EXPECT_EQ(ic.commits.size(), 1);
}

TEST_F(ClakStateTest, GivenSteamApp_WhenTypingFast_ProcessesBufferedKeysInStrictOrder) {
    // keys arriving during deletion must be buffered and replayed in strict order
    platform::setMockActiveWindow(platform::WindowInfo{"steamwebhelper", "Steam", 1234});
    MockInputContext ic(instance_->inputContextManager(), "steamwebhelper");
    ime::ClakState state(engine_.get(), &ic);

    ic.setSurrounding("", 0, 0);
    ic.typeChar('d', &state);
    usleep(15000);
    ic.typeChar('d', &state);

    EXPECT_TRUE(state.isDeleting());

    // user types fast while deletion is in flight
    ic.typeChar('o', &state);
    ic.typeChar('n', &state);
    ic.typeChar('g', &state);

    EXPECT_TRUE(state.isDeleting());
    EXPECT_EQ(state.bufferedKeysCount(), 3);

    auto timer = instance_->eventLoop().addTimeEvent(
        CLOCK_MONOTONIC,
        fcitx::now(CLOCK_MONOTONIC) + 40000,
        0,
        [this](fcitx::EventSourceTime*, uint64_t) {
            instance_->eventLoop().exit();
            return true;
        }
    );
    instance_->eventLoop().exec();

    EXPECT_FALSE(state.isDeleting());
    EXPECT_EQ(state.bufferedKeysCount(), 0);
    // commits must reflect: initial 'd', replacement 'đ', then batched or sequential 'o', 'n', 'g'
    std::string full_commit;
    for (const auto& c : ic.commits) full_commit += c;
    EXPECT_TRUE(full_commit.find("đ") != std::string::npos);
    EXPECT_TRUE(full_commit.find("ong") != std::string::npos);
}

TEST_F(ClakStateTest, GivenSteamApp_StaggersMultipleBackspacesWithTimers) {
    // verify multiple backspaces are staggered across event loop timers for steam
    platform::setMockActiveWindow(platform::WindowInfo{"steamwebhelper", "Steam", 1234});
    MockInputContext ic(instance_->inputContextManager(), "steamwebhelper");
    ime::ClakState state(engine_.get(), &ic);

    ic.setSurrounding("", 0, 0);
    ic.typeChar('d', &state);
    ic.typeChar('u', &state);
    ic.typeChar('o', &state);
    ic.typeChar('n', &state);
    ic.typeChar('g', &state);

    EXPECT_EQ(ic.forwarded_keys.size(), 0);
    ic.typeChar('w', &state);

    EXPECT_TRUE(state.isDeleting());
    // only the first backspace pair should be sent immediately
    EXPECT_EQ(ic.forwarded_keys.size(), 2);

    auto timer = instance_->eventLoop().addTimeEvent(
        CLOCK_MONOTONIC,
        fcitx::now(CLOCK_MONOTONIC) + 80000,
        0,
        [this](fcitx::EventSourceTime*, uint64_t) {
            instance_->eventLoop().exit();
            return true;
        }
    );
    instance_->eventLoop().exec();

    EXPECT_FALSE(state.isDeleting());
    // all 4 backspaces (8 events) must be sent after loop finishes
    EXPECT_EQ(ic.forwarded_keys.size(), 8);
    ASSERT_FALSE(ic.commits.empty());
    EXPECT_EQ(ic.commits.back(), "ương");
}

TEST_F(ClakStateTest, GivenSelection_WhenCursorMoveKeyReceived_DoesNotFilterForNativeSelection) {
    MockInputContext ic(instance_->inputContextManager(), "test-app");
    ime::ClakState state(engine_.get(), &ic);

    // simulate shift+right arrow then shift+down arrow
    fcitx::Key key_right(FcitxKey_Right, fcitx::KeyState::Shift);
    fcitx::KeyEvent event_right(&ic, key_right, false);
    state.keyEvent(event_right);

    // cursor move and selection keys must pass through natively so app handles Shift-selection
    EXPECT_FALSE(event_right.filtered());
    EXPECT_TRUE(ic.forwarded_keys.empty());

    fcitx::Key key_down(FcitxKey_Down, fcitx::KeyState::Shift);
    fcitx::KeyEvent event_down(&ic, key_down, false);
    state.keyEvent(event_down);

    EXPECT_FALSE(event_down.filtered());
    EXPECT_TRUE(ic.forwarded_keys.empty());
}

TEST_F(ClakStateTest, GivenSelection_WhenShiftTapped_AutoFixesSelection) {
    clak_config_set_shift_autofix(const_cast<ClakConfig*>(engine_->config()), true);

    MockInputContext ic(instance_->inputContextManager(), "test-app");
    ic.setSurrounding("ddaay laf moojt baif toans khos", 31, 0);
    ime::ClakState state(engine_.get(), &ic);

    // tap Shift
    ic.sendKey(FcitxKey_Shift_L, fcitx::KeyStates(), false, &state);
    ic.sendKey(FcitxKey_Shift_L, fcitx::KeyStates(), true, &state);

    ASSERT_FALSE(ic.commits.empty());
    EXPECT_EQ(ic.commits.back(), "đây là một bài toán khó");
    EXPECT_TRUE(ic.deletions.empty());
}

TEST_F(ClakStateTest, GivenMultilinePartialSelection_WhenShiftTapped_ReplacesOnlySelectedRange) {
    clak_config_set_shift_autofix(const_cast<ClakConfig*>(engine_->config()), true);

    MockInputContext ic(instance_->inputContextManager(), "test-app");
    ic.auto_update_surrounding = true;
    std::string initial = "ddaay laf moot tinhs nawng hay\nddaay laf";
    // anchor at 31 (start of 2nd line), cursor at 40 (end of 2nd line)
    ic.setSurrounding(initial, 40, 31);
    ime::ClakState state(engine_.get(), &ic);

    // tap Shift
    ic.sendKey(FcitxKey_Shift_L, fcitx::KeyStates(), false, &state);
    ic.sendKey(FcitxKey_Shift_L, fcitx::KeyStates(), true, &state);

    EXPECT_TRUE(ic.deletions.empty());
    ASSERT_FALSE(ic.commits.empty());
    EXPECT_EQ(ic.commits.back(), "đây là");
    EXPECT_EQ(ic.surroundingText().text(), "ddaay laf moot tinhs nawng hay\nđây là");
}

TEST_F(ClakStateTest, GivenRecentAutoFix_WhenShiftTappedAgain_UndoesToOriginal) {
    clak_config_set_shift_autofix(const_cast<ClakConfig*>(engine_->config()), true);

    MockInputContext ic(instance_->inputContextManager(), "test-app");
    ic.setSurrounding("ddaay laf moojt baif toans khos", 31, 0);
    ime::ClakState state(engine_.get(), &ic);

    // 1st tap: autofixes
    ic.sendKey(FcitxKey_Shift_L, fcitx::KeyStates(), false, &state);
    ic.sendKey(FcitxKey_Shift_L, fcitx::KeyStates(), true, &state);
    ASSERT_FALSE(ic.commits.empty());
    EXPECT_EQ(ic.commits.back(), "đây là một bài toán khó");

    // update surrounding text to reflect the fix
    ic.setSurrounding("đây là một bài toán khó", 23, 23);

    // 2nd tap within 2s: undoes
    ic.sendKey(FcitxKey_Shift_L, fcitx::KeyStates(), false, &state);
    ic.sendKey(FcitxKey_Shift_L, fcitx::KeyStates(), true, &state);
    EXPECT_EQ(ic.commits.back(), "ddaay laf moojt baif toans khos");
}

TEST_F(ClakStateTest, GivenWordBeforeCursor_WhenShiftTapped_AutoFixesWord) {
    clak_config_set_shift_autofix(const_cast<ClakConfig*>(engine_->config()), true);

    MockInputContext ic(instance_->inputContextManager(), "test-app");
    ic.setSurrounding("toans", 5, 5);
    ime::ClakState state(engine_.get(), &ic);

    // tap Shift
    ic.sendKey(FcitxKey_Shift_L, fcitx::KeyStates(), false, &state);
    ic.sendKey(FcitxKey_Shift_L, fcitx::KeyStates(), true, &state);

    ASSERT_FALSE(ic.commits.empty());
    EXPECT_EQ(ic.commits.back(), "toán");
}

TEST_F(ClakStateTest, GivenShiftHeldWithOtherKey_DoesNotTriggerAutoFix) {
    clak_config_set_shift_autofix(const_cast<ClakConfig*>(engine_->config()), true);

    MockInputContext ic(instance_->inputContextManager(), "test-app");
    ic.setSurrounding("ddaay", 5, 0);
    ime::ClakState state(engine_.get(), &ic);

    // Shift pressed, then 'a' pressed, 'a' released, Shift released
    ic.sendKey(FcitxKey_Shift_L, fcitx::KeyStates(), false, &state);
    ic.sendKey(FcitxKey_a, fcitx::KeyState::Shift, false, &state);
    ic.sendKey(FcitxKey_a, fcitx::KeyState::Shift, true, &state);
    ic.sendKey(FcitxKey_Shift_L, fcitx::KeyStates(), true, &state);

    // commits should not contain "đây" from autofix
    for (const auto& c : ic.commits) {
        EXPECT_NE(c, "đây");
    }
}

TEST_F(ClakStateTest, GivenShiftHeldWithMouseClick_DoesNotTriggerAutoFix) {
    clak_config_set_shift_autofix(const_cast<ClakConfig*>(engine_->config()), true);

    MockInputContext ic(instance_->inputContextManager(), "test-app");
    ic.setSurrounding("ddaay", 5, 0);
    ime::ClakState state(engine_.get(), &ic);

    // Shift pressed, mouse click (reset called), Shift released
    ic.sendKey(FcitxKey_Shift_L, fcitx::KeyStates(), false, &state);
    state.reset(/*force=*/true);
    ic.sendKey(FcitxKey_Shift_L, fcitx::KeyStates(), true, &state);

    // commits must be empty
    EXPECT_TRUE(ic.commits.empty());
}

TEST_F(ClakStateTest, GivenAutocorrectInGecko_BuffersSubsequentKeystrokesAndReplaysCleanly) {
    clak_config_set_autocorrect(const_cast<ClakConfig*>(engine_->config()), true);
    clak_config_set_auto_restore(const_cast<ClakConfig*>(engine_->config()), false);
    platform::setMockActiveWindow(platform::WindowInfo{"zen", "Zen Browser", 1234});
    MockInputContext ic(instance_->inputContextManager(), "zen");
    ime::ClakState state(engine_.get(), &ic);

    size_t uinput_bs_sent = 0;
    uinput::UinputTool::instance().setMockHandler([&](size_t count, uint32_t, uint32_t, uint32_t) {
        uinput_bs_sent = count;
        return true;
    });

    ic.setSurrounding("", 0, 0);
    // type scrambled word: nheiuef, looping back any intermediate modifier sentinels
    for (char c : std::string("nheiuef")) {
        ic.typeChar(c, &state);
        while (state.isDeleting()) {
            ic.sendCleanBackspace(&state);
        }
    }
    EXPECT_FALSE(state.isDeleting());

    // type space to trigger autocorrect
    ic.typeChar(' ', &state);
    EXPECT_TRUE(state.isDeleting());
    // 6 backspaces for nheìue + 1 sentinel = 7 backspaces
    EXPECT_EQ(uinput_bs_sent, 7);

    // user rapidly types next keys while uinput deletion is in flight
    ic.typeChar('v', &state);
    ic.typeChar('a', &state);
    EXPECT_EQ(state.bufferedKeysCount(), 2);

    // sentinel loopback backspaces arrive
    for (size_t i = 0; i < uinput_bs_sent; ++i) {
        ic.sendCleanBackspace(&state);
    }

    EXPECT_FALSE(state.isDeleting());
    // batched commit contains corrected word with space and the buffered forward keys ("va")
    ASSERT_FALSE(ic.commits.empty());
    EXPECT_EQ(ic.commits.back(), "nhiều va");
}

TEST_F(ClakStateTest, GivenAutocorrect_ShiftTapUndoesAndRestoresOriginalWord) {
    clak_config_set_autocorrect(const_cast<ClakConfig*>(engine_->config()), true);
    clak_config_set_shift_autofix(const_cast<ClakConfig*>(engine_->config()), true);
    clak_config_set_auto_restore(const_cast<ClakConfig*>(engine_->config()), false);

    MockInputContext ic(instance_->inputContextManager(), "test-app");
    ic.auto_update_surrounding = true;
    ime::ClakState state(engine_.get(), &ic);

    ic.setSurrounding("", 0, 0);
    // type scrambled word + space
    for (char c : std::string("nheiuef ")) {
        ic.typeChar(c, &state);
    }

    ASSERT_FALSE(ic.commits.empty());
    EXPECT_EQ(ic.commits.back(), "nhiều ");

    // update surrounding to reflect autocorrect
    ic.setSurrounding("nhiều ", 6, 6);

    // tap Shift to undo autocorrect
    ic.sendKey(FcitxKey_Shift_L, fcitx::KeyStates(), false, &state);
    ic.sendKey(FcitxKey_Shift_L, fcitx::KeyStates(), true, &state);

    ASSERT_FALSE(ic.commits.empty());
    EXPECT_EQ(ic.commits.back(), "nheìue ");
}

TEST_F(ClakStateTest, GivenAutocorrect_CtrlZUndoesAndRestoresOriginalWord) {
    clak_config_set_autocorrect(const_cast<ClakConfig*>(engine_->config()), true);
    clak_config_set_shift_autofix(const_cast<ClakConfig*>(engine_->config()), true);
    clak_config_set_auto_restore(const_cast<ClakConfig*>(engine_->config()), false);

    MockInputContext ic(instance_->inputContextManager(), "test-app");
    ic.auto_update_surrounding = true;
    ime::ClakState state(engine_.get(), &ic);

    ic.setSurrounding("", 0, 0);
    // type scrambled word + space
    for (char c : std::string("nheiuef ")) {
        ic.typeChar(c, &state);
    }

    ASSERT_FALSE(ic.commits.empty());
    EXPECT_EQ(ic.commits.back(), "nhiều ");

    // update surrounding to reflect autocorrect
    ic.setSurrounding("nhiều ", 6, 6);

    // press Ctrl+Z to undo autocorrect
    ic.sendKey(FcitxKey_z, fcitx::KeyState::Ctrl, false, &state);

    ASSERT_FALSE(ic.commits.empty());
    EXPECT_EQ(ic.commits.back(), "nheìue ");
}

TEST_F(ClakStateTest, GivenAutocorrect_BackspaceAfterSpaceIsNotHijacked) {
    clak_config_set_autocorrect(const_cast<ClakConfig*>(engine_->config()), true);

    MockInputContext ic(instance_->inputContextManager(), "test-app");
    ic.auto_update_surrounding = true;
    ime::ClakState state(engine_.get(), &ic);

    ic.setSurrounding("", 0, 0);
    for (char c : std::string("nheiuef ")) {
        ic.typeChar(c, &state);
    }
    EXPECT_EQ(ic.commits.back(), "nhiều ");
    ic.setSurrounding("nhiều ", 6, 6);

    // backspace after space should behave normally (delete the space character, not hijack to restore)
    ic.clearHistory();
    ic.sendKey(FcitxKey_BackSpace, fcitx::KeyStates(), false, &state);
    ic.sendKey(FcitxKey_BackSpace, fcitx::KeyStates(), true, &state);

    // must not commit original scrambled word
    for (const auto& c : ic.commits) {
        EXPECT_NE(c, "nheiuef ");
        EXPECT_NE(c, "nheiuef");
    }
}

TEST_F(ClakStateTest, GivenSpaceAfterWord_WhenBackspacedAndToneKeyTyped_ResumesWord) {
    // typing a word, then space, then backspace allows adding tone to the word
    MockInputContext ic(instance_->inputContextManager(), "test-app");
    ic.auto_update_surrounding = true;
    ime::ClakState state(engine_.get(), &ic);
    ic.setSurrounding("", 0, 0);

    // type "toan" + " "
    for (char c : std::string("toan ")) {
        ic.typeChar(c, &state);
    }
    EXPECT_EQ(ic.surroundingText().text(), "toan ");

    // backspace the trailing space
    ic.sendKey(FcitxKey_BackSpace, fcitx::KeyStates(), false, &state);
    ic.sendKey(FcitxKey_BackSpace, fcitx::KeyStates(), true, &state);
    ic.setSurrounding("toan", 4, 4);
    EXPECT_EQ(ic.surroundingText().text(), "toan");

    // type 's' to add tone
    ic.typeChar('s', &state);
    EXPECT_EQ(ic.surroundingText().text(), "toán");
}

TEST_F(ClakStateTest, PropertyTest_BatchReplayMatchesSequentialReference) {
    platform::setMockActiveWindow(platform::WindowInfo{"kitty", "kitty", 1234});

    std::vector<std::string> test_words = {
        "tieengs", "dduowcj", "nguwowfi", "khoong", "thanh", "tharn",
        "tuyeetj", "vowfi", "nhuwxng", "nawngs", "roojng", "lowsn",
        "chieeus", "roji", "xuoongs", "khawps", "thung", "lungx",
        "mown", "mowrn", "suoosi", "luowjn", "uoosn", "chawm", "chir",
        "kieen", "trif", "gawsn", "bos", "ruoojng", "ddoofng", "vawn",
        "hoas", "ddaamj", "ddaf", "barn", "sawsc", "booj", "gox",
        "vieetj", "hoatj", "ddoojng", "muowjt", "maf", "chongs", "chinhs"
    };

    size_t total_uinput_sequential = 0;
    size_t total_uinput_batched = 0;

    for (const auto& word : test_words) {
        // sequential reference path: each key processed after sentinel returns
        std::string ref_screen;
        size_t uinput_ref = 0;
        {
            MockInputContext ic(instance_->inputContextManager(), "kitty");
            ime::ClakState state(engine_.get(), &ic);
            uinput::UinputTool::instance().setMockHandler([&](size_t count, uint32_t, uint32_t, uint32_t, uint32_t) {
                // real backspaces sent to app are count - 1 (last is sentinel)
                if (count > 1) {
                    utils::popUtf8Chars(ref_screen, count - 1);
                    uinput_ref += (count - 1);
                }
                return true;
            });
            ic.on_commit = [&](const std::string& text) { ref_screen += text; };
            ic.on_forward_key = [&](const fcitx::ForwardKeyEvent& fk) {
                std::string s = fcitx::Key::keySymToUTF8(fk.key().sym());
                if (!s.empty()) ref_screen += s;
            };

            for (char c : word) {
                if (!ic.typeChar(c, &state)) {
                    ref_screen += c;
                }
                if (state.isDeleting()) {
                    state.finishUinputDeletion(100);
                }
            }
        }

        // batched replay path: keys buffered during deletion and replayed in memory
        std::string batched_screen;
        size_t uinput_batched = 0;
        {
            MockInputContext ic(instance_->inputContextManager(), "kitty");
            ime::ClakState state(engine_.get(), &ic);
            uinput::UinputTool::instance().setMockHandler([&](size_t count, uint32_t, uint32_t, uint32_t, uint32_t) {
                if (count > 1) {
                    utils::popUtf8Chars(batched_screen, count - 1);
                    uinput_batched += (count - 1);
                }
                return true;
            });
            ic.on_commit = [&](const std::string& text) { batched_screen += text; };
            ic.on_forward_key = [&](const fcitx::ForwardKeyEvent& fk) {
                std::string s = fcitx::Key::keySymToUTF8(fk.key().sym());
                if (!s.empty()) batched_screen += s;
            };

            size_t i = 0;
            while (i < word.size()) {
                if (!ic.typeChar(word[i++], &state)) {
                    batched_screen += word[i - 1];
                }
                if (state.isDeleting()) break;
            }
            while (i < word.size()) {
                if (!ic.typeChar(word[i++], &state)) {
                    batched_screen += word[i - 1];
                }
            }
            if (state.isDeleting()) {
                state.finishUinputDeletion(100);
            }
        }

        EXPECT_EQ(batched_screen, ref_screen) << "mismatch on word: " << word;
        total_uinput_sequential += uinput_ref;
        total_uinput_batched += uinput_batched;
    }

    EXPECT_GT(total_uinput_sequential, 0u);
    double reduction_pct = 100.0 * (1.0 - static_cast<double>(total_uinput_batched) / static_cast<double>(total_uinput_sequential));
    std::cout << "[BATCH ABSORPTION] sequential uinput BS=" << total_uinput_sequential
              << ", batched uinput BS=" << total_uinput_batched
              << ", reduction=" << reduction_pct << "%" << std::endl;
    EXPECT_GE(reduction_pct, 40.0);
}

TEST_F(ClakStateTest, GivenQueueFull_WhenOverflowOccurs_ForwardsKeysDirectly) {
    platform::setMockActiveWindow(platform::WindowInfo{"kitty", "kitty", 1234});
    MockInputContext ic(instance_->inputContextManager(), "kitty");
    ime::ClakState state(engine_.get(), &ic);

    uinput::UinputTool::instance().setMockHandler([](size_t, uint32_t, uint32_t, uint32_t, uint32_t) {
        return true;
    });

    // trigger deletion to enter isDeleting state
    ic.typeChar('d', &state);
    ic.typeChar('d', &state);
    EXPECT_TRUE(state.isDeleting());

    // fill buffer to capacity
    for (size_t i = 0; i < config::kMaxBufferedKeys; ++i) {
        ic.typeChar('a', &state);
    }
    EXPECT_EQ(state.bufferedKeysCount(), config::kMaxBufferedKeys);

    // subsequent key must trigger flush of buffered keys first, then forward new key
    size_t prev_forwarded = ic.forwarded_keys.size();
    ic.typeChar('z', &state);
    EXPECT_GT(ic.forwarded_keys.size(), prev_forwarded);
    EXPECT_EQ(ic.forwarded_keys.size(), config::kMaxBufferedKeys + 1);
    EXPECT_EQ(ic.forwarded_keys[0].sym(), 'a');
    EXPECT_EQ(ic.forwarded_keys.back().sym(), 'z');
    EXPECT_EQ(state.bufferedKeysCount(), 0);
    EXPECT_FALSE(state.isDeleting());
}

} // namespace test
} // namespace clak
