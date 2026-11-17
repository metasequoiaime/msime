#include "CandidateWindow.h"
#include <cstdio>
#include <stdexcept>

using namespace msime::windows;
namespace {
void require(bool condition, const char *message) {
  if (!condition)
    throw std::runtime_error(message);
}
void verify_layout(bool horizontal) {
  CandidatePresentation frame{};
  frame.visible = true;
  frame.session = frame.generation = frame.render_serial = 1;
  frame.x = frame.y = 100;
  frame.preedit = "synthetic";
  frame.page_count = 3;
  frame.page_size = 1;
  PresentationCandidate item{};
  item.text = "synthetic";
  item.highlighted = true;
  frame.candidates.push_back(item);
  unsigned clicks = 0;
  CandidateWindow window(
      [&]() -> std::optional<CandidatePresentation> { return frame; },
      [&](const CandidateClick &) { ++clicks; }, 16, 32);
  CandidateLayoutSettings layout;
  layout.horizontal = horizontal;
  layout.show_preedit = layout.show_app_logo = layout.show_page_number = false;
  const auto height = [&] {
    window.set_layout(layout);
    ++frame.render_serial;
    const auto deadline = GetTickCount64() + 100;
    do {
      window.refresh();
      UpdateWindow(window.handle());
      MSG message{};
      while (PeekMessageW(&message, nullptr, 0, 0, PM_REMOVE)) {
        TranslateMessage(&message);
        DispatchMessageW(&message);
      }
      Sleep(5);
    } while (GetTickCount64() < deadline && !window.failed());
    require(!window.failed() && IsWindowVisible(window.handle()),
            "Candidate window did not render");
    RECT client{};
    require(GetClientRect(window.handle(), &client) != FALSE,
            "Candidate client geometry unavailable");
    require(window.painted_card().has_value(), "Candidate card was not painted");
    return client.bottom;
  };
  // 旧的全隐藏配置不留首行；新增开关默认开启也必须保持这一点。
  const auto empty_header = height();
  layout.show_preedit_area = false;
  const auto collapsed = height();
  require(empty_header == collapsed, "Hidden header retained a blank preedit row");
  // 收起首行后按真正画出的卡片中心点选，不能仍按旧首行偏移命中。
  const auto card = *window.painted_card();
  const auto point = MAKELPARAM((card.left + card.right) / 2,
                               (card.top + card.bottom) / 2);
  SendMessageW(window.handle(), WM_LBUTTONDOWN, MK_LBUTTON, point);
  SendMessageW(window.handle(), WM_LBUTTONUP, 0, point);
  require(clicks == 1, "Collapsed candidate row did not accept its click");
  for (unsigned kind = 0; kind != 4; ++kind) {
    layout.show_preedit = kind == 0;
    layout.show_app_logo = kind == 1;
    layout.show_page_number = kind == 2;
    frame.caret_edit_suffix = kind == 3;
    layout.show_preedit_area = true;
    const auto expanded = height();
    require(expanded > collapsed, "Visible header content did not reserve its row");
    layout.show_preedit_area = false;
    require(height() == collapsed, "Disabled preedit area retained header space");
    layout.show_preedit_area = true;
    require(height() == expanded, "Reopening preedit area changed the saved layout");
  }
}
} // 匿名命名空间
int main() {
  try {
    verify_layout(false);
    verify_layout(true);
    std::puts("candidate preedit area: legacy geometry, header content, restore and clicks passed");
    return 0;
  } catch (const std::exception &error) {
    std::fprintf(stderr, "candidate preedit area: %s\n", error.what());
    return 1;
  }
}
