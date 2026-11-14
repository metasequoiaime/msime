# Local-only synthetic/isolated clipboard tests. Never launch the input method.
find_program(MSIME_XCRUN xcrun REQUIRED)
add_custom_target(macos-clipboard-tests)

function(msime_clipboard_swift_test name entry)
  set(sources "${CMAKE_CURRENT_SOURCE_DIR}/${entry}")
  foreach(source IN LISTS ARGN)
    list(APPEND sources "${CMAKE_CURRENT_SOURCE_DIR}/${source}")
  endforeach()
  set(executable "${CMAKE_CURRENT_BINARY_DIR}/${name}-test")
  add_custom_command(OUTPUT "${executable}"
    COMMAND "${MSIME_XCRUN}" swiftc -Onone
      -target "${CMAKE_HOST_SYSTEM_PROCESSOR}-apple-macosx${CMAKE_OSX_DEPLOYMENT_TARGET}"
      ${sources} -o "${executable}"
    DEPENDS ${sources} "${CMAKE_CURRENT_SOURCE_DIR}/ClipboardTests.cmake"
    VERBATIM)
  add_custom_target(${name}-build ALL DEPENDS "${executable}")
  add_dependencies(macos-clipboard-tests ${name}-build)
  add_test(NAME ${name} COMMAND "${executable}")
  set_tests_properties(${name} PROPERTIES LABELS "clipboard-local" TIMEOUT 30)
endfunction()

set(clipboard_catalog_sources src/backend/clipboard/BackendClipboardTextLimits.swift src/backend/emoji/BackendEmojiSymbolGroups.swift src/backend/emoji/BackendEmojiCatalog.swift
  src/backend/emoji/BackendEmojiClipboardHistory.swift)
set(clipboard_monitor_sources ${clipboard_catalog_sources} src/backend/clipboard/BackendClipboardCapture.swift
  src/backend/clipboard/BackendClipboardMonitor.swift)
set(clipboard_row_sources src/backend/emoji/BackendEmojiAppearance.swift src/backend/emoji/BackendEmojiClipboardRow.swift
  src/backend/emoji/BackendEmojiClipboardTooltip.swift)

msime_clipboard_swift_test(clipboard-history-adapter tests/emoji/EmojiClipboardHistoryTest.swift
  ${clipboard_catalog_sources})
msime_clipboard_swift_test(clipboard-history-observation tests/emoji/EmojiClipboardObservationTest.swift
  ${clipboard_catalog_sources})
msime_clipboard_swift_test(clipboard-capture tests/clipboard/ClipboardCaptureTest.swift src/backend/clipboard/BackendClipboardCapture.swift src/backend/clipboard/BackendClipboardTextLimits.swift)
msime_clipboard_swift_test(clipboard-monitor tests/clipboard/ClipboardMonitorTest.swift ${clipboard_monitor_sources})
msime_clipboard_swift_test(clipboard-service tests/clipboard/ClipboardServiceTest.swift
  ${clipboard_monitor_sources} src/backend/clipboard/BackendClipboardService.swift)
msime_clipboard_swift_test(clipboard-cloud-auto-policy tests/clipboard/CloudClipboardAutoReceivePolicyTest.swift
  src/backend/clipboard/BackendCloudClipboardAutoReceivePolicy.swift)
msime_clipboard_swift_test(clipboard-preview tests/emoji/EmojiClipboardPreviewTest.swift ${clipboard_row_sources})
msime_clipboard_swift_test(clipboard-tooltip tests/emoji/EmojiClipboardTooltipTest.swift ${clipboard_row_sources})
msime_clipboard_swift_test(clipboard-window-close tests/emoji/EmojiWindowCloseTest.swift src/backend/emoji/BackendEmojiWindowController.swift)
msime_clipboard_swift_test(clipboard-copy-recents tests/emoji/EmojiRecentsTest.swift
  src/backend/emoji/BackendEmojiSymbolGroups.swift src/backend/emoji/BackendEmojiCatalog.swift src/backend/emoji/BackendEmojiRecents.swift)

add_executable(clipboard-preferences-test tests/clipboard/ClipboardPreferencesTest.mm)
target_compile_options(clipboard-preferences-test PRIVATE -fobjc-arc -Wall -Wextra -Werror -UNDEBUG)
target_link_libraries(clipboard-preferences-test PRIVATE "-framework Foundation")
add_dependencies(macos-clipboard-tests clipboard-preferences-test)
add_test(NAME clipboard-preferences COMMAND clipboard-preferences-test)
set_tests_properties(clipboard-preferences PROPERTIES LABELS "clipboard-local" TIMEOUT 30)
