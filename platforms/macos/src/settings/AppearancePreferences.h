#pragma once
#import <AppKit/AppKit.h>
#import "../../../../shared/apple/TextClient.h"
#include "../candidate/CandidateSkin.h"

FOUNDATION_EXPORT NSNotificationName const MSIMEAppearanceDidChangeNotification;
FOUNDATION_EXPORT NSNotificationName const MSIMETranslationPreferencesDidSaveNotification;
/// Set to @YES in the userInfo of an MSIMEAppearanceDidChangeNotification that only moved the Chinese/English mode. The mode is not part of the shared preferences document, so observers refresh what they show but have nothing to save.
FOUNDATION_EXPORT NSString *const MSIMEAppearanceInputModeOnlyKey;

/// 某个输入源是否已在用户的输入法列表里。输入法进程启动时把它设成 `MSIMEInputSourceIsEnabled`；测试和其它链接了设置窗口的程序不设，「菜单栏入口」提示就不出现，设置窗口也因此不必链接 Carbon。
extern BOOL (*MSIMEInputModeEnabledProbe)(NSString *identifier);

// macOS-only presentation settings; never change Engine composition/configuration.
@interface MSIMEAppearancePreferences : NSWindowController
+ (instancetype)sharedPreferences;
- (instancetype)initWithDefaults:(NSUserDefaults *)defaults;
- (instancetype)initWithDefaults:(NSUserDefaults *)defaults skinsRoot:(NSURL *)root;
- (void)reloadSkins;
- (BOOL)applyCloudSettingsSnapshot:(NSDictionary *)values;
- (NSDictionary *)cloudSettingsSnapshot;
/// The skin browser, which is the 皮肤 page itself rather than a window of its own.
- (NSView *)skinSettingsView;
/// Opens the window on a named page. The identifiers are the tails of the shared settings: routes — input, habits, shortcuts, voice, appearance, skin, floating, dictionary, account, help, about — so an entry point that deep-links into the desktop application can hand its native fallback the same name. Returns NO, and leaves the page alone, for a name this version does not have, which is what the retired helpcode, feedback and utilities names now get; callers that do not care which page they land on should not call this at all, so that the window opens on the page the user left it on.
- (BOOL)showSettingsPageWithIdentifier:(NSString *)identifier;
- (void)setTranslationPreferencesDirectory:(NSString *)directory;
/// Applies only settings owned by this window to an existing shared Preferences object.
- (NSDictionary<NSString *, id> *)sharedPreferencesByMerging:(NSDictionary<NSString *, id> *)snapshot;
- (msime::mac::ResolvedSkin)resolvedSkinForDark:(BOOL)dark;
/// The floating toolbar's palette for one mode: the resolved candidate palette (surface, text, hover, border, selected), with the applied package's toolbar stylesheet over it.
- (msime::mac::SkinTokens)toolbarSkinForDark:(BOOL)dark;
/// 这种明暗画的那个皮肤包的装饰图，主题解析时读好；这种明暗没有画包或包没有装饰时为 nil。浅色、深色槽位可以是两个包，所以按明暗各取各的。
- (NSImage *)decorationImageForDark:(BOOL)dark;
/// 这种明暗画的那个皮肤包的背景图，与装饰图一起读好；候选窗只在这种明暗的解析结果带 backgroundPath 时画它。
- (NSImage *)backgroundImageForDark:(BOOL)dark;
/// 主题固定的明暗：浅色、深色两次解析给出同一个固定明暗时才有值（见 msime::mac::FixedThemeMode），候选窗、悬浮工具栏和菜单据此钉住明暗。
- (std::optional<bool>)fixedThemeMode;
@property(nonatomic, readonly) NSURL *skinsRoot;
@property(nonatomic) BOOL vertical;
@property(nonatomic) BOOL candidateFollowCursor;
/// Show the short non-activating Chinese/English mode badge near the caret.
@property(nonatomic) BOOL inputModeHUD;
/// 候选窗和悬浮工具栏左端是否画水杉 logo（共享偏好 `show_app_logo`）。没有共享值也没有本机值时按新装处理，不画。
@property(nonatomic) BOOL showsAppLogo;
@property(nonatomic, copy) NSString *inputScheme;
/// The Chinese scheme to go back to when leaving japanese or korean: the current scheme while it is Chinese, otherwise the one left for japanese or korean here or the shared `last_chinese_scheme`, 全拼 when neither is known.
@property(nonatomic, readonly) NSString *lastChineseScheme;
/// The scheme the input method last showed a system input mode for, kept in this Mac's defaults and never in the shared document. The input controller compares the scheme running now with it, so a scheme picked while the input method was not running, or one whose dictionary was installed after it was picked, still reads as a change on the next sync and gets its opt-in mode enabled; the scheme it already names enables nothing.
@property(nonatomic, copy) NSString *lastSyncedInputScheme;
@property(nonatomic, copy) NSString *shuangpinProfile;
@property(nonatomic) BOOL shuangpinPreeditUsesRaw;
/// 五笔码表版本：`wubi86`（缺省）或 `wubi98`，对应共享偏好的 `wubi_profile`。
@property(nonatomic, copy) NSString *wubiProfile;
/// Allow Pinyin fallback when a Wubi code has no Wubi candidates.
@property(nonatomic) BOOL wubiMixedPinyinEnabled;
/// Shared inline composition display: raw keys, formatted pinyin, or hidden.
@property(nonatomic, readonly) MSIMEInlinePreeditStyle inlinePreeditStyle;
@property(nonatomic) NSUInteger fontSize;
@property(nonatomic, copy) NSString *fontFamily;
/// Optional leading face for Latin glyphs in the candidate cascade.
@property(nonatomic, copy) NSString *candidateEnglishFont;
@property(nonatomic, copy) NSArray<NSString *> *fallbackFonts;
- (NSFont *)candidateFontOfSize:(CGFloat)size;
- (NSFont *)candidateFontOfSize:(CGFloat)size englishFirst:(BOOL)englishFirst;
@property(nonatomic) NSUInteger preeditFontSize;
@property(nonatomic) BOOL showsCandidatePreedit;
/// The candidate window's own style: `candidate_scale_percent` (50-200, 100 by default), which multiplies the candidate fonts and every length of the window; `candidate_opacity_percent` (50-100, 100 by default), which fades only the card surface, its border and a package background; and `candidate_corner_radius` (0-32 points), nil to keep the theme's or the package's card radius. A value outside its range is refused and leaves the setting as it was.
@property(nonatomic) NSInteger candidateScalePercent;
@property(nonatomic) NSInteger candidateOpacityPercent;
@property(nonatomic, copy) NSNumber *candidateCornerRadius;
- (msime::mac::CandidateWindowStyle)candidateWindowStyle;
/// -resolvedSkinForDark: with -candidateWindowStyle laid over it, which is what the candidate window draws. The floating toolbar and the colour wells read the skin as resolved.
- (msime::mac::ResolvedSkin)candidateWindowSkinForDark:(BOOL)dark;
/// The 候选字体 preset: 0 默认, 1 宋体, 2 黑体, 3 楷体, 4 圆体, read back from `fontFamily`, or -1 for a family none of them writes. Setting one writes the preset's family for this host into `fontFamily` and puts all of its families, the other platforms' names included, at the front of `fallbackFonts`; 默认 restores the shared default pair.
@property(nonatomic) NSInteger candidateFontPreset;
@property(nonatomic, copy) NSString *candidateTextColor;
/// The six other candidate pickers of `custom_theme.candidate_colors`, as the same 「#rrggbb」 strings, or nil while the theme's own colour is in use. Setting one selects the custom theme (a theme that was on screen becomes its base); setting nil clears only that slot. The candidate window draws with -resolvedSkinForDark:, which already has them applied.
@property(nonatomic, copy) NSString *candidateNumberColor;
@property(nonatomic, copy) NSString *candidateAccentColor;
@property(nonatomic, copy) NSString *candidateSelectedColor;
@property(nonatomic, copy) NSString *candidateHoverColor;
@property(nonatomic, copy) NSString *candidateSurfaceColor;
@property(nonatomic, copy) NSString *candidateBorderColor;
/// The light/dark choice: system, dark or light for the whole client, and follow, dark or light for
/// the candidate window and the floating toolbar, each of which may override the global one.
@property(nonatomic, copy) NSString *themeMode;
@property(nonatomic, copy) NSString *candidateTheme;
@property(nonatomic, copy) NSString *toolbarTheme;
/// The global theme: system, shuishan, light, paper, night, ink or custom (the ids of msime_client_theme_catalog). An id outside the catalog is ignored when set and reads as system.
@property(nonatomic, copy) NSString *globalTheme;
/// `custom_theme.base`: system or a built-in theme id, the palette the custom theme starts from.
@property(nonatomic, readonly, copy) NSString *customThemeBase;
/// 浅色槽位 `custom_theme.candidate_skin`：浅色模式画的外部皮肤包，深色槽位空着时深色模式也取它；nil 表示没有。
@property(nonatomic, readonly, copy) NSString *customCandidateSkin;
/// 深色槽位 `custom_theme.candidate_skin_dark`：深色模式画的外部皮肤包，nil 表示没有。
@property(nonatomic, readonly, copy) NSString *customCandidateSkinDark;
/// 应用外部皮肤包：选中自定义主题，按清单 base 的明暗把包放进它的槽位（msime::mac::ApplyCandidateSkin，与设置页的 `applyCandidateSkin` 同一规则），base 写成包的 base。
- (void)selectExternalSkin:(NSString *)skinId base:(NSString *)base;
/// 取消使用一款外部皮肤：只清放着它的槽位，另一个槽位和自定义主题的其他部分不动。
- (void)removeCustomCandidateSkin:(NSString *)skinId;
/// 自定义主题不使用外部皮肤：两个槽位一起清，自定义主题的其他部分不动。
- (void)clearCustomCandidateSkin;
/// 这里存的自定义主题（底色、两个皮肤槽位和七个取色器），即宿主解析器要的形状。
- (msime::mac::CustomTheme)customTheme;
@property(nonatomic) NSUInteger pageSize;
// Native routing preferences; English passes keys through without preparing Engine.
@property(nonatomic) BOOL englishMode;
@property(nonatomic, copy) NSString *defaultImeMode;
@property(nonatomic, copy) NSString *imeModeScope;
- (void)activateInputModeForApplication:(NSString *)identifier;
- (void)lockActiveInputMode;
- (void)resetRememberedInputModes;
@property(nonatomic) BOOL inputModeShortcut;
@property(nonatomic) BOOL shiftTapShortcut;
@property(nonatomic) BOOL controlTapShortcut;
@property(nonatomic) BOOL controlOptionSpaceShortcut;
@property(nonatomic) BOOL characterSetShortcut;
@property(nonatomic) BOOL fullWidthShortcut;
@property(nonatomic) BOOL traditionalOutput;
@property(nonatomic) BOOL fullWidthInput;
@property(nonatomic) BOOL chinesePunctuation;
/// The punctuation and width the active application is typing with. They start from the saved `chinesePunctuation` / `fullWidthInput` and the toggles (toolbar, Ctrl+., Ctrl+Shift+Space, Option+Shift+H) change only the active app, in memory: setting them never writes defaults or posts MSIMEAppearanceDidChangeNotification.
@property(nonatomic) BOOL runtimeChinesePunctuation;
@property(nonatomic) BOOL runtimeFullWidthInput;
/// Drops the active app's punctuation toggle so it follows the saved value again, as a Chinese/English switch does in the reference.
- (void)resetRuntimePunctuationForActiveApplication;
/// Drops both toggles of the active app, the counterpart of the reference resetting its compartments on Deactivate/Activate.
- (void)resetRuntimeInputStateForActiveApplication;
- (void)resetAllRuntimeInputState;
@property(nonatomic) BOOL smartPunctuation;
@property(nonatomic) BOOL smartPunctuationRepeatToChinese;
/// A space after a just-committed Chinese mark rewrites it as ASCII. Off by default, like the rest of the
/// family on the Windows baseline: it changes a character the user already saw land.
@property(nonatomic) BOOL smartPunctuationSpaceConvert;
@property(nonatomic) BOOL pairedPunctuation;
@property(nonatomic, copy) NSString *punctuationLock;
@property(nonatomic) BOOL mixedEnglishInput;
@property(nonatomic) NSInteger mixedEnglishMinimumPrefix;
@property(nonatomic) BOOL mixedEmojiInput;
@property(nonatomic) BOOL mixedKaomojiInput;
@property(nonatomic) BOOL candidateLearningEnabled;
@property(nonatomic, copy) NSString *frequencyAdjustmentMode;
@property(nonatomic) NSInteger frequencyTriggerCount;
@property(nonatomic) NSInteger frequencyLinearStep;
@property(nonatomic) BOOL fuzzyPinyinEnabled;
- (BOOL)fuzzyPinyinRuleEnabled:(NSString *)rule;
- (void)setFuzzyPinyinRule:(NSString *)rule enabled:(BOOL)enabled;
@property(nonatomic) BOOL cloudCandidates;
/// Records, once per profile, whether the first-use cloud consent still has to be asked. An existing NSUserDefaults choice, an existing `<directory>/preferences.json`, or a non-empty Engine `userDataDirectory` counts as an upgrade and is never asked about; otherwise the consent becomes pending. Nothing is recorded without a preferences directory. Later calls do nothing.
- (void)resolveCloudCandidatesConsentWithPreferencesDirectory:(NSString *)directory userDataDirectory:(NSString *)userDataDirectory;
/// NO only while the first-use consent is pending. A profile that was never resolved counts as answered.
@property(nonatomic, readonly) BOOL cloudCandidatesAnswered;
/// The gate for sending anything to the cloud candidate service: answered and enabled.
@property(nonatomic, readonly) BOOL cloudCandidatesEnabled;
/// Store the user's answer to the consent prompt (or the settings checkbox) and mark the consent answered.
- (void)answerCloudCandidates:(BOOL)enabled;
@property(nonatomic) BOOL candidateTranslations;
/// Offline Engine glossary lookup; independent from online candidate translation providers.
@property(nonatomic) BOOL candidateEnglishGloss;
@property(nonatomic) BOOL autocorrectTransposition;
@property(nonatomic) BOOL autocorrectNeighbor;
@property(nonatomic) BOOL quanpinHelpcodeEnabled;
@property(nonatomic) BOOL shuangpinHelpcodeEnabled;
- (void)applySharedAssistancePreferences:(NSDictionary *)preferences;
- (NSDictionary *)helpcodeOptionsForScheme:(NSString *)scheme;
/// 共享偏好里这个方案（quanpin 或 shuangpin）选中的辅助码表插件 id（`plugins.helpcode_pack_<方案>`），没选时为 nil。选了插件时 Engine 用插件的码表替代辅助码方案，所以方案下拉框显示的是插件；在这里选一个方案就不再使用该插件，与共享设置页的「辅助码方案」一致。
- (NSString *)helpcodePackForScheme:(NSString *)scheme;
@property(nonatomic) BOOL shuangpinKeymap;
/// 四码唯一候选自动上屏：值随共享偏好 `wubi_auto_commit_unique` 进 Engine，本键只是本机的存储位置；从没设置过时是开。
@property(nonatomic) BOOL wubiAutoCommitUnique;
/// Dictation, which is read straight out of NSUserDefaults by the input method and by the voice module rather than through the shared document. The recogniser and the text it produces are the voice form's; these are the preferences around it — whether dictation runs at all, which language it transcribes, whether it plays a cue, whether it mutes what else is playing, and whether the partial transcript appears inline while it listens.
@property(nonatomic) BOOL voiceInputEnabled;
/// The BCP 47 tag the recogniser is asked for, as one of the two the client offers: zh-CN or en-US.
@property(nonatomic, copy) NSString *voiceLanguage;
@property(nonatomic) BOOL voiceSoundEnabled;
@property(nonatomic) BOOL voiceMuteSystemAudio;
@property(nonatomic) BOOL voiceStreamInlinePreedit;
/// The four ways of starting dictation and the one that locks a held shortcut down. Control + F9 is a press, the other three are holds; 按住时按空格锁定录音 applies to whichever hold is in use.
@property(nonatomic) BOOL voiceHotkeyCtrlF9;
@property(nonatomic) BOOL voiceHotkeyRightAlt;
@property(nonatomic) BOOL voiceHotkeyCtrlCommand;
@property(nonatomic) BOOL voiceHotkeyCtrlOption;
@property(nonatomic) BOOL voiceHotkeyHoldSpace;
@property(nonatomic) BOOL floatingToolbarEnabled;
/// The 中/英 button. It is a component like the eight below it; it had no accessor at all, so the
/// only value the merge could publish for it was a constant.
@property(nonatomic) BOOL floatingToolbarEnglishMode;
@property(nonatomic) BOOL floatingToolbarPunctuation;
@property(nonatomic) BOOL floatingToolbarFullWidth;
@property(nonatomic) BOOL floatingToolbarCharacterSet;
@property(nonatomic) BOOL floatingToolbarEmoji;
/// The handwriting panel and voice buttons, which only this client's toolbar has.
@property(nonatomic) BOOL floatingToolbarHandwriting;
@property(nonatomic) BOOL floatingToolbarInputScheme;
@property(nonatomic) BOOL floatingToolbarScreenKeyboard;
@property(nonatomic) BOOL floatingToolbarVoice;
@property(nonatomic) BOOL floatingToolbarSettings;
@property(nonatomic) NSInteger floatingToolbarScalePercent;
@property(nonatomic) NSInteger floatingToolbarFontSize;
/// Refresh shared toolbar options without persisting them locally.
- (void)applySharedToolbarPreferences:(NSDictionary *)preferences;
/// Cache shared visibility without emitting a local-save notification.
- (void)applySharedToolbarVisibility:(BOOL)enabled;
- (BOOL)localModeEnabled:(NSString *)mode;
- (void)setLocalMode:(NSString *)mode enabled:(BOOL)enabled;
/// Refresh shared state without emitting a local-save notification.
- (void)applySharedLocalModes:(NSDictionary *)modes;
/// Cache input choices from shared storage without saving them back.
- (void)applySharedInputPreferences:(NSDictionary *)preferences;
/// 是否已经从共享偏好文档载入过一次输入选项。在此之前方案等字段读的是 NSUserDefaults 里本输入法自己上次写下的值，而设置页、`msime config set` 和云端同步只写文档，那个值可能早已过时（#4288）。
@property(nonatomic, readonly) BOOL sharedInputPreferencesApplied;
/// 本机有共享文档还没有、但收得下的应用例外（升级前存在 NSUserDefaults 里的规则）。最近一次 -applySharedInputPreferences: 之后为真时，输入法应当保存一次，把它们经 -sharedPreferencesByMerging: 发布进文档，共享设置页才显示得出输入法实际在用的规则。
@property(nonatomic, readonly) BOOL applicationInputModeRulesAwaitPublication;
- (void)applySharedCandidatePreferences:(NSDictionary *)preferences;
/// The native candidate panel appearance override. A nil value means AppKit follows the system.
@property(nonatomic, readonly) NSAppearance *candidateAppearanceOverride;
/// The same override for a theme drawn over the system base, whose mode comes from the light/dark choices alone rather than from the theme on screen: the mode an external package over a system base is drawn in once it is selected. A nil value means the system's mode.
@property(nonatomic, readonly) NSAppearance *systemBaseCandidateAppearanceOverride;
@property(nonatomic, readonly) BOOL candidateAppearanceOverrideConfigured;
/// The paging key group as one of the three presets the menu offers: 0 for -/= (the default), 1 for [/], 2 for Page Up/Page Down. It is read back out of the navigation bindings rather than out of a stored number of its own, so it is -1 when those bindings are in a state no preset names; setting it to anything else is setting it to 0.
@property(nonatomic) NSInteger pageShortcut;
- (BOOL)navigationEnabled:(NSString *)key;
- (void)setNavigation:(NSString *)key enabled:(BOOL)enabled;
- (NSDictionary *)wordCharacterOptions;
- (void)setWordCharacterEnabled:(BOOL)enabled keys:(NSString *)keys;
@end
