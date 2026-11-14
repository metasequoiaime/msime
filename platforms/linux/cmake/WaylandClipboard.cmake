if(NOT WAYLAND_CLIENT_FOUND OR NOT WAYLAND_SCANNER_EXECUTABLE)
  return()
endif()

set(MSIME_DATA_CONTROL_XML "${CMAKE_CURRENT_SOURCE_DIR}/data/wayland/wlr-data-control-unstable-v1.xml")
set(MSIME_DATA_CONTROL_HEADER "${MSIME_WAYLAND_PROTOCOL_DIR}/wlr-data-control-unstable-v1-client-protocol.h")
set(MSIME_DATA_CONTROL_CODE "${MSIME_WAYLAND_PROTOCOL_DIR}/wlr-data-control-unstable-v1-protocol.c")
add_custom_command(
  OUTPUT "${MSIME_DATA_CONTROL_HEADER}" "${MSIME_DATA_CONTROL_CODE}"
  COMMAND "${CMAKE_COMMAND}" -E make_directory "${MSIME_WAYLAND_PROTOCOL_DIR}"
  COMMAND "${WAYLAND_SCANNER_EXECUTABLE}" client-header "${MSIME_DATA_CONTROL_XML}" "${MSIME_DATA_CONTROL_HEADER}"
  COMMAND "${WAYLAND_SCANNER_EXECUTABLE}" private-code "${MSIME_DATA_CONTROL_XML}" "${MSIME_DATA_CONTROL_CODE}"
  DEPENDS "${MSIME_DATA_CONTROL_XML}")
add_library(msime-wayland-clipboard-bridge STATIC src/clipboard/WaylandClipboardBridge.cpp "${MSIME_DATA_CONTROL_CODE}")
target_compile_options(msime-wayland-clipboard-bridge PRIVATE -Wall -Wextra -Werror)
target_include_directories(msime-wayland-clipboard-bridge PRIVATE "${MSIME_WAYLAND_PROTOCOL_DIR}")
target_link_libraries(msime-wayland-clipboard-bridge PUBLIC PkgConfig::WAYLAND_CLIENT nlohmann_json::nlohmann_json)
add_executable(msime-linux-clipboard-watch-wayland src/clipboard/clipboard_watch_wayland.cpp)
target_compile_options(msime-linux-clipboard-watch-wayland PRIVATE -Wall -Wextra -Werror)
target_link_libraries(msime-linux-clipboard-watch-wayland PRIVATE msime-wayland-clipboard-bridge)
install(TARGETS msime-linux-clipboard-watch-wayland RUNTIME DESTINATION "${CMAKE_INSTALL_BINDIR}" COMPONENT clipboard-watch)

if(BUILD_TESTING)
  add_executable(msime-test-wayland-clipboard-owner tests/clipboard/wayland_clipboard_owner.cpp "${MSIME_DATA_CONTROL_CODE}")
  target_compile_options(msime-test-wayland-clipboard-owner PRIVATE -Wall -Wextra -Werror)
  target_include_directories(msime-test-wayland-clipboard-owner PRIVATE "${MSIME_WAYLAND_PROTOCOL_DIR}")
  target_link_libraries(msime-test-wayland-clipboard-owner PRIVATE PkgConfig::WAYLAND_CLIENT)
  find_program(MSIME_SWAY_EXECUTABLE sway)
  find_program(MSIME_WL_COPY_EXECUTABLE wl-copy)
  if(MSIME_SWAY_EXECUTABLE AND MSIME_WL_COPY_EXECUTABLE)
    add_test(NAME linux-wayland-cloud-clipboard
      COMMAND "${Python3_EXECUTABLE}" "${CMAKE_CURRENT_SOURCE_DIR}/tests/clipboard/clipboard_wayland_bridge.py"
        $<TARGET_FILE:msime-linux-clipboard-watch-wayland>
        --owner $<TARGET_FILE:msime-test-wayland-clipboard-owner>
        --crash-controller $<TARGET_FILE:common-capture-only-test>
        --version "${MSIME_LINUX_VERSION}" --client-directory "${MSIME_CLIENT_DIRECTORY}"
        --telemetry-directory "${MSIME_EDITION_FCITX5_ADDON}")
    set_tests_properties(linux-wayland-cloud-clipboard PROPERTIES TIMEOUT 90)
  endif()
endif()
