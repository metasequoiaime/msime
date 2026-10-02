cmake_minimum_required(VERSION 3.25)
set(CMAKE_OSX_DEPLOYMENT_TARGET "13.0" CACHE STRING "Minimum macOS version; match the Cargo build")
project(MSIMEClientMacOS LANGUAGES CXX OBJCXX)
enable_testing()
include_directories("${CMAKE_CURRENT_SOURCE_DIR}/src/backend" "${CMAKE_CURRENT_SOURCE_DIR}/src/voice" "${CMAKE_CURRENT_SOURCE_DIR}/src/candidate" "${CMAKE_CURRENT_SOURCE_DIR}/src/input" "${CMAKE_CURRENT_SOURCE_DIR}/src/cloud" "${CMAKE_CURRENT_SOURCE_DIR}/src/dictionary" "${CMAKE_CURRENT_SOURCE_DIR}/src/settings" "${CMAKE_CURRENT_SOURCE_DIR}/src/core")
add_executable(desktop-input-session-test tests/input/DesktopInputSessionTest.mm src/core/DesktopInputSession.mm)
target_compile_options(desktop-input-session-test PRIVATE -fobjc-arc -UNDEBUG -Wall -Wextra -Werror)
target_link_libraries(desktop-input-session-test PRIVATE "-framework AppKit")
add_test(NAME desktop-input-session COMMAND desktop-input-session-test)
add_executable(voice-provider-socket-test tests/voice/VoiceProviderSocketTest.mm)
target_compile_options(voice-provider-socket-test PRIVATE -fobjc-arc -UNDEBUG -Wall -Wextra -Werror)
target_link_libraries(voice-provider-socket-test PRIVATE "-framework Foundation")
add_test(NAME voice-provider-socket COMMAND voice-provider-socket-test)
add_executable(input-mode-hud-panel-test tests/input/InputModeHUDPanelTest.mm src/input/InputModeHUDPanel.mm)
target_compile_options(input-mode-hud-panel-test PRIVATE -fobjc-arc -UNDEBUG -Wall -Wextra -Werror)
target_link_libraries(input-mode-hud-panel-test PRIVATE "-framework AppKit")
add_test(NAME input-mode-hud-panel COMMAND input-mode-hud-panel-test)
# The selected input mode (中, 英, 日 or 한) follows the controller's state; this checks the mapping to the plist's mode identifiers and that a switch made by either side is not echoed back by the other.
add_executable(input-mode-identifiers-test tests/input/InputModeIdentifiersTest.mm)
target_compile_options(input-mode-identifiers-test PRIVATE -fobjc-arc -UNDEBUG -Wall -Wextra -Werror)
target_link_libraries(input-mode-identifiers-test PRIVATE "-framework Foundation")
add_test(NAME input-mode-identifiers COMMAND input-mode-identifiers-test)
add_executable(desktop-input-interop-test tests/input/DesktopInputInteropTest.mm src/core/DesktopInputSession.mm)
target_compile_options(desktop-input-interop-test PRIVATE -fobjc-arc -UNDEBUG -Wall -Wextra -Werror)
target_link_libraries(desktop-input-interop-test PRIVATE "-framework AppKit")
set(MSIME_PANEL_SESSION_PROBE "" CACHE FILEPATH "Optional Cargo panel_session_probe executable")
if(MSIME_PANEL_SESSION_PROBE)
  add_test(NAME desktop-input-interop COMMAND desktop-input-interop-test "${MSIME_PANEL_SESSION_PROBE}")
endif()
add_subdirectory(../../shared/voice shared-voice)
set(CMAKE_CXX_STANDARD 17)
set(CMAKE_OBJCXX_STANDARD 17)
add_executable(desktop-account-cancellation-test tests/core/DesktopAccountCancellationTest.mm src/core/DesktopCloudClipboard.mm src/core/DesktopInputSession.mm)
target_compile_options(desktop-account-cancellation-test PRIVATE -fobjc-arc -UNDEBUG -Wall -Wextra -Werror)
target_link_libraries(desktop-account-cancellation-test PRIVATE "-framework AppKit")
add_test(NAME desktop-account-cancellation COMMAND desktop-account-cancellation-test)
set_tests_properties(desktop-account-cancellation PROPERTIES TIMEOUT 10)
add_executable(desktop-cloud-dictionary-interop-test tests/dictionary/DesktopCloudDictionaryInteropTest.mm src/core/DesktopCloudClipboard.mm src/core/DesktopInputSession.mm)
target_compile_options(desktop-cloud-dictionary-interop-test PRIVATE -fobjc-arc -UNDEBUG -Wall -Wextra -Werror)
target_link_libraries(desktop-cloud-dictionary-interop-test PRIVATE "-framework AppKit")
set(MSIME_CLOUD_DICTIONARY_PROBE "" CACHE FILEPATH "Optional Cargo cloud_dictionary_probe executable")
if(MSIME_CLOUD_DICTIONARY_PROBE)
  add_test(NAME desktop-cloud-dictionary-interop COMMAND desktop-cloud-dictionary-interop-test "${MSIME_CLOUD_DICTIONARY_PROBE}")
  set_tests_properties(desktop-cloud-dictionary-interop PROPERTIES TIMEOUT 25)
endif()
add_executable(desktop-cloud-clipboard-interop-test tests/clipboard/DesktopCloudClipboardInteropTest.mm src/core/DesktopCloudClipboard.mm src/core/DesktopInputSession.mm)
target_compile_options(desktop-cloud-clipboard-interop-test PRIVATE -fobjc-arc -UNDEBUG -Wall -Wextra -Werror)
target_link_libraries(desktop-cloud-clipboard-interop-test PRIVATE "-framework AppKit")
set(MSIME_CLOUD_CLIPBOARD_PROBE "" CACHE FILEPATH "Optional Cargo cloud_clipboard_probe executable")
if(MSIME_CLOUD_CLIPBOARD_PROBE)
  add_test(NAME desktop-cloud-clipboard-interop COMMAND desktop-cloud-clipboard-interop-test "${MSIME_CLOUD_CLIPBOARD_PROBE}")
  set_tests_properties(desktop-cloud-clipboard-interop PROPERTIES TIMEOUT 25)
endif()
add_executable(desktop-settings-launcher-test tests/settings/DesktopSettingsLauncherTest.mm)
target_compile_options(desktop-settings-launcher-test PRIVATE -fobjc-arc -UNDEBUG -Wall -Wextra -Werror)
target_link_libraries(desktop-settings-launcher-test PRIVATE "-framework AppKit")
add_test(NAME desktop-settings-launcher COMMAND desktop-settings-launcher-test)
add_executable(shared-keyboard-host-test ../../crates/host-macos/native/keyboard_test.mm)
target_compile_options(shared-keyboard-host-test PRIVATE -fobjc-arc -UNDEBUG -Wall -Wextra -Werror)
target_link_libraries(shared-keyboard-host-test PRIVATE "-framework AppKit" "-framework CoreGraphics")
add_test(NAME shared-keyboard-host COMMAND shared-keyboard-host-test)
add_executable(shared-uninstaller-test ../../crates/host-macos/native/uninstaller_test.mm ../../crates/host-macos/native/uninstaller.mm)
target_compile_options(shared-uninstaller-test PRIVATE -fobjc-arc -UNDEBUG -Wall -Wextra -Werror)
target_link_libraries(shared-uninstaller-test PRIVATE "-framework Foundation" "-framework Security")
add_test(NAME shared-uninstaller COMMAND shared-uninstaller-test)
add_executable(shared-file-picker-test ../../crates/host-macos/native/file_picker_test.mm ../../crates/host-macos/native/file_picker.mm)
target_compile_options(shared-file-picker-test PRIVATE -fobjc-arc -UNDEBUG -Wall -Wextra -Werror)
target_link_libraries(shared-file-picker-test PRIVATE "-framework AppKit")
add_test(NAME shared-file-picker COMMAND shared-file-picker-test)
set(MSIME_HOST_LIBRARY "${CMAKE_CURRENT_SOURCE_DIR}/../../target/debug/libmsime_host_api.a" CACHE FILEPATH "Rust host static library for this architecture")
if(NOT EXISTS "${MSIME_HOST_LIBRARY}")
  message(FATAL_ERROR "Build msime-host-api with Cargo first")
endif()
find_package(SQLite3 REQUIRED)
if(TARGET SQLite3::SQLite3)
  set(SQLITE_TARGET SQLite3::SQLite3)
else()
  set(SQLITE_TARGET SQLite::SQLite3)
endif()
add_library(apple-client STATIC ../../shared/apple/MSIMEClientSession.mm ../../shared/apple/TextClient.mm)
target_compile_options(apple-client PRIVATE -fobjc-arc -Wall -Wextra -Werror)
target_include_directories(apple-client PUBLIC ../../shared/apple src ../../crates/host-api/include)
# AppKit for the marked text attributes: a composition carrying an already chosen phrase piece is
# drawn as two clauses, and NSUnderlineStyle* and the marked clause attribute live there. The iOS
# build of these same sources compiles that block out - UIKit's document proxy takes a plain string.
# CoreAudio and AudioToolbox for host-api's microphone capture (cpal), which every consumer of the archive links whether or not it records.
set(MSIME_HOST_LIBRARY_FRAMEWORKS "-framework Foundation" "-framework Security" "-framework CoreText" "-framework CoreAudio" "-framework AudioToolbox")
target_link_libraries(apple-client PUBLIC "${MSIME_HOST_LIBRARY}" ${SQLITE_TARGET} ${MSIME_HOST_LIBRARY_FRAMEWORKS} "-framework AppKit")
if(TARGET shared-doubao-auth)
  # The Rust archive needs the same Apple/system dependencies in shared ABI tests.
  target_link_libraries(shared-doubao-auth PRIVATE ${SQLITE_TARGET} ${MSIME_HOST_LIBRARY_FRAMEWORKS})
endif()
add_executable(MSIMEClientInputMethod MACOSX_BUNDLE src/cloud/CloudFileTransfer.mm src/input/input_method_main.mm src/cloud/CloudClipboardWindowController.mm src/cloud/CloudClipboardClient.mm src/cloud/CloudURLSession.mm src/core/AccountSessionManager.mm src/core/AccountAuthClient.mm src/core/AccountWindowController.mm src/core/AccountKeychain.mm src/cloud/CloudDictionaryClient.mm src/settings/PreferencesWindowController.mm src/core/SupportWindowController.mm src/voice/VoiceInputService.mm src/voice/VoiceWaveOverlay.mm src/voice/VoiceCuePlayer.mm src/voice/VoiceAudioMuter.mm src/voice/VoiceSettings.mm src/voice/VoiceProviderSettings.mm src/input/InputController.mm src/input/InputModeHUDPanel.mm src/candidate/CandidateSkinAppearance.mm src/settings/AppearancePreferences.mm src/core/DiagnosticLog.cpp src/candidate/CandidateSkin.cpp src/candidate/CandidateSkinPreviewView.mm src/candidate/SkinSettingsView.mm src/core/ChineseTextConversion.mm src/settings/ShuangpinKeymapPanel.mm src/input/InputSourceRegistration.mm src/core/FloatingToolbarPanel.mm src/core/UpdateController.mm src/dictionary/DictionaryWindowController.mm src/core/ClientDictionaryRuntime.mm src/dictionary/DictionaryInstaller.mm ../../crates/host-macos/native/uninstaller.mm)
target_sources(MSIMEClientInputMethod PRIVATE src/core/AISettingsWindow.mm src/core/ScreenKeyboardPanel.mm src/cloud/CloudCandidateRequest.mm src/core/CustomTranslationBatch.mm src/voice/VoicePCMBuffer.mm src/voice/HTTPVoiceRequest.mm)
target_sources(MSIMEClientInputMethod PRIVATE src/voice/DoubaoVoiceRequest.mm src/voice/LocalVoiceRequest.mm)
target_sources(MSIMEClientInputMethod PRIVATE src/core/DesktopInputSession.mm)
target_sources(MSIMEClientInputMethod PRIVATE src/core/UsageReporting.mm)
target_sources(MSIMEClientInputMethod PRIVATE src/core/DesktopCloudClipboard.mm)
target_link_libraries(MSIMEClientInputMethod PRIVATE "-framework CoreGraphics" "-framework CoreText" "-framework UniformTypeIdentifiers")
file(GLOB_RECURSE MSIME_BACKEND_SWIFT CONFIGURE_DEPENDS
  "${CMAKE_CURRENT_SOURCE_DIR}/../../shared/backend/account/*.swift"
  "${CMAKE_CURRENT_SOURCE_DIR}/../../shared/backend/clients/*.swift"
  "${CMAKE_CURRENT_SOURCE_DIR}/../../shared/backend/content/*.swift"
  "${CMAKE_CURRENT_SOURCE_DIR}/../../shared/backend/storage/*.swift"
  "${CMAKE_CURRENT_SOURCE_DIR}/../../shared/backend-ui/*.swift"
  "${CMAKE_CURRENT_SOURCE_DIR}/src/backend/*.swift")
set(MSIME_SWIFT_BACKEND_OUTPUT "${CMAKE_CURRENT_BINARY_DIR}/MSIMEBackend.dylib")
add_custom_command(OUTPUT "${MSIME_SWIFT_BACKEND_OUTPUT}"
  COMMAND "${CMAKE_CURRENT_SOURCE_DIR}/scripts/build_backend_swift.sh" "${MSIME_SWIFT_BACKEND_OUTPUT}"
  DEPENDS ${MSIME_BACKEND_SWIFT} "${CMAKE_CURRENT_SOURCE_DIR}/scripts/build_backend_swift.sh")
add_custom_target(macos-swift-backend DEPENDS "${MSIME_SWIFT_BACKEND_OUTPUT}")
add_dependencies(MSIMEClientInputMethod macos-swift-backend)
target_link_libraries(MSIMEClientInputMethod PRIVATE "${MSIME_SWIFT_BACKEND_OUTPUT}")
target_include_directories(MSIMEClientInputMethod PRIVATE src)
target_compile_options(MSIMEClientInputMethod PRIVATE -fobjc-arc -Wall -Wextra -Werror)
target_link_libraries(MSIMEClientInputMethod PRIVATE apple-client msime-voice-providers "-framework AppKit" "-framework InputMethodKit" "-framework Carbon" "-framework AVFoundation" "-framework Speech")
# Sparkle orders releases by CFBundleVersion, not by the marketing version, so a build that ships a
# fixed number can never present itself as newer than the one already installed. Derive it from the
# commit count: monotonic, reproducible from the source alone, and needing no release bookkeeping. A
# source tree without git history (an archive export) falls back to 1, which only means such a build
# cannot be offered as an update - it still installs and runs.
find_package(Git QUIET)
if(GIT_FOUND)
  execute_process(
    COMMAND "${GIT_EXECUTABLE}" rev-list --count HEAD
    WORKING_DIRECTORY "${CMAKE_CURRENT_SOURCE_DIR}"
    OUTPUT_VARIABLE MSIME_BUILD_NUMBER
    OUTPUT_STRIP_TRAILING_WHITESPACE
    ERROR_QUIET
    RESULT_VARIABLE MSIME_BUILD_NUMBER_RESULT)
endif()
if(NOT MSIME_BUILD_NUMBER MATCHES "^[0-9]+$")
  set(MSIME_BUILD_NUMBER 1)
endif()
set(MACOSX_BUNDLE_BUNDLE_VERSION "${MSIME_BUILD_NUMBER}")
message(STATUS "Bundle build number: ${MACOSX_BUNDLE_BUNDLE_VERSION}")
set_target_properties(MSIMEClientInputMethod PROPERTIES MACOSX_BUNDLE_INFO_PLIST "${CMAKE_CURRENT_SOURCE_DIR}/Info.plist.in")
set(MSIME_LEGAL_DIRECTORY "${CMAKE_CURRENT_BINARY_DIR}/Licenses")
file(MAKE_DIRECTORY "${MSIME_LEGAL_DIRECTORY}")
configure_file("${CMAKE_CURRENT_SOURCE_DIR}/../../LICENSE" "${MSIME_LEGAL_DIRECTORY}/GPL-3.0.txt" COPYONLY)
configure_file("${CMAKE_CURRENT_SOURCE_DIR}/resources/Licenses/THIRD_PARTY_NOTICES.txt" "${MSIME_LEGAL_DIRECTORY}/THIRD_PARTY_NOTICES.txt" COPYONLY)
# The dictionary data in the pinned release, the notices of the helpcode tables stage-resources.sh puts in EngineResources/helpcodes (the five the Engine brought and jiajia), and zinnia, whose recognizer crates/engine/src/handwriting ports and host-api links in. The first and last come from resources/licenses, the copies every platform ships.
configure_file("${CMAKE_CURRENT_SOURCE_DIR}/../../resources/licenses/msime-engine-dictionary-NOTICE.md" "${MSIME_LEGAL_DIRECTORY}/msime-engine-dictionary-NOTICE.md" COPYONLY)
configure_file("${CMAKE_CURRENT_SOURCE_DIR}/../../resources/helpcodes/ENGINE-NOTICE.md" "${MSIME_LEGAL_DIRECTORY}/msime-engine-helpcode-NOTICE.md" COPYONLY)
configure_file("${CMAKE_CURRENT_SOURCE_DIR}/../../resources/helpcodes/NOTICE.md" "${MSIME_LEGAL_DIRECTORY}/msime-helpcode-jiajia-NOTICE.md" COPYONLY)
configure_file("${CMAKE_CURRENT_SOURCE_DIR}/../../resources/licenses/Zinnia-LICENSE.txt" "${MSIME_LEGAL_DIRECTORY}/Zinnia-LICENSE.txt" COPYONLY)
# The place names `@` mode offers, compiled into the engine from modood/Administrative-divisions-of-China (WTFPL).
configure_file("${CMAKE_CURRENT_SOURCE_DIR}/../../resources/licenses/Administrative-divisions-of-China-WTFPL.txt" "${MSIME_LEGAL_DIRECTORY}/Administrative-divisions-of-China-WTFPL.txt" COPYONLY)
# The Korean Hanja table, compiled into the engine from libhangul's data/hanja/hanja.txt (BSD-3-Clause, whose clause 2 requires the notice in binary distributions).
configure_file("${CMAKE_CURRENT_SOURCE_DIR}/../../resources/licenses/libhangul-hanja-BSD-3-Clause.txt" "${MSIME_LEGAL_DIRECTORY}/libhangul-hanja-BSD-3-Clause.txt" COPYONLY)
# The Cantonese and Zhuyin schemes, whose syllables and words come from the dictionaries in language-dictionaries/ (when installed), built from rime-cantonese (CC BY 4.0, which requires attribution) and libchewing-data (LGPL-2.1-or-later, which requires the licence text and a source pointer).
configure_file("${CMAKE_CURRENT_SOURCE_DIR}/../../resources/licenses/rime-cantonese-CC-BY-4.0.txt" "${MSIME_LEGAL_DIRECTORY}/rime-cantonese-CC-BY-4.0.txt" COPYONLY)
configure_file("${CMAKE_CURRENT_SOURCE_DIR}/../../resources/licenses/libchewing-data-LGPL-2.1.txt" "${MSIME_LEGAL_DIRECTORY}/libchewing-data-LGPL-2.1.txt" COPYONLY)
# The vi crate the engine links for Vietnamese Telex and VNI; MIT requires its copyright and permission notice in every copy, and macOS is where the Vietnamese scheme ships.
configure_file("${CMAKE_CURRENT_SOURCE_DIR}/../../resources/licenses/vi-MIT.txt" "${MSIME_LEGAL_DIRECTORY}/vi-MIT.txt" COPYONLY)
# symphonia and triple_buffer, which host-api links in for key sounds and music, and rink-core, which the engine links in for V-mode unit conversion, are MPL-2.0: the licence text ships with the binary, and THIRD_PARTY_NOTICES.txt says where their source is.
configure_file("${CMAKE_CURRENT_SOURCE_DIR}/../../resources/licenses/MPL-2.0.txt" "${MSIME_LEGAL_DIRECTORY}/MPL-2.0.txt" COPYONLY)
# The on-device speech runtime staged into Contents/Frameworks below. The upstream archive carries no license file, so the texts come from the copies the other desktops ship: sherpa-onnx's beside its vendored C header, ONNX Runtime's pinned under platforms/linux.
configure_file("${CMAKE_CURRENT_SOURCE_DIR}/../../shared/voice/third_party/sherpa-onnx/LICENSE" "${MSIME_LEGAL_DIRECTORY}/sherpa-onnx-Apache-2.0.txt" COPYONLY)
configure_file("${CMAKE_CURRENT_SOURCE_DIR}/../linux/data/licenses/onnxruntime-MIT.txt" "${MSIME_LEGAL_DIRECTORY}/onnxruntime-MIT.txt" COPYONLY)
configure_file("${CMAKE_CURRENT_SOURCE_DIR}/../linux/data/licenses/onnxruntime-ThirdPartyNotices.txt" "${MSIME_LEGAL_DIRECTORY}/onnxruntime-ThirdPartyNotices.txt" COPYONLY)
file(GLOB MSIME_LEGAL_RESOURCES CONFIGURE_DEPENDS "${MSIME_LEGAL_DIRECTORY}/*")
target_sources(MSIMEClientInputMethod PRIVATE ${MSIME_LEGAL_RESOURCES})
set_source_files_properties(${MSIME_LEGAL_RESOURCES} PROPERTIES MACOSX_PACKAGE_LOCATION "Resources/Licenses")
# The Dock takes its label from the bundle's filename — not CFBundleDisplayName, CFBundleName or a
# localized InfoPlist.strings, all of which it ignores here. Naming the product is the only lever.
set_target_properties(MSIMEClientInputMethod PROPERTIES OUTPUT_NAME "水杉输入法")
# The preferences window runs under NSApplicationActivationPolicyRegular, so the bundle appears in
# the Dock and needs an icon; without CFBundleIconFile plus the file itself it shows the generic one.
target_sources(MSIMEClientInputMethod PRIVATE "${CMAKE_CURRENT_SOURCE_DIR}/resources/MSIMEClientInputMethod.icns")
set_source_files_properties("${CMAKE_CURRENT_SOURCE_DIR}/resources/MSIMEClientInputMethod.icns"
  PROPERTIES MACOSX_PACKAGE_LOCATION Resources)
# The input menu's own icon, a template TIFF rather than the application icon: the bundle's logo, which the switch HUD also shows, and the nine input modes' badged variants of it. See the comment beside tsInputMethodIconFileKey in Info.plist.in for why the logo and the application icon cannot be the same file.
set(MSIME_MENU_ICON_RESOURCES
  "${CMAKE_CURRENT_SOURCE_DIR}/resources/MSIMEClientInputMethodMenuIcon.tiff"
  "${CMAKE_CURRENT_SOURCE_DIR}/resources/MSIMEClientInputMethodMenuIconChinese.tiff"
  "${CMAKE_CURRENT_SOURCE_DIR}/resources/MSIMEClientInputMethodMenuIconShuangpin.tiff"
  "${CMAKE_CURRENT_SOURCE_DIR}/resources/MSIMEClientInputMethodMenuIconWubi.tiff"
  "${CMAKE_CURRENT_SOURCE_DIR}/resources/MSIMEClientInputMethodMenuIconCantonese.tiff"
  "${CMAKE_CURRENT_SOURCE_DIR}/resources/MSIMEClientInputMethodMenuIconZhuyin.tiff"
  "${CMAKE_CURRENT_SOURCE_DIR}/resources/MSIMEClientInputMethodMenuIconJapanese.tiff"
  "${CMAKE_CURRENT_SOURCE_DIR}/resources/MSIMEClientInputMethodMenuIconKorean.tiff"
  "${CMAKE_CURRENT_SOURCE_DIR}/resources/MSIMEClientInputMethodMenuIconVietnamese.tiff"
  "${CMAKE_CURRENT_SOURCE_DIR}/resources/MSIMEClientInputMethodMenuIconEnglish.tiff")
target_sources(MSIMEClientInputMethod PRIVATE ${MSIME_MENU_ICON_RESOURCES})
set_source_files_properties(${MSIME_MENU_ICON_RESOURCES} PROPERTIES MACOSX_PACKAGE_LOCATION Resources)
# The voice start/end cues are the product's own, not system sounds. They are staged straight from the Windows installer's copy rather than adding another copy to the tree; VoiceCuePlayer looks them up in Resources/audios, mirroring the assets/audios layout Windows installs.
set(MSIME_VOICE_CUE_RESOURCES
  "${CMAKE_CURRENT_SOURCE_DIR}/../windows/installer/assets/audios/start.mp3"
  "${CMAKE_CURRENT_SOURCE_DIR}/../windows/installer/assets/audios/end.mp3")
target_sources(MSIMEClientInputMethod PRIVATE ${MSIME_VOICE_CUE_RESOURCES})
set_source_files_properties(${MSIME_VOICE_CUE_RESOURCES} PROPERTIES MACOSX_PACKAGE_LOCATION "Resources/audios")
# The built-in sound packs, synthesized by scripts/generate_sound_packs.py, one folder per pack under Resources/sound-packs: the directory the input method and the settings page name to client-core as the built-in pack root. Beside EngineResources rather than in it, because that directory has to hold exactly what the dictionary lock pins.
# Every folder there is staged, so a pack the generator adds ships without being named here too.
file(GLOB MSIME_SOUND_PACK_DIRECTORIES LIST_DIRECTORIES true CONFIGURE_DEPENDS "${CMAKE_CURRENT_SOURCE_DIR}/../../resources/sound-packs/*")
foreach(MSIME_SOUND_PACK_DIRECTORY ${MSIME_SOUND_PACK_DIRECTORIES})
  if(NOT IS_DIRECTORY "${MSIME_SOUND_PACK_DIRECTORY}")
    continue()
  endif()
  get_filename_component(MSIME_SOUND_PACK "${MSIME_SOUND_PACK_DIRECTORY}" NAME)
  file(GLOB MSIME_SOUND_PACK_FILES CONFIGURE_DEPENDS "${MSIME_SOUND_PACK_DIRECTORY}/*")
  target_sources(MSIMEClientInputMethod PRIVATE ${MSIME_SOUND_PACK_FILES})
  set_source_files_properties(${MSIME_SOUND_PACK_FILES} PROPERTIES MACOSX_PACKAGE_LOCATION "Resources/sound-packs/${MSIME_SOUND_PACK}")
endforeach()
# LaunchServices takes the Dock and menu bar name from a localized InfoPlist.strings, not from the
# CFBundleDisplayName in the plist; without these the Dock falls back to the executable's filename.
# The TIS identifier keys name the entry in the system input menu.
#
# The location has to name the .lproj directory. Every locale's file is called InfoPlist.strings, so a
# bare `Resources` flattens them into one file and the last locale overwrites the rest - the bundle then
# carries only the English names and the input menu loses 水杉输入法. bundle-contents checks each locale.
foreach(MSIME_BUNDLE_LOCALE zh-Hans en)
  target_sources(MSIMEClientInputMethod
    PRIVATE "${CMAKE_CURRENT_SOURCE_DIR}/resources/${MSIME_BUNDLE_LOCALE}.lproj/InfoPlist.strings")
  set_source_files_properties("${CMAKE_CURRENT_SOURCE_DIR}/resources/${MSIME_BUNDLE_LOCALE}.lproj/InfoPlist.strings"
    PROPERTIES MACOSX_PACKAGE_LOCATION "Resources/${MSIME_BUNDLE_LOCALE}.lproj")
endforeach()
# On-device recognition runs in its own process: the input method spawns Contents/MacOS/msime-voice-local (see src/voice/LocalVoiceRequest.h) and the helper loads the pinned sherpa-onnx runtime from Contents/Frameworks, where shared/voice/LocalAsr.cpp looks beside an executable in Contents/MacOS. The runtime is the prebuilt upstream library pinned in resources/voice-runtime.lock.json; scripts/fetch_voice_runtime.py downloads and verifies it once into MSIME_VOICE_RUNTIME_DIR and reuses its cached archive afterwards, and the stamp reruns it whenever the lock or the script changes.
find_package(Python3 REQUIRED COMPONENTS Interpreter)
set(MSIME_VOICE_RUNTIME_DIR "${CMAKE_CURRENT_SOURCE_DIR}/../../target/voice-runtime/macos" CACHE PATH "Where scripts/fetch_voice_runtime.py puts the macOS sherpa-onnx runtime")
set(MSIME_VOICE_RUNTIME_LIBRARY "${MSIME_VOICE_RUNTIME_DIR}/libsherpa-onnx-c-api.dylib")
set(MSIME_VOICE_RUNTIME_STAMP "${CMAKE_CURRENT_BINARY_DIR}/voice-runtime.stamp")
add_custom_command(OUTPUT "${MSIME_VOICE_RUNTIME_STAMP}"
  COMMAND "${Python3_EXECUTABLE}" "${CMAKE_CURRENT_SOURCE_DIR}/../../scripts/fetch_voice_runtime.py" --platform macos --out "${MSIME_VOICE_RUNTIME_DIR}"
  COMMAND "${CMAKE_COMMAND}" -E touch "${MSIME_VOICE_RUNTIME_STAMP}"
  DEPENDS "${CMAKE_CURRENT_SOURCE_DIR}/../../resources/voice-runtime.lock.json" "${CMAKE_CURRENT_SOURCE_DIR}/../../scripts/fetch_voice_runtime.py"
  COMMENT "Fetching the pinned sherpa-onnx runtime for on-device speech recognition"
  VERBATIM)
add_custom_target(macos-voice-runtime DEPENDS "${MSIME_VOICE_RUNTIME_STAMP}")
add_dependencies(MSIMEClientInputMethod macos-voice-runtime msime-voice-local)
add_custom_command(TARGET MSIMEClientInputMethod POST_BUILD
  COMMAND "${CMAKE_COMMAND}" -E make_directory "$<TARGET_BUNDLE_DIR:MSIMEClientInputMethod>/Contents/Frameworks"
  COMMAND "${CMAKE_COMMAND}" -E copy_if_different "${MSIME_SWIFT_BACKEND_OUTPUT}" "$<TARGET_BUNDLE_DIR:MSIMEClientInputMethod>/Contents/Frameworks/MSIMEBackend.dylib"
  COMMAND "${CMAKE_COMMAND}" -E copy_if_different "${MSIME_VOICE_RUNTIME_LIBRARY}" "$<TARGET_BUNDLE_DIR:MSIMEClientInputMethod>/Contents/Frameworks/libsherpa-onnx-c-api.dylib"
  COMMAND "${CMAKE_COMMAND}" -E copy_if_different "$<TARGET_FILE:msime-voice-local>" "$<TARGET_BUNDLE_DIR:MSIMEClientInputMethod>/Contents/MacOS/msime-voice-local")
set(MSIME_OPTIONS_FILE "" CACHE FILEPATH "Optional development runtime configuration; never distribute this file")
if(MSIME_OPTIONS_FILE)
  target_sources(MSIMEClientInputMethod PRIVATE "${MSIME_OPTIONS_FILE}")
  set_source_files_properties("${MSIME_OPTIONS_FILE}" PROPERTIES MACOSX_PACKAGE_LOCATION Resources)
endif()
include(ClipboardTests.cmake)
set(MSIME_DESKTOP_DICTIONARY_PROVIDER_SOURCES
  ../../shared/backend/account/BackendAccountClient.swift ../../shared/backend/account/BackendAccountSession.swift
  ../../shared/backend/content/BackendDictionaryClient.swift ../../shared/backend/clients/BackendCandidateClient.swift
  ../../shared/backend/clients/BackendSnapshotClient.swift src/backend/dictionary/BackendCloudDictionaryProvider.swift
  src/backend/dictionary/BackendLocalSnapshot.swift src/backend/dictionary/BackendFileTransfer.swift src/backend/dictionary/BackendDesktopSnapshots.swift
  tests/dictionary/DesktopCloudDictionaryProviderTest.swift)
list(TRANSFORM MSIME_DESKTOP_DICTIONARY_PROVIDER_SOURCES PREPEND "${CMAKE_CURRENT_SOURCE_DIR}/")
add_custom_command(OUTPUT "${CMAKE_CURRENT_BINARY_DIR}/desktop-dictionary-provider-test"
  COMMAND "${MSIME_XCRUN}" swiftc -Onone -parse-as-library
    -target "${CMAKE_HOST_SYSTEM_PROCESSOR}-apple-macosx${CMAKE_OSX_DEPLOYMENT_TARGET}"
    ${MSIME_DESKTOP_DICTIONARY_PROVIDER_SOURCES} -o "${CMAKE_CURRENT_BINARY_DIR}/desktop-dictionary-provider-test"
  DEPENDS ${MSIME_DESKTOP_DICTIONARY_PROVIDER_SOURCES} VERBATIM)
add_custom_target(desktop-dictionary-provider-test-build ALL DEPENDS "${CMAKE_CURRENT_BINARY_DIR}/desktop-dictionary-provider-test")
add_test(NAME desktop-dictionary-provider COMMAND "${CMAKE_CURRENT_BINARY_DIR}/desktop-dictionary-provider-test")
set_tests_properties(desktop-dictionary-provider PROPERTIES TIMEOUT 30)
set(MSIME_DESKTOP_CLOUD_PROVIDER_SOURCES
  ../../shared/backend/account/BackendAccountClient.swift ../../shared/backend/account/BackendAccountSession.swift
  ../../shared/backend/clients/BackendClipboardClient.swift src/backend/clipboard/BackendCloudClipboardProvider.swift
  tests/clipboard/DesktopCloudClipboardProviderTest.swift)
list(TRANSFORM MSIME_DESKTOP_CLOUD_PROVIDER_SOURCES PREPEND "${CMAKE_CURRENT_SOURCE_DIR}/")
add_custom_command(OUTPUT "${CMAKE_CURRENT_BINARY_DIR}/desktop-cloud-provider-test"
  COMMAND "${MSIME_XCRUN}" swiftc -Onone -parse-as-library
    -target "${CMAKE_HOST_SYSTEM_PROCESSOR}-apple-macosx${CMAKE_OSX_DEPLOYMENT_TARGET}"
    ${MSIME_DESKTOP_CLOUD_PROVIDER_SOURCES} -o "${CMAKE_CURRENT_BINARY_DIR}/desktop-cloud-provider-test"
  DEPENDS ${MSIME_DESKTOP_CLOUD_PROVIDER_SOURCES} VERBATIM)
add_custom_target(desktop-cloud-provider-test-build ALL DEPENDS "${CMAKE_CURRENT_BINARY_DIR}/desktop-cloud-provider-test")
add_test(NAME desktop-cloud-provider COMMAND "${CMAKE_CURRENT_BINARY_DIR}/desktop-cloud-provider-test")
set_tests_properties(desktop-cloud-provider PROPERTIES TIMEOUT 30)
# The account backend harness compiles the whole Swift backend with its test entry point, the same source list build_backend_swift.sh links into the dylib, so the script owns the list rather than a second copy here.
add_custom_command(OUTPUT "${CMAKE_CURRENT_BINARY_DIR}/backend-account-swift-test"
  COMMAND "${CMAKE_CURRENT_SOURCE_DIR}/scripts/test_backend_account_swift.sh" "${CMAKE_CURRENT_BINARY_DIR}/backend-account-swift-test"
  DEPENDS ${MSIME_BACKEND_SWIFT} "${CMAKE_CURRENT_SOURCE_DIR}/tests/core/BackendAccountTests.swift"
    "${CMAKE_CURRENT_SOURCE_DIR}/scripts/test_backend_account_swift.sh" VERBATIM)
add_custom_target(backend-account-swift-test-build ALL DEPENDS "${CMAKE_CURRENT_BINARY_DIR}/backend-account-swift-test")
add_test(NAME backend-account-swift COMMAND "${CMAKE_CURRENT_BINARY_DIR}/backend-account-swift-test")
set_tests_properties(backend-account-swift PROPERTIES TIMEOUT 60)
set(MSIME_EMOJI_WINDOW_CHROME_SOURCES src/backend/emoji/BackendEmojiWindowController.swift tests/emoji/EmojiWindowChromeTest.swift)
list(TRANSFORM MSIME_EMOJI_WINDOW_CHROME_SOURCES PREPEND "${CMAKE_CURRENT_SOURCE_DIR}/")
add_custom_command(OUTPUT "${CMAKE_CURRENT_BINARY_DIR}/emoji-window-chrome-test"
  COMMAND "${MSIME_XCRUN}" swiftc -Onone -parse-as-library -target "${CMAKE_HOST_SYSTEM_PROCESSOR}-apple-macosx${CMAKE_OSX_DEPLOYMENT_TARGET}"
    ${MSIME_EMOJI_WINDOW_CHROME_SOURCES} -o "${CMAKE_CURRENT_BINARY_DIR}/emoji-window-chrome-test"
  DEPENDS ${MSIME_EMOJI_WINDOW_CHROME_SOURCES} VERBATIM)
add_custom_target(emoji-window-chrome-test-build ALL DEPENDS "${CMAKE_CURRENT_BINARY_DIR}/emoji-window-chrome-test")
add_test(NAME emoji-window-chrome COMMAND "${CMAKE_CURRENT_BINARY_DIR}/emoji-window-chrome-test")
set_tests_properties(emoji-window-chrome PROPERTIES LABELS "emoji-local" TIMEOUT 30)
set(MSIME_EMOJI_APPEARANCE_TEST_SOURCES src/backend/emoji/BackendEmojiAppearance.swift tests/emoji/EmojiAppearanceTest.swift)
list(TRANSFORM MSIME_EMOJI_APPEARANCE_TEST_SOURCES PREPEND "${CMAKE_CURRENT_SOURCE_DIR}/")
add_custom_command(OUTPUT "${CMAKE_CURRENT_BINARY_DIR}/emoji-appearance-test"
  COMMAND "${MSIME_XCRUN}" swiftc -Onone
    -target "${CMAKE_HOST_SYSTEM_PROCESSOR}-apple-macosx${CMAKE_OSX_DEPLOYMENT_TARGET}"
    ${MSIME_EMOJI_APPEARANCE_TEST_SOURCES} -o "${CMAKE_CURRENT_BINARY_DIR}/emoji-appearance-test"
  DEPENDS ${MSIME_EMOJI_APPEARANCE_TEST_SOURCES} VERBATIM)
add_custom_target(emoji-appearance-test-build ALL DEPENDS "${CMAKE_CURRENT_BINARY_DIR}/emoji-appearance-test")
add_test(NAME emoji-appearance COMMAND "${CMAKE_CURRENT_BINARY_DIR}/emoji-appearance-test")
set_tests_properties(emoji-appearance PROPERTIES LABELS "emoji-local" TIMEOUT 30)
set(MSIME_EMOJI_DELIVERY_NOTICE_TEST_SOURCES src/backend/emoji/BackendEmojiAppearance.swift src/backend/emoji/BackendEmojiDeliveryNotice.swift tests/emoji/EmojiDeliveryNoticeTest.swift)
list(TRANSFORM MSIME_EMOJI_DELIVERY_NOTICE_TEST_SOURCES PREPEND "${CMAKE_CURRENT_SOURCE_DIR}/")
add_custom_command(OUTPUT "${CMAKE_CURRENT_BINARY_DIR}/emoji-delivery-notice-test"
  COMMAND "${MSIME_XCRUN}" swiftc -Onone
    -target "${CMAKE_HOST_SYSTEM_PROCESSOR}-apple-macosx${CMAKE_OSX_DEPLOYMENT_TARGET}"
    ${MSIME_EMOJI_DELIVERY_NOTICE_TEST_SOURCES} -o "${CMAKE_CURRENT_BINARY_DIR}/emoji-delivery-notice-test"
  DEPENDS ${MSIME_EMOJI_DELIVERY_NOTICE_TEST_SOURCES} VERBATIM)
add_custom_target(emoji-delivery-notice-test-build ALL DEPENDS "${CMAKE_CURRENT_BINARY_DIR}/emoji-delivery-notice-test")
add_test(NAME emoji-delivery-notice COMMAND "${CMAKE_CURRENT_BINARY_DIR}/emoji-delivery-notice-test")
set_tests_properties(emoji-delivery-notice PROPERTIES LABELS "emoji-local" TIMEOUT 30)
set(MSIME_EMOJI_EMPTY_TEST_SOURCES src/backend/emoji/BackendEmojiAppearance.swift src/backend/emoji/BackendEmojiEmptyState.swift tests/emoji/EmojiEmptyStateTest.swift)
list(TRANSFORM MSIME_EMOJI_EMPTY_TEST_SOURCES PREPEND "${CMAKE_CURRENT_SOURCE_DIR}/")
add_custom_command(OUTPUT "${CMAKE_CURRENT_BINARY_DIR}/emoji-empty-test"
  COMMAND "${MSIME_XCRUN}" swiftc -Onone -target "${CMAKE_HOST_SYSTEM_PROCESSOR}-apple-macosx${CMAKE_OSX_DEPLOYMENT_TARGET}"
    ${MSIME_EMOJI_EMPTY_TEST_SOURCES} -o "${CMAKE_CURRENT_BINARY_DIR}/emoji-empty-test"
  DEPENDS ${MSIME_EMOJI_EMPTY_TEST_SOURCES} VERBATIM)
add_custom_target(emoji-empty-test-build ALL DEPENDS "${CMAKE_CURRENT_BINARY_DIR}/emoji-empty-test")
add_test(NAME emoji-empty COMMAND "${CMAKE_CURRENT_BINARY_DIR}/emoji-empty-test")
set_tests_properties(emoji-empty PROPERTIES LABELS "emoji-local" TIMEOUT 30)
set(MSIME_EMOJI_EXIT_SOURCES src/backend/emoji/BackendEmojiExit.swift tests/emoji/EmojiExitTest.swift)
list(TRANSFORM MSIME_EMOJI_EXIT_SOURCES PREPEND "${CMAKE_CURRENT_SOURCE_DIR}/")
add_custom_command(OUTPUT "${CMAKE_CURRENT_BINARY_DIR}/emoji-exit-test"
  COMMAND "${MSIME_XCRUN}" swiftc -Onone
    -target "${CMAKE_HOST_SYSTEM_PROCESSOR}-apple-macosx${CMAKE_OSX_DEPLOYMENT_TARGET}"
    ${MSIME_EMOJI_EXIT_SOURCES} -o "${CMAKE_CURRENT_BINARY_DIR}/emoji-exit-test"
  DEPENDS ${MSIME_EMOJI_EXIT_SOURCES} VERBATIM)
add_custom_target(emoji-exit-test-build ALL DEPENDS "${CMAKE_CURRENT_BINARY_DIR}/emoji-exit-test")
add_test(NAME emoji-exit COMMAND "${CMAKE_CURRENT_BINARY_DIR}/emoji-exit-test")
set_tests_properties(emoji-exit PROPERTIES LABELS "emoji-local" TIMEOUT 30)
add_executable(emoji-host-cursor-test tests/emoji/EmojiHostCursorTest.mm)
target_compile_options(emoji-host-cursor-test PRIVATE -fobjc-arc -UNDEBUG -Wall -Wextra -Werror)
target_link_libraries(emoji-host-cursor-test PRIVATE apple-client)
add_test(NAME emoji-host-cursor COMMAND emoji-host-cursor-test)
set_tests_properties(emoji-host-cursor PROPERTIES LABELS "emoji-local" TIMEOUT 30)
set(MSIME_EMOJI_SWIFT_HOST_SOURCES src/backend/emoji/BackendEmojiSymbolGroups.swift src/backend/emoji/BackendEmojiCatalog.swift
  src/backend/emoji/BackendEmojiCatalogCursor.swift tests/emoji/EmojiSwiftHostTest.swift)
list(TRANSFORM MSIME_EMOJI_SWIFT_HOST_SOURCES PREPEND "${CMAKE_CURRENT_SOURCE_DIR}/")
# swiftc links this one itself, against apple-client, so a sanitized apple-client needs the sanitizer runtime on this link line too; CMake adds it to the ones it drives.
set(MSIME_EMOJI_SWIFT_HOST_SANITIZE "")
if(CMAKE_OBJCXX_FLAGS MATCHES "-fsanitize=[^ ]*address")
  set(MSIME_EMOJI_SWIFT_HOST_SANITIZE -sanitize=address)
endif()
add_custom_command(OUTPUT "${CMAKE_CURRENT_BINARY_DIR}/emoji-swift-host-test"
  COMMAND "${MSIME_XCRUN}" swiftc -Onone ${MSIME_EMOJI_SWIFT_HOST_SANITIZE}
    -target "${CMAKE_HOST_SYSTEM_PROCESSOR}-apple-macosx${CMAKE_OSX_DEPLOYMENT_TARGET}"
    ${MSIME_EMOJI_SWIFT_HOST_SOURCES}
    -Xlinker -force_load -Xlinker "$<TARGET_FILE:apple-client>"
    "${MSIME_HOST_LIBRARY}" -lsqlite3 -lc++ -framework Foundation -framework Security -framework CoreText -framework CoreAudio -framework AudioToolbox -framework AppKit
    -o "${CMAKE_CURRENT_BINARY_DIR}/emoji-swift-host-test"
  DEPENDS ${MSIME_EMOJI_SWIFT_HOST_SOURCES} apple-client "${MSIME_HOST_LIBRARY}" VERBATIM)
add_custom_target(emoji-swift-host-test-build ALL DEPENDS "${CMAKE_CURRENT_BINARY_DIR}/emoji-swift-host-test")
add_test(NAME emoji-swift-host COMMAND "${CMAKE_CURRENT_BINARY_DIR}/emoji-swift-host-test")
set_tests_properties(emoji-swift-host PROPERTIES LABELS "emoji-local" TIMEOUT 30)
set(MSIME_EMOJI_HOME_TEST_SOURCES src/backend/emoji/BackendEmojiSymbolGroups.swift src/backend/emoji/BackendEmojiCatalog.swift src/backend/emoji/BackendEmojiCatalogCursor.swift src/backend/emoji/BackendEmojiClipboardDisabled.swift src/backend/emoji/BackendEmojiEmptyState.swift src/backend/emoji/BackendEmojiBackButton.swift
  src/backend/emoji/BackendEmojiAppearance.swift src/backend/emoji/BackendEmojiScroll.swift src/backend/emoji/BackendEmojiHomeHeading.swift src/backend/emoji/BackendEmojiHome.swift src/backend/emoji/BackendEmojiHomeNavigation.swift
  src/backend/emoji/BackendEmojiKeyboard.swift src/backend/emoji/BackendEmojiFlow.swift src/backend/emoji/BackendEmojiGrid.swift src/backend/emoji/BackendEmojiNavigation.swift tests/emoji/EmojiHomeTest.swift)
list(TRANSFORM MSIME_EMOJI_HOME_TEST_SOURCES PREPEND "${CMAKE_CURRENT_SOURCE_DIR}/")
add_custom_command(OUTPUT "${CMAKE_CURRENT_BINARY_DIR}/emoji-home-test"
  COMMAND "${MSIME_XCRUN}" swiftc -Onone
    -target "${CMAKE_HOST_SYSTEM_PROCESSOR}-apple-macosx${CMAKE_OSX_DEPLOYMENT_TARGET}"
    ${MSIME_EMOJI_HOME_TEST_SOURCES} -o "${CMAKE_CURRENT_BINARY_DIR}/emoji-home-test"
  DEPENDS ${MSIME_EMOJI_HOME_TEST_SOURCES} VERBATIM)
add_custom_target(emoji-home-test-build ALL DEPENDS "${CMAKE_CURRENT_BINARY_DIR}/emoji-home-test")
add_test(NAME emoji-home COMMAND "${CMAKE_CURRENT_BINARY_DIR}/emoji-home-test")
set_tests_properties(emoji-home PROPERTIES LABELS "emoji-local" TIMEOUT 30)
set(MSIME_EMOJI_FLOW_TEST_SOURCES src/backend/emoji/BackendEmojiSymbolGroups.swift src/backend/emoji/BackendEmojiCatalog.swift
  src/backend/emoji/BackendEmojiAppearance.swift src/backend/emoji/BackendEmojiFlow.swift tests/emoji/EmojiFlowTest.swift)
list(TRANSFORM MSIME_EMOJI_FLOW_TEST_SOURCES PREPEND "${CMAKE_CURRENT_SOURCE_DIR}/")
add_custom_command(OUTPUT "${CMAKE_CURRENT_BINARY_DIR}/emoji-flow-test"
  COMMAND "${MSIME_XCRUN}" swiftc -Onone
    -target "${CMAKE_HOST_SYSTEM_PROCESSOR}-apple-macosx${CMAKE_OSX_DEPLOYMENT_TARGET}"
    ${MSIME_EMOJI_FLOW_TEST_SOURCES} -o "${CMAKE_CURRENT_BINARY_DIR}/emoji-flow-test"
  DEPENDS ${MSIME_EMOJI_FLOW_TEST_SOURCES} VERBATIM)
add_custom_target(emoji-flow-test-build ALL DEPENDS "${CMAKE_CURRENT_BINARY_DIR}/emoji-flow-test")
add_test(NAME emoji-flow COMMAND "${CMAKE_CURRENT_BINARY_DIR}/emoji-flow-test")
set_tests_properties(emoji-flow PROPERTIES LABELS "emoji-local" TIMEOUT 30)
set(MSIME_EMOJI_GRID_TEST_SOURCES src/backend/emoji/BackendEmojiSymbolGroups.swift src/backend/emoji/BackendEmojiCatalog.swift
  src/backend/emoji/BackendEmojiAppearance.swift src/backend/emoji/BackendEmojiFlow.swift src/backend/emoji/BackendEmojiGrid.swift
  src/backend/emoji/BackendEmojiKeyboard.swift tests/emoji/EmojiGridTest.swift)
list(TRANSFORM MSIME_EMOJI_GRID_TEST_SOURCES PREPEND "${CMAKE_CURRENT_SOURCE_DIR}/")
add_custom_command(OUTPUT "${CMAKE_CURRENT_BINARY_DIR}/emoji-grid-test"
  COMMAND "${MSIME_XCRUN}" swiftc -Onone
    -target "${CMAKE_HOST_SYSTEM_PROCESSOR}-apple-macosx${CMAKE_OSX_DEPLOYMENT_TARGET}"
    ${MSIME_EMOJI_GRID_TEST_SOURCES} -o "${CMAKE_CURRENT_BINARY_DIR}/emoji-grid-test"
  DEPENDS ${MSIME_EMOJI_GRID_TEST_SOURCES} VERBATIM)
add_custom_target(emoji-grid-test-build ALL DEPENDS "${CMAKE_CURRENT_BINARY_DIR}/emoji-grid-test")
add_test(NAME emoji-grid COMMAND "${CMAKE_CURRENT_BINARY_DIR}/emoji-grid-test")
set_tests_properties(emoji-grid PROPERTIES LABELS "emoji-local" TIMEOUT 30)
set(MSIME_EMOJI_SCROLL_TEST_SOURCES src/backend/emoji/BackendEmojiScroll.swift tests/emoji/EmojiScrollTest.swift)
list(TRANSFORM MSIME_EMOJI_SCROLL_TEST_SOURCES PREPEND "${CMAKE_CURRENT_SOURCE_DIR}/")
add_custom_command(OUTPUT "${CMAKE_CURRENT_BINARY_DIR}/emoji-scroll-test"
  COMMAND "${MSIME_XCRUN}" swiftc -Onone
    -target "${CMAKE_HOST_SYSTEM_PROCESSOR}-apple-macosx${CMAKE_OSX_DEPLOYMENT_TARGET}"
    ${MSIME_EMOJI_SCROLL_TEST_SOURCES} -o "${CMAKE_CURRENT_BINARY_DIR}/emoji-scroll-test"
  DEPENDS ${MSIME_EMOJI_SCROLL_TEST_SOURCES} VERBATIM)
add_custom_target(emoji-scroll-test-build ALL DEPENDS "${CMAKE_CURRENT_BINARY_DIR}/emoji-scroll-test")
add_test(NAME emoji-scroll COMMAND "${CMAKE_CURRENT_BINARY_DIR}/emoji-scroll-test")
set_tests_properties(emoji-scroll PROPERTIES LABELS "emoji-local" TIMEOUT 30)
set(MSIME_EMOJI_SEARCH_TEST_SOURCES src/backend/emoji/BackendEmojiAppearance.swift src/backend/emoji/BackendEmojiSearchField.swift tests/emoji/EmojiSearchChromeTest.swift)
list(TRANSFORM MSIME_EMOJI_SEARCH_TEST_SOURCES PREPEND "${CMAKE_CURRENT_SOURCE_DIR}/")
add_custom_command(OUTPUT "${CMAKE_CURRENT_BINARY_DIR}/emoji-search-test"
  COMMAND "${MSIME_XCRUN}" swiftc -Onone
    -target "${CMAKE_HOST_SYSTEM_PROCESSOR}-apple-macosx${CMAKE_OSX_DEPLOYMENT_TARGET}"
    ${MSIME_EMOJI_SEARCH_TEST_SOURCES} -o "${CMAKE_CURRENT_BINARY_DIR}/emoji-search-test"
  DEPENDS ${MSIME_EMOJI_SEARCH_TEST_SOURCES} VERBATIM)
add_custom_target(emoji-search-test-build ALL DEPENDS "${CMAKE_CURRENT_BINARY_DIR}/emoji-search-test")
add_test(NAME emoji-search COMMAND "${CMAKE_CURRENT_BINARY_DIR}/emoji-search-test")
set_tests_properties(emoji-search PROPERTIES LABELS "emoji-local" TIMEOUT 30)
set(MSIME_EMOJI_TOAST_TEST_SOURCES src/backend/emoji/BackendEmojiAppearance.swift src/backend/emoji/BackendEmojiToast.swift tests/emoji/EmojiToastTest.swift)
list(TRANSFORM MSIME_EMOJI_TOAST_TEST_SOURCES PREPEND "${CMAKE_CURRENT_SOURCE_DIR}/")
add_custom_command(OUTPUT "${CMAKE_CURRENT_BINARY_DIR}/emoji-toast-test"
  COMMAND "${MSIME_XCRUN}" swiftc -Onone
    -target "${CMAKE_HOST_SYSTEM_PROCESSOR}-apple-macosx${CMAKE_OSX_DEPLOYMENT_TARGET}"
    ${MSIME_EMOJI_TOAST_TEST_SOURCES} -o "${CMAKE_CURRENT_BINARY_DIR}/emoji-toast-test"
  DEPENDS ${MSIME_EMOJI_TOAST_TEST_SOURCES} VERBATIM)
add_custom_target(emoji-toast-test-build ALL DEPENDS "${CMAKE_CURRENT_BINARY_DIR}/emoji-toast-test")
add_test(NAME emoji-toast COMMAND "${CMAKE_CURRENT_BINARY_DIR}/emoji-toast-test")
set_tests_properties(emoji-toast PROPERTIES LABELS "emoji-local" TIMEOUT 30)
set(MSIME_EMOJI_TABS_TEST_SOURCES src/backend/emoji/BackendEmojiAppearance.swift src/backend/emoji/BackendEmojiNavigation.swift
  src/backend/emoji/BackendEmojiMainTabs.swift tests/emoji/EmojiMainTabsTest.swift)
list(TRANSFORM MSIME_EMOJI_TABS_TEST_SOURCES PREPEND "${CMAKE_CURRENT_SOURCE_DIR}/")
add_custom_command(OUTPUT "${CMAKE_CURRENT_BINARY_DIR}/emoji-main-tabs-test"
  COMMAND "${MSIME_XCRUN}" swiftc -Onone
    -target "${CMAKE_HOST_SYSTEM_PROCESSOR}-apple-macosx${CMAKE_OSX_DEPLOYMENT_TARGET}"
    ${MSIME_EMOJI_TABS_TEST_SOURCES} -o "${CMAKE_CURRENT_BINARY_DIR}/emoji-main-tabs-test"
  DEPENDS ${MSIME_EMOJI_TABS_TEST_SOURCES} VERBATIM)
add_custom_target(emoji-main-tabs-test-build ALL DEPENDS "${CMAKE_CURRENT_BINARY_DIR}/emoji-main-tabs-test")
add_test(NAME emoji-main-tabs COMMAND "${CMAKE_CURRENT_BINARY_DIR}/emoji-main-tabs-test")
set_tests_properties(emoji-main-tabs PROPERTIES LABELS "emoji-local" TIMEOUT 30)
set(MSIME_EMOJI_CATEGORY_TEST_SOURCES src/backend/emoji/BackendEmojiAppearance.swift src/backend/emoji/BackendEmojiNavigation.swift
  src/backend/emoji/BackendEmojiSymbolGroups.swift src/backend/emoji/BackendEmojiCatalog.swift src/backend/emoji/BackendEmojiCategoryTabs.swift tests/emoji/EmojiCategoryTabsTest.swift)
list(TRANSFORM MSIME_EMOJI_CATEGORY_TEST_SOURCES PREPEND "${CMAKE_CURRENT_SOURCE_DIR}/")
add_custom_command(OUTPUT "${CMAKE_CURRENT_BINARY_DIR}/emoji-category-tabs-test"
  COMMAND "${MSIME_XCRUN}" swiftc -Onone
    -target "${CMAKE_HOST_SYSTEM_PROCESSOR}-apple-macosx${CMAKE_OSX_DEPLOYMENT_TARGET}"
    ${MSIME_EMOJI_CATEGORY_TEST_SOURCES} -o "${CMAKE_CURRENT_BINARY_DIR}/emoji-category-tabs-test"
  DEPENDS ${MSIME_EMOJI_CATEGORY_TEST_SOURCES} VERBATIM)
add_custom_target(emoji-category-tabs-test-build ALL DEPENDS "${CMAKE_CURRENT_BINARY_DIR}/emoji-category-tabs-test")
add_test(NAME emoji-category-tabs COMMAND "${CMAKE_CURRENT_BINARY_DIR}/emoji-category-tabs-test")
set_tests_properties(emoji-category-tabs PROPERTIES LABELS "emoji-local" TIMEOUT 30)
set(MSIME_EMOJI_SECTIONS_TEST_SOURCES src/backend/emoji/BackendEmojiAppearance.swift src/backend/emoji/BackendEmojiSymbolGroups.swift
  src/backend/emoji/BackendEmojiCatalog.swift src/backend/emoji/BackendEmojiFlow.swift src/backend/emoji/BackendEmojiGrid.swift src/backend/emoji/BackendEmojiKeyboard.swift
  src/backend/emoji/BackendEmojiSymbolSections.swift tests/emoji/EmojiSymbolSectionsTest.swift)
list(TRANSFORM MSIME_EMOJI_SECTIONS_TEST_SOURCES PREPEND "${CMAKE_CURRENT_SOURCE_DIR}/")
add_custom_command(OUTPUT "${CMAKE_CURRENT_BINARY_DIR}/emoji-symbol-sections-test"
  COMMAND "${MSIME_XCRUN}" swiftc -Onone
    -target "${CMAKE_HOST_SYSTEM_PROCESSOR}-apple-macosx${CMAKE_OSX_DEPLOYMENT_TARGET}"
    ${MSIME_EMOJI_SECTIONS_TEST_SOURCES} -o "${CMAKE_CURRENT_BINARY_DIR}/emoji-symbol-sections-test"
  DEPENDS ${MSIME_EMOJI_SECTIONS_TEST_SOURCES} VERBATIM)
add_custom_target(emoji-symbol-sections-test-build ALL DEPENDS "${CMAKE_CURRENT_BINARY_DIR}/emoji-symbol-sections-test")
add_test(NAME emoji-symbol-sections COMMAND "${CMAKE_CURRENT_BINARY_DIR}/emoji-symbol-sections-test")
set_tests_properties(emoji-symbol-sections PROPERTIES LABELS "emoji-local" TIMEOUT 30)
set(MSIME_EMOJI_CURSOR_TEST_SOURCES src/backend/emoji/BackendEmojiSymbolGroups.swift src/backend/emoji/BackendEmojiCatalog.swift
  src/backend/emoji/BackendEmojiCatalogCursor.swift tests/emoji/EmojiCatalogCursorTest.swift)
list(TRANSFORM MSIME_EMOJI_CURSOR_TEST_SOURCES PREPEND "${CMAKE_CURRENT_SOURCE_DIR}/")
add_custom_command(OUTPUT "${CMAKE_CURRENT_BINARY_DIR}/emoji-catalog-cursor-test"
  COMMAND "${MSIME_XCRUN}" swiftc -Onone
    -target "${CMAKE_HOST_SYSTEM_PROCESSOR}-apple-macosx${CMAKE_OSX_DEPLOYMENT_TARGET}"
    ${MSIME_EMOJI_CURSOR_TEST_SOURCES} -o "${CMAKE_CURRENT_BINARY_DIR}/emoji-catalog-cursor-test"
  DEPENDS ${MSIME_EMOJI_CURSOR_TEST_SOURCES} VERBATIM)
add_custom_target(emoji-catalog-cursor-test-build ALL DEPENDS "${CMAKE_CURRENT_BINARY_DIR}/emoji-catalog-cursor-test")
add_test(NAME emoji-catalog-cursor COMMAND "${CMAKE_CURRENT_BINARY_DIR}/emoji-catalog-cursor-test")
set_tests_properties(emoji-catalog-cursor PROPERTIES LABELS "emoji-local" TIMEOUT 30)
set(MSIME_EMOJI_CATALOG_TEST_SOURCES src/backend/emoji/BackendEmojiSymbolGroups.swift src/backend/emoji/BackendEmojiCatalog.swift
  tests/emoji/EmojiCatalogTest.swift)
list(TRANSFORM MSIME_EMOJI_CATALOG_TEST_SOURCES PREPEND "${CMAKE_CURRENT_SOURCE_DIR}/")
add_custom_command(OUTPUT "${CMAKE_CURRENT_BINARY_DIR}/emoji-catalog-test"
  COMMAND "${MSIME_XCRUN}" swiftc -Onone
    -target "${CMAKE_HOST_SYSTEM_PROCESSOR}-apple-macosx${CMAKE_OSX_DEPLOYMENT_TARGET}"
    ${MSIME_EMOJI_CATALOG_TEST_SOURCES} -o "${CMAKE_CURRENT_BINARY_DIR}/emoji-catalog-test"
  DEPENDS ${MSIME_EMOJI_CATALOG_TEST_SOURCES} VERBATIM)
add_custom_target(emoji-catalog-test-build ALL DEPENDS "${CMAKE_CURRENT_BINARY_DIR}/emoji-catalog-test")
add_test(NAME emoji-catalog COMMAND "${CMAKE_CURRENT_BINARY_DIR}/emoji-catalog-test")
set_tests_properties(emoji-catalog PROPERTIES LABELS "emoji-local" TIMEOUT 30)
set(MSIME_HANDWRITING_TEST_SOURCES src/backend/handwriting/BackendHandwritingView.swift tests/core/HandwritingProviderTest.swift)
list(TRANSFORM MSIME_HANDWRITING_TEST_SOURCES PREPEND "${CMAKE_CURRENT_SOURCE_DIR}/")
add_custom_command(OUTPUT "${CMAKE_CURRENT_BINARY_DIR}/handwriting-provider-test"
  COMMAND "${MSIME_XCRUN}" swiftc -Onone
    -target "${CMAKE_HOST_SYSTEM_PROCESSOR}-apple-macosx${CMAKE_OSX_DEPLOYMENT_TARGET}"
    ${MSIME_HANDWRITING_TEST_SOURCES} -o "${CMAKE_CURRENT_BINARY_DIR}/handwriting-provider-test"
  DEPENDS ${MSIME_HANDWRITING_TEST_SOURCES} VERBATIM)
add_custom_target(handwriting-provider-test-build ALL DEPENDS "${CMAKE_CURRENT_BINARY_DIR}/handwriting-provider-test")
add_test(NAME handwriting-provider COMMAND "${CMAKE_CURRENT_BINARY_DIR}/handwriting-provider-test")
set_tests_properties(handwriting-provider PROPERTIES LABELS "handwriting-local" TIMEOUT 30)
add_executable(screen-keyboard-panel-test tests/core/ScreenKeyboardPanelTest.mm src/core/ScreenKeyboardPanel.mm)
target_compile_options(screen-keyboard-panel-test PRIVATE -fobjc-arc -Wall -Wextra -Werror -UNDEBUG)
target_link_libraries(screen-keyboard-panel-test PRIVATE "-framework AppKit" "-framework CoreGraphics" "-framework Carbon")
add_test(NAME screen-keyboard-panel COMMAND screen-keyboard-panel-test)
add_executable(preference-snapshot-merge-test tests/settings/PreferenceSnapshotMergeTest.mm)
target_compile_options(preference-snapshot-merge-test PRIVATE -fobjc-arc -Wall -Wextra -Werror -UNDEBUG)
target_link_libraries(preference-snapshot-merge-test PRIVATE "-framework Foundation")
add_test(NAME preference-snapshot-merge COMMAND preference-snapshot-merge-test)
add_executable(macos-diagnostic-log-test tests/core/DiagnosticLogTest.cpp src/core/DiagnosticLog.cpp)
target_compile_options(macos-diagnostic-log-test PRIVATE -UNDEBUG -Wall -Wextra -Werror)
target_include_directories(macos-diagnostic-log-test PRIVATE src)
add_test(NAME macos-diagnostic-log COMMAND macos-diagnostic-log-test)
add_executable(shared-voice-preferences-test tests/voice/SharedVoicePreferencesTest.mm)
target_compile_options(shared-voice-preferences-test PRIVATE -fobjc-arc -Wall -Wextra -Werror -UNDEBUG)
target_link_libraries(shared-voice-preferences-test PRIVATE "-framework Foundation")
add_test(NAME shared-voice-preferences COMMAND shared-voice-preferences-test)
add_executable(voice-provider-options-test tests/voice/VoiceProviderOptionsTest.mm)
target_compile_options(voice-provider-options-test PRIVATE -fobjc-arc -Wall -Wextra -Werror -UNDEBUG)
target_link_libraries(voice-provider-options-test PRIVATE "-framework Foundation")
add_test(NAME voice-provider-options COMMAND voice-provider-options-test)
add_executable(voice-text-commit-test tests/voice/VoiceTextCommitTest.mm)
target_compile_options(voice-text-commit-test PRIVATE -fobjc-arc -Wall -Wextra -Werror -UNDEBUG)
target_link_libraries(voice-text-commit-test PRIVATE "-framework AppKit" "-framework CoreGraphics")
add_test(NAME voice-text-commit COMMAND voice-text-commit-test)
add_executable(voice-deactivation-test tests/voice/VoiceDeactivationTest.mm)
target_compile_options(voice-deactivation-test PRIVATE -fobjc-arc -Wall -Wextra -Werror -UNDEBUG)
target_link_libraries(voice-deactivation-test PRIVATE "-framework AppKit")
add_test(NAME voice-deactivation COMMAND voice-deactivation-test)
add_executable(voice-pcm-buffer-test tests/voice/VoicePCMBufferTest.mm src/voice/VoicePCMBuffer.mm)
target_compile_options(voice-pcm-buffer-test PRIVATE -fobjc-arc -Wall -Wextra -Werror -UNDEBUG)
target_link_libraries(voice-pcm-buffer-test PRIVATE "-framework Foundation" "-framework AVFoundation")
add_test(NAME voice-pcm-buffer COMMAND voice-pcm-buffer-test)
add_executable(voice-transcription-lifecycle-test tests/voice/VoiceTranscriptionLifecycleTest.mm src/voice/VoiceInputService.mm src/voice/VoicePCMBuffer.mm)
target_compile_options(voice-transcription-lifecycle-test PRIVATE -fobjc-arc -Wall -Wextra -Werror -UNDEBUG)
target_link_libraries(voice-transcription-lifecycle-test PRIVATE "-framework Foundation" "-framework AVFoundation" "-framework Speech" "-framework CoreAudio" "-framework AudioToolbox")
add_test(NAME voice-transcription-lifecycle COMMAND voice-transcription-lifecycle-test)
add_executable(voice-capture-device-test tests/voice/VoiceCaptureDeviceTest.mm)
target_compile_options(voice-capture-device-test PRIVATE -fobjc-arc -Wall -Wextra -Werror -UNDEBUG)
target_link_libraries(voice-capture-device-test PRIVATE "-framework Foundation" "-framework CoreAudio" "-framework AudioToolbox")
add_test(NAME voice-capture-device COMMAND voice-capture-device-test)
add_executable(voice-pcm-capture-test tests/voice/VoicePCMCaptureTest.mm src/voice/VoiceInputService.mm src/voice/VoicePCMBuffer.mm)
target_compile_options(voice-pcm-capture-test PRIVATE -fobjc-arc -Wall -Wextra -Werror -UNDEBUG)
target_link_libraries(voice-pcm-capture-test PRIVATE "-framework Foundation" "-framework AVFoundation" "-framework Speech" "-framework CoreAudio" "-framework AudioToolbox")
add_test(NAME voice-pcm-capture COMMAND voice-pcm-capture-test)
add_executable(voice-pcm-stream-test tests/voice/VoicePCMStreamTest.mm src/voice/VoiceInputService.mm src/voice/VoicePCMBuffer.mm)
target_compile_options(voice-pcm-stream-test PRIVATE -fobjc-arc -Wall -Wextra -Werror -UNDEBUG)
target_link_libraries(voice-pcm-stream-test PRIVATE "-framework Foundation" "-framework AVFoundation" "-framework Speech" "-framework AudioToolbox" "-framework CoreAudio")
add_test(NAME voice-pcm-stream COMMAND voice-pcm-stream-test)
add_executable(http-voice-request-test tests/voice/HTTPVoiceRequestTest.mm src/voice/HTTPVoiceRequest.mm)
target_compile_options(http-voice-request-test PRIVATE -fobjc-arc -Wall -Wextra -Werror -UNDEBUG)
target_link_libraries(http-voice-request-test PRIVATE msime-voice-providers "-framework Foundation")
find_package(Python3 REQUIRED COMPONENTS Interpreter)
add_test(NAME http-voice-request COMMAND "${Python3_EXECUTABLE}" "${CMAKE_CURRENT_SOURCE_DIR}/tests/voice/http_voice_fixture.py" "$<TARGET_FILE:http-voice-request-test>")
add_executable(voice-failure-messages-test tests/voice/VoiceFailureMessagesTest.mm)
target_compile_options(voice-failure-messages-test PRIVATE -fobjc-arc -Wall -Wextra -Werror -UNDEBUG)
target_link_libraries(voice-failure-messages-test PRIVATE msime-voice-providers "-framework Foundation")
add_test(NAME voice-failure-messages COMMAND voice-failure-messages-test)
add_executable(doubao-voice-request-test tests/voice/DoubaoVoiceRequestTest.mm src/voice/DoubaoVoiceRequest.mm)
target_compile_options(doubao-voice-request-test PRIVATE -fobjc-arc -Wall -Wextra -Werror -UNDEBUG)
target_link_libraries(doubao-voice-request-test PRIVATE apple-client "-framework Foundation")
add_test(NAME doubao-voice-request COMMAND "${Python3_EXECUTABLE}" "${CMAKE_CURRENT_SOURCE_DIR}/tests/voice/doubao_voice_fixture.py" "$<TARGET_FILE:doubao-voice-request-test>")
# Above the fixture's own 60 s client limit plus the 3 s stack sample it takes of a hung client, so a hang reports where it is stuck instead of being killed silently.
set_tests_properties(doubao-voice-request PROPERTIES TIMEOUT 80)
add_executable(local-voice-request-test tests/voice/LocalVoiceRequestTest.mm src/voice/LocalVoiceRequest.mm)
target_compile_options(local-voice-request-test PRIVATE -fobjc-arc -Wall -Wextra -Werror -UNDEBUG)
target_link_libraries(local-voice-request-test PRIVATE apple-client "-framework Foundation")
# The protocol against a scripted stand-in for msime-voice-local, so it needs neither the runtime nor a model.
add_test(NAME local-voice-request COMMAND local-voice-request-test)
set_tests_properties(local-voice-request PROPERTIES TIMEOUT 60 ENVIRONMENT
  "MSIME_VOICE_LOCAL_HELPER=${CMAKE_CURRENT_SOURCE_DIR}/tests/voice/local_voice_helper_fixture.py;MSIME_LOCAL_VOICE_FIXTURE_LOG=${CMAKE_CURRENT_BINARY_DIR}/local-voice-request.log")
# With an installed model (resources/local-asr-models.json) and a 16 kHz mono 16-bit recording, also stream that recording through the real helper and the fetched runtime, the way the input method does. Both are downloads, so this is opt-in.
set(MSIME_LOCAL_ASR_TEST_WAV "" CACHE FILEPATH "Optional 16 kHz mono 16-bit WAV for the local-voice-model test")
if(MSIME_LOCAL_ASR_TEST_MODEL AND MSIME_LOCAL_ASR_TEST_WAV)
  add_test(NAME local-voice-model COMMAND local-voice-request-test --model "${MSIME_LOCAL_ASR_TEST_MODEL}" --wav "${MSIME_LOCAL_ASR_TEST_WAV}")
  add_dependencies(local-voice-request-test msime-voice-local macos-voice-runtime)
  set_tests_properties(local-voice-model PROPERTIES TIMEOUT 180 ENVIRONMENT
    "MSIME_VOICE_LOCAL_HELPER=$<TARGET_FILE:msime-voice-local>;MSIME_SHERPA_ONNX_LIBRARY=${MSIME_VOICE_RUNTIME_LIBRARY}")
endif()
add_executable(preference-save-state-test tests/settings/PreferenceSaveStateTest.cpp)
target_compile_options(preference-save-state-test PRIVATE -Wall -Wextra -Werror -UNDEBUG)
add_test(NAME preference-save-state COMMAND preference-save-state-test)
add_executable(preference-load-state-test tests/settings/PreferenceLoadStateTest.cpp)
target_compile_options(preference-load-state-test PRIVATE -Wall -Wextra -Werror -UNDEBUG)
add_test(NAME preference-load-state COMMAND preference-load-state-test)
add_executable(typing-statistics-test tests/core/TypingStatisticsTest.cpp)
target_compile_options(typing-statistics-test PRIVATE -Wall -Wextra -Werror -UNDEBUG)
target_link_libraries(typing-statistics-test PRIVATE apple-client)
add_test(NAME typing-statistics COMMAND typing-statistics-test)
add_executable(key-press-statistics-test tests/core/KeyPressStatisticsTest.cpp)
target_compile_options(key-press-statistics-test PRIVATE -Wall -Wextra -Werror -UNDEBUG)
target_link_libraries(key-press-statistics-test PRIVATE apple-client)
add_test(NAME key-press-statistics COMMAND key-press-statistics-test)
add_executable(runtime-options-refresh-test tests/core/RuntimeOptionsRefreshTests.mm)
target_compile_options(runtime-options-refresh-test PRIVATE -fobjc-arc -Wall -Wextra -Werror -UNDEBUG)
target_link_libraries(runtime-options-refresh-test PRIVATE apple-client "-framework Foundation")
add_test(NAME runtime-options-refresh COMMAND runtime-options-refresh-test)
add_executable(typing-statistics-notification-test
    tests/core/TypingStatisticsNotificationTest.mm
    ../../crates/host-macos/native/dictionary.mm)
target_compile_options(typing-statistics-notification-test PRIVATE -fobjc-arc -Wall -Wextra -Werror -UNDEBUG)
target_link_libraries(typing-statistics-notification-test PRIVATE "-framework Foundation")
add_test(NAME typing-statistics-notification COMMAND typing-statistics-notification-test)
add_executable(toolbar-visibility-preferences-test tests/settings/ToolbarVisibilityPreferencesTest.mm src/settings/AppearancePreferences.mm src/candidate/CandidateSkin.cpp src/candidate/CandidateSkinPreviewView.mm src/candidate/SkinSettingsView.mm src/settings/PreferencesWindowController.mm src/candidate/CandidateSkinAppearance.mm src/core/AISettingsWindow.mm)
target_compile_options(toolbar-visibility-preferences-test PRIVATE -fobjc-arc -Wall -Wextra -Werror -UNDEBUG)
target_link_libraries(toolbar-visibility-preferences-test PRIVATE "-framework AppKit")
add_test(NAME toolbar-visibility-preferences COMMAND toolbar-visibility-preferences-test)
add_executable(scheme-round-trip-preferences-test tests/settings/SchemeRoundTripPreferencesTest.mm src/settings/AppearancePreferences.mm src/candidate/CandidateSkin.cpp src/candidate/CandidateSkinPreviewView.mm src/candidate/SkinSettingsView.mm src/settings/PreferencesWindowController.mm src/candidate/CandidateSkinAppearance.mm src/core/AISettingsWindow.mm)
target_compile_options(scheme-round-trip-preferences-test PRIVATE -fobjc-arc -Wall -Wextra -Werror -UNDEBUG)
target_link_libraries(scheme-round-trip-preferences-test PRIVATE "-framework AppKit")
add_test(NAME scheme-round-trip-preferences COMMAND scheme-round-trip-preferences-test)
add_executable(local-mode-preferences-test tests/settings/LocalModePreferencesTest.mm src/settings/AppearancePreferences.mm src/candidate/CandidateSkin.cpp src/candidate/CandidateSkinPreviewView.mm src/candidate/SkinSettingsView.mm src/candidate/CandidateSkinAppearance.mm)
target_compile_options(local-mode-preferences-test PRIVATE -fobjc-arc -Wall -Wextra -Werror -UNDEBUG)
target_link_libraries(local-mode-preferences-test PRIVATE apple-client "-framework AppKit")
add_test(NAME local-mode-preferences COMMAND local-mode-preferences-test)
add_executable(preferences-lifecycle-test tests/settings/PreferencesLifecycleTest.mm src/settings/AppearancePreferences.mm src/candidate/CandidateSkin.cpp src/candidate/CandidateSkinPreviewView.mm src/candidate/SkinSettingsView.mm src/settings/PreferencesWindowController.mm src/candidate/CandidateSkinAppearance.mm src/core/AISettingsWindow.mm)
target_compile_options(preferences-lifecycle-test PRIVATE -fobjc-arc -Wall -Wextra -Werror -UNDEBUG)
target_link_libraries(preferences-lifecycle-test PRIVATE "-framework AppKit")
add_test(NAME preferences-lifecycle COMMAND preferences-lifecycle-test)
add_executable(cloud-appearance-settings-test tests/settings/CloudAppearanceSettingsTest.mm src/candidate/CandidateSkin.cpp)
target_compile_options(cloud-appearance-settings-test PRIVATE -fobjc-arc -Wall -Wextra -Werror -UNDEBUG)
target_link_libraries(cloud-appearance-settings-test PRIVATE "-framework Foundation")
add_test(NAME cloud-appearance-settings COMMAND cloud-appearance-settings-test)
add_executable(cloud-clipboard-selection-test tests/clipboard/CloudClipboardSelectionTest.mm src/cloud/CloudClipboardWindowController.mm)
target_compile_options(cloud-clipboard-selection-test PRIVATE -fobjc-arc -Wall -Wextra -Werror -UNDEBUG)
target_link_libraries(cloud-clipboard-selection-test PRIVATE "-framework AppKit")
add_test(NAME cloud-clipboard-selection COMMAND cloud-clipboard-selection-test)
add_executable(cloud-clipboard-deletion-test tests/clipboard/CloudClipboardDeletionTest.mm src/cloud/CloudClipboardClient.mm src/cloud/CloudURLSession.mm)
target_compile_options(cloud-clipboard-deletion-test PRIVATE -fobjc-arc -Wall -Wextra -Werror -UNDEBUG)
target_link_libraries(cloud-clipboard-deletion-test PRIVATE "-framework Foundation")
add_test(NAME cloud-clipboard-deletion COMMAND cloud-clipboard-deletion-test)
add_executable(backend-account-entry-test tests/core/BackendAccountEntryTest.mm)
target_compile_options(backend-account-entry-test PRIVATE -fobjc-arc -Wall -Wextra -Werror -UNDEBUG)
target_link_libraries(backend-account-entry-test PRIVATE "-framework Foundation")
add_test(NAME backend-account-entry COMMAND backend-account-entry-test)
add_executable(backend-selection-observation-test tests/core/BackendSelectionObservationTest.mm)
target_compile_options(backend-selection-observation-test PRIVATE -fobjc-arc -Wall -Wextra -Werror -UNDEBUG)
target_link_libraries(backend-selection-observation-test PRIVATE "-framework Foundation")
add_test(NAME backend-selection-observation COMMAND backend-selection-observation-test)
add_executable(tool-text-return-test tests/core/ToolTextReturnTest.mm)
target_compile_options(tool-text-return-test PRIVATE -fobjc-arc -Wall -Wextra -Werror -UNDEBUG)
target_link_libraries(tool-text-return-test PRIVATE "-framework Foundation")
add_test(NAME tool-text-return COMMAND tool-text-return-test)
add_executable(tool-application-activation-test tests/core/ToolApplicationActivationTest.mm)
target_compile_options(tool-application-activation-test PRIVATE -fobjc-arc -Wall -Wextra -Werror -UNDEBUG)
target_link_libraries(tool-application-activation-test PRIVATE "-framework AppKit")
add_test(NAME tool-application-activation COMMAND tool-application-activation-test)
add_executable(backend-library-load-test tests/core/BackendLibraryLoadTest.mm)
target_compile_options(backend-library-load-test PRIVATE -fobjc-arc -Wall -Wextra -Werror -UNDEBUG)
target_link_libraries(backend-library-load-test PRIVATE "-framework Foundation")
add_dependencies(backend-library-load-test MSIMEClientInputMethod)
add_test(NAME backend-library-load COMMAND backend-library-load-test "$<TARGET_BUNDLE_DIR:MSIMEClientInputMethod>/Contents/Frameworks/MSIMEBackend.dylib")
set(MSIME_SPARKLE_ROOT "" CACHE PATH "Directory containing the pinned Sparkle.framework (Apple reference: 2.9.6)")
if(NOT MSIME_SPARKLE_ROOT)
  message(FATAL_ERROR "UpdateController requires Sparkle 2.9.6. Set MSIME_SPARKLE_ROOT to the directory containing Sparkle.framework; see platforms/macos/README.md.")
endif()
if(MSIME_SPARKLE_ROOT)
  # Do not reuse a framework cached for a different root on reconfiguration.
  unset(MSIME_SPARKLE_FRAMEWORK CACHE)
  find_library(MSIME_SPARKLE_FRAMEWORK Sparkle PATHS "${MSIME_SPARKLE_ROOT}" NO_DEFAULT_PATH REQUIRED)
  execute_process(COMMAND /usr/libexec/PlistBuddy -c Print:CFBundleShortVersionString
    "${MSIME_SPARKLE_FRAMEWORK}/Resources/Info.plist"
    OUTPUT_VARIABLE MSIME_SPARKLE_VERSION OUTPUT_STRIP_TRAILING_WHITESPACE
    ERROR_QUIET RESULT_VARIABLE MSIME_SPARKLE_VERSION_RESULT)
  if(NOT MSIME_SPARKLE_VERSION_RESULT EQUAL 0 OR NOT MSIME_SPARKLE_VERSION STREQUAL "2.9.6")
    message(FATAL_ERROR "MSIME_SPARKLE_ROOT must contain Sparkle 2.9.6; found '${MSIME_SPARKLE_VERSION}'.")
  endif()
  target_compile_options(MSIMEClientInputMethod PRIVATE "-F${MSIME_SPARKLE_ROOT}")
  target_link_libraries(MSIMEClientInputMethod PRIVATE "${MSIME_SPARKLE_FRAMEWORK}")
  set_property(TARGET MSIMEClientInputMethod APPEND PROPERTY BUILD_RPATH "@executable_path/../Frameworks")
  # ditto, not `cmake -E copy_directory`: the latter follows symlinks, and a macOS framework is mostly
  # symlinks - Headers, Resources, the binary itself all point into Versions/Current. Flattening them
  # tripled the staged framework from 3 MB to 9 MB and, worse, left codesign calling the bundle format
  # ambiguous and refusing to seal it. An input method that cannot be signed cannot be registered as an
  # input source at all, so this decided whether the product could be installed.
  add_custom_command(TARGET MSIMEClientInputMethod POST_BUILD
    COMMAND "${CMAKE_COMMAND}" -E rm -rf
      "$<TARGET_BUNDLE_DIR:MSIMEClientInputMethod>/Contents/Frameworks/Sparkle.framework"
    COMMAND ditto "${MSIME_SPARKLE_FRAMEWORK}"
      "$<TARGET_BUNDLE_DIR:MSIMEClientInputMethod>/Contents/Frameworks/Sparkle.framework")
endif()
add_executable(candidate-skin-test tests/candidate/CandidateSkinTest.cpp src/candidate/CandidateSkin.cpp)
target_include_directories(candidate-skin-test PRIVATE src)
target_compile_options(candidate-skin-test PRIVATE -Wall -Wextra -Werror -UNDEBUG)
add_test(NAME candidate-skin COMMAND candidate-skin-test)
add_executable(wubi-code-hint-test tests/dictionary/WubiCodeHintPolicyTest.cpp)
target_compile_options(wubi-code-hint-test PRIVATE -Wall -Wextra -Werror -UNDEBUG)
add_test(NAME wubi-code-hint COMMAND wubi-code-hint-test)
add_executable(external-skin-test tests/candidate/ExternalSkinTest.cpp src/candidate/CandidateSkin.cpp)
target_include_directories(external-skin-test PRIVATE src)
target_compile_options(external-skin-test PRIVATE -Wall -Wextra -Werror -UNDEBUG)
add_test(NAME external-skin COMMAND external-skin-test)
add_executable(text-client-test tests/core/TextClientTest.mm)
target_compile_options(text-client-test PRIVATE -fobjc-arc -Wall -Wextra -Werror -UNDEBUG)
target_link_libraries(text-client-test PRIVATE apple-client "-lsqlite3")
add_test(NAME text-client COMMAND text-client-test)
add_executable(cloud-candidate-request-test tests/candidate/CloudCandidateRequestTest.mm src/cloud/CloudCandidateRequest.mm)
target_compile_options(cloud-candidate-request-test PRIVATE -fobjc-arc -Wall -Wextra -Werror -UNDEBUG)
target_link_libraries(cloud-candidate-request-test PRIVATE apple-client "-framework Foundation")
add_test(NAME cloud-candidate-request COMMAND cloud-candidate-request-test)
add_executable(custom-translation-batch-test tests/dictionary/CustomTranslationBatchTest.mm src/core/CustomTranslationBatch.mm src/cloud/CloudCandidateRequest.mm)
target_compile_options(custom-translation-batch-test PRIVATE -fobjc-arc -Wall -Wextra -Werror -UNDEBUG)
target_link_libraries(custom-translation-batch-test PRIVATE apple-client)
add_test(NAME custom-translation-batch COMMAND custom-translation-batch-test)
add_executable(niutrans-translation-test tests/dictionary/NiuTransTranslationTest.mm src/core/CustomTranslationBatch.mm src/cloud/CloudCandidateRequest.mm)
target_compile_options(niutrans-translation-test PRIVATE -fobjc-arc -Wall -Wextra -Werror -UNDEBUG)
target_link_libraries(niutrans-translation-test PRIVATE apple-client)
add_test(NAME niutrans-translation COMMAND niutrans-translation-test)
add_executable(translation-cache-test tests/dictionary/TranslationCacheTest.mm src/cloud/TranslationCache.mm)
target_compile_options(translation-cache-test PRIVATE -fobjc-arc -Wall -Wextra -Werror -UNDEBUG)
target_link_libraries(translation-cache-test PRIVATE "-framework Foundation")
add_test(NAME translation-cache COMMAND translation-cache-test)
target_sources(MSIMEClientInputMethod PRIVATE src/cloud/TranslationCache.mm)
add_executable(ai-settings-snapshot-test tests/settings/AISettingsSnapshotTest.mm)
target_compile_options(ai-settings-snapshot-test PRIVATE -fobjc-arc -Wall -Wextra -Werror -UNDEBUG)
target_link_libraries(ai-settings-snapshot-test PRIVATE "-framework Foundation")
add_test(NAME ai-settings-snapshot COMMAND ai-settings-snapshot-test)
add_executable(ai-settings-window-test tests/settings/AISettingsWindowTest.mm src/core/AISettingsWindow.mm)
target_compile_options(ai-settings-window-test PRIVATE -fobjc-arc -Wall -Wextra -Werror -UNDEBUG)
target_link_libraries(ai-settings-window-test PRIVATE apple-client "-framework AppKit")
add_test(NAME ai-settings-window COMMAND ai-settings-window-test)
add_executable(translation-settings-test tests/dictionary/TranslationSettingsTest.mm src/cloud/TranslationSettingsWindow.mm)
target_compile_options(translation-settings-test PRIVATE -fobjc-arc -Wall -Wextra -Werror -UNDEBUG)
target_link_libraries(translation-settings-test PRIVATE apple-client "-framework AppKit")
add_test(NAME translation-settings COMMAND translation-settings-test)
# Run explicitly with the directory matching desktop-dictionary.lock.json.
# This resource-dependent integration test does not enable or trigger CI.
add_executable(snapshot-activation-test tests/core/SnapshotActivationTest.mm)
target_compile_options(snapshot-activation-test PRIVATE -fobjc-arc -Wall -Wextra -Werror -UNDEBUG)
target_link_libraries(snapshot-activation-test PRIVATE apple-client)
add_executable(chinese-text-conversion-test tests/core/ChineseTextConversionTest.mm src/core/ChineseTextConversion.mm)
target_compile_options(chinese-text-conversion-test PRIVATE -fobjc-arc -Wall -Wextra -Werror -UNDEBUG)
target_link_libraries(chinese-text-conversion-test PRIVATE apple-client "-framework Foundation")
add_test(NAME chinese-text-conversion COMMAND chinese-text-conversion-test)
add_executable(session-smoke-test ../../shared/apple/tests/session_smoke.mm)
target_compile_options(session-smoke-test PRIVATE -fobjc-arc -Wall -Wextra -Werror -UNDEBUG)
target_include_directories(session-smoke-test PRIVATE ../../shared/apple ../../crates/host-api/include)
target_link_libraries(session-smoke-test PRIVATE apple-client "-framework Foundation")
add_test(NAME session-smoke COMMAND session-smoke-test)

add_executable(voice-session-test ../../shared/apple/tests/voice_session.mm)
target_compile_options(voice-session-test PRIVATE -fobjc-arc -Wall -Wextra -Werror -UNDEBUG)
target_link_libraries(voice-session-test PRIVATE apple-client)
add_test(NAME voice-session COMMAND voice-session-test)

add_executable(voice-session-delivery-test tests/voice/VoiceSessionDeliveryTest.mm src/voice/VoiceInputService.mm src/voice/VoicePCMBuffer.mm)
target_compile_options(voice-session-delivery-test PRIVATE -fobjc-arc -Wall -Wextra -Werror -UNDEBUG)
target_link_libraries(voice-session-delivery-test PRIVATE apple-client "-framework AVFoundation" "-framework Speech" "-framework CoreAudio" "-framework AudioToolbox")
add_test(NAME voice-session-delivery COMMAND voice-session-delivery-test)
add_executable(shortcut-test tests/input/ShortcutTest.mm src/core/DiagnosticLog.cpp src/core/UpdateController.mm src/settings/AppearancePreferences.mm src/candidate/CandidateSkin.cpp src/candidate/CandidateSkinPreviewView.mm src/candidate/SkinSettingsView.mm src/core/ChineseTextConversion.mm src/settings/ShuangpinKeymapPanel.mm src/input/InputSourceRegistration.mm src/core/FloatingToolbarPanel.mm src/dictionary/DictionaryWindowController.mm src/core/ClientDictionaryRuntime.mm)
target_compile_options(shortcut-test PRIVATE -fobjc-arc -Wall -Wextra -Werror -UNDEBUG)
target_link_libraries(shortcut-test PRIVATE apple-client "-framework AppKit" "-framework InputMethodKit" "-framework Carbon")
target_compile_options(shortcut-test PRIVATE "-F${MSIME_SPARKLE_ROOT}")
target_link_libraries(shortcut-test PRIVATE "${MSIME_SPARKLE_FRAMEWORK}")
target_sources(shortcut-test PRIVATE src/settings/PreferencesWindowController.mm src/core/SupportWindowController.mm src/candidate/CandidateSkinAppearance.mm src/core/ScreenKeyboardPanel.mm src/voice/VoiceInputService.mm src/voice/VoicePCMBuffer.mm src/voice/VoiceWaveOverlay.mm src/voice/VoiceCuePlayer.mm src/voice/VoiceAudioMuter.mm src/voice/VoiceSettings.mm src/voice/VoiceProviderSettings.mm src/cloud/CloudCandidateRequest.mm src/core/CustomTranslationBatch.mm)
target_sources(shortcut-test PRIVATE src/voice/HTTPVoiceRequest.mm src/voice/DoubaoVoiceRequest.mm src/voice/LocalVoiceRequest.mm src/core/DesktopInputSession.mm)
target_sources(shortcut-test PRIVATE src/core/DesktopCloudClipboard.mm)
target_link_libraries(shortcut-test PRIVATE msime-voice-providers)
target_link_libraries(shortcut-test PRIVATE "-framework AudioToolbox")
target_link_libraries(MSIMEClientInputMethod PRIVATE "-framework AudioToolbox")
target_link_libraries(shortcut-test PRIVATE "-framework AVFoundation" "-framework Speech" "-framework CoreGraphics" "-framework CoreText" "-framework CoreAudio" "-framework UniformTypeIdentifiers")
add_test(NAME shortcut COMMAND shortcut-test)
# Around 25 s locally: several cases wait on real input-source and run-loop round trips, which is past the 20 s CI default.
# The controllers under test read runtime-options.json from Application Support, so each run gets an empty home of its own: an installed or demo profile on the developer's Mac must not reach into these cases.
set(MSIME_SHORTCUT_TEST_HOME "${CMAKE_CURRENT_BINARY_DIR}/shortcut-test-home")
file(MAKE_DIRECTORY "${MSIME_SHORTCUT_TEST_HOME}")
# The ceiling is for the sanitizer leg, where the same binary takes several times as long: measured
# at about 16 s unsanitized and about 120 s under AddressSanitizer on one machine. A ceiling that
# only fits the unsanitized run turns the slow leg into a timeout that says nothing about the code,
# which is the opposite of what this number is here to do. It is still an order below the leg's own
# 300 s ctest ceiling, so a genuinely stuck run is still this test's failure rather than the job's.
set_tests_properties(shortcut PROPERTIES TIMEOUT 240 ENVIRONMENT "CFFIXED_USER_HOME=${MSIME_SHORTCUT_TEST_HOME}")
add_test(NAME shortcut-translations COMMAND shortcut-test --translations)
set_tests_properties(shortcut-translations PROPERTIES ENVIRONMENT "CFFIXED_USER_HOME=${MSIME_SHORTCUT_TEST_HOME}")
add_executable(voice-hold-shortcut-test tests/voice/VoiceHoldShortcutTest.mm)
target_compile_options(voice-hold-shortcut-test PRIVATE -fobjc-arc -Wall -Wextra -Werror -UNDEBUG)
target_link_libraries(voice-hold-shortcut-test PRIVATE "-framework AppKit")
add_test(NAME voice-hold-shortcut COMMAND voice-hold-shortcut-test)
get_target_property(MSIME_CONTROLLER_TEST_SOURCES shortcut-test SOURCES)
list(REMOVE_ITEM MSIME_CONTROLLER_TEST_SOURCES tests/input/ShortcutTest.mm)
add_executable(http-voice-controller-test tests/voice/HTTPVoiceControllerTest.mm ${MSIME_CONTROLLER_TEST_SOURCES})
target_sources(http-voice-controller-test PRIVATE src/cloud/TranslationCache.mm src/cloud/TranslationSettingsWindow.mm src/core/AISettingsWindow.mm src/input/InputModeHUDPanel.mm)
get_target_property(MSIME_CONTROLLER_TEST_LIBS shortcut-test LINK_LIBRARIES)
target_link_libraries(http-voice-controller-test PRIVATE ${MSIME_CONTROLLER_TEST_LIBS})
target_compile_options(http-voice-controller-test PRIVATE -fobjc-arc -Wall -Wextra -Werror -UNDEBUG "-F${MSIME_SPARKLE_ROOT}")
add_test(NAME http-voice-controller COMMAND http-voice-controller-test)
add_executable(doubao-voice-controller-test tests/voice/DoubaoVoiceControllerTest.mm ${MSIME_CONTROLLER_TEST_SOURCES} src/cloud/TranslationCache.mm src/cloud/TranslationSettingsWindow.mm src/core/AISettingsWindow.mm src/input/InputModeHUDPanel.mm)
target_link_libraries(doubao-voice-controller-test PRIVATE ${MSIME_CONTROLLER_TEST_LIBS})
target_compile_options(doubao-voice-controller-test PRIVATE -fobjc-arc -Wall -Wextra -Werror -UNDEBUG "-F${MSIME_SPARKLE_ROOT}")
add_test(NAME doubao-voice-controller COMMAND doubao-voice-controller-test)
add_executable(live-voice-controller-test tests/voice/LiveVoiceControllerTest.mm ${MSIME_CONTROLLER_TEST_SOURCES} src/cloud/TranslationCache.mm src/cloud/TranslationSettingsWindow.mm src/core/AISettingsWindow.mm src/input/InputModeHUDPanel.mm)
target_link_libraries(live-voice-controller-test PRIVATE ${MSIME_CONTROLLER_TEST_LIBS})
target_compile_options(live-voice-controller-test PRIVATE -fobjc-arc -Wall -Wextra -Werror -UNDEBUG "-F${MSIME_SPARKLE_ROOT}")
add_test(NAME live-voice-controller COMMAND live-voice-controller-test)
add_test(NAME live-voice-socket-route COMMAND live-voice-controller-test --socket)
set_tests_properties(live-voice-socket-route PROPERTIES ENVIRONMENT "MSIME_VOICE_PROVIDER_SOCKET=/tmp/synthetic-live.sock")
# The window's implementation is linked in so the test can ask the runtime what properties the class
# actually declares; nothing here opens it, reads a keychain item or writes a default.
# Built from the controller's own sources, like the voice controller tests, so it runs whatever happens to
# the larger shortcut suite.
add_executable(smart-punctuation-space-test tests/input/SmartPunctuationSpaceTest.mm ${MSIME_CONTROLLER_TEST_SOURCES}
  src/cloud/TranslationCache.mm src/cloud/TranslationSettingsWindow.mm src/core/AISettingsWindow.mm
  src/input/InputModeHUDPanel.mm)
target_link_libraries(smart-punctuation-space-test PRIVATE ${MSIME_CONTROLLER_TEST_LIBS})
target_compile_options(smart-punctuation-space-test PRIVATE -fobjc-arc -Wall -Wextra -Werror -UNDEBUG "-F${MSIME_SPARKLE_ROOT}")
add_test(NAME smart-punctuation-space COMMAND smart-punctuation-space-test)
set_tests_properties(smart-punctuation-space PROPERTIES TIMEOUT 30)
add_executable(smart-punctuation-shadow-test tests/input/SmartPunctuationShadowTest.mm ${MSIME_CONTROLLER_TEST_SOURCES}
  src/cloud/TranslationCache.mm src/cloud/TranslationSettingsWindow.mm src/core/AISettingsWindow.mm
  src/input/InputModeHUDPanel.mm)
target_link_libraries(smart-punctuation-shadow-test PRIVATE ${MSIME_CONTROLLER_TEST_LIBS})
target_compile_options(smart-punctuation-shadow-test PRIVATE -fobjc-arc -Wall -Wextra -Werror -UNDEBUG "-F${MSIME_SPARKLE_ROOT}")
add_test(NAME smart-punctuation-shadow COMMAND smart-punctuation-shadow-test)
set_tests_properties(smart-punctuation-shadow PROPERTIES TIMEOUT 30)
# Dictionary maintenance from the settings window: real sessions let go of the dictionary lock under a live quiesce lease and come back, mode intact, once it is gone.
add_executable(dictionary-quiesce-controller-test tests/input/DictionaryQuiesceTest.mm ${MSIME_CONTROLLER_TEST_SOURCES}
  src/cloud/TranslationCache.mm src/cloud/TranslationSettingsWindow.mm src/core/AISettingsWindow.mm
  src/input/InputModeHUDPanel.mm)
target_link_libraries(dictionary-quiesce-controller-test PRIVATE ${MSIME_CONTROLLER_TEST_LIBS})
target_compile_options(dictionary-quiesce-controller-test PRIVATE -fobjc-arc -Wall -Wextra -Werror -UNDEBUG "-F${MSIME_SPARKLE_ROOT}")
add_test(NAME dictionary-quiesce-controller COMMAND dictionary-quiesce-controller-test)
set_tests_properties(dictionary-quiesce-controller PROPERTIES TIMEOUT 30)
# The lease file format and its raise/lower, shared with the Linux hosts.
add_executable(common-dictionary-quiesce-lease-test ../common/tests/dictionary_quiesce_lease.cpp)
target_compile_features(common-dictionary-quiesce-lease-test PRIVATE cxx_std_17)
target_compile_options(common-dictionary-quiesce-lease-test PRIVATE -Wall -Wextra -Werror -UNDEBUG)
add_test(NAME common-dictionary-quiesce-lease COMMAND common-dictionary-quiesce-lease-test)
add_executable(voice-provider-settings-keys-test tests/voice/VoiceProviderSettingsKeysTest.mm
  src/voice/VoiceProviderSettings.mm)
target_compile_options(voice-provider-settings-keys-test PRIVATE -fobjc-arc -Wall -Wextra -Werror -UNDEBUG)
target_link_libraries(voice-provider-settings-keys-test PRIVATE "-framework AppKit" "-framework Security"
  "-framework CoreAudio" "-framework AudioToolbox")
add_test(NAME voice-provider-settings-keys COMMAND voice-provider-settings-keys-test)
add_executable(voice-provider-settings-validation-test tests/voice/VoiceSettingsTests.mm
  src/voice/VoiceProviderSettings.mm)
target_compile_options(voice-provider-settings-validation-test PRIVATE -fobjc-arc -Wall -Wextra -Werror -UNDEBUG)
target_link_libraries(voice-provider-settings-validation-test PRIVATE "-framework AppKit" "-framework Security"
  "-framework CoreAudio" "-framework AudioToolbox")
add_test(NAME voice-provider-settings-validation COMMAND voice-provider-settings-validation-test)
add_executable(voice-polish-request-test tests/voice/VoicePolishRequestTest.mm src/voice/HTTPVoiceRequest.mm)
target_compile_options(voice-polish-request-test PRIVATE -fobjc-arc -Wall -Wextra -Werror -UNDEBUG)
target_link_libraries(voice-polish-request-test PRIVATE msime-voice-providers "-framework Foundation")
add_test(NAME voice-polish-request COMMAND "${Python3_EXECUTABLE}" "${CMAKE_CURRENT_SOURCE_DIR}/tests/voice/voice_polish_fixture.py" "$<TARGET_FILE:voice-polish-request-test>")
# The fixture waits out two six-second stalls and gives the run 60 s; without its own limit CI's `ctest --timeout 20` killed it on every run.
set_tests_properties(voice-polish-request PROPERTIES TIMEOUT 65)
# Which prompt each slot resolves to. The request test above sends whatever it is handed; this one says what
# that should be. The header is shared/voice's and this host ships it through HTTPVoiceRequest.mm, but its
# only test ran in the Windows build - so a fallback that had drifted from the reference sat here unchecked.
add_executable(shared-polish-prompt-test ../../shared/voice/tests/polish_prompt.cpp)
target_compile_options(shared-polish-prompt-test PRIVATE -Wall -Wextra -Werror -UNDEBUG)
add_test(NAME shared-polish-prompt COMMAND shared-polish-prompt-test)
target_sources(shortcut-test PRIVATE src/cloud/TranslationCache.mm src/input/InputModeHUDPanel.mm)
add_executable(shuangpin-keymap-input-test tests/input/ShuangpinKeymapInputTest.mm src/settings/ShuangpinKeymapPanel.mm)
target_include_directories(shuangpin-keymap-input-test PRIVATE src)
target_compile_options(shuangpin-keymap-input-test PRIVATE -fobjc-arc -Wall -Wextra -Werror -UNDEBUG)
target_link_libraries(shuangpin-keymap-input-test PRIVATE apple-client "-framework AppKit")
add_test(NAME shuangpin-keymap-input COMMAND shuangpin-keymap-input-test)
add_executable(candidate-panel-test tests/candidate/CandidatePanelTests.mm src/candidate/CandidatePanel.mm src/core/DiagnosticLog.cpp src/candidate/CandidateSkinAppearance.mm src/candidate/CandidateSkin.cpp)
target_compile_options(candidate-panel-test PRIVATE -fobjc-arc -Wall -Wextra -Werror)
target_link_libraries(candidate-panel-test PRIVATE apple-client "-framework AppKit" "-framework InputMethodKit")
add_test(NAME candidate-panel COMMAND candidate-panel-test)
# The shared gloss-sense rule (shared/input/GlossSenses.h) pinned case by case, and the Windows, Linux and macOS forwarders asked the same questions, because a sense that stops being offered on one host looks like a dictionary difference rather than a bug.
add_executable(translation-senses-agreement-test tests/candidate/TranslationSensesAgreementTest.cpp)
target_compile_options(translation-senses-agreement-test PRIVATE -Wall -Wextra -Werror -UNDEBUG)
add_test(NAME translation-senses-agreement COMMAND translation-senses-agreement-test)
# The Windows candidate_item_layout rule, ported: text, 辅助码 and glosses wider than their column wrap inside it and rows take their own heights.
add_executable(candidate-item-layout-test tests/candidate/CandidateItemLayoutTest.cpp)
target_compile_options(candidate-item-layout-test PRIVATE -Wall -Wextra -Werror -UNDEBUG)
add_test(NAME candidate-item-layout COMMAND candidate-item-layout-test)
# The candidate page layout rendered through the same controller shortcut-test imports, on its own so its assertions report independently of that suite.
add_executable(candidate-row-layout-test tests/candidate/CandidateRowLayoutTest.mm ${MSIME_CONTROLLER_TEST_SOURCES} src/cloud/TranslationCache.mm src/cloud/TranslationSettingsWindow.mm src/core/AISettingsWindow.mm src/input/InputModeHUDPanel.mm)
target_link_libraries(candidate-row-layout-test PRIVATE ${MSIME_CONTROLLER_TEST_LIBS})
target_compile_options(candidate-row-layout-test PRIVATE -fobjc-arc -Wall -Wextra -Werror -UNDEBUG "-F${MSIME_SPARKLE_ROOT}")
add_test(NAME candidate-row-layout COMMAND candidate-row-layout-test)
add_executable(candidate-hover-test tests/candidate/CandidateHoverTest.mm)
target_compile_options(candidate-hover-test PRIVATE -fobjc-arc -Wall -Wextra -Werror -UNDEBUG)
target_link_libraries(candidate-hover-test PRIVATE "-framework AppKit")
add_test(NAME candidate-hover COMMAND candidate-hover-test)
# Where the candidate window goes: under the caret, flipped above it when there is no room, and
# clamped into the display it is on - including one whose coordinates are negative.
add_executable(candidate-placement-test tests/candidate/CandidatePlacementTest.mm)
target_compile_options(candidate-placement-test PRIVATE -fobjc-arc -Wall -Wextra -Werror -UNDEBUG)
target_link_libraries(candidate-placement-test PRIVATE "-framework AppKit")
add_test(NAME candidate-placement COMMAND candidate-placement-test)
add_executable(candidate-pagination-test tests/candidate/CandidatePaginationTest.cpp)
target_include_directories(candidate-pagination-test PRIVATE src)
target_compile_options(candidate-pagination-test PRIVATE -Wall -Wextra -Werror -UNDEBUG)
add_test(NAME candidate-pagination COMMAND candidate-pagination-test)
# src/candidate/CandidatePanel.mm is the panel of the Apple snapshot's direct-engine controller, which is gone; only candidate-panel-test builds it.
# Installing a dictionary over the one already in place. Every refusal here protects that file, and
# the install itself must not be able to leave the user without one.
add_executable(dictionary-installer-test tests/dictionary/DictionaryInstallerTest.mm src/dictionary/DictionaryInstaller.mm)
target_compile_options(dictionary-installer-test PRIVATE -fobjc-arc -Wall -Wextra -Werror -UNDEBUG)
target_link_libraries(dictionary-installer-test PRIVATE "-framework Foundation" ${SQLITE_TARGET})
add_test(NAME dictionary-installer COMMAND dictionary-installer-test)
add_executable(dictionary-runtime-test tests/dictionary/DictionaryRuntimeTest.mm src/core/ClientDictionaryRuntime.mm)
target_compile_options(dictionary-runtime-test PRIVATE -fobjc-arc -Wall -Wextra -Werror -UNDEBUG)
target_include_directories(dictionary-runtime-test PRIVATE ../../shared/apple ../../crates/host-api/include)
target_link_libraries(dictionary-runtime-test PRIVATE apple-client "-framework Foundation")
add_test(NAME dictionary-runtime COMMAND dictionary-runtime-test)
add_executable(skin-preview-test tests/candidate/SkinPreviewTest.mm src/settings/AppearancePreferences.mm src/candidate/CandidateSkin.cpp src/candidate/CandidateSkinPreviewView.mm src/candidate/SkinSettingsView.mm src/core/ChineseTextConversion.mm src/settings/PreferencesWindowController.mm src/candidate/CandidateSkinAppearance.mm)
target_include_directories(skin-preview-test PRIVATE src)
target_compile_options(skin-preview-test PRIVATE -fobjc-arc -Wall -Wextra -Werror -UNDEBUG)
target_link_libraries(skin-preview-test PRIVATE "-framework AppKit" "-framework CoreText")
add_test(NAME skin-preview COMMAND skin-preview-test)
add_executable(skin-settings-test tests/candidate/SkinSettingsTest.mm src/settings/AppearancePreferences.mm src/candidate/CandidateSkin.cpp src/candidate/CandidateSkinPreviewView.mm src/candidate/SkinSettingsView.mm src/candidate/CandidateSkinAppearance.mm src/core/ChineseTextConversion.mm)
foreach(translation_settings_consumer MSIMEClientInputMethod toolbar-visibility-preferences-test scheme-round-trip-preferences-test local-mode-preferences-test preferences-lifecycle-test shortcut-test skin-preview-test skin-settings-test)
  target_sources(${translation_settings_consumer} PRIVATE src/cloud/TranslationSettingsWindow.mm)
endforeach()
foreach(ai_settings_consumer toolbar-visibility-preferences-test scheme-round-trip-preferences-test local-mode-preferences-test preferences-lifecycle-test shortcut-test skin-preview-test skin-settings-test)
  target_sources(${ai_settings_consumer} PRIVATE src/core/AISettingsWindow.mm)
endforeach()
foreach(translation_settings_consumer toolbar-visibility-preferences-test scheme-round-trip-preferences-test skin-preview-test skin-settings-test)
  target_link_libraries(${translation_settings_consumer} PRIVATE apple-client)
endforeach()
target_include_directories(skin-settings-test PRIVATE src)
target_compile_options(skin-settings-test PRIVATE -fobjc-arc -Wall -Wextra -Werror -UNDEBUG)
target_link_libraries(skin-settings-test PRIVATE "-framework AppKit")
add_test(NAME skin-settings COMMAND skin-settings-test)
add_executable(floating-toolbar-panel-test tests/settings/FloatingToolbarPanelTest.mm src/core/FloatingToolbarPanel.mm src/core/SupportWindowController.mm src/candidate/CandidateSkinAppearance.mm src/candidate/CandidateSkin.cpp)
target_compile_options(floating-toolbar-panel-test PRIVATE -fobjc-arc -Wall -Wextra -Werror -UNDEBUG)
target_link_libraries(floating-toolbar-panel-test PRIVATE "-framework AppKit")
add_test(NAME floating-toolbar-panel COMMAND floating-toolbar-panel-test)
add_executable(window-presentation-test tests/core/WindowPresentationTest.mm)
target_compile_options(window-presentation-test PRIVATE -fobjc-arc -Wall -Wextra -Werror -UNDEBUG)
target_link_libraries(window-presentation-test PRIVATE "-framework AppKit")
add_test(NAME window-presentation COMMAND window-presentation-test)
# The Sparkle driver is compiled in, but the test binary is not an application bundle, so the shared
# controller must decline to start it - which is the thing worth testing.
add_executable(update-controller-test tests/core/UpdateControllerTest.mm src/core/UpdateController.mm)
target_compile_options(update-controller-test PRIVATE -fobjc-arc -Wall -Wextra -Werror -UNDEBUG "-F${MSIME_SPARKLE_ROOT}")
target_link_libraries(update-controller-test PRIVATE "-framework AppKit" "${MSIME_SPARKLE_FRAMEWORK}")
add_test(NAME update-controller COMMAND update-controller-test)
set_tests_properties(update-controller PROPERTIES TIMEOUT 20)
add_executable(support-window-test tests/core/SupportWindowTest.mm src/core/SupportWindowController.mm)
target_compile_options(support-window-test PRIVATE -fobjc-arc -Wall -Wextra -Werror -UNDEBUG)
target_link_libraries(support-window-test PRIVATE "-framework AppKit")
add_test(NAME support-window COMMAND support-window-test)
add_test(
  NAME support-window-links
  COMMAND "${Python3_EXECUTABLE}"
          "${CMAKE_CURRENT_SOURCE_DIR}/tests/settings/support_window_links.py"
          "${CMAKE_CURRENT_SOURCE_DIR}/src/core/SupportWindowController.mm")
# The template checks say what the build was told to produce; this one says what came out. A resource can
# be declared and not staged, a localisation can exist in the tree and not be copied, and the on-device
# recogniser can be compiled into a library the executable never links - each invisible until an install.
# The settings page is shared, so a preference added for another platform appears on macOS too. When the
# host does not read it the switch saves, the page says it saved, and nothing changes - which is how 本地
# Whisper and the smart-punctuation direct switches sat there doing nothing. Nothing else catches that,
# because the host compiles and the tests pass either way.
# The settings page list lives in TypeScript and the route vocabulary in Rust, so nothing connects them.
# A page added without a category is a page no host can open - which is how the account page ended up with
# macOS showing its own window instead of the one every other platform shows.
add_test(
  NAME settings-route-coverage
  COMMAND "${Python3_EXECUTABLE}"
          "${CMAKE_CURRENT_SOURCE_DIR}/tests/settings/settings_route_coverage.py"
          "${CMAKE_CURRENT_SOURCE_DIR}/../..")
add_test(
  NAME preference-coverage
  COMMAND "${Python3_EXECUTABLE}"
          "${CMAKE_CURRENT_SOURCE_DIR}/tests/settings/preference_coverage.py"
          "${CMAKE_CURRENT_SOURCE_DIR}/../..")
add_test(
  NAME bundle-contents
  COMMAND "${Python3_EXECUTABLE}"
          "${CMAKE_CURRENT_SOURCE_DIR}/tests/settings/bundle_contents.py"
          "$<TARGET_BUNDLE_DIR:MSIMEClientInputMethod>"
          --languages zh-Hans,en)
add_test(
  NAME info-plist-icons
  COMMAND "${Python3_EXECUTABLE}"
          "${CMAKE_CURRENT_SOURCE_DIR}/tests/settings/info_plist_icons.py"
          "${CMAKE_CURRENT_SOURCE_DIR}/Info.plist.in"
          "${CMAKE_CURRENT_SOURCE_DIR}/resources")
add_test(
  NAME info-plist-usage
  COMMAND "${Python3_EXECUTABLE}"
          "${CMAKE_CURRENT_SOURCE_DIR}/tests/settings/info_plist_usage.py"
          "${CMAKE_CURRENT_SOURCE_DIR}/Info.plist.in"
          "${CMAKE_CURRENT_SOURCE_DIR}/src")
# The input source identifiers and the names the input menu shows for them live in different files, so renaming the bundle leaves the names keyed on an identifier nothing declares any more - and the menu then prints the identifier itself.
add_test(
  NAME info-plist-names
  COMMAND "${Python3_EXECUTABLE}"
          "${CMAKE_CURRENT_SOURCE_DIR}/tests/settings/info_plist_names.py"
          "${CMAKE_CURRENT_SOURCE_DIR}/Info.plist.in"
          "${CMAKE_CURRENT_SOURCE_DIR}/resources"
          "${CMAKE_CURRENT_SOURCE_DIR}/../..")
add_test(
  NAME entitlements-guard
  COMMAND "${Python3_EXECUTABLE}"
          "${CMAKE_CURRENT_SOURCE_DIR}/tests/settings/entitlements_guard.py"
          "${CMAKE_CURRENT_SOURCE_DIR}/resources/VoiceInput.entitlements"
          "${CMAKE_CURRENT_SOURCE_DIR}/scripts/install.sh")
add_executable(input-source-registration-test tests/input/InputSourceRegistrationTests.mm src/input/InputSourceRegistration.mm)
target_compile_options(input-source-registration-test PRIVATE -fobjc-arc -Wall -Wextra -Werror -UNDEBUG)
target_link_libraries(input-source-registration-test PRIVATE "-framework AppKit" "-framework Carbon" "-framework Foundation")
add_test(NAME input-source-registration COMMAND input-source-registration-test)
# The page-size rules had a test file and no target: it had never been built, let alone run, which is
# how a clamp that rewrote the shared default stayed in place.
add_executable(candidate-page-size-test tests/candidate/CandidatePageSizeTest.cpp)
target_compile_options(candidate-page-size-test PRIVATE -Wall -Wextra -Werror -UNDEBUG)
add_test(NAME candidate-page-size COMMAND candidate-page-size-test)

add_executable(voice-wave-overlay-test tests/voice/VoiceWaveOverlayTest.mm src/voice/VoiceWaveOverlay.mm)
target_compile_options(voice-wave-overlay-test PRIVATE -fobjc-arc -Wall -Wextra -Werror -UNDEBUG)
target_link_libraries(voice-wave-overlay-test PRIVATE "-framework Cocoa")
add_test(NAME voice-wave-overlay COMMAND voice-wave-overlay-test)

add_executable(input-menu-test tests/input/InputMenuTests.mm)
target_compile_options(input-menu-test PRIVATE -fobjc-arc -Wall -Wextra -Werror -UNDEBUG)
target_link_libraries(input-menu-test PRIVATE "-framework Cocoa")
add_test(NAME input-menu COMMAND input-menu-test)

add_executable(voice-audio-muter-test tests/voice/VoiceAudioMuterTest.mm src/voice/VoiceAudioMuter.mm)
target_compile_options(voice-audio-muter-test PRIVATE -fobjc-arc -Wall -Wextra -Werror -UNDEBUG)
target_link_libraries(voice-audio-muter-test PRIVATE "-framework Foundation" "-framework CoreAudio")
add_test(NAME voice-audio-muter COMMAND voice-audio-muter-test)

add_executable(voice-cue-player-test tests/voice/VoiceCuePlayerTest.mm src/voice/VoiceCuePlayer.mm)
target_compile_options(voice-cue-player-test PRIVATE -fobjc-arc -Wall -Wextra -Werror -UNDEBUG)
target_compile_definitions(voice-cue-player-test PRIVATE MSIME_VOICE_CUE_SOURCE_DIR="${CMAKE_CURRENT_SOURCE_DIR}/../windows/installer/assets/audios")
target_link_libraries(voice-cue-player-test PRIVATE "-framework AppKit")
add_test(NAME voice-cue-player COMMAND voice-cue-player-test)

add_executable(voice-input-level-test tests/voice/VoiceInputLevelTest.mm)
target_compile_options(voice-input-level-test PRIVATE -fobjc-arc -Wall -Wextra -Werror -UNDEBUG)
target_link_libraries(voice-input-level-test PRIVATE "-framework Foundation" "-framework AVFoundation")
add_test(NAME voice-input-level COMMAND voice-input-level-test)

add_executable(voice-mute-recovery-test tests/voice/VoiceMuteRecoveryTest.mm src/voice/VoiceAudioMuter.mm)
target_compile_options(voice-mute-recovery-test PRIVATE -fobjc-arc -Wall -Wextra -Werror -UNDEBUG)
target_link_libraries(voice-mute-recovery-test PRIVATE "-framework Foundation" "-framework CoreAudio")
add_test(NAME voice-mute-recovery COMMAND voice-mute-recovery-test)

target_sources(MSIMEClientInputMethod PRIVATE src/voice/VoiceAudioMuter.mm)
target_link_libraries(MSIMEClientInputMethod PRIVATE "-framework CoreAudio")

add_executable(paired-punctuation-test tests/input/PairedPunctuationTest.cpp)
target_compile_features(paired-punctuation-test PRIVATE cxx_std_17)
target_compile_options(paired-punctuation-test PRIVATE -Wall -Wextra -Werror -UNDEBUG)
add_test(NAME paired-punctuation COMMAND paired-punctuation-test)

add_executable(input-scheme-traits-test tests/input/InputSchemeTraitsTest.cpp)
target_compile_features(input-scheme-traits-test PRIVATE cxx_std_17)
target_compile_options(input-scheme-traits-test PRIVATE -Wall -Wextra -Werror -UNDEBUG)
add_test(NAME input-scheme-traits COMMAND input-scheme-traits-test)

add_executable(candidate-font-cache-test tests/candidate/CandidateFontCacheTest.mm src/candidate/CandidateFontCache.mm)
target_compile_options(candidate-font-cache-test PRIVATE -fobjc-arc -Wall -Wextra -Werror -UNDEBUG)
target_link_libraries(candidate-font-cache-test PRIVATE "-framework AppKit" "-framework CoreText")
add_test(NAME candidate-font-cache COMMAND candidate-font-cache-test)

# AppearancePreferences.mm resolves candidate font families through CandidateFontCache.mm, so every target compiling the one compiles the other; a loop rather than a hand-kept list so a new consumer cannot miss it.
get_property(MSIME_FONT_CACHE_TARGETS DIRECTORY PROPERTY BUILDSYSTEM_TARGETS)
foreach(msime_target IN LISTS MSIME_FONT_CACHE_TARGETS)
  get_target_property(msime_target_sources ${msime_target} SOURCES)
  if(NOT msime_target_sources OR NOT "src/settings/AppearancePreferences.mm" IN_LIST msime_target_sources)
    continue()
  endif()
  target_sources(${msime_target} PRIVATE src/candidate/CandidateFontCache.mm)
  target_link_libraries(${msime_target} PRIVATE "-framework CoreText")
endforeach()

# AppearancePreferences.mm and InputController.mm reach the Swift account pane and the native uninstaller through weak_import: the input method binds them from MSIMEBackend.dylib and from uninstaller.mm, and a process that has neither checks for null before calling. Test targets are that process, so the linker has to be told those names may stay undefined - one name at a time, so every other missing symbol is still an error.
#
# The settings window's dictionary controller is the counter-example and the reason this loop exists rather than a hand-kept list: `feat(macos): open dictionary manager from settings` added the reference and the source file to shortcut-test only, and the other five targets compiling the same file did not link again until now.
set(MSIME_OPTIONAL_BACKEND_SYMBOLS
  _MSIMEAccountPaneView _MSIMEAccountPaneAttach _MSIMEAccountPaneClose
  _MSIMEEnsureAnonymousAccount _MSIMEFetchAccountCandidateGlosses _MSIMECancelAccountCandidateGlosses
  _MSIMEFetchOnDeviceCandidateGlosses
  _msime_macos_uninstall_input_source)
# The input method links MSIMEBackend.dylib itself, but a toolchain older than Swift 6.2 builds that without the on-device translation entry point (BackendOnDeviceGloss.swift), which the controller then finds null.
target_link_options(MSIMEClientInputMethod PRIVATE "LINKER:-U,_MSIMEFetchOnDeviceCandidateGlosses")
get_property(MSIME_DIRECTORY_TARGETS DIRECTORY PROPERTY BUILDSYSTEM_TARGETS)
foreach(msime_target IN LISTS MSIME_DIRECTORY_TARGETS)
  if(msime_target STREQUAL "MSIMEClientInputMethod")
    continue()
  endif()
  get_target_property(msime_target_sources ${msime_target} SOURCES)
  if(NOT msime_target_sources)
    continue()
  endif()
  if(NOT "src/settings/AppearancePreferences.mm" IN_LIST msime_target_sources
     AND NOT "src/input/InputController.mm" IN_LIST msime_target_sources)
    continue()
  endif()
  if(NOT "src/dictionary/DictionaryWindowController.mm" IN_LIST msime_target_sources)
    target_sources(${msime_target} PRIVATE
      src/dictionary/DictionaryWindowController.mm src/core/ClientDictionaryRuntime.mm)
    target_link_libraries(${msime_target} PRIVATE apple-client "-framework UniformTypeIdentifiers")
  endif()
  foreach(msime_optional_symbol IN LISTS MSIME_OPTIONAL_BACKEND_SYMBOLS)
    target_link_options(${msime_target} PRIVATE "LINKER:-U,${msime_optional_symbol}")
  endforeach()
endforeach()

# CandidateSkin.cpp reads external skin manifests through SkinManifestBridge.mm, which calls the shared client-core loader in the Rust host library. Every target compiling the one needs the other and apple-client, so this loop keeps them together instead of a hand-kept list that a new consumer could miss.
set_source_files_properties(src/candidate/SkinManifestBridge.mm PROPERTIES COMPILE_OPTIONS "-fobjc-arc")
get_property(MSIME_SKIN_TARGETS DIRECTORY PROPERTY BUILDSYSTEM_TARGETS)
foreach(msime_target IN LISTS MSIME_SKIN_TARGETS)
  get_target_property(msime_target_sources ${msime_target} SOURCES)
  if(NOT msime_target_sources OR NOT "src/candidate/CandidateSkin.cpp" IN_LIST msime_target_sources)
    continue()
  endif()
  if(NOT "src/candidate/SkinManifestBridge.mm" IN_LIST msime_target_sources)
    target_sources(${msime_target} PRIVATE src/candidate/SkinManifestBridge.mm)
  endif()
  target_link_libraries(${msime_target} PRIVATE apple-client)
endforeach()

# VoiceInputService.mm passes the system recogniser's text through shared/voice's tidy_local_transcript, so every target compiling it links the library that defines it. Found by source rather than listed, for the same reason as the loops above.
get_property(MSIME_VOICE_SERVICE_TARGETS DIRECTORY PROPERTY BUILDSYSTEM_TARGETS)
foreach(msime_target IN LISTS MSIME_VOICE_SERVICE_TARGETS)
  get_target_property(msime_target_sources ${msime_target} SOURCES)
  if(msime_target_sources AND "src/voice/VoiceInputService.mm" IN_LIST msime_target_sources)
    target_link_libraries(${msime_target} PRIVATE msime-voice-local-asr)
  endif()
endforeach()

# InputController.mm draws typing effects through TypingEffectPanel.mm, which animates with Core Animation. Every target compiling the controller next to its HUD panel - the input method and each controller test - takes the panel and QuartzCore, found by source like the loops above so a new controller test cannot miss it.
get_property(MSIME_TYPING_EFFECT_TARGETS DIRECTORY PROPERTY BUILDSYSTEM_TARGETS)
foreach(msime_target IN LISTS MSIME_TYPING_EFFECT_TARGETS)
  get_target_property(msime_target_sources ${msime_target} SOURCES)
  if(NOT msime_target_sources OR NOT "src/input/InputModeHUDPanel.mm" IN_LIST msime_target_sources)
    continue()
  endif()
  if(NOT "src/input/InputController.mm" IN_LIST msime_target_sources
     AND NOT "src/core/FloatingToolbarPanel.mm" IN_LIST msime_target_sources)
    continue()
  endif()
  target_sources(${msime_target} PRIVATE src/input/TypingEffectPanel.mm)
  target_link_libraries(${msime_target} PRIVATE "-framework QuartzCore")
endforeach()

# The typing effect panel on its own: the packed answer decoded, the Reduce Motion and Low Power fallback, and the idle state that leaves no emitter running and no window on screen.
add_executable(typing-effect-panel-test tests/input/TypingEffectPanelTest.mm src/input/TypingEffectPanel.mm)
target_compile_options(typing-effect-panel-test PRIVATE -fobjc-arc -Wall -Wextra -Werror -UNDEBUG)
target_link_libraries(typing-effect-panel-test PRIVATE "-framework AppKit" "-framework QuartzCore")
add_test(NAME typing-effect-panel COMMAND typing-effect-panel-test)
