#pragma once
#include <windows.h>
#include <filesystem>
#include <functional>
#include <memory>
#include <optional>
#include <string>
#include <cstdint>

namespace msime::settings::win7 {
class KeyboardSkinPage {
public:
  enum Control { library = 4700, name, create_skin, rename_skin, delete_skin, save_skin, apply_skin,
    preview, preview_change, refresh, notice, shape, material, pattern, monospaced, gradient,
    horizontal, border_color, photo, remove_photo, undo, redo, templates, use_template, backgrounds,
    use_background, optimize_contrast, reset_design, ai_prompt, ai_generate, ai_cancel, ai_proposals,
    ai_use, first_color = 4750, first_number = 4780 };
  struct Services {
    std::function<bool(HWND,const wchar_t*,const wchar_t*)> confirm;
    std::function<std::optional<std::filesystem::path>(HWND)> pick_photo;
    std::function<std::optional<COLORREF>(HWND,COLORREF)> pick_color;
    std::function<std::string(const std::filesystem::path&,const std::string&,const std::string&,
                             const std::function<void(uint32_t)>&)> generate;
    std::function<void(const std::string&)> cancel;
  };
  explicit KeyboardSkinPage(std::function<std::filesystem::path()> directory, Services services = {});
  ~KeyboardSkinPage();
  bool create(HWND parent, HINSTANCE instance, HFONT font, UINT dpi);
  HWND handle() const;
  static constexpr int height = 3100;
private:
  struct State;
  std::unique_ptr<State> state_;
};
}
