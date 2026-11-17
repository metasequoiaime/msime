#pragma once

#include <windows.h>

namespace msime::tsf {
// 激活前解析全部宿主导入；加载失败只返回 HRESULT，不让异常进入应用。
HRESULT LoadHostImports() noexcept;
}
