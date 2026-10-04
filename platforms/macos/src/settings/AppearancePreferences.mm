#import "AppearancePreferences.h"
#import "SettingsLayout.h"
#import "../backend/account/BackendAccountEntry.h"
#import "../candidate/CandidateFontCache.h"
#import "../candidate/CandidateSkinPreviewView.h"
#import "../candidate/SkinSettingsView.h"
#import "../cloud/CloudAppearanceSettings.h"
#import "RuntimeOptions.h"
#import "../input/InputModeIdentifiers.h"
#import "../dictionary/DictionaryWindowController.h"

extern "C" bool msime_macos_uninstall_input_source(const char *bundle_path,
                                                     const char *user_data_path,
                                                     const char *preferences_domain,
                                                     bool remove_user_data) __attribute__((weak_import));
#import "../cloud/TranslationSettingsWindow.h"
#import "../core/DesktopSettingsLauncher.h"
#import "../core/AISettingsWindow.h"
#import "../core/SharedVoicePreferences.h"
#import "../core/UpdateController.h"
#import "../core/SupportWindowController.h"
#import "../core/BoundedFileReader.h"
#import "../voice/VoiceSettingsEntry.h"
#import "../core/WindowPresentation.h"
#include "ShuangpinProfileNames.h"
#include "../candidate/CandidatePageSize.h"

@interface MSIMETranslationSettingsWindow (Lifecycle)
- (void)invalidatePendingCallbacks;
@end


/// The voice form, looked up at runtime. Linking it here would drag the voice module — and the
/// keychain and CoreAudio with it — into every test executable that builds this window.
@protocol MSIMEVoiceSettingsForm <NSObject>
- (void)reloadSettings;
@end

extern "C" NSView *MSIMEAccountPaneView(void) __attribute__((weak_import));
extern "C" void MSIMEAccountPaneAttach(NSWindow *window) __attribute__((weak_import));
BOOL (*MSIMEInputModeEnabledProbe)(NSString *identifier) = nullptr;
extern "C" void MSIMEAccountPaneClose(void) __attribute__((weak_import));

NSNotificationName const MSIMEAppearanceDidChangeNotification = @"MSIMEClientAppearanceDidChange";
NSNotificationName const MSIMETranslationPreferencesDidSaveNotification = @"MSIMEClientTranslationPreferencesDidSave";
NSString *const MSIMEAppearanceInputModeOnlyKey = @"MSIMEClientAppearanceInputModeOnly";
static NSString *const LayoutKey = @"MSIMEClientCandidatePanelStyle";
static NSString *const CandidateFollowCursorKey = @"MSIMEClientCandidateFollowCursor";
static NSString *const InputModeHUDKey = @"MSIMEClientInputModeHUD";
static NSString *const SchemeKey = @"MSIMEClientInputScheme";
static NSString *const LastSyncedSchemeKey = @"MSIMEClientLastSyncedInputScheme";
static NSString *const ShuangpinProfileKey = @"MSIMEClientShuangpinProfile";
static NSString *const ShuangpinPreeditKey = @"MSIMEClientShuangpinPreeditUsesRaw";
/// 五笔码表版本：`wubi86` 或 `wubi98`，写进共享偏好文档的 `wubi_profile`。
static NSString *const WubiProfileKey = @"MSIMEClientWubiProfile";
static NSString *const LocalModesKey = @"MSIMEClientLocalModes";
static NSArray<NSArray<NSString *> *> *LocalModeControls() {
    return @[@[@"quick_phrase", @"快捷短语（K 模式）"], @[@"date_time", @"日期与时间（T 模式）"],
             @[@"unicode", @"Unicode 录入（U 模式）"], @[@"emoji", @"Emoji（E 模式）"],
             @[@"kaomoji", @"颜文字（M 模式）"], @[@"super_jianpin", @"超级简拼（J 模式）"],
             @[@"temporary_english", @"临时英文（Y 模式）"], @[@"temporary_japanese", @"临时日语（R 模式）"]];
}
static BOOL KnownLocalMode(NSString *mode) {
    for (NSArray *entry in LocalModeControls()) if ([entry[0] isEqual:mode]) return YES;
    return NO;
}
static BOOL LocalModeBoolean(id value) {
    return [value isKindOfClass:NSNumber.class] && CFGetTypeID((__bridge CFTypeRef)value) == CFBooleanGetTypeID();
}
static BOOL ValidMixedPrefix(id value) {
    return [value isKindOfClass:NSNumber.class] && CFGetTypeID((__bridge CFTypeRef)value) != CFBooleanGetTypeID() &&
           !CFNumberIsFloatType((__bridge CFNumberRef)value) && [value integerValue] >= 1 && [value integerValue] <= 8;
}
static NSString *const FontKey = @"MSIMEClientCandidateFontSize";
static NSString *const FontFamilyKey = @"MSIMEClientCandidateFontFamily";
static NSString *const CandidateEnglishFontKey = @"MSIMEClientCandidateEnglishFont";
static NSString *const TextColorKey = @"MSIMEClientCandidateTextColor";
// The six candidate colours beside the text colour. The candidate window has been drawing with them
// all along — InputController.mm asks for every one of them when it fills a candidate row — but they
// arrived only from an account push into an in-memory override, so there was no way to set one here
// and nothing survived a restart.
static NSString *const NumberColorKey = @"MSIMEClientCandidateNumberColor";
static NSString *const AccentColorKey = @"MSIMEClientCandidateAccentColor";
static NSString *const SelectedColorKey = @"MSIMEClientCandidateSelectedColor";
static NSString *const HoverColorKey = @"MSIMEClientCandidateHoverColor";
static NSString *const SurfaceColorKey = @"MSIMEClientCandidateSurfaceColor";
static NSString *const BorderColorKey = @"MSIMEClientCandidateBorderColor";
/// The global light/dark choice and the two surfaces that may override it. All three are read at
/// runtime — the candidate panel resolves its appearance from `theme` and `candidate_theme`, the
/// floating toolbar from `theme` and `toolbar_theme` — and all three used to reach this host only
/// from the cloud, so a machine that had never signed in had no way to choose and a machine that had
/// lost the choice on the next launch.
static NSString *const ThemeKey = @"MSIMEClientTheme";
static NSString *const CandidateThemeKey = @"MSIMEClientCandidateTheme";
static NSString *const ToolbarThemeKey = @"MSIMEClientToolbarTheme";
static BOOL ValidTextColor(id value) {
    if (![value isKindOfClass:NSString.class] || [value length] != 7 || ![value hasPrefix:@"#"]) return NO;
    return [[value substringFromIndex:1] rangeOfCharacterFromSet:[[NSCharacterSet characterSetWithCharactersInString:@"0123456789abcdefABCDEF"] invertedSet]].location == NSNotFound;
}
static NSColor *CandidateColor(id value, NSColor *fallback) {
    if (!ValidTextColor(value)) return fallback;
    unsigned int rgb = 0;
    [[NSScanner scannerWithString:[value substringFromIndex:1]] scanHexInt:&rgb];
    return [NSColor colorWithSRGBRed:((rgb >> 16) & 255) / 255.0 green:((rgb >> 8) & 255) / 255.0 blue:(rgb & 255) / 255.0 alpha:1];
}
static id SharedCandidateColor(NSDictionary *preferences, NSString *key, id current) {
    id value = preferences[key];
    if (!value || value == NSNull.null) return NSNull.null;
    return ValidTextColor(value) ? [value copy] : current;
}
/// The six candidate colours the window draws a well for, each as {property, label, stored key, slot of `custom_theme.candidate_colors`}. One table instead of six copies of "a well, a button, a setter, a stored key and a section entry": the wells are built from it, the well and its 跟随皮肤 button carry the property name as their identifier and are routed back through it, the merge publishes from it, and the 配色 section registers its keys from it.
///
/// 候选文字颜色 is not in it. It is the one colour this window already had, it is set by typing a hex value as well as by the well, and three tests name its selectors, so it keeps the row it has always had rather than being folded in and renamed.
static NSArray<NSArray<NSString *> *> *CandidateColorControls() {
    return @[
        @[ @"candidateNumberColor", @"候选编号颜色", NumberColorKey, @"number" ],
        @[ @"candidateAccentColor", @"候选强调色", AccentColorKey, @"accent" ],
        @[ @"candidateSelectedColor", @"候选选中色", SelectedColorKey, @"selected" ],
        @[ @"candidateHoverColor", @"候选悬停色", HoverColorKey, @"hover" ],
        @[ @"candidateSurfaceColor", @"候选表面色", SurfaceColorKey, @"surface" ],
        @[ @"candidateBorderColor", @"候选边框色", BorderColorKey, @"border" ],
    ];
}
static NSColor *SkinTokenColor(msime::mac::Rgba color) {
    return [NSColor colorWithSRGBRed:color.r green:color.g blue:color.b alpha:color.a];
}
/// The global mode, and the two surface overrides that may take precedence over it. `follow` is not a
/// mode of its own: it is the surface saying it has no opinion, which is why the two lists differ.
static NSArray<NSString *> *ThemeModes() { return @[@"system", @"dark", @"light"]; }
static NSArray<NSString *> *SurfaceThemes() { return @[@"follow", @"dark", @"light"]; }
static NSString *const FallbackFontsKey = @"MSIMEClientCandidateFallbackFonts";
static BOOL ValidFontFamily(id value) {
    return [value isKindOfClass:NSString.class] && [value length] > 0 &&
           [value lengthOfBytesUsingEncoding:NSUTF8StringEncoding] <= 128 &&
           [value rangeOfCharacterFromSet:[NSCharacterSet controlCharacterSet]].location == NSNotFound;
}
/// How many supplementary families the candidate window will carry, as crates/client-core validates.
static const NSUInteger kFallbackFontLimit = 32;
/// The two columns of 应用例外 and the reuse identifier of a cell in each: an application's name, and the mode it is to start in.
static NSUserInterfaceItemIdentifier const MSIMEAppRuleApplicationColumn = @"application";
static NSUserInterfaceItemIdentifier const MSIMEAppRuleModeColumn = @"mode";
static BOOL ValidFallbackFonts(id value) {
    if (![value isKindOfClass:NSArray.class] || [value count] > kFallbackFontLimit) return NO;
    for (id family in value) if (!ValidFontFamily(family)) return NO;
    return YES;
}
static NSString *const PreeditFontKey = @"MSIMEClientCandidatePreeditFontSize";
/// The window's own size, card opacity and corner radius: candidate_scale_percent, candidate_opacity_percent and candidate_corner_radius. The radius is stored as a number of points, or as an empty string once the user has asked to follow the skin again, the same marker 候选窗英文字体 uses, because NSUserDefaults cannot hold NSNull and an absent entry has to keep meaning "never chosen here".
static NSString *const CandidateScaleKey = @"MSIMEClientCandidateScalePercent";
static NSString *const CandidateOpacityKey = @"MSIMEClientCandidateOpacityPercent";
static NSString *const CandidateCornerRadiusKey = @"MSIMEClientCandidateCornerRadius";
/// A whole number in [minimum, maximum], as the shared document's integer fields are: a boolean or a fraction is not one.
static BOOL ValidCandidateStyleInteger(id value, NSInteger minimum, NSInteger maximum) {
    return [value isKindOfClass:NSNumber.class] && CFGetTypeID((__bridge CFTypeRef)value) != CFBooleanGetTypeID() &&
           [value doubleValue] == [value integerValue] && [value integerValue] >= minimum && [value integerValue] <= maximum;
}
/// The ranges crates/client-core validates. The sliders offer less of them (75-150% and 0-16pt), but a value another surface wrote inside the shared range is honoured rather than refused.
static BOOL ValidCandidateScale(id value) { return ValidCandidateStyleInteger(value, 50, 200); }
static BOOL ValidCandidateOpacity(id value) { return ValidCandidateStyleInteger(value, 50, 100); }
static BOOL ValidCandidateCornerRadius(id value) { return ValidCandidateStyleInteger(value, 0, 32); }
/// The 候选字体 presets in the order the popup lists them: a title, then the families the preset writes. The first family is the one this host puts in 候选字体; all of them go to the front of the fallback list, because the document is shared and the Windows and Linux names are what those hosts will find there. 默认 is the shared default pair (crates/client-core) rather than a preset of its own.
static NSArray<NSArray<NSString *> *> *CandidateFontPresets() {
    return @[
        @[ @"默认", @"Noto Sans SC", @"Microsoft YaHei" ],
        @[ @"宋体", @"Songti SC", @"SimSun", @"Noto Serif CJK SC", @"Noto Serif SC" ],
        @[ @"黑体", @"PingFang SC", @"Microsoft YaHei", @"Noto Sans CJK SC", @"Noto Sans SC" ],
        @[ @"楷体", @"Kaiti SC", @"KaiTi", @"STKaiti", @"AR PL UKai CN" ],
        @[ @"圆体", @"Yuanti SC", @"Noto Sans SC" ],
    ];
}
static NSString *const CandidatePreeditKey = @"MSIMEClientCandidatePreeditStyle";
static NSString *const PageShortcutKey = @"MSIMEClientCandidatePageShortcut";
static NSString *const NavigationKey = @"MSIMEClientNavigation";
static NSString *const WordCharacterKey = @"MSIMEClientWordCharacter";
static NSArray<NSArray<NSString *> *> *NavigationControls() {
    return @[@[@"minus_equal", @"减号/等号翻页"], @[@"comma_period", @"逗号/句号翻页"],
             @[@"brackets", @"方括号翻页"], @[@"tab", @"Tab / Shift-Tab 翻页"],
             @[@"page_up_down", @"Page Up / Page Down 翻页"], @[@"mouse_wheel", @"鼠标滚轮翻页"],
             @[@"arrows", @"方向键选择候选"]];
}
static NSString *const PageSizeKey = @"MSIMEClientCandidatePageSize";
/// The global theme and the two parts of `custom_theme` this window stores beside the seven picker colours. The picker colours keep the keys above; they are `custom_theme.candidate_colors` now, so they colour the candidate window only while the custom theme is selected.
static NSString *const GlobalThemeKey = @"MSIMEClientGlobalTheme";
static NSString *const CustomThemeBaseKey = @"MSIMEClientCustomThemeBase";
static NSString *const CustomCandidateSkinKey = @"MSIMEClientCustomCandidateSkin";
/// `custom_theme.candidate_skin`: a package folder name, never a global theme id.
static BOOL ValidCustomCandidateSkin(id value) {
    return [value isKindOfClass:NSString.class] && [value length] > 0 && [value length] <= 64 &&
           msime::mac::IsSafeSkinId([value UTF8String]) && !msime::mac::IsGlobalThemeId([value UTF8String]);
}
static NSString *const EnglishKey = @"MSIMEClientEnglishInputMode";
static NSString *const DefaultImeModeKey = @"MSIMEClientDefaultImeMode";
static NSString *const ImeModeScopeKey = @"MSIMEClientImeModeScope";
/// The applications the user has decided the input mode of, against 「chinese」 or 「english」.
///
/// Deliberately not the same store as the remembered mode: a rule is a decision the user wrote down and a memory is an observation of what they last did, so a rule is saved and a memory is not, and -resetRememberedInputModes throws the observations away on every input-source switch without touching the decisions. Reading them apart is also the only way the lookup below can put the rule first.
///
/// macOS-local for now, and so absent from both -sharedPreferencesByMerging: and -cloudSettingsSnapshot. Publishing it would make it a field of crates/client-core's Preferences, which is a shape Windows, Linux, iOS and HarmonyOS have to agree on before any one host starts writing it.
static NSString *const AppInputModeRulesKey = @"MSIMEClientAppInputModeRules";
static BOOL ValidInputModeRule(id value) {
    return [value isKindOfClass:NSString.class] && [@[@"chinese", @"english"] containsObject:value];
}
static NSString *const TraditionalKey = @"MSIMEClientTraditionalOutput";
static NSString *const FullWidthKey = @"MSIMEClientFullWidthInput";
static NSString *const ChinesePunctuationKey = @"MSIMEClientChinesePunctuation";
static NSString *const SmartPunctuationKey = @"MSIMEClientSmartPunctuation";
static NSString *const SmartPunctuationRepeatToChineseKey = @"MSIMEClientSmartPunctuationRepeatToChinese";
static NSString *const SmartPunctuationSpaceConvertKey = @"MSIMEClientSmartPunctuationSpaceConvert";
static NSString *const PairedPunctuationKey = @"MSIMEClientPairedPunctuation";
static NSString *const PunctuationLockKey = @"MSIMEClientPunctuationLock";
static NSString *const MixedInputKey = @"MSIMEClientMixedInput";
static NSString *const CandidateLearningKey = @"MSIMEClientCandidateLearning";
static NSString *const FrequencyModeKey = @"MSIMEClientFrequencyAdjustmentMode";
static NSString *const FrequencyTriggerCountKey = @"MSIMEClientFrequencyTriggerCount";
static NSString *const FrequencyLinearStepKey = @"MSIMEClientFrequencyLinearStep";
static NSArray<NSString *> *FrequencyModes() { return @[@"disabled", @"pin", @"halve", @"linear", @"promote"]; }
static BOOL ValidFrequencyMode(id value) { return [value isKindOfClass:NSString.class] && [FrequencyModes() containsObject:value]; }
static BOOL ValidFrequencyCount(id value) {
    return [value isKindOfClass:NSNumber.class] && CFGetTypeID((__bridge CFTypeRef)value) != CFBooleanGetTypeID() &&
           !CFNumberIsFloatType((__bridge CFNumberRef)value) && [value integerValue] >= 1 && [value integerValue] <= 10;
}
static NSString *const FuzzyPinyinKey = @"MSIMEClientFuzzyPinyinEnabled";
static NSString *const FuzzyPinyinRulesKey = @"MSIMEClientFuzzyPinyinRules";
static NSArray<NSArray<NSString *> *> *FuzzyPinyinRuleControls() {
    return @[
        @[@"z-zh", @"z / zh"], @[@"c-ch", @"c / ch"], @[@"s-sh", @"s / sh"],
        @[@"n-l", @"n / l"], @[@"f-h", @"f / h"], @[@"r-l", @"r / l"],
        @[@"an-ang", @"an / ang"], @[@"en-eng", @"en / eng"], @[@"in-ing", @"in / ing"],
        @[@"ian-iang", @"ian / iang"], @[@"uan-uang", @"uan / uang"],
    ];
}
static BOOL ValidFuzzyPinyinRules(id value) {
    if (![value isKindOfClass:NSArray.class]) return NO;
    NSMutableSet *known = [NSMutableSet set];
    for (NSArray *entry in FuzzyPinyinRuleControls()) [known addObject:entry[0]];
    NSMutableSet *seen = [NSMutableSet set];
    for (id rule in value) {
        if (![rule isKindOfClass:NSString.class] || ![known containsObject:rule] || [seen containsObject:rule]) return NO;
        [seen addObject:rule];
    }
    return YES;
}
static NSString *const CloudCandidatesKey = @"MSIMEClientCloudCandidates";
// First-use consent for cloud candidates: absent = never evaluated (treated as answered), 1 = waiting for the prompt, 2 = answered or inherited from an existing install.
static NSString *const CloudCandidatesConsentKey = @"MSIMEClientCloudCandidatesConsent";
static const NSInteger CloudCandidatesConsentPending = 1;
static const NSInteger CloudCandidatesConsentAnswered = 2;
static NSString *const CandidateTranslationsKey = @"MSIMEClientCandidateTranslations";
static NSString *const CandidateEnglishGlossKey = @"MSIMEClientCandidateEnglishGloss";
static NSString *const TranspositionKey = @"MSIMEClientAutocorrectTransposition";
static NSString *const NeighborKey = @"MSIMEClientAutocorrectNeighbor";
static NSString *const HelpcodeOptionsKey = @"MSIMEClientHelpcodeOptions";
static NSArray<NSString *> *HelpcodeSchemas() { return @[@"lantian", @"ziranma", @"shouyou2_0", @"shouyouplus", @"xiaohe", @"jiajia"]; }
static BOOL ValidHelpcodeOption(NSString *key, id value) {
    return [key isEqual:@"schema"] ? [HelpcodeSchemas() containsObject:value] :
        ([key isEqual:@"show_in_candidate_window"] && LocalModeBoolean(value));
}
static NSString *const QuanpinHelpcodeKey = @"MSIMEClientQuanpinHelpcodeEnabled";
static NSString *const ShuangpinHelpcodeKey = @"MSIMEClientShuangpinHelpcodeEnabled";
static NSString *const KeymapKey = @"MSIMEClientShuangpinKeymap";
static NSString *const WubiKey = @"MSIMEClientWubiAutoCommitUnique";
static NSString *const WubiMixedPinyinKey = @"MSIMEClientWubiMixedPinyin";
static NSString *const InputModeShortcutKey = @"MSIMEClientInputModeShortcut";
static NSString *const ShiftTapShortcutKey = @"MSIMEClientShiftTapShortcut";
static NSString *const ControlTapShortcutKey = @"MSIMEClientControlTapShortcut";
static NSString *const ControlOptionSpaceShortcutKey = @"MSIMEClientControlOptionSpaceShortcut";
static NSString *const CharacterSetShortcutKey = @"MSIMEClientCharacterSetShortcut";
static NSString *const FullWidthShortcutKey = @"MSIMEClientFullWidthShortcut";
/// The dictation preferences this window now owns. They are read on every recording — by InputController's event tap for the hotkeys, by the recogniser request for the language and the streaming preedit, by the cue player and the audio muter for the other two — and the only place they could be set was MSIMEVoiceSettings, a window nothing in the repository opened.
static NSString *const VoiceEnabledKey = @"MSIMEClientVoiceEnabled";
static NSString *const VoiceLanguageKey = @"MSIMEClientVoiceLanguage";
static NSString *const VoiceSoundKey = @"MSIMEClientVoiceSoundEnabled";
static NSString *const VoiceMuteSystemAudioKey = @"MSIMEClientVoiceMuteSystemAudio";
static NSString *const VoiceStreamInlinePreeditKey = @"MSIMEClientVoiceStreamInlinePreedit";
static NSString *const VoiceHotkeyCtrlF9Key = @"MSIMEClientVoiceHotkeyCtrlF9";
static NSString *const VoiceHotkeyHoldSpaceKey = @"MSIMEClientVoiceHotkeyHoldSpace";
static NSString *const VoiceHotkeyRightAltKey = @"MSIMEClientVoiceHotkeyRightAlt";
static NSString *const VoiceHotkeyCtrlCommandKey = @"MSIMEClientVoiceHotkeyCtrlCommand";
static NSString *const VoiceHotkeyCtrlOptionKey = @"MSIMEClientVoiceHotkeyCtrlOption";
static NSString *const FloatingToolbarKey = @"MSIMEClientFloatingToolbarEnabled";
static NSString *const FloatingToolbarOptionsKey = @"MSIMEClientFloatingToolbarOptions";
static NSArray<NSString *> *FloatingToolbarComponentKeys() {
    return @[@"english_mode", @"input_scheme", @"punctuation", @"fullwidth", @"character_set", @"emoji", @"handwriting",
             @"screen_keyboard", @"voice", @"settings"];
}
static BOOL ValidToolbarScale(id value) {
    return [value isKindOfClass:NSNumber.class] && CFGetTypeID((__bridge CFTypeRef)value) != CFBooleanGetTypeID() &&
           !CFNumberIsFloatType((__bridge CFNumberRef)value) && [@[@75, @100, @125, @150] containsObject:value];
}
static BOOL ValidToolbarFontSize(id value) {
    return [value isKindOfClass:NSNumber.class] && CFGetTypeID((__bridge CFTypeRef)value) != CFBooleanGetTypeID() &&
           !CFNumberIsFloatType((__bridge CFNumberRef)value) && [value integerValue] >= 16 && [value integerValue] <= 28;
}

/// Each stored preference key, against the in-memory override that an account push leaves beside it.
///
/// A value pushed down from the cloud is held in a `_shared*` ivar and every accessor consults it
/// ahead of the stored one, so deleting the NSUserDefaults entry alone leaves the pushed value in
/// force: "恢复默认值" was a button that did nothing at all for anyone who had ever signed in.
/// Clearing the ivar is what makes the accessor fall back to the stored value, which by then is the
/// default because the entry has just been removed.
///
/// The values are the ivar names without their leading underscore, reached through key-value coding.
/// That is the price of having one table instead of two: a second hand-kept list of assignments
/// beside this one is the shape the restore lists were already in, and drifting apart is what they
/// already did. A name here that no ivar answers to raises NSUndefinedKeyException the first time
/// its key is restored, which is loud and immediate rather than silent.
static NSDictionary<NSString *, NSString *> *SharedOverrideProperties() {
    return @{
        LayoutKey : @"sharedVertical",
        CandidateFollowCursorKey : @"sharedCandidateFollowCursor",
        InputModeHUDKey : @"sharedInputModeHUD",
        SchemeKey : @"sharedInputScheme",
        ShuangpinProfileKey : @"sharedShuangpinProfile",
        ShuangpinPreeditKey : @"sharedShuangpinPreeditUsesRaw",
        WubiProfileKey : @"sharedWubiProfile",
        WubiMixedPinyinKey : @"sharedWubiMixedPinyin",
        LocalModesKey : @"sharedLocalModes",
        FontKey : @"sharedFontSize",
        FontFamilyKey : @"sharedFontFamily",
        CandidateEnglishFontKey : @"sharedCandidateEnglishFont",
        TextColorKey : @"sharedTextColor",
        NumberColorKey : @"sharedNumberColor",
        AccentColorKey : @"sharedAccentColor",
        SelectedColorKey : @"sharedSelectedColor",
        HoverColorKey : @"sharedHoverColor",
        SurfaceColorKey : @"sharedSurfaceColor",
        BorderColorKey : @"sharedBorderColor",
        ThemeKey : @"sharedTheme",
        GlobalThemeKey : @"sharedGlobalTheme",
        CustomThemeBaseKey : @"sharedCustomThemeBase",
        CustomCandidateSkinKey : @"sharedCustomCandidateSkin",
        CandidateThemeKey : @"sharedCandidateTheme",
        ToolbarThemeKey : @"sharedToolbarTheme",
        FallbackFontsKey : @"sharedFallbackFonts",
        PreeditFontKey : @"sharedPreeditFontSize",
        CandidateScaleKey : @"sharedCandidateScale",
        CandidateOpacityKey : @"sharedCandidateOpacity",
        CandidateCornerRadiusKey : @"sharedCandidateCornerRadius",
        CandidatePreeditKey : @"sharedCandidatePreedit",
        NavigationKey : @"sharedNavigation",
        WordCharacterKey : @"sharedWordCharacter",
        PageSizeKey : @"sharedPageSize",
        DefaultImeModeKey : @"sharedDefaultImeMode",
        ImeModeScopeKey : @"sharedImeModeScope",
        FullWidthKey : @"sharedFullWidthInput",
        TraditionalKey : @"sharedTraditionalOutput",
        ChinesePunctuationKey : @"sharedChinesePunctuation",
        SmartPunctuationKey : @"sharedSmartPunctuation",
        SmartPunctuationRepeatToChineseKey : @"sharedSmartPunctuationRepeatToChinese",
        SmartPunctuationSpaceConvertKey : @"sharedSmartPunctuationSpaceConvert",
        PairedPunctuationKey : @"sharedPairedPunctuation",
        PunctuationLockKey : @"sharedPunctuationLock",
        MixedInputKey : @"sharedMixedInput",
        CandidateLearningKey : @"sharedCandidateLearning",
        FrequencyModeKey : @"sharedFrequencyMode",
        FrequencyTriggerCountKey : @"sharedFrequencyTriggerCount",
        FrequencyLinearStepKey : @"sharedFrequencyLinearStep",
        FuzzyPinyinKey : @"sharedFuzzyPinyinEnabled",
        FuzzyPinyinRulesKey : @"sharedFuzzyPinyinRules",
        CloudCandidatesKey : @"sharedCloudCandidates",
        CandidateTranslationsKey : @"sharedCandidateTranslations",
        CandidateEnglishGlossKey : @"sharedCandidateEnglishGloss",
        TranspositionKey : @"sharedTransposition",
        NeighborKey : @"sharedNeighbor",
        HelpcodeOptionsKey : @"sharedHelpcodeOptions",
        QuanpinHelpcodeKey : @"sharedQuanpinHelpcode",
        ShuangpinHelpcodeKey : @"sharedShuangpinHelpcode",
        InputModeShortcutKey : @"sharedInputModeShortcut",
        ShiftTapShortcutKey : @"sharedShiftTapShortcut",
        ControlTapShortcutKey : @"sharedControlTapShortcut",
        ControlOptionSpaceShortcutKey : @"sharedControlOptionSpaceShortcut",
        CharacterSetShortcutKey : @"sharedCharacterSetShortcut",
        FullWidthShortcutKey : @"sharedFullWidthShortcut",
        FloatingToolbarKey : @"sharedToolbarEnabled",
        FloatingToolbarOptionsKey : @"sharedToolbarOptions",
    };
}

/// 应用例外, read by the probe table below. It is not in AppearancePreferences.h because nothing outside this file reads or writes a rule — the lookup that consults them is -englishMode, which is public already — and the probes are written above the implementation, so the name has to be declared before they can ask for it by it.
@interface MSIMEAppearancePreferences ()
- (NSDictionary<NSString *, NSString *> *)applicationInputModeRules;
- (instancetype)initSilentlyWithDefaults:(NSUserDefaults *)defaults;
@end

/// What one stored key currently reads as, asked of the accessors rather than of the stored entry.
///
/// A block rather than a key path, so that the compiler checks the names, and so that a key whose value is spread over several accessors — the four mixed-input choices, the eleven fuzzy rules, the twelve floating-toolbar options — can still be answered in one place. Dictionary-valued probes are also what let a section own part of a stored dictionary instead of all of it; see MSIMESettingsSection.fields.
typedef id (^MSIMESettingProbe)(MSIMEAppearancePreferences *preferences);

/// Every stored key a section can offer to restore, against the probe that answers for it.
///
/// This is what tells a key that is at its default from one that is not, and presence cannot do that job: the accessors read `_shared*` ahead of the stored entry, and those ivars are filled on every ordinary preference reload — InputController.mm calls -applySharedInputPreferences:, -applySharedCandidatePreferences:, -applySharedAssistancePreferences:, -applySharedToolbarPreferences: and -applySharedLocalModes: each time, out of fields that are not optional in crates/client-core/src/preferences.rs — so on a machine that has never signed in and never changed anything, every one of them is non-nil. Reading a non-nil ivar as "an account pushed this" put a standing 恢复默认值 on every section of the window opened from the input method's menu, while the standalone --preferences launch, which has no InputController to fill them, correctly showed none.
static NSDictionary<NSString *, MSIMESettingProbe> *SettingProbes() {
    static NSDictionary<NSString *, MSIMESettingProbe> *probes;
    static dispatch_once_t once;
    dispatch_once(&once, ^{
        NSMutableDictionary<NSString *, MSIMESettingProbe> *table = [@{
            DefaultImeModeKey : ^id(MSIMEAppearancePreferences *p) { return p.defaultImeMode ?: NSNull.null; },
            ImeModeScopeKey : ^id(MSIMEAppearancePreferences *p) { return p.imeModeScope ?: NSNull.null; },
            AppInputModeRulesKey : ^id(MSIMEAppearancePreferences *p) { return [p applicationInputModeRules]; },
            SchemeKey : ^id(MSIMEAppearancePreferences *p) { return p.inputScheme ?: NSNull.null; },
            ShuangpinProfileKey : ^id(MSIMEAppearancePreferences *p) { return p.shuangpinProfile ?: NSNull.null; },
            ShuangpinPreeditKey : ^id(MSIMEAppearancePreferences *p) { return @(p.shuangpinPreeditUsesRaw); },
            WubiProfileKey : ^id(MSIMEAppearancePreferences *p) { return p.wubiProfile ?: NSNull.null; },
            KeymapKey : ^id(MSIMEAppearancePreferences *p) { return @(p.shuangpinKeymap); },
            WubiKey : ^id(MSIMEAppearancePreferences *p) { return @(p.wubiAutoCommitUnique); },
            WubiMixedPinyinKey : ^id(MSIMEAppearancePreferences *p) { return @(p.wubiMixedPinyinEnabled); },
            QuanpinHelpcodeKey : ^id(MSIMEAppearancePreferences *p) { return @(p.quanpinHelpcodeEnabled); },
            ShuangpinHelpcodeKey : ^id(MSIMEAppearancePreferences *p) { return @(p.shuangpinHelpcodeEnabled); },
            HelpcodeOptionsKey : ^id(MSIMEAppearancePreferences *p) {
                return @{@"quanpin" : [p helpcodeOptionsForScheme:@"quanpin"],
                         @"shuangpin" : [p helpcodeOptionsForScheme:@"shuangpin"]};
            },
            ChinesePunctuationKey : ^id(MSIMEAppearancePreferences *p) { return @(p.chinesePunctuation); },
            SmartPunctuationKey : ^id(MSIMEAppearancePreferences *p) { return @(p.smartPunctuation); },
            SmartPunctuationRepeatToChineseKey : ^id(MSIMEAppearancePreferences *p) { return @(p.smartPunctuationRepeatToChinese); },
            SmartPunctuationSpaceConvertKey : ^id(MSIMEAppearancePreferences *p) { return @(p.smartPunctuationSpaceConvert); },
            PairedPunctuationKey : ^id(MSIMEAppearancePreferences *p) { return @(p.pairedPunctuation); },
            PunctuationLockKey : ^id(MSIMEAppearancePreferences *p) { return p.punctuationLock ?: NSNull.null; },
            FullWidthKey : ^id(MSIMEAppearancePreferences *p) { return @(p.fullWidthInput); },
            TraditionalKey : ^id(MSIMEAppearancePreferences *p) { return @(p.traditionalOutput); },
            MixedInputKey : ^id(MSIMEAppearancePreferences *p) {
                return @[@(p.mixedEnglishInput), @(p.mixedEnglishMinimumPrefix), @(p.mixedEmojiInput), @(p.mixedKaomojiInput)];
            },
            TranspositionKey : ^id(MSIMEAppearancePreferences *p) { return @(p.autocorrectTransposition); },
            NeighborKey : ^id(MSIMEAppearancePreferences *p) { return @(p.autocorrectNeighbor); },
            FuzzyPinyinKey : ^id(MSIMEAppearancePreferences *p) { return @(p.fuzzyPinyinEnabled); },
            FuzzyPinyinRulesKey : ^id(MSIMEAppearancePreferences *p) {
                NSMutableArray<NSString *> *enabled = [NSMutableArray array];
                for (NSArray<NSString *> *rule in FuzzyPinyinRuleControls())
                    if ([p fuzzyPinyinRuleEnabled:rule[0]]) [enabled addObject:rule[0]];
                return enabled;
            },
            LocalModesKey : ^id(MSIMEAppearancePreferences *p) {
                NSMutableDictionary<NSString *, NSNumber *> *modes = [NSMutableDictionary dictionary];
                for (NSArray<NSString *> *mode in LocalModeControls()) modes[mode[0]] = @([p localModeEnabled:mode[0]]);
                return modes;
            },
            LayoutKey : ^id(MSIMEAppearancePreferences *p) { return @(p.vertical); },
            PageSizeKey : ^id(MSIMEAppearancePreferences *p) { return @(p.pageSize); },
            FontKey : ^id(MSIMEAppearancePreferences *p) { return @(p.fontSize); },
            PreeditFontKey : ^id(MSIMEAppearancePreferences *p) { return @(p.preeditFontSize); },
            CandidateScaleKey : ^id(MSIMEAppearancePreferences *p) { return @(p.candidateScalePercent); },
            CandidateOpacityKey : ^id(MSIMEAppearancePreferences *p) { return @(p.candidateOpacityPercent); },
            CandidateCornerRadiusKey : ^id(MSIMEAppearancePreferences *p) { return p.candidateCornerRadius ?: NSNull.null; },
            CandidatePreeditKey : ^id(MSIMEAppearancePreferences *p) { return @(p.showsCandidatePreedit); },
            CandidateFollowCursorKey : ^id(MSIMEAppearancePreferences *p) { return @(p.candidateFollowCursor); },
            InputModeHUDKey : ^id(MSIMEAppearancePreferences *p) { return @(p.inputModeHUD); },
            FontFamilyKey : ^id(MSIMEAppearancePreferences *p) { return p.fontFamily ?: NSNull.null; },
            CandidateEnglishFontKey : ^id(MSIMEAppearancePreferences *p) { return p.candidateEnglishFont ?: NSNull.null; },
            FallbackFontsKey : ^id(MSIMEAppearancePreferences *p) { return p.fallbackFonts ?: NSNull.null; },
            ThemeKey : ^id(MSIMEAppearancePreferences *p) { return p.themeMode ?: NSNull.null; },
            CandidateThemeKey : ^id(MSIMEAppearancePreferences *p) { return p.candidateTheme ?: NSNull.null; },
            ToolbarThemeKey : ^id(MSIMEAppearancePreferences *p) { return p.toolbarTheme ?: NSNull.null; },
            TextColorKey : ^id(MSIMEAppearancePreferences *p) { return p.candidateTextColor ?: NSNull.null; },
            GlobalThemeKey : ^id(MSIMEAppearancePreferences *p) { return p.globalTheme; },
            CustomThemeBaseKey : ^id(MSIMEAppearancePreferences *p) { return p.customThemeBase; },
            CustomCandidateSkinKey : ^id(MSIMEAppearancePreferences *p) { return p.customCandidateSkin ?: NSNull.null; },
            CandidateLearningKey : ^id(MSIMEAppearancePreferences *p) { return @(p.candidateLearningEnabled); },
            FrequencyModeKey : ^id(MSIMEAppearancePreferences *p) { return p.frequencyAdjustmentMode ?: NSNull.null; },
            FrequencyTriggerCountKey : ^id(MSIMEAppearancePreferences *p) { return @(p.frequencyTriggerCount); },
            FrequencyLinearStepKey : ^id(MSIMEAppearancePreferences *p) { return @(p.frequencyLinearStep); },
            CloudCandidatesKey : ^id(MSIMEAppearancePreferences *p) { return @(p.cloudCandidates); },
            CandidateTranslationsKey : ^id(MSIMEAppearancePreferences *p) { return @(p.candidateTranslations); },
            CandidateEnglishGlossKey : ^id(MSIMEAppearancePreferences *p) { return @(p.candidateEnglishGloss); },
            InputModeShortcutKey : ^id(MSIMEAppearancePreferences *p) { return @(p.inputModeShortcut); },
            ShiftTapShortcutKey : ^id(MSIMEAppearancePreferences *p) { return @(p.shiftTapShortcut); },
            ControlTapShortcutKey : ^id(MSIMEAppearancePreferences *p) { return @(p.controlTapShortcut); },
            ControlOptionSpaceShortcutKey : ^id(MSIMEAppearancePreferences *p) { return @(p.controlOptionSpaceShortcut); },
            CharacterSetShortcutKey : ^id(MSIMEAppearancePreferences *p) { return @(p.characterSetShortcut); },
            FullWidthShortcutKey : ^id(MSIMEAppearancePreferences *p) { return @(p.fullWidthShortcut); },
            VoiceEnabledKey : ^id(MSIMEAppearancePreferences *p) { return @(p.voiceInputEnabled); },
            VoiceLanguageKey : ^id(MSIMEAppearancePreferences *p) { return p.voiceLanguage ?: NSNull.null; },
            VoiceSoundKey : ^id(MSIMEAppearancePreferences *p) { return @(p.voiceSoundEnabled); },
            VoiceMuteSystemAudioKey : ^id(MSIMEAppearancePreferences *p) { return @(p.voiceMuteSystemAudio); },
            VoiceStreamInlinePreeditKey : ^id(MSIMEAppearancePreferences *p) { return @(p.voiceStreamInlinePreedit); },
            VoiceHotkeyCtrlF9Key : ^id(MSIMEAppearancePreferences *p) { return @(p.voiceHotkeyCtrlF9); },
            VoiceHotkeyHoldSpaceKey : ^id(MSIMEAppearancePreferences *p) { return @(p.voiceHotkeyHoldSpace); },
            VoiceHotkeyRightAltKey : ^id(MSIMEAppearancePreferences *p) { return @(p.voiceHotkeyRightAlt); },
            VoiceHotkeyCtrlCommandKey : ^id(MSIMEAppearancePreferences *p) { return @(p.voiceHotkeyCtrlCommand); },
            VoiceHotkeyCtrlOptionKey : ^id(MSIMEAppearancePreferences *p) { return @(p.voiceHotkeyCtrlOption); },
            PageShortcutKey : ^id(MSIMEAppearancePreferences *p) { return @(p.pageShortcut); },
            NavigationKey : ^id(MSIMEAppearancePreferences *p) {
                NSMutableDictionary<NSString *, NSNumber *> *bindings = [NSMutableDictionary dictionary];
                for (NSArray<NSString *> *entry in NavigationControls()) bindings[entry[0]] = @([p navigationEnabled:entry[0]]);
                return bindings;
            },
            WordCharacterKey : ^id(MSIMEAppearancePreferences *p) { return p.wordCharacterOptions ?: NSNull.null; },
            FloatingToolbarKey : ^id(MSIMEAppearancePreferences *p) { return @(p.floatingToolbarEnabled); },
            FloatingToolbarOptionsKey : ^id(MSIMEAppearancePreferences *p) {
                return @{@"english_mode" : @(p.floatingToolbarEnglishMode), @"input_scheme" : @(p.floatingToolbarInputScheme),
                         @"punctuation" : @(p.floatingToolbarPunctuation),
                         @"fullwidth" : @(p.floatingToolbarFullWidth), @"character_set" : @(p.floatingToolbarCharacterSet),
                         @"emoji" : @(p.floatingToolbarEmoji), @"handwriting" : @(p.floatingToolbarHandwriting),
                         @"screen_keyboard" : @(p.floatingToolbarScreenKeyboard), @"voice" : @(p.floatingToolbarVoice),
                         @"settings" : @(p.floatingToolbarSettings), @"scale_percent" : @(p.floatingToolbarScalePercent),
                         @"font_size" : @(p.floatingToolbarFontSize)};
            },
        } mutableCopy];
        // The six colours of CandidateColorControls(), read through the property name that table already carries, so that adding a seventh colour there does not need a line here as well.
        for (NSArray<NSString *> *entry in CandidateColorControls()) {
            NSString *property = entry[0];
            table[entry[2]] = ^id(MSIMEAppearancePreferences *p) { return [p valueForKey:property] ?: NSNull.null; };
        }
        probes = table;
    });
    return probes;
}

/// An NSUserDefaults with nothing in it and nowhere to write.
///
/// A real one built over a private suite would not do: its search list still carries this process's own application domain, so it would read back every value this host has stored, which is the one thing a default has to be free of. The three methods below are the primitive ones — the typed accessors are all written in terms of them — so overriding them is enough to make the whole class answer out of a dictionary that starts empty and stays that way.
@interface MSIMEUntouchedDefaults : NSUserDefaults
@end
@implementation MSIMEUntouchedDefaults {
    NSMutableDictionary<NSString *, id> *_values;
}
- (instancetype)init {
    self = [super initWithSuiteName:nil];
    if (self) _values = [NSMutableDictionary dictionary];
    return self;
}
- (id)objectForKey:(NSString *)key { return _values[key]; }
- (void)setObject:(id)value forKey:(NSString *)key {
    if (value == nil) [_values removeObjectForKey:key]; else _values[key] = value;
}
- (void)removeObjectForKey:(NSString *)key { [_values removeObjectForKey:key]; }
@end

/// What every probe reads on a machine that has never changed a setting, which is the only thing a stored or pushed value has to be compared against to know whether it is worth offering to undo.
///
/// The answers come from the accessors themselves, asked of a preferences object over empty storage, rather than from a second list of default literals written beside them: a hand-kept list of defaults is the shape these lists were already in, and drifting apart is what they already did. That object is built with -initSilentlyWithDefaults: rather than the ordinary initialiser, because resolving the selected skin ends in -preferencesChanged and an object built only to be read must not tell the host its appearance changed: the notification reaches every live MSIMEInputController, whose -appearanceChanged: cancels cloud candidates and resets gloss state. It surfaced as TestOfflineTargetGlosses failing on all three macOS CI jobs and on no local run, because the tests that trigger this path run before it in the same process and only there.
static NSDictionary<NSString *, id> *DefaultSettingValues() {
    static NSDictionary<NSString *, id> *values;
    static dispatch_once_t once;
    dispatch_once(&once, ^{
        MSIMEAppearancePreferences *untouched =
            [[MSIMEAppearancePreferences alloc] initSilentlyWithDefaults:[MSIMEUntouchedDefaults new]];
        NSMutableDictionary<NSString *, id> *defaults = [NSMutableDictionary dictionary];
        [SettingProbes() enumerateKeysAndObjectsUsingBlock:^(NSString *key, MSIMESettingProbe probe, BOOL *stop) {
            (void)stop;
            defaults[key] = probe(untouched);
        }];
        values = defaults;
    });
    return values;
}

// Layout primitives migrated from the upstream preferences window: MSIME-Apple develop
// cd36eba4d2572f747785450959332c7f68f4715c, platforms/macos/PreferencesWindowController.mm.
// A sidebar of grouped navigation buttons drives a page container; each page is a scroll view
// whose content is section labels above bordered cards of label/control rows. The page set, the
// cards, and which setting sits on which page are upstream's and stay that way.
//
// The metrics are not upstream's any more. Upstream sizes this window like a touch surface — 42pt
// sidebar rows at 16pt, 48pt preference rows at 15pt, a 24pt page title — which fits three cards
// on an 800pt-tall window and reads nothing like System Settings sitting next to it. The values
// below are the AppKit standards instead: 13pt body, 28pt sidebar rows, 30pt preference rows. The
// colours come from the system — controlAccentColor, windowBackgroundColor and the sidebar
// material — rather than the fixed sRGB greys and the one hard-coded purple upstream draws with,
// so the window follows the user's accent colour and both appearances on its own.
//
// The client README still pins b637828e for this window. That commit predates the sidebar — it
// had an NSToolbar — so it is not what ships today; the remote default branch is authoritative
// here, per AGENTS.md.

namespace {
using namespace msime::mac::layout;
/// The pages of this window, in the one order that is both the sidebar's and `_preferencePages`'s.
///
/// Four pages are named from outside the method that builds them — the skin browser builds itself on
/// entry, the voice form reloads, the 关于 page asks the update controller, the account page attaches
/// a view owned by the Swift backend — and those names used to be integer literals written beside
/// the array. Reordering the array moved the pages and left the literals pointing at whatever had
/// taken their place, which is a class of mistake a name cannot make. -loadWindow asserts that this
/// enum and the array it numbers are still the same length.
typedef NS_ENUM(NSInteger, MSIMESettingsPage) {
    MSIMESettingsPageInputScheme = 0,
    MSIMESettingsPageInputHabits,
    MSIMESettingsPageKeys,
    MSIMESettingsPageVoice,
    MSIMESettingsPageCandidateWindow,
    MSIMESettingsPageSkin,
    MSIMESettingsPageStatusBar,
    MSIMESettingsPageDictionary,
    MSIMESettingsPageAccount,
    MSIMESettingsPageSupport,
    MSIMESettingsPageAbout,
    MSIMESettingsPageCount,
};
/// The account page hosts a view owned by the Swift backend, so showing and leaving it has to
/// attach and detach that view.
constexpr NSInteger kAccountPageIndex = MSIMESettingsPageAccount;
/// The 皮肤 page is the skin browser, so the native fallback for the shared skin route shows it.
constexpr NSInteger kSkinPageIndex = MSIMESettingsPageSkin;
/// The voice form is also reachable from the input method's toolbar, so the page reloads on entry.
constexpr NSInteger kVoicePageIndex = MSIMESettingsPageVoice;
/// The 关于 page reads the update controller, whose answers — the version, whether automatic checks
/// are on, whether this build can check at all — are about the machine rather than about a stored
/// preference, so the page asks them again every time it is entered.
constexpr NSInteger kAboutPageIndex = MSIMESettingsPageAbout;
/// The page the window was last left on, remembered by name rather than by index: the order of the sidebar changes from version to version, so a stored index points at a different page in the next one, whereas a stored name is either a page this version has or it is not — and if it is not, the window opens on the first page.
NSString *const LastSettingsPageKey = @"MSIMEClientSettingsLastPage";
}  // namespace

/// Scroll views lay an unflipped document view out from the bottom, which would park a short
/// page against the bottom edge instead of under the title.
@interface MSIMEPreferencesDocumentView : NSView
@end
@implementation MSIMEPreferencesDocumentView
- (BOOL)isFlipped { return YES; }
@end

/// One row of the sidebar source list: a group heading when it has children, one page when it does
/// not. The outline view holds these rather than the pages themselves, so that the order the sidebar
/// reads in and the order `_preferencePages` is built in stay independent of one another.
@interface MSIMESettingsSidebarItem : NSObject
@property(nonatomic, copy) NSString *title;
@property(nonatomic, copy) NSString *symbolName;
@property(nonatomic) NSInteger pageIndex;
@property(nonatomic, copy) NSArray<MSIMESettingsSidebarItem *> *children;
@end
@implementation MSIMESettingsSidebarItem
@end

/// One section of one page: the heading the user sees, the preference keys the controls under that
/// heading write, and the link that puts those keys back to their defaults.
///
/// It is the window's single answer to which settings belong together, and that is what keeps the
/// two halves of "restore defaults" from drifting apart the way they had: -restorableKeys is the
/// union of these lists rather than a second list kept by hand beside them, so a key can no longer
/// be restorable from nowhere, and a section can no longer name a key whose control is on another
/// page — or, as the per-page list did for two of them, a key with no control anywhere.
@interface MSIMESettingsSection : NSObject
@property(nonatomic, copy) NSString *title;
@property(nonatomic, copy) NSArray<NSString *> *keys;
/// For a key whose stored dictionary is shared with another section, the entries inside it this section owns; a key that is absent from this map is owned whole. 工具栏缩放 and 工具栏字号 sit in MSIMEClientFloatingToolbarOptions beside the nine component choices, so 显示与组件 restoring that key restored two controls it does not contain, under an alert that promises 「其它设置不受影响」.
@property(nonatomic, copy) NSDictionary<NSString *, NSArray<NSString *> *> *fields;
@property(nonatomic, strong) NSButton *restoreLink;
@end
@implementation MSIMESettingsSection
@end

/// The toolbar items this window owns. The toggle, the flexible space and the tracking separator
/// are the system's, so only the search field and the overflow menu need names of their own.
static NSToolbarItemIdentifier const MSIMESettingsSearchItemIdentifier = @"MSIMESettingsSearchItem";
static NSToolbarItemIdentifier const MSIMESettingsSeparatorItemIdentifier = @"MSIMESettingsSidebarSeparator";
static NSToolbarItemIdentifier const MSIMESettingsMoreItemIdentifier = @"MSIMESettingsMoreItem";

/// The detail half of the split view: the page surface the cards are drawn on, and the one place in
/// the window that answers ⌘F.
///
/// The surface is drawn rather than left to the window because the cards need a page to be lifted
/// off and AppKit gives the two halves of the window the same grey — MSIMESettingsSurfaceColor() has
/// the measurements. Drawing it here rather than under the whole window also keeps the sidebar on
/// the system sidebar material the split view item gives it.
///
/// ⌘F is the window's, not the process's. It used to be registered as 编辑 ▸ 查找设置 in
/// `NSApp.mainMenu`, which is a menu bar this process does not draw — Info.plist.in sets
/// LSBackgroundOnly — and a key equivalent that fires wherever the app happens to be, including
/// while the dictionary window is key. A key equivalent handled inside the window fires only for
/// the window it belongs to.
@interface MSIMESettingsDetailView : NSView
@property(nonatomic, weak) id searchTarget;
@property(nonatomic) SEL searchAction;
@end
@implementation MSIMESettingsDetailView
- (void)drawRect:(NSRect)rect {
    [MSIMESettingsSurfaceColor() setFill];
    NSRectFill(rect);
}
- (BOOL)performKeyEquivalent:(NSEvent *)event {
    const NSEventModifierFlags flags = event.modifierFlags & NSEventModifierFlagDeviceIndependentFlagsMask;
    if (flags == NSEventModifierFlagCommand && [event.charactersIgnoringModifiers isEqualToString:@"f"] &&
        self.searchTarget != nil && self.searchAction != nullptr)
        return [NSApp sendAction:self.searchAction to:self.searchTarget from:self];
    return [super performKeyEquivalent:event];
}
@end

/// A scheme choice: the radio on the left, its scheme-specific popup trailing and disabled until
/// that scheme is the selected one.
static NSView *SchemeChoiceRow(NSButton *choice, NSView *accessory) {
    NSView *row = [[NSView alloc] initWithFrame:NSZeroRect];
    choice.translatesAutoresizingMaskIntoConstraints = NO;
    [row addSubview:choice];
    NSMutableArray<NSLayoutConstraint *> *constraints = [NSMutableArray arrayWithArray:@[
        [row.heightAnchor constraintEqualToConstant:kRowHeight],
        [choice.leadingAnchor constraintEqualToAnchor:row.leadingAnchor],
        [choice.centerYAnchor constraintEqualToAnchor:row.centerYAnchor],
    ]];
    if (accessory == nil) {
        [constraints addObject:[choice.trailingAnchor constraintLessThanOrEqualToAnchor:row.trailingAnchor]];
    } else {
        accessory.translatesAutoresizingMaskIntoConstraints = NO;
        [row addSubview:accessory];
        [constraints addObjectsFromArray:@[
            [choice.trailingAnchor constraintLessThanOrEqualToAnchor:accessory.leadingAnchor constant:-12.0],
            [accessory.trailingAnchor constraintEqualToAnchor:row.trailingAnchor],
            [accessory.centerYAnchor constraintEqualToAnchor:row.centerYAnchor],
            [accessory.widthAnchor constraintEqualToConstant:160.0],
        ]];
    }
    [NSLayoutConstraint activateConstraints:constraints];
    return row;
}

/// A page is its summary and then its cards. The summary used to be assigned to accessibility help
/// and never drawn, which made thirteen written sentences — including the only one that says how
/// the extended input modes are triggered — visible to VoiceOver and to nobody else. It stays on
/// accessibilityHelp as well, because the page is one accessibility element and its help is where a
/// screen reader looks for what the page is for.
///
/// The page's name is not drawn here any more: the unified toolbar's title says which page is in
/// front of the user, and a 20pt heading under it repeated it word for word.
static NSScrollView *PreferencesPage(NSString *title, NSString *summary, NSArray<NSView *> *content) {
    NSScrollView *page = [[NSScrollView alloc] initWithFrame:NSZeroRect];
    page.translatesAutoresizingMaskIntoConstraints = NO;
    page.hasVerticalScroller = YES;
    page.autohidesScrollers = YES;
    page.drawsBackground = NO;
    page.accessibilityLabel = title;
    page.accessibilityHelp = summary;
    NSTextField *summaryLabel = MSIMEDetailLabel(summary);
    // A page summary is a paragraph the user reads once on arriving, not the aside a row's detail
    // line is, so it is set at the body size rather than the detail one.
    summaryLabel.font = [NSFont systemFontOfSize:kBodyFontSize];
    NSStackView *stack = [NSStackView stackViewWithViews:@[summaryLabel]];
    stack.orientation = NSUserInterfaceLayoutOrientationVertical;
    stack.alignment = NSLayoutAttributeLeading;
    stack.distribution = NSStackViewDistributionFill;
    stack.spacing = 10.0;
    NSView *previous = nil;
    for (NSView *view in content) {
        [stack addArrangedSubview:view];
        [view.widthAnchor constraintEqualToAnchor:stack.widthAnchor].active = YES;
        // A section heading introduces the card beneath it, so it sits close to that card and away
        // from whatever came before. One even spacing throughout makes the page a single
        // undifferentiated column, which is how upstream's 18pt everywhere reads.
        if (previous != nil && [view.identifier isEqual:MSIMESettingsSectionIdentifier])
            [stack setCustomSpacing:20.0 afterView:previous];
        previous = view;
    }
    [stack setCustomSpacing:20.0 afterView:summaryLabel];
    stack.translatesAutoresizingMaskIntoConstraints = NO;
    NSView *document = [[MSIMEPreferencesDocumentView alloc] initWithFrame:NSZeroRect];
    document.translatesAutoresizingMaskIntoConstraints = NO;
    page.documentView = document;
    [document addSubview:stack];
    [NSLayoutConstraint activateConstraints:@[
        [document.widthAnchor constraintEqualToAnchor:page.contentView.widthAnchor],
        [document.heightAnchor constraintGreaterThanOrEqualToAnchor:page.contentView.heightAnchor],
        // The document opts out of autoresizing, so its origin needs pinning too: width and
        // height alone leave its position ambiguous.
        [document.leadingAnchor constraintEqualToAnchor:page.contentView.leadingAnchor],
        [document.topAnchor constraintEqualToAnchor:page.contentView.topAnchor],
        [stack.leadingAnchor constraintEqualToAnchor:document.leadingAnchor constant:kPageMargin],
        [stack.trailingAnchor constraintEqualToAnchor:document.trailingAnchor constant:-kPageMargin],
        // The page itself is pinned under the titlebar's safe area, so what is left here is the page
        // margin. The 46pt that used to sit here was the height of the traffic lights, measured by
        // hand; it survived the window growing a toolbar as a title laid out at y = -59.
        [stack.topAnchor constraintEqualToAnchor:document.topAnchor constant:kPageMargin],
        [stack.bottomAnchor constraintLessThanOrEqualToAnchor:document.bottomAnchor constant:-kPageMargin],
    ]];
    NSLayoutConstraint *height = [document.heightAnchor constraintEqualToAnchor:page.contentView.heightAnchor];
    height.priority = NSLayoutPriorityDefaultLow;
    height.active = YES;
    return page;
}

/// How a Chinese setting name is typed on a Latin keyboard: the full spelling and its initials, both without tone marks. 皮肤 answers to `pifu` and to `pf`.
///
/// Transliterated rather than looked up in a table written beside the names. ICU already carries the mapping, and a hand-kept table would have to be extended by whoever next adds a setting — which is exactly the kind of second list this window has spent several commits merging back into the first.
static NSArray<NSString *> *PinyinSpellings(NSString *text) {
    NSMutableString *romanized = [text mutableCopy];
    CFStringTransform((__bridge CFMutableStringRef)romanized, NULL, kCFStringTransformMandarinLatin, NO);
    CFStringTransform((__bridge CFMutableStringRef)romanized, NULL, kCFStringTransformStripDiacritics, NO);
    NSMutableString *full = [NSMutableString string];
    NSMutableString *initials = [NSMutableString string];
    for (NSString *syllable in [romanized componentsSeparatedByCharactersInSet:NSCharacterSet.whitespaceAndNewlineCharacterSet]) {
        if (syllable.length == 0) continue;
        [full appendString:syllable.lowercaseString];
        [initials appendString:[syllable substringToIndex:1].lowercaseString];
    }
    return @[full, initials];
}

/// One searchable setting: what it is called, the other words it answers to, where it lives, and the row to scroll to and flash once it is picked. Eleven pages of a hundred-odd switches with no way to search is the window's biggest usability gap; nothing else here changes how long it takes to find one.
///
/// A page registers these as it builds its cards. Reading them back off the finished view tree instead — which is what this window did — cannot tell the name of a setting from the value one of its controls happens to be showing, because NSPopUpButton is an NSButton and its title is whatever is selected: 「12 pt」, 「横向排列」 and 「蓝天小雨点」 were all indexed as settings, and each one went stale the moment the user changed it. Nor can a walk see the two pages that have no rows of this window's own, 皮肤 and 账号, which were unsearchable altogether.
@interface MSIMESettingsSearchEntry : NSObject
@property(nonatomic, copy) NSString *title;
/// The heading the row sits under, shown in the result's breadcrumb and matched against as well: a user who remembers 「配色」 but not 「候选悬停色」 still gets there.
@property(nonatomic, copy) NSString *sectionTitle;
@property(nonatomic) NSInteger pageIndex;
/// The words this setting is also known by — the name it used to have, the thing it is usually called, the name another platform gives it.
@property(nonatomic, copy) NSArray<NSString *> *synonyms;
@property(nonatomic, copy) NSString *pinyin;
@property(nonatomic, copy) NSString *initials;
/// The row to scroll to, or nil for a page that has no row of this window's own to land on; see -registerSearchKeywords:section:onPage:row:.
@property(nonatomic, weak) NSView *row;
@end
@implementation MSIMESettingsSearchEntry
/// How well this entry answers a query, lower being better, or NSNotFound for no answer at all. The order is what a user expects of a settings search: the setting whose name begins with what was typed, then the one whose name contains it, then the one whose pinyin does, then the ones reached through a synonym or through the name of the section they are in.
- (NSUInteger)rankForQuery:(NSString *)query {
    NSString *folded = query.lowercaseString;
    const NSRange inTitle = [self.title rangeOfString:query options:NSCaseInsensitiveSearch];
    if (inTitle.location == 0) return 0;
    if (inTitle.location != NSNotFound) return 1;
    if ([self.pinyin hasPrefix:folded] || [self.initials hasPrefix:folded]) return 2;
    if ([self.pinyin containsString:folded] || [self.initials containsString:folded]) return 3;
    for (NSString *synonym in self.synonyms)
        if ([synonym rangeOfString:query options:NSCaseInsensitiveSearch].location != NSNotFound) return 4;
    // Only when there is a section to match against: -rangeOfString: sent to nil answers {0, 0}, and a location of 0 is a match at the start of the string, so a setting under no heading at all would come back as an answer to every query ever typed.
    if (self.sectionTitle.length > 0 &&
        [self.sectionTitle rangeOfString:query options:NSCaseInsensitiveSearch].location != NSNotFound)
        return 5;
    return NSNotFound;
}
@end

@interface MSIMEAppearancePreferences () <NSToolbarDelegate, NSOutlineViewDataSource, NSOutlineViewDelegate, NSTableViewDataSource, NSTableViewDelegate, NSTextFieldDelegate>
@end

@implementation MSIMEAppearancePreferences {
    NSUserDefaults *_defaults;
    NSArray<NSView *> *_preferencePages;
    NSArray<NSString *> *_pageTitles;
    NSArray<NSString *> *_pageIdentifiers;
    NSSplitViewController *_splitViewController;
    NSSplitViewItem *_sidebarSplitItem;
    NSOutlineView *_sidebarOutline;
    NSScrollView *_sidebarScroll;
    NSArray<MSIMESettingsSidebarItem *> *_sidebarGroups;
    /// Selecting a row shows a page, and showing a page selects its row; without this the second
    /// half of that pair would answer the first.
    BOOL _updatingSidebarSelection;
    /// Whether a search is what opened the sidebar, so that clearing the field gives the user back
    /// the collapsed sidebar they had rather than keeping one they never asked for.
    BOOL _sidebarOpenedForSearch;
    /// Whether the window has been on screen. A page's entry work is owed to a user looking at it,
    /// and the page restored inside -loadWindow is not one yet.
    BOOL _windowHasAppeared;
    NSSearchToolbarItem *_searchToolbarItem;
    NSSearchField *_searchField;
    NSScrollView *_searchResultsScroll;
    NSStackView *_searchResultsStack;
    /// Every setting this window can take the user to, registered by the page that built its row.
    NSMutableArray<MSIMESettingsSearchEntry *> *_searchEntries;
    /// The settings registered since the last page was assembled. A row knows its own name as it is built but not yet which page will hold it or which heading it will end up under, because the heading that says so is written after the card; -page:title:summary:content: is where the two halves meet.
    NSMutableArray<MSIMESettingsSearchEntry *> *_pendingSearchEntries;
    /// The heading each section header view was made for, so that the page assembly can read the section a card belongs to off the view that introduces it.
    NSMapTable<NSView *, NSString *> *_sectionTitlesByHeader;
    /// The entries currently listed in the sidebar, in the order they are listed: a result carries its position here in its tag.
    NSArray<MSIMESettingsSearchEntry *> *_searchResults;
    /// Filled as the pages are built, in the order the headings are created; a section's restore
    /// link carries its index here in its tag.
    NSMutableArray<MSIMESettingsSection *> *_restorableSections;
    NSBox *_quanpinCard;
    NSBox *_shuangpinCard;
    NSBox *_wubiCard;
    NSInteger _selectedPageIndex;
    NSArray<NSButton *> *_schemeButtons;
    // 当前方案的菜单栏入口还没加入输入法列表时，输入方式卡片底部说明去哪里添加。
    NSView *_inputModeHintRow;
    NSTextField *_inputModeHintLabel;
    NSPopUpButton *_shuangpinSchemeButton;
    NSPopUpButton *_wubiSchemeButton;
    NSTextField *_versionLabel;
    NSTextField *_automaticUpdateLabel;
    NSButton *_updatePageButton;
    NSButton *_uninstallButton;
    NSButton *_removeUserDataButton;
    MSIMEUpdateController *_updateController;
    NSString *_sharedDefaultImeMode;
    NSString *_sharedImeModeScope;
    NSString *_activeModeApplication;
    BOOL _activeModeGlobal;
    NSMutableDictionary<NSString *, NSNumber *> *_applicationInputModes;
    NSNumber *_globalInputMode;
    // Applications whose rule the user has switched out of by hand. A rule says what an application starts in, so an explicit Shift+空格 has to outrank it for as long as they stay there; arriving at the application again hands it back to the rule. In memory, like the remembered mode it overrides.
    NSMutableSet<NSString *> *_inputModeRuleOverrides;
    // Set on the throwaway instance DefaultSettingValues() reads the defaults out of. That object exists to be asked questions, and an object nobody can see has no business telling the host its appearance changed.
    BOOL _silent;
    // Per-app punctuation and width toggles. Like the reference's thread compartments they live only in memory, and a missing entry means the saved starting value.
    NSMutableDictionary<NSString *, NSNumber *> *_runtimeChinesePunctuation;
    NSMutableDictionary<NSString *, NSNumber *> *_runtimeFullWidthInput;
    NSPopUpButton *_defaultImeModeButton;
    NSPopUpButton *_imeModeScopeButton;
    /// 应用例外: the rules table, the identifiers it is currently showing in the order it shows them, and the two controls whose state depends on what is selected in it. The order is held rather than derived on every call because the table asks for it once per row per redraw and a dictionary has none.
    NSTableView *_appRuleTable;
    NSArray<NSString *> *_appRuleIdentifiers;
    NSMutableDictionary<NSString *, NSString *> *_appRuleNames;
    NSButton *_appRuleRemoveButton;
    NSTextField *_appRuleStatusLabel;
    NSNumber *_sharedToolbarEnabled;
    NSMutableDictionary *_sharedToolbarOptions;
    NSButton *_toolbarEnglishModeButton;
    NSButton *_toolbarPunctuationButton;
    NSButton *_toolbarFullWidthButton;
    NSButton *_toolbarCharacterSetButton;
    NSButton *_toolbarEmojiButton;
    NSButton *_toolbarHandwritingButton;
    NSButton *_toolbarInputSchemeButton;
    NSButton *_toolbarScreenKeyboardButton;
    NSButton *_toolbarVoiceButton;
    NSButton *_toolbarSettingsButton;
    NSPopUpButton *_toolbarScaleButton;
    NSPopUpButton *_toolbarFontSizeButton;
    NSNumber *_sharedInputModeShortcut;
    NSNumber *_sharedShiftTapShortcut;
    NSNumber *_sharedControlTapShortcut;
    NSSwitch *_shiftTapShortcutToggle;
    NSSwitch *_controlTapShortcutToggle;
    NSNumber *_sharedControlOptionSpaceShortcut;
    NSSwitch *_controlOptionSpaceShortcutToggle;
    NSNumber *_sharedCharacterSetShortcut;
    NSNumber *_sharedFullWidthShortcut;
    NSSwitch *_characterSetShortcutToggle;
    NSMutableDictionary *_sharedHelpcodeOptions;
    NSMutableDictionary<NSString *, NSPopUpButton *> *_helpcodeSchemaButtons;
    NSMutableDictionary<NSString *, NSSwitch *> *_helpcodeDisplayToggles;
    NSNumber *_sharedChinesePunctuation;
    NSNumber *_sharedSmartPunctuation;
    NSNumber *_sharedSmartPunctuationRepeatToChinese;
    NSNumber *_sharedSmartPunctuationSpaceConvert;
    NSNumber *_sharedPairedPunctuation;
    NSString *_sharedPunctuationLock;
    NSSwitch *_pairedPunctuationToggle;
    NSPopUpButton *_punctuationLockButton;
    NSMutableDictionary *_sharedMixedInput;
    NSSwitch *_mixedEnglishToggle;
    NSPopUpButton *_mixedEnglishPrefixButton;
    NSSwitch *_mixedEmojiToggle;
    NSSwitch *_mixedKaomojiToggle;
    NSNumber *_sharedTraditionalOutput;
    NSNumber *_sharedFullWidthInput;
    NSNumber *_sharedCloudCandidates;
    NSSwitch *_cloudCandidatesToggle;
    NSNumber *_sharedCandidateTranslations;
    NSSwitch *_candidateTranslationsToggle;
    NSNumber *_sharedCandidateEnglishGloss;
    NSSwitch *_candidateEnglishGlossToggle;
    id _sharedTransposition;
    id _sharedNeighbor;
    NSNumber *_sharedQuanpinHelpcode;
    NSNumber *_sharedShuangpinHelpcode;
    NSNumber *_sharedVertical;
    NSNumber *_sharedCandidateFollowCursor;
    NSNumber *_sharedInputModeHUD;
    NSNumber *_sharedFontSize;
    NSString *_sharedFontFamily;
    id _sharedCandidateEnglishFont;
    NSArray<NSString *> *_sharedFallbackFonts;
    NSSwitch *_inputModeHUDToggle;
    id _sharedTextColor;
    id _sharedNumberColor;
    id _sharedAccentColor;
    id _sharedSelectedColor;
    id _sharedHoverColor;
    id _sharedSurfaceColor;
    id _sharedBorderColor;
    id _sharedGlobalTheme;
    id _sharedCustomThemeBase;
    id _sharedCustomCandidateSkin;
    NSTextField *_textColorField;
    NSColorWell *_textColorWell;
    /// The wells and 跟随皮肤 buttons of CandidateColorControls(), by the property each one sets, so
    /// that an action can find the setting it belongs to and -refreshControls can write every well
    /// back without naming six ivars.
    NSMutableDictionary<NSString *, NSColorWell *> *_candidateColorWells;
    NSMutableDictionary<NSString *, NSButton *> *_candidateColorResets;
    NSNumber *_sharedPreeditFontSize;
    NSNumber *_sharedCandidateScale;
    NSNumber *_sharedCandidateOpacity;
    /// A radius in points, or NSNull for a document that follows the skin.
    id _sharedCandidateCornerRadius;
    NSSlider *_candidateScaleSlider;
    NSTextField *_candidateScaleLabel;
    NSSlider *_candidateOpacitySlider;
    NSTextField *_candidateOpacityLabel;
    NSSlider *_candidateCornerRadiusSlider;
    NSTextField *_candidateCornerRadiusLabel;
    NSButton *_candidateCornerRadiusReset;
    NSPopUpButton *_candidateFontPresetButton;
    NSString *_sharedCandidatePreedit;
    NSNumber *_sharedPageSize;
    NSString *_sharedTheme;
    NSString *_sharedCandidateTheme;
    NSString *_sharedToolbarTheme;
    NSPopUpButton *_themeModeButton;
    NSPopUpButton *_candidateThemeButton;
    NSPopUpButton *_toolbarThemeButton;
    NSMutableDictionary *_sharedNavigation;
    NSDictionary *_sharedWordCharacter;
    NSSwitch *_wordCharacterToggle;
    NSPopUpButton *_wordCharacterKeys;
    NSMutableArray<NSButton *> *_navigationButtons;
    NSString *_sharedInputScheme;
    NSString *_lastChineseScheme;
    NSString *_sharedShuangpinProfile;
    NSNumber *_sharedShuangpinPreeditUsesRaw;
    NSString *_sharedWubiProfile;
    NSTextField *_wubiProfileLabel;
    NSNumber *_sharedWubiMixedPinyin;
    NSString *_sharedInlinePreeditStyle;
    NSMutableDictionary *_sharedLocalModes;
    NSMutableArray<NSButton *> *_localModeButtons;
    NSPopUpButton *_layoutButton;
    NSPopUpButton *_profileButton;
    NSPopUpButton *_preeditButton;
    NSPopUpButton *_fontButton;
    NSComboBox *_englishFontFamilyControl;
    NSComboBox *_fontFamilyControl;
    NSPopUpButton *_preeditFontButton;
    NSPopUpButton *_candidatePreeditButton;
    NSPopUpButton *_pageShortcutButton;
    NSPopUpButton *_pageSizeButton;
    NSURL *_skinsRoot;
    NSImage *_decorationImage;
    NSImage *_backgroundImage;
    msime::mac::ResolvedSkin _lightSkin;
    msime::mac::ResolvedSkin _darkSkin;
    msime::mac::SkinTokens _lightToolbarSkin;
    msime::mac::SkinTokens _darkToolbarSkin;
    /// What _lightSkin and _darkSkin were resolved from, so a reload that changed nothing the theme depends on does not read the skin package again.
    NSString *_resolvedThemeKey;
    std::vector<msime::mac::SkinListEntry> _skins;
    MSIMECandidatePreviewView *_preview;
    NSTextField *_previewSampleField;
    MSIMEToolbarPreviewView *_toolbarPreview;
    NSButton *_themeButton;
    MetasequoiaSkinSettingsView *_skinSettingsView;
    NSView *_skinPageContainer;
    NSView<MSIMEVoiceSettingsForm> *_voiceSettingsView;
    NSButton *_skinPageSharedEntry;
    NSString *_translationPreferencesDirectory;
    MSIMETranslationSettingsWindow *_translationWindow;
    MSIMEAISettingsWindow *_aiWindow;
    MSIMEDictionaryWindowController *_dictionaryWindow;
    NSSwitch *_inputModeShortcutToggle;
    NSSwitch *_fullWidthToggle;
    NSSwitch *_keymapToggle;
    NSSwitch *_wubiToggle;
    NSSwitch *_punctuationToggle;
    NSSwitch *_smartPunctuationToggle;
    NSSwitch *_smartPunctuationRepeatToggle;
    NSSwitch *_smartPunctuationSpaceToggle;
    NSSwitch *_traditionalOutputToggle;
    NSSwitch *_fullWidthShortcutToggle;
    NSSwitch *_voiceEnabledToggle;
    NSPopUpButton *_voiceLanguageButton;
    NSSwitch *_voiceSoundToggle;
    NSSwitch *_voiceMuteSystemAudioToggle;
    NSSwitch *_voiceStreamInlinePreeditToggle;
    NSSwitch *_voiceHotkeyCtrlF9Toggle;
    NSSwitch *_voiceHotkeyRightAltToggle;
    NSSwitch *_voiceHotkeyCtrlCommandToggle;
    NSSwitch *_voiceHotkeyCtrlOptionToggle;
    NSSwitch *_voiceHotkeyHoldSpaceToggle;
    NSSwitch *_wubiMixedPinyinToggle;
    NSSwitch *_toolbarToggle;
    NSSwitch *_transpositionToggle;
    NSSwitch *_neighborToggle;
    NSSwitch *_candidateFollowCursorToggle;
    NSSwitch *_candidateLearningToggle;
    NSPopUpButton *_frequencyModeButton;
    NSPopUpButton *_frequencyTriggerButton;
    NSPopUpButton *_frequencyStepButton;
    NSNumber *_sharedFuzzyPinyinEnabled;
    NSArray<NSString *> *_sharedFuzzyPinyinRules;
    NSNumber *_sharedCandidateLearning;
    NSString *_sharedFrequencyMode;
    NSNumber *_sharedFrequencyTriggerCount;
    NSNumber *_sharedFrequencyLinearStep;
    NSSwitch *_fuzzyPinyinToggle;
    NSMutableDictionary<NSString *, NSButton *> *_fuzzyPinyinRuleButtons;
    NSSwitch *_quanpinHelpcodeToggle;
    NSSwitch *_shuangpinHelpcodeToggle;
    /// The two sentences that name the binding already holding a key group. They are written as the
    /// window runs rather than as it is built, because which group is contested is a stored value.
    NSTextField *_wordCharacterConflictLabel;
    NSTextField *_navigationConflictLabel;
    /// The sentence under the paging preset, for the states its three items cannot name. Written as the window runs, for the same reason the two above it are.
    NSTextField *_pagingPresetLabel;
    /// The sentence above 拼音匹配, for the scheme whose candidates none of those settings reach.
    NSTextField *_pinyinMatchingSchemeLabel;
}
+ (instancetype)sharedPreferences {
    static MSIMEAppearancePreferences *preferences;
    static dispatch_once_t once;
    dispatch_once(&once, ^{ preferences = [[self alloc] initWithDefaults:NSUserDefaults.standardUserDefaults]; });
    return preferences;
}
- (instancetype)initWithDefaults:(NSUserDefaults *)defaults {
    const auto root = msime::mac::DefaultSkinsRoot();
    return [self initWithDefaults:defaults skinsRoot:root.empty() ? nil : [NSURL fileURLWithPath:@(root.c_str()) isDirectory:YES]];
}
- (instancetype)initWithDefaults:(NSUserDefaults *)defaults skinsRoot:(NSURL *)root {
    self = [super initWithWindow:nil];
    if (self) {
        _defaults = defaults;
        _skinsRoot = [root copy];
        [self reloadSkins];
    }
    return self;
}
/// The same object over the same storage, minus the announcement. -reloadSkins resolves the selected skin and says so, which is right for the host's own preferences and wrong for an object built only to be asked what a value is when nothing has been set: MSIMEAppearanceDidChangeNotification reaches every live MSIMEInputController, and -appearanceChanged: cancels its cloud candidates and resets its gloss state on the way past. Silence is not an optimisation here; the notification is a lie, because nothing changed.
- (instancetype)initSilentlyWithDefaults:(NSUserDefaults *)defaults {
    self = [super initWithWindow:nil];
    if (self) {
        _defaults = defaults;
        _silent = YES;
        [self reloadSkins];
    }
    return self;
}
- (NSURL *)skinsRoot { return _skinsRoot; }
- (void)setTranslationPreferencesDirectory:(NSString *)directory {
    if ([_translationPreferencesDirectory isEqual:directory]) return;
    MSIMETranslationSettingsWindow *translationWindow = _translationWindow;
    [translationWindow close];
    [translationWindow invalidatePendingCallbacks];
    _translationWindow = nil;
    [_aiWindow close]; _aiWindow = nil;
    _translationPreferencesDirectory = [directory copy];
}
- (void)showTranslationSettings:(id)sender {
    __weak MSIMEAppearancePreferences *weakSelf = self;
    MSIMEOpenDesktopSettings(MSIMEDesktopSettingsPage::Translation, [self desktopSettingsWorkspace], ^{
        [weakSelf showNativeTranslationSettings:sender];
    });
}
- (NSWorkspace *)desktopSettingsWorkspace { return NSWorkspace.sharedWorkspace; }
- (void)showNativeTranslationSettings:(id)sender {
    if (!_translationWindow) {
        __weak MSIMEAppearancePreferences *weakSelf = self;
        _translationWindow = [[MSIMETranslationSettingsWindow alloc] initWithDirectory:_translationPreferencesDirectory saved:^(NSDictionary *preferences) {
            MSIMEAppearancePreferences *current = weakSelf;
            if (!current) return;
            [current applySharedInputPreferences:preferences];
            [[NSNotificationCenter defaultCenter] postNotificationName:MSIMETranslationPreferencesDidSaveNotification object:current userInfo:preferences];
        }];
    }
    [_translationWindow showWindow:sender];
    MSIMEPresentWindow(_translationWindow.window);
}
- (void)showAISettings:(id)sender {
    __weak MSIMEAppearancePreferences *weakSelf = self;
    MSIMEOpenDesktopSettings(MSIMEDesktopSettingsPage::AI, [self desktopSettingsWorkspace], ^{
        [weakSelf showNativeAISettings:sender];
    });
}
- (void)showNativeAISettings:(id)sender {
    if (!_aiWindow) _aiWindow = [[MSIMEAISettingsWindow alloc] initWithDirectory:_translationPreferencesDirectory saved:^(NSDictionary *preferences) {
        [[NSNotificationCenter defaultCenter] postNotificationName:MSIMEAppearanceDidChangeNotification object:self userInfo:preferences];
    }];
    [_aiWindow showWindow:sender];
    MSIMEPresentWindow(_aiWindow.window);
}
- (NSDictionary<NSString *, id> *)sharedPreferencesByMerging:(NSDictionary<NSString *, id> *)snapshot {
    if (![snapshot isKindOfClass:NSDictionary.class]) return nil;
    NSMutableDictionary *merged = [snapshot mutableCopy];
    if ([_defaults objectForKey:DefaultImeModeKey] != nil) merged[@"default_ime_mode"] = self.defaultImeMode;
    if ([_defaults objectForKey:ImeModeScopeKey] != nil) merged[@"ime_mode_scope"] = self.imeModeScope;
    // Apple exposes one switch for both a solitary Shift tap and Shift+Space.
    // Keep the legacy native keys independent when no shared snapshot exists,
    // but publish one shared value for both routes.
    if ([_defaults objectForKey:InputModeShortcutKey] != nil || [_defaults objectForKey:ShiftTapShortcutKey] != nil) {
        id existing = merged[@"keybindings"];
        NSMutableDictionary *keys = [existing isKindOfClass:NSDictionary.class] ? [existing mutableCopy] : [NSMutableDictionary dictionary];
        keys[@"switch_language_shift"] = @(([_defaults objectForKey:ShiftTapShortcutKey] != nil) ? self.shiftTapShortcut : self.inputModeShortcut);
        merged[@"keybindings"] = keys;
    }
    for (NSArray *entry in @[@[ControlTapShortcutKey, @"switch_language_ctrl", @(self.controlTapShortcut)]]) {
        if ([_defaults objectForKey:entry[0]] == nil) continue;
        id existing = merged[@"keybindings"];
        NSMutableDictionary *keys = [existing isKindOfClass:NSDictionary.class] ? [existing mutableCopy] : [NSMutableDictionary dictionary];
        keys[entry[1]] = entry[2];
        merged[@"keybindings"] = keys;
    }
    if ([_defaults objectForKey:ControlOptionSpaceShortcutKey] != nil) {
        id existing = merged[@"keybindings"];
        NSMutableDictionary *keys = [existing isKindOfClass:NSDictionary.class] ? [existing mutableCopy] : [NSMutableDictionary dictionary];
        keys[@"switch_language_ctrl_alt_space"] = @(self.controlOptionSpaceShortcut);
        merged[@"keybindings"] = keys;
    }
    if ([_defaults objectForKey:TraditionalKey] != nil)
        merged[@"traditional_chinese_output"] = @(self.traditionalOutput);
    merged[@"character_width"] = self.fullWidthInput ? @"fullwidth" : @"halfwidth";
    if ([_defaults objectForKey:CloudCandidatesKey] != nil)
        merged[@"cloud_candidates"] = @(self.cloudCandidates);
    if ([_defaults objectForKey:CandidateTranslationsKey] != nil)
        merged[@"candidate_translations"] = @(self.candidateTranslations);
    if ([_defaults objectForKey:CandidateEnglishGlossKey] != nil)
        merged[@"candidate_english_gloss"] = @(self.candidateEnglishGloss);
    if ([_defaults objectForKey:CharacterSetShortcutKey] != nil) {
        id existing = merged[@"keybindings"];
        NSMutableDictionary *keys = [existing isKindOfClass:NSDictionary.class] ? [existing mutableCopy] : [NSMutableDictionary dictionary];
        keys[@"toggle_character_set_ctrl_shift_f"] = @(self.characterSetShortcut);
        merged[@"keybindings"] = keys;
    }
    if ([_defaults objectForKey:FullWidthShortcutKey] != nil) {
        id existing = merged[@"keybindings"];
        NSMutableDictionary *keys = [existing isKindOfClass:NSDictionary.class] ? [existing mutableCopy] : [NSMutableDictionary dictionary];
        keys[@"toggle_fullwidth_option_shift_h"] = @(self.fullWidthShortcut);
        merged[@"keybindings"] = keys;
    }
    merged[@"candidate_layout"] = self.vertical ? @"vertical" : @"horizontal";
    merged[@"candidate_follow_cursor"] = @(self.candidateFollowCursor);
    merged[@"input_mode_hud"] = @(self.inputModeHUD);
    merged[@"scheme"] = self.inputScheme;
    // 切到日文、韩文、越南文或藏文时要留一条回去的路。`last_chinese_scheme` 是其他宿主在方案变化时都会写的字段——Fcitx5、IBus、iOS 和 HarmonyOS 都写——共享设置页也靠它把用户送回五笔而不是全拼。这个窗口自己也会设置方案，包括这四个，所以不写的话这个字段会停在别的界面写下的值，回去的路就指错了方案。在这四个方案之一生效期间，只要知道是从哪个方案进来的，也照样写进去，因为输入菜单的 中 也是这样离开它们的。
    if (![@[@"japanese", @"korean", @"vietnamese", @"tibetan"] containsObject:self.inputScheme] || _lastChineseScheme) merged[@"last_chinese_scheme"] = self.lastChineseScheme;
    merged[@"shuangpin_profile"] = self.shuangpinProfile;
    merged[@"shuangpin_preedit_uses_raw"] = @(self.shuangpinPreeditUsesRaw);
    merged[@"wubi_profile"] = self.wubiProfile;
    merged[@"wubi_mixed_pinyin"] = @(self.wubiMixedPinyinEnabled);
    NSMutableDictionary *qh = [merged[@"quanpin_helpcode"] mutableCopy] ?: [NSMutableDictionary dictionary];
    qh[@"enabled"] = @(self.quanpinHelpcodeEnabled);
    merged[@"quanpin_helpcode"] = qh;
    NSMutableDictionary *sh = [merged[@"shuangpin_helpcode"] mutableCopy] ?: [NSMutableDictionary dictionary];
    sh[@"enabled"] = @(self.shuangpinHelpcodeEnabled);
    merged[@"shuangpin_helpcode"] = sh;
    for (NSString *scheme in @[@"quanpin", @"shuangpin"]) {
        NSDictionary *stored = [_defaults dictionaryForKey:HelpcodeOptionsKey][scheme];
        if (![stored isKindOfClass:NSDictionary.class]) continue;
        NSMutableDictionary *target = merged[[scheme stringByAppendingString:@"_helpcode"]];
        NSDictionary *effective = [self helpcodeOptionsForScheme:scheme];
        for (NSString *key in @[@"schema", @"show_in_candidate_window"])
            if (ValidHelpcodeOption(key, stored[key])) target[key] = effective[key];
    }
    merged[@"candidate_page_size"] = @(self.pageSize);
    if ([_defaults dictionaryForKey:WordCharacterKey]) merged[@"word_character"] = [self wordCharacterOptions];
    NSDictionary *navigationOverrides = [_defaults dictionaryForKey:NavigationKey];
    if (navigationOverrides.count) {
        NSMutableDictionary *navigation = [merged[@"navigation"] mutableCopy] ?: [NSMutableDictionary dictionary];
        for (NSArray *entry in NavigationControls())
            if (LocalModeBoolean(navigationOverrides[entry[0]])) navigation[entry[0]] = @([self navigationEnabled:entry[0]]);
        merged[@"navigation"] = navigation;
    }
    merged[@"candidate_font_size"] = @(self.fontSize);
    if (_sharedFontFamily || [_defaults objectForKey:FontFamilyKey]) merged[@"candidate_font_family"] = self.fontFamily;
    if (_sharedCandidateEnglishFont || [_defaults objectForKey:CandidateEnglishFontKey])
        merged[@"candidate_english_font"] = self.candidateEnglishFont ?: (id)NSNull.null;
    // The global theme and the custom theme it may select. Each part this host has no value for at all is left as the document has it, so that merging an untouched macOS profile does not clear what some other surface set; a stored empty string is the user saying 跟随主题 (or 不使用外部皮肤) and does clear it. A clear is written as an explicit value (`system` for the base, null for the package and a picker slot) rather than by removing the key, because InputController applies this result to the document on disk with MSIMEMergePreferenceSnapshot, where a missing key keeps the old value. `custom_theme.keyboard` is the touch keyboard's and is carried through untouched.
    if (_sharedGlobalTheme || [_defaults objectForKey:GlobalThemeKey]) merged[@"global_theme"] = self.globalTheme;
    NSDictionary *existingCustom = [merged[@"custom_theme"] isKindOfClass:NSDictionary.class] ? merged[@"custom_theme"] : nil;
    NSMutableDictionary *customTheme = [existingCustom mutableCopy] ?: [NSMutableDictionary dictionary];
    if (_sharedCustomThemeBase || [_defaults objectForKey:CustomThemeBaseKey]) customTheme[@"base"] = self.customThemeBase;
    if (_sharedCustomCandidateSkin || [_defaults objectForKey:CustomCandidateSkinKey])
        customTheme[@"candidate_skin"] = self.customCandidateSkin ?: (id)NSNull.null;
    NSMutableDictionary *pickerColors = [existingCustom[@"candidate_colors"] isKindOfClass:NSDictionary.class]
                                            ? [existingCustom[@"candidate_colors"] mutableCopy]
                                            : [NSMutableDictionary dictionary];
    NSArray<NSArray<NSString *> *> *pickers =
        [@[ @[ @"candidateTextColor", @"候选文字颜色", TextColorKey, @"text" ] ] arrayByAddingObjectsFromArray:CandidateColorControls()];
    for (NSArray<NSString *> *entry in pickers) {
        if ([self valueForKey:entry[0]] == nil && [_defaults objectForKey:entry[2]] == nil) continue;
        pickerColors[entry[3]] = [self valueForKey:entry[0]] ?: (id)NSNull.null;
    }
    if (pickerColors.count) customTheme[@"candidate_colors"] = pickerColors;
    if (existingCustom || customTheme.count) merged[@"custom_theme"] = customTheme;
    // The theme keys are the settings application's own names for them, and all three are top-level
    // siblings there rather than fields of the surface they apply to.
    if (_sharedTheme || [_defaults objectForKey:ThemeKey]) merged[@"theme"] = self.themeMode;
    if (_sharedCandidateTheme || [_defaults objectForKey:CandidateThemeKey]) merged[@"candidate_theme"] = self.candidateTheme;
    if (_sharedToolbarTheme || [_defaults objectForKey:ToolbarThemeKey]) merged[@"toolbar_theme"] = self.toolbarTheme;
    if (_sharedFallbackFonts || [_defaults objectForKey:FallbackFontsKey]) merged[@"candidate_fallback_fonts"] = self.fallbackFonts;
    if (_sharedPreeditFontSize || [_defaults objectForKey:PreeditFontKey]) merged[@"candidate_preedit_font_size"] = @(self.preeditFontSize);
    // Published every time, as candidate_font_size beside them is: what this host reads is either what the document said or what the user has since set here, and a restored section has neither, which has to reach the document as 100% and as following the skin rather than leave it holding the value just undone. The shared serializer drops a value at its default, so an untouched profile writes the same document it always did. The radius is cleared with an explicit null, because MSIMEMergePreferenceSnapshot keeps the old value for a missing key.
    merged[@"candidate_scale_percent"] = @(self.candidateScalePercent);
    merged[@"candidate_opacity_percent"] = @(self.candidateOpacityPercent);
    merged[@"candidate_corner_radius"] = self.candidateCornerRadius ?: (id)NSNull.null;
    if (_sharedCandidatePreedit || [_defaults objectForKey:CandidatePreeditKey]) merged[@"candidate_preedit_style"] = self.showsCandidatePreedit ? @"pinyin" : @"empty";
    merged[@"chinese_punctuation"] = @(self.chinesePunctuation);
    merged[@"smart_punctuation"] = @(self.smartPunctuation);
    merged[@"smart_punctuation_repeat"] = @(self.smartPunctuationRepeatToChinese);
    merged[@"smart_punctuation_space_convert"] = @(self.smartPunctuationSpaceConvert);
    merged[@"paired_punctuation"] = @(self.pairedPunctuation);
    merged[@"punctuation_lock"] = self.punctuationLock;
    merged[@"mixed_input"] = @{
        @"english": @(self.mixedEnglishInput),
        @"minimum_prefix": @(self.mixedEnglishMinimumPrefix),
        @"emoji": @(self.mixedEmojiInput),
        @"kaomoji": @(self.mixedKaomojiInput)
    };
    if ([_defaults objectForKey:CandidateLearningKey] != nil || _sharedCandidateLearning != nil)
        merged[@"learning"] = @(self.candidateLearningEnabled);
    if ([_defaults objectForKey:FrequencyModeKey] != nil || [_defaults objectForKey:FrequencyTriggerCountKey] != nil ||
        [_defaults objectForKey:FrequencyLinearStepKey] != nil || _sharedFrequencyMode != nil ||
        _sharedFrequencyTriggerCount != nil || _sharedFrequencyLinearStep != nil) {
        NSMutableDictionary *frequency = [merged[@"frequency"] mutableCopy] ?: [NSMutableDictionary dictionary];
        frequency[@"mode"] = self.frequencyAdjustmentMode;
        frequency[@"trigger_count"] = @(self.frequencyTriggerCount);
        frequency[@"linear_step"] = @(self.frequencyLinearStep);
        merged[@"frequency"] = frequency;
    }
    if ([_defaults objectForKey:FuzzyPinyinKey] != nil || [_defaults objectForKey:FuzzyPinyinRulesKey] != nil ||
        _sharedFuzzyPinyinEnabled != nil || _sharedFuzzyPinyinRules != nil) {
        NSMutableDictionary *fuzzy = [merged[@"fuzzy_pinyin"] mutableCopy] ?: [NSMutableDictionary dictionary];
        fuzzy[@"enabled"] = @(self.fuzzyPinyinEnabled);
        fuzzy[@"rules"] = [self fuzzyPinyinRules];
        merged[@"fuzzy_pinyin"] = fuzzy;
    }
    NSMutableDictionary *quanpin = [merged[@"quanpin"] mutableCopy] ?: [NSMutableDictionary dictionary];
    if (LocalModeBoolean([_defaults objectForKey:TranspositionKey]))
        quanpin[@"autocorrect_transposition"] = _sharedTransposition ?: @(self.autocorrectTransposition);
    if (LocalModeBoolean([_defaults objectForKey:NeighborKey]))
        quanpin[@"autocorrect_neighbor"] = _sharedNeighbor ?: @(self.autocorrectNeighbor);
    if (quanpin.count) merged[@"quanpin"] = quanpin;
    id existingVoice = merged[@"voice_input"];
    NSMutableDictionary *voice = [existingVoice isKindOfClass:NSDictionary.class]
        ? [existingVoice mutableCopy] : [NSMutableDictionary dictionary];
    [voice addEntriesFromDictionary:MSIMEVoicePreferencesFromDefaults(NSUserDefaults.standardUserDefaults)];
    merged[@"voice_input"] = voice;
    NSMutableDictionary *toolbar = [merged[@"floating_toolbar"] mutableCopy];
    if (!toolbar) toolbar = [NSMutableDictionary dictionary];
    toolbar[@"english_mode"] = @(self.floatingToolbarEnglishMode);
    toolbar[@"enabled"] = @(self.floatingToolbarEnabled);
    toolbar[@"punctuation"] = @(self.floatingToolbarPunctuation);
    toolbar[@"fullwidth"] = @(self.floatingToolbarFullWidth);
    toolbar[@"character_set"] = @(self.floatingToolbarCharacterSet);
    toolbar[@"emoji"] = @(self.floatingToolbarEmoji);
    toolbar[@"handwriting"] = @(self.floatingToolbarHandwriting);
    toolbar[@"input_scheme"] = @(self.floatingToolbarInputScheme);
    toolbar[@"screen_keyboard"] = @(self.floatingToolbarScreenKeyboard);
    toolbar[@"voice"] = @(self.floatingToolbarVoice);
    toolbar[@"settings"] = @(self.floatingToolbarSettings);
    toolbar[@"scale_percent"] = @(self.floatingToolbarScalePercent);
    toolbar[@"font_size"] = @(self.floatingToolbarFontSize);
    merged[@"floating_toolbar"] = toolbar;
    NSDictionary *stored = [_defaults dictionaryForKey:LocalModesKey];
    NSMutableDictionary *modes = [merged[@"local_modes"] mutableCopy] ?: [NSMutableDictionary dictionary];
    for (NSArray *entry in LocalModeControls()) {
        NSString *mode = entry[0];
        if (LocalModeBoolean(stored[mode])) modes[mode] = @([self localModeEnabled:mode]);
    }
    if (modes.count) merged[@"local_modes"] = modes;
    return merged;
}
- (BOOL)localModeEnabled:(NSString *)mode {
    if (!KnownLocalMode(mode)) return NO;
    NSNumber *value = _sharedLocalModes[mode] ?: [_defaults dictionaryForKey:LocalModesKey][mode];
    return LocalModeBoolean(value) ? value.boolValue : YES;
}
- (void)setLocalMode:(NSString *)mode enabled:(BOOL)enabled {
    if (!KnownLocalMode(mode)) return;
    NSMutableDictionary *stored = [[_defaults dictionaryForKey:LocalModesKey] mutableCopy] ?: [NSMutableDictionary dictionary];
    stored[mode] = @(enabled);
    [_defaults setObject:stored forKey:LocalModesKey];
    if (!_sharedLocalModes) _sharedLocalModes = [NSMutableDictionary dictionary];
    _sharedLocalModes[mode] = @(enabled);
    [self preferencesChanged];
}
- (void)applySharedLocalModes:(NSDictionary *)modes {
    if (![modes isKindOfClass:NSDictionary.class]) return;
    if (!_sharedLocalModes) _sharedLocalModes = [NSMutableDictionary dictionary];
    for (NSArray *entry in LocalModeControls()) {
        id value = modes[entry[0]];
        if (LocalModeBoolean(value))
            _sharedLocalModes[entry[0]] = value;
    }
    for (NSButton *button in _localModeButtons)
        button.state = [self localModeEnabled:button.identifier] ? NSControlStateValueOn : NSControlStateValueOff;
}
- (void)localModeChanged:(NSButton *)sender {
    [self setLocalMode:sender.identifier enabled:sender.state == NSControlStateValueOn];
}
- (NSImage *)decorationImage { return _decorationImage; }
- (NSImage *)backgroundImage { return _backgroundImage; }
- (msime::mac::ResolvedSkin)resolvedSkinForDark:(BOOL)dark { return dark ? _darkSkin : _lightSkin; }
- (msime::mac::SkinTokens)toolbarSkinForDark:(BOOL)dark { return dark ? _darkToolbarSkin : _lightToolbarSkin; }
- (void)reloadSkins {
    const std::filesystem::path root = _skinsRoot.fileSystemRepresentation ?: "";
    _skins = msime::mac::ListSkins(root);
    [self resolveSelectedSkin];
    [self preferencesChanged];
}
- (BOOL)applyCloudSettingsSnapshot:(NSDictionary *)values {
    if (!MSIMEValidateCloudAppearance(values)) return NO;
    const BOOL punctuationBefore = self.chinesePunctuation, widthBefore = self.fullWidthInput;
    NSInteger preset = [values[@"platform.macos.candidate_page_shortcut"] integerValue];
    NSString *pagingKey = preset == 0 ? @"minus_equal" : preset == 1 ? @"brackets" : @"page_up_down";
    if ([[self wordCharacterOptions][@"enabled"] boolValue] && [[self wordCharacterOptions][@"keys"] isEqual:pagingKey]) return NO;
    if (!MSIMEApplyCloudAppearance(values, _defaults)) return NO;
    [self rememberActiveInputMode:[values[@"platform.macos.english_input_mode"] boolValue]];
    [self applyNavigationPreset:preset];
    // Invalidate only fields represented by the legacy platform cloud snapshot.
    // Newer family, color, preedit-size and per-scheme assistance choices survive.
    _sharedFontSize = nil;
    _sharedPageSize = nil;
    _sharedVertical = nil;
    _sharedInputScheme = nil;
    _sharedShuangpinPreeditUsesRaw = nil;
    _sharedChinesePunctuation = nil;
    _sharedSmartPunctuation = nil;
    _sharedSmartPunctuationRepeatToChinese = nil;
    _sharedSmartPunctuationSpaceConvert = nil;
    _sharedTraditionalOutput = nil;
    _sharedFullWidthInput = nil;
    _sharedToolbarEnabled = nil;
    _sharedCandidateLearning = nil;
    _sharedQuanpinHelpcode = nil;
    _sharedShuangpinHelpcode = nil;
    _sharedHelpcodeOptions = nil;
    _sharedLocalModes = nil;
    _sharedGlobalTheme = nil;
    _sharedCustomThemeBase = nil;
    _sharedCustomCandidateSkin = nil;
    [self dropRuntimeOverridesUnlessPunctuation:punctuationBefore width:widthBefore];
    [self reloadSkins]; // Resolve the imported skin and publish one complete update.
    return YES;
}
- (NSDictionary *)cloudSettingsSnapshot {
    // Shared preferences can be effective without being mirrored into defaults.
    // Export the same values the native controls and host currently consume.
    NSMutableDictionary *snapshot = [MSIMECloudAppearanceSnapshot(_defaults) mutableCopy];
    snapshot[@"platform.macos.english_input_mode"] = @(self.englishMode);
    snapshot[@"platform.macos.candidate_font_size"] = @(self.fontSize);
    snapshot[@"platform.macos.candidate_page_size"] = @(self.pageSize);
    snapshot[@"platform.macos.candidate_panel_style"] = @(self.vertical ? 1 : 0);
    // The theme is a shared-document field too: the settings application's 皮肤 page writes it there and the host draws it from there without mirroring it into defaults.
    snapshot[@"platform.macos.global_theme"] = self.globalTheme;
    snapshot[@"platform.macos.custom_theme_base"] = self.customThemeBase;
    snapshot[@"platform.macos.custom_candidate_skin"] = self.customCandidateSkin ?: @"";
    // The stored preset is kept in step by -syncStoredPageShortcut, but a navigation dictionary an account pushed down never reaches storage at all, and this method's job is to export what the host is actually using; see -storedPageShortcutForCurrentBindings for what a state the three-value contract cannot name exports as.
    snapshot[@"platform.macos.candidate_page_shortcut"] = @([self storedPageShortcutForCurrentBindings]);
    NSArray *schemes = @[@"quanpin", @"shuangpin", @"wubi"];
    NSUInteger schemeIndex = [schemes indexOfObject:self.inputScheme];
    // 固定的 Apple 云端契约只认 quanpin、shuangpin 和 wubi。共享的 Tauri 快照正在用其他引擎方案（japanese、korean、cantonese、zhuyin、vietnamese、tibetan、stroke）时，保留它历来的全拼回退，而不是把 NSNotFound 序列化出去。
    snapshot[@"platform.macos.input_scheme"] = @(schemeIndex == NSNotFound ? 0 : schemeIndex);
    snapshot[@"platform.macos.quanpin_helpcode_schema"] = @([MSIMECloudHelpcodeSchemas() indexOfObject:[self helpcodeOptionsForScheme:@"quanpin"][@"schema"]]);
    snapshot[@"platform.macos.shuangpin_helpcode_schema"] = @([MSIMECloudHelpcodeSchemas() indexOfObject:[self helpcodeOptionsForScheme:@"shuangpin"][@"schema"]]);
    BOOL allLocalModes = YES;
    for (NSString *mode in MSIMECloudLocalModeKeys()) allLocalModes = allLocalModes && [self localModeEnabled:mode];
    snapshot[@"platform.macos.local_input_modes"] = @(allLocalModes);
    snapshot[@"platform.macos.shuangpin_preedit_uses_raw"] = @(self.shuangpinPreeditUsesRaw);
    snapshot[@"platform.macos.chinese_punctuation"] = @(self.chinesePunctuation);
    snapshot[@"platform.macos.traditional_chinese_output"] = @(self.traditionalOutput);
    snapshot[@"platform.macos.candidate_learning"] = @(self.candidateLearningEnabled);
    snapshot[@"platform.macos.floating_toolbar"] = @(self.floatingToolbarEnabled);
    // 本版本不同步的键不导出，规则见 CloudAppearanceSettings.h；full 什么也不去掉。
    return [MSIMENarrowCloudAppearance(snapshot, MSIMEEditionInputSchemes()) copy];
}
/// The custom theme as this window stores it, for the host resolver.
- (msime::mac::CustomTheme)customTheme {
    msime::mac::CustomTheme custom;
    custom.base = self.customThemeBase.UTF8String;
    custom.candidateSkin = self.customCandidateSkin.UTF8String ?: "";
    custom.candidateColors.text = self.candidateTextColor.UTF8String ?: "";
    custom.candidateColors.number = self.candidateNumberColor.UTF8String ?: "";
    custom.candidateColors.accent = self.candidateAccentColor.UTF8String ?: "";
    custom.candidateColors.selected = self.candidateSelectedColor.UTF8String ?: "";
    custom.candidateColors.hover = self.candidateHoverColor.UTF8String ?: "";
    custom.candidateColors.surface = self.candidateSurfaceColor.UTF8String ?: "";
    custom.candidateColors.border = self.candidateBorderColor.UTF8String ?: "";
    return custom;
}
/// Everything the resolved theme depends on besides the package files themselves, which -reloadSkins rereads.
- (NSString *)themeResolutionKey {
    return [@[
        self.globalTheme, self.customThemeBase, self.customCandidateSkin ?: @"", self.candidateTextColor ?: @"",
        self.candidateNumberColor ?: @"", self.candidateAccentColor ?: @"", self.candidateSelectedColor ?: @"",
        self.candidateHoverColor ?: @"", self.candidateSurfaceColor ?: @"", self.candidateBorderColor ?: @"",
        self.vertical ? @"vertical" : @"horizontal", _skinsRoot.path ?: @""
    ] componentsJoinedByString:@"|"];
}
- (void)resolveSelectedSkinIfChanged {
    if (![_resolvedThemeKey isEqual:[self themeResolutionKey]]) [self resolveSelectedSkin];
}
- (void)resolveSelectedSkin {
    const std::filesystem::path root = _skinsRoot.fileSystemRepresentation ?: "";
    const std::string_view layout = self.vertical ? "vertical" : "horizontal";
    const msime::mac::CustomTheme custom = [self customTheme];
    _lightSkin = msime::mac::ResolveSkin(self.globalTheme.UTF8String, custom, false, layout, root);
    _darkSkin = msime::mac::ResolveSkin(self.globalTheme.UTF8String, custom, true, layout, root);
    _lightToolbarSkin = msime::mac::ToolbarSkinTokens(_lightSkin, root);
    _darkToolbarSkin = msime::mac::ToolbarSkinTokens(_darkSkin, root);
    _resolvedThemeKey = [self themeResolutionKey];
    _decorationImage = nil;
    if (_lightSkin.decorationTopDip > 0 && !_lightSkin.decorationPath.empty()) {
        _decorationImage = [[NSImage alloc] initWithContentsOfFile:@(_lightSkin.decorationPath.c_str())];
    }
    if (!_decorationImage && _darkSkin.decorationTopDip > 0 && !_darkSkin.decorationPath.empty()) {
        _decorationImage = [[NSImage alloc] initWithContentsOfFile:@(_darkSkin.decorationPath.c_str())];
    }
    // Both modes draw the same package, so one image serves whichever of them draws a background.
    const std::string &backgroundPath = !_lightSkin.backgroundPath.empty() ? _lightSkin.backgroundPath : _darkSkin.backgroundPath;
    _backgroundImage = backgroundPath.empty() ? nil : [[NSImage alloc] initWithContentsOfFile:@(backgroundPath.c_str())];
}
- (BOOL)vertical { return _sharedVertical ? _sharedVertical.boolValue : [_defaults integerForKey:LayoutKey] == 1; }
- (BOOL)candidateFollowCursor {
    if (_sharedCandidateFollowCursor) return _sharedCandidateFollowCursor.boolValue;
    return [_defaults objectForKey:CandidateFollowCursorKey] == nil ? YES : [_defaults boolForKey:CandidateFollowCursorKey];
}
- (void)setCandidateFollowCursor:(BOOL)value {
    _sharedCandidateFollowCursor = nil;
    [_defaults setBool:value forKey:CandidateFollowCursorKey];
    [self preferencesChanged];
}
- (BOOL)inputModeHUD {
    if (_sharedInputModeHUD) return _sharedInputModeHUD.boolValue;
    return [_defaults objectForKey:InputModeHUDKey] == nil ? YES : [_defaults boolForKey:InputModeHUDKey];
}
- (void)setInputModeHUD:(BOOL)value {
    _sharedInputModeHUD = nil;
    [_defaults setBool:value forKey:InputModeHUDKey];
    [self preferencesChanged];
}
- (BOOL)candidateLearningEnabled {
    if (_sharedCandidateLearning) return _sharedCandidateLearning.boolValue;
    return [_defaults objectForKey:CandidateLearningKey] == nil ? YES : [_defaults boolForKey:CandidateLearningKey];
}
- (void)setCandidateLearningEnabled:(BOOL)value {
    _sharedCandidateLearning = nil;
    [_defaults setBool:value forKey:CandidateLearningKey];
    [self preferencesChanged];
}
- (NSString *)frequencyAdjustmentMode {
    id value = _sharedFrequencyMode ?: [_defaults objectForKey:FrequencyModeKey];
    return ValidFrequencyMode(value) ? value : @"promote";
}
- (void)setFrequencyAdjustmentMode:(NSString *)value {
    if (!ValidFrequencyMode(value)) value = @"promote";
    _sharedFrequencyMode = nil;
    [_defaults setObject:value forKey:FrequencyModeKey];
    [self preferencesChanged];
}
- (NSInteger)frequencyTriggerCount {
    id value = _sharedFrequencyTriggerCount ?: [_defaults objectForKey:FrequencyTriggerCountKey];
    return ValidFrequencyCount(value) ? [value integerValue] : 1;
}
- (void)setFrequencyTriggerCount:(NSInteger)value {
    if (value < 1 || value > 10) value = 1;
    _sharedFrequencyTriggerCount = nil;
    [_defaults setInteger:value forKey:FrequencyTriggerCountKey];
    [self preferencesChanged];
}
- (NSInteger)frequencyLinearStep {
    id value = _sharedFrequencyLinearStep ?: [_defaults objectForKey:FrequencyLinearStepKey];
    return ValidFrequencyCount(value) ? [value integerValue] : 1;
}
- (void)setFrequencyLinearStep:(NSInteger)value {
    if (value < 1 || value > 10) value = 1;
    _sharedFrequencyLinearStep = nil;
    [_defaults setInteger:value forKey:FrequencyLinearStepKey];
    [self preferencesChanged];
}
- (BOOL)fuzzyPinyinEnabled {
    if (_sharedFuzzyPinyinEnabled) return _sharedFuzzyPinyinEnabled.boolValue;
    return [_defaults objectForKey:FuzzyPinyinKey] == nil ? NO : [_defaults boolForKey:FuzzyPinyinKey];
}
- (NSArray<NSString *> *)fuzzyPinyinRules {
    id value = _sharedFuzzyPinyinRules ?: [_defaults objectForKey:FuzzyPinyinRulesKey];
    if (!ValidFuzzyPinyinRules(value)) return @[];
    NSSet *selected = [NSSet setWithArray:value];
    NSMutableArray *ordered = [NSMutableArray array];
    for (NSArray *entry in FuzzyPinyinRuleControls()) if ([selected containsObject:entry[0]]) [ordered addObject:entry[0]];
    return ordered;
}
- (BOOL)fuzzyPinyinRuleEnabled:(NSString *)rule { return [[self fuzzyPinyinRules] containsObject:rule]; }
- (void)setFuzzyPinyinEnabled:(BOOL)value {
    _sharedFuzzyPinyinEnabled = nil;
    [_defaults setBool:value forKey:FuzzyPinyinKey];
    // Match the shared PreferencesStore's first-enable behavior for the native
    // controls while retaining any explicitly pruned rule selection.
    if (value && [_defaults objectForKey:FuzzyPinyinRulesKey] == nil) {
        NSMutableArray *rules = [NSMutableArray array];
        for (NSArray *entry in FuzzyPinyinRuleControls()) [rules addObject:entry[0]];
        [_defaults setObject:rules forKey:FuzzyPinyinRulesKey];
    }
    [self preferencesChanged];
}
- (void)setFuzzyPinyinRule:(NSString *)rule enabled:(BOOL)enabled {
    if (![rule isKindOfClass:NSString.class] || !ValidFuzzyPinyinRules(@[rule])) return;
    NSMutableArray *rules = [[self fuzzyPinyinRules] mutableCopy];
    [rules removeObject:rule];
    if (enabled) [rules addObject:rule];
    _sharedFuzzyPinyinRules = nil;
    [_defaults setObject:rules forKey:FuzzyPinyinRulesKey];
    [self preferencesChanged];
}
- (void)fuzzyPinyinChanged:(NSSwitch *)sender {
    self.fuzzyPinyinEnabled = sender.state == NSControlStateValueOn;
    [self refreshControls];
}
- (void)fuzzyPinyinRuleChanged:(NSButton *)sender {
    [self setFuzzyPinyinRule:sender.identifier enabled:sender.state == NSControlStateValueOn];
}
- (BOOL)cloudCandidates { if (_sharedCloudCandidates) return _sharedCloudCandidates.boolValue; return [_defaults objectForKey:CloudCandidatesKey] == nil ? YES : [_defaults boolForKey:CloudCandidatesKey]; }
- (void)setCloudCandidates:(BOOL)value { _sharedCloudCandidates = nil; [_defaults setBool:value forKey:CloudCandidatesKey]; [self preferencesChanged]; }
- (void)resolveCloudCandidatesConsentWithPreferencesDirectory:(NSString *)directory userDataDirectory:(NSString *)userDataDirectory {
    if ([_defaults objectForKey:CloudCandidatesConsentKey] != nil) return;
    if (![directory isKindOfClass:NSString.class] || !directory.length) return;
    // Mirrors the Windows installer skipping its network page on every upgrade (it checks for config.toml, which any install that has run leaves behind): an existing choice, an existing shared preferences file, or Engine user data from an earlier run belongs to the user already, so it is kept and not asked about again. The user-data check covers a profile that typed but never changed a setting, which has neither of the other two. The decision is stored because this host rewrites preferences.json after any appearance change and the Engine fills user data as soon as a session starts, so their existence is only meaningful the first time it is looked at.
    NSFileManager *files = NSFileManager.defaultManager;
    const BOOL typedBefore = [userDataDirectory isKindOfClass:NSString.class] && userDataDirectory.length &&
        [files contentsOfDirectoryAtPath:userDataDirectory error:nil].count > 0;
    const BOOL existing = [_defaults objectForKey:CloudCandidatesKey] != nil || typedBefore ||
        [files fileExistsAtPath:[directory stringByAppendingPathComponent:@"preferences.json"]];
    [_defaults setInteger:existing ? CloudCandidatesConsentAnswered : CloudCandidatesConsentPending forKey:CloudCandidatesConsentKey];
}
- (BOOL)cloudCandidatesAnswered { return [_defaults integerForKey:CloudCandidatesConsentKey] != CloudCandidatesConsentPending; }
- (BOOL)cloudCandidatesEnabled { return self.cloudCandidatesAnswered && self.cloudCandidates; }
- (void)answerCloudCandidates:(BOOL)enabled {
    // Recorded before the value so observers of the change notification already see the answered state.
    [_defaults setInteger:CloudCandidatesConsentAnswered forKey:CloudCandidatesConsentKey];
    self.cloudCandidates = enabled;
}
- (BOOL)candidateTranslations { if (_sharedCandidateTranslations) return _sharedCandidateTranslations.boolValue; return [_defaults objectForKey:CandidateTranslationsKey] == nil ? YES : [_defaults boolForKey:CandidateTranslationsKey]; }
- (void)setCandidateTranslations:(BOOL)value { _sharedCandidateTranslations = nil; [_defaults setBool:value forKey:CandidateTranslationsKey]; [self preferencesChanged]; }
- (BOOL)candidateEnglishGloss { if (_sharedCandidateEnglishGloss) return _sharedCandidateEnglishGloss.boolValue; return [_defaults boolForKey:CandidateEnglishGlossKey]; }
- (void)setCandidateEnglishGloss:(BOOL)value { _sharedCandidateEnglishGloss = nil; [_defaults setBool:value forKey:CandidateEnglishGlossKey]; [self preferencesChanged]; }
- (BOOL)autocorrectTransposition { id value = _sharedTransposition ?: [_defaults objectForKey:TranspositionKey]; return LocalModeBoolean(value) ? [value boolValue] : YES; }
- (BOOL)autocorrectNeighbor { id value = _sharedNeighbor ?: [_defaults objectForKey:NeighborKey]; return LocalModeBoolean(value) ? [value boolValue] : YES; }
- (void)setAutocorrectTransposition:(BOOL)value { _sharedTransposition = nil; [_defaults setBool:value forKey:TranspositionKey]; [self preferencesChanged]; }
- (void)setAutocorrectNeighbor:(BOOL)value { _sharedNeighbor = nil; [_defaults setBool:value forKey:NeighborKey]; [self preferencesChanged]; }
- (BOOL)quanpinHelpcodeEnabled { if (_sharedQuanpinHelpcode) return _sharedQuanpinHelpcode.boolValue; return [_defaults objectForKey:QuanpinHelpcodeKey] ? [_defaults boolForKey:QuanpinHelpcodeKey] : YES; }
- (BOOL)shuangpinHelpcodeEnabled { if (_sharedShuangpinHelpcode) return _sharedShuangpinHelpcode.boolValue; return [_defaults objectForKey:ShuangpinHelpcodeKey] ? [_defaults boolForKey:ShuangpinHelpcodeKey] : YES; }
- (void)setQuanpinHelpcodeEnabled:(BOOL)value { _sharedQuanpinHelpcode = nil; [_defaults setBool:value forKey:QuanpinHelpcodeKey]; [self preferencesChanged]; }
- (void)setShuangpinHelpcodeEnabled:(BOOL)value { _sharedShuangpinHelpcode = nil; [_defaults setBool:value forKey:ShuangpinHelpcodeKey]; [self preferencesChanged]; }
- (void)applySharedAssistancePreferences:(NSDictionary *)preferences {
    if (![preferences isKindOfClass:NSDictionary.class]) return;
    id learning = preferences[@"learning"];
    NSDictionary *frequency = preferences[@"frequency"];
    NSDictionary *fuzzy = preferences[@"fuzzy_pinyin"];
    id quanpin = preferences[@"quanpin_helpcode"];
    id shuangpin = preferences[@"shuangpin_helpcode"];
    if (LocalModeBoolean(learning)) _sharedCandidateLearning = learning;
    if ([frequency isKindOfClass:NSDictionary.class]) {
        if (ValidFrequencyMode(frequency[@"mode"])) _sharedFrequencyMode = [frequency[@"mode"] copy];
        if (ValidFrequencyCount(frequency[@"trigger_count"])) _sharedFrequencyTriggerCount = frequency[@"trigger_count"];
        if (ValidFrequencyCount(frequency[@"linear_step"])) _sharedFrequencyLinearStep = frequency[@"linear_step"];
    }
    if ([fuzzy isKindOfClass:NSDictionary.class]) {
        if (LocalModeBoolean(fuzzy[@"enabled"])) _sharedFuzzyPinyinEnabled = fuzzy[@"enabled"];
        if (ValidFuzzyPinyinRules(fuzzy[@"rules"])) _sharedFuzzyPinyinRules = [fuzzy[@"rules"] copy];
    }
    id correction = preferences[@"quanpin"];
    if ([correction isKindOfClass:NSDictionary.class]) {
        id transposition = correction[@"autocorrect_transposition"];
        id neighbor = correction[@"autocorrect_neighbor"];
        // A missing field inherits the default, including when a new shared snapshot removes a previously explicit override.
        if (!transposition || transposition == NSNull.null || LocalModeBoolean(transposition)) _sharedTransposition = transposition ?: NSNull.null;
        if (!neighbor || neighbor == NSNull.null || LocalModeBoolean(neighbor)) _sharedNeighbor = neighbor ?: NSNull.null;
    }
    if ([quanpin isKindOfClass:NSDictionary.class] && LocalModeBoolean(quanpin[@"enabled"])) _sharedQuanpinHelpcode = quanpin[@"enabled"];
    if ([shuangpin isKindOfClass:NSDictionary.class] && LocalModeBoolean(shuangpin[@"enabled"])) _sharedShuangpinHelpcode = shuangpin[@"enabled"];
    if (!_sharedHelpcodeOptions) _sharedHelpcodeOptions = [NSMutableDictionary dictionary];
    for (NSString *scheme in @[@"quanpin", @"shuangpin"]) {
        id shared = preferences[[scheme stringByAppendingString:@"_helpcode"]];
        if (![shared isKindOfClass:NSDictionary.class]) continue;
        NSMutableDictionary *values = [_sharedHelpcodeOptions[scheme] mutableCopy] ?: [NSMutableDictionary dictionary];
        for (NSString *key in @[@"schema", @"show_in_candidate_window"])
            if (ValidHelpcodeOption(key, shared[key])) values[key] = shared[key];
        _sharedHelpcodeOptions[scheme] = values;
    }
    [self refreshControls];
}
- (NSDictionary *)helpcodeOptionsForScheme:(NSString *)scheme {
    BOOL shuangpin = [scheme isEqualToString:@"shuangpin"];
    NSMutableDictionary *values = [@{@"schema": shuangpin ? @"lantian" : @"ziranma",
        @"show_in_candidate_window": @(shuangpin)} mutableCopy];
    id stored = [_defaults dictionaryForKey:HelpcodeOptionsKey][scheme];
    if ([stored isKindOfClass:NSDictionary.class])
        for (NSString *key in values.allKeys) if (ValidHelpcodeOption(key, stored[key])) values[key] = stored[key];
    [values addEntriesFromDictionary:_sharedHelpcodeOptions[scheme] ?: @{}];
    return values;
}
- (void)setHelpcodeOption:(NSString *)key value:(id)value scheme:(NSString *)scheme {
    if (![@[@"quanpin", @"shuangpin"] containsObject:scheme] || !ValidHelpcodeOption(key, value)) return;
    NSMutableDictionary *all = [[_defaults dictionaryForKey:HelpcodeOptionsKey] mutableCopy] ?: [NSMutableDictionary dictionary];
    id existing = all[scheme];
    NSMutableDictionary *values = [existing isKindOfClass:NSDictionary.class] ? [existing mutableCopy] : [NSMutableDictionary dictionary];
    values[key] = value;
    all[scheme] = values;
    [_defaults setObject:all forKey:HelpcodeOptionsKey];
    if (!_sharedHelpcodeOptions) _sharedHelpcodeOptions = [NSMutableDictionary dictionary];
    NSMutableDictionary *shared = [_sharedHelpcodeOptions[scheme] mutableCopy] ?: [NSMutableDictionary dictionary];
    shared[key] = value;
    _sharedHelpcodeOptions[scheme] = shared;
    [self preferencesChanged];
}
- (void)helpcodeSchemaChanged:(NSPopUpButton *)sender {
    [self setHelpcodeOption:@"schema" value:sender.selectedItem.representedObject scheme:sender.identifier];
}
- (void)helpcodeDisplayChanged:(NSSwitch *)sender {
    [self setHelpcodeOption:@"show_in_candidate_window" value:@(sender.state == NSControlStateValueOn) scheme:sender.identifier];
}
// 不是本版本的方案（包括引擎不认识的值）读作本版本的默认方案，full 是全拼。
- (NSString *)inputScheme { NSString *value = _sharedInputScheme ?: [_defaults stringForKey:SchemeKey]; return MSIMEEditionOffersScheme(value) ? value : MSIMEEditionDefaultScheme(); }
- (void)setInputScheme:(NSString *)value { if (!MSIMEEditionOffersScheme(value)) value = MSIMEEditionDefaultScheme(); if (![@[@"japanese", @"korean", @"vietnamese", @"tibetan"] containsObject:self.inputScheme]) _lastChineseScheme = self.inputScheme; _sharedInputScheme = nil; [_defaults setObject:value forKey:SchemeKey]; [self preferencesChanged]; }
- (NSString *)lastChineseScheme {
    NSString *scheme = self.inputScheme;
    if (![@[@"japanese", @"korean", @"vietnamese", @"tibetan"] containsObject:scheme]) return scheme;
    return _lastChineseScheme ?: MSIMEEditionDefaultScheme();
}
- (NSString *)lastSyncedInputScheme { return [_defaults stringForKey:LastSyncedSchemeKey]; }
- (void)setLastSyncedInputScheme:(NSString *)value { [_defaults setObject:value forKey:LastSyncedSchemeKey]; }
- (NSString *)shuangpinProfile { NSString *value = _sharedShuangpinProfile ?: [_defaults stringForKey:ShuangpinProfileKey]; return [@[@"xiaohe", @"ziranma", @"shoudao", @"microsoft"] containsObject:value] ? value : @"xiaohe"; }
- (void)setShuangpinProfile:(NSString *)value { if (![@[@"xiaohe", @"ziranma", @"shoudao", @"microsoft"] containsObject:value]) value = @"xiaohe"; _sharedShuangpinProfile = nil; [_defaults setObject:value forKey:ShuangpinProfileKey]; [self preferencesChanged]; }
- (NSString *)wubiProfile { NSString *value = _sharedWubiProfile ?: [_defaults stringForKey:WubiProfileKey]; return [@[@"wubi86", @"wubi98"] containsObject:value] ? value : @"wubi86"; }
- (void)setWubiProfile:(NSString *)value { if (![@[@"wubi86", @"wubi98"] containsObject:value]) value = @"wubi86"; _sharedWubiProfile = nil; [_defaults setObject:value forKey:WubiProfileKey]; [self preferencesChanged]; }
- (BOOL)shuangpinPreeditUsesRaw { if (_sharedShuangpinPreeditUsesRaw) return _sharedShuangpinPreeditUsesRaw.boolValue; return [_defaults objectForKey:ShuangpinPreeditKey] == nil ? YES : [_defaults boolForKey:ShuangpinPreeditKey]; }
- (void)setShuangpinPreeditUsesRaw:(BOOL)value { _sharedShuangpinPreeditUsesRaw = nil; [_defaults setBool:value forKey:ShuangpinPreeditKey]; [self preferencesChanged]; }
// 从没设置过时取本版本的默认值：五笔版默认打开混拼，full 默认关闭（与 client-core 的版本默认偏好一致）。它会被合并进共享偏好文档，所以缺省值不能是一个固定的 NO。
- (BOOL)wubiMixedPinyinEnabled {
    if (_sharedWubiMixedPinyin) return _sharedWubiMixedPinyin.boolValue;
    return [_defaults objectForKey:WubiMixedPinyinKey] == nil ? MSIMEEditionWubiMixedPinyinDefault() : [_defaults boolForKey:WubiMixedPinyinKey];
}
- (void)setWubiMixedPinyinEnabled:(BOOL)value {
    _sharedWubiMixedPinyin = nil;
    [_defaults setBool:value forKey:WubiMixedPinyinKey];
    [self preferencesChanged];
}
- (MSIMEInlinePreeditStyle)inlinePreeditStyle {
    NSString *value = _sharedInlinePreeditStyle ?: @"raw";
    if ([value isEqual:@"raw"]) return MSIMEInlinePreeditStyleRaw;
    if ([value isEqual:@"empty"]) return MSIMEInlinePreeditStyleEmpty;
    return MSIMEInlinePreeditStylePinyin;
}
- (void)applySharedInputPreferences:(NSDictionary *)preferences {
    if (![preferences isKindOfClass:NSDictionary.class]) return;
    const BOOL punctuationBefore = self.chinesePunctuation, widthBefore = self.fullWidthInput;
    id defaultMode = preferences[@"default_ime_mode"], scope = preferences[@"ime_mode_scope"];
    if ([@[@"chinese", @"english"] containsObject:defaultMode]) _sharedDefaultImeMode = defaultMode;
    if ([@[@"app", @"global"] containsObject:scope]) _sharedImeModeScope = scope;
    id keys = preferences[@"keybindings"];
    if ([keys isKindOfClass:NSDictionary.class]) {
        id shift = keys[@"switch_language_shift"];
        if (LocalModeBoolean(shift)) {
            // The shared Tauri setting reaches both native Shift routes.
            _sharedInputModeShortcut = shift;
            _sharedShiftTapShortcut = shift;
        }
        if (LocalModeBoolean(keys[@"switch_language_ctrl"])) _sharedControlTapShortcut = keys[@"switch_language_ctrl"];
        id inputMode = keys[@"switch_language_ctrl_alt_space"];
        if (LocalModeBoolean(inputMode)) _sharedControlOptionSpaceShortcut = inputMode;
        id enabled = keys[@"toggle_character_set_ctrl_shift_f"];
        if (LocalModeBoolean(enabled)) _sharedCharacterSetShortcut = enabled;
        id fullWidth = keys[@"toggle_fullwidth_option_shift_h"];
        if (LocalModeBoolean(fullWidth)) _sharedFullWidthShortcut = fullWidth;
    }
    id punctuation = preferences[@"chinese_punctuation"];
    if (LocalModeBoolean(punctuation)) _sharedChinesePunctuation = punctuation;
    id smart = preferences[@"smart_punctuation"];
    if (LocalModeBoolean(smart)) _sharedSmartPunctuation = smart;
    id smartRepeat = preferences[@"smart_punctuation_repeat"];
    if (LocalModeBoolean(smartRepeat)) _sharedSmartPunctuationRepeatToChinese = smartRepeat;
    id smartSpace = preferences[@"smart_punctuation_space_convert"];
    if (LocalModeBoolean(smartSpace)) _sharedSmartPunctuationSpaceConvert = smartSpace;
    id paired = preferences[@"paired_punctuation"];
    if (LocalModeBoolean(paired)) _sharedPairedPunctuation = paired;
    id punctuationLock = preferences[@"punctuation_lock"];
    if ([@[@"follow", @"chinese", @"english"] containsObject:punctuationLock]) _sharedPunctuationLock = [punctuationLock copy];
    id mixedInput = preferences[@"mixed_input"];
    if ([mixedInput isKindOfClass:NSDictionary.class]) {
        NSMutableDictionary *values = [_sharedMixedInput mutableCopy] ?: [NSMutableDictionary dictionary];
        if (LocalModeBoolean(mixedInput[@"english"])) values[@"english"] = mixedInput[@"english"];
        if (ValidMixedPrefix(mixedInput[@"minimum_prefix"])) values[@"minimum_prefix"] = mixedInput[@"minimum_prefix"];
        if (LocalModeBoolean(mixedInput[@"emoji"])) values[@"emoji"] = mixedInput[@"emoji"];
        if (LocalModeBoolean(mixedInput[@"kaomoji"])) values[@"kaomoji"] = mixedInput[@"kaomoji"];
        _sharedMixedInput = values;
    }
    id traditional = preferences[@"traditional_chinese_output"];
    if (LocalModeBoolean(traditional)) _sharedTraditionalOutput = traditional;
    id characterWidth = preferences[@"character_width"];
    if ([characterWidth isEqual:@"fullwidth"] || [characterWidth isEqual:@"halfwidth"])
        _sharedFullWidthInput = @([characterWidth isEqual:@"fullwidth"]);
    [self dropRuntimeOverridesUnlessPunctuation:punctuationBefore width:widthBefore];
    id cloud = preferences[@"cloud_candidates"];
    if (LocalModeBoolean(cloud)) _sharedCloudCandidates = cloud;
    id translations = preferences[@"candidate_translations"];
    if (LocalModeBoolean(translations)) _sharedCandidateTranslations = translations;
    id englishGloss = preferences[@"candidate_english_gloss"];
    if (LocalModeBoolean(englishGloss)) _sharedCandidateEnglishGloss = englishGloss;
    id scheme = preferences[@"scheme"];
    id profile = preferences[@"shuangpin_profile"];
    id raw = preferences[@"shuangpin_preedit_uses_raw"];
    id wubiMixedPinyin = preferences[@"wubi_mixed_pinyin"];
    id wubiProfile = preferences[@"wubi_profile"];
    if (MSIMEEditionOffersScheme(scheme)) _sharedInputScheme = [scheme copy];
    id lastChinese = preferences[@"last_chinese_scheme"];
    if ([@[@"quanpin", @"shuangpin", @"wubi", @"cantonese", @"zhuyin", @"stroke"] containsObject:lastChinese] && MSIMEEditionOffersScheme(lastChinese)) _lastChineseScheme = [lastChinese copy];
    if ([@[@"xiaohe", @"ziranma", @"shoudao", @"microsoft"] containsObject:profile]) _sharedShuangpinProfile = [profile copy];
    if (LocalModeBoolean(raw)) _sharedShuangpinPreeditUsesRaw = raw;
    if (LocalModeBoolean(wubiMixedPinyin)) _sharedWubiMixedPinyin = wubiMixedPinyin;
    if ([@[@"wubi86", @"wubi98"] containsObject:wubiProfile]) _sharedWubiProfile = [wubiProfile copy];
    id inlinePreedit = preferences[@"tsf_preedit_style"];
    if ([@[@"raw", @"pinyin", @"empty"] containsObject:inlinePreedit]) _sharedInlinePreeditStyle = [inlinePreedit copy];
    [self refreshControls];
}
- (NSString *)defaultImeMode {
    NSString *value = _sharedDefaultImeMode ?: [_defaults stringForKey:DefaultImeModeKey];
    return [value isEqual:@"english"] ? @"english" : @"chinese";
}
- (void)setDefaultImeMode:(NSString *)value {
    if (![@[@"chinese", @"english"] containsObject:value]) return;
    _sharedDefaultImeMode = nil;
    [_defaults setObject:value forKey:DefaultImeModeKey];
    [self preferencesChanged];
}
- (NSString *)imeModeScope {
    NSString *value = _sharedImeModeScope ?: [_defaults stringForKey:ImeModeScopeKey];
    return [value isEqual:@"global"] ? @"global" : @"app";
}
- (void)setImeModeScope:(NSString *)value {
    if (![@[@"app", @"global"] containsObject:value]) return;
    _sharedImeModeScope = nil;
    [_defaults setObject:value forKey:ImeModeScopeKey];
    [self preferencesChanged];
}
- (void)activateInputModeForApplication:(NSString *)identifier {
    NSString *next = [identifier isKindOfClass:NSString.class] && identifier.length ? [identifier copy] : nil;
    // Coming back to an application from somewhere else starts it in its rule again, so the hand-made override from the last visit goes. Refocusing within the same application is not a new visit and keeps it.
    if (next != nil && ![next isEqual:_activeModeApplication]) [_inputModeRuleOverrides removeObject:next];
    _activeModeApplication = next;
    // Scope changes take effect on activation, never in the middle of typing.
    _activeModeGlobal = [self.imeModeScope isEqual:@"global"];
}
/// The rules the user has written down, with anything the stored dictionary has picked up that is not one dropped.
- (NSDictionary<NSString *, NSString *> *)applicationInputModeRules {
    NSDictionary *stored = [_defaults dictionaryForKey:AppInputModeRulesKey];
    if (![stored isKindOfClass:NSDictionary.class]) return @{};
    NSMutableDictionary<NSString *, NSString *> *rules = [NSMutableDictionary dictionary];
    for (id identifier in stored)
        if ([identifier isKindOfClass:NSString.class] && [identifier length] > 0 && ValidInputModeRule(stored[identifier]))
            rules[identifier] = [stored[identifier] copy];
    return rules;
}
/// Writes one rule, or removes it when the mode is nil. The key goes rather than being left holding an empty dictionary, so that 恢复默认值 has nothing to offer once the last rule is gone.
- (void)setInputMode:(NSString *)mode forApplication:(NSString *)identifier {
    if (![identifier isKindOfClass:NSString.class] || identifier.length == 0) return;
    if (mode != nil && !ValidInputModeRule(mode)) return;
    NSMutableDictionary<NSString *, NSString *> *rules = [[self applicationInputModeRules] mutableCopy];
    if (mode == nil) [rules removeObjectForKey:identifier]; else rules[identifier] = mode;
    // Writing a rule is the user saying what this application should be in, so it takes effect now rather than waiting for them to leave and come back.
    [_inputModeRuleOverrides removeObject:identifier];
    if (rules.count > 0) [_defaults setObject:rules forKey:AppInputModeRulesKey];
    else [_defaults removeObjectForKey:AppInputModeRulesKey];
    [self preferencesChanged];
}
- (BOOL)englishMode {
    if (!_activeModeApplication) return [_defaults boolForKey:EnglishKey];
    // Rule, then memory, then the default. A rule is what the user decided this application should start in and it is saved, so it goes on answering after -resetRememberedInputModes has thrown away what they happened to do last time; it also holds under 全局 scope, which is what makes it an exception rather than a second way of saying the same thing. What it does not outrank is the user reaching for Shift+空格 inside the application: that writes an override which stands until they leave and come back.
    NSString *rule = [self applicationInputModeRules][_activeModeApplication];
    if (rule != nil && ![_inputModeRuleOverrides containsObject:_activeModeApplication]) return [rule isEqual:@"english"];
    NSNumber *mode = _activeModeGlobal ? _globalInputMode : _applicationInputModes[_activeModeApplication];
    return mode ? mode.boolValue : [self.defaultImeMode isEqual:@"english"];
}
- (void)rememberActiveInputMode:(BOOL)english {
    if (!_activeModeApplication) return;
    if (_activeModeGlobal) _globalInputMode = @(english);
    else {
        if (!_applicationInputModes) _applicationInputModes = [NSMutableDictionary dictionary];
        _applicationInputModes[_activeModeApplication] = @(english);
    }
}
- (void)lockActiveInputMode { [self rememberActiveInputMode:self.englishMode]; }
// Punctuation and width toggles are always per app, whatever ime_mode_scope says: the reference keeps them in compartments of each UI thread. Without an active application they share one unnamed slot.
- (NSString *)runtimeInputStateKey { return _activeModeApplication ?: @""; }
- (BOOL)runtimeChinesePunctuation {
    NSNumber *value = _runtimeChinesePunctuation[[self runtimeInputStateKey]];
    return value ? value.boolValue : self.chinesePunctuation;
}
- (void)setRuntimeChinesePunctuation:(BOOL)value {
    if (!_runtimeChinesePunctuation) _runtimeChinesePunctuation = [NSMutableDictionary dictionary];
    _runtimeChinesePunctuation[[self runtimeInputStateKey]] = @(value);
}
- (BOOL)runtimeFullWidthInput {
    NSNumber *value = _runtimeFullWidthInput[[self runtimeInputStateKey]];
    return value ? value.boolValue : self.fullWidthInput;
}
- (void)setRuntimeFullWidthInput:(BOOL)value {
    if (!_runtimeFullWidthInput) _runtimeFullWidthInput = [NSMutableDictionary dictionary];
    _runtimeFullWidthInput[[self runtimeInputStateKey]] = @(value);
}
- (void)resetRuntimePunctuationForActiveApplication { [_runtimeChinesePunctuation removeObjectForKey:[self runtimeInputStateKey]]; }
- (void)resetRuntimeInputStateForActiveApplication {
    [self resetRuntimePunctuationForActiveApplication];
    [_runtimeFullWidthInput removeObjectForKey:[self runtimeInputStateKey]];
}
- (void)resetAllRuntimeInputState { _runtimeChinesePunctuation = nil; _runtimeFullWidthInput = nil; }
// A new saved starting value applies to every app at once, as re-activation does in the reference. An unchanged value keeps the toggles: the shared document is re-applied on every reload.
- (void)dropRuntimeOverridesUnlessPunctuation:(BOOL)punctuation width:(BOOL)width {
    if (self.chinesePunctuation != punctuation) _runtimeChinesePunctuation = nil;
    if (self.fullWidthInput != width) _runtimeFullWidthInput = nil;
}
// Switching to another input source ends the mode session in both scopes, as the source's TIP re-activation re-seeds default_ime_mode. The active application and scope stay, so the current activation keeps its scope.
- (void)resetRememberedInputModes {
    _globalInputMode = nil;
    [_applicationInputModes removeAllObjects];
    // An override is an observation of what the user did, not a decision they wrote down, so it goes with the rest of them and the rules are left alone.
    [_inputModeRuleOverrides removeAllObjects];
}
- (BOOL)traditionalOutput { return _sharedTraditionalOutput ? _sharedTraditionalOutput.boolValue : [_defaults boolForKey:TraditionalKey]; }
- (BOOL)fullWidthInput { return _sharedFullWidthInput ? _sharedFullWidthInput.boolValue : [_defaults boolForKey:FullWidthKey]; }
- (BOOL)chinesePunctuation { if (_sharedChinesePunctuation) return _sharedChinesePunctuation.boolValue; return [_defaults objectForKey:ChinesePunctuationKey] == nil ? YES : [_defaults boolForKey:ChinesePunctuationKey]; }
- (BOOL)smartPunctuation { return _sharedSmartPunctuation ? _sharedSmartPunctuation.boolValue : ([_defaults objectForKey:SmartPunctuationKey] == nil ? NO : [_defaults boolForKey:SmartPunctuationKey]); }
- (void)setSmartPunctuation:(BOOL)value { _sharedSmartPunctuation = nil; [_defaults setBool:value forKey:SmartPunctuationKey]; [self preferencesChanged]; }
- (BOOL)smartPunctuationRepeatToChinese { return _sharedSmartPunctuationRepeatToChinese ? _sharedSmartPunctuationRepeatToChinese.boolValue : ([_defaults objectForKey:SmartPunctuationRepeatToChineseKey] == nil ? NO : [_defaults boolForKey:SmartPunctuationRepeatToChineseKey]); }
- (void)setSmartPunctuationRepeatToChinese:(BOOL)value { _sharedSmartPunctuationRepeatToChinese = nil; [_defaults setBool:value forKey:SmartPunctuationRepeatToChineseKey]; [self preferencesChanged]; }
// Off unless asked for, matching the Windows baseline and the shared default.
- (BOOL)smartPunctuationSpaceConvert { return _sharedSmartPunctuationSpaceConvert ? _sharedSmartPunctuationSpaceConvert.boolValue : [_defaults boolForKey:SmartPunctuationSpaceConvertKey]; }
- (void)setSmartPunctuationSpaceConvert:(BOOL)value { _sharedSmartPunctuationSpaceConvert = nil; [_defaults setBool:value forKey:SmartPunctuationSpaceConvertKey]; [self preferencesChanged]; }
- (BOOL)shuangpinKeymap { return [_defaults boolForKey:KeymapKey]; }
- (BOOL)wubiAutoCommitUnique { return [_defaults boolForKey:WubiKey]; }
- (BOOL)floatingToolbarEnabled { return _sharedToolbarEnabled ? _sharedToolbarEnabled.boolValue : ([_defaults objectForKey:FloatingToolbarKey] == nil ? YES : [_defaults boolForKey:FloatingToolbarKey]); }
- (void)setFloatingToolbarEnabled:(BOOL)value { _sharedToolbarEnabled = nil; [_defaults setBool:value forKey:FloatingToolbarKey]; [self preferencesChanged]; }
- (void)applySharedToolbarVisibility:(BOOL)enabled { _sharedToolbarEnabled = @(enabled); [self refreshControls]; }
- (NSDictionary *)floatingToolbarValues {
    NSDictionary *values = _sharedToolbarOptions ?: [_defaults dictionaryForKey:FloatingToolbarOptionsKey];
    return [values isKindOfClass:NSDictionary.class] ? values : @{};
}
- (BOOL)floatingToolbarBoolean:(NSString *)key defaultValue:(BOOL)defaultValue {
    id value = [self floatingToolbarValues][key];
    return LocalModeBoolean(value) ? [value boolValue] : defaultValue;
}
- (void)setFloatingToolbarBoolean:(NSString *)key value:(BOOL)value {
    NSMutableDictionary *values = [[_defaults dictionaryForKey:FloatingToolbarOptionsKey] mutableCopy] ?: [NSMutableDictionary dictionary];
    values[key] = @(value);
    _sharedToolbarOptions = nil;
    [_defaults setObject:values forKey:FloatingToolbarOptionsKey];
    [self preferencesChanged];
}
// The 中/英 button, which the toolbar has always drawn and the merge below has always published as a
// constant true unless an account said otherwise. It is a component like the rest of them now.
- (BOOL)floatingToolbarEnglishMode { return [self floatingToolbarBoolean:@"english_mode" defaultValue:YES]; }
- (void)setFloatingToolbarEnglishMode:(BOOL)value { [self setFloatingToolbarBoolean:@"english_mode" value:value]; }
- (BOOL)floatingToolbarPunctuation { return [self floatingToolbarBoolean:@"punctuation" defaultValue:YES]; }
- (void)setFloatingToolbarPunctuation:(BOOL)value { [self setFloatingToolbarBoolean:@"punctuation" value:value]; }
- (BOOL)floatingToolbarFullWidth { return [self floatingToolbarBoolean:@"fullwidth" defaultValue:YES]; }
- (void)setFloatingToolbarFullWidth:(BOOL)value { [self setFloatingToolbarBoolean:@"fullwidth" value:value]; }
- (BOOL)floatingToolbarCharacterSet { return [self floatingToolbarBoolean:@"character_set" defaultValue:YES]; }
- (void)setFloatingToolbarCharacterSet:(BOOL)value { [self setFloatingToolbarBoolean:@"character_set" value:value]; }
// Emoji, handwriting, voice and the screen keyboard are opt-in, so the toolbar a profile that has not
// chosen gets is the compact five-button one. These match FloatingToolbarPreferences::default() in
// crates/client-core on purpose: this fallback is only reached when neither the shared snapshot nor
// this host's own dictionary carries the key, and a host that disagreed with the shared default would
// draw one toolbar while the settings page showed another.
- (BOOL)floatingToolbarEmoji { return [self floatingToolbarBoolean:@"emoji" defaultValue:NO]; }
- (void)setFloatingToolbarEmoji:(BOOL)value { [self setFloatingToolbarBoolean:@"emoji" value:value]; }
// The handwriting panel and voice buttons, which the reference's toolbar does not have.
// 切换输入方案的按钮默认开启，与 client-core 的 FloatingToolbarPreferences::default() 一致。
- (BOOL)floatingToolbarInputScheme { return [self floatingToolbarBoolean:@"input_scheme" defaultValue:YES]; }
- (void)setFloatingToolbarInputScheme:(BOOL)value { [self setFloatingToolbarBoolean:@"input_scheme" value:value]; }
- (BOOL)floatingToolbarHandwriting { return [self floatingToolbarBoolean:@"handwriting" defaultValue:NO]; }
- (void)setFloatingToolbarHandwriting:(BOOL)value { [self setFloatingToolbarBoolean:@"handwriting" value:value]; }
- (BOOL)floatingToolbarVoice { return [self floatingToolbarBoolean:@"voice" defaultValue:NO]; }
- (void)setFloatingToolbarVoice:(BOOL)value { [self setFloatingToolbarBoolean:@"voice" value:value]; }
- (BOOL)floatingToolbarScreenKeyboard { return [self floatingToolbarBoolean:@"screen_keyboard" defaultValue:NO]; }
- (void)setFloatingToolbarScreenKeyboard:(BOOL)value { [self setFloatingToolbarBoolean:@"screen_keyboard" value:value]; }
- (BOOL)floatingToolbarSettings { return [self floatingToolbarBoolean:@"settings" defaultValue:YES]; }
- (void)setFloatingToolbarSettings:(BOOL)value { [self setFloatingToolbarBoolean:@"settings" value:value]; }
- (NSInteger)floatingToolbarScalePercent {
    id value = [self floatingToolbarValues][@"scale_percent"];
    return ValidToolbarScale(value) ? [value integerValue] : 100;
}
- (void)setFloatingToolbarScalePercent:(NSInteger)value {
    if (!ValidToolbarScale(@(value))) value = 100;
    NSMutableDictionary *values = [[_defaults dictionaryForKey:FloatingToolbarOptionsKey] mutableCopy] ?: [NSMutableDictionary dictionary];
    values[@"scale_percent"] = @(value); _sharedToolbarOptions = nil;
    [_defaults setObject:values forKey:FloatingToolbarOptionsKey]; [self preferencesChanged];
}
- (NSInteger)floatingToolbarFontSize {
    id value = [self floatingToolbarValues][@"font_size"];
    return ValidToolbarFontSize(value) ? [value integerValue] : 24;
}
- (void)setFloatingToolbarFontSize:(NSInteger)value {
    if (!ValidToolbarFontSize(@(value))) value = 24;
    NSMutableDictionary *values = [[_defaults dictionaryForKey:FloatingToolbarOptionsKey] mutableCopy] ?: [NSMutableDictionary dictionary];
    values[@"font_size"] = @(value); _sharedToolbarOptions = nil;
    [_defaults setObject:values forKey:FloatingToolbarOptionsKey]; [self preferencesChanged];
}
- (void)applySharedToolbarPreferences:(NSDictionary *)preferences {
    if (![preferences isKindOfClass:NSDictionary.class]) return;
    NSDictionary *toolbar = preferences[@"floating_toolbar"];
    if (![toolbar isKindOfClass:NSDictionary.class]) return;
    if (!_sharedToolbarOptions) _sharedToolbarOptions = [NSMutableDictionary dictionary];
    for (NSString *key in FloatingToolbarComponentKeys()) {
        id value = toolbar[key];
        // LocalModeBoolean asks whether this is a boolean at all, so false is already carried through;
        // a shared snapshot that turns a component off turns it off here.
        if (LocalModeBoolean(value)) _sharedToolbarOptions[key] = value;
    }
    id scale = toolbar[@"scale_percent"];
    if (ValidToolbarScale(scale)) _sharedToolbarOptions[@"scale_percent"] = scale;
    id font = toolbar[@"font_size"];
    if (ValidToolbarFontSize(font)) _sharedToolbarOptions[@"font_size"] = font;
    [self refreshControls];
}
- (void)setWubiAutoCommitUnique:(BOOL)value { [_defaults setBool:value forKey:WubiKey]; [self preferencesChanged]; }
- (void)setShuangpinKeymap:(BOOL)value {
    [_defaults setBool:value forKey:KeymapKey];
    [self preferencesChanged];
}
- (void)setFullWidthInput:(BOOL)value {
    _sharedFullWidthInput = nil;
    _runtimeFullWidthInput = nil;
    [_defaults setBool:value forKey:FullWidthKey];
    [self preferencesChanged];
}
- (void)setChinesePunctuation:(BOOL)value {
    _sharedChinesePunctuation = nil;
    _runtimeChinesePunctuation = nil;
    [_defaults setBool:value forKey:ChinesePunctuationKey];
    [self preferencesChanged];
}
- (BOOL)pairedPunctuation {
    if (_sharedPairedPunctuation) return _sharedPairedPunctuation.boolValue;
    return [_defaults objectForKey:PairedPunctuationKey] == nil ? YES : [_defaults boolForKey:PairedPunctuationKey];
}
- (void)setPairedPunctuation:(BOOL)value {
    _sharedPairedPunctuation = nil;
    [_defaults setBool:value forKey:PairedPunctuationKey];
    [self preferencesChanged];
}
- (NSString *)punctuationLock {
    NSString *value = _sharedPunctuationLock ?: [_defaults stringForKey:PunctuationLockKey];
    return [@[@"follow", @"chinese", @"english"] containsObject:value] ? value : @"follow";
}
- (void)setPunctuationLock:(NSString *)value {
    if (![@[@"follow", @"chinese", @"english"] containsObject:value]) value = @"follow";
    _sharedPunctuationLock = nil;
    [_defaults setObject:value forKey:PunctuationLockKey];
    [self preferencesChanged];
}
- (NSDictionary *)mixedInputValues {
    NSDictionary *values = _sharedMixedInput ?: [_defaults dictionaryForKey:MixedInputKey];
    return [values isKindOfClass:NSDictionary.class] ? values : @{};
}
- (BOOL)mixedEnglishInput { id value = [self mixedInputValues][@"english"]; return LocalModeBoolean(value) ? [value boolValue] : YES; }
- (void)setMixedEnglishInput:(BOOL)value {
    NSMutableDictionary *values = [[_defaults dictionaryForKey:MixedInputKey] mutableCopy] ?: [NSMutableDictionary dictionary];
    values[@"english"] = @(value); _sharedMixedInput = nil; [_defaults setObject:values forKey:MixedInputKey]; [self preferencesChanged];
}
- (NSInteger)mixedEnglishMinimumPrefix { id value = [self mixedInputValues][@"minimum_prefix"]; return ValidMixedPrefix(value) ? [value integerValue] : 5; }
- (void)setMixedEnglishMinimumPrefix:(NSInteger)value {
    if (value < 1 || value > 8) value = 5;
    NSMutableDictionary *values = [[_defaults dictionaryForKey:MixedInputKey] mutableCopy] ?: [NSMutableDictionary dictionary];
    values[@"minimum_prefix"] = @(value); _sharedMixedInput = nil; [_defaults setObject:values forKey:MixedInputKey]; [self preferencesChanged];
}
// Falls back to on, matching `source_mixed_emoji_default` in client-core and the source's `emoji_mixed_input = true`.
- (BOOL)mixedEmojiInput { id value = [self mixedInputValues][@"emoji"]; return LocalModeBoolean(value) ? [value boolValue] : YES; }
- (void)setMixedEmojiInput:(BOOL)value {
    NSMutableDictionary *values = [[_defaults dictionaryForKey:MixedInputKey] mutableCopy] ?: [NSMutableDictionary dictionary];
    values[@"emoji"] = @(value); _sharedMixedInput = nil; [_defaults setObject:values forKey:MixedInputKey]; [self preferencesChanged];
}
- (BOOL)mixedKaomojiInput { id value = [self mixedInputValues][@"kaomoji"]; return LocalModeBoolean(value) ? [value boolValue] : NO; }
- (void)setMixedKaomojiInput:(BOOL)value {
    NSMutableDictionary *values = [[_defaults dictionaryForKey:MixedInputKey] mutableCopy] ?: [NSMutableDictionary dictionary];
    values[@"kaomoji"] = @(value); _sharedMixedInput = nil; [_defaults setObject:values forKey:MixedInputKey]; [self preferencesChanged];
}
- (void)setTraditionalOutput:(BOOL)value {
    _sharedTraditionalOutput = nil;
    [_defaults setBool:value forKey:TraditionalKey];
    [self preferencesChanged];
}
- (void)setEnglishMode:(BOOL)value {
    [self overrideActiveInputModeRule];
    [self rememberActiveInputMode:value];
    [_defaults setBool:value forKey:EnglishKey];
    [self refreshControls];
    if (_silent) return;
    [[NSNotificationCenter defaultCenter] postNotificationName:MSIMEAppearanceDidChangeNotification object:self
                                                      userInfo:@{MSIMEAppearanceInputModeOnlyKey : @YES}];
}
/// Records that the user switched mode by hand in an application that has a rule, so -englishMode stops answering with the rule until they arrive at the application again. Does nothing where there is no rule to outrank.
- (void)overrideActiveInputModeRule {
    if (!_activeModeApplication) return;
    if ([self applicationInputModeRules][_activeModeApplication] == nil) return;
    if (!_inputModeRuleOverrides) _inputModeRuleOverrides = [NSMutableSet set];
    [_inputModeRuleOverrides addObject:_activeModeApplication];
}
- (BOOL)inputModeShortcut {
    if (_sharedInputModeShortcut) return _sharedInputModeShortcut.boolValue;
    return [_defaults objectForKey:InputModeShortcutKey] == nil || [_defaults boolForKey:InputModeShortcutKey];
}
- (BOOL)shiftTapShortcut {
    if (_sharedShiftTapShortcut) return _sharedShiftTapShortcut.boolValue;
    return [_defaults objectForKey:ShiftTapShortcutKey] == nil || [_defaults boolForKey:ShiftTapShortcutKey];
}
- (void)setShiftTapShortcut:(BOOL)value {
    _sharedInputModeShortcut = nil;
    _sharedShiftTapShortcut = nil;
    [_defaults setBool:value forKey:ShiftTapShortcutKey];
    [self preferencesChanged];
}
- (BOOL)controlTapShortcut {
    return _sharedControlTapShortcut ? _sharedControlTapShortcut.boolValue : [_defaults boolForKey:ControlTapShortcutKey];
}
- (void)setControlTapShortcut:(BOOL)value {
    _sharedControlTapShortcut = nil;
    [_defaults setBool:value forKey:ControlTapShortcutKey];
    [self preferencesChanged];
}
- (BOOL)controlOptionSpaceShortcut {
    if (_sharedControlOptionSpaceShortcut) return _sharedControlOptionSpaceShortcut.boolValue;
    return [_defaults objectForKey:ControlOptionSpaceShortcutKey] == nil || [_defaults boolForKey:ControlOptionSpaceShortcutKey];
}
- (void)setControlOptionSpaceShortcut:(BOOL)value {
    _sharedControlOptionSpaceShortcut = nil;
    [_defaults setBool:value forKey:ControlOptionSpaceShortcutKey];
    [self preferencesChanged];
}
- (BOOL)characterSetShortcut {
    if (_sharedCharacterSetShortcut) return _sharedCharacterSetShortcut.boolValue;
    return [_defaults objectForKey:CharacterSetShortcutKey] == nil || [_defaults boolForKey:CharacterSetShortcutKey];
}
- (void)setCharacterSetShortcut:(BOOL)value {
    _sharedCharacterSetShortcut = nil;
    [_defaults setBool:value forKey:CharacterSetShortcutKey];
    [self preferencesChanged];
}
- (BOOL)fullWidthShortcut {
    if (_sharedFullWidthShortcut) return _sharedFullWidthShortcut.boolValue;
    return [_defaults objectForKey:FullWidthShortcutKey] == nil || [_defaults boolForKey:FullWidthShortcutKey];
}
- (void)setFullWidthShortcut:(BOOL)value {
    _sharedFullWidthShortcut = nil;
    [_defaults setBool:value forKey:FullWidthShortcutKey];
    [self preferencesChanged];
}
// The dictation preferences. There is no `_shared*` override on any of them: an account pushes voice settings down through MSIMEApplySharedVoicePreferences, which writes these same defaults, so the stored entry is already the one both sides read. The two that have a helper in SharedVoicePreferences.h are read through it rather than repeated here, because the window and the input method disagreeing about whether an unset key means on or off is exactly the class of defect that header exists to prevent.
- (BOOL)voiceInputEnabled { return MSIMEVoiceInputEnabled(_defaults); }
- (void)setVoiceInputEnabled:(BOOL)value {
    [_defaults setBool:value forKey:VoiceEnabledKey];
    [self preferencesChanged];
}
- (NSString *)voiceLanguage {
    return [[_defaults stringForKey:VoiceLanguageKey] isEqualToString:@"en-US"] ? @"en-US" : @"zh-CN";
}
- (void)setVoiceLanguage:(NSString *)value {
    [_defaults setObject:[value isEqualToString:@"en-US"] ? @"en-US" : @"zh-CN" forKey:VoiceLanguageKey];
    [self preferencesChanged];
}
- (BOOL)voiceSoundEnabled {
    return [_defaults objectForKey:VoiceSoundKey] == nil || [_defaults boolForKey:VoiceSoundKey];
}
- (void)setVoiceSoundEnabled:(BOOL)value {
    [_defaults setBool:value forKey:VoiceSoundKey];
    [self preferencesChanged];
}
- (BOOL)voiceMuteSystemAudio { return MSIMEVoiceMuteSystemAudioEnabled(_defaults); }
- (void)setVoiceMuteSystemAudio:(BOOL)value {
    [_defaults setBool:value forKey:VoiceMuteSystemAudioKey];
    [self preferencesChanged];
}
- (BOOL)voiceStreamInlinePreedit {
    return [_defaults objectForKey:VoiceStreamInlinePreeditKey] == nil ||
           [_defaults boolForKey:VoiceStreamInlinePreeditKey];
}
- (void)setVoiceStreamInlinePreedit:(BOOL)value {
    [_defaults setBool:value forKey:VoiceStreamInlinePreeditKey];
    [self preferencesChanged];
}
- (BOOL)voiceHotkeyCtrlF9 {
    return [_defaults objectForKey:VoiceHotkeyCtrlF9Key] == nil || [_defaults boolForKey:VoiceHotkeyCtrlF9Key];
}
- (void)setVoiceHotkeyCtrlF9:(BOOL)value {
    [_defaults setBool:value forKey:VoiceHotkeyCtrlF9Key];
    [self preferencesChanged];
}
- (BOOL)voiceHotkeyHoldSpace {
    return [_defaults objectForKey:VoiceHotkeyHoldSpaceKey] == nil || [_defaults boolForKey:VoiceHotkeyHoldSpaceKey];
}
- (void)setVoiceHotkeyHoldSpace:(BOOL)value {
    [_defaults setBool:value forKey:VoiceHotkeyHoldSpaceKey];
    [self preferencesChanged];
}
// The three modifier holds are off unless they were asked for, which is what MSIMEVoiceHoldShortcut is handed in InputController: a modifier combination that starts recording on its own is not something to turn on for a user who never asked for it.
- (BOOL)voiceHotkeyRightAlt { return [_defaults boolForKey:VoiceHotkeyRightAltKey]; }
- (void)setVoiceHotkeyRightAlt:(BOOL)value {
    [_defaults setBool:value forKey:VoiceHotkeyRightAltKey];
    [self preferencesChanged];
}
- (BOOL)voiceHotkeyCtrlCommand { return [_defaults boolForKey:VoiceHotkeyCtrlCommandKey]; }
- (void)setVoiceHotkeyCtrlCommand:(BOOL)value {
    [_defaults setBool:value forKey:VoiceHotkeyCtrlCommandKey];
    [self preferencesChanged];
}
- (BOOL)voiceHotkeyCtrlOption { return [_defaults boolForKey:VoiceHotkeyCtrlOptionKey]; }
- (void)setVoiceHotkeyCtrlOption:(BOOL)value {
    [_defaults setBool:value forKey:VoiceHotkeyCtrlOptionKey];
    [self preferencesChanged];
}
- (void)setInputModeShortcut:(BOOL)value {
    _sharedInputModeShortcut = nil;
    _sharedShiftTapShortcut = nil;
    [_defaults setBool:value forKey:InputModeShortcutKey];
    [self preferencesChanged];
}
- (void)setVertical:(BOOL)value {
    _sharedVertical = nil;
    [_defaults setInteger:value ? 1 : 0 forKey:LayoutKey];
    [self preferencesChanged];
}
- (NSUInteger)fontSize {
    NSInteger size = _sharedFontSize ? _sharedFontSize.integerValue : [_defaults integerForKey:FontKey];
    return size >= 12 && size <= 32 ? size : 18;
}
- (NSString *)fontFamily {
    id value = _sharedFontFamily ?: [_defaults objectForKey:FontFamilyKey];
    return ValidFontFamily(value) ? value : @"Segoe UI";
}
- (NSString *)candidateEnglishFont {
    id value = _sharedCandidateEnglishFont ?: [_defaults objectForKey:CandidateEnglishFontKey];
    return ValidFontFamily(value) ? value : nil;
}
- (NSString *)candidateTextColor {
    id value = _sharedTextColor ?: [_defaults objectForKey:TextColorKey];
    return ValidTextColor(value) ? value : nil;
}
- (void)setCandidateTextColor:(NSString *)value {
    if (value && !ValidTextColor(value)) { NSBeep(); [self refreshControls]; return; }
    _sharedTextColor = nil;
    if (value) [self adoptCustomThemeForPicker];
    [_defaults setObject:value ?: @"" forKey:TextColorKey];
    [self preferencesChanged];
}
/// A picker that is set selects the custom theme (THEME_CONTRACT §5). A theme that was on screen becomes its base and the package it may have drawn is dropped, so the colours around the picked one stay the ones the user was looking at; when custom is already selected only the slot changes. A picker that is reset changes nothing but its own slot.
- (void)adoptCustomThemeForPicker {
    NSString *current = self.globalTheme;
    if ([current isEqual:@"custom"]) return;
    _sharedGlobalTheme = nil;
    _sharedCustomThemeBase = nil;
    _sharedCustomCandidateSkin = nil;
    [_defaults setObject:current forKey:CustomThemeBaseKey];
    [_defaults setObject:@"" forKey:CustomCandidateSkinKey];
    [_defaults setObject:@"custom" forKey:GlobalThemeKey];
}
- (NSString *)globalTheme {
    id value = _sharedGlobalTheme ?: [_defaults objectForKey:GlobalThemeKey];
    return [value isKindOfClass:NSString.class] && msime::mac::IsGlobalThemeId([value UTF8String]) ? value : @"system";
}
- (void)setGlobalTheme:(NSString *)value {
    if (!msime::mac::IsGlobalThemeId(value.UTF8String ?: "")) return;
    _sharedGlobalTheme = nil;
    [_defaults setObject:value forKey:GlobalThemeKey];
    [self preferencesChanged];
}
- (NSString *)customThemeBase {
    id value = _sharedCustomThemeBase ?: [_defaults objectForKey:CustomThemeBaseKey];
    return [value isKindOfClass:NSString.class] && msime::mac::IsThemeBaseId([value UTF8String]) ? value : @"system";
}
- (NSString *)customCandidateSkin {
    id value = _sharedCustomCandidateSkin ?: [_defaults objectForKey:CustomCandidateSkinKey];
    return ValidCustomCandidateSkin(value) ? value : nil;
}
- (void)selectExternalSkin:(NSString *)skinId base:(NSString *)base {
    if (!ValidCustomCandidateSkin(skinId)) return;
    _sharedGlobalTheme = nil;
    _sharedCustomThemeBase = nil;
    _sharedCustomCandidateSkin = nil;
    // The package's manifest base replaces the custom theme's own; the loader has already refused anything but system or a built-in theme.
    [_defaults setObject:msime::mac::IsThemeBaseId(base.UTF8String ?: "") ? base : @"system" forKey:CustomThemeBaseKey];
    [_defaults setObject:skinId forKey:CustomCandidateSkinKey];
    [_defaults setObject:@"custom" forKey:GlobalThemeKey];
    [self preferencesChanged];
}
- (void)clearCustomCandidateSkin {
    _sharedCustomCandidateSkin = nil;
    [_defaults setObject:@"" forKey:CustomCandidateSkinKey];
    [self preferencesChanged];
}
/// The six colours of CandidateColorControls(), which read and write exactly as 候选文字颜色 above
/// does: the account's pushed value first, then the stored one, and an empty stored string means the
/// user has said "follow the skin" rather than that nothing was ever chosen.
- (NSString *)candidateColorStoredAs:(NSString *)key shared:(id)shared {
    id value = shared ?: [_defaults objectForKey:key];
    return ValidTextColor(value) ? value : nil;
}
- (void)setCandidateColor:(NSString *)value storedAs:(NSString *)key {
    if (value && !ValidTextColor(value)) { [self refreshControls]; return; }
    if (value) [self adoptCustomThemeForPicker];
    [_defaults setObject:value ?: @"" forKey:key];
    [self preferencesChanged];
}
/// What one of the seven colours looks like when nothing overrides it: the token the candidate window draws with, taken from the selected theme resolved for the appearance this window is currently drawn in.
- (NSColor *)candidateSkinColorForProperty:(NSString *)property {
    NSAppearanceName match = [NSApp.effectiveAppearance
        bestMatchFromAppearancesWithNames:@[ NSAppearanceNameAqua, NSAppearanceNameDarkAqua ]];
    const msime::mac::SkinTokens tokens = [self resolvedSkinForDark:[match isEqual:NSAppearanceNameDarkAqua]].tokens;
    if ([property isEqual:@"candidateTextColor"]) return SkinTokenColor(tokens.text);
    if ([property isEqual:@"candidateNumberColor"]) return SkinTokenColor(tokens.number);
    if ([property isEqual:@"candidateAccentColor"]) return SkinTokenColor(tokens.accent);
    if ([property isEqual:@"candidateSelectedColor"]) return SkinTokenColor(tokens.selected);
    if ([property isEqual:@"candidateHoverColor"]) return SkinTokenColor(tokens.hover);
    if ([property isEqual:@"candidateSurfaceColor"]) return SkinTokenColor(tokens.surface);
    return SkinTokenColor(tokens.border);
}
- (NSString *)candidateNumberColor { return [self candidateColorStoredAs:NumberColorKey shared:_sharedNumberColor]; }
- (void)setCandidateNumberColor:(NSString *)value { _sharedNumberColor = nil; [self setCandidateColor:value storedAs:NumberColorKey]; }
- (NSString *)candidateAccentColor { return [self candidateColorStoredAs:AccentColorKey shared:_sharedAccentColor]; }
- (void)setCandidateAccentColor:(NSString *)value { _sharedAccentColor = nil; [self setCandidateColor:value storedAs:AccentColorKey]; }
- (NSString *)candidateSelectedColor { return [self candidateColorStoredAs:SelectedColorKey shared:_sharedSelectedColor]; }
- (void)setCandidateSelectedColor:(NSString *)value { _sharedSelectedColor = nil; [self setCandidateColor:value storedAs:SelectedColorKey]; }
- (NSString *)candidateHoverColor { return [self candidateColorStoredAs:HoverColorKey shared:_sharedHoverColor]; }
- (void)setCandidateHoverColor:(NSString *)value { _sharedHoverColor = nil; [self setCandidateColor:value storedAs:HoverColorKey]; }
- (NSString *)candidateSurfaceColor { return [self candidateColorStoredAs:SurfaceColorKey shared:_sharedSurfaceColor]; }
- (void)setCandidateSurfaceColor:(NSString *)value { _sharedSurfaceColor = nil; [self setCandidateColor:value storedAs:SurfaceColorKey]; }
- (NSString *)candidateBorderColor { return [self candidateColorStoredAs:BorderColorKey shared:_sharedBorderColor]; }
- (void)setCandidateBorderColor:(NSString *)value { _sharedBorderColor = nil; [self setCandidateColor:value storedAs:BorderColorKey]; }
- (NSString *)themeMode {
    id value = _sharedTheme ?: [_defaults objectForKey:ThemeKey];
    return [ThemeModes() containsObject:value] ? value : @"system";
}
- (void)setThemeMode:(NSString *)value {
    if (![ThemeModes() containsObject:value]) return;
    _sharedTheme = nil;
    [_defaults setObject:value forKey:ThemeKey];
    [self preferencesChanged];
}
- (NSString *)candidateTheme {
    id value = _sharedCandidateTheme ?: [_defaults objectForKey:CandidateThemeKey];
    return [SurfaceThemes() containsObject:value] ? value : @"follow";
}
- (void)setCandidateTheme:(NSString *)value {
    if (![SurfaceThemes() containsObject:value]) return;
    _sharedCandidateTheme = nil;
    [_defaults setObject:value forKey:CandidateThemeKey];
    [self preferencesChanged];
}
- (NSString *)toolbarTheme {
    id value = _sharedToolbarTheme ?: [_defaults objectForKey:ToolbarThemeKey];
    return [SurfaceThemes() containsObject:value] ? value : @"follow";
}
- (void)setToolbarTheme:(NSString *)value {
    if (![SurfaceThemes() containsObject:value]) return;
    _sharedToolbarTheme = nil;
    [_defaults setObject:value forKey:ToolbarThemeKey];
    [self preferencesChanged];
}
- (void)setFontFamily:(NSString *)value {
    if (!ValidFontFamily(value)) { [self refreshControls]; return; }
    _sharedFontFamily = nil;
    [_defaults setObject:[value copy] forKey:FontFamilyKey];
    [self preferencesChanged];
}
- (void)setCandidateEnglishFont:(NSString *)value {
    if (value && !ValidFontFamily(value)) { [self refreshControls]; return; }
    _sharedCandidateEnglishFont = nil;
    // An empty marker lets the native fallback explicitly clear a previously
    // shared value without putting NSNull into NSUserDefaults.
    [_defaults setObject:value.length ? [value copy] : @"" forKey:CandidateEnglishFontKey];
    [self preferencesChanged];
}
- (NSFont *)candidateFontOfSize:(CGFloat)size {
    return [self candidateFontOfSize:size englishFirst:NO];
}
- (NSFont *)candidateFontOfSize:(CGFloat)size englishFirst:(BOOL)englishFirst {
    // Resolve a family without silently substituting a different installed family.
    // Preserve unavailable cross-platform names in preferences. Resolve installed
    // supplementary families in order, retaining system fallback at the end.
    NSMutableArray<NSFontDescriptor *> *resolved = [NSMutableArray array];
    NSMutableArray<NSString *> *families = [NSMutableArray array];
    if (englishFirst && self.candidateEnglishFont.length) [families addObject:self.candidateEnglishFont];
    [families addObject:self.fontFamily];
    [families addObjectsFromArray:self.fallbackFonts];
    // The family list is read afresh on every call so writes from other processes apply; only the per-family match is cached.
    for (NSString *family in families) {
        NSFontDescriptor *matched = MSIMEInstalledFontFamilyDescriptor(family);
        if (matched) [resolved addObject:matched];
    }
    NSFont *system = [NSFont systemFontOfSize:size];
    if (!resolved.count) return system;
    NSFontDescriptor *primary = resolved.firstObject;
    [resolved removeObjectAtIndex:0];
    if (families.count > 1 || resolved.count) {
        [resolved addObject:system.fontDescriptor];
        primary = [primary fontDescriptorByAddingAttributes:@{NSFontCascadeListAttribute:resolved}];
    }
    return [NSFont fontWithDescriptor:primary size:size] ?: system;
}
- (NSArray<NSString *> *)fallbackFonts {
    id value = _sharedFallbackFonts ?: [_defaults objectForKey:FallbackFontsKey];
    return ValidFallbackFonts(value) ? value : @[];
}
- (void)setFallbackFonts:(NSArray<NSString *> *)value {
    if (!ValidFallbackFonts(value)) return;
    _sharedFallbackFonts = nil;
    [_defaults setObject:[[NSArray alloc] initWithArray:value copyItems:YES] forKey:FallbackFontsKey];
    [self preferencesChanged];
}
- (void)setFontSize:(NSUInteger)value {
    _sharedFontSize = nil;
    [_defaults setInteger:value >= 12 && value <= 32 ? value : 18 forKey:FontKey];
    [self preferencesChanged];
}
- (void)preferencesChanged {
    [self resolveSelectedSkinIfChanged];
    [self refreshControls];
    if (_silent) return;
    [[NSNotificationCenter defaultCenter] postNotificationName:MSIMEAppearanceDidChangeNotification object:self];
}
- (NSUInteger)preeditFontSize {
    NSInteger size = _sharedPreeditFontSize ? _sharedPreeditFontSize.integerValue : [_defaults integerForKey:PreeditFontKey];
    return size >= 12 && size <= 32 ? size : 16;
}
- (void)setPreeditFontSize:(NSUInteger)value {
    _sharedPreeditFontSize = nil;
    [_defaults setInteger:value >= 12 && value <= 32 ? value : 16 forKey:PreeditFontKey];
    [self preferencesChanged];
}
- (NSInteger)candidateScalePercent {
    id value = _sharedCandidateScale ?: [_defaults objectForKey:CandidateScaleKey];
    return ValidCandidateScale(value) ? [value integerValue] : 100;
}
- (void)setCandidateScalePercent:(NSInteger)value {
    if (!ValidCandidateScale(@(value))) { [self refreshControls]; return; }
    _sharedCandidateScale = nil;
    [_defaults setInteger:value forKey:CandidateScaleKey];
    [self preferencesChanged];
}
- (NSInteger)candidateOpacityPercent {
    id value = _sharedCandidateOpacity ?: [_defaults objectForKey:CandidateOpacityKey];
    return ValidCandidateOpacity(value) ? [value integerValue] : 100;
}
- (void)setCandidateOpacityPercent:(NSInteger)value {
    if (!ValidCandidateOpacity(@(value))) { [self refreshControls]; return; }
    _sharedCandidateOpacity = nil;
    [_defaults setInteger:value forKey:CandidateOpacityKey];
    [self preferencesChanged];
}
- (NSNumber *)candidateCornerRadius {
    id value = _sharedCandidateCornerRadius ?: [_defaults objectForKey:CandidateCornerRadiusKey];
    return ValidCandidateCornerRadius(value) ? @([value integerValue]) : nil;
}
- (void)setCandidateCornerRadius:(NSNumber *)value {
    if (value && !ValidCandidateCornerRadius(value)) { [self refreshControls]; return; }
    _sharedCandidateCornerRadius = nil;
    [_defaults setObject:value ? @(value.integerValue) : @"" forKey:CandidateCornerRadiusKey];
    [self preferencesChanged];
}
- (msime::mac::CandidateWindowStyle)candidateWindowStyle {
    msime::mac::CandidateWindowStyle style;
    style.scale = self.candidateScalePercent / 100.0;
    style.opacity = self.candidateOpacityPercent / 100.0;
    if (NSNumber *radius = self.candidateCornerRadius) style.cornerRadius = radius.doubleValue;
    return style;
}
- (msime::mac::ResolvedSkin)candidateWindowSkinForDark:(BOOL)dark {
    return msime::mac::StyledCandidateSkin([self resolvedSkinForDark:dark], [self candidateWindowStyle]);
}
- (NSInteger)candidateFontPreset {
    NSString *family = self.fontFamily;
    NSArray<NSArray<NSString *> *> *presets = CandidateFontPresets();
    // 默认 first: Noto Sans SC is also the last resort of 黑体 and 圆体, and on its own it is the shared default.
    for (NSUInteger index = 0; index < presets.count; ++index) {
        NSArray<NSString *> *families = [presets[index] subarrayWithRange:NSMakeRange(1, presets[index].count - 1)];
        if (index == 0 ? [family isEqual:families.firstObject] : [families containsObject:family]) return (NSInteger)index;
    }
    return -1;
}
- (void)setCandidateFontPreset:(NSInteger)value {
    NSArray<NSArray<NSString *> *> *presets = CandidateFontPresets();
    if (value < 0 || value >= (NSInteger)presets.count) { [self refreshControls]; return; }
    NSArray<NSString *> *families = [presets[(NSUInteger)value] subarrayWithRange:NSMakeRange(1, presets[(NSUInteger)value].count - 1)];
    NSMutableArray<NSString *> *fallbacks = [families mutableCopy];
    // 默认 is the shared default pair as it stands; a preset goes in front of what the user already had, so the families added by hand are still tried after it.
    if (value != 0)
        for (NSString *family in self.fallbackFonts)
            if (fallbacks.count < kFallbackFontLimit && ![fallbacks containsObject:family]) [fallbacks addObject:family];
    // Both fields in one change, so the window redraws and the document is saved once.
    _sharedFontFamily = nil;
    _sharedFallbackFonts = nil;
    [_defaults setObject:families.firstObject forKey:FontFamilyKey];
    [_defaults setObject:fallbacks forKey:FallbackFontsKey];
    [self preferencesChanged];
}
- (BOOL)showsCandidatePreedit {
    return ![(_sharedCandidatePreedit ?: [_defaults stringForKey:CandidatePreeditKey]) isEqual:@"empty"];
}
- (void)setShowsCandidatePreedit:(BOOL)value {
    _sharedCandidatePreedit = nil;
    [_defaults setObject:value ? @"pinyin" : @"empty" forKey:CandidatePreeditKey];
    [self preferencesChanged];
}
/// Which key group the paging preset names, read back out of the navigation dictionary rather than out of a second stored number of its own.
///
/// The preset and the seven paging checkboxes are one setting written two ways, and they used to be two settings: the preset wrote MSIMEClientCandidatePageShortcut and the checkboxes wrote MSIMEClientNavigation, the input method routed keys by the dictionary alone, and the menu went on showing a group the user had since unchecked. The dictionary is what the input method reads, so the dictionary is what the menu now reports.
///
/// All three groups are asked, including the one the third preset names. Answering 2 for everything that was neither bracket nor minus/equal made the menu claim 「Page Up / Page Down」 for a user who had just unticked exactly that box — and 2 is the preset whose setter turns it back on, so the menu was offering to undo the change it was already misreporting. The checkboxes can reach states no preset names — 逗号/句号翻页 alone is one — and -1 is this getter saying so rather than picking the nearest of three.
- (NSInteger)pageShortcut {
    if ([self navigationEnabled:@"brackets"]) return 1;
    if ([self navigationEnabled:@"minus_equal"]) return 0;
    if ([self navigationEnabled:@"page_up_down"]) return 2;
    return -1;
}
/// The preset's own stored value, which is no longer what the window reads: it is the seed the
/// navigation dictionary falls back to for a profile that has never written one, and it is what the
/// cloud snapshot carries (platform.macos.candidate_page_shortcut, written into this key by
/// MSIMEApplyCloudAppearance) for a machine that has no dictionary yet either.
- (NSInteger)storedPageShortcut {
    NSInteger value = [_defaults integerForKey:PageShortcutKey];
    return value == 1 || value == 2 ? value : 0;
}
- (BOOL)navigationEnabled:(NSString *)key {
    id value = _sharedNavigation[key] ?: [_defaults dictionaryForKey:NavigationKey][key];
    if (LocalModeBoolean(value)) return [value boolValue];
    if ([key isEqual:@"minus_equal"]) return [self storedPageShortcut] == 0;
    if ([key isEqual:@"brackets"]) return [self storedPageShortcut] == 1;
    return [@[@"comma_period", @"tab", @"page_up_down", @"arrows"] containsObject:key];
}
- (NSDictionary *)wordCharacterOptions {
    NSDictionary *value = _sharedWordCharacter ?: [_defaults dictionaryForKey:WordCharacterKey];
    return LocalModeBoolean(value[@"enabled"]) && [@[@"brackets", @"minus_equal"] containsObject:value[@"keys"]] ? value : @{@"enabled": @YES, @"keys": @"brackets"};
}
- (void)setWordCharacterEnabled:(BOOL)enabled keys:(NSString *)keys {
    if (![@[@"brackets", @"minus_equal"] containsObject:keys]) return;
    // 以词定字 and 翻页 cannot share a key group, and the window no longer offers the move that would
    // make them: -refreshKeyBindingConflicts disables whichever side does not hold the group. This
    // is the invariant itself, for the callers that are not the window — the setter is public
    // (AppearancePreferences.h) — and it refuses without a sound, where it used to beep at a user
    // who had pressed a control the window had offered them.
    if (enabled && [self navigationEnabled:keys]) { [self refreshControls]; return; }
    _sharedWordCharacter = @{@"enabled": @(enabled), @"keys": keys};
    [_defaults setObject:_sharedWordCharacter forKey:WordCharacterKey];
    [self preferencesChanged];
}
- (void)setNavigation:(NSString *)key enabled:(BOOL)enabled {
    BOOL known = NO;
    for (NSArray *entry in NavigationControls()) if ([entry[0] isEqual:key]) known = YES;
    if (!known) return;
    // The other direction of the same exclusion; see -setWordCharacterEnabled:keys:.
    if (enabled && [[self wordCharacterOptions][@"enabled"] boolValue] && [[self wordCharacterOptions][@"keys"] isEqual:key]) {
        [self refreshControls]; return;
    }
    // The stored preset is still what -navigationEnabled: falls back to for one of these two groups when the dictionary has no entry for it, and it is about to be rewritten, so both are written down at the values they have now: without that, ticking 方括号翻页 would turn 减号/等号翻页 off by moving the fallback out from under it.
    NSDictionary<NSString *, NSNumber *> *presetGroups = @{@"minus_equal" : @([self navigationEnabled:@"minus_equal"]),
                                                           @"brackets" : @([self navigationEnabled:@"brackets"])};
    NSMutableDictionary *values = [[_defaults dictionaryForKey:NavigationKey] mutableCopy] ?: [NSMutableDictionary dictionary];
    values[key] = @(enabled);
    for (NSString *group in presetGroups)
        if (values[group] == nil) values[group] = presetGroups[group];
    [_defaults setObject:values forKey:NavigationKey];
    if (!_sharedNavigation) _sharedNavigation = [NSMutableDictionary dictionary];
    _sharedNavigation[key] = @(enabled);
    [self syncStoredPageShortcut];
    [self preferencesChanged];
}
/// The stored preset, brought back into step with the dictionary the checkboxes have just written.
///
/// -pageShortcut stopped reading this key when the preset and the checkboxes became one setting, but two things still do: -navigationEnabled: falls back to it for a key group no dictionary has an entry for, and the cloud snapshot carries it as platform.macos.candidate_page_shortcut. Left wherever the preset menu last put it, a machine that unticked 方括号翻页 and then synced pushed the bracket preset up to every other host the account signs in on.
- (void)syncStoredPageShortcut {
    [_defaults setInteger:[self storedPageShortcutForCurrentBindings] forKey:PageShortcutKey];
}
/// The current paging bindings as the three-value preset, for the two places that can only carry three values. A state no preset names exports as 2, which is the one value that claims neither of the two key groups the preset can name.
- (NSInteger)storedPageShortcutForCurrentBindings {
    const NSInteger preset = self.pageShortcut;
    return preset < 0 ? 2 : preset;
}
- (NSUInteger)pageSize {
    NSInteger value = _sharedPageSize ? _sharedPageSize.integerValue : [_defaults integerForKey:PageSizeKey];
    // Absent reads as zero, which is not a page size. That is the unset case, and it means the shared
    // default rather than the nearest legal number.
    if (value <= 0) return msime::mac::kDefaultCandidatePageSize;
    return msime::mac::NormalizeCandidatePageSize(static_cast<NSUInteger>(value));
}
- (void)setPageSize:(NSUInteger)value {
    _sharedPageSize = nil;
    [_defaults setInteger:msime::mac::NormalizeCandidatePageSize(value) forKey:PageSizeKey];
    [self preferencesChanged];
}
- (void)applySharedCandidatePreferences:(NSDictionary *)preferences {
    if (![preferences isKindOfClass:NSDictionary.class]) return;
    NSDictionary *navigation = preferences[@"navigation"];
    NSDictionary *wordCharacter = preferences[@"word_character"];
    if ([wordCharacter isKindOfClass:NSDictionary.class] && LocalModeBoolean(wordCharacter[@"enabled"]) &&
        [@[@"brackets", @"minus_equal"] containsObject:wordCharacter[@"keys"]]) _sharedWordCharacter = [wordCharacter copy];
    if ([navigation isKindOfClass:NSDictionary.class]) {
        if (!_sharedNavigation) _sharedNavigation = [NSMutableDictionary dictionary];
        for (NSArray *entry in NavigationControls())
            if (LocalModeBoolean(navigation[entry[0]])) _sharedNavigation[entry[0]] = navigation[entry[0]];
        // Windows names this shared switch candidate_arrow_navigation; accept
        // it at the Apple boundary while retaining navigation.arrows locally.
        if (navigation[@"candidate_arrow_navigation"] != nil)
            _sharedNavigation[@"arrows"] = @([navigation[@"candidate_arrow_navigation"] boolValue]);
    }
    id layout = preferences[@"candidate_layout"];
    if ([@[@"horizontal", @"vertical"] containsObject:layout]) _sharedVertical = @([layout isEqual:@"vertical"]);
    id followCursor = preferences[@"candidate_follow_cursor"];
    if (LocalModeBoolean(followCursor)) _sharedCandidateFollowCursor = followCursor;
    id inputModeHUD = preferences[@"input_mode_hud"];
    if (LocalModeBoolean(inputModeHUD)) _sharedInputModeHUD = inputModeHUD;
    id font = preferences[@"candidate_font_size"];
    // The global theme and the custom theme. Every part of custom_theme is omitted when unset, and omission also clears a previously loaded value, without persisting a local override.
    id globalTheme = preferences[@"global_theme"];
    if ([globalTheme isKindOfClass:NSString.class] && msime::mac::IsGlobalThemeId([globalTheme UTF8String])) _sharedGlobalTheme = [globalTheme copy];
    NSDictionary *customTheme = [preferences[@"custom_theme"] isKindOfClass:NSDictionary.class] ? preferences[@"custom_theme"] : @{};
    id base = customTheme[@"base"];
    if (!base) _sharedCustomThemeBase = @"system";
    else if ([base isKindOfClass:NSString.class] && msime::mac::IsThemeBaseId([base UTF8String])) _sharedCustomThemeBase = [base copy];
    id candidateSkin = customTheme[@"candidate_skin"];
    if (!candidateSkin || candidateSkin == NSNull.null) _sharedCustomCandidateSkin = NSNull.null;
    else if (ValidCustomCandidateSkin(candidateSkin)) _sharedCustomCandidateSkin = [candidateSkin copy];
    NSDictionary *pickerColors = [customTheme[@"candidate_colors"] isKindOfClass:NSDictionary.class] ? customTheme[@"candidate_colors"] : @{};
    _sharedTextColor = SharedCandidateColor(pickerColors, @"text", _sharedTextColor);
    _sharedNumberColor = SharedCandidateColor(pickerColors, @"number", _sharedNumberColor);
    _sharedAccentColor = SharedCandidateColor(pickerColors, @"accent", _sharedAccentColor);
    _sharedSelectedColor = SharedCandidateColor(pickerColors, @"selected", _sharedSelectedColor);
    _sharedHoverColor = SharedCandidateColor(pickerColors, @"hover", _sharedHoverColor);
    _sharedSurfaceColor = SharedCandidateColor(pickerColors, @"surface", _sharedSurfaceColor);
    _sharedBorderColor = SharedCandidateColor(pickerColors, @"border", _sharedBorderColor);
    id family = preferences[@"candidate_font_family"];
    if (ValidFontFamily(family)) _sharedFontFamily = [family copy];
    id englishFamily = preferences[@"candidate_english_font"];
    if (!englishFamily) {
        // An omitted optional field clears a previously loaded shared value,
        // while leaving a legacy native-only value usable on first load.
        if (_sharedCandidateEnglishFont) _sharedCandidateEnglishFont = NSNull.null;
    } else if (englishFamily == NSNull.null) _sharedCandidateEnglishFont = NSNull.null;
    else if (ValidFontFamily(englishFamily)) _sharedCandidateEnglishFont = [englishFamily copy];
    id fallbacks = preferences[@"candidate_fallback_fonts"];
    if (ValidFallbackFonts(fallbacks)) _sharedFallbackFonts = [[NSArray alloc] initWithArray:fallbacks copyItems:YES];
    id preeditFont = preferences[@"candidate_preedit_font_size"];
    id preeditStyle = preferences[@"candidate_preedit_style"];
    if ([preeditFont isKindOfClass:NSNumber.class] && !LocalModeBoolean(preeditFont) && [preeditFont doubleValue] == [preeditFont integerValue] && [preeditFont integerValue] >= 12 && [preeditFont integerValue] <= 32) _sharedPreeditFontSize = preeditFont;
    if ([@[@"pinyin", @"empty"] containsObject:preeditStyle]) _sharedCandidatePreedit = preeditStyle;
    // The shared serializer leaves out a style value at its default, so an omitted key always reads as 100% or as following the skin, on the first document too: these keys are newer than the shared document, so there is no native-only value to keep the way 候选窗英文字体 above does, and a stored value that outlived a reset elsewhere would otherwise draw and then be published back. A value outside the shared ranges is ignored rather than clamped.
    id scale = preferences[@"candidate_scale_percent"];
    if (ValidCandidateScale(scale)) _sharedCandidateScale = scale;
    else if (!scale) _sharedCandidateScale = @100;
    id opacity = preferences[@"candidate_opacity_percent"];
    if (ValidCandidateOpacity(opacity)) _sharedCandidateOpacity = opacity;
    else if (!opacity) _sharedCandidateOpacity = @100;
    id cornerRadius = preferences[@"candidate_corner_radius"];
    if (ValidCandidateCornerRadius(cornerRadius)) _sharedCandidateCornerRadius = cornerRadius;
    else if (!cornerRadius || cornerRadius == NSNull.null) _sharedCandidateCornerRadius = NSNull.null;
    id page = preferences[@"candidate_page_size"];
    // Match the shared integer ranges; booleans and fractions are not sizes.
    if ([font isKindOfClass:NSNumber.class] && !LocalModeBoolean(font) && [font doubleValue] == [font integerValue] && [font integerValue] >= 12 && [font integerValue] <= 32) _sharedFontSize = font;
    if ([page isKindOfClass:NSNumber.class] && !LocalModeBoolean(page) && [page doubleValue] == [page integerValue] && [page integerValue] >= 1 && [page integerValue] <= 9)
        _sharedPageSize = @(msime::mac::NormalizeCandidatePageSize([page unsignedIntegerValue]));
    id theme = preferences[@"theme"];
    if ([ThemeModes() containsObject:theme]) _sharedTheme = [theme copy];
    id candidateTheme = preferences[@"candidate_theme"];
    if ([SurfaceThemes() containsObject:candidateTheme]) _sharedCandidateTheme = [candidateTheme copy];
    // The toolbar's own override arrives here rather than in -applySharedToolbarPreferences: because
    // it is a top-level key beside the other two, and that method reads the floating_toolbar
    // dictionary and returns when there is none.
    id toolbarTheme = preferences[@"toolbar_theme"];
    if ([SurfaceThemes() containsObject:toolbarTheme]) _sharedToolbarTheme = [toolbarTheme copy];
    [self resolveSelectedSkinIfChanged];
    [self refreshControls];
}

- (NSAppearance *)candidateAppearanceOverride {
    // A theme with a mode of its own (水杉, 浅色, 纸, 夜 and 墨, or the custom theme over one of them) is drawn in that mode whatever the light/dark choice says: its palette is the one for that mode.
    if (_lightSkin.fixedDark) return [NSAppearance appearanceNamed:*_lightSkin.fixedDark ? NSAppearanceNameDarkAqua : NSAppearanceNameAqua];
    return self.systemBaseCandidateAppearanceOverride;
}
- (NSAppearance *)systemBaseCandidateAppearanceOverride {
    NSString *surface = self.candidateTheme;
    NSString *resolved = [surface isEqual:@"dark"] || [surface isEqual:@"light"] ? surface : self.themeMode;
    if ([resolved isEqual:@"dark"]) return [NSAppearance appearanceNamed:NSAppearanceNameDarkAqua];
    if ([resolved isEqual:@"light"]) return [NSAppearance appearanceNamed:NSAppearanceNameAqua];
    return nil;
}
- (BOOL)candidateAppearanceOverrideConfigured {
    return _lightSkin.fixedDark.has_value() || _sharedTheme != nil || _sharedCandidateTheme != nil || [_defaults objectForKey:ThemeKey] != nil ||
           [_defaults objectForKey:CandidateThemeKey] != nil;
}
- (void)setPageShortcut:(NSInteger)value {
    value = value == 1 || value == 2 ? value : 0;
    NSString *enabledKey = value == 0 ? @"minus_equal" : value == 1 ? @"brackets" : @"page_up_down";
    // The preset turns paging on for one key group, so it collides the same way a single paging
    // checkbox does; the menu item for a contested group is disabled in -refreshControls.
    if ([[self wordCharacterOptions][@"enabled"] boolValue] && [[self wordCharacterOptions][@"keys"] isEqual:enabledKey]) { [self refreshControls]; return; }
    [self applyNavigationPreset:value];
    [_defaults setInteger:value forKey:PageShortcutKey];
    [self preferencesChanged];
}
/// The preset writes the same dictionary the checkboxes write, because there is nothing else to
/// write it to: choosing 「[ / ]」 is choosing the bracket checkbox and unchoosing the minus/equal one.
///
/// It used to set page_up_down to YES as well, whichever group had been picked — so picking 「- / =」
/// turned Page Up and Page Down back on under a user who had just unchecked them, and the checkbox
/// and the menu disagreed about a key that had only one setting. The third preset is the only one
/// that says anything about that group, and all it can say is to turn it on.
- (void)applyNavigationPreset:(NSInteger)value {
    NSMutableDictionary *navigation = [[_defaults dictionaryForKey:NavigationKey] mutableCopy] ?: [NSMutableDictionary dictionary];
    navigation[@"minus_equal"] = @(value == 0);
    navigation[@"brackets"] = @(value == 1);
    if (value == 2) navigation[@"page_up_down"] = @YES;
    [_defaults setObject:navigation forKey:NavigationKey];
    if (!_sharedNavigation) _sharedNavigation = [NSMutableDictionary dictionary];
    for (NSString *key in @[@"minus_equal", @"brackets", @"page_up_down"])
        if (navigation[key] != nil) _sharedNavigation[key] = navigation[key];
}
- (void)refreshControls {
    [_defaultImeModeButton selectItemAtIndex:[self.defaultImeMode isEqual:@"english"] ? 1 : 0];
    [_imeModeScopeButton selectItemAtIndex:[self.imeModeScope isEqual:@"global"] ? 1 : 0];
    // 应用例外, in the order the user reads rather than the order a dictionary hands them over in. Sorted by the name on screen, so two rules do not swap places between two openings of the window.
    NSDictionary<NSString *, NSString *> *applicationRules = [self applicationInputModeRules];
    _appRuleIdentifiers = [applicationRules.allKeys sortedArrayUsingComparator:^NSComparisonResult(NSString *a, NSString *b) {
        return [[self applicationNameForBundleIdentifier:a] localizedStandardCompare:[self applicationNameForBundleIdentifier:b]];
    }];
    const NSInteger applicationRuleSelection = _appRuleTable.selectedRow;
    [_appRuleTable reloadData];
    if (applicationRuleSelection >= 0 && (NSUInteger)applicationRuleSelection < _appRuleIdentifiers.count)
        [_appRuleTable selectRowIndexes:[NSIndexSet indexSetWithIndex:(NSUInteger)applicationRuleSelection]
                   byExtendingSelection:NO];
    _appRuleRemoveButton.enabled = _appRuleTable.selectedRow >= 0;
    // Written here rather than left where -addApplicationInputModeRule: may have put a refusal: the refusal is about the press that has just happened, and the next thing to happen to this table replaces it with what the table now holds.
    _appRuleStatusLabel.textColor = NSColor.secondaryLabelColor;
    _appRuleStatusLabel.stringValue = applicationRules.count == 0
        ? @"还没有应用例外，所有应用都按上面的设置走。"
        : [NSString stringWithFormat:@"已为 %lu 个应用指定了输入模式。", (unsigned long)applicationRules.count];
    NSDictionary *wordCharacter = [self wordCharacterOptions];
    _wordCharacterToggle.state = [wordCharacter[@"enabled"] boolValue] ? NSControlStateValueOn : NSControlStateValueOff;
    [_wordCharacterKeys selectItemAtIndex:[wordCharacter[@"keys"] isEqual:@"minus_equal"] ? 1 : 0];
    for (NSButton *button in _navigationButtons)
        button.state = [self navigationEnabled:button.identifier] ? NSControlStateValueOn : NSControlStateValueOff;
    for (NSString *scheme in _helpcodeSchemaButtons) {
        NSDictionary *values = [self helpcodeOptionsForScheme:scheme];
        [_helpcodeSchemaButtons[scheme] selectItemAtIndex:[HelpcodeSchemas() indexOfObject:values[@"schema"]]];
        _helpcodeDisplayToggles[scheme].state = [values[@"show_in_candidate_window"] boolValue] ? NSControlStateValueOn : NSControlStateValueOff;
    }
    _fullWidthToggle.state = self.fullWidthInput ? NSControlStateValueOn : NSControlStateValueOff;
    _fullWidthShortcutToggle.state = self.fullWidthShortcut ? NSControlStateValueOn : NSControlStateValueOff;
    _voiceEnabledToggle.state = self.voiceInputEnabled ? NSControlStateValueOn : NSControlStateValueOff;
    [_voiceLanguageButton selectItemAtIndex:[self.voiceLanguage isEqualToString:@"en-US"] ? 1 : 0];
    _voiceSoundToggle.state = self.voiceSoundEnabled ? NSControlStateValueOn : NSControlStateValueOff;
    _voiceMuteSystemAudioToggle.state = self.voiceMuteSystemAudio ? NSControlStateValueOn : NSControlStateValueOff;
    _voiceStreamInlinePreeditToggle.state = self.voiceStreamInlinePreedit ? NSControlStateValueOn : NSControlStateValueOff;
    _voiceHotkeyCtrlF9Toggle.state = self.voiceHotkeyCtrlF9 ? NSControlStateValueOn : NSControlStateValueOff;
    _voiceHotkeyRightAltToggle.state = self.voiceHotkeyRightAlt ? NSControlStateValueOn : NSControlStateValueOff;
    _voiceHotkeyCtrlCommandToggle.state = self.voiceHotkeyCtrlCommand ? NSControlStateValueOn : NSControlStateValueOff;
    _voiceHotkeyCtrlOptionToggle.state = self.voiceHotkeyCtrlOption ? NSControlStateValueOn : NSControlStateValueOff;
    _voiceHotkeyHoldSpaceToggle.state = self.voiceHotkeyHoldSpace ? NSControlStateValueOn : NSControlStateValueOff;
    _traditionalOutputToggle.state = self.traditionalOutput ? NSControlStateValueOn : NSControlStateValueOff;
    _keymapToggle.state = self.shuangpinKeymap ? NSControlStateValueOn : NSControlStateValueOff;
    _wubiToggle.state = self.wubiAutoCommitUnique ? NSControlStateValueOn : NSControlStateValueOff;
    _wubiMixedPinyinToggle.state = self.wubiMixedPinyinEnabled ? NSControlStateValueOn : NSControlStateValueOff;
    _punctuationToggle.state = self.chinesePunctuation ? NSControlStateValueOn : NSControlStateValueOff;
    _smartPunctuationToggle.state = self.smartPunctuation ? NSControlStateValueOn : NSControlStateValueOff;
    _smartPunctuationRepeatToggle.state = self.smartPunctuationRepeatToChinese ? NSControlStateValueOn : NSControlStateValueOff;
    _smartPunctuationSpaceToggle.state = self.smartPunctuationSpaceConvert ? NSControlStateValueOn : NSControlStateValueOff;
    _pairedPunctuationToggle.state = self.pairedPunctuation ? NSControlStateValueOn : NSControlStateValueOff;
    NSDictionary *punctuationLockIndexes = @{@"follow": @0, @"chinese": @1, @"english": @2};
    [_punctuationLockButton selectItemAtIndex:[punctuationLockIndexes[self.punctuationLock] integerValue]];
    _mixedEnglishToggle.state = self.mixedEnglishInput ? NSControlStateValueOn : NSControlStateValueOff;
    [_mixedEnglishPrefixButton selectItemAtIndex:self.mixedEnglishMinimumPrefix - 1];
    _mixedEmojiToggle.state = self.mixedEmojiInput ? NSControlStateValueOn : NSControlStateValueOff;
    _mixedKaomojiToggle.state = self.mixedKaomojiInput ? NSControlStateValueOn : NSControlStateValueOff;
    _toolbarToggle.state = self.floatingToolbarEnabled ? NSControlStateValueOn : NSControlStateValueOff;
    _toolbarEnglishModeButton.state = self.floatingToolbarEnglishMode ? NSControlStateValueOn : NSControlStateValueOff;
    _toolbarPunctuationButton.state = self.floatingToolbarPunctuation ? NSControlStateValueOn : NSControlStateValueOff;
    _toolbarFullWidthButton.state = self.floatingToolbarFullWidth ? NSControlStateValueOn : NSControlStateValueOff;
    _toolbarCharacterSetButton.state = self.floatingToolbarCharacterSet ? NSControlStateValueOn : NSControlStateValueOff;
    _toolbarEmojiButton.state = self.floatingToolbarEmoji ? NSControlStateValueOn : NSControlStateValueOff;
    _toolbarHandwritingButton.state = self.floatingToolbarHandwriting ? NSControlStateValueOn : NSControlStateValueOff;
    _toolbarInputSchemeButton.state = self.floatingToolbarInputScheme ? NSControlStateValueOn : NSControlStateValueOff;
    _toolbarScreenKeyboardButton.state = self.floatingToolbarScreenKeyboard ? NSControlStateValueOn : NSControlStateValueOff;
    _toolbarVoiceButton.state = self.floatingToolbarVoice ? NSControlStateValueOn : NSControlStateValueOff;
    _toolbarSettingsButton.state = self.floatingToolbarSettings ? NSControlStateValueOn : NSControlStateValueOff;
    [_toolbarThemeButton selectItemAtIndex:(NSInteger)[SurfaceThemes() indexOfObject:self.toolbarTheme]];
    [_toolbarScaleButton selectItemAtIndex:[@[@75, @100, @125, @150] indexOfObject:@(self.floatingToolbarScalePercent)]];
    // The menu steps 16, 18 … 28, so the index is the offset halved. Subtracting 16 alone indexes a 1pt menu that does not exist and runs off the end for anything above 22pt.
    [_toolbarFontSizeButton selectItemAtIndex:(self.floatingToolbarFontSize - 16) / 2];
    _transpositionToggle.state = self.autocorrectTransposition ? NSControlStateValueOn : NSControlStateValueOff;
    _neighborToggle.state = self.autocorrectNeighbor ? NSControlStateValueOn : NSControlStateValueOff;
    _candidateFollowCursorToggle.state = self.candidateFollowCursor ? NSControlStateValueOn : NSControlStateValueOff;
    _candidateLearningToggle.state = self.candidateLearningEnabled ? NSControlStateValueOn : NSControlStateValueOff;
    [_frequencyModeButton selectItemAtIndex:[FrequencyModes() indexOfObject:self.frequencyAdjustmentMode]];
    [_frequencyTriggerButton selectItemAtIndex:self.frequencyTriggerCount - 1];
    [_frequencyStepButton selectItemAtIndex:self.frequencyLinearStep - 1];
    _fuzzyPinyinToggle.state = self.fuzzyPinyinEnabled ? NSControlStateValueOn : NSControlStateValueOff;
    for (NSString *rule in _fuzzyPinyinRuleButtons)
        _fuzzyPinyinRuleButtons[rule].state =
            [self fuzzyPinyinRuleEnabled:rule] ? NSControlStateValueOn : NSControlStateValueOff;
    _cloudCandidatesToggle.state = self.cloudCandidates ? NSControlStateValueOn : NSControlStateValueOff;
    _candidateTranslationsToggle.state = self.candidateTranslations ? NSControlStateValueOn : NSControlStateValueOff;
    _candidateEnglishGlossToggle.state = self.candidateEnglishGloss ? NSControlStateValueOn : NSControlStateValueOff;
    _quanpinHelpcodeToggle.state = self.quanpinHelpcodeEnabled ? NSControlStateValueOn : NSControlStateValueOff;
    _shuangpinHelpcodeToggle.state = self.shuangpinHelpcodeEnabled ? NSControlStateValueOn : NSControlStateValueOff;
    for (NSButton *button in _localModeButtons)
        button.state = [self localModeEnabled:button.identifier] ? NSControlStateValueOn : NSControlStateValueOff;
    _inputModeShortcutToggle.state = self.inputModeShortcut ? NSControlStateValueOn : NSControlStateValueOff;
    _shiftTapShortcutToggle.state = self.shiftTapShortcut ? NSControlStateValueOn : NSControlStateValueOff;
    _controlTapShortcutToggle.state = self.controlTapShortcut ? NSControlStateValueOn : NSControlStateValueOff;
    _controlOptionSpaceShortcutToggle.state = self.controlOptionSpaceShortcut ? NSControlStateValueOn : NSControlStateValueOff;
    _characterSetShortcutToggle.state = self.characterSetShortcut ? NSControlStateValueOn : NSControlStateValueOff;
    [_layoutButton selectItemAtIndex:self.vertical ? 1 : 0];
    const NSInteger storedScheme = (NSInteger)[MSIMEInputSchemeNames() indexOfObject:self.inputScheme];
    // The radios and the scheme popups mirror the same stored value; which of the popups is usable
    // is in the dependency table with every other such rule.
    for (NSInteger index = 0; index < (NSInteger)_schemeButtons.count; ++index)
        _schemeButtons[index].state = index == storedScheme ? NSControlStateValueOn : NSControlStateValueOff;
    [self refreshInputModeHint];
    // Options that only apply to one scheme are shown only while it is selected. Leaving them
    // editable under another scheme means the change saves, the page says nothing, and the setting
    // does nothing until the user happens to switch back.
    _quanpinCard.hidden = storedScheme != 0;
    _shuangpinCard.hidden = storedScheme != 1;
    _wubiCard.hidden = storedScheme != 2;
    // 拼音匹配 is on another page than the scheme that decides whether it does anything, so the card says which scheme is selected rather than leaving a disabled group with no cause in sight.
    const BOOL pinyinMatching = storedScheme <= 2;
    NSString *schemeName = pinyinMatching ? nil : @[@"日语", @"韩语", @"粤拼", @"注音", @"越南语", @"藏文", @"笔画"][storedScheme - 3];
    _pinyinMatchingSchemeLabel.stringValue = pinyinMatching ? @"" : [NSString stringWithFormat:@"当前方案为%@，模糊音与全拼纠错只作用于拼音查询，在%@下不生效。", schemeName, schemeName];
    _pinyinMatchingSchemeLabel.hidden = pinyinMatching;
    NSDictionary *profileIndexes = @{@"xiaohe": @0, @"ziranma": @1, @"shoudao": @2, @"microsoft": @3};
    [_profileButton selectItemAtIndex:[profileIndexes[self.shuangpinProfile] integerValue]];
    const BOOL wubi98 = [self.wubiProfile isEqual:@"wubi98"];
    [_wubiSchemeButton selectItemAtIndex:wubi98 ? 1 : 0];
    _wubiProfileLabel.stringValue = wubi98 ? @"98 五笔" : @"86 五笔";
    [_preeditButton selectItemAtIndex:self.shuangpinPreeditUsesRaw ? 1 : 0];
    [_fontButton selectItemAtIndex:self.fontSize - 12];
    _englishFontFamilyControl.stringValue = self.candidateEnglishFont ?: @"";
    _fontFamilyControl.stringValue = self.fontFamily;
    _textColorField.stringValue = self.candidateTextColor ?: @"";
    _textColorWell.color = CandidateColor(self.candidateTextColor, [self candidateSkinColorForProperty:@"candidateTextColor"]);
    _inputModeHUDToggle.state = self.inputModeHUD ? NSControlStateValueOn : NSControlStateValueOff;
    [_themeModeButton selectItemAtIndex:(NSInteger)[ThemeModes() indexOfObject:self.themeMode]];
    [_candidateThemeButton selectItemAtIndex:(NSInteger)[SurfaceThemes() indexOfObject:self.candidateTheme]];
    // A well always shows a colour, so one that is not overridden shows the colour the candidate
    // window is drawing with — the skin's own token, under the appearance this window is being drawn
    // in — rather than black.
    // Iterating the wells rather than the table skips the skin lookups entirely in the input method,
    // where this runs on every setter and there is no window to have built them.
    for (NSString *property in _candidateColorWells)
        _candidateColorWells[property].color =
            CandidateColor([self valueForKey:property], [self candidateSkinColorForProperty:property]);
    [_preeditFontButton selectItemAtIndex:self.preeditFontSize - 12];
    _candidateScaleSlider.integerValue = self.candidateScalePercent;
    _candidateScaleLabel.stringValue = [NSString stringWithFormat:@"%ld%%", (long)self.candidateScalePercent];
    _candidateOpacitySlider.integerValue = self.candidateOpacityPercent;
    _candidateOpacityLabel.stringValue = [NSString stringWithFormat:@"%ld%%", (long)self.candidateOpacityPercent];
    // Following the skin, the slider and the figure show the radius the skin draws, and only a radius of the user's own can be put back.
    NSNumber *cornerRadius = self.candidateCornerRadius;
    if (_candidateCornerRadiusSlider != nil && cornerRadius == nil) {
        NSAppearanceName match = [NSApp.effectiveAppearance bestMatchFromAppearancesWithNames:@[ NSAppearanceNameAqua, NSAppearanceNameDarkAqua ]];
        cornerRadius = @(lround([self resolvedSkinForDark:[match isEqual:NSAppearanceNameDarkAqua]].tokens.radius));
    }
    _candidateCornerRadiusSlider.integerValue = cornerRadius.integerValue;
    _candidateCornerRadiusLabel.stringValue = [NSString stringWithFormat:@"%ld pt", (long)cornerRadius.integerValue];
    _candidateCornerRadiusReset.enabled = self.candidateCornerRadius != nil;
    [_candidateFontPresetButton selectItemAtIndex:self.candidateFontPreset];
    [_candidatePreeditButton selectItemAtIndex:self.showsCandidatePreedit ? 0 : 1];
    // -1 deselects, which is what the menu has to show for a state none of its three items describes: an NSPopUpButton showing 「Page Up / Page Down」 over an unticked Page Up / Page Down box is the menu naming a binding the user does not have. The line under it says where the setting actually is, so the empty menu is not the whole answer.
    const NSInteger pagingPreset = self.pageShortcut;
    [_pageShortcutButton selectItemAtIndex:pagingPreset];
    _pagingPresetLabel.stringValue = pagingPreset < 0 ? @"当前翻页按键不属于以上任何一组，由下方的「独立候选导航」决定。" : @"";
    _pagingPresetLabel.hidden = pagingPreset >= 0;
    [_pageSizeButton selectItemAtIndex:msime::mac::CandidatePageSizeOptionIndex(self.pageSize)];
    [_preview updatePanelStyle:self.vertical ? 1 : 0 pageSize:self.pageSize fontSize:self.fontSize];
    // It reads the toolbar settings itself; what it needs from here is being told that one of them has moved, including when the mover was an account push rather than a control on the page.
    [_toolbarPreview reloadPreview];
    // Everything above this line puts a value into a control. The three below are about the controls rather than their values — which of them the user may reach, which menu items another binding has taken, and which sections have anything to put back — and they are kept together here rather than interleaved with the assignments, so that each of those questions is answered in one place.
    for (NSArray *dependency in [self controlDependencies])
        for (NSControl *control in dependency[1]) control.enabled = [dependency[0] boolValue];
    [self refreshKeyBindingConflicts];
    [self refreshSectionRestoreLinks];
}
/// What the window lets the user reach, as one table rather than as whichever of a hundred
/// assignments happened to remember: a condition, and the controls that condition governs.
///
/// Four of these rules were in the file before and nine were not, so the window offered a word-frequency mode with learning switched off, toolbar buttons for a toolbar that is not shown, a helpcode scheme for helpcodes that are off, a preedit font size for a preedit that is hidden — a user configuring something the window itself had just turned off, and told nothing. A control belongs to exactly one entry: the loop that applies this assigns .enabled once per control, so a second entry naming the same control would be a rule that only sometimes wins.
- (NSArray<NSArray *> *)controlDependencies {
    // Every control below is nil until the pages are built, and -refreshControls runs long before
    // that: every setter calls it, including the ones the input method uses with no window open.
    if (_preferencePages == nil) return @[];
    const NSInteger scheme = (NSInteger)[MSIMEInputSchemeNames() indexOfObject:self.inputScheme];
    const BOOL learning = self.candidateLearningEnabled;
    const BOOL toolbar = self.floatingToolbarEnabled;
    const BOOL voice = self.voiceInputEnabled;
    NSDictionary *wordCharacter = [self wordCharacterOptions];
    NSMutableArray<NSArray *> *dependencies = [NSMutableArray arrayWithArray:@[
        // Only the selected scheme's own popup is usable, so a live row cannot look like it is
        // configuring the scheme that is actually in use.
        @[ @(scheme == 1), @[_shuangpinSchemeButton] ],
        @[ @(scheme == 2), @[_wubiSchemeButton] ],
        // Fuzzy rules and the two quanpin corrections reach the candidates of quanpin, shuangpin and wubi only: Japanese, Korean, Cantonese, Zhuyin and Vietnamese answer false to the Engine's `supports_fuzzy` and `supports_autocorrect`, and Korean's only candidates are the Hanja of the composing syllable, which no spelling rule reaches. refresh_candidates (crates/engine/src/ime/mod.rs) puts all three into the query request whatever the scheme is, and only the quanpin and shuangpin engines read them back out (crates/engine/src/quanpin/engine.rs, crates/engine/src/shuangpin/engine.rs); the Japanese provider never looks. 五笔 keeps them even though its own table ignores them too, because the same method builds a second, quanpin request carrying the same three values when 编码打不出时用拼音候选 is on and the table cannot answer the code — so under 五笔 they decide what that fallback offers.
        @[ @(scheme <= 2), @[_fuzzyPinyinToggle, _transpositionToggle, _neighborToggle] ],
        @[ @(self.fuzzyPinyinEnabled && scheme <= 2), _fuzzyPinyinRuleButtons.allValues ],
        // Both places the space conversion is read — InputController.mm, where a space after a
        // just-committed mark is rewritten — ask for 智能标点 first, so it does nothing without it.
        @[ @(self.smartPunctuation), @[_smartPunctuationSpaceToggle] ],
        @[ @(self.mixedEnglishInput), @[_mixedEnglishPrefixButton] ],
        @[ @(learning), @[_frequencyModeButton, _frequencyTriggerButton] ],
        // The step is read only by the linear mode — host-api passes frequency_linear_step to the engine whatever the mode is (crates/host-api/src/lib.rs) and the engine then ignores it — so it is live only where it does something.
        @[ @(learning && [self.frequencyAdjustmentMode isEqual:@"linear"]), @[_frequencyStepButton] ],
        @[ @(toolbar), @[_toolbarEnglishModeButton, _toolbarInputSchemeButton, _toolbarPunctuationButton, _toolbarFullWidthButton,
                         _toolbarCharacterSetButton, _toolbarEmojiButton, _toolbarHandwritingButton,
                         _toolbarScreenKeyboardButton, _toolbarVoiceButton, _toolbarSettingsButton,
                         _toolbarThemeButton, _toolbarScaleButton, _toolbarFontSizeButton] ],
        @[ @(self.quanpinHelpcodeEnabled),
           @[_helpcodeSchemaButtons[@"quanpin"], _helpcodeDisplayToggles[@"quanpin"]] ],
        @[ @(self.shuangpinHelpcodeEnabled),
           @[_helpcodeSchemaButtons[@"shuangpin"], _helpcodeDisplayToggles[@"shuangpin"]] ],
        @[ @(self.showsCandidatePreedit), @[_preeditFontButton] ],
        // Everything about dictation follows the master switch: InputController returns from -toggleVoiceInput: before it opens a microphone when it is off, and resets the hold shortcut on every event, so none of the rows under it reaches a recording.
        @[ @(voice), @[_voiceLanguageButton, _voiceSoundToggle, _voiceMuteSystemAudioToggle,
                       _voiceStreamInlinePreeditToggle, _voiceHotkeyCtrlF9Toggle, _voiceHotkeyRightAltToggle,
                       _voiceHotkeyCtrlCommandToggle, _voiceHotkeyCtrlOptionToggle] ],
        // The space lock is read by MSIMEVoiceHoldShortcut only while one of the three modifier holds is down; Control + F9 is a press handled by its own monitor and never reaches it.
        @[ @(voice && (self.voiceHotkeyRightAlt || self.voiceHotkeyCtrlCommand || self.voiceHotkeyCtrlOption)),
           @[_voiceHotkeyHoldSpaceToggle] ],
        // 以词定字 cannot be turned on for a key group that paging holds. The popup beneath it stays
        // live even so — it is the way off the contested group, and greying it out would leave the
        // pair with no way out at all.
        @[ @(![self navigationEnabled:wordCharacter[@"keys"]]), @[_wordCharacterToggle] ],
        // Nothing to delete the data of in a build that cannot uninstall.
        @[ @(msime_macos_uninstall_input_source != nullptr), @[_removeUserDataButton] ],
    ]];
    // A 跟随皮肤 button is an offer to drop an override, so it is live only where there is one to
    // drop. It is also the only thing on the row that says whether a colour is overridden at all: a
    // well shows a colour either way, because the skin has one for every row it draws.
    for (NSArray<NSString *> *entry in CandidateColorControls()) {
        NSButton *reset = _candidateColorResets[entry[0]];
        if (reset) [dependencies addObject:@[ @([self valueForKey:entry[0]] != nil), @[reset] ]];
    }
    return dependencies;
}
/// 以词定字 and 翻页 cannot be bound to the same key group, and the window used to answer the attempt
/// with a beep and a control that snapped back — a rejection that names neither what was refused nor
/// why. Whichever of the two holds a group now disables the other's way of taking it and says which
/// binding owns it, which is what the shared settings page does with the same pair
/// (packages/ui/src/index.tsx, where the word-to-character key radio is disabled by the paging
/// switch) and what the sentence below is taken word for word from.
- (void)refreshKeyBindingConflicts {
    NSDictionary *wordCharacter = [self wordCharacterOptions];
    NSString *heldByWordCharacter = [wordCharacter[@"enabled"] boolValue] ? wordCharacter[@"keys"] : nil;
    NSString *heldByPaging = [self navigationEnabled:wordCharacter[@"keys"]] ? wordCharacter[@"keys"] : nil;
    // The two groups both features can be bound to, named as both menus name them.
    NSDictionary<NSString *, NSString *> *groupTitles = @{@"brackets": @"[ / ]", @"minus_equal": @"- / ="};
    // The order the two menus were built in; each entry is the key group its item at that index binds.
    NSArray<NSString *> *wordCharacterItems = @[@"brackets", @"minus_equal"];
    NSArray<NSString *> *pagingPresetItems = @[@"minus_equal", @"brackets", @"page_up_down"];
    // A popup menu enables its own items by asking the target whether it can act, which would undo
    // every line below on the next redraw.
    _wordCharacterKeys.menu.autoenablesItems = NO;
    _pageShortcutButton.menu.autoenablesItems = NO;
    for (NSUInteger index = 0; index < wordCharacterItems.count && index < (NSUInteger)_wordCharacterKeys.numberOfItems; ++index)
        [_wordCharacterKeys itemAtIndex:(NSInteger)index].enabled = ![self navigationEnabled:wordCharacterItems[index]];
    for (NSUInteger index = 0; index < pagingPresetItems.count && index < (NSUInteger)_pageShortcutButton.numberOfItems; ++index)
        [_pageShortcutButton itemAtIndex:(NSInteger)index].enabled = ![pagingPresetItems[index] isEqual:heldByWordCharacter];
    for (NSButton *button in _navigationButtons) button.enabled = ![button.identifier isEqual:heldByWordCharacter];
    _navigationConflictLabel.stringValue = heldByWordCharacter == nil ? @"" :
        [NSString stringWithFormat:@"「%@」已用于以词定字，以词定字和翻页不能使用同一组快捷键。",
                                   groupTitles[heldByWordCharacter]];
    _navigationConflictLabel.hidden = heldByWordCharacter == nil;
    _wordCharacterConflictLabel.stringValue = heldByPaging == nil ? @"" :
        [NSString stringWithFormat:@"「%@」已用于翻页，以词定字和翻页不能使用同一组快捷键。",
                                   groupTitles[heldByPaging]];
    _wordCharacterConflictLabel.hidden = heldByPaging == nil;
}
/// A section offers to put itself back only when one of the settings under it is not at its default — whether it got there from this machine's storage or from a value an account pushed down. Fifteen links that are always there would be fifteen standing offers to undo nothing.
///
/// What a section owns is compared against what it would read with nothing stored and nothing pushed; see SettingProbes() for why the presence of a stored entry or of a `_shared*` ivar cannot answer this question.
- (void)refreshSectionRestoreLinks {
    // Nothing is registered until the pages are built, and -refreshControls runs long before that — every setter calls it, including the ones the input method uses with no window open. Leaving early also keeps DefaultSettingValues() from being built by a process that has no settings window to show.
    if (_restorableSections.count == 0) return;
    // A restore link is something to look at, so it is worth nothing while there is nothing to look at — and it is not free: this sweeps every registered key of every section through its accessor and compares the answer with the untouched default. -refreshControls runs on every preferencesChanged, which is every setter in the host, so paying for it there tripled the settings-heavy test binary's running time (13.3s to 41.1s on the same CI machine) and under a sanitizer pushed it past its budget. The window picks the links up when it appears and on every page change, which is every moment one can be seen.
    if (!_windowHasAppeared) return;
    NSDictionary<NSString *, MSIMESettingProbe> *probes = SettingProbes();
    NSDictionary<NSString *, id> *defaults = DefaultSettingValues();
    for (MSIMESettingsSection *section in _restorableSections) {
        BOOL restorable = NO;
        for (NSString *key in section.keys) {
            MSIMESettingProbe probe = probes[key];
            // -sectionHeader:keys:fields: asserts that every registered key has one, which is where a missing probe is meant to be caught; a release build with the assertions compiled out leaves the link where it is rather than calling a nil block.
            if (probe == nil) continue;
            NSArray<NSString *> *fields = section.fields[key];
            id current = probe(self), untouched = defaults[key];
            if (fields != nil) {
                current = [current dictionaryWithValuesForKeys:fields];
                untouched = [untouched dictionaryWithValuesForKeys:fields];
            }
            if ([current isEqual:untouched]) continue;
            restorable = YES;
            break;
        }
        section.restoreLink.hidden = !restorable;
    }
}
- (NSWindow *)window {
    NSWindow *window = [super window];
    if (!window) {
        [self loadWindow];
        window = [super window];
    }
    return window;
}
- (void)loadWindow {
    NSWindow *window = [[NSWindow alloc] initWithContentRect:NSMakeRect(0, 0, 840, 620)
                                                   styleMask:NSWindowStyleMaskTitled | NSWindowStyleMaskClosable |
                                                             NSWindowStyleMaskMiniaturizable | NSWindowStyleMaskResizable |
                                                             NSWindowStyleMaskFullSizeContentView
                                                     backing:NSBackingStoreBuffered
                                                       defer:NO];
    window.title = [MSIMEEditionDisplayName() stringByAppendingString:@"设置"];
    // A window the user is expected to come back to, at the size and place they left it. The
    // identifier is what makes restorable more than a flag: AppKit keys a window's saved state by
    // it, and a window without one is encoded into the saved-state bundle and then cannot be found
    // again.
    window.restorable = YES;
    window.identifier = MSIMESettingsWindowFrameAutosaveName();
    // The unified toolbar is where the title goes now, and it says which page is in front of the
    // user — the first thing in this window's chrome that ever did. A transparent titlebar was what
    // the hand-pinned sidebar needed to run full height behind it; the split view's sidebar item
    // does that itself, and asking for both leaves the toolbar drawing on nothing.
    window.titleVisibility = NSWindowTitleVisible;
    // 800 rather than 760: the sidebar can be dragged to kSidebarMaxWidth, and what is left after
    // it and the two page margins has to stay above kContentColumnMin.
    window.contentMinSize = NSMakeSize(800, 520);
    window.releasedWhenClosed = NO;
    // Filled by -sectionHeader:keys: as the pages below are built, and read by everything that asks
    // what this window can put back: the union that is -restorableKeys, the links that offer a
    // section, and -restoreDefaults: when one of them is pressed.
    _restorableSections = [NSMutableArray array];
    // Filled as the rows below are built and as each page is assembled; see the settings search registry. The map holds its header views weakly because the pages own them.
    _searchEntries = [NSMutableArray array];
    _pendingSearchEntries = [NSMutableArray array];
    _sectionTitlesByHeader = [NSMapTable weakToStrongObjectsMapTable];
    _layoutButton = [[NSPopUpButton alloc] initWithFrame:NSZeroRect pullsDown:NO];
    [_layoutButton addItemsWithTitles:@[@"横向排列", @"纵向列表"]];
    _layoutButton.accessibilityLabel = @"候选排列";
    _layoutButton.target = self;
    _layoutButton.action = @selector(layoutChanged:);
    _candidateFollowCursorToggle = MSIMESettingSwitch(self, @selector(candidateFollowCursorChanged:), @"候选窗口跟随光标");
    _profileButton = [[NSPopUpButton alloc] initWithFrame:NSZeroRect pullsDown:NO];
    for (const char *identifier : msime::mac::kShuangpinSchemaIdentifiers)
        [_profileButton addItemWithTitle:[NSString stringWithUTF8String:msime::mac::ShuangpinSchemaTitle(identifier)]];
    _profileButton.target = self;
    _profileButton.action = @selector(profileChanged:);
    _preeditButton = [[NSPopUpButton alloc] initWithFrame:NSZeroRect pullsDown:NO];
    [_preeditButton addItemsWithTitles:@[@"全拼显示", @"原始双拼显示"]];
    _preeditButton.target = self;
    _preeditButton.action = @selector(preeditChanged:);
    _fontButton = [[NSPopUpButton alloc] initWithFrame:NSZeroRect pullsDown:NO];
    for (NSUInteger size = 12; size <= 32; ++size)
        [_fontButton addItemWithTitle:[NSString stringWithFormat:@"%lu pt", (unsigned long)size]];
    _fontButton.accessibilityLabel = @"候选字号";
    _fontButton.target = self;
    _fontButton.action = @selector(fontChanged:);
    _englishFontFamilyControl = [[NSComboBox alloc] initWithFrame:NSZeroRect];
    [_englishFontFamilyControl addItemsWithObjectValues:[NSFontManager.sharedFontManager.availableFontFamilies sortedArrayUsingSelector:@selector(localizedCaseInsensitiveCompare:)]];
    _englishFontFamilyControl.completes = YES;
    _englishFontFamilyControl.accessibilityLabel = @"候选窗英文字体";
    _englishFontFamilyControl.target = self;
    _englishFontFamilyControl.action = @selector(englishFontFamilyChanged:);
    _fontFamilyControl = [[NSComboBox alloc] initWithFrame:NSZeroRect];
    [_fontFamilyControl addItemsWithObjectValues:[NSFontManager.sharedFontManager.availableFontFamilies sortedArrayUsingSelector:@selector(localizedCaseInsensitiveCompare:)]];
    _fontFamilyControl.completes = YES;
    _fontFamilyControl.accessibilityLabel = @"候选字体";
    _fontFamilyControl.target = self;
    _fontFamilyControl.action = @selector(fontFamilyChanged:);
    _textColorField = [[NSTextField alloc] initWithFrame:NSZeroRect];
    _textColorField.placeholderString = @"跟随皮肤（留空）";
    _textColorField.accessibilityLabel = @"候选文字颜色";
    _textColorField.target = self;
    _textColorField.action = @selector(textColorChanged:);
    [_textColorField.widthAnchor constraintEqualToConstant:110].active = YES;
    _textColorWell = [[NSColorWell alloc] initWithFrame:NSMakeRect(0, 0, 40, 24)];
    _textColorWell.accessibilityLabel = @"选择候选文字颜色";
    _textColorWell.target = self;
    _textColorWell.action = @selector(textColorWellChanged:);
    NSStackView *textColorControls = [NSStackView stackViewWithViews:@[_textColorField, _textColorWell,
        [NSButton buttonWithTitle:@"跟随皮肤" target:self action:@selector(resetTextColor:)]]];
    textColorControls.orientation = NSUserInterfaceLayoutOrientationHorizontal;
    // The six colours the candidate window draws with beside the text colour. Each row is a well and
    // the button that drops the override again; the property each one writes is carried as the
    // control's identifier, so one pair of actions serves all six.
    _candidateColorWells = [NSMutableDictionary dictionary];
    _candidateColorResets = [NSMutableDictionary dictionary];
    NSMutableArray<NSView *> *candidateColorRows = [NSMutableArray array];
    for (NSArray<NSString *> *entry in CandidateColorControls()) {
        NSColorWell *well = [[NSColorWell alloc] initWithFrame:NSMakeRect(0, 0, 40, 24)];
        well.identifier = entry[0];
        well.accessibilityLabel = [@"选择" stringByAppendingString:entry[1]];
        well.target = self;
        well.action = @selector(candidateColorWellChanged:);
        NSButton *reset = [NSButton buttonWithTitle:@"跟随皮肤" target:self action:@selector(resetCandidateColor:)];
        reset.identifier = entry[0];
        reset.accessibilityLabel = [entry[1] stringByAppendingString:@"跟随皮肤"];
        _candidateColorWells[entry[0]] = well;
        _candidateColorResets[entry[0]] = reset;
        NSStackView *controls = [NSStackView stackViewWithViews:@[ well, reset ]];
        controls.orientation = NSUserInterfaceLayoutOrientationHorizontal;
        [candidateColorRows addObject:[self settingRow:entry[1] control:controls]];
    }
    _themeModeButton = [[NSPopUpButton alloc] initWithFrame:NSZeroRect pullsDown:NO];
    [_themeModeButton addItemsWithTitles:@[@"跟随系统", @"深色", @"浅色"]];
    _themeModeButton.accessibilityLabel = @"颜色模式";
    _themeModeButton.target = self;
    _themeModeButton.action = @selector(themeModeChanged:);
    _candidateThemeButton = [[NSPopUpButton alloc] initWithFrame:NSZeroRect pullsDown:NO];
    [_candidateThemeButton addItemsWithTitles:@[@"跟随颜色模式", @"深色", @"浅色"]];
    _candidateThemeButton.accessibilityLabel = @"候选窗口主题";
    _candidateThemeButton.target = self;
    _candidateThemeButton.action = @selector(candidateThemeChanged:);
    _toolbarThemeButton = [[NSPopUpButton alloc] initWithFrame:NSZeroRect pullsDown:NO];
    [_toolbarThemeButton addItemsWithTitles:@[@"跟随颜色模式", @"深色", @"浅色"]];
    _toolbarThemeButton.accessibilityLabel = @"悬浮工具栏主题";
    _toolbarThemeButton.target = self;
    _toolbarThemeButton.action = @selector(toolbarThemeChanged:);
    _inputModeHUDToggle = MSIMESettingSwitch(self, @selector(inputModeHUDChanged:), @"切换中英文时显示提示");
    _preeditFontButton = [[NSPopUpButton alloc] initWithFrame:NSZeroRect pullsDown:NO];
    for (NSUInteger size = 12; size <= 32; ++size)
        [_preeditFontButton addItemWithTitle:[NSString stringWithFormat:@"%lu pt", (unsigned long)size]];
    _preeditFontButton.accessibilityLabel = @"候选窗拼音字号";
    _preeditFontButton.target = self;
    _preeditFontButton.action = @selector(preeditFontChanged:);
    // The three style settings are a size, a fade and a curve the user judges by looking at the preview above, so they are sliders that stop on the steps the shared settings page offers, each with the value it stands at printed beside it.
    NSSlider *(^styleSlider)(double, double, double, NSString *) = ^NSSlider *(double minimum, double maximum, double step, NSString *label) {
        NSSlider *slider = [NSSlider sliderWithValue:minimum minValue:minimum maxValue:maximum target:self action:@selector(candidateStyleSliderChanged:)];
        slider.numberOfTickMarks = (NSInteger)lround((maximum - minimum) / step) + 1;
        slider.allowsTickMarkValuesOnly = YES;
        slider.continuous = YES;
        slider.accessibilityLabel = label;
        [slider.widthAnchor constraintEqualToConstant:180.0].active = YES;
        return slider;
    };
    NSTextField *(^styleValueLabel)(void) = ^NSTextField * {
        NSTextField *label = [NSTextField labelWithString:@""];
        label.font = [NSFont monospacedDigitSystemFontOfSize:msime::mac::layout::kBodyFontSize weight:NSFontWeightRegular];
        label.alignment = NSTextAlignmentRight;
        [label.widthAnchor constraintEqualToConstant:52.0].active = YES;
        return label;
    };
    _candidateScaleSlider = styleSlider(75.0, 150.0, 5.0, @"整体大小");
    _candidateScaleLabel = styleValueLabel();
    NSStackView *scaleControls = [NSStackView stackViewWithViews:@[ _candidateScaleSlider, _candidateScaleLabel ]];
    scaleControls.orientation = NSUserInterfaceLayoutOrientationHorizontal;
    _candidateOpacitySlider = styleSlider(50.0, 100.0, 5.0, @"不透明度");
    _candidateOpacityLabel = styleValueLabel();
    NSStackView *opacityControls = [NSStackView stackViewWithViews:@[ _candidateOpacitySlider, _candidateOpacityLabel ]];
    opacityControls.orientation = NSUserInterfaceLayoutOrientationHorizontal;
    _candidateCornerRadiusSlider = styleSlider(0.0, 16.0, 1.0, @"圆角大小");
    _candidateCornerRadiusLabel = styleValueLabel();
    _candidateCornerRadiusReset = [NSButton buttonWithTitle:@"跟随皮肤" target:self action:@selector(resetCandidateCornerRadius:)];
    _candidateCornerRadiusReset.accessibilityLabel = @"圆角大小跟随皮肤";
    NSStackView *cornerRadiusControls = [NSStackView stackViewWithViews:@[ _candidateCornerRadiusSlider, _candidateCornerRadiusLabel, _candidateCornerRadiusReset ]];
    cornerRadiusControls.orientation = NSUserInterfaceLayoutOrientationHorizontal;
    _candidateFontPresetButton = [[NSPopUpButton alloc] initWithFrame:NSZeroRect pullsDown:NO];
    NSArray<NSArray<NSString *> *> *fontPresets = CandidateFontPresets();
    for (NSUInteger index = 0; index < fontPresets.count; ++index) {
        // 默认 is whatever the shared default resolves to; a named preset whose family this Mac does not have says so, because choosing it then draws the fallback list rather than the face its name promises.
        NSArray<NSString *> *preset = fontPresets[index];
        const BOOL missing = index > 0 && MSIMEInstalledFontFamilyDescriptor(preset[1]) == nil;
        [_candidateFontPresetButton addItemWithTitle:missing ? [preset[0] stringByAppendingString:@"（未安装）"] : preset[0]];
    }
    _candidateFontPresetButton.accessibilityLabel = @"字体预设";
    _candidateFontPresetButton.target = self;
    _candidateFontPresetButton.action = @selector(candidateFontPresetChanged:);
    _candidatePreeditButton = [[NSPopUpButton alloc] initWithFrame:NSZeroRect pullsDown:NO];
    [_candidatePreeditButton addItemsWithTitles:@[@"显示拼音", @"隐藏"]];
    _candidatePreeditButton.accessibilityLabel = @"候选窗预编辑";
    _candidatePreeditButton.target = self;
    _candidatePreeditButton.action = @selector(candidatePreeditChanged:);
    _pageShortcutButton = [[NSPopUpButton alloc] initWithFrame:NSZeroRect pullsDown:NO];
    [_pageShortcutButton addItemsWithTitles:@[@"- / =", @"[ / ]", @"Page Up / Page Down"]];
    _pageShortcutButton.accessibilityLabel = @"候选翻页快捷键";
    _pageShortcutButton.target = self;
    _pageShortcutButton.action = @selector(pageShortcutChanged:);
    _wordCharacterToggle = MSIMESettingSwitch(self, @selector(wordCharacterChanged:), @"以词定字");
    _wordCharacterKeys = [[NSPopUpButton alloc] initWithFrame:NSZeroRect pullsDown:NO];
    [_wordCharacterKeys addItemsWithTitles:@[@"[ / ]", @"- / ="]];
    _wordCharacterKeys.accessibilityLabel = @"以词定字键组";
    _wordCharacterKeys.target = self;
    _wordCharacterKeys.action = @selector(wordCharacterChanged:);
    _navigationButtons = [NSMutableArray array];
    for (NSArray *entry in NavigationControls()) {
        NSButton *button = [NSButton checkboxWithTitle:entry[1] target:self action:@selector(navigationChanged:)];
        button.identifier = entry[0];
        button.accessibilityLabel = entry[1];
        [_navigationButtons addObject:button];
    }
    _pageSizeButton = [[NSPopUpButton alloc] initWithFrame:NSZeroRect pullsDown:NO];
    for (NSUInteger index = 0; index < msime::mac::kOfferedCandidatePageSizes; ++index)
        [_pageSizeButton addItemWithTitle:[NSString stringWithFormat:@"%lu 个",
            (unsigned long)msime::mac::CandidatePageSizeForOptionIndex(index)]];
    _pageSizeButton.accessibilityLabel = @"每页候选";
    _pageSizeButton.target = self;
    _pageSizeButton.action = @selector(pageSizeChanged:);
    _inputModeShortcutToggle = MSIMESettingSwitch(self, @selector(inputModeShortcutChanged:), @"Shift + 空格切换中英文");
    _defaultImeModeButton = [[NSPopUpButton alloc] initWithFrame:NSZeroRect pullsDown:NO];
    [_defaultImeModeButton addItemsWithTitles:@[@"中文", @"英文"]];
    _defaultImeModeButton.target = self; _defaultImeModeButton.action = @selector(defaultImeModeChanged:);
    _imeModeScopeButton = [[NSPopUpButton alloc] initWithFrame:NSZeroRect pullsDown:NO];
    [_imeModeScopeButton addItemsWithTitles:@[@"按应用", @"全局"]];
    _imeModeScopeButton.target = self; _imeModeScopeButton.action = @selector(imeModeScopeChanged:);
    // 应用例外. 模式作用范围 has offered 按应用 all along with no list of applications anywhere behind it, and the only thing that was per-application was a dictionary in memory that the next input-source switch emptied — so 按应用 meant "until you switch away", and the window said none of it.
    _appRuleNames = [NSMutableDictionary dictionary];
    _appRuleTable = [[NSTableView alloc] initWithFrame:NSZeroRect];
    _appRuleTable.accessibilityLabel = @"应用例外";
    _appRuleTable.headerView = nil;
    _appRuleTable.allowsMultipleSelection = NO;
    _appRuleTable.rowHeight = 24.0;
    _appRuleTable.style = NSTableViewStyleFullWidth;
    NSTableColumn *appRuleApplication = [[NSTableColumn alloc] initWithIdentifier:MSIMEAppRuleApplicationColumn];
    appRuleApplication.resizingMask = NSTableColumnAutoresizingMask;
    [_appRuleTable addTableColumn:appRuleApplication];
    NSTableColumn *appRuleMode = [[NSTableColumn alloc] initWithIdentifier:MSIMEAppRuleModeColumn];
    appRuleMode.width = 108.0;
    appRuleMode.resizingMask = NSTableColumnNoResizing;
    [_appRuleTable addTableColumn:appRuleMode];
    _appRuleTable.dataSource = self;
    _appRuleTable.delegate = self;
    NSScrollView *appRuleScroll = [[NSScrollView alloc] initWithFrame:NSZeroRect];
    appRuleScroll.documentView = _appRuleTable;
    appRuleScroll.hasVerticalScroller = YES;
    appRuleScroll.borderType = NSBezelBorder;
    appRuleScroll.translatesAutoresizingMaskIntoConstraints = NO;
    [appRuleScroll.heightAnchor constraintEqualToConstant:112.0].active = YES;
    [appRuleScroll.widthAnchor constraintGreaterThanOrEqualToConstant:msime::mac::layout::kControlMinWidth].active = YES;
    NSButton *addAppRule = [NSButton buttonWithTitle:@"添加应用…" target:self action:@selector(addApplicationInputModeRule:)];
    addAppRule.accessibilityLabel = @"添加应用例外";
    _appRuleRemoveButton = [NSButton buttonWithTitle:@"移除" target:self action:@selector(removeApplicationInputModeRule:)];
    _appRuleRemoveButton.accessibilityLabel = @"移除应用例外";
    // How many rules there are, and — when the panel is handed something that is not an application — why the last press added nothing.
    _appRuleStatusLabel = MSIMEDetailLabel(@"");
    NSStackView *appRuleButtons = [NSStackView stackViewWithViews:@[ addAppRule, _appRuleRemoveButton ]];
    appRuleButtons.orientation = NSUserInterfaceLayoutOrientationHorizontal;
    NSStackView *appRuleControls = [NSStackView stackViewWithViews:@[ appRuleScroll, appRuleButtons ]];
    appRuleControls.orientation = NSUserInterfaceLayoutOrientationVertical;
    appRuleControls.alignment = NSLayoutAttributeLeading;
    appRuleControls.spacing = 6.0;
    [appRuleScroll.widthAnchor constraintEqualToAnchor:appRuleControls.widthAnchor].active = YES;
    _shiftTapShortcutToggle = MSIMESettingSwitch(self, @selector(shiftTapShortcutChanged:), @"单按 Shift 切换中英文");
    _controlTapShortcutToggle = MSIMESettingSwitch(self, @selector(controlTapShortcutChanged:), @"单按 Control 切换中英文");
    _controlOptionSpaceShortcutToggle = MSIMESettingSwitch(self, @selector(controlOptionSpaceShortcutChanged:), @"Control + Option + 空格切换中英文");
    _characterSetShortcutToggle = MSIMESettingSwitch(self, @selector(characterSetShortcutChanged:), @"Control + Shift + F 切换简繁");
    _fullWidthToggle = MSIMESettingSwitch(self, @selector(fullWidthChanged:), @"全角输入（Option + Shift + H）");
    _fullWidthShortcutToggle = MSIMESettingSwitch(self, @selector(fullWidthShortcutChanged:), @"Option + Shift + H 切换全半角");
    _voiceEnabledToggle = MSIMESettingSwitch(self, @selector(voiceEnabledChanged:), @"启用语音输入");
    _voiceLanguageButton = [[NSPopUpButton alloc] initWithFrame:NSZeroRect pullsDown:NO];
    [_voiceLanguageButton addItemsWithTitles:@[@"中文（简体）", @"English"]];
    _voiceLanguageButton.accessibilityLabel = @"识别语言";
    _voiceLanguageButton.target = self;
    _voiceLanguageButton.action = @selector(voiceLanguageChanged:);
    _voiceSoundToggle = MSIMESettingSwitch(self, @selector(voiceSoundChanged:), @"播放提示音");
    _voiceMuteSystemAudioToggle = MSIMESettingSwitch(self, @selector(voiceMuteSystemAudioChanged:), @"录音时静音系统音频");
    _voiceStreamInlinePreeditToggle =
        MSIMESettingSwitch(self, @selector(voiceStreamInlinePreeditChanged:), @"实时显示识别结果");
    _voiceHotkeyCtrlF9Toggle = MSIMESettingSwitch(self, @selector(voiceHotkeyCtrlF9Changed:), @"Control + F9 开始语音输入");
    _voiceHotkeyRightAltToggle = MSIMESettingSwitch(self, @selector(voiceHotkeyRightAltChanged:), @"按住右 Option 说话");
    _voiceHotkeyCtrlCommandToggle =
        MSIMESettingSwitch(self, @selector(voiceHotkeyCtrlCommandChanged:), @"按住 Control + Command 说话");
    _voiceHotkeyCtrlOptionToggle =
        MSIMESettingSwitch(self, @selector(voiceHotkeyCtrlOptionChanged:), @"按住右 Control + Option 说话");
    _voiceHotkeyHoldSpaceToggle =
        MSIMESettingSwitch(self, @selector(voiceHotkeyHoldSpaceChanged:), @"按住说话时按空格锁定录音");
    _traditionalOutputToggle = MSIMESettingSwitch(self, @selector(traditionalOutputChanged:), @"简繁输入");
    _keymapToggle = MSIMESettingSwitch(self, @selector(keymapChanged:), @"输入时显示双拼键位提示");
    _wubiToggle = MSIMESettingSwitch(self, @selector(wubiChanged:), @"五笔四码唯一候选自动上屏");
    _wubiMixedPinyinToggle = MSIMESettingSwitch(self, @selector(wubiMixedPinyinChanged:), @"编码打不出时用拼音候选");
    _punctuationToggle = MSIMESettingSwitch(self, @selector(punctuationChanged:), @"中文标点");
    _smartPunctuationToggle = MSIMESettingSwitch(self, @selector(smartPunctuationChanged:), @"智能标点");
    _smartPunctuationRepeatToggle = MSIMESettingSwitch(self, @selector(smartPunctuationRepeatChanged:), @"重复标点转中文");
    _smartPunctuationSpaceToggle = MSIMESettingSwitch(self, @selector(smartPunctuationSpaceChanged:), @"中文标点后按空格转换");
    _pairedPunctuationToggle = MSIMESettingSwitch(self, @selector(pairedPunctuationChanged:), @"成对标点");
    _punctuationLockButton = [[NSPopUpButton alloc] initWithFrame:NSZeroRect pullsDown:NO];
    [_punctuationLockButton
        addItemsWithTitles:@[ @"跟随中英文状态", @"始终使用中文标点", @"始终使用英文标点" ]];
    _punctuationLockButton.accessibilityLabel = @"固定标点";
    _punctuationLockButton.target = self;
    _punctuationLockButton.action = @selector(punctuationLockChanged:);
    _mixedEnglishToggle = MSIMESettingSwitch(self, @selector(mixedEnglishChanged:), @"中英混输");
    _mixedEnglishPrefixButton = [[NSPopUpButton alloc] initWithFrame:NSZeroRect pullsDown:NO];
    NSMutableArray<NSString *> *mixedPrefixes = [NSMutableArray array];
    for (NSInteger prefix = 1; prefix <= 8; ++prefix)
        [mixedPrefixes addObject:[NSString stringWithFormat:@"%ld 个字符", (long)prefix]];
    [_mixedEnglishPrefixButton addItemsWithTitles:mixedPrefixes];
    _mixedEnglishPrefixButton.accessibilityLabel = @"中英混输触发字符数";
    _mixedEnglishPrefixButton.target = self;
    _mixedEnglishPrefixButton.action = @selector(mixedEnglishPrefixChanged:);
    _mixedEmojiToggle = MSIMESettingSwitch(self, @selector(mixedEmojiChanged:), @"Emoji 混输");
    _mixedKaomojiToggle = MSIMESettingSwitch(self, @selector(mixedKaomojiChanged:), @"颜文字混输");
    _toolbarToggle = MSIMESettingSwitch(self, @selector(toolbarChanged:), @"显示浮动工具栏");
    // 十个复选框，对应 MSIMEFloatingToolbarPanel 画的十个按钮。中/英、手写和语音三个曾经没有开关，按钮在工具栏上却无法去掉；切换方案按钮是后加的，默认开启。
    _toolbarEnglishModeButton = [NSButton checkboxWithTitle:@"中英文按钮" target:self action:@selector(toolbarEnglishModeChanged:)];
    _toolbarPunctuationButton = [NSButton checkboxWithTitle:@"标点按钮" target:self action:@selector(toolbarPunctuationChanged:)];
    _toolbarFullWidthButton = [NSButton checkboxWithTitle:@"全半角按钮" target:self action:@selector(toolbarFullWidthChanged:)];
    _toolbarCharacterSetButton = [NSButton checkboxWithTitle:@"简繁按钮" target:self action:@selector(toolbarCharacterSetChanged:)];
    _toolbarEmojiButton = [NSButton checkboxWithTitle:@"Emoji 按钮" target:self action:@selector(toolbarEmojiChanged:)];
    _toolbarHandwritingButton = [NSButton checkboxWithTitle:@"手写按钮" target:self action:@selector(toolbarHandwritingChanged:)];
    _toolbarInputSchemeButton = [NSButton checkboxWithTitle:@"切换方案按钮" target:self action:@selector(toolbarInputSchemeChanged:)];
    _toolbarScreenKeyboardButton = [NSButton checkboxWithTitle:@"屏幕键盘按钮" target:self action:@selector(toolbarScreenKeyboardChanged:)];
    _toolbarVoiceButton = [NSButton checkboxWithTitle:@"语音按钮" target:self action:@selector(toolbarVoiceChanged:)];
    _toolbarSettingsButton = [NSButton checkboxWithTitle:@"设置按钮" target:self action:@selector(toolbarSettingsChanged:)];
    _toolbarScaleButton = [[NSPopUpButton alloc] initWithFrame:NSZeroRect pullsDown:NO];
    [_toolbarScaleButton addItemsWithTitles:@[@"75%", @"100%", @"125%", @"150%"]];
    _toolbarScaleButton.accessibilityLabel = @"工具栏缩放";
    _toolbarScaleButton.target = self; _toolbarScaleButton.action = @selector(toolbarScaleChanged:);
    _toolbarFontSizeButton = [[NSPopUpButton alloc] initWithFrame:NSZeroRect pullsDown:NO];
    for (NSInteger size = 16; size <= 28; size += 2)
        [_toolbarFontSizeButton addItemWithTitle:[NSString stringWithFormat:@"%ld pt", (long)size]];
    _toolbarFontSizeButton.accessibilityLabel = @"工具栏字号";
    _toolbarFontSizeButton.target = self; _toolbarFontSizeButton.action = @selector(toolbarFontSizeChanged:);
    _transpositionToggle = MSIMESettingSwitch(self, @selector(transpositionChanged:), @"全拼乱序纠错（sahng → shang）");
    _neighborToggle = MSIMESettingSwitch(self, @selector(neighborChanged:), @"全拼邻键纠错（shabg → shang）");
    _candidateLearningToggle = MSIMESettingSwitch(self, @selector(candidateLearningChanged:), @"学习候选词频");
    _frequencyModeButton = [[NSPopUpButton alloc] initWithFrame:NSZeroRect pullsDown:NO];
    [_frequencyModeButton addItemsWithTitles:@[@"关闭", @"置顶", @"折半", @"线性", @"置前"]];
    _frequencyModeButton.target = self;
    _frequencyModeButton.action = @selector(frequencyModeChanged:);
    _frequencyTriggerButton = [[NSPopUpButton alloc] initWithFrame:NSZeroRect pullsDown:NO];
    _frequencyStepButton = [[NSPopUpButton alloc] initWithFrame:NSZeroRect pullsDown:NO];
    NSMutableArray<NSString *> *frequencyCounts = [NSMutableArray array];
    for (NSInteger count = 1; count <= 10; ++count)
        [frequencyCounts addObject:[NSString stringWithFormat:@"%ld 次", (long)count]];
    [_frequencyTriggerButton addItemsWithTitles:frequencyCounts];
    [_frequencyStepButton addItemsWithTitles:frequencyCounts];
    _frequencyTriggerButton.target = self;
    _frequencyTriggerButton.action = @selector(frequencyTriggerChanged:);
    _frequencyStepButton.target = self;
    _frequencyStepButton.action = @selector(frequencyStepChanged:);
    _fuzzyPinyinToggle = MSIMESettingSwitch(self, @selector(fuzzyPinyinChanged:), @"启用模糊音");
    // The accessibility label keeps the whole sentence the visible label used to carry: it is the
    // name this switch answers to, and the tests find it by that name.
    _cloudCandidatesToggle = MSIMESettingSwitch(self, @selector(cloudCandidatesChanged:), @"启用云候选（将查询发送至 Google 输入工具）");
    _candidateTranslationsToggle = MSIMESettingSwitch(self, @selector(candidateTranslationsChanged:), @"显示候选释义");
    _candidateEnglishGlossToggle = MSIMESettingSwitch(self, @selector(candidateEnglishGlossChanged:), @"显示离线英文释义");
    _quanpinHelpcodeToggle = MSIMESettingSwitch(self, @selector(quanpinHelpcodeChanged:), @"启用全拼辅助码");
    _shuangpinHelpcodeToggle = MSIMESettingSwitch(self, @selector(shuangpinHelpcodeChanged:), @"启用双拼辅助码");
    _helpcodeSchemaButtons = [NSMutableDictionary dictionary];
    _helpcodeDisplayToggles = [NSMutableDictionary dictionary];
    _fuzzyPinyinRuleButtons = [NSMutableDictionary dictionary];
    _localModeButtons = [NSMutableArray array];

    // ---- 输入方案 ---------------------------------------------------------------------------
    NSBox *inputModeCard = MSIMECardWithViews(@[
        [self settingRow:@"输入模式" control:_defaultImeModeButton aka:@[@"中文", @"英文", @"默认"]],
        [self settingRow:@"模式作用范围"
                  detail:@"下一次激活时生效；中英文状态仅在当前输入法进程内记忆。"
                 control:_imeModeScopeButton
                     aka:@[@"按应用", @"全局"]],
    ], 0.0);
    inputModeCard.accessibilityLabel = @"输入模式卡片";
    NSBox *appRuleCard = MSIMECardWithViews(@[
        MSIMEDetailLabel(@"这里的规则优先于「模式作用范围」和记忆：规则保存在本机，切换输入源或重新登录后仍然有效。没有规则的应用按记忆走，而记忆只存在于当前这次输入法进程里，切换到别的输入源就清空了。"),
        [self registerSearchRow:MSIMEStackedPreferenceRow(@"按应用指定输入模式", _appRuleStatusLabel, appRuleControls)
                          named:@"按应用指定输入模式"
                            aka:@[@"应用例外", @"白名单"]],
    ], 6.0);
    appRuleCard.accessibilityLabel = @"应用例外卡片";

    // The scheme is one choice, so it reads as radios with each scheme's own popup trailing it,
    // disabled until that scheme is selected. The stored value stays the same scheme string.
    // In MSIMEInputSchemeNames order, which is also each radio's tag. Cantonese, Zhuyin and Stroke need their dictionary installed beside the resources; without it the radio is disabled and says why, since the Engine would fall back to another scheme.
    NSArray<NSString *> *schemeTitles = @[@"全拼输入", @"双拼输入", @"五笔输入", @"日语输入", @"韩语输入", @"粤拼输入", @"注音输入", @"越南语输入", @"藏文输入", @"笔画输入"];
    NSDictionary *hostOptions = MSIMELoadRuntimeOptions();
    NSMutableArray<NSButton *> *schemeButtons = [NSMutableArray array];
    NSMutableArray<NSView *> *schemeRows = [NSMutableArray arrayWithObjects:MSIMECardHeader(@"输入方式"), MSIMECardSeparator(), nil];
    _shuangpinSchemeButton = _profileButton;
    _wubiSchemeButton = [[NSPopUpButton alloc] initWithFrame:NSZeroRect pullsDown:NO];
    [_wubiSchemeButton addItemsWithTitles:@[@"86 五笔", @"98 五笔"]];
    _wubiSchemeButton.accessibilityLabel = @"五笔方案";
    _wubiSchemeButton.target = self;
    _wubiSchemeButton.action = @selector(wubiProfileChanged:);
    for (NSInteger index = 0; index < (NSInteger)schemeTitles.count; ++index) {
        NSButton *button = [NSButton radioButtonWithTitle:schemeTitles[index] target:self action:@selector(schemeRadioChanged:)];
        button.tag = index;
        button.font = [NSFont systemFontOfSize:13.0 weight:NSFontWeightMedium];
        button.accessibilityLabel = schemeTitles[index];
        if (!MSIMEInputSchemeAvailable(MSIMEInputSchemeNames()[index], hostOptions)) {
            button.enabled = NO;
            button.toolTip = @"未安装该方案的词库，暂不可用";
        }
        [schemeButtons addObject:button];
        NSView *accessory = index == 1 ? _shuangpinSchemeButton : (index == 2 ? _wubiSchemeButton : nil);
        // 本版本没有的方案不列出来（而不是显示为不可用）；按钮照样建好，_schemeButtons 仍按方案编号取。
        if (!MSIMEEditionOffersScheme(MSIMEInputSchemeNames()[index])) continue;
        [self registerSearchRow:button named:schemeTitles[index] aka:@[@"输入方案"]];
        if (schemeRows.count > 2) [schemeRows addObject:MSIMECardSeparator()];
        [schemeRows addObject:SchemeChoiceRow(button, accessory)];
    }
    _schemeButtons = schemeButtons;
    // macOS 27 不允许进程启用键盘输入模式，选中粤拼、注音、笔画这类方案后菜单栏里不会自动出现对应入口；这一行说明它在系统设置「添加」对话框的哪个语言下。
    _inputModeHintLabel = MSIMEDetailLabel(@"");
    NSButton *inputModeHintButton = [NSButton buttonWithTitle:@"打开键盘设置" target:self action:@selector(openInputSourceSettings:)];
    inputModeHintButton.controlSize = NSControlSizeSmall;
    NSStackView *inputModeHint = [NSStackView stackViewWithViews:@[_inputModeHintLabel, inputModeHintButton]];
    inputModeHint.orientation = NSUserInterfaceLayoutOrientationVertical;
    inputModeHint.alignment = NSLayoutAttributeLeading;
    inputModeHint.spacing = 6.0;
    inputModeHint.edgeInsets = NSEdgeInsetsMake(8.0, 0.0, 8.0, 0.0);
    inputModeHint.accessibilityLabel = @"菜单栏入口提示";
    [_inputModeHintLabel.widthAnchor constraintEqualToAnchor:inputModeHint.widthAnchor].active = YES;
    inputModeHint.hidden = YES;
    _inputModeHintRow = inputModeHint;
    [schemeRows addObject:inputModeHint];
    NSBox *schemeCard = MSIMECardWithViews(schemeRows, 0.0);
    schemeCard.accessibilityLabel = @"输入方式卡片";

    // The helpcode rows, which belong to the scheme they qualify and now sit under it. They were a
    // page of their own, reachable under every scheme — so 五笔 and 日语 users met six controls that
    // could be moved and saved and that nothing would ever read, and 全拼 and 双拼 users met the
    // other scheme's three beside their own. Both sets are built whichever scheme is selected and the
    // one that does not apply is hidden rather than skipped: the tests walk hidden pages, and they
    // pin two of each.
    NSMutableArray<NSView *> *quanpinHelpcodeRows = [NSMutableArray array];
    NSMutableArray<NSView *> *shuangpinHelpcodeRows = [NSMutableArray array];
    for (NSString *scheme in @[@"quanpin", @"shuangpin"]) {
        NSString *name = [scheme isEqual:@"quanpin"] ? @"全拼" : @"双拼";
        NSPopUpButton *schemas = [[NSPopUpButton alloc] initWithFrame:NSZeroRect pullsDown:NO];
        [schemas addItemsWithTitles:@[@"蓝天小雨点", @"自然码", @"首右2.0", @"首右plus", @"小鹤", @"加加"]];
        for (NSUInteger index = 0; index < HelpcodeSchemas().count; ++index)
            [schemas itemAtIndex:index].representedObject = HelpcodeSchemas()[index];
        schemas.identifier = scheme;
        schemas.target = self;
        schemas.action = @selector(helpcodeSchemaChanged:);
        schemas.accessibilityLabel = [name stringByAppendingString:@"辅助码方案"];
        NSString *displayTitle = [NSString stringWithFormat:@"在候选窗口中显示%@辅助码", name];
        NSSwitch *display = MSIMESettingSwitch(self, @selector(helpcodeDisplayChanged:), displayTitle);
        display.identifier = scheme;
        _helpcodeSchemaButtons[scheme] = schemas;
        _helpcodeDisplayToggles[scheme] = display;
        NSMutableArray<NSView *> *rows = [scheme isEqual:@"quanpin"] ? quanpinHelpcodeRows : shuangpinHelpcodeRows;
        NSSwitch *master = [scheme isEqual:@"quanpin"] ? _quanpinHelpcodeToggle : _shuangpinHelpcodeToggle;
        [rows addObject:[self settingRow:[NSString stringWithFormat:@"启用%@辅助码", name]
                                  detail:@"在拼音后再打一个形码，缩小候选范围。"
                                 control:master
                                     aka:@[@"形码"]]];
        [rows addObject:[self settingRow:schemas.accessibilityLabel control:schemas]];
        [rows addObject:[self settingRow:displayTitle control:display]];
    }

    // The options belonging to one scheme follow the scheme card and appear only while that scheme
    // is the selected one. Upstream shows the 双拼 options whatever is selected — editable, saved,
    // and with no effect until you come back and pick 双拼 — and puts the 五笔 options on a page of
    // their own reached by a link, with a 返回键盘输入 button to get out. That is a web flow inside
    // a sidebar window: the sidebar stays on the scheme page while the content is somewhere else.
    _quanpinCard = MSIMECardWithViews(quanpinHelpcodeRows, 0.0);
    _quanpinCard.accessibilityLabel = @"全拼选项卡片";
    NSMutableArray<NSView *> *shuangpinRows = [NSMutableArray arrayWithObjects:
        [self settingRow:@"双拼预编辑" control:_preeditButton],
        // Upstream labels this row 双拼初学者 and puts the wording on the checkbox beside it, so the
        // row says the same thing twice. The switch carries no text, so the label carries it.
        [self settingRow:@"输入时显示双拼键位提示" control:_keymapToggle aka:@[@"双拼初学者"]], nil];
    [shuangpinRows addObjectsFromArray:shuangpinHelpcodeRows];
    _shuangpinCard = MSIMECardWithViews(shuangpinRows, 0.0);
    _shuangpinCard.accessibilityLabel = @"双拼选项卡片";
    // 版本在「五笔输入」旁的弹出菜单里选，这里只显示当前所用的码表。
    _wubiProfileLabel = [NSTextField labelWithString:[self.wubiProfile isEqual:@"wubi98"] ? @"98 五笔" : @"86 五笔"];
    _wubiProfileLabel.textColor = [NSColor secondaryLabelColor];
    _wubiCard = MSIMECardWithViews(@[
        [self settingRow:@"编码方案" control:_wubiProfileLabel aka:@[@"86 五笔", @"98 五笔", @"五笔版本"]],
        [self settingRow:@"四码唯一候选自动上屏" control:_wubiToggle],
        [self settingRow:@"编码打不出时用拼音候选"
                  detail:@"五笔词库无法回答当前编码时，用同一串字母查询全拼；词库能回答时不影响。"
                 control:_wubiMixedPinyinToggle
                     aka:@[@"五笔拼音混输"]],
    ], 0.0);
    _wubiCard.accessibilityLabel = @"五笔选项卡片";

    NSScrollView *schemePage = [self page:MSIMESettingsPageInputScheme
                                    title:@"输入方案"
                                  summary:@"选择打什么、怎么打。下面的选项随所选方案变化。"
                                  content:@[
        // The first card is the only one on the page that had no heading, which also left the two
        // settings on it in no section and so out of reach of a section-level restore.
        [self sectionHeader:@"输入模式" keys:@[DefaultImeModeKey, ImeModeScopeKey]], inputModeCard,
        [self sectionHeader:@"应用例外" keys:@[AppInputModeRulesKey]], appRuleCard,
        [self sectionHeader:@"中文输入方案"
                       keys:@[SchemeKey, ShuangpinProfileKey, ShuangpinPreeditKey, KeymapKey, WubiKey, WubiProfileKey,
                              WubiMixedPinyinKey, HelpcodeOptionsKey, QuanpinHelpcodeKey,
                              ShuangpinHelpcodeKey]],
        schemeCard, _quanpinCard, _shuangpinCard, _wubiCard,
    ]];

    // ---- 输入习惯 ---------------------------------------------------------------------------
    NSBox *punctuationCard = MSIMECardWithViews(@[
        [self settingRow:@"中文标点" detail:@"Control + . 切换中英文标点。" control:_punctuationToggle],
        [self settingRow:@"智能标点"
                  detail:@"前一个字符为字母或数字时保留逗号、句号和冒号为 ASCII 形式。"
                 control:_smartPunctuationToggle],
        [self settingRow:@"重复标点转中文"
                  detail:@"短时间重复输入 ASCII 标点时替换为中文标点。"
                 control:_smartPunctuationRepeatToggle],
        [self settingRow:@"中文标点后按空格转换"
                  detail:@"刚输入中文标点后按空格，转换为对应英文标点。"
                 control:_smartPunctuationSpaceToggle],
        [self settingRow:@"成对标点" detail:@"自动插入并配对引号、括号等标点。" control:_pairedPunctuationToggle],
        [self settingRow:@"固定标点" control:_punctuationLockButton],
        // Both of these are states of what is being typed rather than keys, so they sit beside the
        // punctuation they change and not on 按键 beside the chords that toggle them. The chords keep
        // their own switches there; these two are the state those chords flip.
        [self settingRow:@"全角输入"
                  detail:@"Control + Shift + 空格 或 Option + Shift + H 临时切换全半角。"
                 control:_fullWidthToggle
                     aka:@[@"半角", @"全半角"]],
        [self settingRow:@"简繁输入"
                  detail:@"将提交的简体中文转换为繁体中文。"
                 control:_traditionalOutputToggle
                     aka:@[@"繁体", @"繁體"]],
    ], 0.0);
    punctuationCard.accessibilityLabel = @"标点与字符卡片";
    NSBox *mixedCard = MSIMECardWithViews(@[
        [self settingRow:@"中英混输" detail:@"在中文组词中允许英文候选。" control:_mixedEnglishToggle],
        [self settingRow:@"中英混输触发长度" control:_mixedEnglishPrefixButton],
        [self settingRow:@"Emoji 混输" detail:@"在中文组词中提供 Emoji 候选。" control:_mixedEmojiToggle aka:@[@"表情"]],
        [self settingRow:@"颜文字混输" detail:@"在中文组词中提供颜文字候选。" control:_mixedKaomojiToggle],
    ], 0.0);
    mixedCard.accessibilityLabel = @"中英混输卡片";
    // The example moves out of the name and under it: a label is what the setting is called, and 「（sahng → shang）」 is not part of what this one is called, it is what it does.
    //
    // The sentence above them is written as the window runs, because what it has to say depends on a scheme chosen on another page; see -refreshControls.
    _pinyinMatchingSchemeLabel = MSIMEDetailLabel(@"");
    _pinyinMatchingSchemeLabel.hidden = YES;
    NSBox *correctionCard = MSIMECardWithViews(@[
        _pinyinMatchingSchemeLabel,
        [self settingRow:@"全拼乱序纠错" detail:@"例如把 shang 输入为 sahng。" control:_transpositionToggle aka:@[@"打错"]],
        [self settingRow:@"全拼邻键纠错" detail:@"例如把 shang 输入为 shabg。" control:_neighborToggle aka:@[@"打错"]],
    ], 0.0);
    correctionCard.accessibilityLabel = @"拼音纠错卡片";
    // Quanpin correction is enabled by default and is not exposed as a native setting, so the card stays out of the visible and accessible settings page while its controls keep reflecting the shared preferences in -refreshControls.
    correctionCard.hidden = YES;
    correctionCard.accessibilityHidden = YES;
    NSMutableArray<NSButton *> *fuzzyRuleBoxes = [NSMutableArray array];
    for (NSArray *entry in FuzzyPinyinRuleControls()) {
        NSButton *button = [NSButton checkboxWithTitle:entry[1] target:self action:@selector(fuzzyPinyinRuleChanged:)];
        button.identifier = entry[0];
        _fuzzyPinyinRuleButtons[entry[0]] = button;
        [fuzzyRuleBoxes addObject:button];
    }
    NSBox *fuzzyCard = MSIMECardWithViews(@[
        MSIMEDetailLabel(@"全拼与双拼均支持；更改会在当前输入结束后生效。"),
        [self settingRow:@"启用模糊音" control:_fuzzyPinyinToggle],
        MSIMECardSeparator(),
        [self settingCheckboxes:fuzzyRuleBoxes columns:3],
    ], 8.0);
    fuzzyCard.accessibilityLabel = @"模糊音卡片（全拼与双拼）";

    // 实用功能 was a page holding this one card, and its summary was the only place in the window
    // that said how the eight modes are entered. The page is gone and the sentence is not: it is the
    // first thing in the card, where the grid it describes is.
    for (NSArray<NSString *> *entry in LocalModeControls()) {
        NSButton *button = [NSButton checkboxWithTitle:entry[1] target:self action:@selector(localModeChanged:)];
        button.identifier = entry[0];
        [_localModeButtons addObject:button];
    }
    NSBox *localModesCard = MSIMECardWithViews(@[
        MSIMEDetailLabel(@"未组词时按 Shift 加一个字母，临时切到另一种输入方式。"),
        [self settingCheckboxes:_localModeButtons columns:2],
    ], 8.0);
    localModesCard.accessibilityLabel = @"快捷模式卡片";

    NSScrollView *habitsPage = [self page:MSIMESettingsPageInputHabits
                                    title:@"输入习惯"
                                  summary:@"标点、混输、拼音匹配，以及 Shift 加一个字母的快捷模式。"
                                  content:@[
        [self sectionHeader:@"标点与字符"
                       keys:@[ChinesePunctuationKey, SmartPunctuationKey, SmartPunctuationRepeatToChineseKey,
                              SmartPunctuationSpaceConvertKey, PairedPunctuationKey, PunctuationLockKey,
                              FullWidthKey, TraditionalKey]],
        punctuationCard,
        [self sectionHeader:@"中英与符号混输" keys:@[MixedInputKey]], mixedCard,
        // 拼音纠错 and 模糊音 were two headings over two cards, and they answer one question between
        // them: what a mistyped syllable is still allowed to match. One heading, and the restore link
        // on it puts the whole answer back rather than half of it.
        [self sectionHeader:@"拼音匹配（全拼与双拼）"
                       keys:@[TranspositionKey, NeighborKey, FuzzyPinyinKey, FuzzyPinyinRulesKey]],
        correctionCard, fuzzyCard,
        [self sectionHeader:@"快捷模式" keys:@[LocalModesKey]], localModesCard,
    ]];

    // ---- 候选窗口 ---------------------------------------------------------------------------
    _preview = [[MSIMECandidatePreviewView alloc] initWithFrame:NSMakeRect(0, 0, 580, 190)];
    _preview.preferences = self;
    _preview.translatesAutoresizingMaskIntoConstraints = NO;
    _themeButton = [NSButton buttonWithTitle:[_preview forcedThemeButtonTitle] target:self action:@selector(togglePreviewTheme:)];
    _preview.themeButton = _themeButton;
    NSButton *showcase = [NSButton checkboxWithTitle:@"同时预览横排、竖排与状态栏" target:self action:@selector(togglePreviewShowcase:)];
    // The preview answers "what will the candidate window look like" with a fixed list of nine words, which answers it for those nine words. The font a user is choosing between is usually being chosen for their own text — a name, a technical term, the characters they type all day — and this is where they put it.
    _previewSampleField = [NSTextField textFieldWithString:@""];
    _previewSampleField.placeholderString = @"预览示例文字，以空格分隔";
    _previewSampleField.accessibilityLabel = @"预览示例文字";
    _previewSampleField.delegate = self;
    _previewSampleField.target = self;
    _previewSampleField.action = @selector(previewSampleChanged:);
    NSStackView *previewControls = [NSStackView stackViewWithViews:@[showcase, _themeButton, _previewSampleField]];
    previewControls.orientation = NSUserInterfaceLayoutOrientationHorizontal;
    previewControls.spacing = 12.0;
    NSBox *candidateWindowCard = MSIMECardWithViews(@[
        [self settingRow:@"候选排列" control:_layoutButton aka:@[@"横排", @"竖排"]],
        [self settingRow:@"每页候选" control:_pageSizeButton aka:@[@"候选个数"]],
        [self settingRow:@"候选字号" control:_fontButton aka:@[@"字体大小"]],
        [self settingRow:@"整体大小"
                  detail:@"候选文字与窗口的边距、行高和圆角一同缩放。"
                 control:scaleControls
                     aka:@[@"缩放", @"候选窗大小"]],
        [self settingRow:@"不透明度"
                  detail:@"只淡化候选框底色、边框与皮肤背景图，文字和焦点高亮保持不透明。"
                 control:opacityControls
                     aka:@[@"透明度"]],
        [self settingRow:@"圆角大小"
                  detail:@"跟随皮肤时显示皮肤自带的圆角；拖动后以此值覆盖皮肤的圆角。"
                 control:cornerRadiusControls
                     aka:@[@"圆角"]],
        [self settingRow:@"候选窗拼音字号" control:_preeditFontButton],
        [self settingRow:@"候选窗预编辑" control:_candidatePreeditButton],
        [self settingRow:@"候选窗口跟随光标" control:_candidateFollowCursorToggle],
        // The badge is this host's own feature and every other platform already offers a switch for
        // it; macOS drew it, read the preference on every mode change, and had nowhere to turn it off.
        [self settingRow:@"切换中英文时显示提示"
                  detail:@"切换后在光标下方短暂显示「中」或「英」。"
                 control:_inputModeHUDToggle
                     aka:@[@"HUD", @"角标"]],
    ], 0.0);
    candidateWindowCard.accessibilityLabel = @"候选窗口卡片";
    // The three clusters — a field beside a colour well, a field beside a button, a popup beside
    // three buttons — are wider than one popup, and the row now lets them be: the control column is
    // placed against the trailing edge rather than pinned to a width, so the card keeps one control
    // edge without the second fixed width these rows used to ask for.
    NSBox *fontCard = MSIMECardWithViews(@[
        [self settingRow:@"字体预设"
                  detail:@"写入候选字体；本机未安装时使用同类字体，最后使用系统字体。"
                 control:_candidateFontPresetButton
                     aka:@[@"宋体", @"黑体", @"楷体", @"圆体"]],
        [self settingRow:@"候选字体"
                  detail:@"可选择本机字体或输入字体家族名称；未安装或缺字时使用系统字体，并保留原设置。"
                 control:_fontFamilyControl],
        [self settingRow:@"候选窗英文字体"
                  detail:@"优先用于拉丁字符；未安装时使用候选字体，留空表示不设置独立英文字体。"
                 control:_englishFontFamilyControl],
    ], 0.0);
    fontCard.accessibilityLabel = @"候选字体卡片";
    // 候选文字颜色 leaves the font card for this one, which is the only row that moves: it is a colour,
    // the other six are colours, and a card holding one of the seven while a card under it holds the
    // rest is a worse place to look for any of them than either card alone.
    NSMutableArray<NSView *> *colorRows = [NSMutableArray arrayWithObjects:
        [self settingRow:@"颜色模式"
                  detail:@"候选窗口、悬浮工具栏、屏幕键盘与输入法菜单的默认明暗；设置窗口始终跟随系统。"
                 control:_themeModeButton
                     aka:@[@"深色", @"浅色", @"暗黑"]],
        [self settingRow:@"候选窗口主题" detail:@"覆盖颜色模式，只影响候选窗口。" control:_candidateThemeButton],
        [self settingRow:@"候选文字颜色" control:textColorControls], nil];
    [colorRows addObjectsFromArray:candidateColorRows];
    NSBox *colorCard = MSIMECardWithViews(colorRows, 0.0);
    colorCard.accessibilityLabel = @"候选配色卡片";
    NSScrollView *candidateWindowPage = [self page:MSIMESettingsPageCandidateWindow
                                             title:@"候选窗口"
                                           summary:@"候选窗口的排列、字体与配色。"
                                           content:@[
        // The preview writes nothing: 预览深色, the showcase checkbox and the sample text are ways of looking at the settings below, not settings, so this section has nothing to restore and nothing here outlives the window.
        [self sectionHeader:@"效果预览" keys:@[]], _preview, previewControls,
        [self sectionHeader:@"候选窗口"
                       keys:@[LayoutKey, PageSizeKey, FontKey, CandidateScaleKey, CandidateOpacityKey,
                              CandidateCornerRadiusKey, PreeditFontKey, CandidatePreeditKey,
                              CandidateFollowCursorKey, InputModeHUDKey]],
        candidateWindowCard,
        [self sectionHeader:@"候选字体" keys:@[FontFamilyKey, CandidateEnglishFontKey, FallbackFontsKey]],
        fontCard,
        [self sectionHeader:@"配色"
                       keys:@[ThemeKey, CandidateThemeKey, TextColorKey, NumberColorKey, AccentColorKey,
                              SelectedColorKey, HoverColorKey, SurfaceColorKey, BorderColorKey]],
        colorCard,
    ]];

    // ---- 皮肤 -------------------------------------------------------------------------------
    // The page is the skin browser itself. It already existed — cards with a live candidate preview
    // of each skin, in both appearances — but it lived in a window of its own behind a
    // 浏览所有皮肤… button, and the page you actually landed on offered a popup of skin names.
    // Picking a skin by reading its name out of a menu is choosing a look you cannot see.
    // The shared settings application has a skin page too. It stays reachable, but as a named trip
    // to another application rather than as the button you press to pick a skin.
    NSButton *sharedSkinPage = [NSButton buttonWithTitle:@"在设置应用中打开…" target:self action:@selector(showSkinCatalog:)];
    MSIMELinkifyButton(sharedSkinPage, @"在设置应用中打开皮肤页");
    sharedSkinPage.translatesAutoresizingMaskIntoConstraints = NO;
    // The cards are filled in the first time the page is shown. Building them renders a live
    // candidate preview per skin and rescans the skin directory, and rescanning announces an
    // appearance change — doing that while merely opening the window rebuilds the candidate panel
    // for a page the user has not asked for.
    _skinPageContainer = [[NSView alloc] initWithFrame:NSZeroRect];
    _skinPageContainer.translatesAutoresizingMaskIntoConstraints = NO;
    _skinPageContainer.accessibilityLabel = @"皮肤";
    [_skinPageContainer addSubview:sharedSkinPage];
    [NSLayoutConstraint activateConstraints:@[
        [sharedSkinPage.trailingAnchor constraintEqualToAnchor:_skinPageContainer.trailingAnchor constant:-kPageMargin],
        [sharedSkinPage.bottomAnchor constraintEqualToAnchor:_skinPageContainer.bottomAnchor constant:-6.0],
    ]];
    _skinPageSharedEntry = sharedSkinPage;
    NSView *skinPage = _skinPageContainer;
    // The page has no rows of this window's own — it is the skin browser, and it is not built until the user first walks into it — so what the search can offer is the page, under the words somebody would go looking for it by.
    [self registerSearchKeywords:@[ @"皮肤", @"候选窗口皮肤", @"换肤" ]
                         section:nil
                          onPage:MSIMESettingsPageSkin
                             row:nil];

    // ---- 词库与数据 --------------------------------------------------------------------------
    NSBox *learningCard = MSIMECardWithViews(@[
        [self settingRow:@"学习候选词频" control:_candidateLearningToggle aka:@[@"词频学习"]],
        [self settingRow:@"词频调整方式" control:_frequencyModeButton],
        [self settingRow:@"词频触发次数" control:_frequencyTriggerButton],
        [self settingRow:@"线性调整步长" control:_frequencyStepButton],
    ], 0.0);
    learningCard.accessibilityLabel = @"候选与学习卡片";
    NSButton *aiButton = [NSButton buttonWithTitle:@"配置 AI 联想…" target:self action:@selector(showAISettings:)];
    NSButton *translationButton = [NSButton buttonWithTitle:@"配置候选翻译…" target:self action:@selector(showTranslationSettings:)];
    NSButton *dictionaryButton = [NSButton buttonWithTitle:@"打开词库管理…" target:self action:@selector(showDictionary:)];
    dictionaryButton.accessibilityLabel = @"打开本机词库管理";
    NSBox *cloudCard = MSIMECardWithViews(@[
        // Where the query goes is drawn under the switch rather than folded into its name or hidden
        // in a tooltip: it is the one switch here that sends what is being typed off the machine.
        [self settingRow:@"启用云候选"
                  detail:@"将当前输入的拼音发送至 Google 输入工具。"
                 control:_cloudCandidatesToggle
                     aka:@[@"Google"]],
        [self settingRow:@"显示候选释义" control:_candidateTranslationsToggle aka:@[@"翻译"]],
        [self settingRow:@"显示离线英文释义" control:_candidateEnglishGlossToggle aka:@[@"英文"]],
        [self settingRow:@"AI 联想" control:aiButton],
        [self settingRow:@"翻译服务与目标语言" control:translationButton aka:@[@"候选翻译"]],
    ], 0.0);
    cloudCard.accessibilityLabel = @"云端与智能候选卡片";
    NSBox *dictionaryCard = MSIMECardWithViews(@[
        [self settingRow:@"本机用户词库" control:dictionaryButton aka:@[@"自造词", @"用户词"]],
    ], 0.0);
    dictionaryCard.accessibilityLabel = @"本机用户词库卡片";
    NSScrollView *dataPage = [self page:MSIMESettingsPageDictionary
                                  title:@"词库与数据"
                                summary:@"管理本机词库、用户词条与学习数据。"
                                content:@[
        [self sectionHeader:@"候选与学习"
                       keys:@[CandidateLearningKey, FrequencyModeKey, FrequencyTriggerCountKey,
                              FrequencyLinearStepKey]],
        learningCard,
        // The dictionary card opens the dictionary window; the words in it are not a preference and
        // are not what a restore here would touch.
        [self sectionHeader:@"本机词库" keys:@[]], dictionaryCard,
        [self sectionHeader:@"云端与智能候选"
                       keys:@[CloudCandidatesKey, CandidateTranslationsKey, CandidateEnglishGlossKey]],
        cloudCard,
    ]];

    // ---- 关于 -------------------------------------------------------------------------------
    Class updateControllerClass = NSClassFromString(@"MetasequoiaUpdateController");
    if ([updateControllerClass respondsToSelector:@selector(sharedController)])
        _updateController = [updateControllerClass sharedController];
    _versionLabel = [NSTextField labelWithString:@"开发构建"];
    _versionLabel.textColor = [NSColor secondaryLabelColor];
    _versionLabel.alignment = NSTextAlignmentRight;
    _versionLabel.accessibilityLabel = @"当前版本";
    _automaticUpdateLabel = [NSTextField labelWithString:@"检查自动更新状态…"];
    _automaticUpdateLabel.textColor = [NSColor secondaryLabelColor];
    _automaticUpdateLabel.alignment = NSTextAlignmentRight;
    _automaticUpdateLabel.accessibilityLabel = @"自动更新状态";
    _updatePageButton = [NSButton buttonWithTitle:@"检查更新…" target:self action:@selector(checkForUpdates:)];
    _updatePageButton.accessibilityLabel = @"立即检查更新";
    NSBox *updateCard = MSIMECardWithViews(@[
        [self settingRow:@"当前版本" control:_versionLabel aka:@[@"版本号"]],
        [self settingRow:@"自动更新" control:_automaticUpdateLabel],
        [self settingRow:@"立即检查" control:_updatePageButton aka:@[@"检查更新", @"升级"]],
    ], 0.0);
    updateCard.accessibilityLabel = @"软件更新卡片";
    NSButton *websiteButton = [NSButton buttonWithTitle:@"访问 msime.app" target:self action:@selector(openProductWebsite:)];
    MSIMELinkifyButton(websiteButton, @"访问水杉官网");
    _removeUserDataButton = [NSButton checkboxWithTitle:@"同时删除词库、学习记录、偏好与语音密钥"
                                                   target:nil
                                                   action:nil];
    _removeUserDataButton.accessibilityLabel = @"卸载时删除本机数据";
    _uninstallButton = [NSButton buttonWithTitle:@"卸载…" target:self action:@selector(uninstallInputSource:)];
    _uninstallButton.bezelStyle = NSBezelStyleRounded;
    _uninstallButton.contentTintColor = NSColor.systemRedColor;
    _uninstallButton.accessibilityLabel = [@"卸载" stringByAppendingString:MSIMEEditionDisplayName()];
    _uninstallButton.enabled = msime_macos_uninstall_input_source != nullptr;
    // The checkbox goes above the button, because it changes what the button does: below it, it
    // read as a consequence of a press that had already happened.
    NSBox *uninstallCard = MSIMECardWithViews(@[
        _removeUserDataButton, [self settingRow:@"输入源" control:_uninstallButton aka:@[@"卸载", @"删除"]],
    ], 6.0);
    uninstallCard.accessibilityLabel = @"卸载输入源卡片";
    NSBox *aboutCard = MSIMECardWithViews(@[[self settingRow:@"产品主页" control:websiteButton aka:@[@"官网"]]], 0.0);
    aboutCard.accessibilityLabel = @"关于卡片";
    // The sidebar used to open with the icon and the product name above the navigation. Under a
    // transparent titlebar that space belongs to the traffic lights and the search field, so the
    // mark moves here, where a Mac application states what it is.
    NSImageView *logo = [[NSImageView alloc] initWithFrame:NSZeroRect];
    // The bundle's own icon, not a redrawn approximation of it: this mark and the Dock tile are
    // then the same artwork by construction and cannot drift apart in shape or colour.
    logo.image = [NSImage imageNamed:NSImageNameApplicationIcon];
    logo.imageScaling = NSImageScaleProportionallyUpOrDown;
    logo.accessibilityLabel = @"水杉 IME";
    logo.translatesAutoresizingMaskIntoConstraints = NO;
    [logo.widthAnchor constraintEqualToConstant:52.0].active = YES;
    [logo.heightAnchor constraintEqualToConstant:52.0].active = YES;
    NSTextField *brand = [NSTextField labelWithString:MSIMEEditionDisplayName()];
    brand.font = [NSFont systemFontOfSize:17.0 weight:NSFontWeightSemibold];
    NSTextField *tagline = [NSTextField labelWithString:@"Metasequoia IME"];
    tagline.font = [NSFont systemFontOfSize:kBodyFontSize];
    tagline.textColor = NSColor.secondaryLabelColor;
    NSStackView *brandText = [NSStackView stackViewWithViews:@[brand, tagline]];
    brandText.orientation = NSUserInterfaceLayoutOrientationVertical;
    brandText.alignment = NSLayoutAttributeLeading;
    brandText.spacing = 2.0;
    NSStackView *brandRow = [NSStackView stackViewWithViews:@[logo, brandText]];
    brandRow.orientation = NSUserInterfaceLayoutOrientationHorizontal;
    brandRow.alignment = NSLayoutAttributeCenterY;
    brandRow.spacing = 14.0;
    brandRow.translatesAutoresizingMaskIntoConstraints = NO;
    // Nothing on this page is a stored preference — the version and the update state are read from
    // the bundle and from Sparkle, and uninstalling is not a setting — so none of its sections
    // offers a restore.
    NSScrollView *aboutPage = [self page:MSIMESettingsPageAbout
                                   title:@"关于"
                                 summary:@"版本与更新，以及水杉输入法的产品主页。"
                                 content:@[
        brandRow, [self sectionHeader:@"软件更新" keys:@[]], updateCard,
        [self sectionHeader:@"产品信息" keys:@[]], aboutCard,
        [self sectionHeader:@"卸载" keys:@[]], uninstallCard,
    ]];

    // ---- 按键 -------------------------------------------------------------------------------
    // 以词定字 and 翻页 cannot be bound to the same key group, and until now the window let the user
    // ask for it and answered with a beep and a control that snapped back. Both sentences are
    // written in -refreshKeyBindingConflicts, which also disables whichever side does not currently
    // hold the group; the one under the checkboxes names the group 以词定字 is holding, the one
    // under the key-group popup names the group paging is holding.
    _navigationConflictLabel = MSIMEDetailLabel(@"");
    _navigationConflictLabel.hidden = YES;
    _wordCharacterConflictLabel = MSIMEDetailLabel(@"");
    _wordCharacterConflictLabel.hidden = YES;
    _pagingPresetLabel = MSIMEDetailLabel(@"");
    _pagingPresetLabel.hidden = YES;
    NSBox *pagingCard = MSIMECardWithViews(@[
        // The preset and the checkboxes below it are one setting seen twice: the menu names whichever key group is ticked, and picking one from the menu ticks it. They used to be two, and the menu went on naming a group the checkboxes had since given up. Three items cannot name every state seven checkboxes can reach, and the sentence under the menu is what the menu says instead of picking the nearest one.
        [self settingRow:@"上翻 / 下翻" detailLabel:_pagingPresetLabel control:_pageShortcutButton],
        MSIMECardSeparator(),
        MSIMECardHeader(@"独立候选导航"),
        // Six peer key-pairs in the control column of one row is a tall stack pushed against the
        // right edge. They are a group, so they get the card's width and a heading of their own.
        [self settingCheckboxes:_navigationButtons columns:2],
        _navigationConflictLabel,
        MSIMECardSeparator(),
        [self settingRow:@"以词定字（首字／尾字）"
                  detail:@"按所选键组的左键上屏高亮候选的首个汉字，右键上屏末个汉字。"
                 control:_wordCharacterToggle],
        [self settingRow:@"首字／尾字键组" detailLabel:_wordCharacterConflictLabel control:_wordCharacterKeys],
    ], 6.0);
    pagingCard.accessibilityLabel = @"候选翻页卡片";
    NSBox *switchingCard = MSIMECardWithViews(@[
        [self settingRow:@"Shift + 空格切换中英文" control:_inputModeShortcutToggle aka:@[@"中英文切换"]],
        [self settingRow:@"单按 Shift 切换中英文" control:_shiftTapShortcutToggle aka:@[@"中英文切换"]],
        [self settingRow:@"单按 Control 切换中英文" control:_controlTapShortcutToggle aka:@[@"中英文切换"]],
        [self settingRow:@"Control + Option + 空格切换中英文" control:_controlOptionSpaceShortcutToggle aka:@[@"中英文切换"]],
        [self settingRow:@"Control + Shift + F 切换简繁" control:_characterSetShortcutToggle aka:@[@"繁体"]],
        [self settingRow:@"Option + Shift + H 切换全半角"
                  detail:@"关掉后这个组合键交给应用处理；Control + Shift + 空格 与工具栏的全半角按钮不受影响。"
                 control:_fullWidthShortcutToggle
                     aka:@[@"全角", @"半角"]],
        MSIMECardSeparator(),
        MSIMECardHeader(@"语音听写"),
        [self settingRow:@"Control + F9 开始语音输入"
                  detail:@"按一次开始，再按一次结束。"
                 control:_voiceHotkeyCtrlF9Toggle
                     aka:@[@"听写"]],
        [self settingRow:@"按住右 Option 说话" control:_voiceHotkeyRightAltToggle aka:@[@"听写"]],
        [self settingRow:@"按住 Control + Command 说话" control:_voiceHotkeyCtrlCommandToggle aka:@[@"听写"]],
        [self settingRow:@"按住右 Control + Option 说话" control:_voiceHotkeyCtrlOptionToggle aka:@[@"听写"]],
        [self settingRow:@"按住说话时按空格锁定录音"
                  detail:@"锁定后松开按键录音继续，再按一次结束键才停止。"
                 control:_voiceHotkeyHoldSpaceToggle
                     aka:@[@"听写"]],
    ], 0.0);
    switchingCard.accessibilityLabel = @"输入状态切换卡片";
    // Switching between Chinese and English is what this page is opened for; the paging matrix is
    // what it was opened on. At the default height the six switching rows began below the fold, under
    // seven checkboxes for key groups most users never rebind, so the two cards trade places.
    //
    // The five dictation shortcuts are in the same card, under a heading of their own: every other key binding in the window is on this page, and they were the one group that was not — they were in MSIMEVoiceSettings, a window nothing opened.
    NSScrollView *keysPage = [self page:MSIMESettingsPageKeys
                                  title:@"按键"
                                summary:@"切换中英文、翻页选字与开始语音输入使用的按键。"
                                content:@[
        [self sectionHeader:@"输入状态切换"
                       keys:@[InputModeShortcutKey, ShiftTapShortcutKey, ControlTapShortcutKey,
                              ControlOptionSpaceShortcutKey, CharacterSetShortcutKey, FullWidthShortcutKey,
                              VoiceHotkeyCtrlF9Key, VoiceHotkeyRightAltKey, VoiceHotkeyCtrlCommandKey,
                              VoiceHotkeyCtrlOptionKey, VoiceHotkeyHoldSpaceKey]],
        switchingCard,
        [self sectionHeader:@"候选翻页与选字" keys:@[PageShortcutKey, NavigationKey, WordCharacterKey]],
        pagingCard,
    ]];

    // ---- 状态栏 -----------------------------------------------------------------------------
    NSBox *toolbarCard = MSIMECardWithViews(@[
        [self settingRow:@"显示浮动工具栏" control:_toolbarToggle aka:@[@"状态栏", @"悬浮工具栏"]],
        [self settingRow:@"悬浮工具栏主题" detail:@"覆盖颜色模式，只影响悬浮工具栏。" control:_toolbarThemeButton],
        MSIMECardSeparator(),
        MSIMECardHeader(@"工具栏按钮"),
        // 按工具栏画它们的顺序排列，网格从左到右读起来和工具栏一致——MSIMEFloatingToolbarPanel 按 FloatingToolbarComponentKeys() 的顺序排它的十个按钮。
        [self settingCheckboxes:@[
            _toolbarEnglishModeButton, _toolbarInputSchemeButton, _toolbarPunctuationButton, _toolbarFullWidthButton,
            _toolbarCharacterSetButton, _toolbarEmojiButton, _toolbarHandwritingButton,
            _toolbarScreenKeyboardButton, _toolbarVoiceButton, _toolbarSettingsButton,
        ] columns:2],
    ], 6.0);
    toolbarCard.accessibilityLabel = @"悬浮工具栏卡片";
    NSBox *toolbarSizeCard = MSIMECardWithViews(@[
        [self settingRow:@"工具栏缩放" control:_toolbarScaleButton],
        [self settingRow:@"工具栏字号" control:_toolbarFontSizeButton],
    ], 0.0);
    toolbarSizeCard.accessibilityLabel = @"悬浮工具栏尺寸卡片";
    // 工具栏缩放 and 工具栏字号 are four steps and seven sizes of a panel that is not on this page, and their two popups sat over nothing that showed what any pair of them produces. This draws the toolbar those settings build, at the size they build it, and prints that size beside it.
    _toolbarPreview = [[MSIMEToolbarPreviewView alloc] initWithFrame:NSMakeRect(0, 0, 580, 100)];
    _toolbarPreview.preferences = self;
    // The scale and the font size live in the same stored dictionary as the nine component choices, so each of the two sections names the entries of that dictionary it owns rather than the whole key. The alert on either link promises that the other settings are untouched, and the section boundary the page draws between the two cards is one the user can see; a restore that reached across it would be the link disagreeing with both.
    NSScrollView *statusBarPage = [self page:MSIMESettingsPageStatusBar
                                       title:@"状态栏"
                                     summary:@"随时查看输入状态，通过悬浮工具栏切换常用输入选项。"
                                     content:@[
        // Like 候选窗口's preview, this one is a way of looking at the settings below it rather than a setting, so its heading carries no restore link.
        [self sectionHeader:@"效果预览" keys:@[]], _toolbarPreview,
        [self sectionHeader:@"显示与组件"
                       keys:@[FloatingToolbarKey, FloatingToolbarOptionsKey, ToolbarThemeKey]
                     fields:@{FloatingToolbarOptionsKey : FloatingToolbarComponentKeys()}],
        toolbarCard,
        [self sectionHeader:@"尺寸"
                       keys:@[FloatingToolbarOptionsKey]
                     fields:@{FloatingToolbarOptionsKey : @[@"scale_percent", @"font_size"]}],
        toolbarSizeCard,
    ]];

    // ---- 账号 -------------------------------------------------------------------------------
    NSView *accountPaneView = nil;
    if (MSIMEAccountPaneView != nullptr) {
        accountPaneView = MSIMEAccountPaneView();
        [accountPaneView.heightAnchor constraintGreaterThanOrEqualToConstant:520.0].active = YES;
    } else {
        NSButton *accountButton = [NSButton buttonWithTitle:@"管理水杉账号…" target:self action:@selector(showBackendAccount:)];
        accountButton.accessibilityIdentifier = @"MSIMEClientBackendAccount";
        NSBox *accountCard = MSIMECardWithViews(@[[self settingRow:@"登录与账号管理" control:accountButton]], 0.0);
        accountCard.accessibilityLabel = @"水杉账号卡片";
        accountPaneView = MSIMECardWithViews(@[MSIMESectionLabel(@"水杉账号"), accountCard], 0.0);
    }
    NSScrollView *accountPage = [self page:MSIMESettingsPageAccount
                                     title:@"账号"
                                   summary:@"登录水杉账号后，云同步等需要账号的功能才会生效；候选词翻译要在「翻译服务」里选择「水杉账号」才会使用账号。"
                                   content:@[
        accountPaneView,
    ]];
    // The account pane is a SwiftUI view attached by the Swift backend: its text is neither an NSTextField nor an NSButton, so nothing on it could ever be found by name. These are the names the pane goes by, and the result lands on the page that holds it.
    [self registerSearchKeywords:@[ @"登录", @"注销", @"退出登录", @"云同步", @"云剪贴板", @"会员" ]
                         section:@"水杉账号"
                          onPage:MSIMESettingsPageAccount
                             row:nil];

    // ---- 帮助与反馈 --------------------------------------------------------------------------
    // Upstream builds both of these out of its own help copy and a local issue form. This host has
    // neither; it routes to the existing support window instead of inventing the content here.
    //
    // Two buttons, two pages of that window, and now two selectors: they both used to be the
    // no-argument -showSupport:, which only shows the window. MSIMESupportWindowController builds its
    // contentView inside -showPage:, so the window a cold launch put on screen had no content and no
    // title, and a warm one showed whichever page the input method's menu had last opened.
    NSButton *helpButton = [NSButton buttonWithTitle:@"打开使用帮助…" target:self action:@selector(showHelp:)];
    NSBox *helpCard = MSIMECardWithViews(@[[self settingRow:@"常用按键与常见问题" control:helpButton aka:@[@"帮助"]]], 0.0);
    helpCard.accessibilityLabel = @"帮助卡片";
    NSButton *feedbackButton = [NSButton buttonWithTitle:@"提交反馈…" target:self action:@selector(showFeedback:)];
    NSBox *feedbackCard =
        MSIMECardWithViews(@[[self settingRow:@"问题反馈与功能建议" control:feedbackButton aka:@[@"反馈", @"报错"]]], 0.0);
    feedbackCard.accessibilityLabel = @"反馈卡片";
    // One page rather than two, because each of them was a heading over a card over a single button,
    // and the two buttons went to two pages of the same window.
    NSScrollView *supportPage = [self page:MSIMESettingsPageSupport
                                     title:@"帮助与反馈"
                                   summary:@"常用按键与常见问题，以及提交问题和功能建议的渠道。"
                                   content:@[
        [self sectionHeader:@"使用帮助" keys:@[]], helpCard,
        [self sectionHeader:@"问题反馈" keys:@[]], feedbackCard,
    ]];

    // ---- 语音输入 ---------------------------------------------------------------------------
    // The card this window owns. Everything on it is a plain NSUserDefaults key written through the injected _defaults, so it is built here rather than in the voice form: that form is looked up at runtime to keep the keychain and CoreAudio out of the test executables, and these five settings are still settings in a build that has no voice module.
    NSBox *voiceCard = MSIMECardWithViews(@[
        [self settingRow:@"启用语音输入"
                  detail:@"关掉后语音快捷键与工具栏的语音按钮都不再开始录音。"
                 control:_voiceEnabledToggle
                     aka:@[@"听写", @"录音"]],
        [self settingRow:@"识别语言" control:_voiceLanguageButton],
        [self settingRow:@"播放提示音" detail:@"开始与结束录音时各响一声。" control:_voiceSoundToggle],
        [self settingRow:@"录音时静音系统音频" detail:@"录完自动恢复原来的音量。" control:_voiceMuteSystemAudioToggle],
        [self settingRow:@"实时显示识别结果"
                  detail:@"边说边把还没定稿的文字显示在输入位置；关掉则只在识别结束后一次上屏。"
                 control:_voiceStreamInlinePreeditToggle],
    ], 0.0);
    voiceCard.accessibilityLabel = @"语音卡片";
    // The form itself, not a 配置语音输入… button opening a second window with its own 保存 button.
    Class voiceFormClass = NSClassFromString(@"MetasequoiaVoiceProviderSettingsView");
    _voiceSettingsView = [[voiceFormClass alloc] initWithFrame:NSZeroRect];
    NSView *voiceContent = _voiceSettingsView
        ?: (NSView *)MSIMECardWithViews(@[[self settingRow:@"语音输入"
                                                  control:[NSTextField labelWithString:@"此构建不包含语音模块。"]]], 0.0);
    NSScrollView *voicePage = [self page:MSIMESettingsPageVoice
                                   title:@"语音输入"
                                 summary:@"开始录音的方式在「按键」页；这里是识别服务、识别语言，以及识别后的文本整理。"
                                 content:@[
        [self sectionHeader:@"语音"
                       keys:@[VoiceEnabledKey, VoiceLanguageKey, VoiceSoundKey, VoiceMuteSystemAudioKey,
                              VoiceStreamInlinePreeditKey]],
        voiceCard,
        voiceContent,
    ]];
    // The dictation form builds its own rows, in a class resolved at runtime so that the keychain and CoreAudio stay out of the test executables. It cannot register them, so the page registers the names it draws and the result lands on the form.
    [self registerSearchKeywords:@[ @"识别方式", @"服务地址", @"API 密钥", @"识别模型", @"本地模型", @"录音设备",
                                    @"识别后整理文本", @"整理模型", @"整理方案", @"整理提示词" ]
                         section:@"语音识别与文本整理"
                          onPage:MSIMESettingsPageVoice
                             row:voiceContent];

    // The index is both the page index and the sidebar item's page index, and it is also
    // MSIMESettingsPage: the enum is declared in the order this array is written, so the four pages
    // named from elsewhere in the file are named rather than numbered, and the assertion at the end
    // of this method fails the moment the two lists stop being the same length.
    _preferencePages = @[
        schemePage, habitsPage, keysPage, voicePage, candidateWindowPage, skinPage, statusBarPage,
        dataPage, accountPage, supportPage, aboutPage,
    ];

    NSArray<NSString *> *navigationLabels = @[
        @"输入方案", @"输入习惯", @"按键", @"语音输入", @"候选窗口", @"皮肤", @"状态栏", @"词库", @"账号",
        @"帮助与反馈", @"关于",
    ];
    // Nothing here may be one of the symbols SF Symbols localises into a word. 输入习惯 was textformat, which draws the letters "Aa" in English and the two characters 格式 in Chinese, so the row read 「格式 输入习惯」 — an icon column with prose in it.
    NSArray<NSString *> *navigationSymbols = @[
        @"keyboard", @"slider.horizontal.3", @"command", @"mic", @"rectangle.on.rectangle", @"paintpalette",
        @"ellipsis.rectangle", @"book", @"person.crop.circle", @"questionmark.circle", @"info.circle",
    ];
    _pageTitles = navigationLabels;
    // The stable name of each page, parallel to _preferencePages. Both things that have to point at a page from outside this method — the remembered page and -showSettingsPageWithIdentifier: — name it instead of numbering it, because the numbering is the one part of this list that is expected to change. The names are the tails of the shared settings: routes (src/core/DesktopSettingsLauncher.h), so a deep link reads the same whichever settings surface answers it. 输入习惯 is the one page with no such route, because it is the one page the shared surface does not have; it is named for what it holds rather than after 实用功能, which is one of its five cards. Retired names — helpcode, feedback, utilities — resolve to nothing and open the first page, which is where the helpcode controls now are.
    _pageIdentifiers = @[
        @"input", @"habits", @"shortcuts", @"voice", @"appearance", @"skin", @"floating",
        @"dictionary", @"account", @"help", @"about",
    ];
    // Four runs with nothing but a gap between them announce a grouping without saying what it
    // groups by, so each run gets the heading AppKit puts above a source-list section. No heading is
    // the name of a page under it any more: 输入 used to be both the first group and its first
    // member, which reads as a page nested inside itself.
    NSArray<NSString *> *groupTitles = @[@"打字", @"显示", @"数据与账号", @"支持"];
    NSArray<NSArray<NSNumber *> *> *navigationGroups = @[
        @[@(MSIMESettingsPageInputScheme), @(MSIMESettingsPageInputHabits), @(MSIMESettingsPageKeys),
          @(MSIMESettingsPageVoice)],
        @[@(MSIMESettingsPageCandidateWindow), @(MSIMESettingsPageSkin), @(MSIMESettingsPageStatusBar)],
        @[@(MSIMESettingsPageDictionary), @(MSIMESettingsPageAccount)],
        @[@(MSIMESettingsPageSupport), @(MSIMESettingsPageAbout)],
    ];
    NSMutableArray<MSIMESettingsSidebarItem *> *sidebarGroups = [NSMutableArray array];
    for (NSUInteger groupIndex = 0; groupIndex < navigationGroups.count; ++groupIndex) {
        NSMutableArray<MSIMESettingsSidebarItem *> *members = [NSMutableArray array];
        for (NSNumber *pageIndex in navigationGroups[groupIndex]) {
            const NSInteger index = pageIndex.integerValue;
            MSIMESettingsSidebarItem *member = [MSIMESettingsSidebarItem new];
            member.title = navigationLabels[index];
            member.symbolName = navigationSymbols[index];
            member.pageIndex = index;
            [members addObject:member];
        }
        MSIMESettingsSidebarItem *group = [MSIMESettingsSidebarItem new];
        group.title = groupTitles[groupIndex];
        group.pageIndex = -1;
        group.children = members;
        [sidebarGroups addObject:group];
    }
    _sidebarGroups = sidebarGroups;

    // The sidebar is a source list, not thirteen push-on buttons in a stack view. What the system
    // brings with it is everything the drawn version had to imitate and mostly did not: desaturating
    // when the window loses focus, hover, the vibrant selection, truncation with an ellipsis, the
    // focus ring, arrow keys that hand the selection back at either end, and type-select. It is in a
    // scroll view because a stack view with no bottom anchor drew its last rows past the bottom of
    // the sidebar at the minimum window height, where they could not be clicked.
    _sidebarOutline = [[NSOutlineView alloc] initWithFrame:NSZeroRect];
    _sidebarOutline.style = NSTableViewStyleSourceList;
    _sidebarOutline.headerView = nil;
    _sidebarOutline.floatsGroupRows = NO;
    _sidebarOutline.allowsEmptySelection = NO;
    _sidebarOutline.rowSizeStyle = NSTableViewRowSizeStyleCustom;
    _sidebarOutline.accessibilityLabel = @"水杉输入法导航";
    NSTableColumn *sidebarColumn = [[NSTableColumn alloc] initWithIdentifier:@"MSIMESettingsSidebarColumn"];
    sidebarColumn.resizingMask = NSTableColumnAutoresizingMask;
    [_sidebarOutline addTableColumn:sidebarColumn];
    _sidebarOutline.outlineTableColumn = sidebarColumn;
    _sidebarOutline.dataSource = self;
    _sidebarOutline.delegate = self;
    [_sidebarOutline reloadData];
    // The groups are headings, not folders: every row is on screen from the start and stays there.
    [_sidebarOutline expandItem:nil expandChildren:YES];
    _sidebarScroll = [[NSScrollView alloc] initWithFrame:NSZeroRect];
    _sidebarScroll.documentView = _sidebarOutline;
    _sidebarScroll.hasVerticalScroller = YES;
    _sidebarScroll.autohidesScrollers = YES;
    _sidebarScroll.drawsBackground = NO;
    _sidebarScroll.translatesAutoresizingMaskIntoConstraints = NO;

    _searchResultsStack = [NSStackView stackViewWithViews:@[]];
    _searchResultsStack.orientation = NSUserInterfaceLayoutOrientationVertical;
    _searchResultsStack.alignment = NSLayoutAttributeLeading;
    _searchResultsStack.spacing = 1.0;
    _searchResultsStack.translatesAutoresizingMaskIntoConstraints = NO;
    // Fourteen results at 36pt do not fit a 520pt-tall sidebar, and pinned to its bottom edge they
    // broke a required constraint instead of scrolling.
    NSView *searchResultsDocument = [[MSIMEPreferencesDocumentView alloc] initWithFrame:NSZeroRect];
    searchResultsDocument.translatesAutoresizingMaskIntoConstraints = NO;
    [searchResultsDocument addSubview:_searchResultsStack];
    _searchResultsScroll = [[NSScrollView alloc] initWithFrame:NSZeroRect];
    _searchResultsScroll.documentView = searchResultsDocument;
    _searchResultsScroll.hasVerticalScroller = YES;
    _searchResultsScroll.autohidesScrollers = YES;
    _searchResultsScroll.drawsBackground = NO;
    _searchResultsScroll.translatesAutoresizingMaskIntoConstraints = NO;
    _searchResultsScroll.hidden = YES;
    [NSLayoutConstraint activateConstraints:@[
        [searchResultsDocument.widthAnchor constraintEqualToAnchor:_searchResultsScroll.contentView.widthAnchor],
        [searchResultsDocument.leadingAnchor constraintEqualToAnchor:_searchResultsScroll.contentView.leadingAnchor],
        [searchResultsDocument.topAnchor constraintEqualToAnchor:_searchResultsScroll.contentView.topAnchor],
        [_searchResultsStack.leadingAnchor constraintEqualToAnchor:searchResultsDocument.leadingAnchor constant:10.0],
        [_searchResultsStack.trailingAnchor constraintEqualToAnchor:searchResultsDocument.trailingAnchor constant:-10.0],
        [_searchResultsStack.topAnchor constraintEqualToAnchor:searchResultsDocument.topAnchor constant:6.0],
        [_searchResultsStack.bottomAnchor constraintEqualToAnchor:searchResultsDocument.bottomAnchor constant:-6.0],
    ]];

    NSViewController *sidebarController = [[NSViewController alloc] init];
    sidebarController.view = [[NSView alloc] initWithFrame:NSMakeRect(0, 0, kSidebarWidth, 520)];
    [sidebarController.view addSubview:_sidebarScroll];
    [sidebarController.view addSubview:_searchResultsScroll];
    for (NSScrollView *list in @[_sidebarScroll, _searchResultsScroll])
        [NSLayoutConstraint activateConstraints:@[
            [list.leadingAnchor constraintEqualToAnchor:sidebarController.view.leadingAnchor],
            [list.trailingAnchor constraintEqualToAnchor:sidebarController.view.trailingAnchor],
            [list.topAnchor constraintEqualToAnchor:sidebarController.view.topAnchor],
            [list.bottomAnchor constraintEqualToAnchor:sidebarController.view.bottomAnchor],
        ]];

    // The detail side stays what it was: every page pinned edge to edge in one container, shown and
    // hidden rather than added and removed. An NSTabViewController would take the unselected pages
    // out of the view hierarchy, and the tests walk hidden pages to find the control they are about
    // (platforms/macos/tests/settings/PreferenceViewLookup.h).
    NSViewController *detailController = [[NSViewController alloc] init];
    MSIMESettingsDetailView *pageContainer =
        [[MSIMESettingsDetailView alloc] initWithFrame:NSMakeRect(0, 0, 600, 520)];
    pageContainer.searchTarget = self;
    pageContainer.searchAction = @selector(beginSettingsSearch:);
    detailController.view = pageContainer;

    _splitViewController = [[NSSplitViewController alloc] init];
    // Held on to rather than left to the split view controller's array: a search performed while
    // the sidebar is collapsed has to open it, and asking the controller for "the sidebar one"
    // every time is a lookup by position of something this window already knows by name.
    _sidebarSplitItem = [NSSplitViewItem sidebarWithViewController:sidebarController];
    _sidebarSplitItem.allowsFullHeightLayout = YES;
    _sidebarSplitItem.canCollapse = YES;
    _sidebarSplitItem.minimumThickness = kSidebarWidth;
    _sidebarSplitItem.maximumThickness = kSidebarMaxWidth;
    NSSplitViewItem *detailItem = [NSSplitViewItem splitViewItemWithViewController:detailController];
    // The page scrolls under the toolbar, so the line under the titlebar is the system's to draw
    // and to take away again — the cards used to run off the top of the window with nothing there.
    detailItem.titlebarSeparatorStyle = NSTitlebarSeparatorStyleAutomatic;
    [_splitViewController addSplitViewItem:_sidebarSplitItem];
    [_splitViewController addSplitViewItem:detailItem];
    _splitViewController.splitView.autosaveName = @"MSIMESettingsSplit";
    window.contentViewController = _splitViewController;
    // Handing a window a content view controller sizes it to that controller's fitting size, which
    // for a split view is its two minimum thicknesses — the window came up at its own minimum, one
    // card narrower and two cards shorter than it used to. The size the window opens at is a
    // decision of this window's, so it is restated after the assignment rather than left to the
    // solver.
    [window setContentSize:NSMakeSize(860, 640)];

    // The pages go in after the window has its controller, not before. Handing a window a content
    // view controller makes AppKit ask the split view for its fitting size, and that walks every
    // constraint under it — with eleven pages of cards already installed, that one call was most of
    // the cost of opening this window: 93ms to build it on develop against 576ms here, measured.
    // Installed into a container that is still empty, the same call is cheap, and subviews added
    // afterwards do not ask for it again. Pinning to the safe area rather than the container's own
    // top is what keeps the page title out from behind the titlebar, and the safe area is only
    // meaningful once the container is in a window, which by this line it is.
    for (NSView *page in _preferencePages) {
        [pageContainer addSubview:page];
        [NSLayoutConstraint activateConstraints:@[
            [page.leadingAnchor constraintEqualToAnchor:pageContainer.leadingAnchor],
            [page.trailingAnchor constraintEqualToAnchor:pageContainer.trailingAnchor],
            [page.topAnchor constraintEqualToAnchor:pageContainer.safeAreaLayoutGuide.topAnchor],
            [page.bottomAnchor constraintEqualToAnchor:pageContainer.bottomAnchor],
        ]];
    }

    _searchField = [[NSSearchField alloc] initWithFrame:NSZeroRect];
    _searchField.placeholderString = @"搜索设置";
    _searchField.accessibilityLabel = @"搜索设置";
    _searchField.font = [NSFont systemFontOfSize:kBodyFontSize];
    _searchField.sendsWholeSearchString = NO;
    _searchField.sendsSearchStringImmediately = YES;
    _searchField.target = self;
    _searchField.action = @selector(searchChanged:);
    NSToolbar *toolbar = [[NSToolbar alloc] initWithIdentifier:@"MSIMESettingsToolbar"];
    toolbar.delegate = self;
    toolbar.allowsUserCustomization = NO;
    toolbar.displayMode = NSToolbarDisplayModeIconOnly;
    window.toolbar = toolbar;
    window.toolbarStyle = NSWindowToolbarStyleUnified;
    self.window = window;
    // The account pane is a foreign view attached to this window; closing the window from its own
    // close button has to detach it the way the removed Close button used to.
    [NSNotificationCenter.defaultCenter addObserver:self
                                           selector:@selector(preferencesWindowWillClose:)
                                               name:NSWindowWillCloseNotification
                                             object:window];
    // What a page does on entry is owed to the user walking into it, not to the window being built;
    // see -performPageEntrySideEffects. The window is not on screen yet, so the first notification
    // that it is releases the work the restored page below has deferred.
    [NSNotificationCenter.defaultCenter addObserver:self
                                           selector:@selector(preferencesWindowDidBecomeKey:)
                                               name:NSWindowDidBecomeKeyNotification
                                             object:window];
    // A settings window is not a wizard to be read front to back, so it opens on the page it was left on rather than on the first one.
    const NSInteger rememberedPage = [self pageIndexForIdentifier:[_defaults stringForKey:LastSettingsPageKey]];
    const NSInteger initialPage = rememberedPage < 0 ? 0 : rememberedPage;
    [self showPreferencesPageAtIndex:initialPage navigationIndex:initialPage];
    [self refreshControls];
    // Restore first, centre only as the fallback: the two have to be asked in this order, because
    // attaching the autosave name saves the frame the window currently has, after which every
    // launch would "restore" whatever centring had just produced.
    if (![window setFrameUsingName:MSIMESettingsWindowFrameAutosaveName()]) [window center];
    [window setFrameAutosaveName:MSIMESettingsWindowFrameAutosaveName()];
    // MSIMESettingsPage is what kSkinPageIndex, kVoicePageIndex, kAboutPageIndex and
    // kAccountPageIndex are derived from, and it is only true of the array above by being written
    // alongside it. A page added to one list and not the other is caught here rather than as a page
    // that never reloads, or as an account pane attached to the wrong page. The test binaries build
    // with -UNDEBUG, so this runs in all of them.
    NSAssert(_preferencePages.count == (NSUInteger)MSIMESettingsPageCount &&
                 _pageTitles.count == (NSUInteger)MSIMESettingsPageCount &&
                 _pageIdentifiers.count == (NSUInteger)MSIMESettingsPageCount,
             @"MSIMESettingsPage has %ld pages, the window built %lu with %lu titles and %lu identifiers",
             (long)MSIMESettingsPageCount, (unsigned long)_preferencePages.count,
             (unsigned long)_pageTitles.count, (unsigned long)_pageIdentifiers.count);
    // A row registered with the search index but built into a card that no page ever listed. It would be a result that takes the user to page −1, so it is caught here rather than in the sidebar.
    NSAssert(_pendingSearchEntries.count == 0, @"%lu registered settings were never placed on a page, starting with 「%@」",
             (unsigned long)_pendingSearchEntries.count, _pendingSearchEntries.firstObject.title);
}
- (void)preferencesWindowWillClose:(NSNotification *)notification {
    (void)notification;
    _windowHasAppeared = NO;
    if (MSIMEAccountPaneClose != nullptr) MSIMEAccountPaneClose();
}
/// The window is on screen, so the page in front of the user is one they are actually looking at
/// and the work it owes them is due. Every later page change runs it directly; this is only how the
/// page restored while the window was being built gets its turn.
- (void)preferencesWindowDidBecomeKey:(NSNotification *)notification {
    (void)notification;
    // 用户多半是从系统设置添加完回来的，每次回到窗口都重读一次。
    [self refreshInputModeHint];
    if (_windowHasAppeared) return;
    _windowHasAppeared = YES;
    [self performPageEntrySideEffects];
    // The links were skipped for as long as there was nothing on screen to carry them; this is the first moment there is.
    [self refreshSectionRestoreLinks];
}

#pragma mark - Settings search registry

/// A preference row, registered with the search index under the name the row itself is labelled with. Every card on every page is built out of these, so the index is a statement the pages make about what is on them rather than a reading the window takes of what they came out looking like.
- (NSView *)settingRow:(NSString *)title control:(NSView *)control {
    return [self registerSearchRow:MSIMEPreferenceRow(title, control) named:title aka:nil];
}
- (NSView *)settingRow:(NSString *)title control:(NSView *)control aka:(NSArray<NSString *> *)synonyms {
    return [self registerSearchRow:MSIMEPreferenceRow(title, control) named:title aka:synonyms];
}
- (NSView *)settingRow:(NSString *)title detail:(NSString *)detail control:(NSView *)control {
    return [self registerSearchRow:MSIMEPreferenceRowWithDetail(title, detail, control) named:title aka:nil];
}
- (NSView *)settingRow:(NSString *)title
                detail:(NSString *)detail
               control:(NSView *)control
                   aka:(NSArray<NSString *> *)synonyms {
    return [self registerSearchRow:MSIMEPreferenceRowWithDetail(title, detail, control) named:title aka:synonyms];
}
/// The same row, for a sentence written as the window runs rather than as it is built.
- (NSView *)settingRow:(NSString *)title detailLabel:(NSTextField *)detail control:(NSView *)control {
    return [self registerSearchRow:MSIMEPreferenceRowWithDetailLabel(title, detail, control) named:title aka:nil];
}
- (NSView *)settingRow:(NSString *)title
           detailLabel:(NSTextField *)detail
               control:(NSView *)control
                   aka:(NSArray<NSString *> *)synonyms {
    return [self registerSearchRow:MSIMEPreferenceRowWithDetailLabel(title, detail, control) named:title aka:synonyms];
}
/// Peer checkboxes: a fuzzy rule, a toolbar component, an extended input mode. Each box is a setting of its own and carries its own wording, so each is registered under the title it draws and is its own landing target.
- (NSView *)settingCheckboxes:(NSArray<NSButton *> *)boxes columns:(NSInteger)columns {
    for (NSButton *box in boxes) [self registerSearchRow:box named:box.title aka:nil];
    return MSIMECheckboxGrid(boxes, columns);
}
- (NSView *)registerSearchRow:(NSView *)row named:(NSString *)title aka:(NSArray<NSString *> *)synonyms {
    NSArray<NSString *> *spellings = PinyinSpellings(title);
    MSIMESettingsSearchEntry *entry = [MSIMESettingsSearchEntry new];
    entry.title = title;
    entry.synonyms = synonyms;
    entry.pinyin = spellings[0];
    entry.initials = spellings[1];
    entry.pageIndex = -1;
    entry.row = row;
    [_pendingSearchEntries addObject:entry];
    return row;
}
/// Names for a page whose contents this window does not build: the skin browser, which builds itself on first entry; the account pane, which is a SwiftUI view attached by the Swift backend and holds neither an NSTextField nor an NSButton; the dictation form, which is resolved at runtime and builds its own rows. Landing on the page is the whole result — there is no row of this window's to scroll to — so the entries carry the page and nothing else.
- (void)registerSearchKeywords:(NSArray<NSString *> *)titles
                       section:(NSString *)section
                        onPage:(NSInteger)pageIndex
                           row:(NSView *)row {
    for (NSString *title in titles) {
        NSArray<NSString *> *spellings = PinyinSpellings(title);
        MSIMESettingsSearchEntry *entry = [MSIMESettingsSearchEntry new];
        entry.title = title;
        entry.sectionTitle = section;
        entry.pinyin = spellings[0];
        entry.initials = spellings[1];
        entry.pageIndex = pageIndex;
        entry.row = row;
        [_searchEntries addObject:entry];
    }
}
/// Builds a page and hands every setting registered while its cards were being built the page number and the section heading it ended up under.
///
/// The two halves cannot be stated together at either end: a row is built before the page knows it exists, and a section heading is written after the card it introduces. Pairing them here — by asking which of the page's content views actually contains each registered row — is what keeps a section name from having to be repeated beside every card that belongs to it.
- (NSScrollView *)page:(NSInteger)pageIndex
                 title:(NSString *)title
               summary:(NSString *)summary
               content:(NSArray<NSView *> *)content {
    NSScrollView *page = PreferencesPage(title, summary, content);
    NSString *section = nil;
    for (NSView *view in content) {
        NSString *heading = [_sectionTitlesByHeader objectForKey:view];
        if (heading != nil) {
            section = heading;
            continue;
        }
        for (MSIMESettingsSearchEntry *entry in [_pendingSearchEntries copy]) {
            if (![self view:view containsRow:entry.row]) continue;
            entry.pageIndex = pageIndex;
            entry.sectionTitle = section;
            [_searchEntries addObject:entry];
            [_pendingSearchEntries removeObject:entry];
        }
    }
    return page;
}
- (BOOL)view:(NSView *)container containsRow:(NSView *)row {
    for (NSView *view = row; view != nil; view = view.superview)
        if (view == container) return YES;
    return NO;
}
/// Whether the user can currently get to the row an entry points at. 输入方案 shows the 全拼, 双拼 and 五笔 option cards one at a time, so under 五笔 the three 辅助码 rows are on the page but not reachable, and offering them would scroll to nothing. Asked at the moment the query runs rather than recorded when the index was built, which is what stops the index going stale: there is nothing to rebuild when the selected scheme changes.
- (BOOL)searchEntryReachable:(MSIMESettingsSearchEntry *)entry {
    if (entry.pageIndex < 0 || (NSUInteger)entry.pageIndex >= _preferencePages.count) return NO;
    NSView *page = _preferencePages[(NSUInteger)entry.pageIndex];
    // The pages themselves are all hidden but the one in front, so the walk stops at the page.
    for (NSView *view = entry.row; view != nil && view != page; view = view.superview)
        if (view.hidden) return NO;
    return YES;
}
/// A query replaces the navigation with its matches rather than dropping a menu over the sidebar:
/// a menu takes key focus, so the next keystroke would go to the menu instead of the field.
///
/// The results live in the sidebar, and the sidebar collapses — the toolbar has a button for it and
/// the split view remembers the choice — so a query typed with the sidebar shut would build its
/// results somewhere the user cannot see or reach. The search opens the sidebar for as long as
/// there is something in the field. Opening it is the smaller change of the two on offer: the
/// results are navigation and the sidebar is where this window's navigation is, whereas a results
/// list somewhere else would be a second place to look for the same thing depending on a state the
/// user cannot see from the field they are typing in.
- (void)searchChanged:(NSSearchField *)sender {
    for (NSView *result in [_searchResultsStack.arrangedSubviews copy]) [result removeFromSuperview];
    NSString *query = [sender.stringValue
        stringByTrimmingCharactersInSet:NSCharacterSet.whitespaceAndNewlineCharacterSet];
    if (query.length == 0) {
        _searchResults = nil;
        _searchResultsScroll.hidden = YES;
        _sidebarScroll.hidden = NO;
        if (_sidebarOpenedForSearch) {
            _sidebarOpenedForSearch = NO;
            _sidebarSplitItem.collapsed = YES;
        }
        return;
    }
    if (_sidebarSplitItem.collapsed) {
        _sidebarOpenedForSearch = YES;
        _sidebarSplitItem.collapsed = NO;
    }
    _sidebarScroll.hidden = YES;
    _searchResultsScroll.hidden = NO;
    NSMutableArray<MSIMESettingsSearchEntry *> *matches = [NSMutableArray array];
    for (MSIMESettingsSearchEntry *entry in _searchEntries)
        if ([entry rankForQuery:query] != NSNotFound && [self searchEntryReachable:entry]) [matches addObject:entry];
    // Stable, so that two settings the query answers equally well stay in the order their pages were built in rather than swapping places as the user types.
    NSArray<MSIMESettingsSearchEntry *> *ranked =
        [matches sortedArrayWithOptions:NSSortStable
                        usingComparator:^NSComparisonResult(MSIMESettingsSearchEntry *a, MSIMESettingsSearchEntry *b) {
                            const NSUInteger left = [a rankForQuery:query], right = [b rankForQuery:query];
                            return left < right ? NSOrderedAscending : (left > right ? NSOrderedDescending : NSOrderedSame);
                        }];
    const NSUInteger limit = 14;
    _searchResults = ranked.count > limit ? [ranked subarrayWithRange:NSMakeRange(0, limit)] : ranked;
    for (NSUInteger index = 0; index < _searchResults.count; ++index) {
        MSIMESettingsSearchEntry *entry = _searchResults[index];
        // The page, and the heading inside it: 「候选窗口 › 配色」 says where the setting is, which is what makes the next visit to it one the user can make without the search field.
        NSString *breadcrumb = entry.sectionTitle.length > 0
            ? [NSString stringWithFormat:@"%@ › %@", _pageTitles[(NSUInteger)entry.pageIndex], entry.sectionTitle]
            : _pageTitles[(NSUInteger)entry.pageIndex];
        NSButton *result = [NSButton buttonWithTitle:entry.title target:self action:@selector(openSearchResult:)];
        result.bordered = NO;
        result.alignment = NSTextAlignmentLeft;
        result.tag = (NSInteger)index;
        NSMutableAttributedString *label = [[NSMutableAttributedString alloc]
            initWithString:entry.title
                attributes:@{
                    NSFontAttributeName : [NSFont systemFontOfSize:kBodyFontSize],
                    NSForegroundColorAttributeName : NSColor.labelColor,
                }];
        [label appendAttributedString:[[NSAttributedString alloc]
            initWithString:[@"\n" stringByAppendingString:breadcrumb]
                attributes:@{
                    NSFontAttributeName : [NSFont systemFontOfSize:kDetailFontSize],
                    NSForegroundColorAttributeName : NSColor.secondaryLabelColor,
                }]];
        result.attributedTitle = label;
        result.cell.usesSingleLineMode = NO;
        result.cell.lineBreakMode = NSLineBreakByTruncatingTail;
        result.accessibilityLabel = [NSString stringWithFormat:@"%@，在%@", entry.title, breadcrumb];
        [_searchResultsStack addArrangedSubview:result];
        [result.widthAnchor constraintEqualToAnchor:_searchResultsStack.widthAnchor].active = YES;
        [result.heightAnchor constraintEqualToConstant:36.0].active = YES;
    }
    if (_searchResults.count == 0) [self showEmptySearchResultForQuery:query];
}
/// A query with no answer used to be one grey line where the navigation had been, and the sidebar stays replaced for as long as anything is in the field — so the window a user had just been looking at was gone, and nothing on screen said that emptying the field was the way back. It says so now, and offers the way back as a control rather than as a thing to know.
- (void)showEmptySearchResultForQuery:(NSString *)query {
    NSTextField *message = MSIMEDetailLabel([NSString stringWithFormat:@"没有与「%@」匹配的设置。", query]);
    NSButton *clear = [NSButton buttonWithTitle:@"清除搜索，返回导航"
                                         target:self
                                         action:@selector(clearSettingsSearch:)];
    MSIMELinkifyButton(clear, @"清除搜索，返回导航");
    for (NSView *view in @[ message, clear ]) {
        [_searchResultsStack addArrangedSubview:view];
        [view.widthAnchor constraintEqualToAnchor:_searchResultsStack.widthAnchor].active = YES;
    }
    [_searchResultsStack setCustomSpacing:8.0 afterView:message];
}
- (void)clearSettingsSearch:(id)sender {
    (void)sender;
    _searchField.stringValue = @"";
    [self searchChanged:_searchField];
}
- (void)openSearchResult:(NSButton *)sender {
    if (sender.tag < 0 || (NSUInteger)sender.tag >= _searchResults.count) return;
    MSIMESettingsSearchEntry *entry = _searchResults[(NSUInteger)sender.tag];
    [self showPreferencesPageAtIndex:entry.pageIndex navigationIndex:entry.pageIndex];
    // The search is over once it has been answered. Leaving the query in the field left the sidebar showing results for a search the user had already finished, with the page they had just landed on nowhere on screen and its row in the navigation hidden behind them.
    [self clearSettingsSearch:nil];
    NSView *row = entry.row;
    if (row == nil) return;
    // The page was hidden until a moment ago, so its frames mean nothing until layout settles.
    dispatch_async(dispatch_get_main_queue(), ^{
        NSScrollView *scrollView = row.enclosingScrollView;
        if (scrollView == nil) return;
        NSRect visible = [row convertRect:row.bounds toView:scrollView.documentView];
        [scrollView.documentView scrollRectToVisible:NSInsetRect(visible, 0.0, -60.0)];
        // A tinted box fading out behind the row, rather than animating the row's own layer: the
        // test binaries link AppKit but not QuartzCore, and `animator.alphaValue` needs neither.
        NSBox *flash = [[NSBox alloc] initWithFrame:row.bounds];
        flash.boxType = NSBoxCustom;
        flash.borderWidth = 0.0;
        flash.cornerRadius = 5.0;
        flash.fillColor = [[NSColor controlAccentColor] colorWithAlphaComponent:0.28];
        flash.autoresizingMask = NSViewWidthSizable | NSViewHeightSizable;
        [row addSubview:flash positioned:NSWindowBelow relativeTo:nil];
        [NSAnimationContext runAnimationGroup:^(NSAnimationContext *context) {
            context.duration = 1.1;
            flash.animator.alphaValue = 0.0;
        } completionHandler:^{
            [flash removeFromSuperview];
        }];
    });
}
- (void)layoutChanged:(NSPopUpButton *)sender { self.vertical = sender.indexOfSelectedItem == 1; }
- (void)candidateFollowCursorChanged:(NSSwitch *)sender { self.candidateFollowCursor = sender.state == NSControlStateValueOn; }
- (void)transpositionChanged:(NSSwitch *)sender { self.autocorrectTransposition = sender.state == NSControlStateValueOn; }
- (void)neighborChanged:(NSSwitch *)sender { self.autocorrectNeighbor = sender.state == NSControlStateValueOn; }
- (void)candidateLearningChanged:(NSSwitch *)sender { self.candidateLearningEnabled = sender.state == NSControlStateValueOn; }
- (void)frequencyModeChanged:(NSPopUpButton *)sender { self.frequencyAdjustmentMode = FrequencyModes()[sender.indexOfSelectedItem]; }
- (void)frequencyTriggerChanged:(NSPopUpButton *)sender { self.frequencyTriggerCount = sender.indexOfSelectedItem + 1; }
- (void)frequencyStepChanged:(NSPopUpButton *)sender { self.frequencyLinearStep = sender.indexOfSelectedItem + 1; }
// The control is an NSSwitch on develop; the cloud-candidate toggle routes through
// answerCloudCandidates: on this branch. Both apply.
- (void)cloudCandidatesChanged:(NSSwitch *)sender { [self answerCloudCandidates:sender.state == NSControlStateValueOn]; }
- (void)candidateTranslationsChanged:(NSSwitch *)sender { self.candidateTranslations = sender.state == NSControlStateValueOn; }
- (void)candidateEnglishGlossChanged:(NSSwitch *)sender { self.candidateEnglishGloss = sender.state == NSControlStateValueOn; }
- (void)quanpinHelpcodeChanged:(NSSwitch *)sender { self.quanpinHelpcodeEnabled = sender.state == NSControlStateValueOn; }
- (void)shuangpinHelpcodeChanged:(NSSwitch *)sender { self.shuangpinHelpcodeEnabled = sender.state == NSControlStateValueOn; }
- (void)profileChanged:(NSPopUpButton *)sender { self.shuangpinProfile = @[@"xiaohe", @"ziranma", @"shoudao", @"microsoft"][sender.indexOfSelectedItem]; }
- (void)wubiProfileChanged:(NSPopUpButton *)sender { self.wubiProfile = sender.indexOfSelectedItem == 1 ? @"wubi98" : @"wubi86"; }
- (void)preeditChanged:(NSPopUpButton *)sender { self.shuangpinPreeditUsesRaw = sender.indexOfSelectedItem == 1; }
- (void)inputModeShortcutChanged:(NSSwitch *)sender { self.inputModeShortcut = sender.state == NSControlStateValueOn; }
- (void)defaultImeModeChanged:(NSPopUpButton *)sender { self.defaultImeMode = sender.indexOfSelectedItem == 1 ? @"english" : @"chinese"; }
- (void)imeModeScopeChanged:(NSPopUpButton *)sender { self.imeModeScope = sender.indexOfSelectedItem == 1 ? @"global" : @"app"; }
- (void)shiftTapShortcutChanged:(NSSwitch *)sender { self.shiftTapShortcut = sender.state == NSControlStateValueOn; }
- (void)controlTapShortcutChanged:(NSSwitch *)sender { self.controlTapShortcut = sender.state == NSControlStateValueOn; }
- (void)controlOptionSpaceShortcutChanged:(NSSwitch *)sender { self.controlOptionSpaceShortcut = sender.state == NSControlStateValueOn; }
- (void)characterSetShortcutChanged:(NSSwitch *)sender { self.characterSetShortcut = sender.state == NSControlStateValueOn; }
- (void)fullWidthChanged:(NSSwitch *)sender { self.fullWidthInput = sender.state == NSControlStateValueOn; }
- (void)fullWidthShortcutChanged:(NSSwitch *)sender { self.fullWidthShortcut = sender.state == NSControlStateValueOn; }
- (void)voiceEnabledChanged:(NSSwitch *)sender { self.voiceInputEnabled = sender.state == NSControlStateValueOn; }
- (void)voiceLanguageChanged:(NSPopUpButton *)sender { self.voiceLanguage = sender.indexOfSelectedItem == 1 ? @"en-US" : @"zh-CN"; }
- (void)voiceSoundChanged:(NSSwitch *)sender { self.voiceSoundEnabled = sender.state == NSControlStateValueOn; }
- (void)voiceMuteSystemAudioChanged:(NSSwitch *)sender { self.voiceMuteSystemAudio = sender.state == NSControlStateValueOn; }
- (void)voiceStreamInlinePreeditChanged:(NSSwitch *)sender { self.voiceStreamInlinePreedit = sender.state == NSControlStateValueOn; }
- (void)voiceHotkeyCtrlF9Changed:(NSSwitch *)sender { self.voiceHotkeyCtrlF9 = sender.state == NSControlStateValueOn; }
- (void)voiceHotkeyRightAltChanged:(NSSwitch *)sender { self.voiceHotkeyRightAlt = sender.state == NSControlStateValueOn; }
- (void)voiceHotkeyCtrlCommandChanged:(NSSwitch *)sender { self.voiceHotkeyCtrlCommand = sender.state == NSControlStateValueOn; }
- (void)voiceHotkeyCtrlOptionChanged:(NSSwitch *)sender { self.voiceHotkeyCtrlOption = sender.state == NSControlStateValueOn; }
- (void)voiceHotkeyHoldSpaceChanged:(NSSwitch *)sender { self.voiceHotkeyHoldSpace = sender.state == NSControlStateValueOn; }
- (void)traditionalOutputChanged:(NSSwitch *)sender { self.traditionalOutput = sender.state == NSControlStateValueOn; }
- (void)keymapChanged:(NSSwitch *)sender { self.shuangpinKeymap = sender.state == NSControlStateValueOn; }
- (void)wubiChanged:(NSSwitch *)sender { self.wubiAutoCommitUnique = sender.state == NSControlStateValueOn; }
- (void)wubiMixedPinyinChanged:(NSSwitch *)sender { self.wubiMixedPinyinEnabled = sender.state == NSControlStateValueOn; }
- (void)punctuationChanged:(NSSwitch *)sender { self.chinesePunctuation = sender.state == NSControlStateValueOn; }
- (void)smartPunctuationChanged:(NSSwitch *)sender { self.smartPunctuation = sender.state == NSControlStateValueOn; }
- (void)smartPunctuationRepeatChanged:(NSSwitch *)sender { self.smartPunctuationRepeatToChinese = sender.state == NSControlStateValueOn; }
- (void)smartPunctuationSpaceChanged:(NSSwitch *)sender { self.smartPunctuationSpaceConvert = sender.state == NSControlStateValueOn; }
- (void)pairedPunctuationChanged:(NSSwitch *)sender { self.pairedPunctuation = sender.state == NSControlStateValueOn; }
- (void)punctuationLockChanged:(NSPopUpButton *)sender { self.punctuationLock = @[@"follow", @"chinese", @"english"][sender.indexOfSelectedItem]; }
- (void)mixedEnglishChanged:(NSSwitch *)sender { self.mixedEnglishInput = sender.state == NSControlStateValueOn; }
- (void)mixedEnglishPrefixChanged:(NSPopUpButton *)sender { self.mixedEnglishMinimumPrefix = sender.indexOfSelectedItem + 1; }
- (void)mixedEmojiChanged:(NSSwitch *)sender { self.mixedEmojiInput = sender.state == NSControlStateValueOn; }
- (void)mixedKaomojiChanged:(NSSwitch *)sender { self.mixedKaomojiInput = sender.state == NSControlStateValueOn; }
- (void)toolbarChanged:(NSSwitch *)sender { self.floatingToolbarEnabled = sender.state == NSControlStateValueOn; }
- (void)toolbarEnglishModeChanged:(NSButton *)sender { self.floatingToolbarEnglishMode = sender.state == NSControlStateValueOn; }
- (void)toolbarInputSchemeChanged:(NSButton *)sender { self.floatingToolbarInputScheme = sender.state == NSControlStateValueOn; }
- (void)toolbarHandwritingChanged:(NSButton *)sender { self.floatingToolbarHandwriting = sender.state == NSControlStateValueOn; }
- (void)toolbarVoiceChanged:(NSButton *)sender { self.floatingToolbarVoice = sender.state == NSControlStateValueOn; }
- (void)toolbarThemeChanged:(NSPopUpButton *)sender { self.toolbarTheme = SurfaceThemes()[sender.indexOfSelectedItem]; }
- (void)toolbarPunctuationChanged:(NSButton *)sender { self.floatingToolbarPunctuation = sender.state == NSControlStateValueOn; }
- (void)toolbarFullWidthChanged:(NSButton *)sender { self.floatingToolbarFullWidth = sender.state == NSControlStateValueOn; }
- (void)toolbarCharacterSetChanged:(NSButton *)sender { self.floatingToolbarCharacterSet = sender.state == NSControlStateValueOn; }
- (void)toolbarEmojiChanged:(NSButton *)sender { self.floatingToolbarEmoji = sender.state == NSControlStateValueOn; }
- (void)toolbarScreenKeyboardChanged:(NSButton *)sender { self.floatingToolbarScreenKeyboard = sender.state == NSControlStateValueOn; }
- (void)toolbarSettingsChanged:(NSButton *)sender { self.floatingToolbarSettings = sender.state == NSControlStateValueOn; }
- (void)toolbarScaleChanged:(NSPopUpButton *)sender { self.floatingToolbarScalePercent = [@[@75, @100, @125, @150][sender.indexOfSelectedItem] integerValue]; }
- (void)toolbarFontSizeChanged:(NSPopUpButton *)sender { self.floatingToolbarFontSize = 16 + sender.indexOfSelectedItem * 2; }
- (MetasequoiaSkinSettingsView *)ensureSkinSettingsView {
    (void)self.window;  // The page container is built with the rest of the pages.
    if (_skinSettingsView != nil) return _skinSettingsView;
    _skinSettingsView = [[MetasequoiaSkinSettingsView alloc] initWithFrame:NSZeroRect preferences:self];
    [_skinPageContainer addSubview:_skinSettingsView];
    [NSLayoutConstraint activateConstraints:@[
        [_skinSettingsView.leadingAnchor constraintEqualToAnchor:_skinPageContainer.leadingAnchor],
        [_skinSettingsView.trailingAnchor constraintEqualToAnchor:_skinPageContainer.trailingAnchor],
        [_skinSettingsView.topAnchor constraintEqualToAnchor:_skinPageContainer.topAnchor],
        [_skinSettingsView.bottomAnchor constraintEqualToAnchor:_skinPageSharedEntry.topAnchor constant:-6.0],
    ]];
    return _skinSettingsView;
}
- (NSView *)skinSettingsView {
    MetasequoiaSkinSettingsView *view = [self ensureSkinSettingsView];
    [view reload];
    return view;
}
- (void)showSkinCatalog:(id)sender {
    (void)sender;
    __weak MSIMEAppearancePreferences *weakSelf = self;
    // The native fallback used to open a second window holding the same browser the 皮肤 page now
    // is, so falling back means showing that page rather than a duplicate of it.
    MSIMEOpenDesktopSettings(MSIMEDesktopSettingsPage::Skin, [self desktopSettingsWorkspace], ^{
        MSIMEAppearancePreferences *strongSelf = weakSelf;
        if (strongSelf == nil) return;
        [[strongSelf ensureSkinSettingsView] reload];
        [strongSelf showPreferencesPageAtIndex:kSkinPageIndex navigationIndex:kSkinPageIndex];
        [strongSelf showWindow:nil];
        MSIMEPresentWindow(strongSelf.window);
    });
}
- (void)showDictionary:(id)sender {
    (void)sender;
    __weak MSIMEAppearancePreferences *weakSelf = self;
    MSIMEOpenDesktopRoute(@"settings:dictionary", NSWorkspace.sharedWorkspace, ^{
        MSIMEAppearancePreferences *strongSelf = weakSelf;
        if (!strongSelf) return;
        NSDictionary *options = MSIMELoadRuntimeOptions();
        if (![options isKindOfClass:NSDictionary.class]) {
            NSAlert *alert = [NSAlert new];
            alert.messageText = @"词库管理暂不可用";
            alert.informativeText = @"请先激活输入法，再从输入法菜单打开词库管理。";
            [alert addButtonWithTitle:@"好"];
            [alert runModal];
            return;
        }
        strongSelf->_dictionaryWindow = [[MSIMEDictionaryWindowController alloc] initWithOptions:options];
        [strongSelf->_dictionaryWindow showWindow:nil];
        MSIMEPresentWindow(strongSelf->_dictionaryWindow.window);
    });
}
- (void)togglePreviewTheme:(id)sender { (void)sender; [_preview toggleForcedTheme]; }
- (void)togglePreviewShowcase:(NSButton *)sender { [_preview setShowsLayoutShowcase:sender.state == NSControlStateValueOn]; }
- (void)previewSampleChanged:(NSTextField *)sender { [_preview setSampleText:sender.stringValue]; }
/// The sample text as it is typed, rather than on Return or on leaving the field: the preview is being watched while the words go in. -setSampleText: is what keeps that from redrawing once per keystroke.
- (void)controlTextDidChange:(NSNotification *)notification {
    if (notification.object == _previewSampleField) [_preview setSampleText:_previewSampleField.stringValue];
}
/// The page a stable identifier names, or -1 when nothing is named — an identifier written by a
/// version that had a page this one does not is not an error, it is a page that went away.
- (NSInteger)pageIndexForIdentifier:(NSString *)identifier {
    if (identifier.length == 0) return -1;
    const NSUInteger index = [_pageIdentifiers indexOfObject:identifier];
    return index == NSNotFound ? -1 : (NSInteger)index;
}
- (BOOL)showSettingsPageWithIdentifier:(NSString *)identifier {
    (void)self.window;  // The page list is built with the window; a deep link can arrive before it.
    const NSInteger pageIndex = [self pageIndexForIdentifier:identifier];
    if (pageIndex < 0) return NO;
    [self showPreferencesPageAtIndex:pageIndex navigationIndex:pageIndex];
    return YES;
}
- (void)showPreferencesPageAtIndex:(NSInteger)pageIndex navigationIndex:(NSInteger)navigationIndex {
    _selectedPageIndex = pageIndex;
    if (pageIndex >= 0 && pageIndex < (NSInteger)_pageIdentifiers.count)
        [_defaults setObject:_pageIdentifiers[pageIndex] forKey:LastSettingsPageKey];
    for (NSInteger index = 0; index < (NSInteger)_preferencePages.count; ++index)
        _preferencePages[index].hidden = index != pageIndex;
    [self selectSidebarRowForPageAtIndex:navigationIndex];
    // The window's chrome says which page you are on, which is what the title bar is for and what
    // this window has never used it for.
    if (navigationIndex >= 0 && navigationIndex < (NSInteger)_pageTitles.count)
        [super window].title = _pageTitles[navigationIndex];
    [self performPageEntrySideEffects];
}
/// The four pages that do something when they are entered: the skin browser builds itself and
/// renders a live candidate preview per built-in skin, the voice form re-reads its provider
/// settings, the 关于 page asks the update controller where the build stands, and the account page
/// attaches a view owned by the Swift backend that every other page has to detach again.
///
/// The update state is asked for here rather than once while the window is built because none of it
/// is a preference: the answers change while the window is open — a check finishes, Sparkle's
/// automatic checking is turned on elsewhere — and a window left open on another page used to go on
/// reporting what was true when it was first opened.
///
/// None of it is owed to a page that is merely selected. The window opens on the page it was left on, and doing this work while the window is still being built means a launch that lands on 皮肤 pays for a rescan of the skin directory and a live candidate preview per skin found before anything is on screen. So it waits for the window to appear, and -preferencesWindowDidBecomeKey: is what lets it through.
- (void)performPageEntrySideEffects {
    if (!_windowHasAppeared) return;
    if (_selectedPageIndex == kSkinPageIndex) [self ensureSkinSettingsView];
    if (_selectedPageIndex == kVoicePageIndex) [_voiceSettingsView reloadSettings];
    if (_selectedPageIndex == kAboutPageIndex) [self refreshUpdateControls];
    if (_selectedPageIndex == kAccountPageIndex && MSIMEAccountPaneAttach != nullptr)
        MSIMEAccountPaneAttach(self.window);
    else if (MSIMEAccountPaneClose != nullptr)
        MSIMEAccountPaneClose();
}
- (void)selectSidebarRowForPageAtIndex:(NSInteger)navigationIndex {
    if (_sidebarOutline == nil) return;
    for (MSIMESettingsSidebarItem *group in _sidebarGroups)
        for (MSIMESettingsSidebarItem *member in group.children) {
            if (member.pageIndex != navigationIndex) continue;
            const NSInteger row = [_sidebarOutline rowForItem:member];
            if (row < 0 || row == _sidebarOutline.selectedRow) return;
            _updatingSidebarSelection = YES;
            [_sidebarOutline selectRowIndexes:[NSIndexSet indexSetWithIndex:(NSUInteger)row]
                         byExtendingSelection:NO];
            [_sidebarOutline scrollRowToVisible:row];
            _updatingSidebarSelection = NO;
            return;
        }
}

#pragma mark - Sidebar source list

- (NSInteger)outlineView:(NSOutlineView *)outlineView numberOfChildrenOfItem:(id)item {
    (void)outlineView;
    if (item == nil) return (NSInteger)_sidebarGroups.count;
    return (NSInteger)[(MSIMESettingsSidebarItem *)item children].count;
}
- (id)outlineView:(NSOutlineView *)outlineView child:(NSInteger)index ofItem:(id)item {
    (void)outlineView;
    if (item == nil) return _sidebarGroups[(NSUInteger)index];
    return [(MSIMESettingsSidebarItem *)item children][(NSUInteger)index];
}
- (BOOL)outlineView:(NSOutlineView *)outlineView isItemExpandable:(id)item {
    (void)outlineView;
    return [(MSIMESettingsSidebarItem *)item children].count > 0;
}
- (BOOL)outlineView:(NSOutlineView *)outlineView isGroupItem:(id)item {
    (void)outlineView;
    return [(MSIMESettingsSidebarItem *)item children].count > 0;
}
- (BOOL)outlineView:(NSOutlineView *)outlineView shouldSelectItem:(id)item {
    (void)outlineView;
    return [(MSIMESettingsSidebarItem *)item children].count == 0;
}
/// The four groups — 打字, 显示, 数据与账号, 支持 — are the window's structure, not something to fold away: collapsing 打字 would hide four of the eleven pages behind a triangle nothing else in the window mentions.
- (BOOL)outlineView:(NSOutlineView *)outlineView shouldCollapseItem:(id)item {
    (void)outlineView;
    (void)item;
    return NO;
}
- (BOOL)outlineView:(NSOutlineView *)outlineView shouldShowOutlineCellForItem:(id)item {
    (void)outlineView;
    (void)item;
    return NO;
}
- (CGFloat)outlineView:(NSOutlineView *)outlineView heightOfRowByItem:(id)item {
    (void)outlineView;
    (void)item;
    return kSidebarRowHeight;
}
- (NSView *)outlineView:(NSOutlineView *)outlineView
     viewForTableColumn:(NSTableColumn *)tableColumn
                   item:(id)item {
    (void)tableColumn;
    MSIMESettingsSidebarItem *entry = item;
    const BOOL group = entry.children.count > 0;
    NSUserInterfaceItemIdentifier identifier = group ? @"MSIMESettingsSidebarGroupCell" : @"MSIMESettingsSidebarCell";
    NSTableCellView *cell = [outlineView makeViewWithIdentifier:identifier owner:self];
    if (cell == nil) {
        cell = [[NSTableCellView alloc] initWithFrame:NSZeroRect];
        cell.identifier = identifier;
        NSTextField *label = [NSTextField labelWithString:@""];
        label.font = [NSFont systemFontOfSize:kBodyFontSize];
        // A sidebar narrow enough to be dragged to 204pt truncates 帮助与反馈 rather than drawing it past its own edge.
        label.lineBreakMode = NSLineBreakByTruncatingTail;
        label.translatesAutoresizingMaskIntoConstraints = NO;
        [cell addSubview:label];
        cell.textField = label;
        [label.centerYAnchor constraintEqualToAnchor:cell.centerYAnchor].active = YES;
        [label.trailingAnchor constraintLessThanOrEqualToAnchor:cell.trailingAnchor].active = YES;
        if (group) {
            [label.leadingAnchor constraintEqualToAnchor:cell.leadingAnchor].active = YES;
        } else {
            NSImageView *icon = [[NSImageView alloc] initWithFrame:NSZeroRect];
            icon.imageScaling = NSImageScaleProportionallyDown;
            icon.translatesAutoresizingMaskIntoConstraints = NO;
            [cell addSubview:icon];
            cell.imageView = icon;
            [NSLayoutConstraint activateConstraints:@[
                [icon.leadingAnchor constraintEqualToAnchor:cell.leadingAnchor],
                [icon.centerYAnchor constraintEqualToAnchor:cell.centerYAnchor],
                [icon.widthAnchor constraintEqualToConstant:18.0],
                [label.leadingAnchor constraintEqualToAnchor:icon.trailingAnchor constant:6.0],
            ]];
        }
    }
    cell.textField.stringValue = entry.title;
    // Named explicitly rather than left to be inferred from the text field: the cell is a container
    // with an image in it as well, and the row is what VoiceOver and the tests address.
    cell.accessibilityLabel = entry.title;
    cell.imageView.image = entry.symbolName.length > 0
        ? [NSImage imageWithSystemSymbolName:entry.symbolName accessibilityDescription:nil]
        : nil;
    return cell;
}
- (void)outlineViewSelectionDidChange:(NSNotification *)notification {
    (void)notification;
    if (_updatingSidebarSelection) return;
    MSIMESettingsSidebarItem *item = [_sidebarOutline itemAtRow:_sidebarOutline.selectedRow];
    if (item == nil || item.children.count > 0) return;
    [self showPreferencesPageAtIndex:item.pageIndex navigationIndex:item.pageIndex];
}

#pragma mark - Toolbar

- (NSArray<NSToolbarItemIdentifier> *)toolbarDefaultItemIdentifiers:(NSToolbar *)toolbar {
    (void)toolbar;
    return @[
        NSToolbarToggleSidebarItemIdentifier, MSIMESettingsSearchItemIdentifier,
        MSIMESettingsSeparatorItemIdentifier, NSToolbarFlexibleSpaceItemIdentifier,
        MSIMESettingsMoreItemIdentifier,
    ];
}
- (NSArray<NSToolbarItemIdentifier> *)toolbarAllowedItemIdentifiers:(NSToolbar *)toolbar {
    return [self toolbarDefaultItemIdentifiers:toolbar];
}
- (NSToolbarItem *)toolbar:(NSToolbar *)toolbar
        itemForItemIdentifier:(NSToolbarItemIdentifier)identifier
    willBeInsertedIntoToolbar:(BOOL)inserted {
    (void)toolbar;
    (void)inserted;
    if ([identifier isEqualToString:MSIMESettingsSearchItemIdentifier]) {
        _searchToolbarItem = [[NSSearchToolbarItem alloc] initWithItemIdentifier:identifier];
        _searchToolbarItem.searchField = _searchField;
        _searchToolbarItem.resignsFirstResponderWithCancel = YES;
        // Wide enough that the sidebar's own width is what the field spans, which is where a Mac
        // settings window puts its search field.
        _searchToolbarItem.preferredWidthForSearchField = kSidebarWidth - 24.0;
        return _searchToolbarItem;
    }
    if ([identifier isEqualToString:MSIMESettingsSeparatorItemIdentifier])
        return [NSTrackingSeparatorToolbarItem trackingSeparatorToolbarItemWithIdentifier:identifier
                                                                                splitView:_splitViewController.splitView
                                                                             dividerIndex:0];
    if ([identifier isEqualToString:MSIMESettingsMoreItemIdentifier]) {
        NSMenuToolbarItem *more = [[NSMenuToolbarItem alloc] initWithItemIdentifier:identifier];
        more.image = [NSImage imageWithSystemSymbolName:@"ellipsis.circle" accessibilityDescription:@"更多设置操作"];
        more.label = @"更多";
        more.toolTip = @"更多设置操作";
        NSMenu *menu = [[NSMenu alloc] initWithTitle:@"更多"];
        // Above the separator, and so above 恢复全部设置…, because they are what makes that one recoverable: moving settings between machines used to mean signing in and waiting on a server, and the one item in this menu that throws every setting away stood here with no way to keep a copy first.
        for (NSArray *entry in @[ @[@"导出设置…", NSStringFromSelector(@selector(exportSettings:))],
                                  @[@"导入设置…", NSStringFromSelector(@selector(importSettings:))] ]) {
            NSMenuItem *item = [[NSMenuItem alloc] initWithTitle:entry[0]
                                                          action:NSSelectorFromString(entry[1])
                                                   keyEquivalent:@""];
            item.target = self;
            [menu addItem:item];
        }
        [menu addItem:[NSMenuItem separatorItem]];
        // The global reset used to sit in a footer strip that cost all thirteen pages 42pt, one row
        // above the red 卸载… button on 关于. It is an action taken once, so it belongs in a menu.
        NSMenuItem *restore = [[NSMenuItem alloc] initWithTitle:@"恢复全部设置…"
                                                         action:@selector(restoreAllDefaults:)
                                                  keyEquivalent:@""];
        restore.target = self;
        [menu addItem:restore];
        more.menu = menu;
        return more;
    }
    return nil;
}
/// ⌘F and clicking the toolbar's own field are the same interaction, so the key equivalent asks the
/// toolbar item to begin it rather than reaching for the field and making it first responder behind
/// the item's back. MSIMESettingsDetailView is where the key equivalent is caught.
- (void)beginSettingsSearch:(id)sender {
    (void)sender;
    [_searchToolbarItem beginSearchInteraction];
}
- (void)schemeRadioChanged:(NSButton *)sender {
    self.inputScheme = MSIMEInputSchemeNames()[sender.tag];
}
// 当前方案在菜单栏的入口还没加入输入法列表时显示提示；没有探针（测试与其它链接了设置窗口的程序）时不显示。
// 系统设置的键盘设置扩展跑在沙盒里，用的是自己容器里的输入源缓存，替换 bundle 后不刷新，所以新版本加的模式要注销重新登录后才进得了「添加」对话框，提示里一并说明。
- (void)refreshInputModeHint {
    if (!_inputModeHintRow) return;
    NSString *mode = MSIMEInputModeIDForSchemeIn(MSIMEEditionInfo(), self.inputScheme);
    const BOOL missing = MSIMEInputModeEnabledProbe != nullptr && !MSIMEInputModeEnabledProbe(mode);
    _inputModeHintRow.hidden = !missing;
    if (!missing) return;
    _inputModeHintLabel.stringValue = [NSString stringWithFormat:@"菜单栏里还没有「%@」，要先把它加进输入法列表才能从菜单栏切过去。macOS 只允许你自己添加：点「打开键盘设置」，在「输入法」一行点「编辑…」，再点左下角「+」，在左栏选或搜索「%@」后添加。刚安装或刚更新出来的入口，要注销并重新登录一次才会出现在「添加」对话框里。", MSIMEInputModeMenuName(mode), MSIMEInputModeAddDialogLanguage(mode)];
}
- (void)openInputSourceSettings:(id)sender {
    (void)sender;
    [NSWorkspace.sharedWorkspace openURL:[NSURL URLWithString:@"x-apple.systempreferences:com.apple.Keyboard-Settings.extension"]];
}
- (void)showBackendAccount:(id)sender {
    (void)sender;
    if (MSIMEAccountPaneAttach != nullptr) {
        [self showPreferencesPageAtIndex:kAccountPageIndex navigationIndex:kAccountPageIndex];
        MSIMEAccountPaneAttach(self.window);
    } else {
        MSIMEOpenBackendAccount(NSClassFromString(@"MSIMEBackendAccountWindow"));
    }
}
- (void)showHelp:(id)sender { (void)sender; [self showSupportPage:MSIMESupportPageHelp]; }
- (void)showFeedback:(id)sender { (void)sender; [self showSupportPage:MSIMESupportPageFeedback]; }
/// The support window, on the page that was asked for.
///
/// Both buttons used to send one no-argument selector that called -showWindow:, and that window
/// builds its contentView in -showPage: — so the first press of either opened a blank, untitled
/// window, and a later press showed whichever page something else had opened last. The class is
/// still resolved at runtime: it is linked into the input method and into two test binaries, and
/// naming it here would pull it into the other five that build this window.
- (void)showSupportPage:(MSIMESupportPage)page {
    Class supportClass = NSClassFromString(@"MSIMESupportWindowController");
    if (![supportClass respondsToSelector:@selector(sharedController)]) return;
    MSIMESupportWindowController *controller = [supportClass sharedController];
    // -showPage: presents the window and activates the application itself.
    [controller showPage:page];
}
- (void)openProductWebsite:(id)sender {
    (void)sender;
    [NSWorkspace.sharedWorkspace openURL:[NSURL URLWithString:@"https://msime.app"]];
}
- (void)uninstallInputSource:(id)sender {
    (void)sender;
    if (msime_macos_uninstall_input_source == nullptr) return;
    NSAlert *confirmation = [NSAlert new];
    confirmation.alertStyle = NSAlertStyleWarning;
    confirmation.messageText = [NSString stringWithFormat:@"确认卸载%@？", MSIMEEditionDisplayName()];
    confirmation.informativeText = _removeUserDataButton.state == NSControlStateValueOn
        ? @"输入法会移到废纸篓，并删除本机词库、学习记录、偏好与语音密钥。"
        : @"输入法会移到废纸篓；本机词库、学习记录和偏好会保留。";
    // Cancel first, so it is the button Return presses — the same rule the restore alerts follow,
    // and this is the one alert in the window where the other button moves an application to the
    // trash and can take the user's dictionary with it.
    [confirmation addButtonWithTitle:@"取消"];
    [confirmation addButtonWithTitle:@"确认卸载"];
    if ([confirmation runModal] == NSAlertFirstButtonReturn) return;

    NSFileManager *fileManager = NSFileManager.defaultManager;
    NSURL *library = [fileManager URLForDirectory:NSLibraryDirectory
                                          inDomain:NSUserDomainMask
                                 appropriateForURL:nil
                                            create:NO
                                             error:nil];
    NSURL *inputMethods = [library URLByAppendingPathComponent:@"Input Methods" isDirectory:YES];
    // 只卸载本版本的 bundle，同时安装的其他版本不动。
    NSString *bundleName = MSIMEInputMethodBundleName();
    if (!bundleName) return;
    NSURL *bundle = [inputMethods URLByAppendingPathComponent:bundleName isDirectory:YES];
    NSDictionary *runtime = MSIMELoadRuntimeOptions();
    NSString *configuredState = [runtime[ @"preferences_directory"] isKindOfClass:NSString.class]
        && [runtime[@"preferences_directory"] isAbsolutePath] ? runtime[@"preferences_directory"] : nil;
    NSURL *defaultState = MSIMEDefaultClientStateDirectory(fileManager);
    NSString *userData = configuredState ?: defaultState.path;
    BOOL ok = msime_macos_uninstall_input_source(bundle.path.fileSystemRepresentation,
                                                  userData.fileSystemRepresentation,
                                                  MSIMEInputMethodBundleIdentifier().UTF8String,
                                                  _removeUserDataButton.state == NSControlStateValueOn);
    if (!ok) {
        NSAlert *failure = [NSAlert new];
        failure.alertStyle = NSAlertStyleCritical;
        failure.messageText = @"卸载未能完成";
        failure.informativeText = @"输入法没有移到废纸篓；本机数据未被删除。";
        [failure runModal];
        return;
    }
    [NSApp terminate:nil];
}
- (void)checkForUpdates:(id)sender { [_updateController checkForUpdates:sender]; }
- (void)refreshUpdateControls {
    NSString *version = [NSBundle.mainBundle objectForInfoDictionaryKey:@"CFBundleShortVersionString"];
    _versionLabel.stringValue = version.length == 0 ? @"开发构建" : [NSString stringWithFormat:@"v%@", version];
    // Without the update controller linked in, the page still renders and simply reports that
    // this build cannot check; it must not claim automatic checks are on.
    const BOOL automatic = [_updateController automaticallyChecksForUpdates];
    _automaticUpdateLabel.stringValue = automatic ? @"已开启自动检查" : @"自动检查已关闭";
    _updatePageButton.enabled = [_updateController canCheckForUpdates];
}
/// The heading above a card, and the registration of the settings under it as one thing the window can put back. A section with no stored settings of its own passes an empty list and gets a heading with no link, which is how both 效果预览 sections, 本机词库, 使用帮助, 问题反馈 and the three headings of 关于 stay quiet.
- (NSView *)sectionHeader:(NSString *)title keys:(NSArray<NSString *> *)keys {
    return [self sectionHeader:title keys:keys fields:nil];
}
/// The same heading, for a section that owns named entries of a stored dictionary rather than the whole key; see MSIMESettingsSection.fields.
- (NSView *)sectionHeader:(NSString *)title
                     keys:(NSArray<NSString *> *)keys
                   fields:(NSDictionary<NSString *, NSArray<NSString *> *> *)fields {
    NSButton *link = nil;
    if (keys.count > 0) {
        link = [NSButton buttonWithTitle:@"恢复默认值" target:self action:@selector(restoreDefaults:)];
        MSIMELinkifyButton(link, [NSString stringWithFormat:@"恢复「%@」的默认设置", title]);
        // Which section was pressed, rather than which page happens to be in front: a page has
        // several of these and they restore different things.
        link.tag = (NSInteger)_restorableSections.count;
        // Shown by -refreshSectionRestoreLinks, and only where there is something to restore.
        link.hidden = YES;
        // A key with no probe is a key this window cannot tell from its default, which would leave the section either always offering a restore or never offering one. The test binaries build with -UNDEBUG, so a key registered here and forgotten in SettingProbes() fails loudly in all of them rather than turning into a link that lies.
        for (NSString *key in keys)
            NSAssert(SettingProbes()[key] != nil, @"「%@」registers %@, which SettingProbes() has no probe for", title, key);
        MSIMESettingsSection *section = [MSIMESettingsSection new];
        section.title = title;
        section.keys = keys;
        section.fields = fields;
        section.restoreLink = link;
        [_restorableSections addObject:section];
    }
    NSView *header = MSIMESectionHeaderRow(title, link);
    // Which heading this is, for -page:title:summary:content: to hand to the settings registered by the card that follows it. A section with nothing to restore has no link and so no entry in _restorableSections, but it still names the settings under it.
    [_sectionTitlesByHeader setObject:title forKey:header];
    return header;
}
// Clears only this host's own preference domain, by an explicit key list. It never touches the
// Apple product's domain or the user dictionary.
//
// The list is the union of what the sections declare rather than a list of its own. The two used to be written out separately and had drifted: the whole-window list named twenty-one of the fifty keys this file declares, and the per-page list covered six pages of the thirteen there were then and named two keys — MSIMEClientInputModeHUD and MSIMEClientFullWidthShortcut — whose controls were not in this window at all. A key reaches this list now by being under a heading, which is also the only way it can be reached by the restore the user actually presses. A section that owns only part of a stored dictionary still contributes the whole key here, because this list is what 恢复全部设置 removes and that one really does mean everything.
- (NSArray<NSString *> *)restorableKeys {
    NSMutableArray<NSString *> *keys = [NSMutableArray array];
    for (MSIMESettingsSection *section in _restorableSections)
        for (NSString *key in section.keys)
            if (![keys containsObject:key]) [keys addObject:key];
    return keys;
}
- (void)removeStoredKeys:(NSArray<NSString *> *)keys {
    [self removeStoredKeys:keys fields:nil];
}
/// The same removal, restricted for a key named in `fields` to the entries of its stored dictionary that the section owns; everything else in that dictionary, and the account's pushed value for it, is left exactly as it was.
- (void)removeStoredKeys:(NSArray<NSString *> *)keys
                  fields:(NSDictionary<NSString *, NSArray<NSString *> *> *)fields {
    NSDictionary<NSString *, NSString *> *overrides = SharedOverrideProperties();
    for (NSString *key in keys) {
        NSArray<NSString *> *owned = fields[key];
        // And the account's pushed value, which the accessors read first; see
        // SharedOverrideProperties() for why deleting the stored entry alone was not a restore.
        NSString *override = overrides[key];
        if (owned == nil) {
            [_defaults removeObjectForKey:key];
            if (override != nil) [self setValue:nil forKey:override];
            continue;
        }
        NSMutableDictionary *stored = [[_defaults dictionaryForKey:key] mutableCopy];
        [stored removeObjectsForKeys:owned];
        // A dictionary with nothing left in it is not the same as no dictionary at all for a key whose accessors fall back on the stored entry being absent, so the key goes rather than being left holding an empty one.
        if (stored.count > 0) [_defaults setObject:stored forKey:key]; else [_defaults removeObjectForKey:key];
        id pushed = override == nil ? nil : [self valueForKey:override];
        if ([pushed isKindOfClass:NSMutableDictionary.class]) [pushed removeObjectsForKeys:owned];
    }
    [self refreshControls];
    [NSNotificationCenter.defaultCenter postNotificationName:MSIMEAppearanceDidChangeNotification object:self];
}
#pragma mark - Settings document

/// What a settings document says it is, so that the file this window is handed can be turned down for a reason rather than by silently failing to be a snapshot.
static NSString *const MSIMESettingsDocumentFormat = @"app.msime.client.settings";
static NSString *const MSIMESettingsDocumentFormatField = @"format";
static NSString *const MSIMESettingsDocumentVersionField = @"version";
static NSString *const MSIMESettingsDocumentSettingsField = @"settings";
/// The scope of the document, said in the two panels rather than left to be discovered. The payload is -cloudSettingsSnapshot, which is the set of settings that already travels between machines through the account; it is not everything this window holds, and a user about to reinstall should know that before they rely on the file.
static NSString *const MSIMESettingsDocumentScope =
    @"包含可跨机器同步的那部分设置：皮肤、候选排列与字号、每页候选、输入方案与辅助码方案、翻页键组，以及标点、简繁、云候选等开关。字体、配色、主题、快捷键、语音与应用例外不在其中。";
static const NSUInteger MSIMESettingsDocumentLimit = 1 << 20;

/// Writes the snapshot the account sync already speaks to a file the user keeps.
- (void)exportSettings:(id)sender {
    (void)sender;
    NSSavePanel *panel = [NSSavePanel savePanel];
    panel.nameFieldStringValue = @"水杉输入法设置.json";
    panel.prompt = @"导出";
    panel.message = MSIMESettingsDocumentScope;
    if ([panel runModal] != NSModalResponseOK || panel.URL == nil) return;
    NSDictionary *document = @{
        MSIMESettingsDocumentFormatField : MSIMESettingsDocumentFormat,
        MSIMESettingsDocumentVersionField : @1,
        MSIMESettingsDocumentSettingsField : [self cloudSettingsSnapshot],
    };
    NSError *error = nil;
    // Sorted and indented: the file is something a user may well open, diff against another machine's or keep in a repository of their own, and none of that works on one line in dictionary order.
    NSData *data = [NSJSONSerialization dataWithJSONObject:document
                                                   options:NSJSONWritingPrettyPrinted | NSJSONWritingSortedKeys
                                                     error:&error];
    if (data == nil || ![data writeToURL:panel.URL options:NSDataWritingAtomic error:&error]) {
        [self reportSettingsDocumentFailure:@"导出设置未能完成"
                                     reason:error.localizedDescription ?: @"无法写入所选位置。"];
        return;
    }
}
/// Reads one back, and turns down anything it cannot make sense of with the reason it could not.
- (void)importSettings:(id)sender {
    (void)sender;
    NSOpenPanel *panel = [NSOpenPanel openPanel];
    panel.canChooseFiles = YES;
    panel.canChooseDirectories = NO;
    panel.allowsMultipleSelection = NO;
    panel.prompt = @"导入";
    panel.message = MSIMESettingsDocumentScope;
    if ([panel runModal] != NSModalResponseOK || panel.URL == nil) return;
    NSData *data = MSIMEReadFileUpTo(panel.URL, MSIMESettingsDocumentLimit, nil);
    id document = data == nil ? nil : [NSJSONSerialization JSONObjectWithData:data options:0 error:nil];
    if (![document isKindOfClass:NSDictionary.class] ||
        ![document[MSIMESettingsDocumentFormatField] isEqual:MSIMESettingsDocumentFormat]) {
        [self reportSettingsDocumentFailure:@"无法导入这个文件"
                                     reason:@"它不是水杉输入法导出的设置文件。"];
        return;
    }
    // 另一个版本导出的文件先收窄到本版本，见 MSIMEAdoptCloudAppearance。
    id values = MSIMEAdoptCloudAppearance(document[MSIMESettingsDocumentSettingsField], [self cloudSettingsSnapshot], MSIMEEditionInputSchemes());
    // The same check the account sync puts a downloaded snapshot through — +[MSIMEPreferencesWindowController validateCloudSettingsSnapshot:] is one line around this function — asked here first so that a document this host cannot read is told apart from one it can read and still has to refuse.
    if (![values isKindOfClass:NSDictionary.class] || !MSIMEValidateCloudAppearance(values)) {
        [self reportSettingsDocumentFailure:@"无法导入这个文件"
                                     reason:@"文件里的设置无法识别，可能来自更新版本的水杉输入法，或者已经被改动过。"];
        return;
    }
    if (![self applyCloudSettingsSnapshot:values]) {
        // Validation passed, so the only thing left that -applyCloudSettingsSnapshot: refuses is the one conflict it is written to refuse: the document's paging keys are the key group 以词定字 is currently holding on this machine.
        [self reportSettingsDocumentFailure:@"设置没有导入"
                                     reason:@"文件里的翻页键组正被本机的「以词定字」占用。请先在「按键」页改掉其中一个，再导入。"];
        return;
    }
    [self refreshControls];
    NSAlert *done = [NSAlert new];
    done.alertStyle = NSAlertStyleInformational;
    done.messageText = @"设置已导入";
    done.informativeText = MSIMESettingsDocumentScope;
    [done addButtonWithTitle:@"好"];
    [done runModal];
}
- (void)reportSettingsDocumentFailure:(NSString *)message reason:(NSString *)reason {
    NSAlert *alert = [NSAlert new];
    alert.alertStyle = NSAlertStyleWarning;
    alert.messageText = message;
    alert.informativeText = reason;
    [alert addButtonWithTitle:@"好"];
    [alert runModal];
}

/// Every page at once, from the toolbar's ⋯ menu — the whole-window restore, beside the per-section
/// ones the headings carry.
- (void)restoreAllDefaults:(id)sender {
    (void)sender;
    NSAlert *alert = [NSAlert new];
    alert.alertStyle = NSAlertStyleWarning;
    alert.messageText = @"恢复全部设置？";
    alert.informativeText = @"所有页的设置都会恢复为默认值。词库、学习记录、账号与语音密钥不受影响。";
    // Cancel is added first so it is the default button: the other one throws settings away, and a
    // destructive action should not be what Return picks.
    [alert addButtonWithTitle:@"取消"];
    [alert addButtonWithTitle:@"恢复全部设置"];
    if ([alert runModal] == NSAlertFirstButtonReturn) return;
    [self removeStoredKeys:[self restorableKeys]];
}
/// One section, from the link on its heading. The link is only there when that section has
/// something to restore, so this no longer has to offer the whole window as a consolation for a page
/// with nothing saved on it — an offer that stood one line above the red 卸载… button.
- (void)restoreDefaults:(id)sender {
    NSButton *link = [sender isKindOfClass:NSButton.class] ? (NSButton *)sender : nil;
    if (link == nil || link.tag < 0 || (NSUInteger)link.tag >= _restorableSections.count) return;
    MSIMESettingsSection *section = _restorableSections[(NSUInteger)link.tag];
    NSAlert *alert = [NSAlert new];
    alert.alertStyle = NSAlertStyleWarning;
    alert.messageText = [NSString stringWithFormat:@"恢复「%@」的默认设置？", section.title];
    alert.informativeText = @"这一组设置会恢复为默认值，其它设置不受影响。";
    // Cancel is added first so it is the default button: the other one throws away settings, and a
    // destructive action should not be what Return picks.
    [alert addButtonWithTitle:@"取消"];
    [alert addButtonWithTitle:@"恢复默认值"];
    if ([alert runModal] == NSAlertFirstButtonReturn) return;
    [self removeStoredKeys:section.keys fields:section.fields];
}
- (void)showWindow:(id)sender { [self reloadSkins]; [super showWindow:sender]; }
- (void)pageShortcutChanged:(NSPopUpButton *)sender { self.pageShortcut = sender.indexOfSelectedItem; }
- (void)navigationChanged:(NSButton *)sender { [self setNavigation:sender.identifier enabled:sender.state == NSControlStateValueOn]; }
- (void)wordCharacterChanged:(id)sender {
    (void)sender;
    [self setWordCharacterEnabled:_wordCharacterToggle.state == NSControlStateValueOn keys:_wordCharacterKeys.indexOfSelectedItem == 1 ? @"minus_equal" : @"brackets"];
}
- (void)pageSizeChanged:(NSPopUpButton *)sender {
    self.pageSize = msime::mac::CandidatePageSizeForOptionIndex(MAX(0, sender.indexOfSelectedItem));
}
- (void)fontChanged:(NSPopUpButton *)sender {
    NSInteger index = sender.indexOfSelectedItem;
    self.fontSize = index >= 0 && index <= 20 ? index + 12 : 18;
}
- (void)englishFontFamilyChanged:(NSComboBox *)sender {
    self.candidateEnglishFont = sender.stringValue.length ? sender.stringValue : nil;
}
- (void)fontFamilyChanged:(NSComboBox *)sender { self.fontFamily = sender.stringValue; }
- (void)textColorChanged:(NSTextField *)sender { self.candidateTextColor = sender.stringValue.length ? sender.stringValue : nil; }
- (void)resetTextColor:(id)sender { (void)sender; self.candidateTextColor = nil; }
- (void)textColorWellChanged:(NSColorWell *)sender {
    NSColor *color = [sender.color colorUsingColorSpace:NSColorSpace.sRGBColorSpace];
    if (!color) return;
    auto channel = [](CGFloat value) { return (unsigned int)lround(MAX(0.0, MIN(1.0, value)) * 255); };
    self.candidateTextColor = [NSString stringWithFormat:@"#%02X%02X%02X", channel(color.redComponent), channel(color.greenComponent), channel(color.blueComponent)];
}
/// The stored form of a colour the user picked in a well. Same six digits the shared settings page
/// writes, so a colour set here reads back there as the colour it is.
static NSString *CandidateColorHex(NSColor *color) {
    NSColor *srgb = [color colorUsingColorSpace:NSColorSpace.sRGBColorSpace];
    if (!srgb) return nil;
    auto channel = [](CGFloat value) { return (unsigned int)lround(MAX(0.0, MIN(1.0, value)) * 255); };
    return [NSString stringWithFormat:@"#%02X%02X%02X", channel(srgb.redComponent), channel(srgb.greenComponent),
                                      channel(srgb.blueComponent)];
}
// The six wells and their 跟随皮肤 buttons share one action each; which colour a control belongs to is
// the property name it carries as its identifier, the same name CandidateColorControls() built it from.
- (void)candidateColorWellChanged:(NSColorWell *)sender {
    NSString *hex = CandidateColorHex(sender.color);
    if (hex && sender.identifier) [self setValue:hex forKey:sender.identifier];
}
- (void)resetCandidateColor:(NSButton *)sender {
    if (sender.identifier) [self setValue:nil forKey:sender.identifier];
}
- (void)inputModeHUDChanged:(NSSwitch *)sender { self.inputModeHUD = sender.state == NSControlStateValueOn; }
- (void)themeModeChanged:(NSPopUpButton *)sender { self.themeMode = ThemeModes()[sender.indexOfSelectedItem]; }
- (void)candidateThemeChanged:(NSPopUpButton *)sender { self.candidateTheme = SurfaceThemes()[sender.indexOfSelectedItem]; }
- (void)preeditFontChanged:(NSPopUpButton *)sender { self.preeditFontSize = sender.indexOfSelectedItem + 12; }
- (void)candidateStyleSliderChanged:(NSSlider *)sender {
    const NSInteger value = sender.integerValue;
    NSString *unit = sender == _candidateCornerRadiusSlider ? @" pt" : @"%";
    NSTextField *label = sender == _candidateScaleSlider ? _candidateScaleLabel
                       : sender == _candidateOpacitySlider ? _candidateOpacityLabel : _candidateCornerRadiusLabel;
    label.stringValue = [NSString stringWithFormat:@"%ld%@", (long)value, unit];
    // While the knob is dragged only the figure follows it. The value is written when the knob is let go, or at once from the keyboard, so one drag redraws the candidate window and saves the shared document once rather than at every step it passes.
    if (NSApp.currentEvent.type == NSEventTypeLeftMouseDragged) return;
    if (sender == _candidateScaleSlider) { if (value != self.candidateScalePercent) self.candidateScalePercent = value; }
    else if (sender == _candidateOpacitySlider) { if (value != self.candidateOpacityPercent) self.candidateOpacityPercent = value; }
    else if (sender == _candidateCornerRadiusSlider && ![self.candidateCornerRadius isEqual:@(value)]) self.candidateCornerRadius = @(value);
}
- (void)resetCandidateCornerRadius:(id)sender { (void)sender; self.candidateCornerRadius = nil; }
- (void)candidateFontPresetChanged:(NSPopUpButton *)sender { self.candidateFontPreset = sender.indexOfSelectedItem; }
- (void)candidatePreeditChanged:(NSPopUpButton *)sender { self.showsCandidatePreedit = sender.indexOfSelectedItem == 0; }

#pragma mark - Application exception table
// This object is the sidebar's data source as well, and an NSOutlineView is an NSTableView, so each of these answers for the one table it was written for and says so rather than assuming it can only have been called by that table.
- (NSInteger)numberOfRowsInTableView:(NSTableView *)tableView {
    if (tableView == _appRuleTable) return (NSInteger)_appRuleIdentifiers.count;
    return 0;
}
- (NSView *)tableView:(NSTableView *)tableView viewForTableColumn:(NSTableColumn *)tableColumn row:(NSInteger)row {
    if (tableView == _appRuleTable) return [self applicationRuleCellForColumn:tableColumn row:row];
    return nil;
}
/// One row of 应用例外: the application on the leading edge, the mode it is to start in trailing it.
///
/// The cells are built fresh rather than reused. There are as many rows as the user has written rules, which is a handful, and a reused popup would have to have its row number rewritten anyway.
- (NSView *)applicationRuleCellForColumn:(NSTableColumn *)column row:(NSInteger)row {
    if (row < 0 || (NSUInteger)row >= _appRuleIdentifiers.count) return nil;
    NSString *identifier = _appRuleIdentifiers[(NSUInteger)row];
    if ([column.identifier isEqual:MSIMEAppRuleModeColumn]) {
        NSPopUpButton *mode = [[NSPopUpButton alloc] initWithFrame:NSZeroRect pullsDown:NO];
        [mode addItemsWithTitles:@[ @"中文", @"英文" ]];
        [mode selectItemAtIndex:[[self applicationInputModeRules][identifier] isEqual:@"english"] ? 1 : 0];
        mode.controlSize = NSControlSizeSmall;
        mode.font = [NSFont systemFontOfSize:[NSFont systemFontSizeForControlSize:NSControlSizeSmall]];
        mode.accessibilityLabel = [NSString stringWithFormat:@"%@的输入模式", [self applicationNameForBundleIdentifier:identifier]];
        mode.tag = row;
        mode.target = self;
        mode.action = @selector(applicationInputModeRuleChanged:);
        return mode;
    }
    NSTableCellView *cell = [[NSTableCellView alloc] initWithFrame:NSZeroRect];
    NSTextField *label = [NSTextField labelWithString:[self applicationNameForBundleIdentifier:identifier]];
    label.font = [NSFont systemFontOfSize:msime::mac::layout::kBodyFontSize weight:NSFontWeightRegular];
    label.lineBreakMode = NSLineBreakByTruncatingTail;
    // The bundle identifier, for the two applications whose names read the same and for the rule left behind by one that has since been deleted, whose name is the identifier anyway.
    label.toolTip = identifier;
    label.translatesAutoresizingMaskIntoConstraints = NO;
    [cell addSubview:label];
    cell.textField = label;
    [NSLayoutConstraint activateConstraints:@[
        [label.leadingAnchor constraintEqualToAnchor:cell.leadingAnchor constant:4.0],
        [label.trailingAnchor constraintLessThanOrEqualToAnchor:cell.trailingAnchor],
        [label.centerYAnchor constraintEqualToAnchor:cell.centerYAnchor],
    ]];
    return cell;
}
/// What the user calls the application, falling back on the bundle identifier for one that is not installed on this machine — which is what a rule carried over from another Mac, or left behind by an application since deleted, looks like.
///
/// Remembered for as long as the window is open. The answer comes from LaunchServices, and -refreshControls asks for it once per rule to draw the list and O(n log n) times to sort it — on every switch flipped anywhere in the window.
- (NSString *)applicationNameForBundleIdentifier:(NSString *)identifier {
    NSString *remembered = _appRuleNames[identifier];
    if (remembered != nil) return remembered;
    NSURL *url = [NSWorkspace.sharedWorkspace URLForApplicationWithBundleIdentifier:identifier];
    NSString *name = url == nil ? nil : [NSFileManager.defaultManager displayNameAtPath:url.path];
    NSString *resolved = name.length > 0 ? name : identifier;
    _appRuleNames[identifier] = resolved;
    return resolved;
}
- (void)tableViewSelectionDidChange:(NSNotification *)notification {
    if (notification.object != _appRuleTable) return;
    _appRuleRemoveButton.enabled = _appRuleTable.selectedRow >= 0;
}
- (void)applicationInputModeRuleChanged:(NSPopUpButton *)sender {
    if (sender.tag < 0 || (NSUInteger)sender.tag >= _appRuleIdentifiers.count) return;
    [self setInputMode:sender.indexOfSelectedItem == 1 ? @"english" : @"chinese"
        forApplication:_appRuleIdentifiers[(NSUInteger)sender.tag]];
}
/// Picks the application a rule is about. An open panel rather than a list of what happens to be running: a rule is most often written for the application the user has just been annoyed by, and that one may well have been quit before they got here.
- (void)addApplicationInputModeRule:(id)sender {
    (void)sender;
    NSOpenPanel *panel = [NSOpenPanel openPanel];
    panel.canChooseFiles = YES;
    panel.canChooseDirectories = NO;
    panel.allowsMultipleSelection = NO;
    // An application is a package, and left as a directory the panel would let the user walk into one and pick something inside it.
    panel.treatsFilePackagesAsDirectories = NO;
    panel.directoryURL = [NSURL fileURLWithPath:@"/Applications" isDirectory:YES];
    panel.prompt = @"添加";
    panel.message = @"选择一个应用程序，为它指定启动时的输入模式。";
    if ([panel runModal] != NSModalResponseOK || panel.URL == nil) return;
    NSString *identifier = [NSBundle bundleWithURL:panel.URL].bundleIdentifier;
    if (identifier.length == 0) {
        // Said where the press went rather than beeped: the panel will happily hand back a document, a script, or a folder that merely ends in .app, and none of those has an identifier to write a rule against.
        _appRuleStatusLabel.stringValue = @"所选项目不是应用程序，没有可用的 Bundle ID。";
        _appRuleStatusLabel.textColor = NSColor.systemRedColor;
        return;
    }
    // A rule for an application that already has one is that application's rule, so the selection moves to it rather than a second row appearing.
    if ([self applicationInputModeRules][identifier] == nil) [self setInputMode:@"chinese" forApplication:identifier];
    [self refreshControls];
    const NSUInteger index = [_appRuleIdentifiers indexOfObject:identifier];
    if (index != NSNotFound) {
        [_appRuleTable selectRowIndexes:[NSIndexSet indexSetWithIndex:index] byExtendingSelection:NO];
        [_appRuleTable scrollRowToVisible:(NSInteger)index];
    }
}
- (void)removeApplicationInputModeRule:(id)sender {
    (void)sender;
    const NSInteger row = _appRuleTable.selectedRow;
    if (row < 0 || (NSUInteger)row >= _appRuleIdentifiers.count) return;
    [self setInputMode:nil forApplication:_appRuleIdentifiers[(NSUInteger)row]];
    [self refreshControls];
    // The row under the one that was removed, so that removing several in a row does not need the pointer to go back to the list between each.
    const NSInteger next = MIN(row, (NSInteger)_appRuleIdentifiers.count - 1);
    if (next >= 0) [_appRuleTable selectRowIndexes:[NSIndexSet indexSetWithIndex:(NSUInteger)next] byExtendingSelection:NO];
}
@end
