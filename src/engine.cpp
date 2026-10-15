#include "engine.h"
#include "platform/mouse_tracker.h"
#include "platform/window_info.h"
#include "uinput/uinput.h"

#include "utils/log.h"
#include <sys/inotify.h>
#include <unistd.h>
#include <fstream>

namespace clak {

ClakSettingsAction::ClakSettingsAction() {
    setShortText("Cài đặt Clak...");
    setLongText("Mở bảng điều khiển cấu hình Clak");
    setIcon("org.fcitx.Fcitx5.clak");
}

void ClakSettingsAction::activate(fcitx::InputContext* ic) {
    FCITX_UNUSED(ic);
    const char* home = getenv("HOME");
    std::string local_bin = home ? std::string(home) + "/.local/bin/clak-gui" : "";
    if (!local_bin.empty() && access(local_bin.c_str(), X_OK) == 0) {
        fcitx::startProcess({local_bin});
    } else {
        fcitx::startProcess({"clak-gui"});
    }
}

ClakEngine::ClakEngine(fcitx::Instance* instance)
    : instance_(instance),
      factory_([this](fcitx::InputContext& ic) -> ime::ClakState* {
          return new ime::ClakState(this, &ic);
      }) {
    instance_->inputContextManager().registerProperty("clakState", &factory_);
    instance_->userInterfaceManager().registerAction("clak-settings", &settings_action_);
    // pre-warm uinput device so kernel and libinput enumerate it before first use
    uinput::UinputTool::instance();
    loadConfig();
    setupConfigWatcher();
    mouse_tracker_ = std::make_unique<platform::MouseTracker>(
        instance_->eventLoop(),
        [this]() {
            onMouseClick();
        }
    );
}

ClakEngine::~ClakEngine() {
    instance_->userInterfaceManager().unregisterAction(&settings_action_);
    if (mouse_tracker_) {
        mouse_tracker_.reset();
    }
    if (config_io_) {
        config_io_.reset();
    }
    if (inotify_wd_ >= 0 && inotify_fd_ >= 0) {
        inotify_rm_watch(inotify_fd_, inotify_wd_);
    }
    if (inotify_fd_ >= 0) {
        close(inotify_fd_);
    }
    if (config_) {
        clak_config_free(config_);
        config_ = nullptr;
    }
}

void ClakEngine::onMouseClick() {
    auto* ic = instance_->lastFocusedInputContext();
    if (ic) {
        auto* state = ic->propertyFor(&factory_);
        if (state) {
            utils::clakLog("mouse_tracker: click detected, resetting composition for active window");
            state->reset(/*force=*/true);
        }
    }
}

void ClakEngine::loadConfig() {
    if (config_) {
        clak_config_free(config_);
    }
    config_ = clak_config_load();
    config_version_++;
    if (config_) {
        global_enabled_ = (clak_config_get_startup_mode(config_) == 0);
        bool debug_log = clak_config_get_debug_log(config_);
        const char* env = getenv("CLAK_LOG");
        if (!env) {
            utils::setLogEnabled(debug_log);
        }
        utils::clakLog("config loaded: version=" + std::to_string(config_version_) +
                       " startup=" + (global_enabled_ ? "vietnamese" : "english"));
    }
}

void ClakEngine::setupConfigWatcher() {
    char* path_c = clak_config_path();
    if (!path_c) return;
    std::string config_path(path_c);
    clak_free_string(path_c);

    size_t last_slash = config_path.find_last_of('/');
    std::string dir = (last_slash != std::string::npos) ? config_path.substr(0, last_slash) : ".";

    inotify_fd_ = inotify_init1(IN_NONBLOCK | IN_CLOEXEC);
    if (inotify_fd_ < 0) return;

    inotify_wd_ = inotify_add_watch(inotify_fd_, dir.c_str(), IN_CLOSE_WRITE | IN_MOVED_TO | IN_CREATE);
    if (inotify_wd_ < 0) {
        close(inotify_fd_);
        inotify_fd_ = -1;
        return;
    }

    config_io_ = instance_->eventLoop().addIOEvent(
        inotify_fd_,
        fcitx::IOEventFlag::In,
        [this](fcitx::EventSourceIO*, int fd, fcitx::IOEventFlags) {
            char buffer[4096];
            while (read(fd, buffer, sizeof(buffer)) > 0) {}
            utils::clakLog("config file change detected via inotify, reloading");
            loadConfig();
            return true;
        }
    );
}

bool ClakEngine::isAppEnabled(const std::string& app) {
    if (config_ && clak_config_is_app_excluded(config_, app.c_str())) {
        return false;
    }
    if (config_ && clak_config_get_remember_state(config_)) {
        auto it = app_states_.find(app);
        if (it != app_states_.end()) {
            return it->second;
        }
    }
    return global_enabled_;
}

void ClakEngine::toggleAppEnabled(const std::string& app) {
    bool current = isAppEnabled(app);
    bool next = !current;
    if (config_ && clak_config_get_remember_state(config_)) {
        if (app_states_.size() >= kMaxTrackedAppStates) {
            app_states_.clear();
        }
        app_states_[app] = next;
    } else {
        global_enabled_ = next;
    }
    utils::clakLog("toggled input state for app '" + app + "' -> " + (next ? "VI" : "EN"));
}

void ClakEngine::setAppEnabled(const std::string& app, bool enabled) {
    if (config_ && clak_config_get_remember_state(config_)) {
        if (app_states_.size() >= kMaxTrackedAppStates) {
            app_states_.clear();
        }
        app_states_[app] = enabled;
    } else {
        global_enabled_ = enabled;
    }
}

void ClakEngine::keyEvent(const fcitx::InputMethodEntry& entry, fcitx::KeyEvent& keyEvent) {
    FCITX_UNUSED(entry);
    auto* ic = keyEvent.inputContext();
    auto* state = ic ? ic->propertyFor(&factory_) : nullptr;
    if (state) {
        state->keyEvent(keyEvent);
    } else {
        utils::clakLog("engine::keyEvent: NO state for ic program='" + (ic ? ic->program() : "") + "'");
    }
}

void ClakEngine::activate(const fcitx::InputMethodEntry& entry, fcitx::InputContextEvent& event) {
    FCITX_UNUSED(entry);
    platform::clearWindowCache();
    auto* ic = event.inputContext();
    if (ic) {
        ic->statusArea().addAction(fcitx::StatusGroup::InputMethod, &settings_action_);
        settings_action_.update(ic);
        ic->updateUserInterface(fcitx::UserInterfaceComponent::StatusArea);
    }
    auto* state = ic ? ic->propertyFor(&factory_) : nullptr;
    std::string app = state ? state->appKey() : (ic ? ic->program() : "");
    utils::clakLog("engine::activate: ic program='" + (ic ? ic->program() : "") + "' app='" + app + "' enabled=" + std::to_string(isAppEnabled(app)));
    if (checkBinaryReplaced() && !notification_sent_) {
        notification_sent_ = true;
        utils::clakLog("engine: libclak.so updated on disk ((deleted) inode), reload fcitx5 to apply");
        int ret = system("command -v notify-send >/dev/null 2>&1 && notify-send -u normal -a Clak -i org.fcitx.Fcitx5 'Clak có bản cập nhật mới' 'Đã cài đặt bản mới thành công. Hãy khởi động lại Fcitx5 để áp dụng.' &");
        (void)ret;
    }
    if (state && state->allRealBackspacesReceived()) {
        // complete uinput deletion early if all real backspaces were already processed before text-input reactivation
        utils::clakLog("engine::activate: completing uinput deletion early during text-input reactivation (real backspaces: " +
                       std::to_string(state->currentBackspaceCount()) + "/" + std::to_string(state->expectedBackspaces()) + ")");
        state->finishUinputDeletion();
    }
}

void ClakEngine::deactivate(const fcitx::InputMethodEntry& entry, fcitx::InputContextEvent& event) {
    FCITX_UNUSED(entry);
    auto* ic = event.inputContext();
    utils::clakLog("engine::deactivate: ic program='" + (ic ? ic->program() : "") + "'");
}

void ClakEngine::reset(const fcitx::InputMethodEntry& entry, fcitx::InputContextEvent& event) {
    FCITX_UNUSED(entry);
    auto* state = event.inputContext()->propertyFor(&factory_);
    if (state) {
        state->reset(/*force=*/false);
    }
}

std::string ClakEngine::subMode(const fcitx::InputMethodEntry& entry, fcitx::InputContext& ic) {
    FCITX_UNUSED(entry);
    auto* state = ic.propertyFor(&factory_);
    std::string app = state ? state->appKey() : (ic.program().empty() ? "default" : ic.program());
    if (!isAppEnabled(app)) {
        return "English";
    }
    if (!config_) return "Telex";
    int method = clak_config_get_method(config_);
    switch (method) {
        case 1: return "VNI";
        case 2: return "VIQR";
        case 3: return "TeipVNI";
        default: return "Telex";
    }
}

std::string ClakEngine::subModeIconImpl(const fcitx::InputMethodEntry& entry, fcitx::InputContext& ic) {
    FCITX_UNUSED(entry);
    FCITX_UNUSED(ic);
    return "org.fcitx.Fcitx5.clak";
}

std::string ClakEngine::subModeLabelImpl(const fcitx::InputMethodEntry& entry, fcitx::InputContext& ic) {
    FCITX_UNUSED(entry);
    auto* state = ic.propertyFor(&factory_);
    std::string app = state ? state->appKey() : (ic.program().empty() ? "default" : ic.program());
    return isAppEnabled(app) ? "VI" : "EN";
}

bool ClakEngine::checkBinaryReplaced() {
    uint64_t now = fcitx::now(CLOCK_MONOTONIC);
    // rate-limit check to once every 60 seconds
    if (now - last_replacement_check_us_ < 60000000) {
        return binary_replaced_;
    }
    last_replacement_check_us_ = now;

    std::ifstream maps("/proc/self/maps");
    if (!maps.is_open()) {
        return binary_replaced_;
    }

    std::string line;
    while (std::getline(maps, line)) {
        if (line.find("clak") != std::string::npos && line.find("(deleted)") != std::string::npos) {
            binary_replaced_ = true;
            break;
        }
    }
    return binary_replaced_;
}

} // namespace clak

class ClakEngineFactoryExport : public fcitx::AddonFactory {
public:
    fcitx::AddonInstance* create(fcitx::AddonManager* manager) override {
        return new clak::ClakEngine(manager->instance());
    }
};

FCITX_ADDON_FACTORY(ClakEngineFactoryExport);
