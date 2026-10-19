#!/usr/bin/env bash
# common path definitions for clak scripts

get_system_addon_dirs() {
    echo "/usr/lib/fcitx5"
    echo "/usr/lib/x86_64-linux-gnu/fcitx5"
    echo "/usr/lib64/fcitx5"
    echo "/usr/local/lib/fcitx5"
}

get_user_addon_dirs() {
    echo "${HOME}/.local/lib/fcitx5"
    echo "${HOME}/.local/lib/x86_64-linux-gnu/fcitx5"
}

# detect if target file is owned by package manager
check_package_owner() {
    local target="$1"
    [ -e "$target" ] || return 1

    if command -v pacman >/dev/null 2>&1; then
        local pkg
        pkg=$(pacman -Qo "$target" 2>/dev/null | awk '{print $5}' || true)
        if [ -n "$pkg" ]; then
            echo "pacman:${pkg}"
            return 0
        fi
    fi

    if command -v dpkg >/dev/null 2>&1; then
        local dpkg_out
        dpkg_out=$(dpkg -S "$target" 2>/dev/null || true)
        if [ -n "$dpkg_out" ]; then
            local pkg="${dpkg_out%%:*}"
            echo "dpkg:${pkg}"
            return 0
        fi
    fi

    if command -v rpm >/dev/null 2>&1; then
        local rpm_out
        rpm_out=$(rpm -qf "$target" 2>/dev/null || true)
        if [ -n "$rpm_out" ] && ! echo "$rpm_out" | grep -q "is not owned"; then
            echo "rpm:${rpm_out}"
            return 0
        fi
    fi

    return 1
}
