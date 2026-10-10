#!/usr/bin/env bash
set -euo pipefail

# configuration and defaults
dry_run=0
target_mode="auto"
assume_yes=0
purge_config=0

# parse command-line arguments
for arg in "$@"; do
    case "$arg" in
        --dry-run|--simulate|-d)
            dry_run=1
            ;;
        --system|-s)
            target_mode="system"
            ;;
        --user|-u)
            target_mode="user"
            ;;
        --purge|-p)
            purge_config=1
            ;;
        --yes|-y)
            assume_yes=1
            ;;
        --help|-h)
            echo "Cách dùng: curl -fsSL https://raw.githubusercontent.com/versenilvis/clak/main/scripts/uninstall.sh | bash [options]"
            echo "Hoặc: bash scripts/uninstall.sh [options]"
            echo ""
            echo "Tùy chọn:"
            echo "  -d, --dry-run, --simulate  Chạy giả lập kiểm tra các mục sẽ gỡ bỏ (không xóa file)"
            echo "  -u, --user                 Chỉ gỡ bỏ bản cài đặt cá nhân (~/.local)"
            echo "  -s, --system               Gỡ bỏ cả bản cài đặt toàn hệ thống (/usr) [cần sudo]"
            echo "  -p, --purge                Gỡ bỏ toàn bộ bao gồm cả cấu hình (~/.config/clak)"
            echo "  -y, --yes                  Tự động đồng ý mọi thao tác gỡ bỏ"
            echo "  -h, --help                 Hiển thị trợ giúp này"
            exit 0
            ;;
    esac
done

# visual palette and terminal formatting
c_reset="\033[0m"
c_dim="\033[38;5;244m"
c_bold="\033[1m"
c_cyan="\033[1;36m"
c_blue="\033[1;34m"
c_green="\033[1;32m"
c_yellow="\033[1;33m"
c_purple="\033[1;35m"
c_red="\033[1;31m"
c_gray="\033[38;5;240m"
c_accent="\033[38;5;75m"

sep="${c_gray}│${c_reset}"
lbl_check="${c_blue}KIỂM TRA${c_reset}"
lbl_remove="${c_red}GỠ BỎ   ${c_reset}"
lbl_clean="${c_yellow}DỌN DẸP ${c_reset}"
lbl_fcitx="${c_cyan}FCITX5  ${c_reset}"
lbl_wps="${c_yellow}WPS     ${c_reset}"
lbl_steam="${c_blue}STEAM   ${c_reset}"
lbl_warn="${c_yellow}LƯU Ý   ${c_reset}"
lbl_err="${c_red}LỖI     ${c_reset}"

get_ts() {
    printf "%b[%s]%b" "$c_dim" "$(date +%T)" "$c_reset"
}

log_step() {
    local lbl="$1"
    local msg="$2"
    printf "%s %b %b %b\n" "$(get_ts)" "$lbl" "$sep" "$msg"
}

err() {
    log_step "$lbl_err" "${c_red}$1${c_reset}"
    exit 1
}

spin_step() {
    local text="$1"
    if [ ! -t 1 ]; then
        return
    fi
    local frames=("⠋" "⠙" "⠹" "⠸" "⠼" "⠴" "⠦" "⠧" "⠇" "⠏")
    for ((pct=0; pct<=100; pct+=10)); do
        local f=${frames[(pct / 10) % 10]}
        printf "\r\033[38;5;75m%s\033[0m \033[1;36m%3d%%\033[0m \033[38;5;244m%s\033[0m\033[K" "$f" "$pct" "$text"
        sleep 0.03
    done
    printf "\r\033[K"
}

spin_pid() {
    local pid="$1"
    local text="$2"
    if [ ! -t 1 ]; then
        wait "$pid" 2>/dev/null || true
        return
    fi
    local frames=("⠋" "⠙" "⠹" "⠸" "⠼" "⠴" "⠦" "⠧" "⠇" "⠏")
    local i=0
    while kill -0 "$pid" 2>/dev/null; do
        local f=${frames[i % 10]}
        printf "\r\033[38;5;75m%s\033[0m \033[38;5;244m%s\033[0m\033[K" "$f" "$text"
        i=$((i + 1))
        sleep 0.08
    done
    printf "\r\033[K"
}

run_sudo() {
    if [ "$EUID" -eq 0 ]; then
        "$@"
        return $?
    fi

    if ! command -v sudo >/dev/null 2>&1; then
        err "Cần quyền quản trị viên (sudo) để thực thi lệnh này"
    fi

    if ! sudo -n true 2>/dev/null; then
        if [ -e /dev/tty ] && [ -r /dev/tty ]; then
            sudo -v < /dev/tty || return 1
        else
            sudo -v || return 1
        fi
    fi

    sudo "$@"
}

# simulation flow
run_simulation() {
    log_step "$lbl_check" "Bắt đầu chạy giả lập quy trình gỡ bỏ Clak (không sửa đổi hệ thống)"
    spin_step "Đang quét các thành phần Clak trên hệ thống..."
    
    local has_user=0
    local has_system=0
    local has_wps=0
    local has_steam=0
    local has_purge=0
    local has_profile=0
    local has_autostart=0
    local has_config=0
    local has_fcitx_bak=0

    [ -f "${HOME}/.local/lib/fcitx5/libclak.so" ] && has_user=1
    [ -f "${HOME}/.local/share/fcitx5/addon/clak.conf" ] && has_user=1
    [ -f "${HOME}/.local/share/fcitx5/inputmethod/clak.conf" ] && has_user=1
    [ -f "${HOME}/.local/bin/clak" ] && has_user=1
    [ -f "${HOME}/.local/bin/clak-gui" ] && has_user=1
    [ -f "${HOME}/.local/share/applications/clak-gui.desktop" ] && has_user=1
    [ -d "${HOME}/.local/share/clak" ] && has_user=1
    [ -n "$(find "${HOME}/.local/share/icons" -name '*clak*' -print -quit 2>/dev/null)" ] && has_user=1

    [ -f "/usr/lib/fcitx5/libclak.so" ] && has_system=1
    [ -f "/usr/bin/clak" ] && has_system=1
    [ -f "/usr/bin/clak-gui" ] && has_system=1
    [ -f "/usr/share/applications/clak-gui.desktop" ] && has_system=1
    [ -f "/usr/share/fcitx5/addon/clak.conf" ] && has_system=1
    [ -f "/usr/share/fcitx5/inputmethod/clak.conf" ] && has_system=1
    [ -n "$(find /usr/share/icons -name '*clak*' -print -quit 2>/dev/null)" ] && has_system=1

    [ -f "${HOME}/.config/environment.d/99-clak-wps.conf" ] && has_wps=1
    [ -f "${HOME}/.local/share/applications/steam.desktop" ] && grep -q "GTK_IM_MODULE=xim" "${HOME}/.local/share/applications/steam.desktop" 2>/dev/null && has_steam=1
    [ -f "${HOME}/.local/bin/steam" ] && grep -q "GTK_IM_MODULE=xim" "${HOME}/.local/bin/steam" 2>/dev/null && has_steam=1
    [ -d "${HOME}/.config/clak" ] && [ "$purge_config" -eq 1 ] && has_config=1
    [ -f "${HOME}/.config/autostart/clak-autostart.desktop" ] && has_autostart=1
    [ -f "${HOME}/.config/environment.d/99-clak-im.conf" ] && has_autostart=1
    [ -f "${HOME}/.config/fcitx5/config.bak-clak" ] && has_fcitx_bak=1
    if [ -f "${HOME}/.config/fcitx5/profile" ] && grep -q "clak" "${HOME}/.config/fcitx5/profile" 2>/dev/null; then
        has_profile=1
    fi

    if [ "$has_user" -eq 0 ] && [ "$has_system" -eq 0 ] && [ "$has_profile" -eq 0 ] && [ "$has_autostart" -eq 0 ] && [ "$has_config" -eq 0 ] && [ "$has_wps" -eq 0 ] && [ "$has_steam" -eq 0 ] && [ "$has_fcitx_bak" -eq 0 ]; then
        echo ""
        log_step "$lbl_warn" "Không tìm thấy file cài đặt hoặc cấu hình Clak nào trên hệ thống"
        return 0
    fi

    echo ""
    echo -e "  ${c_bold}Các mục sẽ được gỡ bỏ:${c_reset}"
    if [ "$has_user" -eq 1 ]; then
        echo -e "  • Thư viện và ứng dụng cá nhân:     ${c_accent}~/.local/lib/fcitx5/libclak.so, ~/.local/bin/clak, ~/.local/bin/clak-gui${c_reset}"
        echo -e "  • Khai báo addon & bộ gõ cá nhân:   ${c_accent}~/.local/share/fcitx5/{addon,inputmethod}/clak.conf${c_reset}"
        echo -e "  • Menu ứng dụng cá nhân:            ${c_accent}~/.local/share/applications/clak-gui.desktop${c_reset}"
        echo -e "  • Dữ liệu phiên bản & icon cá nhân: ${c_accent}~/.local/share/clak/, ~/.local/share/icons/**/clak*${c_reset}"
    fi

    if [ "$has_system" -eq 1 ]; then
        echo -e "  • File hệ thống (/usr):             ${c_accent}/usr/lib/fcitx5/libclak.so, /usr/bin/clak, /usr/bin/clak-gui${c_reset}"
        echo -e "  • Khai báo addon & bộ gõ hệ thống:  ${c_accent}/usr/share/fcitx5/{addon,inputmethod}/clak.conf${c_reset}"
        echo -e "  • Menu ứng dụng hệ thống:           ${c_accent}/usr/share/applications/clak-gui.desktop${c_reset}"
        echo -e "  • Biểu tượng hệ thống:              ${c_accent}/usr/share/icons/hicolor/**/clak*${c_reset}"
    fi

    if [ "$has_profile" -eq 1 ]; then
        echo -e "  • Mục Clak trong cấu hình Fcitx5:   ${c_accent}~/.config/fcitx5/profile${c_reset}"
    fi

    if [ "$has_fcitx_bak" -eq 1 ]; then
        echo -e "  • Khôi phục cấu hình Fcitx5 gốc:    ${c_accent}~/.config/fcitx5/config${c_reset}"
    fi

    if [ "$has_autostart" -eq 1 ]; then
        echo -e "  • Cấu hình khởi động cùng hệ thống: ${c_accent}~/.config/autostart/clak-autostart.desktop${c_reset}"
        echo -e "  • Cấu hình biến môi trường:         ${c_accent}~/.config/environment.d/99-clak-im.conf${c_reset}"
    fi

    if [ "$has_wps" -eq 1 ]; then
        echo -e "  • Cấu hình tương thích WPS Office:  ${c_accent}~/.config/environment.d/99-clak-wps.conf${c_reset}"
        echo -e "  • Phím tắt WPS launcher tùy chỉnh:  ${c_accent}~/.local/share/applications/wps-office-*.desktop${c_reset}"
    fi

    if [ "$has_steam" -eq 1 ]; then
        echo -e "  • Phím tắt Steam launcher tùy chỉnh:  ${c_accent}~/.local/share/applications/steam.desktop${c_reset}"
        echo -e "  • Wrapper Steam terminal:            ${c_accent}~/.local/bin/steam${c_reset}"
    fi

    if [ "$has_config" -eq 1 ] && [ "$purge_config" -eq 1 ]; then
        echo -e "  • Thư mục cấu hình cá nhân:         ${c_accent}~/.config/clak/${c_reset}"
    fi

    echo ""
    log_step "$lbl_clean" "[Giả lập] Dọn dẹp cấu hình khởi động và mục Clak trong profile Fcitx5"
    log_step "$lbl_fcitx" "[Giả lập] Tự động nạp lại daemon Fcitx5 để cập nhật khay hệ thống"
    echo -e "${c_green}✔ Quá trình giả lập gỡ bỏ hoàn tất thành công!${c_reset}"
    echo ""
}

# execution flow
run_uninstall() {
    # 1. detect installed components and configurations
    local has_user_files=0
    local has_sys_files=0
    local has_profile=0
    local has_autostart=0
    local has_config=0
    local has_wps=0
    local has_fcitx_bak=0

    [ -f "${HOME}/.local/lib/fcitx5/libclak.so" ] && has_user_files=1
    [ -f "${HOME}/.local/share/fcitx5/addon/clak.conf" ] && has_user_files=1
    [ -f "${HOME}/.local/share/fcitx5/inputmethod/clak.conf" ] && has_user_files=1
    [ -f "${HOME}/.local/bin/clak" ] && has_user_files=1
    [ -f "${HOME}/.local/bin/clak-gui" ] && has_user_files=1
    [ -f "${HOME}/.local/share/applications/clak-gui.desktop" ] && has_user_files=1
    [ -d "${HOME}/.local/share/clak" ] && has_user_files=1
    [ -n "$(find "${HOME}/.local/share/icons" -name '*clak*' -print -quit 2>/dev/null)" ] && has_user_files=1

    [ -f "/usr/lib/fcitx5/libclak.so" ] && has_sys_files=1
    [ -f "/usr/bin/clak" ] && has_sys_files=1
    [ -f "/usr/bin/clak-gui" ] && has_sys_files=1
    [ -f "/usr/share/applications/clak-gui.desktop" ] && has_sys_files=1
    [ -f "/usr/share/fcitx5/addon/clak.conf" ] && has_sys_files=1
    [ -f "/usr/share/fcitx5/inputmethod/clak.conf" ] && has_sys_files=1
    [ -n "$(find /usr/share/icons -name '*clak*' -print -quit 2>/dev/null)" ] && has_sys_files=1

    [ -f "${HOME}/.config/autostart/clak-autostart.desktop" ] && has_autostart=1
    [ -f "${HOME}/.config/environment.d/99-clak-im.conf" ] && has_autostart=1
    [ -f "${HOME}/.config/environment.d/99-clak-wps.conf" ] && has_wps=1
    [ -f "${HOME}/.local/share/applications/steam.desktop" ] && grep -q "GTK_IM_MODULE=xim" "${HOME}/.local/share/applications/steam.desktop" 2>/dev/null && has_steam=1
    [ -f "${HOME}/.local/bin/steam" ] && grep -q "GTK_IM_MODULE=xim" "${HOME}/.local/bin/steam" 2>/dev/null && has_steam=1
    [ -f "${HOME}/.config/fcitx5/config.bak-clak" ] && has_fcitx_bak=1
    [ -d "${HOME}/.config/clak" ] && [ "$purge_config" -eq 1 ] && has_config=1
    if [ -f "${HOME}/.config/fcitx5/profile" ] && grep -q "clak" "${HOME}/.config/fcitx5/profile" 2>/dev/null; then
        has_profile=1
    fi

    if [ "$has_user_files" -eq 0 ] && [ "$has_sys_files" -eq 0 ] && [ "$has_profile" -eq 0 ] && [ "$has_autostart" -eq 0 ] && [ "$has_config" -eq 0 ] && [ "$has_wps" -eq 0 ] && [ "$has_steam" -eq 0 ] && [ "$has_fcitx_bak" -eq 0 ] && [ "$target_mode" != "system" ]; then
        log_step "$lbl_warn" "Không tìm thấy file cài đặt hoặc cấu hình Clak nào trên hệ thống"
        exit 0
    fi

    # 2. confirm prompt if interactive
    if [ "$assume_yes" -ne 1 ] && [ -t 0 ]; then
        echo -e "${c_bold}Bạn có chắc chắn muốn gỡ bỏ hoàn toàn bộ gõ Clak khỏi máy?${c_reset}"
        read -r -p "Xác nhận gỡ bỏ? [y/N]: " confirm_choice
        case "$confirm_choice" in
            [yY][eE][sS]|[yY])
                ;;
            *)
                echo "Đã hủy thao tác gỡ bỏ."
                exit 0
                ;;
        esac
    fi

    # 3. remove user-space binaries, addons, and shortcuts
    if [ "$has_user_files" -eq 1 ] && [ "$target_mode" != "system" ]; then
        spin_step "Đang xóa thư viện và cấu hình cá nhân (~/.local)..."
        rm -f "${HOME}/.local/bin/clak"
        rm -f "${HOME}/.local/bin/clak-gui"
        rm -f "${HOME}/.local/share/applications/clak-gui.desktop"
        rm -f "${HOME}/.local/lib/fcitx5/libclak.so"
        rm -f "${HOME}/.local/lib/x86_64-linux-gnu/fcitx5/libclak.so"
        rm -f "${HOME}/.local/share/fcitx5/addon/clak.conf"
        rm -f "${HOME}/.local/share/fcitx5/inputmethod/clak.conf"
        rm -rf "${HOME}/.local/share/clak"
        if command -v update-desktop-database >/dev/null 2>&1; then
            update-desktop-database "${HOME}/.local/share/applications" 2>/dev/null || true
        fi
        log_step "$lbl_remove" "Đã xóa file thư viện và ứng dụng Clak trong ~/.local"
    fi

    # 4. remove system binaries if requested or detected
    if [ "$has_sys_files" -eq 1 ] && ([ "$target_mode" = "system" ] || [ "$target_mode" = "auto" ]); then
        spin_step "Đang xóa file Clak toàn hệ thống (/usr)..."
        run_sudo rm -f "/usr/bin/clak"
        run_sudo rm -f "/usr/bin/clak-gui"
        run_sudo rm -f "/usr/share/applications/clak-gui.desktop"
        run_sudo rm -f "/usr/lib/fcitx5/libclak.so"
        run_sudo rm -f "/usr/lib/x86_64-linux-gnu/fcitx5/libclak.so"
        run_sudo rm -f "/usr/share/fcitx5/addon/clak.conf"
        run_sudo rm -f "/usr/share/fcitx5/inputmethod/clak.conf"
        run_sudo find "/usr/share/icons" -type f -name "*clak*" -delete 2>/dev/null || true
        if command -v gtk-update-icon-cache >/dev/null 2>&1; then
            run_sudo gtk-update-icon-cache -f -q -t "/usr/share/icons/hicolor" 2>/dev/null || true
        fi
        if command -v update-desktop-database >/dev/null 2>&1; then
            run_sudo update-desktop-database "/usr/share/applications" 2>/dev/null || true
        fi
        log_step "$lbl_remove" "Đã xóa file Clak trong hệ thống (/usr)"
    fi

    # 5. cleanup wps office compatibility overrides
    local wps_cleaned=0
    if [ -f "${HOME}/.config/environment.d/99-clak-wps.conf" ]; then
        rm -f "${HOME}/.config/environment.d/99-clak-wps.conf"
        wps_cleaned=1
    fi

    for df in "${HOME}/.local/share/applications"/wps-office-*.desktop; do
        if [ -f "$df" ] && grep -q "QT_IM_MODULE=fcitx" "$df" 2>/dev/null; then
            rm -f "$df"
            wps_cleaned=1
        fi
    done

    for b in wps wpp et wpspdf; do
        local wb="${HOME}/.local/bin/$b"
        if [ -f "$wb" ] && grep -q "QT_IM_MODULE=fcitx" "$wb" 2>/dev/null; then
            rm -f "$wb"
            wps_cleaned=1
        fi
    done

    if [ "$wps_cleaned" -eq 1 ]; then
        command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "${HOME}/.local/share/applications" 2>/dev/null || true
        log_step "$lbl_wps" "Đã dọn dẹp các cấu hình tương thích WPS Office"
    fi

    # 6. cleanup steam compatibility overrides
    local steam_cleaned=0
    local steam_df="${HOME}/.local/share/applications/steam.desktop"
    if [ -f "$steam_df" ] && grep -q "GTK_IM_MODULE=xim" "$steam_df" 2>/dev/null; then
        rm -f "$steam_df"
        steam_cleaned=1
    fi
    local steam_wb="${HOME}/.local/bin/steam"
    if [ -f "$steam_wb" ] && grep -q "GTK_IM_MODULE=xim" "$steam_wb" 2>/dev/null; then
        rm -f "$steam_wb"
        steam_cleaned=1
    fi
    if command -v flatpak >/dev/null 2>&1 && flatpak list --app 2>/dev/null | grep -q "com.valvesoftware.Steam"; then
        flatpak override --user --reset com.valvesoftware.Steam 2>/dev/null || true
    fi
    if [ "$steam_cleaned" -eq 1 ]; then
        command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "${HOME}/.local/share/applications" 2>/dev/null || true
        log_step "$lbl_steam" "Đã dọn dẹp các cấu hình tương thích Steam"
    fi

    # 7. cleanup autostart and environment configuration
    spin_step "Đang dọn dẹp cấu hình khởi động cùng hệ thống..."
    local target_home="${HOME}"
    local target_user="${USER:-$(id -un)}"
    if [ -n "${SUDO_USER:-}" ] && [ "$SUDO_USER" != "root" ]; then
        target_user="$SUDO_USER"
        target_home="$(getent passwd "$SUDO_USER" | cut -d: -f6)"
    fi

    rm -f "${target_home}/.config/autostart/clak-autostart.desktop"
    rm -f "${target_home}/.config/environment.d/99-clak-im.conf"

    # restore ibus autostart if disabled by clak
    for f in "${target_home}/.config/autostart"/ibus-*.disabled-by-clak; do
        if [ -f "$f" ]; then
            mv "$f" "${f%.disabled-by-clak}" 2>/dev/null || true
        fi
    done

    # restore original fcitx5 config backup if available
    if [ -f "${target_home}/.config/fcitx5/config.bak-clak" ]; then
        mv "${target_home}/.config/fcitx5/config.bak-clak" "${target_home}/.config/fcitx5/config" 2>/dev/null || true
        log_step "$lbl_clean" "Đã khôi phục file cấu hình Fcitx5 gốc (~/.config/fcitx5/config)"
    fi

    local prof="${target_home}/.config/fcitx5/profile"
    if [ -f "$prof" ]; then
        if command -v python3 >/dev/null 2>&1; then
            python3 -c '
import sys, re
path = sys.argv[1]
try:
    with open(path, "r") as f:
        content = f.read()
    content = re.sub(r"^DefaultIM=clak", "DefaultIM=keyboard-us", content, flags=re.MULTILINE)
    sections = [s.strip() for s in re.split(r"(?=\n?\[[^\]]+\])", content) if s.strip()]
    groups = {}
    new_sections = []
    for s in sections:
        m = re.match(r"\[([^\]]+)\]", s)
        if not m: continue
        sec_name = m.group(1)
        if sec_name.startswith("Groups/") and "/Items/" in sec_name:
            grp = sec_name.split("/Items/")[0]
            if grp not in groups: groups[grp] = []
            if not re.search(r"Name=clak\b", s):
                groups[grp].append(s)
        else:
            new_sections.append(s)
    final_output = []
    for sec in new_sections:
        final_output.append(sec)
        m = re.match(r"\[(Groups/\d+)\]", sec)
        if m:
            grp = m.group(1)
            if grp in groups:
                for idx, item_sec in enumerate(groups[grp]):
                    renumbered = re.sub(r"\[Groups/\d+/Items/\d+\]", f"[{grp}/Items/{idx}]", item_sec)
                    final_output.append(renumbered)
    with open(path, "w") as f:
        f.write("\n\n".join(final_output) + "\n")
except Exception:
    pass
' "$prof" 2>/dev/null || true
        else
            sed -i 's/^DefaultIM=clak/DefaultIM=keyboard-us/' "$prof" 2>/dev/null || true
        fi
    fi
    log_step "$lbl_clean" "Đã dọn dẹp cấu hình khởi động cùng hệ thống"

    # 7. cleanup icons
    spin_step "Đang dọn dẹp icon Clak..."
    find "${target_home}/.local/share/icons" -type f -name "*clak*" -delete 2>/dev/null || true
    if command -v gtk-update-icon-cache >/dev/null 2>&1; then
        for icondir in "${target_home}/.local/share/icons"/*; do
            [ -d "$icondir" ] && gtk-update-icon-cache -f -q -t "$icondir" 2>/dev/null || true
        done
    fi
    log_step "$lbl_clean" "Đã dọn dẹp biểu tượng Clak trong hệ thống"

    # 7b. cleanup user configuration if purge requested
    if [ "$purge_config" -eq 1 ]; then
        rm -rf "${target_home}/.config/clak"
        log_step "$lbl_clean" "Đã xóa thư mục cấu hình cá nhân (~/.config/clak)"
    fi

    # 8. restart fcitx5 via dbus controller to flush in-memory im list
    spin_step "Đang làm mới và khởi động lại Fcitx5..."
    if [ -n "${SUDO_USER:-}" ] && [ "$SUDO_USER" != "root" ]; then
        local user_uid
        user_uid="$(id -u "$target_user" 2>/dev/null || true)"
        if [ -n "$user_uid" ]; then
            local user_bus="unix:path=/run/user/${user_uid}/bus"
            sudo -u "$target_user" env DBUS_SESSION_BUS_ADDRESS="$user_bus" busctl --user call org.fcitx.Fcitx5 /controller org.fcitx.Fcitx.Controller1 Restart >/dev/null 2>&1 || \
            sudo -u "$target_user" env DBUS_SESSION_BUS_ADDRESS="$user_bus" fcitx5 -r -d >/dev/null 2>&1 || true
        fi
    else
        busctl --user call org.fcitx.Fcitx5 /controller org.fcitx.Fcitx.Controller1 Restart >/dev/null 2>&1 || \
        fcitx5 -r -d >/dev/null 2>&1 || true
    fi
    log_step "$lbl_fcitx" "Đã khởi động lại Fcitx5 và làm mới danh sách bộ gõ thành công"

    printf "\r\033[K\n"
    echo -e "${c_green}✔ Đã gỡ bỏ hoàn toàn bộ gõ Clak khỏi hệ thống!${c_reset}"
    echo ""
}

# main router
if [ "$dry_run" -eq 1 ]; then
    run_simulation
else
    run_uninstall
fi
