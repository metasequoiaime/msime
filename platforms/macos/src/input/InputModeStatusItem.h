#pragma once

#import <AppKit/AppKit.h>

NS_ASSUME_NONNULL_BEGIN

/// 菜单栏里常驻显示当前输入模式的状态项（共享偏好 `input_mode_status_item`，默认关闭，#7066）。系统的输入法图标只在用户把对应模式（例如「英」）也加成输入源后才跟着变，这个状态项由输入法进程自己放，不依赖那些输入源。
///
/// 状态项画当前模式的字形（与系统输入法菜单里该模式的图标同一张），外面套一圈圆角框，作为模板图随菜单栏的浅色、深色着色：字形随中英文和方案变化，圆角框让它与相邻系统输入源的光秃字形区分开（#8548、#10630）。首版只显示，不挂菜单，点击没有动作。只在主线程使用。
@interface MSIMEInputModeStatusItem : NSObject
+ (instancetype)sharedItem;
/// 显示 `modeID` 这个模式；状态项还没有建时先建出来。与正在显示的模式相同时什么也不做。
- (void)showMode:(NSString *)modeID;
/// 把状态项从菜单栏移除。没有显示时什么也不做。
- (void)hide;
/// 正在显示的模式，没有显示时为 nil。
@property(nonatomic, readonly, copy, nullable) NSString *shownMode;
@end

/// 状态项显示 `modeID` 时用的模板图，图像资源从 `bundle` 里取，`info` 是这个 bundle 的 Info.plist。单独成函数让原生测试用仓库里的资源目录核对它，不必往菜单栏放东西。
FOUNDATION_EXPORT NSImage *_Nullable MSIMEInputModeStatusItemImage(NSDictionary *info, NSString *modeID, NSBundle *bundle);

NS_ASSUME_NONNULL_END
