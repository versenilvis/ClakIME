# Module Nền tảng và Nhận diện Ứng dụng (Platform & Window Info)

Thư mục: [src/de/](../src/de/), [src/window/](../src/window/), [src/config/](../src/config/)

Các file chính: [hyprland.cpp](../src/de/hyprland.cpp), [kde.cpp](../src/de/kde.cpp), [gnome.cpp](../src/de/gnome.cpp), [window_tracker.cpp](../src/window/window_tracker.cpp), [window_info.h](../src/window/window_info.h), [sites.cpp](../src/config/sites.cpp)

Module này thu thập ngữ cảnh của ứng dụng đang được người dùng tương tác để Clak tự động cấu hình hành vi thích hợp mà không cần người dùng phải bấm phím chuyển chế độ thủ công

---

## 1. Giao tiếp Socket IPC với Hyprland

Trên Wayland, vì lý do bảo mật, ứng dụng thông thường không thể tự do truy vấn thông tin các cửa sổ khác. Tuy nhiên compositor Hyprland cung cấp giao tiếp UNIX domain socket tại:

`$XDG_RUNTIME_DIR/hypr/$HYPRLAND_INSTANCE_SIGNATURE/.socket.sock`

Trong file [hyprland.cpp](../src/de/hyprland.cpp):

1. Clak mở kết nối Non-blocking Socket tới socket của Hyprland
2. Gửi lệnh ngắn `j/activewindow`
3. Dùng `poll` với timeout cực nhanh (15ms) để nhận JSON phản hồi chứa `class`, `title`, và `pid`
4. **Bộ nhớ đệm (Cache 500ms)**: Kết quả truy vấn được lưu lại trong 500ms. Trong thời gian này, mọi phím gõ liên tiếp đều dùng lại kết quả cũ, giảm tải 100% việc tạo socket lặp đi lặp lại

---

## 2. Tích hợp Môi trường KDE Plasma

Trong file [kde.cpp](../src/de/kde.cpp):

1. **Nhận diện phiên làm việc**: Kiểm tra biến môi trường `XDG_CURRENT_DESKTOP` (chứa `KDE` hoặc `Plasma`) hoặc `KDE_FULL_SESSION=true`
2. **Cấu hình lặp phím tự động**: Phân tích trực tiếp file `~/.config/kcminputrc` trong nhóm `[Keyboard]` để lấy `RepeatDelay` và `RepeatRate`, kết hợp fallback qua `kreadconfig6` hoặc `kreadconfig5`
3. **Lọc Desktop Shell và Overlays**: Bỏ qua các thành phần giao diện nền như `plasmashell`, `krunner`, `kscreenlocker_greet`, `plasma-desktop`, `polkit-kde` để tránh gán nhầm làm ứng dụng nhập liệu
4. **Tích hợp KWin VirtualKeyboard**: Hỗ trợ chẩn đoán và tự động gán `InputMethod` trong `kwinrc` trỏ tới Fcitx5, kèm lệnh reload qua DBus để áp dụng ngay lập tức

---

## 3. Nhận diện Domain Website và Nhóm Ứng dụng

File [sites.cpp](../src/config/sites.cpp) phân tích tiêu đề cửa sổ trình duyệt để trích xuất domain đang hoạt động:

- **Họ Gecko (isGeckoApp)**: Firefox, Zen Browser, Librewolf, Floorp, Waterfox...
- **Họ Chromium (isChromiumApp)**: Chrome, Chromium, Brave, Helium, Edge, Vivaldi, Thorium...
- **Họ Terminal (isTerminalApp)**: Kitty, Ghostty, Alacritty, Foot, Wezterm, Konsole, Yakuake, GNOME Terminal...
- **Dịch vụ buộc dùng Uinput (isForceUinputSite)**: Google Docs, Google Sheets, Google Slides, TikTok, X (Twitter)...
- **Dịch vụ Meta (isMetaSite)**: Facebook, Messenger, Instagram, Threads, WhatsApp (dùng cơ chế SurroundingText tiêu chuẩn để tránh lỗi con trỏ văn bản của Draft.js)

---

## 4. Nhận diện Trình soạn thảo Modal (Modal Editors)

File [modal_handler.cpp](../src/ime/modal_handler.cpp):

Khi người dùng làm việc trong Neovim, Vim, Kakoune, hoặc Helix (chạy trong Terminal hoặc bản GUI như Neovide):

- Clak theo dõi trạng thái biên tập: `NORMAL`, `INSERT`, `COMMAND`
- Khi ở chế độ `NORMAL`:
    - Các phím lệnh như `h`, `j`, `k`, `l`, `d`, `y`, `w` được truyền thẳng ra ngoài mà không bao giờ bị biến đổi thành ký tự có dấu
    - Các phím chuyển chế độ như `i`, `a`, `o`, `c`, `s` chuyển trạng thái sang `INSERT` và reset engine để chuẩn bị gõ tiếng Việt mượt mà
- Khi người dùng nhấn `Escape` hoặc `Ctrl+[`: Clak lập tức quay về trạng thái `NORMAL` và dọn sạch bộ đệm gõ
