#pragma once

namespace Global
{
// Alt+Shift+H 切换全角/半角，对应 macOS 的 Option+Shift+H，由共享偏好 `keybindings.toggle_fullwidth_option_shift_h` 控制（默认开，关掉后交给应用）。和 macOS 一样只在中文模式下认：英文模式里这个组合键原样交给应用。带 Ctrl（AltGr 也报成 Ctrl+Alt）或 Win 时不是它。按虚拟键 'H' 认，虚拟键本来就随键盘布局变，和 macOS 按布局字母认是同一回事。Ctrl+Shift+Space 不受这个开关影响。
inline bool IsFullwidthAltShiftH(unsigned code, bool shift, bool ctrl, bool alt, bool win, bool chinese, bool enabled)
{
    return enabled && chinese && code == 'H' && shift && alt && !ctrl && !win;
}
} // namespace Global
