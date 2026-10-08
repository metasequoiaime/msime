cmake_minimum_required(VERSION 3.25)

project(MSIMEClientTsf LANGUAGES CXX)
include("${CMAKE_CURRENT_SOURCE_DIR}/../Edition.cmake")
enable_testing()
add_subdirectory(tests/registration_profiles)
add_subdirectory(tests/registration_categories)
add_test(NAME msime-tsf-version-resource COMMAND "${CMAKE_COMMAND}"
  "-DSOURCE_ROOT=${CMAKE_CURRENT_SOURCE_DIR}"
  -P "${CMAKE_CURRENT_SOURCE_DIR}/tests/version_resource.cmake")
add_executable(msime-tsf-key-repeat-guard-test tests/input/key_repeat_guard.cpp)
target_compile_features(msime-tsf-key-repeat-guard-test PRIVATE cxx_std_17)
target_include_directories(msime-tsf-key-repeat-guard-test PRIVATE .)
if(MSVC)
  target_compile_options(msime-tsf-key-repeat-guard-test PRIVATE /UNDEBUG /W4 /WX)
else()
  target_compile_options(msime-tsf-key-repeat-guard-test PRIVATE -UNDEBUG -Wall -Wextra -Werror)
endif()
add_test(NAME msime-tsf-key-repeat-guard COMMAND msime-tsf-key-repeat-guard-test)
add_executable(msime-tsf-smart-punctuation-fingerprint-test tests/input/smart_punctuation_fingerprint.cpp)
target_compile_features(msime-tsf-smart-punctuation-fingerprint-test PRIVATE cxx_std_17)
target_include_directories(msime-tsf-smart-punctuation-fingerprint-test PRIVATE Global)
if(MSVC)
  target_compile_options(msime-tsf-smart-punctuation-fingerprint-test PRIVATE /UNDEBUG /W4 /WX /utf-8)
else()
  target_compile_options(msime-tsf-smart-punctuation-fingerprint-test PRIVATE -UNDEBUG -Wall -Wextra -Werror)
endif()
add_test(NAME msime-tsf-smart-punctuation-fingerprint COMMAND msime-tsf-smart-punctuation-fingerprint-test)
add_executable(msime-tsf-client-key-router-test tests/input/client_key_router.cpp)
target_compile_features(msime-tsf-client-key-router-test PRIVATE cxx_std_17)
target_include_directories(msime-tsf-client-key-router-test PRIVATE . Global IME Header Key)
if(MSVC)
  target_compile_options(msime-tsf-client-key-router-test PRIVATE /UNDEBUG /W4 /WX)
else()
  target_compile_options(msime-tsf-client-key-router-test PRIVATE -UNDEBUG -Wall -Wextra -Werror)
endif()
add_test(NAME msime-tsf-client-key-router COMMAND msime-tsf-client-key-router-test)
add_executable(msime-tsf-authenticated-client-key-router-test tests/input/authenticated_client_key_router.cpp)
target_compile_features(msime-tsf-authenticated-client-key-router-test PRIVATE cxx_std_17)
target_include_directories(msime-tsf-authenticated-client-key-router-test PRIVATE .)
if(MSVC)
  target_compile_options(msime-tsf-authenticated-client-key-router-test PRIVATE /UNDEBUG /W4 /WX)
else()
  target_compile_options(msime-tsf-authenticated-client-key-router-test PRIVATE -UNDEBUG -Wall -Wextra -Werror)
endif()
add_test(NAME msime-tsf-authenticated-client-key-router COMMAND msime-tsf-authenticated-client-key-router-test)
add_executable(msime-tsf-keyboard-cancellation-test tests/input/keyboard_cancellation.cpp)
target_compile_features(msime-tsf-keyboard-cancellation-test PRIVATE cxx_std_17)
if(MSVC)
  target_compile_options(msime-tsf-keyboard-cancellation-test PRIVATE /UNDEBUG /W4 /WX)
else()
  target_compile_options(msime-tsf-keyboard-cancellation-test PRIVATE -UNDEBUG -Wall -Wextra -Werror)
endif()
add_test(NAME msime-tsf-keyboard-cancellation COMMAND msime-tsf-keyboard-cancellation-test)
foreach(path_standard IN ITEMS 17 20)
  add_executable(msime-tsf-host-path-${path_standard}-test tests/runtime/host_path_encoding.cpp)
  set_target_properties(msime-tsf-host-path-${path_standard}-test PROPERTIES
    CXX_STANDARD ${path_standard} CXX_STANDARD_REQUIRED YES CXX_EXTENSIONS NO)
  if(MSVC)
    target_compile_options(msime-tsf-host-path-${path_standard}-test PRIVATE /UNDEBUG /W4 /WX /utf-8)
  else()
    target_compile_options(msime-tsf-host-path-${path_standard}-test PRIVATE -UNDEBUG -Wall -Wextra -Werror)
  endif()
  add_test(NAME msime-tsf-host-path-${path_standard} COMMAND msime-tsf-host-path-${path_standard}-test)
endforeach()
add_test(NAME msime-tsf-host-library-config COMMAND "${CMAKE_COMMAND}"
  "-DSOURCE_ROOT=${CMAKE_CURRENT_SOURCE_DIR}"
  "-DTEST_ROOT=${CMAKE_CURRENT_BINARY_DIR}/host-library-config-test"
  -P "${CMAKE_CURRENT_SOURCE_DIR}/tests/host_library_config.cmake")
add_executable(msime-tsf-character-result-test tests/input/character_result.cpp)
target_compile_features(msime-tsf-character-result-test PRIVATE cxx_std_17)
target_include_directories(msime-tsf-character-result-test PRIVATE ../../../crates/host-api/include)
add_test(NAME msime-tsf-character-result COMMAND msime-tsf-character-result-test)
add_executable(msime-tsf-host-focus-test tests/runtime/host_focus.cpp)
target_compile_features(msime-tsf-host-focus-test PRIVATE cxx_std_17)
add_test(NAME msime-tsf-host-focus COMMAND msime-tsf-host-focus-test)
add_executable(msime-tsf-prepared-options-test tests/runtime/prepared_options.cpp PreparedHostOptions.cpp)
target_compile_features(msime-tsf-prepared-options-test PRIVATE cxx_std_17)
add_test(NAME msime-tsf-prepared-options COMMAND msime-tsf-prepared-options-test)
add_executable(msime-tsf-module-path-test tests/runtime/module_path.cpp)
target_compile_features(msime-tsf-module-path-test PRIVATE cxx_std_17)
add_test(NAME msime-tsf-module-path COMMAND msime-tsf-module-path-test)
add_executable(msime-tsf-candidate-ownership-test tests/candidate/candidate_ownership.cpp)
target_compile_features(msime-tsf-candidate-ownership-test PRIVATE cxx_std_17)
add_test(NAME msime-tsf-candidate-ownership COMMAND msime-tsf-candidate-ownership-test)
add_executable(msime-tsf-preedit-caret-test tests/input/preedit_caret.cpp)
target_compile_features(msime-tsf-preedit-caret-test PRIVATE cxx_std_17)
add_test(NAME msime-tsf-preedit-caret COMMAND msime-tsf-preedit-caret-test)
find_package(nlohmann_json 3.11 REQUIRED)
add_executable(msime-tsf-raw-commit-test tests/input/raw_commit.cpp EngineResponse.cpp)
target_compile_features(msime-tsf-raw-commit-test PRIVATE cxx_std_17)
target_include_directories(msime-tsf-raw-commit-test PRIVATE ../../../crates/host-api/include)
target_link_libraries(msime-tsf-raw-commit-test PRIVATE nlohmann_json::nlohmann_json)
add_test(NAME msime-tsf-raw-commit COMMAND msime-tsf-raw-commit-test)
add_executable(msime-tsf-host-composition-test tests/input/host_composition.cpp EngineResponse.cpp)
target_compile_features(msime-tsf-host-composition-test PRIVATE cxx_std_17)
target_include_directories(msime-tsf-host-composition-test PRIVATE ../../../crates/host-api/include)
target_link_libraries(msime-tsf-host-composition-test PRIVATE nlohmann_json::nlohmann_json)
add_test(NAME msime-tsf-host-composition COMMAND msime-tsf-host-composition-test)
add_executable(msime-tsf-engine-response-test tests/input/engine_response.cpp EngineResponse.cpp)
target_compile_features(msime-tsf-engine-response-test PRIVATE cxx_std_17)
target_include_directories(msime-tsf-engine-response-test PRIVATE ../../../crates/host-api/include)
target_link_libraries(msime-tsf-engine-response-test PRIVATE nlohmann_json::nlohmann_json)
add_test(NAME msime-tsf-engine-response COMMAND msime-tsf-engine-response-test)
add_executable(msime-tsf-punctuation-key-policy-test tests/input/punctuation_key_policy.cpp)
target_compile_features(msime-tsf-punctuation-key-policy-test PRIVATE cxx_std_17)
add_test(NAME msime-tsf-punctuation-key-policy COMMAND msime-tsf-punctuation-key-policy-test)
add_executable(msime-tsf-korean-key-test tests/input/korean_key.cpp)
target_compile_features(msime-tsf-korean-key-test PRIVATE cxx_std_17)
target_include_directories(msime-tsf-korean-key-test PRIVATE ../../../crates/host-api/include)
add_test(NAME msime-tsf-korean-key COMMAND msime-tsf-korean-key-test)
add_executable(msime-tsf-host-composed-key-test tests/input/host_composed_key.cpp)
target_compile_features(msime-tsf-host-composed-key-test PRIVATE cxx_std_17)
target_include_directories(msime-tsf-host-composed-key-test PRIVATE ../../../crates/host-api/include)
add_test(NAME msime-tsf-host-composed-key COMMAND msime-tsf-host-composed-key-test)
# How the TIP routes the keys of the V, "/" and "@" modes, which must match the Server's EditPolicy.h.
add_executable(msime-tsf-local-mode-key-policy-test tests/input/local_mode_key_policy.cpp)
target_compile_features(msime-tsf-local-mode-key-policy-test PRIVATE cxx_std_17)
if(MSVC)
  target_compile_options(msime-tsf-local-mode-key-policy-test PRIVATE /W4 /WX)
else()
  target_compile_options(msime-tsf-local-mode-key-policy-test PRIVATE -Wall -Wextra -Werror)
endif()
add_test(NAME msime-tsf-local-mode-key-policy COMMAND msime-tsf-local-mode-key-policy-test)
# AltGr arrives as Ctrl+Alt: a chord that produces a character is typing, not an application shortcut.
add_executable(msime-tsf-altgr-key-policy-test tests/input/altgr_key_policy.cpp)
target_compile_features(msime-tsf-altgr-key-policy-test PRIVATE cxx_std_17)
if(MSVC)
  target_compile_options(msime-tsf-altgr-key-policy-test PRIVATE /W4 /WX)
else()
  target_compile_options(msime-tsf-altgr-key-policy-test PRIVATE -Wall -Wextra -Werror)
endif()
add_test(NAME msime-tsf-altgr-key-policy COMMAND msime-tsf-altgr-key-policy-test)
add_executable(msime-tsf-paired-punctuation-wiring-test tests/input/paired_punctuation_wiring.cpp)
target_compile_features(msime-tsf-paired-punctuation-wiring-test PRIVATE cxx_std_17)
add_test(NAME msime-tsf-paired-punctuation-wiring COMMAND msime-tsf-paired-punctuation-wiring-test ${CMAKE_CURRENT_SOURCE_DIR})
add_executable(msime-tsf-smart-punctuation-focus-wiring-test tests/input/smart_punctuation_focus_wiring.cpp)
target_compile_features(msime-tsf-smart-punctuation-focus-wiring-test PRIVATE cxx_std_17)
add_test(NAME msime-tsf-smart-punctuation-focus-wiring COMMAND msime-tsf-smart-punctuation-focus-wiring-test ${CMAKE_CURRENT_SOURCE_DIR})

# The TSF tip is an in-process DLL.  It is intentionally a separate target
# from the out-of-process Windows Server.
if(NOT WIN32)
  return()
endif()
add_executable(msime-tsf-class-factory-test tests/runtime/class_factory.cpp)
target_compile_features(msime-tsf-class-factory-test PRIVATE cxx_std_17)
target_link_libraries(msime-tsf-class-factory-test PRIVATE ole32)
if(MSVC)
  target_compile_options(msime-tsf-class-factory-test PRIVATE /UNDEBUG /W4 /WX)
else()
  target_compile_options(msime-tsf-class-factory-test PRIVATE -UNDEBUG -Wall -Wextra -Werror)
endif()
add_test(NAME msime-tsf-class-factory COMMAND msime-tsf-class-factory-test)
add_executable(msime-tsf-paired-punctuation-policy-test tests/input/paired_punctuation_policy.cpp)
target_compile_features(msime-tsf-paired-punctuation-policy-test PRIVATE cxx_std_17)
add_test(NAME msime-tsf-paired-punctuation-policy COMMAND msime-tsf-paired-punctuation-policy-test)
# 游戏候选窗判定：用户列表、内置表和模块规则的先后顺序。比较用 CompareStringOrdinal，所以放在 WIN32 段里。
add_executable(msime-tsf-candidate-overlay-policy-test tests/input/candidate_overlay_policy.cpp)
target_compile_features(msime-tsf-candidate-overlay-policy-test PRIVATE cxx_std_17)
if(MSVC)
  target_compile_options(msime-tsf-candidate-overlay-policy-test PRIVATE /W4 /WX /utf-8)
else()
  target_compile_options(msime-tsf-candidate-overlay-policy-test PRIVATE -Wall -Wextra -Werror)
endif()
add_test(NAME msime-tsf-candidate-overlay-policy COMMAND msime-tsf-candidate-overlay-policy-test)
set(MSIME_HOST_LIBRARY "" CACHE FILEPATH "Cargo-built host library or import library matching the TSF architecture")
include("${CMAKE_CURRENT_SOURCE_DIR}/RequireHostLibrary.cmake")
enable_language(RC)

add_library(msime-tsf SHARED
  Candidate/EnumTfCandidates.cpp Candidate/SearchCandidateProvider.cpp
  Candidate/TipCandidateList.cpp Candidate/TipCandidateString.cpp
  Compartment/Compartment.cpp
  Composition/Composition.cpp Composition/CompositionProcessorEngine.cpp
  Composition/EndComposition.cpp Composition/StartComposition.cpp
  DisplayAttribute/DisplayAttribute.cpp DisplayAttribute/DisplayAttributeInfo.cpp
  DisplayAttribute/DisplayAttributeProvider.cpp DisplayAttribute/EnumDisplayAttributeInfo.cpp
  Edit/EditSession.cpp Edit/GetTextExtentEditSession.cpp Edit/TextEditSink.cpp
  Global/Globals.cpp
  IME/ActiveLanguageProfileNotifySink.cpp IME/FunctionProviderSink.cpp
  IME/MetasequoiaIME.cpp IME/MetasequoiaIMEBaseStructure.cpp
  IPC/Ipc.cpp IPC/PassthroughStatisticsQueue.cpp IPC/KeyPressStatisticsQueue.cpp
  Key/KeyEventSink.cpp Key/KeyHandler.cpp Key/KeyHandlerEditSession.cpp
  Key/KeyStateCategory.cpp
  LanguageBar/LanguageBar.cpp
  Register/Register.cpp Register/RegKey.cpp
  Tf/TfInputProcessorProfile.cpp Tf/TfTextLayoutSink.cpp
  Thread/ThreadFocusSink.cpp Thread/ThreadMgrEventSink.cpp Thread/ThreadState.cpp
  UI/CandidateSessionState.cpp UI/CandidateListUIPresenter.cpp
  Utils/CommonUtils.cpp Utils/FanyUtils.cpp
  DllMain.cpp Server.cpp
  EngineSessionAdapter.cpp EngineSessionAdapter.h EngineResponse.cpp
  HostOptionsPaths.cpp HostOptionsPaths.h PreparedHostOptions.cpp
  IME/MetasequoiaIME.rc
  IME/MetasequoiaIME.def
)
add_dependencies(msime-tsf-class-factory-test msime-tsf)

find_package(fmt CONFIG REQUIRED)
find_path(UTF8CPP_INCLUDE_DIR NAMES utf8cpp/utf8.h REQUIRED)

target_include_directories(msime-tsf PRIVATE
  . Candidate Compartment Composition DisplayAttribute Edit FanyLog Global
  Header IME IPC Key LanguageBar Register Tf Thread UI Utils
  ../../../shared/contracts
  ${UTF8CPP_INCLUDE_DIR}
  ../../../crates/host-api/include
)
target_compile_features(msime-tsf PRIVATE cxx_std_17)
target_compile_definitions(msime-tsf PRIVATE UNICODE _UNICODE)
target_link_libraries(msime-tsf PRIVATE nlohmann_json::nlohmann_json fmt::fmt-header-only ole32 oleaut32 user32 advapi32 shell32 uuid)
# TSF objects are obtained through COM; GUIDs come from uuid. No direct TF_*
# exports are used, and MinGW distributions need not ship an msctf import lib.
if(MINGW)
  target_sources(msime-tsf PRIVATE Global/TsfPropertyGuids.cpp)
endif()
target_link_libraries(msime-tsf PRIVATE "${MSIME_HOST_LIBRARY}")
set_target_properties(msime-tsf PROPERTIES OUTPUT_NAME MetasequoiaImeTsf)
install(TARGETS msime-tsf
  RUNTIME DESTINATION bin
  LIBRARY DESTINATION lib
  ARCHIVE DESTINATION lib
)
if(MSVC)
  target_compile_options(msime-tsf PRIVATE /W4 /Zc:__cplusplus /Zc:threadSafeInit-)
  set_property(TARGET msime-tsf PROPERTY MSVC_RUNTIME_LIBRARY "MultiThreaded$<$<CONFIG:Debug>:Debug>")
endif()

# Windows on Arm loads the 64-bit TIP into native ARM64 processes and into emulated x64 ones, which share one registry view and so one InprocServer32 path. The DLL there is Arm64X: native ARM64 code for the first, ARM64EC code for the second (https://learn.microsoft.com/windows/arm/arm64x-build). It is linked in two passes over two build trees: the ARM64 pass (-A ARM64) records its linker inputs in a response file, and the ARM64EC pass (-A ARM64EC) links them beside its own with /MACHINE:ARM64X. Each pass links its own MSIME_HOST_LIBRARY, so the two halves import different host DLLs: Build-Client.ps1 gives the ARM64 pass an import library naming the ARM64 host and the ARM64EC pass the x64 one, which the emulated process runs.
set(MSIME_TSF_ARM64X "" CACHE STRING "Arm64X pass of the TSF DLL: empty, ARM64 or ARM64EC")
set(MSIME_TSF_ARM64X_RESPONSE "" CACHE FILEPATH "Linker input list the ARM64 pass writes and the ARM64EC pass reads")
if(MSIME_TSF_ARM64X)
  if(NOT MSVC OR NOT IS_ABSOLUTE "${MSIME_TSF_ARM64X_RESPONSE}")
    message(FATAL_ERROR "MSIME_TSF_ARM64X needs MSVC and an absolute MSIME_TSF_ARM64X_RESPONSE")
  endif()
  if(MSIME_TSF_ARM64X STREQUAL "ARM64")
    get_filename_component(msime_arm64x_response_dir "${MSIME_TSF_ARM64X_RESPONSE}" DIRECTORY)
    file(MAKE_DIRECTORY "${msime_arm64x_response_dir}")
    target_link_options(msime-tsf PRIVATE "/LINKREPROFULLPATHRSP:${MSIME_TSF_ARM64X_RESPONSE}")
  elseif(MSIME_TSF_ARM64X STREQUAL "ARM64EC")
    if(NOT EXISTS "${MSIME_TSF_ARM64X_RESPONSE}")
      message(FATAL_ERROR "Build the ARM64 pass first: ${MSIME_TSF_ARM64X_RESPONSE} does not exist")
    endif()
    # One quoted absolute path per line. The ARM64 pass's resource file is not taken: this pass compiles the same one.
    file(STRINGS "${MSIME_TSF_ARM64X_RESPONSE}" msime_arm64_objects REGEX "obj\"$")
    file(STRINGS "${MSIME_TSF_ARM64X_RESPONSE}" msime_arm64_libraries REGEX "lib\"$")
    file(STRINGS "${MSIME_TSF_ARM64X_RESPONSE}" msime_arm64_definition REGEX "def\"$")
    if(NOT msime_arm64_objects OR NOT msime_arm64_definition)
      message(FATAL_ERROR "${MSIME_TSF_ARM64X_RESPONSE} lists no ARM64 objects or module definition")
    endif()
    string(REPLACE "\"" "" msime_arm64_objects "${msime_arm64_objects}")
    string(REPLACE "\"" "" msime_arm64_libraries "${msime_arm64_libraries}")
    string(REPLACE "\"" "" msime_arm64_definition "${msime_arm64_definition}")
    string(REGEX REPLACE "^/[Dd][Ee][Ff]:" "/DEFARM64NATIVE:" msime_arm64_definition "${msime_arm64_definition}")
    target_sources(msime-tsf PRIVATE ${msime_arm64_objects})
    # CMake puts /machine:ARM64EC into the default linker flags for this platform, after the target's own options, where it would win and produce an ARM64EC image (x64 machine type, no ARM64 view) instead.
    string(REGEX REPLACE "/[Mm][Aa][Cc][Hh][Ii][Nn][Ee]:[Aa][Rr][Mm]64[Ee][Cc]" "" CMAKE_SHARED_LINKER_FLAGS "${CMAKE_SHARED_LINKER_FLAGS}")
    target_link_options(msime-tsf PRIVATE /MACHINE:ARM64X "${msime_arm64_definition}" ${msime_arm64_libraries})
    set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS "${MSIME_TSF_ARM64X_RESPONSE}")
  else()
    message(FATAL_ERROR "MSIME_TSF_ARM64X must be empty, ARM64 or ARM64EC, not ${MSIME_TSF_ARM64X}")
  endif()
endif()
add_executable(msime-tsf-commit-and-continue-payload-test
  tests/commit_candidate_and_continue_payload.cpp)
target_include_directories(msime-tsf-commit-and-continue-payload-test PRIVATE .)
target_compile_features(msime-tsf-commit-and-continue-payload-test PRIVATE cxx_std_17)
add_test(NAME msime-tsf-commit-and-continue-payload
  COMMAND msime-tsf-commit-and-continue-payload-test)
add_executable(msime-tsf-passthrough-statistics-test
  tests/passthrough_statistics.cpp)
target_include_directories(msime-tsf-passthrough-statistics-test PRIVATE IPC)
target_compile_features(msime-tsf-passthrough-statistics-test PRIVATE cxx_std_17)
add_test(NAME msime-tsf-passthrough-statistics
  COMMAND msime-tsf-passthrough-statistics-test)
add_executable(msime-tsf-key-press-statistics-test
  tests/key_press_statistics.cpp)
target_include_directories(msime-tsf-key-press-statistics-test PRIVATE IPC)
target_compile_features(msime-tsf-key-press-statistics-test PRIVATE cxx_std_17)
add_test(NAME msime-tsf-key-press-statistics
  COMMAND msime-tsf-key-press-statistics-test)
