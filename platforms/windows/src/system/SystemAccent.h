#pragma once
#include <cstdint>
#include <optional>
// windows.h 先于 COM 头文件。
#include <windows.h>
#include <inspectable.h>
#include <wrl/client.h>

namespace msime::windows {
// 系统强调色（设置 › 个性化 › 颜色里选的强调色），对应 macOS 的 NSColor.controlAccentColor。打字特效的火花、闪光和连击徽标在特效包没给颜色时用它，与 macOS 一样，而不是主题的强调色。
//
// 取自 Windows.UI.ViewManagement.UISettings 的 GetColorValue(UIColorType::Accent)。对象建一次留着，之后每次取到的都是用户当前的选择，改了强调色不用重启。只在创建它的线程上用，那个线程要已经初始化 COM（候选窗的界面线程是单线程套间）。
class SystemAccent {
public:
  // 当前的强调色，0xRRGGBB；UISettings 建不起来（取不到 WinRT 运行时函数、激活失败）时为空，调用方退回主题的强调色。
  std::optional<uint32_t> rgb();

private:
  Microsoft::WRL::ComPtr<IInspectable> settings_;
  // 建过一次失败了，不再每次按键都重试。
  bool unavailable_ = false;
};
} // namespace msime::windows
