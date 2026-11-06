# 老发行版可显式关闭 GTK4 窗口；当前发行包必须包含原生管理入口。
pkg_check_modules(LOCAL_TRANSLATION_GTK QUIET IMPORTED_TARGET gtk4>=4.8)
option(MSIME_LOCAL_TRANSLATION_SETTINGS "Build native local translation settings" ${LOCAL_TRANSLATION_GTK_FOUND})
if(MSIME_LOCAL_TRANSLATION_SETTINGS)
  add_compile_definitions(MSIME_HAS_LOCAL_TRANSLATION_SETTINGS)
  pkg_check_modules(LOCAL_TRANSLATION_GTK REQUIRED IMPORTED_TARGET gtk4>=4.8)
  find_package(Threads REQUIRED)
  add_executable(msime-linux-local-translation src/settings/LocalTranslationWindow.cpp)
  target_include_directories(msime-linux-local-translation PRIVATE ../../crates/host-api/include)
  target_compile_options(msime-linux-local-translation PRIVATE -Wall -Wextra -Werror)
  target_link_libraries(msime-linux-local-translation PRIVATE PkgConfig::LOCAL_TRANSLATION_GTK
    nlohmann_json::nlohmann_json Threads::Threads "${MSIME_HOST_LIBRARY}")
  set_target_properties(msime-linux-local-translation PROPERTIES
    OUTPUT_NAME "${MSIME_EDITION_PACKAGE}-local-translation"
    INSTALL_RPATH "$ORIGIN/${MSIME_HOST_LIBRARY_RELATIVE_DIR}")
  install(TARGETS msime-linux-local-translation RUNTIME DESTINATION "${CMAKE_INSTALL_BINDIR}")
  if(MSIME_EDITION_SYSTEM_PATHS)
    file(CREATE_LINK "${CMAKE_INSTALL_FULL_BINDIR}/${MSIME_EDITION_PACKAGE}-local-translation"
      "${CMAKE_CURRENT_BINARY_DIR}/edition/usr-bin/${MSIME_EDITION_PACKAGE}-local-translation" SYMBOLIC)
    install(FILES "${CMAKE_CURRENT_BINARY_DIR}/edition/usr-bin/${MSIME_EDITION_PACKAGE}-local-translation" DESTINATION "/usr/bin")
  endif()
endif()

if(NOT MSIME_LOCAL_TRANSLATION_SETTINGS)
  message(STATUS "本地翻译设置未构建；安装 GTK 4.8+ 开发包并设置 MSIME_LOCAL_TRANSLATION_SETTINGS=ON 可启用")
endif()
