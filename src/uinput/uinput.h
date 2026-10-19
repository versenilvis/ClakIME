#ifndef CLAK_UINPUT_UINPUT_H
#define CLAK_UINPUT_UINPUT_H

#include <cstddef>
#include <cstdint>
#include <mutex>

#include <functional>

namespace clak {
namespace uinput {

using BackspaceHook = std::function<bool(size_t count, uint32_t post_delay_ms, uint32_t pre_delay_ms, uint32_t gap_ms)>;

class UinputTool {
public:
    static UinputTool& instance();

    bool send_backspace(size_t count, uint32_t post_delay_ms = 0, uint32_t pre_delay_ms = 0, uint32_t gap_ms = 0);
    bool send_select(size_t count);

    void setMockHandler(BackspaceHook handler) { mock_handler_ = std::move(handler); }
    void clearMockHandler() { mock_handler_ = nullptr; }

private:
    UinputTool();
    ~UinputTool();

    bool init_direct_uinput();
    bool send_backspace_direct(size_t count, uint32_t post_delay_ms, uint32_t pre_delay_ms, uint32_t gap_ms);
    bool send_select_direct(size_t count);

    bool init_xtest();
    bool send_backspace_xtest(size_t count, uint32_t post_delay_ms, uint32_t pre_delay_ms, uint32_t gap_ms);
    bool send_select_xtest(size_t count);

    int direct_fd_{-1};
    std::mutex uinput_mutex_;
    void* xcb_lib_{nullptr};
    void* xtest_lib_{nullptr};
    void* xcb_conn_{nullptr};
    std::mutex xtest_mutex_;
    BackspaceHook mock_handler_;
};

} // namespace uinput
} // namespace clak

#endif
