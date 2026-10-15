#ifndef CLAK_ENGINE_H
#define CLAK_ENGINE_H

#include <fcitx/action.h>
#include <fcitx/addonfactory.h>
#include <fcitx/addonmanager.h>
#include <fcitx/inputcontextproperty.h>
#include <fcitx/inputmethodengine.h>
#include <fcitx/instance.h>
#include <fcitx/statusarea.h>
#include <fcitx/userinterfacemanager.h>
#include <fcitx-config/configuration.h>
#include <fcitx-config/option.h>
#include "ime/state.h"

namespace clak {
namespace mouse {
class MouseTracker;
}

class ClakSettingsAction : public fcitx::SimpleAction {
public:
    ClakSettingsAction();
    void activate(fcitx::InputContext* ic) override;
};

FCITX_CONFIGURATION(
    ClakFcitxConfig,
    fcitx::ExternalOption settings{this, "settings", "Cài đặt Clak", "clak-gui"};
);

class ClakEngine : public fcitx::InputMethodEngineV2 {
public:
    ClakEngine(fcitx::Instance* instance);
    ~ClakEngine() override;

    void keyEvent(const fcitx::InputMethodEntry& entry, fcitx::KeyEvent& keyEvent) override;
    void activate(const fcitx::InputMethodEntry& entry, fcitx::InputContextEvent& event) override;
    void deactivate(const fcitx::InputMethodEntry& entry, fcitx::InputContextEvent& event) override;
    void reset(const fcitx::InputMethodEntry& entry, fcitx::InputContextEvent& event) override;

    std::string subMode(const fcitx::InputMethodEntry& entry, fcitx::InputContext& ic) override;
    std::string subModeIconImpl(const fcitx::InputMethodEntry& entry, fcitx::InputContext& ic) override;
    std::string subModeLabelImpl(const fcitx::InputMethodEntry& entry, fcitx::InputContext& ic) override;

    const fcitx::Configuration* getConfig() const override { return &fcitx_config_; }

    fcitx::Instance* instance() { return instance_; }

    void loadConfig();
    void setupConfigWatcher();
    const ClakConfig* config() const { return config_; }
    uint64_t configVersion() const { return config_version_; }
    void setConfigForTest(ClakConfig* config) {
        if (config_) {
            clak_config_free(config_);
        }
        config_ = config;
        config_version_++;
    }

    bool isAppEnabled(const std::string& app);
    void toggleAppEnabled(const std::string& app);
    void setAppEnabled(const std::string& app, bool enabled);
    void onMouseClick();
    fcitx::FactoryFor<ime::ClakState>& factory() { return factory_; }
    bool checkBinaryReplaced();
    bool isBinaryReplaced() const { return binary_replaced_; }

private:
    fcitx::Instance* instance_;
    fcitx::FactoryFor<ime::ClakState> factory_;

    ClakConfig* config_{nullptr};
    std::unique_ptr<fcitx::EventSourceIO> config_io_;
    int inotify_fd_{-1};
    int inotify_wd_{-1};
    static constexpr size_t kMaxTrackedAppStates = 256;
    std::unordered_map<std::string, bool> app_states_;
    bool global_enabled_{true};
    uint64_t config_version_{0};
    std::unique_ptr<mouse::MouseTracker> mouse_tracker_;
    ClakSettingsAction settings_action_;
    ClakFcitxConfig fcitx_config_;
    uint64_t last_replacement_check_us_{0};
    bool binary_replaced_{false};
    bool notification_sent_{false};
};

class ClakEngineFactory : public fcitx::AddonFactory {
public:
    fcitx::AddonInstance* create(fcitx::AddonManager* manager) override {
        return new ClakEngine(manager->instance());
    }
};

} // namespace clak

#endif // CLAK_ENGINE_H
