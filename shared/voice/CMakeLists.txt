cmake_minimum_required(VERSION 3.25)
project(MSIMEVoiceProviders LANGUAGES CXX)
if(PROJECT_IS_TOP_LEVEL)
  include(CTest)
endif()
find_package(CURL REQUIRED)
find_package(nlohmann_json CONFIG REQUIRED)
# On-device recognition through the pinned sherpa-onnx runtime (resources/voice-runtime.lock.json). Built on every host: it needs nothing at build time beyond the vendored C header, and loads the runtime library with dlopen/LoadLibrary when a dictation first asks for it. See LocalAsr.h.
find_package(Threads REQUIRED)
add_library(msime-voice-local-asr STATIC LocalAsr.cpp)
target_compile_features(msime-voice-local-asr PUBLIC cxx_std_17)
target_include_directories(msime-voice-local-asr
  PUBLIC "${CMAKE_CURRENT_SOURCE_DIR}"
  PRIVATE "${CMAKE_CURRENT_SOURCE_DIR}/third_party")
target_link_libraries(msime-voice-local-asr PRIVATE nlohmann_json::nlohmann_json ${CMAKE_DL_LIBS} PUBLIC Threads::Threads)
# The recognizer as its own process, which is how the macOS and Linux input methods use it; see LocalAsrHelper.cpp.
add_executable(msime-voice-local LocalAsrHelper.cpp)
target_link_libraries(msime-voice-local PRIVATE msime-voice-local-asr nlohmann_json::nlohmann_json)
add_library(msime-voice-providers STATIC VoiceProviders.cpp)
target_compile_features(msime-voice-providers PUBLIC cxx_std_17)
target_include_directories(msime-voice-providers PUBLIC "${CMAKE_CURRENT_SOURCE_DIR}")
# 百炼请求把录音编成 Base64 数据 URL，编码用原样放在 third_party/cppcodec 的 tplgy/cppcodec v0.2（MIT，只有头文件），见 docs/third-party.md。
target_include_directories(msime-voice-providers SYSTEM PRIVATE "${CMAKE_CURRENT_SOURCE_DIR}/third_party/cppcodec")
target_link_libraries(msime-voice-providers PUBLIC msime-voice-local-asr PRIVATE CURL::libcurl nlohmann_json::nlohmann_json)
foreach(voice_target msime-voice-providers msime-voice-local-asr msime-voice-local)
  if(MSVC)
    target_compile_options(${voice_target} PRIVATE /W4 /utf-8)
  else()
    target_compile_options(${voice_target} PRIVATE -Wall -Wextra -Werror)
  endif()
endforeach()
if(BUILD_TESTING)
  find_package(Python3 REQUIRED COMPONENTS Interpreter)
  add_executable(shared-voice-transport tests/transport.cpp)
  target_link_libraries(shared-voice-transport PRIVATE msime-voice-providers)
  add_test(NAME shared-voice-http-transport
    COMMAND "${Python3_EXECUTABLE}" "${CMAKE_CURRENT_SOURCE_DIR}/tests/transport.py"
      $<TARGET_FILE:shared-voice-transport>)
  set_tests_properties(shared-voice-http-transport PROPERTIES TIMEOUT 30)
  if(MSIME_HOST_LIBRARY)
    add_executable(shared-doubao-auth tests/doubao_auth.cpp)
    target_compile_features(shared-doubao-auth PRIVATE cxx_std_17)
    target_include_directories(shared-doubao-auth PRIVATE . ../../crates/host-api/include)
    target_link_libraries(shared-doubao-auth PRIVATE "${MSIME_HOST_LIBRARY}" nlohmann_json::nlohmann_json)
    if(MSVC)
      target_compile_options(shared-doubao-auth PRIVATE /UNDEBUG)
    else()
      target_compile_options(shared-doubao-auth PRIVATE -UNDEBUG)
    endif()
    add_test(NAME shared-doubao-auth COMMAND shared-doubao-auth)
  endif()
  add_executable(shared-voice-provider-routing tests/routing.cpp)
  target_link_libraries(shared-voice-provider-routing PRIVATE msime-voice-providers)
  if(MSVC)
    target_compile_options(shared-voice-provider-routing PRIVATE /UNDEBUG)
  else()
    target_compile_options(shared-voice-provider-routing PRIVATE -UNDEBUG)
  endif()
  add_test(NAME shared-voice-provider-routing COMMAND shared-voice-provider-routing)
  add_executable(shared-voice-local-asr tests/local_asr.cpp)
  target_link_libraries(shared-voice-local-asr PRIVATE msime-voice-local-asr)
  if(MSVC)
    target_compile_options(shared-voice-local-asr PRIVATE /UNDEBUG /utf-8)
  else()
    target_compile_options(shared-voice-local-asr PRIVATE -UNDEBUG)
  endif()
  add_test(NAME shared-voice-local-asr COMMAND shared-voice-local-asr)
  add_test(NAME shared-voice-local-helper-limits
    COMMAND "${Python3_EXECUTABLE}" "${CMAKE_CURRENT_SOURCE_DIR}/tests/local_helper_limits.py"
      $<TARGET_FILE:msime-voice-local>)
  add_test(NAME shared-voice-local-helper-protocol-limits
    COMMAND "${Python3_EXECUTABLE}" "${CMAKE_CURRENT_SOURCE_DIR}/tests/local_helper_protocol_limits.py"
      $<TARGET_FILE:msime-voice-local>)
  add_test(NAME shared-voice-local-helper-malformed
    COMMAND "${Python3_EXECUTABLE}" "${CMAKE_CURRENT_SOURCE_DIR}/tests/local_helper_malformed.py"
      $<TARGET_FILE:msime-voice-local>)
  if(NOT WIN32)
    add_test(NAME shared-voice-local-helper-null-stdin
      COMMAND "${Python3_EXECUTABLE}" "${CMAKE_CURRENT_SOURCE_DIR}/tests/local_helper_null_stdin.py"
        $<TARGET_FILE:msime-voice-local>)
    add_test(NAME shared-voice-local-helper-line-buffer
      COMMAND "${Python3_EXECUTABLE}" "${CMAKE_CURRENT_SOURCE_DIR}/tests/local_helper_line_buffer.py"
        $<TARGET_FILE:msime-voice-local>)
  endif()
  add_executable(shared-voice-local-helper-queue tests/local_helper_queue.cpp)
  target_compile_features(shared-voice-local-helper-queue PRIVATE cxx_std_17)
  target_include_directories(shared-voice-local-helper-queue PRIVATE "${CMAKE_CURRENT_SOURCE_DIR}")
  if(MSVC)
    target_compile_options(shared-voice-local-helper-queue PRIVATE /W4 /WX)
  else()
    target_compile_options(shared-voice-local-helper-queue PRIVATE -Wall -Wextra -Werror)
  endif()
  add_test(NAME shared-voice-local-helper-queue COMMAND shared-voice-local-helper-queue)
  # With a fetched runtime and an installed model (scripts/fetch_voice_runtime.py, then a model from resources/local-asr-models.json) this also transcribes a real recording through the helper. Both are downloads, so the test is opt-in.
  if(MSIME_LOCAL_ASR_TEST_MODEL AND MSIME_LOCAL_ASR_TEST_RUNTIME)
    add_test(NAME shared-voice-local-helper
      COMMAND "${Python3_EXECUTABLE}" "${CMAKE_CURRENT_SOURCE_DIR}/tests/local_helper.py"
        $<TARGET_FILE:msime-voice-local> "${MSIME_LOCAL_ASR_TEST_RUNTIME}" "${MSIME_LOCAL_ASR_TEST_MODEL}")
    set_tests_properties(shared-voice-local-helper PROPERTIES TIMEOUT 120)
  endif()
endif()
