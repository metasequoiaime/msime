#pragma once

#include "../../common/InputSchemeTraits.h"

// 任务栏输入指示器（语言栏模式按钮）画哪一个图标。与 macOS 输入菜单里每个方案一个大字的图标对应（platforms/macos/src/input/InputModeIdentifiers.h）：中、双、五、粤、注、日、한、越、藏、笔，英文是 英，Caps Lock 开着时是 A。纯计算，不依赖 Windows 头文件，主机上的单测直接包含；资源编号的对照在 LanguageBar.cpp。
namespace msime::tsf
{
enum class ModeIcon
{
    Chinese,
    English,
    CapsLock,
    Shuangpin,
    Wubi,
    Japanese,
    Korean,
    Cantonese,
    Zhuyin,
    Vietnamese,
    Tibetan,
    Stroke,
};

// `open` 是键盘开关 compartment（中文为开），`disabled` 是按钮被禁用（没有焦点上下文），`capsLock` 是 Caps Lock，`mode` 是 Server 上次在 InputModeChanged 里给的模式。Caps Lock 只要按钮可用就排在最前，与以前的 cap 图标一样：它改变每个字母键的作用，不论哪个模式。藏文显示 藏 而不是 macOS 的 ཀ，和 Windows 托盘、悬浮工具栏的语言按钮一致。
constexpr ModeIcon mode_icon(bool open, bool disabled, bool capsLock, msime::windows::scheme::InputMode mode)
{
    using msime::windows::scheme::InputMode;
    if (disabled)
        return ModeIcon::English;
    if (capsLock)
        return ModeIcon::CapsLock;
    if (!open)
        return ModeIcon::English;
    switch (mode)
    {
    case InputMode::Shuangpin:
        return ModeIcon::Shuangpin;
    case InputMode::Wubi:
        return ModeIcon::Wubi;
    case InputMode::Japanese:
        return ModeIcon::Japanese;
    case InputMode::Korean:
        return ModeIcon::Korean;
    case InputMode::Cantonese:
        return ModeIcon::Cantonese;
    case InputMode::Zhuyin:
        return ModeIcon::Zhuyin;
    case InputMode::Vietnamese:
        return ModeIcon::Vietnamese;
    case InputMode::Tibetan:
        return ModeIcon::Tibetan;
    case InputMode::Stroke:
        return ModeIcon::Stroke;
    case InputMode::Chinese:
        break;
    }
    return ModeIcon::Chinese;
}
} // namespace msime::tsf
