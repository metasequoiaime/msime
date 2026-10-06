set(TEST_EXECUTABLE_NAME "msimeui-tests")

add_executable(${TEST_EXECUTABLE_NAME}
    # The suite's own runner, not the server's entry point. Two renames in a row
    # rewrote this line as if it named platforms/windows/src/system/main.cpp,
    # leaving a path outside the repository that CMake cannot resolve - so the
    # whole msimeui suite stopped configuring, and therefore stopped reporting.
    src/main.cpp
    src/test_device_resources.cpp
    src/test_debug_log.cpp
    src/test_font_fallback.cpp
    src/test_fonts.cpp
    src/test_layout.cpp
    src/test_lru_cache.cpp
    src/test_menu_flyout_item.cpp
    src/test_text_selection.cpp
    includes/test_framework.h
)

# The test sources include their headers as "tests/includes/...", mirroring how
# server/ lays its tests out.
target_include_directories(${TEST_EXECUTABLE_NAME} PRIVATE ${CMAKE_CURRENT_SOURCE_DIR}/..)
target_link_libraries(${TEST_EXECUTABLE_NAME} PRIVATE msimeui)

if(MSVC)
    target_compile_options(${TEST_EXECUTABLE_NAME} PRIVATE /W4 /permissive-)
endif()

add_test(NAME ${TEST_EXECUTABLE_NAME} COMMAND ${TEST_EXECUTABLE_NAME})
