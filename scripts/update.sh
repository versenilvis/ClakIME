#!/usr/bin/env bash
set -euo pipefail

# configuration and defaults
repo="versenilvis/clak"
api_url="${CLAK_API_URL:-https://api.github.com}"
version_file="${HOME}/.local/share/clak/version"
dry_run=0
target_version=""
force_update=0
assume_yes=0
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
            ;;
        --force|-f)
            force_update=1
            ;;
        --yes|-y)
            assume_yes=1
            ;;
        --version=*|--tag=*)
            target_version="${arg#*=}"
            ;;
        --help|-h)
            echo "Cách dùng: curl -fsSL https://raw.githubusercontent.com/versenilvis/clak/main/scripts/update.sh | bash [options]"
            echo "Hoặc: bash scripts/update.sh [options]"
            echo ""
            echo "Tùy chọn:"
            echo "  -d, --dry-run, --simulate  Chạy giả lập kiểm tra quy trình cập nhật"
            echo "  -f, --force                Cập nhật đè phiên bản mới nhất ngay cả khi đang ở bản mới"
            echo "  -y, --yes                  Tự động đồng ý cập nhật qua trình quản lý gói nếu có"
            echo "      --version=<phiên bản>  Chỉ định phiên bản cụ thể cần update (ví dụ: v0.1.1)"
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
lbl_update="${c_green}CẬP NHẬT${c_reset}"
lbl_fcitx="${c_cyan}FCITX5  ${c_reset}"
lbl_aur="${c_purple}AUR     ${c_reset}"
lbl_nix="${c_cyan}NIX     ${c_reset}"
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
                printf "\r\033[38;5;75m%s\033[0m \033[38;5;244m%s\033[0m\033[K" \
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

# reload fcitx5 daemon via systemctl or direct command
reload_fcitx5() {
    if command -v systemctl >/dev/null 2>&1 && systemctl --user is-active fcitx5.service >/dev/null 2>&1; then
        systemctl --user restart fcitx5.service >/dev/null 2>&1 || true
    elif command -v fcitx5 >/dev/null 2>&1; then
        fcitx5 -r -d >/dev/null 2>&1 || true
    fi
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

# get current installed version
get_installed_version() {
    if [ -f "$version_file" ]; then
        cat "$version_file" | tr -d ' \n'
        return
    fi

    # check if installed via pacman or aur
    if command -v pacman >/dev/null 2>&1; then
        local p_ver
        p_ver=$(pacman -Q clak 2>/dev/null | awk '{print $2}' || true)
        if [ -n "$p_ver" ]; then
            echo "v${p_ver%%-*}"
            return
        fi
        p_ver=$(pacman -Q clak-git 2>/dev/null | awk '{print $2}' || true)
        if [ -n "$p_ver" ]; then
            echo "v${p_ver%%-*}"
            return
        fi
        p_ver=$(pacman -Q fcitx5-clak 2>/dev/null | awk '{print $2}' || true)
        if [ -n "$p_ver" ]; then
            echo "v${p_ver%%-*}"
            return
        fi
    fi

    echo "v0.1.0"
}

# detect installation method across services
detect_install_source() {
    # 1. arch linux pacman or aur
    if command -v pacman >/dev/null 2>&1; then
        if pacman -Qi clak >/dev/null 2>&1 || pacman -Qi clak-git >/dev/null 2>&1 || pacman -Qi fcitx5-clak >/dev/null 2>&1; then
            echo "aur"
            return
        fi
    fi

    # 2. nix package manager or flake
    if command -v nix >/dev/null 2>&1; then
        if nix profile list 2>/dev/null | grep -q "clak" || [ -L "/run/current-system/sw/lib/fcitx5/libclak.so" ]; then
            echo "nix"
            return
        fi
    fi

    # 3. git repository source build
    if [ -f "./CMakeLists.txt" ] && [ -d "./.git" ] && grep -q "project(clak" ./CMakeLists.txt 2>/dev/null; then
        echo "git"
        return
    fi

    # 4. user local directory
    if [ -f "${HOME}/.local/lib/fcitx5/libclak.so" ] || [ -f "${HOME}/.local/lib/x86_64-linux-gnu/fcitx5/libclak.so" ]; then
        echo "user"
        return
    fi

    # 5. system wide directory
    if [ -f "/usr/lib/fcitx5/libclak.so" ] || [ -f "/usr/local/lib/fcitx5/libclak.so" ] || [ -f "/usr/lib/x86_64-linux-gnu/fcitx5/libclak.so" ]; then
        echo "system"
        return
    fi

    echo "unknown"
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
        echo -e "  ${c_yellow}• Bạn vẫn có thể sử dụng và cập nhật hệ thống (pacman/yay) bình thường${c_reset}"

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

# simulation flow
run_simulation() {
    local cur_ver
    cur_ver=$(get_installed_version)
    local new_ver="${target_version:-v0.1.1}"

    # 1. detect installation source instantly
    local source_type
    source_type=$(detect_install_source)
    case "$source_type" in
        aur)
            log_step "$lbl_aur" "Phát hiện Clak được quản lý bởi ${c_purple}Arch Linux (AUR / Pacman)${c_reset}"
            ;;
        nix)
            log_step "$lbl_nix" "Phát hiện Clak được quản lý bởi ${c_cyan}Nix Flake${c_reset}"
            ;;
        git)
            log_step "$lbl_check" "Phát hiện mã nguồn Clak từ ${c_accent}Git repository${c_reset}"
            ;;
        system)
            log_step "$lbl_check" "Phát hiện Clak cài đặt toàn hệ thống: ${c_accent}/usr/lib/fcitx5${c_reset}"
            ;;
        *)
            log_step "$lbl_check" "Phát hiện Clak cài đặt người dùng: ${c_accent}~/.local/lib/fcitx5${c_reset}"
            ;;
    esac

    log_step "$lbl_check" "Phiên bản hiện tại: ${c_cyan}${cur_ver}${c_reset}"

    # 2. check update result
    log_step "$lbl_fetch" "Đã tìm thấy bản phát hành mới: ${c_green}${new_ver}${c_reset}"

    # 3. download package with spinner
    local arch
    arch=$(uname -m)
    local bundle_name="clak-${new_ver#v}-linux-${arch}.tar.gz"
    spin_download "Đang tải ${bundle_name} (~1.8 MB)" 1843
    log_step "$lbl_fetch" "Tải về thành công · Mã băm ${c_green}SHA256${c_reset} chính xác"

    # 4. update files instantly
    log_step "$lbl_update" "Đã cập nhật ${c_accent}~/.local/lib/fcitx5/libclak.so${c_reset}"
    log_step "$lbl_update" "Đã cập nhật ${c_accent}~/.local/share/fcitx5/addon/clak.conf${c_reset}"

    # 5. configure wps office compatibility
    configure_wps_compatibility 1

    # 6. reload fcitx5 daemon with final spinner
    spin_step "Đang nạp lại daemon Fcitx5..." 0.6

    # final line directly after spin completes
    printf "\r\033[K\n"
    echo -e "${c_green}Đã update Clak lên phiên bản ${new_ver}${c_reset}"
    echo ""
}

# actual update flow
run_update() {
    local cur_ver
    cur_ver=$(get_installed_version)

    # 1. detect installation method instantly
    local source_type
    source_type=$(detect_install_source)

    log_step "$lbl_check" "Phiên bản hiện tại: ${c_cyan}${cur_ver}${c_reset}"

    # if installed via aur or pacman, handle through aur helper
    if [ "$source_type" = "aur" ]; then
        log_step "$lbl_aur" "Clak được quản lý bởi ${c_purple}Arch Linux (AUR / Pacman)${c_reset}"
        local helper=""
        if command -v yay >/dev/null 2>&1; then
            helper="yay"
        elif command -v paru >/dev/null 2>&1; then
            helper="paru"
        fi

        if [ -n "$helper" ]; then
            echo ""
            echo -e "  ${c_purple}╭─ CẬP NHẬT QUA AUR ─────────────────────────────────────────╮${c_reset}"
            echo -e "  ${c_purple}│${c_reset}  Khuyên dùng công cụ AUR để cập nhật đồng bộ package:      ${c_purple}│${c_reset}"
            if [ "$helper" = "yay" ]; then
                echo -e "  ${c_purple}│${c_reset}    ${c_bold}yay -S clak${c_reset}                                             ${c_purple}│${c_reset}"
            else
                echo -e "  ${c_purple}│${c_reset}    ${c_bold}paru -S clak${c_reset}                                            ${c_purple}│${c_reset}"
            fi
            echo -e "  ${c_purple}╰────────────────────────────────────────────────────────────╯${c_reset}"
            echo ""

            local aur_reply="y"
            if [ "$assume_yes" -eq 0 ]; then
                read -r -p "Bạn có muốn chạy '${helper} -S clak' để cập nhật ngay bây giờ? [Y/n] " aur_reply < /dev/tty || aur_reply="y"
            fi

            case "$aur_reply" in
                [yY][eE][sS]|[yY]|"")
                    $helper -S clak < /dev/tty
                    spin_step "Đang nạp lại daemon Fcitx5..." 0.6
                    reload_fcitx5
                    local updated_ver
                    updated_ver=$(get_installed_version)
                    printf "\r\033[K\n"
                    echo -e "${c_green}Đã update Clak lên phiên bản ${updated_ver}${c_reset}"
                    echo ""
                    exit 0
                    ;;
                *)
                    log_step "$lbl_warn" "Bỏ qua cập nhật AUR, chuyển sang tải bản phát hành trực tiếp"
                    ;;
            esac
        fi
    fi

    # if installed via nix flake
    if [ "$source_type" = "nix" ]; then
        log_step "$lbl_nix" "Clak được quản lý bởi ${c_cyan}Nix Flake${c_reset}"
        echo ""
        echo -e "  ${c_cyan}╭─ CẬP NHẬT QUA NIX ─────────────────────────────────────────╮${c_reset}"
        echo -e "  ${c_cyan}│${c_reset}  Khuyên dùng Nix để cập nhật flake hoặc profile:           ${c_cyan}│${c_reset}"
        echo -e "  ${c_cyan}│${c_reset}    ${c_bold}nix profile upgrade clak${c_reset}                                ${c_cyan}│${c_reset}"
        echo -e "  ${c_cyan}╰────────────────────────────────────────────────────────────╯${c_reset}"
        echo ""

        local nix_reply="y"
        if [ "$assume_yes" -eq 0 ]; then
            read -r -p "Bạn có muốn chạy 'nix profile upgrade clak'? [Y/n] " nix_reply < /dev/tty || nix_reply="y"
        fi

        case "$nix_reply" in
            [yY][eE][sS]|[yY]|"")
                nix profile upgrade clak < /dev/tty
                spin_step "Đang nạp lại daemon Fcitx5..." 0.6
                reload_fcitx5
                local updated_ver
                updated_ver=$(get_installed_version)
                printf "\r\033[K\n"
                echo -e "${c_green}Đã update Clak lên phiên bản ${updated_ver}${c_reset}"
                echo ""
                exit 0
                ;;
            *)
                log_step "$lbl_warn" "Bỏ qua cập nhật Nix, chuyển sang tải bản phát hành trực tiếp"
                ;;
        esac
    fi

    # if running from local git source tree
    if [ "$source_type" = "git" ]; then
        log_step "$lbl_check" "Phát hiện mã nguồn trong Git repository"
        if command -v just >/dev/null 2>&1; then
            local git_reply="y"
            if [ "$assume_yes" -eq 0 ]; then
                read -r -p "Bạn có muốn git pull và build lại bằng just? [Y/n] " git_reply < /dev/tty || git_reply="y"
            fi
            case "$git_reply" in
                [yY][eE][sS]|[yY]|"")
                    git pull
                    just build-release
                    just install
                    spin_step "Đang nạp lại daemon Fcitx5..." 0.6
                    reload_fcitx5
                    local updated_ver
                    updated_ver=$(get_installed_version)
                    printf "\r\033[K\n"
                    echo -e "${c_green}Đã update Clak lên phiên bản ${updated_ver}${c_reset}"
                    echo ""
                    exit 0
                    ;;
                *)
                    log_step "$lbl_warn" "Bỏ qua build từ source, tiếp tục tải bản phát hành"
                    ;;
            esac
        fi
    fi

    # 2. query latest release from github api
    local release_path="/releases/latest"
    if [ -n "$target_version" ]; then
        release_path="/releases/tags/${target_version}"
    fi

    local tmp_response
    tmp_response=$(mktemp)

    (
        curl -sL -w "\n%{http_code}" \
            ${GITHUB_TOKEN:+-H "Authorization: Bearer ${GITHUB_TOKEN}"} \
            "${api_url}/repos/${repo}${release_path}" > "$tmp_response"
    ) &
    spin_pid $! "Đang kiểm tra phiên bản mới nhất từ GitHub..." 1

    local http_code
    http_code=$(tail -n1 "$tmp_response" 2>/dev/null || echo "200")
    local releases_json
    releases_json=$(sed '$d' "$tmp_response" 2>/dev/null || cat "$tmp_response")
    rm -f "$tmp_response"

    if [ "$http_code" = "404" ]; then
        err "Không tìm thấy bản phát hành nào trên GitHub"
    fi

    local latest_tag
    latest_tag=$(echo "$releases_json" | grep '"tag_name":' | head -1 | cut -d '"' -f 4 || echo "")

    if [ -z "$latest_tag" ]; then
        err "Không đọc được thông tin phiên bản phát hành từ GitHub"
    fi

    if [ "$latest_tag" = "$cur_ver" ] && [ "$force_update" -eq 0 ]; then
        log_step "$lbl_check" "Clak đã ở phiên bản mới nhất (${c_green}${cur_ver}${c_reset})"
        echo ""
        echo "Không cần cập nhật. (Dùng cờ -f hoặc --force để cập nhật đè)"
        exit 0
    fi

    log_step "$lbl_fetch" "Cập nhật từ ${c_cyan}${cur_ver}${c_reset} -> ${c_green}${latest_tag}${c_reset}"

    # 3. download release archive
    local arch
    arch=$(uname -m)
    local download_url=""
    local archive_pat="linux-${arch}\.tar\.xz"
    download_url=$(echo "$releases_json" | grep "browser_download_url" | grep "linux-${arch}\.tar\.xz" | head -1 | cut -d '"' -f 4 || true)
    if [ -z "$download_url" ]; then
        download_url=$(echo "$releases_json" | grep "browser_download_url" | grep "linux-${arch}\.tar\.gz" | head -1 | cut -d '"' -f 4 || true)
        archive_pat="linux-${arch}\.tar\.gz"
    fi

    if [ -z "$download_url" ]; then
        err "Không tìm thấy file nén cho kiến trúc linux-${arch}"
    fi

    local archive_size_bytes=0
    archive_size_bytes=$(echo "$releases_json" | awk -v pat="${archive_pat}" 'index($0, pat) {flag=1} flag && /"size":/ {gsub(/[^0-9]/, "", $0); print; exit}' || echo 0)
    local archive_kb=0
    if [ -n "$archive_size_bytes" ] && [ "$archive_size_bytes" -gt 0 ] 2>/dev/null; then
        archive_kb=$((archive_size_bytes / 1024))
    fi

    tmp_dir=$(mktemp -d)

    local archive_name
    archive_name=$(basename "$download_url")

    (
        cd "$tmp_dir"
        curl -sLO "$download_url"
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
            curl -sLO "$checksums_url"
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

    # 4. extract archive
    tar -xf "${tmp_dir}/${archive_name}" -C "$tmp_dir"

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
        err "Gói cập nhật không hợp lệ: thiếu libclak.so"
    fi

    # 5. install files
    local lib_dest="${HOME}/.local/lib/fcitx5"
    local addon_dest="${HOME}/.local/share/fcitx5/addon"
    local im_dest="${HOME}/.local/share/fcitx5/inputmethod"

    if [ "$source_type" = "system" ]; then
        lib_dest="/usr/lib/fcitx5"
        addon_dest="/usr/share/fcitx5/addon"
        im_dest="/usr/share/fcitx5/inputmethod"

        echo "Yêu cầu quyền sudo để ghi đè file hệ thống (/usr)..."
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
    fi

    mkdir -p "${HOME}/.local/share/clak"
    echo "$latest_tag" > "$version_file"

    # clean up temporary archive directory
    if [ -n "${tmp_dir:-}" ] && [ -d "$tmp_dir" ]; then
        rm -rf "$tmp_dir"
        tmp_dir=""
    fi

    log_step "$lbl_update" "Đã cập nhật ${c_accent}${lib_dest}/libclak.so${c_reset}"

    # 6. check and configure wps office compatibility
    configure_wps_compatibility 0

    # 7. reload fcitx5 daemon with final spinner
    spin_step "Đang nạp lại daemon Fcitx5..." 0.6
    reload_fcitx5

    # final line directly after spin completes
    printf "\r\033[K\n"
    echo -e "${c_green}Đã update Clak lên phiên bản ${latest_tag}${c_reset}"
    echo ""
}

# main router
if [ "$dry_run" -eq 1 ]; then
    run_simulation
else
    run_update
fi
