#pragma once

#include <array>
#include <string>
#include <string_view>

#include "../candidates/ShuangpinProfileNames.h"
#include "../candidates/WubiProfileNames.h"
#include "LinuxEdition.h"

namespace msime::linux_host {

// IBus 属性菜单与 Fcitx5 状态区共用的菜单文案，照 macOS 输入法菜单（platforms/macos/src/input/InputController.mm 的 `menu` 与 `inputSchemeMenuWithCurrentTitle:`）写，两个前端不再各写一份。
//
// macOS 菜单项的快捷键画在右侧，IBusProperty 和 fcitx::Action 都没有快捷键字段，只能写进文字里，用全角括号跟在名字后面。写进去的都是两个宿主固定处理、设置里不能改的组合键：Ctrl+Shift+E、Ctrl+Shift+Space 和 Ctrl+.。
namespace menu_label {
inline constexpr const char *kChineseInput = "中文输入";
inline constexpr const char *kEnglishCandidateMode = "英文候选模式（Ctrl+Shift+E）";
inline constexpr const char *kTraditionalOutput = "繁体输出";
inline constexpr const char *kFullwidth = "全角字符（Ctrl+Shift+Space）";
inline constexpr const char *kChinesePunctuation = "中文标点（Ctrl+.）";
inline constexpr const char *kTranslations = "显示译文";
inline constexpr const char *kScheme = "输入方案";
inline constexpr const char *kTheme = "主题";
// macOS 的「悬浮工具栏」。这个开关同时决定 X11 上的悬浮工具栏和菜单里的「工具栏」子菜单（画不出悬浮窗口的会话只有后者，见 2026-10-09-linux-toolbar-settings-menu-copy.md 与 2026-10-21-linux-x11-floating-panels.md），所以仍写成「显示工具栏」。
inline constexpr const char *kToolbarEnabled = "显示工具栏";
inline constexpr const char *kEmojiPanel = "水杉表情面板…";
inline constexpr const char *kCloudClipboard = "云剪贴板…";
inline constexpr const char *kCloudDictionary = "云词库…";
inline constexpr const char *kScreenKeyboard = "水杉屏幕键盘…";
inline constexpr const char *kHandwriting = "手写输入…";
inline constexpr const char *kVoice = "开始/结束语音输入";
inline constexpr const char *kSettings = MSIME_EDITION_DISPLAY_NAME "设置…";
inline constexpr const char *kAbout = "关于" MSIME_EDITION_DISPLAY_NAME "…";
inline constexpr const char *kCheckUpdate = "检查更新…";
// 以下几项 macOS 放在悬浮工具栏的齿轮菜单里（FloatingToolbarPanel.mm），Linux 的工具栏是菜单里的「工具栏」子菜单，所以放在那里。
inline constexpr const char *kHelp = "使用帮助…";
inline constexpr const char *kFeedback = "问题反馈…";
inline constexpr const char *kPromotion = "宣传" MSIME_EDITION_DISPLAY_NAME;
inline constexpr const char *kWebsite = "访问 msime.app";
// macOS 菜单里没有的 Linux 入口。
inline constexpr const char *kLocalClipboard = "本地剪贴板…";
inline constexpr const char *kVoicePanel = "语音面板…";
inline constexpr const char *kDictionary = "词库…";
// 「中英混输」是设置页对 mixed_input.english 的叫法；与 Engine 的「英文候选模式」是两件事，名字不能撞在一起。
inline constexpr const char *kMixedEnglish = "中英混输";
}  // namespace menu_label

// 「宣传水杉输入法」上屏的固定文字，由工具栏菜单项使用。
inline constexpr const char *kPromotionText =
    "水杉输入法\n一款博采众长的输入法，目前正在广纳言路，茁壮成长中\n官网：https://msime.app/\nGit 项目：https://github.com/metasequoiaime/msime";
inline constexpr const char *kWebsiteUrl = "https://msime.app/";

// 输入方案子菜单的顺序，与 Engine 的方案编号（kInputSchemeIds）和 macOS 的子菜单相同。
inline constexpr std::array<const char *, 10> kSchemeMenuOrder = {
    "quanpin", "shuangpin", "wubi", "japanese", "korean", "cantonese", "zhuyin", "vietnamese", "tibetan", "stroke"};

// 一个方案在菜单里的名字，与 macOS 相同：双拼带键位方案（「双拼（小鹤）」），五笔带码表版本（「五笔 86」），外语方案叫「日语」「韩语」「越南语」「藏文」。不认识的键位方案只写「双拼」。
inline std::string scheme_menu_title(std::string_view scheme, std::string_view shuangpin_profile,
                                     std::string_view wubi_profile) {
  if (scheme == "shuangpin") {
    for (const auto &profile : kShuangpinProfileNames)
      if (shuangpin_profile == profile.value) return std::string("双拼（") + profile.label + "）";
    return "双拼";
  }
  if (scheme == "wubi") return wubi_scheme_label(wubi_profile);
  if (scheme == "japanese") return "日语";
  if (scheme == "korean") return "韩语";
  if (scheme == "cantonese") return "粤拼";
  if (scheme == "zhuyin") return "注音";
  if (scheme == "vietnamese") return "越南语";
  if (scheme == "tibetan") return "藏文";
  if (scheme == "stroke") return "笔画";
  return "全拼";
}

// 子菜单所在那一行的名字带上当前选中的项，与 macOS 的「输入方案（双拼（小鹤））」「主题（水杉）」相同；没有选中项时只写标题。
inline std::string menu_heading(std::string_view title, std::string_view current) {
  if (current.empty()) return std::string(title);
  return std::string(title) + "（" + std::string(current) + "）";
}

}  // namespace msime::linux_host
