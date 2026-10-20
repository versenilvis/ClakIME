find_package(GTest QUIET)

add_executable(clak_cpp_tests EXCLUDE_FROM_ALL
    mock_input_context.cpp
    test_clak_state.cpp
    test_regression.cpp
    test_adaptive_pacing.cpp
    $<TARGET_OBJECTS:clak_core>
)

target_include_directories(clak_cpp_tests PRIVATE
    ${CMAKE_CURRENT_SOURCE_DIR}
    ${CMAKE_SOURCE_DIR}/src
    ${CMAKE_SOURCE_DIR}/engine
)

target_link_directories(clak_cpp_tests PRIVATE
    $ENV{HOME}/.local/usr/lib/x86_64-linux-gnu
    /usr/lib/x86_64-linux-gnu
)


target_link_libraries(clak_cpp_tests PRIVATE
    GTest::gtest
    GTest::gtest_main
    Fcitx5::Core
    clak_rust_engine
    input
    udev
    pthread
    dl
    m
)

add_test(NAME clak_cpp_tests COMMAND clak_cpp_tests)
