#pragma once

#import <AppKit/AppKit.h>

NS_ASSUME_NONNULL_BEGIN

/// 菜单栏里常驻显示当前输入模式的状态项（共享偏好 `input_mode_status_item`，默认关闭，#7066）。系统的输入法图标只在用户把对应模式（例如「英」）也加成输入源后才跟着变，这个状态项由输入法进程自己放，不依赖那些输入源。
///
/// 图标与系统输入法菜单里该模式的图标是同一张 TIFF（Info.plist 的 `tsInputModeMenuIconFileKey`，见 MSIMEInputModeMenuIconFileIn），作为模板图随菜单栏的浅色、深色着色。首版只显示，不挂菜单，点击没有动作。只在主线程使用。
@interface MSIMEInputModeStatusItem : NSObject
+ (instancetype)sharedItem;
/// 显示 `modeID` 这个模式；状态项还没有建时先建出来。与正在显示的模式相同时什么也不做。
- (void)showMode:(NSString *)modeID;
/// 把状态项从菜单栏移除。没有显示时什么也不做。
- (void)hide;
/// 正在显示的模式，没有显示时为 nil。
@property(nonatomic, readonly, copy, nullable) NSString *shownMode;
@end

NS_ASSUME_NONNULL_END
