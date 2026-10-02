#pragma once

#import <AppKit/AppKit.h>
#include "../candidate/CandidateSkin.h"

@class MetasequoiaFloatingToolbarPanel;

@protocol MetasequoiaFloatingToolbarDelegate <NSObject>
- (void)floatingToolbarDidRequestToggleInputMode:(MetasequoiaFloatingToolbarPanel *)toolbar;
- (void)floatingToolbarDidRequestTogglePunctuation:(MetasequoiaFloatingToolbarPanel *)toolbar;
- (void)floatingToolbarDidRequestToggleFullWidth:(MetasequoiaFloatingToolbarPanel *)toolbar;
- (void)floatingToolbarDidRequestToggleTraditionalOutput:(MetasequoiaFloatingToolbarPanel *)toolbar;
- (void)floatingToolbarDidRequestOpenCharacterPalette:(MetasequoiaFloatingToolbarPanel *)toolbar;
- (void)floatingToolbarDidRequestOpenEmoji:(MetasequoiaFloatingToolbarPanel *)toolbar;
- (void)floatingToolbarDidRequestOpenHandwriting:(MetasequoiaFloatingToolbarPanel *)toolbar;
- (void)floatingToolbarDidRequestOpenScreenKeyboard:(MetasequoiaFloatingToolbarPanel *)toolbar;
- (void)floatingToolbarDidRequestToggleVoice:(MetasequoiaFloatingToolbarPanel *)toolbar;
- (void)floatingToolbarDidRequestOpenSettings:(MetasequoiaFloatingToolbarPanel *)toolbar;
- (void)floatingToolbarDidRequestCheckForUpdates:(MetasequoiaFloatingToolbarPanel *)toolbar;
- (void)floatingToolbarDidRequestOpenWebsite:(MetasequoiaFloatingToolbarPanel *)toolbar;
- (void)floatingToolbarDidRequestHide:(MetasequoiaFloatingToolbarPanel *)toolbar;
/// 切换输入方案按钮弹出的菜单：可用的方案，正在用的那个打勾，选中即切换。
- (NSMenu *)floatingToolbarInputSchemeMenu:(MetasequoiaFloatingToolbarPanel *)toolbar;
@end

FOUNDATION_EXPORT NSRect MetasequoiaFloatingToolbarFrame(NSRect proposedFrame, NSRect visibleFrame, BOOL hasSavedFrame);
/// Windows parity policy: a configured toolbar is visible only while the IME is active and the
/// foreground display is not owned by a full-screen application.
FOUNDATION_EXPORT BOOL MetasequoiaFloatingToolbarShouldShow(BOOL configuredEnabled, BOOL imeActive, BOOL fullscreen);
/// Return whether a foreground window covers the complete display rectangle, allowing a
/// small coordinate tolerance for the borderless edge used by native full-screen windows.
FOUNDATION_EXPORT BOOL MetasequoiaWindowCoversDisplay(CGRect windowBounds, CGRect displayBounds);
/// Return whether the foreground application, other than this process, has a window covering a whole display. Walks the on-screen window list: not for every key press.
FOUNDATION_EXPORT BOOL MetasequoiaFrontmostApplicationOwnsFullscreenDisplay(void);
FOUNDATION_EXPORT NSMenu *CreateMetasequoiaFloatingToolbarUtilityMenu(id target);

@interface MetasequoiaFloatingToolbarPanel : NSPanel
@property(nonatomic, weak) id<MetasequoiaFloatingToolbarDelegate> toolbarDelegate;
+ (instancetype)sharedPanel;
- (void)updateEnglishInputMode:(BOOL)englishInputMode
          chinesePunctuationEnabled:(BOOL)chinesePunctuationEnabled
                   fullWidthEnabled:(BOOL)fullWidthEnabled
    traditionalChineseOutputEnabled:(BOOL)traditionalChineseOutputEnabled;
/// The mode button reads A under Caps Lock, then 英 for English, En for English candidates, and otherwise the scheme's badge - 中 for quanpin, 双 for shuangpin, 五 for wubi, 日 for japanese and 한 for korean, the badges the input menu shows. One glyph cannot tell the Shuangpin keymaps apart, so `schemeTitle` (小鹤双拼, 五笔 86, ...) leads the button's tooltip and accessibility label; nil leaves just the action.
- (void)updateEnglishInputMode:(BOOL)englishInputMode
         englishCandidateMode:(BOOL)englishCandidateMode
                        scheme:(NSString *)scheme
                   schemeTitle:(NSString *)schemeTitle
                      capsLock:(BOOL)capsLock
          chinesePunctuationEnabled:(BOOL)chinesePunctuationEnabled
                   fullWidthEnabled:(BOOL)fullWidthEnabled
    traditionalChineseOutputEnabled:(BOOL)traditionalChineseOutputEnabled;
- (void)updateEnglishInputMode:(BOOL)englishInputMode
             japaneseInputMode:(BOOL)japaneseInputMode
                      capsLock:(BOOL)capsLock
          chinesePunctuationEnabled:(BOOL)chinesePunctuationEnabled
                   fullWidthEnabled:(BOOL)fullWidthEnabled
    traditionalChineseOutputEnabled:(BOOL)traditionalChineseOutputEnabled;
- (void)activateForDelegate:(id<MetasequoiaFloatingToolbarDelegate>)delegate visible:(BOOL)visible;
/// Force the toolbar visible for a real input event, repairing stale ownership/focus state.
- (void)wakeForInputDelegate:(id<MetasequoiaFloatingToolbarDelegate>)delegate;
/// Reset the 10-second idle timeout and restore the toolbar for its current owner.
- (void)noteInputForDelegate:(id<MetasequoiaFloatingToolbarDelegate>)delegate;
- (void)setVisible:(BOOL)visible forDelegate:(id<MetasequoiaFloatingToolbarDelegate>)delegate;
- (void)deactivateForDelegate:(id<MetasequoiaFloatingToolbarDelegate>)delegate;
/// Counterpart of the reference's WM_IMEDEACTIVATE: the user selected another input source, so the toolbar hides and releases whichever controller owns it. A client focus-out does not call this; the toolbar stays resident with its owner until the next activation hands it on.
- (void)deactivateForInputSourceSwitch;
/// Apply validated shared preferences without persisting platform-local defaults.
- (void)applyThemePreferences:(NSDictionary *)preferences;
- (void)applySizingPreferences:(NSDictionary *)preferences;
/// Use the active host's resolved palette without reading another preference store.
- (void)applyLightSkin:(const msime::mac::SkinTokens &)light darkSkin:(const msime::mac::SkinTokens &)dark;
/// Use the toolbar's own palette, independent of candidate color overrides.
- (void)applyLightToolbarSkin:(const msime::mac::SkinTokens &)light darkSkin:(const msime::mac::SkinTokens &)dark;
@end
#define MSIMEFloatingToolbarDelegate MetasequoiaFloatingToolbarDelegate
#define MSIMEFloatingToolbarPanel MetasequoiaFloatingToolbarPanel
#define MSIMEFloatingToolbarFrame MetasequoiaFloatingToolbarFrame
#define CreateMSIMEFloatingToolbarUtilityMenu CreateMetasequoiaFloatingToolbarUtilityMenu
