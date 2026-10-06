# run "just -l" to view all commands
default:
    @just --list

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
    fcitx5 -r -d

# clean build artifacts
[group("dev")]
clean:
    rm -rf build engine/target ui/target target .scratch vendor clak-vendor.tar.gz clak-vendor.tar.gz.sha256

# prune stale cargo cache and incremental artifacts
[group("dev")]
prune:
    cargo clean --manifest-path engine/Cargo.toml
    cargo clean --manifest-path ui/Cargo.toml
    rm -rf .scratch


# run clak settings gui
[group("gui")]
gui:
    cargo run --manifest-path ui/Cargo.toml --release

# build clak settings gui binary
[group("gui")]
build-gui:
    cargo build --manifest-path ui/Cargo.toml --release

# run all automated tests
[group("test")]
test: test-unit test-cpp test-scenario

# run all cargo tests
[group("test")]
test-unit:
    cargo test --manifest-path engine/Cargo.toml

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
    cargo fmt --manifest-path engine/Cargo.toml -- --check
    cargo clippy --manifest-path engine/Cargo.toml -- -D warnings
    cargo test --manifest-path engine/Cargo.toml
    cargo test --manifest-path diagnostics/Cargo.toml
    cargo test --manifest-path cli/Cargo.toml
    cargo test --manifest-path ui/Cargo.toml
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
    cargo run --manifest-path cli/Cargo.toml --release -- bench {{args}}

# run environment diagnostics
[group("quality")]
doctor *args:
    cargo run --manifest-path cli/Cargo.toml --release -- doctor {{args}}

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

    if ! git diff-index --quiet HEAD --; then
        echo "error: working tree is dirty, please commit or stash changes first"
        exit 1
    fi

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

    cargo check --manifest-path engine/Cargo.toml --quiet
    cargo check --manifest-path ui/Cargo.toml --quiet
    (cd packaging/aur && makepkg --printsrcinfo > .SRCINFO)
    (cd packaging/aur-bin && makepkg --printsrcinfo > .SRCINFO)

    git add CMakeLists.txt data/clak-addon.conf.in engine/Cargo.toml engine/Cargo.lock ui/Cargo.toml ui/Cargo.lock packaging/aur/PKGBUILD packaging/aur/.SRCINFO packaging/aur-bin/PKGBUILD packaging/aur-bin/.SRCINFO
    git commit -m "chore: release ${tag}"
    git tag -a "${tag}" -m "release: ${tag}"

    git push origin "${branch}"
    git push origin "${tag}"
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
    cargo vendor vendor/ --manifest-path engine/Cargo.toml
    tar -czf clak-vendor.tar.gz vendor/
    sha256sum clak-vendor.tar.gz > clak-vendor.tar.gz.sha256
    rm -rf vendor/

# update clak to latest release
[group("release")]
update args="":
    bash scripts/update.sh {{args}}
