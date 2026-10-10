# Module Điều hướng Trạng thái Gõ (IME State & Dispatch)

Thư mục: [src/ime/](../src/ime/)

Files:
- [state.h](../src/ime/state.h), [state.cpp](../src/ime/state.cpp): Điều phối chính (orchestrator)
- [repeat_handler.h](../src/ime/repeat_handler.h), [repeat_handler.cpp](../src/ime/repeat_handler.cpp): Mô phỏng lặp phím khi IME giữ phím
- [modal_handler.h](../src/ime/modal_handler.h), [modal_handler.cpp](../src/ime/modal_handler.cpp): Máy trạng thái modal editor (Vim / Helix)
- [surrounding_verifier.h](../src/ime/surrounding_verifier.h), [surrounding_verifier.cpp](../src/ime/surrounding_verifier.cpp): Kiểm tra surrounding text và phát hiện DOM lệch
- [key_buffer.h](../src/ime/key_buffer.h), [key_buffer.cpp](../src/ime/key_buffer.cpp): Hàng đợi FIFO và phát lại phím có gom nhóm (batching)
- [steam_pipeline.h](../src/ime/steam_pipeline.h), [steam_pipeline.cpp](../src/ime/steam_pipeline.cpp): Pipeline cô lập riêng cho Steam Client

Đây là khối điều khiển trung tâm của Clak trên Fcitx5, quản lý toàn bộ vòng đời phím bấm, quyết định sử dụng kênh Wayland SurroundingText, Uinput hay Steam Pipeline, và đảm bảo tính toàn vẹn (correctness) của văn bản hiển thị.

---

## 1. Cấu trúc Mô-đun sau khi Refactor

Nhằm tránh mô hình "God Object" khi mở rộng tính năng, tầng IME được phân tách thành 5 sub-component chuyên biệt:

```text
src/ime/
├── state.h / .cpp              # Điều phối chính Fcitx5 InputContext (orchestrator)
├── repeat_handler.h / .cpp     # Quản lý repeat timer, đọc delay/rate từ Hyprland qua popen
├── modal_handler.h / .cpp      # Máy trạng thái modal editor (NORMAL, INSERT, COMMAND)
├── surrounding_verifier.h/.cpp # Xác thực surrounding text, đếm mismatch, tự chuyển uinput
├── key_buffer.h / .cpp         # Hàng đợi đệm phím khi đang xóa, replay có batching
└── steam_pipeline.h / .cpp     # Xử lý cô lập cho Steam client (dedup, paced backspace)
```

[`ClakState`](../src/ime/state.h) đóng vai trò điều phối trung tâm: nhận sự kiện từ Fcitx5 `InputContext`, gọi FFI sang Rust engine, và phân phối kết quả tới các pipeline chuyên biệt tương ứng.

---

## 2. Cơ chế Quyết định Kênh Xóa (shouldUseUinput)

Trên Wayland, các ứng dụng có cách cài đặt giao thức `zwp_text_input_v3` rất khác nhau:

- **Chromium / Chrome / Brave**: Hỗ trợ `delete_surrounding_text` cực tốt, độ trễ < 1ms. Clak dùng kênh SurroundingText trực tiếp khi gõ bình thường ở cuối câu.
- **WPS Office**: Trên môi trường XWayland, WPS Office bỏ qua lệnh `forwardKey` của DBus. Clak bắt buộc điều hướng sang **Uinput**.
- **Họ VSCode (Antigravity IDE, VSCode, Cursor, Windsurf, VSCodium)**: Terminal tích hợp bên trong chạy bằng `xterm.js`. Thành phần này hoàn toàn không hỗ trợ lệnh `delete_surrounding_text` của Wayland. Clak gom họ VSCode vào nhóm Terminal và chuyển sang **Uinput** với nhịp pacing 15ms/4ms.
- **Gecko (Firefox / Zen Browser)**: Lỗi xóa xung quanh kéo dài nhiều năm trên Wayland (DOM không cập nhật kịp thời, text bị stale). Clak bắt buộc điều hướng sang kênh **Uinput**.
- **Google Docs (Canvas Editor)**: Không dùng DOM HTML thông thường mà vẽ chữ lên HTML5 Canvas, surrounding text chỉ là 2 dấu cách giả lập (`  `). Clak bắt buộc điều hướng sang **Uinput**.
- **Terminal Native (Kitty, Ghostty, Alacritty, Foot)**: Terminal bảo vệ buffer PTY, không hỗ trợ xóa lùi ngữ cảnh Wayland. Clak dùng **Uinput**.
- **Address Bar (Thanh địa chỉ URL)**: Khi xuất hiện gợi ý tự động (autofill), con trỏ bị bôi đen hoặc nhảy về cuối. Clak dùng Uinput kèm thuật toán đếm bù 1 ký tự gợi ý.
- **Steam Client**: Steam chạy CEF qua XWayland trong container Pressure-Vessel. Sự kiện uinput kernel không đi xuyên container một cách đồng bộ. Clak chuyển riêng sang **Steam Pipeline**.

```cpp
bool ClakState::shouldUseUinput(bool use_surrounding, uint32_t action_type, const fcitx::SurroundingText& /*surr*/) {
    if (modal_handler_.isModalEditor()) return true;

    std::string app = appKey();
    std::string site = activeSite();

    if (config::isWpsOfficeApp(app)) return true;
    if (config::isTerminalApp(app)) return true;
    if (config::isGeckoApp(app)) return true;
    if (config::isMetaSite(site) || config::isMetaSite(app)) return false;
    if (config::isSteamApp(app)) return false;
    if (action_type == CLAK_ACTION_ADDRESS_BAR_FIX) return true;
    if (config::isForceUinputSite(site) || isDraftJsEditor()) return true;
    if (verifier_.isCanvasEditor() || verifier_.isRichTextEditor()) return true;
    return !use_surrounding;
}
```

---

## 3. Pipeline Cô lập Riêng cho Steam Client (`SteamPipeline`)

Steam client trên Linux sở hữu kiến trúc đặc thù:
1. Giao diện được xây dựng bằng CEF (Chromium Embedded Framework) chạy trên XWayland.
2. Ứng dụng chạy bên trong container Pressure-Vessel (Steam Runtime), khiến các kernel input event từ `/dev/uinput` bị cô lập hoặc mất đồng bộ.
3. Qua giao thức XIM, khi IME forward một phím in được thô (`forwardKey`), CEF xử lý và dội ngược lại (echo reflection) cùng một mã phím đó về IME sau khoảng 700 microsecond. Nếu không xử lý, hiện tượng nhân đôi ký tự hoặc nuốt phím sẽ xảy ra liên tục.

### Giải pháp kỹ thuật trong [SteamPipeline](../src/ime/steam_pipeline.cpp):

1. **Lọc phản xạ phím trong 2.000us (`isDuplicateKey`)**:
   Khi ở trong Steam, nếu một phím in được giống hệt phím vừa gõ dội ngược lại trong vòng 2.000us, Clak nhận diện đây là echo từ CEF/XIM và lập tức nuốt phím (`keyEvent.filterAndAccept()`).

2. **Commit trực tiếp ký tự in được**:
   Khi action là `CLAK_ACTION_FORWARD` trong Steam, thay vì trả về `false` để Fcitx5 forward phím thô qua X11, Clak gọi `doCommitString(key_str)` và trả về `true` (filter và accept). Điều này giúp đưa ký tự thẳng vào buffer của CEF qua XIM commit, triệt tiêu hoàn toàn race condition giữa X11 event thread và XIM thread.

3. **Hẹn giờ xóa so le (`startStaggeredDeletion`)**:
   Khi cần thay thế âm tiết tiếng Việt trong Steam (ví dụ gõ `d` + `d` -> `đ`):
   - Thay vì bắn nhiều Backspace cùng lúc, Clak phát phím Backspace đầu tiên ngay lập tức (press + release)
   - Nếu còn phím Backspace tiếp theo: hẹn giờ 2ms cho bước xóa kế tiếp
   - Khi đã xóa đủ: hẹn giờ 8ms trước khi commit ký tự mới
   - Tổng thời gian thay thế chỉ mất ~10ms (so với ~45ms trước đây), ngăn chặn hoàn toàn việc lệch hàng đợi khi người dùng gõ phím cực nhanh

4. **Cô lập tuyệt đối**:
   Toàn bộ logic trên chỉ áp dụng khi `isSteam()` trả về `true`. Các ứng dụng khác (trình duyệt, terminal, IDE) hoàn toàn giữ nguyên luồng xử lý native.

---

## 4. Kỹ thuật Phím Chốt Uinput (Sentinel Backspace Protocol)

Khi Clak gửi phím BackSpace qua `/dev/uinput`, sự kiện này đi vào nhân Linux, chuyển qua compositor Hyprland, rồi mới đến ứng dụng đích và vòng ngược lại Fcitx5.

Nếu Clak gửi lệnh `commitString` ngay lập tức, chữ mới sẽ đến ứng dụng **trước** khi các phím BackSpace kịp xóa chữ cũ, gây ra hiện tượng nhân đôi chữ (ví dụ: `d` + commit `đ` = `dđ`).

Để giải quyết bài toán bất đồng bộ này, Clak áp dụng **Sentinel Backspace Protocol**:

1. Giả sử cần xóa $N$ ký tự thật. Clak sẽ phát qua Uinput tổng cộng $N + 1$ phím BackSpace.
2. $N$ phím đầu tiên là phím xóa thật, đến ứng dụng để xóa ký tự trong DOM/buffer.
3. Phím thứ $N + 1$ là **phím chốt (sentinel)**.
4. Khi phím thứ $N + 1$ vòng lặp lại hàm `keyEvent` của Clak:
    - Clak nuốt phím này (`keyEvent.filterAndAccept()`), không cho ứng dụng xóa thêm.
    - Lúc này chắc chắn $N$ ký tự cũ đã được xóa hoàn tất.
    - Clak lập tức gọi `doCommitString(pending_commit_string_)`.

---

## 5. Hàng đợi Đệm và Replay Gom nhóm (`KeyBuffer`)

Trong lúc một chu trình xóa bất đồng bộ đang diễn ra (`is_deleting_ = true`):

- Người dùng gõ phím tiếp theo sẽ không bị chặn hoặc mất phím; các phím này được đưa vào hàng đợi FIFO [KeyBuffer](../src/ime/key_buffer.cpp).
- Sau khi chu trình xóa hoàn tất và ký tự tiếng Việt đã được commit, `replay()` sẽ duyệt lại danh sách phím theo thứ tự gốc:
    - Các ký tự in được liên tiếp không có phím bổ trợ (Ctrl/Alt) được gom vào chuỗi `batch_commit` và gửi một lần qua `doCommitString`, giảm tối đa số lần round-trip IPC qua Wayland.
    - Các phím điều hướng hoặc tổ hợp phím được chuyển tiếp thô nguyên bản (`forwardKey`).

---

## 6. Bộ Đếm An toàn và Co giãn Độ trễ (Safety Timer & Adaptive Latency)

Đề phòng trường hợp ứng dụng đích bị treo hoặc compositor nuốt phím chốt:

- Mỗi khi phát phím Uinput, Clak hẹn giờ `safety_timer_` với thời gian 50ms (hoặc 100ms trên thanh địa chỉ).
- Nếu hết thời gian mà chưa nhận đủ phím BackSpace mong muốn, timer sẽ tự động kích hoạt: giải phóng cờ `is_deleting_`, ép commit ký tự đang chờ, và giải phóng bộ đệm phím.
- **Co giãn độ trễ thích ứng**: Nếu đo được roundtrip latency $\ge 35\text{ms}$, Clak tự động tăng thêm 10ms thời gian an toàn (tối đa 100ms). Khi hệ thống ổn định qua 4 giao dịch nhanh liên tiếp ($\le 20\text{ms}$), Clak tự động hạ dần thời gian chờ về 0.
