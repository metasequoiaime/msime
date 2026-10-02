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
@property(nonatomic, readonly) NSImage *decorationImage;
/// The drawn package's background image, read with the decoration when the theme is resolved; the candidate window draws it only in a mode whose resolved skin has a backgroundPath.
@property(nonatomic, readonly) NSImage *backgroundImage;
@property(nonatomic, readonly) NSURL *skinsRoot;
@property(nonatomic) BOOL vertical;
@property(nonatomic) BOOL candidateFollowCursor;
/// Show the short non-activating Chinese/English mode badge near the caret.
@property(nonatomic) BOOL inputModeHUD;
@property(nonatomic, copy) NSString *inputScheme;
/// The Chinese scheme to go back to when leaving japanese or korean: the current scheme while it is Chinese, otherwise the one left for japanese or korean here or the shared `last_chinese_scheme`, 全拼 when neither is known.
@property(nonatomic, readonly) NSString *lastChineseScheme;
/// The scheme the input method last showed a system input mode for, kept in this Mac's defaults and never in the shared document. The input controller compares the scheme running now with it, so a scheme picked while the input method was not running, or one whose dictionary was installed after it was picked, still reads as a change on the next sync and gets its opt-in mode enabled; the scheme it already names enables nothing.
@property(nonatomic, copy) NSString *lastSyncedInputScheme;
@property(nonatomic, copy) NSString *shuangpinProfile;
@property(nonatomic) BOOL shuangpinPreeditUsesRaw;
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
/// `custom_theme.candidate_skin`: the external package the custom theme draws, or nil for none.
@property(nonatomic, readonly, copy) NSString *customCandidateSkin;
/// Applies an external package: the custom theme, drawing that package, over the base its manifest names.
- (void)selectExternalSkin:(NSString *)skinId base:(NSString *)base;
/// 自定义主题不使用外部皮肤: drops the package and keeps the rest of the custom theme.
- (void)clearCustomCandidateSkin;
/// The custom theme as stored here (base, package and the seven pickers), as the host resolver takes it.
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
@property(nonatomic) BOOL shuangpinKeymap;
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
