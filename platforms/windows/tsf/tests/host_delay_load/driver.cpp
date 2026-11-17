#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>
#include <filesystem>
#include <cstdio>
#include <cstring>
#include <stdexcept>
#include <string>

namespace fs = std::filesystem;
namespace {
void require(bool condition, const char *message) {
  if (!condition) throw std::runtime_error(message);
}
struct Sandbox {
  fs::path previous = fs::current_path();
  fs::path root = fs::temp_directory_path() /
      (L"msime-delay-test-" + std::to_wstring(GetCurrentProcessId()));
  HMODULE probe = nullptr;
  Sandbox() { require(fs::create_directory(root), "无法创建合成测试目录"); }
  ~Sandbox() {
    if (probe) FreeLibrary(probe);
    fs::current_path(previous);
    std::error_code ignored;
    fs::remove_all(root, ignored);
  }
};
}

int wmain(int count, wchar_t **args) {
  SetErrorMode(SEM_FAILCRITICALERRORS | SEM_NOGPFAULTERRORBOX);
  try {
    require(count == 5, "缺少夹具参数");
    const std::wstring scenario = args[1];
    Sandbox sandbox;
    const auto probe = sandbox.root / L"probe.dll";
    const auto dependency = sandbox.root / L"msime_test_delay_host.dll";
    fs::copy_file(args[2], probe);
    if (scenario == L"complete") fs::copy_file(args[3], dependency);
    if (scenario == L"missing-export") fs::copy_file(args[4], dependency);
    if (scenario == L"decoy") {
      const auto decoy = sandbox.root / L"decoy";
      fs::create_directory(decoy);
      fs::copy_file(args[3], decoy / dependency.filename());
      fs::current_path(decoy);
    }
    sandbox.probe = LoadLibraryW(probe.c_str());
    require(sandbox.probe != nullptr, "延迟依赖不应阻止夹具 DLL 装载");
    const auto address = GetProcAddress(sandbox.probe, "msime_test_probe");
    HRESULT (*invoke)() = nullptr;
    static_assert(sizeof(invoke) == sizeof(address));
    std::memcpy(&invoke, &address, sizeof(invoke));
    require(invoke != nullptr, "夹具缺少探测入口");
    const auto result = invoke();
    const auto expected = scenario == L"complete" ? S_OK : HRESULT_FROM_WIN32(
        scenario == L"missing-export" ? ERROR_PROC_NOT_FOUND : ERROR_MOD_NOT_FOUND);
    require(result == expected, "加载失败必须返回准确 HRESULT，不能终止应用或从当前目录加载");
    if (scenario == L"retry") {
      fs::copy_file(args[3], dependency);
      require(invoke() == S_OK, "补齐依赖后必须允许重新激活");
    }
    std::puts("合成宿主延迟导入回归通过");
    return 0;
  } catch (const std::exception &error) {
    std::fprintf(stderr, "%s\n", error.what());
    return 1;
  }
}
