pkg_check_modules(DICTIONARY_GROUPS_GTK QUIET IMPORTED_TARGET gtk4>=4.8)
if(DICTIONARY_GROUPS_GTK_FOUND)
  add_compile_definitions(MSIME_HAS_DICTIONARY_COLLECTIONS)
  find_package(Threads REQUIRED)
  add_executable(msime-linux-dictionary-groups src/settings/DictionaryCollectionsWindow.cpp)
  target_include_directories(msime-linux-dictionary-groups PRIVATE ../../crates/host-api/include)
  target_compile_options(msime-linux-dictionary-groups PRIVATE -Wall -Wextra -Werror)
  target_link_libraries(msime-linux-dictionary-groups PRIVATE PkgConfig::DICTIONARY_GROUPS_GTK
    nlohmann_json::nlohmann_json Threads::Threads "${MSIME_HOST_LIBRARY}")
  set_target_properties(msime-linux-dictionary-groups PROPERTIES
    OUTPUT_NAME "${MSIME_EDITION_PACKAGE}-dictionary-groups"
    INSTALL_RPATH "$ORIGIN/${MSIME_HOST_LIBRARY_RELATIVE_DIR}")
  install(TARGETS msime-linux-dictionary-groups RUNTIME DESTINATION "${CMAKE_INSTALL_BINDIR}")
  if(MSIME_EDITION_SYSTEM_PATHS)
    file(CREATE_LINK "${CMAKE_INSTALL_FULL_BINDIR}/${MSIME_EDITION_PACKAGE}-dictionary-groups"
      "${CMAKE_CURRENT_BINARY_DIR}/edition/usr-bin/${MSIME_EDITION_PACKAGE}-dictionary-groups" SYMBOLIC)
    install(FILES "${CMAKE_CURRENT_BINARY_DIR}/edition/usr-bin/${MSIME_EDITION_PACKAGE}-dictionary-groups" DESTINATION "/usr/bin")
  endif()
endif()
