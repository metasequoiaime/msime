# Linux 原生设置窗口：清空快捷短语必须走宿主 API，不能在共享 WebView 里新增入口。
pkg_check_modules(QUICK_PHRASE_CLEAR_GTK QUIET IMPORTED_TARGET gtk4>=4.8)
option(MSIME_QUICK_PHRASE_CLEAR_SETTINGS "Build native quick phrase clearing settings" ${QUICK_PHRASE_CLEAR_GTK_FOUND})
if(MSIME_QUICK_PHRASE_CLEAR_SETTINGS)
  add_compile_definitions(MSIME_HAS_QUICK_PHRASE_CLEAR_SETTINGS)
  pkg_check_modules(QUICK_PHRASE_CLEAR_GTK REQUIRED IMPORTED_TARGET gtk4>=4.8)
  add_executable(msime-linux-quick-phrase-clear src/settings/QuickPhraseClearWindow.cpp)
  target_include_directories(msime-linux-quick-phrase-clear PRIVATE ../../crates/host-api/include src)
  target_compile_options(msime-linux-quick-phrase-clear PRIVATE -Wall -Wextra -Werror)
  target_link_libraries(msime-linux-quick-phrase-clear PRIVATE PkgConfig::QUICK_PHRASE_CLEAR_GTK
    nlohmann_json::nlohmann_json "${MSIME_HOST_LIBRARY}")
  set_target_properties(msime-linux-quick-phrase-clear PROPERTIES
    OUTPUT_NAME "${MSIME_EDITION_PACKAGE}-quick-phrase-clear"
    INSTALL_RPATH "$ORIGIN/${MSIME_HOST_LIBRARY_RELATIVE_DIR}")
  install(TARGETS msime-linux-quick-phrase-clear RUNTIME DESTINATION "${CMAKE_INSTALL_BINDIR}")
  if(MSIME_EDITION_SYSTEM_PATHS)
    file(CREATE_LINK "${CMAKE_INSTALL_FULL_BINDIR}/${MSIME_EDITION_PACKAGE}-quick-phrase-clear"
      "${CMAKE_CURRENT_BINARY_DIR}/edition/usr-bin/${MSIME_EDITION_PACKAGE}-quick-phrase-clear" SYMBOLIC)
    install(FILES "${CMAKE_CURRENT_BINARY_DIR}/edition/usr-bin/${MSIME_EDITION_PACKAGE}-quick-phrase-clear" DESTINATION "/usr/bin")
  endif()
endif()
