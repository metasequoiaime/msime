#define NOMINMAX
#include <windows.h>
#include <filesystem>
#include <fstream>
#include <iostream>
#include <map>
#include <sstream>
#include <stdexcept>
#include <string>
#include <vector>

namespace {
std::filesystem::path executable_path() {
  wchar_t value[32768]{};
  const DWORD length = GetModuleFileNameW(nullptr, value, 32768);
  if (!length || length >= 32768) throw std::runtime_error("cannot resolve probe executable");
  return value;
}

class ColdProbe {
 public:
  ColdProbe() {
    wchar_t temporary[MAX_PATH]{};
    const DWORD length = GetTempPathW(MAX_PATH, temporary);
    if (!length || length >= MAX_PATH) throw std::runtime_error("cannot resolve temporary directory");
    // 原探针常随产品放置；复制到独占空目录才能暴露 Win7 插件的 CRT 转发限制。
    for (unsigned attempt = 0; attempt < 100; ++attempt) {
      directory_ = std::filesystem::path(temporary) /
          (L"msime-cold-load-" + std::to_wstring(GetCurrentProcessId()) + L"-" + std::to_wstring(attempt));
      if (!CreateDirectoryW(directory_.c_str(), nullptr)) {
        if (GetLastError() == ERROR_ALREADY_EXISTS) continue;
        throw std::runtime_error("cannot create isolated probe directory");
      }
      executable_ = directory_ / L"probe.exe";
      if (!CopyFileW(executable_path().c_str(), executable_.c_str(), TRUE)) {
        RemoveDirectoryW(directory_.c_str());
        throw std::runtime_error("cannot copy isolated probe");
      }
      return;
    }
    throw std::runtime_error("isolated probe directory collisions");
  }
  ~ColdProbe() {
    DeleteFileW(executable_.c_str());
    RemoveDirectoryW(directory_.c_str());
  }
  ColdProbe(const ColdProbe&) = delete;
  ColdProbe& operator=(const ColdProbe&) = delete;

  void load(const std::filesystem::path& image) const {
    std::wstring command = L"\"" + executable_.wstring() + L"\" --cold-load \"" + image.wstring() + L"\"";
    STARTUPINFOW startup{};
    startup.cb = sizeof(startup);
    PROCESS_INFORMATION process{};
    if (!CreateProcessW(executable_.c_str(), command.data(), nullptr, nullptr, FALSE, 0,
                        nullptr, directory_.c_str(), &startup, &process))
      throw std::runtime_error("cannot start cold-load probe");
    CloseHandle(process.hThread);
    const DWORD wait = WaitForSingleObject(process.hProcess, 30000);
    DWORD result = 1;
    const bool complete = wait == WAIT_OBJECT_0 && GetExitCodeProcess(process.hProcess, &result);
    if (!complete) {
      TerminateProcess(process.hProcess, 1);
      WaitForSingleObject(process.hProcess, 5000);
    }
    CloseHandle(process.hProcess);
    if (!complete || result != 0)
      throw std::runtime_error("cold load failed: " + image.filename().u8string());
  }

 private:
  std::filesystem::path directory_;
  std::filesystem::path executable_;
};
}

int wmain(int argc, wchar_t** argv) {
  SetErrorMode(SEM_FAILCRITICALERRORS | SEM_NOOPENFILEERRORBOX);
  try {
    if (argc == 3 && std::wstring(argv[1]) == L"--cold-load") {
      const auto image = std::filesystem::absolute(argv[2]);
      if (std::filesystem::equivalent(executable_path().parent_path(), image.parent_path()))
        throw std::runtime_error("cold-load probe must run outside the image directory");
      if (!LoadLibraryExW(image.c_str(), nullptr, LOAD_WITH_ALTERED_SEARCH_PATH))
        throw std::runtime_error("cannot cold load " + image.filename().u8string() + ": " + std::to_string(GetLastError()));
      return 0;
    }
    if (argc < 4) throw std::runtime_error("expected manifest, image directory and plugin DLL names");
    const auto root = std::filesystem::absolute(argv[2]);
    if (!std::filesystem::equivalent(executable_path().parent_path(), root))
      throw std::runtime_error("run the import probe from the tested application directory; plugin children are isolated automatically");
    size_t cold_images = 0;
    {
      ColdProbe probe;
      for (int index = 3; index < argc; ++index) {
        const std::filesystem::path name(argv[index]);
        if (name.filename() != name || name.extension() != L".dll" || name.wstring().find(L':') != std::wstring::npos)
          throw std::runtime_error("invalid plugin DLL name");
        probe.load(root / name);
        ++cold_images;
      }
    }
    std::ifstream input{std::filesystem::path(argv[1])};
    if (!input) throw std::runtime_error("cannot open import manifest");
    std::map<std::string, HMODULE> modules;
    size_t count = 0;
    for (std::string line; std::getline(input, line);) {
      if (!line.empty() && line.back() == '\r') line.pop_back();
      std::vector<std::string> fields; std::istringstream stream(line);
      for (std::string field; std::getline(stream, field, '|');) fields.push_back(field);
      if (fields.size() != 5) throw std::runtime_error("invalid import manifest row");
      if (fields[4] == "1") continue;
      if (fields[4] != "0") throw std::runtime_error("invalid delay marker");
      const auto& name = fields[1];
      if (std::filesystem::path(name).filename().u8string() != name || name.find(':') != name.npos)
        throw std::runtime_error("invalid import module name");
      if (!modules.count(name)) {
        auto path = root / name;
        if (!std::filesystem::is_regular_file(path)) {
          wchar_t system[32768]{}; const auto length = GetSystemDirectoryW(system, 32768);
          if (!length || length >= 32768) throw std::runtime_error("cannot resolve system directory");
          path = std::filesystem::path(system) / name;
        }
        modules[name] = LoadLibraryExW(path.c_str(), nullptr, LOAD_WITH_ALTERED_SEARCH_PATH);
        if (!modules[name]) throw std::runtime_error("cannot load " + name + ": " + std::to_string(GetLastError()));
      }
      const auto address = GetProcAddress(modules[name], fields[2].c_str());
      if (!address) throw std::runtime_error("missing " + name + "!" + fields[2]);
      if (fields[3] != "-") {
        const auto ordinal = std::stoul(fields[3]);
        if (!ordinal || ordinal > 65535 || address != GetProcAddress(modules[name], MAKEINTRESOURCEA(ordinal)))
          throw std::runtime_error("ordinal mismatch: " + name + "!" + fields[3]);
      }
      ++count;
    }
    if (!count || !input.eof()) throw std::runtime_error("empty or unreadable import manifest");
    size_t images = 0;
    for (const auto& file : std::filesystem::directory_iterator(root)) {
      if (file.path().extension() != L".dll") continue;
      if (!LoadLibraryExW(file.path().c_str(),nullptr,LOAD_WITH_ALTERED_SEARCH_PATH))
        throw std::runtime_error("cannot load package image: " + file.path().filename().u8string() + ": " + std::to_string(GetLastError()));
      ++images;
    }
    std::cout << "Win7 actual module exports and ordinals passed: " << count << " imports, " << modules.size() << " modules, " << images << " package DLLs, " << cold_images << " cold loads\n";
    // 所有模块保留到本进程退出，避免测试卸载顺序改变运行库的依赖生命周期。
    return 0;
  } catch (const std::exception& error) { std::cerr << error.what() << '\n'; return 1; }
}
