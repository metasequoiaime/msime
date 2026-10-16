#pragma once

#import <AppKit/AppKit.h>

NS_ASSUME_NONNULL_BEGIN

FOUNDATION_EXPORT NSString *MSIMEInputModeHUDText(BOOL englishInputMode);
FOUNDATION_EXPORT BOOL MSIMEInputModeHUDUsableCaretRect(NSRect caretRect);
FOUNDATION_EXPORT NSRect MSIMEInputModeHUDFrame(NSRect caretRect, NSSize panelSize, NSRect visibleFrame);

/// A short, non-activating badge shown after the user switches Chinese/English input.
@interface MSIMEInputModeHUDPanel : NSPanel
+ (instancetype)sharedPanel;
- (void)showEnglishInputMode:(BOOL)englishInputMode nearCaretRect:(NSRect)caretRect;
/// The badge's fill, outline and glyph colour: the floating toolbar's, so it follows the selected theme and skin. Pass dynamic colours to follow the appearance. Until set, the system window colours.
- (void)setSurfaceColor:(NSColor *)surface borderColor:(NSColor *)border textColor:(NSColor *)text;
/// 提示画成哪种明暗。它用悬浮工具栏的调色板，所以也按工具栏的规则：取 `theme`，为 `system` 或缺省时跟随系统外观。控制器传入交给工具栏的同一个字典，全局主题自带明暗时其中的 `theme` 已被钉住。工具栏和提示在 macOS 上没有单独的明暗设置。
- (void)applyThemePreferences:(NSDictionary *)preferences;
/// 紧凑提示独立使用 18pt 字号和 32pt 高度，图标遵循 `show_app_logo`；悬浮工具栏字号与缩放不影响它。
- (void)applySizingPreferences:(NSDictionary *)preferences;
@property(nonatomic, readonly) NSSize panelSize;
@property(nonatomic, readonly) NSColor *surfaceColor;
@property(nonatomic, readonly) NSColor *borderColor;
@property(nonatomic, readonly) NSColor *textColor;
@property(nonatomic, copy, readonly, nullable) NSString *displayedText;
@property(nonatomic, readonly) BOOL showsLogo;
@end

NS_ASSUME_NONNULL_END
