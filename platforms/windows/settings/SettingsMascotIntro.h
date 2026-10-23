#pragma once
#include <array>
#include <optional>
#include <string_view>

// 页首的水杉娘引导，照共享设置 UI 的 `settings-mascot-intro.tsx`（macOS 桌面设置的 `SettingsMascotIntro`）：左边一句 13 DIP 的说明，右边一张场景插画。文案逐字取自那里的 `introductions` 和 `tabIntroductions`，页面 id 换成本窗口的（`SettingsNavigation.h`）。不依赖 Windows 头文件，主机上的单元测试能直接查。
namespace msime::settings {

// 插画场景，与共享 UI `core/mascot.tsx` 的 `MascotScene` 一一对应。插画是 `packages/ui/src/assets/mascot/*.webp` 转成的 PNG，作为 RCDATA 打进 Settings.rc，资源号是 `mascot_resource_base` 加上这里的序号，所以顺序要和 Settings.rc 一致（`scripts/test-windows-settings-art.py` 核对）。
enum class MascotScene {
  settings,
  welcome,
  input,
  notes,
  privacy,
  sync,
  updated,
  empty,
  offline,
  downloading,
};

inline constexpr int mascot_resource_base = 110;

constexpr int mascot_resource(MascotScene scene) {
  return mascot_resource_base + static_cast<int>(scene);
}

struct MascotIntro {
  MascotScene scene;
  std::wstring_view description;
};

struct PageMascotIntro {
  std::string_view page;
  // 页内标签的 id（与共享 UI 的标签 id 相同）；没有标签的页面为空。
  std::string_view tab;
  MascotIntro intro;
};

// 有标签的页面每个标签一条，第一条是打开页面时的标签。「关于」没有引导：共享 UI 里关于页顶上已经是品牌卡。
inline constexpr std::array<PageMascotIntro, 18> mascot_intros{{
    {"typing", "", {MascotScene::input, L"选择输入方案，调整选词与输出习惯。"}},
    {"lexicon", "", {MascotScene::downloading, L"管理词库、词条与常用表达。"}},
    {"shortcuts", "", {MascotScene::input, L"查看按键速查，设置常用快捷键。"}},
    {"expression", "", {MascotScene::input, L"设置标点、候选释义与翻译服务。"}},
    {"voice", "input", {MascotScene::notes, L"配置语音识别、服务与输入快捷键。"}},
    {"voice", "recording", {MascotScene::settings, L"调整录音提示音与录音期间的静音行为。"}},
    {"osk", "keyboard", {MascotScene::input, L"调整屏幕键盘布局与外观。"}},
    {"osk", "handwriting", {MascotScene::notes, L"设置手写识别与输入方式。"}},
    {"themes", "", {MascotScene::welcome, L"选择皮肤与配色；候选窗口的布局、字体和显示方式在「候选窗口」中设置。"}},
    {"candidate", "", {MascotScene::welcome, L"调整候选窗口的布局、字体和显示方式；皮肤与配色在「主题」中设置。"}},
    {"toolbar", "", {MascotScene::settings, L"设置工具栏的显示与常用功能。"}},
    {"clip", "clipboard", {MascotScene::sync, L"设置剪贴板历史的保存与云端同步。"}},
    {"clip", "history", {MascotScene::notes, L"查看、复制和管理已保存的剪贴板记录。"}},
    {"ai", "", {MascotScene::notes, L"配置候选辅助与文本处理服务。"}},
    {"plugins", "", {MascotScene::settings, L"管理插件与功能扩展。"}},
    {"stats", "", {MascotScene::notes, L"查看输入统计与使用习惯。"}},
    {"account", "", {MascotScene::sync, L"管理账号与跨设备设置同步。"}},
    {"dev", "", {MascotScene::settings, L"检查输入法状态与维护工具。"}},
}};

// 页面在当前标签下的页首引导。`tab` 为空或这一页没有这个标签时，取这一页的第一条，也就是打开页面时的标签。设置窗口把这一页在 `page_tabs_` 里记下的页内标签传进来（`main.cpp` 的 `page_tab_bar`）；「语音输入」的「服务配置」子视图在那里记作 `service`，这里没有它，取第一条，与共享页打开子视图时仍显示「识别与输入」那句相同。「关于」和不认识的页面没有引导。
inline std::optional<MascotIntro> mascot_intro(std::string_view page, std::string_view tab = {}) {
  std::optional<MascotIntro> first;
  for (const auto &entry : mascot_intros) {
    if (entry.page != page)
      continue;
    if (!tab.empty() && entry.tab == tab)
      return entry.intro;
    if (!first)
      first = entry.intro;
  }
  return first;
}

} // namespace msime::settings
