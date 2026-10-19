#!/usr/bin/env bash

# test script for typing speed and reliability with wtype
# author: verse

RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

if ! command -v wtype &>/dev/null; then
    echo -e "${RED}Lỗi: wtype chưa được cài đặt${NC}"
    echo "Hãy cài bằng: sudo pacman -S wtype (hoặc apt/dnf tương ứng)"
    exit 1
fi

countdown() {
    local secs=${1:-3}
    echo -e "${YELLOW}👉 Chuyển con trỏ / click vào cửa sổ cần test (Address bar, Terminal, Docs...)${NC}"
    for ((i=secs; i>0; i--)); do
        echo -ne "${CYAN}Bắt đầu sau: ${i}s...${NC}\r"
        sleep 1
    done
    echo -e "${GREEN}Đang gõ...                      ${NC}"
}

# test target sentences
SENTENCES=(
    "ddaay laf booj gox tieengs vieetj clak"
    "nguwowif haam mooj cuoongf nhieetj"
    "thuyeefn buoofm xuooi nguwowcj suoost ddeem"
    "tooi laf nguwowif vieetj nam yeeu hoaf binhf"
    "ddoongf ddooj thieen nhieen hoa khieeus"
    "giowf anh em chuyeern qua delta force heets rooif af"
)

EXPECTED=(
    "đây là bộ gõ tiếng việt clak"
    "người hâm mộ cuồng nhiệt"
    "thuyền buồm xuôi ngược suốt đêm"
    "tôi là người việt nam yêu hoà bình"
    "đồng độ thiên nhiên hoa khiếu"
    "giờ anh em chuyển qua delta force hết rồi à"
)

run_auto_test() {
    local delay=${1:-20}
    echo -e "\n${BLUE}=== CHẠY TEST TỰ ĐỘNG BẰNG TERMINAL TEST CỦA HỆ THỐNG ===${NC}"
    echo -e "Tốc độ: ${YELLOW}${delay}ms / phím${NC} (~$(( 1000 / delay )) ký tự/giây)\n"

    local tmp_file="/tmp/wtype_test_result.txt"
    rm -f "$tmp_file"

    local pass_count=0
    local total=${#SENTENCES[@]}

    for i in "${!SENTENCES[@]}"; do
        local raw="${SENTENCES[$i]}"
        local exp="${EXPECTED[$i]}"
        rm -f "$tmp_file"

        # launch temporary terminal receiver
        kitty --title "wtype_runner" bash -c "sleep 0.2; cat > $tmp_file" 2>/dev/null &
        local pid=$!
        sleep 1.0
        fcitx5-remote -s clak 2>/dev/null || true

        wtype -d "$delay" "$raw"
        sleep 0.3
        wtype -k Return
        sleep 0.3

        kill "$pid" 2>/dev/null || true
        wait "$pid" 2>/dev/null || true

        local result=""
        if [[ -f "$tmp_file" ]]; then
            result=$(cat "$tmp_file" | tr -d '\r\n')
        fi

        local exp_alt="${exp//hoà/hòa}"
        if [[ "$result" == "$exp" || "$result" == "$exp_alt" ]]; then
            echo -e "  [${GREEN}PASS${NC}] Gõ: \"$raw\""
            echo -e "         -> Kết quả: \"${GREEN}$result${NC}\""
            ((pass_count++))
        else
            echo -e "  [${RED}FAIL${NC}] Gõ: \"$raw\""
            echo -e "         -> Mong đợi: \"$exp\""
            echo -e "         -> Thực tế:  \"${RED}$result${NC}\""
        fi
        echo ""
    done

    rm -f "$tmp_file"
    echo -e "Kết quả: ${GREEN}${pass_count}/${total} câu PASS${NC} ở tốc độ ${delay}ms/phím"
}

run_focus_test() {
    local delay=${1:-20}
    echo -e "\n${BLUE}=== TEST TRỰC TIẾP TRÊN CỬA SỔ HIỆN TẠI ===${NC}"
    echo -e "Tốc độ: ${YELLOW}${delay}ms / phím${NC}"
    countdown 4

    for i in "${!SENTENCES[@]}"; do
        local raw="${SENTENCES[$i]}"
        wtype -d "$delay" "$raw "
        sleep 0.2
    done
    wtype -k Return
    echo -e "${GREEN}Đã gõ xong! Hãy kiểm tra nội dung hiển thị trên màn hình${NC}"
}

run_address_bar_test() {
    echo -e "\n${BLUE}=== TEST LỖI XÓA LÙI & GÕ LẠI (ADDRESS BAR / BACKSPACE BUG) ===${NC}"
    echo -e "Kịch bản:"
    echo "  1. Gõ 'dd' -> 'đ'"
    echo "  2. Bấm Backspace xóa 'đ'"
    echo "  3. Gõ 'dd' lại -> xem có bị lỗi d -> đ hay kẹt buffer không"
    echo "  4. Gõ 'ddaay' -> 'đây', xóa 3 lần còn 'đ', gõ 'oo' -> 'đô'"
    countdown 4

    # type dd
    wtype -d 20 "dd"
    sleep 0.5
    # backspace to delete
    wtype -k BackSpace
    sleep 0.5
    # type dd again
    wtype -d 20 "dd "
    sleep 0.5
    # type ddaay
    wtype -d 20 "ddaay"
    sleep 0.5
    # backspace 3 times
    wtype -k BackSpace -s 100 -k BackSpace -s 100 -k BackSpace
    sleep 0.5
    # type oo
    wtype -d 20 "oo "
    sleep 0.3
    wtype -k Return

    echo -e "${GREEN}Hoàn thành test address bar!${NC}"
}

run_stress_test() {
    echo -e "\n${BLUE}=== STRESS TEST SIÊU TỐC (8ms / PHÍM - 125 PHÍM/GIÂY) ===${NC}"
    countdown 4

    local paragraph="ddaay laf baif kieemr tra toocs ddooj gox sieeu nhanh cuar booj gox clak treen heej thooong wayland vaf fcitx5. neeus khoong bij maats tuwf hoaajc ddesync thif tuyeetj vowif!"
    wtype -d 8 "$paragraph"
    wtype -k Return
    echo -e "${GREEN}Đã bắn xong 1 đoạn văn dài trong chớp mắt!${NC}"
}

# command line arguments parsing
case "$1" in
    --auto|-a)
        run_auto_test "${2:-15}"
        exit 0
        ;;
    --focus|-f)
        run_focus_test "${2:-20}"
        exit 0
        ;;
    --address-bar|-b)
        run_address_bar_test
        exit 0
        ;;
    --stress|-s)
        run_stress_test
        exit 0
        ;;
    --help|-h)
        echo "Cách dùng: $0 [tùy chọn] [delay_ms]"
        echo "  --auto, -a [ms]        Tự bật cửa sổ terminal và test tự động (mặc định 15ms)"
        echo "  --focus, -f [ms]       Đếm ngược 4s để bạn click vào bất kỳ app nào (mặc định 20ms)"
        echo "  --address-bar, -b      Test kịch bản xóa lùi và gõ lại d -> đ"
        echo "  --stress, -s           Bắn chuỗi siêu tốc 8ms/phím (125 phím/giây)"
        exit 0
        ;;
esac

# interactive menu if no args given
echo -e "${CYAN}╔═══════════════════════════════════════════════════════════╗${NC}"
echo -e "${CYAN}║             BỘ TEST GÕ TỐC ĐỘ CAO CHO CLAK                ║${NC}"
echo -e "${CYAN}╚═══════════════════════════════════════════════════════════╝${NC}"
echo "1. Chạy test tự động (bật terminal ngầm, chấm điểm PASS/FAIL)"
echo "2. Test trên cửa sổ đang mở (Click vào Chrome/Terminal/Docs sau khi đếm ngược)"
echo "3. Test riêng lỗi Address bar (gõ dd -> xoá -> gõ lại dd -> kiểm tra đ)"
echo "4. Stress test siêu tốc (8ms/phím ~ 125 phím/s)"
echo "5. Thoát"
echo ""
read -p "Chọn chế độ (1-5): " choice

case "$choice" in
    1)
        read -p "Nhập delay giữa các phím (ms, mặc định 15): " d
        run_auto_test "${d:-15}"
        ;;
    2)
        read -p "Nhập delay giữa các phím (ms, mặc định 20): " d
        run_focus_test "${d:-20}"
        ;;
    3)
        run_address_bar_test
        ;;
    4)
        run_stress_test
        ;;
    *)
        echo "Thoát"
        exit 0
        ;;
esac
