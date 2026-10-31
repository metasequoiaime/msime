# 三个桌面平台使用真实 Host API；仅测试目标替代发送函数，故障现场和本地事务均保持原样。
find_package(Threads REQUIRED)
find_package(Python3 COMPONENTS Interpreter REQUIRED)
add_executable(common-input-capture-test
  "${CMAKE_CURRENT_LIST_DIR}/input_capture.cpp"
  "${CMAKE_CURRENT_LIST_DIR}/../Telemetry.cpp"
  "${CMAKE_CURRENT_LIST_DIR}/../CrashRecorder.cpp")
target_compile_features(common-input-capture-test PRIVATE cxx_std_17)
target_compile_definitions(common-input-capture-test PRIVATE MSIME_TELEMETRY_TEST_FLUSH MSIME_TELEMETRY_TEST_BEGIN)
target_include_directories(common-input-capture-test PRIVATE
  "${CMAKE_CURRENT_LIST_DIR}/.." "${CMAKE_CURRENT_LIST_DIR}/../../../crates/host-api/include")
target_link_libraries(common-input-capture-test PRIVATE
  nlohmann_json::nlohmann_json "${MSIME_HOST_LIBRARY}" Threads::Threads ${CMAKE_DL_LIBS})
if(MSVC)
  target_compile_options(common-input-capture-test PRIVATE /W4 /WX /utf-8)
else()
  target_compile_options(common-input-capture-test PRIVATE -Wall -Wextra -Werror)
endif()
if(WIN32)
  target_link_libraries(common-input-capture-test PRIVATE shell32 ole32 advapi32 uuid)
endif()
add_test(NAME common-input-capture
  COMMAND "${Python3_EXECUTABLE}" "${CMAKE_CURRENT_LIST_DIR}/input_capture.py" $<TARGET_FILE:common-input-capture-test>)
set_tests_properties(common-input-capture PROPERTIES TIMEOUT 120)
