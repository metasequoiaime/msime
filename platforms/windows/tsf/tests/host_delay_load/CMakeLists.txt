cmake_minimum_required(VERSION 3.25)
project(MSIMEHostDelayLoadTests LANGUAGES CXX)
if(NOT MSVC)
  message(FATAL_ERROR "宿主延迟导入专项需要 MSVC 与 Windows SEH")
endif()
enable_testing()
set(tsf "${CMAKE_CURRENT_SOURCE_DIR}/../..")
foreach(kind IN ITEMS complete incomplete)
  add_library(host-delay-${kind} SHARED fixture.cpp)
  target_compile_definitions(host-delay-${kind} PRIVATE $<$<STREQUAL:${kind},incomplete>:MSIME_TEST_MISSING_EXPORT>)
  set_target_properties(host-delay-${kind} PROPERTIES
    OUTPUT_NAME msime_test_delay_host
    RUNTIME_OUTPUT_DIRECTORY "${CMAKE_CURRENT_BINARY_DIR}/${kind}"
    ARCHIVE_OUTPUT_DIRECTORY "${CMAKE_CURRENT_BINARY_DIR}/${kind}")
endforeach()
add_library(host-delay-probe SHARED probe.cpp probe.def
  "${tsf}/TsfHostDelayLoad.cpp" "${tsf}/TsfHostImports.cpp")
target_compile_definitions(host-delay-probe PRIVATE
  MSIME_EDITION_FULL
  MSIME_TSF_HOST_DLL_NAME=L"msime_test_delay_host.dll")
target_include_directories(host-delay-probe PRIVATE
  "${tsf}" "${tsf}/../../../shared/contracts")
target_link_libraries(host-delay-probe PRIVATE host-delay-complete delayimp)
target_link_options(host-delay-probe PRIVATE /DELAYLOAD:msime_test_delay_host.dll)
add_executable(host-delay-test driver.cpp)
foreach(target IN ITEMS host-delay-complete host-delay-incomplete host-delay-probe host-delay-test)
  target_compile_features(${target} PRIVATE cxx_std_17)
  target_compile_options(${target} PRIVATE /W4 /WX /utf-8)
endforeach()
foreach(scenario IN ITEMS complete missing missing-export retry decoy)
  add_test(NAME tsf-host-delay-${scenario} COMMAND host-delay-test ${scenario}
    $<TARGET_FILE:host-delay-probe> $<TARGET_FILE:host-delay-complete>
    $<TARGET_FILE:host-delay-incomplete>)
  set_tests_properties(tsf-host-delay-${scenario} PROPERTIES TIMEOUT 15)
endforeach()
