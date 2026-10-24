#pragma once

#include <windows.h>
#include <array>

namespace msime::settings::win7::chrome {

// 页面索引仍与深链接契约一致；导航顺序只决定呈现，不改变设置或出厂值。
struct Page { const wchar_t* title; const wchar_t* description; };
inline constexpr std::array<Page, 23> pages{{
    {L"快捷键", L"让常用操作更顺手，按你的习惯设置切换方式。"},
    {L"悬浮工具栏", L"选择桌面上常用的输入工具与显示方式。"},
    {L"维护与诊断", L"管理本机设置、服务和诊断记录。"},
    {L"输入", L"从输入方案到选词习惯，调整每一次输入。"},
    {L"标点与翻译", L"设置标点样式、候选释义和翻译服务。"},
    {L"候选窗口", L"调整候选的排列、字体和显示大小。"},
    {L"AI 输入辅助", L"连接你选择的服务，设置输入辅助。"},
    {L"账号与同步", L"管理水杉账号、登录设备和同步数据。"},
    {L"主题与皮肤", L"选择喜欢的配色，让输入窗口更合心意。"},
    {L"社区皮肤", L"发现、安装和管理社区分享的皮肤。"},
    {L"打字统计", L"查看输入习惯和速度，管理统计与保留时间。"},
    {L"剪贴板", L"查找、固定和管理本机剪贴板历史。"},
    {L"云剪贴板", L"管理跨设备的剪贴板内容和连接方式。"},
    {L"词库", L"查找和编辑词条，管理导入与更新。"},
    {L"背单词", L"管理生词和复习计划。"},
    {L"插件", L"管理本机插件、脚本和输入扩展。"},
    {L"关于与帮助", L"查看版本、更新与使用帮助。"},
    {L"社区插件", L"发现和管理社区分享的输入插件。"},
    {L"社区词库与回复模板", L"发现词库与回复模板，管理收藏和安装。"},
    {L"语音输入", L"选择识别方式，设置麦克风、模型和润色。"},
    {L"屏幕键盘与手写", L"设置屏幕键盘和手写输入工具。"},
    {L"连接 AI 助手", L"管理 AI 助手的本机连接与授权。"},
    {L"键盘皮肤制作", L"设计键盘外观，预览并保存自己的皮肤。"},
}};
inline constexpr COLORREF background = RGB(255, 255, 255);
inline constexpr COLORREF sidebar = RGB(246, 248, 247);
inline constexpr COLORREF ink = RGB(31, 44, 39);
inline constexpr COLORREF muted = RGB(106, 117, 111);
inline constexpr COLORREF accent = RGB(30, 119, 81);
inline constexpr COLORREF selection = RGB(225, 239, 231);
inline constexpr COLORREF line = RGB(230, 235, 232);

inline void fill(HDC dc, RECT rect, COLORREF color) {
  const auto brush = CreateSolidBrush(color);
  FillRect(dc, &rect, brush);
  DeleteObject(brush);
}

} // namespace msime::settings::win7::chrome
