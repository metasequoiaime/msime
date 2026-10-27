cmake_minimum_required(VERSION 3.25)
project(TsfExportContract LANGUAGES CXX)
if(NOT WIN32)
  message(FATAL_ERROR "Use a Windows toolchain for the PE export contract test")
endif()
enable_language(RC)
enable_testing()
# Exercise the exact production definition file without pretending to build
# the complete TIP or providing stub definitions for its native dependencies.
add_library(tsf-export-fixture SHARED exports.cpp ../../IME/MetasequoiaIME.def ../../IME/MetasequoiaIME.rc)
# The version resource takes its numbers from the header the product version module generates.
include("${CMAKE_CURRENT_SOURCE_DIR}/../../../ProductVersion.cmake")
target_include_directories(tsf-export-fixture PRIVATE ../.. ../../Header "${MSIME_TSF_VERSION_INCLUDE_DIR}")
if(MINGW)
  target_link_options(tsf-export-fixture PRIVATE -Wl,--enable-stdcall-fixup)
endif()
if(MSVC)
  find_program(EXPORT_INSPECTOR dumpbin REQUIRED)
  set(EXPORT_MODE dumpbin)
else()
  set(EXPORT_INSPECTOR "${CMAKE_OBJDUMP}")
  set(EXPORT_MODE objdump)
endif()
add_test(NAME tsf-export-contract COMMAND "${CMAKE_COMMAND}"
  "-DDLL=$<TARGET_FILE:tsf-export-fixture>" "-DINSPECTOR=${EXPORT_INSPECTOR}"
  "-DMODE=${EXPORT_MODE}" -P "${CMAKE_CURRENT_SOURCE_DIR}/verify.cmake")
add_library(tsf-missing-exports SHARED missing_exports.cpp)
add_test(NAME tsf-reject-missing-exports COMMAND "${CMAKE_COMMAND}"
  "-DDLL=$<TARGET_FILE:tsf-missing-exports>" "-DINSPECTOR=${EXPORT_INSPECTOR}"
  "-DMODE=${EXPORT_MODE}" -P "${CMAKE_CURRENT_SOURCE_DIR}/verify.cmake")
set_tests_properties(tsf-reject-missing-exports PROPERTIES WILL_FAIL TRUE)
