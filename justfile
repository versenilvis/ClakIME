# run "just -l" to view all commands
default:
    @just --list

# automatically include user-local pkg-config path if present (e.g. non-root Debian/Ubuntu)
export PKG_CONFIG_PATH := if path_exists(home_directory() + "/.local/usr/lib/x86_64-linux-gnu/pkgconfig") == "true" {
    home_directory() + "/.local/usr/lib/x86_64-linux-gnu/pkgconfig" + (if env("PKG_CONFIG_PATH", "") != "" { ":" + env("PKG_CONFIG_PATH", "") } else { "" })
} else {
    env("PKG_CONFIG_PATH", "")
}

# build, install and restart fcitx5
[group("dev")]
dev: build install restart

# build clak fcitx5 addon
[group("dev")]
build:
    cmake --build build

# build in release mode
[group("dev")]
build-release:
    cmake -B build -DCMAKE_BUILD_TYPE=Release
    cmake --build build

# install addon for current user
[group("dev")]
install:
    cmake --build build --target install-user

# restart fcitx5 daemon
[group("dev")]
restart:
    #!/usr/bin/env bash
    set -euo pipefail
    profile_dir="${XDG_CONFIG_HOME:-$HOME/.config}/fcitx5"
    profile_file="${profile_dir}/profile"

    fcitx5_is_ready() {
        if ! command -v fcitx5-remote >/dev/null 2>&1; then
            return 1
        fi
        if fcitx5-remote --check >/dev/null 2>&1; then
            return 0
        fi
        # fallback for fcitx5 < 5.1.8 where --check is unrecognized (declared minimum is 5.0.14)
        if pgrep -u "$(id -u)" -x fcitx5 >/dev/null 2>&1; then
            fcitx5-remote >/dev/null 2>&1
        else
            return 1
        fi
    }

    # detect if systemd user unit manages fcitx5
    systemd_unit=""
    for u in fcitx5.service app-org.fcitx.Fcitx5.service app-org.fcitx.Fcitx5@autostart.service; do
        if command -v systemctl >/dev/null 2>&1 && systemctl --user is-active --quiet "${u}" 2>/dev/null; then
            systemd_unit="${u}"
            break
        fi
    done

    # stop running daemon first so exiting instance never overwrites profile after we check it
    if [ -n "${systemd_unit}" ]; then
        systemctl --user stop "${systemd_unit}" || true
    elif fcitx5_is_ready; then
        fcitx5-remote -e >/dev/null 2>&1 || true
    fi
    for _ in $(seq 1 25); do
        if ! pgrep -u "$(id -u)" -x fcitx5 >/dev/null 2>&1; then
            break
        fi
        sleep 0.1
    done

    # verify daemon exited; retry shutdown or return error before modifying profile
    if pgrep -u "$(id -u)" -x fcitx5 >/dev/null 2>&1; then
        if [ -n "${systemd_unit}" ]; then
            systemctl --user kill --signal=SIGTERM "${systemd_unit}" || true
        elif command -v pkill >/dev/null 2>&1; then
            pkill -u "$(id -u)" -x fcitx5 2>/dev/null || true
        fi
        for _ in $(seq 1 15); do
            if ! pgrep -u "$(id -u)" -x fcitx5 >/dev/null 2>&1; then
                break
            fi
            sleep 0.1
        done
        if pgrep -u "$(id -u)" -x fcitx5 >/dev/null 2>&1; then
            echo "error: failed to stop existing fcitx5 daemon before reloading profile" >&2
            exit 1
        fi
    fi

    # restore clak entry in profile if dropped by a prior reload
    mkdir -p "${profile_dir}"
    if [ ! -f "${profile_file}" ]; then
        cat <<'EOF' > "${profile_file}"
    [Groups/0]
    Name=Default
    Default Layout=us
    DefaultIM=clak

    [Groups/0/Items/0]
    Name=clak
    Layout=

    [Groups/0/Items/1]
    Name=keyboard-us
    Layout=

    [GroupOrder]
    0=Default
    EOF
    elif ! grep -q '^Name=clak$' "${profile_file}"; then
        item_count=$(grep -c '^\[Groups/0/Items/[0-9]\+\]$' "${profile_file}" 2>/dev/null || true)
        item_count="${item_count:-0}"
        tmp_profile=$(mktemp "${profile_dir}/.profile.tmp.XXXXXX")
        awk -v idx="${item_count}" '
            /^\[Groups\/[1-9]|^\[GroupOrder\]/ && !inserted {
                print "[Groups/0/Items/" idx "]"
                print "Name=clak"
                print "Layout="
                print ""
                inserted=1
            }
            { print }
            END {
                if (!inserted) {
                    print ""
                    print "[Groups/0/Items/" idx "]"
                    print "Name=clak"
                    print "Layout="
                }
            }
        ' "${profile_file}" > "${tmp_profile}" && mv -f "${tmp_profile}" "${profile_file}"
    fi

    if [ -n "${systemd_unit}" ]; then
        systemctl --user start "${systemd_unit}"
    else
        fcitx5 -r -d </dev/null >/dev/null 2>&1
    fi
    for _ in $(seq 1 30); do
        if fcitx5_is_ready; then
            break
        fi
        sleep 0.1
    done

    # re-add via dbus controller if active group still lacks clak
    if command -v busctl >/dev/null 2>&1; then
        group=$(busctl --user call org.fcitx.Fcitx5 /controller org.fcitx.Fcitx.Controller1 CurrentInputMethodGroup 2>/dev/null | sed -n 's/.*"\([^"]*\)".*/\1/p' || true)
        group="${group:-Default}"
        info=$(busctl --user call org.fcitx.Fcitx5 /controller org.fcitx.Fcitx.Controller1 InputMethodGroupInfo s "${group}" 2>/dev/null || true)
        if [ -n "${info}" ] && [[ "${info}" != *'"clak"'* ]] && [ -x "$HOME/.local/bin/clak" ]; then
            "$HOME/.local/bin/clak" doctor --fix >/dev/null 2>&1 || true
        fi
    fi
    fcitx5-remote -s clak >/dev/null 2>&1 || true

# clean build artifacts
[group("dev")]
clean:
    rm -rf build engine/target ui/target cli/target diagnostics/target target .scratch vendor clak-vendor.tar.gz clak-vendor.tar.gz.sha256

# prune stale cargo cache and incremental artifacts
[group("dev")]
prune:
    cargo clean
    rm -rf .scratch build/cargo-target engine/target ui/target cli/target diagnostics/target


# run clak settings gui
[group("gui")]
gui:
    cargo run -p clak-gui --release

# build clak settings gui binary
[group("gui")]
build-gui:
    cargo build -p clak-gui --release

# run all automated tests
[group("test")]
test: test-unit test-cpp test-scenario

# run all cargo tests
[group("test")]
test-unit:
    cargo test -p clak-engine

# run c++ state machine and regression test suites
[group("test")]
test-cpp:
    cmake --build build --target clak_cpp_tests
    ./build/src/tests/clak_cpp_tests

# test user typing scenario
[group("test")]
test-scenario:
    bash scripts/tests/test_user_scenario.sh

# test typing speed and accuracy
[group("test")]
test-speed delay="20":
    bash scripts/tests/test_speed.sh {{delay}}

# test chromium address bar typing and backspacing
[group("test")]
test-chromium delay="15":
    bash scripts/tests/test_chromium.sh {{delay}}

# test autocomplete selection in address bar
[group("test")]
test-autocomplete:
    bash scripts/tests/test_autocomplete_dd.sh

# test installer simulation flow
[group("test")]
test-install mode="":
    bash scripts/install.sh --dry-run {{mode}}

# test updater simulation flow
[group("test")]
test-update args="":
    bash scripts/update.sh --dry-run {{args}}

# run all fmt, clippy, unit tests and build check before pushing
[group("quality")]
check:
    cargo fmt --all -- --check
    cargo clippy --workspace -- -D warnings
    cargo test --workspace
    @if [ ! -f build/build.ninja ] && [ ! -f build/Makefile ]; then cmake -B build; fi
    cmake --build build --target clak_cpp_tests
    ./build/src/tests/clak_cpp_tests
    cmake --build build

# install git pre-push hook to run checks before pushing
[group("quality")]
install-hooks:
    @echo '#!/bin/sh' > .git/hooks/pre-push
    @echo 'echo "running pre-push checks..."' >> .git/hooks/pre-push
    @echo 'just check || exit 1' >> .git/hooks/pre-push
    @chmod +x .git/hooks/pre-push
    @echo "pre-push hook installed successfully"

# run latency benchmark analysis and regression assertion
[group("quality")]
bench *args:
    cargo run -p clak --release -- bench {{args}}

# run environment diagnostics
[group("quality")]
doctor *args:
    cargo run -p clak --release -- doctor {{args}}

# tail debug log
[group("debug")]
log:
    tail -f /tmp/clak.log

# clear debug log
[group("debug")]
clean-log:
    rm -f /tmp/clak.log

# release a new version, bump files, commit, tag and push
[group("release")]
tag version:
    #!/usr/bin/env bash
    set -euo pipefail
    raw="{{version}}"
    ver="${raw#v}"
    tag="v${ver}"
    branch=$(git branch --show-current)

    if git rev-parse "${tag}" >/dev/null 2>&1; then
        echo "error: tag ${tag} already exists, latest tag is $(git describe --tags --abbrev=0 2>/dev/null || echo 'none')"
        exit 1
    fi

    git update-index -q --refresh
    if ! git diff-index --quiet HEAD --; then
        echo "error: working tree is dirty, please commit or stash changes first"
        exit 1
    fi

    echo "Running pre-release quality checks..."
    just check

    echo "Releasing ${tag} (version ${ver})..."

    # update versions across project files
    sed -i "s/project(clak VERSION [0-9.]\+/project(clak VERSION ${ver}/" CMakeLists.txt
    sed -i "s/^Version=[0-9.]\+/Version=${ver}/" data/clak-addon.conf.in
    sed -i "0,/^version = \"[0-9.]\+\"/s//version = \"${ver}\"/" engine/Cargo.toml
    sed -i "0,/^version = \"[0-9.]\+\"/s//version = \"${ver}\"/" ui/Cargo.toml
    sed -i "s/^pkgver=[0-9.]\+/pkgver=${ver}/" packaging/aur/PKGBUILD
    sed -i "s/^pkgrel=[0-9]\+/pkgrel=1/" packaging/aur/PKGBUILD
    sed -i "s/^pkgver=[0-9.]\+/pkgver=${ver}/" packaging/aur-bin/PKGBUILD
    sed -i "s/^pkgrel=[0-9]\+/pkgrel=1/" packaging/aur-bin/PKGBUILD

    cargo check --workspace --quiet
    (cd packaging/aur && makepkg --printsrcinfo > .SRCINFO)
    (cd packaging/aur-bin && makepkg --printsrcinfo > .SRCINFO)

    git add CMakeLists.txt data/clak-addon.conf.in engine/Cargo.toml ui/Cargo.toml Cargo.lock packaging/aur/PKGBUILD packaging/aur/.SRCINFO packaging/aur-bin/PKGBUILD packaging/aur-bin/.SRCINFO
    git commit -m "chore: release ${tag}"
    git tag -a "${tag}" -m "release: ${tag}"

    # clean up local tag if push is rejected or interrupted
    trap 'echo "Push failed, removing tag ${tag}..."; git tag -d "${tag}" 2>/dev/null || true' ERR

    git push origin "${branch}"
    git push origin "${tag}"
    trap - ERR
    echo "Successfully released and pushed ${tag}"

# generate changelog with git-cliff
[group("release")]
changelog:
    git-cliff --unreleased

# update aur .srcinfo metadata
[group("release")]
pkg-aur:
    cd packaging/aur && makepkg --printsrcinfo > .SRCINFO
    cd packaging/aur-bin && makepkg --printsrcinfo > .SRCINFO

# create cargo vendor archive for offline packaging
[group("release")]
vendor:
    cargo vendor vendor/
    tar -czf clak-vendor.tar.gz vendor/
    sha256sum clak-vendor.tar.gz > clak-vendor.tar.gz.sha256
    rm -rf vendor/

# update clak to latest release
[group("release")]
update args="":
    bash scripts/update.sh {{args}}

# sync dev build to quickemu vm and restart fcitx5
[group("dev")]
vm user="verse":
    cmake --build build
    scp -P 22220 build/libclak.so {{user}}@localhost:/tmp/
    ssh -t -p 22220 {{user}}@localhost "mkdir -p ~/.local/lib/fcitx5 && cp /tmp/libclak.so ~/.local/lib/fcitx5/libclak.so; sudo cp /tmp/libclak.so /usr/lib/x86_64-linux-gnu/fcitx5/ 2>/dev/null || true; WAYLAND_DISPLAY=wayland-0 XDG_RUNTIME_DIR=/run/user/\$(id -u) fcitx5 -r -d"
