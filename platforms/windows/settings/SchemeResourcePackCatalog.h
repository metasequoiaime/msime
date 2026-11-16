#pragma once

#include "SchemeResourcePackModel.h"
#include <filesystem>
#include <optional>
#include <string>
#include <winrt/Windows.Foundation.Collections.h>
#include <winrt/Windows.Data.Json.h>

namespace msime::settings::scheme_resource {

// 仅在资源包工作线程调用；不改共享资源包的安装状态或校验规则。
inline std::string with_bundled_resources(
    std::string_view response, std::optional<std::string> const &options) {
  if (!options) return std::string(response);
  using namespace winrt::Windows::Data::Json;
  try {
    const auto host = JsonObject::Parse(winrt::to_hstring(*options));
    const auto resources = std::filesystem::path(std::wstring(host.GetNamedString(L"resources", L"")));
    const auto languages = std::filesystem::path(std::wstring(host.GetNamedString(L"language_dictionaries", L"")));
    auto catalog = JsonObject::Parse(winrt::to_hstring(response));
    for (auto const &item : catalog.GetNamedArray(L"value")) {
      auto status = item.GetObject();
      const auto *pack = find(std::wstring(status.GetNamedString(L"id", L"")));
      if (!pack) continue;
      const auto &directory = pack->id == L"language-dictionaries" ? languages : resources;
      if (directory.is_absolute() && bundled(*pack, [&directory](auto name) {
            std::error_code error;
            return std::filesystem::is_regular_file(directory / name, error);
          }))
        status.SetNamedValue(L"state", JsonValue::CreateStringValue(L"bundled"));
    }
    return winrt::to_string(catalog.Stringify());
  } catch (...) {
    // 运行时选项暂时不可读时仍显示共享安装器读回的状态，不让后台异常结束设置进程。
    return std::string(response);
  }
}

} // namespace msime::settings::scheme_resource
