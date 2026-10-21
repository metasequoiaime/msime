#pragma once

#include <map>
#include <memory>
#include <optional>
#include <string>

#include "ShuangpinKeymapModel.h"
#include "ShuangpinKeymapPainter.h"

// 只前置声明 Xlib 的连接类型：这个头文件会被 IBus 宿主和 Fcitx5 插件的大源文件包含，Xlib.h 的 None、Status 这些宏会把它们里面同名的枚举值改坏。
typedef struct _XDisplay Display;

namespace msime::linux_host {

class OverlayX11Window;

// 键位图要摆的位置：光标矩形（根窗口的设备像素，只有 X11 客户端报的光标是这种坐标）和要让开的候选窗高度（逻辑像素，见 shuangpin_keymap_candidate_clearance）。宿主自己画候选窗时（Fcitx5 的原生画布）知道它真实的矩形，measured_clearance 是由此算出的设备像素高度（shuangpin_keymap_measured_clearance），有它时不用估计值。
struct ShuangpinKeymapAnchor {
  int caret_x = 0;
  int caret_y = 0;
  int caret_width = 0;
  int caret_height = 0;
  double clearance = 0;
  std::optional<int> measured_clearance;
};

// X11 上的双拼键位图：一块点击穿透的 override-redirect 窗口，放在光标下方、让开候选窗，下面放不下就翻到光标上方，与 macOS 的 MSIMEShuangpinKeymapPanel 相同。IBus 宿主和 Fcitx5 插件共用。Wayland 原生客户端报的光标是它自己表面里的局部坐标，layer-shell 也不能把表面放到光标旁，所以那里不画键位图（宿主不调用这个类），候选窗照常工作。
class ShuangpinKeymapX11Surface {
 public:
  ShuangpinKeymapX11Surface();
  ~ShuangpinKeymapX11Surface();
  ShuangpinKeymapX11Surface(const ShuangpinKeymapX11Surface &) = delete;
  ShuangpinKeymapX11Surface &operator=(const ShuangpinKeymapX11Surface &) = delete;

  // display 是 X 显示名（":0"），空串用环境变量 DISPLAY。键位表按方案名从 host-api 读一次后缓存。返回 false 表示画不出来（连不上 X 服务器、光标矩形不可用），这时窗口已收起。
  bool show(const std::string &display, const ShuangpinKeymapHint &hint, const ShuangpinKeymapColors &colors,
            const ShuangpinKeymapAnchor &anchor);
  void hide();
  bool visible() const;

 private:
  bool open(const std::string &display);
  void close();
  const ShuangpinKeymapContent &content(const std::string &profile);

  Display *display_ = nullptr;
  // 这条连接断了（见 open_overlay_display），下一次 show 关掉重连。
  bool lost_ = false;
  std::string display_name_;
  std::unique_ptr<OverlayX11Window> window_;
  bool randr_monitors_ = false;
  std::map<std::string, ShuangpinKeymapContent> tables_;
};

}  // namespace msime::linux_host
