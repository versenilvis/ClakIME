cmake_minimum_required(VERSION 3.16)
project(clak VERSION 0.3.2 LANGUAGES C CXX)

if(NOT CMAKE_BUILD_TYPE AND NOT CMAKE_CONFIGURATION_TYPES)
    set(CMAKE_BUILD_TYPE "Release" CACHE STRING "Choose the type of build." FORCE)
endif()

include(CheckIPOSupported)
check_ipo_supported(RESULT ipo_supported OUTPUT ipo_error)
if(ipo_supported)
    set(CMAKE_INTERPROCEDURAL_OPTIMIZATION_RELEASE TRUE)
endif()

set(CMAKE_EXPORT_COMPILE_COMMANDS ON)
set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

find_package(ECM 1.0.0 REQUIRED)
set(CMAKE_MODULE_PATH ${ECM_MODULE_PATH} "${CMAKE_CURRENT_SOURCE_DIR}/cmake" ${CMAKE_MODULE_PATH})

include(FeatureSummary)
include(KDEInstallDirs)
include(KDECMakeSettings)
include(KDECompilerSettings NO_POLICY_SCOPE)

string(REPLACE "-fno-operator-names" "" CMAKE_CXX_FLAGS "${CMAKE_CXX_FLAGS}")
string(REPLACE "-fno-exceptions" "" CMAKE_CXX_FLAGS "${CMAKE_CXX_FLAGS}")

find_package(Fcitx5Core 5.0.14 REQUIRED)
include("${FCITX_INSTALL_CMAKECONFIG_DIR}/Fcitx5Utils/Fcitx5CompilerSettings.cmake")

include(CheckCXXSourceCompiles)
set(CMAKE_REQUIRED_LIBRARIES Fcitx5::Core)
check_cxx_source_compiles("
#include <fcitx/instance.h>
int main() {
    fcitx::Instance *inst = nullptr;
    inst->showCustomInputMethodInformation(nullptr, \"\");
    return 0;
}
" FCITX5_HAVE_SHOW_CUSTOM_IM_INFO)

find_program(CCACHE_PROGRAM ccache)
if(CCACHE_PROGRAM)
    set(CMAKE_C_COMPILER_LAUNCHER "${CCACHE_PROGRAM}")
    set(CMAKE_CXX_COMPILER_LAUNCHER "${CCACHE_PROGRAM}")
endif()

if(FCITX5_HAVE_SHOW_CUSTOM_IM_INFO)
    add_compile_definitions(FCITX5_HAVE_SHOW_CUSTOM_IM_INFO=1)
endif()

set(CARGO_TARGET_DIR "${CMAKE_CURRENT_BINARY_DIR}/cargo-target")
set(CLAK_ENGINE_DIR "${CMAKE_CURRENT_SOURCE_DIR}/engine")
set(CLAK_ENGINE_LIB "${CARGO_TARGET_DIR}/release/libclak_engine.a")

file(GLOB_RECURSE CLAK_ENGINE_SOURCES CONFIGURE_DEPENDS
    "${CLAK_ENGINE_DIR}/src/*.rs"
    "${CLAK_ENGINE_DIR}/data/*"
    "${CLAK_ENGINE_DIR}/Cargo.toml"
)

add_custom_command(
    OUTPUT ${CLAK_ENGINE_LIB}
    COMMAND cargo build --release
        --manifest-path "${CLAK_ENGINE_DIR}/Cargo.toml"
        --target-dir "${CARGO_TARGET_DIR}"
    WORKING_DIRECTORY ${CLAK_ENGINE_DIR}
    DEPENDS ${CLAK_ENGINE_SOURCES}
    VERBATIM
)
add_custom_target(clak_engine_build DEPENDS ${CLAK_ENGINE_LIB})

option(ENABLE_GUI "Build GUI configuration app" ON)

if(ENABLE_GUI AND EXISTS "${CMAKE_CURRENT_SOURCE_DIR}/ui/Cargo.toml")
    set(CLAK_UI_DIR "${CMAKE_CURRENT_SOURCE_DIR}/ui")
    set(CLAK_UI_BIN "${CARGO_TARGET_DIR}/release/clak-gui")

    file(GLOB_RECURSE CLAK_UI_SOURCES CONFIGURE_DEPENDS
        "${CLAK_UI_DIR}/src/*"
        "${CLAK_UI_DIR}/Cargo.toml"
        "${CLAK_UI_DIR}/build.rs"
    )

    add_custom_command(
        OUTPUT ${CLAK_UI_BIN}
        COMMAND cargo build --release
            --manifest-path "${CLAK_UI_DIR}/Cargo.toml"
            --target-dir "${CARGO_TARGET_DIR}"
        WORKING_DIRECTORY ${CLAK_UI_DIR}
        DEPENDS ${CLAK_UI_SOURCES} ${CLAK_ENGINE_SOURCES}
        VERBATIM
    )
    add_custom_target(clak_gui_build ALL DEPENDS ${CLAK_UI_BIN})
endif()

option(ENABLE_CLI "Build CLI utility app" ON)

if(ENABLE_CLI AND EXISTS "${CMAKE_CURRENT_SOURCE_DIR}/cli/Cargo.toml")
    set(CLAK_CLI_DIR "${CMAKE_CURRENT_SOURCE_DIR}/cli")
    set(CLAK_CLI_BIN "${CARGO_TARGET_DIR}/release/clak")

    file(GLOB_RECURSE CLAK_CLI_SOURCES CONFIGURE_DEPENDS
        "${CLAK_CLI_DIR}/src/*"
        "${CLAK_CLI_DIR}/Cargo.toml"
    )

    file(GLOB_RECURSE CLAK_DIAG_SOURCES CONFIGURE_DEPENDS
        "${CMAKE_CURRENT_SOURCE_DIR}/diagnostics/src/*"
        "${CMAKE_CURRENT_SOURCE_DIR}/diagnostics/Cargo.toml"
    )

    add_custom_command(
        OUTPUT ${CLAK_CLI_BIN}
        COMMAND cargo build --release
            --manifest-path "${CLAK_CLI_DIR}/Cargo.toml"
            --target-dir "${CARGO_TARGET_DIR}"
        WORKING_DIRECTORY ${CLAK_CLI_DIR}
        DEPENDS ${CLAK_CLI_SOURCES} ${CLAK_DIAG_SOURCES}
        VERBATIM
    )
    add_custom_target(clak_cli_build ALL DEPENDS ${CLAK_CLI_BIN})
endif()

add_library(clak_rust_engine INTERFACE)
target_link_libraries(clak_rust_engine INTERFACE "${CLAK_ENGINE_LIB}")
add_dependencies(clak_rust_engine clak_engine_build)

set(CLAK_SOURCES
    src/config/sites.cpp
    src/platform/hyprland.cpp
    src/platform/modal_editor.cpp
    src/platform/mouse_tracker.cpp
    src/uinput/uinput.cpp
    src/utils/log.cpp
    src/utils/text_utils.cpp
    src/ime/state.cpp
    src/engine.cpp
)

add_library(clak_core OBJECT ${CLAK_SOURCES})
set_target_properties(clak_core PROPERTIES
    POSITION_INDEPENDENT_CODE ON
)
target_include_directories(clak_core PUBLIC
    ${CMAKE_CURRENT_SOURCE_DIR}/src
    ${CMAKE_CURRENT_SOURCE_DIR}/engine
)
target_link_libraries(clak_core PUBLIC
    Fcitx5::Core
    clak_rust_engine
    input
    udev
    pthread
    dl
    m
)

add_library(clak MODULE $<TARGET_OBJECTS:clak_core>)
set_target_properties(clak PROPERTIES
    OUTPUT_NAME "clak"
    PREFIX "lib"
    POSITION_INDEPENDENT_CODE ON
)
target_link_options(clak PRIVATE "-Wl,-exclude-libs,ALL")

target_include_directories(clak PRIVATE
    ${CMAKE_CURRENT_SOURCE_DIR}/src
    ${CMAKE_CURRENT_SOURCE_DIR}/engine
)

target_link_libraries(clak PRIVATE
    Fcitx5::Core
    clak_rust_engine
    input
    udev
    pthread
    dl
    m
)

configure_file(
    "${CMAKE_CURRENT_SOURCE_DIR}/data/clak-addon.conf.in"
    "${CMAKE_CURRENT_BINARY_DIR}/clak.conf"
    COPYONLY
)

file(GENERATE
    OUTPUT "${CMAKE_CURRENT_BINARY_DIR}/clak-local.conf"
    CONTENT "[Addon]\nName=Clak\nName[vi]=Bộ gõ tiếng Việt Clak\nComment=Fast and stable Vietnamese input method for Fcitx5\nCategory=InputMethod\nVersion=${PROJECT_VERSION}\nEnabled=True\nIcon=org.fcitx.Fcitx5.clak\nLibrary=${CMAKE_CURRENT_BINARY_DIR}/libclak\nType=SharedLibrary\nOnDemand=True\nConfigurable=True\n[Dependencies]\n0=core:5.0.14\n"
)

install(TARGETS clak DESTINATION "${FCITX_INSTALL_ADDONDIR}")
if(TARGET clak_gui_build)
    install(PROGRAMS "${CLAK_UI_BIN}" DESTINATION "${CMAKE_INSTALL_BINDIR}")
    install(FILES "${CMAKE_CURRENT_SOURCE_DIR}/data/clak-gui.desktop" DESTINATION "${CMAKE_INSTALL_DATADIR}/applications")
endif()
if(TARGET clak_cli_build)
    install(PROGRAMS "${CLAK_CLI_BIN}" DESTINATION "${CMAKE_INSTALL_BINDIR}")
endif()
install(FILES "${CMAKE_CURRENT_BINARY_DIR}/clak.conf" DESTINATION "${FCITX_INSTALL_PKGDATADIR}/addon")
install(FILES "${CMAKE_CURRENT_SOURCE_DIR}/data/clak.conf" DESTINATION "${FCITX_INSTALL_PKGDATADIR}/inputmethod")
install(DIRECTORY "${CMAKE_CURRENT_SOURCE_DIR}/data/icons/" DESTINATION "${CMAKE_INSTALL_DATADIR}/icons")

add_custom_target(install-user
    COMMAND ${CMAKE_COMMAND} -E make_directory "$ENV{HOME}/.local/share/fcitx5/addon"
    COMMAND ${CMAKE_COMMAND} -E make_directory "$ENV{HOME}/.local/share/fcitx5/inputmethod"
    COMMAND ${CMAKE_COMMAND} -E copy "${CMAKE_CURRENT_BINARY_DIR}/clak-local.conf" "$ENV{HOME}/.local/share/fcitx5/addon/clak.conf"
    COMMAND ${CMAKE_COMMAND} -E copy "${CMAKE_CURRENT_SOURCE_DIR}/data/clak.conf" "$ENV{HOME}/.local/share/fcitx5/inputmethod/clak.conf"
    COMMAND ${CMAKE_COMMAND} -E copy_directory "${CMAKE_CURRENT_SOURCE_DIR}/data/icons/hicolor" "$ENV{HOME}/.local/share/icons/hicolor"
    COMMAND gtk-update-icon-cache -f -t "$ENV{HOME}/.local/share/icons/hicolor" || true
    DEPENDS clak
)
if(TARGET clak_gui_build)
    add_custom_command(TARGET install-user POST_BUILD
        COMMAND ${CMAKE_COMMAND} -E make_directory "$ENV{HOME}/.local/bin"
        COMMAND ${CMAKE_COMMAND} -E make_directory "$ENV{HOME}/.local/share/applications"
        COMMAND ${CMAKE_COMMAND} -E copy "${CLAK_UI_BIN}" "$ENV{HOME}/.local/bin/clak-gui"
        COMMAND ${CMAKE_COMMAND} -E copy "${CMAKE_CURRENT_SOURCE_DIR}/data/clak-gui.desktop" "$ENV{HOME}/.local/share/applications/clak-gui.desktop"
        COMMAND sed -i "s|^Exec=.*|Exec=$ENV{HOME}/.local/bin/clak-gui|" "$ENV{HOME}/.local/share/applications/clak-gui.desktop"
        COMMAND update-desktop-database "$ENV{HOME}/.local/share/applications" || true
    )
    add_dependencies(install-user clak_gui_build)
endif()
if(TARGET clak_cli_build)
    add_custom_command(TARGET install-user POST_BUILD
        COMMAND ${CMAKE_COMMAND} -E make_directory "$ENV{HOME}/.local/bin"
        COMMAND ${CMAKE_COMMAND} -E copy "${CLAK_CLI_BIN}" "$ENV{HOME}/.local/bin/clak"
    )
    add_dependencies(install-user clak_cli_build)
endif()

include(CTest)
if(BUILD_TESTING)
    find_package(GTest QUIET)
    if(GTest_FOUND)
        add_subdirectory(src/tests)
    else()
        message(STATUS "GTest not found; C++ unit tests will not be built")
    endif()
endif()
