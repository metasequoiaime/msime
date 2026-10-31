# 两个宿主都链接真正的共享 ABI；测试只用合成目录，不调用网络传输。
find_package(Threads REQUIRED)
add_executable(common-capture-only-test
  "${CMAKE_CURRENT_LIST_DIR}/capture_only.cpp"
  "${CMAKE_CURRENT_LIST_DIR}/../Telemetry.cpp"
  "${CMAKE_CURRENT_LIST_DIR}/../CrashRecorder.cpp")
target_compile_features(common-capture-only-test PRIVATE cxx_std_17)
target_compile_definitions(common-capture-only-test PRIVATE MSIME_TELEMETRY_TEST_CAPTURE=1)
target_include_directories(common-capture-only-test PRIVATE
  "${CMAKE_CURRENT_LIST_DIR}/.." "${CMAKE_CURRENT_LIST_DIR}/../../../crates/host-api/include")
target_link_libraries(common-capture-only-test PRIVATE
  nlohmann_json::nlohmann_json "${MSIME_HOST_LIBRARY}" Threads::Threads ${CMAKE_DL_LIBS})
if(MSVC)
  target_compile_options(common-capture-only-test PRIVATE /W4 /WX /utf-8)
else()
  target_compile_options(common-capture-only-test PRIVATE -Wall -Wextra -Werror)
endif()
if(WIN32)
  target_link_libraries(common-capture-only-test PRIVATE shell32 ole32 advapi32 uuid)
endif()
find_package(Python3 COMPONENTS Interpreter REQUIRED)
add_test(NAME common-capture-only
  COMMAND "${Python3_EXECUTABLE}" "${CMAKE_CURRENT_LIST_DIR}/capture_only.py"
    $<TARGET_FILE:common-capture-only-test>)
set_tests_properties(common-capture-only PROPERTIES TIMEOUT 120)

include("${CMAKE_CURRENT_LIST_DIR}/input_capture.cmake")
