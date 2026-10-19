#ifndef CLAK_UINPUT_UINPUT_H
#define CLAK_UINPUT_UINPUT_H

#include <cstddef>
#include <cstdint>
#include <mutex>
#include <atomic>
#include <ctime>
#include <functional>

namespace clak {
namespace uinput {

inline uint64_t monotonic_now_us() {
    struct timespec ts{};
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return static_cast<uint64_t>(ts.tv_sec) * 1000000ULL +
           static_cast<uint64_t>(ts.tv_nsec) / 1000ULL;
}

using BackspaceHook = std::function<bool(size_t count, uint32_t post_delay_ms, uint32_t pre_delay_ms, uint32_t gap_ms)>;

class UinputTool {
public:
    static UinputTool& instance();

    bool send_backspace(size_t count, uint32_t post_delay_ms = 0, uint32_t pre_delay_ms = 0, uint32_t gap_ms = 0);
    bool send_select(size_t count);

    uint64_t consume_sentinel_emit_time_us() noexcept {
        return sentinel_emit_time_us_.exchange(0, std::memory_order_acq_rel);
    }

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
    std::atomic<uint64_t> sentinel_emit_time_us_{0};
    void* xcb_lib_{nullptr};
    void* xtest_lib_{nullptr};
    void* xcb_conn_{nullptr};
    std::mutex xtest_mutex_;
    BackspaceHook mock_handler_;
};

} // namespace uinput
} // namespace clak

#endif
