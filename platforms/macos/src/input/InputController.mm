#import <AppKit/AppKit.h>
#import <InputMethodKit/InputMethodKit.h>
#import <CoreText/CoreText.h>
#import "MSIMEClientSession.h"
#import "../settings/RuntimeOptions.h"
#import "../../../../shared/apple/TextClient.h"
#include "msime_client.h"
#import "../candidate/CandidatePlacement.h"
#import "../candidate/CandidateGlossSenses.h"
#import "InputSourceRegistration.h"
#import "../core/UpdateController.h"
#include "../../../common/DictionaryQuiesceLease.h"
#import "../core/ScreenKeyboardPanel.h"
#import "../dictionary/DictionaryWindowController.h"
#import "../core/ClientDictionaryRuntime.h"
#import "../settings/AppearancePreferences.h"
#import "InputMenu.h"
#import "../settings/PreferencesWindowController.h"
#import "../core/DesktopSettingsLauncher.h"
#import "../core/DesktopInputSession.h"
#import "../core/DesktopCloudClipboard.h"
#import "../core/SharedVoicePreferences.h"
#import "../voice/VoiceProviderOptions.h"
#import "../voice/VoiceTextCommit.h"
#include "SmartPunctuationRewrite.h"
#import "../voice/VoiceDeactivation.h"
#import "../voice/HTTPVoiceRequest.h"
#import "../voice/VoiceHoldShortcut.h"
#import "../voice/DoubaoVoiceRequest.h"
#import "../voice/LocalVoiceRequest.h"
#import "../voice/VoiceFailureMessages.h"
#import "../core/SupportWindowController.h"
#import "../backend/account/BackendAccountEntry.h"
#import "../backend/core/BackendSelectionObservation.h"
#include "../core/ToolTextReturn.h"
#include "../core/ToolApplicationActivation.h"
#include "../settings/PreferenceSaveState.h"
#include "../settings/PreferenceLoadState.h"
#include "../settings/PreferenceSnapshotMerge.h"
#import "../candidate/CandidateChrome.h"
#import "../core/WindowPresentation.h"
#import "../candidate/CandidateTypography.h"
#import "../candidate/CandidateTextMetrics.h"
#include "../candidate/CandidateSkin.h"
#include "../candidate/CandidateWheelRouting.h"
#import "../core/ChineseTextConversion.h"
#include "../core/FullWidthInput.h"
#include "InputControllerPhysicalKeys.h"
#include "../core/ModifierTap.h"
#import "../settings/ShuangpinKeymapPanel.h"
#import "../core/FloatingToolbarPanel.h"
#import "InputModeHUDPanel.h"
#import "InputModeIdentifiers.h"
#import "../voice/VoiceInputService.h"
#import "../voice/VoiceProviderSocket.h"
#import "../voice/VoiceWaveOverlay.h"
#import "../voice/VoiceInputLevel.h"
#include "../../../../shared/voice/CaptureDuration.h"
#include "../../../../shared/voice/VoiceProviders.h"
#import "../voice/VoiceCuePlayer.h"
#import "../voice/VoiceAudioMuter.h"
#import "../voice/VoiceProviderSettings.h"
#import "../voice/VoiceSettings.h"
#import "../cloud/CloudCandidateRequest.h"
#import "../core/CustomTranslationBatch.h"
#import "../cloud/TranslationCache.h"
#include "../core/WubiCommitPolicy.h"
#include "../core/WubiCodeHintPolicy.h"
#include "../core/PairedPunctuation.h"
#include "../core/PairedPunctuation.h"
#include "../core/TypingStatistics.h"
#include "../core/DiagnosticLog.h"
#include "../../../../shared/input/EnglishModeOutput.h"
#include <atomic>

// Implemented by the Swift backend dylib loaded by input_method_main.mm. The account provider
// keeps credentials and transport on the Swift side; this process receives only bounded glosses.
extern "C" void MSIMEFetchAccountCandidateGlosses(const char *wordsJSON, const char *primaryCode,
                                                    const char *secondaryCode, unsigned long long generation)
    __attribute__((weak_import));
extern "C" void MSIMEEnsureAnonymousAccount(void) __attribute__((weak_import));
// Apple's on-device translation, also in the Swift backend. Null when the backend was built by a toolchain older than the macOS 26 SDK.
extern "C" void MSIMEFetchOnDeviceCandidateGlosses(const char *wordsJSON, const char *targetsJSON) __attribute__((weak_import));

static dispatch_queue_t MSIMETypingStatisticsQueue(void) {
    static dispatch_queue_t queue;
    static dispatch_once_t once;
    dispatch_once(&once, ^{ queue = dispatch_queue_create("app.msime.client.typing-statistics", DISPATCH_QUEUE_SERIAL); });
    return queue;
}

static NSString * const MSIMETypingStatisticsEnabledChangedNotification =
    @"MetasequoiaTypingStatisticsEnabledChangedNotification";
// Posted by the settings window (crates/host-macos/native/dictionary.mm) and by the native dictionary window once the quiesce lease is up. It only wakes the controllers; the lease beside the dictionary lock is what they check before letting go.
static NSString * const MSIMEDictionaryMaintenanceWillBeginNotification =
    @"MSIMEDictionaryMaintenanceWillBeginNotification";

// Dictionary maintenance needs the Engine's exclusive lock, and every open session holds it shared. While the settings window's lease (platforms/common/DictionaryQuiesceLease.h) is live on a session's user directory, that session is closed and none is opened on it.
static BOOL MSIMEDictionaryQuiesced(NSDictionary *options) {
    id userData = [options isKindOfClass:NSDictionary.class] ? options[@"user_data"] : nil;
    if (![userData isKindOfClass:NSString.class] || ![userData isAbsolutePath]) return NO;
    return msime::dictionary_lease::dictionary_quiesced(std::string([userData fileSystemRepresentation]));
}
// Privacy-preserving default: until the persisted opt-in is loaded, the capture boundary is shut.
static std::atomic_bool MSIMETypingStatisticsEnabled{false};

static void MSIMEReloadTypingStatisticsEnabled(NSString *directory) {
    MSIMETypingStatisticsEnabled.store(false, std::memory_order_relaxed);
    if (![directory isKindOfClass:NSString.class] || !directory.isAbsolutePath) return;
    NSData *bytes = [directory dataUsingEncoding:NSUTF8StringEncoding];
    if (!bytes || bytes.length > 16384) return;
    const int32_t enabled = msime_client_typing_statistics_enabled(
        static_cast<const uint8_t *>(bytes.bytes), bytes.length);
    if (enabled >= 0) MSIMETypingStatisticsEnabled.store(enabled == 1, std::memory_order_relaxed);
    else msime_macos_diagnostic_write("stats: enabled_read_failed");
}

static void MSIMERecordTypingStatistics(NSString *directory, NSString *text, msime::mac::TypingSource source) {
    // Match the Windows capture contract: an opt-out does not inspect or classify committed text,
    // allocate a request, enter the worker queue, or touch the statistics store.
    if (!MSIMETypingStatisticsEnabled.load(std::memory_order_relaxed)) return;
    if (![directory isKindOfClass:NSString.class] || !directory.isAbsolutePath ||
        ![text isKindOfClass:NSString.class] || text.length == 0) return;
    NSDateComponents *components = [NSCalendar.currentCalendar components:NSCalendarUnitYear | NSCalendarUnitMonth |
        NSCalendarUnitDay | NSCalendarUnitHour fromDate:NSDate.date];
    NSString *day = [NSString stringWithFormat:@"%04ld-%02ld-%02ld", (long)components.year,
        (long)components.month, (long)components.day];
    const std::string_view sourceID = msime::mac::TypingSourceId(source);
    NSString *sourceString = [[NSString alloc] initWithBytes:sourceID.data() length:sourceID.size()
                                                     encoding:NSUTF8StringEncoding];
    if (!sourceString) {
        msime_macos_diagnostic_write("stats: invalid_source");
        return;
    }
    NSDictionary *request = @{ @"directory": directory, @"action": @{
        @"operation": @"record", @"text": text,
        @"source": sourceString,
        @"day": day,
        // The hour axis has to come from the same calendar as the day beside it.
        @"hour": @((long)components.hour) } };
    NSData *data = [NSJSONSerialization dataWithJSONObject:request options:0 error:nil];
    if (!data || data.length > 65536) {
        msime_macos_diagnostic_write(data ? "stats: request_too_large" : "stats: request_encode_failed");
        return;
    }
    dispatch_async(MSIMETypingStatisticsQueue(), ^{
        char *response = msime_client_typing_statistics(static_cast<const uint8_t *>(data.bytes), data.length);
        // Statistics are best effort and must never affect text commitment, so the response carries no UI state. A failure is logged as a label only, like the source's stats open/persist/retention lines: the error string can name files, and the log never carries it.
        if (!response) {
            msime_macos_diagnostic_write("stats: record_failed reason=no_response");
            return;
        }
        if (msime_macos_diagnostic_enabled()) {
            NSData *body = [NSData dataWithBytesNoCopy:response length:strlen(response) freeWhenDone:NO];
            NSDictionary *envelope = [NSJSONSerialization JSONObjectWithData:body options:0 error:nil];
            if (![envelope isKindOfClass:NSDictionary.class])
                msime_macos_diagnostic_write("stats: record_failed reason=malformed_response");
            else if (![envelope[@"ok"] isEqual:@YES])
                msime_macos_diagnostic_write("stats: record_failed reason=store");
        }
        msime_client_string_free(response);
    });
}

static NSString *MSIMEAICacheKey(NSDictionary *online) {
    NSDictionary *config = online[@"ai_assistant"];
    NSArray *segments = online[@"pinyin_segments"];
    if (![config isKindOfClass:NSDictionary.class] || ![config[@"enabled"] boolValue] ||
        ![segments isKindOfClass:NSArray.class] || !segments.count ||
        ![NSJSONSerialization isValidJSONObject:segments]) return nil;
    NSDictionary *identity = @{ @"provider": [config[@"provider"] isKindOfClass:NSString.class] ? config[@"provider"] : @"",
        @"endpoint": [config[@"endpoint"] isKindOfClass:NSString.class] ? config[@"endpoint"] : @"",
        @"model": [config[@"model"] isKindOfClass:NSString.class] ? config[@"model"] : @"",
        @"pinyin_segments": segments };
    NSData *data = [NSJSONSerialization dataWithJSONObject:identity options:0 error:nil];
    return data ? [[NSString alloc] initWithData:data encoding:NSUTF8StringEncoding] : nil;
}

static BOOL MSIMEViewContainsAICandidate(NSDictionary *view) {
    for (NSDictionary *candidate in view[@"candidates"])
        if ([candidate isKindOfClass:NSDictionary.class] && [candidate[@"source"] integerValue] == 1) return YES;
    return NO;
}

static msime::mac::TypingSource MSIMEResolveTypingSource(NSDictionary *context, NSDictionary *view,
                                                          NSDictionary *hostOptions, BOOL englishMode) {
    NSDictionary *effectiveContext = [context isKindOfClass:NSDictionary.class] ? context : view;
    NSNumber *scheme = effectiveContext[@"scheme"];
    if (![scheme isKindOfClass:NSNumber.class] || CFGetTypeID((__bridge CFTypeRef)scheme) == CFBooleanGetTypeID() ||
        CFNumberIsFloatType((__bridge CFNumberRef)scheme)) return msime::mac::TypingSource::Unknown;
    NSString *localMode = effectiveContext[@"local_mode"];
    if (![localMode isKindOfClass:NSString.class]) localMode = @"none";
    NSNumber *nineKey = view[@"nine_key"];
    BOOL dedicatedEnglish = [view[@"dedicated_english"] boolValue] || englishMode;
    NSDictionary *preferences = [hostOptions[@"preferences"] isKindOfClass:NSDictionary.class] ? hostOptions[@"preferences"] : @{};
    NSString *profile = view[@"shuangpin_profile"];
    if (![profile isKindOfClass:NSString.class]) profile = preferences[@"shuangpin_profile"];
    if (![profile isKindOfClass:NSString.class]) profile = @"xiaohe";
    return msime::mac::ResolveTypingSource(scheme.intValue, [nineKey boolValue], dedicatedEnglish,
        localMode.UTF8String ?: "none", profile.UTF8String ?: "xiaohe");
}

static NSDictionary *MSIMEStatisticsHostOptions(MSIMEClientSession *session) {
    return [session respondsToSelector:@selector(hostOptions)] ? session.hostOptions : @{};
}

static BOOL MSIMEScriptConversionApplies(id value) {
    if (![value isKindOfClass:NSDictionary.class] || ![value[@"scheme"] isKindOfClass:NSNumber.class]) return NO;
    if ([value[@"scheme"] integerValue] < 0 || [value[@"scheme"] integerValue] > 2) return NO;
    NSString *mode = value[@"local_mode"];
    // Temporary Japanese retains the original Chinese scheme in the host snapshot.
    return ![mode isKindOfClass:NSString.class] ||
        (![mode isEqualToString:@"unicode"] && ![mode isEqualToString:@"temporary_japanese"]);
}

static NSString *CandidateDisplay(NSDictionary *candidate, BOOL traditional) {
    NSString *annotation = candidate[@"annotation"];
    NSString *text = candidate[@"text"];
    id corrected = candidate[@"corrected"];
    if ([corrected isKindOfClass:NSNumber.class] && CFGetTypeID((__bridge CFTypeRef)corrected) == CFBooleanGetTypeID() &&
        [corrected boolValue]) text = [text stringByAppendingString:@"*"];
    if ([annotation isKindOfClass:NSString.class]) text = [text stringByAppendingString:annotation];
    text = MSIMEChineseOutputString(text, traditional);
    id source = candidate[@"source"];
    // Engine CandidateSource: CloudSuggestion=2, AiSuggestion=3. These badges
    // are presentation-only, matching the Windows candidate-view suffixes.
    if ([source isKindOfClass:NSNumber.class] && CFGetTypeID((__bridge CFTypeRef)source) != CFBooleanGetTypeID() &&
        !CFNumberIsFloatType((__bridge CFNumberRef)source)) {
        if ([source isEqual:@2]) return [text stringByAppendingString:@" ☁️"];
        if ([source isEqual:@3]) return [text stringByAppendingString:@" 🤖"];
    }
    return text;
}

static NSString *MSIMEWubiCodeHint(NSDictionary *candidate, NSDictionary *view, BOOL enabled) {
    if (![candidate isKindOfClass:NSDictionary.class] || ![view isKindOfClass:NSDictionary.class]) return @"";
    NSString *code = candidate[@"code"];
    NSString *typed = [view[@"preedit"] isKindOfClass:NSString.class] ? view[@"preedit"] : view[@"editing_text"];
    NSNumber *scheme = view[@"scheme"];
    NSString *localMode = [view[@"local_mode"] isKindOfClass:NSString.class] ? view[@"local_mode"] : @"none";
    if (![code isKindOfClass:NSString.class] || ![typed isKindOfClass:NSString.class] ||
        ![scheme isKindOfClass:NSNumber.class]) return @"";
    const std::string codeUTF8 = code.UTF8String ? code.UTF8String : "";
    const std::string typedUTF8 = typed.UTF8String ? typed.UTF8String : "";
    const std::string hint = msime::mac::WubiCodeHint(codeUTF8, typedUTF8, enabled, scheme.intValue,
                                                       localMode.UTF8String ?: "none",
                                                       [view[@"answered_by_pinyin_fallback"] boolValue]);
    return hint.empty() ? @"" : [[NSString alloc] initWithBytes:hint.data() length:hint.size() encoding:NSUTF8StringEncoding];
}

static NSString *CandidateDisplayWithWubiHint(NSDictionary *candidate, BOOL traditional, NSString *hint) {
    if (![hint isKindOfClass:NSString.class] || hint.length == 0) return CandidateDisplay(candidate, traditional);
    NSMutableDictionary *annotated = [candidate mutableCopy];
    annotated[@"annotation"] = [NSString stringWithFormat:@"(%@)", hint];
    return CandidateDisplay(annotated, traditional);
}

// The panel draws the 辅助码 (or the engine's annotation) as a run of its own after the candidate text, so it can move under a text that leaves it no room, as the Windows presenter does. The tooltip and accessibility label keep the combined CandidateDisplayWithWubiHint form.
static NSString *CandidateTextRun(NSDictionary *candidate, BOOL traditional) {
    if (candidate[@"annotation"] == nil) return CandidateDisplay(candidate, traditional);
    NSMutableDictionary *plain = [candidate mutableCopy];
    [plain removeObjectForKey:@"annotation"];
    return CandidateDisplay(plain, traditional);
}

static NSString *CandidateAnnotationRun(NSDictionary *candidate, BOOL traditional, NSString *hint) {
    NSString *annotation = [hint isKindOfClass:NSString.class] && hint.length ? [NSString stringWithFormat:@"(%@)", hint]
                                                                              : candidate[@"annotation"];
    if (![annotation isKindOfClass:NSString.class] || annotation.length == 0) return @"";
    return MSIMEChineseOutputString(annotation, traditional);
}

// One page of candidate rows laid out at the card width they get: what the panel draws, what its buttons answer clicks in, and what the keymap panel keeps clear of.
struct MSIMECandidatePageGeometry {
    std::vector<msime::mac::CandidateRowLayout> rows;
    NSArray<NSString *> *texts = @[];
    NSArray<NSString *> *annotations = @[];
    NSArray<NSString *> *displays = @[];
    // Card width, the width the rows share inside it, the height they stack to, and the x of the candidate text inside a row.
    CGFloat width = 0;
    CGFloat lineWidth = 0;
    CGFloat rowsHeight = 0;
    CGFloat contentLeft = 0;
};

static NSString *CandidateTranslation(NSDictionary *candidate) {
    id text = candidate[@"translation"];
    return [text isKindOfClass:NSString.class] ? text : @"";
}

// Candidate pinning is a macOS presentation preference. The Engine's ranking is
// shared by every host, while a user who always wants one word first expects that
// choice to stay local to this input method. Keep an ordered list per code so
// multiple pinned words retain the order in which they were pinned.
static NSString *const MSIMEPinnedCandidatesPreferenceKey = @"MSIMEClientPinnedCandidates";

static NSString *MSIMECandidatePinCode(NSDictionary *view) {
    NSString *code = [view[@"preedit"] isKindOfClass:NSString.class] ? view[@"preedit"] : nil;
    if (code.length == 0 && [view[@"editing_text"] isKindOfClass:NSString.class]) code = view[@"editing_text"];
    return code.length > 0 ? code : @"";
}

static NSArray<NSString *> *MSIMEPinnedWords(NSString *code) {
    if (code.length == 0) return @[];
    NSDictionary *all = [NSUserDefaults.standardUserDefaults dictionaryForKey:MSIMEPinnedCandidatesPreferenceKey];
    NSArray *words = [all[code] isKindOfClass:NSArray.class] ? all[code] : @[];
    NSMutableArray<NSString *> *valid = [NSMutableArray arrayWithCapacity:words.count];
    for (id word in words) {
        if (![word isKindOfClass:NSString.class]) continue;
        NSString *string = (NSString *)word;
        if (string.length > 0 && ![valid containsObject:string]) [valid addObject:string];
    }
    return valid;
}

static BOOL MSIMECandidateIsPinned(NSString *code, NSString *word) {
    return code.length > 0 && word.length > 0 && [MSIMEPinnedWords(code) containsObject:word];
}

static void MSIMETogglePinnedCandidate(NSString *code, NSString *word) {
    if (code.length == 0 || word.length == 0) return;
    NSDictionary *stored = [NSUserDefaults.standardUserDefaults dictionaryForKey:MSIMEPinnedCandidatesPreferenceKey];
    NSMutableDictionary *all = [stored isKindOfClass:NSDictionary.class] ? [stored mutableCopy] : [NSMutableDictionary dictionary];
    NSMutableArray<NSString *> *words = [MSIMEPinnedWords(code) mutableCopy] ?: [NSMutableArray array];
    NSUInteger existing = [words indexOfObject:word];
    if (existing != NSNotFound) [words removeObjectAtIndex:existing];
    else [words insertObject:word atIndex:0];
    if (words.count > 0) all[code] = words;
    else [all removeObjectForKey:code];
    [NSUserDefaults.standardUserDefaults setObject:all forKey:MSIMEPinnedCandidatesPreferenceKey];
}

static NSArray<NSDictionary *> *MSIMEReorderedPinnedCandidates(NSArray *candidates, NSString *code) {
    if (![candidates isKindOfClass:NSArray.class] || candidates.count == 0 || code.length == 0) return candidates ?: @[];
    NSArray<NSString *> *pinned = MSIMEPinnedWords(code);
    if (pinned.count == 0) return candidates;
    NSMutableArray<NSDictionary *> *remaining = [candidates mutableCopy];
    NSMutableArray<NSDictionary *> *ordered = [NSMutableArray arrayWithCapacity:candidates.count];
    for (NSString *word in pinned) {
        for (NSInteger index = (NSInteger)remaining.count - 1; index >= 0; --index) {
            NSDictionary *candidate = remaining[(NSUInteger)index];
            if ([candidate isKindOfClass:NSDictionary.class] && [candidate[@"text"] isEqual:word]) {
                [ordered addObject:candidate];
                [remaining removeObjectAtIndex:(NSUInteger)index];
                break;
            }
        }
    }
    [ordered addObjectsFromArray:remaining];
    return ordered;
}

static NSString *MSIMECandidateTranslationColumn(NSDictionary *candidate, NSInteger column) {
    if (column <= 0) return @"";
    NSArray<NSString *> *parts = [CandidateTranslation(candidate) componentsSeparatedByString:@"\n"];
    NSUInteger index = (NSUInteger)(column - 1);
    return index < parts.count && [parts[index] isKindOfClass:NSString.class] ? parts[index] : @"";
}

static NSSize MSIMETranslationTextSize(NSString *text, NSFont *font) {
    if (!text.length) return NSZeroSize;
    NSRect bounds = [text boundingRectWithSize:NSMakeSize(CGFLOAT_MAX, CGFLOAT_MAX)
        options:NSStringDrawingUsesLineFragmentOrigin
        attributes:@{NSFontAttributeName:font}];
    return NSMakeSize(ceil(bounds.size.width), ceil(bounds.size.height));
}

static NSArray<NSString *> *MSIMETranslationTargets(NSDictionary *query) {
    NSArray *supported = @[@"en", @"fr", @"ja", @"es", @"ru", @"de", @"ko"];
    NSMutableArray<NSString *> *targets = [NSMutableArray arrayWithCapacity:2];
    NSArray *raw = [query[@"target_languages"] isKindOfClass:NSArray.class] ? query[@"target_languages"] : @[];
    for (id value in raw) {
        if (![value isKindOfClass:NSString.class] || ![(NSString *)value length] || ![supported containsObject:value] || [targets containsObject:value]) continue;
        [targets addObject:(NSString *)value];
    }
    NSString *primary = [query[@"target_language"] isKindOfClass:NSString.class] ? query[@"target_language"] : nil;
    if ([supported containsObject:primary]) {
        [targets removeObject:primary];
        [targets insertObject:primary atIndex:0];
    }
    return targets.count ? [targets copy] : @[];
}

// Account glosses live in the process-wide translation cache, not on the controller. IMKit builds one
// controller per text input client - a dozen or more over a session - so the instance that fetched a gloss
// is usually not the one composing next time. Kept per instance, the answers scatter into controllers that
// are no longer composing and every new text field starts from nothing, which is what "it shows up the
// second time but not the first" actually is: the second time happened to land on the same instance.
//
// The key is language and word, so sharing is safe: a gloss any instance fetched is correct for all of
// them. Three elements and a leading scope of its own, so it cannot collide with the five-element
// identities the user's own translator uses.
static NSArray<NSString *> *MSIMEAccountGlossIdentity(NSString *target, NSString *text) {
    return @[@"account", target ?: @"", text ?: @""];
}

static NSString *MSIMEAccountGlossCached(NSString *target, NSString *text) {
    id value = [[MSIMETranslationCache sharedCache] valueForIdentity:MSIMEAccountGlossIdentity(target, text)];
    return [value isKindOfClass:NSString.class] ? value : nil;
}

// On-device glosses share the process-wide cache for the same reason as account ones, under a scope of their own. A word the model had nothing useful for is cached as an empty answer, so it is not asked about again on the next keystroke.
static NSArray<NSString *> *MSIMEOnDeviceGlossIdentity(NSString *target, NSString *text) {
    return @[@"on-device", target ?: @"", text ?: @""];
}

// Which candidates the account gloss endpoint may be asked about. Only Chinese ones: a model has nothing
// to say about "cun", "123", "OpenAI", a punctuation candidate or an emoji, and asking spends the account's
// bounded quota to put noise under candidates that should carry no gloss - a pinyin buffer candidate also
// hands the user's raw keystrokes to a remote service. The shared query answers this per candidate so every
// host applies the same rule.
//
// This is the gloss path only. The user's own translator (custom, Tencent TMT, NiuTrans) keeps seeing
// English candidates, because there the direction is detected per candidate and English to Chinese is a
// translation someone asked for. The offline dictionary is not filtered either - it answers for English and
// never leaves the machine.
static NSArray<NSDictionary *> *MSIMEOnlineGlossCandidates(NSDictionary *query) {
    NSArray *raw = [query[@"candidates"] isKindOfClass:NSArray.class] ? query[@"candidates"] : @[];
    NSMutableArray<NSDictionary *> *candidates = [NSMutableArray arrayWithCapacity:raw.count];
    for (NSDictionary *candidate in raw) {
        if (![candidate isKindOfClass:NSDictionary.class] ||
            ![candidate[@"text"] isKindOfClass:NSString.class] ||
            ![candidate[@"online_gloss"] isEqual:@YES]) continue;
        [candidates addObject:candidate];
    }
    return [candidates copy];
}

static NSArray<NSString *> *MSIMETranslationTargetsFromPreferences(NSDictionary *preferences, NSString *fallback) {
    NSArray *supported = @[@"en", @"fr", @"ja", @"es", @"ru", @"de", @"ko"];
    NSString *primary = [preferences[@"translation_target_language"] isKindOfClass:NSString.class]
        ? preferences[@"translation_target_language"] : fallback;
    NSMutableArray<NSString *> *targets = [NSMutableArray array];
    if ([supported containsObject:primary]) [targets addObject:primary];
    id secondary = preferences[@"translation_secondary_language"];
    if ([supported containsObject:secondary] && ![targets containsObject:secondary])
        [targets addObject:secondary];
    return [targets copy];
}

static NSString *MSIMETranslationWorkKey(NSString *target, NSString *text) {
    return [NSString stringWithFormat:@"%@\u001f%@", target ?: @"", text ?: @""];
}

static NSString *MSIMEJoinedTranslations(NSDictionary<NSString *, NSString *> *values,
                                          NSArray<NSString *> *targets) {
    NSMutableArray<NSString *> *ordered = [NSMutableArray array];
    for (NSString *target in targets) [ordered addObject:values[target] ?: @""];
    while (ordered.count && ![ordered.lastObject length]) [ordered removeLastObject];
    BOOL hasValue = NO;
    for (NSString *value in ordered) if (value.length) { hasValue = YES; break; }
    return hasValue ? [ordered componentsJoinedByString:@"\n"] : @"";
}

static NSColor *SkinColor(msime::mac::Rgba color) {
    return [NSColor colorWithSRGBRed:color.r green:color.g blue:color.b alpha:color.a];
}

static BOOL MSIMEUnsignedCandidateIdentityValue(id value) {
    return [value isKindOfClass:NSNumber.class] && CFGetTypeID((__bridge CFTypeRef)value) != CFBooleanGetTypeID() &&
           !CFNumberIsFloatType((__bridge CFNumberRef)value) && [value compare:@0] != NSOrderedAscending;
}
static NSUInteger MSIMECandidateDeletionSlot(NSEvent *event) {
    const NSEventModifierFlags required = NSEventModifierFlagControl | NSEventModifierFlagOption | NSEventModifierFlagShift;
    if ((event.modifierFlags & (required | NSEventModifierFlagCommand)) != required) return NSNotFound;
    // Main-row physical digits only; Option/Shift characters vary by layout.
    const unsigned short codes[] = {18, 19, 20, 21, 23, 22, 26, 28};
    for (NSUInteger slot = 0; slot < 8; ++slot) if (event.keyCode == codes[slot]) return slot;
    return NSNotFound;
}
static BOOL MSIMEPunctuationToggle(NSEvent *event) {
    const NSEventModifierFlags modifiers = NSEventModifierFlagControl | NSEventModifierFlagShift | NSEventModifierFlagOption | NSEventModifierFlagCommand;
    return event.keyCode == 47 && (event.modifierFlags & modifiers) == NSEventModifierFlagControl;
}
// The marks this host closes for the user, opening first. The reference keeps the same list in its
// TIP (`GetPairedPunctuationClosing`).
static NSArray<NSArray<NSString *> *> *MSIMEPunctuationPairs(void) {
    static NSArray<NSArray<NSString *> *> *pairs;
    static dispatch_once_t once;
    dispatch_once(&once, ^{
        pairs = @[@[@"（", @"）"], @[@"【", @"】"], @[@"《", @"》"], @[@"“", @"”"], @[@"‘", @"’"],
                  @[@"〈", @"〉"], @[@"「", @"」"]];
    });
    return pairs;
}

static BOOL MSIMEPairedPunctuationExcludedBundleIdentifier(NSString *identifier) {
    if (![identifier isKindOfClass:NSString.class]) return NO;
    return [identifier caseInsensitiveCompare:@"com.microsoft.Excel"] == NSOrderedSame;
}
static BOOL MSIMEPairedPunctuationExcludedHost(void) {
    NSRunningApplication *app = NSWorkspace.sharedWorkspace.frontmostApplication;
    return MSIMEPairedPunctuationExcludedBundleIdentifier(app.bundleIdentifier);
}
static BOOL MSIMECurrentCandidateIdentity(id identifier, NSDictionary *view) {
    if (![identifier isKindOfClass:NSDictionary.class] || ![view[@"focused"] isEqual:@YES]) return NO;
    for (NSString *key in @[@"session", @"generation", @"index"])
        if (!MSIMEUnsignedCandidateIdentityValue(identifier[key])) return NO;
    return [identifier[@"session"] isEqual:view[@"session"]] && [identifier[@"generation"] isEqual:view[@"generation"]] &&
           [identifier[@"index"] compare:@(NSUIntegerMax)] != NSOrderedDescending;
}

// Background readers borrow the controller strongly. Its last release must not
// land on their queue, where -dealloc would tear down AppKit objects off main.
// Takes the caller's reference and clears it before main can drop the handoff.
static void MSIMEReleaseControllerOnMain(__strong id *controller) {
    if (!*controller) return;
    CFTypeRef owner = CFBridgingRetain(*controller);
    *controller = nil;
    dispatch_async(dispatch_get_main_queue(), ^{
        (void)CFBridgingRelease(owner);
    });
}

// Numeric and space selection must use the candidate identities captured by the
// panel that is actually on screen. AppKit can deliver another key event before
// the previous content view has painted, while _view already points at the next
// Engine generation. Falling back to _view in that window can select a different
// word than the one the user sees.
static NSDictionary *MSIMERenderedCandidateIdentity(NSPanel *panel, NSInteger slot) {
    if (!panel || ![panel.contentView isKindOfClass:NSView.class]) return nil;
    for (NSView *subview in panel.contentView.subviews) {
        if (![subview isKindOfClass:MSIMECandidateButton.class] || subview.tag != slot) continue;
        NSDictionary *identity = ((MSIMECandidateButton *)subview).candidateID;
        return [identity isKindOfClass:NSDictionary.class] ? identity : nil;
    }
    return nil;
}

static NSDictionary *MSIMERenderedHighlightedCandidateIdentity(NSPanel *panel) {
    if (!panel || ![panel.contentView isKindOfClass:NSView.class]) return nil;
    for (NSView *subview in panel.contentView.subviews) {
        if (![subview isKindOfClass:MSIMECandidateButton.class] || subview.tag < 0) continue;
        MSIMECandidateButton *button = (MSIMECandidateButton *)subview;
        if (!button.candidateHighlighted) continue;
        NSDictionary *identity = button.candidateID;
        return [identity isKindOfClass:NSDictionary.class] ? identity : nil;
    }
    return nil;
}

static BOOL MSIMESmartPunctuationKey(unichar character) {
    return character == ',' || character == '.' || character == ':';
}
static BOOL MSIMEASCIIAlphanumeric(unichar character) {
    return (character >= '0' && character <= '9') || (character >= 'A' && character <= 'Z') ||
           (character >= 'a' && character <= 'z');
}
// Match the Windows TSF classifier's CapsLock special case. CapsLock turns an
// unshifted alphabetic key into an uppercase character, but an uppercase key
// must remain a native application key when a new composition would otherwise
// start. Once a composition or candidate list exists, the same key belongs to
// Engine and must not be bypassed.
static BOOL MSIMECapsLockFreshUppercaseBypass(NSEvent *event, NSDictionary *view) {
    if (!event || event.type != NSEventTypeKeyDown) return NO;
    const NSEventModifierFlags competing = NSEventModifierFlagShift | NSEventModifierFlagControl |
                                           NSEventModifierFlagOption | NSEventModifierFlagCommand;
    if (!(event.modifierFlags & NSEventModifierFlagCapsLock) || (event.modifierFlags & competing)) return NO;
    if (event.characters.length != 1) return NO;
    const unichar character = [event.characters characterAtIndex:0];
    if (character < 'A' || character > 'Z') return NO;
    NSString *editing = [view[@"editing_text"] isKindOfClass:NSString.class] ? view[@"editing_text"] : @"";
    NSArray *candidates = [view[@"candidates"] isKindOfClass:NSArray.class] ? view[@"candidates"] : @[];
    return editing.length == 0 && candidates.count == 0;
}
static NSString *MSIMEChinesePunctuationForSmart(unichar character) {
    switch (character) {
    case ',': return @"，";
    case '.': return @"。";
    case ':': return @"：";
    default: return nil;
    }
}
static unichar MSIMEASCIIForSmartChinesePunctuation(unichar character) {
    switch (character) {
    case 0x3002: return '.'; // 。
    case 0xFF0C: return ','; // ，
    case 0xFF01: return '!'; // ！
    case 0xFF1F: return '?'; // ？
    case 0xFF1B: return ';'; // ；
    case 0xFF1A: return ':'; // ：
    case 0x3001: return '/'; // 、
    case 0x201C: case 0x201D: return '"'; // “ ”
    case 0x2018: case 0x2019: return '\''; // ‘ ’
    case 0x3010: return '['; // 【
    case 0x3011: return ']'; // 】
    case 0x300A: return '<'; // 《
    case 0x300B: return '>'; // 》
    case 0xFF08: return '('; // （
    case 0xFF09: return ')'; // ）
    default: return 0;
    }
}
static BOOL MSIMEASCIIPunctuation(unichar character) {
    return character > 0x20 && character < 0x7F && !MSIMEASCIIAlphanumeric(character);
}
static BOOL MSIMESmartPunctuationSpaceKey(unichar character) {
    switch (character) {
    case '.': case ',': case '!': case '?': case ';': case ':': case '/':
    case '"': case '\'': case '[': case ']': case '<': case '>': case '(': case ')':
        return YES;
    default:
        return NO;
    }
}
static NSString *MSIMEFullWidthSmartMark(unichar character, BOOL fullWidth) {
    if (!fullWidth) return [NSString stringWithCharacters:&character length:1];
    const unichar converted = msime::mac::FullWidthCharacter(character);
    return [NSString stringWithCharacters:&converted length:1];
}

@interface MSIMECandidatePanel : NSPanel
@property(nonatomic) BOOL mouseWheelEnabled;
@property(nonatomic) BOOL hasPreviousPage;
@property(nonatomic) BOOL hasNextPage;
@property(nonatomic, copy) void (^pageHandler)(BOOL previous);
// True while a wheel page is being applied, so the re-render it causes keeps the scroll remainder.
@property(nonatomic, readonly) BOOL wheelPaging;
- (void)resetWheelAccumulator;
@end
@implementation MSIMECandidatePanel {
    double _wheelAccumulator;
}
- (BOOL)canBecomeKeyWindow { return NO; }
- (BOOL)canBecomeMainWindow { return NO; }
- (void)resetWheelAccumulator { _wheelAccumulator = 0.0; }
- (void)orderOut:(id)sender {
    _wheelAccumulator = 0.0;
    [super orderOut:sender];
}
- (void)scrollWheel:(NSEvent *)event {
    const auto action = msime::mac::CandidateWheelPageAction(
        event.scrollingDeltaY, self.mouseWheelEnabled, self.hasPreviousPage, self.hasNextPage);
    if (action == msime::mac::CandidateWheelAction::None || !self.pageHandler) {
        _wheelAccumulator = 0.0;
        [super scrollWheel:event];
        return;
    }
    const int steps = msime::mac::ConsumeCandidateWheelDelta(_wheelAccumulator, event.scrollingDeltaY,
        event.hasPreciseScrollingDeltas, (event.phase & (NSEventPhaseBegan | NSEventPhaseMayBegin)) != 0,
        event.momentumPhase != NSEventPhaseNone);
    const BOOL previous = steps > 0;
    _wheelPaging = YES;
    for (int remaining = previous ? steps : -steps; remaining > 0 && self.pageHandler && (previous ? self.hasPreviousPage : self.hasNextPage); --remaining)
        self.pageHandler(previous);
    _wheelPaging = NO;
}
@end

// Match the pinned Windows TextBlock preedit marker spacing in logical points.
static constexpr CGFloat MSIMEPreeditCaretWidth = 1.25;
static constexpr CGFloat MSIMEPreeditCaretSideAir = 0.85;
static constexpr CGFloat MSIMEPreeditCaretEndAir = 1.5;
static constexpr CGFloat MSIMEPreeditCaretGap = MSIMEPreeditCaretWidth + 2 * MSIMEPreeditCaretSideAir;
struct MSIMEPreeditSlotMetrics { CGFloat ascent; CGFloat descent; };
static void MSIMEPreeditSlotRelease(void *context) { delete static_cast<MSIMEPreeditSlotMetrics *>(context); }
static CGFloat MSIMEPreeditSlotAscent(void *context) { return static_cast<MSIMEPreeditSlotMetrics *>(context)->ascent; }
static CGFloat MSIMEPreeditSlotDescent(void *context) { return static_cast<MSIMEPreeditSlotMetrics *>(context)->descent; }
static CGFloat MSIMEPreeditSlotWidth(void *) { return MSIMEPreeditCaretGap; }

@interface MSIMECandidatePreeditField : NSTextField
@property(nonatomic) NSUInteger caretIndex;
@property(nonatomic) BOOL showsCaret;
@property(nonatomic, strong) NSColor *caretColor;
@property(nonatomic, readonly) NSRect caretRect;
@end

@implementation MSIMECandidatePreeditField
- (void)setCaretIndex:(NSUInteger)value { _caretIndex = value; self.needsDisplay = YES; }
- (void)setShowsCaret:(BOOL)value { _showsCaret = value; self.needsDisplay = YES; }
- (void)setCaretColor:(NSColor *)value { _caretColor = value; self.needsDisplay = YES; }
- (CTLineRef)newPreeditLine CF_RETURNS_RETAINED {
    NSFont *font = self.font ?: [NSFont systemFontOfSize:16];
    NSMutableAttributedString *text = [[NSMutableAttributedString alloc] initWithString:self.stringValue attributes:@{
        NSFontAttributeName:font,
        NSForegroundColorAttributeName:self.textColor ?: NSColor.labelColor
    }];
    if (self.showsCaret && self.caretIndex < self.stringValue.length) {
        CTRunDelegateCallbacks callbacks = {kCTRunDelegateVersion1, MSIMEPreeditSlotRelease,
            MSIMEPreeditSlotAscent, MSIMEPreeditSlotDescent, MSIMEPreeditSlotWidth};
        CTRunDelegateRef slot = CTRunDelegateCreate(&callbacks, new MSIMEPreeditSlotMetrics{font.ascender, -font.descender});
        NSAttributedString *gap = [[NSAttributedString alloc] initWithString:@"\uFFFC" attributes:@{
            (__bridge NSString *)kCTRunDelegateAttributeName:(__bridge id)slot, NSFontAttributeName:font
        }];
        [text insertAttributedString:gap atIndex:self.caretIndex];
        CFRelease(slot);
    }
    return CTLineCreateWithAttributedString((__bridge CFAttributedStringRef)text);
}
- (NSRect)caretRectForLine:(CTLineRef)line origin:(CGFloat *)origin baseline:(CGFloat *)baseline {
    CGFloat ascent = 0, descent = 0;
    CTLineGetTypographicBounds(line, &ascent, &descent, nullptr);
    CGFloat offset = CTLineGetOffsetForStringIndex(line, MIN(self.caretIndex, self.stringValue.length), nullptr);
    if (self.showsCaret) offset += self.caretIndex < self.stringValue.length ? MSIMEPreeditCaretSideAir : MSIMEPreeditCaretEndAir;
    CGFloat available = MAX(0.0, NSWidth(self.bounds) - 4.0 - MSIMEPreeditCaretWidth);
    // Scroll just enough to keep the insertion point inside the clipped row.
    *origin = 2.0 - MAX(0.0, offset - available);
    CGFloat top = MAX(0.0, (NSHeight(self.bounds) - ascent - descent) / 2.0);
    *baseline = top + ascent;
    return NSMakeRect(*origin + offset, top, MSIMEPreeditCaretWidth, MIN(ascent + descent, NSHeight(self.bounds)));
}
- (NSRect)caretRect {
    if (!self.showsCaret || !self.stringValue.length) return NSZeroRect;
    CTLineRef line = [self newPreeditLine];
    CGFloat origin, baseline;
    NSRect caret = [self caretRectForLine:line origin:&origin baseline:&baseline];
    CFRelease(line);
    return caret;
}
- (BOOL)isFlipped { return YES; }
- (void)drawRect:(NSRect)dirtyRect {
    (void)dirtyRect;
    CTLineRef line = [self newPreeditLine];
    CGFloat origin, baseline;
    NSRect caret = [self caretRectForLine:line origin:&origin baseline:&baseline];
    CGContextRef context = NSGraphicsContext.currentContext.CGContext;
    CGContextSaveGState(context);
    CGContextClipToRect(context, NSRectToCGRect(self.bounds));
    CGContextTranslateCTM(context, origin, baseline);
    CGContextScaleCTM(context, 1, -1);
    CGContextSetTextMatrix(context, CGAffineTransformIdentity);
    CGContextSetTextPosition(context, 0, 0);
    CTLineDraw(line, context);
    CGContextRestoreGState(context);
    CFRelease(line);
    if (self.showsCaret && self.stringValue.length) {
        [self.caretColor ?: NSColor.controlAccentColor setFill];
        NSRectFill(NSIntersectionRect(caret, self.bounds));
    }
}
@end

// Button target for the non-modal cloud consent prompt; NSAlert's own buttons only end a modal session.
@interface MSIMECloudConsentTarget : NSObject
@property(nonatomic, copy) void (^handler)(BOOL enabled);
@end
@implementation MSIMECloudConsentTarget
- (void)enable:(id)sender { (void)sender; if (self.handler) self.handler(YES); }
- (void)disable:(id)sender { (void)sender; if (self.handler) self.handler(NO); }
@end

@interface MSIMEInputController : IMKInputController <MSIMEFloatingToolbarDelegate>
- (MSIMECustomTranslationBatch *)aiBatchForItems:(NSArray<NSDictionary *> *)items
                                       completion:(void (^)(NSArray<NSDictionary *> *))completion;
- (NSDictionary *)recoverPreferencesInDirectory:(NSString *)directory error:(NSError **)error;
@end

@implementation MSIMEInputController {
    MSIMEClientSession *_session;
    MSIMEVoiceInputService *_voiceService;
    MSIMEVoiceWaveOverlay *_voiceOverlay;
    MSIMEVoiceCuePlayer *_voiceCuePlayer;
    BOOL _voiceCueRecording;
    MSIMEVoiceAudioMuter *_voiceAudioMuter;
    MSIMEHTTPVoiceRequest *_httpVoiceRequest;
    MSIMEClientSession *_httpVoiceSession;
    id _httpVoiceClient;
    uint64_t _httpVoiceGeneration;
    BOOL _httpVoiceProcessing;
    MSIMEVoiceCommitRoute _httpVoiceCommit;
    MSIMEVoiceCommitRoute _doubaoVoiceCommit;
    MSIMEVoiceCommitRoute _liveVoiceCommit;
    // Doubao's websocket or the on-device helper: the two streaming providers share this path.
    id<MSIMEStreamingVoiceRequest> _doubaoVoiceRequest;
    MSIMEHTTPVoiceRequest *_doubaoPolishRequest;
    BOOL _doubaoFinalReceived;
    MSIMEClientSession *_doubaoVoiceSession;
    id _doubaoVoiceClient;
    uint64_t _doubaoVoiceGeneration;
    BOOL _doubaoVoiceProcessing;
    BOOL _doubaoVoiceInline;
    BOOL _doubaoVoiceMarked;
    id _liveVoiceToken;
    MSIMEHTTPVoiceRequest *_livePolishRequest;
    BOOL _liveVoiceFinalReceived;
    MSIMEClientSession *_liveVoiceSession;
    id _liveVoiceClient;
    NSString *_liveVoiceSocket;
    uint64_t _liveVoiceGeneration;
    BOOL _liveVoiceInline;
    BOOL _liveVoiceMarked;
    BOOL _liveVoiceProcessing;
    id _globalVoiceHotkeyMonitor;
    id _voicePermissionToken;
    uint64_t _voiceGeneration;
    id _activeClient;
    MSIMEToolTextReturn _emojiReturn;
    MSIMEDesktopInputSession *_desktopInputSession;
    MSIMEPanelTextCompletion _desktopEmojiCompletion;
    double _desktopEmojiDeadline;
    NSDictionary *_view;
    NSObject *_candidateMenuToken;
    NSPanel *_panel;
    NSRect _candidateAnchorCaret;
    BOOL _candidateAnchorValid;
    BOOL _candidateFollowCursorMode;
    BOOL _candidateFollowCursorModeKnown;
    MSIMEShuangpinKeymapPanel *_keymapPanel;
    MSIMEFloatingToolbarPanel *_toolbar;
    NSString *_preferencesDirectory;
    NSTimer *_preferencesTimer;
    MSIMEPreferenceLoadState _preferenceLoadState;
    MSIMEPreferenceSaveState _preferenceSaveState;
    MSIMEAppearancePreferences *_appearance;
    BOOL _wubiCodeHintEnabled;
    msime::input::EnglishPunctuationState _englishPunctuation;
    BOOL _capsLock;
    // A physical Backspace hold that began while this controller owned a
    // composition stays ours after one repeat deletes the last preedit byte.
    // Otherwise the next repeat falls through to the client and starts
    // deleting document text even though the user never released the key.
    BOOL _backspaceHoldArmed;
    NSUInteger _requestedPageSize;
    BOOL _skinShowsSelectedBar;
    CGFloat _tallestVerticalCandidateHeight;
    NSInteger _armedGlossColumn;
    // Ctrl+Enter turns the highlighted candidate's gloss into a page of its senses. The composition
    // is untouched while that page is up - nothing was typed - so leaving it only needs the view
    // that was on screen put back, which is what `_glossSenseSavedView` holds.
    // Japanese conversion in progress: which candidate Space has stepped to, and the reading it
    // belongs to. A reading that has changed is a different conversion, so the pair travels
    // together - the mobile hosts keep exactly this pair for the same reason.
    NSNumber *_japaneseConversionIndex;
    NSString *_japaneseConversionReading;
    NSArray<NSString *> *_glossSenses;
    NSDictionary *_glossSenseSavedView;
    NSUInteger _glossSenseCursor;
    BOOL _focusPending;
    // Dedicated English lives in the Engine session, so a session released for dictionary maintenance takes it along; the reopen puts it back.
    BOOL _resumeDedicatedEnglish;
    // The last reason prepareSession could not open a session, so the report is made once per reason rather than per focus.
    NSString *_sessionUnavailableReason;
    unichar _lastSmartPunctuation;
    NSTimeInterval _lastSmartPunctuationTime;
    __weak id _smartPunctuationClient;
    unichar _rejectedSmartPunctuation;
    BOOL _smartPunctuationRejected;
    // The last character known to have reached the application (Windows _smartPunctuationShadowChar). Terminal-like hosts never hand typed text back through attributedSubstringFromRange: - keys they pass straight to a pty leave no trace - so passthrough keys and commits are tracked here, and the document is read only while this is invalid. `_smartPunctuationShadowWritten` says a commit or rewrite recorded the shadow during the key event being handled, so the key itself does not overwrite it.
    unichar _smartPunctuationShadow;
    BOOL _smartPunctuationShadowValid;
    BOOL _smartPunctuationShadowWritten;
    // The ASCII form of the Chinese mark a punctuation commit has just left in the document (from the mark map, not the key pressed), and the client it landed in. A space arriving next rewrites that mark as ASCII; anything else disarms.
    unichar _spaceConvertMark;
    __weak id _spaceConvertClient;
    // After a space has turned a Chinese mark into ASCII, the same key within the repeat window writes the Chinese mark back (Windows Kind::AsciiConverted). The key, the mark that was actually replaced, when, and in which client; anything else disarms.
    unichar _spaceRevertKey;
    unichar _spaceRevertChinese;
    NSTimeInterval _spaceRevertTime;
    __weak id _spaceRevertClient;
    msime::mac::PairedPunctuationTracker _pairedPunctuation;
    // The closing mark this host owes the document while a pair is open. It rides in the marked
    // text after the caret, because IMK gives an input method no way to move a client's insertion
    // point; see MSIMEApplyTransitionWithPendingClosing.
    NSString *_pendingPairedClosing;
    // The closing mark for a pair this host opened itself from the `{` key, which the Engine commits as ASCII and so never names as an opening mark. Consumed by the next apply:.
    NSString *_hostOpenedClosing;
    NSNumber *_typingSourceOverride;
    // The ASCII punctuation key behind the transition about to be applied, or 0. Set only on the punctuation-key routes and consumed by the next apply:, so pairing can read the last mark of a commit that finished a composition (`nihao(` gives `你好（`) without ever rewriting a candidate that merely ends in a mark.
    unichar _punctuationKeyInFlight;
    MSIMEModifierTap _modifierTap;
    MSIMEVoiceHoldShortcut _voiceHoldShortcut;
    uint64_t _voiceHoldGeneration;
    BOOL _voiceHoldStarting;
    NSDictionary *_voiceThemePreferences;
    NSDictionary *_menuThemePreferences;
    MSIMEDictionaryWindowController *_dictionaryWindow;
    NSTimer *_cloudTimer;
    NSTimer *_settledTimer;
    MSIMECloudCandidateRequest *_cloudRequest;
    NSDictionary *_cloudQuery;
    uint64_t _cloudEpoch;
    NSOperationQueue *_glossQueue;
    NSDictionary *_glossRequest;
    uint64_t _glossEpoch;
    NSNumber *_glossEnabled;
    NSString *_glossTargetLanguage;
    NSArray<NSString *> *_glossTargetLanguages;
    NSArray<NSDictionary *> *_glossResults;
    NSOperationQueue *_targetGlossQueue;
    NSDictionary *_targetGlossRequest;
    uint64_t _targetGlossEpoch;
    NSDictionary<NSString *, NSDictionary<NSString *, NSString *> *> *_targetGlossResults;
    MSIMECustomTranslationBatch *_customBatch;
    NSMutableArray<MSIMECustomTranslationBatch *> *_customBatches;
    NSTimer *_customTimer;
    NSDictionary *_customQuery;
    NSDictionary *_customTranslationConfig;
    NSDictionary *_tencentTranslationConfig;
    NSDictionary *_niuTransConfig;
    NSArray<NSDictionary *> *_customResults;
    NSString *_accountGlossSignature;
    NSDictionary *_accountGlossRequest;
    NSArray<NSDictionary *> *_accountGlossResults;
    uint64_t _accountGlossEpoch;
    NSDictionary *_onDeviceGlossRequest;
    uint64_t _customEpoch;
    MSIMECustomTranslationBatch *_aiBatch;
    NSTimer *_aiTimer;
    NSDictionary *_aiQuery;
    uint64_t _aiEpoch;
    NSMutableDictionary<NSString *, NSArray<NSString *> *> *_aiCandidateCache;
}

- (void)resetSmartPunctuationState {
    _lastSmartPunctuation = 0;
    _lastSmartPunctuationTime = 0;
    _smartPunctuationClient = nil;
    _rejectedSmartPunctuation = 0;
    _smartPunctuationRejected = NO;
}

- (void)clearSmartPunctuationSpaceConversion {
    _spaceConvertMark = 0;
    _spaceConvertClient = nil;
}

- (void)clearSmartPunctuationSpaceRevert {
    _spaceRevertKey = 0;
    _spaceRevertChinese = 0;
    _spaceRevertTime = 0;
    _spaceRevertClient = nil;
}

- (void)invalidateSmartPunctuationShadow {
    _smartPunctuationShadow = 0;
    _smartPunctuationShadowValid = NO;
}

// Records the character that now sits left of the caret because this host put it there.
- (void)noteSmartPunctuationShadow:(unichar)character {
    if (!character) {
        [self invalidateSmartPunctuationShadow];
        return;
    }
    _smartPunctuationShadow = character;
    _smartPunctuationShadowValid = YES;
    _smartPunctuationShadowWritten = YES;
}

// Port of Windows _UpdateSmartPunctuationShadow, run once per key down after the key has been handled. Editing and caret keys - Delete, Forward Delete, Return, Enter, Tab, Escape, the arrows, Home/End, Page Up/Down - and any Control, Option or Command chord leave the caret somewhere the shadow cannot follow, so they clear it. An eaten key keeps what a commit or rewrite recorded while it was handled and otherwise clears the shadow: it fed the composition, and the document answers until something is committed. A printable key that passed through to the application is the new shadow. The reference skips its smart punctuation keys because its resolver records them itself; here a smart key that is eaten is recorded by that same commit, and one that passes through (English mode, local modes) is what the application typed, so it is recorded like any other passthrough key.
- (void)noteKeyForSmartPunctuationShadow:(NSEvent *)event eaten:(BOOL)eaten {
    const BOOL written = _smartPunctuationShadowWritten;
    _smartPunctuationShadowWritten = NO;
    if (eaten) {
        if (!written) [self invalidateSmartPunctuationShadow];
        return;
    }
    switch (event.keyCode) {
    case 51: case 117: case 36: case 76: case 48: case 53:
    case 123: case 124: case 125: case 126:
    case 115: case 119: case 116: case 121:
        [self invalidateSmartPunctuationShadow];
        return;
    default:
        break;
    }
    NSString *characters = event.characters;
    const unichar character = characters.length == 1 ? [characters characterAtIndex:0] : 0;
    if ((event.modifierFlags & (NSEventModifierFlagControl | NSEventModifierFlagOption | NSEventModifierFlagCommand)) ||
        character < 0x20 || character == 0x7F || (character >= 0xF700 && character <= 0xF8FF)) {
        [self invalidateSmartPunctuationShadow];
        return;
    }
    _smartPunctuationShadow = character;
    _smartPunctuationShadowValid = YES;
}

// The preceding character the direct-output decision works from (Windows _GetPrecedingCharForSmartPunctuation): the shadow first, the document second.
- (uint32_t)precedingForSmartPunctuation:(id<MSIMETextClient>)client {
    return _smartPunctuationShadowValid ? _smartPunctuationShadow : MSIMETextClientPrecedingUnicodeScalar(client);
}

// The posted fallback for a host whose document read returned nothing, so the shadow is the only evidence of what is on screen (the reference's _SmartPunctuationFingerprintMatches trusts the state when it reads nothing back). Overridden in tests.
- (BOOL)postSmartPunctuationRewrite:(unichar)replacement client:(id)client {
    const NSTimeInterval now = NSProcessInfo.processInfo.systemUptime;
    return MSIMECaptureSmartPunctuationRewrite(client, now).deliver(replacement);
}

// Whether the host truly exposes no text, rather than a preceding-character read that came back empty for another reason. MSIMETextClientPrecedingUnicodeScalar also returns 0 for a real selection (a mouse drag the shadow never saw) and for a caret at the start of the document; a posted Delete there would erase the selection or insert the mark at the start, so both count as readable. Terminal-like hosts hand back nothing even for the first character, and only they keep the posted route, as the reference only sends input when a collapsed caret's document read comes back empty.
- (BOOL)textClientExposesNoText:(id<MSIMETextClient>)client {
    if (![client respondsToSelector:@selector(selectedRange)]) return YES;
    const NSRange selected = [client selectedRange];
    if (selected.location == NSNotFound) return YES;
    if (selected.length != 0) return NO;
    if (selected.location == 0 && [client respondsToSelector:@selector(attributedSubstringFromRange:)] &&
        [client attributedSubstringFromRange:NSMakeRange(0, 1)].string.length)
        return NO;
    return YES;
}

// Replaces the character left of the caret when the host exposes no text there and the shadow says it is `expected`. Returns NO, having posted nothing, when the document is readable (including a selection or a caret at its start), the shadow disagrees or is unknown, or the posted rewrite is not available.
- (BOOL)rewriteUnreadablePreceding:(unichar)expected with:(unichar)replacement client:(id<MSIMETextClient>)client {
    if (!_smartPunctuationShadowValid || _smartPunctuationShadow != expected) return NO;
    if (![self textClientExposesNoText:client]) return NO;
    if (![self postSmartPunctuationRewrite:replacement client:client]) return NO;
    [self noteSmartPunctuationShadow:replacement];
    return YES;
}

// Mirrors Windows Global::JapaneseInputModeEnabled: the configured Japanese scheme, not the temporary J mode. KeyEventSink only claims the two reversible smart punctuation gestures (space-to-ASCII and repeat-to-Chinese) when it is off.
- (BOOL)japaneseSchemeActive {
    return [_view[@"scheme"] integerValue] == 3;
}

// Arms the space conversion from what a punctuation commit actually put in the document, as the reference's _NoteCommittedChinesePunctuation does: the committed tail decides, so a candidate committed together with its mark (nihao, gives 你好，) arms too, and the ASCII target comes from the mark map rather than the key pressed (the backslash key gives 、, which converts to /). Called after apply:, so an opening mark this host has just auto-closed is seen as a pending closing and does not arm. Anything else disarms.
- (void)noteCommittedChinesePunctuation:(NSDictionary *)transition client:(id)client {
    NSString *commit = transition[@"commit"];
    const unichar ascii = [commit isKindOfClass:NSString.class] && commit.length
        ? MSIMEASCIIForSmartChinesePunctuation([commit characterAtIndex:commit.length - 1])
        : 0;
    if (!_appearance.smartPunctuation || !_appearance.smartPunctuationSpaceConvert || !ascii ||
        _pendingPairedClosing || [self japaneseSchemeActive]) {
        [self clearSmartPunctuationSpaceConversion];
        return;
    }
    _spaceConvertMark = ascii;
    _spaceConvertClient = client;
}

// A space right after a Chinese mark the user did not want takes the mark back to ASCII. It is the mirror of repeat-to-Chinese and shares its caution: the preceding character is read back and has to still be the mark that was committed, in the same client, with nothing composing - otherwise a character the user already saw land would be rewritten out from under them. A host that reads back nothing at all (a terminal) is taken on the shadow's word instead and rewritten with posted events, as the reference does when its document read returns nothing.
- (BOOL)convertSmartPunctuationSpace:(NSEvent *)event client:(id<MSIMETextClient>)client {
    if (!_spaceConvertMark) return NO;
    if (event.characters.length != 1 || [event.characters characterAtIndex:0] != ' ' ||
        (event.modifierFlags & (NSEventModifierFlagShift | NSEventModifierFlagControl | NSEventModifierFlagOption |
                                NSEventModifierFlagCommand))) {
        [self clearSmartPunctuationSpaceConversion];
        return NO;
    }
    const unichar mark = _spaceConvertMark;
    id armed = _spaceConvertClient;
    [self clearSmartPunctuationSpaceConversion];
    if (!_appearance.smartPunctuation || !_appearance.smartPunctuationSpaceConvert ||
        _appearance.runtimeFullWidthInput || armed != client || _pendingPairedClosing.length ||
        [self japaneseSchemeActive])
        return NO;
    if ([_view[@"editing_text"] length] ||
        ([_view[@"candidates"] isKindOfClass:NSArray.class] && [_view[@"candidates"] count]))
        return NO;
    uint32_t preceding = MSIMETextClientPrecedingUnicodeScalar(client);
    if (!preceding) {
        // Terminals and proxy stores expose no document text; the shadow is the only evidence of what is on screen, and the rewrite has to be posted.
        const unichar shadow = _smartPunctuationShadowValid ? _smartPunctuationShadow : 0;
        if (!shadow || MSIMEASCIIForSmartChinesePunctuation(shadow) != mark ||
            ![self rewriteUnreadablePreceding:shadow with:mark client:client])
            return NO;
        preceding = shadow;
    } else {
        if (MSIMEASCIIForSmartChinesePunctuation((unichar)preceding) != mark) return NO;
        const NSRange selected =
            [client respondsToSelector:@selector(selectedRange)] ? [client selectedRange] : NSMakeRange(NSNotFound, 0);
        if (selected.location == NSNotFound || selected.location < 1) return NO;
        NSString *ascii = [NSString stringWithCharacters:&mark length:1];
        [client insertText:ascii replacementRange:NSMakeRange(selected.location - 1, 1)];
        [self noteSmartPunctuationShadow:mark];
    }
    // Windows records chineseLeft as the mark actually read back, so a right quote comes back as a right quote.
    if (_appearance.smartPunctuationRepeatToChinese) {
        _spaceRevertKey = mark;
        _spaceRevertChinese = (unichar)preceding;
        _spaceRevertTime = NSProcessInfo.processInfo.systemUptime;
        _spaceRevertClient = client;
    }
    // This space is the conversion gesture, not document content. Matching the
    // reference also avoids leaving a surprising trailing blank after the user
    // has just corrected the punctuation form.
    return YES;
}

// The same key again right after a space conversion takes the ASCII character back to the Chinese mark it replaced (Windows _CanInterceptSmartPunctuationRevert). It is one-shot: the state is cleared before any check. Every check that fails hands the key to the Engine, which then just types the Chinese mark, as the reference falls back to inserting it.
- (BOOL)revertSmartPunctuationSpace:(NSEvent *)event client:(id<MSIMETextClient>)client {
    if (!_spaceRevertKey) return NO;
    const unichar key = _spaceRevertKey;
    const unichar chinese = _spaceRevertChinese;
    const NSTimeInterval armedAt = _spaceRevertTime;
    id armed = _spaceRevertClient;
    [self clearSmartPunctuationSpaceRevert];
    if (event.characters.length != 1 || [event.characters characterAtIndex:0] != key ||
        (event.modifierFlags & (NSEventModifierFlagControl | NSEventModifierFlagOption | NSEventModifierFlagCommand)))
        return NO;
    if (!_appearance.smartPunctuation || !_appearance.smartPunctuationRepeatToChinese ||
        _appearance.runtimeFullWidthInput || armed != client || _pendingPairedClosing.length ||
        [self japaneseSchemeActive] || NSProcessInfo.processInfo.systemUptime - armedAt > 2.0)
        return NO;
    if ([_view[@"editing_text"] length] ||
        ([_view[@"candidates"] isKindOfClass:NSArray.class] && [_view[@"candidates"] count]))
        return NO;
    const uint32_t preceding = MSIMETextClientPrecedingUnicodeScalar(client);
    // A host that reads back nothing is rewritten with posted events on the shadow's word, as the conversion was.
    if (!preceding) return [self rewriteUnreadablePreceding:key with:chinese client:client];
    if (preceding != key) return NO;
    const NSRange selected =
        [client respondsToSelector:@selector(selectedRange)] ? [client selectedRange] : NSMakeRange(NSNotFound, 0);
    if (selected.location == NSNotFound || selected.location < 1) return NO;
    [client insertText:[NSString stringWithCharacters:&chinese length:1]
      replacementRange:NSMakeRange(selected.location - 1, 1)];
    [self noteSmartPunctuationShadow:chinese];
    return YES;
}

- (BOOL)handleSmartPunctuation:(NSEvent *)event client:(id<MSIMETextClient>)client {
    if (event.characters.length != 1 ||
        (event.modifierFlags & (NSEventModifierFlagControl | NSEventModifierFlagOption | NSEventModifierFlagCommand)))
        return NO;
    const unichar character = [event.characters characterAtIndex:0];
    if (!MSIMESmartPunctuationSpaceKey(character)) return NO;
    if (!_appearance.smartPunctuation) {
        [self resetSmartPunctuationState];
        [self clearSmartPunctuationSpaceConversion];
        [self clearSmartPunctuationSpaceRevert];
        return NO;
    }
    const BOOL hasComposition = [_view[@"editing_text"] length] ||
        ([_view[@"candidates"] isKindOfClass:NSArray.class] && [_view[@"candidates"] count]);
    const BOOL japanese = [self japaneseSchemeActive];
    if (!MSIMESmartPunctuationKey(character)) return NO;
    if (!_appearance.smartPunctuationRepeatToChinese) {
        _lastSmartPunctuation = 0;
        _smartPunctuationRejected = NO;
    }
    const BOOL repeat = _lastSmartPunctuation == character && !_smartPunctuationRejected &&
        _smartPunctuationClient == client && NSProcessInfo.processInfo.systemUptime - _lastSmartPunctuationTime <= 2.0;
    if (repeat && !japanese && _appearance.smartPunctuationRepeatToChinese &&
        ![_view[@"editing_text"] length] && [_view[@"candidates"] isKindOfClass:NSArray.class] && ![_view[@"candidates"] count]) {
        const uint32_t preceding = MSIMETextClientPrecedingUnicodeScalar(client);
        NSString *expected = MSIMEFullWidthSmartMark(character, _appearance.runtimeFullWidthInput);
        NSString *chinese = MSIMEChinesePunctuationForSmart(character);
        if (preceding && expected.length == 1 && [expected characterAtIndex:0] == (unichar)preceding) {
            const NSRange selected = [client respondsToSelector:@selector(selectedRange)] ? [client selectedRange] : NSMakeRange(NSNotFound, 0);
            const NSUInteger length = expected.length;
            if (selected.location != NSNotFound && selected.location >= length) {
                [client insertText:chinese replacementRange:NSMakeRange(selected.location - length, length)];
                [self noteSmartPunctuationShadow:[chinese characterAtIndex:chinese.length - 1]];
                [self resetSmartPunctuationState];
                return YES;
            }
        } else if (!preceding && expected.length == 1 && chinese.length == 1 &&
                   [self rewriteUnreadablePreceding:[expected characterAtIndex:0] with:[chinese characterAtIndex:0] client:client]) {
            // Nothing to read back: the shadow vouches for the ASCII mark just committed, and posted events replace it. Without the posting permission this falls through and the Engine types the Chinese mark after it, as the reference did before its SendInput rewrite.
            [self resetSmartPunctuationState];
            return YES;
        }
    }
    if (_lastSmartPunctuation && _lastSmartPunctuation != character) [self resetSmartPunctuationState];
    const BOOL rejected = _smartPunctuationRejected && _rejectedSmartPunctuation == character;
    uint32_t preceding = 0;
    if (hasComposition) {
        for (NSDictionary *candidate in _view[@"candidates"]) {
            if (![candidate isKindOfClass:NSDictionary.class] || ![candidate[@"highlighted"] isEqual:@YES]) continue;
            NSString *text = candidate[@"text"];
            if ([text isKindOfClass:NSString.class] && text.length && MSIMEASCIIAlphanumeric([text characterAtIndex:text.length - 1]))
                preceding = [text characterAtIndex:text.length - 1];
            break;
        }
    } else if (!rejected) {
        preceding = [self precedingForSmartPunctuation:client];
    }
    if (!rejected && preceding && (preceding < 0x80) && MSIMEASCIIAlphanumeric((unichar)preceding)) {
        _punctuationKeyInFlight = character;
        NSDictionary *transition = hasComposition
            ? [_session punctuationASCII:(uint8_t)character error:nil]
            : [_session punctuation:(uint8_t)character preceding:preceding error:nil];
        if (!transition) { _punctuationKeyInFlight = 0; return NO; }
        if (hasComposition) {
            NSString *commit = transition[@"commit"];
            if (_appearance.runtimeFullWidthInput && [commit isKindOfClass:NSString.class] && commit.length && [commit characterAtIndex:commit.length - 1] == character) {
                NSMutableDictionary *converted = [transition mutableCopy];
                converted[@"commit"] = [[commit substringToIndex:commit.length - 1] stringByAppendingString:MSIMEFullWidthSmartMark(character, YES)];
                transition = converted;
            }
            [self apply:transition];
        } else if ([transition[@"handled"] boolValue]) {
            NSString *commit = transition[@"commit"];
            if (_appearance.runtimeFullWidthInput && [commit isKindOfClass:NSString.class] && commit.length &&
                [commit characterAtIndex:commit.length - 1] == character) {
                NSMutableDictionary *converted = [transition mutableCopy];
                converted[@"commit"] = [[commit substringToIndex:commit.length - 1] stringByAppendingString:MSIMEFullWidthSmartMark(character, YES)];
                transition = converted;
            }
            [self apply:transition];
        } else {
            _punctuationKeyInFlight = 0;
            return NO;
        }
        _lastSmartPunctuation = character;
        _lastSmartPunctuationTime = NSProcessInfo.processInfo.systemUptime;
        _smartPunctuationClient = client;
        _smartPunctuationRejected = NO;
        _rejectedSmartPunctuation = 0;
        return YES;
    }
    if (rejected) [self resetSmartPunctuationState];
    // Falling through means the Engine takes the key and commits the Chinese mark; the typeASCII route in handleEvent: notes that commit so a space arriving next can take it back, and the conversion re-reads the document before touching anything.
    return NO;
}

- (void)cancelAITranslations {
    ++_aiEpoch;
    [_aiTimer invalidate]; _aiTimer = nil;
    [_aiBatch cancel]; _aiBatch = nil;
    _aiQuery = nil;
}

- (void)cancelCustomTranslations {
    ++_customEpoch;
    [_customTimer invalidate];
    _customTimer = nil;
    [_customBatch cancel];
    for (MSIMECustomTranslationBatch *batch in [_customBatches copy])
        if (batch != _customBatch) [batch cancel];
    [_customBatches removeAllObjects];
    _customBatch = nil;
    _customQuery = nil;
    _customResults = nil;
}
- (void)cancelCandidateTranslations {
    [self cancelCandidateGloss];
    [self cancelTargetGloss];
    [self cancelCustomTranslations];
    [self cancelAITranslations];
    // The account gloss arrived after this method did and was never added to it. Its request outlived
    // deactivation, so a controller whose client had gone away still answered every gloss broadcast -
    // IMKit keeps a controller per text input client, so that is a dozen of them merging results and
    // pushing them into sessions nobody is composing in. Its siblings are all cancelled here; so is it.
    [self cancelAccountGloss];
    [self cancelOnDeviceGloss];
}

- (NSDictionary *)highlightedCandidateForGloss {
    NSArray *candidates = MSIMEReorderedPinnedCandidates(_view[@"candidates"], MSIMECandidatePinCode(_view));
    if (![candidates isKindOfClass:NSArray.class]) return nil;
    for (NSDictionary *candidate in candidates)
        if ([candidate isKindOfClass:NSDictionary.class] && [candidate[@"highlighted"] boolValue]) return candidate;
    return nil;
}

// Ctrl+Enter offers the highlighted candidate's gloss as a page of its senses.
//
// The reference does this in its Server ("副候选框"), and both Linux front ends follow it: one sense
// commits straight away, several become a short-lived page the user picks from with space, a digit
// or the arrow keys. Nothing was typed to get there, so leaving the page only has to put back the
// view that was on screen. This host had no such page at all - Ctrl+Enter fell into "any Ctrl chord
// finishes the composition and goes back to the application", so it committed what was being typed.
// Space converts and Enter takes what is on screen - the way every Japanese input method works.
//
// Romaji is not what the user typed; かな is. The Engine keeps both (`editing_text` is the romaji,
// `reading` the kana it converts to), and it has one command for each ending: `MSIME_COMMIT_RAW`
// gives the romaji back and `MSIME_COMMIT_READING` gives the kana. Every desktop host here sent
// COMMIT_RAW on Enter for every scheme, so Japanese input committed `nihon` where the user meant
// にほん - measured against the real Engine, not inferred. The touch hosts already do this
// correctly (`MSIMEInputService.enter` and `KeyboardViewController.handleReturn`), which is where
// the rule below comes from.
//
// - Space starts the conversion rather than committing it: the first press means "convert", and
//   further presses step through the candidates. This host committed the first candidate outright,
//   so there was no way to reach the second.
// - Enter commits the candidate the user stepped to, or, if they never pressed Space, the kana.
//
// Returns NO for every other scheme and for keys this does not claim, which then run as before.
- (BOOL)handleJapaneseConversionKey:(NSEvent *)event client:(id)sender {
    if (!_session || !_activeClient || event.type != NSEventTypeKeyDown) return NO;
    if ([_view[@"scheme"] integerValue] != 3) return NO;
    if (event.modifierFlags & (NSEventModifierFlagControl | NSEventModifierFlagOption |
                               NSEventModifierFlagCommand | NSEventModifierFlagShift))
        return NO;
    NSString *reading = [_view[@"editing_text"] isKindOfClass:NSString.class] ? _view[@"editing_text"] : @"";
    if (!reading.length) return NO;
    NSArray *candidates = [_view[@"candidates"] isKindOfClass:NSArray.class] ? _view[@"candidates"] : @[];
    // Editing the reading abandons the conversion that was running on the old one.
    if (![reading isEqualToString:_japaneseConversionReading ?: @""]) {
        _japaneseConversionIndex = nil;
        _japaneseConversionReading = nil;
    }
    if (event.keyCode == 49) { // Space
        if (!candidates.count) return NO;
        // Let Space reach the Engine's commit when the only row is the raw-text Fallback.
        NSDictionary *first = [candidates.firstObject isKindOfClass:NSDictionary.class] ? candidates.firstObject : nil;
        const int firstSource = [first[@"source"] isKindOfClass:NSNumber.class] ? [first[@"source"] intValue] : -1;
        if (msime::mac::JapaneseSpaceCommitsFallback(candidates.count, firstSource)) return NO;
        if (!_japaneseConversionIndex) {
            // The first press is the conversion itself. The panel already highlights the first
            // candidate, so nothing has to move - what changes is that Enter now means "take it".
            _japaneseConversionIndex = @0;
            _japaneseConversionReading = [reading copy];
            return YES;
        }
        const NSUInteger next = _japaneseConversionIndex.unsignedIntegerValue + 1;
        const BOOL wraps = next >= candidates.count;
        _japaneseConversionIndex = @(wraps ? 0 : next);
        NSDictionary *transition = [_session command:wraps ? MSIME_FIRST_CANDIDATE : MSIME_NEXT_CANDIDATE
                                               error:nil];
        if (transition) [self apply:transition];
        return YES;
    }
    if (event.keyCode != 36 && event.keyCode != 76) return NO; // Return, keypad Return
    if (_japaneseConversionIndex) {
        NSDictionary *chosen = _japaneseConversionIndex.unsignedIntegerValue < candidates.count
            ? candidates[_japaneseConversionIndex.unsignedIntegerValue] : nil;
        NSDictionary *identifier = chosen[@"id"];
        _japaneseConversionIndex = nil;
        _japaneseConversionReading = nil;
        if (!MSIMECurrentCandidateIdentity(identifier, _view)) return NO;
        NSDictionary *transition = [_session selectGeneration:[identifier[@"generation"] unsignedLongLongValue]
                                                        index:[identifier[@"index"] unsignedIntegerValue]
                                                        error:nil];
        if (!transition) return NO;
        [self apply:transition];
        return YES;
    }
    NSDictionary *transition = [_session command:MSIME_COMMIT_READING error:nil];
    // The Engine answers nothing for a composition it cannot read back as kana; that key then
    // means what it always meant.
    if (!transition || ![transition[@"handled"] boolValue]) return NO;
    [self apply:transition];
    return YES;
}

- (BOOL)glossSensePageActive { return _glossSenses.count > 0; }

- (NSArray<NSString *> *)sensesForHighlightedCandidate {
    NSDictionary *candidate = [self highlightedCandidateForGloss];
    NSString *gloss = CandidateTranslation(candidate);
    if (gloss.length == 0 || gloss.length > 4096) return @[];
    const std::string utf8 = gloss.UTF8String ? gloss.UTF8String : "";
    NSMutableArray<NSString *> *senses = [NSMutableArray array];
    for (const auto &sense : msime::mac::candidate_gloss_senses(utf8)) {
        NSString *text = [[NSString alloc] initWithBytes:sense.data() length:sense.size()
                                               encoding:NSUTF8StringEncoding];
        if (text.length) [senses addObject:text];
    }
    return senses;
}

// The page the panel draws while the senses are up: the same shape as an Engine view, so the
// renderer, the placement and the skin all work unchanged.
- (NSDictionary *)glossSenseView {
    NSMutableArray *candidates = [NSMutableArray array];
    const NSUInteger pageSize = MAX((NSUInteger)1, (NSUInteger)_appearance.pageSize);
    const NSUInteger page = _glossSenseCursor / pageSize;
    const NSUInteger start = page * pageSize;
    for (NSUInteger index = start; index < MIN(start + pageSize, _glossSenses.count); ++index)
        [candidates addObject:@{ @"text": _glossSenses[index],
                                 @"highlighted": @(index == _glossSenseCursor) }];
    NSMutableDictionary *view = [(_glossSenseSavedView ?: @{}) mutableCopy];
    view[@"candidates"] = candidates;
    view[@"page"] = @(page);
    view[@"page_count"] = @((_glossSenses.count + pageSize - 1) / pageSize);
    return view;
}

- (void)showGlossSensePage:(NSArray<NSString *> *)senses {
    _glossSenses = senses;
    _glossSenseCursor = 0;
    _glossSenseSavedView = _view;
    _armedGlossColumn = 0;
    _view = [self glossSenseView];
    [self renderCandidates];
}

// Dropping the page without putting anything back, for the paths that are about to replace the view
// themselves. The saved view is a snapshot of a composition that no longer exists once the Engine
// has answered or the client has gone away; restoring it there would put a dead candidate list on
// screen.
- (void)discardGlossSensePage {
    _glossSenses = nil;
    _glossSenseSavedView = nil;
    _glossSenseCursor = 0;
}

- (void)leaveGlossSensePage {
    if (!_glossSenses.count) return;
    _glossSenses = nil;
    _glossSenseCursor = 0;
    if (_glossSenseSavedView) _view = _glossSenseSavedView;
    _glossSenseSavedView = nil;
    [self renderCandidates];
}

- (BOOL)commitGlossSenseAtIndex:(NSUInteger)index client:(id)sender {
    if (index >= _glossSenses.count || ![sender respondsToSelector:@selector(insertText:replacementRange:)])
        return NO;
    NSString *sense = _glossSenses[index];
    // The page is drawn through the same conversion (renderCandidates), and _view is still the page view carrying the saved composition's scheme and local_mode, so what is inserted is what is shown.
    if (_appearance.traditionalOutput && MSIMEScriptConversionApplies(_view)) sense = MSIMEChineseOutputString(sense, YES);
    [self discardGlossSensePage];
    [(id<MSIMETextClient>)sender insertText:sense replacementRange:NSMakeRange(NSNotFound, NSNotFound)];
    // The sense is the output; what was being composed goes away rather than following it out.
    NSDictionary *cancelled = _session ? [_session command:MSIME_CANCEL error:nil] : nil;
    if (cancelled) [self apply:cancelled];
    else [self renderCandidates];
    return YES;
}

// Every key while the page is up. Anything this does not claim closes the page and is then handled
// as usual, so no key is swallowed by a mode the user has forgotten about.
- (BOOL)handleGlossSenseEvent:(NSEvent *)event client:(id)sender {
    if (!_glossSenses.count || event.type != NSEventTypeKeyDown) return NO;
    const NSEventModifierFlags modifiers = event.modifierFlags &
        (NSEventModifierFlagShift | NSEventModifierFlagControl | NSEventModifierFlagOption | NSEventModifierFlagCommand);
    const NSUInteger pageSize = MAX((NSUInteger)1, (NSUInteger)_appearance.pageSize);
    const NSUInteger count = _glossSenses.count;
    if (modifiers == 0) {
        const int slot = msime::mac::PhysicalCandidateDigitSlot(event.keyCode);
        if (slot >= 0) {
            const NSUInteger index = (_glossSenseCursor / pageSize) * pageSize + (NSUInteger)slot;
            if (index < count && (NSUInteger)slot < pageSize) return [self commitGlossSenseAtIndex:index client:sender];
            return YES; // A slot this page does not have stays inside the page rather than typing.
        }
        switch (event.keyCode) {
            case 49: case 36: case 76: // Space and both Returns take the highlighted sense.
                return [self commitGlossSenseAtIndex:_glossSenseCursor client:sender];
            case 53: [self leaveGlossSensePage]; return YES;
            case 125: case 124: // Down and right move to the next sense.
                if (_glossSenseCursor + 1 < count) ++_glossSenseCursor;
                _view = [self glossSenseView];
                [self renderCandidates];
                return YES;
            case 126: case 123:
                if (_glossSenseCursor > 0) --_glossSenseCursor;
                _view = [self glossSenseView];
                [self renderCandidates];
                return YES;
            case 121: // Page down.
                _glossSenseCursor = MIN(count - 1, _glossSenseCursor + pageSize);
                _view = [self glossSenseView];
                [self renderCandidates];
                return YES;
            case 116:
                _glossSenseCursor = _glossSenseCursor > pageSize ? _glossSenseCursor - pageSize : 0;
                _view = [self glossSenseView];
                [self renderCandidates];
                return YES;
            default: break;
        }
    }
    [self leaveGlossSensePage];
    return NO;
}

- (BOOL)commitCandidateGlossColumn:(NSInteger)column candidate:(NSDictionary *)candidate client:(id)sender {
    if (!_session || ![sender respondsToSelector:@selector(insertText:replacementRange:)] ||
        ![candidate isKindOfClass:NSDictionary.class] || column <= 0) return NO;
    NSString *gloss = MSIMECandidateTranslationColumn(candidate, column);
    if (!gloss.length) return NO;
    // Inserted text follows the traditional-output switch like every other commit (the reference's CandidateTextForOutput); Latin and kana glosses pass through unchanged.
    if (_appearance.traditionalOutput && MSIMEScriptConversionApplies(_view)) gloss = MSIMEChineseOutputString(gloss, YES);
    [(id<MSIMETextClient>)sender insertText:gloss replacementRange:NSMakeRange(NSNotFound, NSNotFound)];
    _armedGlossColumn = 0;
    // The gloss is what the user asked for, so the composition goes away rather than being
    // committed after it: finishing commits the highlighted candidate too, which put the Chinese
    // word into the document behind the translation - and behind the wrong word at that, since the
    // gloss can be taken from a candidate that is not the highlighted one.
    //
    // The reference commits the sense and clears its state, and the Linux hosts follow it with an
    // explicit MSIME_CANCEL after committing the text.
    NSDictionary *cancelled = [_session command:MSIME_CANCEL error:nil];
    if (cancelled) [self apply:cancelled];
    return YES;
}

- (BOOL)commitHighlightedGlossColumn:(NSInteger)column client:(id)sender {
    return [self commitCandidateGlossColumn:column candidate:[self highlightedCandidateForGloss] client:sender];
}

- (BOOL)cycleArmedGlossColumnBackwards:(BOOL)backwards {
    if (!_session || ![_view[@"editing_text"] length]) return NO;
    NSDictionary *candidate = [self highlightedCandidateForGloss];
    if (!candidate) return NO;
    BOOL hasPrimary = MSIMECandidateTranslationColumn(candidate, 1).length > 0;
    BOOL hasSecondary = MSIMECandidateTranslationColumn(candidate, 2).length > 0;
    if (!hasPrimary && !hasSecondary) return NO;
    NSMutableArray<NSNumber *> *available = [NSMutableArray arrayWithObject:@0];
    if (hasPrimary) [available addObject:@1];
    if (hasSecondary) [available addObject:@2];
    NSUInteger current = [available indexOfObject:@(_armedGlossColumn)];
    if (current == NSNotFound) current = 0;
    NSInteger step = backwards ? -1 : 1;
    NSInteger next = (NSInteger)current + step;
    if (next < 0) next = (NSInteger)available.count - 1;
    if (next >= (NSInteger)available.count) next = 0;
    _armedGlossColumn = available[(NSUInteger)next].integerValue;
    return YES;
}

- (void)synchronizeAITranslations {
    // AI suggestions are gated by the AI assistant switch alone, like the source's ai_eligible / UpdateAiInput. The gloss switch (candidate_translations) only governs glosses and translations, so it is deliberately not checked here.
    if (!_activeClient || !_session || _focusPending || _appearance.englishMode) { [self cancelAITranslations]; return; }
    NSDictionary *online = [_session onlineQueryWithError:nil];
    NSDictionary *config = online[@"ai_assistant"];
    // OnlineQuery serializes Engine's segmentation as pinyin_segments. Keep the
    // provider request field name (segmented_pinyin) at the HTTP boundary only.
    NSArray *segments = online[@"pinyin_segments"];
    if (![config isKindOfClass:NSDictionary.class] || ![config[@"enabled"] boolValue] ||
        ![segments isKindOfClass:NSArray.class] || !segments.count) { [self cancelAITranslations]; return; }
    if (!_aiCandidateCache) _aiCandidateCache = [NSMutableDictionary dictionary];
    NSDictionary *query = @{ @"online": online, @"config": config };
    if ([_aiQuery isEqual:query]) return;
    [self cancelAITranslations]; _aiQuery = query;
    NSString *cacheKey = MSIMEAICacheKey(online);
    NSArray<NSString *> *cachedCandidates = (!_view || !MSIMEViewContainsAICandidate(_view)) && cacheKey
        ? _aiCandidateCache[cacheKey] : nil;
    if (cachedCandidates.count) {
        NSDictionary *transition = [_session applyOnlineCandidates:cachedCandidates source:1 query:online error:nil];
        if ([transition[@"applied"] boolValue]) {
            // The same shape the provider path records, not the bare online query: this value is compared
            // against a freshly built @{online, config} on the next render, and a bare one never matches,
            // so the render that follows this apply asked the session for another descriptor.
            NSDictionary *postOnline = [_session onlineQueryWithError:nil];
            NSDictionary *postConfig = postOnline[@"ai_assistant"];
            _aiQuery = [postOnline isKindOfClass:NSDictionary.class] && [postConfig isKindOfClass:NSDictionary.class]
                ? @{ @"online": [postOnline copy], @"config": [postConfig copy] } : nil;
            [self apply:transition];
            return;
        }
    }
    NSDictionary *descriptor = [_session aiRequestForQuery:online error:nil];
    if (!descriptor) {
        // A malformed or temporarily unavailable provider descriptor must not
        // poison this query identity. A later render may observe corrected
        // settings and should be allowed to construct a fresh request.
        _aiQuery = nil;
        return;
    }
    NSArray *items = @[ @{ @"text": @"ai", @"request": descriptor,
        @"candidate_limit": config[@"candidate_limit"] ?: @3 } ];
    uint64_t epoch = _aiEpoch; MSIMEClientSession *session = _session; id client = _activeClient;
    __weak MSIMEInputController *weakSelf = self;
    _aiTimer = [NSTimer scheduledTimerWithTimeInterval:0.65 repeats:NO block:^(NSTimer *timer) {
        MSIMEInputController *owner = weakSelf;
        if (!owner || owner->_aiEpoch != epoch || owner->_aiTimer != timer || owner->_session != session || owner->_activeClient != client) return;
        owner->_aiTimer = nil;
        owner->_aiBatch = [owner aiBatchForItems:items completion:^(NSArray *results) {
            MSIMEInputController *current = weakSelf;
            if (!current || current->_aiEpoch != epoch || current->_session != session || current->_activeClient != client || ![current->_aiQuery isEqual:query]) return;
            current->_aiBatch = nil;
            NSMutableArray *texts = [NSMutableArray array];
            for (NSDictionary *result in results) if ([result[@"translation"] isKindOfClass:NSString.class]) [texts addObject:result[@"translation"]];
            NSDictionary *transition = [session applyOnlineCandidates:texts source:1 query:query[@"online"] error:nil];
            if ([transition[@"applied"] boolValue]) {
                // Cache what the session took, not what the provider said. Caching before the attempt
                // meant a refused suggestion was kept and served straight back on the next render, so the
                // retry re-applied the text the session had just declined instead of asking again.
                if (texts.count && cacheKey) {
                    if (current->_aiCandidateCache.count >= 4096) [current->_aiCandidateCache removeAllObjects];
                    current->_aiCandidateCache[cacheKey] = [texts copy];
                }
                // apply_online_candidates advances the shared generation. Keep
                // the post-apply identity before applying the view so the
                // render pass does not enqueue the same AI request again.
                NSDictionary *postOnline = [session onlineQueryWithError:nil];
                NSDictionary *postConfig = postOnline[@"ai_assistant"];
                current->_aiQuery = [postOnline isKindOfClass:NSDictionary.class] &&
                    [postConfig isKindOfClass:NSDictionary.class]
                    ? @{ @"online": [postOnline copy], @"config": [postConfig copy] } : nil;
                [current apply:transition];
            } else {
                // Empty or rejected provider results remain eligible for a later
                // render after the local candidate generation is rebuilt.
                current->_aiQuery = nil;
            }
        }];
        [owner->_aiBatch start];
    }];
}
- (MSIMECustomTranslationBatch *)aiBatchForItems:(NSArray<NSDictionary *> *)items
                                       completion:(void (^)(NSArray<NSDictionary *> *))completion {
    return [[MSIMECustomTranslationBatch alloc]
        initWithAIItems:items
           configuration:NSURLSessionConfiguration.ephemeralSessionConfiguration
              completion:completion];
}
- (NSDictionary *)currentCustomTranslationRequest {
    if (!_activeClient || !_session || _focusPending || _appearance.englishMode ||
        (_appearance && !_appearance.candidateTranslations) || (_glossEnabled && !_glossEnabled.boolValue)) return nil;
    NSDictionary *query = [_session translationQueryWithError:nil];
    NSDictionary *config = query[@"niutrans"];
    BOOL niuTrans = [config isKindOfClass:NSDictionary.class] && [config[@"enabled"] isEqual:@YES];
    if (!niuTrans) config = query[@"custom_translation"];
    BOOL custom = !niuTrans && [config isKindOfClass:NSDictionary.class] && [config[@"enabled"] isEqual:@YES];
    if (!niuTrans && !custom) config = query[@"tencent_tmt"];
    if (![config isKindOfClass:NSDictionary.class] || ![config[@"enabled"] isEqual:@YES]) return nil;
    // A preference snapshot can be pending in Engine while the composition is active.
    if ((niuTrans && _niuTransConfig && ![_niuTransConfig isEqual:config]) ||
        (!niuTrans && [_niuTransConfig[@"enabled"] isEqual:@YES]) ||
        (custom && _customTranslationConfig && ![_customTranslationConfig isEqual:config]) ||
        (!niuTrans && !custom && ([_customTranslationConfig[@"enabled"] isEqual:@YES] ||
            (_tencentTranslationConfig && ![_tencentTranslationConfig isEqual:config]))) ||
        (_glossTargetLanguages && ![_glossTargetLanguages isEqual:MSIMETranslationTargets(query)])) return nil;
    NSDictionary *view = [_session viewWithError:nil];
    if ([view[@"scheme"] isEqual:@3] || [view[@"local_mode"] isEqual:@"temporary_japanese"] ||
        ![view[@"generation"] isEqual:query[@"generation"]]) return nil;
    NSDictionary *gloss = [self currentGlossRequest];
    // Resolve the local dictionary first; never transmit an already-resolved key.
    if (gloss && (![_glossRequest isEqual:gloss] || !_glossResults)) return nil;
    NSMutableArray *candidates = [NSMutableArray array];
    for (NSDictionary *candidate in view[@"candidates"]) {
        if (![candidate[@"text"] isKindOfClass:NSString.class] || ![candidate[@"source"] isKindOfClass:NSNumber.class]) continue;
        [candidates addObject:@{@"text":candidate[@"text"], @"source":candidate[@"source"]}];
    }
    if (!candidates.count) return nil;
    return @{@"generation":query[@"generation"], @"target_language":query[@"target_language"],
        @"target_languages":MSIMETranslationTargets(query),
        (niuTrans ? @"niutrans" : custom ? @"custom_translation" : @"tencent_tmt"):config, @"candidates":[candidates copy],
        @"directory":_preferencesDirectory ?: @""};
}
// Each offline dictionary answers a single target language, so the rows are merged per candidate in the user's target order. The English gloss and any account gloss fill the targets they cover; otherwise whichever source answered a candidate first would hide the other target rows. On-device translation comes last and fills only what every dictionary left empty: it translates the word without context, which a dictionary entry does not.
- (NSArray<NSDictionary *> *)offlineGlossResults:(NSDictionary *)targetGloss english:(NSArray<NSDictionary *> *)english
                                        onDevice:(NSDictionary *)onDevice {
    NSMutableDictionary<NSString *, NSMutableDictionary<NSString *, NSString *> *> *values = [NSMutableDictionary dictionary];
    void (^fill)(id, NSString *, id) = ^(id text, NSString *target, id value) {
        if (![text isKindOfClass:NSString.class] || ![value isKindOfClass:NSString.class] || ![(NSString *)value length]) return;
        NSMutableDictionary *byTarget = values[text];
        if (!byTarget) { byTarget = [NSMutableDictionary dictionary]; values[text] = byTarget; }
        if (!byTarget[target]) byTarget[target] = value;
    };
    for (NSDictionary *entry in english) fill(entry[@"text"], @"en", entry[@"translation"]);
    [targetGloss enumerateKeysAndObjectsUsingBlock:^(NSString *text, NSDictionary *byTarget, BOOL *stop) {
        (void)stop;
        for (NSString *target in byTarget) fill(text, target, byTarget[target]);
    }];
    if (_accountGlossResults && [_accountGlossRequest isEqual:[self currentAccountGlossRequest]]) {
        NSArray *accountTargets = _accountGlossRequest[@"target_languages"];
        for (NSDictionary *entry in _accountGlossResults) {
            if (![entry[@"translation"] isKindOfClass:NSString.class]) continue;
            NSArray *lines = [entry[@"translation"] componentsSeparatedByString:@"\n"];
            for (NSUInteger index = 0; index < MIN(lines.count, accountTargets.count); ++index)
                fill(entry[@"text"], accountTargets[index], lines[index]);
        }
    }
    for (NSDictionary *candidate in onDevice[@"candidates"])
        for (NSString *target in onDevice[@"target_languages"])
            fill(candidate[@"text"], target, [[MSIMETranslationCache sharedCache] valueForIdentity:MSIMEOnDeviceGlossIdentity(target, candidate[@"text"])]);
    // Without a target dictionary the page order comes from the English answers and the on-device request, which between them cover every candidate that can carry a gloss here.
    NSMutableArray *texts = [NSMutableArray array];
    for (NSDictionary *candidate in targetGloss ? _targetGlossRequest[@"candidates"] : @[]) [texts addObject:candidate[@"text"]];
    for (NSDictionary *entry in english) if (![texts containsObject:entry[@"text"]]) [texts addObject:entry[@"text"]];
    for (NSDictionary *candidate in onDevice[@"candidates"]) if (![texts containsObject:candidate[@"text"]]) [texts addObject:candidate[@"text"]];
    NSArray *targets = (targetGloss ? _targetGlossRequest : onDevice)[@"target_languages"];
    NSMutableArray *results = [NSMutableArray array];
    for (NSString *text in texts) {
        NSString *translation = MSIMEJoinedTranslations(values[text], targets);
        if (translation.length) [results addObject:@{@"text":text, @"translation":translation}];
    }
    return results;
}
- (void)applyCandidateTranslationResults {
    NSMutableArray *results = [NSMutableArray array];
    BOOL customCurrent = _customResults && [_customQuery isEqual:[self currentCustomTranslationRequest]];
    BOOL glossCurrent = _glossResults && [_glossRequest isEqual:[self currentGlossRequest]];
    NSDictionary *targetGloss = _targetGlossResults.count && [_targetGlossRequest isEqual:[self currentTargetGlossRequest]]
        ? _targetGlossResults : nil;
    NSDictionary *onDevice = _onDeviceGlossRequest && [_onDeviceGlossRequest isEqual:[self currentOnDeviceGlossRequest]]
        ? _onDeviceGlossRequest : nil;
    if (customCurrent && _customResults.count) [results addObjectsFromArray:_customResults];
    else if (targetGloss || onDevice)
        [results addObjectsFromArray:[self offlineGlossResults:targetGloss english:glossCurrent ? _glossResults : nil onDevice:onDevice]];
    else if (glossCurrent) [results addObjectsFromArray:_glossResults];
    if (_accountGlossResults && [_accountGlossRequest isEqual:[self currentAccountGlossRequest]]) {
        NSMutableSet *existing = [NSMutableSet setWithArray:[results valueForKey:@"text"] ?: @[]];
        for (NSDictionary *entry in _accountGlossResults) {
            NSString *text = entry[@"text"];
            if ([text isKindOfClass:NSString.class] && ![existing containsObject:text]) {
                [results addObject:entry];
                [existing addObject:text];
            }
        }
    }
    NSDictionary *view = [_session viewWithError:nil];
    if (!view) return;
    NSDictionary *applied = [_session applyTranslations:results generation:[view[@"generation"] unsignedLongLongValue] error:nil];
    if ([applied[@"applied"] boolValue]) [self apply:applied];
}

- (void)cancelAccountGloss {
    ++_accountGlossEpoch;
    _accountGlossRequest = nil;
    _accountGlossResults = nil;
    _accountGlossSignature = nil;
}

- (NSDictionary *)currentAccountGlossRequest {
    if (!_activeClient || !_session || _focusPending || _appearance.englishMode || !_appearance.candidateTranslations)
        return nil;
    NSDictionary *query = [_session translationQueryWithError:nil];
    if (![query isKindOfClass:NSDictionary.class] || !query[@"generation"] ||
        ![query[@"target_languages"] isKindOfClass:NSArray.class]) return nil;
    // The account endpoint (api.msime.app) is used only when the user explicitly chose it in settings. The shared core already folds in candidate_translations and the precedence of the user's own services, so this flag is the whole decision.
    if (![query[@"translation_account"] isEqual:@YES]) return nil;
    NSArray *candidates = MSIMEOnlineGlossCandidates(query);
    return candidates.count
        ? @{ @"generation": query[@"generation"], @"target_languages": query[@"target_languages"],
             @"candidates": candidates } : nil;
}

- (NSArray<NSDictionary *> *)accountGlossResultsForRequest:(NSDictionary *)request {
    NSMutableArray *results = [NSMutableArray array];
    for (NSDictionary *candidate in request[@"candidates"]) {
        NSString *text = candidate[@"text"];
        if (![text isKindOfClass:NSString.class]) continue;
        NSMutableArray *values = [NSMutableArray array];
        for (NSString *target in request[@"target_languages"]) {
            [values addObject:MSIMEAccountGlossCached(target, text) ?: @""];
        }
        BOOL hasValue = NO;
        for (NSString *value in values) if (value.length) { hasValue = YES; break; }
        if (hasValue) [results addObject:@{ @"text": text, @"translation": [values componentsJoinedByString:@"\n"] }];
    }
    return results;
}

- (void)synchronizeAccountGloss:(NSDictionary *)request {
    if (!request) { [self cancelAccountGloss]; return; }
    if ([_accountGlossRequest isEqual:request]) return;
    [self cancelAccountGloss];
    _accountGlossRequest = [request copy];
    NSArray *targets = request[@"target_languages"];
    NSMutableArray *pending = [NSMutableArray array];
    NSMutableString *signature = [NSMutableString string];
    for (NSString *target in targets) {
        for (NSDictionary *candidate in request[@"candidates"]) {
            NSString *text = candidate[@"text"];
            if (![target isKindOfClass:NSString.class] || ![text isKindOfClass:NSString.class]) continue;
            [signature appendFormat:@"|%@|%@", target, text];
            if (!MSIMEAccountGlossCached(target, text)) [pending addObject:text];
        }
    }
    _accountGlossResults = [self accountGlossResultsForRequest:request];
    [self applyCandidateTranslationResults];
    if (!pending.count || [_accountGlossSignature isEqualToString:signature]) return;
    _accountGlossSignature = [signature copy];
    NSMutableArray *unique = [NSMutableArray array];
    for (NSString *text in pending) if (![unique containsObject:text]) [unique addObject:text];
    NSData *payload = [NSJSONSerialization dataWithJSONObject:unique options:0 error:nil];
    if (!payload) return;
    NSString *primary = targets.firstObject ?: @"";
    NSString *secondary = targets.count > 1 ? targets[1] : @"";
    // weak_import: the Swift backend supplies this, and a process without the dylib binds it to null.
    // Calling through that is a jump to address zero, which is what the settings window's three call
    // sites have always guarded against and these two did not.
    if (MSIMEFetchAccountCandidateGlosses == nullptr) return;
    MSIMEFetchAccountCandidateGlosses([[NSString alloc] initWithData:payload encoding:NSUTF8StringEncoding].UTF8String,
                                      primary.UTF8String, secondary.UTF8String,
                                      [request[@"generation"] unsignedLongLongValue]);
}

- (void)accountCandidateTranslationsDidArrive:(NSNotification *)notification {
    NSDictionary *info = notification.userInfo;
    if (![_accountGlossRequest isKindOfClass:NSDictionary.class] || ![info isKindOfClass:NSDictionary.class]) return;
    // The account backend can finish a request after this controller has moved on to a newer
    // candidate generation. Do not let that late response populate the current page (or its
    // shared cache) with data belonging to the old request.
    NSNumber *generation = info[@"generation"];
    if (![generation isKindOfClass:NSNumber.class] ||
        ![generation isEqual:_accountGlossRequest[@"generation"]]) return;
    void (^merge)(NSDictionary *, NSString *) = ^(NSDictionary *values, NSString *target) {
        if (![values isKindOfClass:NSDictionary.class]) return;
        for (NSString *text in values) {
            NSString *value = values[text];
            if ([text isKindOfClass:NSString.class] && [value isKindOfClass:NSString.class] && value.length)
                [[MSIMETranslationCache sharedCache] rememberTranslation:value
                                                                identity:MSIMEAccountGlossIdentity(target, text)];
        }
    };
    NSArray *targets = _accountGlossRequest[@"target_languages"];
    merge(info[@"translations"], targets.firstObject ?: @"");
    if (targets.count > 1) merge(info[@"secondaryTranslations"], targets[1]);
    _accountGlossResults = [self accountGlossResultsForRequest:_accountGlossRequest];
    [self applyCandidateTranslationResults];
}

- (void)cancelOnDeviceGloss {
    _onDeviceGlossRequest = nil;
}

// Apple's on-device models fill the rows the offline dictionaries leave empty, for users who translate without a service of their own. A service the user configured, or the MSIME account, answers every candidate itself, so this stays idle then. Only Chinese candidates are asked about, by the same per-candidate answer the account path uses.
- (NSDictionary *)currentOnDeviceGlossRequest {
    if (!_activeClient || !_session || _focusPending || _appearance.englishMode || !_appearance.candidateTranslations ||
        (_glossEnabled && !_glossEnabled.boolValue)) return nil;
    NSDictionary *query = [_session translationQueryWithError:nil];
    if (![query isKindOfClass:NSDictionary.class] || !query[@"generation"] || [query[@"translation_account"] isEqual:@YES]) return nil;
    for (NSString *service in @[@"niutrans", @"custom_translation", @"tencent_tmt"]) {
        NSDictionary *config = query[service];
        if ([config isKindOfClass:NSDictionary.class] && [config[@"enabled"] isEqual:@YES]) return nil;
    }
    NSArray *targets = MSIMETranslationTargets(query);
    if (!targets.count || (_glossTargetLanguages && ![_glossTargetLanguages isEqual:targets])) return nil;
    NSDictionary *view = [_session viewWithError:nil];
    if ([view[@"scheme"] isEqual:@3] || [view[@"local_mode"] isEqual:@"temporary_japanese"] ||
        ![view[@"generation"] isEqual:query[@"generation"]]) return nil;
    NSMutableArray *candidates = [NSMutableArray array];
    for (NSDictionary *candidate in MSIMEOnlineGlossCandidates(query)) [candidates addObject:@{@"text":candidate[@"text"]}];
    return candidates.count ? @{@"generation":query[@"generation"], @"target_languages":targets, @"candidates":[candidates copy]} : nil;
}

- (void)fetchOnDeviceGlosses:(NSArray<NSString *> *)words targets:(NSArray<NSString *> *)targets {
    if (MSIMEFetchOnDeviceCandidateGlosses == nullptr) return;
    NSData *wordsJSON = [NSJSONSerialization dataWithJSONObject:words options:0 error:nil];
    NSData *targetsJSON = [NSJSONSerialization dataWithJSONObject:targets options:0 error:nil];
    if (!wordsJSON || !targetsJSON) return;
    MSIMEFetchOnDeviceCandidateGlosses([[NSString alloc] initWithData:wordsJSON encoding:NSUTF8StringEncoding].UTF8String,
                                       [[NSString alloc] initWithData:targetsJSON encoding:NSUTF8StringEncoding].UTF8String);
}

- (void)synchronizeOnDeviceGloss {
    NSDictionary *request = [self currentOnDeviceGlossRequest];
    if (!request) { [self cancelOnDeviceGloss]; return; }
    if ([_onDeviceGlossRequest isEqual:request]) return;
    _onDeviceGlossRequest = request;
    // Only what no earlier page already answered. The backend keeps one batch in flight per language and lets a newer page replace a waiting one, so typing does not queue up stale work.
    NSMutableArray<NSString *> *words = [NSMutableArray array];
    NSMutableArray<NSString *> *targets = [NSMutableArray array];
    for (NSString *target in request[@"target_languages"]) {
        for (NSDictionary *candidate in request[@"candidates"]) {
            NSString *text = candidate[@"text"];
            if ([[MSIMETranslationCache sharedCache] valueForIdentity:MSIMEOnDeviceGlossIdentity(target, text)]) continue;
            if (![words containsObject:text]) [words addObject:text];
            if (![targets containsObject:target]) [targets addObject:target];
        }
    }
    // The backend takes at most 32 words, more than any page shows.
    if (words.count > 32) [words removeObjectsInRange:NSMakeRange(32, words.count - 32)];
    [self applyCandidateTranslationResults];
    if (words.count) [self fetchOnDeviceGlosses:words targets:targets];
}

- (void)onDeviceCandidateTranslationsDidArrive:(NSNotification *)notification {
    NSString *target = notification.userInfo[@"target"];
    NSDictionary *values = notification.userInfo[@"translations"];
    if (![target isKindOfClass:NSString.class] || ![values isKindOfClass:NSDictionary.class]) return;
    for (NSString *text in values) {
        NSString *value = values[text];
        if ([text isKindOfClass:NSString.class] && [value isKindOfClass:NSString.class])
            [[MSIMETranslationCache sharedCache] rememberTranslation:value identity:MSIMEOnDeviceGlossIdentity(target, text)];
    }
    // Every controller hears the reply; only one still composing the page it asked about merges it.
    if (_onDeviceGlossRequest && [_onDeviceGlossRequest isEqual:[self currentOnDeviceGlossRequest]]) [self applyCandidateTranslationResults];
}
- (MSIMECustomTranslationBatch *)customBatchForItems:(NSArray<NSDictionary *> *)items completion:(void (^)(NSArray<NSDictionary *> *))completion {
    return [[MSIMECustomTranslationBatch alloc] initWithItems:items configuration:NSURLSessionConfiguration.ephemeralSessionConfiguration completion:completion];
}
- (MSIMECustomTranslationBatch *)tencentBatchForItems:(NSArray<NSDictionary *> *)items config:(NSDictionary *)config
                                         completion:(void (^)(NSArray<NSDictionary *> *))completion {
    return [[MSIMECustomTranslationBatch alloc] initWithTencentItems:items config:config
        configuration:NSURLSessionConfiguration.ephemeralSessionConfiguration completion:completion];
}
- (MSIMECustomTranslationBatch *)niuTransBatchForItems:(NSArray<NSDictionary *> *)items config:(NSDictionary *)config
                                           completion:(void (^)(NSArray<NSDictionary *> *))completion {
    return [[MSIMECustomTranslationBatch alloc] initWithNiuTransItems:items config:config
        configuration:NSURLSessionConfiguration.ephemeralSessionConfiguration completion:completion];
}
- (NSTimer *)customTranslationTimerWithBlock:(void (^)(NSTimer *))block {
    NSTimer *timer = [NSTimer timerWithTimeInterval:0.5 repeats:NO block:block];
    [NSRunLoop.mainRunLoop addTimer:timer forMode:NSRunLoopCommonModes];
    return timer;
}
- (void)synchronizeCustomTranslations {
    NSDictionary *query = [self currentCustomTranslationRequest];
    if (!query) {
        [self cancelCustomTranslations];
        [self synchronizeAccountGloss:[self currentAccountGlossRequest]];
        return;
    }
    [self cancelAccountGloss];
    if ([_customQuery isEqual:query]) return;
    [self cancelCustomTranslations];
    _customQuery = query;
    NSArray<NSString *> *targets = MSIMETranslationTargets(query);
    NSMutableArray<NSDictionary *> *chunks = [NSMutableArray array];
    NSMutableDictionary<NSString *, NSMutableDictionary<NSString *, NSString *> *> *values = [NSMutableDictionary dictionary];
    MSIMETranslationCache *cache = [MSIMETranslationCache sharedCache];
    BOOL tencent = query[@"tencent_tmt"] != nil;
    BOOL niuTrans = query[@"niutrans"] != nil;
    NSString *scope = niuTrans ? [@"niutrans:" stringByAppendingString:query[@"niutrans"][@"app_id"] ?: @""] :
        tencent ? @"tencent" : [@"custom:" stringByAppendingString:query[@"custom_translation"][@"endpoint"] ?: @""];
    NSSet *glossTexts = [NSSet setWithArray:[_glossResults valueForKey:@"text"] ?: @[]];
    if ([_glossRequest isEqual:[self currentGlossRequest]]) {
        for (NSDictionary *result in _glossResults) {
            NSString *text = result[@"text"];
            NSString *translation = result[@"translation"];
            if (![text isKindOfClass:NSString.class] || ![translation isKindOfClass:NSString.class] || !translation.length) continue;
            NSMutableDictionary *byTarget = values[text];
            if (!byTarget) { byTarget = [NSMutableDictionary dictionary]; values[text] = byTarget; }
            byTarget[@"en"] = translation;
        }
    }
    for (NSString *target in targets) {
        NSMutableArray *targetCandidates = [NSMutableArray array];
        for (NSDictionary *candidate in query[@"candidates"]) {
            // A packaged/user English gloss already satisfies the English row. Keep
            // other target rows eligible when English is only the secondary language.
            if ([target isEqual:@"en"] && [glossTexts containsObject:candidate[@"text"]]) continue;
            [targetCandidates addObject:candidate];
        }
        if (!targetCandidates.count) continue;
        NSArray *plan = [MSIMEClientSession customTranslationPlan:@{@"target_language":target,
            @"candidates":targetCandidates} error:nil];
        NSMutableArray *pending = [NSMutableArray array];
        NSMutableDictionary *identities = [NSMutableDictionary dictionary];
        for (NSDictionary *item in plan) {
            NSArray *identity = @[scope, target, item[@"source_language"], item[@"target_language"], item[@"key"]];
            NSString *workKey = MSIMETranslationWorkKey(target, item[@"text"]);
            id value = [cache valueForIdentity:identity];
            if (value) {
                if ([value isKindOfClass:NSString.class]) {
                    NSMutableDictionary *byTarget = values[item[@"text"]];
                    if (!byTarget) { byTarget = [NSMutableDictionary dictionary]; values[item[@"text"]] = byTarget; }
                    byTarget[target] = value;
                }
                continue;
            }
            if (tencent || niuTrans) {
                [pending addObject:item];
                identities[workKey] = identity;
                continue;
            }
            NSDictionary *descriptor = [MSIMEClientSession customTranslationHTTPRequest:@{@"config":query[@"custom_translation"],
                @"text":item[@"key"], @"source_language":item[@"source_language"], @"target_language":item[@"target_language"]} error:nil];
            if (descriptor) {
                [pending addObject:@{@"text":item[@"text"], @"request":descriptor}];
                identities[workKey] = identity;
            } else {
                [cache rememberTranslation:nil identity:identity];
            }
        }
        for (NSUInteger offset = 0; offset < pending.count; offset += 9) {
            NSUInteger length = MIN((NSUInteger)9, pending.count - offset);
            NSArray *items = [pending subarrayWithRange:NSMakeRange(offset, length)];
            NSMutableDictionary *chunk = [@{@"target":target, @"items":items} mutableCopy];
            NSMutableDictionary *chunkIdentities = [NSMutableDictionary dictionary];
            for (NSDictionary *item in items) {
                NSString *key = MSIMETranslationWorkKey(target, item[@"text"]);
                NSArray *identity = identities[key];
                if (identity) chunkIdentities[key] = identity;
            }
            chunk[@"identities"] = chunkIdentities;
            [chunks addObject:chunk];
        }
    }
    NSMutableArray *(^combinedResults)(void) = ^NSMutableArray *{
        NSMutableArray *result = [NSMutableArray array];
        for (NSDictionary *candidate in query[@"candidates"]) {
            NSString *text = candidate[@"text"];
            NSString *translation = MSIMEJoinedTranslations(values[text], targets);
            if (translation.length) [result addObject:@{@"text":text, @"translation":translation}];
        }
        return result;
    };
    _customResults = [combinedResults() copy];
    if (_customResults.count) [self applyCandidateTranslationResults];
    if (!chunks.count) return;
    if (![[self currentCustomTranslationRequest] isEqual:query]) return;
    uint64_t epoch = _customEpoch;
    MSIMEClientSession *session = _session;
    id client = _activeClient;
    __weak MSIMEInputController *weakSelf = self;
    _customTimer = [self customTranslationTimerWithBlock:^(NSTimer *timer) {
        MSIMEInputController *owner = weakSelf;
        if (!owner || owner->_customEpoch != epoch || owner->_customTimer != timer) return;
        [timer invalidate]; owner->_customTimer = nil;
        if (owner->_session != session || owner->_activeClient != client ||
            ![[owner currentCustomTranslationRequest] isEqual:query]) return;
        owner->_customBatches = [NSMutableArray array];
        for (NSDictionary *chunk in chunks) {
            NSString *target = chunk[@"target"];
            NSArray *items = chunk[@"items"];
            NSDictionary *identities = chunk[@"identities"];
            void (^completion)(NSArray<NSDictionary *> *) = ^(NSArray<NSDictionary *> *results) {
                MSIMEInputController *latest = weakSelf;
                if (!latest || latest->_customEpoch != epoch || latest->_session != session || latest->_activeClient != client ||
                    ![[latest currentCustomTranslationRequest] isEqual:query]) return;
                for (NSDictionary *item in items) {
                    NSString *text = item[@"text"];
                    NSString *workKey = MSIMETranslationWorkKey(target, text);
                    NSString *translation = nil;
                    for (NSDictionary *result in results)
                        if ([result[@"text"] isEqual:text]) { translation = result[@"translation"]; break; }
                    [cache rememberTranslation:translation identity:identities[workKey]];
                    if (translation.length) {
                        NSMutableDictionary *byTarget = values[text];
                        if (!byTarget) { byTarget = [NSMutableDictionary dictionary]; values[text] = byTarget; }
                        byTarget[target] = translation;
                    }
                }
                // Windows saves every successful English fetch to the user gloss store (cloud_translation.cpp PersistGloss), not just the committed candidate.
                if ([target isEqual:@"en"]) [latest persistFetchedTranslations:results forQuery:query];
                latest->_customResults = [combinedResults() copy];
                [latest applyCandidateTranslationResults];
                latest->_customBatch = nil;
            };
            MSIMECustomTranslationBatch *batch = niuTrans ? [owner niuTransBatchForItems:items config:query[@"niutrans"] completion:completion]
                : tencent ? [owner tencentBatchForItems:items config:query[@"tencent_tmt"] completion:completion]
                : [owner customBatchForItems:items completion:completion];
            owner->_customBatch = batch;
            [owner->_customBatches addObject:batch];
            [batch start];
        }
    }];
}
- (void)cancelCandidateGloss {
    ++_glossEpoch;
    [_glossQueue cancelAllOperations];
    _glossRequest = nil;
    _glossResults = nil;
}
- (NSDictionary *)currentGlossRequest {
    if (!_activeClient || !_session || _focusPending || _appearance.englishMode ||
        (_appearance && !_appearance.candidateTranslations && !_appearance.candidateEnglishGloss) ||
        (_glossEnabled && !_glossEnabled.boolValue && !_appearance.candidateEnglishGloss)) return nil;
    NSDictionary *query = [_session translationQueryWithError:nil];
    NSArray *targets = MSIMETranslationTargets(query);
    if (!query || ![targets containsObject:@"en"]) return nil;
    if (_glossTargetLanguages && ![_glossTargetLanguages isEqual:targets]) return nil;
    NSDictionary *view = [_session viewWithError:nil];
    // Windows suppresses candidate translations in Japanese, including a
    // temporary Japanese composition whose view retains its original scheme.
    if ([view[@"scheme"] isEqual:@3] || [view[@"local_mode"] isEqual:@"temporary_japanese"]) return nil;
    if (![view[@"generation"] isEqual:query[@"generation"]]) return nil;
    NSMutableArray *candidates = [NSMutableArray array];
    for (NSDictionary *candidate in view[@"candidates"])
        if ([candidate[@"text"] isKindOfClass:NSString.class] && [candidate[@"source"] isKindOfClass:NSNumber.class])
            [candidates addObject:@{@"text":candidate[@"text"], @"source":candidate[@"source"]}];
    return candidates.count ? @{@"generation":query[@"generation"], @"target_languages":targets,
        @"candidates":[candidates copy],
        @"directory":_preferencesDirectory ?: @""} : nil;
}
- (NSDictionary *)readCandidateGloss:(NSDictionary *)request resources:(NSString *)resources {
    return [MSIMEClientSession candidateGlossRequest:@{@"generation":request[@"generation"],
        @"candidates":request[@"candidates"]} resources:resources error:nil];
}
+ (dispatch_queue_t)learnedTranslationQueue {
    static dispatch_queue_t queue;
    static dispatch_once_t once;
    dispatch_once(&once, ^{ queue = dispatch_queue_create("msime.learned-translations", DISPATCH_QUEUE_SERIAL); });
    return queue;
}
- (NSArray *)learnedTranslationItems:(NSArray *)candidates results:(NSArray *)results {
    NSArray *plan = [MSIMEClientSession customTranslationPlan:@{@"target_language":@"en", @"candidates":candidates} error:nil];
    NSMutableArray *items = [NSMutableArray array];
    for (NSDictionary *item in plan) {
        NSMutableDictionary *entry = [@{@"text":item[@"text"], @"direction":[item[@"source_language"] isEqual:@"en"]
            ? @"english_to_chinese" : @"chinese_to_english"} mutableCopy];
        if (results) {
            for (NSDictionary *result in results)
                if ([result[@"text"] isEqual:item[@"text"]]) { entry[@"translation"] = result[@"translation"]; break; }
            if (!entry[@"translation"]) continue;
        }
        [items addObject:entry];
    }
    return items;
}
- (void)persistFetchedTranslations:(NSArray<NSDictionary *> *)results forQuery:(NSDictionary *)query {
    NSString *directory = query[@"directory"] ?: _preferencesDirectory;
    if (![MSIMETranslationTargets(query) containsObject:@"en"] || ![directory isKindOfClass:NSString.class] || !directory.isAbsolutePath) return;
    // Reuse the query's candidates so each keeps the Engine source the plan requires; a bare {text:} would be rejected.
    NSMutableArray *candidates = [NSMutableArray array];
    for (NSDictionary *candidate in query[@"candidates"])
        for (NSDictionary *result in results)
            if ([result[@"text"] isEqual:candidate[@"text"]] && [result[@"translation"] isKindOfClass:NSString.class] && [result[@"translation"] length]) {
                [candidates addObject:candidate];
                break;
            }
    if (!candidates.count) return;
    NSArray *items = [self learnedTranslationItems:candidates results:results];
    if (!items.count) return;
    NSDictionary *request = @{ @"directory":[directory copy], @"generation":query[@"generation"] ?: @0,
        @"target_language":@"en", @"action":@"remember", @"items":items};
    dispatch_async([MSIMEInputController learnedTranslationQueue], ^{
        [MSIMEClientSession learnedTranslationRequest:request error:nil];
    });
}
- (void)persistCommittedCandidateTranslation:(NSString *)text {
    if (![text isKindOfClass:NSString.class] || !text.length) return;
    // Provider glosses are already saved when they are fetched; only hosted account glosses wait for the commit.
    if (_customQuery) return;
    NSDictionary *query = _accountGlossRequest;
    NSArray *targets = query ? (query[@"target_languages"] ?: @[]) : @[];
    NSString *directory = query[@"directory"] ?: _preferencesDirectory;
    if (!directory.isAbsolutePath || ![targets containsObject:@"en"]) return;
    NSDictionary *match = nil;
    for (NSDictionary *entry in _accountGlossResults)
        if ([entry[@"text"] isEqual:text] && [entry[@"translation"] isKindOfClass:NSString.class] && [entry[@"translation"] length]) {
            match = entry;
            break;
        }
    if (!match) return;
    // The plan rejects a candidate without its Engine source, so reuse the one the query was built from rather than synthesising a bare {text:}. A synthesised candidate parses as invalid, the plan comes back empty, and nothing is ever written to the glossary.
    NSDictionary *committed = nil;
    for (NSDictionary *candidate in query[@"candidates"])
        if ([candidate[@"text"] isEqual:text]) { committed = candidate; break; }
    if (!committed) return;
    NSArray *items = [self learnedTranslationItems:@[committed] results:@[match]];
    if (!items.count) return;
    NSDictionary *request = @{ @"directory":[directory copy], @"generation":query[@"generation"] ?: @0,
        @"target_language":@"en", @"action":@"remember", @"items":items};
    dispatch_async([MSIMEInputController learnedTranslationQueue], ^{
        [MSIMEClientSession learnedTranslationRequest:request error:nil];
    });
}
- (void)synchronizeCandidateGloss {
    NSDictionary *request = [self currentGlossRequest];
    if (!request) { [self cancelCandidateGloss]; return; }
    if ([_glossRequest isEqual:request]) return;
    [self cancelCandidateGloss];
    _glossRequest = request;
    NSString *resources = [_session.hostOptions[@"resources"] copy];
    BOOL hasResources = [resources isKindOfClass:NSString.class] && resources.isAbsolutePath;
    NSString *directory = request[@"directory"];
    if (!hasResources && !directory.isAbsolutePath) { _glossResults = @[]; return; }
    NSArray *learnedItems = [self learnedTranslationItems:request[@"candidates"] results:nil];
    if (!_glossQueue) { _glossQueue = [NSOperationQueue new]; _glossQueue.maxConcurrentOperationCount = 1; _glossQueue.qualityOfService = NSQualityOfServiceUtility; }
    const uint64_t epoch = _glossEpoch;
    MSIMEClientSession *session = _session;
    id client = _activeClient;
    __weak MSIMEInputController *weakSelf = self;
    [_glossQueue addOperationWithBlock:^{
        id reader = hasResources ? weakSelf : nil; // id: handed to MSIMEReleaseControllerOnMain
        NSDictionary *result = reader ? [reader readCandidateGloss:request resources:resources] : nil;
        MSIMEReleaseControllerOnMain(&reader);
        if (result && ![result[@"generation"] isEqual:request[@"generation"]]) return;
        NSMutableArray *translations = [result[@"translations"] mutableCopy] ?: [NSMutableArray array];
        if (directory.isAbsolutePath && learnedItems.count) {
            __block NSDictionary *learned;
            dispatch_sync([MSIMEInputController learnedTranslationQueue], ^{
                learned = [MSIMEClientSession learnedTranslationRequest:@{@"directory":directory,
                    @"generation":request[@"generation"], @"target_language":@"en", @"action":@"lookup", @"items":learnedItems} error:nil];
            });
            for (NSDictionary *entry in learned[@"translations"]) {
                NSUInteger index = [translations indexOfObjectPassingTest:^BOOL(NSDictionary *existing, NSUInteger position, BOOL *stop) {
                    (void)position; (void)stop;
                    return [entry[@"text"] isEqual:existing[@"text"]];
                }];
                // Learned records override packaged glosses, matching Engine's
                // user glossary overlay and Windows INSERT OR REPLACE semantics.
                if (index == NSNotFound) [translations addObject:entry];
                else translations[index] = entry;
            }
        }
        dispatch_async(dispatch_get_main_queue(), ^{
            MSIMEInputController *current = weakSelf;
            if (!current || current->_glossEpoch != epoch || current->_session != session || current->_activeClient != client ||
                ![[current currentGlossRequest] isEqual:request] || (result && ![result[@"generation"] isEqual:request[@"generation"]])) return;
            current->_glossResults = [translations copy];
            [current applyCandidateTranslationResults];
        });
    }];
}
- (void)cancelTargetGloss {
    ++_targetGlossEpoch;
    [_targetGlossQueue cancelAllOperations];
    _targetGlossRequest = nil;
    _targetGlossResults = nil;
}
// The non-English targets with an installed offline dictionary. English keeps its own path above, which also overlays the user's learned glossary.
- (NSDictionary *)currentTargetGlossRequest {
    if (!_activeClient || !_session || _focusPending || _appearance.englishMode ||
        (_appearance && !_appearance.candidateTranslations && !_appearance.candidateEnglishGloss) ||
        (_glossEnabled && !_glossEnabled.boolValue && !_appearance.candidateEnglishGloss)) return nil;
    NSDictionary *query = [_session translationQueryWithError:nil];
    NSArray *targets = MSIMETranslationTargets(query);
    if (_glossTargetLanguages && ![_glossTargetLanguages isEqual:targets]) return nil;
    NSArray *installed = [query[@"offline_gloss_languages"] isKindOfClass:NSArray.class] ? query[@"offline_gloss_languages"] : @[];
    NSMutableArray<NSString *> *languages = [NSMutableArray array];
    for (NSString *target in targets)
        if (![target isEqual:@"en"] && [installed containsObject:target]) [languages addObject:target];
    if (!languages.count) return nil;
    NSDictionary *view = [_session viewWithError:nil];
    if ([view[@"scheme"] isEqual:@3] || [view[@"local_mode"] isEqual:@"temporary_japanese"]) return nil;
    if (![view[@"generation"] isEqual:query[@"generation"]]) return nil;
    NSMutableArray *candidates = [NSMutableArray array];
    for (NSDictionary *candidate in view[@"candidates"])
        if ([candidate[@"text"] isKindOfClass:NSString.class] && [candidate[@"source"] isKindOfClass:NSNumber.class])
            [candidates addObject:@{@"text":candidate[@"text"], @"source":candidate[@"source"]}];
    return candidates.count ? @{@"generation":query[@"generation"], @"target_languages":targets,
        @"offline_languages":[languages copy], @"candidates":[candidates copy]} : nil;
}
- (NSDictionary *)readTargetGloss:(NSDictionary *)request language:(NSString *)language resources:(NSString *)resources {
    return [MSIMEClientSession candidateGlossRequest:@{@"generation":request[@"generation"],
        @"target_language":language, @"candidates":request[@"candidates"]} resources:resources error:nil];
}
- (void)synchronizeTargetGloss {
    NSDictionary *request = [self currentTargetGlossRequest];
    if (!request) { [self cancelTargetGloss]; return; }
    if ([_targetGlossRequest isEqual:request]) return;
    [self cancelTargetGloss];
    _targetGlossRequest = request;
    NSString *resources = [_session.hostOptions[@"resources"] copy];
    if (![resources isKindOfClass:NSString.class] || !resources.isAbsolutePath) { _targetGlossResults = @{}; return; }
    // A queue of its own: cancelCandidateGloss drains the English queue whenever the English request changes, which would otherwise drop this read and leave the request without results.
    if (!_targetGlossQueue) { _targetGlossQueue = [NSOperationQueue new]; _targetGlossQueue.maxConcurrentOperationCount = 1; _targetGlossQueue.qualityOfService = NSQualityOfServiceUtility; }
    const uint64_t epoch = _targetGlossEpoch;
    MSIMEClientSession *session = _session;
    id client = _activeClient;
    __weak MSIMEInputController *weakSelf = self;
    [_targetGlossQueue addOperationWithBlock:^{
        NSMutableDictionary<NSString *, NSMutableDictionary<NSString *, NSString *> *> *values = [NSMutableDictionary dictionary];
        id reader = weakSelf;
        for (NSString *language in request[@"offline_languages"]) {
            NSDictionary *result = [reader readTargetGloss:request language:language resources:resources];
            if (![result[@"generation"] isEqual:request[@"generation"]]) continue;
            for (NSDictionary *entry in result[@"translations"]) {
                NSString *text = entry[@"text"];
                NSString *translation = entry[@"translation"];
                if (![text isKindOfClass:NSString.class] || ![translation isKindOfClass:NSString.class] || !translation.length) continue;
                NSMutableDictionary *byTarget = values[text];
                if (!byTarget) { byTarget = [NSMutableDictionary dictionary]; values[text] = byTarget; }
                byTarget[language] = translation;
            }
        }
        MSIMEReleaseControllerOnMain(&reader);
        dispatch_async(dispatch_get_main_queue(), ^{
            MSIMEInputController *current = weakSelf;
            if (!current || current->_targetGlossEpoch != epoch || current->_session != session || current->_activeClient != client ||
                ![[current currentTargetGlossRequest] isEqual:request]) return;
            current->_targetGlossResults = [values copy];
            [current applyCandidateTranslationResults];
        });
    }];
}

- (void)cancelCloudCandidates {
    ++_cloudEpoch;
    [_cloudTimer invalidate];
    _cloudTimer = nil;
    [self cancelSettledRerank];
    [_cloudRequest cancel];
    _cloudRequest = nil;
    _cloudQuery = nil;
}

- (MSIMECloudCandidateRequest *)cloudRequestForURL:(NSURL *)url completion:(void (^)(NSData *))completion {
    return [[MSIMECloudCandidateRequest alloc] initWithURL:url configuration:NSURLSessionConfiguration.ephemeralSessionConfiguration completion:completion];
}

/// How long the composition stands still before the larger model ranks it.
///
/// Shorter than the half second the cloud path waits, because this one is local and it changes
/// what the user is reading rather than annotating it. Long enough that ordinary typing never lets
/// it fire, which is what keeps the 24M model off the keystroke path where it measures p95 153ms
/// against a 16ms frame.
static const NSTimeInterval kSettledRerankDelay = 0.15;

- (void)cancelSettledRerank {
    [_settledTimer invalidate];
    _settledTimer = nil;
}

/// Re-rank once typing stops. Every refresh reschedules, so it only runs on a real pause.
///
/// Inert unless a settled model was installed: the shared host answers `moved: NO` immediately
/// when none is attached, which is every installation that ships one model. The window is redrawn
/// only when the order actually moved — repainting an identical candidate list on every pause is
/// a flicker with no explanation behind it.
- (void)scheduleSettledRerank {
    [self cancelSettledRerank];
    if (!_activeClient || !_session || _focusPending || _appearance.englishMode) return;
    const uint64_t epoch = _cloudEpoch;
    MSIMEClientSession *session = _session;
    id client = _activeClient;
    __weak MSIMEInputController *weakSelf = self;
    _settledTimer = [NSTimer timerWithTimeInterval:kSettledRerankDelay repeats:NO block:^(NSTimer *timer) {
        (void)timer;
        MSIMEInputController *controller = weakSelf;
        if (!controller || controller->_cloudEpoch != epoch || controller->_session != session ||
            controller->_activeClient != client || controller->_focusPending ||
            controller->_appearance.englishMode) return;
        controller->_settledTimer = nil;
        NSDictionary *applied = [session rerankSettledWithError:nil];
        // Same shape the cloud path applies — a transition carrying the refreshed view — so it
        // goes through the same method rather than a second way of updating the window.
        if ([applied[@"moved"] boolValue]) [controller apply:applied];
    }];
    [NSRunLoop.mainRunLoop addTimer:_settledTimer forMode:NSRunLoopCommonModes];
}

- (void)synchronizeCloudCandidates {
    NSDictionary *query = _activeClient && _session && !_focusPending && !_appearance.englishMode &&
        (!_appearance || _appearance.cloudCandidatesEnabled) ? [_session onlineQueryWithError:nil] : nil;
    NSString *url = query ? [MSIMEClientSession cloudRequestURLForQuery:query error:nil] : nil;
    if (!url) { [self cancelCloudCandidates]; return; }
    if ([_cloudQuery isEqual:query]) return;
    [self cancelCloudCandidates];
    _cloudQuery = [query copy];
    const uint64_t epoch = _cloudEpoch;
    MSIMEClientSession *session = _session;
    id client = _activeClient;
    __weak MSIMEInputController *weakSelf = self;
    _cloudTimer = [NSTimer timerWithTimeInterval:0.5 repeats:NO block:^(NSTimer *timer) {
        (void)timer;
        MSIMEInputController *controller = weakSelf;
        if (!controller || controller->_cloudEpoch != epoch || controller->_session != session ||
            controller->_activeClient != client || controller->_focusPending || controller->_appearance.englishMode ||
            (controller->_appearance && !controller->_appearance.cloudCandidatesEnabled) ||
            ![[session onlineQueryWithError:nil] isEqual:query]) return;
        controller->_cloudTimer = nil;
        controller->_cloudRequest = [controller cloudRequestForURL:[NSURL URLWithString:url] completion:^(NSData *body) {
            MSIMEInputController *current = weakSelf;
            if (!current || current->_cloudEpoch != epoch || current->_session != session ||
                current->_activeClient != client || current->_focusPending || current->_appearance.englishMode ||
                (current->_appearance && !current->_appearance.cloudCandidatesEnabled) ||
                ![[session onlineQueryWithError:nil] isEqual:query]) return;
            current->_cloudRequest = nil;
            if (!body) {
                current->_cloudQuery = nil;
                return;
            }
            NSDictionary *result = [session applyCloudResponse:body query:query error:nil];
            const BOOL applied = [result[@"applied"] boolValue];
            if (applied) {
                // Remember the post-apply identity so rendering does not re-request this result.
                current->_cloudQuery = [[session onlineQueryWithError:nil] copy];
                [current apply:result];
            } else {
                // A valid response can still be rejected when the local candidate page has
                // disappeared (or the provider returned no usable candidate). Leave the
                // query eligible for a later render after local candidates are restored.
                current->_cloudQuery = nil;
            }
        }];
        [controller->_cloudRequest start];
    }];
    [NSRunLoop.mainRunLoop addTimer:_cloudTimer forMode:NSRunLoopCommonModes];
}

- (void)ensureAppearance {
    if (_appearance) return;
    _wubiCodeHintEnabled = YES;
    _appearance = [MSIMEAppearancePreferences sharedPreferences];
    [[NSNotificationCenter defaultCenter] addObserver:self selector:@selector(appearanceChanged:) name:MSIMEAppearanceDidChangeNotification object:_appearance];
    [[NSNotificationCenter defaultCenter] addObserver:self selector:@selector(translationPreferencesSaved:) name:MSIMETranslationPreferencesDidSaveNotification object:_appearance];
    [[NSNotificationCenter defaultCenter] addObserver:self selector:@selector(appearanceChanged:) name:MSIMEVoiceSettingsDidChangeNotification object:nil];
    [[NSNotificationCenter defaultCenter] addObserver:self selector:@selector(voiceProviderSettingsChanged:) name:MSIMEVoiceProviderSettingsDidChangeNotification object:nil];
    [[NSNotificationCenter defaultCenter] addObserver:self selector:@selector(accountCandidateTranslationsDidArrive:) name:@"MSIMEBackendCandidateTranslationsDidArrive" object:nil];
    [[NSNotificationCenter defaultCenter] addObserver:self selector:@selector(onDeviceCandidateTranslationsDidArrive:) name:@"MSIMEBackendOnDeviceTranslationsDidArrive" object:nil];
    [NSDistributedNotificationCenter.defaultCenter addObserver:self
        selector:@selector(typingStatisticsEnabledChanged:)
        name:MSIMETypingStatisticsEnabledChangedNotification object:nil
        suspensionBehavior:NSNotificationSuspensionBehaviorDeliverImmediately];
    _globalVoiceHotkeyMonitor = [NSEvent addGlobalMonitorForEventsMatchingMask:NSEventMaskKeyDown handler:[self globalVoiceHotkeyHandler]];
}
- (void)typingStatisticsEnabledChanged:(NSNotification *)notification {
    NSNumber *enabled = [notification.userInfo[@"enabled"] isKindOfClass:NSNumber.class]
        ? notification.userInfo[@"enabled"] : nil;
    if (enabled) MSIMETypingStatisticsEnabled.store(enabled.boolValue, std::memory_order_relaxed);
    else MSIMEReloadTypingStatisticsEnabled(_preferencesDirectory);
}
- (void (^)(NSEvent *))globalVoiceHotkeyHandler {
    __weak MSIMEInputController *weakSelf = self;
    return ^(NSEvent *event) {
        if (event.keyCode != 101 || event.isARepeat || (event.modifierFlags & (NSEventModifierFlagControl | NSEventModifierFlagShift | NSEventModifierFlagOption | NSEventModifierFlagCommand)) != NSEventModifierFlagControl) return;
        MSIMEInputController *controller = weakSelf;
        if (!controller || !controller->_activeClient || !MSIMEVoiceInputEnabled(NSUserDefaults.standardUserDefaults) || ([NSUserDefaults.standardUserDefaults objectForKey:@"MSIMEClientVoiceHotkeyCtrlF9"] != nil && ![NSUserDefaults.standardUserDefaults boolForKey:@"MSIMEClientVoiceHotkeyCtrlF9"])) return;
        // AppKit delivers event-monitor handlers on the main thread. Handle
        // this gesture now: another main-queue hop could toggle a new client
        // or recording after focus or voice state has changed.
        [controller toggleVoiceInput:nil];
    };
}
- (void)resetCandidateAnchor {
    _candidateAnchorCaret = NSZeroRect;
    _candidateAnchorValid = NO;
}
- (NSRect)candidateCaretForRendering:(NSRect)reported {
    const BOOL followCursor = _appearance == nil || _appearance.candidateFollowCursor;
    if (!_candidateFollowCursorModeKnown || _candidateFollowCursorMode != followCursor) {
        [self resetCandidateAnchor];
        _candidateFollowCursorMode = followCursor;
        _candidateFollowCursorModeKnown = YES;
    }
    if (followCursor) return reported;
    if (!_candidateAnchorValid && MSIMEValidCaret(reported)) {
        _candidateAnchorCaret = reported;
        _candidateAnchorValid = YES;
    }
    return _candidateAnchorValid ? _candidateAnchorCaret : reported;
}
- (void)voiceProviderSettingsChanged:(NSNotification *)notification { (void)notification; _preferenceLoadState.reset(); _voicePermissionToken = nil; _voiceHoldShortcut.reset(); [self cancelLiveVoiceInput]; [self cancelDoubaoVoiceInput]; [self cancelHTTPVoiceInput]; if (_voiceService.active) [_voiceService cancelWithError:nil]; [_voiceOverlay dismissFailure]; [self persistAppearancePreferences]; }
- (void)translationPreferencesSaved:(NSNotification *)notification {
    _preferenceLoadState.reset();
    [self applySharedToolbarPreferences:notification.userInfo];
    _view = [_session viewWithError:nil] ?: _view;
    [self refreshFloatingToolbarState];
    if (_activeClient) { [self renderCandidates]; [self reloadPreferences]; }
}
- (void)refreshFloatingToolbarState {
    if (!_toolbar || !_appearance) return;
    const BOOL englishCandidateMode = [_view[@"dedicated_english"] boolValue] && !_appearance.englishMode;
    const BOOL japaneseInputMode = [_view[@"scheme"] integerValue] == 3;
    [_toolbar updateEnglishInputMode:_appearance.englishMode
             englishCandidateMode:englishCandidateMode
                   japaneseInputMode:japaneseInputMode
                            capsLock:_capsLock
              chinesePunctuationEnabled:_appearance.runtimeChinesePunctuation
                       fullWidthEnabled:_appearance.runtimeFullWidthInput
        traditionalChineseOutputEnabled:_appearance.traditionalOutput];
}
- (void)appearanceChanged:(NSNotification *)notification {
    (void)notification;
    _preferenceLoadState.reset(); // Local edits invalidate older disk reads.
    if (!_appearance.cloudCandidatesEnabled) [self cancelCloudCandidates];
    if (_appearance) _glossEnabled = @(_appearance.candidateTranslations);
    if (_appearance && !_appearance.candidateTranslations) {
        [self cancelCustomTranslations];
        if (!_appearance.candidateEnglishGloss) { [self cancelCandidateGloss]; [self cancelTargetGloss]; }
    }
    if (_appearance && !_appearance.candidateTranslations && !_appearance.candidateEnglishGloss) {
        NSDictionary *view = [_session viewWithError:nil];
        if (view) {
            NSDictionary *cleared = [_session applyTranslations:@[] generation:[view[@"generation"] unsignedLongLongValue] error:nil];
            if (cleared[@"view"]) _view = cleared[@"view"];
        }
    }
    if (_appearance.englishMode && _activeClient && ([_view[@"editing_text"] length] || [_view[@"candidates"] count])) {
        [self apply:[_session command:MSIME_FINISH_COMPOSITION error:nil]];
    }
    [self syncPageSize];
    [self syncPunctuation];
    [self syncCharacterWidth];
    [_toolbar applyLightSkin:[_appearance resolvedSkinForDark:NO].tokens darkSkin:[_appearance resolvedSkinForDark:YES].tokens];
    [_toolbar applyLightToolbarSkin:msime::mac::ToolbarSkinTokens(_appearance.skinID.UTF8String, NO)
                            darkSkin:msime::mac::ToolbarSkinTokens(_appearance.skinID.UTF8String, YES)];
    [self refreshFloatingToolbarState];
    if (_activeClient) [self renderCandidates];
    if (_activeClient) [_toolbar setVisible:_appearance.floatingToolbarEnabled forDelegate:self];
    // The settings window can move the scheme in or out of japanese, which moves the menu bar between 中 and 日.
    if (_activeClient) [self syncSystemInputModeForClient:_activeClient];
    [self persistAppearancePreferences];
}
- (void)persistAppearancePreferences {
    if (!_preferencesDirectory) return;
    if (!_preferenceSaveState.request()) return;
    NSString *directory = [_preferencesDirectory copy];
    // Capture all host-owned fields together on the main thread. Both CAS
    // attempts use this same snapshot; later changes schedule a fresh save.
    NSDictionary *overrides = [_appearance sharedPreferencesByMerging:@{}];
    __weak MSIMEInputController *weakSelf = self;
    dispatch_async(dispatch_get_global_queue(QOS_CLASS_UTILITY, 0), ^{
        NSError *loadError = nil;
        NSDictionary *snapshot = [MSIMEClientSession loadPreferencesInDirectory:directory error:&loadError];
        NSDictionary *preferences = snapshot ? MSIMEMergePreferenceSnapshot(snapshot[@"preferences"], overrides) : nil;
        uint64_t revision = [snapshot[@"revision"] unsignedLongLongValue];
        NSError *saveError = nil;
        NSDictionary *saved = nil;
        if (preferences) {
            saved = [MSIMEClientSession savePreferencesInDirectory:directory expectedRevision:revision snapshot:@{ @"format_version": @1, @"revision": @(revision), @"preferences": preferences } error:&saveError];
        }
        if (!saved && snapshot) {
            NSError *retryLoadError = nil;
            NSDictionary *latest = [MSIMEClientSession loadPreferencesInDirectory:directory error:&retryLoadError];
            NSDictionary *latestPreferences = latest ? MSIMEMergePreferenceSnapshot(latest[@"preferences"], overrides) : nil;
            if (latestPreferences) {
                saveError = nil;
                saved = [MSIMEClientSession savePreferencesInDirectory:directory expectedRevision:[latest[@"revision"] unsignedLongLongValue] snapshot:@{ @"format_version": @1, @"revision": latest[@"revision"] ?: @0, @"preferences": latestPreferences } error:&saveError];
            }
        }
        dispatch_async(dispatch_get_main_queue(), ^{
            MSIMEInputController *controller = weakSelf;
            if (!controller) return;
            const bool again = controller->_preferenceSaveState.finish();
            if (saved && !saveError) [controller reloadPreferences];
            else msime_macos_diagnostic_write("preferences_save_failed");
            if (again) [controller persistAppearancePreferences];
        });
    });
}
- (void)syncPageSize {
    if (!_session) return;
    [self ensureAppearance];
    NSUInteger size = _appearance.pageSize;
    if (_requestedPageSize == size) return;
    NSDictionary *result = [_session setCandidatePageSize:(uint8_t)size error:nil];
    if (result) {
        _requestedPageSize = size;
        _view = result[@"view"];
    }
}
- (void)syncPunctuation {
    if (!_session) return;
    NSDictionary *view = [_session setChinesePunctuationEnabled:_appearance.runtimeChinesePunctuation error:nil];
    if (view) [self apply:@{@"view":view}];
    view = [_session setPairedPunctuationEnabled:_appearance.pairedPunctuation && !MSIMEPairedPunctuationExcludedHost() error:nil];
    if (view) [self apply:@{@"view":view}];
    view = [_session setPunctuationLock:_appearance.punctuationLock error:nil];
    if (view) [self apply:@{@"view":view}];
}
- (void)syncCharacterWidth {
    if (!_session) return;
    NSDictionary *view = [_session setCharacterWidthFull:_appearance.runtimeFullWidthInput error:nil];
    if (view) [self apply:@{@"view":view}];
}
- (NSMenu *)menu {
    [self ensureAppearance];
    NSMenu *menu = [[NSMenu alloc] initWithTitle:@"水杉输入法"];
    menu.autoenablesItems = NO;
    ApplyMetasequoiaMenuTheme(menu, _menuThemePreferences ?: @{});
    for (NSUInteger mode = 0; mode < 2; ++mode) {
        NSMenuItem *item = [[NSMenuItem alloc] initWithTitle:mode ? @"英文输入" : @"中文输入" action:mode ? @selector(selectEnglishMode:) : @selector(selectChineseMode:) keyEquivalent:@""];
        item.target = self;
        item.state = (_appearance.englishMode == (mode == 1) && (mode == 1 || ![_view[@"dedicated_english"] isEqual:@YES])) ? NSControlStateValueOn : NSControlStateValueOff;
        [menu addItem:item];
    }
    NSMenuItem *englishCandidates = [[NSMenuItem alloc] initWithTitle:@"英文候选模式" action:@selector(toggleDedicatedEnglishMode:) keyEquivalent:@"e"];
    englishCandidates.target = self;
    englishCandidates.keyEquivalentModifierMask = NSEventModifierFlagControl | NSEventModifierFlagShift;
    englishCandidates.state = !_appearance.englishMode && [_view[@"dedicated_english"] isEqual:@YES] ? NSControlStateValueOn : NSControlStateValueOff;
    [menu addItem:englishCandidates];
    [menu addItem:NSMenuItem.separatorItem];
    for (NSUInteger script = 0; script < 2; ++script) {
        NSMenuItem *item = [[NSMenuItem alloc] initWithTitle:script ? @"繁体输出" : @"简体输出" action:script ? @selector(selectTraditionalOutput:) : @selector(selectSimplifiedOutput:) keyEquivalent:@""];
        item.target = self;
        item.state = _appearance.traditionalOutput == (script == 1) ? NSControlStateValueOn : NSControlStateValueOff;
        [menu addItem:item];
    }
    [menu addItem:NSMenuItem.separatorItem];
    // The floating toolbar is one click from the language bar in the reference - the first item of
    // its tray menu, with a tick showing the state. Here it could only be reached by opening the
    // settings window and finding a checkbox, which is a long way round for something the user
    // turns on and off while typing.
    NSMenuItem *toolbar = [[NSMenuItem alloc] initWithTitle:@"悬浮工具栏" action:@selector(toggleFloatingToolbar:) keyEquivalent:@""];
    toolbar.target = self;
    toolbar.state = _appearance.floatingToolbarEnabled ? NSControlStateValueOn : NSControlStateValueOff;
    [menu addItem:toolbar];

    // Keep the live input tools one click away. Account, dictionary, update and support destinations
    // stay in the settings window; listing those management pages here made this menu taller than
    // the screen, but hiding the tools behind a second submenu made the useful part too hard to reach.
    NSMenuItem *emoji = [[NSMenuItem alloc] initWithTitle:@"水杉表情面板…" action:@selector(showEmoji:) keyEquivalent:@""];
    emoji.target = self;
    [menu addItem:emoji];
    NSMenuItem *keyboard = [[NSMenuItem alloc] initWithTitle:@"水杉屏幕键盘…" action:@selector(showScreenKeyboard:) keyEquivalent:@""];
    keyboard.target = self;
    [menu addItem:keyboard];
    NSMenuItem *handwriting = [[NSMenuItem alloc] initWithTitle:@"手写输入…" action:@selector(showHandwriting:) keyEquivalent:@""];
    handwriting.target = self;
    [menu addItem:handwriting];
    NSMenuItem *voice = [[NSMenuItem alloc] initWithTitle:@"开始/结束语音输入" action:@selector(showVoicePanel) keyEquivalent:@""];
    voice.target = self;
    [menu addItem:voice];

    [menu addItem:NSMenuItem.separatorItem];
    NSMenuItem *settings = [[NSMenuItem alloc] initWithTitle:@"水杉输入法设置…" action:@selector(showAppearance:) keyEquivalent:@""];
    settings.target = self;
    [menu addItem:settings];
    // The reference tray menu ends with 关于, which opens the settings window on its about page. One row
    // does not make the menu too tall, and without it the version and licence notices are only reachable
    // by knowing to open settings and scroll to the last page.
    NSMenuItem *about = [[NSMenuItem alloc] initWithTitle:@"关于水杉输入法…" action:@selector(showAbout:) keyEquivalent:@""];
    about.target = self;
    [menu addItem:about];
    return menu;
}
- (void)showAccount:(id)sender {
    (void)sender;
    // The shared settings page owns the account surface, the same as every other entry in this menu. The
    // bundled SwiftUI window stays as the fallback for a host without the desktop application, which is
    // what it was before this route existed - it was simply being opened first.
    MSIMEOpenDesktopRoute(@"settings:account", NSWorkspace.sharedWorkspace, ^{
        if (MSIMEOpenBackendAccount(NSClassFromString(@"MSIMEBackendAccountWindow"))) return;
        NSAlert *alert = [[NSAlert alloc] init];
        alert.messageText = @"账户窗口暂不可用";
        alert.informativeText = @"请重新启动输入法；若仍无法打开，请检查安装是否完整。";
        [alert runModal];
    });
}
- (void)showCloudClipboard:(id)sender {
    NSRunningApplication *application = NSWorkspace.sharedWorkspace.frontmostApplication;
    if (_activeClient && application &&
        application.processIdentifier != NSProcessInfo.processInfo.processIdentifier &&
        MSIMEToolApplicationMatches([(id<IMKTextInput>)_activeClient bundleIdentifier], application.bundleIdentifier)) {
        [self showSharedTextTool:@"cloud-clipboard" options:[self runtimeOptions] bridge:nil];
        return;
    }
    MSIMEOpenDesktopCloudClipboard(MSIMERuntimeOptionsPath(), NSWorkspace.sharedWorkspace, ^{
        if (!MSIMEOpenBackendClipboard(NSClassFromString(@"MSIMEBackendAccountWindow"))) [self showAccount:sender];
    });
}
- (void)showCloudDictionary:(id)sender { (void)sender; MSIMEOpenDesktopCloudDictionary(MSIMERuntimeOptionsPath(), NSWorkspace.sharedWorkspace, ^{ [self showAccount:nil]; }); }
- (void)showHandwriting:(id)sender {
    (void)sender;
    Class bridge = NSClassFromString(@"MSIMEBackendWindowBridge");
    id shared = [bridge respondsToSelector:@selector(shared)] ? [bridge performSelector:@selector(shared)] : nil;
    if (![shared respondsToSelector:@selector(showHandwritingWithSelectionAttempt:)]) { [self showAccount:nil]; return; }
    if (!_activeClient) {
        MSIMEOpenDesktopRoute(@"handwriting", NSWorkspace.sharedWorkspace, ^{ [shared performSelector:@selector(showHandwriting)]; });
        return;
    }
    [self showSharedTextTool:@"handwriting" options:[self runtimeOptions] bridge:shared];
}
- (void)showEmoji:(id)sender {
    (void)sender;
    Class bridge = NSClassFromString(@"MSIMEBackendWindowBridge");
    id shared = [bridge respondsToSelector:@selector(shared)] ? [bridge performSelector:@selector(shared)] : nil;
    NSDictionary *options = [self runtimeOptions];
    // The shared Tauri/Swift surface is the normal path, but the input source
    // can be alive before that bridge is loaded (or while the desktop bundle
    // is being repaired). Keep the macOS character viewer as a useful,
    // platform-native fallback instead of silently dropping the menu action.
    if (![shared respondsToSelector:@selector(showEmojiWithOptions:selectionAttempt:)]) {
        [self showSystemCharacterPalette];
        return;
    }
    [self showSharedTextTool:@"emoji" options:options bridge:shared];
}
- (void)showVoicePanel {
    NSString *providerSocket = MSIMEVoiceProviderSocket();
    if (![providerSocket isKindOfClass:NSString.class] || !providerSocket.isAbsolutePath ||
        ![[NSFileManager defaultManager] fileExistsAtPath:providerSocket]) {
        // Direct macOS Speech/HTTP/Doubao providers remain native. The shared
        // panel is only advertised when a session-scoped provider socket can
        // actually serve its recognition requests.
        [self toggleVoiceInput:nil];
        return;
    }
    NSRunningApplication *application = NSWorkspace.sharedWorkspace.frontmostApplication;
    id targetClient = _activeClient;
    if (!targetClient || !application || application.processIdentifier == NSProcessInfo.processInfo.processIdentifier ||
        !MSIMEToolApplicationMatches([(id<IMKTextInput>)targetClient bundleIdentifier], application.bundleIdentifier)) {
        return;
    }
    // Match the native voice providers and the Windows session: voice starts
    // from a committed Engine state, so panel text cannot be appended to a
    // stale preedit or be resent after the panel closes.
    if (_session) {
        NSDictionary *finished = [_session command:MSIME_FINISH_COMPOSITION error:nil];
        if (!finished) {
            [self toggleVoiceInput:nil];
            return;
        }
        [self apply:finished];
    }
    [_desktopInputSession stop];
    _desktopInputSession = nil;
    if (_desktopEmojiCompletion) {
        _desktopEmojiCompletion(NO);
        _desktopEmojiCompletion = nil;
    }
    __weak MSIMEInputController *weakSelf = self;
    MSIMEDesktopInputSession *session = [[MSIMEDesktopInputSession alloc]
        initWithTargetPID:application.processIdentifier
        launchTime:application.launchDate.timeIntervalSince1970
        handler:^(NSString *text, double deadline, MSIMEPanelTextCompletion completion) {
            (void)deadline;
            MSIMEInputController *controller = weakSelf;
            if (!controller || application.terminated || controller->_activeClient != targetClient ||
                NSWorkspace.sharedWorkspace.frontmostApplication.processIdentifier != application.processIdentifier ||
                !text.length) {
                completion(NO);
                return;
            }
            NSDictionary *voiceOptions = MSIMEVoiceProviderOptions(@{}, NSUserDefaults.standardUserDefaults);
            MSIMEVoiceCommitRoute route = MSIMECaptureVoiceCommit(voiceOptions[@"commit_mode"], targetClient);
            // Text from outside the key path lands without passing the shadow, so the document answers next.
            [controller invalidateSmartPunctuationShadow];
            const MSIMEVoiceCommitOutcome outcome = route.deliver(text);
            if (outcome == MSIMEVoiceCommitOutcome::stale) {
                // The external editor lost focus after the panel submitted. The
                // route may have posted a partial result, so never retry through IMK.
                completion(NO);
                return;
            }
            if (outcome == MSIMEVoiceCommitOutcome::posted) {
                NSDictionary *options = controller->_session ? MSIMEStatisticsHostOptions(controller->_session) : @{};
                MSIMERecordTypingStatistics(controller->_preferencesDirectory ?: options[@"preferences_directory"],
                                            text, msime::mac::TypingSource::Voice);
                completion(YES);
                return;
            }
            @try {
                [targetClient insertText:text replacementRange:NSMakeRange(NSNotFound, 0)];
                NSDictionary *options = controller->_session ? MSIMEStatisticsHostOptions(controller->_session) : @{};
                MSIMERecordTypingStatistics(controller->_preferencesDirectory ?: options[@"preferences_directory"],
                                            text, msime::mac::TypingSource::Voice);
                completion(YES);
            } @catch (NSException *) {
                completion(NO);
            }
        }];
    _desktopInputSession = session;
    dispatch_block_t fallback = ^{
        [session stop];
        MSIMEInputController *controller = weakSelf;
        if (controller && controller->_activeClient == targetClient) [controller toggleVoiceInput:nil];
    };
    if (!session) {
        fallback();
        return;
    }
    MSIMEOpenDesktopRouteWithContext(@"voice", MSIMERuntimeOptionsPath(), session.launchEnvironment,
        NSWorkspace.sharedWorkspace, ^(NSRunningApplication *peer) {
            [session authorizePID:peer.processIdentifier stillValid:^BOOL { return !peer.terminated; }];
        }, fallback);
}
- (void)showSharedTextTool:(NSString *)route options:(NSDictionary *)options bridge:(id)shared {
    NSRunningApplication *application = NSWorkspace.sharedWorkspace.frontmostApplication;
    if (!_activeClient || !application || application.processIdentifier == NSProcessInfo.processInfo.processIdentifier) return;
    if (!MSIMEToolApplicationMatches([(id<IMKTextInput>)_activeClient bundleIdentifier], application.bundleIdentifier)) return;
    [_desktopInputSession stop];
    _desktopInputSession = nil;
    if (_desktopEmojiCompletion) { _desktopEmojiCompletion(NO); _desktopEmojiCompletion = nil; }
    const uint64_t token = _emojiReturn.capture(_activeClient);
    __weak MSIMEInputController *weakSelf = self;
    BOOL (^selection)(NSString *) = ^BOOL(NSString *text) {
        MSIMEInputController *controller = weakSelf;
        if (!controller || application.terminated ||
            !controller->_emojiReturn.queue(text, token, NSProcessInfo.processInfo.systemUptime)) return NO;
        if (controller->_desktopEmojiCompletion)
            controller->_emojiReturn.deadline = std::min(controller->_emojiReturn.deadline, controller->_desktopEmojiDeadline);
        // The Swift bridge closes its window before this activation is executed.
        dispatch_async(dispatch_get_main_queue(), ^{
            MSIMEInputController *current = weakSelf;
            if (!current || current->_emojiReturn.generation != token || !current->_emojiReturn.pending) return;
            if (current->_desktopEmojiCompletion &&
                (![current->_desktopInputSession isAuthorizedPeerAlive] ||
                 NSWorkspace.sharedWorkspace.frontmostApplication.processIdentifier != application.processIdentifier)) {
                if (current->_emojiReturn.fail(token)) [current reportEmojiDeliveryFailure];
                return;
            }
            if (NSWorkspace.sharedWorkspace.frontmostApplication.processIdentifier == application.processIdentifier &&
                current->_activeClient && current->_activeClient == current->_emojiReturn.target) {
                [current commitPendingEmojiForClient:current->_activeClient];
                return;
            }
            if (!MSIMEActivateToolApplication(NSApp, NSRunningApplication.currentApplication, application)) {
                if (current->_emojiReturn.fail(token)) [current reportEmojiDeliveryFailure];
            }
        });
        dispatch_after(dispatch_time(DISPATCH_TIME_NOW, 2 * NSEC_PER_SEC), dispatch_get_main_queue(), ^{
            MSIMEInputController *current = weakSelf;
            if (current && current->_emojiReturn.fail(token)) [current reportEmojiDeliveryFailure];
        });
        return YES;
    };
    MSIMEDesktopInputSession *inputSession = [[MSIMEDesktopInputSession alloc]
        initWithTargetPID:application.processIdentifier launchTime:application.launchDate.timeIntervalSince1970
        clipboard:[route isEqualToString:@"cloud-clipboard"] || [route isEqualToString:@"emoji"]
        handler:^(NSString *text, double deadline, MSIMEPanelTextCompletion completion) {
            MSIMEInputController *controller = weakSelf;
            if (!controller || controller->_emojiReturn.generation != token || controller->_desktopEmojiCompletion) {
                completion(NO); return;
            }
            controller->_desktopEmojiCompletion = completion;
            controller->_desktopEmojiDeadline = deadline;
            if (!selection(text)) {
                controller->_desktopEmojiCompletion = nil;
                completion(NO);
            }
        }];
    _desktopInputSession = inputSession;
    dispatch_block_t fallback = ^{
        [inputSession stop];
        MSIMEInputController *controller = weakSelf;
        if (!controller || controller->_emojiReturn.generation != token) return;
        if ([route isEqualToString:@"cloud-clipboard"]) {
            controller->_emojiReturn.discard(token);
            if (!MSIMEOpenBackendClipboard(NSClassFromString(@"MSIMEBackendAccountWindow"))) [controller showAccount:nil];
        } else if ([route isEqualToString:@"handwriting"])
            [shared performSelector:@selector(showHandwritingWithSelectionAttempt:) withObject:selection];
        else [shared performSelector:@selector(showEmojiWithOptions:selectionAttempt:) withObject:options withObject:selection];
    };
    if (!inputSession) { fallback(); return; }
    if ([route isEqualToString:@"cloud-clipboard"]) {
        MSIMEOpenDesktopCloudClipboardWithInput(MSIMERuntimeOptionsPath(), NSWorkspace.sharedWorkspace, inputSession, fallback);
        return;
    }
    MSIMEOpenDesktopRouteWithContext(route, MSIMERuntimeOptionsPath(), inputSession.launchEnvironment,
        NSWorkspace.sharedWorkspace, ^(NSRunningApplication *peer) {
            [inputSession authorizePID:peer.processIdentifier stillValid:^BOOL { return !peer.terminated; }];
        }, fallback);
}
- (void)showScreenKeyboard:(id)sender {
    (void)sender;
    MSIMEOpenDesktopRoute(@"keyboard", NSWorkspace.sharedWorkspace, ^{ [[MSIMEScreenKeyboardPanel sharedPanel] showKeyboard]; });
}
- (void)setEnglishInputMode:(BOOL)enabled {
    [self ensureAppearance];
    const BOOL changed = _appearance.englishMode != enabled;
    // Read before the cancel below, whose reply need not repeat the flag.
    const BOOL leaveEnglishCandidates = enabled && [_view[@"dedicated_english"] isEqual:@YES];
    if (enabled && !_appearance.englishMode && _session && _activeClient) {
        // Switching into English drops what was being composed rather than committing it. The
        // reference has two different rules here and this host had copied the wrong one onto both
        // entry points: its Shift toggle is FUNCTION_TOGGLE_IME_MODE, whose handler commits the raw
        // keystroke buffer, while its English-mode switch (Ctrl+Shift+E) is FUNCTION_CANCEL, whose
        // handler terminates the composition and sends nothing. Committing instead put a Chinese
        // candidate nobody chose into the document - the user reached for English precisely because
        // the candidates on screen were not what they wanted.
        //
        // The Shift tap keeps its own rule: it commits the raw letters before calling this, which
        // leaves nothing here to cancel.
        NSDictionary *cancelled = [_session command:MSIME_CANCEL error:nil];
        if (!cancelled) return; // Do not hide an unsettled composition after an Engine failure.
        [self apply:cancelled];
    }
    // Every switch to English also leaves the English candidate mode, as the reference's status task calls SetEnglishInputMode(false) and ClearState() whenever the mode turns English (event_listener.cpp), so switching back always gives pinyin. An Engine failure leaves the mode as it was.
    if (leaveEnglishCandidates && _session) {
        NSError *error = nil;
        NSDictionary *view = [_session setDedicatedEnglishEnabled:NO error:&error];
        if (!view) { if (error) NSBeep(); return; }
        [self apply:@{@"view":view}];
    }
    // A Chinese/English switch puts punctuation back in step with the mode, as the reference's SyncPunctuationWithImeMode does: English mode gets English punctuation unless punctuation_lock pins Chinese. Returning to Chinese goes back to the saved starting value, the macOS adaptation for the shared chinese_punctuation setting. Set before the mode is saved so the resulting sync and toolbar refresh already see it.
    if (changed) {
        _englishPunctuation = {};
        if (enabled) _appearance.runtimeChinesePunctuation = [_appearance.punctuationLock isEqual:@"chinese"];
        else [_appearance resetRuntimePunctuationForActiveApplication];
    }
    _appearance.englishMode = enabled;
    [self resetCandidateAnchor];
    [self hideCandidatePanel:"mode_switch"];
    [_keymapPanel orderOut:nil];
    [self syncSystemInputModeForClient:_activeClient ?: self.client];
    if (changed && _appearance.inputModeHUD && _activeClient) {
        NSRect caret = NSZeroRect;
        [(id<IMKTextInput>)_activeClient attributesForCharacterIndex:0 lineHeightRectangle:&caret];
        [[MSIMEInputModeHUDPanel sharedPanel] showEnglishInputMode:enabled nearCaretRect:caret];
    }
}
// Keeps the selected input mode - 中, 英 or 日 in the input menu - in step with the Chinese/English state and the scheme. A switch the system reported is already recorded as shown, so this does not echo it back.
- (void)syncSystemInputModeForClient:(id)client {
    NSString *mode = MSIMEInputModeIDFor(_appearance.englishMode, [_appearance.inputScheme isEqualToString:@"japanese"]);
    MSIMESelectSystemInputMode(MSIMESharedSystemInputModeState(), mode, client, MSIMEInputSourceIsEnabled);
}
// The system reports the mode the user picked from the input menu or reached with Ctrl+Space; the controller's Chinese/English state and, for 中 and 日, its scheme follow it. A report that only repeats the mode already shown, or one delivered from inside this controller's own selectInputMode:, leaves the state alone.
- (void)setValue:(id)value forTag:(long)tag client:(id)sender {
    if (tag == kTextServiceInputModePropertyTag) [self systemDidReportInputMode:value client:sender];
    [super setValue:value forTag:tag client:sender];
}
- (void)systemDidReportInputMode:(id)value client:(id)sender {
    if (!MSIMEAdoptReportedInputMode(MSIMESharedSystemInputModeState(), value)) return;
    [self ensureAppearance];
    // The report can arrive before activateServer: or handleEvent: has named the client, and the mode is remembered per application.
    [_appearance activateInputModeForApplication:[sender respondsToSelector:@selector(bundleIdentifier)] ? [sender bundleIdentifier] : nil];
    // Moving from 英 to 日 changes two things, and each change syncs the menu bar on its own: between them it would select 中 or 英 again and the system would report that back as a new choice. Holding `selecting` keeps both quiet, and the sync below selects the one mode they add up to. 英 leaves the scheme alone, so returning to 中 or 日 afterwards finds it where it was.
    MSIMESystemInputModeState &state = MSIMESharedSystemInputModeState();
    state.selecting = true;
    NSString *scheme = _appearance.inputScheme;
    if (MSIMEJapaneseForInputModeID(value) && ![scheme isEqualToString:@"japanese"]) _appearance.inputScheme = @"japanese";
    else if ([value isEqualToString:MSIMEChineseInputModeID] && [scheme isEqualToString:@"japanese"])
        _appearance.inputScheme = _appearance.lastChineseScheme;
    [self setEnglishInputMode:MSIMEEnglishForInputModeID(value)];
    state.selecting = false;
    // setEnglishInputMode: can refuse the switch (an Engine cancel failure while composing) after the report was already recorded as shown; select the mode the controller actually has so the menu bar does not keep the refused one. A switch that went through makes this a no-op.
    [self syncSystemInputModeForClient:sender];
}
- (void)selectChineseMode:(id)sender {
    (void)sender;
    if ([_view[@"dedicated_english"] isEqual:@YES]) [self setDedicatedEnglishInputMode:NO];
    else [self setEnglishInputMode:NO];
}
- (void)setDedicatedEnglishInputMode:(BOOL)enabled {
    if (!_activeClient) return;
    [self ensureAppearance];
    if (!_session || _focusPending) [self prepareSession];
    if (!_session) return;
    if ([_view[@"editing_text"] isKindOfClass:NSString.class] && [_view[@"editing_text"] length]) {
        // Ctrl+Shift+E is the reference's FUNCTION_CANCEL in both directions: _HandleCancel terminates the composition and commits nothing, so neither the highlighted Chinese candidate nor the English word being spelled reaches the document. An Engine failure leaves the mode as it was.
        NSDictionary *cancelled = [_session command:MSIME_CANCEL error:nil];
        if (!cancelled) return;
        [self apply:cancelled];
    }
    NSError *error = nil;
    NSDictionary *view = [_session setDedicatedEnglishEnabled:enabled error:&error];
    if (!view) { if (error) NSBeep(); return; }
    _appearance.englishMode = NO;
    [self syncSystemInputModeForClient:_activeClient];
    [self apply:@{@"view":view}];
}
- (void)toggleDedicatedEnglishMode:(id)sender {
    (void)sender;
    [self setDedicatedEnglishInputMode:_appearance.englishMode || ![_view[@"dedicated_english"] isEqual:@YES]];
}
- (void)selectSimplifiedOutput:(id)sender { (void)sender; [self ensureAppearance]; _appearance.traditionalOutput = NO; }
- (void)selectTraditionalOutput:(id)sender { (void)sender; [self ensureAppearance]; _appearance.traditionalOutput = YES; }
- (void)selectEnglishMode:(id)sender { (void)sender; [self setEnglishInputMode:YES]; }
- (void)showSystemCharacterPalette { [NSApp orderFrontCharacterPalette:nil]; }
- (void)checkForUpdates:(id)sender {
    (void)sender;
    MSIMEOpenDesktopUpdateSettings(NSWorkspace.sharedWorkspace, ^{
        [[MSIMEUpdateController sharedController] checkForUpdates:nil];
    });
}
- (void)showVoiceSettings:(id)sender {
    (void)sender;
    MSIMEOpenDesktopSettings(MSIMEDesktopSettingsPage::Voice, NSWorkspace.sharedWorkspace, ^{
        [[MetasequoiaVoiceProviderSettingsWindow sharedController] showAndActivate];
    });
}
- (BOOL)usesLocalModelVoice {
    NSUserDefaults *defaults = NSUserDefaults.standardUserDefaults;
    NSString *provider = [defaults stringForKey:@"MSIMEClientVoiceASRProvider"] ?: @"";
    return MSIMEVoiceUsesLocalModelHelper(provider, MSIMEVoiceProviderSocket() != nil,
                                          MSIMELocalVoiceModelDirectory([defaults stringForKey:@"MSIMEClientVoiceASRModelPath"]));
}
- (BOOL)usesNativeHTTPVoice {
    NSString *provider = [NSUserDefaults.standardUserDefaults stringForKey:@"MSIMEClientVoiceASRProvider"] ?: @"";
    // "local" with a Whisper model file recognises on this machine rather than over HTTP, but it is the same batch shape - record, hand the samples to one request, commit what comes back - so it travels the same path. A build without the Whisper recognizer falls through to the platform recognizer here if a preference file names one anyway. An installed model directory streams through the helper instead.
    if ([self usesLocalModelVoice]) return NO;
    return MSIMEVoiceUsesNativeHTTPProvider(provider, MSIMEVoiceProviderSocket() != nil, MSIMEVoiceLocalWhisperBuilt());
}
- (BOOL)usesNativeDoubaoVoice {
    NSString *provider = [NSUserDefaults.standardUserDefaults stringForKey:@"MSIMEClientVoiceASRProvider"] ?: @"doubao";
    return (!MSIMEVoiceProviderSocket() && [provider.lowercaseString isEqual:@"doubao"]) || [self usesLocalModelVoice];
}
- (void)dealloc {
    [[NSNotificationCenter defaultCenter] removeObserver:self];
    [NSDistributedNotificationCenter.defaultCenter removeObserver:self];
    // The toolbar outlives focus-outs, so a controller freed by IMK releases it here; the panel's owner check leaves a newer owner in place.
    [_toolbar deactivateForDelegate:self];
    if (_globalVoiceHotkeyMonitor) [NSEvent removeMonitor:_globalVoiceHotkeyMonitor];
    [_desktopInputSession stop]; [_httpVoiceRequest cancel]; [_doubaoVoiceRequest cancel];
    [_doubaoPolishRequest cancel]; [_livePolishRequest cancel];
    // A client that dies without deactivateServer: leaves the repeating timer on the run loop, and its candidate window on screen.
    [_preferencesTimer invalidate];
    [_panel orderOut:nil];
}
- (MSIMEHTTPVoiceRequest *)makeDoubaoPolishRequest:(NSDictionary *)options {
    if (!([options[@"polish_enabled"] boolValue] || [options[@"polish_text"] boolValue]) || ![options[@"polish_token"] length]) return nil;
    return [[MSIMEHTTPVoiceRequest alloc] initWithPolishOptions:options error:nil];
}
- (void)applyDoubaoFinalText:(NSString *)text request:(id<MSIMEStreamingVoiceRequest>)request {
    if (_doubaoVoiceRequest != request) return;
    if ([self ownsDoubaoVoiceFocus]) {
        if (!text.length) { [self reportVoiceFailure:MSIMEVoiceFailureNoSpeech]; return; }
        NSDictionary *result = text.length ? [_doubaoVoiceSession applyVoiceText:text generation:_doubaoVoiceGeneration error:nil] : nil;
        if (result) { _doubaoVoiceMarked = NO; [self applyVoiceResult:result route:_doubaoVoiceCommit]; }
    }
    [self cancelDoubaoVoiceInput];
}
- (MSIMEDoubaoVoiceRequest *)makeDoubaoVoiceRequest:(NSDictionary *)options error:(NSError **)error {
    return [[MSIMEDoubaoVoiceRequest alloc] initWithOptions:options error:error];
}
- (id<MSIMEStreamingVoiceRequest>)makeLocalVoiceRequest:(NSDictionary *)options error:(NSError **)error {
    return [[MSIMELocalVoiceRequest alloc] initWithOptions:options hostOptions:_session.hostOptions error:error];
}
- (id<MSIMEStreamingVoiceRequest>)makeStreamingVoiceRequest:(NSDictionary *)options error:(NSError **)error {
    id provider = options[@"asr_provider"];
    if ([provider isKindOfClass:NSString.class] && [[provider lowercaseString] isEqual:@"local"])
        return [self makeLocalVoiceRequest:options error:error];
    return [self makeDoubaoVoiceRequest:options error:error];
}
- (BOOL)ownsDoubaoVoiceFocus {
    return _doubaoVoiceRequest && _activeClient == _doubaoVoiceClient &&
        _session == _doubaoVoiceSession && _voiceGeneration == _doubaoVoiceGeneration && _voiceService.active;
}
- (void)voiceCaptureDidStart {
    if (_voiceCueRecording) return;
    _voiceCueRecording = YES;
    NSUserDefaults *defaults = NSUserDefaults.standardUserDefaults;
    // Windows plays the start cue and only then mutes other audio. The macOS mute is device-wide and would swallow the cue, so it waits for the cue to finish; any restore before then cancels it.
    void (^mute)(void) = nil;
    if (MSIMEVoiceMuteSystemAudioEnabled(defaults)) mute = [_voiceAudioMuter deferredMute];
    if (MSIMEVoiceCueEnabled(defaults, YES)) [_voiceCuePlayer playStartCueThen:mute];
    else if (mute) mute();
}
- (void)voiceCaptureDidEnd {
    if (!_voiceCueRecording) return;
    _voiceCueRecording = NO;
    if (MSIMEVoiceCueEnabled(NSUserDefaults.standardUserDefaults, NO)) [_voiceCuePlayer playStopCue];
}
- (NSScreen *)voiceInputScreen {
    if (!_activeClient) return nil;
    NSRect caret = NSZeroRect;
    [(id<IMKTextInput>)_activeClient attributesForCharacterIndex:0 lineHeightRectangle:&caret];
    if (!MSIMEValidCaret(caret)) return nil;
    const NSPoint point = NSMakePoint(NSMidX(caret), NSMidY(caret));
    for (NSScreen *screen in NSScreen.screens)
        if (NSPointInRect(point, screen.frame)) return screen;
    return nil;
}
- (void)refreshVoiceOverlayScreen {
    if (_voiceOverlay) _voiceOverlay.preferredScreen = [self voiceInputScreen];
}
- (void)reportVoiceFailure:(MSIMEVoiceFailure)failure { [self reportVoiceFailure:failure detail:nil]; }
- (void)reportVoiceFailure:(MSIMEVoiceFailure)failure detail:(NSString *)detail {
    _voicePermissionToken = nil; _voiceHoldShortcut.reset();
    [self cancelHTTPVoiceInput]; [self cancelDoubaoVoiceInput]; [self cancelLiveVoiceInput];
    if (_voiceService.active) [_voiceService cancelWithError:nil];
    [_voiceAudioMuter restore]; [self voiceCaptureDidEnd];
    if (!_activeClient) return;
    if (!_voiceOverlay) {
        _voiceOverlay = [MSIMEVoiceWaveOverlay new];
        [_voiceOverlay applyThemePreferences:_voiceThemePreferences ?: @{}];
    }
    [self refreshVoiceOverlayScreen];
    [_voiceOverlay showFailure:failure detail:detail];
}
- (void)cancelDoubaoVoiceInput {
    if (!_doubaoVoiceRequest) return;
    if (_doubaoVoiceMarked && [self ownsDoubaoVoiceFocus])
        [(id<MSIMETextClient>)_doubaoVoiceClient setMarkedText:@"" selectionRange:NSMakeRange(0, 0) replacementRange:NSMakeRange(NSNotFound, NSNotFound)];
    [_doubaoVoiceRequest cancel];
    [_doubaoPolishRequest cancel];
    _doubaoPolishRequest = nil;
    _doubaoFinalReceived = NO;
    _doubaoVoiceRequest = nil;
    _doubaoVoiceSession = nil;
    _doubaoVoiceClient = nil;
    _doubaoVoiceMarked = NO;
    _doubaoVoiceProcessing = NO;
    [_voiceService cancelWithError:nil];
    [_voiceAudioMuter restore];
    [_voiceOverlay setListening:NO];
    [self voiceCaptureDidEnd];
}
- (BOOL)startDoubaoVoiceInputWithOptions:(NSDictionary *)options {
    NSError *error = nil;
    id<MSIMEStreamingVoiceRequest> request = [self makeStreamingVoiceRequest:options error:&error];
    NSDictionary *finished = request && _activeClient && _session ? [_session command:MSIME_FINISH_COMPOSITION error:&error] : nil;
    if (!finished) {
        [request cancel]; [_voiceService cancelWithError:nil]; [_voiceAudioMuter restore]; [_voiceOverlay setListening:NO];
        [self reportVoiceFailure:request ? MSIMEVoiceFailureSession : MSIMEVoiceFailureProvider detail:MSIMEVoiceFailureDetail(error)];
        return NO;
    }
    [self apply:finished];
    _doubaoVoiceRequest = request;
    _doubaoPolishRequest = [self makeDoubaoPolishRequest:options];
    _doubaoFinalReceived = NO;
    _doubaoVoiceSession = _session;
    _doubaoVoiceClient = _activeClient;
    _doubaoVoiceGeneration = _voiceGeneration;
    _doubaoVoiceProcessing = NO;
    _doubaoVoiceMarked = NO;
    _doubaoVoiceCommit = MSIMECaptureVoiceCommit(options[@"commit_mode"], _activeClient);
    _doubaoVoiceInline = [options[@"stream"] boolValue] && [_doubaoVoiceCommit.mode isEqual:@"tsf"];
    [self bindVoiceOverlayActions];
    __weak MSIMEInputController *weakSelf = self;
    __weak id<MSIMEStreamingVoiceRequest> weakRequest = request;
    if (![request startWithResult:^(NSString *text, BOOL final, NSError *failure) {
        MSIMEInputController *controller = weakSelf;
        id<MSIMEStreamingVoiceRequest> liveRequest = weakRequest;
        if (!controller || !liveRequest || controller->_doubaoVoiceRequest != liveRequest) return;
        if (![controller ownsDoubaoVoiceFocus]) { [controller cancelDoubaoVoiceInput]; return; }
        if (controller->_doubaoFinalReceived) return;
        if (failure) { [controller reportVoiceFailure:MSIMEVoiceFailureProvider detail:MSIMEVoiceFailureDetail(failure)]; return; }
        if (!final) {
            if (controller->_doubaoVoiceInline && text) {
                [(id<MSIMETextClient>)controller->_doubaoVoiceClient setMarkedText:text selectionRange:NSMakeRange(text.length, 0) replacementRange:NSMakeRange(NSNotFound, NSNotFound)];
                controller->_doubaoVoiceMarked = YES;
            }
            if (!controller->_doubaoVoiceInline && text.length <= 65536) [controller->_voiceOverlay setTranscript:text ?: @""];
            return; // Partial text must not consume the runtime's final-only token.
        }
        controller->_doubaoFinalReceived = YES;
        if (!controller->_doubaoVoiceProcessing) {
            controller->_doubaoVoiceProcessing = YES;
            // Final already arrived: stop capture without sending a final packet.
            [controller->_voiceService finishPCMStreamingWithError:nil];
            [controller->_voiceAudioMuter restore];
            [controller voiceCaptureDidEnd];
        }
        if (msime::voice::short_capture(controller->_voiceService.recordedDuration)) {
            [controller cancelDoubaoVoiceInput]; return;
        }
        MSIMEHTTPVoiceRequest *polisher = controller->_doubaoPolishRequest;
        if (text.length && polisher) {
            [controller->_voiceOverlay setProcessing:YES];
            if (!controller->_doubaoVoiceInline) [controller->_voiceOverlay setTranscript:text];
        }
        if (text.length && polisher && [polisher polishText:text completion:^(NSString *polished, NSError *polishError) {
            [weakSelf applyDoubaoFinalText:!polishError && polished.length ? polished : text request:liveRequest];
        } error:nil]) return;
        [controller applyDoubaoFinalText:text request:liveRequest];
    } error:&error]) { [self reportVoiceFailure:MSIMEVoiceFailureProvider detail:MSIMEVoiceFailureDetail(error)]; return NO; }
    NSString *device = [NSUserDefaults.standardUserDefaults stringForKey:@"MSIMEClientVoiceCaptureDevice"];
    if (![_voiceService startPCMStreaming:^(NSData *pcm, NSError *failure) {
        id<MSIMEStreamingVoiceRequest> liveRequest = weakRequest;
        if (!liveRequest) return;
        NSError *sendError = failure;
        BOOL sent = !failure && pcm && [liveRequest appendPCM:pcm error:&sendError];
        const float *samples = static_cast<const float *>(pcm.bytes);
        const float level = sent ? msime::voice::input_level(samples, pcm.length / sizeof(float)) : 0;
        // Never stop the capture engine while holding its stream admission lock.
        dispatch_async(dispatch_get_main_queue(), ^{
            MSIMEInputController *controller = weakSelf;
            if (!controller || controller->_doubaoVoiceRequest != liveRequest) return;
            if (controller->_doubaoFinalReceived) return;
            if (![controller ownsDoubaoVoiceFocus]) { [controller cancelDoubaoVoiceInput]; return; }
            if (!sent) [controller reportVoiceFailure:MSIMEVoiceFailureCapture];
            else if (!controller->_doubaoVoiceProcessing) [controller->_voiceOverlay setInputLevel:level];
        });
    } deviceUID:device error:&error]) { [self reportVoiceFailure:MSIMEVoiceFailureCapture]; return NO; }
    [self voiceCaptureDidStart];
    return YES;
}
- (void)finishDoubaoVoiceInput {
    if (!_doubaoVoiceRequest) return;
    if (_doubaoVoiceProcessing) { [self cancelDoubaoVoiceInput]; return; }
    _doubaoVoiceProcessing = YES;
    NSError *error = nil;
    NSData *tail = [_voiceService finishPCMStreamingWithError:&error];
    if (tail && !error && msime::voice::short_capture(_voiceService.recordedDuration)) { [self cancelDoubaoVoiceInput]; return; }
    [_voiceAudioMuter restore];
    [_voiceOverlay setProcessing:NO];
    [self voiceCaptureDidEnd];
    if (!tail || error || (tail.length && ![_doubaoVoiceRequest appendPCM:tail error:&error]) ||
        ![_doubaoVoiceRequest finishWithError:&error]) [self reportVoiceFailure:MSIMEVoiceFailureProvider detail:MSIMEVoiceFailureDetail(error)];
}
- (MSIMEHTTPVoiceRequest *)makeHTTPVoiceRequest:(NSDictionary *)options error:(NSError **)error {
    return [[MSIMEHTTPVoiceRequest alloc] initWithOptions:options error:error];
}
- (void)cancelHTTPVoiceInput {
    if (!_httpVoiceRequest) return;
    [_httpVoiceRequest cancel];
    _httpVoiceRequest = nil;
    _httpVoiceSession = nil;
    _httpVoiceClient = nil;
    _httpVoiceProcessing = NO;
    [_voiceService cancelWithError:nil];
    [_voiceAudioMuter restore];
    [_voiceOverlay setListening:NO];
    [self voiceCaptureDidEnd];
}
- (BOOL)startHTTPVoiceInputWithOptions:(NSDictionary *)options {
    NSError *error = nil;
    MSIMEHTTPVoiceRequest *request = [self makeHTTPVoiceRequest:options error:&error];
    if (!request || !_activeClient || !_session) {
        [_voiceService cancelWithError:nil]; [_voiceAudioMuter restore]; [_voiceOverlay setListening:NO];
        [self reportVoiceFailure:request ? MSIMEVoiceFailureSession : MSIMEVoiceFailureProvider detail:MSIMEVoiceFailureDetail(error)];
        return NO;
    }
    _httpVoiceRequest = request;
    _httpVoiceSession = _session;
    _httpVoiceClient = _activeClient;
    _httpVoiceCommit = MSIMECaptureVoiceCommit(options[@"commit_mode"], _activeClient);
    _httpVoiceGeneration = _voiceGeneration;
    _httpVoiceProcessing = NO;
    __weak MSIMEInputController *weakSelf = self;
    [self bindVoiceOverlayActions];
    __weak MSIMEHTTPVoiceRequest *weakRequest = request;
    request.polishingHandler = ^{
        MSIMEInputController *controller = weakSelf;
        if (!controller || !weakRequest || controller->_httpVoiceRequest != weakRequest || !controller->_httpVoiceProcessing ||
            controller->_activeClient != controller->_httpVoiceClient || controller->_session != controller->_httpVoiceSession ||
            controller->_voiceGeneration != controller->_httpVoiceGeneration || !controller->_voiceService.active) return;
        [controller->_voiceOverlay setProcessing:YES];
    };
    NSString *device = [NSUserDefaults.standardUserDefaults stringForKey:@"MSIMEClientVoiceCaptureDevice"];
    const NSUInteger sampleLimit = request.sampleLimit;
    if (![_voiceService startPCMRecording:^(AVAudioPCMBuffer *buffer) {
        const float level = MSIMEVoiceInputLevel(buffer);
        dispatch_async(dispatch_get_main_queue(), ^{
            MSIMEInputController *controller = weakSelf;
            if (!controller || controller->_httpVoiceRequest != request || controller->_httpVoiceProcessing ||
                controller->_activeClient != controller->_httpVoiceClient || controller->_session != controller->_httpVoiceSession ||
                controller->_voiceGeneration != controller->_httpVoiceGeneration || !controller->_voiceService.active) return;
            [controller->_voiceOverlay setInputLevel:level];
            // MSIME-Windows keeps every sample and tells the user when the upload is too large; this host keeps only what the provider takes. Once it has that much, end the recording the way a release does, so the overlay turns to 识别中... and the end cue plays instead of the wave animating over audio that is no longer kept. The physical release that follows must not cancel the recognition.
            if (controller->_voiceService.recordedDuration * 16000.0 >= sampleLimit) {
                controller->_voiceHoldShortcut.reset();
                [controller finishHTTPVoiceInput];
            }
        });
    } deviceUID:device failure:^(NSError *failure) {
        (void)failure;
        MSIMEInputController *controller = weakSelf;
        if (!controller || controller->_httpVoiceRequest != request) return;
        if (controller->_activeClient != controller->_httpVoiceClient || controller->_session != controller->_httpVoiceSession ||
            controller->_voiceGeneration != controller->_httpVoiceGeneration) { [controller cancelHTTPVoiceInput]; return; }
        [controller reportVoiceFailure:MSIMEVoiceFailureCapture];
    } error:&error]) { [self reportVoiceFailure:MSIMEVoiceFailureCapture]; return NO; }
    [self voiceCaptureDidStart];
    return YES;
}
- (void)finishHTTPVoiceInput {
    MSIMEHTTPVoiceRequest *request = _httpVoiceRequest;
    if (!request) return;
    if (_httpVoiceProcessing) { [self cancelHTTPVoiceInput]; return; }
    _httpVoiceProcessing = YES;
    NSError *error = nil;
    NSData *pcm = [_voiceService finishPCMRecordingWithError:&error];
    if (pcm && !error && msime::voice::short_capture(_voiceService.recordedDuration)) { [self cancelHTTPVoiceInput]; return; }
    [_voiceAudioMuter restore];
    [_voiceOverlay setProcessing:NO];
    [self voiceCaptureDidEnd];
    if (!pcm.length || error) { [self reportVoiceFailure:MSIMEVoiceFailureCapture]; return; }
    MSIMEClientSession *session = _httpVoiceSession;
    id client = _httpVoiceClient;
    const uint64_t generation = _httpVoiceGeneration;
    __weak MSIMEInputController *weakSelf = self;
    if (![request recognizePCM:pcm completion:^(NSString *text, NSError *failure) {
        MSIMEInputController *controller = weakSelf;
        if (!controller || controller->_httpVoiceRequest != request) return;
        if (controller->_activeClient != client || controller->_session != session ||
            controller->_voiceGeneration != generation || !controller->_voiceService.active) { [controller cancelHTTPVoiceInput]; return; }
        if (failure || !text.length) { [controller reportVoiceFailure:failure ? MSIMEVoiceFailureProvider : MSIMEVoiceFailureNoSpeech detail:MSIMEVoiceFailureDetail(failure)]; return; }
        if (!failure && text.length && controller->_activeClient == client &&
            controller->_session == session && controller->_voiceGeneration == generation && controller->_voiceService.active) {
            // Native host session methods are main-thread-only. Recheck the
            // departing focus identity before the runtime's own generation check.
            NSDictionary *result = [session applyVoiceText:text generation:generation error:nil];
            if (result) [controller applyVoiceResult:result route:controller->_httpVoiceCommit];
        }
        [controller cancelHTTPVoiceInput];
    } error:&error]) [self reportVoiceFailure:MSIMEVoiceFailureProvider detail:MSIMEVoiceFailureDetail(error)];
}
- (BOOL)ownsLiveVoiceToken:(id)token {
    return token && token == _liveVoiceToken && _activeClient == _liveVoiceClient &&
        _session == _liveVoiceSession && _voiceGeneration == _liveVoiceGeneration && _voiceService.active;
}
- (void)cancelLiveVoiceInput {
    if (!_liveVoiceToken) return;
    [_livePolishRequest cancel]; _livePolishRequest = nil;
    if (_liveVoiceMarked && [self ownsLiveVoiceToken:_liveVoiceToken])
        [(id<MSIMETextClient>)_liveVoiceClient setMarkedText:@"" selectionRange:NSMakeRange(0, 0) replacementRange:NSMakeRange(NSNotFound, NSNotFound)];
    MSIMEDeactivateVoice(_voiceService, _liveVoiceSession, _voiceAudioMuter, _voiceOverlay, _liveVoiceSocket, _liveVoiceGeneration);
    [self voiceCaptureDidEnd];
    _liveVoiceToken = nil; _liveVoiceSession = nil; _liveVoiceClient = nil; _liveVoiceSocket = nil;
    _liveVoiceMarked = NO; _liveVoiceProcessing = NO; _liveVoiceFinalReceived = NO;
}
- (MSIMEHTTPVoiceRequest *)makeLiveVoicePolishRequest:(NSDictionary *)options {
    if (!([options[@"polish_enabled"] boolValue] || [options[@"polish_text"] boolValue]) || ![options[@"polish_token"] length]) return nil;
    return [[MSIMEHTTPVoiceRequest alloc] initWithPolishOptions:options error:nil];
}
- (void)expireLiveVoice:(id)token {
    if ([self ownsLiveVoiceToken:token]) [self reportVoiceFailure:MSIMEVoiceFailureTimeout];
    else if (token && _liveVoiceToken == token) [self cancelLiveVoiceInput];
}
- (id)beginLiveVoiceWithOptions:(NSDictionary *)options socket:(NSString *)socket {
    NSDictionary *finished = _session && _activeClient ? [_session command:MSIME_FINISH_COMPOSITION error:nil] : nil;
    if (!finished) {
        MSIMEDeactivateVoice(_voiceService, _session, _voiceAudioMuter, _voiceOverlay, socket, _voiceGeneration);
        [self reportVoiceFailure:MSIMEVoiceFailureSession];
        return nil;
    }
    [self apply:finished];
    _liveVoiceToken = [NSObject new]; _liveVoiceSession = _session; _liveVoiceClient = _activeClient;
    _liveVoiceGeneration = _voiceGeneration; _liveVoiceSocket = [socket copy];
    // External providers already own their optional polish stage. Snapshot local
    // settings at recording start through the shared native request adapter.
    _livePolishRequest = socket.length ? nil : [self makeLiveVoicePolishRequest:options];
    _liveVoiceFinalReceived = NO;
    _liveVoiceCommit = MSIMECaptureVoiceCommit(options[@"commit_mode"], _activeClient);
    _liveVoiceInline = [options[@"stream"] boolValue] && [_liveVoiceCommit.mode isEqual:@"tsf"];
    _liveVoiceMarked = NO; _liveVoiceProcessing = NO;
    [self bindVoiceOverlayActions];
    id token = _liveVoiceToken;
    __weak MSIMEInputController *weakSelf = self;
    dispatch_after(dispatch_time(DISPATCH_TIME_NOW, 100 * NSEC_PER_SEC), dispatch_get_main_queue(), ^{
        [weakSelf expireLiveVoice:token];
    });
    return token;
}
- (void)bindVoiceOverlayActions {
    __weak MSIMEInputController *weakSelf = self;
    id client = _activeClient;
    MSIMEClientSession *session = _session;
    const uint64_t generation = _voiceGeneration;
    id http = _httpVoiceRequest, doubao = _doubaoVoiceRequest, live = _liveVoiceToken;
    _voiceOverlay.actionHandler = ^(BOOL cancel) {
        MSIMEInputController *controller = weakSelf;
        if (!controller || controller->_activeClient != client || controller->_session != session ||
            controller->_voiceGeneration != generation || !controller->_voiceService.active ||
            controller->_httpVoiceRequest != http || controller->_doubaoVoiceRequest != doubao ||
            controller->_liveVoiceToken != live || !(http || doubao || live)) return;
        // A subsequent physical hold release must not toggle pending recognition.
        controller->_voiceHoldShortcut.reset();
        if (cancel) {
            [controller cancelHTTPVoiceInput]; [controller cancelDoubaoVoiceInput]; [controller cancelLiveVoiceInput];
        } else if (controller->_httpVoiceProcessing || controller->_doubaoVoiceProcessing || controller->_liveVoiceProcessing) {
            [controller->_voiceOverlay dismissProcessing];
        } else if (http) [controller finishHTTPVoiceInput];
        else if (doubao) [controller finishDoubaoVoiceInput];
        else [controller finishLiveVoiceInput];
    };
}
- (void)applyLiveVoiceFinalText:(NSString *)text token:(id)token {
    if (!token || token != _liveVoiceToken) return;
    if ([self ownsLiveVoiceToken:token]) {
        if (!text.length) { [self reportVoiceFailure:MSIMEVoiceFailureNoSpeech]; return; }
        NSDictionary *result = text.length ? [_liveVoiceSession applyVoiceText:text generation:_liveVoiceGeneration error:nil] : nil;
        if (result) { _liveVoiceMarked = NO; [self applyVoiceResult:result route:_liveVoiceCommit]; }
    }
    [self cancelLiveVoiceInput];
}
- (void)applyLiveVoiceText:(NSString *)text final:(BOOL)final token:(id)token {
    if (!token || token != _liveVoiceToken) return;
    if (![self ownsLiveVoiceToken:token]) { [self cancelLiveVoiceInput]; return; }
    if (_liveVoiceFinalReceived) return;
    if (!final) {
        if (_liveVoiceInline && text.length <= 65536) {
            [(id<MSIMETextClient>)_liveVoiceClient setMarkedText:text ?: @"" selectionRange:NSMakeRange(text.length, 0) replacementRange:NSMakeRange(NSNotFound, NSNotFound)];
            _liveVoiceMarked = YES;
        }
        if (!_liveVoiceInline && text.length <= 65536) [_voiceOverlay setTranscript:text ?: @""];
        return;
    }
    _liveVoiceFinalReceived = YES;
    if (!_liveVoiceSocket.length && !_liveVoiceProcessing) {
        [self finishLiveVoiceInput];
        if (![self ownsLiveVoiceToken:token]) return;
    }
    if (text.length && _livePolishRequest) {
        // Final recognition may arrive before key release. Stop audio while
        // polishing, retaining only this session's final-result authorization.
        if (!_liveVoiceProcessing) [self finishLiveVoiceInput];
        [_voiceService stopTranscription];
        [_voiceOverlay setProcessing:YES];
        if (!_liveVoiceInline) [_voiceOverlay setTranscript:text];
        NSString *original = [text copy];
        __weak MSIMEInputController *weakSelf = self;
        if ([_livePolishRequest polishText:original completion:^(NSString *polished, NSError *error) {
            [weakSelf applyLiveVoiceFinalText:!error && polished.length ? polished : original token:token];
        } error:nil]) return;
    }
    [self applyLiveVoiceFinalText:text token:token];
}
- (void)applyLiveVoicePhase:(NSUInteger)phase token:(id)token {
    if (![self ownsLiveVoiceToken:token]) return;
    if (phase == 0 && !_liveVoiceProcessing) [self voiceCaptureDidStart];
    else if (phase == 1 || phase == 2) {
        _liveVoiceProcessing = YES;
        [_voiceAudioMuter restore]; [_voiceOverlay setProcessing:phase == 2];
        [self voiceCaptureDidEnd];
    }
}
- (void)finishLiveVoiceInput {
    if (!_liveVoiceToken) return;
    if (_liveVoiceProcessing) { [self cancelLiveVoiceInput]; return; }
    _liveVoiceProcessing = YES;
    // endAudio is sent by stopMicrophoneCapture; leave Speech alive for its final.
    [_voiceService stopMicrophoneCapture];
    if (!_liveVoiceSocket.length && msime::voice::short_capture(_voiceService.recordedDuration)) { [self cancelLiveVoiceInput]; return; }
    [_voiceAudioMuter restore]; [_voiceOverlay setProcessing:NO];
    [self voiceCaptureDidEnd];
    if (_liveVoiceSocket.length) {
        NSString *socket = _liveVoiceSocket; MSIMEClientSession *session = _liveVoiceSession;
        uint64_t generation = _liveVoiceGeneration;
        dispatch_async(dispatch_get_global_queue(QOS_CLASS_USER_INITIATED, 0), ^{ [session voiceProviderStopSocket:socket generation:generation error:nil]; });
    }
    id token = _liveVoiceToken;
    __weak MSIMEInputController *weakSelf = self;
    dispatch_after(dispatch_time(DISPATCH_TIME_NOW, 30 * NSEC_PER_SEC), dispatch_get_main_queue(), ^{
        [weakSelf expireLiveVoice:token];
    });
}
- (void)finishVoiceInputForDisable {
    _voicePermissionToken = nil;
    // Windows RefreshKeyboardHook stops capture on disable without discarding
    // its final result. Repeated preference loads must not act as a second stop.
    _voiceHoldShortcut.reset();
    if (_httpVoiceRequest && !_httpVoiceProcessing) [self finishHTTPVoiceInput];
    if (_doubaoVoiceRequest && !_doubaoVoiceProcessing) [self finishDoubaoVoiceInput];
    if (_liveVoiceToken && !_liveVoiceProcessing) [self finishLiveVoiceInput];
}
- (void)requestVoicePermissionForSpeech:(BOOL)speech resume:(BOOL)resume {
    id token = [NSObject new], client = _activeClient;
    MSIMEClientSession *session = _session;
    __weak MSIMEVoiceInputService *service = _voiceService;
    const uint64_t generation = _voiceGeneration;
    NSString *provider = [NSUserDefaults.standardUserDefaults stringForKey:@"MSIMEClientVoiceASRProvider"] ?: @"";
    _voicePermissionToken = token;
    __weak MSIMEInputController *weakSelf = self;
    void (^completion)(BOOL) = ^(BOOL granted) {
        MSIMEInputController *controller = weakSelf;
        if (!controller || controller->_voicePermissionToken != token) return;
        // Consume before resuming: Speech may chain a microphone request, and
        // duplicate callbacks must not consume that request or toggle capture.
        controller->_voicePermissionToken = nil;
        if (controller->_activeClient != client || controller->_session != session ||
            controller->_voiceService != service || controller->_voiceGeneration != generation ||
            ![provider isEqual:([NSUserDefaults.standardUserDefaults stringForKey:@"MSIMEClientVoiceASRProvider"] ?: @"")]) return;
        if (!granted) { [controller reportVoiceFailure:speech ? MSIMEVoiceFailureSpeechPermission : MSIMEVoiceFailureMicrophonePermission]; return; }
        if (!resume) return;
        [controller toggleVoiceInput:nil];
    };
    if (speech) [_voiceService requestSpeechPermission:completion];
    else [_voiceService requestMicrophonePermission:completion];
}
- (void)toggleVoiceInput:(id)sender {
    (void)sender;
    if (_voicePermissionToken) { _voicePermissionToken = nil; return; }
    // All menu, toolbar, local/global shortcut and permission callbacks converge
    // here. Recheck after asynchronous permission delivery, before any capture.
    if (!MSIMEVoiceInputEnabled(NSUserDefaults.standardUserDefaults)) {
        [self finishVoiceInputForDisable];
        return;
    }
    if (!_activeClient) return;
    if (!_session) [self prepareSession];
    if (!_session) { [self reportVoiceFailure:MSIMEVoiceFailureSession]; return; }
    if (!_voiceService) _voiceService = [[MSIMEVoiceInputService alloc] init];
    if (!_voiceCuePlayer) _voiceCuePlayer = [[MSIMEVoiceCuePlayer alloc] init];
    if (!_voiceAudioMuter) _voiceAudioMuter = [[MSIMEVoiceAudioMuter alloc] init];
    if (!_voiceOverlay) {
        _voiceOverlay = [[MSIMEVoiceWaveOverlay alloc] init];
        [_voiceOverlay applyThemePreferences:_voiceThemePreferences ?: @{}];
    }
    [self refreshVoiceOverlayScreen];
    if (_httpVoiceRequest) { [self finishHTTPVoiceInput]; return; }
    if (_doubaoVoiceRequest) { [self finishDoubaoVoiceInput]; return; }
    if (_liveVoiceToken) { [self finishLiveVoiceInput]; return; }
    if (_voiceService.active) { [_voiceService cancelWithError:nil]; [_voiceAudioMuter restore]; [_voiceOverlay setListening:NO]; return; }
    // Checked before any permission prompt or capture, where MSIME-Windows StartRecording checks it: without a token the recording could only fail after the user had spoken.
    NSUserDefaults *voiceDefaults = NSUserDefaults.standardUserDefaults;
    if (MSIMEVoiceASRTokenMissing([voiceDefaults stringForKey:@"MSIMEClientVoiceASRProvider"],
                                  [voiceDefaults stringForKey:@"MSIMEClientVoiceASRToken"], MSIMEVoiceProviderSocket() != nil)) {
        [self reportVoiceFailure:MSIMEVoiceFailureMissingToken];
        return;
    }
    __weak MSIMEInputController *weakSelf = self;
    void (^start)(void) = ^{
        MSIMEInputController *controller = weakSelf;
        if (!controller || !controller->_session) return;
        NSUserDefaults *defaults = NSUserDefaults.standardUserDefaults;
        if (!MSIMEVoiceCaptureBackendSupported([defaults objectForKey:@"MSIMEClientVoiceCaptureBackend"])) {
            [controller reportVoiceFailure:MSIMEVoiceFailureCapture];
            return;
        }
        NSError *error = nil;
        if (![controller->_voiceService startWithSession:controller->_session generation:&controller->_voiceGeneration error:&error]) { [controller reportVoiceFailure:MSIMEVoiceFailureSession]; return; }
        [controller->_voiceOverlay setListening:YES];
        NSString *language = [[NSUserDefaults standardUserDefaults] stringForKey:@"MSIMEClientVoiceLanguage"] ?: @"zh-CN";
        NSString *socket = MSIMEVoiceProviderSocket();
        NSDictionary *query = @{ @"language": language.lowercaseString, @"generation": @(controller->_voiceGeneration), @"stream": @([defaults objectForKey:@"MSIMEClientVoiceStreamInlinePreedit"] == nil || [defaults boolForKey:@"MSIMEClientVoiceStreamInlinePreedit"]), @"asr_provider": [defaults stringForKey:@"MSIMEClientVoiceASRProvider"] ?: @"doubao", @"asr_endpoint": [defaults stringForKey:@"MSIMEClientVoiceASREndpoint"] ?: @"", @"asr_model": [defaults stringForKey:@"MSIMEClientVoiceASRModel"] ?: @"", @"asr_model_path": [defaults stringForKey:@"MSIMEClientVoiceASRModelPath"] ?: @"", @"asr_token": [defaults stringForKey:@"MSIMEClientVoiceASRToken"] ?: @"", @"doubao_boosting_table_id": [defaults stringForKey:@"MSIMEClientVoiceDoubaoBoostingTableID"] ?: @"", @"asr_app_key": [defaults stringForKey:@"MSIMEClientVoiceDoubaoAppKey"] ?: @"", @"asr_resource_id": [defaults stringForKey:@"MSIMEClientVoiceDoubaoResourceID"] ?: @"", @"polish_enabled": @([defaults boolForKey:@"MSIMEClientVoicePolish"]), @"polish_prompt_id": [defaults stringForKey:@"MSIMEClientVoicePolishPromptID"] ?: @"cleanup", @"polish_provider": [defaults stringForKey:@"MSIMEClientVoicePolishProvider"] ?: MSIMEVoicePolishDefaultProvider, @"polish_model": [defaults stringForKey:@"MSIMEClientVoicePolishModel"] ?: @"", @"polish_endpoint": [defaults stringForKey:@"MSIMEClientVoicePolishEndpoint"] ?: @"", @"polish_token": [defaults stringForKey:@"MSIMEClientVoicePolishToken"] ?: @"", @"polish_prompt": [defaults stringForKey:@"MSIMEClientVoicePolishPrompt"] ?: @"", @"polish_prompt_custom_1": [defaults stringForKey:@"MSIMEClientVoicePolishPromptCustom1"] ?: @"", @"polish_prompt_custom_2": [defaults stringForKey:@"MSIMEClientVoicePolishPromptCustom2"] ?: @"", @"polish_prompt_custom_3": [defaults stringForKey:@"MSIMEClientVoicePolishPromptCustom3"] ?: @"" };
        query = MSIMEVoiceProviderOptions(query, defaults);
        if (socket.length) {
            id token = [controller beginLiveVoiceWithOptions:query socket:socket];
            if (!token) return;
            MSIMEClientSession *session = controller->_liveVoiceSession;
            dispatch_async(dispatch_get_global_queue(QOS_CLASS_USER_INITIATED, 0), ^{
                NSError *providerError = nil;
                [session voiceProviderStream:query socket:socket update:^(NSString *text, BOOL final) {
                    dispatch_async(dispatch_get_main_queue(), ^{
                        [weakSelf applyLiveVoiceText:text final:final token:token];
                    });
                } phase:^(NSUInteger phase) {
                    dispatch_async(dispatch_get_main_queue(), ^{ [weakSelf applyLiveVoicePhase:phase token:token]; });
                } error:&providerError];
                dispatch_async(dispatch_get_main_queue(), ^{
                    MSIMEInputController *live = weakSelf;
                    if ([live ownsLiveVoiceToken:token]) [live reportVoiceFailure:MSIMEVoiceFailureProvider];
                    else if (live && live->_liveVoiceToken == token) [live cancelLiveVoiceInput];
                });
            });
            return;
        }
        if ([controller usesNativeHTTPVoice]) { [controller startHTTPVoiceInputWithOptions:query]; return; }
        if ([controller usesNativeDoubaoVoice]) { [controller startDoubaoVoiceInputWithOptions:query]; return; }
        id token = [controller beginLiveVoiceWithOptions:query socket:nil];
        if (!token) return;
        if (![controller->_voiceService startTranscriptionWithLanguage:language textHandler:^(NSString *text, BOOL final) {
            [weakSelf applyLiveVoiceText:text final:final token:token];
        } error:&error]) { [controller reportVoiceFailure:MSIMEVoiceFailureProvider]; return; }
        NSString *deviceUID = [NSUserDefaults.standardUserDefaults stringForKey:@"MSIMEClientVoiceCaptureDevice"];
        if (![controller->_voiceService startMicrophoneCapture:^(AVAudioPCMBuffer *buffer) {
            const float level = MSIMEVoiceInputLevel(buffer);
            dispatch_async(dispatch_get_main_queue(), ^{
                MSIMEInputController *liveController = weakSelf;
                if ([liveController ownsLiveVoiceToken:token] && !liveController->_liveVoiceProcessing)
                    [liveController->_voiceOverlay setInputLevel:level];
            });
        } deviceUID:deviceUID error:&error]) { [controller reportVoiceFailure:MSIMEVoiceFailureCapture]; return; }
        [controller voiceCaptureDidStart];
    };
    // A permission sheet can outlive the physical hold. Require a fresh hold
    // after authorization instead of starting capture after the key was released.
    const BOOL resumeAfterPermission = !_voiceHoldStarting;
    if (![self usesNativeHTTPVoice] && ![self usesNativeDoubaoVoice] && !MSIMEVoiceProviderSocket() &&
        _voiceService.speechAuthorizationStatus != SFSpeechRecognizerAuthorizationStatusAuthorized) {
        [self requestVoicePermissionForSpeech:YES resume:resumeAfterPermission];
        return;
    }
    if (_voiceService.microphoneAuthorizationStatus != AVAuthorizationStatusAuthorized) {
        [self requestVoicePermissionForSpeech:NO resume:resumeAfterPermission];
        return;
    }
    start();
}
- (void)openWebsite:(id)sender { (void)sender; [[NSWorkspace sharedWorkspace] openURL:[NSURL URLWithString:@"https://msime.app/"]]; }
- (void)showHelp:(id)sender { (void)sender; MSIMEOpenDesktopRoute(@"settings:help", NSWorkspace.sharedWorkspace, ^{ [[MSIMESupportWindowController sharedController] showPage:MSIMESupportPageHelp]; }); }
- (void)showAbout:(id)sender { (void)sender; MSIMEOpenDesktopRoute(@"settings:about", NSWorkspace.sharedWorkspace, ^{ [[MSIMESupportWindowController sharedController] showPage:MSIMESupportPageAbout]; }); }
- (void)showFeedback:(id)sender { (void)sender; MSIMEOpenDesktopRoute(@"settings:feedback", NSWorkspace.sharedWorkspace, ^{ [[MSIMESupportWindowController sharedController] showPage:MSIMESupportPageFeedback]; }); }
- (void)openCharacterPalette:(id)sender {
    (void)sender;
    if (_session && _activeClient) {
        NSDictionary *finished = [_session command:MSIME_FINISH_COMPOSITION error:nil];
        if (!finished) return;
        [self apply:finished];
    }
    [self showSystemCharacterPalette];
}
- (void)showAppearance:(id)sender {
    (void)sender;
    // The desktop application answers this route on its 外观 page, so the native window has to open
    // on its own 外观 page rather than on whichever one it happens to be showing. Which page the
    // route named was the one thing the fallback threw away.
    MSIMEOpenDesktopSettings(MSIMEDesktopSettingsPage::Appearance, NSWorkspace.sharedWorkspace, ^{
        [[MSIMEPreferencesWindowController sharedController] showAndActivateWithPageIdentifier:@"appearance"];
    });
}
- (void)showDictionary:(id)sender { (void)sender; MSIMEOpenDesktopRoute(@"settings:dictionary", NSWorkspace.sharedWorkspace, ^{ if (!self->_session) [self prepareSession]; if (!self->_session) return; self->_dictionaryWindow = [[MSIMEDictionaryWindowController alloc] initWithOptions:self->_session.hostOptions]; [self->_dictionaryWindow showWindow:nil]; MSIMEPresentWindow(self->_dictionaryWindow.window); }); }
- (void)prepareDictionary:(id)sender {
    (void)sender;
    if (_session && _activeClient) {
        NSDictionary *finished = [_session command:MSIME_FINISH_COMPOSITION error:nil];
        if (!finished) return;
        [self apply:finished];
        [_session setFocused:NO error:nil];
        [_session closeWithError:nil];
        _session = nil;
        [_preferencesTimer invalidate];
        _preferencesTimer = nil;
    }
    NSOpenPanel *panel = [NSOpenPanel openPanel];
    panel.canChooseFiles = NO; panel.canChooseDirectories = YES; panel.allowsMultipleSelection = NO;
    [panel beginWithCompletionHandler:^(NSModalResponse response) {
        if (response != NSModalResponseOK || !panel.URL) return;
        NSURL *support = [[NSFileManager defaultManager] URLsForDirectory:NSApplicationSupportDirectory inDomains:NSUserDomainMask].firstObject;
        NSURL *state = [support URLByAppendingPathComponent:@"app.msime.client" isDirectory:YES];
        [MSIMEDictionaryRuntime prepareResourcesDirectory:panel.URL.path stateRoot:state.path completion:^(NSDictionary *options, NSError *error) {
            if (!options) { NSAlert *alert = [NSAlert new]; alert.messageText = @"词库准备失败"; alert.informativeText = error.localizedDescription ?: @"无法准备词库"; [alert runModal]; return; }
            NSData *data = [NSJSONSerialization dataWithJSONObject:options options:0 error:nil];
            NSURL *target = [state URLByAppendingPathComponent:@"runtime-options.json"];
            [[NSFileManager defaultManager] createDirectoryAtURL:state withIntermediateDirectories:YES attributes:nil error:nil];
            [data writeToURL:target options:NSDataWritingAtomic error:nil];
        }];
    }];
}

// Every controller that has a session open, whether or not it is the one typing: IMK keeps a controller per text input client, and each holds the dictionary lock through its own session.
+ (NSHashTable<MSIMEInputController *> *)dictionarySessionHolders {
    static NSHashTable<MSIMEInputController *> *holders;
    static dispatch_once_t once;
    dispatch_once(&once, ^{
        holders = [NSHashTable weakObjectsHashTable];
        // Registered once, on the class, so one notification releases every controller exactly once. The distributed center holds notifications for a background process unless told to deliver them at once, and IMK never becomes the active application.
        [NSDistributedNotificationCenter.defaultCenter addObserver:self
            selector:@selector(dictionaryMaintenanceWillBegin:)
            name:MSIMEDictionaryMaintenanceWillBeginNotification object:nil
            suspensionBehavior:NSNotificationSuspensionBehaviorDeliverImmediately];
        // The native dictionary window runs in this process and posts locally.
        [NSNotificationCenter.defaultCenter addObserver:self selector:@selector(dictionaryMaintenanceWillBegin:)
            name:MSIMEDictionaryMaintenanceWillBeginNotification object:nil];
    });
    return holders;
}
+ (void)holdDictionarySession:(MSIMEInputController *)controller {
    [[self dictionarySessionHolders] addObject:controller];
}
+ (void)dictionaryMaintenanceWillBegin:(NSNotification *)notification {
    (void)notification;
    [self releaseQuiescedDictionarySessions];
}
+ (void)releaseQuiescedDictionarySessions {
    for (MSIMEInputController *controller in [self dictionarySessionHolders].allObjects)
        [controller releaseDictionarySessionIfQuiesced];
}
// Let go of the session, and with it the shared dictionary lock, while the settings window holds the maintenance lease. What was being typed is committed first, and work bound to the session is cancelled the way a focus change cancels it; the next key after the lease is gone opens a new session (prepareSession).
- (void)releaseDictionarySessionIfQuiesced {
    if (!_session || !MSIMEDictionaryQuiesced(_session.hostOptions)) return;
    [self cancelLiveVoiceInput];
    [self cancelDoubaoVoiceInput];
    [self cancelHTTPVoiceInput];
    MSIMEDeactivateVoice(_voiceService, _session, _voiceAudioMuter, _voiceOverlay,
        MSIMEVoiceProviderSocket(), _voiceGeneration);
    [self cancelCandidateTranslations];
    [self cancelCloudCandidates];
    _modifierTap.reset();
    _preferenceLoadState.reset();
    if (_activeClient) {
        NSDictionary *finished = [_session command:MSIME_FINISH_COMPOSITION error:nil];
        if (finished) [self apply:finished];
        else MSIMEApplyTransition(@{@"view": @{@"editing_text": @"", @"preedit": @"", @"caret_position": @0}}, (id<MSIMETextClient>)_activeClient);
    }
    [self discardGlossSensePage];
    _resumeDedicatedEnglish = [_view[@"dedicated_english"] isEqual:@YES];
    [_session setFocused:NO error:nil];
    [_session closeWithError:nil];
    _session = nil;
    _focusPending = YES;
    [_panel orderOut:nil];
    [[MSIMEInputController dictionarySessionHolders] removeObject:self];
}

- (void)activateServer:(id)sender {
    // A newly activated IME session may target a different document/client.
    // Never carry host-owned closings across that boundary - including one this host still owed
    // the previous document, which cannot be written into this one.
    _pendingPairedClosing = nil;
    _pairedPunctuation.clear();
    _englishPunctuation = {};
    [self clearSmartPunctuationSpaceConversion];
    [self clearSmartPunctuationSpaceRevert];
    [_voiceOverlay dismissFailure];
    _voicePermissionToken = nil;
    _voiceHoldShortcut.reset();
    if (_activeClient && _activeClient != sender) [self cancelLiveVoiceInput];
    if (_activeClient && _activeClient != sender) [self cancelDoubaoVoiceInput];
    if (_activeClient && _activeClient != sender) [self cancelHTTPVoiceInput];
    [self cancelCandidateTranslations];
    [self cancelCloudCandidates];
    [self resetCandidateAnchor];
    [self claimCandidatePanel];
    _modifierTap.reset();
    [super activateServer:sender];
    [self ensureAppearance];
    msime_macos_diagnostic_write("focus_in");
    if (_activeClient && _activeClient != sender) [self apply:[_session setFocused:NO error:nil]];
    [_appearance activateInputModeForApplication:[sender respondsToSelector:@selector(bundleIdentifier)] ? [sender bundleIdentifier] : nil];
    // The Chinese/English state is remembered per application and survives a restart, while the menu bar shows whichever mode was selected last; align the two as this client takes focus. That also covers a toggle made while no client could be asked to switch.
    [self syncSystemInputModeForClient:sender];
    _capsLock = ([NSEvent modifierFlags] & NSEventModifierFlagCapsLock) != 0;
    _toolbar = [MSIMEFloatingToolbarPanel sharedPanel];
    [_toolbar applyLightSkin:[_appearance resolvedSkinForDark:NO].tokens darkSkin:[_appearance resolvedSkinForDark:YES].tokens];
    [_toolbar applyLightToolbarSkin:msime::mac::ToolbarSkinTokens(_appearance.skinID.UTF8String, NO)
                            darkSkin:msime::mac::ToolbarSkinTokens(_appearance.skinID.UTF8String, YES)];
    [_toolbar activateForDelegate:self visible:_appearance.floatingToolbarEnabled];
    _activeClient = sender;
    _preferenceLoadState.reset();
    [[NSNotificationCenter defaultCenter] removeObserver:self name:MSIMEClientSessionDidReplaceSnapshotNotification object:nil];
    [[NSNotificationCenter defaultCenter] addObserver:self selector:@selector(snapshotSessionReplaced:) name:MSIMEClientSessionDidReplaceSnapshotNotification object:nil];
    MSIMESetBackendSelectionObservation([NSNotificationCenter defaultCenter], self, @selector(handwritingCandidateSelected:), YES);
    [self refreshFloatingToolbarState];
    [self ensureAppearance];
    _focusPending = _appearance.englishMode;
    if (!_appearance.englishMode) [self prepareSession];
    else [self startPreferencesMonitoring];
    [self requestCloudCandidatesConsentIfNeeded];
    [self commitPendingEmojiForClient:sender];
}

- (void)resolveCloudCandidatesConsentWithOptions:(NSDictionary *)options {
    if (![options isKindOfClass:NSDictionary.class]) return;
    id directory = options[@"preferences_directory"];
    id userData = options[@"user_data"];
    if (![directory isKindOfClass:NSString.class] || ![directory isAbsolutePath]) return;
    [_appearance resolveCloudCandidatesConsentWithPreferencesDirectory:directory
        userDataDirectory:[userData isKindOfClass:NSString.class] && [userData isAbsolutePath] ? userData : nil];
}

// One consent prompt per process at a time, however many controllers IMK creates.
static BOOL MSIMECloudConsentPrompting = NO;

/// Ask once, on a fresh profile, before the first cloud candidate query is sent.
///
/// The Windows installer asks this on its 联网功能 page. macOS has no installer step that every user passes through, and the IME types without the settings app ever being opened, so the IME process asks itself. Until an answer arrives nothing is sent; a prompt closed without an answer is asked again on the next activation.
- (void)requestCloudCandidatesConsentIfNeeded {
    if (!_appearance || _appearance.cloudCandidatesAnswered || MSIMECloudConsentPrompting) return;
    MSIMECloudConsentPrompting = YES;
    MSIMEAppearancePreferences *appearance = _appearance;
    __weak MSIMEInputController *weakSelf = self;
    // Off the activation path, so the client's first keystrokes are not held behind the prompt.
    dispatch_async(dispatch_get_main_queue(), ^{
        MSIMEInputController *controller = weakSelf;
        if (!controller || appearance.cloudCandidatesAnswered) { MSIMECloudConsentPrompting = NO; return; }
        [controller presentCloudConsent:^(NSNumber *enabled) {
            MSIMECloudConsentPrompting = NO;
            if (enabled) [appearance answerCloudCandidates:enabled.boolValue];
        }];
    });
}

static NSString *const MSIMECloudConsentTitle = @"联网功能";
static NSString *const MSIMECloudConsentMessage =
    @"拼音切分、候选排序和词频学习全部在本机完成，不联网。\n\n"
    @"启用云候选：输入过程中把正在输入的拼写通过 HTTPS 发送给 Google 的 input-tools 服务（inputtools.google.com），换回一条额外候选。已上屏的文本、词库内容和学习到的词频都不会发送。\n\n"
    @"这是唯一一项装完就会联网的功能。AI 联想、候选翻译、语音输入都需要你自己填入 API token 之后才会发出任何请求。之后可在「设置 → 输入」的「云候选」里更改。";

/// Show the consent prompt and report the choice: @YES, @NO, or nil when it closed without one. Overridden by tests.
///
/// Non-modal on purpose: a modal loop would stop this process from serving IMK while the prompt is up, which would freeze typing in every other app until it is answered. The bundle is LSBackgroundOnly, so it has to activate itself for the window to take focus.
- (void)presentCloudConsent:(void (^)(NSNumber *enabled))completion {
    static NSAlert *alert;
    static void (^pending)(NSNumber *);
    pending = [completion copy];
    alert = [NSAlert new];
    alert.messageText = MSIMECloudConsentTitle;
    alert.informativeText = MSIMECloudConsentMessage;
    NSButton *enable = [alert addButtonWithTitle:@"启用云候选"];
    NSButton *disable = [alert addButtonWithTitle:@"不启用"];
    static MSIMECloudConsentTarget *target;
    target = [MSIMECloudConsentTarget new];
    target.handler = ^(BOOL enabled) {
        [alert.window orderOut:nil];
        void (^finish)(NSNumber *) = pending;
        pending = nil;
        if (finish) finish(@(enabled));
        // Released after the button action returns; the target and the window are still on the stack here.
        dispatch_async(dispatch_get_main_queue(), ^{ alert = nil; target = nil; });
    };
    enable.target = target;
    enable.action = @selector(enable:);
    disable.target = target;
    disable.action = @selector(disable:);
    [alert layout];
    alert.window.level = NSFloatingWindowLevel;
    [NSApp activateIgnoringOtherApps:YES];
    [alert.window center];
    [alert.window makeKeyAndOrderFront:nil];
}

- (void)commitPendingEmojiForClient:(id)client {
    const BOOL hadPending = _emojiReturn.pending != nil;
    if (hadPending && _desktopEmojiCompletion && ![_desktopInputSession isAuthorizedPeerAlive]) {
        _emojiReturn.discard(_emojiReturn.generation);
        [self reportEmojiDeliveryFailure];
        return;
    }
    NSString *toolText = _emojiReturn.take(client, NSProcessInfo.processInfo.systemUptime);
    if (toolText) {
        BOOL committed = NO;
        [self invalidateSmartPunctuationShadow];
        @try { [client insertText:toolText replacementRange:NSMakeRange(NSNotFound, 0)]; committed = YES; }
        @catch (NSException *) { /* Never log input or client exception details. */ }
        if (committed) MSIMERecordTypingStatistics(_preferencesDirectory ?: [self runtimeOptions][@"preferences_directory"],
                                                    toolText, msime::mac::TypingSource::Local);
        if (_desktopEmojiCompletion) {
            MSIMEPanelTextCompletion completion = _desktopEmojiCompletion;
            _desktopEmojiCompletion = nil;
            completion(committed);
        } else if (!committed) [self reportEmojiDeliveryFailure];
    }
    else if (hadPending) [self reportEmojiDeliveryFailure];
}

- (void)reportEmojiDeliveryFailure {
    if (_desktopEmojiCompletion) {
        MSIMEPanelTextCompletion completion = _desktopEmojiCompletion;
        _desktopEmojiCompletion = nil;
        completion(NO);
        return;
    }
    Class bridge = NSClassFromString(@"MSIMEBackendWindowBridge");
    id shared = [bridge respondsToSelector:@selector(shared)] ? [bridge performSelector:@selector(shared)] : nil;
    if ([shared respondsToSelector:@selector(showEmojiDeliveryFailure)])
        [shared performSelector:@selector(showEmojiDeliveryFailure)];
}

- (void)handwritingCandidateSelected:(NSNotification *)notification {
    NSString *text = notification.userInfo[@"text"];
    if (![text isKindOfClass:NSString.class] || text.length == 0 || !_activeClient) return;
    [self invalidateSmartPunctuationShadow];
    @try {
        [_activeClient insertText:text replacementRange:NSMakeRange(NSNotFound, 0)];
        MSIMERecordTypingStatistics(_preferencesDirectory ?: [self runtimeOptions][@"preferences_directory"],
                                    text, msime::mac::TypingSource::Handwriting);
    } @catch (NSException *) { /* Never log input or client exception details. */ }
}

- (void)snapshotSessionReplaced:(NSNotification *)notification {
    if (notification.object != _session) return;
    [self resetCandidateAnchor];
    [self cancelCandidateTranslations];
    [self cancelCloudCandidates];
    _modifierTap.reset();
    _requestedPageSize = 0;
    _preferenceLoadState.reset();
    _focusPending = YES;
    if (_activeClient) {
        MSIMEApplyTransition(@{@"view": @{@"editing_text": @"", @"preedit": @"", @"caret_position": @0}}, (id<MSIMETextClient>)_activeClient);
    }
    _view = [_session viewWithError:nil] ?: @{};
    [self refreshFloatingToolbarState];
    [self hideCandidatePanel:"session_replaced"];
    [_keymapPanel orderOut:nil];
}

- (NSDictionary *)runtimeOptions { return MSIMELoadRuntimeOptions(); }

// What this host asks a session for, on top of what the options file carries.
//
// The file is shared with hosts that render a view differently, so a behaviour this host draws is
// requested here rather than written into it. Its own function because the session it produces is
// built inside prepareSession, where a test would have to stand up a whole Engine to see it.
static NSDictionary *MSIMESessionOptions(NSDictionary *runtimeOptions) {
    if (![runtimeOptions isKindOfClass:NSDictionary.class]) return nil;
    NSMutableDictionary *requested = [runtimeOptions mutableCopy];
    // This host draws view.phrase_prefix, so a phrase being assembled out of several selections
    // stays in the composition instead of arriving in the document one piece at a time.
    requested[@"phrase_preedit"] = @YES;
    return requested;
}

// The two reasons a session can be missing, as its own function for the reason MSIMESessionOptions is: what
// it decides is reachable in a test, while the branch it is decided in needs a whole Engine to enter.
// Nothing prepared the dictionary, or something did and the Engine would not take it - a different next step
// each, which is the whole point of saying which one it was.
static NSString *MSIMESessionUnavailableReason(NSDictionary *options) {
    return options ? @"session refused the runtime options" : @"no usable runtime options";
}

// Why there is no session, said once per reason.
//
// A controller without a session passes every key straight to the application, which is indistinguishable from the user having chosen English - and until now it left nothing behind at all: the error from initWithOptions: was discarded and the path where the options themselves are missing said nothing. That silence is what a dictionary that was never prepared looks like, and it is what #912 was diagnosed from: the report was "the input method hands English keys to the application in Chinese mode", the cause was guessed at as a hardcoded resource path, and the fix landed in src/dictionary/DictionaryRuntime.mm, which no target compiles. The two reasons below are the ones that need telling apart - nothing prepared the dictionary, or something prepared it and the Engine would not take it - and each has a different next step.
//
// A category rather than the underlying error, because the Host API's errors can name private directories (see the note on MSIMERefreshRuntimeOptionsWith) and this goes to the unified log, which leaves the machine in a sysdiagnose. NSLog rather than the diagnostic log for the same reason the diagnostic log cannot carry it: it is configured from the preferences directory, which arrives in the very runtime options that are missing here.
- (void)reportSessionUnavailable:(NSString *)reason {
    if ([_sessionUnavailableReason isEqualToString:reason]) return;
    _sessionUnavailableReason = [reason copy];
    NSLog(@"MSIME has no input session (%@); keys pass through to the application until one opens", reason);
    msime_macos_diagnostic_writef("session_unavailable reason=%s", reason.UTF8String);
}

- (void)prepareSession {
    BOOL reopened = NO;
    if (!_session) {
        NSDictionary *options = MSIMESessionOptions([self runtimeOptions]);
        // Dictionary maintenance is running: open nothing, so keys pass through to the application until the lease is gone. The preferences timer keeps running, so settings still apply meanwhile.
        if (options && MSIMEDictionaryQuiesced(options)) {
            if (!_preferencesTimer) [self startPreferencesMonitoring];
            return;
        }
        // Before the session exists, so nothing this process writes can be mistaken for an earlier install.
        [self resolveCloudCandidatesConsentWithOptions:options];
        if (options) {
            _session = [[MSIMEClientSession alloc] initWithOptions:options error:nil];
            _requestedPageSize = 0;
            id directory = options[@"preferences_directory"];
            if ([directory isKindOfClass:NSString.class] && [directory isAbsolutePath]) _preferencesDirectory = [directory copy];
            if (_session) {
                [MSIMEInputController holdDictionarySession:self];
                reopened = _resumeDedicatedEnglish;
                _resumeDedicatedEnglish = NO;
            }
        }
        if (_session) _sessionUnavailableReason = nil;
        else [self reportSessionUnavailable:MSIMESessionUnavailableReason(options)];
    }
    [self syncPageSize];
    if (_session) {
        [self syncPunctuation];
        [self syncCharacterWidth];
        if (reopened) {
            NSDictionary *view = [_session setDedicatedEnglishEnabled:YES error:nil];
            if (view) _view = view;
        }
        [self apply:[_session setFocused:YES error:nil]];
        _focusPending = NO;
    }
    [self startPreferencesMonitoring];
}

- (void)startPreferencesMonitoring {
    if (!_preferencesDirectory) {
        NSDictionary *options = [self runtimeOptions];
        id directory = options[@"preferences_directory"];
        if ([directory isKindOfClass:NSString.class] && [directory isAbsolutePath]) _preferencesDirectory = [directory copy];
        // English-mode activation reaches here without prepareSession; a directory set earlier was already resolved where it was set.
        [self resolveCloudCandidatesConsentWithOptions:options];
    }
    if (_activeClient && _preferencesDirectory) {
        // Activation may happen after the setting changed while the IMK process was not running.
        // Load once here; subsequent changes arrive through the distributed notification above.
        MSIMEReloadTypingStatisticsEnabled(_preferencesDirectory);
        [_appearance setTranslationPreferencesDirectory:_preferencesDirectory];
        [_preferencesTimer invalidate];
        __weak MSIMEInputController *weakSelf = self;
        _preferencesTimer = [NSTimer scheduledTimerWithTimeInterval:1.0 repeats:YES block:^(NSTimer *timer) {
            // The run loop, not the controller, keeps this timer; it stops once its owner is gone.
            MSIMEInputController *owner = weakSelf;
            if (!owner) { [timer invalidate]; return; }
            // Catches a maintenance notification that never arrived.
            [MSIMEInputController releaseQuiescedDictionarySessions];
            [owner reloadPreferences];
        }];
        [self reloadPreferences];
    }
}

- (NSDictionary *)readPreferencesSnapshotInDirectory:(NSString *)directory error:(NSError **)error {
    return [MSIMEClientSession loadPreferencesInDirectory:directory error:error];
}

- (NSDictionary *)recoverPreferencesInDirectory:(NSString *)directory error:(NSError **)error {
    return [MSIMEClientSession recoverPreferencesInDirectory:directory error:error];
}

// The Windows source repairs an unparseable config.toml as the IME starts (InitImeConfig before LoadImeConfig). Here the document is polled by every controller from a background queue, so the repair is claimed once per directory per process under a lock: a document that cannot be repaired, or a read that keeps failing for another reason, is not retried every second or once more for each client application.
static BOOL MSIMEClaimPreferenceRecovery(NSString *directory) {
    static NSMutableSet<NSString *> *claimed;
    static dispatch_once_t once;
    dispatch_once(&once, ^{ claimed = [NSMutableSet set]; });
    @synchronized(claimed) {
        if ([claimed containsObject:directory]) return NO;
        [claimed addObject:directory];
        return YES;
    }
}

- (void)completePreferenceLoad:(NSDictionary *)snapshot error:(NSError *)error generation:(uint64_t)generation
                       session:(MSIMEClientSession *)session client:(id)client {
    if (!_preferenceLoadState.finish(generation)) return;
    if (!snapshot || error) {
        msime_macos_diagnostic_write("preferences_load_failed");
        return;
    }
    if (!_activeClient || _activeClient != client || _session != session) return;
    // The poll reads this document once a second. Applying an unchanged one costs a full pass over
    // every preference, another trip into the Engine and a diagnostic line, a second at a time, for
    // nothing - and it buried the log this was found in. A revision of zero predates the field and
    // is always applied.
    const uint64_t revision = [snapshot[@"revision"] isKindOfClass:NSNumber.class]
        ? [snapshot[@"revision"] unsignedLongLongValue] : 0;
    if (!_preferenceLoadState.needsApply(revision)) return;
    _preferenceLoadState.applied(revision);
    NSDictionary *preferences = snapshot[@"preferences"];
    NSMutableDictionary *inputPreferences = [preferences isKindOfClass:NSDictionary.class] ? [preferences mutableCopy] : nil;
    id inlinePreedit = inputPreferences[@"tsf_preedit_style"];
    if (![inlinePreedit isKindOfClass:NSString.class] ||
        ![@[@"raw", @"pinyin", @"empty"] containsObject:inlinePreedit])
        inputPreferences[@"tsf_preedit_style"] = @"raw";
    if (!session) {
        if (inputPreferences) [_appearance applySharedInputPreferences:inputPreferences];
        [self applySharedToolbarPreferences:snapshot[@"preferences"]];
        return;
    }
    NSError *updateError = nil;
    NSDictionary *result = [session updatePreferencesSnapshot:snapshot error:&updateError];
    // Failed loads/updates retain the existing window appearance and runtime.
    if (result && !updateError) {
        if (inputPreferences) [_appearance applySharedInputPreferences:inputPreferences];
        [self applySharedToolbarPreferences:snapshot[@"preferences"]];
        _view = [session viewWithError:nil] ?: result[@"view"];
        if (_view) {
            MSIMEApplyTransitionWithPreeditStyle(@{@"view": _view}, (id<MSIMETextClient>)_activeClient,
                                                 _appearance.inlinePreeditStyle);
        }
        [self refreshFloatingToolbarState];
        // Another surface - the shared settings page, an account push - can have changed the scheme.
        [self syncSystemInputModeForClient:_activeClient];
        [self renderCandidates];
        [self synchronizeCloudCandidates];
    [self scheduleSettledRerank];
        [self synchronizeCandidateGloss];
        [self synchronizeTargetGloss];
        [self synchronizeOnDeviceGloss];
        [self synchronizeAccountGloss:[self currentAccountGlossRequest]];
        [self synchronizeCustomTranslations];
        [self synchronizeAITranslations];
    } else {
        msime_macos_diagnostic_write("preferences_apply_failed");
    }
}

- (void)reloadPreferences {
    if (!_activeClient || !_preferencesDirectory || _preferenceSaveState.saving || !_preferenceLoadState.begin()) return;
    const uint64_t generation = _preferenceLoadState.generation;
    MSIMEClientSession *session = _session;
    id client = _activeClient;
    NSString *directory = [_preferencesDirectory copy];
    __weak MSIMEInputController *weakSelf = self;
    dispatch_async(dispatch_get_global_queue(QOS_CLASS_UTILITY, 0), ^{
        MSIMEInputController *current = weakSelf;
        if (!current) return;
        NSError *error = nil;
        NSDictionary *snapshot = [current readPreferencesSnapshotInDirectory:directory error:&error];
        // A document that is not JSON at all is backed up and repaired here, so the IME comes back on the salvaged values instead of running on whatever it last applied. The host refuses a well-formed document it cannot read - most likely a newer build's - and leaves that to the settings page's explicit repair.
        NSString *backupName = nil;
        if (!snapshot && error && MSIMEClaimPreferenceRecovery(directory)) {
            NSError *recoveryError = nil;
            NSDictionary *recovery = [current recoverPreferencesInDirectory:directory error:&recoveryError];
            NSDictionary *recoveredSnapshot = [recovery[@"snapshot"] isKindOfClass:NSDictionary.class] ? recovery[@"snapshot"] : nil;
            if (recoveredSnapshot && !recoveryError) {
                snapshot = recoveredSnapshot;
                error = nil;
                if ([recovery[@"recovered"] isEqual:@YES])
                    backupName = [recovery[@"backup_name"] isKindOfClass:NSString.class] ? recovery[@"backup_name"] : @"";
            } else if (recoveryError) {
                error = recoveryError;
            }
        }
        dispatch_async(dispatch_get_main_queue(), ^{
            [weakSelf completePreferenceLoad:snapshot error:error generation:generation session:session client:client];
            // After the completion, which is what configures the diagnostic log from the repaired document.
            if (backupName)
                msime_macos_diagnostic_write(std::string("preferences_recovered backup=") + (backupName.UTF8String ?: ""));
        });
    });
}

- (void)applySharedToolbarPreferences:(NSDictionary *)preferences {
    NSDictionary *diagnostic = [preferences isKindOfClass:NSDictionary.class] ? preferences[@"diagnostic_log"] : nil;
    const BOOL diagnosticEnabled = [diagnostic isKindOfClass:NSDictionary.class] &&
        [diagnostic[@"server"] isKindOfClass:NSNumber.class] &&
        CFGetTypeID((__bridge CFTypeRef)diagnostic[@"server"]) == CFBooleanGetTypeID() &&
        [diagnostic[@"server"] boolValue];
    const std::string directory = _preferencesDirectory.UTF8String ? _preferencesDirectory.UTF8String : "";
    msime_macos_diagnostic_configure(directory, diagnosticEnabled);
    if (diagnosticEnabled) msime_macos_diagnostic_write("preferences_applied");
    if ([preferences isKindOfClass:NSDictionary.class]) {
        id wubiCodeHint = preferences[@"wubi_code_hint"];
        if ([wubiCodeHint isKindOfClass:NSNumber.class] &&
            CFGetTypeID((__bridge CFTypeRef)wubiCodeHint) == CFBooleanGetTypeID())
            _wubiCodeHintEnabled = [wubiCodeHint boolValue];
        _voiceThemePreferences = [preferences copy];
        _menuThemePreferences = [preferences copy];
        if (_voiceOverlay) [_voiceOverlay applyThemePreferences:preferences];
    }
    if (MSIMEApplySharedVoicePreferences(preferences[@"voice_input"], NSUserDefaults.standardUserDefaults))
        _voicePermissionToken = nil;
    if (!MSIMEVoiceInputEnabled(NSUserDefaults.standardUserDefaults))
        [self finishVoiceInputForDisable];
    BOOL translationChanged = NO;
    BOOL candidateTranslationsEnabled = _appearance.candidateTranslations;
    BOOL candidateEnglishGlossEnabled = _appearance.candidateEnglishGloss;
    id glossEnabled = preferences[@"candidate_translations"];
    if ([glossEnabled isKindOfClass:NSNumber.class] && CFGetTypeID((__bridge CFTypeRef)glossEnabled) == CFBooleanGetTypeID()) {
        translationChanged = ![_glossEnabled isEqual:glossEnabled];
        _glossEnabled = glossEnabled;
        candidateTranslationsEnabled = [glossEnabled boolValue];
    }
    id englishGlossEnabled = preferences[@"candidate_english_gloss"];
    if ([englishGlossEnabled isKindOfClass:NSNumber.class] &&
        CFGetTypeID((__bridge CFTypeRef)englishGlossEnabled) == CFBooleanGetTypeID()) {
        translationChanged |= candidateEnglishGlossEnabled != [englishGlossEnabled boolValue];
        candidateEnglishGlossEnabled = [englishGlossEnabled boolValue];
    }
    id target = preferences[@"translation_target_language"];
    if ([@[@"en", @"fr", @"ja", @"es", @"ru", @"de", @"ko"] containsObject:target]) {
        translationChanged |= ![_glossTargetLanguage isEqual:target];
        _glossTargetLanguage = [target copy];
    }
    if (target || preferences[@"translation_secondary_language"] ||
        [preferences.allKeys containsObject:@"translation_secondary_language"]) {
        NSArray *targets = MSIMETranslationTargetsFromPreferences(preferences, _glossTargetLanguage ?: @"en");
        translationChanged |= ![_glossTargetLanguages isEqual:targets];
        _glossTargetLanguages = [targets copy];
    }
    NSDictionary *custom = preferences[@"custom_translation"];
    if ([custom isKindOfClass:NSDictionary.class]) {
        if (_customTranslationConfig && ![_customTranslationConfig isEqual:custom]) [[MSIMETranslationCache sharedCache] clear];
        translationChanged |= ![_customTranslationConfig isEqual:custom];
        _customTranslationConfig = [custom copy];
    }
    NSDictionary *tencent = preferences[@"tencent_tmt"];
    if ([tencent isKindOfClass:NSDictionary.class]) {
        if (_tencentTranslationConfig && ![_tencentTranslationConfig isEqual:tencent]) [[MSIMETranslationCache sharedCache] clear];
        translationChanged |= ![_tencentTranslationConfig isEqual:tencent];
        _tencentTranslationConfig = [tencent copy];
    }
    NSDictionary *niuTrans = preferences[@"niutrans"];
    if ([niuTrans isKindOfClass:NSDictionary.class]) {
        if (_niuTransConfig && ![_niuTransConfig isEqual:niuTrans]) [[MSIMETranslationCache sharedCache] clear];
        translationChanged |= ![_niuTransConfig isEqual:niuTrans];
        _niuTransConfig = [niuTrans copy];
    }
    if (translationChanged || (_glossEnabled && !_glossEnabled.boolValue)) {
        // None of these settings feed the AI request, so a pending AI suggestion survives them; AI config changes are caught by the _aiQuery identity check on the next render.
        [self cancelCustomTranslations];
        if (!candidateEnglishGlossEnabled) { [self cancelCandidateGloss]; [self cancelTargetGloss]; }
        NSDictionary *view = [_session viewWithError:nil];
        if (!candidateTranslationsEnabled && !candidateEnglishGlossEnabled && view)
            [_session applyTranslations:@[] generation:[view[@"generation"] unsignedLongLongValue] error:nil];
    }
    id pageSize = preferences[@"candidate_page_size"];
    if ([pageSize isKindOfClass:NSNumber.class] &&
        CFGetTypeID((__bridge CFTypeRef)pageSize) != CFBooleanGetTypeID() &&
        [pageSize doubleValue] == [pageSize integerValue] && [pageSize integerValue] >= 1 && [pageSize integerValue] <= 9 &&
        [pageSize unsignedIntegerValue] != _requestedPageSize) _requestedPageSize = 0;
    [_appearance applySharedInputPreferences:preferences];
    [_appearance applySharedCandidatePreferences:preferences];
    if (!_appearance.inputModeHUD) [[MSIMEInputModeHUDPanel sharedPanel] orderOut:nil];
    [_appearance applySharedAssistancePreferences:preferences];
    [_appearance applySharedToolbarPreferences:preferences];
    [_appearance applySharedLocalModes:preferences[@"local_modes"]];
    [self refreshFloatingToolbarState];
    Class bridge = NSClassFromString(@"MSIMEBackendWindowBridge");
    id shared = [bridge respondsToSelector:@selector(shared)] ? [bridge performSelector:@selector(shared)] : nil;
    if ([shared respondsToSelector:@selector(applyEmojiPreferences:)])
        [shared performSelector:@selector(applyEmojiPreferences:) withObject:preferences];
    if ([shared respondsToSelector:@selector(applyHandwritingPreferences:)])
        [shared performSelector:@selector(applyHandwritingPreferences:) withObject:preferences];
    [[MSIMEScreenKeyboardPanel sharedPanel] applyThemePreferences:preferences];
    [_toolbar applyThemePreferences:preferences];
    [_toolbar applySizingPreferences:preferences];
    NSDictionary *toolbar = preferences[@"floating_toolbar"];
    id enabled = [toolbar isKindOfClass:NSDictionary.class] ? toolbar[@"enabled"] : nil;
    if ([enabled isKindOfClass:NSNumber.class]) {
        [_appearance applySharedToolbarVisibility:[enabled boolValue]];
        [_toolbar setVisible:_appearance.floatingToolbarEnabled forDelegate:self];
    }
}

- (void)deactivateServer:(id)sender {
    [[MSIMEInputModeHUDPanel sharedPanel] orderOut:nil];
    [self flushPendingPairedClosing];
    _pairedPunctuation.clear();
    // A delayed callback from the previous client must not tear down the
    // active client's composition, panels, monitoring or pending modifier tap.
    if (!sender || sender != _activeClient) return;
    _backspaceHoldArmed = NO;
    [self clearSmartPunctuationSpaceConversion];
    [self clearSmartPunctuationSpaceRevert];
    [self invalidateSmartPunctuationShadow];
    msime_macos_diagnostic_write("focus_out");
    _voicePermissionToken = nil;
    _voiceHoldShortcut.reset();
    [self cancelLiveVoiceInput];
    [self cancelDoubaoVoiceInput];
    [self cancelHTTPVoiceInput];
    MSIMEDeactivateVoice(_voiceService, _session, _voiceAudioMuter, _voiceOverlay,
        MSIMEVoiceProviderSocket(), _voiceGeneration);
    [self cancelCandidateTranslations];
    [self cancelCloudCandidates];
    [self discardGlossSensePage];
    _modifierTap.reset();
    MSIMESetBackendSelectionObservation([NSNotificationCenter defaultCenter], self, @selector(handwritingCandidateSelected:), NO);
    _preferenceLoadState.reset();
    // A focus-out is the reference's ClientSuspended, which never changes floating-toolbar visibility: the toolbar keeps this controller as its owner and stays on screen until the next activateServer: hands it on or the user selects another input source.
    [_keymapPanel orderOut:nil];
    [_preferencesTimer invalidate];
    _preferencesTimer = nil;
    if (_session) [self apply:[_session setFocused:NO error:nil]];
    [self resetCandidateAnchor];
    [self hideCandidatePanel:"focus_out"];
    _activeClient = nil;
    [super deactivateServer:sender];
}

- (void)floatingToolbarDidRequestToggleInputMode:(MSIMEFloatingToolbarPanel *)toolbar {
    (void)toolbar;
    if (!_appearance.englishMode && [_view[@"dedicated_english"] isEqual:@YES]) [self setDedicatedEnglishInputMode:NO];
    else [self setEnglishInputMode:!_appearance.englishMode];
}
- (void)floatingToolbarDidRequestTogglePunctuation:(MSIMEFloatingToolbarPanel *)toolbar {
    (void)toolbar;
    [self ensureAppearance];
    if (_session && _activeClient && [_view[@"editing_text"] length]) {
        NSDictionary *finished = [_session command:MSIME_FINISH_COMPOSITION error:nil];
        if (!finished) return;
        [self apply:finished];
    }
    if ([_appearance.punctuationLock isEqual:@"chinese"] || [_appearance.punctuationLock isEqual:@"english"]) {
        _appearance.runtimeChinesePunctuation = [_appearance.punctuationLock isEqual:@"chinese"];
        [self syncPunctuation];
        [self refreshFloatingToolbarState];
        return;
    }
    // Like the reference's compartment, the toggle is this app's runtime state; the saved value stays the starting point.
    _appearance.runtimeChinesePunctuation = !_appearance.runtimeChinesePunctuation;
    [self syncPunctuation];
    [self refreshFloatingToolbarState];
}
- (void)floatingToolbarDidRequestToggleFullWidth:(MSIMEFloatingToolbarPanel *)toolbar { (void)toolbar; [self toggleRuntimeFullWidthInput]; }
- (void)toggleRuntimeFullWidthInput {
    [self ensureAppearance];
    _appearance.runtimeFullWidthInput = !_appearance.runtimeFullWidthInput;
    [self syncCharacterWidth];
    [self refreshFloatingToolbarState];
}
- (void)floatingToolbarDidRequestToggleTraditionalOutput:(MSIMEFloatingToolbarPanel *)toolbar { (void)toolbar; _appearance.traditionalOutput = !_appearance.traditionalOutput; }
- (void)floatingToolbarDidRequestOpenCharacterPalette:(MSIMEFloatingToolbarPanel *)toolbar { (void)toolbar; [self openCharacterPalette:nil]; }
- (void)floatingToolbarDidRequestOpenEmoji:(MSIMEFloatingToolbarPanel *)toolbar { (void)toolbar; [self showEmoji:nil]; }
- (void)floatingToolbarDidRequestOpenHandwriting:(MSIMEFloatingToolbarPanel *)toolbar { (void)toolbar; [self showHandwriting:nil]; }
- (void)floatingToolbarDidRequestOpenScreenKeyboard:(MSIMEFloatingToolbarPanel *)toolbar { (void)toolbar; [self showScreenKeyboard:nil]; }
- (void)floatingToolbarDidRequestToggleVoice:(MSIMEFloatingToolbarPanel *)toolbar { (void)toolbar; [self showVoicePanel]; }
- (void)floatingToolbarDidRequestOpenSettings:(MSIMEFloatingToolbarPanel *)toolbar { (void)toolbar; [self showAppearance:nil]; }
- (void)floatingToolbarDidRequestCheckForUpdates:(MSIMEFloatingToolbarPanel *)toolbar {
    (void)toolbar;
    [self checkForUpdates:nil];
}
- (void)floatingToolbarDidRequestOpenWebsite:(MSIMEFloatingToolbarPanel *)toolbar {
    (void)toolbar;
    [[NSWorkspace sharedWorkspace] openURL:[NSURL URLWithString:@"https://msime.app/"]];
}
- (void)floatingToolbarDidRequestHide:(MSIMEFloatingToolbarPanel *)toolbar {
    (void)toolbar;
    _appearance.floatingToolbarEnabled = NO;
    [_toolbar setVisible:NO forDelegate:self];
}

// The menu item writes the same preference the settings page's checkbox writes, so the two never disagree and the choice survives a restart. Showing or hiding it acts through the toolbar's current owner: the controller that last activated keeps the toolbar across focus-outs, so it can toggle it even while no client is focused, and a stale controller's request is ignored by the panel's owner check.
- (void)toggleFloatingToolbar:(id)sender {
    (void)sender;
    const BOOL enabled = !_appearance.floatingToolbarEnabled;
    _appearance.floatingToolbarEnabled = enabled;
    [_toolbar setVisible:enabled forDelegate:self];
}



- (NSUInteger)recognizedEvents:(id)sender {
    (void)sender;
    return NSEventMaskKeyDown | NSEventMaskKeyUp | NSEventMaskFlagsChanged;
}

- (void)restartCurrentInputMethod {
    MSIMELaunchInputSourceReregistration(NSBundle.mainBundle.bundleURL, NSWorkspace.sharedWorkspace,
        ^(BOOL launched) {
            if (launched) [NSApp terminate:nil];
            else NSBeep();
        });
}

- (void)terminateCurrentInputMethod {
    [NSApp terminate:nil];
}

// Whether word-to-character is enabled and bound to the pair this event belongs to. Shared by the paging
// exclusion and the branch that acts on it so the two can never disagree about who owns the key.
- (BOOL)wordCharacterClaimsEvent:(NSEvent *)event {
    NSDictionary *wordCharacter = [_appearance wordCharacterOptions];
    if (![wordCharacter[@"enabled"] boolValue]) return NO;
    NSString *characters = event.charactersIgnoringModifiers;
    if (characters.length != 1) return NO;
    const unichar character = [characters characterAtIndex:0];
    const BOOL brackets = [wordCharacter[@"keys"] isEqual:@"brackets"];
    return msime::mac::IsPhysicalWordCharacterKey(event.keyCode, brackets, static_cast<char>(character)) &&
        (character == (brackets ? '[' : '-') || character == (brackets ? ']' : '='));
}

// The height the horizontal panel keeps under every candidate for its glosses, whether or not this page
// has any yet. Sizing by content instead means the first composition reserves nothing - the gloss request
// is debounced and answered seconds later, so no candidate carries one - and the panel grows the moment
// the answer lands, which is the jump the reservation exists to prevent. Reserved per configured target
// language, so a single-language setup does not pay for a row it will never fill.
//
// Horizontal only. Vertical draws the gloss on the candidate's own row, where it costs width rather than
// height, and width still follows the content: reserving it would widen the panel for nothing.
- (CGFloat)reservedGlossHeightForFont:(NSFont *)glossFont {
    if (_appearance.vertical) return 0;
    if (!_appearance.candidateTranslations && !_appearance.candidateEnglishGloss) return 0;
    if (_glossEnabled && !_glossEnabled.boolValue && !_appearance.candidateEnglishGloss) return 0;
    const NSUInteger lines = MIN(MAX(_glossTargetLanguages.count, (NSUInteger)1), (NSUInteger)2);
    NSString *placeholder = lines > 1 ? @"X\nX" : @"X";
    return MSIMETranslationTextSize(placeholder, glossFont).height + 4;
}

// Every key down leaves through here, so the smart punctuation shadow sees each one exactly once, after it has been handled and with what became of it. Events this host posted itself (the voice sendinput route and the smart punctuation rewrite) are skipped: whoever posted them has already recorded what they carry.
- (BOOL)handleEvent:(NSEvent *)event client:(id)sender {
    const bool timed = msime_macos_diagnostic_enabled();
    const uint64_t started = timed ? clock_gettime_nsec_np(CLOCK_UPTIME_RAW) : 0;
    _smartPunctuationShadowWritten = NO;
    const BOOL handled = [self handleKeyEvent:event client:sender];
    CGEventRef nativeEvent = event.CGEvent;
    const BOOL selfPosted = nativeEvent && CGEventGetIntegerValueField(nativeEvent, kCGEventSourceUserData) == MSIMEVoiceCommitEventTag;
    if (event.type == NSEventTypeKeyDown && sender && !selfPosted) [self noteKeyForSmartPunctuationShadow:event eaten:handled];
    if (!handled) [self recordPassthroughKey:event client:sender];
    const double elapsedMs = timed ? static_cast<double>(clock_gettime_nsec_np(CLOCK_UPTIME_RAW) - started) / 1e6 : 0;
    if (timed && elapsedMs >= 8.0) {
        const NSEventType type = event.type;
        const char *label = type == NSEventTypeKeyDown ? "down" : type == NSEventTypeKeyUp ? "up" : type == NSEventTypeFlagsChanged ? "flags" : "other";
        msime_macos_diagnostic_writef("[key-latency] stage=handle type=%s handled=%d elapsed_ms=%.3f", label, handled ? 1 : 0, elapsedMs);
    }
    return handled;
}

- (void)recordPassthroughKey:(NSEvent *)event client:(id)sender {
    if (!MSIMETypingStatisticsEnabled.load(std::memory_order_relaxed)) return;
    if (event.type != NSEventTypeKeyDown || !sender) return;
    CGEventRef nativeEvent = event.CGEvent;
    if (nativeEvent && CGEventGetIntegerValueField(nativeEvent, kCGEventSourceUserData) == MSIMEVoiceCommitEventTag) return;
    NSString *characters = event.characters;
    if (characters.length != 1) return;
    if (!msime::mac::ShouldCountPassthroughCharacter([characters characterAtIndex:0], (event.modifierFlags & NSEventModifierFlagControl) != 0, (event.modifierFlags & NSEventModifierFlagCommand) != 0)) return;
    const msime::mac::TypingSource source = _appearance.englishMode ? msime::mac::TypingSource::English : MSIMEResolveTypingSource(_view, _view, MSIMEStatisticsHostOptions(_session), NO);
    MSIMERecordTypingStatistics(_preferencesDirectory ?: MSIMEStatisticsHostOptions(_session)[@"preferences_directory"], characters, source);
}

- (BOOL)handleKeyEvent:(NSEvent *)event client:(id)sender {
    CGEventRef nativeEvent = event.CGEvent;
    if (nativeEvent && CGEventGetIntegerValueField(nativeEvent, kCGEventSourceUserData) == MSIMEVoiceCommitEventTag) return NO;
    if (event.type != NSEventTypeKeyDown && event.type != NSEventTypeKeyUp && event.type != NSEventTypeFlagsChanged) return NO;
    const BOOL capsLock = (event.modifierFlags & NSEventModifierFlagCapsLock) != 0;
    if (_capsLock != capsLock) {
        _capsLock = capsLock;
        [self refreshFloatingToolbarState];
    }
    if (!sender) {
        _voicePermissionToken = nil;
        [_voiceOverlay dismissFailure];
        _modifierTap.reset();
        _voiceHoldShortcut.reset();
        _backspaceHoldArmed = NO;
        [self clearSmartPunctuationSpaceConversion];
        [self clearSmartPunctuationSpaceRevert];
        [self invalidateSmartPunctuationShadow];
        return NO;
    }
    [self ensureAppearance];
    if (sender != _activeClient) {
        [_voiceOverlay dismissFailure];
        _voicePermissionToken = nil;
        _voiceHoldShortcut.reset();
        [self cancelLiveVoiceInput];
        [self cancelDoubaoVoiceInput];
        [self cancelHTTPVoiceInput];
        [self cancelCandidateTranslations];
        [self cancelCloudCandidates];
        [self resetCandidateAnchor];
        _modifierTap.reset();
        _preferenceLoadState.reset();
        [self flushPendingPairedClosing];
        _pairedPunctuation.clear();
        [self resetSmartPunctuationState];
        [self clearSmartPunctuationSpaceConversion];
        [self clearSmartPunctuationSpaceRevert];
        _backspaceHoldArmed = NO;
        // Clear the previous client's marked text before accepting the new focus.
        [self apply:[_session setFocused:NO error:nil]];
        _activeClient = sender;
        // Whatever the previous client was last given says nothing about this one.
        [self invalidateSmartPunctuationShadow];
        [_appearance activateInputModeForApplication:[sender respondsToSelector:@selector(bundleIdentifier)] ? [sender bundleIdentifier] : nil];
        [self syncSystemInputModeForClient:sender];
        // Punctuation and width are per app, so the new client's values reach the Engine before it types.
        [self syncPunctuation];
        [self syncCharacterWidth];
        [self refreshFloatingToolbarState];
        _focusPending = _appearance.englishMode;
        if (!_appearance.englishMode) [self apply:[_session setFocused:YES error:nil]];
    }
    NSUserDefaults *voiceDefaults = NSUserDefaults.standardUserDefaults;
    if (event.type == NSEventTypeKeyDown && event.keyCode == 53) [_voiceOverlay dismissFailure];
    if (_voicePermissionToken && event.type == NSEventTypeKeyDown && event.keyCode == 53) {
        _voicePermissionToken = nil; _voiceHoldShortcut.reset(); _modifierTap.reset(); return YES;
    }
    const BOOL voiceEnabled = MSIMEVoiceInputEnabled(voiceDefaults);
    if (!voiceEnabled) _voiceHoldShortcut.reset();
    const auto voiceShortcut = voiceEnabled ? _voiceHoldShortcut.observe(event, {
        [voiceDefaults boolForKey:@"MSIMEClientVoiceHotkeyRightAlt"] != NO,
        [voiceDefaults boolForKey:@"MSIMEClientVoiceHotkeyCtrlCommand"] != NO,
        [voiceDefaults boolForKey:@"MSIMEClientVoiceHotkeyCtrlOption"] != NO,
        [voiceDefaults objectForKey:@"MSIMEClientVoiceHotkeyHoldSpace"] == nil || [voiceDefaults boolForKey:@"MSIMEClientVoiceHotkeyHoldSpace"]
    }, _voiceService.active) : MSIMEVoiceHoldShortcut::Result{};
    if (voiceShortcut.consumed || voiceShortcut.action != MSIMEVoiceHoldShortcut::Action::None) _modifierTap.reset();
    // MSIME-Windows shows the overlay's cancel and confirm buttons once the hold is locked (ControlCommand::Lock).
    if (voiceShortcut.consumed && _voiceHoldShortcut.locked() && _voiceService.active) [_voiceOverlay setRecordingLocked:YES];
    if (voiceShortcut.action == MSIMEVoiceHoldShortcut::Action::Toggle) {
        _voiceHoldStarting = !voiceShortcut.onRelease && !_voiceService.active;
        if (!voiceShortcut.onRelease || _voiceHoldGeneration == _voiceGeneration) [self toggleVoiceInput:nil];
        _voiceHoldStarting = NO;
        if (!voiceShortcut.onRelease) _voiceHoldGeneration = _voiceGeneration;
    }
    else if (voiceShortcut.action == MSIMEVoiceHoldShortcut::Action::Cancel) {
        [self cancelHTTPVoiceInput]; [self cancelDoubaoVoiceInput]; [self cancelLiveVoiceInput];
    }
    if (voiceShortcut.consumed) return YES;
    if (_modifierTap.observe(event, _appearance.shiftTapShortcut, _appearance.controlTapShortcut)) {
        // A tap during a composition sends out the letters that were typed, not the highlighted candidate.
        // Reaching for Shift mid-word is how a word the dictionary does not carry - a name, a command, an
        // acronym - gets out without losing what was already typed; finishing the composition instead
        // commits the Chinese candidate, which is the opposite of what was asked for.
        //
        // Commit first, then switch: switching rebuilds the input session, and the other order loses the
        // letters still being composed. setEnglishInputMode: finishes any composition of its own, which is
        // a no-op once this has run.
        if ([_view[@"editing_text"] length] && _session && _activeClient) {
            NSDictionary *raw = [_session command:MSIME_COMMIT_RAW error:nil];
            if (raw) [self apply:raw];
        }
        [self setEnglishInputMode:!_appearance.englishMode];
        return YES;
    }
    if (event.type != NSEventTypeKeyDown) return NO;
    [_appearance lockActiveInputMode];
    if (event.keyCode == 51) {
        const BOOL compositionActive = [_view[@"editing_text"] length] || [_view[@"candidates"] count] ||
            [_view[@"phrase_prefix"] length];
        BOOL suppressEscapedRepeat = NO;
        if (!event.isARepeat) {
            _backspaceHoldArmed = compositionActive;
        } else suppressEscapedRepeat = _backspaceHoldArmed && !compositionActive;
        [self clearSmartPunctuationSpaceRevert];
        if (_lastSmartPunctuation) {
            _smartPunctuationRejected = YES;
            _rejectedSmartPunctuation = _lastSmartPunctuation;
            _lastSmartPunctuation = 0;
        }
        if (suppressEscapedRepeat) {
            // The same hold already consumed the composition. Keep consuming
            // its repeats locally until a fresh Backspace press re-evaluates
            // ownership; no Engine request or document edit is needed here.
            return YES;
        }
    } else if (_smartPunctuationRejected && event.characters.length == 1 &&
               [event.characters characterAtIndex:0] != _rejectedSmartPunctuation) {
        _smartPunctuationRejected = NO;
        _rejectedSmartPunctuation = 0;
    } else if (_lastSmartPunctuation && event.characters.length == 1 &&
               [event.characters characterAtIndex:0] != _lastSmartPunctuation) {
        [self resetSmartPunctuationState];
    }
    if (voiceEnabled && !event.isARepeat && event.keyCode == 101 &&
        (event.modifierFlags & (NSEventModifierFlagControl | NSEventModifierFlagShift | NSEventModifierFlagOption | NSEventModifierFlagCommand)) == NSEventModifierFlagControl &&
        ([NSUserDefaults.standardUserDefaults objectForKey:@"MSIMEClientVoiceHotkeyCtrlF9"] == nil || [NSUserDefaults.standardUserDefaults boolForKey:@"MSIMEClientVoiceHotkeyCtrlF9"])) {
        [self toggleVoiceInput:nil];
        return YES;
    }
    const NSEventModifierFlags competing = NSEventModifierFlagCommand | NSEventModifierFlagControl | NSEventModifierFlagOption;
    if (_doubaoVoiceRequest) [self cancelDoubaoVoiceInput];
    if (_liveVoiceToken) [self cancelLiveVoiceInput];
    if (_appearance.controlOptionSpaceShortcut && event.keyCode == 49 &&
        (event.modifierFlags & (competing | NSEventModifierFlagShift)) == (NSEventModifierFlagControl | NSEventModifierFlagOption)) {
        if (!event.isARepeat) [self setEnglishInputMode:!_appearance.englishMode];
        return YES;
    }
    if (_appearance.inputModeShortcut && event.keyCode == 49 && (event.modifierFlags & NSEventModifierFlagShift) && !(event.modifierFlags & competing)) {
        if (!event.isARepeat) [self setEnglishInputMode:!_appearance.englishMode];
        return YES;
    }
    if (MSIMEPunctuationToggle(event)) {
        if (!event.isARepeat) [self floatingToolbarDidRequestTogglePunctuation:nil];
        return YES;
    }
    if (_appearance.characterSetShortcut && event.keyCode == 3 &&
        (event.modifierFlags & (competing | NSEventModifierFlagShift)) == (NSEventModifierFlagControl | NSEventModifierFlagShift)) {
        // Like the Windows host, reserve the chord but only toggle in Chinese mode.
        if (!event.isARepeat && !_appearance.englishMode) [self floatingToolbarDidRequestToggleTraditionalOutput:nil];
        return YES;
    }
    if (event.keyCode == 14 && (event.modifierFlags & (competing | NSEventModifierFlagShift)) == (NSEventModifierFlagControl | NSEventModifierFlagShift)) {
        if (!event.isARepeat) [self toggleDedicatedEnglishMode:nil];
        return YES;
    }
    // Only the Option+Shift+H arm is a preference; Ctrl+Shift+Space is the chord the Windows host
    // reserves too, and the settings page says nothing about it.
    if (msime::mac::IsFullWidthInputToggle(event.keyCode, event.modifierFlags) &&
        (event.keyCode == 49 || _appearance.fullWidthShortcut) &&
        (!_appearance.englishMode || event.keyCode == 49)) {
        if (!event.isARepeat) [self toggleRuntimeFullWidthInput];
        return YES;
    }
    if (event.keyCode == 40 &&
        (event.modifierFlags & (competing | NSEventModifierFlagShift)) ==
            (NSEventModifierFlagControl | NSEventModifierFlagShift | NSEventModifierFlagCommand)) {
        if (!event.isARepeat) [self showScreenKeyboard:nil];
        return YES;
    }
    const auto maintenanceShortcut = msime::mac::PhysicalMaintenanceShortcut(
        event.keyCode,
        (event.modifierFlags & NSEventModifierFlagControl) != 0,
        (event.modifierFlags & NSEventModifierFlagShift) != 0,
        (event.modifierFlags & NSEventModifierFlagOption) != 0,
        (event.modifierFlags & NSEventModifierFlagCommand) != 0);
    if (maintenanceShortcut != msime::mac::MaintenanceShortcutAction::None) {
        if (event.isARepeat) return YES;
        switch (maintenanceShortcut) {
            case msime::mac::MaintenanceShortcutAction::ClearCache: {
                if (!_session) [self prepareSession];
                NSError *error = nil;
                NSDictionary *transition = [_session resetCacheWithError:&error];
                if (transition) [self apply:transition];
                else if (error) NSBeep();
                break;
            }
            case msime::mac::MaintenanceShortcutAction::Restart:
                [self restartCurrentInputMethod];
                break;
            case msime::mac::MaintenanceShortcutAction::Terminate:
                [self terminateCurrentInputMethod];
                break;
            case msime::mac::MaintenanceShortcutAction::None:
                break;
        }
        return YES;
    }
    // English mode normally leaves keys to the application, but the Chinese
    // punctuation lock and full-width setting still belong to the IME. This
    // mirrors the closed-IME path used by the other native hosts.
    if (_appearance.englishMode) {
        const NSEventModifierFlags competing = NSEventModifierFlagControl | NSEventModifierFlagOption |
                                                NSEventModifierFlagCommand;
        if (!(event.modifierFlags & competing) && event.characters.length == 1) {
            const BOOL keypad = (event.modifierFlags & NSEventModifierFlagNumericPad) != 0 || event.keyCode == 65;
            const BOOL chinese = [_appearance.punctuationLock isEqual:@"chinese"] ||
                ([_appearance.punctuationLock isEqual:@"follow"] && _appearance.runtimeChinesePunctuation);
            const unichar character = [event.characters characterAtIndex:0];
            const std::string output = msime::input::english_mode_output(
                character, keypad, chinese, _appearance.runtimeFullWidthInput, _englishPunctuation);
            if (!output.empty()) {
                NSString *text = [[NSString alloc] initWithBytes:output.data() length:output.size()
                                                         encoding:NSUTF8StringEncoding];
                [(id<MSIMETextClient>)sender insertText:text replacementRange:NSMakeRange(NSNotFound, NSNotFound)];
                MSIMERecordTypingStatistics(_preferencesDirectory ?: [self runtimeOptions][@"preferences_directory"], text,
                                            msime::mac::TypingSource::English);
                return YES;
            }
        }
        return NO;
    }
    if (MSIMECapsLockFreshUppercaseBypass(event, _view)) return NO;
    if (!_session) [self prepareSession];
    if (!_session) return NO;
    if (_focusPending) [self prepareSession];
    [self syncPageSize];
    NSUInteger deletionSlot = MSIMECandidateDeletionSlot(event);
    if (_panel.isVisible && deletionSlot != NSNotFound) {
        if (event.isARepeat) return YES;
        NSDictionary *identifier = MSIMERenderedCandidateIdentity(_panel, (NSInteger)deletionSlot);
        if (!MSIMECurrentCandidateIdentity(identifier, _view)) return YES;
        NSError *error = nil;
        NSDictionary *result = [_session removeGeneration:[identifier[@"generation"] unsignedLongLongValue] index:[identifier[@"index"] unsignedIntegerValue] error:&error];
        if (result) [self apply:result];
        else if (error) NSBeep();
        return YES; // Never finish composition or leak a reserved deletion chord.
    }
    // Candidate numbers follow the physical ANSI number row, matching the
    // Windows TSF path even when the active keyboard layout emits different
    // characters.  Let nine-key mode and modified chords reach the Engine.
    const NSEventModifierFlags candidateDigitModifiers = NSEventModifierFlagShift | NSEventModifierFlagControl |
                                                          NSEventModifierFlagOption | NSEventModifierFlagCommand;
    const int physicalDigit = msime::mac::PhysicalCandidateDigitSlot(event.keyCode);
    NSArray *visibleCandidates = [_view[@"candidates"] isKindOfClass:NSArray.class] ? _view[@"candidates"] : @[];
    const NSEventModifierFlags glossModifiers = event.modifierFlags &
        (NSEventModifierFlagShift | NSEventModifierFlagControl | NSEventModifierFlagOption | NSEventModifierFlagCommand);
    if (_panel.isVisible && physicalDigit >= 0 &&
        glossModifiers == NSEventModifierFlagOption) {
        NSDictionary *candidate = ((NSUInteger)physicalDigit < visibleCandidates.count) ? visibleCandidates[(NSUInteger)physicalDigit] : nil;
        if ([self commitCandidateGlossColumn:1 candidate:candidate client:sender]) return YES;
    }
    if (_panel.isVisible && physicalDigit >= 0 &&
        glossModifiers == NSEventModifierFlagControl) {
        NSDictionary *candidate = ((NSUInteger)physicalDigit < visibleCandidates.count) ? visibleCandidates[(NSUInteger)physicalDigit] : nil;
        if ([self commitCandidateGlossColumn:2 candidate:candidate client:sender]) return YES;
    }
    if (_panel.isVisible && physicalDigit >= 0 && glossModifiers == 0 && _armedGlossColumn > 0) {
        NSDictionary *candidate = ((NSUInteger)physicalDigit < visibleCandidates.count) ? visibleCandidates[(NSUInteger)physicalDigit] : nil;
        if ([self commitCandidateGlossColumn:_armedGlossColumn candidate:candidate client:sender]) return YES;
    }
    const BOOL unicodeComposition = [_view[@"local_mode"] isEqual:@"unicode"];
    if (msime::mac::ShouldRoutePhysicalCandidateDigit(
            _panel.isVisible, [_view[@"nine_key"] boolValue], unicodeComposition,
            (event.modifierFlags & candidateDigitModifiers) != 0) ||
        msime::mac::ShouldRouteUnicodeShiftCandidateDigit(
            _panel.isVisible, unicodeComposition,
            (event.modifierFlags & candidateDigitModifiers) == NSEventModifierFlagShift)) {
        const int slot = msime::mac::PhysicalCandidateDigitSlot(event.keyCode);
        if (slot >= 0) {
            // The panel owns the rendered snapshot. If it is from an older
            // generation, consume the key until the new page is visible instead
            // of letting it fall through to Engine numeric input.
            NSDictionary *identifier = MSIMERenderedCandidateIdentity(_panel, slot);
            if (!MSIMECurrentCandidateIdentity(identifier, _view)) return YES;
            NSDictionary *selected = [_session selectGeneration:[identifier[@"generation"] unsignedLongLongValue]
                                                           index:[identifier[@"index"] unsignedIntegerValue]
                                                           error:nil];
            if (selected) [self apply:selected];
            return YES;
        }
    }
    // Match Windows keypad punctuation and Linux's physical keypad route.
    // Decimal always remains ASCII '.', while arithmetic/separator keys use
    // the Engine punctuation policy when idle. With a composition, every
    // keypad mark finishes the highlighted candidate and appends its literal
    // ASCII byte. Physical routing keeps '-' and '=' out of main-row paging.
    const char keypadPunctuation = msime::mac::KeypadPunctuation(event.keyCode);
    if (keypadPunctuation &&
        !(event.modifierFlags & (NSEventModifierFlagShift | NSEventModifierFlagControl |
                                 NSEventModifierFlagOption | NSEventModifierFlagCommand))) {
        const BOOL hasComposition = [_view[@"editing_text"] length] ||
            ([_view[@"candidates"] isKindOfClass:NSArray.class] && [_view[@"candidates"] count]);
        _punctuationKeyInFlight = (unichar)keypadPunctuation;
        NSDictionary *transition = hasComposition || keypadPunctuation == '.'
            ? [_session punctuationASCII:(uint8_t)keypadPunctuation error:nil]
            : [_session punctuation:(uint8_t)keypadPunctuation error:nil];
        if (!transition) _punctuationKeyInFlight = 0;
        if (transition) {
            [self apply:transition];
            // Only the idle route commits the Engine's Chinese form; the ASCII route leaves the tail ASCII.
            if (!hasComposition && keypadPunctuation != '.') [self noteCommittedChinesePunctuation:transition client:sender];
            if ([transition[@"handled"] boolValue]) return YES;
        }
        if (keypadPunctuation == '.') {
            NSString *text = @".";
            [(id<MSIMETextClient>)sender insertText:text replacementRange:NSMakeRange(NSNotFound, NSNotFound)];
            MSIMERecordTypingStatistics(_preferencesDirectory ?: [self runtimeOptions][@"preferences_directory"], text,
                                        MSIMEResolveTypingSource(_view, _view, MSIMEStatisticsHostOptions(_session), _appearance.englishMode));
            return YES;
        }
        // A normal punctuation transition can be unhandled in an English or
        // local mode. Leave that key to the host rather than reinterpreting it
        // as the main-row '-'/'=' candidate navigation shortcut.
        return [transition[@"handled"] boolValue];
    }
    if ([self revertSmartPunctuationSpace:event client:(id<MSIMETextClient>)sender]) return YES;
    if ([self convertSmartPunctuationSpace:event client:(id<MSIMETextClient>)sender]) return YES;
    if ([self handleSmartPunctuation:event client:(id<MSIMETextClient>)sender]) return YES;
    // The sense page owns the keyboard while it is up, and hands back anything it does not claim.
    if ([self glossSensePageActive] && [self handleGlossSenseEvent:event client:sender]) return YES;
    // Ctrl+Enter offers the highlighted candidate's gloss: one sense commits, several open the page
    // above. The reference and both Linux hosts do the same; here the chord used to fall through to
    // the rule below and commit the composition instead.
    if ((event.modifierFlags & (NSEventModifierFlagControl | NSEventModifierFlagShift |
                                NSEventModifierFlagOption | NSEventModifierFlagCommand)) ==
            NSEventModifierFlagControl &&
        (event.keyCode == 36 || event.keyCode == 76) && event.type == NSEventTypeKeyDown &&
        _appearance.candidateTranslations && _panel.isVisible) {
        NSArray<NSString *> *senses = [self sensesForHighlightedCandidate];
        if (senses.count == 1) {
            NSDictionary *highlighted = [self highlightedCandidateForGloss];
            NSMutableDictionary *single = [highlighted mutableCopy];
            single[@"translation"] = senses.firstObject;
            if ([self commitCandidateGlossColumn:1 candidate:single client:sender]) return YES;
        } else if (senses.count > 1) {
            [self showGlossSensePage:senses];
            return YES;
        }
    }
    // Ctrl+Backspace deletes a segmentation unit and Ctrl+Left / Ctrl+Right move the caret by one,
    // which is what the reference's composition editor does (`IsSegmentBackspaceKey` and
    // `IsSegmentCaretKey` in its input_key_policy.h) and what both Linux front ends and the Windows
    // host already route. It has to be decided before the rule below, which hands every Ctrl, Option
    // and Command chord back to the application after finishing the composition - that rule is what
    // left this host without segment editing.
    //
    // Only the bare Ctrl chord is the input method's: with Shift, Option or Command also held, or
    // with nothing being composed, the key stays the application's.
    if ((event.modifierFlags & (NSEventModifierFlagControl | NSEventModifierFlagShift |
                                NSEventModifierFlagOption | NSEventModifierFlagCommand)) ==
            NSEventModifierFlagControl &&
        _session && _activeClient && ([_view[@"editing_text"] length] || [_view[@"candidates"] count] ||
                                      [_view[@"phrase_prefix"] length])) {
        uint32_t segment = UINT32_MAX;
        if (event.keyCode == 51) segment = MSIME_BACKSPACE_SEGMENT;
        else if (event.keyCode == 123) segment = MSIME_MOVE_LEFT_SEGMENT;
        else if (event.keyCode == 124) segment = MSIME_MOVE_RIGHT_SEGMENT;
        if (segment != UINT32_MAX) {
            NSDictionary *transition = [_session command:segment error:nil];
            // An Engine failure leaves the composition alone rather than finishing it below: the
            // user asked to edit what is there, not to commit it.
            if (transition) { [self apply:transition]; return YES; }
            return YES;
        }
    }
    if (event.modifierFlags & (NSEventModifierFlagCommand | NSEventModifierFlagControl | NSEventModifierFlagOption)) {
        [self apply:[_session command:MSIME_FINISH_COMPOSITION error:nil]];
        return NO;
    }
    uint32_t command = UINT32_MAX;
    [self ensureAppearance];
    if (_panel.isVisible && event.keyCode == 48 &&
        !(event.modifierFlags & (NSEventModifierFlagControl | NSEventModifierFlagOption | NSEventModifierFlagCommand))) {
        if ([self cycleArmedGlossColumnBackwards:(event.modifierFlags & NSEventModifierFlagShift) != 0]) {
            [self renderCandidates];
            return YES;
        }
        if ([_appearance navigationEnabled:@"tab"]) {
            [self apply:[_session command:(event.modifierFlags & NSEventModifierFlagShift) ? MSIME_PREVIOUS_PAGE : MSIME_NEXT_PAGE error:nil]];
            return YES;
        }
        // With candidates visible, a disabled Tab paging shortcut is still owned by the IME:
        // do not let the host application move focus away from the composition.
        return YES;
    }
    // Candidate paging is keyed by the physical ANSI key, matching Windows
    // even when the current keyboard layout produces a different glyph (or no
    // text at all). Unicode '+' is an Engine code-sequence character, and the
    // Japanese minus/equal keys remain composition input.
    const int physicalPageDirection = msime::mac::PhysicalCandidatePageDirection(event.keyCode);
    const BOOL japaneseMinusEqual = msime::mac::IsJapaneseMinusEqualKey(
        [_view[@"scheme"] intValue], [_view[@"local_mode"] isEqual:@"temporary_japanese"],
        event.keyCode, 0);
    const BOOL unicodePlus = [_view[@"local_mode"] isEqual:@"unicode"] &&
        [event.charactersIgnoringModifiers isEqual:@"+"];
    // Word-to-character owns whichever pair it is bound to, and paging does not get to take it. The two are
    // alternatives, which applyCloudSettingsSnapshot: already says by refusing a snapshot whose paging
    // preset collides - but that only guards the cloud path, so a locally enabled bracket or minus paging
    // shortcut quietly won here and left 以词定字 doing nothing at all, with nothing to say why.
    //
    // The test is the one the word-to-character branch below applies, so the key is excluded from paging
    // exactly when that branch will claim it: whichever way it goes, the keystroke has an owner.
    const BOOL wordCharacterOwnsKey = [self wordCharacterClaimsEvent:event];
    if (_panel.isVisible && !(event.modifierFlags & NSEventModifierFlagShift) &&
        physicalPageDirection != 0 && !japaneseMinusEqual && !unicodePlus && !wordCharacterOwnsKey) {
        const BOOL previous = physicalPageDirection < 0 &&
            ((event.keyCode == 27 && [_appearance navigationEnabled:@"minus_equal"]) ||
             (event.keyCode == 33 && [_appearance navigationEnabled:@"brackets"]) ||
             (event.keyCode == 43 && [_appearance navigationEnabled:@"comma_period"]) ||
             (event.keyCode == 116 && [_appearance navigationEnabled:@"page_up_down"]));
        const BOOL next = physicalPageDirection > 0 &&
            ((event.keyCode == 24 && [_appearance navigationEnabled:@"minus_equal"]) ||
             (event.keyCode == 30 && [_appearance navigationEnabled:@"brackets"]) ||
             (event.keyCode == 47 && [_appearance navigationEnabled:@"comma_period"]) ||
             (event.keyCode == 121 && [_appearance navigationEnabled:@"page_up_down"]));
        if (previous || next) {
            [self apply:[_session command:previous ? MSIME_PREVIOUS_PAGE : MSIME_NEXT_PAGE error:nil]];
            return YES;
        }
    }
    if (_panel.isVisible && !(event.modifierFlags & NSEventModifierFlagShift)) {
        NSString *characters = event.charactersIgnoringModifiers;
        if (characters.length == 1) {
            const unichar character = [characters characterAtIndex:0];
            // In temporary Japanese mode '-' and '=' are composition input (the
            // Windows TSF path gives these keys to the engine as well). Do not
            // consume them as candidate paging shortcuts while the panel is up.
            if (msime::mac::IsJapaneseMinusEqualKey([_view[@"scheme"] intValue],
                                                     [_view[@"local_mode"] isEqual:@"temporary_japanese"],
                                                     event.keyCode, static_cast<char>(character))) {
                // Fall through to the normal engine dispatch below.
            } else {
            BOOL brackets = [[_appearance wordCharacterOptions][@"keys"] isEqual:@"brackets"];
            BOOL first = character == (brackets ? '[' : '-');
            // The same claim the paging exclusion above asks about, so the two cannot disagree about who
            // owns the key and leave it doing nothing.
            if ([self wordCharacterClaimsEvent:event]) {
                if (![_view[@"focused"] isEqual:@YES] || ![_view[@"candidates"] isKindOfClass:NSArray.class]) return YES;
                for (NSDictionary *candidate in _view[@"candidates"]) {
                    if (![candidate isKindOfClass:NSDictionary.class]) continue;
                    if (![candidate[@"highlighted"] isEqual:@YES]) continue;
                    NSDictionary *identifier = candidate[@"id"];
                    if (!MSIMECurrentCandidateIdentity(identifier, _view)) return YES;
                    NSDictionary *selected = [_session selectEdgeGeneration:[identifier[@"generation"] unsignedLongLongValue] index:[identifier[@"index"] unsignedIntegerValue] edge:first ? MSIME_FIRST_HAN : MSIME_LAST_HAN error:nil];
                    if (selected) {
                        NSMutableDictionary *annotated = [selected mutableCopy];
                        annotated[@"word_character_candidate"] = candidate[@"text"] ?: @"";
                        annotated[@"word_character_prefix"] = _view[@"phrase_prefix"] ?: @"";
                        annotated[@"word_character_first"] = @(first);
                        [self apply:annotated];
                    }
                    return YES; // Unsupported/stale candidates must not turn into punctuation.
                }
                return YES;
            }
            }
        }
    }
    if (_panel.isVisible && [_appearance navigationEnabled:@"arrows"] && event.keyCode >= 123 && event.keyCode <= 126) {
        const BOOL horizontal = event.keyCode == 123 || event.keyCode == 124;
        // A vertical panel leaves Left/Right to the composition caret below, as Windows maps VK_LEFT/VK_RIGHT to FUNCTION_MOVE_LEFT/RIGHT while candidates are shown; the Engine answers the move with candidates for the new caret. Up/Down in a horizontal panel are still consumed so they never reach the host.
        if (horizontal != _appearance.vertical) {
            const BOOL backwards = event.keyCode == 123 || event.keyCode == 126;
            [self apply:[_session command:backwards ? MSIME_PREVIOUS_CANDIDATE : MSIME_NEXT_CANDIDATE error:nil]];
            return YES;
        }
        if (!horizontal) return YES;
    }
    if (_panel.isVisible && event.keyCode == 49 &&
        !(event.modifierFlags & (NSEventModifierFlagShift | NSEventModifierFlagControl |
                                 NSEventModifierFlagOption | NSEventModifierFlagCommand))) {
        // Space commits the highlighted item shown by the panel. Keep the
        // identity fence symmetric with numeric and mouse selection.
        NSDictionary *identifier = MSIMERenderedHighlightedCandidateIdentity(_panel);
        if (identifier) {
            if (!MSIMECurrentCandidateIdentity(identifier, _view)) return YES;
            if (_armedGlossColumn > 0 && [self commitHighlightedGlossColumn:_armedGlossColumn client:sender]) return YES;
            NSDictionary *selected = [_session selectGeneration:[identifier[@"generation"] unsignedLongLongValue]
                                                           index:[identifier[@"index"] unsignedIntegerValue]
                                                           error:nil];
            if (selected) [self apply:selected];
            return YES;
        }
    }
    if (_panel.isVisible && (event.keyCode == 36 || event.keyCode == 76) &&
        !(event.modifierFlags & (NSEventModifierFlagShift | NSEventModifierFlagControl |
                                 NSEventModifierFlagOption | NSEventModifierFlagCommand)) &&
        _armedGlossColumn > 0 && [self commitHighlightedGlossColumn:_armedGlossColumn client:sender]) return YES;
    // NSTextInputClient has no caret setter; replacing the known following
    // closing mark atomically advances the caret without duplicating text.
    if (_appearance.pairedPunctuation && event.characters.length == 1 &&
        !(event.modifierFlags & (NSEventModifierFlagControl | NSEventModifierFlagOption | NSEventModifierFlagCommand))) {
        NSString *following = MSIMETextClientFollowingCharacter((id<MSIMETextClient>)sender);
        NSString *typed = event.characters;
        if (!following && !_pairedPunctuation.empty()) _pairedPunctuation.clear();
        if (following.length == 1 && [following isEqualToString:typed] &&
            msime::mac::paired_closing_should_skip(_pairedPunctuation, typed.UTF8String, following.UTF8String, YES,
                                                    event.modifierFlags, NSEventModifierFlagControl,
                                                    NSEventModifierFlagOption, NSEventModifierFlagCommand)) {
            NSRange selected = [(id<MSIMETextClient>)sender selectedRange];
            [(id<MSIMETextClient>)sender insertText:typed replacementRange:NSMakeRange(selected.location, 1)];
            MSIMERecordTypingStatistics(_preferencesDirectory ?: [self runtimeOptions][@"preferences_directory"], typed,
                                        MSIMEResolveTypingSource(_view, _view, MSIMEStatisticsHostOptions(_session), _appearance.englishMode));
            return YES;
        }
    }
    // Japanese converts with Space and commits with Enter; every other scheme keeps the mapping
    // below. See handleJapaneseConversionKey: for why the two keys cannot be the shared ones.
    if ([self handleJapaneseConversionKey:event client:sender]) return YES;
    // The reference sends `{` down its punctuation path and closes it with `}` (`_GetPairedPunctuationClosingFor`), whether or not a composition is live, and the Linux host does the same. The Engine answers `{` on its ASCII route: while composing it commits the candidate followed by `{`, and idle it leaves the key alone, so the host commits the opening mark itself. The mark is not in MSIMEPunctuationPairs because a symbol candidate that is exactly `{` is not paired by the reference.
    if ([event.characters isEqualToString:@"{"] &&
        !(event.modifierFlags & (NSEventModifierFlagControl | NSEventModifierFlagOption | NSEventModifierFlagCommand)) &&
        _appearance.runtimeChinesePunctuation && _appearance.pairedPunctuation && !_pendingPairedClosing &&
        !MSIMEPairedPunctuationExcludedHost()) {
        NSString *opening = _appearance.runtimeFullWidthInput ? @"｛" : @"{";
        NSString *closing = _appearance.runtimeFullWidthInput ? @"｝" : @"}";
        NSDictionary *engine = [_session punctuationASCII:'{' error:nil];
        NSMutableDictionary *transition = nil;
        if ([engine[@"handled"] boolValue]) {
            transition = [engine mutableCopy];
            NSString *commit = engine[@"commit"];
            if ([commit isKindOfClass:NSString.class] && [commit hasSuffix:@"{"])
                transition[@"commit"] = [[commit substringToIndex:commit.length - 1] stringByAppendingString:opening];
        } else {
            transition = [NSMutableDictionary dictionaryWithObject:opening forKey:@"commit"];
            if ([engine[@"view"] isKindOfClass:NSDictionary.class]) transition[@"view"] = engine[@"view"];
            else if (_view) transition[@"view"] = _view;
        }
        _hostOpenedClosing = closing;
        [self apply:transition];
        _hostOpenedClosing = nil;
        return YES;
    }
    switch (event.keyCode) {
        case 48: return NO;
        case 51: command = MSIME_BACKSPACE; break;
        case 36: case 76: command = MSIME_COMMIT_RAW; break;
        case 53: [self flushPendingPairedClosing]; _pairedPunctuation.clear(); command = MSIME_CANCEL; break;
        case 49: command = MSIME_COMMIT_CANDIDATE; break;
        case 123: command = MSIME_MOVE_LEFT; break;
        case 124: command = MSIME_MOVE_RIGHT; break;
        case 115: command = _panel.isVisible ? MSIME_FIRST_CANDIDATE : MSIME_MOVE_HOME; break;
        case 119: command = _panel.isVisible ? MSIME_LAST_CANDIDATE : MSIME_MOVE_END; break;
        case 117: command = MSIME_DELETE_FORWARD; break;
        case 116: if (![_appearance navigationEnabled:@"page_up_down"]) return _panel.isVisible; command = MSIME_PREVIOUS_PAGE; break;
        case 121: if (![_appearance navigationEnabled:@"page_up_down"]) return _panel.isVisible; command = MSIME_NEXT_PAGE; break;
        case 126: if (![_appearance navigationEnabled:@"arrows"]) return _panel.isVisible; command = MSIME_PREVIOUS_CANDIDATE; break;
        case 125: if (![_appearance navigationEnabled:@"arrows"]) return _panel.isVisible; command = MSIME_NEXT_CANDIDATE; break;
    }
    NSDictionary *transition = nil;
    if (command != UINT32_MAX) transition = [_session command:command error:nil];
    else if (event.characters.length == 1 && [event.characters characterAtIndex:0] <= 127) {
        const unichar typed = [event.characters characterAtIndex:0];
        _punctuationKeyInFlight = MSIMEASCIIPunctuation(typed) ? typed : 0;
        transition = [_session typeASCII:(uint8_t)[event.characters characterAtIndex:0] shift:(event.modifierFlags & NSEventModifierFlagShift) != 0 error:nil];
    }
    if (!transition) { _punctuationKeyInFlight = 0; return NO; }
    [self apply:transition];
    // A punctuation key is the only route that arms the space conversion, as in the reference, whose punctuation handler is the one caller: a candidate picked with Space or a digit never arms, even when its text ends in a mark.
    if (command == UINT32_MAX && event.characters.length == 1 && MSIMEASCIIPunctuation([event.characters characterAtIndex:0]))
        [self noteCommittedChinesePunctuation:transition client:sender];
    if (command == UINT32_MAX && [transition[@"handled"] boolValue] &&
        ![transition[@"commit"] isKindOfClass:NSString.class] && event.characters.length == 1 &&
        [event.characters characterAtIndex:0] >= 'a' && [event.characters characterAtIndex:0] <= 'z' &&
        MSIMEShouldAutoCommitWubi(_appearance.wubiAutoCommitUnique, transition[@"view"])) {
        NSDictionary *committed = [_session command:MSIME_COMMIT_CANDIDATE error:nil];
        if (committed) [self apply:committed];
    }
    if ([transition[@"handled"] boolValue]) return YES;
    // Match Apple: Engine gets first refusal, then finish any composition before fallback.
    if ([_view[@"editing_text"] length]) {
        NSDictionary *finished = [_session command:MSIME_FINISH_COMPOSITION error:nil];
        if (!finished) return NO;
        [self apply:finished];
    }
    if (_appearance.runtimeFullWidthInput && [_view[@"editing_text"] isKindOfClass:NSString.class] &&
        ![_view[@"editing_text"] length] && event.characters.length == 1 &&
        msime::mac::IsFullWidthDirectCharacter([event.characters characterAtIndex:0], event.modifierFlags)) {
        const unichar converted = msime::mac::FullWidthCharacter([event.characters characterAtIndex:0]);
        NSString *fullWidthText = [NSString stringWithCharacters:&converted length:1];
        [(id<MSIMETextClient>)sender insertText:fullWidthText replacementRange:NSMakeRange(NSNotFound, NSNotFound)];
        MSIMERecordTypingStatistics(_preferencesDirectory ?: [self runtimeOptions][@"preferences_directory"], fullWidthText,
                                    MSIMEResolveTypingSource(_view, _view, MSIMEStatisticsHostOptions(_session), _appearance.englishMode));
        return YES;
    }
    return NO;
}

- (void)commitComposition:(id)sender {
    if (sender != _activeClient || !_session) return;
    [self flushPendingPairedClosing];
    _pairedPunctuation.clear();
    [self resetSmartPunctuationState];
    [self clearSmartPunctuationSpaceConversion];
    [self clearSmartPunctuationSpaceRevert];
    [self apply:[_session command:MSIME_FINISH_COMPOSITION error:nil]];
    // IMK asks for this when the user clicks into the document or focus moves, so the caret is about to leave the text just committed.
    [self invalidateSmartPunctuationShadow];
}

- (MSIMEVoiceCommitOutcome)postVoiceText:(NSString *)text route:(const MSIMEVoiceCommitRoute &)route {
    return route.deliver(text);
}

- (void)applyVoiceResult:(NSDictionary *)transition route:(const MSIMEVoiceCommitRoute &)route {
    NSString *text = transition[@"commit"];
    if ([route.mode isEqual:@"tsf"] || ![text isKindOfClass:NSString.class] || !text.length) {
        _typingSourceOverride = @(static_cast<NSInteger>(msime::mac::TypingSource::Voice));
        [self apply:transition];
        _typingSourceOverride = nil;
        return;
    }
    if (_appearance.traditionalOutput && MSIMEScriptConversionApplies(transition[@"commit_context"]))
        text = MSIMEChineseOutputString(text, YES);
    // Posted voice text reaches the application without passing the shadow.
    [self invalidateSmartPunctuationShadow];
    if ([self postVoiceText:text route:route] == MSIMEVoiceCommitOutcome::unavailable) {
        _typingSourceOverride = @(static_cast<NSInteger>(msime::mac::TypingSource::Voice));
        [self apply:transition];
        _typingSourceOverride = nil;
        return;
    }
    MSIMERecordTypingStatistics(_preferencesDirectory ?: MSIMEStatisticsHostOptions(_session)[@"preferences_directory"], text,
                                msime::mac::TypingSource::Voice);
    NSMutableDictionary *remaining = [transition mutableCopy];
    [remaining removeObjectForKey:@"commit"];
    [self apply:remaining];
}

- (void)flushPendingPairedClosing {
    // The closing mark lives in the marked text, so inserting it replaces that range rather than
    // adding a second one. Called wherever the composition is torn down: the pair the user opened
    // is always closed, never dropped along with the composition that was holding it open.
    NSString *closing = _pendingPairedClosing;
    _pendingPairedClosing = nil;
    if (!closing.length || !_activeClient) return;
    [(id<MSIMETextClient>)_activeClient insertText:closing
                                  replacementRange:NSMakeRange(NSNotFound, NSNotFound)];
    _pairedPunctuation.push(closing.UTF8String);
}

- (void)apply:(NSDictionary *)transition {
    if (!transition || !_activeClient) return;
    // An Engine answer replaces the view the sense page was drawn over, so the page goes with it.
    [self discardGlossSensePage];
    // A commit ends the conversion it belonged to; so does a composition that has gone away.
    if ([transition[@"commit"] isKindOfClass:NSString.class] ||
        ![transition[@"view"][@"editing_text"] length]) {
        _japaneseConversionIndex = nil;
        _japaneseConversionReading = nil;
    }
    const auto sourceOverride = _typingSourceOverride
        ? static_cast<msime::mac::TypingSource>(_typingSourceOverride.integerValue)
        : msime::mac::TypingSource::Unknown;
    _typingSourceOverride = nil;
    const unichar punctuationKey = _punctuationKeyInFlight;
    _punctuationKeyInFlight = 0;
    NSDictionary *previousView = _view;
    NSDictionary *wordCharacter = transition[@"word_character_candidate"] ? transition : nil;
    NSString *commit = transition[@"commit"];
    if (_appearance.traditionalOutput && wordCharacter && [commit isKindOfClass:NSString.class] &&
        MSIMEScriptConversionApplies(transition[@"commit_context"]) &&
        [wordCharacter[@"word_character_candidate"] isKindOfClass:NSString.class] &&
        [wordCharacter[@"word_character_prefix"] isKindOfClass:NSString.class]) {
        NSString *prefix = wordCharacter[@"word_character_prefix"];
        NSString *candidate = wordCharacter[@"word_character_candidate"];
        NSString *convertedWord = MSIMEChineseOutputString([prefix stringByAppendingString:candidate], YES);
        NSString *edge = MSIMEEdgeHanCharacter(convertedWord, [wordCharacter[@"word_character_first"] boolValue]);
        if (edge.length) {
            NSMutableDictionary *converted = [transition mutableCopy];
            converted[@"commit"] = [MSIMEChineseOutputString(prefix, YES) stringByAppendingString:edge];
            converted[@"word_character_converted"] = @YES;
            transition = converted;
        }
    }
    NSString *commitForTracking = transition[@"commit"];
    if ([commitForTracking isKindOfClass:NSString.class] && commitForTracking.length)
        [self persistCommittedCandidateTranslation:commitForTracking];
    if ([commitForTracking isKindOfClass:NSString.class] && commitForTracking.length)
        _armedGlossColumn = 0;
    // The Engine commits the opening mark alone; closing the pair is this host's job, the way the
    // Windows TIP appends its closing mark and the Linux host appends its own. What differs is the
    // caret: those two move it back a character and IMK cannot. So the opening goes in as committed
    // text and the closing becomes the tail of the marked text until the composition ends.
    // With pairing on, every press of a quote key starts a fresh pair.
    //
    // The Engine alternates the quote keys - one press gives “, the next ”, because that is what a
    // host without pairing needs. A host that supplies the closing half itself never sends the
    // second press, so the alternation is left pointing at the closing mark and the *next* quote
    // the user types opens with ”. The reference rewrites it at the same point and says the same
    // thing: in paired mode every press starts a pair rather than following the toggle.
    //
    // A punctuation key typed during a composition finishes it, and the Engine commits the candidate and the mark as one string (`nihao(` gives `你好（`). The reference decides pairing from `punctuationStr.back()` inside its punctuation-key path, so the mark read here is the last character of such a commit. Any other commit is read whole, which keeps a candidate or phrase that merely ends in a quote or bracket exactly as it is.
    NSString *mark = commitForTracking;
    BOOL markEndsLongerCommit = NO;
    if (punctuationKey && [commitForTracking isKindOfClass:NSString.class] && commitForTracking.length) {
        const NSRange last = [commitForTracking rangeOfComposedCharacterSequenceAtIndex:commitForTracking.length - 1];
        if (last.location > 0) {
            mark = [commitForTracking substringWithRange:last];
            markEndsLongerCommit = YES;
        }
    }
    const BOOL quoteKey = !markEndsLongerCommit || punctuationKey == '"' || punctuationKey == '\'';
    if (_appearance.pairedPunctuation && [mark isKindOfClass:NSString.class] && quoteKey &&
        ([mark isEqualToString:@"”"] || [mark isEqualToString:@"’"]) &&
        !_pendingPairedClosing && !MSIMEPairedPunctuationExcludedHost()) {
        NSMutableDictionary *reopened = [transition mutableCopy];
        NSString *opening = [mark isEqualToString:@"”"] ? @"“" : @"‘";
        commitForTracking = [[commitForTracking substringToIndex:commitForTracking.length - mark.length] stringByAppendingString:opening];
        mark = opening;
        reopened[@"commit"] = commitForTracking;
        transition = reopened;
    }
    NSString *openedClosing = nil;
    if ([mark isKindOfClass:NSString.class] && mark.length &&
        _appearance.pairedPunctuation && !_pendingPairedClosing && !MSIMEPairedPunctuationExcludedHost()) {
        for (NSArray<NSString *> *pair in MSIMEPunctuationPairs())
            if ([mark isEqualToString:pair[0]]) { openedClosing = pair[1]; break; }
    }
    NSString *hostOpenedClosing = _hostOpenedClosing;
    _hostOpenedClosing = nil;
    if (hostOpenedClosing && !openedClosing && [commitForTracking isKindOfClass:NSString.class] &&
        [commitForTracking hasSuffix:[hostOpenedClosing isEqualToString:@"｝"] ? @"｛" : @"{"] &&
        _appearance.pairedPunctuation && !_pendingPairedClosing && !MSIMEPairedPunctuationExcludedHost())
        openedClosing = hostOpenedClosing;
    // A pair this host closed is a pair the Engine still counts as open. Book title marks are the
    // ones that notice: 《 inside 《 is 〈, so an unbalanced count turns the next pair the user
    // types into 〈〉. Both halves of that nesting come from the same key, hence one call for both.
    if (openedClosing && ([mark isEqualToString:@"《"] || [mark isEqualToString:@"〈"]))
        [_session balancePairedPunctuationAfterAutoClose:'<' error:nil];
    if ([commitForTracking isKindOfClass:NSString.class] && commitForTracking.length >= 2 && _appearance.pairedPunctuation) {
        for (NSArray<NSString *> *pair in MSIMEPunctuationPairs())
            if ([commitForTracking hasPrefix:pair[0]] && [commitForTracking hasSuffix:pair[1]]) { _pairedPunctuation.push(pair[1].UTF8String); break; }
    }
    NSDictionary *displayTransition = transition;
    if (_appearance.traditionalOutput && ![transition[@"word_character_converted"] boolValue] &&
        MSIMEScriptConversionApplies(transition[@"commit_context"]) && [transition[@"commit"] isKindOfClass:NSString.class]) {
        NSMutableDictionary *converted = [transition mutableCopy];
        converted[@"commit"] = MSIMEChineseOutputString(transition[@"commit"], YES);
        displayTransition = converted;
    }
    NSString *pendingClosing = _pendingPairedClosing;
    MSIMEApplyTransitionWithPendingClosing(displayTransition, (id<MSIMETextClient>)_activeClient,
                                           _appearance.inlinePreeditStyle, pendingClosing);
    // What a commit leaves left of the caret is known for certain, even in a host that never reads it back (Windows sets the shadow after every commit it makes). A pending closing mark goes in behind the commit, so it is the character the caret follows.
    if ([displayTransition[@"commit"] isKindOfClass:NSString.class]) {
        NSString *landed = pendingClosing.length ? pendingClosing : displayTransition[@"commit"];
        if (landed.length) [self noteSmartPunctuationShadow:[landed characterAtIndex:landed.length - 1]];
    }
    if (pendingClosing && [displayTransition[@"commit"] isKindOfClass:NSString.class]) {
        // The commit took the closing mark with it, so the pair is done and a later duplicate of
        // that mark should be skipped rather than typed twice.
        _pairedPunctuation.push(pendingClosing.UTF8String);
        _pendingPairedClosing = nil;
    }
    if (openedClosing) {
        _pendingPairedClosing = openedClosing;
        [(id<MSIMETextClient>)_activeClient setMarkedText:openedClosing
                                          selectionRange:NSMakeRange(0, 0)
                                        replacementRange:NSMakeRange(NSNotFound, NSNotFound)];
    }
    if ([displayTransition[@"commit"] isKindOfClass:NSString.class] && [displayTransition[@"commit"] length]) {
        const auto source = sourceOverride == msime::mac::TypingSource::Unknown
            ? MSIMEResolveTypingSource(transition[@"commit_context"], previousView, MSIMEStatisticsHostOptions(_session), _appearance.englishMode)
            : sourceOverride;
        MSIMERecordTypingStatistics(_preferencesDirectory ?: MSIMEStatisticsHostOptions(_session)[@"preferences_directory"],
                                    displayTransition[@"commit"], source);
    }
    _view = transition[@"view"];
    if (![_view[@"candidates"] isKindOfClass:NSArray.class] || ![_view[@"candidates"] count])
        _armedGlossColumn = 0;
    [self refreshFloatingToolbarState];
    [self renderCandidates];
    [self synchronizeCloudCandidates];
    [self scheduleSettledRerank];
    [self synchronizeCandidateGloss];
    [self synchronizeTargetGloss];
    [self synchronizeOnDeviceGloss];
    [self synchronizeAccountGloss:[self currentAccountGlossRequest]];
    [self synchronizeCustomTranslations];
    [self synchronizeAITranslations];
}

// The card is at most half the screen's visible width, as the Windows card is at most half its work area, and at least seven times the candidate font size, as the Windows card and the source skins' `min-width: 7em` are. Every row is laid out at the width it then gets: text, 辅助码 and gloss wider than their column wrap inside it and the row takes their height (CandidateItemLayout.h).
- (MSIMECandidatePageGeometry)candidatePageGeometry:(NSArray *)candidates font:(NSFont *)font glossFont:(NSFont *)glossFont
                                     showSelectedBar:(BOOL)showSelectedBar inset:(CGFloat)inset paging:(BOOL)paging
                                             visible:(NSRect)visible preeditWidth:(CGFloat)preeditWidth
                                        minimumWidth:(CGFloat)minimumWidth {
    MSIMECandidatePageGeometry geometry;
    const BOOL vertical = _appearance.vertical;
    const BOOL traditional = _appearance.traditionalOutput && MSIMEScriptConversionApplies(_view);
    NSFont *numberFont = MSIMECandidateNumberFont(font);
    NSMutableArray<NSString *> *texts = [NSMutableArray arrayWithCapacity:candidates.count];
    NSMutableArray<NSString *> *annotations = [NSMutableArray arrayWithCapacity:candidates.count];
    NSMutableArray<NSString *> *displays = [NSMutableArray arrayWithCapacity:candidates.count];
    NSMutableArray<NSString *> *translations = [NSMutableArray arrayWithCapacity:candidates.count];
    CGFloat candidateRow = MSIMECandidateTextHeight(@"", font) + MSIMECandidateRowPadding;
    CGFloat numberWidth = 0;
    NSUInteger index = 0;
    for (NSDictionary *candidate in candidates) {
        NSString *hint = MSIMEWubiCodeHint(candidate, _view, _wubiCodeHintEnabled);
        NSString *text = CandidateTextRun(candidate, traditional);
        NSString *annotation = CandidateAnnotationRun(candidate, traditional, hint);
        [texts addObject:text];
        [annotations addObject:annotation];
        [displays addObject:CandidateDisplayWithWubiHint(candidate, traditional, hint)];
        [translations addObject:CandidateTranslation(candidate)];
        NSString *number = [NSString stringWithFormat:@"%lu", (unsigned long)++index];
        numberWidth = MAX(numberWidth, [number sizeWithAttributes:@{NSFontAttributeName: numberFont}].width);
        // Fallback glyphs can stand taller than the primary font; a one-line row is as tall as its tallest.
        candidateRow = MAX(candidateRow, MSIMECandidateTextHeight([text stringByAppendingString:annotation], font) + MSIMECandidateRowPadding);
    }
    geometry.texts = texts;
    geometry.annotations = annotations;
    geometry.displays = displays;
    geometry.contentLeft = MSIMECandidateTextLeft(showSelectedBar) + ceil(numberWidth) + MSIMECandidateNumberGap;
    const msime::mac::CandidateLayoutMetrics metrics =
        MSIMECandidateLayoutMetrics(font, glossFont, candidateRow, geometry.contentLeft + MSIMECandidateTextRight);
    std::vector<msime::mac::CandidateItemWidths> items;
    items.reserve(candidates.count);
    CGFloat natural = 0;
    for (NSUInteger i = 0; i < texts.count; ++i) {
        items.push_back(MSIMECandidateItemWidths(texts[i], annotations[i], translations[i], font, glossFont));
        const CGFloat itemWidth = ceil(msime::mac::CandidateItemNaturalWidth(items.back(), metrics, !vertical));
        natural = vertical ? MAX(natural, itemWidth) : natural + itemWidth;
    }
    const CGFloat pagingWidth = paging && !vertical ? 56 : 0;
    CGFloat width = MAX(20, natural + 2 * inset + pagingWidth);
    width = MAX(width, preeditWidth);
    const CGFloat widthCap = MAX(80, floor(visible.size.width * 0.5));
    width = MIN(width, widthCap);
    if (paging) width = MAX(width, 76);
    // At least 7em of the candidate font, raised by the skin's floor; the 7em part stays within the half-screen cap (CandidateItemLayout.h).
    width = MAX(width, msime::mac::CandidateCardMinimumWidth(font.pointSize, minimumWidth, widthCap));
    geometry.width = width;
    geometry.lineWidth = MAX(1, width - 2 * inset - pagingWidth);
    const msime::mac::CandidatePageMeasure measure = [texts, annotations, translations, font, glossFont](std::size_t row, msime::mac::CandidateRun run, double runWidth) {
        return MSIMECandidateRunMeasure(texts[row], annotations[row], translations[row], font, glossFont)(run, runWidth);
    };
    // Horizontal rows keep the height a gloss will need even before it arrives, so the card does not grow under the user seconds after the composition started.
    const CGFloat minimumHeight = vertical ? 0 : candidateRow + [self reservedGlossHeightForFont:glossFont];
    geometry.rows = msime::mac::LayoutCandidatePage(items, geometry.lineWidth, metrics, !vertical, measure, minimumHeight);
    for (auto &row : geometry.rows) {
        // Whole points keep neighbouring buttons touching and their text on the pixel grid.
        const double left = round(row.x), right = round(row.x + row.width);
        const double top = round(row.y), bottom = round(row.y + row.height);
        row.x = left;
        row.width = right - left;
        row.y = top;
        row.height = bottom - top;
    }
    geometry.rowsHeight = ceil(msime::mac::CandidatePageHeight(geometry.rows));
    return geometry;
}

- (void)updateKeymapPanel {
    NSString *editing = MSIMEShuangpinKeymapEditingText(_view);
    NSNumber *scheme = _view[@"scheme"];
    NSString *profile = _view[@"shuangpin_profile"];
    NSString *mode = _view[@"local_mode"];
    NSNumber *dedicatedEnglish = _view[@"dedicated_english"];
    if (!_session || !_activeClient || _appearance.englishMode ||
        ![scheme isKindOfClass:NSNumber.class] || scheme.integerValue != 1 ||
        ![mode isKindOfClass:NSString.class] || ![mode isEqualToString:@"none"] ||
        ![dedicatedEnglish isKindOfClass:NSNumber.class] || dedicatedEnglish.boolValue ||
        ![profile isKindOfClass:NSString.class] || profile.length == 0 ||
        !MSIMEShouldShowShuangpinKeymap(YES, _appearance.shuangpinKeymap, editing.length > 0)) {
        [_keymapPanel orderOut:nil];
        return;
    }
    NSRect cursor = NSZeroRect;
    [_activeClient attributesForCharacterIndex:0 lineHeightRectangle:&cursor];
    if (!MSIMEValidCaret(cursor)) { [_keymapPanel orderOut:nil]; return; }
    if (!_keymapPanel) _keymapPanel = [[MSIMEShuangpinKeymapPanel alloc] init];
    [_keymapPanel setProfileName:profile];
    [_keymapPanel updateHighlightedKey:MSIMEShuangpinKeymapHighlightedKey(_view)];
    CGFloat clearance = _appearance.fontSize + 42.0;
    if (_appearance.vertical) clearance = (_appearance.fontSize + 10.0) * MIN([_view[@"candidates"] count], _appearance.pageSize) + 24.0;
    NSArray *candidates = MSIMEReorderedPinnedCandidates(_view[@"candidates"], MSIMECandidatePinCode(_view));
    if ([candidates isKindOfClass:NSArray.class] && candidates.count) {
        // The same rows the candidate panel lays out, so a card that wraps a long candidate onto several lines is kept clear of in full.
        NSScreen *screen = nil;
        for (NSScreen *candidateScreen in NSScreen.screens)
            if (NSPointInRect(NSMakePoint(NSMinX(cursor), NSMidY(cursor)), candidateScreen.frame)) { screen = candidateScreen; break; }
        screen = screen ?: NSScreen.mainScreen;
        NSFont *font = [_appearance candidateFontOfSize:_appearance.fontSize englishFirst:YES];
        NSFont *glossFont = [_appearance candidateFontOfSize:font.pointSize * 0.78 englishFirst:YES];
        const MSIMECandidatePageGeometry geometry =
            [self candidatePageGeometry:candidates font:font glossFont:glossFont showSelectedBar:_skinShowsSelectedBar inset:12
                                 paging:[_view[@"page_count"] unsignedIntegerValue] > 1
                                visible:screen ? screen.visibleFrame : NSMakeRect(0, 0, 1440, 900) preeditWidth:0 minimumWidth:0];
        clearance = MAX(clearance, geometry.rowsHeight + 24);
    }
    id preedit = [_view[@"preedit"] isKindOfClass:NSString.class] ? _view[@"preedit"] : editing;
    if (_appearance.showsCandidatePreedit && [preedit length] && [_view[@"candidates"] count]) {
        NSFont *preeditFont = [_appearance candidateFontOfSize:_appearance.preeditFontSize englishFirst:YES];
        clearance += MAX(22.0, MSIMECandidateTextHeight(preedit, preeditFont) + 6.0);
    }
    [_keymapPanel showNearCaretRect:cursor candidateClearance:clearance];
}

// IMK keeps a controller per text input client and each owns its own candidate window, so a controller that loses its client without a matching deactivateServer: (a mismatched sender, or a client that goes away) would leave its last frame on screen after the next controller commits. Only one composition is ever on screen, so whichever controller shows its window or takes focus hides the previous owner's.
static __weak MSIMEInputController *MSIMECandidatePanelOwner;
- (void)claimCandidatePanel {
    MSIMEInputController *previous = MSIMECandidatePanelOwner;
    if (previous && previous != self) [previous hideCandidatePanel:"superseded"];
    MSIMECandidatePanelOwner = self;
}

// Every hide of the candidate window goes through here so the diagnostic log can say why it went away, like the source's candidate hide lines. Only a window that was on screen is logged.
- (void)hideCandidatePanel:(const char *)reason {
    if (msime_macos_diagnostic_enabled() && _panel.isVisible) msime_macos_diagnostic_writef("candidate hide reason=%s", reason);
    [_panel orderOut:nil];
}

- (void)renderCandidates {
    const bool timed = msime_macos_diagnostic_enabled();
    const uint64_t buildStarted = timed ? clock_gettime_nsec_np(CLOCK_UPTIME_RAW) : 0;
    _candidateMenuToken = [NSObject new];
    [self updateKeymapPanel];
    if (_appearance.englishMode) { [self resetCandidateAnchor]; [self hideCandidatePanel:"english_mode"]; return; }
    NSArray *candidates = MSIMEReorderedPinnedCandidates(_view[@"candidates"], MSIMECandidatePinCode(_view));
    if (![candidates isKindOfClass:NSArray.class] || candidates.count == 0) {
        _armedGlossColumn = 0;
        [self resetCandidateAnchor];
        [self hideCandidatePanel:"empty"];
        return;
    }
    if (_armedGlossColumn > 0) {
        NSDictionary *highlighted = [self highlightedCandidateForGloss];
        if (!MSIMECandidateTranslationColumn(highlighted, _armedGlossColumn).length) _armedGlossColumn = 0;
    }
    NSRect reportedCursor = NSZeroRect;
    [(id<IMKTextInput>)_activeClient attributesForCharacterIndex:0 lineHeightRectangle:&reportedCursor];
    NSRect cursor = [self candidateCaretForRendering:reportedCursor];
    if (!MSIMEValidCaret(cursor)) { [self hideCandidatePanel:"invalid_caret"]; return; }
    NSScreen *screen = nil;
    for (NSScreen *candidate in NSScreen.screens) {
        if (NSPointInRect(NSMakePoint(NSMinX(cursor), NSMidY(cursor)), candidate.frame)) { screen = candidate; break; }
    }
    screen = screen ?: NSScreen.mainScreen;
    if (!screen) { [self hideCandidatePanel:"no_screen"]; return; }
    NSRect visible = screen.visibleFrame;
    [self ensureAppearance];
    NSAppearance *candidateAppearance = [_appearance candidateAppearanceOverride];
    if (_appearance.candidateAppearanceOverrideConfigured) _panel.appearance = candidateAppearance;
    const BOOL vertical = _appearance.vertical;
    NSAppearance *currentAppearance = candidateAppearance ?: _panel.effectiveAppearance ?: NSApp.effectiveAppearance;
    NSString *currentTheme = [currentAppearance bestMatchFromAppearancesWithNames:@[NSAppearanceNameAqua, NSAppearanceNameDarkAqua]];
    const auto skin = [_appearance resolvedSkinForDark:[currentTheme isEqual:NSAppearanceNameDarkAqua]];
    const auto geometry = skin.tokens;
    _skinShowsSelectedBar = geometry.showSelectedBar;
    const CGFloat inset = MAX(2.0, geometry.pad);
    NSFont *font = [_appearance candidateFontOfSize:_appearance.fontSize englishFirst:YES];
    id preeditValue = _view[@"preedit"];
    if (![preeditValue isKindOfClass:NSString.class]) preeditValue = _view[@"editing_text"];
    NSString *preedit = _appearance.showsCandidatePreedit && [preeditValue isKindOfClass:NSString.class] ? preeditValue : @"";
    NSFont *preeditFont = [_appearance candidateFontOfSize:_appearance.preeditFontSize englishFirst:YES];
    CGFloat preeditHeight = preedit.length ? MAX(22.0, MSIMECandidateTextHeight(preedit, preeditFont) + 6.0) : 0;
    const NSUInteger page = [_view[@"page"] unsignedIntegerValue];
    const NSUInteger pageCount = [_view[@"page_count"] unsignedIntegerValue];
    const BOOL paging = pageCount > 1;
    NSFont *numberFont = MSIMECandidateNumberFont(font);
    NSFont *glossFont = [_appearance candidateFontOfSize:font.pointSize * 0.78 englishFirst:YES];
    const CGFloat preeditWidth = preedit.length ? ceil([preedit sizeWithAttributes:@{NSFontAttributeName:preeditFont}].width) + 2 * inset + 4 + MSIMEPreeditCaretGap : 0;
    const MSIMECandidatePageGeometry pageGeometry =
        [self candidatePageGeometry:candidates font:font glossFont:glossFont showSelectedBar:geometry.showSelectedBar inset:inset
                             paging:paging visible:visible preeditWidth:preeditWidth
                       minimumWidth:MAX(skin.minWidthDip, skin.decorationWidthDip)];
    const CGFloat width = pageGeometry.width;
    if (!_panel) {
        MSIMECandidatePanel *panel = [[MSIMECandidatePanel alloc] initWithContentRect:NSZeroRect styleMask:NSWindowStyleMaskBorderless | NSWindowStyleMaskNonactivatingPanel backing:NSBackingStoreBuffered defer:NO];
        __weak MSIMEInputController *weakSelf = self;
        panel.pageHandler = ^(BOOL previous) {
            MSIMEInputController *controller = weakSelf;
            if (!controller || !controller->_activeClient || !controller->_session || !controller->_panel.isVisible) return;
            [controller apply:[controller->_session command:previous ? MSIME_PREVIOUS_PAGE : MSIME_NEXT_PAGE error:nil]];
        };
        _panel = panel;
        _panel.level = NSPopUpMenuWindowLevel;
        _panel.hasShadow = YES;
        _panel.hidesOnDeactivate = NO;
        _panel.becomesKeyOnlyIfNeeded = YES;
        _panel.collectionBehavior = NSWindowCollectionBehaviorCanJoinAllSpaces | NSWindowCollectionBehaviorFullScreenAuxiliary;
        if (_appearance.candidateAppearanceOverrideConfigured) _panel.appearance = candidateAppearance;
    }
    MSIMECandidatePanel *candidatePanel = (MSIMECandidatePanel *)_panel;
    if (!candidatePanel.wheelPaging) [candidatePanel resetWheelAccumulator];
    candidatePanel.mouseWheelEnabled = [_appearance navigationEnabled:@"mouse_wheel"];
    candidatePanel.hasPreviousPage = page > 0;
    candidatePanel.hasNextPage = page + 1 < pageCount;
    _panel.opaque = NO;
    _panel.backgroundColor = NSColor.clearColor;
    const CGFloat decorationHeight = skin.decorationTopDip;
    const CGFloat height = pageGeometry.rowsHeight + 2 * inset + (paging && vertical ? 26 : 0) + decorationHeight + preeditHeight;
    [_panel setContentSize:NSMakeSize(width, height)];
    MSIMECandidateChromeView *content = [[MSIMECandidateChromeView alloc] initWithFrame:NSMakeRect(0, 0, width, height)];
    NSUInteger slot = 0;
    const CGFloat rowsTop = height - inset - decorationHeight - preeditHeight;
    for (NSDictionary *candidate in candidates) {
        const msime::mac::CandidateRowLayout &row = pageGeometry.rows[slot];
        NSString *display = pageGeometry.displays[slot];
        NSString *title = [NSString stringWithFormat:@"%lu  %@", (unsigned long)(slot + 1), pageGeometry.texts[slot]];
        MSIMECandidateButton *button = [MSIMECandidateButton buttonWithTitle:title target:self action:@selector(selectCandidate:)];
        button.candidateID = candidate[@"id"];
        button.menu = [self menuForCandidate:candidate];
        button.tag = (NSInteger)slot;
        // The frame is the laid out row, so a wrapped line is inside the area that takes the click.
        button.frame = NSMakeRect(inset + row.x, rowsTop - row.y - row.height, row.width, row.height);
        button.accessibilityLabel = [NSString stringWithFormat:@"%lu  %@", (unsigned long)(slot + 1), display];
        ++slot;
        button.font = font;
        button.numberFont = numberFont;
        button.lineBreakMode = NSLineBreakByWordWrapping;
        button.toolTip = display;
        button.annotation = row.item.annotation.width > 0 ? pageGeometry.annotations[slot - 1] : @"";
        button.translation = CandidateTranslation(candidate);
        button.armedGlossColumn = _armedGlossColumn;
        button.translationFont = glossFont;
        button.itemLayout = row.item;
        button.hasItemLayout = YES;
        button.contentLeft = pageGeometry.contentLeft;
        button.translationBelow = button.translation.length ? row.item.translation.below : !vertical;
        if (button.translation.length) button.toolTip = [display stringByAppendingFormat:@"\n%@", button.translation];
        button.bordered = NO;
        button.candidateHighlighted = [candidate[@"highlighted"] boolValue];
        id fixed = candidate[@"fixed_position"];
        button.candidateFixed = MSIMEUnsignedCandidateIdentityValue(fixed) &&
            [fixed compare:@0] == NSOrderedDescending && [fixed compare:@255] != NSOrderedDescending;
        button.alignment = NSTextAlignmentLeft;
        [content addSubview:button];
    }
    if (paging) {
        for (NSUInteger direction = 0; direction < 2; ++direction) {
            MSIMECandidateButton *button = [MSIMECandidateButton buttonWithTitle:direction == 0 ? @"‹" : @"›" target:self action:@selector(changeCandidatePage:)];
            button.frame = NSMakeRect((vertical ? inset : inset + pageGeometry.lineWidth) + direction * 28, inset, 28,
                                      vertical ? 26 : pageGeometry.rows.back().height);
            button.bordered = NO;
            button.tag = direction == 0 ? -1 : -2;
            button.enabled = direction == 0 ? page > 0 : page < pageCount - 1;
            button.accessibilityLabel = direction == 0 ? @"上一页候选" : @"下一页候选";
            // A retained button from an old page cannot navigate a newer view.
            button.candidateID = _view;
            [content addSubview:button];
        }
    }
    if (preedit.length) {
        MSIMECandidatePreeditField *label = [MSIMECandidatePreeditField labelWithString:preedit];
        label.identifier = @"candidate-preedit";
        label.accessibilityLabel = @"候选窗预编辑";
        label.font = preeditFont;
        NSString *editing = [_view[@"editing_text"] isKindOfClass:NSString.class] ? _view[@"editing_text"] : @"";
        label.caretIndex = MSIMEPreeditCaretPosition(editing, preedit, _view[@"caret_position"]);
        // The chosen part of a phrase in progress is held out of the document, so the window has to
        // show it as well; without this the reading in the window would disagree with the marked
        // text in the client, which carries it.
        NSString *phrase = _view[@"phrase_prefix"];
        if ([phrase isKindOfClass:NSString.class] && phrase.length) {
            label.stringValue = [phrase stringByAppendingString:label.stringValue];
            label.caretIndex += phrase.length;
        }
        label.showsCaret = [_view[@"focused"] isEqual:@YES];
        label.frame = NSMakeRect(inset, height - inset - decorationHeight - preeditHeight, width - 2 * inset, preeditHeight);
        [content addSubview:label];
    }
    if (decorationHeight > 0 && _appearance.decorationImage) {
        NSImageView *decoration = [[NSImageView alloc] initWithFrame:NSMakeRect(width - skin.decorationWidthDip, height - decorationHeight, skin.decorationWidthDip, decorationHeight)];
        decoration.image = _appearance.decorationImage;
        decoration.imageScaling = NSImageScaleProportionallyUpOrDown;
        decoration.imageAlignment = NSImageAlignTopRight;
        decoration.wantsLayer = YES;
        [content addSubview:decoration];
    }
    _panel.contentView = content;
    content.appearanceTarget = self;
    content.appearanceAction = @selector(refreshCandidateSkin);
    [self refreshCandidateSkin];
    const NSSize panelSize = _panel.frame.size;
    // Every hide path orders the panel out, so a panel that is not on screen yet starts a fresh flip memory.
    _tallestVerticalCandidateHeight = MSIMETallestCandidateHeight(_tallestVerticalCandidateHeight, panelSize.height, vertical, _panel.isVisible);
    const NSPoint origin = MSIMECandidateOrigin(cursor, panelSize, visible, vertical ? _tallestVerticalCandidateHeight : 0);
    [_panel setFrameOrigin:origin];
    [self claimCandidatePanel];
    [_panel orderFrontRegardless];
    // Geometry and counts only, the macOS form of the source's candidate-frame/candidate-position audit: flipped=1 means the window went above the caret for lack of room below.
    if (timed)
        msime_macos_diagnostic_writef("candidate-frame show rows=%lu vertical=%d caret=(%.0f,%.0f,%.0f,%.0f) size=(%.0f,%.0f) origin=(%.0f,%.0f) visible=(%.0f,%.0f,%.0f,%.0f) flipped=%d build_ms=%.3f",
            (unsigned long)candidates.count, vertical ? 1 : 0, NSMinX(cursor), NSMinY(cursor), NSWidth(cursor), NSHeight(cursor),
            panelSize.width, panelSize.height, origin.x, origin.y, NSMinX(visible), NSMinY(visible), NSWidth(visible), NSHeight(visible),
            origin.y >= NSMaxY(cursor) ? 1 : 0, static_cast<double>(clock_gettime_nsec_np(CLOCK_UPTIME_RAW) - buildStarted) / 1e6);
}

- (void)refreshCandidateSkin {
    if (![_panel.contentView isKindOfClass:MSIMECandidateChromeView.class]) return;
    MSIMECandidateChromeView *content = (id)_panel.contentView;
    NSString *match = [content.effectiveAppearance bestMatchFromAppearancesWithNames:@[NSAppearanceNameAqua, NSAppearanceNameDarkAqua]];
    const auto tokens = [_appearance resolvedSkinForDark:[match isEqual:NSAppearanceNameDarkAqua]].tokens;
    if (tokens.showSelectedBar != _skinShowsSelectedBar) { [self renderCandidates]; return; }
    content.fillColor = [_appearance candidateSurfaceColorWithDefault:SkinColor(tokens.surface)];
    content.strokeColor = [_appearance candidateBorderColorWithDefault:SkinColor(tokens.border)];
    content.cornerRadius = tokens.radius;
    content.lineWidth = tokens.borderWidth;
    NSArray<MSIMECandidateButton *> *candidateButtons = [content.subviews filteredArrayUsingPredicate:
        [NSPredicate predicateWithBlock:^BOOL(NSView *view, NSDictionary *_) {
            return ([view isKindOfClass:MSIMECandidateButton.class] ||
                    [view isKindOfClass:MSIMECandidatePreeditField.class]) &&
                view.tag >= 0;
        }]];
    for (MSIMECandidateButton *button in candidateButtons) {
        if ([button.identifier isEqual:@"candidate-preedit"] && [button isKindOfClass:NSTextField.class]) {
            ((NSTextField *)(id)button).textColor = [_appearance candidateTextColorWithDefault:SkinColor(tokens.text)];
            if ([button isKindOfClass:MSIMECandidatePreeditField.class])
                ((MSIMECandidatePreeditField *)(id)button).caretColor = [_appearance candidateAccentColorWithDefault:SkinColor(tokens.accent)];
        }
        if (![button isKindOfClass:MSIMECandidateButton.class]) continue;
        button.fillColor = [_appearance candidateSelectedColorWithDefault:SkinColor(tokens.selected)];
        button.hoverColor = [_appearance candidateHoverColorWithDefault:SkinColor(tokens.hover)];
        button.titleColor = button.candidateHighlighted ? SkinColor(tokens.selectedText) : [_appearance candidateTextColorWithDefault:SkinColor(tokens.text)];
        // Windows fixed-position span overrides candidate text, not its number.
        if (button.candidateFixed) button.titleColor = [NSColor colorWithSRGBRed:55.0/255 green:154.0/255 blue:211.0/255 alpha:1];
        // The annotation and translation are children of the candidate text in
        // the Windows renderer, so they inherit its final color, including the
        // fixed-position override above. Translation keeps its reduced opacity.
        button.translationColor = [button.titleColor colorWithAlphaComponent:MSIMECandidateTranslationOpacity];
        button.numberColor = button.candidateHighlighted ? SkinColor(tokens.selectedText) : [_appearance candidateNumberColorWithDefault:SkinColor(tokens.number)];
        button.barColor = [_appearance candidateAccentColorWithDefault:SkinColor(tokens.accent)];
        button.showSelectedBar = tokens.showSelectedBar;
        const BOOL first = button == candidateButtons.firstObject;
        const BOOL last = button == candidateButtons.lastObject;
        button.cornerRadius = msime::mac::CandidateRowRadius(tokens, button.candidateHighlighted, first, last);
        button.selectionLeftInset = 1.0;
        button.contentTintColor = SkinColor(tokens.text);
        button.needsDisplay = YES;
    }
    content.needsDisplay = YES;
}

- (void)selectCandidate:(MSIMECandidateButton *)button {
    if (!_activeClient || !_session || !_panel.isVisible || !button.enabled || button.superview != _panel.contentView) return;
    NSDictionary *identifier = button.candidateID;
    if (!MSIMECurrentCandidateIdentity(identifier, _view)) return;
    [self apply:[_session selectGeneration:[identifier[@"generation"] unsignedLongLongValue] index:[identifier[@"index"] unsignedIntegerValue] error:nil]];
}

- (NSMenu *)menuForCandidate:(NSDictionary *)candidate {
    NSDictionary *identifier = candidate[@"id"];
    if (!MSIMECurrentCandidateIdentity(identifier, _view) || !_candidateMenuToken) return nil;
    NSString *text = [candidate[@"text"] isKindOfClass:NSString.class] ? candidate[@"text"] : @"";
    NSDictionary *context = @{@"id":[identifier copy], @"text":text, @"render":_candidateMenuToken};
    NSMenuItem *(^item)(NSString *, NSInteger) = ^NSMenuItem *(NSString *title, NSInteger tag) {
        NSMenuItem *entry = [[NSMenuItem alloc] initWithTitle:title action:@selector(candidateMenuAction:) keyEquivalent:@""];
        entry.target = self;
        entry.tag = tag;
        entry.representedObject = context;
        return entry;
    };
    NSMenu *menu = [[NSMenu alloc] initWithTitle:@"候选操作"];
    menu.autoenablesItems = NO;
    ApplyMetasequoiaMenuTheme(menu, _menuThemePreferences ?: @{});
    NSString *pinTitle = MSIMECandidateIsPinned(MSIMECandidatePinCode(_view), text) ? @"取消置顶" : @"置顶";
    [menu addItem:item(pinTitle, 0)];
    NSMenuItem *fixed = [[NSMenuItem alloc] initWithTitle:@"固定排位" action:nil keyEquivalent:@""];
    NSMenu *positions = [[NSMenu alloc] initWithTitle:@"固定排位"];
    positions.autoenablesItems = NO;
    ApplyMetasequoiaMenuTheme(positions, _menuThemePreferences ?: @{});
    for (NSInteger position = 1; position <= 5; ++position)
        [positions addItem:item([NSString stringWithFormat:@"第 %ld 位", (long)position], 10 + position)];
    [positions addItem:NSMenuItem.separatorItem];
    [positions addItem:item(@"取消固定", 2)];
    fixed.submenu = positions;
    [menu addItem:fixed];
    // Windows hides deletion for one Unicode scalar, including supplementary Han.
    if ([text isKindOfClass:NSString.class] && [text lengthOfBytesUsingEncoding:NSUTF32LittleEndianStringEncoding] / 4 > 1) {
        NSMenuItem *remove = item(@"删除", 1);
        NSArray *candidates = MSIMEReorderedPinnedCandidates(_view[@"candidates"], MSIMECandidatePinCode(_view));
        NSUInteger slot = [candidates isKindOfClass:NSArray.class] ? [candidates indexOfObjectIdenticalTo:candidate] : NSNotFound;
        if (slot < 8) {
            remove.keyEquivalent = [NSString stringWithFormat:@"%lu", (unsigned long)slot + 1];
            remove.keyEquivalentModifierMask = NSEventModifierFlagControl | NSEventModifierFlagOption | NSEventModifierFlagShift;
        }
        [menu addItem:remove];
    }
    return menu;
}

- (void)candidateMenuAction:(NSMenuItem *)item {
    if (!_activeClient || !_session || !_panel.isVisible || !item.enabled) return;
    NSDictionary *context = item.representedObject;
    if (![context isKindOfClass:NSDictionary.class] || context[@"render"] != _candidateMenuToken) return;
    NSDictionary *identifier = context[@"id"];
    if (!MSIMECurrentCandidateIdentity(identifier, _view)) return;
    uint64_t generation = [identifier[@"generation"] unsignedLongLongValue];
    NSUInteger index = [identifier[@"index"] unsignedIntegerValue];
    NSError *error = nil;
    NSDictionary *result = nil;
    switch (item.tag) {
        case 0:
            result = [_session pinGeneration:generation index:index error:&error];
            // Older test/session doubles report a successful maintenance action
            // with a nil transition. An explicit error is the only failure
            // signal, so keep the local pin in sync in both forms.
            if (!error && [context[@"text"] isKindOfClass:NSString.class])
                MSIMETogglePinnedCandidate(MSIMECandidatePinCode(_view), context[@"text"]);
            break;
        case 1: result = [_session removeGeneration:generation index:index error:&error]; break;
        case 2: result = [_session clearPositionGeneration:generation index:index error:&error]; break;
        default:
            if (item.tag < 11 || item.tag > 15) return;
            result = [_session fixGeneration:generation index:index position:(uint8_t)(item.tag - 10) error:&error];
    }
    if (result) [self apply:result];
    else if (error) NSBeep();
}

- (void)changeCandidatePage:(MSIMECandidateButton *)button {
    if (!_activeClient || !_session || !_panel.isVisible || !button.enabled || button.superview != _panel.contentView) return;
    if (![_view isKindOfClass:NSDictionary.class] || ![_view[@"focused"] isEqual:@YES] ||
        ![button.candidateID isKindOfClass:NSDictionary.class]) return;
    if (![_view[@"focused"] isKindOfClass:NSNumber.class] ||
        CFGetTypeID((__bridge CFTypeRef)_view[@"focused"]) != CFBooleanGetTypeID()) return;
    for (NSString *key in @[@"session", @"generation", @"page"]) {
        if (!MSIMEUnsignedCandidateIdentityValue(button.candidateID[key]) ||
            !MSIMEUnsignedCandidateIdentityValue(_view[key]) || ![button.candidateID[key] isEqual:_view[key]]) return;
    }
    if (!MSIMEUnsignedCandidateIdentityValue(_view[@"page_count"])) return;
    const NSUInteger page = [_view[@"page"] unsignedIntegerValue];
    const NSUInteger count = [_view[@"page_count"] unsignedIntegerValue];
    if (count == 0 || page >= count) return;
    if (button.tag == -1 && page > 0) [self apply:[_session command:MSIME_PREVIOUS_PAGE error:nil]];
    if (button.tag == -2 && count > 0 && page < count - 1) [self apply:[_session command:MSIME_NEXT_PAGE error:nil]];
}
@end
