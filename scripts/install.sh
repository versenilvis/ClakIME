#!/usr/bin/env bash
set -euo pipefail

# configuration and defaults
repo="versenilvis/clak"
api_url="${CLAK_API_URL:-https://api.github.com}"
release_tag="${CLAK_RELEASE_TAG:-}"
dry_run=0
demo_uinput=0
skip_uinput=0
target_mode="user"
tmp_dir=""

cleanup() {
    if [ -n "${tmp_dir:-}" ] && [ -d "$tmp_dir" ]; then
        rm -rf "$tmp_dir"
    fi
}
trap cleanup EXIT INT TERM

# parse command-line arguments
for arg in "$@"; do
    case "$arg" in
        --dry-run|--simulate|-d)
            dry_run=1
            demo_uinput=1
            ;;
        --demo-uinput)
            demo_uinput=1
            ;;
        --skip-uinput)
            skip_uinput=1
            demo_uinput=0
            ;;
        --system|-s)
            target_mode="system"
            ;;
        --user|-u)
            target_mode="user"
            ;;
        --tag=*)
            release_tag="${arg#*=}"
            ;;
        --help|-h)
            echo "Cách dùng: curl -fsSL https://raw.githubusercontent.com/versenilvis/clak/main/scripts/install.sh | bash [options]"
            echo "Hoặc: bash scripts/install.sh [options]"
            echo ""
            echo "Tùy chọn:"
            echo "  -d, --dry-run, --simulate  Chạy giả lập kiểm tra giao diện và quy trình (không sửa hệ thống)"
            echo "      --demo-uinput          Giả lập tình huống cần cấu hình quyền /dev/uinput"
            echo "  -u, --user                 Cài vào thư mục người dùng (~/.local) [mặc định, không cần sudo]"
            echo "  -s, --system               Cài vào toàn hệ thống (/usr) [sẽ hỏi sudo khi chép file]"
            echo "      --tag=<phiên bản>      Chỉ định phiên bản release cần cài (ví dụ: v0.1.0)"
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
lbl_fetch="${c_yellow}TẢI VỀ  ${c_reset}"
lbl_install="${c_green}CÀI ĐẶT ${c_reset}"
lbl_uinput="${c_purple}UINPUT  ${c_reset}"
lbl_fcitx="${c_cyan}FCITX5  ${c_reset}"
lbl_de="${c_purple}MÔI TRƯỜNG${c_reset}"
lbl_jb="${c_cyan}JETBRAINS ${c_reset}"
lbl_wps="${c_yellow}WPS     ${c_reset}"
lbl_warn="${c_yellow}LƯU Ý   ${c_reset}"
lbl_err="${c_red}LỖI     ${c_reset}"

# format timestamp
get_ts() {
    printf "%b[%s]%b" "$c_dim" "$(date +%T)" "$c_reset"
}

# formatted log line
log_step() {
    local lbl="$1"
    local msg="$2"
    printf "%s %b %b %b\n" "$(get_ts)" "$lbl" "$sep" "$msg"
}

# in-place animated spinner with percentage
spin_step() {
    local text="$1"
    local duration="${2:-0.5}"
    if [ ! -t 1 ]; then
        return
    fi
    local frames=("⠋" "⠙" "⠹" "⠸" "⠼" "⠴" "⠦" "⠧" "⠇" "⠏")
    for ((pct=0; pct<=100; pct+=10)); do
        local f=${frames[(pct / 10) % 10]}
        printf "\r\033[38;5;75m%s\033[0m \033[1;36m%3d%%\033[0m \033[38;5;244m%s\033[0m\033[K" "$f" "$pct" "$text"
        sleep 0.04
    done
    printf "\r\033[K"
}

# in-place animated download spinner with percentage and kb counter
spin_download() {
    local text="$1"
    local total_kb="${2:-0}"
    local track_pid="${3:-}"
    local track_file="${4:-}"

    if [ ! -t 1 ]; then
        if [ -n "$track_pid" ]; then
            wait "$track_pid" 2>/dev/null || true
        fi
        return
    fi

    local frames=("⠋" "⠙" "⠹" "⠸" "⠼" "⠴" "⠦" "⠧" "⠇" "⠏")

    if [ -n "$track_pid" ] && [ -n "$track_file" ]; then
        local i=0
        while kill -0 "$track_pid" 2>/dev/null; do
            local f=${frames[i % 10]}
            local cur_bytes=0
            if [ -f "$track_file" ]; then
                cur_bytes=$(wc -c < "$track_file" 2>/dev/null || echo 0)
            fi
            local cur_kb=$((cur_bytes / 1024))
            if [ "$total_kb" -gt 0 ]; then
                local pct=$((cur_kb * 100 / total_kb))
                [ $pct -gt 99 ] && pct=99
                printf "\r\033[38;5;75m%s\033[0m \033[1;36m%3d%%\033[0m \033[38;5;244m%s (%d/%d KB)\033[0m\033[K" \
                    "$f" "$pct" "$text" "$cur_kb" "$total_kb"
            else
                printf "\r\033[38;5;75m%s\033[0m \033[38;5;244m%s (%d KB)\033[0m\033[K" \
                    "$f" "$text" "$cur_kb"
            fi
            i=$((i + 1))
            sleep 0.08
        done
        wait "$track_pid" 2>/dev/null || true
        local cur_bytes=0
        if [ -f "$track_file" ]; then
            cur_bytes=$(wc -c < "$track_file" 2>/dev/null || echo 0)
        fi
        local final_kb=$((cur_bytes / 1024))
        local f=${frames[i % 10]}
        if [ "$total_kb" -gt 0 ]; then
            printf "\r\033[38;5;75m%s\033[0m \033[1;36m100%%\033[0m \033[38;5;244m%s (%d/%d KB)\033[0m\033[K" \
                "$f" "$text" "$final_kb" "$final_kb"
        else
            printf "\r\033[38;5;75m%s\033[0m \033[1;36m100%%\033[0m \033[38;5;244m%s (%d KB)\033[0m\033[K" \
                "$f" "$text" "$final_kb"
        fi
        sleep 0.04
    else
        for ((pct=0; pct<=100; pct+=4)); do
            local f=${frames[(pct / 4) % 10]}
            if [ "$total_kb" -gt 0 ]; then
                local cur_kb=$((total_kb * pct / 100))
                printf "\r\033[38;5;75m%s\033[0m \033[1;36m%3d%%\033[0m \033[38;5;244m%s (%d/%d KB)\033[0m\033[K" \
                    "$f" "$pct" "$text" "$cur_kb" "$total_kb"
            else
                printf "\r\033[38;5;75m%s\033[0m \033[1;36m%3d%%\033[0m \033[38;5;244m%s\033[0m\033[K" \
                    "$f" "$pct" "$text"
            fi
            sleep 0.04
        done
    fi
    printf "\r\033[K"
}

# in-place animated spinner for asynchronous process
spin_pid() {
    local pid=$1
    local text=$2
    local expected_sec="${3:-1}"
    if [ ! -t 1 ]; then
        wait "$pid" 2>/dev/null || true
        return
    fi
    local frames=("⠋" "⠙" "⠹" "⠸" "⠼" "⠴" "⠦" "⠧" "⠇" "⠏")
    local i=0
    local pct=0
    while kill -0 "$pid" 2>/dev/null; do
        local f=${frames[i % 10]}
        pct=$((i * 100 / (expected_sec * 12)))
        [ $pct -gt 95 ] && pct=95
        printf "\r\033[38;5;75m%s\033[0m \033[1;36m%3d%%\033[0m \033[38;5;244m%s\033[0m\033[K" "$f" "$pct" "$text"
        i=$((i + 1))
        sleep 0.08
    done
    wait "$pid" 2>/dev/null || true
    local f=${frames[i % 10]}
    printf "\r\033[38;5;75m%s\033[0m \033[1;36m100%%\033[0m \033[38;5;244m%s\033[0m\033[K" "$f" "$text"
    sleep 0.04
    printf "\r\033[K"
}

# prompt user for y/n with fallback to /dev/tty
prompt_yn() {
    local prompt_msg="$1"
    local default_ans="${2:-n}"

    if [ "${dry_run:-0}" -eq 1 ]; then
        echo -e "  ${c_dim}[Giả lập] Mặc định chọn: [${default_ans}]${c_reset}"
        [ "$default_ans" = "y" ] && return 0 || return 1
    fi

    local can_prompt=0
    local prompt_from_tty=0
    if [ -t 0 ]; then
        can_prompt=1
    elif [ -e /dev/tty ] && [ -r /dev/tty ]; then
        can_prompt=1
        prompt_from_tty=1
    fi

    if [ "$can_prompt" -eq 0 ]; then
        [ "$default_ans" = "y" ] && return 0 || return 1
    fi

    local ans="$default_ans"
    if [ "$prompt_from_tty" -eq 1 ]; then
        read -r -p "$prompt_msg" ans < /dev/tty || ans="$default_ans"
    else
        read -r -p "$prompt_msg" ans || ans="$default_ans"
    fi

    case "$ans" in
        [yY][eE][sS]|[yY])
            return 0
            ;;
        *)
            return 1
            ;;
    esac
}

# run command with sudo via tty if needed
run_sudo() {
    if [ "$EUID" -eq 0 ]; then
        "$@"
        return $?
    fi

    if ! command -v sudo >/dev/null 2>&1; then
        return 1
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

# error exit
err() {
    log_step "$lbl_err" "$1"
    exit 1
}

# detect architecture
get_arch() {
    local os
    local arch
    os=$(uname -s | tr '[:upper:]' '[:lower:]')
    arch=$(uname -m)

    if [ "$os" != "linux" ]; then
        err "Hệ điều hành không hỗ trợ: $os (Clak chỉ hỗ trợ Linux)"
    fi

    case "$arch" in
        x86_64|amd64)
            echo "linux-x86_64"
            ;;
        *)
            err "Kiến trúc CPU không hỗ trợ: $arch (hiện tại hỗ trợ x86_64)"
            ;;
    esac
}

# ensure fcitx5 is installed or prompt user to install it
ensure_fcitx5() {
    local is_sim="${1:-0}"
    if command -v fcitx5 >/dev/null 2>&1; then
        local f_ver
        f_ver=$(fcitx5 --version 2>/dev/null | head -n1 || echo "fcitx5")
        log_step "$lbl_check" "Đã phát hiện Fcitx5: ${c_cyan}${f_ver}${c_reset}"
        return 0
    fi

    log_step "$lbl_warn" "Chưa tìm thấy Fcitx5 trên hệ thống"

    local pkg_manager=""
    local pkg_hint=""

    if command -v pacman >/dev/null 2>&1; then
        pkg_manager="pacman"
        pkg_hint="sudo pacman -S fcitx5 fcitx5-configtool"
    elif command -v apt-get >/dev/null 2>&1; then
        pkg_manager="apt"
        pkg_hint="sudo apt install fcitx5 fcitx5-frontend-gtk3 fcitx5-frontend-gtk4 fcitx5-frontend-qt5 fcitx5-config-qt"
    elif command -v dnf >/dev/null 2>&1; then
        pkg_manager="dnf"
        pkg_hint="sudo dnf install fcitx5 fcitx5-configtool"
    elif command -v zypper >/dev/null 2>&1; then
        pkg_manager="zypper"
        pkg_hint="sudo zypper install fcitx5"
    fi

    if [ "$is_sim" -eq 1 ]; then
        if [ -n "$pkg_hint" ]; then
            log_step "$lbl_check" "[Giả lập] Sẽ hỏi người dùng có muốn cài đặt Fcitx5 qua ${pkg_manager} không [y/N]"
        else
            log_step "$lbl_warn" "Cần cài đặt Fcitx5 bằng trình quản lý gói của hệ thống trước khi cài Clak"
        fi
        return 0
    fi

    if [ -n "$pkg_manager" ]; then
        echo -e "  ${c_yellow}• Clak là bộ gõ hoạt động trên nền Fcitx5 (framework quản lý input method)${c_reset}"
        echo -e "  ${c_yellow}• Để sử dụng Clak, hệ thống cần được cài đặt Fcitx5 trước${c_reset}"
        if prompt_yn "  Bạn có muốn cài đặt Fcitx5 ngay bây giờ không? [y/N]: " "n"; then
            log_step "$lbl_install" "Đang cài đặt Fcitx5 qua ${pkg_manager}..."
            if [ "$pkg_manager" = "pacman" ]; then
                run_sudo pacman -S --noconfirm fcitx5 fcitx5-configtool || true
            elif [ "$pkg_manager" = "apt" ]; then
                run_sudo apt-get update -y 2>/dev/null || true
                run_sudo apt-get install -y fcitx5 fcitx5-frontend-gtk3 fcitx5-frontend-gtk4 fcitx5-frontend-qt5 fcitx5-config-qt || true
                command -v im-config >/dev/null 2>&1 && im-config -n fcitx5 2>/dev/null || true
            elif [ "$pkg_manager" = "dnf" ]; then
                run_sudo dnf install -y fcitx5 fcitx5-configtool || true
            elif [ "$pkg_manager" = "zypper" ]; then
                run_sudo zypper install -y fcitx5 || true
            fi

            # verify after install
            if command -v fcitx5 >/dev/null 2>&1; then
                local f_ver
                f_ver=$(fcitx5 --version 2>/dev/null | head -n1 || echo "fcitx5")
                log_step "$lbl_check" "Cài đặt Fcitx5 thành công: ${c_cyan}${f_ver}${c_reset}"
                return 0
            else
                err "Cài đặt Fcitx5 không thành công hoặc bị hủy. Vui lòng cài thủ công: ${pkg_hint}"
            fi
        else
            echo -e "  ${c_dim}Bạn đã chọn không cài Fcitx5. Quá trình cài đặt Clak dừng lại tại đây.${c_reset}"
            if [ -n "$pkg_hint" ]; then
                echo -e "  ${c_yellow}Gợi ý cài đặt khi bạn sẵn sàng:${c_reset} ${c_bold}${pkg_hint}${c_reset}"
            fi
            exit 0
        fi
    else
        echo -e "  ${c_yellow}• Clak là bộ gõ hoạt động trên nền Fcitx5, không thể hoạt động độc lập${c_reset}"
        if [ -n "$pkg_hint" ]; then
            echo -e "  ${c_yellow}Vui lòng cài Fcitx5 trước:${c_reset} ${c_bold}${pkg_hint}${c_reset}"
        else
            echo -e "  ${c_yellow}Vui lòng cài gói fcitx5 bằng trình quản lý gói của hệ thống trước khi cài Clak${c_reset}"
        fi
        exit 1
    fi
}

# explain why sudo is required for uinput
print_uinput_sudo_notice() {
    echo ""
    echo -e "  ${c_red}╭─ CẦN QUYỀN SUDO: CẤU HÌNH /dev/uinput ─────────────────────╮${c_reset}"
    echo -e "  ${c_red}│${c_reset}  Clak dùng thiết bị ảo ${c_bold}/dev/uinput${c_reset} để gửi phím trực tiếp,  ${c_red}│${c_reset}"
    echo -e "  ${c_red}│${c_reset}  giúp ${c_green}chống nuốt chữ và mất dấu${c_reset} trên các ứng dụng:         ${c_red}│${c_reset}"
    echo -e "  ${c_red}│${c_reset}   • Terminal: Kitty, Ghostty, Alacritty                    ${c_red}│${c_reset}"
    echo -e "  ${c_red}│${c_reset}   • Trình duyệt: Zen Browser, Firefox Gecko, Chromium      ${c_red}│${c_reset}"
    echo -e "  ${c_red}│${c_reset}   • Họ VSCode: Antigravity IDE, Cursor, VSCode, Windsurf   ${c_red}│${c_reset}"
    echo -e "  ${c_red}│${c_reset}   • Và nhiều thứ khác nữa                                  ${c_red}│${c_reset}"
    echo -e "  ${c_red}│${c_reset}                                                            ${c_red}│${c_reset}"
    echo -e "  ${c_red}│${c_reset}  Thao tác: Tạo file ${c_cyan}/etc/udev/rules.d/99-uinput.rules${c_reset}      ${c_red}│${c_reset}"
    echo -e "  ${c_red}│${c_reset}  Hệ thống sẽ yêu cầu mật khẩu sudo bên dưới để kích hoạt.  ${c_red}│${c_reset}"
    echo -e "  ${c_red}╰────────────────────────────────────────────────────────────╯${c_reset}"
    echo ""
}

# explain why sudo is required for system install
print_system_sudo_notice() {
    echo ""
    echo -e "  ${c_red}╭─ CẦN QUYỀN SUDO: CÀI VÀO THƯ MỤC HỆ THỐNG ─────────────────╮${c_reset}"
    echo -e "  ${c_red}│${c_reset}  Bạn đã chọn cài đặt Clak vào thư mục: ${c_bold}/usr/lib/fcitx5${c_reset}     ${c_red}│${c_reset}"
    echo -e "  ${c_red}│${c_reset}  Cần sudo ở bước này để sao chép file addon vào hệ thống.  ${c_red}│${c_reset}"
    echo -e "  ${c_red}╰────────────────────────────────────────────────────────────╯${c_reset}"
    echo ""
}

# detect and configure wps office compatibility
configure_wps_compatibility() {
    local is_sim="${1:-0}"
    local has_wps=0
    if command -v wps >/dev/null 2>&1 || [ -d /usr/lib/office6 ] || compgen -G "/usr/share/applications/wps-office-*.desktop" >/dev/null; then
        has_wps=1
    fi

    if [ "$has_wps" -eq 1 ]; then
        log_step "$lbl_wps" "Phát hiện hệ thống có cài đặt WPS Office (ứng dụng Qt5 XWayland)"
        echo -e "  ${c_yellow}• WPS Office sử dụng Qt5 nội bộ, cần biến QT_IM_MODULE=fcitx để nạp bộ gõ${c_reset}"
        echo -e "  ${c_yellow}• Clak tự động whitelist WPS: kích hoạt uinput trực tiếp và lọc Surrounding Text rác ('10')${c_reset}"
        echo -e "  ${c_yellow}• Hoàn toàn không ghi đè file gốc của WPS, không ảnh hưởng gì tới app chính${c_reset}"
        echo -e "  ${c_yellow}• Bạn vẫn có thể sử dụng và cập nhật WPS Office bình thường${c_reset}"

        if [ "$is_sim" -eq 1 ]; then
            echo -e "  ${c_yellow}  [Giả lập] Tối ưu shortcut WPS tại ~/.local/share/applications/ (an toàn khi update hệ thống)${c_reset}"
            log_step "$lbl_wps" "Đã tối ưu tương thích WPS Office (${c_green}sử dụng được ngay với mọi launcher${c_reset})"
        else
            mkdir -p "${HOME}/.local/share/applications"
            local patched_count=0
            for df in /usr/share/applications/wps-office-*.desktop; do
                if [ -f "$df" ]; then
                    local bname
                    bname=$(basename "$df")
                    sed 's|^Exec=/usr/bin/|Exec=env QT_IM_MODULE=fcitx /usr/bin/|' "$df" > "${HOME}/.local/share/applications/${bname}"
                    patched_count=$((patched_count + 1))
                fi
            done
            if [ "$patched_count" -gt 0 ]; then
                command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "${HOME}/.local/share/applications" 2>/dev/null || true
                log_step "$lbl_wps" "Đã tạo shortcut tương thích tại ${c_accent}~/.local/share/applications/${c_reset} (${c_green}${patched_count} ứng dụng${c_reset})"
            fi
            # configure systemd environment.d to export QT_IM_MODULE=fcitx
            mkdir -p "${HOME}/.config/environment.d"
            echo "QT_IM_MODULE=fcitx" > "${HOME}/.config/environment.d/99-clak-wps.conf"
            log_step "$lbl_wps" "Đã khai báo biến ${c_accent}QT_IM_MODULE=fcitx${c_reset} trong environment.d (${c_green}hoàn toàn không ghi đè file gốc của WPS${c_reset})"

            # create user-level terminal wrappers in ~/.local/bin for terminal usage
            mkdir -p "${HOME}/.local/bin"
            local bin_count=0
            for b in wps wpp et wpspdf; do
                if command -v "/usr/bin/$b" >/dev/null 2>&1; then
                    cat << 'EOF' > "${HOME}/.local/bin/$b"
#!/bin/sh
exec env QT_IM_MODULE=fcitx "/usr/bin/$(basename "$0")" "$@"
EOF
                    chmod +x "${HOME}/.local/bin/$b"
                    bin_count=$((bin_count + 1))
                fi
            done
            if [ "$bin_count" -gt 0 ]; then
                log_step "$lbl_wps" "Đã tạo wrapper tương thích tại ${c_accent}~/.local/bin/${c_reset} (${c_green}${bin_count} lệnh${c_reset})"
            fi
        fi
    fi
}

# configure autostart with system
configure_autostart() {
    local is_sim="${1:-0}"
    if [ "$is_sim" -eq 1 ]; then
        log_step "$lbl_install" "[Giả lập] Tự động bật khởi động Clak Tiếng Việt cùng hệ thống"
        return
    fi

    local autostart_dir="${HOME}/.config/autostart"
    mkdir -p "$autostart_dir"
    cat << 'EOF' > "${autostart_dir}/clak-autostart.desktop"
[Desktop Entry]
Type=Application
Name=Clak Vietnamese Input Method
Comment=Autostart Fcitx5 with Clak input method
Exec=fcitx5 -d
Icon=org.fcitx.Fcitx5
Terminal=false
Categories=System;Utility;
StartupNotify=false
X-GNOME-Autostart-Phase=Applications
X-GNOME-AutoRestart=true
X-GNOME-Autostart-Notify=false
X-KDE-autostart-after=panel
EOF

    local env_dir="${HOME}/.config/environment.d"
    mkdir -p "$env_dir"
    cat << 'EOF' > "${env_dir}/99-clak-im.conf"
GTK_IM_MODULE=fcitx
QT_IM_MODULE=fcitx
XMODIFIERS=@im=fcitx
INPUT_METHOD=fcitx5
SDL_IM_MODULE=fcitx
EOF

    # stop running fcitx5 before writing profile to prevent in-memory profile overwrite
    if command -v fcitx5 >/dev/null 2>&1 && pidof fcitx5 >/dev/null 2>&1; then
        pkill -u "$(id -u)" fcitx5 2>/dev/null || true
        sleep 0.2
    fi

    local profile_file="${HOME}/.config/fcitx5/profile"
    if [ ! -f "$profile_file" ]; then
        mkdir -p "${HOME}/.config/fcitx5"
        cat << 'EOF' > "$profile_file"
[Groups/0]
Name=Default
Default Layout=us
DefaultIM=clak

[Groups/0/Items/0]
Name=keyboard-us

[Groups/0/Items/1]
Name=clak

[GroupOrder]
0=Default
EOF
        if ! grep -q "Name=clak" "$profile_file" 2>/dev/null; then
            local count
            count=$(grep -c "\[Groups/0/Items/" "$profile_file" 2>/dev/null || echo "1")
            sed -i "/\[GroupOrder\]/i \\\n[Groups/0/Items/${count}]\nName=clak\n" "$profile_file" 2>/dev/null || true
        fi
        sed -i 's/^DefaultIM=.*/DefaultIM=clak/' "$profile_file" 2>/dev/null || true
        # ensure base layout is us (not vn or vn-us, which map dead keys and break telex/vni)
        sed -i 's/^Default Layout=vn.*/Default Layout=us/' "$profile_file" 2>/dev/null || true
    fi

    # configure gtk im module for gnome and wayland compatibility
    for gtk_ver in "gtk-3.0" "gtk-4.0"; do
        local gtk_dir="${HOME}/.config/${gtk_ver}"
        local gtk_ini="${gtk_dir}/settings.ini"
        mkdir -p "$gtk_dir"
        if [ ! -f "$gtk_ini" ]; then
            cat << 'EOF' > "$gtk_ini"
[Settings]
gtk-im-module=fcitx
EOF
        elif ! grep -q "gtk-im-module" "$gtk_ini" 2>/dev/null; then
            if grep -q '^\[Settings\]' "$gtk_ini" 2>/dev/null; then
                sed -i '/^\[Settings\]/a gtk-im-module=fcitx' "$gtk_ini" 2>/dev/null || true
            else
                printf "\n[Settings]\ngtk-im-module=fcitx\n" >> "$gtk_ini" 2>/dev/null || true
            fi
        fi
    done

    log_step "$lbl_install" "Đã kích hoạt tự động chạy Clak Tiếng Việt cùng hệ thống"
}

# detect conflicting ibus and ask user before making changes
disable_ibus_conflict() {
    local is_sim="${1:-0}"
    local has_ibus=0
    if pidof ibus-daemon >/dev/null 2>&1 || [ -f "${HOME}/.config/autostart/ibus-daemon.desktop" ]; then
        has_ibus=1
    fi

    if [ "$has_ibus" -eq 1 ]; then
        if [ "$is_sim" -eq 1 ]; then
            log_step "$lbl_check" "Phát hiện IBus đang hoạt động (khi cài thật sẽ hỏi trước khi chuyển sang Fcitx5)"
            return
        fi

        log_step "$lbl_warn" "Phát hiện IBus đang chạy trên hệ thống"
        echo -e "  ${c_yellow}• Cả IBus và Fcitx5 cùng chạy có thể gây xung đột bộ gõ${c_reset}"
        echo -e "  ${c_yellow}• Nếu bạn đang dùng IBus cho bộ gõ khác (như ibus-mozc, ibus-hangul), bạn có thể giữ lại${c_reset}"
        if prompt_yn "  Bạn có muốn chuyển sang Fcitx5 và tạm dừng IBus không? [y/N]: " "n"; then
            if pidof ibus-daemon >/dev/null 2>&1; then
                ibus exit 2>/dev/null || true
                pkill -u "$(id -u)" ibus-daemon 2>/dev/null || true
            fi

            if command -v systemctl >/dev/null 2>&1 && systemctl --user is-active ibus-daemon.service >/dev/null 2>&1; then
                systemctl --user stop ibus-daemon.service 2>/dev/null || true
                systemctl --user disable ibus-daemon.service 2>/dev/null || true
            fi

            for f in "${HOME}/.config/autostart/ibus-daemon.desktop" "${HOME}/.config/autostart/ibus-ui-gtk3.desktop"; do
                if [ -f "$f" ]; then
                    mv "$f" "${f}.disabled-by-clak" 2>/dev/null || true
                fi
            done

            if command -v im-config >/dev/null 2>&1; then
                im-config -n fcitx5 2>/dev/null || true
            fi
            log_step "$lbl_install" "Đã ưu tiên Fcitx5 và tạm dừng xung đột IBus"
        else
            log_step "$lbl_check" "Giữ nguyên cấu hình IBus hiện tại theo lựa chọn của bạn"
            echo -e "  ${c_dim}Gợi ý: Nếu sau này muốn chuyển sang Fcitx5, bạn có thể chạy: im-config -n fcitx5${c_reset}"
        fi
    fi
}

# configure fcitx5 active by default and share input state
configure_fcitx5_behavior() {
    local is_sim="${1:-0}"
    if [ "$is_sim" -eq 1 ]; then
        return
    fi

    local cfg_dir="${HOME}/.config/fcitx5"
    local cfg_file="${cfg_dir}/config"
    mkdir -p "$cfg_dir"

    if [ -f "$cfg_file" ]; then
        # backup existing config before modifying
        [ ! -f "${cfg_file}.bak-clak" ] && cp "$cfg_file" "${cfg_file}.bak-clak" 2>/dev/null || true

        if grep -q '^\[Behavior\]' "$cfg_file" 2>/dev/null; then
            if ! grep -q '^ActiveByDefault=' "$cfg_file" 2>/dev/null; then
                sed -i '/^\[Behavior\]/a ActiveByDefault=True' "$cfg_file" 2>/dev/null || true
            fi
            if ! grep -q '^ShareInputState=' "$cfg_file" 2>/dev/null; then
                sed -i '/^\[Behavior\]/a ShareInputState=All' "$cfg_file" 2>/dev/null || true
            fi
        else
            cat << 'EOF' >> "$cfg_file"

[Behavior]
ActiveByDefault=True
ShareInputState=All
EOF
        fi
    else
        cat << 'EOF' > "$cfg_file"
[Behavior]
ActiveByDefault=True
ShareInputState=All
EOF
    fi
}

# apply im environment variables to live session
apply_live_environment() {
    local is_sim="${1:-0}"
    if [ "$is_sim" -eq 1 ]; then
        return
    fi

    export GTK_IM_MODULE=fcitx
    export QT_IM_MODULE=fcitx
    export XMODIFIERS=@im=fcitx
    export SDL_IM_MODULE=fcitx

    if command -v systemctl >/dev/null 2>&1 && systemctl --user show-environment >/dev/null 2>&1; then
        systemctl --user set-environment \
            GTK_IM_MODULE=fcitx \
            QT_IM_MODULE=fcitx \
            XMODIFIERS=@im=fcitx \
            SDL_IM_MODULE=fcitx 2>/dev/null || true
    fi

    if command -v dbus-update-activation-environment >/dev/null 2>&1; then
        dbus-update-activation-environment --systemd \
            GTK_IM_MODULE=fcitx \
            QT_IM_MODULE=fcitx \
            XMODIFIERS=@im=fcitx \
            SDL_IM_MODULE=fcitx 2>/dev/null || true
    fi
}

# detect and configure desktop environments and window managers
configure_desktop_environment() {
    local is_sim="${1:-0}"
    local de="${XDG_CURRENT_DESKTOP:-}"
    local session_type="${XDG_SESSION_TYPE:-}"

    # 1. Hyprland
    if [ -n "${HYPRLAND_INSTANCE_SIGNATURE:-}" ] || [ "$de" = "Hyprland" ] || [ -d "${HOME}/.config/hypr" ]; then
        log_step "$lbl_de" "Phát hiện môi trường ${c_cyan}Hyprland${c_reset}"
        local hypr_cfg=""
        local is_lua=0
        if [ -f "${HOME}/.config/hypr/hyprland.lua" ]; then
            hypr_cfg="${HOME}/.config/hypr/hyprland.lua"
            is_lua=1
        elif [ -f "${HOME}/.config/hypr/hyprland.conf" ]; then
            hypr_cfg="${HOME}/.config/hypr/hyprland.conf"
        fi

        local need_env=1
        if [ -n "$hypr_cfg" ] && grep -q "XMODIFIERS" "$hypr_cfg" 2>/dev/null; then
            need_env=0
        fi

        if [ "$need_env" -eq 0 ]; then
            log_step "$lbl_de" "Cấu hình biến môi trường Hyprland đã có sẵn (${c_green}OK${c_reset})"
        else
            echo -e "  ${c_yellow}• Hyprland cần khai báo biến môi trường (XMODIFIERS, QT_IM_MODULE...)${c_reset}"
            echo -e "  ${c_yellow}• Giúp các ứng dụng XWayland, Java, game nhận diện đúng bộ gõ Fcitx5${c_reset}"
            if [ -n "$hypr_cfg" ]; then
                echo -e "  ${c_yellow}• File cấu hình: ${c_accent}${hypr_cfg}${c_reset}"
                if [ "$is_sim" -eq 1 ]; then
                    echo -e "  ${c_dim}[Giả lập] Sẽ hỏi người dùng có muốn tự động cấu hình Hyprland không [y/N]${c_reset}"
                elif prompt_yn "  Bạn có muốn Clak tự động thêm biến môi trường vào ${hypr_cfg} không? [y/N]: " "y"; then
                    if [ "$is_lua" -eq 1 ]; then
                        cat << 'EOF' >> "$hypr_cfg"

-- clak input method environment
hl.env("XMODIFIERS", "@im=fcitx")
hl.env("INPUT_METHOD", "fcitx5")
hl.env("SDL_IM_MODULE", "fcitx")
hl.env("QT_IM_MODULE", "fcitx")
EOF
                    else
                        cat << 'EOF' >> "$hypr_cfg"

# clak input method environment
env = XMODIFIERS,@im=fcitx
env = INPUT_METHOD,fcitx5
env = SDL_IM_MODULE,fcitx
env = QT_IM_MODULE,fcitx
exec-once = fcitx5 -d
EOF
                    fi
                    command -v hyprctl >/dev/null 2>&1 && hyprctl reload >/dev/null 2>&1 || true
                    log_step "$lbl_de" "Đã cập nhật cấu hình Hyprland thành công"
                else
                    echo ""
                    echo -e "  ${c_cyan}╭─ CẤU HÌNH THỦ CÔNG CHO HYPRLAND ───────────────────────────╮${c_reset}"
                    if [ "$is_lua" -eq 1 ]; then
                        echo -e "  ${c_cyan}│${c_reset}  Thêm các dòng sau vào file ${c_accent}${hypr_cfg}${c_reset}:"
                        echo -e "  ${c_cyan}│${c_reset}    ${c_bold}hl.env(\"XMODIFIERS\", \"@im=fcitx\")${c_reset}"
                        echo -e "  ${c_cyan}│${c_reset}    ${c_bold}hl.env(\"INPUT_METHOD\", \"fcitx5\")${c_reset}"
                        echo -e "  ${c_cyan}│${c_reset}    ${c_bold}hl.env(\"SDL_IM_MODULE\", \"fcitx\")${c_reset}"
                        echo -e "  ${c_cyan}│${c_reset}    ${c_bold}hl.env(\"QT_IM_MODULE\", \"fcitx\")${c_reset}"
                    else
                        echo -e "  ${c_cyan}│${c_reset}  Thêm các dòng sau vào file ${c_accent}${hypr_cfg}${c_reset}:"
                        echo -e "  ${c_cyan}│${c_reset}    ${c_bold}env = XMODIFIERS,@im=fcitx${c_reset}"
                        echo -e "  ${c_cyan}│${c_reset}    ${c_bold}env = INPUT_METHOD,fcitx5${c_reset}"
                        echo -e "  ${c_cyan}│${c_reset}    ${c_bold}env = SDL_IM_MODULE,fcitx${c_reset}"
                        echo -e "  ${c_cyan}│${c_reset}    ${c_bold}env = QT_IM_MODULE,fcitx${c_reset}"
                        echo -e "  ${c_cyan}│${c_reset}    ${c_bold}exec-once = fcitx5 -d${c_reset}"
                    fi
                    echo -e "  ${c_cyan}╰────────────────────────────────────────────────────────────╯${c_reset}"
                    echo ""
                fi
            fi
        fi
        return
    fi

    # 2. KDE Plasma
    if echo "$de" | grep -qi "kde\|plasma"; then
        log_step "$lbl_de" "Phát hiện môi trường ${c_cyan}KDE Plasma${c_reset}"
        if [ "$session_type" = "wayland" ]; then
            local kwinrc="${HOME}/.config/kwinrc"
            local cur_im=""
            if command -v kreadconfig6 >/dev/null 2>&1; then
                cur_im=$(kreadconfig6 --file kwinrc --group Wayland --key InputMethod 2>/dev/null || true)
            elif command -v kreadconfig5 >/dev/null 2>&1; then
                cur_im=$(kreadconfig5 --file kwinrc --group Wayland --key InputMethod 2>/dev/null || true)
            elif [ -f "$kwinrc" ]; then
                cur_im=$(grep -i "InputMethod=" "$kwinrc" 2>/dev/null | cut -d= -f2 || true)
            fi

            if echo "$cur_im" | grep -qi "fcitx"; then
                log_step "$lbl_de" "VirtualKeyboard trên KDE Plasma đã đặt Fcitx5 (${c_green}OK${c_reset})"
            else
                echo -e "  ${c_yellow}• Trên KDE Plasma Wayland, KWin cần đặt VirtualKeyboard thành Fcitx5${c_reset}"
                echo -e "  ${c_yellow}• Giúp bộ gõ tích hợp mượt mà và hiển thị candidate popup đúng vị trí${c_reset}"
                if [ "$is_sim" -eq 1 ]; then
                    echo -e "  ${c_dim}[Giả lập] Sẽ hỏi người dùng có muốn cấu hình VirtualKeyboard không [y/N]${c_reset}"
                elif prompt_yn "  Bạn có muốn Clak tự động cấu hình VirtualKeyboard cho KDE Plasma không? [y/N]: " "y"; then
                    local set_ok=0
                    if command -v kwriteconfig6 >/dev/null 2>&1; then
                        kwriteconfig6 --file kwinrc --group Wayland --key InputMethod /usr/share/applications/org.fcitx.Fcitx5.desktop 2>/dev/null && set_ok=1 || true
                    elif command -v kwriteconfig5 >/dev/null 2>&1; then
                        kwriteconfig5 --file kwinrc --group Wayland --key InputMethod /usr/share/applications/org.fcitx.Fcitx5.desktop 2>/dev/null && set_ok=1 || true
                    fi
                    if [ "$set_ok" -eq 1 ]; then
                        log_step "$lbl_de" "Đã cấu hình VirtualKeyboard thành Fcitx5 cho KDE Plasma"
                    else
                        log_step "$lbl_warn" "Chưa tự động cấu hình được kwinrc, vui lòng chọn thủ công"
                        echo ""
                        echo -e "  ${c_cyan}╭─ CẤU HÌNH THỦ CÔNG CHO KDE PLASMA ─────────────────────────╮${c_reset}"
                        echo -e "  ${c_cyan}│${c_reset}  Mở ${c_bold}System Settings${c_reset} -> ${c_bold}Keyboard${c_reset} -> ${c_bold}Virtual Keyboard${c_reset}       ${c_cyan}│${c_reset}"
                        echo -e "  ${c_cyan}│${c_reset}  Chọn: ${c_green}Fcitx 5 Wayland Launcher${c_reset} rồi bấm Apply             ${c_cyan}│${c_reset}"
                        echo -e "  ${c_cyan}╰────────────────────────────────────────────────────────────╯${c_reset}"
                        echo ""
                    fi
                else
                    echo ""
                    echo -e "  ${c_cyan}╭─ CẤU HÌNH THỦ CÔNG CHO KDE PLASMA ─────────────────────────╮${c_reset}"
                    echo -e "  ${c_cyan}│${c_reset}  Mở ${c_bold}System Settings${c_reset} -> ${c_bold}Keyboard${c_reset} -> ${c_bold}Virtual Keyboard${c_reset}       ${c_cyan}│${c_reset}"
                    echo -e "  ${c_cyan}│${c_reset}  Chọn: ${c_green}Fcitx 5 Wayland Launcher${c_reset} rồi bấm Apply             ${c_cyan}│${c_reset}"
                    echo -e "  ${c_cyan}╰────────────────────────────────────────────────────────────╯${c_reset}"
                    echo ""
                fi
            fi
        fi
        return
    fi

    # 3. GNOME
    if echo "$de" | grep -qi "gnome"; then
        log_step "$lbl_de" "Phát hiện môi trường ${c_cyan}GNOME${c_reset}"
        if command -v gsettings >/dev/null 2>&1; then
            local cur_src
            cur_src=$(gsettings get org.gnome.desktop.input-sources sources 2>/dev/null || true)
            if echo "$cur_src" | grep -q "vn"; then
                echo -e "  ${c_yellow}• GNOME đang kích hoạt layout bàn phím Tiếng Việt của hệ điều hành${c_reset}"
                echo -e "  ${c_yellow}• Bàn phím này có phím chết (dead keys) sẽ xung đột với Fcitx5 và Clak${c_reset}"
                if [ "$is_sim" -eq 1 ]; then
                    echo -e "  ${c_dim}[Giả lập] Sẽ hỏi người dùng có muốn chuyển layout về chuẩn 'us' không [y/N]${c_reset}"
                elif prompt_yn "  Bạn có muốn Clak chuyển layout GNOME về chuẩn 'us' (tiếng Anh) để gõ qua Clak không? [y/N]: " "y"; then
                    local new_src
                    new_src=$(echo "$cur_src" | sed -e "s/'vn+us'/'us'/g" -e "s/'vn'/'us'/g")
                    gsettings set org.gnome.desktop.input-sources sources "$new_src" 2>/dev/null || true
                    log_step "$lbl_de" "Đã chuyển layout GNOME sang chuẩn 'us' thành công"
                else
                    echo ""
                    echo -e "  ${c_cyan}╭─ LƯU Ý CHO GNOME ──────────────────────────────────────────╮${c_reset}"
                    echo -e "  ${c_cyan}│${c_reset}  Mở ${c_bold}Settings${c_reset} -> ${c_bold}Keyboard${c_reset} -> mục ${c_bold}Input Sources${c_reset}                ${c_cyan}│${c_reset}"
                    echo -e "  ${c_cyan}│${c_reset}  Chỉ giữ lại ${c_green}English (US)${c_reset} và xóa bỏ bộ gõ Tiếng Việt mặc định ${c_cyan}│${c_reset}"
                    echo -e "  ${c_cyan}╰────────────────────────────────────────────────────────────╯${c_reset}"
                    echo ""
                fi
            fi
        fi
        return
    fi

    # 4. Sway / Niri / River / i3
    local wm_file=""
    local wm_name=""
    local wm_cmd=""
    if [ -n "${SWAYSOCK:-}" ] || [ "$de" = "sway" ] || [ -f "${HOME}/.config/sway/config" ]; then
        wm_file="${HOME}/.config/sway/config"
        wm_name="Sway"
        wm_cmd="exec --no-startup-id fcitx5 -d"
    elif [ -n "${NIRI_SOCKET:-}" ] || [ -f "${HOME}/.config/niri/config.kdl" ]; then
        wm_file="${HOME}/.config/niri/config.kdl"
        wm_name="Niri"
        wm_cmd='spawn-at-startup "fcitx5" "-d"'
    elif [ -f "${HOME}/.config/river/init" ]; then
        wm_file="${HOME}/.config/river/init"
        wm_name="River"
        wm_cmd="riverctl spawn 'fcitx5 -d'"
    elif [ -f "${HOME}/.config/i3/config" ] || [ "$de" = "i3" ]; then
        wm_file="${HOME}/.config/i3/config"
        wm_name="i3"
        wm_cmd="exec --no-startup-id fcitx5 -d"
    fi

    if [ -n "$wm_name" ]; then
        log_step "$lbl_de" "Phát hiện Window Manager: ${c_cyan}${wm_name}${c_reset}"
        if [ -f "$wm_file" ] && grep -q "fcitx5" "$wm_file" 2>/dev/null; then
            log_step "$lbl_de" "Lệnh khởi động fcitx5 đã có trong ${wm_name} (${c_green}OK${c_reset})"
        else
            echo -e "  ${c_yellow}• ${wm_name} không tự động chạy các ứng dụng trong ~/.config/autostart${c_reset}"
            echo -e "  ${c_yellow}• Cần thêm lệnh khởi động Fcitx5 vào file cấu hình: ${c_accent}${wm_file}${c_reset}"
            if [ "$is_sim" -eq 1 ]; then
                echo -e "  ${c_dim}[Giả lập] Sẽ hỏi người dùng có muốn thêm lệnh khởi động vào ${wm_name} không [y/N]${c_reset}"
            elif [ -f "$wm_file" ] && prompt_yn "  Bạn có muốn Clak tự động thêm dòng khởi động vào ${wm_file} không? [y/N]: " "y"; then
                printf "\n# clak autostart\n%s\n" "$wm_cmd" >> "$wm_file"
                log_step "$lbl_de" "Đã thêm lệnh khởi động fcitx5 vào ${wm_name}"
            else
                echo ""
                echo -e "  ${c_cyan}╭─ CẤU HÌNH KHỞI ĐỘNG CHO ${wm_name} ────────────────────────╮${c_reset}"
                echo -e "  ${c_cyan}│${c_reset}  Thêm dòng sau vào file cấu hình ${c_accent}${wm_file}${c_reset}:"
                echo -e "  ${c_cyan}│${c_reset}    ${c_bold}${wm_cmd}${c_reset}"
                echo -e "  ${c_cyan}╰────────────────────────────────────────────────────────────╯${c_reset}"
                echo ""
            fi
        fi
    fi
}

# detect and configure jetbrains ides for wayland/xwayland stability
configure_jetbrains_compatibility() {
    local is_sim="${1:-0}"
    local jb_root="${HOME}/.config/JetBrains"

    # only run if jetbrains is detected on the system
    local jb_dirs=()
    if [ -d "$jb_root" ]; then
        for d in "$jb_root"/*; do
            if [ -d "$d" ]; then
                local bname
                bname=$(basename "$d")
                case "$bname" in
                    Idea*|IntelliJ*|PyCharm*|CLion*|WebStorm*|Rider*|DataGrip*|RustRover*|GoLand*|PhpStorm*|RubyMine*|Aqua*|Fleet*|DataSpell*|Gateway*|JetBrainsGateway*|MPS*|AndroidStudio*|*Studio*)
                        jb_dirs+=("$d")
                        ;;
                esac
            fi
        done
    fi

    # also check android studio config path
    if [ -d "${HOME}/.config/Google" ]; then
        for d in "${HOME}/.config/Google"/AndroidStudio*; do
            [ -d "$d" ] && jb_dirs+=("$d")
        done
    fi

    # if no jetbrains product is installed, exit silently
    if [ ${#jb_dirs[@]} -eq 0 ]; then
        return 0
    fi

    # check which ides still need the xwayland vmoptions fix
    local unconfigured=()
    for d in "${jb_dirs[@]}"; do
        local has_fix=0
        for vmo in "$d"/*64.vmoptions; do
            if [ -f "$vmo" ] && grep -q "sun\.awt\.X11\.XToolkit" "$vmo" 2>/dev/null && grep -q "idea\.input\.method\.disabler\.notification\.muted=true" "$vmo" 2>/dev/null; then
                has_fix=1
                break
            fi
        done
        if [ "$has_fix" -eq 0 ]; then
            unconfigured+=("$d")
        fi
    done

    log_step "$lbl_jb" "Phát hiện hệ thống có cài đặt JetBrains IDE (${c_cyan}${#jb_dirs[@]} phiên bản${c_reset})"

    if [ ${#unconfigured[@]} -eq 0 ]; then
        log_step "$lbl_jb" "Tất cả IDE JetBrains đã được tối ưu sẵn cho Wayland (${c_green}OK${c_reset})"
        return 0
    fi

    echo -e "  ${c_yellow}• Trên Wayland, JetBrains Runtime có lỗi mất focus bộ gõ khi chuyển tab (JBR-5672)${c_reset}"
    echo -e "  ${c_yellow}• Giải pháp: Ép JetBrains qua XWayland và tắt cơ chế disabler thông báo tắt bộ gõ${c_reset}"
    echo -e "  ${c_yellow}• Màn hình hiển thị sắc nét 1:1, không mất focus và gõ tiếng Việt ổn định tuyệt đối${c_reset}"

    if [ "$is_sim" -eq 1 ]; then
        echo -e "  ${c_dim}[Giả lập] Sẽ hỏi người dùng có muốn cấu hình vmoptions cho ${#unconfigured[@]} IDE JetBrains không [y/N]${c_reset}"
        return 0
    fi

    if prompt_yn "  Bạn có muốn Clak tự động cấu hình vmoptions cho ${#unconfigured[@]} IDE này không? [y/N]: " "y"; then
        local patched_count=0
        for d in "${unconfigured[@]}"; do
            local dir_name
            dir_name=$(basename "$d")
            local vmo_target=""

            local existing_vmo
            existing_vmo=$(compgen -G "$d/*64.vmoptions" | head -1 || true)
            if [ -n "$existing_vmo" ] && [ -f "$existing_vmo" ]; then
                vmo_target="$existing_vmo"
            else
                local prefix="idea"
                case "$dir_name" in
                    Idea*|IntelliJ*) prefix="idea" ;;
                    PyCharm*) prefix="pycharm" ;;
                    CLion*) prefix="clion" ;;
                    WebStorm*) prefix="webstorm" ;;
                    Rider*) prefix="rider" ;;
                    DataGrip*) prefix="datagrip" ;;
                    RustRover*) prefix="rustrover" ;;
                    GoLand*) prefix="goland" ;;
                    PhpStorm*) prefix="phpstorm" ;;
                    RubyMine*) prefix="rubymine" ;;
                    Aqua*) prefix="aqua" ;;
                    DataSpell*) prefix="dataspell" ;;
                    Gateway*|JetBrainsGateway*) prefix="gateway" ;;
                    MPS*) prefix="mps" ;;
                    *Studio*) prefix="studio" ;;
                esac
                vmo_target="${d}/${prefix}64.vmoptions"
            fi

            local patched=0
            if [ -f "$vmo_target" ]; then
                if ! grep -q "sun\.awt\.X11\.XToolkit" "$vmo_target" 2>/dev/null; then
                    printf "\n-Dawt.toolkit.name=sun.awt.X11.XToolkit\n" >> "$vmo_target"
                    patched=1
                fi
                if ! grep -q "idea\.input\.method\.disabler\.notification\.muted=true" "$vmo_target" 2>/dev/null; then
                    printf -- "-Didea.input.method.disabler.notification.muted=true\n" >> "$vmo_target"
                    patched=1
                fi
            else
                cat << 'EOF' > "$vmo_target"
-Dawt.toolkit.name=sun.awt.X11.XToolkit
-Didea.input.method.disabler.notification.muted=true
EOF
                patched=1
            fi
            if [ "$patched" -eq 1 ]; then
                patched_count=$((patched_count + 1))
            fi
        done
        log_step "$lbl_jb" "Đã tối ưu cấu hình vmoptions cho ${c_green}${patched_count} IDE JetBrains${c_reset}"
    else
        echo ""
        echo -e "  ${c_cyan}╭─ CẤU HÌNH THỦ CÔNG CHO JETBRAINS ──────────────────────────╮${c_reset}"
        echo -e "  ${c_cyan}│${c_reset}  Mở IDE JetBrains -> Menu ${c_bold}Help${c_reset} -> ${c_bold}Edit Custom VM Options...${c_reset}    ${c_cyan}│${c_reset}"
        echo -e "  ${c_cyan}│${c_reset}  Thêm 2 dòng sau vào cuối file rồi khởi động lại IDE:         ${c_cyan}│${c_reset}"
        echo -e "  ${c_cyan}│${c_reset}                                                               ${c_cyan}│${c_reset}"
        echo -e "  ${c_cyan}│${c_reset}    ${c_bold}-Dawt.toolkit.name=sun.awt.X11.XToolkit${c_reset}                    ${c_cyan}│${c_reset}"
        echo -e "  ${c_cyan}│${c_reset}    ${c_bold}-Didea.input.method.disabler.notification.muted=true${c_reset}       ${c_cyan}│${c_reset}"
        echo -e "  ${c_cyan}╰────────────────────────────────────────────────────────────╯${c_reset}"
        echo ""
    fi
}

# activate clak input method and reconnect wayland compositor
activate_clak_and_reconnect_compositor() {
    local is_sim="${1:-0}"
    if [ "$is_sim" -eq 1 ]; then
        return
    fi

    # best-effort compositor virtual keyboard reset
    if [ "${XDG_SESSION_TYPE:-}" = "wayland" ]; then
        case "${XDG_CURRENT_DESKTOP:-}" in
            KDE|kde|KDE-Plasma|plasma)
                local qdbus_cmd=""
                if command -v qdbus6 >/dev/null 2>&1; then
                    qdbus_cmd="qdbus6"
                elif command -v qdbus >/dev/null 2>&1; then
                    qdbus_cmd="qdbus"
                fi
                if [ -n "$qdbus_cmd" ]; then
                    $qdbus_cmd org.kde.KWin /VirtualKeyboard org.freedesktop.DBus.Properties.Set \
                        org.kde.kwin.VirtualKeyboard enabled false 2>/dev/null || true
                    sleep 0.1
                    $qdbus_cmd org.kde.KWin /VirtualKeyboard org.freedesktop.DBus.Properties.Set \
                        org.kde.kwin.VirtualKeyboard enabled true 2>/dev/null || true
                fi
                ;;
            GNOME|gnome|GNOME-Classic|X-Cinnamon|Cinnamon|cinnamon)
                if command -v gsettings >/dev/null 2>&1; then
                    local cur_src
                    cur_src=$(gsettings get org.gnome.desktop.input-sources sources 2>/dev/null || true)
                    if [ -n "$cur_src" ] && [ "$cur_src" != "@as []" ]; then
                        # If current GNOME layout is vn or vn+us, switch to us to avoid dead key conflicts with IME
                        if echo "$cur_src" | grep -q "vn"; then
                            cur_src=$(echo "$cur_src" | sed -e "s/'vn+us'/'us'/g" -e "s/'vn'/'us'/g")
                        fi
                        gsettings set org.gnome.desktop.input-sources sources "[]" 2>/dev/null || true
                        sleep 0.1
                        gsettings set org.gnome.desktop.input-sources sources "$cur_src" 2>/dev/null || true
                    fi
                fi
                ;;
        esac
    fi

    # retry loop to wait for fcitx5 dbus interface to become ready
    local retry=0
    while [ "$retry" -lt 15 ]; do
        if command -v gdbus >/dev/null 2>&1; then
            if gdbus call --session --dest org.fcitx.Fcitx5 --object-path /controller \
                --method org.fcitx.Fcitx.Controller1.SetCurrentIM "clak" >/dev/null 2>&1; then
                break
            fi
        elif command -v qdbus6 >/dev/null 2>&1; then
            if qdbus6 org.fcitx.Fcitx5 /controller org.fcitx.Fcitx.Controller1.SetCurrentIM "clak" >/dev/null 2>&1; then
                break
            fi
        elif command -v qdbus >/dev/null 2>&1; then
            if qdbus org.fcitx.Fcitx5 /controller org.fcitx.Fcitx.Controller1.SetCurrentIM "clak" >/dev/null 2>&1; then
                break
            fi
        elif command -v busctl >/dev/null 2>&1; then
            if busctl --user call org.fcitx.Fcitx5 /controller org.fcitx.Fcitx.Controller1 SetCurrentIM s "clak" >/dev/null 2>&1; then
                break
            fi
        fi
        sleep 0.2
        retry=$((retry + 1))
    done
}

# simulation flow
run_simulation() {
    # 1. detect os and arch
    local arch
    arch=$(uname -m)
    log_step "$lbl_check" "Hệ thống hợp lệ: ${c_purple}linux-${arch}${c_reset}"

    # 2. check fcitx5 and ibus
    ensure_fcitx5 1
    disable_ibus_conflict 1

    # 3. simulated version tag
    local sim_tag="${release_tag:-v0.1.0}"
    log_step "$lbl_fetch" "Phiên bản cài đặt: ${c_cyan}${sim_tag}${c_reset}"

    # 4. simulated download
    local bundle_name="clak-${sim_tag#v}-linux-${arch}.tar.gz"
    spin_download "Đang tải ${bundle_name} (~1.8 MB)" 1843
    log_step "$lbl_fetch" "Tải về thành công · Mã băm ${c_green}SHA256${c_reset} chính xác"

    # 5. simulated install destination
    local dest_dir="${HOME}/.local/lib/fcitx5"
    if [ "$target_mode" = "system" ]; then
        dest_dir="/usr/lib/fcitx5"
        print_system_sudo_notice
        echo -e "  ${c_dim}[Giả lập] Sẽ hỏi mật khẩu sudo tại đây để sao chép file${c_reset}"
    fi
    log_step "$lbl_install" "Đã cài ${c_accent}${dest_dir}/libclak.so${c_reset}"
    log_step "$lbl_install" "Đã cài ${c_accent}${dest_dir%/lib*}/share/fcitx5/addon/clak.conf${c_reset}"
    log_step "$lbl_install" "Đã cài ${c_accent}${dest_dir%/lib*}/share/fcitx5/inputmethod/clak.conf${c_reset}"

    # 6. check uinput permissions
    if [ "$demo_uinput" -eq 1 ] || [ ! -w /dev/uinput ]; then
        log_step "$lbl_warn" "Người dùng hiện tại chưa có quyền ghi vào /dev/uinput"
        print_uinput_sudo_notice
        echo -e "  ${c_dim}[Giả lập] Bạn chỉ cần nhập mật khẩu sudo tại đây khi chạy thật${c_reset}"
        spin_step "Đang áp dụng rule udev cho /dev/uinput..." 0.5
        log_step "$lbl_uinput" "Đã cấu hình quyền uinput thành công"
    else
        log_step "$lbl_uinput" "Quyền truy cập /dev/uinput đã sẵn sàng (${c_green}sử dụng được ngay${c_reset})"
    fi

    # 7. check wps compatibility
    configure_wps_compatibility 1

    # 8. check and configure de environment
    configure_desktop_environment 1

    # 9. check and configure jetbrains compatibility
    configure_jetbrains_compatibility 1

    # 10. check and configure autostart
    configure_autostart 1

    # 11. reload fcitx5 daemon with final spinner
    spin_step "Đang nạp lại daemon Fcitx5..." 0.6

    # final line directly after spin completes
    printf "\r\033[K\n"
    echo -e "${c_green}✔ Cài đặt Clak thành công vào hệ thống Fcitx5!${c_reset}"
    echo -e "  ${c_cyan}::${c_reset} Vào cấu hình Fcitx5 (${c_cyan}fcitx5-configtool${c_reset}) để thêm Clak vào danh sách bộ gõ"
    echo -e "  ${c_yellow}:: Lưu ý:${c_reset} ${c_bold}Hãy tắt hết các bàn phím khác và chỉ thêm mỗi Clak. Nó đã hỗ trợ sẵn gõ cả tiếng Anh và tiếng Việt${c_reset}"
    echo ""
}

# actual installation flow
run_install() {
    # 1. detect architecture
    local arch
    arch=$(get_arch)
    log_step "$lbl_check" "Kiến trúc phần cứng: ${c_purple}${arch}${c_reset}"

    # 2. check fcitx5 and ibus
    ensure_fcitx5 0
    disable_ibus_conflict 0

    # 3. check curl or wget
    local has_curl=0
    local has_wget=0
    if command -v curl >/dev/null 2>&1; then
        has_curl=1
    elif command -v wget >/dev/null 2>&1; then
        has_wget=1
    else
        err "Cần có curl hoặc wget để tải bộ cài Clak"
    fi

    # 3. get download url from github
    local release_path="/releases/latest"
    if [ -n "$release_tag" ]; then
        release_path="/releases/tags/${release_tag}"
    fi

    local tmp_response
    tmp_response=$(mktemp)

    (
        if [ "$has_curl" -eq 1 ]; then
            curl -sL -w "\n%{http_code}" \
                ${GITHUB_TOKEN:+-H "Authorization: Bearer ${GITHUB_TOKEN}"} \
                "${api_url}/repos/${repo}${release_path}" > "$tmp_response"
        else
            wget -q -O "$tmp_response" \
                ${GITHUB_TOKEN:+--header "Authorization: Bearer ${GITHUB_TOKEN}"} \
                "${api_url}/repos/${repo}${release_path}"
            echo "200" >> "$tmp_response"
        fi
    ) &
    spin_pid $! "Đang truy vấn phiên bản phát hành từ GitHub..." 1

    local http_code
    http_code=$(tail -n1 "$tmp_response" 2>/dev/null || echo "200")
    local releases_json
    releases_json=$(sed '$d' "$tmp_response" 2>/dev/null || cat "$tmp_response")
    rm -f "$tmp_response"

    if [ "$http_code" = "404" ]; then
        err "Không tìm thấy bản phát hành nào trên GitHub. Có thể dự án chưa công bố release."
    fi

    local tag_name
    tag_name=$(echo "$releases_json" | grep '"tag_name":' | head -1 | cut -d '"' -f 4 || echo "$release_tag")
    local download_url
    download_url=$(echo "$releases_json" | grep "browser_download_url" | grep "${arch}\.tar\.gz" | head -1 | cut -d '"' -f 4 || true)

    if [ -z "$download_url" ]; then
        err "Không tìm thấy file nén '${arch}.tar.gz' trong bản phát hành ${tag_name}"
    fi

    local archive_size_bytes=0
    archive_size_bytes=$(echo "$releases_json" | awk -v pat="${arch}.tar.gz" 'index($0, pat) {flag=1} flag && /"size":/ {gsub(/[^0-9]/, "", $0); print; exit}' || echo 0)
    local archive_kb=0
    if [ -n "$archive_size_bytes" ] && [ "$archive_size_bytes" -gt 0 ] 2>/dev/null; then
        archive_kb=$((archive_size_bytes / 1024))
    fi

    log_step "$lbl_fetch" "Phiên bản chỉ định: ${c_cyan}${tag_name}${c_reset}"

    # 4. download release archive
    tmp_dir=$(mktemp -d)

    local archive_name
    archive_name=$(basename "$download_url")

    (
        cd "$tmp_dir"
        if [ "$has_curl" -eq 1 ]; then
            curl -sLO "$download_url"
        else
            wget -q "$download_url"
        fi
    ) &
    local dl_pid=$!
    spin_download "Đang tải ${archive_name}" "$archive_kb" "$dl_pid" "${tmp_dir}/${archive_name}"
    wait "$dl_pid" 2>/dev/null || true

    log_step "$lbl_fetch" "Tải về thành công"

    # verify checksum if checksums.txt exists in release
    local checksums_url
    checksums_url=$(echo "$releases_json" | grep "browser_download_url" | grep "checksums\.txt" | head -1 | cut -d '"' -f 4 || true)
    if [ -n "$checksums_url" ]; then
        (
            cd "$tmp_dir"
            if [ "$has_curl" -eq 1 ]; then
                curl -sLO "$checksums_url"
            else
                wget -q "$checksums_url"
            fi
        )
        if [ -f "${tmp_dir}/checksums.txt" ] && command -v sha256sum >/dev/null 2>&1; then
            local expected_hash
            expected_hash=$(grep "${archive_name}" "${tmp_dir}/checksums.txt" | awk '{print $1}')
            if [ -n "$expected_hash" ]; then
                local actual_hash
                actual_hash=$(sha256sum "${tmp_dir}/${archive_name}" | awk '{print $1}')
                if [ "$expected_hash" != "$actual_hash" ]; then
                    err "Xác thực mã băm SHA256 thất bại! File tải về có thể đã bị can thiệp."
                fi
                log_step "$lbl_check" "Xác thực SHA256 thành công (${actual_hash:0:16}...)"
            fi
        fi
    fi

    # 5. extract archive
    tar -xzf "${tmp_dir}/${archive_name}" -C "$tmp_dir"

    local lib_src=""
    if [ -f "${tmp_dir}/usr/lib/fcitx5/libclak.so" ]; then
        lib_src="${tmp_dir}/usr/lib/fcitx5/libclak.so"
    elif compgen -G "${tmp_dir}/usr/lib/*/fcitx5/libclak.so" >/dev/null 2>&1; then
        lib_src=$(compgen -G "${tmp_dir}/usr/lib/*/fcitx5/libclak.so" | head -1)
    else
        lib_src=$(find "${tmp_dir}" -name "libclak.so" 2>/dev/null | head -1 || true)
    fi

    local addon_src="${tmp_dir}/usr/share/fcitx5/addon/clak.conf"
    local im_src="${tmp_dir}/usr/share/fcitx5/inputmethod/clak.conf"
    local gui_src="${tmp_dir}/usr/bin/clak-gui"
    local cli_src="${tmp_dir}/usr/bin/clak"
    local desktop_src="${tmp_dir}/usr/share/applications/clak-gui.desktop"
    local icons_src="${tmp_dir}/usr/share/icons"

    if [ -z "$lib_src" ] || [ ! -f "$lib_src" ]; then
        err "Gói cài đặt bị lỗi: không tìm thấy file libclak.so"
    fi

    # check glibc compatibility of downloaded binary
    if command -v ldd >/dev/null 2>&1; then
        local ldd_check
        ldd_check=$(ldd "$lib_src" 2>&1 || true)
        if echo "$ldd_check" | grep -q "version \`GLIBC_.*' not found"; then
            local missing_glibc
            missing_glibc=$(echo "$ldd_check" | grep "version \`GLIBC_" | head -1 | sed -e 's/.*version `//' -e 's/'\'' not found.*//')
            log_step "$lbl_warn" "Cảnh báo tương thích: gói nhị phân yêu cầu GLIBC $missing_glibc (hệ thống hiện tại chưa có)"
        fi
    fi

    # 6. copy files (only request sudo if system install)
    local lib_dest
    local addon_dest
    local im_dest

    if [ "$target_mode" = "system" ]; then
        lib_dest="/usr/lib/fcitx5"
        addon_dest="/usr/share/fcitx5/addon"
        im_dest="/usr/share/fcitx5/inputmethod"
        print_system_sudo_notice

        run_sudo mkdir -p "$lib_dest" "$addon_dest" "$im_dest"
        run_sudo cp "$lib_src" "${lib_dest}/libclak.so"
        run_sudo cp "$addon_src" "${addon_dest}/clak.conf"
        run_sudo cp "$im_src" "${im_dest}/clak.conf"
        run_sudo chmod 755 "${lib_dest}/libclak.so"
        run_sudo chmod 644 "${addon_dest}/clak.conf" "${im_dest}/clak.conf"

        # symlink into debian/ubuntu multiarch path for fcitx5
        run_sudo mkdir -p "/usr/lib/x86_64-linux-gnu/fcitx5"
        run_sudo ln -sf "${lib_dest}/libclak.so" "/usr/lib/x86_64-linux-gnu/fcitx5/libclak.so" 2>/dev/null || true

        if [ -f "$cli_src" ]; then
            run_sudo mkdir -p "/usr/bin"
            run_sudo cp "$cli_src" "/usr/bin/clak"
            run_sudo chmod 755 "/usr/bin/clak"
        fi
        if [ -f "$gui_src" ]; then
            run_sudo mkdir -p "/usr/bin"
            run_sudo cp "$gui_src" "/usr/bin/clak-gui"
            run_sudo chmod 755 "/usr/bin/clak-gui"
        fi
        if [ -f "$desktop_src" ]; then
            run_sudo mkdir -p "/usr/share/applications"
            run_sudo cp "$desktop_src" "/usr/share/applications/clak-gui.desktop"
            run_sudo sed -i "s|^Exec=.*|Exec=/usr/bin/clak-gui|" "/usr/share/applications/clak-gui.desktop" 2>/dev/null || true
            run_sudo chmod 644 "/usr/share/applications/clak-gui.desktop"
            command -v update-desktop-database >/dev/null 2>&1 && run_sudo update-desktop-database "/usr/share/applications" 2>/dev/null || true
        fi
        if [ -d "$icons_src" ]; then
            run_sudo cp -r "$icons_src"/* /usr/share/icons/ 2>/dev/null || true
            if command -v gtk-update-icon-cache >/dev/null 2>&1; then
                run_sudo gtk-update-icon-cache -f -q -t "/usr/share/icons/hicolor" 2>/dev/null || true
            fi
        fi
    else
        lib_dest="${HOME}/.local/lib/fcitx5"
        addon_dest="${HOME}/.local/share/fcitx5/addon"
        im_dest="${HOME}/.local/share/fcitx5/inputmethod"

        mkdir -p "$lib_dest" "$addon_dest" "$im_dest"
        cp "$lib_src" "${lib_dest}/libclak.so"
        cp "$addon_src" "${addon_dest}/clak.conf"
        cp "$im_src" "${im_dest}/clak.conf"
        chmod 755 "${lib_dest}/libclak.so"
        chmod 644 "${addon_dest}/clak.conf" "${im_dest}/clak.conf"

        # for user-level install, addon configuration must point to the absolute library path
        # because fcitx5 only resolves relative Library paths against system PKGLIBDIR
        sed -i "s|^Library=.*|Library=${lib_dest}/libclak|" "${addon_dest}/clak.conf"

        # symlink into debian/ubuntu multiarch path for fcitx5
        mkdir -p "${HOME}/.local/lib/x86_64-linux-gnu/fcitx5"
        ln -sf "${lib_dest}/libclak.so" "${HOME}/.local/lib/x86_64-linux-gnu/fcitx5/libclak.so" 2>/dev/null || true

        if [ -f "$cli_src" ]; then
            mkdir -p "${HOME}/.local/bin"
            cp "$cli_src" "${HOME}/.local/bin/clak"
            chmod 755 "${HOME}/.local/bin/clak"
        fi
        if [ -f "$gui_src" ]; then
            mkdir -p "${HOME}/.local/bin"
            cp "$gui_src" "${HOME}/.local/bin/clak-gui"
            chmod 755 "${HOME}/.local/bin/clak-gui"
        fi
        if [ -f "$desktop_src" ]; then
            mkdir -p "${HOME}/.local/share/applications"
            cp "$desktop_src" "${HOME}/.local/share/applications/clak-gui.desktop"
            # use absolute path for Exec so desktop environment finds binary without ~/.local/bin in PATH
            sed -i "s|^Exec=.*|Exec=${HOME}/.local/bin/clak-gui|" "${HOME}/.local/share/applications/clak-gui.desktop" 2>/dev/null || true
            chmod 644 "${HOME}/.local/share/applications/clak-gui.desktop"
            command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "${HOME}/.local/share/applications" 2>/dev/null || true
        fi
        if [ -d "$icons_src" ]; then
            mkdir -p "${HOME}/.local/share/icons"
            cp -r "$icons_src"/* "${HOME}/.local/share/icons/" 2>/dev/null || true
            if command -v gtk-update-icon-cache >/dev/null 2>&1; then
                for icondir in "${HOME}/.local/share/icons"/*; do
                    [ -d "$icondir" ] && gtk-update-icon-cache -f -q -t "$icondir" 2>/dev/null || true
                done
            fi
        fi

        # propagate ~/.local/bin to current path and systemd user environment
        if [[ ":$PATH:" != *":${HOME}/.local/bin:"* ]]; then
            export PATH="${HOME}/.local/bin:${PATH}"
        fi
        command -v systemctl >/dev/null 2>&1 && systemctl --user set-environment PATH="${HOME}/.local/bin:${PATH}" 2>/dev/null || true
    fi

    log_step "$lbl_install" "Đã chép ${c_accent}${lib_dest}/libclak.so${c_reset}"
    log_step "$lbl_install" "Đã chép ${c_accent}${addon_dest}/clak.conf${c_reset}"
    log_step "$lbl_install" "Đã chép ${c_accent}${im_dest}/clak.conf${c_reset}"
    [ -f "$cli_src" ] && log_step "$lbl_install" "Đã cài đặt công cụ dòng lệnh clak"
    [ -f "$gui_src" ] && log_step "$lbl_install" "Đã cài đặt giao diện điều khiển cấu hình clak-gui"

    # save installed version for updater
    mkdir -p "${HOME}/.local/share/clak"
    echo "$tag_name" > "${HOME}/.local/share/clak/version"

    # clean up temporary archive directory
    if [ -n "${tmp_dir:-}" ] && [ -d "$tmp_dir" ]; then
        rm -rf "$tmp_dir"
        tmp_dir=""
    fi

    # 7. check and configure uinput permission automatically
    if [ "$skip_uinput" -eq 1 ]; then
        log_step "$lbl_uinput" "Đã bỏ qua cấu hình uinput (--skip-uinput)"
    elif [ -w /dev/uinput ] 2>/dev/null; then
        log_step "$lbl_uinput" "Quyền truy cập /dev/uinput đã sẵn sàng (${c_green}OK${c_reset})"
    else
        log_step "$lbl_warn" "Người dùng hiện tại chưa có quyền ghi vào /dev/uinput"
        print_uinput_sudo_notice
        local uinput_ok=0
        if command -v sudo >/dev/null 2>&1; then
            if run_sudo sh -c 'mkdir -p /etc/udev/rules.d /etc/modules-load.d && echo "KERNEL==\"uinput\", SUBSYSTEM==\"misc\", OPTIONS+=\"static_node=uinput\", TAG+=\"uaccess\"" > /etc/udev/rules.d/99-uinput.rules && echo "uinput" > /etc/modules-load.d/uinput.conf'; then
                run_sudo udevadm control --reload-rules 2>/dev/null || true
                run_sudo udevadm trigger --name-match=uinput 2>/dev/null || run_sudo udevadm trigger /dev/uinput 2>/dev/null || true
                run_sudo modprobe uinput 2>/dev/null || true
                uinput_ok=1
            fi
        fi

        if [ "$uinput_ok" -eq 1 ]; then
            log_step "$lbl_uinput" "Đã cấu hình /dev/uinput thành công"
        else
            log_step "$lbl_warn" "Chưa cấp quyền uinput, Clak sẽ hoạt động ở chế độ Wayland mặc định"
        fi
    fi

    # 8. check and configure wps compatibility
    configure_wps_compatibility 0

    # 9. configure de environment and jetbrains
    configure_desktop_environment 0
    configure_jetbrains_compatibility 0

    # 10. configure autostart with system
    configure_autostart 0
    configure_fcitx5_behavior 0
    apply_live_environment 0

    # 11. reload fcitx5 daemon with final spinner
    if command -v fcitx5 >/dev/null 2>&1; then
        (
            fcitx5 -r -d >/dev/null 2>&1 || true
            sleep 0.4
        ) &
        local reload_pid=$!
        spin_pid "$reload_pid" "Đang khởi động lại daemon Fcitx5..." 1
        wait "$reload_pid" 2>/dev/null || true
        activate_clak_and_reconnect_compositor 0
    fi

    # final line directly after spin completes
    printf "\r\033[K\n"
    echo -e "${c_green}✔ Cài đặt Clak thành công vào hệ thống Fcitx5!${c_reset}"
    echo -e "  ${c_cyan}::${c_reset} Vào cấu hình Fcitx5 (${c_cyan}fcitx5-configtool${c_reset}) để thêm Clak vào danh sách bộ gõ để sử dụng"
    echo -e "  ${c_yellow}:: Lưu ý:${c_reset} ${c_bold}Hãy tắt hết các bàn phím khác và chỉ thêm mỗi Clak. Nó hỗ trợ gõ cả tiếng Anh và tiếng Việt${c_reset}"
    echo ""
    if command -v notify-send >/dev/null 2>&1; then
        notify-send -i org.fcitx.Fcitx5.clak "Clak" "Cài đặt thành công! Hãy tắt hết các bàn phím khác và chỉ thêm mỗi Clak. Nó hỗ trợ gõ cả tiếng Anh và tiếng Việt" 2>/dev/null || true
    fi
}

# main router
if [ "$dry_run" -eq 1 ]; then
    run_simulation
else
    run_install
fi
