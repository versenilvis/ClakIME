#include "engine.h"
#include "mouse/mouse_tracker.h"
#include "window/window_info.h"
#include "uinput/uinput.h"

#include "utils/log.h"
#include <sys/inotify.h>
#include <unistd.h>
#include <fstream>

namespace clak {

ClakSettingsAction::ClakSettingsAction() {
  setShortText("Cài đặt Clak...");
  setLongText("Mở bảng điều khiển cấu hình Clak");
  setIcon("clak-tray-settings");
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
  initTrayActions();

  // re-add tray actions whenever any IC gains focus so they don't vanish between windows
  event_watchers_.emplace_back(
    instance_->watchEvent<fcitx::EventType::InputContextFocusIn>(
      fcitx::EventWatcherPhase::Default,
      [this](fcitx::FocusInEvent& e) {
        auto* ic = e.inputContext();
        if (!ic) return;
        if (instance_->inputMethodEngine(ic) != this) return;
        attachTrayActions(ic);
      }));
  // pre-warm uinput device so kernel and libinput enumerate it before first use
  uinput::UinputTool::instance();
  loadConfig();
  setupConfigWatcher();
  mouse_tracker_ = std::make_unique<mouse::MouseTracker>(
    instance_->eventLoop(),
    [this]() {
      onMouseClick();
    });
}

ClakEngine::~ClakEngine() {
  unregisterTrayActions();
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
    int startup = clak_config_get_startup_mode(config_);
    if (!initial_config_loaded_ || startup != last_startup_mode_) {
      global_enabled_ = (startup == 0);
      last_startup_mode_ = startup;
      initial_config_loaded_ = true;
    }
    bool debug_log = clak_config_get_debug_log(config_);
    const char* env = getenv("CLAK_LOG");
    if (!env) {
      utils::setLogEnabled(debug_log);
    }
    bool is_adaptive_env = (!utils::clakGetEnv("CLAK_SIMPLE_BACKOFF") &&
                            !(utils::clakGetEnv("CLAK_ADAPTIVE") &&
                              std::string(utils::clakGetEnv("CLAK_ADAPTIVE")) == "0"));
    std::string pacing_mode = utils::clakGetEnv("CLAK_FIXED_DELAY_MS") ? "fixed" : (!is_adaptive_env ? "fixed_13_backoff" : "adaptive");
    utils::clakLog("config loaded: version=" + std::to_string(config_version_) +
                   " startup=" + (global_enabled_ ? "vietnamese" : "english") +
                   " mode=" + pacing_mode);
    if (auto* last_ic = instance_->lastFocusedInputContext()) {
      syncTrayActions(last_ic);
      last_ic->updateUserInterface(fcitx::UserInterfaceComponent::StatusArea, true);
    }
  }
}

namespace {

constexpr const char* kMethodNames[] = {"Telex", "VNI", "VIQR", "Telex + VNI"};
constexpr int kMethodCount = 4;

void setToggleActionVisual(fcitx::SimpleAction& action, const char* label, const char* icon_on, const char* icon_off, bool enabled, fcitx::InputContext* ic) {
  action.setIcon(enabled ? icon_on : icon_off);
  action.setShortText(label);
  action.update(ic);
}

} // namespace

std::string ClakEngine::resolveAppKey(fcitx::InputContext* ic) {
  if (!ic) return "default";
  auto* state = ic->propertyFor(&factory_);
  std::string app = state ? state->appKey() : ic->program();
  return app.empty() ? "default" : app;
}

void ClakEngine::initTrayActions() {
  auto& ui = instance_->userInterfaceManager();

  // match ngosen and lotus: avoid checkable radio flags and colons in labels
  toggle_vi_action_.setCheckable(false);
  toggle_vi_action_.setIcon("clak-tray-vi-on");
  toggle_vi_action_.setShortText("Tiếng Việt");
  toggle_vi_action_.setLongText("Bật hoặc tắt chế độ gõ Tiếng Việt");
  tray_connections_.emplace_back(
    toggle_vi_action_.connect<fcitx::SimpleAction::Activated>([this](fcitx::InputContext* ic) {
      auto* target_ic = ic ? ic : instance_->lastFocusedInputContext();
      std::string app = resolveAppKey(target_ic);
      toggleAppEnabled(app);
      if (target_ic) {
        if (auto* state = target_ic->propertyFor(&factory_)) {
          state->reset(/*force=*/false);
        }
      }
      syncTrayActions(target_ic);
      if (target_ic) {
        target_ic->updateUserInterface(fcitx::UserInterfaceComponent::StatusArea, true);
      }
    }));
  ui.registerAction("clak-toggle-vi", &toggle_vi_action_);

  method_action_.setCheckable(false);
  method_action_.setIcon("clak-tray-keyboard");
  method_action_.setShortText("Kiểu gõ (Telex)");
  method_action_.setLongText("Bấm để chuyển vòng kiểu gõ (Telex / VNI / VIQR / Telex + VNI)");
  tray_connections_.emplace_back(
    method_action_.connect<fcitx::SimpleAction::Activated>([this](fcitx::InputContext* ic) {
      applyAndSaveTrayConfig(ic, [](ClakConfig* cfg) {
        int cur = clak_config_get_method(cfg);
        int next = (cur + 1) % kMethodCount;
        clak_config_set_method(cfg, next);
      });
    }));
  ui.registerAction("clak-method", &method_action_);

  spelling_action_.setCheckable(false);
  spelling_action_.setIcon("clak-tray-spelling-on");
  spelling_action_.setShortText("Chính tả");
  spelling_action_.setLongText("Ngăn từ sai quy tắc tiếng Việt và tự khôi phục từ tiếng Anh");
  tray_connections_.emplace_back(
    spelling_action_.connect<fcitx::SimpleAction::Activated>([this](fcitx::InputContext* ic) {
      applyAndSaveTrayConfig(ic, [](ClakConfig* cfg) {
        bool next = !clak_config_get_spelling(cfg);
        clak_config_set_spelling(cfg, next);
        clak_config_set_auto_restore(cfg, next);
      });
    }));
  ui.registerAction("clak-spelling", &spelling_action_);

  autocorrect_action_.setCheckable(false);
  autocorrect_action_.setIcon("clak-tray-autocorrect-on");
  autocorrect_action_.setShortText("Tự sửa");
  autocorrect_action_.setLongText("Tự động sửa lỗi đảo phím khi gõ nhanh");
  tray_connections_.emplace_back(
    autocorrect_action_.connect<fcitx::SimpleAction::Activated>([this](fcitx::InputContext* ic) {
      applyAndSaveTrayConfig(ic, [](ClakConfig* cfg) {
        bool next = !clak_config_get_autocorrect(cfg);
        clak_config_set_autocorrect(cfg, next);
      });
    }));
  ui.registerAction("clak-autocorrect", &autocorrect_action_);

  ui.registerAction("clak-settings", &settings_action_);
}

void ClakEngine::unregisterTrayActions() {
  event_watchers_.clear();
  tray_connections_.clear();
  auto& ui = instance_->userInterfaceManager();
  ui.unregisterAction(&toggle_vi_action_);
  ui.unregisterAction(&method_action_);
  ui.unregisterAction(&spelling_action_);
  ui.unregisterAction(&autocorrect_action_);
  ui.unregisterAction(&settings_action_);
}

void ClakEngine::attachTrayActions(fcitx::InputContext* ic) {
  if (!ic) return;
  ic->statusArea().clearGroup(fcitx::StatusGroup::InputMethod);
  ic->statusArea().addAction(fcitx::StatusGroup::InputMethod, &toggle_vi_action_);
  ic->statusArea().addAction(fcitx::StatusGroup::InputMethod, &method_action_);
  ic->statusArea().addAction(fcitx::StatusGroup::InputMethod, &spelling_action_);
  ic->statusArea().addAction(fcitx::StatusGroup::InputMethod, &autocorrect_action_);
  ic->statusArea().addAction(fcitx::StatusGroup::InputMethod, &settings_action_);
  syncTrayActions(ic);
  ic->updateUserInterface(fcitx::UserInterfaceComponent::StatusArea);
}

void ClakEngine::syncTrayActions(fcitx::InputContext* ic) {
  std::string app = resolveAppKey(ic);
  bool vi_on = isAppEnabled(app);
  toggle_vi_action_.setIcon("clak-tray-vi-on");
  toggle_vi_action_.setShortText(vi_on ? "Tiếng Việt" : "Tiếng Anh");
  toggle_vi_action_.update(ic);

  int method = config_ ? clak_config_get_method(config_) : 0;
  if (method < 0 || method >= kMethodCount) {
    method = 0;
  }
  method_action_.setIcon("clak-tray-keyboard");
  method_action_.setShortText(std::string("Kiểu gõ (") + kMethodNames[method] + ")");
  method_action_.update(ic);

  bool spelling = config_ ? clak_config_get_spelling(config_) : true;
  setToggleActionVisual(spelling_action_, "Chính tả", "clak-tray-spelling-on", "clak-tray-spelling-off", spelling, ic);

  bool autocorrect = config_ ? clak_config_get_autocorrect(config_) : true;
  setToggleActionVisual(autocorrect_action_, "Tự sửa", "clak-tray-autocorrect-on", "clak-tray-autocorrect-off", autocorrect, ic);

  settings_action_.update(ic);
}

void ClakEngine::applyAndSaveTrayConfig(fcitx::InputContext* ic, const std::function<void(ClakConfig*)>& mutator) {
  if (!config_) return;
  mutator(config_);
  config_version_++;
  clak_config_save_toml(config_);

  auto* target_ic = ic ? ic : instance_->lastFocusedInputContext();
  if (target_ic) {
    if (auto* state = target_ic->propertyFor(&factory_)) {
      state->syncConfig();
    }
  }
  syncTrayActions(target_ic);
  if (target_ic) {
    target_ic->updateUserInterface(fcitx::UserInterfaceComponent::StatusArea, true);
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
    });
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
  if (instance_) {
    toggle_vi_action_.setIcon("clak-tray-vi-on");
    toggle_vi_action_.setShortText(next ? "Tiếng Việt" : "Tiếng Anh");
    toggle_vi_action_.update(instance_->lastFocusedInputContext());
  }
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
  if (instance_) {
    toggle_vi_action_.setIcon("clak-tray-vi-on");
    toggle_vi_action_.setShortText(enabled ? "Tiếng Việt" : "Tiếng Anh");
    toggle_vi_action_.update(instance_->lastFocusedInputContext());
  }
}

static std::string makeQuirkKey(const std::string& app, const std::string& site) {
  if (app.empty()) return "";
  return site.empty() ? app : (app + "|" + site);
}

RuntimeAppQuirks ClakEngine::getAppQuirks(const std::string& app, const std::string& site) const {
  std::string key = makeQuirkKey(app, site);
  if (key.empty()) return {};
  auto it = app_quirks_.find(key);
  if (it != app_quirks_.end()) {
    return it->second;
  }
  return {};
}

void ClakEngine::updateAppQuirks(const std::string& app, const std::string& site, const RuntimeAppQuirks& quirks) {
  std::string key = makeQuirkKey(app, site);
  if (key.empty()) return;
  if (!quirks.is_rich_text && !quirks.is_draftjs && !quirks.is_canvas) {
    app_quirks_.erase(key);
    return;
  }
  if (app_quirks_.size() >= kMaxTrackedAppQuirks && app_quirks_.find(key) == app_quirks_.end()) {
    app_quirks_.clear();
  }
  app_quirks_[key] = quirks;
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
  attachTrayActions(ic);
  auto* state = ic ? ic->propertyFor(&factory_) : nullptr;
  std::string app = state ? state->appKey() : (ic ? ic->program() : "");
  utils::clakLog("engine::activate: ic program='" + (ic ? ic->program() : "") + "' app='" + app + "' enabled=" + std::to_string(isAppEnabled(app)));
  if (checkBinaryReplaced() && !notification_sent_) {
    notification_sent_ = true;
    utils::clakLog("engine: libclak.so updated on disk ((deleted) inode), reload fcitx5 to apply");
    int ret = system("command -v notify-send >/dev/null 2>&1 && notify-send -u normal -a Clak -i org.fcitx.Fcitx5 'Clak có bản cập nhật mới' 'Đã cài đặt bản mới thành công. Hãy khởi động lại Fcitx5 để áp dụng.' &");
    (void)ret;
  }
}

void ClakEngine::deactivate(const fcitx::InputMethodEntry& entry, fcitx::InputContextEvent& event) {
  FCITX_UNUSED(entry);
  auto* ic = event.inputContext();
  utils::clakLog("engine::deactivate: ic program='" + (ic ? ic->program() : "") + "'");
  auto* state = ic ? ic->propertyFor(&factory_) : nullptr;
  if (state) {
    state->cancelRepeat();
    state->onDeactivate();
  }
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
  case 1:
    return "VNI";
  case 2:
    return "VIQR";
  case 3:
    return "TeipVNI";
  default:
    return "Telex";
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
