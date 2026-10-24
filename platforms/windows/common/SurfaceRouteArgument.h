#pragma once

#include <windows.h>
#include <shellapi.h>
#include <optional>
#include <string>
#include <string_view>

namespace msime::windows {
// 两种原生兼容宿主使用同一个启动契约：命令行优先，再读 Server 注入的环境变量。
inline std::optional<std::string> surface_route_argument(int count, const wchar_t* const* arguments,
    std::wstring_view environment, std::string_view fallback) {
  std::wstring_view value;
  if (count == 1) value = environment;
  else if (count == 2 && arguments && arguments[1] && std::wstring_view(arguments[1]).substr(0, 8) == L"--route=")
    value = std::wstring_view(arguments[1]).substr(8);
  else return std::nullopt;
  if (value.empty()) return count == 1 ? std::make_optional(std::string(fallback)) : std::nullopt;
  if (value.size() > 64) return std::nullopt;
  std::string route;
  for (const auto character : value) {
    if (!((character >= L'a' && character <= L'z') || (character >= L'0' && character <= L'9') || character == L'-' || character == L':')) return std::nullopt;
    route.push_back(static_cast<char>(character));
  }
  return route;
}
inline std::optional<std::string> requested_surface_route(std::string_view fallback) {
  int count = 0; auto** arguments = CommandLineToArgvW(GetCommandLineW(), &count);
  if (!arguments) return std::nullopt;
  wchar_t environment[65]{};
  const auto length = count == 1 ? GetEnvironmentVariableW(L"MSIME_CLIENT_ROUTE", environment, 65) : 0;
  const auto result = length >= 65 ? std::nullopt : surface_route_argument(count, arguments, environment, fallback);
  LocalFree(arguments); return result;
}
} // namespace msime::windows
