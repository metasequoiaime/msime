#include "FloatingToolbarPosition.h"
#include <iostream>
#include <stdexcept>
#include <string>

// 生产 Server 记下的工具栏位置：写出去的能原样读回，读不懂的文件当作没有记过。
using namespace msime::windows;
namespace {
[[noreturn]] void require_failed(int line) {
  throw std::runtime_error("Toolbar position check failed at line " + std::to_string(line));
}
#define require(value)                                                         \
  do {                                                                         \
    if (!(value))                                                              \
      require_failed(__LINE__);                                                \
  } while (false)
} // namespace

int main() {
  try {
    for (const auto &[x, y] : {std::pair{0, 0}, std::pair{1700, 980}, std::pair{-1920, -40},
                               std::pair{32767, -32768}}) {
      const auto parsed = parse_floating_toolbar_position(
          serialize_floating_toolbar_position({x, y}));
      require(parsed && parsed->x == x && parsed->y == y);
    }
    // 坐标范围与 PreviewConfig 对 floating_toolbar_x / floating_toolbar_y 的检查一致。
    require(!parse_floating_toolbar_position(R"({"x":32768,"y":0})"));
    require(!parse_floating_toolbar_position(R"({"x":0,"y":-32769})"));
    require(!parse_floating_toolbar_position(R"({"x":18446744073709551615,"y":0})"));
    // 缺键、类型不对、不是对象、根本不是 JSON，都当作没有记过位置。
    require(!parse_floating_toolbar_position(R"({"x":10})"));
    require(!parse_floating_toolbar_position(R"({"x":"10","y":20})"));
    require(!parse_floating_toolbar_position(R"({"x":10.5,"y":20})"));
    require(!parse_floating_toolbar_position(R"([10,20])"));
    require(!parse_floating_toolbar_position(""));
    require(!parse_floating_toolbar_position("{"));
    // 多出来的键不妨碍读出位置。
    const auto extra = parse_floating_toolbar_position(R"({"x":5,"y":6,"screen":"DISPLAY1"})");
    require(extra && extra->x == 5 && extra->y == 6);
    std::cout << "Toolbar position: stored positions round-trip\n";
  } catch (const std::exception &failure) {
    std::cerr << failure.what() << '\n';
    return 1;
  }
}
