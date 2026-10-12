#pragma once
#import <AppKit/AppKit.h>

// 候选窗以及同样贴着光标出现的输入模式提示、双拼键位图所在的窗口层级。截图工具（如 PixPin）的框选遮罩和其他屏保层级的置顶窗口都高于 NSPopUpMenuWindowLevel，候选窗放在弹出菜单层级就被盖在下面，只剩内嵌的组字（#7326）。取光标层级减 100，和鼠须管的做法一致：高于应用能开的各种置顶层级，仍在光标之下。固定取 NSScreenSaverWindowLevel 不够，遮罩开在更高层级时照样输。候选窗的右键菜单由 AppKit 开在来源窗口层级加一，自然在候选窗之上，不要在菜单开着时调低候选窗层级，否则候选窗和菜单会一起掉到遮罩下面。
inline NSWindowLevel MSIMECandidateWindowLevel() { return CGWindowLevelForKey(kCGCursorWindowLevelKey) - 100; }
