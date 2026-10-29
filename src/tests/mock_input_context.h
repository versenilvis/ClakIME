#ifndef CLAK_TESTS_MOCK_INPUT_CONTEXT_H
#define CLAK_TESTS_MOCK_INPUT_CONTEXT_H

#include <fcitx/inputcontext.h>
#include <fcitx/inputcontextmanager.h>
#include <fcitx-utils/key.h>
#include <string>
#include <vector>
#include <utility>
#include <functional>

namespace clak {
namespace ime {
class ClakState;
}

namespace test {

class MockInputContext : public fcitx::InputContext {
public:
    MockInputContext(fcitx::InputContextManager& mgr, const std::string& program = "test-app");
    ~MockInputContext() override;

    const char* frontend() const override { return frontend_.c_str(); }
    void setFrontend(const std::string& f) { frontend_ = f; }

    void commitStringImpl(const std::string& text) override;
    void deleteSurroundingTextImpl(int offset, unsigned int size) override;
    void forwardKeyImpl(const fcitx::ForwardKeyEvent& key) override;
    void updatePreeditImpl() override;

    // surrounding text and capability controls
    void setSurrounding(const std::string& text, unsigned int cursor, unsigned int anchor = 0);
    void invalidateSurrounding();
    void setSurroundingEnabled(bool enabled);
    void setUrlCapability(bool enabled);

    // key event dispatch helpers
    bool sendKey(fcitx::KeySym sym, fcitx::KeyStates states = fcitx::KeyStates(), bool isRelease = false, ime::ClakState* state = nullptr);
    bool typeChar(char c, ime::ClakState* state = nullptr);
    bool typeString(const std::string& str, ime::ClakState* state = nullptr);
    bool sendCleanBackspace(ime::ClakState* state = nullptr);

    // history tracking
    std::vector<std::string> commits;
    std::vector<std::pair<int, unsigned int>> deletions;
    std::vector<fcitx::Key> forwarded_keys;

    // optional hook overrides for test interception
    std::function<void(const std::string&)> on_commit;
    std::function<void(int, unsigned int)> on_delete;
    std::function<void(const fcitx::ForwardKeyEvent&)> on_forward_key;

    // auto-update internal surrounding text buffer on commit/delete
    bool auto_update_surrounding{false};
    std::string frontend_{"mock"};

    void clearHistory();
};

} // namespace test
} // namespace clak

#endif
