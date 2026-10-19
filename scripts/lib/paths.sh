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
