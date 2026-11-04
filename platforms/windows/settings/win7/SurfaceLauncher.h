#pragma once

#include "ControlText.h"
#include <filesystem>

namespace msime::settings::win7 {
// 路由交给哪个程序：设置类别给设置窗口，其余给面板宿主。面板源码同时编进 Win7 包（这里的 Win7 设置和 msime-client-panels-win7.exe）和现代 Windows 包（msime-client-panels.exe，旁边的设置是 WinUI 的 msime-client-settings.exe），由 panels/win7/CMakeLists.txt 的 MSIME_PANELS_MODERN_WINDOWS 区分。
inline const wchar_t* surface_executable_name(std::string_view route) {
  const bool settings = route.substr(0,9) == "settings:";
#if defined(MSIME_PANELS_MODERN_WINDOWS) && MSIME_PANELS_MODERN_WINDOWS
  return settings ? L"msime-client-settings.exe" : L"msime-client-panels.exe";
#else
  return settings ? L"msime-client-settings-win7.exe" : L"msime-client-panels-win7.exe";
#endif
}
inline bool launch_surface(std::string_view route) {
  if (route.empty() || route.find_first_not_of("abcdefghijklmnopqrstuvwxyz0123456789:-") != std::string_view::npos) return false;
  wchar_t executable[32768]{}; const auto length = GetModuleFileNameW(nullptr,executable,32768);
  if (!length || length >= 32768) return false;
  const auto file = surface_executable_name(route);
  const auto path = std::filesystem::path(executable).parent_path() / file;
  auto command = L"\"" + path.wstring() + L"\" --route=" + control_text::wide(std::string(route));
  STARTUPINFOW startup{}; startup.cb = sizeof(startup); PROCESS_INFORMATION process{};
  if (!CreateProcessW(path.c_str(),command.data(),nullptr,nullptr,FALSE,0,nullptr,nullptr,&startup,&process)) return false;
  CloseHandle(process.hThread); CloseHandle(process.hProcess); return true;
}
}
