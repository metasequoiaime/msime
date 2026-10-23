cmake_minimum_required(VERSION 3.25)
project(MSIMEClientLinux LANGUAGES CXX)
find_package(CURL REQUIRED)
include(GNUInstallDirs)
include(cmake/Edition.cmake)
# The user units msime-linux-setup can enable. The CMake uninstall and the Debian maintainer scripts all stop them, so the list is defined once here; each socket precedes its service so stopping in this order does not activate the service again. The maintainer scripts restart only the services: sockets run no program.
# 单元名以版本的包名开头（full 是 msime-linux），几个版本的用户服务可以同时启用。
set(MSIME_USER_UNITS ${MSIME_EDITION_PACKAGE}-online.socket ${MSIME_EDITION_PACKAGE}-online.service ${MSIME_EDITION_PACKAGE}-voice.socket
    ${MSIME_EDITION_PACKAGE}-voice.service ${MSIME_EDITION_PACKAGE}-clipboard.service)
set(MSIME_USER_SERVICES ${MSIME_USER_UNITS})
list(FILTER MSIME_USER_SERVICES INCLUDE REGEX "\\.service$")
list(JOIN MSIME_USER_UNITS " " MSIME_USER_UNIT_LIST)
list(JOIN MSIME_USER_SERVICES " " MSIME_USER_SERVICE_LIST)
# Session managers read XDG autostart entries from $XDG_CONFIG_DIRS/autostart, /etc/xdg/autostart by default. GNUInstallDirs maps the system configuration directory of a /usr prefix to /etc, so only then is the entry installed there, as an absolute directory the CMake uninstall accepts as a second root. Every other prefix keeps the directory relative, so `cmake --install --prefix` relocates it with the rest of the installation; that includes /opt/<name>, whose /etc/opt/<name> no session manager reads.
# 装在 /opt/<package> 下的版本，GNUInstallDirs 给的系统配置目录是 /etc/opt/<package>，会话管理器不读那里，所以自启动项同样装进 /etc/xdg/autostart。
if(CMAKE_INSTALL_FULL_SYSCONFDIR STREQUAL "/etc" OR MSIME_EDITION_SYSTEM_PATHS)
  set(MSIME_XDG_AUTOSTART_DIR "/etc/xdg/autostart")
else()
  set(MSIME_XDG_AUTOSTART_DIR "${CMAKE_INSTALL_SYSCONFDIR}/xdg/autostart")
endif()
msime_edition_source(MSIME_UNINSTALL_SOURCE cmake/uninstall.cmake.in)
configure_file("${MSIME_UNINSTALL_SOURCE}" ${CMAKE_CURRENT_BINARY_DIR}/uninstall.cmake @ONLY)
# Configured for every build, not only packaging ones, so the gate's ctest exercises the scripts the .deb ships; packaging.cmake hands them to CPack.
foreach(MSIME_DEB_SCRIPT prerm postinst)
  msime_edition_source(MSIME_DEB_SCRIPT_SOURCE cmake/deb-${MSIME_DEB_SCRIPT}.in)
  configure_file("${MSIME_DEB_SCRIPT_SOURCE}" ${CMAKE_CURRENT_BINARY_DIR}/debian/${MSIME_DEB_SCRIPT} @ONLY
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
  add_test(NAME linux-ibus-component-contract
           COMMAND "${Python3_EXECUTABLE}" "${CMAKE_CURRENT_SOURCE_DIR}/tests/core/ibus_component_contract.py")
endif()
set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
# GCC 8（Debian 10 的编译器，package-legacy-container.sh 用它）的 std::filesystem 在单独的静态库 libstdc++fs 里，GCC 9 起才并入 libstdc++。加在每条链接命令的末尾，排在所有用到它的目标文件和静态库之后。
if(CMAKE_CXX_COMPILER_ID STREQUAL "GNU" AND CMAKE_CXX_COMPILER_VERSION VERSION_LESS 9)
  string(APPEND CMAKE_CXX_STANDARD_LIBRARIES " -lstdc++fs")
endif()
find_package(PkgConfig REQUIRED)
include_directories("${CMAKE_CURRENT_SOURCE_DIR}/src/core" "${CMAKE_CURRENT_SOURCE_DIR}/src/candidates" "${CMAKE_CURRENT_SOURCE_DIR}/src/clipboard" "${CMAKE_CURRENT_SOURCE_DIR}/src/providers" "${CMAKE_CURRENT_SOURCE_DIR}/src/voice" "${CMAKE_CURRENT_SOURCE_DIR}/src/overlay" "${CMAKE_CURRENT_SOURCE_DIR}/src/system" "${CMAKE_CURRENT_SOURCE_DIR}/src/entrypoints")
# IBus 与 Python 的最低版本。配置时按前者检查 IBus 开发包，cmake/packaging.cmake 把两者写进 .deb 的 Depends 和 RPM 的 Requires。缺省值是发布页的包所声明的；Debian 10 基线的 legacy 包（package-legacy-container.sh）传 1.5.19 和 3.7，IBus 1.5.20 起才有的接口在源码里按 IBUS_CHECK_VERSION 条件编译。
set(MSIME_IBUS_MIN_VERSION "1.5.20" CACHE STRING "Oldest IBus the host builds against and the packages depend on")
set(MSIME_PYTHON_MIN_VERSION "3.9" CACHE STRING "Oldest Python 3 the packages depend on for the installed scripts")
pkg_check_modules(IBUS REQUIRED IMPORTED_TARGET ibus-1.0>=${MSIME_IBUS_MIN_VERSION})
pkg_check_modules(XKBCOMMON REQUIRED IMPORTED_TARGET xkbcommon)
find_package(nlohmann_json 3.11 REQUIRED)
# Optional on Wayland-only systems; the monitor retains bounded polling.
pkg_check_modules(X11_CLIPBOARD QUIET IMPORTED_TARGET x11 xfixes)
# 语音浮层用 RandR 1.5 枚举显示器，以跟随获得焦点的窗口；凡是带 libXfixes 的 X 桌面都带 libXrandr。只有 Xlib 也能构建 X11 的 Caps Lock 释放（`src/system/CapsLockRelease.cpp`，`MSIME_LINUX_HAS_X11_SURFACE`）。
pkg_check_modules(X11_SURFACE QUIET IMPORTED_TARGET x11 xfixes xrandr)
# 两个浮层后端都经 `WaveOverlayPainter.h` 用 Cairo 和 Pango 绘制（X11 上用 cairo-xlib），装了 GTK 的桌面都有；缺了它们浮层就退到辅助文本。这项检查与 `X11_SURFACE` 分开，缺这些库的机器照样能得到 Caps Lock 释放。
pkg_check_modules(X11_OVERLAY_DRAW QUIET IMPORTED_TARGET cairo-xlib pangocairo)
pkg_check_modules(WAYLAND_CLIENT QUIET IMPORTED_TARGET wayland-client)
pkg_check_modules(WAYLAND_TEXT QUIET IMPORTED_TARGET pangocairo)
find_program(WAYLAND_SCANNER_EXECUTABLE wayland-scanner)
if(WAYLAND_CLIENT_FOUND AND WAYLAND_SCANNER_EXECUTABLE)
  enable_language(C)
  # Wayland 浮层用 shm_open 建共享内存缓冲。glibc 2.34 起它在 libc 里，更早的 glibc（legacy 包所在的 Debian 10 是 2.28）要另外链接 librt。
  include(CheckSymbolExists)
  check_symbol_exists(shm_open "sys/mman.h" MSIME_LIBC_HAS_SHM_OPEN)
  set(MSIME_SHM_LIBRARIES "")
  if(NOT MSIME_LIBC_HAS_SHM_OPEN)
    set(MSIME_SHM_LIBRARIES rt)
  endif()
  set(MSIME_WAYLAND_PROTOCOL_DIR "${CMAKE_CURRENT_BINARY_DIR}/wayland-protocols")
  set(MSIME_LAYER_SHELL_HEADER
      "${MSIME_WAYLAND_PROTOCOL_DIR}/wlr-layer-shell-unstable-v1-client-protocol.h")
  set(MSIME_LAYER_SHELL_CODE
      "${MSIME_WAYLAND_PROTOCOL_DIR}/wlr-layer-shell-unstable-v1-protocol.c")
  # wayland-protocols 的 .pc 给出它的数据目录；只搜 /usr/share 的话，装在别处的（例如 Nix store）
  # 找不到，Wayland 浮层被静默跳过。原有路径留作没有 .pc 时的退路。
  pkg_get_variable(MSIME_WAYLAND_PROTOCOLS_DIR wayland-protocols pkgdatadir)
  find_path(MSIME_XDG_SHELL_DIR NAMES xdg-shell.xml
            HINTS "${MSIME_WAYLAND_PROTOCOLS_DIR}/stable/xdg-shell"
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
option(MSIME_BUNDLE_HANDWRITING_MODEL "Install the offline handwriting model with its license and require it when packaging; OFF leaves it to the settings app to download on demand" ON)
set(MSIME_EMOJI_RESOURCES "" CACHE PATH "Trusted packaged Engine Emoji resource directory")
set(MSIME_ENGINE_RESOURCES "" CACHE PATH "Trusted packaged Engine desktop resource directory")
if(NOT EXISTS "${MSIME_HOST_LIBRARY}")
  message(FATAL_ERROR "Build msime-host-api for Linux first")
endif()
msime_edition_host_library(MSIME_HOST_LIBRARY)
add_library(msime-ibus-host STATIC
  src/core/ClientEngine.cpp src/system/DiagnosticLog.cpp src/system/ChineseTextConversion.cpp src/system/CapsLockRelease.cpp src/overlay/WaveOverlayIbusSurface.cpp
  src/overlay/WaveOverlaySurfaceFactory.cpp)
target_compile_options(msime-ibus-host PRIVATE -Wall -Wextra -Werror)
target_include_directories(msime-ibus-host PUBLIC src src/core src/candidates src/clipboard src/providers src/voice src/overlay src/system src/entrypoints ../../crates/host-api/include)
# ClientEngine.cpp 用到 pthread，由这个库自己公开声明，链接它的可执行文件不必各自补 `Threads::Threads`。glibc 2.34 之前 pthread 不在 libc 里，漏掉就只在 Debian 10 这类旧系统上链接失败（#7849 新加的 ibus-mode-hint-lifecycle 就是这样）。
find_package(Threads REQUIRED)
target_link_libraries(msime-ibus-host PUBLIC PkgConfig::IBUS PkgConfig::XKBCOMMON nlohmann_json::nlohmann_json "${MSIME_HOST_LIBRARY}" Threads::Threads)
if(X11_SURFACE_FOUND)
  target_compile_definitions(msime-ibus-host PRIVATE MSIME_LINUX_HAS_X11_SURFACE)
  target_link_libraries(msime-ibus-host PUBLIC PkgConfig::X11_SURFACE)
endif()
# X11 浮层需要 Xlib、XFixes 和 RandR（`X11_SURFACE`），绘制还需要 cairo-xlib 和 pangocairo；浮层工厂按 `MSIME_LINUX_HAS_X11_OVERLAY` 选用它。
set(MSIME_WAVE_X11_SURFACE OFF)
if(X11_SURFACE_FOUND AND X11_OVERLAY_DRAW_FOUND)
  set(MSIME_WAVE_X11_SURFACE ON)
endif()
if(MSIME_WAVE_X11_SURFACE)
  target_sources(msime-ibus-host PRIVATE src/overlay/WaveOverlayX11Surface.cpp)
  target_compile_definitions(msime-ibus-host PRIVATE MSIME_LINUX_HAS_X11_OVERLAY)
  target_link_libraries(msime-ibus-host PUBLIC PkgConfig::X11_OVERLAY_DRAW)
endif()
# Wayland 浮层需要 layer-shell 和 xdg-shell 的协议代码，绘制需要 pangocairo。
set(MSIME_WAVE_WAYLAND_SURFACE OFF)
if(WAYLAND_CLIENT_FOUND AND WAYLAND_SCANNER_EXECUTABLE AND MSIME_XDG_SHELL_XML AND WAYLAND_TEXT_FOUND)
  set(MSIME_WAVE_WAYLAND_SURFACE ON)
endif()
if(MSIME_WAVE_WAYLAND_SURFACE)
  target_sources(msime-ibus-host PRIVATE src/overlay/WaveOverlayWaylandSurface.cpp
                 "${MSIME_LAYER_SHELL_CODE}" ${MSIME_XDG_SHELL_CODE})
  target_include_directories(msime-ibus-host PRIVATE "${MSIME_WAYLAND_PROTOCOL_DIR}")
  target_compile_definitions(msime-ibus-host PRIVATE MSIME_LINUX_HAS_WAYLAND_SURFACE)
  target_link_libraries(msime-ibus-host PUBLIC PkgConfig::WAYLAND_CLIENT PkgConfig::WAYLAND_TEXT ${MSIME_SHM_LIBRARIES})
endif()
# 中英文切换徽章，与 Fcitx5 插件共用 src/overlay/ModeBadge*。X11 后端能把徽章放到光标旁；Wayland 后端只在有 layer-shell 的合成器上可用，固定在屏幕角上。两个都没有时宿主只用辅助文本提示。
pkg_check_modules(MODE_BADGE_X11 QUIET IMPORTED_TARGET x11 xext xrandr cairo-xlib pangocairo)
set(MSIME_IBUS_MODE_BADGE OFF)
if(MODE_BADGE_X11_FOUND)
  target_sources(msime-ibus-host PRIVATE src/overlay/ModeBadgeX11Surface.cpp)
  target_compile_definitions(msime-ibus-host PRIVATE MSIME_BADGE_HAS_X11)
  target_link_libraries(msime-ibus-host PUBLIC PkgConfig::MODE_BADGE_X11)
  set(MSIME_IBUS_MODE_BADGE ON)
  # 悬浮工具栏和双拼键位图与徽章用同一套 X11、cairo 和 pango：没有它们时宿主只有菜单里的「工具栏」子菜单，也不画键位图。
  target_sources(msime-ibus-host PRIVATE src/overlay/OverlayX11Window.cpp src/overlay/ShuangpinKeymapX11Surface.cpp
                 src/overlay/FloatingToolbar.cpp src/overlay/FloatingToolbarFactory.cpp
                 src/overlay/FloatingToolbarX11Surface.cpp)
  target_compile_definitions(msime-ibus-host PRIVATE MSIME_LINUX_FLOATING_PANELS)
  # libX11 1.7 起每条连接可以装自己的退出处理（XSetIOErrorExitHandler）：工具栏和键位图的连接断开时不再让整个引擎退出（见 OverlayX11Window.h 的 open_overlay_display）。legacy 包的 Debian 10 只有 1.6.7，那里照旧。
  if(MODE_BADGE_X11_x11_VERSION VERSION_GREATER_EQUAL 1.7.0)
    set(MSIME_X11_IO_ERROR_EXIT_HANDLER ON)
    target_compile_definitions(msime-ibus-host PRIVATE MSIME_X11_IO_ERROR_EXIT_HANDLER)
  endif()
endif()
if(WAYLAND_CLIENT_FOUND AND WAYLAND_SCANNER_EXECUTABLE AND MSIME_XDG_SHELL_XML AND WAYLAND_TEXT_FOUND)
  target_sources(msime-ibus-host PRIVATE src/overlay/ModeBadgeWaylandSurface.cpp)
  target_compile_definitions(msime-ibus-host PRIVATE MSIME_BADGE_HAS_WAYLAND)
  set(MSIME_IBUS_MODE_BADGE ON)
endif()
if(MSIME_IBUS_MODE_BADGE)
  target_sources(msime-ibus-host PRIVATE src/overlay/ModeBadgeSurfaceFactory.cpp)
  target_compile_definitions(msime-ibus-host PRIVATE MSIME_IBUS_MODE_BADGE
      MSIME_MODE_BADGE_ICON="${MSIME_SYSTEM_FULL_DATADIR}/icons/hicolor/128x128/apps/${MSIME_EDITION_PACKAGE}.png")
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
target_link_libraries(msime-linux-ibus PRIVATE Threads::Threads)
target_compile_options(msime-linux-ibus PRIVATE -Wall -Wextra -Werror)
target_link_libraries(msime-linux-ibus PRIVATE msime-ibus-host)
add_executable(msime-linux-dictionary src/providers/dictionary_request.cpp)
add_executable(msime-linux-prepare src/entrypoints/prepare_host_main.cpp)
target_compile_options(msime-linux-prepare PRIVATE -Wall -Wextra -Werror)
target_link_libraries(msime-linux-prepare PRIVATE msime-ibus-host)
file(RELATIVE_PATH MSIME_INSTALLED_RESOURCE_RELATIVE_DIR
     "${CMAKE_INSTALL_FULL_BINDIR}"
     "${CMAKE_INSTALL_FULL_DATADIR}/${MSIME_CLIENT_DIRECTORY}/resources")
target_compile_definitions(msime-linux-prepare PRIVATE
  MSIME_INSTALLED_RESOURCE_RELATIVE_DIR="${MSIME_INSTALLED_RESOURCE_RELATIVE_DIR}")
# The IBus engine names the installed built-in sound packs to the Host API, found from its own executable as the prepare tool finds the resources (core/PreparePaths.h).
file(RELATIVE_PATH MSIME_INSTALLED_SOUND_PACKS_RELATIVE_DIR
     "${CMAKE_INSTALL_FULL_BINDIR}"
     "${CMAKE_INSTALL_FULL_DATADIR}/${MSIME_CLIENT_DIRECTORY}/sound-packs")
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
if(BUILD_TESTING)
  add_executable(msime-linux-online-provider-contract tests/provider/online_provider_contract.cpp)
  target_compile_options(msime-linux-online-provider-contract PRIVATE -Wall -Wextra -Werror)
  target_include_directories(msime-linux-online-provider-contract PRIVATE ../../crates/host-api/include)
  target_link_libraries(msime-linux-online-provider-contract PRIVATE nlohmann_json::nlohmann_json "${MSIME_HOST_LIBRARY}" Threads::Threads)
  add_test(NAME linux-online-provider-contract COMMAND msime-linux-online-provider-contract)
endif()
add_executable(msime-linux-translation src/providers/translation_request.cpp)
target_compile_options(msime-linux-translation PRIVATE -Wall -Wextra -Werror)
target_link_libraries(msime-linux-translation PRIVATE msime-ibus-host)
add_executable(msime-linux-clipboard src/clipboard/clipboard_history.cpp)
target_compile_options(msime-linux-clipboard PRIVATE -Wall -Wextra -Werror)
target_link_libraries(msime-linux-clipboard PRIVATE nlohmann_json::nlohmann_json)
if(BUILD_TESTING)
  add_executable(linux-cloud-clipboard-route-test tests/clipboard/cloud_clipboard_route.cpp)
  target_compile_options(linux-cloud-clipboard-route-test PRIVATE -Wall -Wextra -Werror -UNDEBUG)
  target_link_libraries(linux-cloud-clipboard-route-test PRIVATE nlohmann_json::nlohmann_json)
  add_test(NAME linux-cloud-clipboard-route COMMAND linux-cloud-clipboard-route-test)
  add_executable(linux-clipboard-atomic-write-test tests/clipboard/clipboard_atomic_write.cpp)
  target_compile_options(linux-clipboard-atomic-write-test PRIVATE -Wall -Wextra -Werror -UNDEBUG)
  target_include_directories(linux-clipboard-atomic-write-test PRIVATE src)
  add_test(NAME linux-clipboard-atomic-write COMMAND linux-clipboard-atomic-write-test)
endif()
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
if(BUILD_TESTING)
  add_executable(ibus-mode-hint-lifecycle tests/runtime/mode_hint_lifecycle.cpp)
  target_compile_options(ibus-mode-hint-lifecycle PRIVATE -Wall -Wextra -Werror)
  target_link_libraries(ibus-mode-hint-lifecycle PRIVATE msime-ibus-host Threads::Threads)
  foreach(scenario english blocked destroyed focus-preferences)
    add_test(NAME ibus-mode-hint-${scenario} COMMAND ibus-mode-hint-lifecycle ${scenario})
    set_tests_properties(ibus-mode-hint-${scenario} PROPERTIES TIMEOUT 15)
  endforeach()
  add_test(NAME ibus-native-compose-no-session
    COMMAND ibus-mode-hint-lifecycle compose)
  set_tests_properties(ibus-native-compose-no-session PROPERTIES TIMEOUT 15)
  # IBus 在 X11 会话里把切换徽章画在光标下方，这条路要一个 X 服务器：有 xvfb-run 时（编译门禁镜像）在 Xvfb 里跑，验证获得焦点时先等光标（全 0 的报告不算）、XIM 那样的点状光标也画在它下方、画了徽章就不发辅助文本、失焦时立刻收起、开始打字就作废还在等光标的获得焦点提示、引擎销毁后徽章照常收起。
  find_program(MSIME_XVFB_RUN xvfb-run)
  if(MODE_BADGE_X11_FOUND AND MSIME_XVFB_RUN)
    target_compile_definitions(ibus-mode-hint-lifecycle PRIVATE MSIME_TEST_X11_BADGE)
    foreach(scenario x11-badge x11-badge-no-caret x11-badge-point x11-toolbar focus-preferences-x11)
      add_test(NAME ibus-mode-hint-${scenario}
        COMMAND "${MSIME_XVFB_RUN}" -a $<TARGET_FILE:ibus-mode-hint-lifecycle> ${scenario})
      set_tests_properties(ibus-mode-hint-${scenario} PROPERTIES TIMEOUT 30)
    endforeach()
  endif()
  add_executable(ibus-engine-smoke tests/runtime/engine_smoke.cpp)
  target_compile_options(ibus-engine-smoke PRIVATE -Wall -Wextra -Werror)
  target_link_libraries(ibus-engine-smoke PRIVATE msime-ibus-host Threads::Threads)
  # 注音和笔画用例把测试词库安装到独立目录，锁定资源中没有这些词库。
  target_compile_definitions(ibus-engine-smoke PRIVATE
    MSIME_HELPCODE_FIXTURE_RESOURCES="${CMAKE_CURRENT_SOURCE_DIR}/../../resources"
    MSIME_ZHUYIN_DICTIONARY_FIXTURE="${CMAKE_CURRENT_SOURCE_DIR}/tests/runtime/zhuyin_dictionary.py"
    MSIME_STROKE_DICTIONARY_FIXTURE="${CMAKE_CURRENT_SOURCE_DIR}/tests/runtime/stroke_dictionary.py")
  if(MSIME_ENGINE_RESOURCES)
    add_test(NAME ibus-helpcode-trigger
      COMMAND ibus-engine-smoke "${MSIME_ENGINE_RESOURCES}" --helpcode-trigger)
    set_tests_properties(ibus-helpcode-trigger PROPERTIES TIMEOUT 60)
    add_test(NAME ibus-page-number-visibility
      COMMAND ibus-engine-smoke "${MSIME_ENGINE_RESOURCES}" --page-number)
    set_tests_properties(ibus-page-number-visibility PROPERTIES TIMEOUT 60)
    add_test(NAME ibus-empty-translation
      COMMAND ibus-engine-smoke "${MSIME_ENGINE_RESOURCES}" --empty-translation)
    set_tests_properties(ibus-empty-translation PROPERTIES TIMEOUT 60)
    add_test(NAME ibus-caps-lock-tap
      COMMAND ibus-engine-smoke "${MSIME_ENGINE_RESOURCES}" --caps-lock)
    set_tests_properties(ibus-caps-lock-tap PROPERTIES TIMEOUT 60)
    add_test(NAME ibus-app-input-mode-rules
      COMMAND ibus-engine-smoke "${MSIME_ENGINE_RESOURCES}" --app-rules)
    set_tests_properties(ibus-app-input-mode-rules PROPERTIES TIMEOUT 60 SKIP_RETURN_CODE 77)
    add_test(NAME ibus-japanese-toggle
      COMMAND ibus-engine-smoke "${MSIME_ENGINE_RESOURCES}" --japanese-toggle)
    set_tests_properties(ibus-japanese-toggle PROPERTIES TIMEOUT 60)
    add_test(NAME ibus-chaizi
      COMMAND ibus-engine-smoke "${MSIME_ENGINE_RESOURCES}" --chaizi)
    set_tests_properties(ibus-chaizi PROPERTIES TIMEOUT 60)
    add_test(NAME ibus-stale-candidate-click
      COMMAND ibus-engine-smoke "${MSIME_ENGINE_RESOURCES}" --stale-candidate-click)
    set_tests_properties(ibus-stale-candidate-click PROPERTIES TIMEOUT 60)
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
  add_executable(linux-input-schemes-test tests/input/input_schemes.cpp)
  target_compile_options(linux-input-schemes-test PRIVATE -Wall -Wextra -Werror -UNDEBUG)
  target_include_directories(linux-input-schemes-test PRIVATE src)
  target_link_libraries(linux-input-schemes-test PRIVATE nlohmann_json::nlohmann_json)
  add_test(NAME linux-input-schemes COMMAND linux-input-schemes-test)
  add_executable(linux-korean-hanja-test tests/input/korean_hanja.cpp)
  target_compile_options(linux-korean-hanja-test PRIVATE -Wall -Wextra -Werror -UNDEBUG)
  target_include_directories(linux-korean-hanja-test PRIVATE src)
  target_link_libraries(linux-korean-hanja-test PRIVATE nlohmann_json::nlohmann_json PkgConfig::IBUS)
  add_test(NAME linux-korean-hanja COMMAND linux-korean-hanja-test)
  add_executable(linux-key-sound-test tests/input/key_sound.cpp)
  target_compile_options(linux-key-sound-test PRIVATE -Wall -Wextra -Werror -UNDEBUG)
  target_include_directories(linux-key-sound-test PRIVATE src)
  add_test(NAME linux-key-sound COMMAND linux-key-sound-test)
  add_executable(linux-lifecycle-feedback-test tests/input/lifecycle_feedback.cpp)
  target_compile_options(linux-lifecycle-feedback-test PRIVATE -Wall -Wextra -Werror -UNDEBUG)
  target_include_directories(linux-lifecycle-feedback-test PRIVATE src ../../crates/host-api/include)
  target_link_libraries(linux-lifecycle-feedback-test PRIVATE nlohmann_json::nlohmann_json)
  add_test(NAME linux-lifecycle-feedback COMMAND linux-lifecycle-feedback-test)
  add_executable(linux-client-input-mode-memory-test tests/input/client_input_mode_memory.cpp)
  target_compile_options(linux-client-input-mode-memory-test PRIVATE -Wall -Wextra -Werror)
  target_include_directories(linux-client-input-mode-memory-test PRIVATE src)
  add_test(NAME linux-client-input-mode-memory COMMAND linux-client-input-mode-memory-test)
  add_executable(linux-app-input-mode-rules-test tests/input/app_input_mode_rules.cpp)
  target_compile_options(linux-app-input-mode-rules-test PRIVATE -Wall -Wextra -Werror -UNDEBUG)
  target_include_directories(linux-app-input-mode-rules-test PRIVATE src)
  target_link_libraries(linux-app-input-mode-rules-test PRIVATE nlohmann_json::nlohmann_json)
  add_test(NAME linux-app-input-mode-rules COMMAND linux-app-input-mode-rules-test)
  add_executable(linux-shuangpin-profile-names-test tests/input/shuangpin_profile_names.cpp)
  target_compile_options(linux-shuangpin-profile-names-test PRIVATE -Wall -Wextra -Werror)
  target_include_directories(linux-shuangpin-profile-names-test PRIVATE src)
  add_test(NAME linux-shuangpin-profile-names COMMAND linux-shuangpin-profile-names-test)
  add_executable(linux-wubi-profile-names-test tests/input/wubi_profile_names.cpp)
  target_compile_options(linux-wubi-profile-names-test PRIVATE -Wall -Wextra -Werror -UNDEBUG)
  target_include_directories(linux-wubi-profile-names-test PRIVATE src)
  add_test(NAME linux-wubi-profile-names COMMAND linux-wubi-profile-names-test)
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
  # fcitx5-gtk 的客户端输入面板用 GKeyFile 读生成的主题；GLib 随 IBus 开发包而来，不是新依赖。
  pkg_check_modules(GLIB2 REQUIRED IMPORTED_TARGET glib-2.0)
  add_executable(linux-candidate-fcitx-theme-gkeyfile-test tests/candidate/candidate_fcitx_theme_gkeyfile.cpp)
  target_compile_options(linux-candidate-fcitx-theme-gkeyfile-test PRIVATE -Wall -Wextra -Werror)
  target_include_directories(linux-candidate-fcitx-theme-gkeyfile-test PRIVATE src)
  target_link_libraries(linux-candidate-fcitx-theme-gkeyfile-test PRIVATE nlohmann_json::nlohmann_json PkgConfig::GLIB2)
  add_test(NAME linux-candidate-fcitx-theme-gkeyfile COMMAND linux-candidate-fcitx-theme-gkeyfile-test)
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
  add_executable(linux-candidate-atomic-write-test tests/candidate/candidate_atomic_write.cpp)
  target_compile_options(linux-candidate-atomic-write-test PRIVATE -Wall -Wextra -Werror -UNDEBUG)
  target_include_directories(linux-candidate-atomic-write-test PRIVATE src)
  add_test(NAME linux-candidate-atomic-write COMMAND linux-candidate-atomic-write-test)
  add_executable(linux-input-status-test tests/core/input_status.cpp)
  target_compile_options(linux-input-status-test PRIVATE -Wall -Wextra -Werror)
  target_include_directories(linux-input-status-test PRIVATE src)
  target_link_libraries(linux-input-status-test PRIVATE nlohmann_json::nlohmann_json)
  add_test(NAME linux-input-status COMMAND linux-input-status-test)
  add_executable(linux-json-integer-test tests/core/json_integer.cpp)
  target_compile_options(linux-json-integer-test PRIVATE -Wall -Wextra -Werror -UNDEBUG)
  target_include_directories(linux-json-integer-test PRIVATE src)
  target_link_libraries(linux-json-integer-test PRIVATE nlohmann_json::nlohmann_json)
  add_test(NAME linux-json-integer COMMAND linux-json-integer-test)
  add_executable(linux-candidate-translation-policy-test tests/candidate/candidate_translation_policy.cpp)
  target_compile_options(linux-candidate-translation-policy-test PRIVATE -Wall -Wextra -Werror)
  target_include_directories(linux-candidate-translation-policy-test PRIVATE src)
  add_test(NAME linux-candidate-translation-policy COMMAND linux-candidate-translation-policy-test)
  add_executable(linux-candidate-row-text-test tests/candidate/candidate_row_text.cpp)
  target_compile_options(linux-candidate-row-text-test PRIVATE -Wall -Wextra -Werror -UNDEBUG)
  target_include_directories(linux-candidate-row-text-test PRIVATE src)
  add_test(NAME linux-candidate-row-text COMMAND linux-candidate-row-text-test)
  add_executable(linux-candidate-script-annotation-test tests/candidate/candidate_script_annotation.cpp)
  target_compile_options(linux-candidate-script-annotation-test PRIVATE -Wall -Wextra -Werror -UNDEBUG)
  target_include_directories(linux-candidate-script-annotation-test PRIVATE src)
  target_link_libraries(linux-candidate-script-annotation-test PRIVATE nlohmann_json::nlohmann_json)
  add_test(NAME linux-candidate-script-annotation COMMAND linux-candidate-script-annotation-test)
  add_executable(linux-paired-punctuation-test tests/input/paired_punctuation.cpp)
  target_compile_options(linux-paired-punctuation-test PRIVATE -Wall -Wextra -Werror)
  target_include_directories(linux-paired-punctuation-test PRIVATE src)
  add_test(NAME linux-paired-punctuation COMMAND linux-paired-punctuation-test)
  add_executable(linux-dictionary-quiesce-lease-test tests/core/dictionary_quiesce_lease.cpp)
  target_compile_options(linux-dictionary-quiesce-lease-test PRIVATE -Wall -Wextra -Werror)
  target_include_directories(linux-dictionary-quiesce-lease-test PRIVATE src)
  add_test(NAME linux-dictionary-quiesce-lease COMMAND linux-dictionary-quiesce-lease-test)
  add_executable(linux-emoji-plugin-groups-test tests/core/emoji_plugin_groups.cpp)
  target_compile_options(linux-emoji-plugin-groups-test PRIVATE -Wall -Wextra -Werror -UNDEBUG)
  target_include_directories(linux-emoji-plugin-groups-test PRIVATE src)
  target_link_libraries(linux-emoji-plugin-groups-test PRIVATE nlohmann_json::nlohmann_json)
  add_test(NAME linux-emoji-plugin-groups COMMAND linux-emoji-plugin-groups-test)
  add_executable(common-dictionary-quiesce-lease-test ../common/tests/dictionary_quiesce_lease.cpp)
  target_compile_options(common-dictionary-quiesce-lease-test PRIVATE -Wall -Wextra -Werror)
  add_test(NAME common-dictionary-quiesce-lease COMMAND common-dictionary-quiesce-lease-test)
  add_executable(common-telemetry-test ../common/tests/telemetry.cpp ../common/Telemetry.cpp)
  target_compile_options(common-telemetry-test PRIVATE -Wall -Wextra -Werror -UNDEBUG)
  target_include_directories(common-telemetry-test PRIVATE ../common ../../crates/host-api/include)
  target_link_libraries(common-telemetry-test PRIVATE nlohmann_json::nlohmann_json "${MSIME_HOST_LIBRARY}" Threads::Threads)
  add_test(NAME common-telemetry COMMAND common-telemetry-test)
  add_executable(linux-input-mode-indicator-test tests/core/input_mode_indicator.cpp)
  target_compile_options(linux-input-mode-indicator-test PRIVATE -Wall -Wextra -Werror)
  target_include_directories(linux-input-mode-indicator-test PRIVATE src)
  add_test(NAME linux-input-mode-indicator COMMAND linux-input-mode-indicator-test)
  add_executable(linux-menu-labels-test tests/core/menu_labels.cpp)
  target_compile_options(linux-menu-labels-test PRIVATE -Wall -Wextra -Werror -UNDEBUG)
  target_include_directories(linux-menu-labels-test PRIVATE src)
  target_link_libraries(linux-menu-labels-test PRIVATE nlohmann_json::nlohmann_json)
  add_test(NAME linux-menu-labels COMMAND linux-menu-labels-test)
  add_executable(linux-view-composition-test tests/core/view_composition.cpp)
  target_compile_options(linux-view-composition-test PRIVATE -Wall -Wextra -Werror -UNDEBUG)
  target_include_directories(linux-view-composition-test PRIVATE src)
  target_link_libraries(linux-view-composition-test PRIVATE nlohmann_json::nlohmann_json)
  add_test(NAME linux-view-composition COMMAND linux-view-composition-test)
  add_executable(linux-voice-provider-options-test tests/voice/voice_provider_options.cpp)
  target_compile_options(linux-voice-provider-options-test PRIVATE -Wall -Wextra -Werror)
  target_include_directories(linux-voice-provider-options-test PRIVATE src)
  target_link_libraries(linux-voice-provider-options-test PRIVATE nlohmann_json::nlohmann_json)
  add_test(NAME linux-voice-provider-options COMMAND linux-voice-provider-options-test)
  add_executable(linux-prepare-paths-test tests/core/prepare_paths.cpp)
  target_compile_options(linux-prepare-paths-test PRIVATE -Wall -Wextra -Werror)
  target_include_directories(linux-prepare-paths-test PRIVATE src)
  add_test(NAME linux-prepare-paths COMMAND linux-prepare-paths-test)
  add_executable(linux-safe-path-test tests/core/safe_path.cpp)
  target_compile_options(linux-safe-path-test PRIVATE -Wall -Wextra -Werror -UNDEBUG)
  target_include_directories(linux-safe-path-test PRIVATE src)
  add_test(NAME linux-safe-path COMMAND linux-safe-path-test)
  add_executable(linux-key-router-adapter-test tests/input/key_router_adapter.cpp)
  target_compile_options(linux-key-router-adapter-test PRIVATE -Wall -Wextra -Werror)
  target_include_directories(linux-key-router-adapter-test PRIVATE src ../../crates/host-api/include)
  add_test(NAME linux-key-router-adapter COMMAND linux-key-router-adapter-test)
  add_executable(linux-backspace-hold-policy-test tests/input/backspace_hold_policy.cpp)
  target_compile_options(linux-backspace-hold-policy-test PRIVATE -Wall -Wextra -Werror)
  target_include_directories(linux-backspace-hold-policy-test PRIVATE src)
  add_test(NAME linux-backspace-hold-policy COMMAND linux-backspace-hold-policy-test)
  add_executable(linux-caps-lock-tap-test tests/input/caps_lock_tap.cpp)
  target_compile_options(linux-caps-lock-tap-test PRIVATE -Wall -Wextra -Werror)
  target_include_directories(linux-caps-lock-tap-test PRIVATE src)
  add_test(NAME linux-caps-lock-tap COMMAND linux-caps-lock-tap-test)
  add_executable(linux-diagnostic-log-test tests/core/diagnostic_log.cpp)
  target_compile_options(linux-diagnostic-log-test PRIVATE -Wall -Wextra -Werror)
  target_include_directories(linux-diagnostic-log-test PRIVATE src)
  target_sources(linux-diagnostic-log-test PRIVATE src/system/DiagnosticLog.cpp)
  add_test(NAME linux-diagnostic-log COMMAND linux-diagnostic-log-test)
  add_executable(linux-surrounding-characters-test tests/core/surrounding_characters.cpp)
  target_compile_options(linux-surrounding-characters-test PRIVATE -Wall -Wextra -Werror)
  target_include_directories(linux-surrounding-characters-test PRIVATE src)
  target_link_libraries(linux-surrounding-characters-test PRIVATE PkgConfig::IBUS)
  add_test(NAME linux-surrounding-characters COMMAND linux-surrounding-characters-test)
  # 两个前端在组词时都把已选片段放在读音之前；光标越过它时，Fcitx5 按字节计数，IBus 按 Unicode 标量计数。
  # 空格转换、回车提交当前显示内容，是两个前端共用的规则；日文回车因此不能发送引擎的原始输入命令。
  add_executable(linux-japanese-conversion-test tests/core/japanese_conversion.cpp)
  target_compile_options(linux-japanese-conversion-test PRIVATE -Wall -Wextra -Werror -UNDEBUG)
  target_include_directories(linux-japanese-conversion-test PRIVATE src)
  add_test(NAME linux-japanese-conversion COMMAND linux-japanese-conversion-test)
  # 两个菜单对同一设置共用名称表，避免把首右误写成另一家输入法的名字搜狗。
  add_executable(linux-helpcode-schema-names-test tests/core/helpcode_schema_names.cpp)
  target_compile_options(linux-helpcode-schema-names-test PRIVATE -Wall -Wextra -Werror -UNDEBUG)
  target_include_directories(linux-helpcode-schema-names-test PRIVATE src)
  add_test(NAME linux-helpcode-schema-names COMMAND linux-helpcode-schema-names-test)
  # 选了辅助码表包时两个菜单显示插件，在菜单里选内置方案时停用插件，与共享设置页同一约定。
  add_executable(linux-helpcode-pack-test tests/core/helpcode_pack.cpp)
  target_compile_options(linux-helpcode-pack-test PRIVATE -Wall -Wextra -Werror -UNDEBUG)
  target_include_directories(linux-helpcode-pack-test PRIVATE src)
  target_link_libraries(linux-helpcode-pack-test PRIVATE nlohmann_json::nlohmann_json)
  add_test(NAME linux-helpcode-pack COMMAND linux-helpcode-pack-test)
  # 托盘和状态栏菜单切换智能标点时连同两半一起写，与设置页同一约定。
  add_executable(linux-smart-punctuation-choice-test tests/core/smart_punctuation_choice.cpp)
  target_compile_options(linux-smart-punctuation-choice-test PRIVATE -Wall -Wextra -Werror -UNDEBUG)
  target_include_directories(linux-smart-punctuation-choice-test PRIVATE src)
  target_link_libraries(linux-smart-punctuation-choice-test PRIVATE nlohmann_json::nlohmann_json)
  add_test(NAME linux-smart-punctuation-choice COMMAND linux-smart-punctuation-choice-test)
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
  add_test(NAME linux-setup-gnome-wayland
           COMMAND "${Python3_EXECUTABLE}" "${CMAKE_CURRENT_SOURCE_DIR}/tests/core/setup_gnome_wayland.py")
  add_test(NAME linux-setup-linglong
           COMMAND "${Python3_EXECUTABLE}" "${CMAKE_CURRENT_SOURCE_DIR}/tests/core/setup_linglong.py")
  add_test(NAME linux-first-run-guide
           COMMAND "${Python3_EXECUTABLE}" "${CMAKE_CURRENT_SOURCE_DIR}/tests/core/first_run_guide.py")
  add_test(NAME linux-settings-launcher-contract
           COMMAND "${Python3_EXECUTABLE}" "${CMAKE_CURRENT_SOURCE_DIR}/tests/core/settings_launcher_contract.py")
  add_test(NAME linux-deb-maintainer-scripts
           COMMAND "${Python3_EXECUTABLE}" "${CMAKE_CURRENT_SOURCE_DIR}/tests/core/deb_maintainer_scripts.py"
                   "${CMAKE_CURRENT_BINARY_DIR}/debian" "${CMAKE_CURRENT_BINARY_DIR}/uninstall.cmake" "${MSIME_USER_UNIT_LIST}"
                   "${CMAKE_INSTALL_FULL_BINDIR}/${MSIME_EDITION_PACKAGE}-setup")
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
  add_test(NAME linux-voice-provider-catalog
           COMMAND "${Python3_EXECUTABLE}" "${CMAKE_CURRENT_SOURCE_DIR}/tests/voice/provider_catalog.py")
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
  target_link_libraries(linux-typing-statistics-test PRIVATE "${MSIME_HOST_LIBRARY}" Threads::Threads)
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
endif()
# IBus 组件文件以版本的包名命名，组件名和引擎名也随版本（full 仍是 msime-linux.xml）。
msime_edition_source(MSIME_IBUS_COMPONENT_SOURCE data/msime-linux.xml.in)
configure_file("${MSIME_IBUS_COMPONENT_SOURCE}" ${CMAKE_CURRENT_BINARY_DIR}/${MSIME_EDITION_PACKAGE}.xml @ONLY)
if(BUILD_TESTING)
  add_test(NAME linux-ibus-demand-activation
           COMMAND "${Python3_EXECUTABLE}" "${CMAKE_CURRENT_SOURCE_DIR}/tests/runtime/daemon_activation.py"
                   $<TARGET_FILE:msime-linux-ibus> "${CMAKE_CURRENT_BINARY_DIR}/${MSIME_EDITION_PACKAGE}.xml")
  set_tests_properties(linux-ibus-demand-activation PROPERTIES TIMEOUT 30 SKIP_RETURN_CODE 77)
endif()
install(TARGETS msime-linux-ibus msime-linux-dictionary msime-linux-cloud-dictionary msime-linux-cloud-clipboard msime-linux-online msime-linux-translation msime-linux-clipboard
                msime-linux-handwriting msime-linux-voice msime-linux-emoji msime-linux-clipboard-capture msime-linux-prepare
                RUNTIME DESTINATION "${CMAKE_INSTALL_BINDIR}")
if(BUILD_TESTING AND MSIME_LINUX_VOICE)
  install(TARGETS msime-voice-worker-test RUNTIME DESTINATION "${CMAKE_INSTALL_BINDIR}")
endif()
# 带着每用户目录名、单元名或命令名的脚本按版本改写（full 原样安装）；其余脚本只按自己的目录找同级程序，各版本相同。
foreach(MSIME_EDITION_SCRIPT msime-linux-ibus-launcher msime-linux-first-run-guide msime-linux-online-provider msime-linux-provider-session)
  msime_edition_source(MSIME_EDITION_SCRIPT_SOURCE scripts/${MSIME_EDITION_SCRIPT})
  install(PROGRAMS "${MSIME_EDITION_SCRIPT_SOURCE}" DESTINATION "${CMAKE_INSTALL_BINDIR}")
endforeach()
install(PROGRAMS scripts/msime-linux-voice-provider DESTINATION "${CMAKE_INSTALL_BINDIR}")
install(PROGRAMS scripts/msime-linux-clipboard-monitor DESTINATION "${CMAKE_INSTALL_BINDIR}")
# 安装后的首次配置入口。词库与状态目录此前没有任何产品侧的路径：词库要么在配置阶段随包装好，要么只能在仓库检出里跑一个 Cargo 示例去取，于是只拿到安装包的用户凑不齐词库，而没有词库一个候选都出不来。词库锁无条件随装，它是这个入口据以校验和取回的依据——它此前只在 MSIME_ENGINE_RESOURCES 存在时才安装，也就是最需要它的那种安装里反而没有。
# 命令名以版本的包名开头（full 仍是 msime-linux-setup）；随装的词库锁是本版本的锁（full 是 desktop-dictionary.lock.json 本身，其他版本是 resources/editions/<id>.lock.json），装成同一个文件名，msime-linux-setup 据它校验和取回本版本的词库。
msime_edition_source(MSIME_SETUP_SOURCE scripts/msime-linux-setup)
install(PROGRAMS "${MSIME_SETUP_SOURCE}" DESTINATION "${CMAKE_INSTALL_BINDIR}" RENAME ${MSIME_EDITION_PACKAGE}-setup)
if(NOT MSIME_ENGINE_RESOURCES)
  install(FILES "${MSIME_EDITION_RESOURCE_LOCK}"
          DESTINATION "${CMAKE_INSTALL_DATADIR}/${MSIME_CLIENT_DIRECTORY}" RENAME desktop-dictionary.lock.json)
endif()
set(MSIME_SYSTEMD_USER_UNIT_DIR "${MSIME_SYSTEM_DATADIR}/systemd/user" CACHE PATH "systemd user unit installation directory")
foreach(MSIME_PROVIDER online voice clipboard)
  msime_edition_source(MSIME_UNIT_SOURCE data/msime-linux-${MSIME_PROVIDER}.service.in)
  configure_file("${MSIME_UNIT_SOURCE}"
                 ${CMAKE_CURRENT_BINARY_DIR}/${MSIME_EDITION_PACKAGE}-${MSIME_PROVIDER}.service @ONLY)
  install(FILES ${CMAKE_CURRENT_BINARY_DIR}/${MSIME_EDITION_PACKAGE}-${MSIME_PROVIDER}.service
          DESTINATION "${MSIME_SYSTEMD_USER_UNIT_DIR}")
endforeach()
# Starts the clipboard service at login on sessions without graphical-session.target, for users who enabled it; see the comment in the file. Its own component lets the test install just this file.
msime_edition_source(MSIME_CLIPBOARD_AUTOSTART_SOURCE data/msime-linux-clipboard.desktop)
install(FILES "${MSIME_CLIPBOARD_AUTOSTART_SOURCE}" DESTINATION "${MSIME_XDG_AUTOSTART_DIR}"
        RENAME ${MSIME_EDITION_PACKAGE}-clipboard.desktop COMPONENT clipboard-autostart)
# Socket units keep the provider paths present from login, so the input method and the panels find the services the way the Windows server is simply there; systemd starts a provider on its first request.
foreach(MSIME_PROVIDER online voice)
  msime_edition_source(MSIME_UNIT_SOURCE data/msime-linux-${MSIME_PROVIDER}.socket)
  install(FILES "${MSIME_UNIT_SOURCE}" DESTINATION "${MSIME_SYSTEMD_USER_UNIT_DIR}"
          RENAME ${MSIME_EDITION_PACKAGE}-${MSIME_PROVIDER}.socket)
endforeach()
install(FILES scripts/msime_voice_doubao.py scripts/msime_voice_local.py scripts/msime_voice_polish_prompts.py scripts/msime_voice_provider_catalog.py scripts/msime_provider_runtime.py scripts/host_api.py DESTINATION "${CMAKE_INSTALL_BINDIR}")
# The helper and the sherpa-onnx runtime share the host library's private directory: the helper finds the runtime in its own directory, and libsherpa-onnx-c-api.so finds libonnxruntime.so through its $ORIGIN RPATH.
install(TARGETS msime-voice-local RUNTIME DESTINATION "${CMAKE_INSTALL_LIBDIR}/${MSIME_CLIENT_DIRECTORY}")
set(MSIME_VOICE_RUNTIME_DIR "" CACHE PATH "sherpa-onnx runtime for on-device recognition, as unpacked by scripts/fetch_voice_runtime.py --platform linux-<arch>")
set(MSIME_VOICE_RUNTIME_LIBRARIES libsherpa-onnx-c-api.so libonnxruntime.so)
if(MSIME_VOICE_RUNTIME_DIR)
  foreach(MSIME_VOICE_RUNTIME_LIBRARY IN LISTS MSIME_VOICE_RUNTIME_LIBRARIES)
    if(NOT EXISTS "${MSIME_VOICE_RUNTIME_DIR}/${MSIME_VOICE_RUNTIME_LIBRARY}")
      message(FATAL_ERROR "MSIME_VOICE_RUNTIME_DIR has no ${MSIME_VOICE_RUNTIME_LIBRARY}; run scripts/fetch_voice_runtime.py --platform linux-<arch> --out <dir>")
    endif()
    install(FILES "${MSIME_VOICE_RUNTIME_DIR}/${MSIME_VOICE_RUNTIME_LIBRARY}" DESTINATION "${CMAKE_INSTALL_LIBDIR}/${MSIME_CLIENT_DIRECTORY}")
  endforeach()
elseif(MSIME_ENABLE_PACKAGING)
  # A package without the runtime would offer on-device recognition that fails at the first dictation.
  message(FATAL_ERROR "Packaging requires MSIME_VOICE_RUNTIME_DIR; fetch it with scripts/fetch_voice_runtime.py (package-container.sh does)")
endif()
# 上面只核对运行库的文件在不在；这里让构建出的 msime-voice-local 真正打开它（dlopen 并解析全部符号），各打包路线的 ctest 都会跑到。
if(BUILD_TESTING AND MSIME_VOICE_RUNTIME_DIR)
  add_test(NAME linux-voice-local-runtime
           COMMAND "${Python3_EXECUTABLE}" "${CMAKE_CURRENT_SOURCE_DIR}/tests/voice/local_runtime.py"
                   $<TARGET_FILE:msime-voice-local> "${MSIME_VOICE_RUNTIME_DIR}/libsherpa-onnx-c-api.so")
endif()
install(FILES scripts/requirements-voice.txt DESTINATION "${CMAKE_INSTALL_DATADIR}/${MSIME_CLIENT_DIRECTORY}")
file(RELATIVE_PATH MSIME_VOICE_CUE_RELATIVE "${CMAKE_INSTALL_FULL_BINDIR}"
     "${CMAKE_INSTALL_FULL_DATADIR}/${MSIME_CLIENT_DIRECTORY}/voice")
file(RELATIVE_PATH MSIME_VOICE_LOCAL_HELPER_RELATIVE "${CMAKE_INSTALL_FULL_BINDIR}"
     "${CMAKE_INSTALL_FULL_LIBDIR}/${MSIME_CLIENT_DIRECTORY}/msime-voice-local")
get_filename_component(MSIME_HOST_LIBRARY_NAME "${MSIME_HOST_LIBRARY}" NAME)
file(RELATIVE_PATH MSIME_HOST_LIBRARY_RELATIVE "${CMAKE_INSTALL_FULL_BINDIR}"
     "${CMAKE_INSTALL_FULL_LIBDIR}/${MSIME_CLIENT_DIRECTORY}/${MSIME_HOST_LIBRARY_NAME}")
configure_file(data/msime_voice_paths.py.in msime_voice_paths.py @ONLY)
install(FILES "${CMAKE_CURRENT_BINARY_DIR}/msime_voice_paths.py" DESTINATION "${CMAKE_INSTALL_BINDIR}")
install(FILES data/voice/start.pcm data/voice/end.pcm data/voice/SOURCE.md
        DESTINATION "${CMAKE_INSTALL_DATADIR}/${MSIME_CLIENT_DIRECTORY}/voice")
# `msime-linux-handwriting --local` 识别用的 zinnia 模型及其 LGPL-2.1 许可证。它有 26.8 MB，不进版本库：scripts/fetch_handwriting_model.py 把 resources/handwriting-model.lock.json 锁定的文件下载到 target/handwriting-model，存在时这里取用，并逐个文件按锁校验。MSIME_BUNDLE_HANDWRITING_MODEL 打开时（默认如此，packaging/ 下的发行版构建和 nix 都依赖这个默认值）打包必须带上模型，否则包里的离线识别器第一笔就会失败；关闭时（我们自己发布的 deb/rpm，由 package-container.sh 关闭）不安装模型，由设置应用按需下载到用户目录。开发安装没有模型时，只是 --local 报告未安装模型。
# 模型只认汉字：不提供手写的版本（版本表 `features.handwriting` 为 false 的日文、越南文和藏文版）即使下载或指定了模型，也既不装模型也不装它的许可证，打包时也不要求有模型。
# 我们自己发布的 deb/rpm（package-container.sh）传 -DMSIME_BUNDLE_HANDWRITING_MODEL=OFF：包里不带模型和它的许可证，设置应用在需要时按同一份锁把两者下载到用户的状态目录（client-providers 的手写资源包）。发行版仓库的包不传这个选项，照旧随包。
if(NOT MSIME_EDITION_HANDWRITING)
  if(MSIME_HANDWRITING_MODEL_DIR)
    message(STATUS "Edition ${MSIME_EDITION} offers no handwriting; not installing the model in ${MSIME_HANDWRITING_MODEL_DIR}")
  endif()
  set(MSIME_HANDWRITING_MODEL_DIR "")
elseif(NOT MSIME_BUNDLE_HANDWRITING_MODEL)
  if(MSIME_HANDWRITING_MODEL_DIR)
    message(STATUS "MSIME_BUNDLE_HANDWRITING_MODEL is OFF; not installing the model in ${MSIME_HANDWRITING_MODEL_DIR}")
  endif()
  set(MSIME_HANDWRITING_MODEL_DIR "")
elseif(NOT MSIME_HANDWRITING_MODEL_DIR AND
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
            DESTINATION "${CMAKE_INSTALL_DATADIR}/${MSIME_CLIENT_DIRECTORY}/handwriting")
  endforeach()
  install(FILES "${MSIME_HANDWRITING_LOCK_FILE}" DESTINATION "${CMAKE_INSTALL_DATADIR}/${MSIME_CLIENT_DIRECTORY}")
elseif(MSIME_ENABLE_PACKAGING AND MSIME_EDITION_HANDWRITING AND MSIME_BUNDLE_HANDWRITING_MODEL)
  message(FATAL_ERROR "Packaging requires the handwriting model; fetch it with scripts/fetch_handwriting_model.py, or configure with -DMSIME_BUNDLE_HANDWRITING_MODEL=OFF to leave it to the on-demand download")
endif()
# 上面只按锁核对模型的字节；这里用构建出的识别入口真正加载它并识别一笔，各打包路线的 ctest 都会跑到。
# 识别的是复制到构建目录里的一份，不是模型目录里的原件：Nix 的 store 去重会把原件合并成硬链接，而构建沙箱里 store 的属主显示为 nobody，引擎不会信任那个目录里的多链接文件（#6386）。复制出来的是新的单链接文件。
if(BUILD_TESTING AND MSIME_HANDWRITING_MODEL_DIR)
  set(MSIME_HANDWRITING_TEST_MODEL "${CMAKE_CURRENT_BINARY_DIR}/handwriting-test/handwriting-zh_CN.model")
  file(MAKE_DIRECTORY "${CMAKE_CURRENT_BINARY_DIR}/handwriting-test")
  file(COPY_FILE "${MSIME_HANDWRITING_MODEL_DIR}/handwriting-zh_CN.model" "${MSIME_HANDWRITING_TEST_MODEL}"
       ONLY_IF_DIFFERENT)
  add_test(NAME linux-handwriting-local-model
           COMMAND "${Python3_EXECUTABLE}" "${CMAKE_CURRENT_SOURCE_DIR}/tests/provider/handwriting_local_model.py"
                   $<TARGET_FILE:msime-linux-handwriting> "${MSIME_HANDWRITING_TEST_MODEL}")
endif()
if(MSIME_EMOJI_RESOURCES)
  if(NOT IS_DIRECTORY "${MSIME_EMOJI_RESOURCES}")
    message(FATAL_ERROR "MSIME_EMOJI_RESOURCES is not a directory: ${MSIME_EMOJI_RESOURCES}")
  endif()
  install(DIRECTORY "${MSIME_EMOJI_RESOURCES}/"
          DESTINATION "${CMAKE_INSTALL_DATADIR}/${MSIME_CLIENT_DIRECTORY}/emoji")
endif()
# 辅助码表不在词库发布里，由仓库自带在 resources/helpcodes，Engine 从资源目录下的 helpcodes/ 读它们（crates/engine/src/assets.rs 列出七个文件）。宿主的校验放行真实的 helpcodes/ 目录。它们的来源声明与下面其他声明一起安装。
install(FILES
          "${CMAKE_CURRENT_SOURCE_DIR}/../../resources/helpcodes/helpcode.txt"
          "${CMAKE_CURRENT_SOURCE_DIR}/../../resources/helpcodes/zrm_helpcode_big_unique.txt"
          "${CMAKE_CURRENT_SOURCE_DIR}/../../resources/helpcodes/shouyou2_0_helpcode.txt"
          "${CMAKE_CURRENT_SOURCE_DIR}/../../resources/helpcodes/shouyouplus_helpcode.txt"
          "${CMAKE_CURRENT_SOURCE_DIR}/../../resources/helpcodes/xiaohe_helpcode.txt"
          "${CMAKE_CURRENT_SOURCE_DIR}/../../resources/helpcodes/jiajia_helpcode.txt"
          "${CMAKE_CURRENT_SOURCE_DIR}/../../resources/helpcodes/wubi86_helpcode.txt"
        DESTINATION "${CMAKE_INSTALL_DATADIR}/${MSIME_CLIENT_DIRECTORY}/resources/helpcodes")
if(MSIME_ENGINE_RESOURCES)
  if(NOT IS_DIRECTORY "${MSIME_ENGINE_RESOURCES}")
    message(FATAL_ERROR "MSIME_ENGINE_RESOURCES is not a directory: ${MSIME_ENGINE_RESOURCES}")
  endif()
  file(READ "${MSIME_EDITION_RESOURCE_LOCK}" MSIME_ENGINE_RESOURCE_LOCK)
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
            DESTINATION "${CMAKE_INSTALL_DATADIR}/${MSIME_CLIENT_DIRECTORY}/resources")
  endforeach()
  # Keep the reviewed lock beside the installed bundle as provenance metadata;
  # it is not copied into the runtime directory because the Engine verifier
  # requires that directory to contain exactly the pinned artifacts.
  install(FILES "${MSIME_EDITION_RESOURCE_LOCK}"
          DESTINATION "${CMAKE_INSTALL_DATADIR}/${MSIME_CLIENT_DIRECTORY}" RENAME desktop-dictionary.lock.json)
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
# 不带中文主词库的版本（cmake/Edition.cmake 的 MSIME_EDITION_CHINESE_MAIN）没有中文整句可以重排，不装这个模型，也不从暂存目录捡。
if(NOT MSIME_EDITION_CHINESE_MAIN)
  if(MSIME_SETTLED_MODEL)
    message(STATUS "Edition ${MSIME_EDITION} has no Chinese main dictionary; not installing the settled model")
  endif()
  set(MSIME_SETTLED_MODEL "")
elseif(NOT MSIME_SETTLED_MODEL AND
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
            DESTINATION "${CMAKE_INSTALL_DATADIR}/${MSIME_CLIENT_DIRECTORY}/settled-model")
  endforeach()
  install(FILES "${CMAKE_CURRENT_SOURCE_DIR}/../../resources/settled-model.lock.json"
          DESTINATION "${CMAKE_INSTALL_DATADIR}/${MSIME_CLIENT_DIRECTORY}")
  install(FILES "${CMAKE_CURRENT_SOURCE_DIR}/../../resources/neural-model.lock.json"
          DESTINATION "${CMAKE_INSTALL_DATADIR}/${MSIME_CLIENT_DIRECTORY}")
endif()
# Offline glosses for non-English translation targets, one zh-<lang>.db per language built by `scripts/build_offline_glosses.py`, installed as a sibling of the resource bundle for the same reason as the settled model; the Engine looks for them there. Optional: without them the candidate glosses stay English only. Their notice is required with them, since the databases are CC BY-SA and must travel with their attribution.
set(MSIME_OFFLINE_GLOSSES "" CACHE PATH "Offline gloss databases with their notice, e.g. target/offline-glosses")
# 离线释义按中文候选查：不提供中文方案的版本（版本表 `features.offline_glosses` 为 false 的日文、越南文和藏文版）即使指定了 `MSIME_OFFLINE_GLOSSES` 也不装。
if(MSIME_OFFLINE_GLOSSES AND NOT MSIME_EDITION_OFFLINE_GLOSSES)
  message(STATUS "Edition ${MSIME_EDITION} offers no Chinese scheme; not installing the offline glosses in ${MSIME_OFFLINE_GLOSSES}")
elseif(MSIME_OFFLINE_GLOSSES)
  if(NOT EXISTS "${MSIME_OFFLINE_GLOSSES}/offline-glosses-NOTICE.txt")
    message(FATAL_ERROR "MSIME_OFFLINE_GLOSSES has no offline-glosses-NOTICE.txt: ${MSIME_OFFLINE_GLOSSES}")
  endif()
  file(GLOB MSIME_OFFLINE_GLOSS_FILES LIST_DIRECTORIES false "${MSIME_OFFLINE_GLOSSES}/zh-*.db")
  if(NOT MSIME_OFFLINE_GLOSS_FILES)
    message(FATAL_ERROR "MSIME_OFFLINE_GLOSSES has no zh-*.db: ${MSIME_OFFLINE_GLOSSES}")
  endif()
  install(FILES ${MSIME_OFFLINE_GLOSS_FILES} "${MSIME_OFFLINE_GLOSSES}/offline-glosses-NOTICE.txt"
          DESTINATION "${CMAKE_INSTALL_DATADIR}/${MSIME_CLIENT_DIRECTORY}/offline-glosses")
endif()
# 背单词的内置词书（scripts/fetch_wordbooks.py 从 ECDICT 生成），装在资源目录的同级 wordbooks：设置窗口按 HostOptions 求随包资源的目录（host-api 的 bundled_resource_root：resources 的上一级，词库在线更新后是原安装包目录的上一级）。只随许可声明 wordbooks-NOTICE.txt 一起装；没有时背单词只列用户导入的词表。词书不分版本。没指定时从 target/wordbooks 捡；MSIME_REQUIRE_WORDBOOKS 让缺了 resources/wordbook.lock.json 列出的任何一本的构建失败。
set(MSIME_WORDBOOKS "" CACHE PATH "Bundled 背单词 wordbooks with their notice, e.g. target/wordbooks")
option(MSIME_REQUIRE_WORDBOOKS "Fail unless every wordbook resources/wordbook.lock.json lists is installed" OFF)
if(NOT MSIME_WORDBOOKS AND IS_DIRECTORY "${CMAKE_CURRENT_SOURCE_DIR}/../../target/wordbooks")
  get_filename_component(MSIME_WORDBOOKS "${CMAKE_CURRENT_SOURCE_DIR}/../../target/wordbooks" ABSOLUTE)
  message(STATUS "Using wordbook staging directory: ${MSIME_WORDBOOKS}")
endif()
if(MSIME_WORDBOOKS)
  if(NOT EXISTS "${MSIME_WORDBOOKS}/wordbooks-NOTICE.txt")
    message(FATAL_ERROR "MSIME_WORDBOOKS has no wordbooks-NOTICE.txt: ${MSIME_WORDBOOKS}")
  endif()
  file(GLOB MSIME_WORDBOOK_FILES LIST_DIRECTORIES false "${MSIME_WORDBOOKS}/*.json")
  if(NOT MSIME_WORDBOOK_FILES)
    message(FATAL_ERROR "MSIME_WORDBOOKS has no *.json wordbooks: ${MSIME_WORDBOOKS}")
  endif()
  install(FILES ${MSIME_WORDBOOK_FILES} "${MSIME_WORDBOOKS}/wordbooks-NOTICE.txt"
          DESTINATION "${CMAKE_INSTALL_DATADIR}/${MSIME_CLIENT_DIRECTORY}/wordbooks")
else()
  message(STATUS "No wordbooks; 背单词 offers only imported books")
endif()
if(MSIME_REQUIRE_WORDBOOKS)
  if(NOT MSIME_WORDBOOKS)
    message(FATAL_ERROR "MSIME_REQUIRE_WORDBOOKS is ON but no wordbooks were found; run python3 scripts/fetch_wordbooks.py or set MSIME_WORDBOOKS")
  endif()
  file(READ "${CMAKE_CURRENT_SOURCE_DIR}/../../resources/wordbook.lock.json" MSIME_WORDBOOK_LOCK_JSON)
  string(JSON MSIME_WORDBOOK_LOCK_LENGTH LENGTH "${MSIME_WORDBOOK_LOCK_JSON}" books)
  if(MSIME_WORDBOOK_LOCK_LENGTH EQUAL 0)
    message(FATAL_ERROR "MSIME_REQUIRE_WORDBOOKS is ON but resources/wordbook.lock.json lists no books")
  endif()
  math(EXPR MSIME_WORDBOOK_LOCK_LAST "${MSIME_WORDBOOK_LOCK_LENGTH} - 1")
  foreach(MSIME_WORDBOOK_LOCK_INDEX RANGE ${MSIME_WORDBOOK_LOCK_LAST})
    string(JSON MSIME_WORDBOOK_ID GET "${MSIME_WORDBOOK_LOCK_JSON}" books ${MSIME_WORDBOOK_LOCK_INDEX} id)
    if(NOT EXISTS "${MSIME_WORDBOOKS}/${MSIME_WORDBOOK_ID}.json")
      message(FATAL_ERROR "MSIME_REQUIRE_WORDBOOKS is ON but ${MSIME_WORDBOOKS} has no ${MSIME_WORDBOOK_ID}.json")
    endif()
  endforeach()
endif()
# The Cantonese, Zhuyin and Stroke dictionaries fetched by scripts/fetch_language_dictionaries.py (or built by `msime-dict-build languages`), installed as the `language-dictionaries` sibling of the resource bundle for the same reason as the settled model: the resource directory must match the dictionary lock exactly. msime_client_refresh_host names the directory in the runtime options when the host starts, and a scheme whose dictionary is missing is shown as unavailable and falls back. Optional, picked up from target/language-dictionaries when present; each dictionary is installed only with its licence text, which must travel with the data. MSIME_REQUIRE_LANGUAGE_DICTIONARIES makes a package fail instead of shipping without any dictionary the lock pins.
set(MSIME_LANGUAGE_DICTIONARIES "" CACHE PATH "Cantonese, Zhuyin and Stroke dictionaries with their licences, e.g. target/language-dictionaries")
option(MSIME_REQUIRE_LANGUAGE_DICTIONARIES "Fail unless every dictionary resources/language-dictionaries.lock.json pins is installed" OFF)
# 不用粤语、注音和笔画的版本（版本表的 language_dictionaries 为空）不装这些词库，也不从暂存目录捡。
if(NOT MSIME_EDITION_LANGUAGE_DICTIONARIES)
  set(MSIME_LANGUAGE_DICTIONARIES "")
elseif(NOT MSIME_LANGUAGE_DICTIONARIES AND
   IS_DIRECTORY "${CMAKE_CURRENT_SOURCE_DIR}/../../target/language-dictionaries")
  get_filename_component(MSIME_LANGUAGE_DICTIONARIES
                         "${CMAKE_CURRENT_SOURCE_DIR}/../../target/language-dictionaries" ABSOLUTE)
  message(STATUS "Using language dictionary staging directory: ${MSIME_LANGUAGE_DICTIONARIES}")
endif()
set(MSIME_LANGUAGE_DICTIONARY_COUNT 0)
set(MSIME_LANGUAGE_DICTIONARIES_INSTALLED)
if(MSIME_LANGUAGE_DICTIONARIES)
  if(NOT IS_DIRECTORY "${MSIME_LANGUAGE_DICTIONARIES}")
    message(FATAL_ERROR "MSIME_LANGUAGE_DICTIONARIES is not a directory: ${MSIME_LANGUAGE_DICTIONARIES}")
  endif()
  foreach(MSIME_LANGUAGE_PAIR "msime-cantonese.db:msime-rime_cantonese_LICENSE.txt" "msime-zhuyin.db:msime-libchewing_data_LICENSE.txt"
          "msime-stroke.db:msime-rime_stroke_LICENSE.txt")
    string(REPLACE ":" ";" MSIME_LANGUAGE_PAIR "${MSIME_LANGUAGE_PAIR}")
    list(GET MSIME_LANGUAGE_PAIR 0 MSIME_LANGUAGE_DATABASE)
    list(GET MSIME_LANGUAGE_PAIR 1 MSIME_LANGUAGE_LICENSE)
    set(MSIME_LANGUAGE_DATABASE_FILE "${MSIME_LANGUAGE_DICTIONARIES}/${MSIME_LANGUAGE_DATABASE}")
    if(NOT EXISTS "${MSIME_LANGUAGE_DATABASE_FILE}" OR IS_DIRECTORY "${MSIME_LANGUAGE_DATABASE_FILE}")
      continue()
    endif()
    if(NOT EXISTS "${MSIME_LANGUAGE_DICTIONARIES}/${MSIME_LANGUAGE_LICENSE}")
      message(FATAL_ERROR "${MSIME_LANGUAGE_DATABASE_FILE} has no ${MSIME_LANGUAGE_LICENSE} beside it; refusing to install the data without its licence")
    endif()
    install(FILES "${MSIME_LANGUAGE_DATABASE_FILE}" "${MSIME_LANGUAGE_DICTIONARIES}/${MSIME_LANGUAGE_LICENSE}"
            DESTINATION "${CMAKE_INSTALL_DATADIR}/${MSIME_CLIENT_DIRECTORY}/language-dictionaries")
    math(EXPR MSIME_LANGUAGE_DICTIONARY_COUNT "${MSIME_LANGUAGE_DICTIONARY_COUNT} + 1")
    list(APPEND MSIME_LANGUAGE_DICTIONARIES_INSTALLED "${MSIME_LANGUAGE_DATABASE}")
  endforeach()
endif()
if(MSIME_LANGUAGE_DICTIONARY_COUNT EQUAL 0)
  message(STATUS "No language dictionaries; Cantonese, Zhuyin and Stroke stay unavailable")
endif()
# A release requires every dictionary resources/language-dictionaries.lock.json pins, not a fixed list: a dictionary that has not been released yet is installed when present but cannot fail a release, and the lock bump that publishes it makes it required.
if(MSIME_REQUIRE_LANGUAGE_DICTIONARIES)
  set(MSIME_LANGUAGE_DICTIONARIES_LOCK "${CMAKE_CURRENT_SOURCE_DIR}/../../resources/language-dictionaries.lock.json")
  if(NOT EXISTS "${MSIME_LANGUAGE_DICTIONARIES_LOCK}")
    message(FATAL_ERROR "MSIME_REQUIRE_LANGUAGE_DICTIONARIES is ON but ${MSIME_LANGUAGE_DICTIONARIES_LOCK} does not exist")
  endif()
  file(READ "${MSIME_LANGUAGE_DICTIONARIES_LOCK}" MSIME_LANGUAGE_LOCK_JSON)
  string(JSON MSIME_LANGUAGE_LOCK_LENGTH LENGTH "${MSIME_LANGUAGE_LOCK_JSON}" artifacts)
  set(MSIME_LANGUAGE_DICTIONARIES_PINNED)
  if(MSIME_LANGUAGE_LOCK_LENGTH GREATER 0)
    math(EXPR MSIME_LANGUAGE_LOCK_LAST "${MSIME_LANGUAGE_LOCK_LENGTH} - 1")
    foreach(MSIME_LANGUAGE_LOCK_INDEX RANGE ${MSIME_LANGUAGE_LOCK_LAST})
      string(JSON MSIME_LANGUAGE_LOCK_NAME GET "${MSIME_LANGUAGE_LOCK_JSON}" artifacts ${MSIME_LANGUAGE_LOCK_INDEX} name)
      if(MSIME_LANGUAGE_LOCK_NAME MATCHES "\\.db$")
        list(APPEND MSIME_LANGUAGE_DICTIONARIES_PINNED "${MSIME_LANGUAGE_LOCK_NAME}")
      endif()
    endforeach()
  endif()
  if(NOT MSIME_LANGUAGE_DICTIONARIES_PINNED)
    message(FATAL_ERROR "MSIME_REQUIRE_LANGUAGE_DICTIONARIES is ON but ${MSIME_LANGUAGE_DICTIONARIES_LOCK} pins no dictionary")
  endif()
  foreach(MSIME_LANGUAGE_DATABASE IN LISTS MSIME_LANGUAGE_DICTIONARIES_PINNED)
    if(NOT MSIME_LANGUAGE_DATABASE IN_LIST MSIME_LANGUAGE_DICTIONARIES_INSTALLED)
      message(FATAL_ERROR "MSIME_REQUIRE_LANGUAGE_DICTIONARIES is ON but ${MSIME_LANGUAGE_DATABASE}, pinned by resources/language-dictionaries.lock.json, was not found in '${MSIME_LANGUAGE_DICTIONARIES}'")
    endif()
  endforeach()
endif()
if(MSIME_DESKTOP_BINARY)
  if(NOT EXISTS "${MSIME_DESKTOP_BINARY}")
    message(FATAL_ERROR "MSIME_DESKTOP_BINARY does not exist: ${MSIME_DESKTOP_BINARY}")
  endif()
  # Quote the configured path as one POSIX shell word, including apostrophes.
  string(REPLACE "'" "'\\''" MSIME_SETTINGS_SYSTEM_CONFIG
         "${CMAKE_INSTALL_FULL_SYSCONFDIR}/${MSIME_CLIENT_DIRECTORY}/runtime-options.json")
  # 设置命令和桌面入口以版本的包名命名（full 仍是 msime-linux-settings 和 msime-linux.desktop）。
  msime_edition_source(MSIME_SETTINGS_SOURCE data/msime-linux-settings.in)
  msime_edition_source(MSIME_DESKTOP_ENTRY_SOURCE data/msime-linux.desktop.in)
  configure_file("${MSIME_SETTINGS_SOURCE}" ${CMAKE_CURRENT_BINARY_DIR}/${MSIME_EDITION_PACKAGE}-settings @ONLY)
  configure_file("${MSIME_DESKTOP_ENTRY_SOURCE}" ${CMAKE_CURRENT_BINARY_DIR}/${MSIME_EDITION_PACKAGE}.desktop @ONLY)
  install(PROGRAMS "${MSIME_DESKTOP_BINARY}" DESTINATION "${CMAKE_INSTALL_BINDIR}" RENAME msime-linux-desktop)
  install(PROGRAMS ${CMAKE_CURRENT_BINARY_DIR}/${MSIME_EDITION_PACKAGE}-settings DESTINATION "${CMAKE_INSTALL_BINDIR}")
  install(FILES ${CMAKE_CURRENT_BINARY_DIR}/${MSIME_EDITION_PACKAGE}.desktop DESTINATION "${MSIME_SYSTEM_DATADIR}/applications")
  # The Omarchy theme-set hook runs msime-linux-settings, so it ships with it; msime-linux-setup links it into ~/.config/omarchy/hooks/theme-set.d on an Omarchy desktop.
  msime_edition_source(MSIME_OMARCHY_SOURCE data/omarchy/theme-set.in)
  configure_file("${MSIME_OMARCHY_SOURCE}" ${CMAKE_CURRENT_BINARY_DIR}/omarchy/theme-set @ONLY)
  install(PROGRAMS ${CMAKE_CURRENT_BINARY_DIR}/omarchy/theme-set DESTINATION "${CMAKE_INSTALL_DATADIR}/${MSIME_CLIENT_DIRECTORY}/omarchy")
  # The bar widget and its menu stand in for the tray Omarchy disables; the menu opens msime-linux-settings panels, so both ship with it too. msime-linux-setup links the plugin directory into ~/.config/omarchy/plugins.
  msime_edition_source(MSIME_OMARCHY_SOURCE data/omarchy/menu.in)
  configure_file("${MSIME_OMARCHY_SOURCE}" ${CMAKE_CURRENT_BINARY_DIR}/omarchy/menu @ONLY)
  msime_edition_source(MSIME_OMARCHY_SOURCE data/omarchy/plugin/Widget.qml.in)
  configure_file("${MSIME_OMARCHY_SOURCE}" ${CMAKE_CURRENT_BINARY_DIR}/omarchy/plugin/Widget.qml @ONLY)
  msime_edition_source(MSIME_OMARCHY_MANIFEST_SOURCE data/omarchy/plugin/manifest.json)
  install(PROGRAMS ${CMAKE_CURRENT_BINARY_DIR}/omarchy/menu DESTINATION "${CMAKE_INSTALL_DATADIR}/${MSIME_CLIENT_DIRECTORY}/omarchy")
  install(FILES "${MSIME_OMARCHY_MANIFEST_SOURCE}" ${CMAKE_CURRENT_BINARY_DIR}/omarchy/plugin/Widget.qml
          DESTINATION "${CMAKE_INSTALL_DATADIR}/${MSIME_CLIENT_DIRECTORY}/omarchy/plugin")
endif()
# Nothing here starts the MCP server: an agent's MCP configuration names the command and runs it over stdio, so it only has to be on PATH.
if(MSIME_MCP_BINARY)
  if(NOT EXISTS "${MSIME_MCP_BINARY}")
    message(FATAL_ERROR "MSIME_MCP_BINARY does not exist: ${MSIME_MCP_BINARY}")
  endif()
  install(PROGRAMS "${MSIME_MCP_BINARY}" DESTINATION "${CMAKE_INSTALL_BINDIR}")
  # full 再以 msime 的名字放一个指向它的相对链接，供在终端里手动敲 msime expand、msime config，与 macOS 的 Homebrew cask 相同。其他版本装在自己的前缀下，那里的 bin 不在 PATH 上，同名的命令也只能有一个，留给 full。
  if(MSIME_EDITION_IS_FULL)
    file(MAKE_DIRECTORY "${CMAKE_CURRENT_BINARY_DIR}/mcp")
    file(CREATE_LINK msime-mcp "${CMAKE_CURRENT_BINARY_DIR}/mcp/msime" SYMBOLIC)
    install(FILES "${CMAKE_CURRENT_BINARY_DIR}/mcp/msime" DESTINATION "${CMAKE_INSTALL_BINDIR}")
  endif()
endif()
# 内置音效包，由 `scripts/generate_sound_packs.py` 合成并贡献到公有领域，装在资源目录旁边而不是里面：资源目录必须恰好只放词库锁固定的内容。各宿主和设置窗口把这个目录作为内置包根目录告诉 client-services。
install(DIRECTORY "${CMAKE_CURRENT_SOURCE_DIR}/../../resources/sound-packs/"
        DESTINATION "${CMAKE_INSTALL_DATADIR}/${MSIME_CLIENT_DIRECTORY}/sound-packs")
# 随桌面安装包分发的官方插件包（plugins/official-packs.json 登记的脚本插件和短语表），由 scripts/build-official-plugins.sh 构建到 target/plugin-packs，装在内置音效包旁边：IBus 和 Fcitx5 只传 sound_packs，host-api 和设置窗口从那里推出 plugin-packs。选项给了才装：发布页的 deb、rpm 由 package-container.sh 传它；从源码构建的发行版定义（Arch、Gentoo、nix、OBS）不跑 wasm 构建，不带官方包。
set(MSIME_PLUGIN_PACKS "" CACHE PATH "Official plugin packs built by scripts/build-official-plugins.sh, e.g. target/plugin-packs")
if(MSIME_PLUGIN_PACKS)
  if(NOT EXISTS "${MSIME_PLUGIN_PACKS}/script" OR NOT EXISTS "${MSIME_PLUGIN_PACKS}/phrase_table")
    message(FATAL_ERROR "MSIME_PLUGIN_PACKS holds no official plugin packs: ${MSIME_PLUGIN_PACKS}")
  endif()
  install(DIRECTORY "${MSIME_PLUGIN_PACKS}/"
          DESTINATION "${CMAKE_INSTALL_DATADIR}/${MSIME_CLIENT_DIRECTORY}/plugin-packs")
endif()
# 桌面入口的 Icon=msime-linux、Fcitx5 输入法条目的 Icon 和 IBus 组件的 <icon> 都按名字
# 解析，所以这些文件决定了托盘、输入法列表和启动器上显示的是 logo 还是占位图。它们此前
# 装在桌面二进制那个分支里，于是只装输入法、不装设置窗口的安装一张图标都没有，而那恰恰
# 是最小可用的安装方式。
# 所有版本用同一个图标，其他版本把同样的文件装成以自己包名命名的图标，几个版本的包才不会争同一个文件。
if(MSIME_EDITION_IS_FULL)
  install(DIRECTORY "${CMAKE_CURRENT_SOURCE_DIR}/data/icons/"
          DESTINATION "${CMAKE_INSTALL_DATADIR}/icons")
else()
  file(GLOB MSIME_ICON_FILES RELATIVE "${CMAKE_CURRENT_SOURCE_DIR}/data/icons"
       "${CMAKE_CURRENT_SOURCE_DIR}/data/icons/hicolor/*/apps/msime-linux.*")
  foreach(MSIME_ICON_FILE IN LISTS MSIME_ICON_FILES)
    get_filename_component(MSIME_ICON_DIRECTORY "${MSIME_ICON_FILE}" DIRECTORY)
    get_filename_component(MSIME_ICON_EXTENSION "${MSIME_ICON_FILE}" LAST_EXT)
    install(FILES "${CMAKE_CURRENT_SOURCE_DIR}/data/icons/${MSIME_ICON_FILE}"
            DESTINATION "${MSIME_SYSTEM_DATADIR}/icons/${MSIME_ICON_DIRECTORY}"
            RENAME "${MSIME_EDITION_PACKAGE}${MSIME_ICON_EXTENSION}")
  endforeach()
endif()
configure_file(cmake/icon-cache.cmake.in ${CMAKE_CURRENT_BINARY_DIR}/icon-cache.cmake @ONLY)
install(SCRIPT ${CMAKE_CURRENT_BINARY_DIR}/icon-cache.cmake)
if(MSIME_RUNTIME_OPTIONS_FILE)
 install(FILES "${MSIME_RUNTIME_OPTIONS_FILE}" DESTINATION "${CMAKE_INSTALL_SYSCONFDIR}/${MSIME_CLIENT_DIRECTORY}" RENAME runtime-options.json)
endif()
install(FILES ${CMAKE_CURRENT_BINARY_DIR}/${MSIME_EDITION_PACKAGE}.xml DESTINATION "${MSIME_SYSTEM_DATADIR}/ibus/component")
# 其他版本在前缀的 bin 目录里声明自己的版本（crates/client-core 的 Edition::of_linux_package），设置应用和 msime-mcp 据此找到本版本的状态目录和 socket；full 不带这个文件。
if(NOT MSIME_EDITION_IS_FULL)
  execute_process(
    COMMAND "${Python3_EXECUTABLE}" "${CMAKE_CURRENT_SOURCE_DIR}/scripts/edition_linux.py"
            marker --edition "${MSIME_EDITION}" --output "${CMAKE_CURRENT_BINARY_DIR}/edition/edition.json"
    RESULT_VARIABLE MSIME_EDITION_MARKER_RESULT)
  if(NOT MSIME_EDITION_MARKER_RESULT EQUAL 0)
    message(FATAL_ERROR "Cannot write the edition marker for ${MSIME_EDITION}")
  endif()
  install(FILES "${CMAKE_CURRENT_BINARY_DIR}/edition/edition.json" DESTINATION "${CMAKE_INSTALL_BINDIR}")
endif()
# 装在自己前缀下的版本在 /usr/bin 放两个指向前缀的命令（<package>-setup、<package>-settings），用户和桌面入口不必知道前缀在哪。符号链接按原样安装；链接目标是前缀里的绝对路径。
if(MSIME_EDITION_SYSTEM_PATHS)
  set(MSIME_USER_COMMANDS setup)
  if(MSIME_DESKTOP_BINARY)
    list(APPEND MSIME_USER_COMMANDS settings)
  endif()
  file(MAKE_DIRECTORY "${CMAKE_CURRENT_BINARY_DIR}/edition/usr-bin")
  foreach(MSIME_USER_COMMAND IN LISTS MSIME_USER_COMMANDS)
    file(CREATE_LINK "${CMAKE_INSTALL_FULL_BINDIR}/${MSIME_EDITION_PACKAGE}-${MSIME_USER_COMMAND}"
         "${CMAKE_CURRENT_BINARY_DIR}/edition/usr-bin/${MSIME_EDITION_PACKAGE}-${MSIME_USER_COMMAND}" SYMBOLIC)
    install(FILES "${CMAKE_CURRENT_BINARY_DIR}/edition/usr-bin/${MSIME_EDITION_PACKAGE}-${MSIME_USER_COMMAND}"
            DESTINATION "/usr/bin")
  endforeach()
endif()
install(FILES "${MSIME_HOST_LIBRARY}" DESTINATION "${CMAKE_INSTALL_LIBDIR}/${MSIME_CLIENT_DIRECTORY}")
file(RELATIVE_PATH MSIME_HOST_LIBRARY_RELATIVE_DIR
  "${CMAKE_INSTALL_FULL_BINDIR}" "${CMAKE_INSTALL_FULL_LIBDIR}/${MSIME_CLIENT_DIRECTORY}")
set_target_properties(msime-linux-ibus msime-linux-dictionary msime-linux-cloud-dictionary msime-linux-cloud-clipboard msime-linux-online msime-linux-translation msime-linux-handwriting
                      msime-linux-voice msime-linux-emoji msime-linux-clipboard-capture msime-linux-prepare PROPERTIES
  INSTALL_RPATH "$ORIGIN/${MSIME_HOST_LIBRARY_RELATIVE_DIR}")

if(BUILD_TESTING)
  add_executable(linux-native-compose-test tests/input/native_compose.cpp)
  add_executable(linux-wave-overlay-model-test
    tests/runtime/wave_overlay_model.cpp src/overlay/WaveOverlayIbusSurface.cpp
    src/overlay/WaveOverlaySurfaceFactory.cpp)
  target_compile_features(linux-wave-overlay-model-test PRIVATE cxx_std_17)
  target_compile_options(linux-wave-overlay-model-test PRIVATE -Wall -Wextra -Werror -UNDEBUG)
  target_include_directories(linux-wave-overlay-model-test PRIVATE src)
  target_link_libraries(linux-wave-overlay-model-test PRIVATE PkgConfig::IBUS)
  if(MSIME_WAVE_X11_SURFACE)
    target_sources(linux-wave-overlay-model-test PRIVATE src/overlay/WaveOverlayX11Surface.cpp)
    target_compile_definitions(linux-wave-overlay-model-test PRIVATE MSIME_LINUX_HAS_X11_OVERLAY)
    target_link_libraries(linux-wave-overlay-model-test PRIVATE PkgConfig::X11_SURFACE PkgConfig::X11_OVERLAY_DRAW)
  endif()
  if(MSIME_WAVE_WAYLAND_SURFACE)
    target_sources(linux-wave-overlay-model-test PRIVATE
                   src/overlay/WaveOverlayWaylandSurface.cpp "${MSIME_LAYER_SHELL_CODE}"
                   ${MSIME_XDG_SHELL_CODE})
    target_include_directories(linux-wave-overlay-model-test PRIVATE
                               "${MSIME_WAYLAND_PROTOCOL_DIR}")
    target_compile_definitions(linux-wave-overlay-model-test PRIVATE
                               MSIME_LINUX_HAS_WAYLAND_SURFACE)
    target_link_libraries(linux-wave-overlay-model-test PRIVATE
                          PkgConfig::WAYLAND_CLIENT PkgConfig::WAYLAND_TEXT ${MSIME_SHM_LIBRARIES})
  endif()
  add_test(NAME linux-wave-overlay-model COMMAND linux-wave-overlay-model-test)
  # 共用的绘制：布局尺寸、失败提示胶囊、只留最新三行的转写文字和画出的面板，用真实的 Pango 测量。设置 `MSIME_TEST_CANVAS_ARTIFACTS` 时把每种呈现存成 PNG，供目视检查。
  if(WAYLAND_TEXT_FOUND)
    add_executable(linux-wave-overlay-painter-test tests/runtime/wave_overlay_painter.cpp)
    target_compile_features(linux-wave-overlay-painter-test PRIVATE cxx_std_17)
    target_compile_options(linux-wave-overlay-painter-test PRIVATE -Wall -Wextra -Werror -UNDEBUG)
    target_include_directories(linux-wave-overlay-painter-test PRIVATE src)
    target_link_libraries(linux-wave-overlay-painter-test PRIVATE PkgConfig::WAYLAND_TEXT)
    add_test(NAME linux-wave-overlay-painter COMMAND linux-wave-overlay-painter-test)
  endif()
  # 此测试检查 X11 的定位并读取 RandR 头文件，仅在构建 X11 浮层时启用。
  if(X11_SURFACE_FOUND)
    add_executable(linux-wave-overlay-placement-test tests/runtime/wave_overlay_placement.cpp)
    target_compile_features(linux-wave-overlay-placement-test PRIVATE cxx_std_17)
    target_compile_options(linux-wave-overlay-placement-test PRIVATE -Wall -Wextra -Werror -UNDEBUG)
    target_include_directories(linux-wave-overlay-placement-test PRIVATE src)
    target_link_libraries(linux-wave-overlay-placement-test PRIVATE nlohmann_json::nlohmann_json)
    add_test(NAME linux-wave-overlay-placement COMMAND linux-wave-overlay-placement-test)
  endif()
  add_executable(linux-mode-badge-style-test tests/runtime/mode_badge_style.cpp)
  target_compile_features(linux-mode-badge-style-test PRIVATE cxx_std_17)
  target_compile_options(linux-mode-badge-style-test PRIVATE -Wall -Wextra -Werror)
  target_include_directories(linux-mode-badge-style-test PRIVATE src)
  target_link_libraries(linux-mode-badge-style-test PRIVATE nlohmann_json::nlohmann_json)
  add_test(NAME linux-mode-badge-style COMMAND linux-mode-badge-style-test)
  # 用真实的 cairo 与 pango 画一遍徽章，只在有 pangocairo 的构建里跑。
  if(WAYLAND_TEXT_FOUND)
    add_executable(linux-mode-badge-render-test tests/runtime/mode_badge_render.cpp)
    target_compile_features(linux-mode-badge-render-test PRIVATE cxx_std_17)
    target_compile_options(linux-mode-badge-render-test PRIVATE -Wall -Wextra -Werror -UNDEBUG)
    target_include_directories(linux-mode-badge-render-test PRIVATE src)
    target_link_libraries(linux-mode-badge-render-test PRIVATE PkgConfig::WAYLAND_TEXT nlohmann_json::nlohmann_json)
    add_test(NAME linux-mode-badge-render COMMAND linux-mode-badge-render-test)
  endif()
  # 双拼键位图和悬浮工具栏：纯计算的规则、几何和控制逻辑（假的事件循环和后端），不需要显示服务器。
  add_executable(linux-floating-panels-model-test tests/runtime/floating_panels_model.cpp src/overlay/FloatingToolbar.cpp)
  target_compile_features(linux-floating-panels-model-test PRIVATE cxx_std_17)
  target_compile_options(linux-floating-panels-model-test PRIVATE -Wall -Wextra -Werror -UNDEBUG)
  target_include_directories(linux-floating-panels-model-test PRIVATE src)
  target_link_libraries(linux-floating-panels-model-test PRIVATE nlohmann_json::nlohmann_json)
  add_test(NAME linux-floating-panels-model COMMAND linux-floating-panels-model-test)
  if(MODE_BADGE_X11_FOUND)
    # 用真实的 cairo 与 pango 画一遍键位图、工具栏、菜单和提示。
    add_executable(linux-floating-panels-render-test tests/runtime/floating_panels_render.cpp)
    target_compile_features(linux-floating-panels-render-test PRIVATE cxx_std_17)
    target_compile_options(linux-floating-panels-render-test PRIVATE -Wall -Wextra -Werror -UNDEBUG)
    target_include_directories(linux-floating-panels-render-test PRIVATE src)
    target_link_libraries(linux-floating-panels-render-test PRIVATE PkgConfig::MODE_BADGE_X11 nlohmann_json::nlohmann_json)
    add_test(NAME linux-floating-panels-render COMMAND linux-floating-panels-render-test)
    # 在 Xvfb 里真的建出窗口：工具栏的位置、按钮、提示、菜单、拖动和位置记忆，键位图的摆放和翻转。
    if(MSIME_XVFB_RUN)
      add_executable(linux-floating-panels-x11-test tests/runtime/floating_panels_x11.cpp
        src/overlay/OverlayX11Window.cpp src/overlay/ShuangpinKeymapX11Surface.cpp src/overlay/FloatingToolbar.cpp
        src/overlay/FloatingToolbarFactory.cpp src/overlay/FloatingToolbarX11Surface.cpp)
      target_compile_features(linux-floating-panels-x11-test PRIVATE cxx_std_17)
      target_compile_options(linux-floating-panels-x11-test PRIVATE -Wall -Wextra -Werror)
      target_include_directories(linux-floating-panels-x11-test PRIVATE src ../../crates/host-api/include)
      target_link_libraries(linux-floating-panels-x11-test PRIVATE PkgConfig::MODE_BADGE_X11 nlohmann_json::nlohmann_json
        "${MSIME_HOST_LIBRARY}")
      if(MSIME_X11_IO_ERROR_EXIT_HANDLER)
        target_compile_definitions(linux-floating-panels-x11-test PRIVATE MSIME_X11_IO_ERROR_EXIT_HANDLER)
      endif()
      add_test(NAME linux-floating-panels-x11
        COMMAND "${MSIME_XVFB_RUN}" -a $<TARGET_FILE:linux-floating-panels-x11-test>)
      set_tests_properties(linux-floating-panels-x11 PROPERTIES TIMEOUT 60)
    endif()
  endif()
  target_compile_options(linux-native-compose-test PRIVATE -Wall -Wextra -Werror)
  target_include_directories(linux-native-compose-test PRIVATE src)
  target_link_libraries(linux-native-compose-test PRIVATE PkgConfig::XKBCOMMON)
  add_test(NAME linux-native-compose COMMAND linux-native-compose-test
    "${CMAKE_CURRENT_SOURCE_DIR}/tests/input/compose.fixture")
endif()

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
# License and third-party notices, the Linux counterpart of the macOS bundle's Resources/Licenses. Every install redistributes the code compiled into the host library, so plain `cmake --install` gets them as well, not only packages. The project license is `copyright` here (GPL-3.0.txt on macOS); it also covers the input engine (crates/engine), which is this project's code. What the engine carries from elsewhere keeps its own notice: zinnia, whose recognizer crates/engine/src/handwriting ports; the place names `@` mode offers, from modood/Administrative-divisions-of-China (WTFPL); the Korean Hanja table, from libhangul's data/hanja/hanja.txt (BSD-3-Clause); the component reverse lookup table of 拆字反查 (C mode), adapted from kfcd/chaizi (CC BY 3.0, whose attribution and list of changes travel with it), with readings, ranks and traditional variants from the Unicode Han Database (Unicode License V3); the Jyutping and bopomofo data of the Cantonese and Zhuyin schemes, from rime-cantonese (CC BY 4.0) and libchewing-data (LGPL-2.1-or-later), and the stroke orders of the Stroke scheme, from rime-stroke (LGPL-3.0, with the CNS11643 attribution), whose notices travel with every engine build even where those schemes are not offered; the provenance of the dictionary data msime-linux-setup downloads, carried verbatim from msime-engine as resources/licenses/msime-engine-dictionary-NOTICE.md, the copy every platform ships; and the provenance of the helpcode tables installed into the packaged resource directory: the five the Engine brought (ENGINE-NOTICE.md) and jiajia (NOTICE.md).
set(MSIME_NOTICE_SOURCES
  "${CMAKE_CURRENT_SOURCE_DIR}/../../LICENSE"
  "${CMAKE_CURRENT_SOURCE_DIR}/data/THIRD_PARTY_NOTICES.txt"
  "${CMAKE_CURRENT_SOURCE_DIR}/../../resources/licenses/msime-engine-dictionary-NOTICE.md"
  "${CMAKE_CURRENT_SOURCE_DIR}/../../resources/helpcodes/ENGINE-NOTICE.md"
  "${CMAKE_CURRENT_SOURCE_DIR}/../../resources/helpcodes/NOTICE.md"
  # 五笔 86 辅助码表的来源与生成方法，附它所派生的极点码表的 Apache-2.0 全文。
  "${CMAKE_CURRENT_SOURCE_DIR}/../../resources/helpcodes/NOTICE-wubi86.md"
  "${CMAKE_CURRENT_SOURCE_DIR}/../../resources/licenses/Zinnia-LICENSE.txt"
  "${CMAKE_CURRENT_SOURCE_DIR}/../../resources/licenses/Administrative-divisions-of-China-WTFPL.txt"
  "${CMAKE_CURRENT_SOURCE_DIR}/../../resources/licenses/libhangul-hanja-BSD-3-Clause.txt"
  "${CMAKE_CURRENT_SOURCE_DIR}/../../resources/licenses/librime-octagram-BSD-3-Clause.txt"
  "${CMAKE_CURRENT_SOURCE_DIR}/../../resources/licenses/kfcd-chaizi-CC-BY-3.0.txt"
  "${CMAKE_CURRENT_SOURCE_DIR}/../../resources/licenses/rime-wanxiang-CC-BY-4.0.txt"
  "${CMAKE_CURRENT_SOURCE_DIR}/../../resources/licenses/Unicode-License-V3.txt"
  "${CMAKE_CURRENT_SOURCE_DIR}/../../resources/licenses/rime-cantonese-CC-BY-4.0.txt"
  "${CMAKE_CURRENT_SOURCE_DIR}/../../resources/licenses/libchewing-data-LGPL-2.1.txt"
  "${CMAKE_CURRENT_SOURCE_DIR}/../../resources/licenses/ewts-MIT.txt"
  "${CMAKE_CURRENT_SOURCE_DIR}/../../resources/licenses/rime-stroke-LGPL-3.0.txt"
  "${CMAKE_CURRENT_SOURCE_DIR}/../../resources/licenses/OpenCC-Apache-2.0.txt")
# `copyright` is the name Debian policy requires for the package license.
set(MSIME_NOTICE_NAMES
  copyright
  THIRD_PARTY_NOTICES.txt
  msime-engine-dictionary-NOTICE.md
  msime-engine-helpcode-NOTICE.md
  msime-helpcode-jiajia-NOTICE.md
  msime-helpcode-wubi86-NOTICE.md
  Zinnia-LICENSE.txt
  Administrative-divisions-of-China-WTFPL.txt
  libhangul-hanja-BSD-3-Clause.txt
  librime-octagram-BSD-3-Clause.txt
  kfcd-chaizi-CC-BY-3.0.txt
  rime-wanxiang-CC-BY-4.0.txt
  Unicode-License-V3.txt
  rime-cantonese-CC-BY-4.0.txt
  libchewing-data-LGPL-2.1.txt
  ewts-MIT.txt
  rime-stroke-LGPL-3.0.txt
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
# ZIP_LISTS 按位置配对，两张表差一项时其后的声明全部错名，多出的名字配上空路径，不报错也装不上；长度不等直接停下。
list(LENGTH MSIME_NOTICE_SOURCES MSIME_NOTICE_SOURCE_COUNT)
list(LENGTH MSIME_NOTICE_NAMES MSIME_NOTICE_NAME_COUNT)
if(NOT MSIME_NOTICE_SOURCE_COUNT EQUAL MSIME_NOTICE_NAME_COUNT)
  message(FATAL_ERROR "MSIME_NOTICE_SOURCES has ${MSIME_NOTICE_SOURCE_COUNT} entries but MSIME_NOTICE_NAMES has ${MSIME_NOTICE_NAME_COUNT}")
endif()
set(MSIME_MISSING_NOTICES "")
foreach(MSIME_NOTICE_SOURCE MSIME_NOTICE_NAME IN ZIP_LISTS MSIME_NOTICE_SOURCES MSIME_NOTICE_NAMES)
  if(EXISTS "${MSIME_NOTICE_SOURCE}" AND NOT IS_DIRECTORY "${MSIME_NOTICE_SOURCE}")
    install(FILES "${MSIME_NOTICE_SOURCE}"
            DESTINATION "${CMAKE_INSTALL_DATADIR}/doc/${MSIME_CLIENT_DIRECTORY}" RENAME "${MSIME_NOTICE_NAME}")
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
# Fcitx5 插件先配置：打包要知道它的插件目录（装在自己前缀下的版本取自 Fcitx5Core.pc），才能把那个归 fcitx5 所有的目录排除在 RPM 的文件列表之外。
if(MSIME_ENABLE_FCITX5)
  add_subdirectory(fcitx5)
endif()
if(MSIME_ENABLE_PACKAGING)
  include(cmake/packaging.cmake)
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
