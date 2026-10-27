# 本次构建所属产品线的版本号，写进 TSF DLL 的版本资源：安装包构建脚本（Build-Client.ps1、Build-Windows7Client.ps1）显式传 MSIME_WINDOWS_VERSION；没传时现代 Windows 读 version.txt，Win7（MSIME_WINDOWS7_CLIENT）读 version-win7.txt。来源与 Server 上报的版本相同（CMakeLists.txt 里 msime-client-server 的 MSIME_WINDOWS_VERSION），文件属性里看到的版本因此与关于页、更新检查一致。
# 生成的 msime_tsf_version.h 放在 MSIME_TSF_VERSION_INCLUDE_DIR，TSF 和只编资源的导出测试工程都把它加进头文件路径。
include_guard(DIRECTORY)
set(msime_product_version_file "${CMAKE_CURRENT_LIST_DIR}/version.txt")
if(MSIME_WINDOWS7_CLIENT)
  set(msime_product_version_file "${CMAKE_CURRENT_LIST_DIR}/version-win7.txt")
endif()
set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS "${msime_product_version_file}")
if(MSIME_WINDOWS_VERSION)
  set(msime_product_version "${MSIME_WINDOWS_VERSION}")
else()
  file(STRINGS "${msime_product_version_file}" msime_product_version LIMIT_COUNT 1)
  string(STRIP "${msime_product_version}" msime_product_version)
endif()
# 预发布后缀（-beta、-alpha）不进数字版本资源，Windows 的版本资源只有四段数字。
if(NOT msime_product_version MATCHES "^([0-9]+)\\.([0-9]+)\\.([0-9]+)([-+].*)?$")
  message(FATAL_ERROR "Windows product version '${msime_product_version}' is not MAJOR.MINOR.PATCH")
endif()
set(MSIME_TSF_VERSION_MAJOR "${CMAKE_MATCH_1}")
set(MSIME_TSF_VERSION_MINOR "${CMAKE_MATCH_2}")
set(MSIME_TSF_VERSION_PATCH "${CMAKE_MATCH_3}")
set(MSIME_TSF_VERSION_INCLUDE_DIR "${CMAKE_CURRENT_BINARY_DIR}/generated")
configure_file("${CMAKE_CURRENT_LIST_DIR}/tsf/IME/msime_tsf_version.h.in"
  "${MSIME_TSF_VERSION_INCLUDE_DIR}/msime_tsf_version.h" @ONLY)
