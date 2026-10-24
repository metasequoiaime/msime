#pragma once

#include "../../common/StateDirectory.h"
#include "../../common/ClientAgent.h"
#include "../../settings/win7/ThemeData.h"
#include <fstream>

namespace msime::windows::win7 {
using Json = msime::settings::win7::Json;
inline std::filesystem::path panel_environment_path(const wchar_t* name) {
  const auto size = GetEnvironmentVariableW(name, nullptr, 0);
  if (!size || size > 32768) return {};
  std::wstring value(size, L'\0'); const auto length = GetEnvironmentVariableW(name, value.data(), size);
  if (!length || length >= size) return {};
  value.resize(length); return value;
}
struct PanelContext {
  std::filesystem::path directory;
  Json options;
};
inline PanelContext panel_context() {
  auto directory = panel_environment_path(L"MSIME_CLIENT_STATE_DIR");
  const bool injected = directory.is_absolute();
  if (!injected) directory = resolve_state_directory();
  auto file = panel_environment_path(L"MSIME_CLIENT_HOST_OPTIONS");
  if (!file.is_absolute()) file = directory / L"runtime-options.json";
  std::ifstream input(file, std::ios::binary); std::string bytes(1024 * 1024 + 1, '\0');
  if (input) input.read(bytes.data(), static_cast<std::streamsize>(bytes.size()));
  bytes.resize(static_cast<size_t>(input.gcount()));
  auto options = bytes.size() <= 1024 * 1024 ? Json::parse(bytes, nullptr, false) : Json();
  // 配置缺失或损坏时用空值表示不可用，不能将解析器的 discarded 状态写进请求 JSON。
  if (!options.is_object()) options = Json();
  if (!injected && options.is_object() && options.contains("preferences_directory") && options.at("preferences_directory").is_string()) {
    const auto value = std::filesystem::u8path(options.at("preferences_directory").get<std::string>());
    if (value.is_absolute()) directory = value;
  }
  return {directory, std::move(options)};
}
inline RECT panel_frame(int width, int height, UINT& dpi) {
  MONITORINFO monitor{sizeof(monitor)};
  if (!GetMonitorInfoW(MonitorFromWindow(GetForegroundWindow(), MONITOR_DEFAULTTONEAREST), &monitor)) {
    if (!SystemParametersInfoW(SPI_GETWORKAREA, 0, &monitor.rcWork, 0)) monitor.rcWork = {0, 0, GetSystemMetrics(SM_CXSCREEN), GetSystemMetrics(SM_CYSCREEN)};
  }
  const auto available_width = std::max<LONG>(1, monitor.rcWork.right - monitor.rcWork.left);
  const auto available_height = std::max<LONG>(1, monitor.rcWork.bottom - monitor.rcWork.top);
  dpi = std::min(dpi, static_cast<UINT>(std::max<LONG>(48, std::min(available_width * 96 / width, available_height * 96 / height))));
  const auto actual_width = MulDiv(width, static_cast<int>(dpi), USER_DEFAULT_SCREEN_DPI);
  const auto actual_height = MulDiv(height, static_cast<int>(dpi), USER_DEFAULT_SCREEN_DPI);
  const auto left = monitor.rcWork.left + (available_width - actual_width) / 2;
  const auto top = monitor.rcWork.top + (available_height - actual_height) / 2;
  return {left, top, left + actual_width, top + actual_height};
}
inline std::filesystem::path panel_account_directory() {
  PWSTR local = nullptr;
  if (FAILED(SHGetKnownFolderPath(FOLDERID_LocalAppData, 0, nullptr, &local))) { CoTaskMemFree(local); return {}; }
  const auto result = std::filesystem::path(local) / MSIME_EDITION_USER_DATA_DIRECTORY / L"account";
  CoTaskMemFree(local); return result;
}
inline Json panel_account_request(const Json& action) {
  Json request{{"directory", panel_account_directory().u8string()}, {"action", action}};
  PWSTR local = nullptr;
  if (SUCCEEDED(SHGetKnownFolderPath(FOLDERID_LocalAppData, 0, nullptr, &local)) && local)
    request["legacy_directory"] = (std::filesystem::path(local) / MSIME_EDITION_TAURI_IDENTIFIER).u8string();
  CoTaskMemFree(local);
  static const auto agent = [] {
    const auto value = msime::settings::win7::theme_data::value(msime::settings::win7::theme_data::call(msime_client_client_agent,
        {{"platform", "windows"}, {"version", MSIME_WINDOWS_VERSION}, {"edition", MSIME_EDITION_ID},
         {"device", windows_device_model()}, {"system", windows_system_description()}}));
    return value && value->is_string() ? value->get<std::string>() : std::string{};
  }();
  if (!agent.empty()) request["client_agent"] = agent;
  return request;
}
} // namespace msime::windows::win7
