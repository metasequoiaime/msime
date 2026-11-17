#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include "TsfHostImports.h"
#include "msime_edition.h"

#ifdef _MSC_VER
#include <delayimp.h>

#ifndef MSIME_TSF_HOST_DLL_NAME
#define MSIME_TSF_HOST_DLL_NAME MSIME_EDITION_HOST_DLL
#endif

namespace {
HRESULT load_imports(const char *name) noexcept {
  // 仅处理 delayimp 的两种加载失败；访问越界等程序错误仍交给原异常链。
  __try {
    return __HrLoadAllImportsForDll(name);
  } __except (
      GetExceptionCode() == VcppException(ERROR_SEVERITY_ERROR, ERROR_MOD_NOT_FOUND) ||
              GetExceptionCode() == VcppException(ERROR_SEVERITY_ERROR, ERROR_PROC_NOT_FOUND)
          ? EXCEPTION_EXECUTE_HANDLER : EXCEPTION_CONTINUE_SEARCH) {
    return HRESULT_FROM_WIN32(GetExceptionCode() & 0xffff);
  }
}
}
#endif

namespace msime::tsf {
HRESULT LoadHostImports() noexcept {
#ifdef _MSC_VER
  // DLL 名来自编译期版本表，长度和字符集在进入 SEH 边界前检查。
  constexpr wchar_t expected[] = MSIME_TSF_HOST_DLL_NAME;
  char name[sizeof(expected) / sizeof(wchar_t)]{};
  for (size_t index = 0; index + 1 < sizeof(name); ++index) {
    if (expected[index] <= 0 || expected[index] > 0x7f) return E_INVALIDARG;
    name[index] = static_cast<char>(expected[index]);
  }
  return load_imports(name);
#else
  // MinGW 使用静态导入，系统已在进入 TSF 前完成解析。
  return S_OK;
#endif
}
}
