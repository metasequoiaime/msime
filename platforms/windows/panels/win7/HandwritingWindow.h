#pragma once

#include <windows.h>
#include <functional>
#include <filesystem>
#include <memory>
#include <string_view>
#include <vector>

namespace msime::windows::win7 {

enum HandwritingControl {
  handwriting_canvas = 100, handwriting_undo, handwriting_clear, handwriting_status,
  handwriting_model_status, handwriting_model_progress, handwriting_model_download,
  handwriting_model_import, handwriting_model_remove, handwriting_model_cancel, handwriting_model_refresh,
  handwriting_candidate = 200,
};

class HandwritingWindow final {
public:
  // 测试只在输入提交边界注入；笔划、窗口消息和识别均使用实际实现。
  using SubmitCandidate = std::function<bool(std::wstring_view)>;
  // 系统文件选择边界可注入合成路径，导入仍经过真实共享安装器。
  using SelectModelFiles = std::function<std::vector<std::filesystem::path>(HWND)>;
  explicit HandwritingWindow(SubmitCandidate submit = {}, SelectModelFiles select = {});
  ~HandwritingWindow();
  HandwritingWindow(const HandwritingWindow&) = delete;
  HandwritingWindow& operator=(const HandwritingWindow&) = delete;
  bool create(HINSTANCE instance);
  int run(int show);
  HWND handle() const;

private:
  struct State;
  std::unique_ptr<State> state_;
};

} // namespace msime::windows::win7
