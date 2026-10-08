#include "candidate/GameCandidateAnchor.h"
#include <iostream>
#include <limits>
#include <stdexcept>
#include <string>

using namespace msime::windows;
namespace {
[[noreturn]] void require_failed(int line) {
  throw std::runtime_error("Game candidate anchor test failed at line " +
                           std::to_string(line));
}
#define require(value)                                                         \
  do {                                                                         \
    if (!(value))                                                              \
      require_failed(__LINE__);                                                \
  } while (false)
GameAnchorInput game(RECT client, std::optional<POINT> anchor) {
  GameAnchorInput input;
  input.anchor = anchor;
  input.game_host = true;
  input.foreground_owned = true;
  input.client = client;
  return input;
}
bool at(const std::optional<POINT> &point, LONG x, LONG y) {
  return point && point->x == x && point->y == y;
}
} // namespace
int main() {
  try {
    const RECT screen{0, 0, 1920, 1080};
    // INVALID_Y：游戏没给出 extent，放到客户区左下部。横向取 24 DIP 和宽度 1/20 里较大的一个，纵向在底边往上 1/5 处。
    require(at(game_candidate_anchor(game(screen, std::nullopt)), 96, 864));
    // 左上角的垃圾值，以及贴着客户区顶边的锚点：真实的文本行不会贴着顶边。
    require(at(game_candidate_anchor(game(screen, POINT{0, 0})), 96, 864));
    require(at(game_candidate_anchor(game(screen, POINT{500, 1})), 96, 864));
    require(!game_candidate_anchor(game(screen, POINT{500, 2})));
    // 窗口化的游戏：客户区外的锚点是垃圾值，兜底点跟着客户区走。
    const RECT window{100, 100, 900, 700};
    require(at(game_candidate_anchor(game(window, POINT{0, 0})), 140, 580));
    require(at(game_candidate_anchor(game(window, POINT{950, 400})), 140, 580));
    require(at(game_candidate_anchor(game(window, POINT{300, 750})), 140, 580));
    // 有效锚点保持不变，客户区的右边和底边也算在里面。
    require(!game_candidate_anchor(game(window, POINT{300, 400})));
    require(!game_candidate_anchor(game(window, POINT{900, 700})));
    // 非游戏会话不兜底：INVALID_Y 照旧隐藏，垃圾值照旧使用。
    auto ordinary = game(screen, std::nullopt);
    ordinary.game_host = false;
    require(!game_candidate_anchor(ordinary));
    ordinary.anchor = POINT{0, 0};
    require(!game_candidate_anchor(ordinary));
    // 前台不属于这个客户端进程（Alt-Tab 切走、前台是启动器）时不兜底，免得锚到别的应用上。
    auto elsewhere = game(screen, std::nullopt);
    elsewhere.foreground_owned = false;
    require(!game_candidate_anchor(elsewhere));
    // 客户区为空（最小化或还没有大小）时不兜底。
    require(!game_candidate_anchor(game(RECT{10, 10, 10, 400}, std::nullopt)));
    require(!game_candidate_anchor(game(RECT{10, 10, 400, 10}, std::nullopt)));
    // 主显示器左边的副屏，坐标是负的。
    const RECT left{-1920, 0, 0, 1080};
    require(at(game_candidate_anchor(game(left, std::nullopt)), -1824, 864));
    require(!game_candidate_anchor(game(left, POINT{-1000, 500})));
    require(at(game_candidate_anchor(game(left, POINT{10, 500})), -1824, 864));
    // DPI 1.5：小客户区里 24 DIP 的边距按缩放放大，比宽度的 1/20 大。
    const RECT compact{0, 0, 400, 300};
    auto scaled = game(compact, std::nullopt);
    scaled.scale = 1.5;
    require(at(game_candidate_anchor(scaled), 36, 240));
    scaled.scale = 1.0;
    require(at(game_candidate_anchor(scaled), 24, 240));
    // 拿不到 DPI 时按 1 倍处理。
    scaled.scale = 0.0;
    require(at(game_candidate_anchor(scaled), 24, 240));
    scaled.scale = std::numeric_limits<double>::quiet_NaN();
    require(at(game_candidate_anchor(scaled), 24, 240));
    std::cout << "Game candidate anchor: fallback only for owned game sessions\n";
  } catch (const std::exception &failure) {
    std::cerr << failure.what() << '\n';
    return 1;
  } catch (...) {
    std::cerr << "Game candidate anchor test failed with an unknown error\n";
    return 1;
  }
}
