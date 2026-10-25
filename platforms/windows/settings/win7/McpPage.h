#pragma once
#include <windows.h>
#include <functional>
#include <memory>
#include <string>
#include <string_view>
#include <cstdint>

namespace msime::settings::win7 {
class McpPage {
public:
  enum Control { assistant = 2600, description, instructions, primary, copy_primary,
    remove_instructions, removal, copy_removal, write, permission_write, permission_read,
    permission_write_detail, permission_read_detail, notice, server, refresh };
  struct Services {
    std::function<std::string(const std::string&)> status;
    std::function<std::string(const std::string&)> install;
    std::function<uint32_t()> load_flags;
    std::function<void(uint32_t)> save_flags;
    std::function<bool(HWND, std::wstring_view)> copy;
    std::function<bool(HWND, const wchar_t*, const wchar_t*)> confirm;
  };
  explicit McpPage(Services services = {});
  ~McpPage();
  McpPage(const McpPage&) = delete;
  McpPage& operator=(const McpPage&) = delete;
  bool create(HWND parent, HINSTANCE instance, HFONT font, UINT dpi);
  HWND handle() const;
  static constexpr int height = 1440;
private:
  struct State;
  std::unique_ptr<State> state_;
};
}
