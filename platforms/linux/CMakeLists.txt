cmake_minimum_required(VERSION 3.25)
project(MSIMEClientLinux LANGUAGES CXX)
find_package(CURL REQUIRED)
include(GNUInstallDirs)
# The user units msime-linux-setup can enable. The CMake uninstall and the Debian maintainer scripts all stop them, so the list is defined once here; each socket precedes its service so stopping in this order does not activate the service again. The maintainer scripts restart only the services: sockets run no program.
set(MSIME_USER_UNITS msime-linux-online.socket msime-linux-online.service msime-linux-voice.socket
    msime-linux-voice.service msime-linux-clipboard.service)
set(MSIME_USER_SERVICES ${MSIME_USER_UNITS})
list(FILTER MSIME_USER_SERVICES INCLUDE REGEX "\\.service$")
list(JOIN MSIME_USER_UNITS " " MSIME_USER_UNIT_LIST)
list(JOIN MSIME_USER_SERVICES " " MSIME_USER_SERVICE_LIST)
# Session managers read XDG autostart entries from $XDG_CONFIG_DIRS/autostart, /etc/xdg/autostart by default. GNUInstallDirs maps the system configuration directory of a /usr prefix to /etc, so only then is the entry installed there, as an absolute directory the CMake uninstall accepts as a second root. Every other prefix keeps the directory relative, so `cmake --install --prefix` relocates it with the rest of the installation; that includes /opt/<name>, whose /etc/opt/<name> no session manager reads.
if(CMAKE_INSTALL_FULL_SYSCONFDIR STREQUAL "/etc")
  set(MSIME_XDG_AUTOSTART_DIR "/etc/xdg/autostart")
else()
  set(MSIME_XDG_AUTOSTART_DIR "${CMAKE_INSTALL_SYSCONFDIR}/xdg/autostart")
endif()
configure_file(cmake/uninstall.cmake.in ${CMAKE_CURRENT_BINARY_DIR}/uninstall.cmake @ONLY)
# Configured for every build, not only packaging ones, so the gate's ctest exercises the scripts the .deb ships; packaging.cmake hands them to CPack.
foreach(MSIME_DEB_SCRIPT prerm postinst)
  configure_file(cmake/deb-${MSIME_DEB_SCRIPT}.in ${CMAKE_CURRENT_BINARY_DIR}/debian/${MSIME_DEB_SCRIPT} @ONLY
                 FILE_PERMISSIONS OWNER_READ OWNER_WRITE OWNER_EXECUTE GROUP_READ GROUP_EXECUTE WORLD_READ WORLD_EXECUTE)
endforeach()
add_custom_target(uninstall
  COMMAND "${CMAKE_COMMAND}" -P "${CMAKE_CURRENT_BINARY_DIR}/uninstall.cmake"
  COMMENT "Remove installed MSIME files while preserving runtime configuration")
include(CTest)
if(BUILD_TESTING)
  find_package(Python3 COMPONENTS Interpreter REQUIRED)
  add_test(NAME linux-fcitx5-contract
           COMMAND "${Python3_EXECUTABLE}" "${CMAKE_CURRENT_SOURCE_DIR}/tests/core/fcitx5_contract.py")
endif()
set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
find_package(PkgConfig REQUIRED)
include_directories("${CMAKE_CURRENT_SOURCE_DIR}/src/core" "${CMAKE_CURRENT_SOURCE_DIR}/src/candidates" "${CMAKE_CURRENT_SOURCE_DIR}/src/clipboard" "${CMAKE_CURRENT_SOURCE_DIR}/src/providers" "${CMAKE_CURRENT_SOURCE_DIR}/src/voice" "${CMAKE_CURRENT_SOURCE_DIR}/src/overlay" "${CMAKE_CURRENT_SOURCE_DIR}/src/system" "${CMAKE_CURRENT_SOURCE_DIR}/src/entrypoints")
pkg_check_modules(IBUS REQUIRED IMPORTED_TARGET ibus-1.0>=1.5.20)
pkg_check_modules(XKBCOMMON REQUIRED IMPORTED_TARGET xkbcommon)
find_package(nlohmann_json 3.11 REQUIRED)
# Optional on Wayland-only systems; the monitor retains bounded polling.
pkg_check_modules(X11_CLIPBOARD QUIET IMPORTED_TARGET x11 xfixes)
# The voice overlay enumerates monitors with RandR 1.5 to follow the focused window; libXrandr ships with every X desktop that has libXfixes.
pkg_check_modules(X11_SURFACE QUIET IMPORTED_TARGET x11 xfixes xrandr)
pkg_check_modules(WAYLAND_CLIENT QUIET IMPORTED_TARGET wayland-client)
pkg_check_modules(WAYLAND_TEXT QUIET IMPORTED_TARGET pangocairo)
find_program(WAYLAND_SCANNER_EXECUTABLE wayland-scanner)
if(WAYLAND_CLIENT_FOUND AND WAYLAND_SCANNER_EXECUTABLE)
  enable_language(C)
  set(MSIME_WAYLAND_PROTOCOL_DIR "${CMAKE_CURRENT_BINARY_DIR}/wayland-protocols")
  set(MSIME_LAYER_SHELL_HEADER
      "${MSIME_WAYLAND_PROTOCOL_DIR}/wlr-layer-shell-unstable-v1-client-protocol.h")
  set(MSIME_LAYER_SHELL_CODE
      "${MSIME_WAYLAND_PROTOCOL_DIR}/wlr-layer-shell-unstable-v1-protocol.c")
  find_path(MSIME_XDG_SHELL_DIR NAMES xdg-shell.xml
            PATHS /usr/share/wayland-protocols/stable/xdg-shell
                  /usr/local/share/wayland-protocols/stable/xdg-shell)
  set(MSIME_XDG_SHELL_XML "${MSIME_XDG_SHELL_DIR}/xdg-shell.xml")
  set(MSIME_XDG_SHELL_HEADER
      "${MSIME_WAYLAND_PROTOCOL_DIR}/xdg-shell-client-protocol.h")
  set(MSIME_XDG_SHELL_CODE
      "${MSIME_WAYLAND_PROTOCOL_DIR}/xdg-shell-protocol.c")
  if(NOT EXISTS "${MSIME_XDG_SHELL_XML}")
    unset(MSIME_XDG_SHELL_XML)
  endif()
  add_custom_command(
    OUTPUT "${MSIME_LAYER_SHELL_HEADER}" "${MSIME_LAYER_SHELL_CODE}"
    COMMAND "${CMAKE_COMMAND}" -E make_directory "${MSIME_WAYLAND_PROTOCOL_DIR}"
    COMMAND "${WAYLAND_SCANNER_EXECUTABLE}" client-header
            "${CMAKE_CURRENT_SOURCE_DIR}/data/wayland/wlr-layer-shell-unstable-v1.xml"
            "${MSIME_LAYER_SHELL_HEADER}"
    COMMAND "${WAYLAND_SCANNER_EXECUTABLE}" private-code
            "${CMAKE_CURRENT_SOURCE_DIR}/data/wayland/wlr-layer-shell-unstable-v1.xml"
            "${MSIME_LAYER_SHELL_CODE}"
    DEPENDS data/wayland/wlr-layer-shell-unstable-v1.xml)
  if(MSIME_XDG_SHELL_XML)
    add_custom_command(
      OUTPUT "${MSIME_XDG_SHELL_HEADER}" "${MSIME_XDG_SHELL_CODE}"
      COMMAND "${CMAKE_COMMAND}" -E make_directory "${MSIME_WAYLAND_PROTOCOL_DIR}"
      COMMAND "${WAYLAND_SCANNER_EXECUTABLE}" client-header
              "${MSIME_XDG_SHELL_XML}" "${MSIME_XDG_SHELL_HEADER}"
      COMMAND "${WAYLAND_SCANNER_EXECUTABLE}" private-code
              "${MSIME_XDG_SHELL_XML}" "${MSIME_XDG_SHELL_CODE}"
      DEPENDS "${MSIME_XDG_SHELL_XML}")
  endif()
endif()
if(X11_CLIPBOARD_FOUND)
  add_executable(msime-linux-clipboard-watch-x11 src/clipboard/clipboard_watch_x11.cpp)
  target_compile_options(msime-linux-clipboard-watch-x11 PRIVATE -Wall -Wextra -Werror)
  target_link_libraries(msime-linux-clipboard-watch-x11 PRIVATE PkgConfig::X11_CLIPBOARD)
  install(TARGETS msime-linux-clipboard-watch-x11 RUNTIME DESTINATION "${CMAKE_INSTALL_BINDIR}" COMPONENT clipboard-watch)
  if(BUILD_TESTING)
    add_executable(msime-test-x11-string-owner tests/clipboard/clipboard_x11_string_owner.cpp)
    target_compile_options(msime-test-x11-string-owner PRIVATE -Wall -Wextra -Werror)
    target_link_libraries(msime-test-x11-string-owner PRIVATE PkgConfig::X11_CLIPBOARD)
  endif()
endif()
option(MSIME_LINUX_VOICE "Build voice worker test" OFF)
set(MSIME_HOST_LIBRARY "${CMAKE_CURRENT_SOURCE_DIR}/../../target/debug/libmsime_host_api.so" CACHE FILEPATH "Cargo-built Linux host library")
set(MSIME_RUNTIME_OPTIONS_FILE "" CACHE FILEPATH "Prepared runtime options")
set(MSIME_DESKTOP_BINARY "" CACHE FILEPATH "Cargo-built Tauri desktop binary")
set(MSIME_MCP_BINARY "" CACHE FILEPATH "Cargo-built MCP server (msime-mcp)")
set(MSIME_HANDWRITING_MODEL_DIR "" CACHE PATH "Offline handwriting model with its license, as fetched by scripts/fetch_handwriting_model.py; empty uses target/handwriting-model when it is there")
set(MSIME_EMOJI_RESOURCES "" CACHE PATH "Trusted packaged Engine Emoji resource directory")
set(MSIME_ENGINE_RESOURCES "" CACHE PATH "Trusted packaged Engine desktop resource directory")
if(NOT EXISTS "${MSIME_HOST_LIBRARY}")
  message(FATAL_ERROR "Build msime-host-api for Linux first")
endif()
add_library(msime-ibus-host STATIC
  src/core/ClientEngine.cpp src/system/DiagnosticLog.cpp src/system/ChineseTextConversion.cpp src/overlay/WaveOverlayIbusSurface.cpp
  src/overlay/WaveOverlaySurfaceFactory.cpp)
target_compile_options(msime-ibus-host PRIVATE -Wall -Wextra -Werror)
target_include_directories(msime-ibus-host PUBLIC src src/core src/candidates src/clipboard src/providers src/voice src/overlay src/system src/entrypoints ../../crates/host-api/include)
target_link_libraries(msime-ibus-host PUBLIC PkgConfig::IBUS PkgConfig::XKBCOMMON nlohmann_json::nlohmann_json "${MSIME_HOST_LIBRARY}")
if(X11_SURFACE_FOUND)
  target_sources(msime-ibus-host PRIVATE src/overlay/WaveOverlayX11Surface.cpp)
  target_compile_definitions(msime-ibus-host PRIVATE MSIME_LINUX_HAS_X11_SURFACE)
  target_link_libraries(msime-ibus-host PUBLIC PkgConfig::X11_SURFACE)
endif()
if(WAYLAND_CLIENT_FOUND AND WAYLAND_SCANNER_EXECUTABLE AND MSIME_XDG_SHELL_XML)
  target_sources(msime-ibus-host PRIVATE src/overlay/WaveOverlayWaylandSurface.cpp
                 "${MSIME_LAYER_SHELL_CODE}" ${MSIME_XDG_SHELL_CODE})
  target_include_directories(msime-ibus-host PRIVATE "${MSIME_WAYLAND_PROTOCOL_DIR}")
  target_compile_definitions(msime-ibus-host PRIVATE MSIME_LINUX_HAS_WAYLAND_SURFACE)
  target_link_libraries(msime-ibus-host PUBLIC PkgConfig::WAYLAND_CLIENT)
  if(WAYLAND_TEXT_FOUND)
    target_compile_definitions(msime-ibus-host PRIVATE MSIME_LINUX_WAYLAND_TEXT)
    target_link_libraries(msime-ibus-host PUBLIC PkgConfig::WAYLAND_TEXT)
  endif()
endif()
# The version the IBus host reports in telemetry, which is also the package version packaging.cmake hands to CPack. Like MSIME_WINDOWS_VERSION on Windows it defaults to this platform's version.txt, the file the release workflow reads for the linux-vVERSION tag; package-container.sh passes the release version explicitly.
set(MSIME_PACKAGE_VERSION "" CACHE STRING "Package version; empty uses platforms/linux/version.txt")
if(MSIME_PACKAGE_VERSION)
  set(MSIME_LINUX_VERSION "${MSIME_PACKAGE_VERSION}")
else()
  set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS "${CMAKE_CURRENT_SOURCE_DIR}/version.txt")
  file(STRINGS "${CMAKE_CURRENT_SOURCE_DIR}/version.txt" MSIME_LINUX_VERSION LIMIT_COUNT 1)
  string(STRIP "${MSIME_LINUX_VERSION}" MSIME_LINUX_VERSION)
endif()
if(NOT MSIME_LINUX_VERSION MATCHES "^[0-9]+\\.[0-9]+\\.[0-9]+$")
  message(FATAL_ERROR "The Linux package version must be MAJOR.MINOR.PATCH: ${MSIME_LINUX_VERSION}")
endif()
find_package(Threads REQUIRED)
add_executable(msime-linux-ibus src/entrypoints/ibus_main.cpp src/system/SystemTheme.cpp)
target_sources(msime-linux-ibus PRIVATE ../common/Telemetry.cpp)
target_include_directories(msime-linux-ibus PRIVATE ../common)
target_compile_definitions(msime-linux-ibus PRIVATE MSIME_LINUX_VERSION="${MSIME_LINUX_VERSION}")
target_link_libraries(msime-linux-ibus PRIVATE CURL::libcurl Threads::Threads)
target_compile_options(msime-linux-ibus PRIVATE -Wall -Wextra -Werror)
target_link_libraries(msime-linux-ibus PRIVATE msime-ibus-host)
add_executable(msime-linux-dictionary src/providers/dictionary_request.cpp)
add_executable(msime-linux-prepare src/entrypoints/prepare_host_main.cpp)
target_compile_options(msime-linux-prepare PRIVATE -Wall -Wextra -Werror)
target_link_libraries(msime-linux-prepare PRIVATE msime-ibus-host)
file(RELATIVE_PATH MSIME_INSTALLED_RESOURCE_RELATIVE_DIR
     "${CMAKE_INSTALL_FULL_BINDIR}"
     "${CMAKE_INSTALL_FULL_DATADIR}/msime-client/resources")
target_compile_definitions(msime-linux-prepare PRIVATE
  MSIME_INSTALLED_RESOURCE_RELATIVE_DIR="${MSIME_INSTALLED_RESOURCE_RELATIVE_DIR}")
# The IBus engine names the installed built-in sound packs to the Host API, found from its own executable as the prepare tool finds the resources (core/PreparePaths.h).
file(RELATIVE_PATH MSIME_INSTALLED_SOUND_PACKS_RELATIVE_DIR
     "${CMAKE_INSTALL_FULL_BINDIR}"
     "${CMAKE_INSTALL_FULL_DATADIR}/msime-client/sound-packs")
target_compile_definitions(msime-ibus-host PRIVATE
  MSIME_INSTALLED_SOUND_PACKS_RELATIVE_DIR="${MSIME_INSTALLED_SOUND_PACKS_RELATIVE_DIR}")
target_compile_options(msime-linux-dictionary PRIVATE -Wall -Wextra -Werror)
target_link_libraries(msime-linux-dictionary PRIVATE msime-ibus-host)
add_executable(msime-linux-cloud-dictionary src/providers/cloud_dictionary_request.cpp)
target_compile_options(msime-linux-cloud-dictionary PRIVATE -Wall -Wextra -Werror)
target_link_libraries(msime-linux-cloud-dictionary PRIVATE msime-ibus-host)
add_executable(msime-linux-cloud-clipboard src/providers/cloud_clipboard_request.cpp)
target_compile_options(msime-linux-cloud-clipboard PRIVATE -Wall -Wextra -Werror)
target_link_libraries(msime-linux-cloud-clipboard PRIVATE msime-ibus-host)
add_executable(msime-linux-online src/providers/online_request.cpp)
target_compile_options(msime-linux-online PRIVATE -Wall -Wextra -Werror)
target_link_libraries(msime-linux-online PRIVATE msime-ibus-host)
add_executable(msime-linux-online-provider-contract tests/provider/online_provider_contract.cpp)
target_compile_options(msime-linux-online-provider-contract PRIVATE -Wall -Wextra -Werror)
target_include_directories(msime-linux-online-provider-contract PRIVATE ../../crates/host-api/include)
target_link_libraries(msime-linux-online-provider-contract PRIVATE nlohmann_json::nlohmann_json "${MSIME_HOST_LIBRARY}")
add_test(NAME linux-online-provider-contract COMMAND msime-linux-online-provider-contract)
add_executable(msime-linux-translation src/providers/translation_request.cpp)
target_compile_options(msime-linux-translation PRIVATE -Wall -Wextra -Werror)
target_link_libraries(msime-linux-translation PRIVATE msime-ibus-host)
add_executable(msime-linux-clipboard src/clipboard/clipboard_history.cpp)
target_compile_options(msime-linux-clipboard PRIVATE -Wall -Wextra -Werror)
target_link_libraries(msime-linux-clipboard PRIVATE nlohmann_json::nlohmann_json)
add_executable(linux-clipboard-atomic-write-test tests/clipboard/clipboard_atomic_write.cpp)
target_compile_options(linux-clipboard-atomic-write-test PRIVATE -Wall -Wextra -Werror -UNDEBUG)
target_include_directories(linux-clipboard-atomic-write-test PRIVATE src)
add_test(NAME linux-clipboard-atomic-write COMMAND linux-clipboard-atomic-write-test)
add_executable(msime-linux-clipboard-capture src/clipboard/clipboard_capture.cpp)
target_compile_options(msime-linux-clipboard-capture PRIVATE -Wall -Wextra -Werror)
target_link_libraries(msime-linux-clipboard-capture PRIVATE msime-ibus-host)
add_executable(msime-linux-handwriting src/providers/handwriting_request.cpp)
target_compile_options(msime-linux-handwriting PRIVATE -Wall -Wextra -Werror)
target_link_libraries(msime-linux-handwriting PRIVATE msime-ibus-host)
add_executable(msime-linux-voice src/voice/voice_request.cpp)
target_compile_options(msime-linux-voice PRIVATE -Wall -Wextra -Werror)
target_link_libraries(msime-linux-voice PRIVATE msime-ibus-host)
# On-device recognition for the `local` voice provider: the voice service spawns this helper (scripts/msime_voice_local.py) and speaks its JSON-lines protocol, so the model never loads into the service or the input method. It opens the sherpa-onnx runtime with dlopen from its own directory, so it builds without one; MSIME_VOICE_RUNTIME_DIR below installs the runtime beside it. Compiled from the shared sources directly because shared/voice/CMakeLists.txt also builds the cloud providers and everything they link.
find_package(Threads REQUIRED)
set(MSIME_SHARED_VOICE_DIR "${CMAKE_CURRENT_SOURCE_DIR}/../../shared/voice")
add_executable(msime-voice-local "${MSIME_SHARED_VOICE_DIR}/LocalAsrHelper.cpp" "${MSIME_SHARED_VOICE_DIR}/LocalAsr.cpp")
target_compile_options(msime-voice-local PRIVATE -Wall -Wextra -Werror)
target_include_directories(msime-voice-local PRIVATE "${MSIME_SHARED_VOICE_DIR}" "${MSIME_SHARED_VOICE_DIR}/third_party")
target_link_libraries(msime-voice-local PRIVATE nlohmann_json::nlohmann_json Threads::Threads ${CMAKE_DL_LIBS})
add_executable(msime-linux-emoji src/providers/emoji_request.cpp)
target_compile_options(msime-linux-emoji PRIVATE -Wall -Wextra -Werror)
target_link_libraries(msime-linux-emoji PRIVATE msime-ibus-host)
add_executable(ibus-engine-smoke tests/runtime/engine_smoke.cpp)
target_compile_options(ibus-engine-smoke PRIVATE -Wall -Wextra -Werror)
target_link_libraries(ibus-engine-smoke PRIVATE msime-ibus-host)
if(BUILD_TESTING AND MSIME_ENGINE_RESOURCES)
  add_test(NAME ibus-page-number-visibility
    COMMAND ibus-engine-smoke "${MSIME_ENGINE_RESOURCES}" --page-number)
  set_tests_properties(ibus-page-number-visibility PROPERTIES TIMEOUT 60)
endif()
add_executable(linux-navigation-bindings-test tests/input/navigation_bindings.cpp)
target_compile_options(linux-navigation-bindings-test PRIVATE -Wall -Wextra -Werror)
target_include_directories(linux-navigation-bindings-test PRIVATE src ../../crates/host-api/include)
target_link_libraries(linux-navigation-bindings-test PRIVATE PkgConfig::IBUS)
add_test(NAME linux-navigation-bindings COMMAND linux-navigation-bindings-test)
add_executable(linux-spelling-symbols-test tests/input/spelling_symbols.cpp)
target_compile_options(linux-spelling-symbols-test PRIVATE -Wall -Wextra -Werror -UNDEBUG)
target_include_directories(linux-spelling-symbols-test PRIVATE src)
target_link_libraries(linux-spelling-symbols-test PRIVATE nlohmann_json::nlohmann_json)
add_test(NAME linux-spelling-symbols COMMAND linux-spelling-symbols-test)
add_executable(linux-korean-hanja-test tests/input/korean_hanja.cpp)
target_compile_options(linux-korean-hanja-test PRIVATE -Wall -Wextra -Werror -UNDEBUG)
target_include_directories(linux-korean-hanja-test PRIVATE src)
target_link_libraries(linux-korean-hanja-test PRIVATE nlohmann_json::nlohmann_json PkgConfig::IBUS)
add_test(NAME linux-korean-hanja COMMAND linux-korean-hanja-test)
add_executable(linux-key-sound-test tests/input/key_sound.cpp)
target_compile_options(linux-key-sound-test PRIVATE -Wall -Wextra -Werror -UNDEBUG)
target_include_directories(linux-key-sound-test PRIVATE src)
add_test(NAME linux-key-sound COMMAND linux-key-sound-test)
add_executable(linux-client-input-mode-memory-test tests/input/client_input_mode_memory.cpp)
target_compile_options(linux-client-input-mode-memory-test PRIVATE -Wall -Wextra -Werror)
target_include_directories(linux-client-input-mode-memory-test PRIVATE src)
add_test(NAME linux-client-input-mode-memory COMMAND linux-client-input-mode-memory-test)
add_executable(linux-shuangpin-profile-names-test tests/input/shuangpin_profile_names.cpp)
target_compile_options(linux-shuangpin-profile-names-test PRIVATE -Wall -Wextra -Werror)
target_include_directories(linux-shuangpin-profile-names-test PRIVATE src)
add_test(NAME linux-shuangpin-profile-names COMMAND linux-shuangpin-profile-names-test)
add_executable(linux-candidate-palette-test tests/candidate/candidate_palette.cpp)
target_compile_options(linux-candidate-palette-test PRIVATE -Wall -Wextra -Werror)
target_include_directories(linux-candidate-palette-test PRIVATE src)
target_link_libraries(linux-candidate-palette-test PRIVATE nlohmann_json::nlohmann_json)
add_test(NAME linux-candidate-palette COMMAND linux-candidate-palette-test)
add_executable(linux-candidate-fcitx-theme-test tests/candidate/candidate_fcitx_theme.cpp)
target_compile_options(linux-candidate-fcitx-theme-test PRIVATE -Wall -Wextra -Werror)
target_include_directories(linux-candidate-fcitx-theme-test PRIVATE src)
target_link_libraries(linux-candidate-fcitx-theme-test PRIVATE nlohmann_json::nlohmann_json)
add_test(NAME linux-candidate-fcitx-theme COMMAND linux-candidate-fcitx-theme-test)
add_executable(linux-candidate-font-policy-test tests/candidate/candidate_font_policy.cpp)
target_compile_options(linux-candidate-font-policy-test PRIVATE -Wall -Wextra -Werror)
target_include_directories(linux-candidate-font-policy-test PRIVATE src)
target_link_libraries(linux-candidate-font-policy-test PRIVATE nlohmann_json::nlohmann_json)
add_test(NAME linux-candidate-font-policy COMMAND linux-candidate-font-policy-test)
add_executable(linux-candidate-wheel-paging-test tests/candidate/candidate_wheel_paging.cpp)
target_compile_options(linux-candidate-wheel-paging-test PRIVATE -Wall -Wextra -Werror)
target_include_directories(linux-candidate-wheel-paging-test PRIVATE src)
target_link_libraries(linux-candidate-wheel-paging-test PRIVATE nlohmann_json::nlohmann_json)
add_test(NAME linux-candidate-wheel-paging COMMAND linux-candidate-wheel-paging-test)
add_executable(linux-panel-restore-record-test tests/candidate/panel_restore_record.cpp)
target_compile_options(linux-panel-restore-record-test PRIVATE -Wall -Wextra -Werror -UNDEBUG)
target_include_directories(linux-panel-restore-record-test PRIVATE src)
target_link_libraries(linux-panel-restore-record-test PRIVATE nlohmann_json::nlohmann_json)
add_test(NAME linux-panel-restore-record COMMAND linux-panel-restore-record-test)
add_executable(linux-candidate-action-policy-test tests/candidate/candidate_action_policy.cpp)
target_compile_options(linux-candidate-action-policy-test PRIVATE -Wall -Wextra -Werror)
target_include_directories(linux-candidate-action-policy-test PRIVATE src)
add_test(NAME linux-candidate-action-policy COMMAND linux-candidate-action-policy-test)
add_executable(linux-candidate-local-mode-labels-test tests/candidate/candidate_local_mode_labels.cpp)
target_compile_options(linux-candidate-local-mode-labels-test PRIVATE -Wall -Wextra -Werror)
target_include_directories(linux-candidate-local-mode-labels-test PRIVATE src)
add_test(NAME linux-candidate-local-mode-labels COMMAND linux-candidate-local-mode-labels-test)
add_executable(linux-candidate-panel-status-test tests/candidate/candidate_panel_status.cpp)
target_compile_options(linux-candidate-panel-status-test PRIVATE -Wall -Wextra -Werror)
target_include_directories(linux-candidate-panel-status-test PRIVATE src)
add_test(NAME linux-candidate-panel-status COMMAND linux-candidate-panel-status-test)
add_executable(linux-input-status-test tests/core/input_status.cpp)
target_compile_options(linux-input-status-test PRIVATE -Wall -Wextra -Werror)
target_include_directories(linux-input-status-test PRIVATE src)
target_link_libraries(linux-input-status-test PRIVATE nlohmann_json::nlohmann_json)
add_test(NAME linux-input-status COMMAND linux-input-status-test)
add_executable(linux-candidate-translation-policy-test tests/candidate/candidate_translation_policy.cpp)
target_compile_options(linux-candidate-translation-policy-test PRIVATE -Wall -Wextra -Werror)
target_include_directories(linux-candidate-translation-policy-test PRIVATE src)
add_test(NAME linux-candidate-translation-policy COMMAND linux-candidate-translation-policy-test)
add_executable(linux-paired-punctuation-test tests/input/paired_punctuation.cpp)
target_compile_options(linux-paired-punctuation-test PRIVATE -Wall -Wextra -Werror)
target_include_directories(linux-paired-punctuation-test PRIVATE src)
add_test(NAME linux-paired-punctuation COMMAND linux-paired-punctuation-test)
add_executable(linux-dictionary-quiesce-lease-test tests/core/dictionary_quiesce_lease.cpp)
target_compile_options(linux-dictionary-quiesce-lease-test PRIVATE -Wall -Wextra -Werror)
target_include_directories(linux-dictionary-quiesce-lease-test PRIVATE src)
add_test(NAME linux-dictionary-quiesce-lease COMMAND linux-dictionary-quiesce-lease-test)
add_executable(common-dictionary-quiesce-lease-test ../common/tests/dictionary_quiesce_lease.cpp)
target_compile_options(common-dictionary-quiesce-lease-test PRIVATE -Wall -Wextra -Werror)
add_test(NAME common-dictionary-quiesce-lease COMMAND common-dictionary-quiesce-lease-test)
add_executable(common-telemetry-store-test ../common/tests/telemetry_store.cpp)
target_compile_options(common-telemetry-store-test PRIVATE -Wall -Wextra -Werror -UNDEBUG)
target_include_directories(common-telemetry-store-test PRIVATE ../common)
target_link_libraries(common-telemetry-store-test PRIVATE CURL::libcurl Threads::Threads)
add_test(NAME common-telemetry-store COMMAND common-telemetry-store-test)
add_executable(linux-input-mode-indicator-test tests/core/input_mode_indicator.cpp)
target_compile_options(linux-input-mode-indicator-test PRIVATE -Wall -Wextra -Werror)
target_include_directories(linux-input-mode-indicator-test PRIVATE src)
add_test(NAME linux-input-mode-indicator COMMAND linux-input-mode-indicator-test)
add_executable(linux-voice-provider-options-test tests/voice/voice_provider_options.cpp)
target_compile_options(linux-voice-provider-options-test PRIVATE -Wall -Wextra -Werror)
target_include_directories(linux-voice-provider-options-test PRIVATE src)
target_link_libraries(linux-voice-provider-options-test PRIVATE nlohmann_json::nlohmann_json)
add_test(NAME linux-voice-provider-options COMMAND linux-voice-provider-options-test)
add_executable(linux-prepare-paths-test tests/core/prepare_paths.cpp)
target_compile_options(linux-prepare-paths-test PRIVATE -Wall -Wextra -Werror)
target_include_directories(linux-prepare-paths-test PRIVATE src)
add_test(NAME linux-prepare-paths COMMAND linux-prepare-paths-test)
add_executable(linux-key-router-adapter-test tests/input/key_router_adapter.cpp)
target_compile_options(linux-key-router-adapter-test PRIVATE -Wall -Wextra -Werror)
target_include_directories(linux-key-router-adapter-test PRIVATE src ../../crates/host-api/include)
add_test(NAME linux-key-router-adapter COMMAND linux-key-router-adapter-test)
add_executable(linux-backspace-hold-policy-test tests/input/backspace_hold_policy.cpp)
target_compile_options(linux-backspace-hold-policy-test PRIVATE -Wall -Wextra -Werror)
target_include_directories(linux-backspace-hold-policy-test PRIVATE src)
add_test(NAME linux-backspace-hold-policy COMMAND linux-backspace-hold-policy-test)
add_executable(linux-diagnostic-log-test tests/core/diagnostic_log.cpp)
target_compile_options(linux-diagnostic-log-test PRIVATE -Wall -Wextra -Werror)
target_include_directories(linux-diagnostic-log-test PRIVATE src)
target_sources(linux-diagnostic-log-test PRIVATE src/system/DiagnosticLog.cpp)
add_test(NAME linux-diagnostic-log COMMAND linux-diagnostic-log-test)
# The composition both front ends draw while a phrase is being assembled: the piece already picked
# leads the reading, and the caret has to move past it in the unit each toolkit counts in - bytes
# for fcitx5, Unicode scalars for IBus.
# Space converts and Enter takes what is on screen: the decision both front ends share, and the
# reason the Engine's raw-input command cannot be what Enter sends in Japanese.
add_executable(linux-japanese-conversion-test tests/core/japanese_conversion.cpp)
target_compile_options(linux-japanese-conversion-test PRIVATE -Wall -Wextra -Werror -UNDEBUG)
target_include_directories(linux-japanese-conversion-test PRIVATE src)
add_test(NAME linux-japanese-conversion COMMAND linux-japanese-conversion-test)
# One table for the two menus over one setting, because they disagreed: 首右 had been written as
# 搜狗, which names a different company's input method.
add_executable(linux-helpcode-schema-names-test tests/core/helpcode_schema_names.cpp)
target_compile_options(linux-helpcode-schema-names-test PRIVATE -Wall -Wextra -Werror -UNDEBUG)
target_include_directories(linux-helpcode-schema-names-test PRIVATE src)
add_test(NAME linux-helpcode-schema-names COMMAND linux-helpcode-schema-names-test)
add_executable(linux-phrase-preedit-test tests/core/phrase_preedit.cpp)
target_compile_options(linux-phrase-preedit-test PRIVATE -Wall -Wextra -Werror -UNDEBUG)
target_include_directories(linux-phrase-preedit-test PRIVATE src)
add_test(NAME linux-phrase-preedit COMMAND linux-phrase-preedit-test)
add_executable(linux-replaced-program-test tests/core/replaced_program.cpp)
target_compile_options(linux-replaced-program-test PRIVATE -Wall -Wextra -Werror -UNDEBUG)
target_include_directories(linux-replaced-program-test PRIVATE src)
add_test(NAME linux-replaced-program COMMAND linux-replaced-program-test)
add_executable(linux-smart-punctuation-space-test tests/core/smart_punctuation_space.cpp)
target_compile_options(linux-smart-punctuation-space-test PRIVATE -Wall -Wextra -Werror)
target_include_directories(linux-smart-punctuation-space-test PRIVATE src)
add_test(NAME linux-smart-punctuation-space COMMAND linux-smart-punctuation-space-test)
add_test(NAME linux-setup-resolution
         COMMAND "${Python3_EXECUTABLE}" "${CMAKE_CURRENT_SOURCE_DIR}/tests/core/setup_resolution.py")
add_test(NAME linux-setup-update
         COMMAND "${Python3_EXECUTABLE}" "${CMAKE_CURRENT_SOURCE_DIR}/tests/core/setup_update.py"
                 $<TARGET_FILE:msime-linux-prepare>)
add_test(NAME linux-setup-registration
         COMMAND "${Python3_EXECUTABLE}" "${CMAKE_CURRENT_SOURCE_DIR}/tests/core/setup_registration.py")
add_test(NAME linux-setup-omarchy
         COMMAND "${Python3_EXECUTABLE}" "${CMAKE_CURRENT_SOURCE_DIR}/tests/core/setup_omarchy.py")
add_test(NAME linux-setup-panel-restore
         COMMAND "${Python3_EXECUTABLE}" "${CMAKE_CURRENT_SOURCE_DIR}/tests/core/setup_panel_restore.py")
add_test(NAME linux-first-run-guide
         COMMAND "${Python3_EXECUTABLE}" "${CMAKE_CURRENT_SOURCE_DIR}/tests/core/first_run_guide.py")
add_test(NAME linux-settings-launcher-contract
         COMMAND "${Python3_EXECUTABLE}" "${CMAKE_CURRENT_SOURCE_DIR}/tests/core/settings_launcher_contract.py")
add_test(NAME linux-deb-maintainer-scripts
         COMMAND "${Python3_EXECUTABLE}" "${CMAKE_CURRENT_SOURCE_DIR}/tests/core/deb_maintainer_scripts.py"
                 "${CMAKE_CURRENT_BINARY_DIR}/debian" "${CMAKE_CURRENT_BINARY_DIR}/uninstall.cmake" "${MSIME_USER_UNIT_LIST}"
                 "${CMAKE_INSTALL_FULL_BINDIR}/msime-linux-setup")
add_test(NAME linux-uninstall-unregister
         COMMAND "${Python3_EXECUTABLE}" "${CMAKE_CURRENT_SOURCE_DIR}/tests/core/uninstall_unregister.py"
                 "${CMAKE_COMMAND}" "${CMAKE_CURRENT_BINARY_DIR}/uninstall.cmake")
add_test(NAME linux-clipboard-autostart
         COMMAND "${Python3_EXECUTABLE}" "${CMAKE_CURRENT_SOURCE_DIR}/tests/core/clipboard_autostart.py"
                 "${CMAKE_COMMAND}" "${CMAKE_CURRENT_BINARY_DIR}" "${MSIME_XDG_AUTOSTART_DIR}" "${CMAKE_CURRENT_BINARY_DIR}/uninstall.cmake")
add_test(NAME linux-ibus-launcher-supervisor
         COMMAND "${Python3_EXECUTABLE}" "${CMAKE_CURRENT_SOURCE_DIR}/tests/core/ibus_launcher_supervisor.py")
add_executable(linux-ibus-registration-bus tests/core/ibus_registration_bus.cpp)
target_compile_options(linux-ibus-registration-bus PRIVATE -Wall -Wextra -Werror)
target_link_libraries(linux-ibus-registration-bus PRIVATE PkgConfig::IBUS)
add_test(NAME linux-ibus-startup-telemetry
         COMMAND "${Python3_EXECUTABLE}" "${CMAKE_CURRENT_SOURCE_DIR}/tests/core/ibus_startup_telemetry.py"
                 $<TARGET_FILE:msime-linux-ibus> $<TARGET_FILE:linux-ibus-registration-bus> "${MSIME_LINUX_VERSION}")
set_tests_properties(linux-ibus-startup-telemetry PROPERTIES TIMEOUT 60 SKIP_RETURN_CODE 77)
add_test(NAME linux-voice-provider-degraded-start
         COMMAND "${Python3_EXECUTABLE}" "${CMAKE_CURRENT_SOURCE_DIR}/tests/voice/provider_degraded_start.py")
add_test(NAME linux-voice-local-stream
         COMMAND "${Python3_EXECUTABLE}" "${CMAKE_CURRENT_SOURCE_DIR}/tests/voice/local_stream.py")
add_test(NAME linux-voice-provider-local
         COMMAND "${Python3_EXECUTABLE}" "${CMAKE_CURRENT_SOURCE_DIR}/tests/voice/provider_local.py"
                 $<TARGET_FILE:msime-voice-local>)
set_tests_properties(linux-voice-local-stream linux-voice-provider-local PROPERTIES TIMEOUT 120)
add_test(NAME linux-voice-command-bounds
         COMMAND "${Python3_EXECUTABLE}" "${CMAKE_CURRENT_SOURCE_DIR}/tests/voice/voice_command_bounds.py")
add_test(NAME linux-voice-capture-device
         COMMAND "${Python3_EXECUTABLE}" "${CMAKE_CURRENT_SOURCE_DIR}/tests/voice/capture_device_contract.py")
add_test(NAME linux-clipboard-tool-parity
         COMMAND "${Python3_EXECUTABLE}" "${CMAKE_CURRENT_SOURCE_DIR}/tests/clipboard/clipboard_tool_parity.py"
                 $<TARGET_FILE:msime-linux-clipboard>)
add_executable(linux-first-run-guidance-test tests/core/first_run_guidance.cpp)
target_compile_options(linux-first-run-guidance-test PRIVATE -Wall -Wextra -Werror -UNDEBUG)
target_include_directories(linux-first-run-guidance-test PRIVATE src)
add_test(NAME linux-first-run-guidance COMMAND linux-first-run-guidance-test)
add_executable(linux-candidate-skin-catalog-test tests/core/candidate_skin_catalog.cpp)
target_compile_options(linux-candidate-skin-catalog-test PRIVATE -Wall -Wextra -Werror)
target_include_directories(linux-candidate-skin-catalog-test PRIVATE src)
target_link_libraries(linux-candidate-skin-catalog-test PRIVATE nlohmann_json::nlohmann_json)
add_test(NAME linux-candidate-skin-catalog COMMAND linux-candidate-skin-catalog-test)
add_executable(linux-typing-statistics-test tests/core/typing_statistics.cpp)
target_compile_options(linux-typing-statistics-test PRIVATE -Wall -Wextra -Werror)
target_include_directories(linux-typing-statistics-test PRIVATE src ../../crates/host-api/include)
target_link_libraries(linux-typing-statistics-test PRIVATE "${MSIME_HOST_LIBRARY}")
add_test(NAME linux-typing-statistics COMMAND linux-typing-statistics-test)
add_executable(linux-panel-input-channel-test tests/core/panel_input_channel.cpp)
target_compile_options(linux-panel-input-channel-test PRIVATE -Wall -Wextra -Werror)
target_include_directories(linux-panel-input-channel-test PRIVATE src src/system)
target_link_libraries(linux-panel-input-channel-test PRIVATE nlohmann_json::nlohmann_json)
add_test(NAME linux-panel-input-channel COMMAND linux-panel-input-channel-test)
add_executable(linux-traditional-output-test tests/input/traditional_output.cpp src/system/ChineseTextConversion.cpp)
target_compile_options(linux-traditional-output-test PRIVATE -Wall -Wextra -Werror)
target_include_directories(linux-traditional-output-test PRIVATE src ../../crates/host-api/include)
target_link_libraries(linux-traditional-output-test PRIVATE "${MSIME_HOST_LIBRARY}")
add_test(NAME linux-traditional-output COMMAND linux-traditional-output-test)
if(MSIME_LINUX_VOICE)
 add_executable(msime-voice-worker-test tests/voice/voice_worker.cpp)
 target_compile_options(msime-voice-worker-test PRIVATE -Wall -Wextra -Werror)
 target_include_directories(msime-voice-worker-test PRIVATE src)
 add_test(NAME linux-voice-worker COMMAND msime-voice-worker-test)
endif()
configure_file(data/msime-linux.xml.in ${CMAKE_CURRENT_BINARY_DIR}/msime-linux.xml @ONLY)
install(TARGETS msime-linux-ibus msime-linux-dictionary msime-linux-cloud-dictionary msime-linux-cloud-clipboard msime-linux-online msime-linux-translation msime-linux-clipboard
                msime-linux-handwriting msime-linux-voice msime-linux-emoji msime-linux-clipboard-capture msime-linux-prepare
                RUNTIME DESTINATION "${CMAKE_INSTALL_BINDIR}")
if(MSIME_LINUX_VOICE)
 install(TARGETS msime-voice-worker-test RUNTIME DESTINATION "${CMAKE_INSTALL_BINDIR}")
endif()
install(PROGRAMS scripts/msime-linux-ibus-launcher scripts/msime-linux-first-run-guide scripts/msime-linux-online-provider scripts/msime-linux-voice-provider scripts/msime-linux-provider-session DESTINATION "${CMAKE_INSTALL_BINDIR}")
install(PROGRAMS scripts/msime-linux-clipboard-monitor DESTINATION "${CMAKE_INSTALL_BINDIR}")
# 安装后的首次配置入口。词库与状态目录此前没有任何产品侧的路径：词库要么在配置阶段随包装好，要么只能在仓库检出里跑一个 Cargo 示例去取，于是只拿到安装包的用户凑不齐词库，而没有词库一个候选都出不来。词库锁无条件随装，它是这个入口据以校验和取回的依据——它此前只在 MSIME_ENGINE_RESOURCES 存在时才安装，也就是最需要它的那种安装里反而没有。
install(PROGRAMS scripts/msime-linux-setup DESTINATION "${CMAKE_INSTALL_BINDIR}")
if(NOT MSIME_ENGINE_RESOURCES)
  install(FILES "${CMAKE_CURRENT_SOURCE_DIR}/../../resources/desktop-dictionary.lock.json"
          DESTINATION "${CMAKE_INSTALL_DATADIR}/msime-client")
endif()
set(MSIME_SYSTEMD_USER_UNIT_DIR "${CMAKE_INSTALL_DATADIR}/systemd/user" CACHE PATH "systemd user unit installation directory")
foreach(MSIME_PROVIDER online voice clipboard)
  configure_file(data/msime-linux-${MSIME_PROVIDER}.service.in
                 ${CMAKE_CURRENT_BINARY_DIR}/msime-linux-${MSIME_PROVIDER}.service @ONLY)
  install(FILES ${CMAKE_CURRENT_BINARY_DIR}/msime-linux-${MSIME_PROVIDER}.service
          DESTINATION "${MSIME_SYSTEMD_USER_UNIT_DIR}")
endforeach()
# Starts the clipboard service at login on sessions without graphical-session.target, for users who enabled it; see the comment in the file. Its own component lets the test install just this file.
install(FILES data/msime-linux-clipboard.desktop DESTINATION "${MSIME_XDG_AUTOSTART_DIR}" COMPONENT clipboard-autostart)
# Socket units keep the provider paths present from login, so the input method and the panels find the services the way the Windows server is simply there; systemd starts a provider on its first request.
install(FILES data/msime-linux-online.socket data/msime-linux-voice.socket
        DESTINATION "${MSIME_SYSTEMD_USER_UNIT_DIR}")
install(FILES scripts/msime_voice_doubao.py scripts/msime_voice_local.py scripts/msime_provider_runtime.py DESTINATION "${CMAKE_INSTALL_BINDIR}")
# The helper and the sherpa-onnx runtime share the host library's private directory: the helper finds the runtime in its own directory, and libsherpa-onnx-c-api.so finds libonnxruntime.so through its $ORIGIN RPATH.
install(TARGETS msime-voice-local RUNTIME DESTINATION "${CMAKE_INSTALL_LIBDIR}/msime-client")
set(MSIME_VOICE_RUNTIME_DIR "" CACHE PATH "sherpa-onnx runtime for on-device recognition, as unpacked by scripts/fetch_voice_runtime.py --platform linux-<arch>")
set(MSIME_VOICE_RUNTIME_LIBRARIES libsherpa-onnx-c-api.so libonnxruntime.so)
if(MSIME_VOICE_RUNTIME_DIR)
  foreach(MSIME_VOICE_RUNTIME_LIBRARY IN LISTS MSIME_VOICE_RUNTIME_LIBRARIES)
    if(NOT EXISTS "${MSIME_VOICE_RUNTIME_DIR}/${MSIME_VOICE_RUNTIME_LIBRARY}")
      message(FATAL_ERROR "MSIME_VOICE_RUNTIME_DIR has no ${MSIME_VOICE_RUNTIME_LIBRARY}; run scripts/fetch_voice_runtime.py --platform linux-<arch> --out <dir>")
    endif()
    install(FILES "${MSIME_VOICE_RUNTIME_DIR}/${MSIME_VOICE_RUNTIME_LIBRARY}" DESTINATION "${CMAKE_INSTALL_LIBDIR}/msime-client")
  endforeach()
elseif(MSIME_ENABLE_PACKAGING)
  # A package without the runtime would offer on-device recognition that fails at the first dictation.
  message(FATAL_ERROR "Packaging requires MSIME_VOICE_RUNTIME_DIR; fetch it with scripts/fetch_voice_runtime.py (package-container.sh does)")
endif()
install(FILES scripts/requirements-voice.txt DESTINATION "${CMAKE_INSTALL_DATADIR}/msime-client")
file(RELATIVE_PATH MSIME_VOICE_CUE_RELATIVE "${CMAKE_INSTALL_FULL_BINDIR}"
     "${CMAKE_INSTALL_FULL_DATADIR}/msime-client/voice")
file(RELATIVE_PATH MSIME_VOICE_LOCAL_HELPER_RELATIVE "${CMAKE_INSTALL_FULL_BINDIR}"
     "${CMAKE_INSTALL_FULL_LIBDIR}/msime-client/msime-voice-local")
get_filename_component(MSIME_HOST_LIBRARY_NAME "${MSIME_HOST_LIBRARY}" NAME)
file(RELATIVE_PATH MSIME_HOST_LIBRARY_RELATIVE "${CMAKE_INSTALL_FULL_BINDIR}"
     "${CMAKE_INSTALL_FULL_LIBDIR}/msime-client/${MSIME_HOST_LIBRARY_NAME}")
configure_file(data/msime_voice_paths.py.in msime_voice_paths.py @ONLY)
install(FILES "${CMAKE_CURRENT_BINARY_DIR}/msime_voice_paths.py" DESTINATION "${CMAKE_INSTALL_BINDIR}")
install(FILES data/voice/start.pcm data/voice/end.pcm data/voice/SOURCE.md
        DESTINATION "${CMAKE_INSTALL_DATADIR}/msime-client/voice")
# The zinnia model `msime-linux-handwriting --local` recognises with, and its LGPL-2.1 license. At 26.8 MB it is not committed: scripts/fetch_handwriting_model.py downloads what resources/handwriting-model.lock.json pins into target/handwriting-model, which is picked up when present, and every file is checked against the lock here. Packaging requires it, because a package without it would offer an offline recognizer that fails on the first stroke; a development install without it only leaves --local reporting that no model is installed.
if(NOT MSIME_HANDWRITING_MODEL_DIR AND
   EXISTS "${CMAKE_CURRENT_SOURCE_DIR}/../../target/handwriting-model/handwriting-zh_CN.model")
  get_filename_component(MSIME_HANDWRITING_MODEL_DIR
                         "${CMAKE_CURRENT_SOURCE_DIR}/../../target/handwriting-model" ABSOLUTE)
  message(STATUS "Using handwriting model staging directory: ${MSIME_HANDWRITING_MODEL_DIR}")
endif()
if(MSIME_HANDWRITING_MODEL_DIR)
  if(NOT IS_DIRECTORY "${MSIME_HANDWRITING_MODEL_DIR}")
    message(FATAL_ERROR "MSIME_HANDWRITING_MODEL_DIR is not a directory: ${MSIME_HANDWRITING_MODEL_DIR}")
  endif()
  set(MSIME_HANDWRITING_LOCK_FILE "${CMAKE_CURRENT_SOURCE_DIR}/../../resources/handwriting-model.lock.json")
  set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS "${MSIME_HANDWRITING_LOCK_FILE}")
  file(READ "${MSIME_HANDWRITING_LOCK_FILE}" MSIME_HANDWRITING_LOCK)
  string(JSON MSIME_HANDWRITING_COUNT LENGTH "${MSIME_HANDWRITING_LOCK}" artifacts)
  math(EXPR MSIME_HANDWRITING_LAST "${MSIME_HANDWRITING_COUNT} - 1")
  foreach(MSIME_HANDWRITING_INDEX RANGE 0 ${MSIME_HANDWRITING_LAST})
    string(JSON MSIME_HANDWRITING_NAME GET "${MSIME_HANDWRITING_LOCK}"
           artifacts ${MSIME_HANDWRITING_INDEX} name)
    set(MSIME_HANDWRITING_FILE "${MSIME_HANDWRITING_MODEL_DIR}/${MSIME_HANDWRITING_NAME}")
    if(NOT EXISTS "${MSIME_HANDWRITING_FILE}" OR
       IS_DIRECTORY "${MSIME_HANDWRITING_FILE}" OR
       IS_SYMLINK "${MSIME_HANDWRITING_FILE}")
      message(FATAL_ERROR "Missing ${MSIME_HANDWRITING_NAME} in ${MSIME_HANDWRITING_MODEL_DIR}; run scripts/fetch_handwriting_model.py")
    endif()
    string(JSON MSIME_HANDWRITING_SIZE GET "${MSIME_HANDWRITING_LOCK}"
           artifacts ${MSIME_HANDWRITING_INDEX} size)
    string(JSON MSIME_HANDWRITING_SHA256 GET "${MSIME_HANDWRITING_LOCK}"
           artifacts ${MSIME_HANDWRITING_INDEX} sha256)
    file(SIZE "${MSIME_HANDWRITING_FILE}" MSIME_HANDWRITING_ACTUAL_SIZE)
    file(SHA256 "${MSIME_HANDWRITING_FILE}" MSIME_HANDWRITING_ACTUAL_SHA256)
    if(NOT MSIME_HANDWRITING_ACTUAL_SIZE STREQUAL MSIME_HANDWRITING_SIZE OR
       NOT MSIME_HANDWRITING_ACTUAL_SHA256 STREQUAL MSIME_HANDWRITING_SHA256)
      message(FATAL_ERROR
        "${MSIME_HANDWRITING_NAME} in ${MSIME_HANDWRITING_MODEL_DIR} does not match resources/handwriting-model.lock.json")
    endif()
    install(FILES "${MSIME_HANDWRITING_FILE}"
            DESTINATION "${CMAKE_INSTALL_DATADIR}/msime-client/handwriting")
  endforeach()
  install(FILES "${MSIME_HANDWRITING_LOCK_FILE}" DESTINATION "${CMAKE_INSTALL_DATADIR}/msime-client")
elseif(MSIME_ENABLE_PACKAGING)
  message(FATAL_ERROR "Packaging requires the handwriting model; fetch it with scripts/fetch_handwriting_model.py (package-container.sh does)")
endif()
if(MSIME_EMOJI_RESOURCES)
  if(NOT IS_DIRECTORY "${MSIME_EMOJI_RESOURCES}")
    message(FATAL_ERROR "MSIME_EMOJI_RESOURCES is not a directory: ${MSIME_EMOJI_RESOURCES}")
  endif()
  install(DIRECTORY "${MSIME_EMOJI_RESOURCES}/"
          DESTINATION "${CMAKE_INSTALL_DATADIR}/msime-client/emoji")
endif()
if(MSIME_ENGINE_RESOURCES)
  if(NOT IS_DIRECTORY "${MSIME_ENGINE_RESOURCES}")
    message(FATAL_ERROR "MSIME_ENGINE_RESOURCES is not a directory: ${MSIME_ENGINE_RESOURCES}")
  endif()
  file(READ "${CMAKE_CURRENT_SOURCE_DIR}/../../resources/desktop-dictionary.lock.json"
       MSIME_ENGINE_RESOURCE_LOCK)
  string(JSON MSIME_ENGINE_RESOURCE_COUNT LENGTH "${MSIME_ENGINE_RESOURCE_LOCK}" artifacts)
  math(EXPR MSIME_ENGINE_RESOURCE_LAST "${MSIME_ENGINE_RESOURCE_COUNT} - 1")
  foreach(MSIME_ENGINE_RESOURCE_INDEX RANGE 0 ${MSIME_ENGINE_RESOURCE_LAST})
    string(JSON MSIME_ENGINE_RESOURCE_NAME GET "${MSIME_ENGINE_RESOURCE_LOCK}"
           artifacts ${MSIME_ENGINE_RESOURCE_INDEX} name)
    set(MSIME_ENGINE_RESOURCE_FILE "${MSIME_ENGINE_RESOURCES}/${MSIME_ENGINE_RESOURCE_NAME}")
    if(NOT EXISTS "${MSIME_ENGINE_RESOURCE_FILE}" OR
       IS_DIRECTORY "${MSIME_ENGINE_RESOURCE_FILE}" OR
       IS_SYMLINK "${MSIME_ENGINE_RESOURCE_FILE}")
      message(FATAL_ERROR "Missing Engine resource ${MSIME_ENGINE_RESOURCE_NAME} in ${MSIME_ENGINE_RESOURCES}")
    endif()
    string(JSON MSIME_ENGINE_RESOURCE_SIZE GET "${MSIME_ENGINE_RESOURCE_LOCK}"
           artifacts ${MSIME_ENGINE_RESOURCE_INDEX} size)
    string(JSON MSIME_ENGINE_RESOURCE_SHA256 GET "${MSIME_ENGINE_RESOURCE_LOCK}"
           artifacts ${MSIME_ENGINE_RESOURCE_INDEX} sha256)
    file(SIZE "${MSIME_ENGINE_RESOURCE_FILE}" MSIME_ENGINE_RESOURCE_ACTUAL_SIZE)
    file(SHA256 "${MSIME_ENGINE_RESOURCE_FILE}" MSIME_ENGINE_RESOURCE_ACTUAL_SHA256)
    if(NOT MSIME_ENGINE_RESOURCE_ACTUAL_SIZE STREQUAL MSIME_ENGINE_RESOURCE_SIZE OR
       NOT MSIME_ENGINE_RESOURCE_ACTUAL_SHA256 STREQUAL MSIME_ENGINE_RESOURCE_SHA256)
      message(FATAL_ERROR
        "Engine resource ${MSIME_ENGINE_RESOURCE_NAME} does not match the reviewed lock")
    endif()
    install(FILES "${MSIME_ENGINE_RESOURCE_FILE}"
            DESTINATION "${CMAKE_INSTALL_DATADIR}/msime-client/resources")
  endforeach()
  # Earlier releases installed dict_pinyin.dat, the C++ Engine's system dictionary, into this directory. The lock no longer pins it. The hosts delete it themselves when they verify a directory they can write, but an installed prefix is normally root-owned, where the deletion fails and they refuse the whole directory instead, so an install over such a prefix removes it, as the Windows installer does in DataDir on upgrade. Packages need no help: the package manager drops what the new package no longer carries.
  install(CODE "file(REMOVE \"\$ENV{DESTDIR}${CMAKE_INSTALL_FULL_DATADIR}/msime-client/resources/dict_pinyin.dat\")")
  # The helpcode tables are not part of the dictionary release; the repository carries them in resources/helpcodes, and the Engine reads them from helpcodes/ under the resource directory (crates/engine/src/assets.rs names the six files). The hosts' verification lets a real helpcodes/ directory through. Their notices are installed with the others below.
  install(FILES
            "${CMAKE_CURRENT_SOURCE_DIR}/../../resources/helpcodes/helpcode.txt"
            "${CMAKE_CURRENT_SOURCE_DIR}/../../resources/helpcodes/zrm_helpcode_big_unique.txt"
            "${CMAKE_CURRENT_SOURCE_DIR}/../../resources/helpcodes/shouyou2_0_helpcode.txt"
            "${CMAKE_CURRENT_SOURCE_DIR}/../../resources/helpcodes/shouyouplus_helpcode.txt"
            "${CMAKE_CURRENT_SOURCE_DIR}/../../resources/helpcodes/xiaohe_helpcode.txt"
            "${CMAKE_CURRENT_SOURCE_DIR}/../../resources/helpcodes/jiajia_helpcode.txt"
          DESTINATION "${CMAKE_INSTALL_DATADIR}/msime-client/resources/helpcodes")
  # Keep the reviewed lock beside the installed bundle as provenance metadata;
  # it is not copied into the runtime directory because the Engine verifier
  # requires that directory to contain exactly the pinned artifacts.
  install(FILES "${CMAKE_CURRENT_SOURCE_DIR}/../../resources/desktop-dictionary.lock.json"
          DESTINATION "${CMAKE_INSTALL_DATADIR}/msime-client")
endif()
# The settled-rerank model, installed as a sibling of the resource bundle.
#
# A sibling and not a member: the Engine verifier requires the resource directory to hold exactly
# the artifacts the dictionary lock pins, so an extra file there fails the check that proves a
# shipped dictionary is intact. `prepare_host_configuration` looks for it here and writes the path
# into the runtime options; without it the host behaves exactly as it does today.
#
# Optional because it is 25 MB and buys a desktop-only improvement — 49 points of top-1 on the
# harvested failure set, at a cost that keeps it off the keystroke path and on the settle timer.
# Fetch it with `scripts/fetch_neural_model.py`, which writes `target/neural-model` by default.
# The historical one-artifact `scripts/fetch_settled_model.py` output remains accepted below.
if(NOT MSIME_SETTLED_MODEL AND
   EXISTS "${CMAKE_CURRENT_SOURCE_DIR}/../../target/neural-model/sentence-model-desktop.safetensors")
  get_filename_component(MSIME_SETTLED_MODEL
                         "${CMAKE_CURRENT_SOURCE_DIR}/../../target/neural-model" ABSOLUTE)
  message(STATUS "Using neural model staging directory: ${MSIME_SETTLED_MODEL}")
endif()
if(MSIME_SETTLED_MODEL)
  if(NOT IS_DIRECTORY "${MSIME_SETTLED_MODEL}")
    message(FATAL_ERROR "MSIME_SETTLED_MODEL is not a directory: ${MSIME_SETTLED_MODEL}")
  endif()
  file(READ "${CMAKE_CURRENT_SOURCE_DIR}/../../resources/settled-model.lock.json"
       MSIME_SETTLED_LOCK)
  string(JSON MSIME_SETTLED_COUNT LENGTH "${MSIME_SETTLED_LOCK}" artifacts)
  math(EXPR MSIME_SETTLED_LAST "${MSIME_SETTLED_COUNT} - 1")
  foreach(MSIME_SETTLED_INDEX RANGE 0 ${MSIME_SETTLED_LAST})
    string(JSON MSIME_SETTLED_NAME GET "${MSIME_SETTLED_LOCK}"
           artifacts ${MSIME_SETTLED_INDEX} name)
    set(MSIME_SETTLED_FILE "${MSIME_SETTLED_MODEL}/${MSIME_SETTLED_NAME}")
    if(NOT EXISTS "${MSIME_SETTLED_FILE}" OR
       IS_DIRECTORY "${MSIME_SETTLED_FILE}" OR
       IS_SYMLINK "${MSIME_SETTLED_FILE}")
      message(FATAL_ERROR "Missing settled model ${MSIME_SETTLED_NAME} in ${MSIME_SETTLED_MODEL}")
    endif()
    string(JSON MSIME_SETTLED_SIZE GET "${MSIME_SETTLED_LOCK}"
           artifacts ${MSIME_SETTLED_INDEX} size)
    string(JSON MSIME_SETTLED_SHA256 GET "${MSIME_SETTLED_LOCK}"
           artifacts ${MSIME_SETTLED_INDEX} sha256)
    file(SIZE "${MSIME_SETTLED_FILE}" MSIME_SETTLED_ACTUAL_SIZE)
    file(SHA256 "${MSIME_SETTLED_FILE}" MSIME_SETTLED_ACTUAL_SHA256)
    if(NOT MSIME_SETTLED_ACTUAL_SIZE STREQUAL MSIME_SETTLED_SIZE OR
       NOT MSIME_SETTLED_ACTUAL_SHA256 STREQUAL MSIME_SETTLED_SHA256)
      message(FATAL_ERROR
        "Settled model ${MSIME_SETTLED_NAME} does not match the reviewed lock")
    endif()
    install(FILES "${MSIME_SETTLED_FILE}"
            DESTINATION "${CMAKE_INSTALL_DATADIR}/msime-client/settled-model")
  endforeach()
  install(FILES "${CMAKE_CURRENT_SOURCE_DIR}/../../resources/settled-model.lock.json"
          DESTINATION "${CMAKE_INSTALL_DATADIR}/msime-client")
  install(FILES "${CMAKE_CURRENT_SOURCE_DIR}/../../resources/neural-model.lock.json"
          DESTINATION "${CMAKE_INSTALL_DATADIR}/msime-client")
endif()
# Offline glosses for non-English translation targets, one zh-<lang>.db per language built by `scripts/build_offline_glosses.py`, installed as a sibling of the resource bundle for the same reason as the settled model; the Engine looks for them there. Optional: without them the candidate glosses stay English only. Their notice is required with them, since the databases are CC BY-SA and must travel with their attribution.
set(MSIME_OFFLINE_GLOSSES "" CACHE PATH "Offline gloss databases with their notice, e.g. target/offline-glosses")
if(MSIME_OFFLINE_GLOSSES)
  if(NOT EXISTS "${MSIME_OFFLINE_GLOSSES}/offline-glosses-NOTICE.txt")
    message(FATAL_ERROR "MSIME_OFFLINE_GLOSSES has no offline-glosses-NOTICE.txt: ${MSIME_OFFLINE_GLOSSES}")
  endif()
  file(GLOB MSIME_OFFLINE_GLOSS_FILES LIST_DIRECTORIES false "${MSIME_OFFLINE_GLOSSES}/zh-*.db")
  if(NOT MSIME_OFFLINE_GLOSS_FILES)
    message(FATAL_ERROR "MSIME_OFFLINE_GLOSSES has no zh-*.db: ${MSIME_OFFLINE_GLOSSES}")
  endif()
  install(FILES ${MSIME_OFFLINE_GLOSS_FILES} "${MSIME_OFFLINE_GLOSSES}/offline-glosses-NOTICE.txt"
          DESTINATION "${CMAKE_INSTALL_DATADIR}/msime-client/offline-glosses")
endif()
if(MSIME_DESKTOP_BINARY)
  if(NOT EXISTS "${MSIME_DESKTOP_BINARY}")
    message(FATAL_ERROR "MSIME_DESKTOP_BINARY does not exist: ${MSIME_DESKTOP_BINARY}")
  endif()
  # Quote the configured path as one POSIX shell word, including apostrophes.
  string(REPLACE "'" "'\\''" MSIME_SETTINGS_SYSTEM_CONFIG
         "${CMAKE_INSTALL_FULL_SYSCONFDIR}/msime-client/runtime-options.json")
  configure_file(data/msime-linux-settings.in ${CMAKE_CURRENT_BINARY_DIR}/msime-linux-settings @ONLY)
  configure_file(data/msime-linux.desktop.in ${CMAKE_CURRENT_BINARY_DIR}/msime-linux.desktop @ONLY)
  install(PROGRAMS "${MSIME_DESKTOP_BINARY}" DESTINATION "${CMAKE_INSTALL_BINDIR}" RENAME msime-linux-desktop)
  install(PROGRAMS ${CMAKE_CURRENT_BINARY_DIR}/msime-linux-settings DESTINATION "${CMAKE_INSTALL_BINDIR}")
  install(FILES ${CMAKE_CURRENT_BINARY_DIR}/msime-linux.desktop DESTINATION "${CMAKE_INSTALL_DATADIR}/applications")
  # The Omarchy theme-set hook runs msime-linux-settings, so it ships with it; msime-linux-setup links it into ~/.config/omarchy/hooks/theme-set.d on an Omarchy desktop.
  configure_file(data/omarchy/theme-set.in ${CMAKE_CURRENT_BINARY_DIR}/omarchy/theme-set @ONLY)
  install(PROGRAMS ${CMAKE_CURRENT_BINARY_DIR}/omarchy/theme-set DESTINATION "${CMAKE_INSTALL_DATADIR}/msime-client/omarchy")
  # The bar widget and its menu stand in for the tray Omarchy disables; the menu opens msime-linux-settings panels, so both ship with it too. msime-linux-setup links the plugin directory into ~/.config/omarchy/plugins.
  configure_file(data/omarchy/menu.in ${CMAKE_CURRENT_BINARY_DIR}/omarchy/menu @ONLY)
  configure_file(data/omarchy/plugin/Widget.qml.in ${CMAKE_CURRENT_BINARY_DIR}/omarchy/plugin/Widget.qml @ONLY)
  install(PROGRAMS ${CMAKE_CURRENT_BINARY_DIR}/omarchy/menu DESTINATION "${CMAKE_INSTALL_DATADIR}/msime-client/omarchy")
  install(FILES data/omarchy/plugin/manifest.json ${CMAKE_CURRENT_BINARY_DIR}/omarchy/plugin/Widget.qml
          DESTINATION "${CMAKE_INSTALL_DATADIR}/msime-client/omarchy/plugin")
endif()
# Nothing here starts the MCP server: an agent's MCP configuration names the command and runs it over stdio, so it only has to be on PATH.
if(MSIME_MCP_BINARY)
  if(NOT EXISTS "${MSIME_MCP_BINARY}")
    message(FATAL_ERROR "MSIME_MCP_BINARY does not exist: ${MSIME_MCP_BINARY}")
  endif()
  install(PROGRAMS "${MSIME_MCP_BINARY}" DESTINATION "${CMAKE_INSTALL_BINDIR}")
endif()
# The built-in sound packs, synthesized by scripts/generate_sound_packs.py and dedicated to the public domain, beside the resource directory rather than in it: that directory has to hold exactly what the dictionary lock pins. The hosts and the settings window name this directory to client-core as the built-in pack root.
install(DIRECTORY "${CMAKE_CURRENT_SOURCE_DIR}/../../resources/sound-packs/"
        DESTINATION "${CMAKE_INSTALL_DATADIR}/msime-client/sound-packs")
# 桌面入口的 Icon=msime-linux、Fcitx5 输入法条目的 Icon 和 IBus 组件的 <icon> 都按名字
# 解析，所以这些文件决定了托盘、输入法列表和启动器上显示的是 logo 还是占位图。它们此前
# 装在桌面二进制那个分支里，于是只装输入法、不装设置窗口的安装一张图标都没有，而那恰恰
# 是最小可用的安装方式。
install(DIRECTORY "${CMAKE_CURRENT_SOURCE_DIR}/data/icons/"
        DESTINATION "${CMAKE_INSTALL_DATADIR}/icons")
configure_file(cmake/icon-cache.cmake.in ${CMAKE_CURRENT_BINARY_DIR}/icon-cache.cmake @ONLY)
install(SCRIPT ${CMAKE_CURRENT_BINARY_DIR}/icon-cache.cmake)
if(MSIME_RUNTIME_OPTIONS_FILE)
 install(FILES "${MSIME_RUNTIME_OPTIONS_FILE}" DESTINATION "${CMAKE_INSTALL_SYSCONFDIR}/msime-client" RENAME runtime-options.json)
endif()
install(FILES ${CMAKE_CURRENT_BINARY_DIR}/msime-linux.xml DESTINATION "${CMAKE_INSTALL_DATADIR}/ibus/component")
install(FILES "${MSIME_HOST_LIBRARY}" DESTINATION "${CMAKE_INSTALL_LIBDIR}/msime-client")
file(RELATIVE_PATH MSIME_HOST_LIBRARY_RELATIVE_DIR
  "${CMAKE_INSTALL_FULL_BINDIR}" "${CMAKE_INSTALL_FULL_LIBDIR}/msime-client")
set_target_properties(msime-linux-ibus msime-linux-dictionary msime-linux-cloud-dictionary msime-linux-cloud-clipboard msime-linux-online msime-linux-translation msime-linux-handwriting
                      msime-linux-voice msime-linux-emoji msime-linux-clipboard-capture msime-linux-prepare PROPERTIES
  INSTALL_RPATH "$ORIGIN/${MSIME_HOST_LIBRARY_RELATIVE_DIR}")

add_executable(linux-native-compose-test tests/input/native_compose.cpp)
add_executable(linux-wave-overlay-model-test
  tests/runtime/wave_overlay_model.cpp src/overlay/WaveOverlayIbusSurface.cpp
  src/overlay/WaveOverlaySurfaceFactory.cpp)
target_compile_features(linux-wave-overlay-model-test PRIVATE cxx_std_17)
target_compile_options(linux-wave-overlay-model-test PRIVATE -Wall -Wextra -Werror)
target_include_directories(linux-wave-overlay-model-test PRIVATE src)
target_link_libraries(linux-wave-overlay-model-test PRIVATE PkgConfig::IBUS)
if(X11_SURFACE_FOUND)
  target_sources(linux-wave-overlay-model-test PRIVATE src/overlay/WaveOverlayX11Surface.cpp)
  target_compile_definitions(linux-wave-overlay-model-test PRIVATE MSIME_LINUX_HAS_X11_SURFACE)
  target_link_libraries(linux-wave-overlay-model-test PRIVATE PkgConfig::X11_SURFACE)
endif()
if(WAYLAND_CLIENT_FOUND AND WAYLAND_SCANNER_EXECUTABLE AND MSIME_XDG_SHELL_XML)
  target_sources(linux-wave-overlay-model-test PRIVATE
                 src/overlay/WaveOverlayWaylandSurface.cpp "${MSIME_LAYER_SHELL_CODE}"
                 ${MSIME_XDG_SHELL_CODE})
  target_include_directories(linux-wave-overlay-model-test PRIVATE
                             "${MSIME_WAYLAND_PROTOCOL_DIR}")
  target_compile_definitions(linux-wave-overlay-model-test PRIVATE
                             MSIME_LINUX_HAS_WAYLAND_SURFACE)
  target_link_libraries(linux-wave-overlay-model-test PRIVATE
                        PkgConfig::WAYLAND_CLIENT)
  if(WAYLAND_TEXT_FOUND)
    target_compile_definitions(linux-wave-overlay-model-test PRIVATE
                               MSIME_LINUX_WAYLAND_TEXT)
    target_link_libraries(linux-wave-overlay-model-test PRIVATE
                          PkgConfig::WAYLAND_TEXT)
  endif()
endif()
add_test(NAME linux-wave-overlay-model COMMAND linux-wave-overlay-model-test)
add_executable(linux-wave-overlay-placement-test tests/runtime/wave_overlay_placement.cpp)
target_compile_features(linux-wave-overlay-placement-test PRIVATE cxx_std_17)
target_compile_options(linux-wave-overlay-placement-test PRIVATE -Wall -Wextra -Werror)
target_include_directories(linux-wave-overlay-placement-test PRIVATE src)
target_link_libraries(linux-wave-overlay-placement-test PRIVATE nlohmann_json::nlohmann_json)
add_test(NAME linux-wave-overlay-placement COMMAND linux-wave-overlay-placement-test)
add_executable(linux-mode-badge-style-test tests/runtime/mode_badge_style.cpp)
target_compile_features(linux-mode-badge-style-test PRIVATE cxx_std_17)
target_compile_options(linux-mode-badge-style-test PRIVATE -Wall -Wextra -Werror)
target_include_directories(linux-mode-badge-style-test PRIVATE src)
target_link_libraries(linux-mode-badge-style-test PRIVATE nlohmann_json::nlohmann_json)
add_test(NAME linux-mode-badge-style COMMAND linux-mode-badge-style-test)
target_compile_options(linux-native-compose-test PRIVATE -Wall -Wextra -Werror)
target_include_directories(linux-native-compose-test PRIVATE src)
target_link_libraries(linux-native-compose-test PRIVATE PkgConfig::XKBCOMMON)
add_test(NAME linux-native-compose COMMAND linux-native-compose-test
  "${CMAKE_CURRENT_SOURCE_DIR}/tests/input/compose.fixture")

option(MSIME_ENABLE_PACKAGING "Enable Linux TGZ and Debian package generation" OFF)
# IBus 与 Fcitx5 是并列的两个系统入口，不是宿主和它的插件，所以一次普通的开发构建
# 没有理由把其中一个整个落下。这个默认值原先跟着打包开关走，于是直接配置本目录只会
# 构建 IBus 宿主，Fcitx5 插件仅对正在做包的人编译。改为跟着环境走：装了开发包就构建
# 它，没装就把取得它的办法打出来，而不是悄悄丢掉这个平台的一半。打包仍然强制包含它，
# -DMSIME_ENABLE_FCITX5=OFF 仍然可以显式退出。
find_package(Fcitx5Core 5.0.20 QUIET)
if(MSIME_ENABLE_PACKAGING OR Fcitx5Core_FOUND)
  set(MSIME_ENABLE_FCITX5_DEFAULT ON)
else()
  set(MSIME_ENABLE_FCITX5_DEFAULT OFF)
endif()
option(MSIME_ENABLE_FCITX5 "Build the native Fcitx5 input method addon"
       ${MSIME_ENABLE_FCITX5_DEFAULT})
if(NOT MSIME_ENABLE_FCITX5)
  message(STATUS "跳过：Fcitx5 插件；请安装 Fcitx5 开发包（Fcitx5Core 5.0.20+），"
                 "或在配置时传入 -DMSIME_ENABLE_FCITX5=ON")
endif()
# License and third-party notices, the Linux counterpart of the macOS bundle's Resources/Licenses. Every install redistributes the code compiled into the host library, so plain `cmake --install` gets them as well, not only packages. The project license is `copyright` here (GPL-3.0.txt on macOS); it also covers the input engine (crates/engine), which is this project's code. What the engine carries from elsewhere keeps its own notice: zinnia, whose recognizer crates/engine/src/handwriting ports; the place names `@` mode offers, from modood/Administrative-divisions-of-China (WTFPL); the Korean Hanja table, from libhangul's data/hanja/hanja.txt (BSD-3-Clause); the provenance of the dictionary data msime-linux-setup downloads, carried verbatim from msime-engine as resources/licenses/msime-engine-dictionary-NOTICE.md, the copy every platform ships; and the provenance of the helpcode tables installed into the packaged resource directory: the five the Engine brought (ENGINE-NOTICE.md) and jiajia (NOTICE.md).
set(MSIME_NOTICE_SOURCES
  "${CMAKE_CURRENT_SOURCE_DIR}/../../LICENSE"
  "${CMAKE_CURRENT_SOURCE_DIR}/data/THIRD_PARTY_NOTICES.txt"
  "${CMAKE_CURRENT_SOURCE_DIR}/../../resources/licenses/msime-engine-dictionary-NOTICE.md"
  "${CMAKE_CURRENT_SOURCE_DIR}/../../resources/helpcodes/ENGINE-NOTICE.md"
  "${CMAKE_CURRENT_SOURCE_DIR}/../../resources/helpcodes/NOTICE.md"
  "${CMAKE_CURRENT_SOURCE_DIR}/../../resources/licenses/Zinnia-LICENSE.txt"
  "${CMAKE_CURRENT_SOURCE_DIR}/../../resources/licenses/Administrative-divisions-of-China-WTFPL.txt"
  "${CMAKE_CURRENT_SOURCE_DIR}/../../resources/licenses/libhangul-hanja-BSD-3-Clause.txt"
  "${CMAKE_CURRENT_SOURCE_DIR}/../../crates/client-core/data/opencc/LICENSE")
# `copyright` is the name Debian policy requires for the package license.
set(MSIME_NOTICE_NAMES
  copyright
  THIRD_PARTY_NOTICES.txt
  msime-engine-dictionary-NOTICE.md
  msime-engine-helpcode-NOTICE.md
  msime-helpcode-jiajia-NOTICE.md
  Zinnia-LICENSE.txt
  Administrative-divisions-of-China-WTFPL.txt
  libhangul-hanja-BSD-3-Clause.txt
  OpenCC-Apache-2.0.txt)
# The handwriting model's LGPL-2.1 license travels with the model, so it is a notice only when the model is installed.
if(MSIME_HANDWRITING_MODEL_DIR)
  list(APPEND MSIME_NOTICE_SOURCES "${MSIME_HANDWRITING_MODEL_DIR}/HandwritingModel-LICENSE.txt")
  list(APPEND MSIME_NOTICE_NAMES HandwritingModel-LICENSE.txt)
endif()
# Header-only nlohmann/json is compiled into the IBus engine, the provider programs and the Fcitx5 addon; no runtime package carries its copyright, so a pinned copy of its license ships instead.
list(APPEND MSIME_NOTICE_SOURCES "${CMAKE_CURRENT_SOURCE_DIR}/data/licenses/nlohmann_json-MIT.txt")
list(APPEND MSIME_NOTICE_NAMES nlohmann_json-MIT.txt)
# msime-voice-local compiles the sherpa-onnx C API header in; the runtime libraries, when installed, add ONNX Runtime and the notices of what it bundles.
list(APPEND MSIME_NOTICE_SOURCES "${MSIME_SHARED_VOICE_DIR}/third_party/sherpa-onnx/LICENSE")
list(APPEND MSIME_NOTICE_NAMES sherpa-onnx-Apache-2.0.txt)
if(MSIME_VOICE_RUNTIME_DIR)
  list(APPEND MSIME_NOTICE_SOURCES "${CMAKE_CURRENT_SOURCE_DIR}/data/licenses/onnxruntime-MIT.txt"
       "${CMAKE_CURRENT_SOURCE_DIR}/data/licenses/onnxruntime-ThirdPartyNotices.txt")
  list(APPEND MSIME_NOTICE_NAMES onnxruntime-MIT.txt onnxruntime-ThirdPartyNotices.txt)
endif()
# wayland-scanner compiles the layer-shell and xdg-shell protocol code into the IBus host, the wave overlay and the Fcitx5 addon, and only when xdg-shell.xml was found (both the IBus and the Fcitx5 side require it). The layer-shell license (HPND) asks for its notice in supporting documentation and xdg-shell's (MIT) in all copies, so the <copyright> block of the XML actually compiled is written out as the notice.
if(MSIME_XDG_SHELL_XML)
  set(MSIME_PROTOCOL_XMLS "${CMAKE_CURRENT_SOURCE_DIR}/data/wayland/wlr-layer-shell-unstable-v1.xml" "${MSIME_XDG_SHELL_XML}")
  set(MSIME_PROTOCOL_NOTICES wayland-wlr-layer-shell-LICENSE.txt wayland-xdg-shell-LICENSE.txt)
  foreach(MSIME_PROTOCOL_XML MSIME_PROTOCOL_NOTICE IN ZIP_LISTS MSIME_PROTOCOL_XMLS MSIME_PROTOCOL_NOTICES)
    file(READ "${MSIME_PROTOCOL_XML}" MSIME_PROTOCOL_CONTENT)
    if(NOT MSIME_PROTOCOL_CONTENT MATCHES "<copyright>(.*)</copyright>")
      message(FATAL_ERROR "No <copyright> block in ${MSIME_PROTOCOL_XML}")
    endif()
    get_filename_component(MSIME_PROTOCOL_FILE "${MSIME_PROTOCOL_XML}" NAME)
    file(WRITE "${CMAKE_CURRENT_BINARY_DIR}/notices/${MSIME_PROTOCOL_NOTICE}" "Wayland protocol ${MSIME_PROTOCOL_FILE}, compiled in as wayland-scanner output\n${CMAKE_MATCH_1}")
    set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS "${MSIME_PROTOCOL_XML}")
    list(APPEND MSIME_NOTICE_SOURCES "${CMAKE_CURRENT_BINARY_DIR}/notices/${MSIME_PROTOCOL_NOTICE}")
    list(APPEND MSIME_NOTICE_NAMES "${MSIME_PROTOCOL_NOTICE}")
  endforeach()
endif()
# Rust crates are statically linked into the host library (a cdylib), the MCP server and the desktop binary, and the desktop binary embeds a frontend bundled from npm packages. collect-notices.py gathers their license files from the resolved graphs; package-container.sh runs it and passes the results here. A development checkout has no such files, so only packaging requires them.
set(MSIME_RUST_NOTICES "" CACHE FILEPATH "Collected license files of the Rust crates linked into the host library, the MCP server and the desktop binary")
set(MSIME_FRONTEND_NOTICES "" CACHE FILEPATH "Collected license files of the npm packages bundled into the desktop frontend")
if(MSIME_ENABLE_PACKAGING AND NOT MSIME_RUST_NOTICES)
  message(FATAL_ERROR "Packaging requires MSIME_RUST_NOTICES; generate it with platforms/linux/collect-notices.py cargo (package-container.sh does)")
endif()
if(MSIME_ENABLE_PACKAGING AND MSIME_DESKTOP_BINARY AND NOT MSIME_FRONTEND_NOTICES)
  message(FATAL_ERROR "Packaging the desktop binary requires MSIME_FRONTEND_NOTICES; generate it with platforms/linux/collect-notices.py npm (package-container.sh does)")
endif()
if(MSIME_RUST_NOTICES)
  list(APPEND MSIME_NOTICE_SOURCES "${MSIME_RUST_NOTICES}")
  list(APPEND MSIME_NOTICE_NAMES rust-crates-NOTICES.txt)
endif()
if(MSIME_FRONTEND_NOTICES)
  list(APPEND MSIME_NOTICE_SOURCES "${MSIME_FRONTEND_NOTICES}")
  list(APPEND MSIME_NOTICE_NAMES frontend-npm-NOTICES.txt)
endif()
set(MSIME_MISSING_NOTICES "")
foreach(MSIME_NOTICE_SOURCE MSIME_NOTICE_NAME IN ZIP_LISTS MSIME_NOTICE_SOURCES MSIME_NOTICE_NAMES)
  if(EXISTS "${MSIME_NOTICE_SOURCE}" AND NOT IS_DIRECTORY "${MSIME_NOTICE_SOURCE}")
    install(FILES "${MSIME_NOTICE_SOURCE}"
            DESTINATION "${CMAKE_INSTALL_DATADIR}/doc/msime-client" RENAME "${MSIME_NOTICE_NAME}")
  else()
    list(APPEND MSIME_MISSING_NOTICES "${MSIME_NOTICE_SOURCE}")
  endif()
endforeach()
if(MSIME_MISSING_NOTICES)
  list(JOIN MSIME_MISSING_NOTICES "\n  " MSIME_MISSING_NOTICE_LIST)
  # A redistributable package without the notices of the code it contains must not be produced; a development install only warns.
  if(MSIME_ENABLE_PACKAGING)
    message(FATAL_ERROR "Packaging requires every third-party notice; missing:\n  ${MSIME_MISSING_NOTICE_LIST}")
  endif()
  message(WARNING "Installing without these third-party notices:\n  ${MSIME_MISSING_NOTICE_LIST}")
endif()
if(MSIME_ENABLE_PACKAGING)
  include(cmake/packaging.cmake)
endif()
if(MSIME_ENABLE_FCITX5)
  add_subdirectory(fcitx5)
endif()

# assert() 是这些测试唯一的断言手段，而 Release 配置定义的 NDEBUG 会把它们整个消掉：
# 测试于是一个条件都不检查，还照样报告通过，只在 assert 里出现的局部变量同时变成未使
# 用变量，-Werror 再把构建打断。此前只有两个目标单独带了 -UNDEBUG，其余三十来个没有。
# 断言对构建类型的依赖不该逐个目标去记，统一加在这里。
get_property(msime_linux_targets DIRECTORY PROPERTY BUILDSYSTEM_TARGETS)
foreach(msime_linux_target IN LISTS msime_linux_targets)
  if(msime_linux_target MATCHES "-test$")
    target_compile_options(${msime_linux_target} PRIVATE -UNDEBUG)
  endif()
endforeach()
