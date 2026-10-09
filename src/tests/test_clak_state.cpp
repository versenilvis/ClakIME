#include <gtest/gtest.h>
#include <fcitx/instance.h>
#include <fcitx-utils/event.h>
#include "mock_input_context.h"
#include "ime/state.h"
#include "engine.h"
#include "core.h"
#include "uinput/uinput.h"
#include "platform/window_info.h"
#include "platform/modal_editor.h"

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

    // run event loop on the same thread with an exit timer at 400ms (safety timer is 50ms)
    auto exit_timer = instance_->eventLoop().addTimeEvent(
        CLOCK_MONOTONIC,
        fcitx::now(CLOCK_MONOTONIC) + 400000,
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
    ASSERT_EQ(ic.commits.size(), 2);
    EXPECT_EQ(ic.commits[0], "đ");
    EXPECT_EQ(ic.commits[1], " ba");
}

TEST_F(ClakStateTest, GivenChromiumNormalPage_UsesSurroundingText) {
    // normal browser page with valid surrounding text should use surrounding text deletion
    MockInputContext ic(instance_->inputContextManager(), "google-chrome");
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

} // namespace test
} // namespace clak
