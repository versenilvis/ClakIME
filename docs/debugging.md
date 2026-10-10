# Hướng dẫn Gỡ lỗi và Chẩn đoán (Debugging & Troubleshooting)

Thư mục liên quan: [src/utils/](../src/utils/), [src/ime/](../src/ime/), [diagnostics/](../diagnostics/)

Tài liệu này cung cấp hướng dẫn toàn diện về cách kiểm tra log, phân tích hành vi của bộ gõ, chẩn đoán các lỗi thường gặp (đặc biệt là Steam Client, trình duyệt, terminal), và cách chạy các công cụ kiểm thử tự động.

---

## 1. Hệ thống Log của Clak

Clak ghi nhật ký chi tiết vào file log riêng biệt để không làm ô nhiễm log chung của hệ thống:

- **Đường dẫn file log**: `~/.local/share/fcitx5/clak.log`
- **Cơ chế ghi**: [src/utils/log.cpp](../src/utils/log.cpp) tự động xoay vòng file (log rotation) khi kích thước vượt quá giới hạn (mặc định 2MB, giữ lại file cũ `clak.log.old`)

### Xem log theo thời gian thực

Để theo dõi trực tiếp các sự kiện phím và trạng thái bộ gõ:

```bash
tail -f ~/.local/share/fcitx5/clak.log
```

Để lọc riêng các thông báo về một ứng dụng hoặc tính năng cụ thể:

```bash
# Lọc log cho Steam
tail -f ~/.local/share/fcitx5/clak.log | grep -i steam

# Lọc các giao dịch đo độ trễ (latency)
tail -f ~/.local/share/fcitx5/clak.log | grep "\[LATENCY\]"

# Lọc các sự kiện phát hiện DOM mismatch (Google Docs, Draft.js)
tail -f ~/.local/share/fcitx5/clak.log | grep "verifySurrounding"
```

### Chạy Fcitx5 ở chế độ Debug

Khi cần quan sát toàn bộ log của Fcitx5 và các addon khác trên terminal:

```bash
fcitx5 -r -d
```

---

## 2. Giải mã các mẫu Log quan trọng

### 2.1. Phân phối và xử lý phím (`handleKey`)

```text
handleKey: sym=100 ('d') app=google-chrome site='google.com' -> REPLACE del=1 commit='đ'
```

- `sym`: Mã phím X11 keysym
- `app`: Class hoặc tên tiến trình đang active (lấy từ [appKey](../src/ime/state.h))
- `site`: Domain website trích xuất từ tiêu đề cửa sổ (đối với trình duyệt)
- `action`: Hành động từ Rust engine (`FORWARD`, `COMMIT`, hoặc `REPLACE`)
- `del`: Số ký tự UTF-8 cần xóa lùi
- `commit`: Chuỗi ký tự tiếng Việt cần chèn vào

### 2.2. Giao thức xóa Uinput và Phím chốt Sentinel

```text
uinput waiting for sentinel: expected=3 (real=2 + 1 sentinel) post_delay=2ms gap=2ms commit='tiếng' app=zen site='default'
```

- Khi ứng dụng dùng kênh Uinput (Gecko, terminal, Google Docs), Clak phát $N+1$ Backspace qua `/dev/uinput`
- `expected`: Tổng số phím Backspace cần nhận lại vòng lặp
- `real`: Số phím Backspace thật để xóa chữ cũ
- Phím thứ $N+1$ là phím chốt (sentinel). Khi phím này quay lại qua `keyEvent`, Clak nuốt nó và lập tức chèn `commit`

### 2.3. Hết giờ chờ phím chốt (`SENTINEL TIMEOUT`)

```text
SENTINEL TIMEOUT (50ms expired): giving up waiting, only 1/3 BS returned! force committing 'tiếng' app=zen site='default'
```

- Nếu ứng dụng đích phản hồi chậm hoặc compositor nuốt phím Backspace, timer an toàn (`safety_timer_`) sẽ kích hoạt
- Clak tự động ép commit ký tự đang chờ để không làm mất chữ của người dùng, đồng thời tăng thời gian chờ thích ứng (`adaptive_extra_us_`) thêm 20ms

### 2.4. Xác thực Surrounding Text và Tự động nhận diện DOM lệch

```text
verifySurrounding mismatch #1: expected='tiếng' actual='tien'
verifySurrounding: detected draftjs / react contenteditable editor, switched to uinput
```

- [SurroundingVerifier](../src/ime/surrounding_verifier.cpp) kiểm tra từ trước con trỏ sau khi xóa
- Nếu DOM của trang web (như Draft.js trên Facebook/Twitter hoặc React ContentEditable) không cập nhật đúng, Clak tự động chuyển nhánh sang Uinput để bảo toàn tính toàn vẹn của văn bản

### 2.5. Đo lường độ trễ giao dịch (`[LATENCY]`)

```text
[LATENCY] group=Chromium-SurroundingText delta_ms=0.342000 action=REPLACE
[LATENCY] group=Steam-ForwardKey delta_ms=8.450000 action=REPLACE
```

- Báo cáo thời gian tính từ lúc bắt đầu nhận keydown cho đến khi hoàn thành commit chuỗi ký tự mới

---

## 3. Chẩn đoán và xử lý sự cố Steam Client

### 3.1. Đặc thù kỹ thuật của Steam trên Linux

Steam client sử dụng giao diện Chromium Embedded Framework (CEF) chạy qua XWayland. Khi chạy trên Linux, Steam thường chạy bên trong container Pressure-Vessel (Steam Runtime). Điều này dẫn đến các đặc thù:

1. **Phản xạ phím dội ngược (Key Reflection)**: Khi Fcitx5 chuyển tiếp phím in được thô (`forwardKey`), CEF qua XIM thường dội ngược lại cùng mã phím đó sau khoảng 600us đến 1.500us
2. **Không dùng được Uinput**: Kernel input phát qua `/dev/uinput` không đi xuyên qua được container Pressure-Vessel một cách đồng bộ
3. **Xung đột xóa nhanh**: Gửi liên tiếp nhiều phím Backspace cùng lúc qua XIM khiến CEF xử lý lệch thứ tự so với luồng commit

### 3.2. Cơ chế giải quyết trong Clak

Clak cô lập hoàn toàn xử lý Steam trong [SteamPipeline](../src/ime/steam_pipeline.cpp) và [state.cpp](../src/ime/state.cpp):

- **Commit trực tiếp ký tự in được**: Đối với phím in được (`sym < 0xff00`), Clak commit trực tiếp qua `doCommitString` và filter phím thay vì forward thô, loại bỏ hoàn toàn hiện tượng dội phím
- **Lọc phím trùng lặp 2.000us**: `steam_pipeline_.isDuplicateKey()` tự động phát hiện và nuốt các phím phản xạ dội ngược trong cửa sổ 2.000 microsecond
- **Hẹn giờ xóa so le (Staggered Deletion)**: Mỗi phím Backspace được gửi cách nhau 2ms, và chuỗi tiếng Việt mới được commit sau 8ms, sau đó lập tức phát lại các phím đã đệm trong hàng đợi FIFO

### 3.3. Kiểm tra môi trường Steam

Để Steam nhận diện đúng bộ gõ tiếng Việt, các biến môi trường sau cần được thiết lập trước khi Steam khởi động:

```bash
export XMODIFIERS=@im=fcitx
export GTK_IM_MODULE=
export QT_IM_MODULE=fcitx
export SDL_IM_MODULE=fcitx
```

> [!NOTE]
> Tuyệt đối không đặt `GTK_IM_MODULE=fcitx` vì biến này sẽ ép các ứng dụng GTK dùng module cũ và gây lỗi không gõ được trên nhiều môi trường Wayland.

Các script cài đặt [install.sh](../scripts/install.sh) và cập nhật [update.sh](../scripts/update.sh) của Clak tự động cấu hình các lớp bảo vệ sau:

1. **File cấu hình môi trường**: `~/.config/environment.d/99-clak-im.conf`
2. **Desktop entry override**: `~/.local/share/applications/steam.desktop` (ghi đè Exec để nạp biến môi trường Fcitx5 mà không sửa file hệ thống)
3. **Binary wrapper**: `~/.local/bin/steam` (cho người dùng khởi động Steam từ terminal)
4. **Flatpak override** (nếu dùng Steam Flatpak):
   ```bash
   flatpak override --user --env=XMODIFIERS=@im=fcitx com.valvesoftware.Steam
   flatpak override --user --env=QT_IM_MODULE=fcitx com.valvesoftware.Steam
   ```

---

## 4. Kiểm tra quyền hạn `/dev/uinput`

Nếu bạn gặp lỗi không gõ được tiếng Việt trong terminal, Google Docs hoặc Zen Browser:

```bash
# Kiểm tra quyền truy cập /dev/uinput
ls -la /dev/uinput
```

Kết quả chuẩn phải cho phép user thông thường hoặc nhóm `input` đọc và ghi:

```text
crw-rw---- 1 root input 10, 223 /dev/uinput
```

Nếu quyền là `crw------- 1 root root`:
1. Đảm bảo user của bạn nằm trong nhóm `input`:
   ```bash
   sudo usermod -aG input $USER
   ```
2. Cài đặt udev rule từ repo:
   ```bash
   sudo cp data/99-uinput-clak.rules /etc/udev/rules.d/
   sudo udevadm control --reload-rules && sudo udevadm trigger
   ```
3. Đảm bảo kernel module `uinput` đã được nạp:
   ```bash
   sudo modprobe uinput
   ```

---

## 5. Chạy Công cụ Chẩn đoán và Bộ Kiểm thử

### 5.1. Công cụ chẩn đoán tự động (`clak doctor`)

Clak tích hợp sẵn công cụ chẩn đoán môi trường trong crate [diagnostics/](../diagnostics/):

```bash
# Chạy chẩn đoán qua CLI
cargo run --manifest-path cli/Cargo.toml -- doctor
```

Công cụ sẽ kiểm tra:
- Trạng thái Fcitx5 daemon đang chạy
- Quyền hạn file `/dev/uinput`
- Các biến môi trường `XMODIFIERS`, `QT_IM_MODULE`, `GTK_IM_MODULE`
- Module Clak addon đã được cài đặt vào `~/.local/lib/fcitx5/` hoặc thư mục hệ thống chưa

### 5.2. Chạy bộ kiểm thử C++

```bash
# Build và chạy toàn bộ 30 bài test C++
cmake --build build --target clak_cpp_tests && ./build/src/tests/clak_cpp_tests

# Chỉ chạy các test liên quan đến Steam
./build/src/tests/clak_cpp_tests --gtest_filter="ClakStateTest.GivenSteamApp*"

# Chỉ chạy các test về uinput và pacing
./build/src/tests/clak_cpp_tests --gtest_filter="ClakStateTest.GivenGeckoApp*:ClakStateTest.GivenSentinel*"
```

### 5.3. Chạy bộ kiểm thử toàn diện (`just check`)

Lệnh chuẩn hóa kiểm tra tất cả các tầng trước khi commit hoặc push:

```bash
just check
```

Bao gồm:
1. `cargo fmt --check` trên engine Rust
2. `cargo clippy` kiểm tra cảnh báo và chất lượng code
3. Kiểm thử đơn vị lõi Rust
4. Biên dịch và chạy toàn bộ bài test C++
5. Build hoàn chỉnh thư viện release và các binary GUI / CLI
