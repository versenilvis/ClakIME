# Tài liệu kỹ thuật bộ gõ Clak

> [!IMPORTANT]
> **Tài liệu này được tạo ra bởi AI**

Clak là bộ gõ tiếng Việt hiệu năng cao dành cho Linux / Wayland, được xây dựng như một plugin (addon) cho hệ thống Fcitx5 kết hợp cùng bộ máy xử lý Telex/VNI viết bằng Rust.

Bộ gõ giải quyết dứt điểm các bài toán cố hữu của Wayland như: gõ trên thanh địa chỉ trình duyệt bị nuốt ký tự hoặc nhân đôi `đ`, gõ trên các ứng dụng nền Gecko (Zen Browser, Firefox) bị xung đột bộ đệm, giật lag trên Google Docs, cũng như nhận diện thông minh chế độ Normal/Insert trong các trình soạn thảo modal như Neovim và Helix.

---

## 1. Cấu trúc thư mục

```text
input-method/
├── CMakeLists.txt              # Cấu hình build C++ addon cho Fcitx5
├── justfile                    # Phím tắt tác vụ (build, deploy, test)
├── engine/                     # Lõi xử lý tiếng Việt bằng Rust
│   ├── Cargo.toml
│   ├── tests/                  # Bộ kiểm thử tích hợp và bất biến (proptest)
│   └── src/
│       ├── lib.rs              # C-FFI exports (clak_context_new, clak_process_key...)
│       ├── engine.rs           # Máy trạng thái Telex/VNI
│       ├── spelling.rs         # Quy tắc ghép âm tiếng Việt và từ điển
│       ├── charset.rs          # Bảng mã (Unicode, VNI, TCVN3...)
│       └── ime.rs              # Mô phỏng tích hợp xung quanh văn bản
├── src/                        # Tầng kết nối Fcitx5 bằng C++
│   ├── engine.h / .cpp         # Điểm khởi động Fcitx5 addon
│   ├── ime/
│   │   ├── state.h / .cpp      # Điều phối chính Fcitx5 InputContext (orchestrator)
│   │   ├── repeat_handler.h / .cpp     # Quản lý repeat timer và đọc cấu hình Hyprland
│   │   ├── modal_handler.h / .cpp      # Quản lý trạng thái modal editor (Vim/Helix)
│   │   ├── surrounding_verifier.h/.cpp # Xác thực surrounding text và phát hiện DOM desync
│   │   ├── key_buffer.h / .cpp         # Hàng đợi đệm phím và replay có batching
│   │   └── steam_pipeline.h / .cpp     # Pipeline cô lập cho Steam client (XIM/CEF)
│   ├── uinput/
│   │   ├── uinput.h / .cpp     # Phát phím phần cứng ảo qua /dev/uinput Linux
│   ├── platform/
│   │   ├── hyprland.cpp        # Giao tiếp IPC socket với compositor Hyprland
│   │   ├── modal_editor.cpp    # Nhận diện Vim/Neovim/Helix và tự động chuyển chế độ
│   │   └── window_info.h       # Cấu trúc thông tin cửa sổ đang active
│   ├── config/
│   │   ├── config.h            # Các hằng số thời gian và ngưỡng an toàn
│   │   └── sites.h / .cpp      # Nhận diện nhóm ứng dụng và domain website
│   ├── utils/
│   │   ├── log.h / .cpp        # Ghi log gỡ lỗi kèm xoay vòng dung lượng
│   │   └── text_utils.h / .cpp # Xử lý chuỗi UTF-8, đếm ký tự, tách từ
│   └── tests/                  # Bộ kiểm thử C++ state machine và regression
├── scripts/
│   └── tests/                  # Bộ script kiểm thử tự động và đo latency
└── docs/                       # Tài liệu chi tiết từng module
```

---

## 2. Kiến trúc và luồng xử lý dữ liệu

```text
Fcitx5 keyEvent
       │
       ▼
 ┌───────────────┐  Có (Vim/Helix NORMAL)
 │ Modal editor? ├─────────────────────────► Forward phím thô (không gõ dấu)
 └───────┬───────┘
         │ Không (INSERT / ứng dụng thông thường)
         ▼
 ┌───────────────┐
 │ Rust engine   ├─────────────────────────► FFI clak_process_key()
 └───────┬───────┘
         │
         ▼
   ClakAction
   ├── FORWARD               ──► Chuyển tiếp phím gốc cho frontend
   ├── COMMIT                ──► Chèn chuỗi ký tự trực tiếp (doCommitString)
   └── REPLACE / SURROUNDING ──► Cần xóa N ký tự và chèn chuỗi tiếng Việt mới
               │
               ├────────────────────────┬────────────────────────┬────────────────────────┐
               ▼                        ▼                        ▼                        ▼
       SurroundingText            Uinput Pacing          Address bar fix       Steam Pipeline
       (Chromium, Brave, web)    (Gecko, Docs, term)   (Omnibox autocomplete)  (Steam CEF / XIM)
       deleteSurroundingText     N+1 sentinel BS        N+1 BS + 1 BS autofill  2ms staggered BS
       + doCommitString          + loopback check       + doCommitString        + 8ms commit
```

### Các bước xử lý tuần tự

1. **Fcitx5 event loop**: Bắt sự kiện phím bấm trong [state.cpp](../src/ime/state.cpp) `keyEvent`.
2. **Kiểm tra modal editor**: Nếu đang ở trong Vim/Neovim/Helix ở chế độ NORMAL, phím được chuyển thẳng (forward) không qua gõ dấu.
3. **Rust engine FFI**: Gọi [clak_process_key](../engine/src/lib.rs) kèm văn bản ngữ cảnh xung quanh (surrounding text).
4. **Phân nhánh thực thi action**:
    - `FORWARD`: Không biến đổi, chuyển tiếp phím gốc.
    - `COMMIT`: Nhận diện từ hoàn chỉnh hoặc ký tự đặc biệt, chèn chuỗi ký tự qua Fcitx5.
    - `REPLACE`: Cần xóa $N$ ký tự trước con trỏ và thay thế bằng âm tiết tiếng Việt mới.
5. **Điều hướng kênh xóa văn bản**:
    - **Kênh SurroundingText**: Áp dụng cho các ô nhập liệu tiêu chuẩn trên Chromium, Brave, mạng xã hội Facebook/Messenger qua `deleteSurroundingText`.
    - **Kênh Uinput Pacing**: Áp dụng cho Gecko (Zen Browser, Firefox), Google Docs canvas, terminal, và thanh địa chỉ (Omnibox).
    - **Kênh Steam Pipeline**: Áp dụng riêng cho Steam Client qua hẹn giờ xóa so le 2ms/8ms và lọc phím dội ngược 2.000us.

---

## 3. Phân nhánh kênh thực thi

| Kênh thực thi | Ứng dụng áp dụng | Cơ chế hoạt động | Ưu điểm và bảo vệ |
| --- | --- | --- | --- |
| **SurroundingText** | Chromium, Chrome, Brave, Facebook, Messenger | Gọi trực tiếp `deleteSurroundingText()` của Wayland rồi commit | Độ trễ cực thấp (<0.3ms), bảo toàn cấu trúc DOM và vùng chọn |
| **Uinput Sentinel Pacing** | Gecko (Zen Browser, Firefox), Google Docs, terminal | Phát $N+1$ Backspace qua `/dev/uinput`, chờ phím chốt thứ $N+1$ loopback | Khắc phục triệt để lỗi nuốt phím và xung đột bộ đệm của Gecko/canvas |
| **Address Bar Fix** | Thanh địa chỉ Chromium / Brave khi có autocomplete | Phát $N + 1 + 1$ Backspace (thêm 1 Backspace xóa inline autocomplete) | Ngăn mất ký tự đầu hoặc kẹt chuỗi `dđ` khi trình duyệt tự gợi ý URL |
| **Steam Pipeline** | Steam Client (Discussion, Store, Chat, search box) | Commit trực tiếp phím in được, xóa so le 2ms, commit sau 8ms, dedup 2.000us | Loại bỏ hoàn toàn dội phím XIM và kẹt chuỗi gõ nhanh trong container Pressure-Vessel |
| **Modal Editor Bypass** | Neovim, Helix, Vim (trong Kitty, Alacritty, WezTerm...) | Đọc IPC Hyprland socket và kiểm tra chế độ NORMAL/INSERT | Không làm phiền khi gõ lệnh Vim, tự bật lại bộ gõ khi vào INSERT |

---

## 4. Các bất biến hệ thống (Invariants)

- **Hàng đợi FIFO khi đang xóa**: Trong lúc uinput đang phát phím xóa (`is_deleting_ = true`), các phím người dùng gõ tiếp theo được đưa vào hàng đợi `buffered_keys_` và phát lại đúng thứ tự sau khi commit hoàn tất.
- **Không block main thread**: Tuyệt đối không dùng `sleep()` trong thread chính của Fcitx5; toàn bộ nhịp pacing, delay và safety timer đều dùng `sd-event` loop bất đồng bộ.
- **Phím chốt sentinel $N+1$**: Luôn gửi $N+1$ phím Backspace khi xóa qua uinput; phím thứ $N+1$ được filter để xác nhận chắc chắn $N$ ký tự cũ đã bị xóa trước khi commit ký tự mới.
- **Bộ đếm an toàn và co giãn độ trễ**: Hẹn giờ an toàn 50ms (hoặc 100ms trên thanh địa chỉ). Nếu trễ (>35ms), hệ thống tự nâng thời gian chờ an toàn (+10ms) và tự giảm (-10ms) khi ổn định trở lại.

---

## 5. Tùy chọn cấu hình nhập liệu

| Tùy chọn | Mặc định | Hành vi khi kích hoạt |
| --- | --- | --- |
| `auto_capitalize` | `true` | Tự động viết hoa chữ cái đầu câu sau `. `, `? `, `! `, Enter hoặc đầu văn bản |
| `double_space_period` | `true` | Nhấn 2 lần Space liên tiếp sẽ tự động chuyển thành `. ` |
| `macro_expansion` | `true` | Mở rộng bảng gõ tắt (khớp hoa/thường) khi nhấn Space, Enter hoặc Tab |
| `auto_restore` | `true` | Tự phục hồi từ tiếng Anh khi từ gõ vào vi phạm quy tắc chính tả tiếng Việt |
| `modern_tone` | `true` | Đặt dấu thanh kiểu mới (`hoá`, `oà`, `uý`) thay vì kiểu cũ (`hóa`, `òa`, `úy`) |

---

## 6. Giao diện FFI C/Rust

Tầng C++ của Fcitx5 giao tiếp với lõi Rust engine qua FFI ngoại vi [engine/src/lib.rs](../engine/src/lib.rs):

```c
ClakContext* clak_context_new(uint32_t method);
void clak_context_free(ClakContext* ctx);
void clak_context_reset(ClakContext* ctx);

ClakAction clak_process_key(
    ClakContext* ctx,
    uint32_t key_sym,
    const char* key_str,
    bool has_ctrl_alt,
    const char* surrounding_text,
    size_t cursor_pos,
    size_t anchor_pos
);
```

### Cấu trúc ClakAction

- `action_type`:
    - `0 (CLAK_ACTION_FORWARD)`: Phím thô không đổi, chuyển tiếp cho ứng dụng.
    - `1 (CLAK_ACTION_COMMIT)`: Chèn chuỗi ký tự mới mà không cần lùi xóa.
    - `2 (CLAK_ACTION_REPLACE)`: Xóa `delete_count` ký tự trước con trỏ và chèn `commit_str`.
    - `3 (CLAK_ACTION_REPLACE_SURROUNDING)`: Thay thế dựa trên văn bản ngữ cảnh xung quanh.
    - `4 (CLAK_ACTION_ADDRESS_BAR_FIX)`: Tín hiệu xóa bù cho thanh địa chỉ trình duyệt.
- `delete_count`: Số ký tự UTF-8 cần xóa lùi.
- `commit_str`: Chuỗi tiếng Việt cần chèn vào.

---

## 7. Tài liệu chi tiết từng phần

Mỗi module được giải thích cặn kẽ trong các tài liệu sau:

- [Lõi xử lý ngôn ngữ Rust](./engine.md): Cơ chế Telex, quy tắc đặt dấu, kiểm tra ngữ pháp tiếng Việt và giao tiếp C-FFI.
- [Quản lý trạng thái và điều hướng phím](./ime-state-dispatch.md): Thuật toán chọn kênh Surrounding vs Uinput, cơ chế phím chốt sentinel, bộ đệm phím, và bộ đếm an toàn.
- [Môi trường cửa sổ và nhận diện ứng dụng](./platform-window.md): Giao tiếp IPC socket với Hyprland, nhận diện domain web, và chuyển đổi trạng thái Vim.
- [Bộ phát phím ảo và nhịp thời gian uinput](./uinput-pacing.md): Trình điều khiển `/dev/uinput`, kỹ thuật pacing với post_delay và gap_ms để trình duyệt không bị nuốt phím.
- [Đo đạc và tối ưu độ trễ](./benchmark-latency.md): Phương pháp đo latency từ lúc nhận keydown đến khi commit, bảng số liệu p50/p95/p99 của 5 nhóm ứng dụng.
- [Hệ thống kiểm thử tự động](./test.md): Danh mục kiểm thử Rust engine, C++ state machine, kịch bản phòng ngừa lỗi và hướng dẫn chạy test.
- [Hướng dẫn gỡ lỗi và chẩn đoán (Debugging)](./debugging.md): Kiểm tra log, phân tích hành vi bộ gõ, giải quyết lỗi Steam Client, quyền uinput và công cụ chẩn đoán.
- [Đặc tả giao diện lập trình (API Reference)](./api.md): Danh mục hàm C-FFI, cấu trúc ImeAction, bảng mã ký tự và quy tắc quản lý bộ nhớ.
