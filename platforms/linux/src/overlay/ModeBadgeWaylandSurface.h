#pragma once
#include <cstddef>
#include <cstdint>
#include <memory>
#include <optional>
#include <string>

#include "ModeBadgeSurface.h"

struct wl_compositor;
struct wl_display;
struct wl_registry;
struct wl_shm;
struct wl_surface;
struct wl_buffer;
struct zwlr_layer_shell_v1;
struct zwlr_layer_surface_v1;

namespace msime::linux_host {

// 中英文切换后短暂显示的徽章：产品 logo 加一个「中」或「英」。
//
// 这是这个平台上第一个由输入法自己绘制的浮层，与 README 长期记录的「Linux 不创建脱离
// 输入上下文的悬浮窗口」相反，属于仓库所有者明确要求的例外：Fcitx5 面板那个提示只接受
// 一个字符串，带不了图，而这个提示要的就是那张图。
//
// 不与语音波形浮层合并，是因为后者连着 IBus 回退面和 X11 实现，把它整套拖进 Fcitx5 插件
// 会带上 glib 和 xfixes；这里只需要 layer-shell 加一块共享内存。两者共用的只有技术手法，
// 各自的模型和生命周期没有重合。
class ModeBadgeWaylandSurface final : public ModeBadgeSurface {
 public:
  ~ModeBadgeWaylandSurface() override;
  ModeBadgeWaylandSurface(const ModeBadgeWaylandSurface &) = delete;
  ModeBadgeWaylandSurface &operator=(const ModeBadgeWaylandSurface &) = delete;

  // 连不上合成器、没有 layer-shell、或者当前不是 Wayland 会话时返回空指针，由调用方
  // 回退到面板自己的文字提示。
  static std::unique_ptr<ModeBadgeWaylandSurface> create();

  // 画出徽章并立即提交。icon_path 为空或读不到时只画文字，不报错。layer-shell 拿不到光标的屏幕坐标，caret 一律忽略，徽章固定在右下角。
  bool show(const std::string &text, const std::string &icon_path, const ModeBadgeStyle &style,
            std::optional<ModeBadgeCaret> caret) override;
  bool set_opacity(double alpha) override;
  void hide() override;
  bool follows_caret() const override { return false; }

 private:
  ModeBadgeWaylandSurface() = default;
  bool ensure_surface();
  void resize(int width, int height);
  void release_buffer();
  void destroy_surface();
  bool paint(double alpha);

  static void registry_global(void *data, wl_registry *registry, uint32_t name,
                              const char *interface, uint32_t version);
  static void registry_remove(void *data, wl_registry *registry, uint32_t name);
  static void layer_configure(void *data, zwlr_layer_surface_v1 *surface,
                              uint32_t serial, uint32_t width, uint32_t height);
  static void layer_closed(void *data, zwlr_layer_surface_v1 *surface);

  wl_display *display_ = nullptr;
  wl_registry *registry_ = nullptr;
  wl_compositor *compositor_ = nullptr;
  wl_shm *shm_ = nullptr;
  zwlr_layer_shell_v1 *layer_shell_ = nullptr;
  wl_surface *surface_ = nullptr;
  zwlr_layer_surface_v1 *layer_surface_ = nullptr;
  wl_buffer *buffer_ = nullptr;
  void *pixels_ = nullptr;
  // 徽章在表面像素里的尺寸，每次 show() 按字体测量；变了就重新分配缓冲。初值是默认徽章（不带标志、带阴影边）的大致尺寸，第一次 configure 就能要到差不多的大小。
  int width_ = 50;
  int height_ = 48;
  // 上一次 show 画的内容，set_opacity 按它重画。
  std::string text_;
  std::string icon_path_;
  FloatingSurfaceColors colors_;
  ModeBadgeLayout layout_;
  std::size_t buffer_bytes_ = 0;
  bool configured_ = false;
  bool closed_ = false;
  bool visible_ = false;
};

}  // namespace msime::linux_host
