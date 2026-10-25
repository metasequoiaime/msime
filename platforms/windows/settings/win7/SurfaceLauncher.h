#pragma once

#include "ControlText.h"
#include <filesystem>

namespace msime::settings::win7 {
inline bool launch_surface(std::string_view route) {
  if (route.empty() || route.find_first_not_of("abcdefghijklmnopqrstuvwxyz0123456789:-") != std::string_view::npos) return false;
  wchar_t executable[32768]{}; const auto length = GetModuleFileNameW(nullptr,executable,32768);
  if (!length || length >= 32768) return false;
  const auto file = route.substr(0,9) == "settings:" ? L"msime-client-settings-win7.exe" : L"msime-client-panels-win7.exe";
  const auto path = std::filesystem::path(executable).parent_path() / file;
  auto command = L"\"" + path.wstring() + L"\" --route=" + control_text::wide(std::string(route));
  STARTUPINFOW startup{}; startup.cb = sizeof(startup); PROCESS_INFORMATION process{};
  if (!CreateProcessW(path.c_str(),command.data(),nullptr,nullptr,FALSE,0,nullptr,nullptr,&startup,&process)) return false;
  CloseHandle(process.hThread); CloseHandle(process.hProcess); return true;
}
}
