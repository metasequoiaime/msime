#include "ToolbarImageState.h"
#include <cassert>
#include <fstream>

int main() {
  using namespace msime::windows;
  wchar_t temporary[MAX_PATH]{}, unique[MAX_PATH]{};
  assert(GetTempPathW(MAX_PATH, temporary) && GetTempFileNameW(temporary, L"mst", 0, unique));
  assert(DeleteFileW(unique) && CreateDirectoryW(unique, nullptr));
  const std::filesystem::path root(unique);
  // 合成 APNG 两帧分别点亮左右端，中间始终透明。
  const unsigned char png[] = {137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 4, 0, 0, 0, 1, 8, 6, 0, 0, 0, 249, 60, 15, 205, 0, 0, 0, 8, 97, 99, 84, 76, 0, 0, 0, 2, 0, 0, 0, 0, 243, 141, 147, 112, 0, 0, 0, 26, 102, 99, 84, 76, 0, 0, 0, 0, 0, 0, 0, 4, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 100, 3, 232, 0, 0, 141, 244, 238, 119, 0, 0, 0, 15, 73, 68, 65, 84, 120, 156, 99, 248, 207, 192, 0, 68, 8, 0, 0, 28, 244, 1, 255, 129, 122, 246, 218, 0, 0, 0, 26, 102, 99, 84, 76, 0, 0, 0, 1, 0, 0, 0, 4, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 100, 3, 232, 0, 0, 22, 135, 4, 163, 0, 0, 0, 18, 102, 100, 65, 84, 0, 0, 0, 2, 120, 156, 99, 96, 64, 2, 255, 129, 8, 0, 5, 12, 1, 255, 160, 243, 228, 3, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130};
  {
    std::ofstream file(root / L"bar.png", std::ios::binary);
    file.write(reinterpret_cast<const char *>(png), sizeof(png));
    assert(file.good());
  }
  const auto manifest = nlohmann::json::parse(R"({
    "image":"bar.png","imageWidthDip":80,"imageHeightDip":40,
    "buttonDefaults":{"widthDip":20,"heightDip":20},
    "buttonFlow":{"xDip":0,"yDip":40,"direction":"horizontal","gapDip":0},
    "buttons":{"status":{"xDip":90,"yDip":0,"images":{"cn":"bar.png","en":"bar.png"}},
               "fullwidth":{"visible":false,"images":{}}}
  })");
  ToolbarImageState image;
  image.set_skin(manifest, root);
  image.update({0,1,6}, {"cn","half","cn","simplified","","",""});
  assert(image.active() && image.buttons.size() == 2 && image.width == 110 && image.height == 60);
  assert(image.buttons[0].image == "bar.png" && std::abs(image.buttons[0].icon - 16.4) < 0.001);
  assert(image.button_at(100, 10) == 0 && image.button_at(10, 50) == 1);
  assert(!image.button_at(85, 10) && !image.button_at(100, 30));
  assert(image.drag_at(10, 10) && image.drag_at(70, 10) && !image.drag_at(40, 10));
  assert(!image.drag_at(100, 10) && !image.drag_at(100, 30));
  HWND window = CreateWindowExW(WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE, L"STATIC", L"synthetic toolbar", WS_POPUP,
                                0, 0, 165, 90, nullptr, nullptr, GetModuleHandleW(nullptr), nullptr);
  assert(window);
  image.apply_region(window, 1.5);
  HRGN shape = CreateRectRgn(0,0,0,0);
  assert(shape && GetWindowRgn(window, shape) != ERROR);
  assert(PtInRegion(shape, 10, 10) && PtInRegion(shape, 110, 10));
  assert(!PtInRegion(shape, 60, 10) && PtInRegion(shape, 140, 10) && PtInRegion(shape, 5, 70));
  assert(!PtInRegion(shape, 140, 50));
  DeleteObject(shape);
  DestroyWindow(window);
  // 已加载的图在状态切换时不读文件；主动重载后才应用资源变化。
  assert(std::filesystem::remove(root / L"bar.png"));
  image.update({0,6}, {"en","half","cn","simplified","","",""});
  assert(image.drag_at(10,10) && !image.drag_at(40,10));
  image.set_skin(manifest, root);
  image.update({0,6}, {"en","half","cn","simplified","","",""});
  assert(image.drag_at(40,10));
  image.set_skin(nullptr, {});
  assert(!image.active() && image.buttons.empty());
  assert(std::filesystem::remove(root));
}
