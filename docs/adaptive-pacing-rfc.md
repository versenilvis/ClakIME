# RFC: Cơ Chế Tự Thích Ứng Nhịp Pacing (Adaptive Uinput Pacing)

Trạng thái: Dự thảo thiết kế (Draft)
Tác giả: Clak Core Team
Mục tiêu: Đánh giá tính khả thi, phân tích các điểm mù kỹ thuật và phác thảo kiến trúc điều khiển độ trễ Uinput tự động.

---

## 1. Đặt Vấn Đề Và Bối Cảnh

Trên Linux Wayland, các ứng dụng có cách xử lý phím và chu kỳ hoàn tất xóa/chèn ký tự rất phân mảnh:

- **Môi trường nhẹ, tần số quét cao (Hyprland / KDE 144Hz-240Hz, CPU AMD Zen 4)**: Luồng xử lý phím đáp ứng ngay lập tức (< 1-2ms), yêu cầu độ trễ cực thấp để tránh hiện tượng xóa đi vẽ lại gây nháy chữ (flicker).
- **Môi trường đệm VSync, tần số quét 60Hz (Ubuntu GNOME 46 Mutter, CPU Intel big.LITTLE P+E cores)**: Luồng compositor có độ trễ Triple Buffering (16ms-33ms), nhân E-core xung thấp (~1GHz) xử lý DOM React (DraftJS trên Twitter/X) chậm, đòi hỏi khoảng giãn cách (gap/post_delay) đủ dài để không bị nuốt phím hoặc rơi phím sentinel.

Ý tưởng tự động dò tìm độ trễ tối ưu cho từng máy là cần thiết, nhưng bản thiết kế sơ bộ trước đây tồn tại bốn lỗ hổng cốt lõi. Tài liệu này mổ xẻ chi tiết từng điểm và đề xuất kiến trúc xử lý chuẩn xác.

---

## 2. Điểm Mù 1: Khả Năng Quan Sát Bug Trên DraftJS (Observability Blindspot)

### 2.1 Bản chất vấn đề

Trước đây ta giả định rằng: khi DraftJS bị lỗi, Clak sẽ nhận được tín hiệu `SENTINEL TIMEOUT` hoặc `mismatchCount`. **Giả định này không đúng trên thực tế.**

Khi Twitter/X (DraftJS) nuốt chữ:

1. Clak gửi đủ số phím Backspace qua `/dev/uinput`.
2. Phím Sentinel Backspace phát đi và quay trở lại Fcitx5 đúng hạn.
3. Sentinel hoàn toàn không bị timeout, số lượng backspace nhận về khớp 100%.
4. Nhưng ở tầng Chromium và React DOM: sự kiện `beforeinput` / `keydown` bị hoãn trong hàng đợi JavaScript của DraftJS. Khi lệnh commit ký tự mới đến, React đang cập nhật state dở dang nên bỏ qua ký tự hoặc làm sập vùng chọn (selection collapse về 0).

Kết quả: **Chữ bị mất trên màn hình, nhưng Clak ghi nhận giao dịch thành công 100%. Không có timeout hay mismatch nào được kích hoạt.**

### 2.2 Nghịch lý vòng lặp Surrounding Text

Cách duy nhất để engine biết chữ trên màn hình có đúng hay không là đọc lại `ic_->surroundingText()` sau khi commit và so sánh với chuỗi mong đợi.

Tuy nhiên, đây là một nghịch lý:

- Ta ép `x.com` dùng Uinput chính vì `surroundingText` trên trang này không đáng tin cậy (Chromium thường xuyên trả về text cũ, cursor = 0, hoặc không cập nhật kịp thời).
- Nếu dùng chính nguồn dữ liệu không đáng tin cậy đó để làm căn cứ đánh giá lỗi, thuật toán tự cân chỉnh sẽ bị "ảo giác" (nhận định sai lệch liên tục).

### 2.3 Kế hoạch kiểm chứng thực nghiệm bắt buộc

Trước khi viết bất kỳ dòng code auto-delay nào cho DraftJS, ta phải thực hiện kiểm chứng:

1. Bật log chẩn đoán trên `x.com`.
2. Gõ các cụm từ Telex có dấu và đọc giá trị `surroundingText` ngay sau commit.
3. Đối chiếu xem text mà Fcitx5 đọc được có phản ánh đúng 100% những gì hiển thị trên màn hình hay không.
4. **Nguyên tắc quyết định**:
    - Nếu `surroundingText` đọc lại khớp với màn hình: Có thể dùng cơ chế hậu kiểm (post-verification) để phát hiện bug DraftJS.
    - Nếu `surroundingText` bị trễ/lệch: Engine hoàn toàn bị mù đối với lỗi của DraftJS. Khi đó, tự cân chỉnh cho DraftJS là bất khả thi, bắt buộc phải dùng **Profile ứng dụng cố định + Bảng chỉnh tay thủ công**.

---

## 3. Điểm Mù 2: Động Lực Học Điều Khiển & Cơ Chế Decay (AIMD)

### 3.1 Sai lầm của "Đóng băng và ngủ đông"

Bản thiết kế cũ đề xuất: khi tăng delay đến mức gõ 50 từ không lỗi thì "đóng băng vĩnh viễn và tắt controller để tiết kiệm 100% CPU". Cách tiếp cận này có ba khuyết điểm:

1. **Bẫy nghẽn tạm thời**: Nếu người dùng đang biên dịch code nặng (như rustc LTO) hoặc máy vừa khởi động bị lag, delay có thể bị đẩy vọt lên 35ms. Nếu đóng băng tại đây, máy sẽ bị kẹt ở mức 35ms mãi mãi, khiến trải nghiệm gõ luôn bị chậm và nháy hình.
2. **50 từ an toàn không phải là điểm ngọt**: Một mức delay cao luôn an toàn, nhưng mục tiêu là tìm mức delay **nhỏ nhất mà vẫn an toàn**. Dừng lại ở 50 từ chỉ chứng minh nó không chết, chứ không chứng minh nó tối ưu.
3. **Tiết kiệm CPU ảo**: Bộ điều khiển pacing chỉ gồm vài phép toán số nguyên cộng trừ và so sánh mỗi khi gõ từ, tiêu tốn chưa đến 0.0001% CPU. Việc xây dựng một máy trạng thái bật/tắt ngủ đông chỉ làm tăng độ phức tạp của code mà không mang lại giá trị hiệu năng thực tế.

### 3.2 Mô hình AIMD liên tục (Additive Increase, Slow Decay)

Thay vì đóng băng, bộ điều khiển cần chạy liên tục với thuật toán tăng nhanh giảm chậm:

- **Additive Increase (Tăng nhanh khi gặp sự cố)**: Khi phát hiện pacing starvation thực sự, tăng ngay `post_delay += 4ms`, `gap += 2ms`.
- **Slow Decay (Hạ dần khi ổn định)**: Cứ sau mỗi chu kỳ $N$ từ gõ thành công liên tiếp (ví dụ 100 từ), tự động hạ nhẹ `post_delay -= 1ms` (không hạ xuống dưới mức sàn Floor). Quá trình này giúp hệ thống liên tục thăm dò để tìm về mức delay tối thiểu tối ưu nhất.

```
       Delay (ms)
          ^
      30 _|         /\ (Lag spike)
         |        /  \
      20 _|  /\  /    \_______
         | /  \/              \______ (Slow Decay về Floor)
      10 _|/                         \_________ (Floor)
         +----------------------------------------> Thời gian
```

### 3.3 Kiến trúc lưu trữ trạng thái (State Persistence)

Nếu lưu giá trị đã học để không phải dò lại từ đầu sau mỗi lần reboot:

- **Phân tách namespace (Composite Key)**: Không dùng một mức delay chung. Phải lưu theo cặp `(compositor, app_group)`. Ví dụ:
    - `hyprland:terminal` -> 4ms
    - `gnome:draftjs` -> 20ms
- **Tuân thủ chuẩn XDG**: Lưu vào file state `~/.local/state/clak/pacing.json` (hoặc `XDG_STATE_HOME`), tuyệt đối không ghi đè vào file cấu hình người dùng `~/.config/clak/`.
- **Không I/O trên Hot Path**: Việc ghi đĩa phải thực hiện bất đồng bộ hoặc thông qua cơ chế xả chậm (flush on idle/timer), không bao giờ gọi hàm ghi file trong luồng `handleKey`.
- **Ưu tiên cấu hình thủ công**: Nếu người dùng đã gán giá trị cố định trong file config hoặc GUI, giá trị chỉnh tay luôn có quyền ưu tiên cao nhất, vô hiệu hóa việc tự học trên nhóm đó.

---

## 4. Điểm Mù 3: Phân Loại Dị Thường (Anomaly Taxonomy)

Không phải mọi sự kiện timeout đều do "thiếu delay". Đếm dồn tất cả timeout vào một biến phạt chung sẽ dẫn đến phán đoán sai.

### 4.1 Bảng phân loại nguyên nhân timeout

| Loại sự kiện                 | Bản chất kỹ thuật                                                                            | Hành vi của Pacing Controller                                                                      |
| :--------------------------- | :------------------------------------------------------------------------------------------- | :------------------------------------------------------------------------------------------------- |
| **Pacing Starvation**        | Phím sentinel bị nuốt do ứng dụng chưa sẵn sàng nhận phím, không có phím nào khác chen ngang | **Hợp lệ**: Tăng `post_delay` và `gap`                                                             |
| **Key Collision**            | Người dùng gõ phím tiếp theo quá nhanh khi uinput đang phát dở, làm đứt mạch xóa             | **Bỏ qua**: Tăng delay trong ca này sẽ làm tăng thời gian chờ, khiến va chạm phím xảy ra nhiều hơn |
| **Selection Deletion**       | Xóa vùng bôi đen dài (timeout 250ms), phụ thuộc vào render khối văn bản                      | **Bỏ qua**: Được điều phối bởi logic vùng chọn riêng                                               |
| **Address Bar Autocomplete** | Chờ đóng popup gợi ý của Chromium/Firefox (20ms)                                             | **Bỏ qua**: Đã có pacing cố định của Address Bar                                                   |

### 4.2 Cửa sổ trượt (Sliding Time Window)

Thay vì đếm dồn biến đếm tổng (dễ bị cộng dồn lỗi rải rác sau nhiều ngày sử dụng), hệ thống dùng một cửa sổ trượt (Ring Buffer):

- Chỉ tính các lỗi Pacing Starvation hợp lệ xuất hiện trong vòng **10 giây gần nhất**.
- Nếu trong 10 giây có từ 2 lỗi starvation trở lên, mới kích hoạt nấc tăng delay.

---

## 5. Điểm Mù 4: Cơ Sở Đo Đạc RTT Thực Nghiệm (Empirical Grounding)

Mọi con số như "RTT 20-30ms trên GNOME", trần 35ms hay sàn 4ms hiện tại đều là ước lượng lý thuyết.

### 5.1 Kế hoạch thu thập dữ liệu

Clak đã có module ghi log độ trễ `utils::logRttCsv`. Quy trình lấy số đo thực tế:

1. Chạy kịch bản gõ chuẩn trên cả 2 môi trường:
    - Môi trường A: Arch Linux + Hyprland (144Hz+)
    - Môi trường B: Ubuntu 24.04 + GNOME 46 Mutter (60Hz, cắm sạc và dùng pin)
2. Thu thập tối thiểu 500 mẫu RTT của phím sentinel trên các nhóm: `Terminal`, `DraftJS`, `Gecko`, `Default`.
3. Trích xuất biểu đồ phân phối: Giá trị trung bình (P50), P95 và P99.

### 5.2 Xác định mức Sàn (Floor) và Trần (Ceiling)

- **Floor (Mức tối thiểu)**: Đặt bằng giá trị P50 của RTT cộng dung sai 2ms. Không bao giờ cho phép decay xuống dưới mức Floor này để tránh rơi vào vùng nghẽn chắc chắn.
- **Ceiling (Mức trần an toàn)**: Đặt bằng giá trị P99. Nếu chạm trần mà vẫn phát hiện lỗi, hệ thống dừng tăng và giữ nguyên mức trần, tránh việc đẩy delay lên vô tận làm liệt bộ gõ.

---

## 6. Thiết Kế Module Thuần & Mock Testing (Architecture)

Toàn bộ logic tự thích ứng được tách biệt thành một module hàm thuần (Pure Logic), không phụ thuộc vào I/O hay Fcitx5 context, cho phép kiểm thử tự động 100% bằng Unit Test.

### 6.1 Cấu trúc dữ liệu đề xuất

```cpp
// namespace clak::pacing

struct PacingMetrics {
    uint32_t post_delay_ms;
    uint32_t gap_ms;
    uint32_t floor_post_ms;
    uint32_t ceiling_post_ms;
    uint32_t consecutive_clean_words;
};

enum class IncidentType {
    PacingStarvation, // sentinel lost without intervening keystrokes
    KeyInterruption,  // user keystroke collided with in-flight uinput
    SelectionTimeout, // deliberate 250ms selection timeout
    AddressBarDelay   // deliberate 20ms autofill delay
};

class AdaptivePacingEngine {
public:
    explicit AdaptivePacingEngine(PacingMetrics initial_metrics);

    // pure logic: calculate next pacing metrics based on observed event
    void onTransactionComplete(uint64_t rtt_us);
    void onIncident(IncidentType incident, uint64_t timestamp_us);

    PacingMetrics currentMetrics() const;

private:
    PacingMetrics metrics_;
    std::deque<uint64_t> incident_timestamps_us_;
};
```

### 6.2 Chiến lược kiểm thử bằng Mock

- **Test Case 1 (Thích ứng tăng nhanh)**: Giả lập 2 sự kiện `PacingStarvation` trong 3 giây -> Xác nhận `post_delay` tăng đúng bước nhảy và không vượt trần.
- **Test Case 2 (Lọc nhiễu va chạm phím)**: Giả lập `KeyInterruption` -> Xác nhận `post_delay` giữ nguyên, không bị tăng sai.
- **Test Case 3 (Hạ dần theo Decay)**: Giả lập 100 giao dịch thành công liên tiếp -> Xác nhận `post_delay` giảm dần từng bước 1ms và dừng lại đúng mức Floor.

---

## 7. Kết Luận & Lộ Trình Triển Khai

| Giai đoạn       | Nhiệm vụ kỹ thuật                                                                            | Đầu ra                                                         |
| :-------------- | :------------------------------------------------------------------------------------------- | :------------------------------------------------------------- |
| **Giai đoạn 1** | Đưa `gnome-terminal` / `x-terminal-emul` vào đúng `VteTerminalProfile` để xóa bằng `\x7f`    | Giải quyết dứt điểm 100% lỗi nháy chữ và dư chữ trên Terminal  |
| **Giai đoạn 2** | Kiểm chứng thực nghiệm log `surroundingText` trên `x.com`                                    | Xác định xem DraftJS có quan sát được hay bị "mù"              |
| **Giai đoạn 3** | Thêm bảng chọn Preset (Siêu tốc / Cân bằng / An toàn) và Slider chỉnh tay trong Tab Nâng cao | Người dùng có ngay công cụ giải quyết vấn đề tức thì           |
| **Giai đoạn 4** | Thu thập dữ liệu CSV RTT thực tế trên Ubuntu GNOME và hiện thực hóa `AdaptivePacingEngine`   | Cơ chế tự học thông minh hoạt động an toàn và có cơ sở số liệu |
