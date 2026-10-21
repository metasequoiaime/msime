#pragma once

#include <filesystem>
#include <functional>
#include <memory>
#include <optional>
#include <string>

#include "FloatingPanelColors.h"
#include "FloatingToolbarModel.h"
#include "FloatingToolbarSurface.h"
#include "OverlayLoop.h"

namespace msime::linux_host {

// 悬浮工具栏（macOS 的 MetasequoiaFloatingToolbarPanel）在 Linux 上的控制部分：一个宿主进程只有一条，跟着当前「属主」（IBus 的引擎对象、Fcitx5 的输入上下文）走。规则照 macOS：
//
// - 输入法激活（获得焦点、切换到本输入法）时属主交给它；属主失焦不收起工具栏，换到别的输入法或属主销毁时才收起，与 macOS 的 deactivateForDelegate:、deactivateForInputSourceSwitch 相同。
// - 连续 10 秒没有按键或对工具栏的点击就自动隐藏，下一次输入再回来（noteInputForDelegate:）；只是换个输入框不唤醒已经空闲隐藏的工具栏，新的激活或换了属主才唤醒。
// - 前台窗口全屏时不显示。这一条跟 Windows 走：macOS 的 MetasequoiaFloatingToolbarShouldShow 还留着全屏这个参数，但现在总是传「不是全屏」，面板盖在全屏应用上面；X11 的 override-redirect 窗口会盖住全屏的视频和游戏，所以 Linux 收起。
// - 用户拖动后的位置记在状态目录里，下次显示时恢复并收进工作区（frame autosave）。
//
// 按钮和菜单项的动作交给宿主，宿主照「工具栏」子菜单里同一项的路径去做。
class FloatingToolbar {
 public:
  using Owner = const void *;
  using Handler = std::function<void(Owner owner, FloatingToolbarAction action, const std::string &argument)>;
  // 按回调建一个显示后端；拿不到时返回空指针。
  using SurfaceFactory =
      std::function<std::unique_ptr<FloatingToolbarSurface>(FloatingToolbarSurface::Callbacks callbacks)>;

  struct Options {
    // 产品图标（PNG）；空时左端只画简化标志。
    std::string icon_path;
    // 这一版的产品名，写进设置按钮的提示和工具菜单。
    std::string display_name;
    // 记位置的文件；空时只在这次运行里记住。
    std::optional<std::filesystem::path> position_file;
    // 属主失焦时也收起。Wayland 会话里工具栏经 Xwayland 显示，看不到 Wayland 原生窗口是否全屏，失焦后留在屏幕上会盖住全屏的视频，所以那里失焦就收起，下一次获得焦点再出现。X11 会话照 macOS 留着。
    bool hide_on_focus_out = false;
    // 切换类的动作只对有焦点的属主生效（IBus：属性路径只认获得焦点的引擎）。属主失焦后工具栏留在屏幕上时，这些按钮和菜单项画成不能用，免得点了没反应；Fcitx5 把失焦的上下文留给状态区菜单，动作照样生效，不设这一项。
    bool actions_need_focus = false;
  };

  // 后端建不出来时返回空指针，宿主只用菜单里的「工具栏」子菜单。
  static std::unique_ptr<FloatingToolbar> create(OverlayLoop loop, Handler handler, Options options,
                                                 const SurfaceFactory &surfaces);
  // 这个构建里能用的显示后端：X11（GNOME 的 Wayland 会话经 Xwayland 也用它）。见 FloatingToolbarFactory.cpp。
  static SurfaceFactory default_surfaces(OverlayLoop loop);
  ~FloatingToolbar();
  FloatingToolbar(const FloatingToolbar &) = delete;
  FloatingToolbar &operator=(const FloatingToolbar &) = delete;

  // 偏好或主题变了。新打开开关算一次输入（工具栏随即出现并重新计时）。
  void configure(const FloatingToolbarConfig &config, const FloatingToolbarColors &colors);
  // 模式、标点、全半角、简繁或方案变了；只有属主的状态才显示。
  void update(Owner owner, const FloatingToolbarState &state);
  // 属主激活：获得焦点或切换到本输入法。state 是它的状态，与属主一起换上，工具栏不会先画出上一个属主的状态。
  void activate(Owner owner, const FloatingToolbarState &state);
  // 属主上的一次按键。
  void note_input(Owner owner);
  // 属主失焦。macOS 照旧留着工具栏，只有 hide_on_focus_out 时收起，直到下一次 activate。
  void focus_out(Owner owner);
  // 属主不再是本输入法（换了输入法、上下文销毁）：收起并放开属主。owner 不是当前属主时什么也不做。
  void deactivate(Owner owner);
  Owner owner() const { return owner_; }
  bool visible() const;

 private:
  FloatingToolbar(OverlayLoop loop, Handler handler, Options options);
  void refresh();
  void restart_idle_timer();
  void interacted();
  void moved(WaveOverlayPosition position);

  OverlayLoop loop_;
  Handler handler_;
  Options options_;
  std::unique_ptr<FloatingToolbarSurface> surface_;
  OverlayLoop::Token idle_timer_;
  FloatingToolbarConfig config_;
  FloatingToolbarColors colors_;
  FloatingToolbarState state_;
  bool configured_ = false;
  Owner owner_ = nullptr;
  bool active_ = false;
  bool focused_ = false;
  bool idle_hidden_ = false;
  bool fullscreen_ = false;
  // 上一次交给后端的内容，相同就不重画。
  std::optional<FloatingToolbarSurface::Content> presented_;
  std::optional<WaveOverlayPosition> saved_;
};

}  // namespace msime::linux_host
