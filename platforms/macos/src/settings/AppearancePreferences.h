#pragma once
#import <AppKit/AppKit.h>
#import "../../../../shared/apple/TextClient.h"
#include "../candidate/CandidateSkin.h"
#include "../input/ToggleHotkeys.h"

@class MSIMECandidateCanvasAssets;

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
/// 选中的皮肤包在原处被同名替换（安装、皮肤编辑器或 MCP 的 replace）或被手工改过时重新解析，返回是否重新解析了。只对浅色、深色槽位的包目录、skin.toml 和画出的装饰图、背景图各做一次 stat，不列目录、不读文件，可以在候选窗出现前和每秒的偏好轮询里调用。不发 MSIMEAppearanceDidChangeNotification：设置没有变，调用方自己刷新它画的东西。
- (BOOL)reloadReplacedSkinPackages;
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
// 已解码的 canvas 资源；候选布局复用，不在按键路径读文件。
- (MSIMECandidateCanvasAssets *)canvasAssetsForDark:(BOOL)dark;
/// 主题固定的明暗：浅色、深色两次解析给出同一个固定明暗时才有值（见 msime::mac::FixedThemeMode），候选窗、悬浮工具栏和菜单据此钉住明暗。
- (std::optional<bool>)fixedThemeMode;
@property(nonatomic, readonly) NSURL *skinsRoot;
@property(nonatomic) BOOL vertical;
@property(nonatomic) BOOL candidateFollowCursor;
/// Show the short non-activating Chinese/English mode badge near the caret.
@property(nonatomic) BOOL inputModeHUD;
/// 在菜单栏放一个状态项常驻显示当前输入模式（共享偏好 `input_mode_status_item`），没有共享值也没有本机值时不显示。
@property(nonatomic) BOOL inputModeStatusItem;
/// 候选窗和悬浮工具栏左端是否画水杉 logo（共享偏好 `show_app_logo`）。没有共享值也没有本机值时按新装处理，不画。
@property(nonatomic) BOOL showsAppLogo;
/// 候选窗首行是否画「1 / 3」和 ‹ › 翻页按钮（共享偏好 `show_candidate_page_number`）。没有共享值也没有本机值时画，与共享偏好的默认一致。它不改变键盘和滚轮翻页，但滚轮翻页（`navigation.mouse_wheel`）默认关，关掉它后鼠标只能靠滚轮翻页。
@property(nonatomic) BOOL showsCandidatePageNumber;
/// 「展开候选」（共享偏好 `expand_candidates`）：横排候选窗按 ↓ 展开成多行候选网格。没有共享值也没有本机值时关闭。
@property(nonatomic) BOOL expandCandidates;
@property(nonatomic) BOOL showsCandidateExpandButton;
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
/// 共享偏好中的译文字号，缺省为 12 点；设置入口由共享设置页提供。
@property(nonatomic, readonly) NSUInteger translationFontSize;
@property(nonatomic) BOOL showsCandidatePreedit;
/// The candidate window's own style: `candidate_scale_percent` (50-200, 100 by default), which multiplies the candidate fonts and every length of the window; `candidate_opacity_percent` (50-100, 100 by default), which fades only the card surface, its border and a package background; `candidate_corner_radius` (0-32 points), nil to keep the theme's or the package's card radius; and `candidate_padding` (0-16 points between the card's edge and its candidates), nil to keep the theme's or the package's `pad`. A value outside its range is refused and leaves the setting as it was.
@property(nonatomic) NSInteger candidateScalePercent;
@property(nonatomic) NSInteger candidateOpacityPercent;
@property(nonatomic, copy) NSNumber *candidateCornerRadius;
@property(nonatomic, copy) NSNumber *candidatePadding;
/// 共享偏好 `candidate_window_fit_content`：候选窗严格按文字宽度收紧，与皮肤 `fit_content` 是或关系；缺省为关。
@property(nonatomic, readonly) BOOL candidateWindowFitContent;
- (msime::mac::CandidateWindowStyle)candidateWindowStyle;
/// -resolvedSkinForDark: with -candidateWindowStyle laid over it, which is what the candidate window draws. The floating toolbar and the colour wells read the skin as resolved.
- (msime::mac::ResolvedSkin)candidateWindowSkinForDark:(BOOL)dark;
/// The 候选字体 preset: 0 默认, 1 宋体, 2 黑体, 3 楷体, 4 圆体, read back from `fontFamily`, or -1 for a family none of them writes. Setting one writes the preset's family for this host into `fontFamily` and puts all of its families, the other platforms' names included, at the front of `fallbackFonts`; 默认 restores the shared default pair.
@property(nonatomic) NSInteger candidateFontPreset;
@property(nonatomic, copy) NSString *candidateTextColor;
/// `custom_theme.candidate_colors` 里的其余候选取色器，同样是「#rrggbb」字符串，用主题自己的颜色时为 nil。设置任何一个都会选中自定义主题（正在显示的主题成为它的底）；设为 nil 只清掉这一个槽位。候选窗用 -resolvedSkinForDark: 画，那里已经套用了它们。编号色和边框色不在其中：macOS 上它们固定跟随主题和皮肤。
@property(nonatomic, copy) NSString *candidateAccentColor;
@property(nonatomic, copy) NSString *candidateSelectedColor;
@property(nonatomic, copy) NSString *candidateHoverColor;
@property(nonatomic, copy) NSString *candidateSurfaceColor;
/// 明暗选择：整个客户端取 system、dark 或 light；候选窗口取 follow、dark 或 light，可以覆盖全局那一项。悬浮工具栏、菜单和各面板一律跟随颜色模式。
@property(nonatomic, copy) NSString *themeMode;
@property(nonatomic, copy) NSString *candidateTheme;
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
/// Vim 模式（共享偏好 `vim_mode_apps`）：这个 bundle id 的前台应用是否把无修饰 Esc 搭车切英文。
- (BOOL)vimModeEnabledForApplication:(NSString *)identifier;
- (void)lockActiveInputMode;
- (void)resetRememberedInputModes;
@property(nonatomic) BOOL inputModeShortcut;
/// 中日英循环切换（共享偏好 `keybindings.trilingual_cycle`，默认关）：打开后「切换中英文」那一组快捷键按 中 → 日 → 英 → 中 转圈。顺序和「有组字时先上屏原始字母」在共享层，见 `crates/client-core/src/language_cycle.rs`。
@property(nonatomic) BOOL trilingualCycle;
@property(nonatomic) BOOL shiftTapShortcut;
@property(nonatomic) BOOL controlTapShortcut;
/// 单按 Caps Lock 切换中英文（共享偏好 `keybindings.switch_language_caps_lock`），缺省为关。
@property(nonatomic) BOOL capsLockTapShortcut;
@property(nonatomic) BOOL controlOptionSpaceShortcut;
@property(nonatomic) BOOL characterSetShortcut;
@property(nonatomic) BOOL fullWidthShortcut;
/// 三个可重绑切换热键（共享偏好 `keybindings.toggle_english_candidates`、`toggle_full_width`、`toggle_punctuation`）解码出的认键组合；写法为空或不合法时 bound=NO 不绑定。共享文档还没载入或没有 keybindings 时按出厂值（client-core 的 `TOGGLE_*_DEFAULT`）。
@property(nonatomic, readonly) msime::mac::ToggleChord englishCandidatesToggleChord;
@property(nonatomic, readonly) msime::mac::ToggleChord fullWidthToggleChord;
@property(nonatomic, readonly) msime::mac::ToggleChord punctuationToggleChord;
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
@property(nonatomic) BOOL mixedEmojiInput;
@property(nonatomic) BOOL mixedKaomojiInput;
/// 符号混输（共享偏好 `mixed_input.symbols`），默认开；共享文档只在关掉时才有这个键。
@property(nonatomic) BOOL mixedSymbolInput;
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
/// The four ways of starting dictation. Control + F9 is a press, the other three are holds. 按住时按空格锁定录音在 macOS 上固定为开，不再是偏好。
@property(nonatomic) BOOL voiceHotkeyCtrlF9;
@property(nonatomic) BOOL voiceHotkeyRightAlt;
@property(nonatomic) BOOL voiceHotkeyCtrlCommand;
@property(nonatomic) BOOL voiceHotkeyCtrlOption;
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
