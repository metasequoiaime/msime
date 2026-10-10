#pragma once
#include "ToolbarIcons.h"
#include <optional>
#include <string>
#include <string_view>

namespace msime::windows {
// 语言按钮提示里的方案名，与 macOS 工具栏的 schemeTitle 用同一套名字：双拼写出键位（小鹤双拼等），五笔写出码表版本。不认识的键位按 macOS 的规范化当作小鹤；用户自己定义的键位（`custom`）是真实存在的一种键位，写「自定义双拼」：把它说成小鹤双拼，提示和读屏就报错了用户正在用的方案。不认识的方案返回空串，提示只说动作。
inline std::wstring toolbar_scheme_title(std::string_view scheme,
                                         std::string_view shuangpin_profile,
                                         std::string_view wubi_profile) {
  if (scheme == "quanpin")
    return L"全拼";
  if (scheme == "shuangpin") {
    if (shuangpin_profile == "ziranma")
      return L"自然码双拼";
    if (shuangpin_profile == "shoudao")
      return L"首道双拼";
    if (shuangpin_profile == "microsoft")
      return L"微软双拼";
    if (shuangpin_profile == "custom")
      return L"自定义双拼";
    return L"小鹤双拼";
  }
  if (scheme == "wubi")
    return wubi_profile == "wubi98" ? L"五笔 98" : L"五笔 86";
  if (scheme == "japanese")
    return L"日语";
  if (scheme == "korean")
    return L"韩语";
  if (scheme == "cantonese")
    return L"粤拼";
  if (scheme == "zhuyin")
    return L"注音";
  if (scheme == "vietnamese")
    return L"越南语";
  if (scheme == "tibetan")
    return L"藏文";
  if (scheme == "stroke")
    return L"笔画";
  return {};
}

// 工具栏按钮的悬停提示，文字与 macOS 工具栏的 toolTip / accessibilityLabel 一致：两态按钮说点了会切到哪一边，其余按钮说点了打开什么。`state` 与 toolbar_icon 的含义相同，还不知道时只给按钮的名字，不猜一个方向。`panels` 是共享应用在不在：不在时表情和屏幕键盘打开的是系统自带的面板，提示照实说。`product` 是这个版本的产品名。
inline std::wstring toolbar_tooltip(int button, std::optional<bool> state,
                                    const ToolbarLanguageState &language,
                                    std::wstring_view scheme_title, bool panels,
                                    std::wstring_view product) {
  switch (button) {
  case kToolbarLanguage: {
    if (!state && !language.dedicated_english)
      return std::wstring(scheme_title.empty() ? L"中英文切换" : scheme_title);
    const bool english = (state && !*state) || language.dedicated_english;
    const std::wstring action = english ? L"切换到中文输入" : L"切换到英文输入";
    if (scheme_title.empty())
      return action;
    return std::wstring(scheme_title) + L" · " + action;
  }
  case kToolbarInputScheme:
    return L"切换输入方案";
  case kToolbarPunctuation:
    if (!state)
      return L"中英文标点";
    return *state ? L"切换到西文标点" : L"切换到中文标点";
  case kToolbarFullwidth:
    if (!state)
      return L"全角 / 半角";
    return *state ? L"切换到半角输入" : L"切换到全角输入";
  case kToolbarCharacterSet:
    if (!state)
      return L"简繁切换";
    return *state ? L"切换到简体输出" : L"切换到繁体输出";
  case kToolbarEmoji:
    return panels ? L"打开水杉表情面板" : L"打开系统表情面板";
  case kToolbarHandwriting:
    return L"打开水杉手写识别板";
  case kToolbarScreenKeyboard:
    return panels ? L"打开水杉屏幕键盘" : L"打开系统屏幕键盘";
  case kToolbarVoice:
    return L"开始或结束语音输入";
  case kToolbarSettings:
    return L"打开" + std::wstring(product) + L"设置";
  case kToolbarHide:
    return L"隐藏悬浮状态栏";
  default:
    return {};
  }
}
} // namespace msime::windows
