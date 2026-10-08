cmake_minimum_required(VERSION 3.25)
project(MSIMEClientFcitx5 LANGUAGES CXX)
include(GNUInstallDirs)
include(CTest)
# 单独配置这个目录时也要选版本；作为上级工程的子目录时上级已经包含过，这里什么也不做。
include(${CMAKE_CURRENT_SOURCE_DIR}/../cmake/Edition.cmake)
# 作为上级工程的子目录构建时不继承它钉死的 C++17。Fcitx5 5.1 的公开头文件用了
# std::span 和 ranges，语言标准应当由 Fcitx5::Core 的接口要求决定；继承下来的 17 会
# 让这个插件在新版本 Fcitx5 上整个编译不过，而单独配置这个目录时它是好的——上级工程
# 的全局设置本来就不该管到一个并列入口用什么标准。
unset(CMAKE_CXX_STANDARD)
unset(CMAKE_CXX_STANDARD_REQUIRED)
find_package(Fcitx5Core 5.0.20 REQUIRED)
# 中英文切换提示用的 showCustomInputMethodInformation 是 5.1.11 才有的；低于这个版本
# 时不声明这个宏，提示静默缺席而不是让插件在老发行版上编不出来。
if(Fcitx5Core_VERSION VERSION_GREATER_EQUAL 5.1.11)
  set(MSIME_FCITX5_CUSTOM_IM_INFORMATION ON)
endif()
# Fcitx5 5.1.13 起公开头文件 fcitx-utils/log.h 用到 std::span（更新的版本还用到 string_view::starts_with 等 C++20 特性），它的 CMake 配置却没有把 C++20 作为接口要求导出，按 17 编译会在 Fcitx5 自己的头文件里报错（Fedora 43 配 GCC 15、Ubuntu 26.04 都是这样；Tumbleweed 的 GCC 16 默认就是 C++20，所以没暴露）。插件与它的测试一律按 C++20 编译，支持的发行版里最旧的编译器（Ubuntu 24.04 的 GCC 13）也完整支持。
find_package(nlohmann_json 3.11 REQUIRED)
find_package(PkgConfig REQUIRED)
pkg_check_modules(GIO REQUIRED IMPORTED_TARGET gio-2.0)
# The brand mark leading the candidate card is the installed application icon, decoded with cairo, which the classic UI itself is built on.
pkg_check_modules(CAIRO REQUIRED IMPORTED_TARGET cairo)
set(MSIME_HOST_LIBRARY "${CMAKE_CURRENT_SOURCE_DIR}/../../../target/debug/libmsime_host_api.so" CACHE FILEPATH "Cargo-built Linux Host API")
if(NOT EXISTS "${MSIME_HOST_LIBRARY}")
  message(FATAL_ERROR "Build msime-host-api for Linux first")
endif()
msime_edition_host_library(MSIME_HOST_LIBRARY)
# 装在自己前缀下的版本把插件装进 fcitx5 自己的插件目录（Fcitx5Core.pc 的 libdir 下），fcitx5 只从那里加载；其余情况与今天相同，跟着前缀走。
if(MSIME_EDITION_SYSTEM_PATHS)
  pkg_get_variable(MSIME_FCITX5_SYSTEM_LIBDIR Fcitx5Core libdir)
  if(NOT IS_ABSOLUTE "${MSIME_FCITX5_SYSTEM_LIBDIR}")
    message(FATAL_ERROR "Cannot find the Fcitx5 library directory from Fcitx5Core.pc")
  endif()
  set(MSIME_FCITX5_ADDON_DIR "${MSIME_FCITX5_SYSTEM_LIBDIR}/fcitx5" CACHE PATH "Fcitx5 addon directory")
else()
  set(MSIME_FCITX5_ADDON_DIR "${CMAKE_INSTALL_LIBDIR}/fcitx5" CACHE PATH "Fcitx5 addon directory")
endif()
# 中英文切换徽章：自绘的 layer-shell 浮层，需要 wayland-client、pangocairo 和 wlr-layer-shell
# 的协议代码。缺其中任何一样就不编译它，宿主回退到面板自己的文字提示，插件照常可用。
pkg_check_modules(MSIME_BADGE_WAYLAND QUIET IMPORTED_TARGET wayland-client pangocairo)
find_program(MSIME_BADGE_WAYLAND_SCANNER wayland-scanner)
set(MSIME_BADGE_SOURCES "")
if(MSIME_BADGE_WAYLAND_FOUND AND MSIME_BADGE_WAYLAND_SCANNER)
  # 协议代码是 C。这个子工程声明的是 LANGUAGES CXX，不打开 C 的话生成的 .c 不会被编译，
  # 插件照常链接成功却在加载时报 undefined symbol: zwlr_layer_surface_v1_interface。
  enable_language(C)
  set(MSIME_BADGE_PROTOCOL_DIR "${CMAKE_CURRENT_BINARY_DIR}/wayland-protocols")
  add_custom_command(
    OUTPUT "${MSIME_BADGE_PROTOCOL_DIR}/wlr-layer-shell-unstable-v1-client-protocol.h"
           "${MSIME_BADGE_PROTOCOL_DIR}/wlr-layer-shell-unstable-v1-protocol.c"
    COMMAND "${CMAKE_COMMAND}" -E make_directory "${MSIME_BADGE_PROTOCOL_DIR}"
    COMMAND "${MSIME_BADGE_WAYLAND_SCANNER}" client-header
            "${CMAKE_CURRENT_SOURCE_DIR}/../data/wayland/wlr-layer-shell-unstable-v1.xml"
            "${MSIME_BADGE_PROTOCOL_DIR}/wlr-layer-shell-unstable-v1-client-protocol.h"
    COMMAND "${MSIME_BADGE_WAYLAND_SCANNER}" private-code
            "${CMAKE_CURRENT_SOURCE_DIR}/../data/wayland/wlr-layer-shell-unstable-v1.xml"
            "${MSIME_BADGE_PROTOCOL_DIR}/wlr-layer-shell-unstable-v1-protocol.c"
    DEPENDS ../data/wayland/wlr-layer-shell-unstable-v1.xml)
  # layer-shell 的协议引用了 xdg_popup，所以 xdg-shell 的生成代码必须一起编进来，
  # 否则插件链接得过、加载时报 undefined symbol: xdg_popup_interface。
  # wayland-protocols 的 .pc 给出它的数据目录；只搜 /usr/share 的话，装在别处的（例如 Nix store）
  # 找不到，Wayland 浮层被静默跳过。原有路径留作没有 .pc 时的退路。
  pkg_get_variable(MSIME_WAYLAND_PROTOCOLS_DIR wayland-protocols pkgdatadir)
  find_path(MSIME_BADGE_XDG_SHELL_DIR NAMES xdg-shell.xml
            HINTS "${MSIME_WAYLAND_PROTOCOLS_DIR}/stable/xdg-shell"
            PATHS /usr/share/wayland-protocols/stable/xdg-shell
                  /usr/local/share/wayland-protocols/stable/xdg-shell)
  if(MSIME_BADGE_XDG_SHELL_DIR)
    add_custom_command(
      OUTPUT "${MSIME_BADGE_PROTOCOL_DIR}/xdg-shell-client-protocol.h"
             "${MSIME_BADGE_PROTOCOL_DIR}/xdg-shell-protocol.c"
      COMMAND "${CMAKE_COMMAND}" -E make_directory "${MSIME_BADGE_PROTOCOL_DIR}"
      COMMAND "${MSIME_BADGE_WAYLAND_SCANNER}" client-header
              "${MSIME_BADGE_XDG_SHELL_DIR}/xdg-shell.xml"
              "${MSIME_BADGE_PROTOCOL_DIR}/xdg-shell-client-protocol.h"
      COMMAND "${MSIME_BADGE_WAYLAND_SCANNER}" private-code
              "${MSIME_BADGE_XDG_SHELL_DIR}/xdg-shell.xml"
              "${MSIME_BADGE_PROTOCOL_DIR}/xdg-shell-protocol.c"
      DEPENDS "${MSIME_BADGE_XDG_SHELL_DIR}/xdg-shell.xml")
    set(MSIME_BADGE_WAYLAND_SOURCES ../src/overlay/ModeBadgeWaylandSurface.cpp
        "${MSIME_BADGE_PROTOCOL_DIR}/wlr-layer-shell-unstable-v1-protocol.c"
        "${MSIME_BADGE_PROTOCOL_DIR}/xdg-shell-protocol.c")
  endif()
endif()
add_library(msime-fcitx5 MODULE FcitxEngine.cpp SystemTheme.cpp ../src/system/ChineseTextConversion.cpp ../src/system/DiagnosticLog.cpp ${MSIME_BADGE_WAYLAND_SOURCES})
# X11 会话下的后端。两个后端互不依赖：只有一种可用时徽章照常工作，都没有时宿主只发面板那半文字提示。xrandr 用来找焦点所在的显示器，和语音浮层同一套定位。
pkg_check_modules(MSIME_BADGE_X11 QUIET IMPORTED_TARGET x11 xext xrandr cairo-xlib pangocairo)
set(MSIME_BADGE_X11_SOURCES "")
if(MSIME_BADGE_X11_FOUND)
  set(MSIME_BADGE_X11_SOURCES ../src/overlay/ModeBadgeX11Surface.cpp)
endif()
if(MSIME_BADGE_WAYLAND_SOURCES OR MSIME_BADGE_X11_SOURCES)
  target_sources(msime-fcitx5 PRIVATE ../src/overlay/ModeBadgeSurfaceFactory.cpp
      ${MSIME_BADGE_X11_SOURCES})
  target_compile_definitions(msime-fcitx5 PRIVATE MSIME_FCITX5_MODE_BADGE
      MSIME_MODE_BADGE_ICON="${MSIME_SYSTEM_FULL_DATADIR}/icons/hicolor/128x128/apps/${MSIME_EDITION_PACKAGE}.png")
  target_include_directories(msime-fcitx5 PRIVATE ../src/overlay "${MSIME_BADGE_PROTOCOL_DIR}")
  if(MSIME_BADGE_WAYLAND_SOURCES)
    target_compile_definitions(msime-fcitx5 PRIVATE MSIME_BADGE_HAS_WAYLAND)
    target_link_libraries(msime-fcitx5 PRIVATE PkgConfig::MSIME_BADGE_WAYLAND)
  endif()
  if(MSIME_BADGE_X11_SOURCES)
    target_compile_definitions(msime-fcitx5 PRIVATE MSIME_BADGE_HAS_X11)
    target_link_libraries(msime-fcitx5 PRIVATE PkgConfig::MSIME_BADGE_X11)
  endif()
endif()
set(MSIME_FCITX5_WAVE_SOURCES)
if(X11_SURFACE_FOUND)
  list(APPEND MSIME_FCITX5_WAVE_SOURCES ../src/overlay/WaveOverlayX11Surface.cpp)
  target_compile_definitions(msime-fcitx5 PRIVATE MSIME_LINUX_HAS_X11_SURFACE)
  target_link_libraries(msime-fcitx5 PRIVATE PkgConfig::X11_SURFACE)
endif()
if(WAYLAND_CLIENT_FOUND AND WAYLAND_SCANNER_EXECUTABLE AND MSIME_XDG_SHELL_XML)
  list(APPEND MSIME_FCITX5_WAVE_SOURCES ../src/overlay/WaveOverlayWaylandSurface.cpp)
  # The badge's Wayland backend already compiles the same layer-shell and xdg-shell protocol code into this module; a second copy is a multiple-definition link error. It only shows where pangocairo is installed (any desktop with GTK development files), which the build-gate image does not have.
  if(NOT MSIME_BADGE_WAYLAND_SOURCES)
    list(APPEND MSIME_FCITX5_WAVE_SOURCES "${MSIME_LAYER_SHELL_CODE}" "${MSIME_XDG_SHELL_CODE}")
  endif()
  target_include_directories(msime-fcitx5 PRIVATE "${MSIME_WAYLAND_PROTOCOL_DIR}")
  target_compile_definitions(msime-fcitx5 PRIVATE MSIME_LINUX_HAS_WAYLAND_SURFACE)
  target_link_libraries(msime-fcitx5 PRIVATE PkgConfig::WAYLAND_CLIENT)
  if(WAYLAND_TEXT_FOUND)
    target_compile_definitions(msime-fcitx5 PRIVATE MSIME_LINUX_WAYLAND_TEXT)
    target_link_libraries(msime-fcitx5 PRIVATE PkgConfig::WAYLAND_TEXT)
  endif()
endif()
if(MSIME_FCITX5_WAVE_SOURCES)
  target_sources(msime-fcitx5 PRIVATE ${MSIME_FCITX5_WAVE_SOURCES})
endif()
target_compile_features(msime-fcitx5 PRIVATE cxx_std_20)
target_compile_options(msime-fcitx5 PRIVATE -Wall -Wextra -Werror)
target_include_directories(msime-fcitx5 PRIVATE ../../../crates/host-api/include)
target_link_libraries(msime-fcitx5 PRIVATE Fcitx5::Core PkgConfig::GIO PkgConfig::CAIRO nlohmann_json::nlohmann_json "${MSIME_HOST_LIBRARY}")
# The built-in sound packs the parent project installs beside the resource bundle; the addon names them to the Host API as sound_packs.
target_compile_definitions(msime-fcitx5 PRIVATE MSIME_SYSTEM_OPTIONS="${CMAKE_INSTALL_FULL_SYSCONFDIR}/${MSIME_CLIENT_DIRECTORY}/runtime-options.json"
  MSIME_BINDIR="${CMAKE_INSTALL_FULL_BINDIR}" MSIME_ICON_DIR="${MSIME_SYSTEM_FULL_DATADIR}/icons/hicolor"
  MSIME_SOUND_PACKS="${CMAKE_INSTALL_FULL_DATADIR}/${MSIME_CLIENT_DIRECTORY}/sound-packs")
if(MSIME_FCITX5_CUSTOM_IM_INFORMATION)
  target_compile_definitions(msime-fcitx5 PRIVATE MSIME_FCITX5_CUSTOM_IM_INFORMATION)
endif()
# 修饰状态回读自 5.1.22 才公开，旧发行版保留按下一次无 Shift 按键回移的兼容路径。
if(Fcitx5Core_VERSION VERSION_GREATER_EQUAL 5.1.22)
  target_compile_definitions(msime-fcitx5 PRIVATE MSIME_FCITX5_XKB_STATE_MASK)
endif()
# classicui 从 5.1.18 起单独按磅设置候选序号的字号，候选字体要写成磅序号才与候选一样大；更早的版本照写像素（见 CandidateFontUnit）。
if(Fcitx5Core_VERSION VERSION_GREATER_EQUAL 5.1.18)
  target_compile_definitions(msime-fcitx5 PRIVATE MSIME_FCITX5_LABEL_POINTS)
endif()
# Usage reporting, through the same wrapper as the IBus host and with the same version: the parent project's MSIME_LINUX_VERSION, or platforms/linux/version.txt when this directory is configured on its own. Only the addon reports; the native test builds the engine without it.
if(NOT MSIME_LINUX_VERSION)
  set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS "${CMAKE_CURRENT_SOURCE_DIR}/../version.txt")
  file(STRINGS "${CMAKE_CURRENT_SOURCE_DIR}/../version.txt" MSIME_LINUX_VERSION LIMIT_COUNT 1)
  string(STRIP "${MSIME_LINUX_VERSION}" MSIME_LINUX_VERSION)
endif()
target_sources(msime-fcitx5 PRIVATE ../../common/Telemetry.cpp)
target_include_directories(msime-fcitx5 PRIVATE ../../common)
target_compile_definitions(msime-fcitx5 PRIVATE MSIME_FCITX5_TELEMETRY MSIME_LINUX_VERSION="${MSIME_LINUX_VERSION}")
if(MSIME_EDITION_SYSTEM_PATHS)
  # 插件在系统的 fcitx5 目录、宿主库在版本前缀下，两者不随同一个前缀移动，RPATH 直接写宿主库的绝对目录。
  set_target_properties(msime-fcitx5 PROPERTIES INSTALL_RPATH "${CMAKE_INSTALL_FULL_LIBDIR}/${MSIME_CLIENT_DIRECTORY}")
else()
  # 插件目录可能是绝对路径（CMAKE_INSTALL_LIBDIR 传成绝对路径时它跟着是，nixpkgs 的 cmake 钩子就这样传）。直接拼在前缀后面会多出整段前缀，RUNPATH 算成 $ORIGIN/../../../../../lib/msime-client，插件加载时找不到 Host API。
  cmake_path(ABSOLUTE_PATH MSIME_FCITX5_ADDON_DIR BASE_DIRECTORY "${CMAKE_INSTALL_PREFIX}" NORMALIZE
             OUTPUT_VARIABLE MSIME_FCITX5_FULL_ADDON_DIR)
  file(RELATIVE_PATH MSIME_FCITX5_HOST_RELATIVE "${MSIME_FCITX5_FULL_ADDON_DIR}" "${CMAKE_INSTALL_FULL_LIBDIR}/${MSIME_CLIENT_DIRECTORY}")
  set_target_properties(msime-fcitx5 PROPERTIES INSTALL_RPATH "$ORIGIN/${MSIME_FCITX5_HOST_RELATIVE}")
endif()
if(NOT MSIME_EDITION_IS_FULL)
  # 插件库名、插件名和输入法条目名都随版本（lib<fcitx5_addon>-fcitx5.so），几个版本的插件可以同时装进同一个 fcitx5。fcitx5 用 RTLD_LOCAL 加载插件，但 GCC 把内联函数里的静态变量导出成 STB_GNU_UNIQUE，动态链接器会在整个进程里把同名的合并成一份；隐藏符号不参与合并，所以两个版本的插件各用各的。导出的只有 FCITX_ADDON_FACTORY 声明的工厂函数。
  set_target_properties(msime-fcitx5 PROPERTIES OUTPUT_NAME "${MSIME_EDITION_FCITX5_ADDON}-fcitx5"
                        CXX_VISIBILITY_PRESET hidden VISIBILITY_INLINES_HIDDEN ON)
endif()
install(TARGETS msime-fcitx5 LIBRARY DESTINATION "${MSIME_FCITX5_ADDON_DIR}")
install(FILES "${MSIME_HOST_LIBRARY}" DESTINATION "${CMAKE_INSTALL_LIBDIR}/${MSIME_CLIENT_DIRECTORY}")
msime_edition_source(MSIME_FCITX5_ADDON_CONF fcitx5/msime.conf)
msime_edition_source(MSIME_FCITX5_INPUT_METHOD_CONF fcitx5/msime-inputmethod.conf)
# 装出去的插件条目把 Library 写成插件的绝对路径（fcitx5 自 5.0.20 起按绝对路径直接加载，不再到插件目录里找）。.deb 按 Debian 的多架构目录装到 /usr/lib/x86_64-linux-gnu/fcitx5，而 AOSC OS 这类也用 dpkg、却不分多架构目录的发行版，fcitx5 只在 /usr/lib/fcitx5 里找，只写库名时插件装上了也加载不到。源码树里的条目仍只写库名，测试 daemon 用 FCITX_ADDON_DIRS 从构建目录加载它。
cmake_path(ABSOLUTE_PATH MSIME_FCITX5_ADDON_DIR BASE_DIRECTORY "${CMAKE_INSTALL_PREFIX}" NORMALIZE
           OUTPUT_VARIABLE MSIME_FCITX5_INSTALLED_ADDON_DIR)
set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS "${MSIME_FCITX5_ADDON_CONF}")
file(READ "${MSIME_FCITX5_ADDON_CONF}" MSIME_FCITX5_ADDON_CONF_TEXT)
string(REGEX REPLACE "\nLibrary=([^\n/]+)\n" "\nLibrary=${MSIME_FCITX5_INSTALLED_ADDON_DIR}/\\1\n"
       MSIME_FCITX5_INSTALLED_ADDON_CONF_TEXT "${MSIME_FCITX5_ADDON_CONF_TEXT}")
if(MSIME_FCITX5_INSTALLED_ADDON_CONF_TEXT STREQUAL MSIME_FCITX5_ADDON_CONF_TEXT)
  message(FATAL_ERROR "No Library= line to make absolute in ${MSIME_FCITX5_ADDON_CONF}")
endif()
set(MSIME_FCITX5_ADDON_CONF "${CMAKE_CURRENT_BINARY_DIR}/installed/msime.conf")
file(CONFIGURE OUTPUT "${MSIME_FCITX5_ADDON_CONF}" CONTENT "${MSIME_FCITX5_INSTALLED_ADDON_CONF_TEXT}" @ONLY)
install(FILES "${MSIME_FCITX5_ADDON_CONF}" DESTINATION "${MSIME_SYSTEM_DATADIR}/fcitx5/addon" RENAME ${MSIME_EDITION_FCITX5_ADDON}.conf)
install(FILES "${MSIME_FCITX5_INPUT_METHOD_CONF}" DESTINATION "${MSIME_SYSTEM_DATADIR}/fcitx5/inputmethod" RENAME ${MSIME_EDITION_FCITX5_ADDON}.conf)
if(BUILD_TESTING)
  add_executable(fcitx5-preceding-characters-test tests/preceding_characters.cpp)
  target_compile_features(fcitx5-preceding-characters-test PRIVATE cxx_std_20)
  target_compile_options(fcitx5-preceding-characters-test PRIVATE -Wall -Wextra -Werror)
  target_link_libraries(fcitx5-preceding-characters-test PRIVATE Fcitx5::Core)
  add_test(NAME fcitx5-preceding-characters COMMAND fcitx5-preceding-characters-test)
  add_executable(fcitx5-native-test tests/native.cpp SystemTheme.cpp ../src/system/ChineseTextConversion.cpp ../src/system/DiagnosticLog.cpp)
  if(X11_SURFACE_FOUND)
    target_sources(fcitx5-native-test PRIVATE ../src/overlay/WaveOverlayX11Surface.cpp)
    target_compile_definitions(fcitx5-native-test PRIVATE MSIME_LINUX_HAS_X11_SURFACE)
    target_link_libraries(fcitx5-native-test PRIVATE PkgConfig::X11_SURFACE)
  endif()
  if(WAYLAND_CLIENT_FOUND AND WAYLAND_SCANNER_EXECUTABLE AND MSIME_XDG_SHELL_XML)
    target_sources(fcitx5-native-test PRIVATE ../src/overlay/WaveOverlayWaylandSurface.cpp
      "${MSIME_LAYER_SHELL_CODE}" "${MSIME_XDG_SHELL_CODE}")
    target_include_directories(fcitx5-native-test PRIVATE "${MSIME_WAYLAND_PROTOCOL_DIR}")
    target_compile_definitions(fcitx5-native-test PRIVATE MSIME_LINUX_HAS_WAYLAND_SURFACE)
    target_link_libraries(fcitx5-native-test PRIVATE PkgConfig::WAYLAND_CLIENT)
    if(WAYLAND_TEXT_FOUND)
      target_compile_definitions(fcitx5-native-test PRIVATE MSIME_LINUX_WAYLAND_TEXT)
      target_link_libraries(fcitx5-native-test PRIVATE PkgConfig::WAYLAND_TEXT)
    endif()
  endif()
  target_compile_features(fcitx5-native-test PRIVATE cxx_std_20)
  target_compile_options(fcitx5-native-test PRIVATE -Wall -Wextra -Werror)
  target_include_directories(fcitx5-native-test PRIVATE ../../../crates/host-api/include)
  target_link_libraries(fcitx5-native-test PRIVATE Fcitx5::Core PkgConfig::GIO PkgConfig::CAIRO nlohmann_json::nlohmann_json "${MSIME_HOST_LIBRARY}")
  if(Fcitx5Core_VERSION VERSION_GREATER_EQUAL 5.1.22)
    target_compile_definitions(fcitx5-native-test PRIVATE MSIME_FCITX5_XKB_STATE_MASK)
  endif()
  if(Fcitx5Core_VERSION VERSION_GREATER_EQUAL 5.1.18)
    target_compile_definitions(fcitx5-native-test PRIVATE MSIME_FCITX5_LABEL_POINTS)
  endif()
  # The first-run case in tests/native.cpp needs a system options file that does not exist and a guide program it can stub, so neither may point at the installed locations.
  target_compile_definitions(fcitx5-native-test PRIVATE
    MSIME_SYSTEM_OPTIONS="${CMAKE_CURRENT_BINARY_DIR}/first-run-absent/runtime-options.json"
    MSIME_BINDIR="${CMAKE_CURRENT_BINARY_DIR}/first-run-bin"
    MSIME_ICON_DIR="${CMAKE_CURRENT_SOURCE_DIR}/../data/icons/hicolor"
    MSIME_ZHUYIN_DICTIONARY_FIXTURE="${CMAKE_CURRENT_SOURCE_DIR}/../tests/runtime/zhuyin_dictionary.py"
    MSIME_STROKE_DICTIONARY_FIXTURE="${CMAKE_CURRENT_SOURCE_DIR}/../tests/runtime/stroke_dictionary.py")
  # The candidate card's brand mark, decoded from the icons the package installs.
  add_executable(linux-candidate-fcitx-logo-test ../tests/candidate/candidate_fcitx_logo.cpp)
  target_compile_options(linux-candidate-fcitx-logo-test PRIVATE -Wall -Wextra -Werror -UNDEBUG)
  target_link_libraries(linux-candidate-fcitx-logo-test PRIVATE PkgConfig::CAIRO nlohmann_json::nlohmann_json)
  add_test(NAME linux-candidate-fcitx-logo
    COMMAND linux-candidate-fcitx-logo-test "${CMAKE_CURRENT_SOURCE_DIR}/../data/icons/hicolor")
  # A skin's decoration resampled to its declared width with cairo, so the classic UI draws it whole in the band above the card.
  add_executable(linux-candidate-fcitx-overlay-scale-test ../tests/candidate/candidate_fcitx_overlay_scale.cpp)
  target_compile_features(linux-candidate-fcitx-overlay-scale-test PRIVATE cxx_std_20)
  target_compile_options(linux-candidate-fcitx-overlay-scale-test PRIVATE -Wall -Wextra -Werror -UNDEBUG)
  target_link_libraries(linux-candidate-fcitx-overlay-scale-test PRIVATE PkgConfig::CAIRO nlohmann_json::nlohmann_json)
  add_test(NAME linux-candidate-fcitx-overlay-scale COMMAND linux-candidate-fcitx-overlay-scale-test)
  set(MSIME_FCITX5_TEST_RESOURCES "" CACHE PATH "Verified resources directory for the native Fcitx5 test")
  if(MSIME_FCITX5_TEST_RESOURCES)
    add_test(NAME fcitx5-native-context
      COMMAND fcitx5-native-test "${MSIME_FCITX5_TEST_RESOURCES}")
    add_test(NAME fcitx5-native-ai
      COMMAND fcitx5-native-test "${MSIME_FCITX5_TEST_RESOURCES}" --ai)
    add_test(NAME fcitx5-local-mode-shortcuts
      COMMAND fcitx5-native-test "${MSIME_FCITX5_TEST_RESOURCES}" --local-modes)
    find_package(Python3 COMPONENTS Interpreter)
    find_program(DBUS_RUN_SESSION dbus-run-session)
    if(Python3_Interpreter_FOUND AND DBUS_RUN_SESSION)
      add_test(NAME fcitx5-daemon-frontend
        COMMAND "${DBUS_RUN_SESSION}" --
          "${Python3_EXECUTABLE}" "${CMAKE_CURRENT_SOURCE_DIR}/tests/daemon.py"
          "${MSIME_FCITX5_TEST_RESOURCES}" "${MSIME_HOST_LIBRARY}"
          "$<TARGET_FILE:msime-fcitx5>")
      find_program(XVFB_RUN xvfb-run)
      if(XVFB_RUN)
        add_test(NAME fcitx5-page-number-visibility
          COMMAND "${XVFB_RUN}" -a "${DBUS_RUN_SESSION}" --
            "${Python3_EXECUTABLE}" "${CMAKE_CURRENT_SOURCE_DIR}/tests/daemon.py"
            "${MSIME_FCITX5_TEST_RESOURCES}" "${MSIME_HOST_LIBRARY}"
            "$<TARGET_FILE:msime-fcitx5>" --page-number)
        set_tests_properties(fcitx5-page-number-visibility PROPERTIES TIMEOUT 60)
      endif()
      find_program(XDOTOOL xdotool)
      if(XVFB_RUN AND XDOTOOL)
        add_test(NAME fcitx5-gtk-editor
          COMMAND "${CMAKE_COMMAND}" -E env MSIME_TEST_GTK=1
            "${XVFB_RUN}" -a "${DBUS_RUN_SESSION}" --
            "${Python3_EXECUTABLE}" "${CMAKE_CURRENT_SOURCE_DIR}/tests/daemon.py"
            "${MSIME_FCITX5_TEST_RESOURCES}" "${MSIME_HOST_LIBRARY}"
            "$<TARGET_FILE:msime-fcitx5>")
        set_tests_properties(fcitx5-gtk-editor PROPERTIES TIMEOUT 90)
      endif()
    endif()
  endif()
endif()
