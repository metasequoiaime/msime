#pragma once
#include "ComponentFailure.h"
#include "TypingEffectOverlayPolicy.h"
#include <cstdint>
#include <optional>
#include <string>
#include <vector>
// 先包含 windows.h：它的 DrawText 宏要作用到 Direct2D 的声明上。
#include <windows.h>
#include <d2d1.h>
#include <dwrite.h>
#include <wrl/client.h>

namespace msime::windows {
// 一次打字特效要画的东西，由候选窗在界面线程上取到特效后交来。
struct TypingEffectPresentation {
  TypingEffect effect;
  TypingEffectSettings settings;
  TypingEffectPalette palette;
  // 候选卡片（不含阴影）的屏幕矩形，物理像素；候选窗不在屏幕上时为空。
  std::optional<TypingRect> card;
  // 候选窗本身：浮层排在它下面，火花不挡候选。
  HWND candidate_window = nullptr;
  // 这个会话最近一次组字的锚点（TSF 报来的物理像素，文字底边的左端），系统光标取不到时用它估一个光标行。
  std::optional<POINT> anchor;
  // 主题的强调色 0xRRGGBB，特效包没有颜色时用它。
  uint32_t accent = 0x3B82F6u;
  // Windows 的「显示动画」开关（SPI_GETCLIENTAREAANIMATION）和节电模式。
  bool animations = true;
  bool power_saver = false;
};

// 光标处的打字特效浮层，对应 macOS 的 MSIMETypingEffectPanel：在光标处迸出火花（火花、Power Mode，特效包的颜色一键一色、颗数照包里的），候选窗不在时在光标所在行闪一下，再加上连击徽标（有卡片时在卡片右上角上方，没有时在光标旁），升档或 Power Mode 时徽标弹一下。卡片本身的闪光和 Power Mode 的抖动由候选窗自己画。
// 一个不激活、不进任务栏、置顶的分层窗口，WS_EX_LAYERED | WS_EX_TRANSPARENT 让鼠标点击穿过去；用 UpdateLayeredWindow 按像素带透明度贴上去，没有合成器（远程桌面、Wine）也能画。特效播完后计时器停掉、窗口收起，不留任何在跑的东西。每帧只用一把自己的画刷改颜色和不透明度，不经 msimeui 的画刷缓存，帧数再多也不会让缓存变大。只在 Server 的界面线程上使用。
class TypingEffectOverlay final {
public:
  TypingEffectOverlay();
  ~TypingEffectOverlay();
  TypingEffectOverlay(const TypingEffectOverlay &) = delete;
  TypingEffectOverlay &operator=(const TypingEffectOverlay &) = delete;
  // 画一次特效。返回连击数是否由浮层负责：true 时候选卡片不再在拼音行里画连击数。全屏应用在前台时什么都不画，和 macOS 一样，但仍返回 true，免得卡片在全屏应用上画出计数。
  bool present(const TypingEffectPresentation &presentation);
  // 收起：火花清掉、计时器停掉、窗口隐藏。
  void settle();
  // 失败后保持隐藏，之后的调用都不再做事，present 返回 false，候选卡片退回自己画连击数和闪光；Server 去掉这个浮层继续运行。
  bool failed() const { return failed_; }
  const std::optional<ComponentFailureSite> &failure_site() const { return failure_site_; }
  // 前台线程的系统光标，物理像素；取不到时为空。
  static std::optional<TypingRect> caret_rect(HWND foreground, const std::optional<POINT> &anchor);
  // 系统的节电模式开着没有；开着时火花和 Power Mode 退回闪光、卡片不抖，和 macOS 的低电量模式一样。
  static bool power_saver();

private:
  static LRESULT CALLBACK procedure(HWND, UINT, WPARAM, LPARAM) noexcept;
  void fail(ComponentFailureSite site);
  void tick();
  void draw(uint64_t now);
  bool ensure_surface(int width, int height);
  void discard_surface();
  float badge_text_width(const std::wstring &text);
  IDWriteTextFormat *badge_format();
  static uint64_t now_millis();

  HWND window_ = nullptr;
  bool failed_ = false;
  std::optional<ComponentFailureSite> failure_site_;
  // 位图和绑在它上面的 Direct2D 目标；尺寸只增不减，浮层变小时只用左上角的一块。
  HDC surface_dc_ = nullptr;
  HBITMAP surface_bitmap_ = nullptr;
  HGDIOBJ surface_previous_ = nullptr;
  int surface_width_ = 0;
  int surface_height_ = 0;
  // 目标当前绑定的那一块。
  int bound_width_ = 0;
  int bound_height_ = 0;
  float badge_format_size_ = 0.0f;
  Microsoft::WRL::ComPtr<ID2D1Factory> factory_;
  Microsoft::WRL::ComPtr<IDWriteFactory> write_factory_;
  Microsoft::WRL::ComPtr<IDWriteTextFormat> badge_format_;
  Microsoft::WRL::ComPtr<ID2D1DCRenderTarget> target_;
  Microsoft::WRL::ComPtr<ID2D1SolidColorBrush> brush_;
  // 当前这一帧的状态，屏幕坐标、物理像素。
  TypingRect frame_{};
  bool shown_ = false;
  float scale_ = 1.0f;
  std::vector<TypingSpark> sparks_;
  uint32_t seed_ = 0x2545F491u;
  std::optional<TypingRect> caret_flash_;
  uint64_t caret_flash_started_ = 0;
  uint32_t caret_flash_millis_ = 0;
  float caret_flash_peak_ = 0.0f;
  uint32_t flash_color_ = 0;
  std::optional<TypingRect> badge_;
  std::wstring badge_text_;
  TypingBadgeBounce bounce_ = TypingBadgeBounce::none;
  uint64_t bounce_started_ = 0;
  uint64_t settle_at_ = 0;
  bool ticking_ = false;
};
} // namespace msime::windows
