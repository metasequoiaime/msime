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
#include "../settings/ShuangpinProfileNames.h"
#include "../candidate/CandidateWheelRouting.h"
#import "../core/ChineseTextConversion.h"
#include "../core/FullWidthInput.h"
#include "InputControllerPhysicalKeys.h"
#include "InputSchemeTraits.h"
#include "../core/ModifierTap.h"
#import "../settings/ShuangpinKeymapPanel.h"
#import "../core/FloatingToolbarPanel.h"
#import "InputModeHUDPanel.h"
#import "TypingEffectPanel.h"
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
#include <memory>

// Implemented by the Swift backend dylib loaded by input_method_main.mm. The account provider
// keeps credentials and transport on the Swift side; this process receives only bounded glosses.
extern "C" void MSIMEFetchAccountCandidateGlosses(const char *wordsJSON, const char *primaryCode,
                                                    const char *secondaryCode, unsigned long long generation)
    __attribute__((weak_import));
extern "C" void MSIMECancelAccountCandidateGlosses(void) __attribute__((weak_import));
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

// The local calendar day of this moment, as the statistics store names days.
static NSString *MSIMETypingStatisticsLocalDay(void) {
    NSDateComponents *components = [NSCalendar.currentCalendar components:NSCalendarUnitYear | NSCalendarUnitMonth |
        NSCalendarUnitDay fromDate:NSDate.date];
    return [NSString stringWithFormat:@"%04ld-%02ld-%02ld", (long)components.year, (long)components.month,
        (long)components.day];
}

// Writes one batch of key heatmap counts. The request is built and sent on the statistics worker, so the key path only hands over the batch it has already collected. A caller about to exit the process waits for the write, and with it every earlier write still queued on the serial worker, because exit does not run queued blocks.
static void MSIMERecordKeyPresses(NSString *directory, msime::mac::KeyPressFlush flush, bool waitUntilWritten) {
    if (!MSIMETypingStatisticsEnabled.load(std::memory_order_relaxed)) return;
    if (![directory isKindOfClass:NSString.class] || !directory.isAbsolutePath || flush.keys.empty()) return;
    auto batch = std::make_shared<msime::mac::KeyPressFlush>(std::move(flush));
    NSString *path = [directory copy];
    (waitUntilWritten ? dispatch_sync : dispatch_async)(MSIMETypingStatisticsQueue(), ^{
        NSMutableDictionary<NSString *, NSNumber *> *keys = [NSMutableDictionary dictionaryWithCapacity:batch->keys.size()];
        for (const auto &[key, count] : batch->keys)
            keys[[NSString stringWithUTF8String:key.c_str()]] = @(count);
        NSDictionary *request = @{ @"directory": path, @"action": @{
            @"operation": @"record_keys", @"day": [NSString stringWithUTF8String:batch->day.c_str()], @"keys": keys } };
        NSData *data = [NSJSONSerialization dataWithJSONObject:request options:0 error:nil];
        if (!data) {
            msime_macos_diagnostic_write("stats: record_keys_failed reason=encode");
            return;
        }
        char *response = msime_client_typing_statistics(static_cast<const uint8_t *>(data.bytes), data.length);
        // Like the text path, a failure is logged as a label only and never reaches the key path.
        if (!response) {
            msime_macos_diagnostic_write("stats: record_keys_failed reason=no_response");
            return;
        }
        if (msime_macos_diagnostic_enabled()) {
            NSData *body = [NSData dataWithBytesNoCopy:response length:strlen(response) freeWhenDone:NO];
            NSDictionary *envelope = [NSJSONSerialization JSONObjectWithData:body options:0 error:nil];
            if (![envelope isKindOfClass:NSDictionary.class])
                msime_macos_diagnostic_write("stats: record_keys_failed reason=malformed_response");
            else if (![envelope[@"ok"] isEqual:@YES])
                msime_macos_diagnostic_write("stats: record_keys_failed reason=store");
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

// A view or commit context says itself whether Simplified-to-Traditional conversion applies to its text (`script_conversion`): the runtime decides it from the scheme and the local mode the text was made in. The runtime always writes the field and this host builds no view or commit context of its own, so a dictionary without it did not come from the runtime and its text is left as it is.
static BOOL MSIMEScriptConversionApplies(id value) {
    return [value isKindOfClass:NSDictionary.class] && [value[@"script_conversion"] isEqual:@YES];
}

// commit_context.typing_statistics is false for text the expression, command and mention modes produced: a result the Engine worked out, not something the user typed. A context without the field predates it and counts.
static BOOL MSIMECommitCountsAsTyping(id context) {
    if (![context isKindOfClass:NSDictionary.class]) return YES;
    id counts = context[@"typing_statistics"];
    return ![counts isKindOfClass:NSNumber.class] || [counts boolValue];
}

// Whether the Engine takes this character as input in the view's state: View.spelling_symbols lists the non-letter keys the active mode spells with (digits and operators in expression mode, digits in Unicode mode, Zhuyin's bopomofo and tone keys including Space, Cantonese's syllable apostrophe) and, with nothing composed, the keys that open a mode (/ and @). Such a key belongs to the Engine even where this host would otherwise read it as a candidate digit, a paging key or a punctuation shortcut.
static BOOL MSIMESpellingSymbol(NSDictionary *view, unichar character) {
    NSString *symbols = [view isKindOfClass:NSDictionary.class] ? view[@"spelling_symbols"] : nil;
    if (![symbols isKindOfClass:NSString.class] || character == 0 || character > 0x7F) return NO;
    return [symbols rangeOfString:[NSString stringWithCharacters:&character length:1]].location != NSNotFound;
}
static BOOL MSIMESpellingSymbolString(NSDictionary *view, NSString *characters) {
    return characters.length == 1 && MSIMESpellingSymbol(view, [characters characterAtIndex:0]);
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
    // The reading each row draws on its gloss run above any translation (a Korean Hanja's 훈음, empty elsewhere), and the tooltip that shows the row's text with everything under it.
    NSArray<NSString *> *readings = @[];
    NSArray<NSString *> *tooltips = @[];
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

// A word is known once the account answered it, with a gloss or with nothing (a negative entry that lapses after eight minutes), and a known word is not asked about again.
static BOOL MSIMEAccountGlossKnown(NSString *target, NSString *text) {
    return [[MSIMETranslationCache sharedCache] valueForIdentity:MSIMEAccountGlossIdentity(target, text)] != nil;
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

// A theme colour for a floating panel that resolves in whichever appearance the panel is drawn in: the light palette's value in Aqua, the dark palette's in Dark Aqua.
static NSColor *MSIMEThemedSkinColor(NSString *name, msime::mac::Rgba lightColor, msime::mac::Rgba darkColor) {
    NSColor *lightValue = SkinColor(lightColor), *darkValue = SkinColor(darkColor);
    return [NSColor colorWithName:name dynamicProvider:^NSColor *(NSAppearance *appearance) {
        return [appearance bestMatchFromAppearancesWithNames:@[NSAppearanceNameAqua, NSAppearanceNameDarkAqua]] == NSAppearanceNameDarkAqua ? darkValue : lightValue;
    }];
}

// candidate_scale_percent as a factor: the candidate window is laid out at 100% and every font and length of it is multiplied by this, so 150% is the same window half as large again rather than a larger font in the old frame.
static CGFloat MSIMECandidateScale(MSIMEAppearancePreferences *appearance) {
    return appearance.candidateScalePercent / 100.0;
}

// The reading in the candidate window's top row is set semibold (dc.html L1325) at the size the user picked for it. A resolved family is named by its face (Menlo-Regular), which pins the weight, so the face is swapped for the family and the fallback cascade is kept; a family whose nearest heavier face is bold draws bold, and one with no heavier face keeps its regular one.
static NSFont *MSIMECandidatePreeditFont(MSIMEAppearancePreferences *appearance) {
    const CGFloat size = appearance.preeditFontSize * MSIMECandidateScale(appearance);
    NSFont *regular = [appearance candidateFontOfSize:size englishFirst:YES];
    NSMutableDictionary *attributes = [regular.fontDescriptor.fontAttributes mutableCopy];
    if (attributes[NSFontNameAttribute] && regular.familyName) {
        [attributes removeObjectForKey:NSFontNameAttribute];
        attributes[NSFontFamilyAttribute] = regular.familyName;
    }
    attributes[NSFontTraitsAttribute] = @{NSFontWeightTrait: @(NSFontWeightSemibold)};
    return [NSFont fontWithDescriptor:[NSFontDescriptor fontDescriptorWithFontAttributes:attributes] size:size] ?: regular;
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

// Translation replies may replace the view while leaving every actionable field unchanged.
// Comparing the rest of the view also protects preedit, paging and candidate menu actions.
static BOOL MSIMEOnlyCandidateTranslationsChanged(NSDictionary *before, NSDictionary *after) {
    if (![before isKindOfClass:NSDictionary.class] || ![after isKindOfClass:NSDictionary.class]) return NO;
    NSArray *oldCandidates = before[@"candidates"], *newCandidates = after[@"candidates"];
    if (![oldCandidates isKindOfClass:NSArray.class] || ![newCandidates isKindOfClass:NSArray.class] ||
        oldCandidates.count == 0 || oldCandidates.count != newCandidates.count) return NO;
    NSMutableDictionary *oldView = [before mutableCopy], *newView = [after mutableCopy];
    [oldView removeObjectForKey:@"candidates"];
    [newView removeObjectForKey:@"candidates"];
    if (![oldView isEqual:newView]) return NO;
    BOOL changed = NO;
    for (NSUInteger index = 0; index < oldCandidates.count; ++index) {
        NSDictionary *oldCandidate = oldCandidates[index], *newCandidate = newCandidates[index];
        if (![oldCandidate isKindOfClass:NSDictionary.class] || ![newCandidate isKindOfClass:NSDictionary.class]) return NO;
        changed |= ![CandidateTranslation(oldCandidate) isEqual:CandidateTranslation(newCandidate)];
        NSMutableDictionary *oldFields = [oldCandidate mutableCopy], *newFields = [newCandidate mutableCopy];
        [oldFields removeObjectForKey:@"translation"];
        [newFields removeObjectForKey:@"translation"];
        if (![oldFields isEqual:newFields]) return NO;
    }
    return changed;
}


static BOOL MSIMESmartPunctuationKey(unichar character) {
    return character == ',' || character == '.' || character == ':';
}
static BOOL MSIMEASCIIAlphanumeric(unichar character) {
    return (character >= '0' && character <= '9') || (character >= 'A' && character <= 'Z') ||
           (character >= 'a' && character <= 'z');
}
// The scheme's own rules hold: dedicated English and the local modes keep their own rules inside any scheme, so a scheme trait only applies outside them.
static BOOL MSIMESchemeRulesApply(NSDictionary *view) {
    if (![view isKindOfClass:NSDictionary.class] || [view[@"dedicated_english"] isEqual:@YES]) return NO;
    id mode = view[@"local_mode"];
    return ![mode isKindOfClass:NSString.class] || [mode isEqualToString:@"none"];
}
// The view's scheme number; a view without one reads as quanpin, the Engine's default scheme.
static int MSIMEViewScheme(NSDictionary *view) {
    return [view isKindOfClass:NSDictionary.class] ? [view[@"scheme"] intValue] : msime::mac::scheme::Quanpin;
}
// A scheme trait that holds while the view's scheme rules apply.
static BOOL MSIMESchemeTrait(NSDictionary *view, bool (*trait)(int)) {
    return MSIMESchemeRulesApply(view) && trait(MSIMEViewScheme(view));
}
// The scheme's openable candidate list (the Korean Hanja list) is showing, as the view reports it (msime_client.h, MSIME_OPEN_CANDIDATE_LIST).
static BOOL MSIMECandidateListOpen(NSDictionary *view) {
    return [view isKindOfClass:NSDictionary.class] && [view[@"candidate_list_open"] isEqual:@YES];
}
// The opened list is the Korean Hanja list, whose rows carry each Hanja's 훈음 as their annotation; a Zhuyin list carries none.
static BOOL MSIMEKoreanHanjaListOpen(NSDictionary *view) {
    return MSIMECandidateListOpen(view) && MSIMEViewScheme(view) == msime::mac::scheme::Korean;
}
// Candidates may carry translation glosses. Temporary Japanese keeps the Chinese scheme number in the view but writes Japanese, which has no glosses.
static BOOL MSIMEViewShowsGlosses(NSDictionary *view) {
    return msime::mac::scheme::ShowsGlosses(MSIMEViewScheme(view)) && ![view[@"local_mode"] isEqual:@"temporary_japanese"];
}
// Option+Return (or keypad Enter) with no other modifier while a scheme that opens a candidate list composes, the system Korean input method's Hanja key. Either Option key counts.
static BOOL MSIMECandidateListTrigger(NSEvent *event, NSDictionary *view) {
    if (event.type != NSEventTypeKeyDown || (event.keyCode != 36 && event.keyCode != 76)) return NO;
    if ((event.modifierFlags & (NSEventModifierFlagControl | NSEventModifierFlagShift | NSEventModifierFlagOption |
                                NSEventModifierFlagCommand)) != NSEventModifierFlagOption) return NO;
    if (!MSIMESchemeTrait(view, msime::mac::scheme::OpensCandidateList)) return NO;
    id editing = view[@"editing_text"];
    return [editing isKindOfClass:NSString.class] && [editing length] > 0;
}
// Down with no modifier while a Zhuyin conversion composes and its list is closed: libchewing's key for opening the candidate list. It belongs to the input method whatever the arrow navigation binding says, because with the list closed there is no highlight for that binding to move and nothing else to do with the key. Once the list is open Down moves the highlight as in every other list.
static BOOL MSIMECandidateListDownKey(NSEvent *event, NSDictionary *view) {
    if (event.type != NSEventTypeKeyDown || event.keyCode != 125) return NO;
    if (event.modifierFlags & (NSEventModifierFlagControl | NSEventModifierFlagShift | NSEventModifierFlagOption |
                               NSEventModifierFlagCommand)) return NO;
    if (!MSIMESchemeRulesApply(view) || MSIMEViewScheme(view) != msime::mac::scheme::Zhuyin || MSIMECandidateListOpen(view)) return NO;
    id editing = view[@"editing_text"];
    return [editing isKindOfClass:NSString.class] && [editing length] > 0;
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
- (NSDictionary *)serviceSnapshotQuery;
- (NSDictionary *)serviceSnapshotView;
- (void)invalidateServiceSnapshots;
@end

// The full-colour brand mark the floating toolbar and the mode HUD lead with, loaded once. Nil when the bundle has no icon, and the top row then carries no mark.
static NSImage *MSIMECandidateLogoImage() {
    static NSImage *logo;
    static dispatch_once_t once;
    dispatch_once(&once, ^{
        NSString *path = [[NSBundle bundleForClass:MSIMEInputController.class] pathForResource:@"MSIMEClientInputMethod" ofType:@"icns"];
        logo = path ? [[NSImage alloc] initWithContentsOfFile:path] : nil;
    });
    return logo;
}

@implementation MSIMEInputController {
    MSIMEClientSession *_session;
    // Stands in for TISEnableInputSource when set, so a test can see which opt-in mode a scheme change enables.
    OSStatus (^_optInInputModeEnabler)(NSString *identifier);
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
    NSDictionary *_renderedCandidateView;
    // Bumped by every apply:. Writing marked text is a synchronous call into the client, and IMK services the next key inside it, so an apply: can finish after a newer one that ran nested in it.
    uint64_t _applySequence;
    // Bumped when a gloss arrival replaces the view, which can also happen inside an apply:'s marked-text write.
    uint64_t _glossViewSequence;
    // Whether the client may still hold marked text this controller's compositions wrote; see MSIMEApplyTransitionTrackingMarkedText. Wrongly YES costs one redundant clear, wrongly NO leaves a composition stranded in the document, so anything that might mark text sets it.
    BOOL _clientHasMarkedText;
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
    // Key heatmap counts not yet written, and the timer that writes them if typing stops before a batch fills.
    msime::mac::KeyPressBatch _keyPressBatch;
    NSTimer *_keyPressFlushTimer;
    // The closing mark this host owes the document while a pair is open. It rides in the marked
    // text after the caret, because IMK gives an input method no way to move a client's insertion
    // point; see MSIMEApplyTransitionWithPendingClosing.
    NSString *_pendingPairedClosing;
    // The closing mark for a pair this host opened itself from the `{` key, which the Engine commits as ASCII and so never names as an opening mark. Consumed by the next apply:.
    NSString *_hostOpenedClosing;
    NSNumber *_typingSourceOverride;
    // Whether secure event input was on at the last key or activation. Nothing is played while it is, and background music waits for it to go off; see secureEventInputActive.
    BOOL _secureEventInput;
    // Typing effects: whether the foreground application holds a full-screen display, and the answers of msime_client_typing_effect not yet drawn. The full-screen state walks the window list, so it is read on the main queue's next turn, never on a key: after activation, after a preference update, when the active space changes (an application entering or leaving native full screen), and at most once a second while keys arrive (a borderless full-screen window changes no space). Keys only record their answer here; drawing waits for the main queue's next turn so a key never waits on it, and keys arriving in between are drawn once with the latest count.
    BOOL _typingEffectFullscreen;
    NSTimeInterval _typingEffectFullscreenCheckedAt;
    BOOL _typingEffectScheduled;
    BOOL _typingEffectCommit;
    uint32_t _typingEffectPacked;
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
    // Batches whose page went away while a paid request was in flight. They are held here until that request lands, so its answer still reaches the cache and the glossary (Windows' cloud worker caches before its staleness check, cloud_translation.cpp); a batch deallocated early would cancel it.
    NSMutableSet<MSIMECustomTranslationBatch *> *_detachedCustomBatches;
    // Bumped only when custom work is cancelled outright (the provider or its switch changed, or the controller went away); a reply from before that must not reach the cache, since the cache identity carries no credentials.
    uint64_t _customHardEpoch;
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
    // The account is asked only once typing has been idle for 500 ms, as Windows' cloud worker waits kIdleDelay after the latest job (cloud_translation.cpp): each keystroke replaces the pending request instead of sending one.
    NSTimer *_accountGlossTimer;
    // The view generation the held translation results were last applied to. The session drops its translations whenever the generation moves, and a page whose words did not change does not change any request either, so this is what tells synchronizeCandidateServices to put them back.
    NSNumber *_translationAppliedGeneration;
    // Each word this controller sent to the account for an English gloss, mapped to the preferences directory of the page that sent it: every controller hears every reply, so only the one that asked saves it, and the reply often lands after that page has moved on.
    NSMutableDictionary<NSString *, NSString *> *_accountEnglishQueries;
    NSDictionary *_onDeviceGlossRequest;
    // Candidate services all inspect the same Engine state during one render pass. Cache each
    // snapshot lazily for that pass; applyTranslations invalidates it before changing the state.
    BOOL _serviceSnapshotActive;
    BOOL _serviceSnapshotQueryLoaded;
    BOOL _serviceSnapshotViewLoaded;
    NSDictionary *_serviceSnapshotQuery;
    NSDictionary *_serviceSnapshotView;
    // Each English word this controller sent to the on-device model, mapped to the English gloss request of the page that sent it: that request carries the Engine source and directory persisting needs, and the reply often lands after the page has moved on.
    NSMutableDictionary<NSString *, NSDictionary *> *_onDeviceEnglishQueries;
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

// The configured scheme, not the temporary J mode, decides whether the two reversible smart punctuation gestures (space-to-ASCII and repeat-to-Chinese) may run (`host_smart_punctuation`), as Windows KeyEventSink only claims them outside Global::JapaneseInputModeEnabled. Korean and Vietnamese write half-width ASCII punctuation, so neither gesture has a Chinese mark to convert from or back to.
- (BOOL)schemeAllowsSmartPunctuationGestures {
    return msime::mac::scheme::HostSmartPunctuation(MSIMEViewScheme(_view));
}

// The scheme's punctuation goes through the Chinese table (`uses_chinese_punctuation`). Where it does not (Korean, Vietnamese) the marks are ASCII already, and the Engine refuses to arm the repeat gesture that would turn them Chinese.
- (BOOL)schemeUsesChinesePunctuation {
    return msime::mac::scheme::UsesChinesePunctuation(MSIMEViewScheme(_view));
}

// Arms the space conversion from what a punctuation commit actually put in the document, as the reference's _NoteCommittedChinesePunctuation does: the committed tail decides, so a candidate committed together with its mark (nihao, gives 你好，) arms too, and the ASCII target comes from the mark map rather than the key pressed (the backslash key gives 、, which converts to /). Called after apply:, so an opening mark this host has just auto-closed is seen as a pending closing and does not arm. Anything else disarms.
- (void)noteCommittedChinesePunctuation:(NSDictionary *)transition client:(id)client {
    NSString *commit = transition[@"commit"];
    const unichar ascii = [commit isKindOfClass:NSString.class] && commit.length
        ? MSIMEASCIIForSmartChinesePunctuation([commit characterAtIndex:commit.length - 1])
        : 0;
    if (!_appearance.smartPunctuation || !_appearance.smartPunctuationSpaceConvert || !ascii ||
        _pendingPairedClosing || ![self schemeAllowsSmartPunctuationGestures]) {
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
        ![self schemeAllowsSmartPunctuationGestures])
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
        ![self schemeAllowsSmartPunctuationGestures] || NSProcessInfo.processInfo.systemUptime - armedAt > 2.0)
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
    // Korean and Vietnamese marks are ASCII already, and the repeat gesture that would turn them Chinese never arms there: the key goes to the Engine, which writes the mark after the open composition or leaves it to the application. Zhuyin's marks are bopomofo keys or its Shift overlay, which the Engine writes in any state, so the contextual ASCII route below never takes them either.
    if (![self schemeUsesChinesePunctuation] || MSIMEViewScheme(_view) == msime::mac::scheme::Zhuyin) {
        [self resetSmartPunctuationState];
        return NO;
    }
    if (!_appearance.smartPunctuation) {
        [self resetSmartPunctuationState];
        [self clearSmartPunctuationSpaceConversion];
        [self clearSmartPunctuationSpaceRevert];
        return NO;
    }
    const BOOL hasComposition = [_view[@"editing_text"] length] ||
        ([_view[@"candidates"] isKindOfClass:NSArray.class] && [_view[@"candidates"] count]);
    const BOOL gestures = [self schemeAllowsSmartPunctuationGestures];
    if (!MSIMESmartPunctuationKey(character)) return NO;
    // Expression mode's decimal point is part of the number being typed, not a mark to convert.
    if (MSIMESpellingSymbol(_view, character)) return NO;
    if (!_appearance.smartPunctuationRepeatToChinese) {
        _lastSmartPunctuation = 0;
        _smartPunctuationRejected = NO;
    }
    const BOOL repeat = _lastSmartPunctuation == character && !_smartPunctuationRejected &&
        _smartPunctuationClient == client && NSProcessInfo.processInfo.systemUptime - _lastSmartPunctuationTime <= 2.0;
    if (repeat && gestures && _appearance.smartPunctuationRepeatToChinese &&
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
    ++_customHardEpoch;
    [_customTimer invalidate];
    _customTimer = nil;
    [_customBatch cancel];
    for (MSIMECustomTranslationBatch *batch in [_customBatches copy])
        if (batch != _customBatch) [batch cancel];
    [_customBatches removeAllObjects];
    for (MSIMECustomTranslationBatch *batch in [_detachedCustomBatches copy]) [batch cancel];
    [_detachedCustomBatches removeAllObjects];
    _customBatch = nil;
    _customQuery = nil;
    _customResults = nil;
}
// The page moved on: nothing more is sent for it, but a request already paid for is left to land in the cache instead of being thrown away.
- (void)detachCustomTranslations {
    ++_customEpoch;
    [_customTimer invalidate];
    _customTimer = nil;
    if (!_detachedCustomBatches) _detachedCustomBatches = [NSMutableSet set];
    __weak MSIMEInputController *weakSelf = self;
    for (MSIMECustomTranslationBatch *batch in [_customBatches copy]) {
        __weak MSIMECustomTranslationBatch *weakBatch = batch;
        BOOL running = [batch detachWithCompletion:^{
            MSIMEInputController *owner = weakSelf;
            MSIMECustomTranslationBatch *ended = weakBatch;
            if (owner && ended) [owner->_detachedCustomBatches removeObject:ended];
        }];
        if (running) [_detachedCustomBatches addObject:batch];
    }
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
    [self stopAccountGloss];
    [self cancelOnDeviceGloss];
    // A later session can reuse the same generation number, and nothing is held now to re-apply anyway.
    _translationAppliedGeneration = nil;
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
- (NSDictionary *)serviceSnapshotQuery {
    if (!_serviceSnapshotActive) return [_session translationQueryWithError:nil];
    if (!_serviceSnapshotQueryLoaded) {
        _serviceSnapshotQueryLoaded = YES;
        _serviceSnapshotQuery = [_session translationQueryWithError:nil];
    }
    return _serviceSnapshotQuery;
}
- (NSDictionary *)serviceSnapshotView {
    if (!_serviceSnapshotActive) return [_session viewWithError:nil];
    if (!_serviceSnapshotViewLoaded) {
        _serviceSnapshotViewLoaded = YES;
        _serviceSnapshotView = [_session viewWithError:nil];
    }
    return _serviceSnapshotView;
}
- (void)invalidateServiceSnapshots {
    _serviceSnapshotActive = NO;
    _serviceSnapshotQueryLoaded = NO;
    _serviceSnapshotViewLoaded = NO;
    _serviceSnapshotQuery = nil;
    _serviceSnapshotView = nil;
}
- (NSDictionary *)currentCustomTranslationRequest {
    if (!_activeClient || !_session || _focusPending || _appearance.englishMode) return nil;
    // A `/fy` request (command mode) is the user's explicit ask, so the shared query carries it whatever the candidate translation switches say: one English sentence for the selected service of the user's own, into the query's own target language, with no offline dictionary in front of it. None of the gloss gates below apply to it, so the query is read before them, which costs one call per pass while translations are off.
    const BOOL candidateTranslations = !(_appearance && !_appearance.candidateTranslations) && !(_glossEnabled && !_glossEnabled.boolValue);
    NSDictionary *query = [self serviceSnapshotQuery];
    const BOOL command = [query[@"sentence"] isEqual:@YES];
    if (!candidateTranslations && !command) return nil;
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
        (!command && _glossTargetLanguages && ![_glossTargetLanguages isEqual:MSIMETranslationTargets(query)])) return nil;
    NSDictionary *view = [self serviceSnapshotView];
    if (command) {
        NSArray *sentences = [query[@"candidates"] isKindOfClass:NSArray.class] ? query[@"candidates"] : @[];
        NSDictionary *sentence = sentences.count == 1 && [sentences.firstObject isKindOfClass:NSDictionary.class] ? sentences.firstObject : nil;
        NSString *text = [sentence[@"text"] isKindOfClass:NSString.class] ? sentence[@"text"] : nil;
        NSString *target = [query[@"target_language"] isKindOfClass:NSString.class] ? query[@"target_language"] : nil;
        if (!text.length || !target.length || ![view[@"generation"] isEqual:query[@"generation"]]) return nil;
        return @{@"command":@YES, @"target_language":target, @"target_languages":@[target],
            (niuTrans ? @"niutrans" : custom ? @"custom_translation" : @"tencent_tmt"):config, @"candidates":@[@{@"text":text}]};
    }
    // Korean Hanja rows are translated like Chinese ones; their 훈음 is drawn whatever comes back. Japanese, including temporary Japanese composition, is still left out.
    if (!MSIMEViewShowsGlosses(view) || ![view[@"generation"] isEqual:query[@"generation"]]) return nil;
    NSDictionary *gloss = [self currentGlossRequest];
    // Resolve the local dictionary first; never transmit an already-resolved key.
    if (gloss && (![_glossRequest isEqual:gloss] || !_glossResults)) return nil;
    NSMutableArray *candidates = [NSMutableArray array];
    for (NSDictionary *candidate in view[@"candidates"]) {
        if (![candidate[@"text"] isKindOfClass:NSString.class] || ![candidate[@"source"] isKindOfClass:NSNumber.class]) continue;
        [candidates addObject:@{@"text":candidate[@"text"], @"source":candidate[@"source"]}];
    }
    if (!candidates.count) return nil;
    return @{@"target_language":query[@"target_language"], @"target_languages":MSIMETranslationTargets(query),
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
// Returns whether the session took the results and the services were synchronized behind them.
- (BOOL)applyCandidateTranslationResults {
    const BOOL ownSnapshot = !_serviceSnapshotActive;
    if (ownSnapshot) {
        _serviceSnapshotActive = YES;
        _serviceSnapshotQueryLoaded = NO;
        _serviceSnapshotViewLoaded = NO;
        _serviceSnapshotQuery = nil;
        _serviceSnapshotView = nil;
    }
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
    NSDictionary *view = [self serviceSnapshotView];
    if (!view) {
        if (ownSnapshot) [self invalidateServiceSnapshots];
        return NO;
    }
    // Applying translations advances the Engine snapshot. Any enclosing service pass must
    // fetch the new query/view before it asks another provider to synchronize.
    [self invalidateServiceSnapshots];
    NSDictionary *applied = [_session applyTranslations:results generation:[view[@"generation"] unsignedLongLongValue] error:nil];
    if (![applied[@"applied"] boolValue]) return NO;
    // A gloss changes what the card shows, never the composition, so only the card is redrawn. Going through apply: re-sent the marked text on every arrival, and IMK services the next key inside that synchronous setMarkedText: call - the whole keystroke, reranking included, ran nested in it, after which the outer apply: wrote the older view back over the newer one.
    NSDictionary *next = applied[@"view"];
    if (![next isKindOfClass:NSDictionary.class]) return NO;
    // _view is next from here on, so synchronizeCandidateServices below finds this generation already applied and does not come back.
    _translationAppliedGeneration = next[@"generation"];
    if (![next isEqual:_view]) {
        [self discardGlossSensePage];
        _view = next;
        ++_glossViewSequence;
        [self renderCandidates];
    }
    // What one source answered decides what the next asks for, even when it answered nothing for this page: the online fallback waits for the offline lookup.
    [self synchronizeCandidateServices];
    return YES;
}

// The backend keeps one account request in flight and one page waiting, shared by every controller. Only the timer is this controller's, so when it stops asking the account (turned off, a service of the user's own took over, or the field was left) the waiting page is dropped too; otherwise it would go out when the request in flight finishes, after the user opted out.
- (void)stopAccountGloss {
    [self cancelAccountGloss];
    [self cancelQueuedAccountGlosses];
}

- (void)cancelQueuedAccountGlosses {
    // weak_import, like the fetch: a process without the backend dylib binds this to null.
    if (MSIMECancelAccountCandidateGlosses != nullptr) MSIMECancelAccountCandidateGlosses();
}

- (void)cancelAccountGloss {
    ++_accountGlossEpoch;
    [_accountGlossTimer invalidate];
    _accountGlossTimer = nil;
    _accountGlossRequest = nil;
    _accountGlossResults = nil;
    _accountGlossSignature = nil;
}

- (NSDictionary *)currentAccountGlossRequest {
    if (!_activeClient || !_session || _focusPending || _appearance.englishMode || !_appearance.candidateTranslations)
        return nil;
    NSDictionary *query = [self serviceSnapshotQuery];
    if (![query isKindOfClass:NSDictionary.class] || !query[@"generation"] ||
        ![query[@"target_languages"] isKindOfClass:NSArray.class]) return nil;
    // The account endpoint (api.msime.app) is used only when the user explicitly chose it in settings. The shared core already folds in candidate_translations and the precedence of the user's own services, so this flag is the whole decision.
    if (![query[@"translation_account"] isEqual:@YES]) return nil;
    NSArray *candidates = MSIMEOnlineGlossCandidates(query);
    return candidates.count
        ? @{ @"target_languages": query[@"target_languages"], @"candidates": candidates } : nil;
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
    if (!request) { [self stopAccountGloss]; return; }
    if ([_accountGlossRequest isEqual:request]) return;
    // The local dictionaries answer first and the account is asked only for what they left empty, as Windows hands only the misses to RequestMisses (event_listener.cpp ApplyCandidateTranslations). Each dictionary's completion calls back here, so waiting costs one local read rather than a request.
    NSDictionary *gloss = [self currentGlossRequest];
    if (gloss && (![_glossRequest isEqual:gloss] || !_glossResults)) return;
    NSDictionary *targetGloss = [self currentTargetGlossRequest];
    if (targetGloss && (![_targetGlossRequest isEqual:targetGloss] || !_targetGlossResults)) return;
    NSMutableSet<NSString *> *englishAnswered = [NSMutableSet set];
    for (NSDictionary *entry in gloss ? _glossResults : @[])
        if ([entry[@"text"] isKindOfClass:NSString.class] && [entry[@"translation"] isKindOfClass:NSString.class] &&
            [entry[@"translation"] length]) [englishAnswered addObject:entry[@"text"]];
    NSDictionary *targetAnswered = targetGloss ? _targetGlossResults : nil;
    [self cancelAccountGloss];
    _accountGlossRequest = [request copy];
    NSArray *targets = request[@"target_languages"];
    NSMutableArray *pending = [NSMutableArray array];
    // Words whose English row the local dictionaries left empty. Only their English replies are saved to the learned glossary: a word that went out for another target's missing row already has a packaged English gloss, and a learned entry overrides the packaged one, so saving the account's reply would replace the curated gloss for good.
    NSMutableSet<NSString *> *englishPending = [NSMutableSet set];
    NSMutableString *signature = [NSMutableString string];
    for (NSString *target in targets) {
        for (NSDictionary *candidate in request[@"candidates"]) {
            NSString *text = candidate[@"text"];
            if (![target isKindOfClass:NSString.class] || ![text isKindOfClass:NSString.class]) continue;
            [signature appendFormat:@"|%@|%@", target, text];
            // The request carries one word list for every target, so a word goes out while any target still lacks a local answer.
            BOOL answered = [target isEqualToString:@"en"] ? [englishAnswered containsObject:text]
                : [targetAnswered[text][target] isKindOfClass:NSString.class] && [targetAnswered[text][target] length];
            if (answered || MSIMEAccountGlossKnown(target, text)) continue;
            [pending addObject:text];
            if ([target isEqualToString:@"en"]) [englishPending addObject:text];
        }
    }
    _accountGlossResults = [self accountGlossResultsForRequest:request];
    [self applyCandidateTranslationResults];
    if (!pending.count || [_accountGlossSignature isEqualToString:signature]) return;
    _accountGlossSignature = [signature copy];
    NSMutableArray *unique = [NSMutableArray array];
    for (NSString *text in pending) if (![unique containsObject:text]) [unique addObject:text];
    NSString *primary = targets.firstObject ?: @"";
    NSString *secondary = targets.count > 1 ? targets[1] : @"";
    // Cached answers were applied above without waiting; only the network request waits for the idle delay. A newer page cancels this timer through cancelAccountGloss, so the fire-time checks only guard a timer that was already running its block.
    uint64_t epoch = _accountGlossEpoch;
    MSIMEClientSession *session = _session;
    id client = _activeClient;
    NSDictionary *pendingRequest = _accountGlossRequest;
    __weak MSIMEInputController *weakSelf = self;
    _accountGlossTimer = [self customTranslationTimerWithBlock:^(NSTimer *timer) {
        MSIMEInputController *owner = weakSelf;
        if (!owner || owner->_accountGlossEpoch != epoch || owner->_accountGlossTimer != timer) return;
        [timer invalidate]; owner->_accountGlossTimer = nil;
        if (owner->_session != session || owner->_activeClient != client ||
            ![[owner currentAccountGlossRequest] isEqual:pendingRequest]) {
            // Nothing went out, so the same page synced again later must ask rather than match this request and return early.
            owner->_accountGlossRequest = nil;
            owner->_accountGlossSignature = nil;
            return;
        }
        // Recorded when the words actually go out, so a request the user typed past never marks its words as this controller's to save.
        if (englishPending.count && owner->_preferencesDirectory.isAbsolutePath) {
            // Words a newer page displaced never reply, so the map is bounded rather than drained.
            if (!owner->_accountEnglishQueries || owner->_accountEnglishQueries.count > 64)
                owner->_accountEnglishQueries = [NSMutableDictionary dictionary];
            for (NSString *text in englishPending) owner->_accountEnglishQueries[text] = [owner->_preferencesDirectory copy];
        }
        // The request carries no generation, so the one on screen when the words go out is passed along; nothing compares it since replies are cached by word.
        [owner fetchAccountGlosses:unique primary:primary secondary:secondary
                        generation:[[owner->_session translationQueryWithError:nil][@"generation"] unsignedLongLongValue]];
    }];
}

- (void)fetchAccountGlosses:(NSArray<NSString *> *)words primary:(NSString *)primary secondary:(NSString *)secondary
                 generation:(uint64_t)generation {
    // weak_import: the Swift backend supplies this, and a process without the dylib binds it to null. Calling through that is a jump to address zero, which is what the settings window's three call sites have always guarded against and these two did not.
    if (MSIMEFetchAccountCandidateGlosses == nullptr) return;
    NSData *payload = [NSJSONSerialization dataWithJSONObject:words options:0 error:nil];
    if (!payload) return;
    MSIMEFetchAccountCandidateGlosses([[NSString alloc] initWithData:payload encoding:NSUTF8StringEncoding].UTF8String,
                                      primary.UTF8String, secondary.UTF8String, generation);
}

- (void)accountCandidateTranslationsDidArrive:(NSNotification *)notification {
    NSDictionary *info = notification.userInfo;
    if (![info isKindOfClass:NSDictionary.class]) return;
    NSString *target = info[@"target"];
    NSDictionary *values = info[@"translations"];
    if (![target isKindOfClass:NSString.class] || ![values isKindOfClass:NSDictionary.class]) return;
    // Persisted before any check on the current page, as Windows persists every fetch that completes: a reply for a page the user typed past, or a candidate already committed, would otherwise never reach the glossary.
    if ([target isEqualToString:@"en"]) [self persistAccountGlosses:values];
    // Cached before any check on the current page, as Windows caches every completed fetch before its staleness check (cloud_translation.cpp): the key is language and word, not generation, so an answer for a page the user typed past is still right when they back up or type the word again. Every controller hears the reply, and writing the same answer twice is harmless.
    MSIMETranslationCache *cache = [MSIMETranslationCache sharedCache];
    for (NSString *text in values) {
        NSString *value = values[text];
        if (![text isKindOfClass:NSString.class] || ![value isKindOfClass:NSString.class]) continue;
        NSArray *identity = MSIMEAccountGlossIdentity(target, text);
        // An empty answer is remembered as a negative entry so the word is not asked about again for eight minutes, but it must not evict a gloss another reply already supplied for the same word.
        if (value.length) [cache rememberTranslation:value identity:identity];
        else if (![[cache valueForIdentity:identity] isKindOfClass:NSString.class]) [cache rememberTranslation:nil identity:identity];
    }
    // Only a controller still showing the page it asked about redraws. Its results are rebuilt from the cache, which now holds this reply, so a late reply for a word the current page still shows is used rather than dropped.
    if (![_accountGlossRequest isKindOfClass:NSDictionary.class] || ![_accountGlossRequest isEqual:[self currentAccountGlossRequest]]) return;
    // A late reply usually answers words this page does not show, and then there is nothing to redraw.
    NSArray *results = [self accountGlossResultsForRequest:_accountGlossRequest];
    if ([results isEqual:_accountGlossResults]) return;
    _accountGlossResults = results;
    [self applyCandidateTranslationResults];
}

// Windows saves every English gloss it fetches to the user glossary the moment it arrives (cloud_translation.cpp PersistGloss), so the offline lookup answers that word from then on, across restarts. Account glosses used to wait for the commit, which never saved anything: the account query rows carry no Engine source, so the custom translation plan rejected them. The learned glossary needs no source, only the direction, which is always Chinese to English here because the account is only asked about Chinese candidates; it checks eligibility and formats the gloss itself.
- (void)persistAccountGlosses:(NSDictionary *)values {
    NSMutableDictionary<NSString *, NSMutableArray *> *batches = [NSMutableDictionary dictionary];
    for (NSString *text in values) {
        NSString *directory = [text isKindOfClass:NSString.class] ? _accountEnglishQueries[text] : nil;
        if (!directory) continue;
        [_accountEnglishQueries removeObjectForKey:text];
        NSString *value = values[text];
        // The store rejects a whole batch over one word longer than 40 characters or one oversized gloss, so those are left out here rather than taking their neighbours down with them.
        if (![value isKindOfClass:NSString.class] || !value.length || [value lengthOfBytesUsingEncoding:NSUTF8StringEncoding] > 4096 ||
            [text lengthOfBytesUsingEncoding:NSUTF32StringEncoding] > 40 * 4) continue;
        if (!batches[directory]) batches[directory] = [NSMutableArray array];
        [batches[directory] addObject:@{@"text":text, @"direction":@"chinese_to_english", @"translation":value}];
    }
    for (NSString *directory in batches) {
        NSArray *items = batches[directory];
        // The store takes at most nine items per request.
        for (NSUInteger start = 0; start < items.count; start += 9) {
            NSDictionary *request = @{@"directory":directory, @"generation":@0, @"target_language":@"en", @"action":@"remember",
                @"items":[items subarrayWithRange:NSMakeRange(start, MIN((NSUInteger)9, items.count - start))]};
            dispatch_async([MSIMEInputController learnedTranslationQueue], ^{
                [MSIMEClientSession learnedTranslationRequest:request error:nil];
            });
        }
    }
}

- (void)cancelOnDeviceGloss {
    _onDeviceGlossRequest = nil;
}

// Apple's on-device models fill the rows the offline dictionaries leave empty, for users who translate without a service of their own. A service the user configured, or the MSIME account, answers every candidate itself, so this stays idle then. Only Chinese candidates are asked about, by the same per-candidate answer the account path uses.
- (NSDictionary *)currentOnDeviceGlossRequest {
    if (!_activeClient || !_session || _focusPending || _appearance.englishMode || !_appearance.candidateTranslations ||
        (_glossEnabled && !_glossEnabled.boolValue)) return nil;
    NSDictionary *query = [self serviceSnapshotQuery];
    if (![query isKindOfClass:NSDictionary.class] || !query[@"generation"] || [query[@"translation_account"] isEqual:@YES]) return nil;
    for (NSString *service in @[@"niutrans", @"custom_translation", @"tencent_tmt"]) {
        NSDictionary *config = query[service];
        if ([config isKindOfClass:NSDictionary.class] && [config[@"enabled"] isEqual:@YES]) return nil;
    }
    NSArray *targets = MSIMETranslationTargets(query);
    if (!targets.count || (_glossTargetLanguages && ![_glossTargetLanguages isEqual:targets])) return nil;
    NSDictionary *view = [self serviceSnapshotView];
    // Korean Hanja rows are translated like Chinese ones; their 훈음 is drawn whatever comes back. Japanese, including temporary Japanese composition, is still left out.
    if (!MSIMEViewShowsGlosses(view) || ![view[@"generation"] isEqual:query[@"generation"]]) return nil;
    NSMutableArray *candidates = [NSMutableArray array];
    for (NSDictionary *candidate in MSIMEOnlineGlossCandidates(query)) [candidates addObject:@{@"text":candidate[@"text"]}];
    return candidates.count ? @{@"target_languages":targets, @"candidates":[candidates copy]} : nil;
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
    // The on-device model spends about half a second on each word, one word at a time, so a word the packaged English dictionary already answers is not worth its place in that line. The dictionary lookup takes milliseconds; wait for it, as the user's own services do, and its completion comes back here.
    NSDictionary *gloss = [self currentGlossRequest];
    if (gloss && (![_glossRequest isEqual:gloss] || !_glossResults)) return;
    NSMutableSet<NSString *> *dictionaryAnswered = [NSMutableSet set];
    for (NSDictionary *result in gloss ? _glossResults : @[])
        if ([result[@"text"] isKindOfClass:NSString.class] && [result[@"translation"] isKindOfClass:NSString.class] &&
            [result[@"translation"] length])
            [dictionaryAnswered addObject:result[@"text"]];
    // The same holds for the other targets' offline dictionaries, as Windows hands the slow path only what the local dictionaries missed (event_listener.cpp ApplyCandidateTranslations); their completion comes back here too.
    NSDictionary *targetGloss = [self currentTargetGlossRequest];
    if (targetGloss && (![_targetGlossRequest isEqual:targetGloss] || !_targetGlossResults)) return;
    NSDictionary *targetAnswered = targetGloss ? _targetGlossResults : nil;
    _onDeviceGlossRequest = request;
    [self applyCandidateTranslationResults];
    // Only what no earlier page or dictionary already answered, per target, in page order so the first candidate is translated first. The backend works one word at a time and a newer page takes over after the word in flight, so typing does not queue up stale work.
    for (NSString *target in request[@"target_languages"]) {
        NSMutableArray<NSString *> *words = [NSMutableArray array];
        for (NSDictionary *candidate in request[@"candidates"]) {
            NSString *text = candidate[@"text"];
            if ([target isEqualToString:@"en"] && [dictionaryAnswered containsObject:text]) continue;
            if ([targetAnswered[text][target] isKindOfClass:NSString.class] && [targetAnswered[text][target] length]) continue;
            if ([[MSIMETranslationCache sharedCache] valueForIdentity:MSIMEOnDeviceGlossIdentity(target, text)]) continue;
            if (![words containsObject:text]) [words addObject:text];
        }
        // The backend takes at most 32 words, more than any page shows.
        if (words.count > 32) [words removeObjectsInRange:NSMakeRange(32, words.count - 32)];
        if (!words.count) continue;
        if ([target isEqualToString:@"en"] && gloss) {
            // Words a newer page displaced from the backend's queue never reply, so the map is bounded rather than drained.
            if (!_onDeviceEnglishQueries || _onDeviceEnglishQueries.count > 64) _onDeviceEnglishQueries = [NSMutableDictionary dictionary];
            for (NSString *word in words) _onDeviceEnglishQueries[word] = gloss;
        }
        [self fetchOnDeviceGlosses:words targets:@[target]];
    }
}

- (void)onDeviceCandidateTranslationsDidArrive:(NSNotification *)notification {
    NSString *target = notification.userInfo[@"target"];
    NSDictionary *values = notification.userInfo[@"translations"];
    if (![target isKindOfClass:NSString.class] || ![values isKindOfClass:NSDictionary.class]) return;
    // Formatted before it is cached, shown or saved, as Windows formats every fetched gloss first (translation_gloss.cpp FormatGloss): the model can answer over several lines, and a newline kept in the gloss starts a row of its own that pushes the next target's gloss out of place. A gloss that formats to nothing, or to the word itself, is kept as an empty answer so the word is not asked about again.
    NSMutableDictionary<NSString *, NSString *> *formatted = [NSMutableDictionary dictionary];
    for (NSString *text in values) {
        NSString *value = values[text];
        if (![text isKindOfClass:NSString.class] || ![value isKindOfClass:NSString.class]) continue;
        NSString *gloss = [MSIMEClientSession formatTranslationGloss:value error:nil];
        if (!gloss || [gloss isEqualToString:[text stringByTrimmingCharactersInSet:NSCharacterSet.whitespaceAndNewlineCharacterSet]]) gloss = @"";
        formatted[text] = gloss;
        [[MSIMETranslationCache sharedCache] rememberTranslation:gloss identity:MSIMEOnDeviceGlossIdentity(target, text)];
    }
    values = formatted;
    // Persisted whether or not the page is still on screen, as Windows persists every fetch that completes: a reply for a page the user typed past would otherwise sit in the process cache, never be asked for again, and never reach the glossary.
    if ([target isEqualToString:@"en"]) [self persistOnDeviceGlosses:values];
    // Every controller hears the reply; only one still composing the page it asked about merges it.
    if (!_onDeviceGlossRequest || ![_onDeviceGlossRequest isEqual:[self currentOnDeviceGlossRequest]]) return;
    [self applyCandidateTranslationResults];
}

// Windows saves every English gloss it fetches to the user glossary the moment it arrives (cloud_translation.cpp PersistGloss), so the offline lookup answers that word from then on, across restarts. On-device glosses get the same treatment: the model spends about half a second per word, one at a time, and the process cache it otherwise lives in is gone when the input method restarts. Only English, because the learned glossary is the English one, and only words this controller asked about, since every controller hears every reply; each is written with the gloss request of the page that asked, which carries the source the plan needs.
- (void)persistOnDeviceGlosses:(NSDictionary *)values {
    for (NSString *text in values) {
        NSDictionary *query = [text isKindOfClass:NSString.class] ? _onDeviceEnglishQueries[text] : nil;
        if (!query) continue;
        [_onDeviceEnglishQueries removeObjectForKey:text];
        NSString *value = values[text];
        if ([value isKindOfClass:NSString.class] && value.length)
            [self persistFetchedTranslations:@[@{@"text":text, @"translation":value}] forQuery:query];
    }
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
        [self detachCustomTranslations];
        [self synchronizeAccountGloss:[self currentAccountGlossRequest]];
        return;
    }
    [self stopAccountGloss];
    if ([_customQuery isEqual:query]) return;
    [self detachCustomTranslations];
    _customQuery = query;
    // `/fy` translates one English sentence into the query's own target, Chinese, which the candidate target list does not name and the candidate plan refuses; it is its own plan item and is neither read from nor written to the gloss cache.
    const BOOL command = [query[@"command"] isEqual:@YES];
    NSArray<NSString *> *targets = command ? query[@"target_languages"] : MSIMETranslationTargets(query);
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
        NSArray *plan = command ? @[@{@"text":targetCandidates.firstObject[@"text"], @"key":targetCandidates.firstObject[@"text"],
            @"source_language":@"en", @"target_language":target}]
            : [MSIMEClientSession customTranslationPlan:@{@"target_language":target, @"candidates":targetCandidates} error:nil];
        NSMutableArray *pending = [NSMutableArray array];
        NSMutableDictionary *identities = [NSMutableDictionary dictionary];
        for (NSDictionary *item in plan) {
            NSArray *identity = @[scope, target, item[@"source_language"], item[@"target_language"], item[@"key"]];
            NSString *workKey = MSIMETranslationWorkKey(target, item[@"text"]);
            id value = command ? nil : [cache valueForIdentity:identity];
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
                if (!command) identities[workKey] = identity;
                continue;
            }
            NSDictionary *descriptor = [MSIMEClientSession customTranslationHTTPRequest:@{@"config":query[@"custom_translation"],
                @"text":item[@"key"], @"source_language":item[@"source_language"], @"target_language":item[@"target_language"]} error:nil];
            if (descriptor) {
                [pending addObject:@{@"text":item[@"text"], @"request":descriptor}];
                if (!command) identities[workKey] = identity;
            } else if (!command) {
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
    uint64_t hardEpoch = _customHardEpoch;
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
            // Each response is cached and saved as it lands, whether or not its page is still on screen, as Windows caches and runs PersistGloss before its staleness check (cloud_translation.cpp). Only the items a response answered are cached: one the deadline kept from being sent is left free to be asked again, where caching the whole chunk used to hide it for eight minutes.
            void (^onReply)(NSArray<NSDictionary *> *, NSArray<NSString *> *) = ^(NSArray<NSDictionary *> *results, NSArray<NSString *> *answeredTexts) {
                MSIMEInputController *latest = weakSelf;
                if (!latest || latest->_customHardEpoch != hardEpoch) return;
                for (NSString *text in answeredTexts) {
                    NSString *translation = nil;
                    for (NSDictionary *result in results)
                        if ([result[@"text"] isEqual:text]) { translation = result[@"translation"]; break; }
                    NSArray *identity = identities[MSIMETranslationWorkKey(target, text)];
                    if (identity) [cache rememberTranslation:translation identity:identity];
                }
                // Windows saves every successful English fetch to the user gloss store (cloud_translation.cpp PersistGloss), not just the committed candidate.
                if ([target isEqual:@"en"]) [latest persistFetchedTranslations:results forQuery:query];
            };
            void (^completion)(NSArray<NSDictionary *> *) = ^(NSArray<NSDictionary *> *results) {
                MSIMEInputController *latest = weakSelf;
                if (!latest || latest->_customEpoch != epoch || latest->_session != session || latest->_activeClient != client ||
                    ![[latest currentCustomTranslationRequest] isEqual:query]) return;
                for (NSDictionary *item in items) {
                    NSString *text = item[@"text"];
                    NSString *translation = nil;
                    for (NSDictionary *result in results)
                        if ([result[@"text"] isEqual:text]) { translation = result[@"translation"]; break; }
                    if (translation.length) {
                        NSMutableDictionary *byTarget = values[text];
                        if (!byTarget) { byTarget = [NSMutableDictionary dictionary]; values[text] = byTarget; }
                        byTarget[target] = translation;
                    }
                }
                latest->_customResults = [combinedResults() copy];
                [latest applyCandidateTranslationResults];
                latest->_customBatch = nil;
            };
            MSIMECustomTranslationBatch *batch = niuTrans ? [owner niuTransBatchForItems:items config:query[@"niutrans"] completion:completion]
                : tencent ? [owner tencentBatchForItems:items config:query[@"tencent_tmt"] completion:completion]
                : [owner customBatchForItems:items completion:completion];
            batch.onReply = onReply;
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
    NSDictionary *query = [self serviceSnapshotQuery];
    NSArray *targets = MSIMETranslationTargets(query);
    if (!query || ![targets containsObject:@"en"]) return nil;
    if (_glossTargetLanguages && ![_glossTargetLanguages isEqual:targets]) return nil;
    NSDictionary *view = [self serviceSnapshotView];
    // Windows suppresses candidate translations in Japanese, including a temporary Japanese composition whose view retains its original scheme. Korean Hanja rows are glossed like Chinese ones: the gloss request also looks a Traditional Hanja up under its Simplified characters.
    if (!MSIMEViewShowsGlosses(view)) return nil;
    if (![view[@"generation"] isEqual:query[@"generation"]]) return nil;
    NSMutableArray *candidates = [NSMutableArray array];
    for (NSDictionary *candidate in view[@"candidates"])
        if ([candidate[@"text"] isKindOfClass:NSString.class] && [candidate[@"source"] isKindOfClass:NSNumber.class])
            [candidates addObject:@{@"text":candidate[@"text"], @"source":candidate[@"source"]}];
    return candidates.count ? @{@"target_languages":targets, @"candidates":[candidates copy],
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
    // The request leaves the generation out so a page whose content did not change compares equal; the reader still needs the one it was read for, captured here from the same query the request was built from, so a synchronization pass asks the session once.
    NSNumber *generation = [self serviceSnapshotQuery][@"generation"] ?: @0;
    NSMutableDictionary *read = [request mutableCopy];
    read[@"generation"] = generation;
    if (!_glossQueue) { _glossQueue = [NSOperationQueue new]; _glossQueue.maxConcurrentOperationCount = 1; _glossQueue.qualityOfService = NSQualityOfServiceUtility; }
    const uint64_t epoch = _glossEpoch;
    MSIMEClientSession *session = _session;
    id client = _activeClient;
    __weak MSIMEInputController *weakSelf = self;
    [_glossQueue addOperationWithBlock:^{
        id reader = hasResources ? weakSelf : nil; // id: handed to MSIMEReleaseControllerOnMain
        NSDictionary *result = reader ? [reader readCandidateGloss:read resources:resources] : nil;
        MSIMEReleaseControllerOnMain(&reader);
        if (result && ![result[@"generation"] isEqual:generation]) return;
        NSMutableArray *translations = [result[@"translations"] mutableCopy] ?: [NSMutableArray array];
        if (directory.isAbsolutePath && learnedItems.count) {
            __block NSDictionary *learned;
            dispatch_sync([MSIMEInputController learnedTranslationQueue], ^{
                learned = [MSIMEClientSession learnedTranslationRequest:@{@"directory":directory,
                    @"generation":generation, @"target_language":@"en", @"action":@"lookup", @"items":learnedItems} error:nil];
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
                ![[current currentGlossRequest] isEqual:request] || (result && ![result[@"generation"] isEqual:generation])) return;
            current->_glossResults = [translations copy];
            [current applyCandidateTranslationResults];
            // On-device translation and the account request wait for the dictionary so they can skip what the dictionary answered.
            [current synchronizeOnDeviceGloss];
            [current synchronizeAccountGloss:[current currentAccountGlossRequest]];
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
    NSDictionary *query = [self serviceSnapshotQuery];
    NSArray *targets = MSIMETranslationTargets(query);
    if (_glossTargetLanguages && ![_glossTargetLanguages isEqual:targets]) return nil;
    NSArray *installed = [query[@"offline_gloss_languages"] isKindOfClass:NSArray.class] ? query[@"offline_gloss_languages"] : @[];
    NSMutableArray<NSString *> *languages = [NSMutableArray array];
    for (NSString *target in targets)
        if (![target isEqual:@"en"] && [installed containsObject:target]) [languages addObject:target];
    if (!languages.count) return nil;
    NSDictionary *view = [self serviceSnapshotView];
    if (!MSIMEViewShowsGlosses(view)) return nil;
    if (![view[@"generation"] isEqual:query[@"generation"]]) return nil;
    NSMutableArray *candidates = [NSMutableArray array];
    for (NSDictionary *candidate in view[@"candidates"])
        if ([candidate[@"text"] isKindOfClass:NSString.class] && [candidate[@"source"] isKindOfClass:NSNumber.class])
            [candidates addObject:@{@"text":candidate[@"text"], @"source":candidate[@"source"]}];
    return candidates.count ? @{@"target_languages":targets, @"offline_languages":[languages copy],
        @"candidates":[candidates copy]} : nil;
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
    // As for the English read, the generation is captured here rather than carried in the request, from the query the request was built from.
    NSNumber *generation = [self serviceSnapshotQuery][@"generation"] ?: @0;
    NSMutableDictionary *read = [request mutableCopy];
    read[@"generation"] = generation;
    if (!_targetGlossQueue) { _targetGlossQueue = [NSOperationQueue new]; _targetGlossQueue.maxConcurrentOperationCount = 1; _targetGlossQueue.qualityOfService = NSQualityOfServiceUtility; }
    const uint64_t epoch = _targetGlossEpoch;
    MSIMEClientSession *session = _session;
    id client = _activeClient;
    __weak MSIMEInputController *weakSelf = self;
    [_targetGlossQueue addOperationWithBlock:^{
        NSMutableDictionary<NSString *, NSMutableDictionary<NSString *, NSString *> *> *values = [NSMutableDictionary dictionary];
        id reader = weakSelf;
        for (NSString *language in request[@"offline_languages"]) {
            NSDictionary *result = [reader readTargetGloss:read language:language resources:resources];
            if (![result[@"generation"] isEqual:generation]) continue;
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
            // On-device translation and the account request wait for this dictionary so they can skip what it answered.
            [current synchronizeOnDeviceGloss];
            [current synchronizeAccountGloss:[current currentAccountGlossRequest]];
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
    // Switching statistics off drops what was collected rather than writing it.
    if (!MSIMETypingStatisticsEnabled.load(std::memory_order_relaxed)) {
        _keyPressBatch.clear();
        [_keyPressFlushTimer invalidate];
        _keyPressFlushTimer = nil;
    }
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
    // The view's scheme numbers, in the Engine's order: quanpin, shuangpin, wubi, japanese, korean, cantonese, zhuyin, vietnamese.
    NSArray<NSString *> *schemes = MSIMEInputSchemeNames();
    const NSInteger index = [_view[@"scheme"] integerValue];
    NSString *scheme = index >= 0 && index < (NSInteger)schemes.count ? schemes[index] : @"quanpin";
    NSString *profile = _view[@"shuangpin_profile"];
    if (![profile isKindOfClass:NSString.class]) profile = _appearance.shuangpinProfile;
    NSDictionary<NSString *, NSString *> *schemeTitles = @{
        @"quanpin": @"全拼",
        @"shuangpin": @(msime::mac::ShuangpinSchemaTitle(profile.UTF8String ?: "")),
        @"wubi": @"五笔 86",
        @"japanese": @"日语",
        @"korean": @"韩语",
        @"cantonese": @"粤拼",
        @"zhuyin": @"注音",
        @"vietnamese": @"越南语",
    };
    [_toolbar updateEnglishInputMode:_appearance.englishMode
             englishCandidateMode:englishCandidateMode
                              scheme:scheme
                         schemeTitle:schemeTitles[scheme]
                            capsLock:_capsLock
              chinesePunctuationEnabled:_appearance.runtimeChinesePunctuation
                       fullWidthEnabled:_appearance.runtimeFullWidthInput
        traditionalChineseOutputEnabled:_appearance.traditionalOutput];
}
- (void)appearanceChanged:(NSNotification *)notification {
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
    [_toolbar applyLightToolbarSkin:[_appearance toolbarSkinForDark:NO]
                            darkSkin:[_appearance toolbarSkinForDark:YES]];
    [self refreshFloatingToolbarState];
    if (_activeClient) [self renderCandidates];
    if (_activeClient) [_toolbar setVisible:_appearance.floatingToolbarEnabled forDelegate:self];
    // The settings window can move the scheme in or out of japanese or korean, which moves the menu bar between 中, 日 and 한.
    if (_activeClient) [self syncSystemInputModeForClient:_activeClient];
    // Every Shift tap lands here. Saving then wrote this process's whole view of the settings over the shared document once per mode switch, so a controller that had not yet reloaded an edit made elsewhere put the old value back, and two writers that disagreed traded the document on every tap.
    if (![notification.userInfo[MSIMEAppearanceInputModeOnlyKey] isEqual:@YES]) [self persistAppearancePreferences];
}
// One save in flight for the whole process. IMK keeps a controller per text input client and every one of them observes the same shared appearance, so one change asks each of them to save the same document; with a save state per controller they each ran a load and a compare-and-swap save under the exclusive preferences lock, fsync included, at the same time. What they save is the shared appearance, whichever controller asks, so one save and at most one queued behind it cover them all.
static MSIMEPreferenceSaveState MSIMESharedPreferenceSaveState;
// The controller behind the queued save, which runs it when the one in flight finishes, whether or not the controller that started that one is still alive.
static __weak MSIMEInputController *MSIMEQueuedPreferenceSaver;
// 当前持有焦点客户端的控制器。每个控制器都观察同一份外观设置，改动由最先注册的那个保存，而它往往是早已失去焦点的控制器；所以保存之后，要把文档载入会话的是这一个。
static __weak MSIMEInputController *MSIMEFocusedController;
// 方案只能经由共享偏好文档到达 Engine，而外观改动触发的保存在主线程之外进行，所以从 한 切到 中 之后紧接着敲的键仍按韩文组字，未完成的音节又会把新方案一直挡到它结束。因此切换时在这里、在下一个按键之前于主线程写入文档，并把写入的内容原样交给持有焦点的会话。切换方案前已经结束了组字，会话处于空闲状态，会立即应用新方案。外观改动另外触发的那次保存会再写一遍相同的偏好，会话视为没有变化。Fcitx5 切换方案时同样等待保存完成；这里保存失败时，切换交由那次后台保存完成。
- (void)applyPreferencesToSessionNow {
    if (!_session || !_activeClient || !_preferencesDirectory || !_appearance) return;
    NSDictionary *overrides = [_appearance sharedPreferencesByMerging:@{}];
    NSDictionary *saved = nil;
    // 后台保存可能在中间抢先占用 revision；第二次尝试会像那次保存一样，在它写入的内容上合并。
    for (int attempt = 0; attempt < 2 && !saved; ++attempt) {
        NSDictionary *snapshot = [MSIMEClientSession loadPreferencesInDirectory:_preferencesDirectory error:nil];
        NSDictionary *preferences = snapshot ? MSIMEMergePreferenceSnapshot(snapshot[@"preferences"], overrides) : nil;
        if (!preferences) break;
        id revision = snapshot[@"revision"] ?: @0;
        saved = [MSIMEClientSession savePreferencesInDirectory:_preferencesDirectory expectedRevision:[revision unsignedLongLongValue]
                                                     snapshot:@{ @"format_version": @1, @"revision": revision, @"preferences": preferences } error:nil];
    }
    if (![saved isKindOfClass:NSDictionary.class]) { msime_macos_diagnostic_write("preferences_save_failed"); return; }
    // 走载入路径，这样 revision 会记为已应用，仍在进行的读取会被丢弃，而不是覆盖到它上面。
    _preferenceLoadState.reset();
    if (!_preferenceLoadState.begin()) return;
    [self completePreferenceLoad:saved error:nil generation:_preferenceLoadState.generation session:_session client:_activeClient];
}
- (void)persistAppearancePreferences {
    if (!_preferencesDirectory) return;
    if (!MSIMESharedPreferenceSaveState.request()) { MSIMEQueuedPreferenceSaver = self; return; }
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
            const bool again = MSIMESharedPreferenceSaveState.finish();
            MSIMEInputController *controller = weakSelf;
            if (!saved || saveError) msime_macos_diagnostic_write("preferences_save_failed");
            else {
                if (controller) [controller reloadPreferences];
                // 从 한 切到 中 是一次方案切换，只能通过这份文档到达 Engine。如果等持有焦点的控制器每秒一次的轮询，期间敲的键仍按韩文组字，正在组的音节又会把新方案挡到它结束，于是菜单栏已经显示 中，用户打出来的却还是韩文。
                MSIMEInputController *focused = MSIMEFocusedController;
                if (focused && focused != controller) [focused reloadPreferences];
            }
            if (again) {
                MSIMEInputController *next = MSIMEQueuedPreferenceSaver ?: controller;
                MSIMEQueuedPreferenceSaver = nil;
                [next persistAppearancePreferences];
            }
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
// The menus take the global theme's mode as the toolbar does (THEME_CONTRACT §6): a theme that fixes its own mode fixes theirs over menu_theme and the interface theme, which still decide for `system` and a custom theme over it. NSMenu has no palette of its own to recolour, so the mode is what a theme can reach.
- (NSDictionary *)resolvedMenuThemePreferences {
    NSDictionary *preferences = _menuThemePreferences ?: @{};
    if (!_appearance) return preferences;
    const auto fixed = [_appearance resolvedSkinForDark:NO].fixedDark;
    if (!fixed) return preferences;
    NSMutableDictionary *pinned = [preferences mutableCopy];
    pinned[@"menu_theme"] = *fixed ? @"dark" : @"light";
    return pinned;
}
- (NSMenu *)menu {
    [self ensureAppearance];
    NSMenu *menu = [[NSMenu alloc] initWithTitle:@"水杉输入法"];
    menu.autoenablesItems = NO;
    ApplyMetasequoiaMenuTheme(menu, [self resolvedMenuThemePreferences]);
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
    // Simplified output is the off state of this one toggle rather than a second row: a radio pair spent a row on the default.
    NSMenuItem *traditional = [[NSMenuItem alloc] initWithTitle:@"繁体输出" action:@selector(toggleTraditionalOutput:) keyEquivalent:@""];
    traditional.target = self;
    traditional.state = _appearance.traditionalOutput ? NSControlStateValueOn : NSControlStateValueOff;
    [menu addItem:traditional];
    // The three typing toggles the floating toolbar also carries, so they stay reachable with the toolbar hidden. The key equivalents are only labels for the chords handleEvent already claims (Ctrl+Shift+Space and Ctrl+.), not a second binding.
    NSMenuItem *fullWidth = [[NSMenuItem alloc] initWithTitle:@"全角字符" action:@selector(toggleFullWidthInput:) keyEquivalent:@" "];
    fullWidth.target = self;
    fullWidth.keyEquivalentModifierMask = NSEventModifierFlagControl | NSEventModifierFlagShift;
    fullWidth.state = _appearance.runtimeFullWidthInput ? NSControlStateValueOn : NSControlStateValueOff;
    [menu addItem:fullWidth];
    NSMenuItem *punctuation = [[NSMenuItem alloc] initWithTitle:@"中文标点" action:@selector(toggleChinesePunctuation:) keyEquivalent:@"."];
    punctuation.target = self;
    punctuation.keyEquivalentModifierMask = NSEventModifierFlagControl;
    punctuation.state = _appearance.runtimeChinesePunctuation ? NSControlStateValueOn : NSControlStateValueOff;
    // A punctuation lock pins the runtime state, so the toggle would do nothing; say so instead of offering it.
    punctuation.enabled = !([_appearance.punctuationLock isEqual:@"chinese"] || [_appearance.punctuationLock isEqual:@"english"]);
    [menu addItem:punctuation];
    NSMenuItem *translations = [[NSMenuItem alloc] initWithTitle:@"显示译文" action:@selector(toggleCandidateTranslations:) keyEquivalent:@""];
    translations.target = self;
    translations.state = _appearance.candidateTranslations ? NSControlStateValueOn : NSControlStateValueOff;
    [menu addItem:translations];
    [menu addItem:NSMenuItem.separatorItem];
    // The scheme and the theme are each one choice out of several, so each is a submenu whose row names the current one, the way the system lists an input source's modes. As a radio list under a header the scheme alone took six rows.
    NSString *profile = [NSString stringWithUTF8String:msime::mac::ShuangpinSchemaTitle(_appearance.shuangpinProfile.UTF8String ?: "")];
    if ([profile hasSuffix:@"双拼"] && profile.length > 2) profile = [profile substringToIndex:profile.length - 2];
    NSArray<NSString *> *schemes = MSIMEInputSchemeNames();
    NSArray<NSString *> *schemeTitles = @[@"全拼", [NSString stringWithFormat:@"双拼（%@）", profile], @"五笔 86", @"日语", @"韩语", @"粤拼", @"注音", @"越南语"];
    NSMenu *schemeMenu = [[NSMenu alloc] initWithTitle:@"输入方案"];
    schemeMenu.autoenablesItems = NO;
    NSString *currentSchemeTitle = nil;
    // A scheme that cannot run here (Cantonese or Zhuyin without its dictionary) is not offered, and the check is on the scheme actually running, so a preference naming one shows the scheme it fell back to.
    NSDictionary *hostOptions = [self inputSchemeHostOptions];
    NSString *effectiveScheme = MSIMEEffectiveInputScheme(_appearance.inputScheme, _appearance.lastChineseScheme, hostOptions);
    for (NSUInteger index = 0; index < schemes.count; ++index) {
        if (!MSIMEInputSchemeAvailable(schemes[index], hostOptions)) continue;
        NSMenuItem *item = [[NSMenuItem alloc] initWithTitle:schemeTitles[index] action:@selector(selectInputScheme:) keyEquivalent:@""];
        item.target = self;
        item.representedObject = schemes[index];
        item.state = [effectiveScheme isEqual:schemes[index]] ? NSControlStateValueOn : NSControlStateValueOff;
        if (item.state == NSControlStateValueOn) currentSchemeTitle = schemeTitles[index];
        [schemeMenu addItem:item];
    }
    NSMenuItem *scheme = [[NSMenuItem alloc] initWithTitle:currentSchemeTitle ? [NSString stringWithFormat:@"输入方案（%@）", currentSchemeTitle] : @"输入方案" action:nil keyEquivalent:@""];
    scheme.submenu = schemeMenu;
    [menu addItem:scheme];
    NSString *currentTheme = _appearance.globalTheme ?: @"system";
    NSMenu *themes = [[NSMenu alloc] initWithTitle:@"主题"];
    themes.autoenablesItems = NO;
    NSString *currentThemeTitle = nil;
    for (const auto &entry : msime::mac::ThemeCatalog()) {
        NSString *identifier = [NSString stringWithUTF8String:entry.id.c_str()];
        NSString *title = [NSString stringWithUTF8String:entry.title.c_str()];
        NSMenuItem *item = [[NSMenuItem alloc] initWithTitle:title action:@selector(selectGlobalTheme:) keyEquivalent:@""];
        item.target = self;
        item.representedObject = identifier;
        item.state = [currentTheme isEqual:identifier] ? NSControlStateValueOn : NSControlStateValueOff;
        if (item.state == NSControlStateValueOn) currentThemeTitle = title;
        [themes addItem:item];
    }
    NSMenuItem *theme = [[NSMenuItem alloc] initWithTitle:currentThemeTitle ? [NSString stringWithFormat:@"主题（%@）", currentThemeTitle] : @"主题" action:nil keyEquivalent:@""];
    theme.submenu = themes;
    [menu addItem:theme];
    [menu addItem:NSMenuItem.separatorItem];
    // The floating toolbar is one click from the language bar in the reference - the first item of its tray menu, with a tick showing the state. Here it could only be reached by opening the settings window and finding a checkbox, which is a long way round for something the user turns on and off while typing.
    NSMenuItem *toolbar = [[NSMenuItem alloc] initWithTitle:@"悬浮工具栏" action:@selector(toggleFloatingToolbar:) keyEquivalent:@""];
    toolbar.target = self;
    toolbar.state = _appearance.floatingToolbarEnabled ? NSControlStateValueOn : NSControlStateValueOff;
    [menu addItem:toolbar];

    // Keep the live input tools one click away. Account, update and support destinations stay in the settings window; listing those management pages here made this menu taller than the screen, but hiding the tools behind a second submenu made the useful part too hard to reach.
    NSMenuItem *emoji = [[NSMenuItem alloc] initWithTitle:@"水杉表情面板…" action:@selector(showEmoji:) keyEquivalent:@""];
    emoji.target = self;
    [menu addItem:emoji];
    // The cloud clipboard panel pastes into the editor like the emoji panel's clipboard page, so it sits beside it; the account, sync switch and history management stay in the settings window.
    NSMenuItem *cloudClipboard = [[NSMenuItem alloc] initWithTitle:@"云剪贴板…" action:@selector(showCloudClipboard:) keyEquivalent:@""];
    cloudClipboard.target = self;
    [menu addItem:cloudClipboard];
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
    // The reference tray menu ends with 关于, which opens the settings window on its about page. One row does not make the menu too tall, and without it the version and licence notices are only reachable by knowing to open settings and scroll to the last page.
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
        alert.messageText = @"账号窗口暂不可用";
        alert.informativeText = @"请重新启动输入法；若仍无法打开，请检查安装是否完整。";
        [alert runModal];
    });
}
- (void)showCloudClipboard:(id)sender {
    // Cloud history is never listed, let alone pasted, while a password field or a terminal's secure keyboard entry holds the keyboard.
    if ([self secureEventInputActive]) { NSBeep(); return; }
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
        //
        // A scheme that commits on blur (`commits_on_blur`: a Korean syllable, a Zhuyin or Vietnamese composition) is the exception: its composition is text the user already wrote, not candidates they are rejecting, so it is committed, as the scheme switches and the other hosts do.
        const BOOL writtenText = MSIMESchemeTrait(_view, msime::mac::scheme::CommitsOnBlur) && [_view[@"editing_text"] length];
        NSDictionary *cancelled = [_session command:writtenText ? MSIME_FINISH_COMPOSITION : MSIME_CANCEL error:nil];
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
        // The badge takes the floating toolbar's palette in each appearance, so it follows the selected theme and skin.
        const auto light = [_appearance toolbarSkinForDark:NO];
        const auto dark = [_appearance toolbarSkinForDark:YES];
        MSIMEInputModeHUDPanel *hud = [MSIMEInputModeHUDPanel sharedPanel];
        [hud setSurfaceColor:MSIMEThemedSkinColor(@"MSIMEInputModeHUDSurface", light.surface, dark.surface)
                 borderColor:MSIMEThemedSkinColor(@"MSIMEInputModeHUDBorder", light.border, dark.border)
                   textColor:MSIMEThemedSkinColor(@"MSIMEInputModeHUDText", light.text, dark.text)];
        [hud showEnglishInputMode:enabled nearCaretRect:caret];
    }
}
// The HostOptions document the scheme checks read: the session's, or the one on disk before a session exists. Test sessions stand in without hostOptions, as MSIMEStatisticsHostOptions allows for.
- (NSDictionary *)inputSchemeHostOptions {
    return [_session respondsToSelector:@selector(hostOptions)] ? _session.hostOptions : MSIMELoadRuntimeOptions();
}
// Turns an opt-in mode on in the system's input menu. Only an Engine session reaches TIS; a test's stand-in session reaches the enabler the test set, or nothing, so no test changes the developer's input menu. NO means nothing was asked, and the change is tried again on the next sync.
- (BOOL)enableOptInInputMode:(NSString *)identifier {
    if (_optInInputModeEnabler) {
        _optInInputModeEnabler(identifier);
        return YES;
    }
    if (![_session isKindOfClass:MSIMEClientSession.class]) return NO;
    MSIMEEnableInputMode(identifier, TISCreateInputSourceList, TISEnableInputSource);
    return YES;
}
// Keeps the selected input mode - 中, 双, 五, 粤, 注, 英, 日, 한 or 越 in the input menu - in step with the Chinese/English state and the scheme actually running. A switch the system reported is already recorded as shown, so this does not echo it back.
//
// Every scheme change reaches this, whether made from the input menu, either settings window, the Tauri settings or a mode the user picked, so this is also where an opt-in mode is turned on: when the scheme running moves to cantonese, zhuyin or vietnamese from the one last synced (lastSyncedInputScheme, kept across launches), its mode is enabled before it is selected. System Settings cannot add it, since its add dialog does not list a third-party input method's modes. The running scheme is the effective one, so a scheme picked before its dictionary was installed counts as picked once the dictionary arrives. A scheme that was already the synced one enables nothing, so a mode the user removed from the input menu is not added back at every launch, and the first sync ever only records the scheme.
- (void)syncSystemInputModeForClient:(id)client {
    NSString *scheme = MSIMEEffectiveInputScheme(_appearance.inputScheme, _appearance.lastChineseScheme, [self inputSchemeHostOptions]);
    NSString *synced = _appearance.lastSyncedInputScheme;
    if (![scheme isEqualToString:synced]) {
        NSString *optIn = MSIMEOptInInputModeToEnable(synced, scheme, YES);
        if (!optIn || [self enableOptInInputMode:optIn]) _appearance.lastSyncedInputScheme = scheme;
    }
    NSString *mode = MSIMEInputModeID(MSIMEInputModeFor(_appearance.englishMode, scheme));
    MSIMESelectSystemInputMode(MSIMESharedSystemInputModeState(), mode, client, MSIMEInputSourceIsEnabled);
}
// The system reports the mode the user picked from the input menu or reached with Ctrl+Space; the controller's Chinese/English state and, for every mode but 英, its scheme follow it. A report that only repeats the mode already shown, or one delivered from inside this controller's own selectInputMode:, leaves the state alone.
- (void)setValue:(id)value forTag:(long)tag client:(id)sender {
    if (tag == kTextServiceInputModePropertyTag) [self systemDidReportInputMode:value client:sender];
    [super setValue:value forTag:tag client:sender];
}
- (void)systemDidReportInputMode:(id)value client:(id)sender {
    // 切换属于将收到下一个按键的会话，也就是持有焦点的控制器的会话，不论系统通知的是哪个控制器。
    MSIMEInputController *focused = MSIMEFocusedController;
    if (focused && focused != self) { [focused systemDidReportInputMode:value client:sender]; return; }
    if (!MSIMEAdoptReportedInputMode(MSIMESharedSystemInputModeState(), value)) return;
    [self ensureAppearance];
    // The report can arrive before activateServer: or handleEvent: has named the client, and the mode is remembered per application.
    [_appearance activateInputModeForApplication:[sender respondsToSelector:@selector(bundleIdentifier)] ? [sender bundleIdentifier] : nil];
    // Moving from 英 to 日 changes two things, and each change syncs the menu bar on its own: between them it would select 中 or 英 again and the system would report that back as a new choice. Holding `selecting` keeps both quiet, and the sync below selects the one mode they add up to. 英 leaves the scheme alone, so returning to any other mode afterwards finds it where it was.
    MSIMESystemInputModeState &state = MSIMESharedSystemInputModeState();
    state.selecting = true;
    const MSIMEInputMode mode = MSIMEInputModeForID(value);
    NSString *scheme = _appearance.inputScheme;
    NSString *target =
        MSIMESchemeForReportedInputMode(mode, scheme, _appearance.lastChineseScheme, MSIMEInputSourceIsEnabled);
    if (target && ![target isEqualToString:scheme]) {
        // The composition was typed under the old scheme and a scheme switch discards it, so commit it first, as the scheme menu does. A Korean syllable is text the user already wrote.
        if (_session && _activeClient && [_view[@"editing_text"] length]) {
            NSDictionary *finished = [_session command:MSIME_FINISH_COMPOSITION error:nil];
            if (finished) [self apply:finished];
        }
        _appearance.inputScheme = target;
        [self applyPreferencesToSessionNow];
    }
    [self setEnglishInputMode:mode == MSIMEInputMode::English];
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
        // Ctrl+Shift+E is the reference's FUNCTION_CANCEL in both directions: _HandleCancel terminates the composition and commits nothing, so neither the highlighted Chinese candidate nor the English word being spelled reaches the document. A scheme that commits on blur (a Korean syllable, a Zhuyin or Vietnamese composition) is already text the user wrote, so it is committed instead. An Engine failure leaves the mode as it was.
        NSDictionary *cancelled = [_session command:MSIMESchemeTrait(_view, msime::mac::scheme::CommitsOnBlur) ? MSIME_FINISH_COMPOSITION : MSIME_CANCEL error:nil];
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
- (void)toggleTraditionalOutput:(id)sender { (void)sender; [self ensureAppearance]; _appearance.traditionalOutput = !_appearance.traditionalOutput; }
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
    // An installed model directory streams through the helper instead; "local" naming anything else never records (MSIMEVoiceLocalModelMissing).
    if ([self usesLocalModelVoice]) return NO;
    return MSIMEVoiceUsesNativeHTTPProvider(provider, MSIMEVoiceProviderSocket() != nil);
}
- (BOOL)usesNativeDoubaoVoice {
    NSString *provider = [NSUserDefaults.standardUserDefaults stringForKey:@"MSIMEClientVoiceASRProvider"] ?: @"doubao";
    return (!MSIMEVoiceProviderSocket() && [provider.lowercaseString isEqual:@"doubao"]) || [self usesLocalModelVoice];
}
- (void)dealloc {
    [[NSNotificationCenter defaultCenter] removeObserver:self];
    [NSDistributedNotificationCenter.defaultCenter removeObserver:self];
    [NSWorkspace.sharedWorkspace.notificationCenter removeObserver:self];
    // The toolbar outlives focus-outs, so a controller freed by IMK releases it here; the panel's owner check leaves a newer owner in place.
    [_toolbar deactivateForDelegate:self];
    if (_globalVoiceHotkeyMonitor) [NSEvent removeMonitor:_globalVoiceHotkeyMonitor];
    [_desktopInputSession stop]; [_httpVoiceRequest cancel]; [_doubaoVoiceRequest cancel];
    [_doubaoPolishRequest cancel]; [_livePolishRequest cancel];
    // A client that dies without deactivateServer: leaves the repeating timer on the run loop, and its candidate window on screen.
    [_preferencesTimer invalidate];
    [self flushKeyPresses];
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
                controller->_clientHasMarkedText = YES;
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
            _clientHasMarkedText = YES;
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
    if (MSIMEVoiceLocalModelMissing([voiceDefaults stringForKey:@"MSIMEClientVoiceASRProvider"], MSIMEVoiceProviderSocket() != nil,
                                    MSIMELocalVoiceModelDirectory([voiceDefaults stringForKey:@"MSIMEClientVoiceASRModelPath"]))) {
        [self reportVoiceFailure:MSIMEVoiceFailureMissingLocalModel];
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
        NSURL *state = MSIMEDefaultClientStateDirectory(NSFileManager.defaultManager);
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
    [_toolbar applyLightToolbarSkin:[_appearance toolbarSkinForDark:NO]
                            darkSkin:[_appearance toolbarSkinForDark:YES]];
    [_toolbar activateForDelegate:self visible:_appearance.floatingToolbarEnabled];
    _activeClient = sender;
    MSIMEFocusedController = self;
    _preferenceLoadState.reset();
    [[NSNotificationCenter defaultCenter] removeObserver:self name:MSIMEClientSessionDidReplaceSnapshotNotification object:nil];
    [[NSNotificationCenter defaultCenter] addObserver:self selector:@selector(snapshotSessionReplaced:) name:MSIMEClientSessionDidReplaceSnapshotNotification object:nil];
    MSIMESetBackendSelectionObservation([NSNotificationCenter defaultCenter], self, @selector(handwritingCandidateSelected:), YES);
    [self refreshFloatingToolbarState];
    [self ensureAppearance];
    _focusPending = _appearance.englishMode;
    if (!_appearance.englishMode) [self prepareSession];
    else [self startPreferencesMonitoring];
    [self claimBackgroundMusic];
    NSNotificationCenter *workspaceCenter = NSWorkspace.sharedWorkspace.notificationCenter;
    [workspaceCenter removeObserver:self name:NSWorkspaceActiveSpaceDidChangeNotification object:nil];
    [workspaceCenter addObserver:self selector:@selector(typingEffectSpaceChanged:) name:NSWorkspaceActiveSpaceDidChangeNotification object:nil];
    [self refreshTypingEffectFullscreen];
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
    [alert.window center];
    MSIMEPresentWindow(alert.window);
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
//
// soundPacks is the bundle's built-in sound-pack directory, or nil when the bundle has none. The host library otherwise looks for it beside `resources`, and here that is EngineResources in Application Support, not in the bundle.
static NSDictionary *MSIMESessionOptions(NSDictionary *runtimeOptions, NSString *soundPacks) {
    if (![runtimeOptions isKindOfClass:NSDictionary.class]) return nil;
    NSMutableDictionary *requested = [runtimeOptions mutableCopy];
    // This host draws view.phrase_prefix, so a phrase being assembled out of several selections
    // stays in the composition instead of arriving in the document one piece at a time.
    requested[@"phrase_preedit"] = @YES;
    // An options file that names a directory itself keeps it.
    if (soundPacks.length && !requested[@"sound_packs"]) requested[@"sound_packs"] = soundPacks;
    return requested;
}

// Resources/sound-packs of this bundle, where CMakeLists.txt stages the built-in packs; nil when it is not there.
static NSString *MSIMEBundleSoundPacks(NSBundle *bundle) {
    NSString *directory = [bundle.resourcePath stringByAppendingPathComponent:@"sound-packs"];
    BOOL isDirectory = NO;
    return directory.isAbsolutePath && [NSFileManager.defaultManager fileExistsAtPath:directory isDirectory:&isDirectory] && isDirectory
        ? directory : nil;
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
    // The device's anonymous MSIME account is registered the first time the input method activates, so a new install has one before any feature asks for it. It runs once per process and returns at once when a signed-in or anonymous session is already saved; only the random identity is sent, never input.
    if (MSIMEEnsureAnonymousAccount != nullptr) MSIMEEnsureAnonymousAccount();
    BOOL reopened = NO;
    if (!_session) {
        NSDictionary *options = MSIMESessionOptions([self runtimeOptions], MSIMEBundleSoundPacks(NSBundle.mainBundle));
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
        [self refreshTypingEffectSettings];
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
        // After the preferences, which the session's resolved effect refines with the selected pack.
        [self refreshTypingEffectSettings];
        _view = [session viewWithError:nil] ?: result[@"view"];
        if (_view) {
            MSIMEApplyTransitionTrackingMarkedText(@{@"view": _view}, (id<MSIMETextClient>)_activeClient,
                                                   _appearance.inlinePreeditStyle, nil, &_clientHasMarkedText);
        }
        [self refreshFloatingToolbarState];
        if (MSIMEMusicOwner == self) [self claimBackgroundMusic];
        // Another surface - the shared settings page, an account push - can have changed the scheme.
        [self syncSystemInputModeForClient:_activeClient];
        [self renderCandidates];
        [self synchronizeCandidateServices];
    } else {
        msime_macos_diagnostic_write("preferences_apply_failed");
    }
}

- (void)reloadPreferences {
    if (!_activeClient || !_preferencesDirectory || MSIMESharedPreferenceSaveState.saving || !_preferenceLoadState.begin()) return;
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
    // The global theme arrives with the rest of the document, so the toolbar takes the palette resolved from it here as well as on activation. A theme with a mode of its own fixes the toolbar's mode as it fixes the candidate window's.
    NSDictionary *toolbarThemePreferences = preferences;
    if (const auto fixed = [_appearance resolvedSkinForDark:NO].fixedDark) {
        NSMutableDictionary *pinned = [preferences mutableCopy];
        pinned[@"toolbar_theme"] = *fixed ? @"dark" : @"light";
        toolbarThemePreferences = pinned;
    }
    [_toolbar applyLightSkin:[_appearance resolvedSkinForDark:NO].tokens darkSkin:[_appearance resolvedSkinForDark:YES].tokens];
    [_toolbar applyLightToolbarSkin:[_appearance toolbarSkinForDark:NO]
                            darkSkin:[_appearance toolbarSkinForDark:YES]];
    [_toolbar applyThemePreferences:toolbarThemePreferences];
    // The mode badge wears the toolbar's palette, so it is drawn in the toolbar's mode too; it is updated whether or not the toolbar is shown.
    [[MSIMEInputModeHUDPanel sharedPanel] applyThemePreferences:toolbarThemePreferences];
    [_toolbar applySizingPreferences:preferences];
    // And it is the toolbar's size, from the same font size and scale.
    [[MSIMEInputModeHUDPanel sharedPanel] applySizingPreferences:preferences];
    // Typing effects follow preferences.plugins; a partial document without it leaves them as they are.
    [[MSIMETypingEffectPanel sharedPanel] applyPreferences:preferences];
    // Effects switched on mid-session were never checked against the foreground application at activation.
    if (_activeClient) [self refreshTypingEffectFullscreen];
    NSDictionary *toolbar = preferences[@"floating_toolbar"];
    id enabled = [toolbar isKindOfClass:NSDictionary.class] ? toolbar[@"enabled"] : nil;
    if ([enabled isKindOfClass:NSNumber.class]) {
        [_appearance applySharedToolbarVisibility:[enabled boolValue]];
        [_toolbar setVisible:_appearance.floatingToolbarEnabled forDelegate:self];
    }
}

- (void)deactivateServer:(id)sender {
    [[MSIMEInputModeHUDPanel sharedPanel] orderOut:nil];
    [[MSIMETypingEffectPanel sharedPanel] settle];
    // Every focus loss writes the key heatmap counts, including a late one for a previous client: they are this controller's presses either way.
    [self flushKeyPresses];
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
    [self releaseBackgroundMusic];
    if (_session) [self apply:[_session setFocused:NO error:nil]];
    [self resetCandidateAnchor];
    [self hideCandidatePanel:"focus_out"];
    _activeClient = nil;
    if (MSIMEFocusedController == self) MSIMEFocusedController = nil;
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
// The input menu's toggles run the same paths as the floating toolbar and the chords, so all three stay one state.
- (void)toggleFullWidthInput:(id)sender { (void)sender; [self toggleRuntimeFullWidthInput]; }
- (void)toggleChinesePunctuation:(id)sender { (void)sender; [self floatingToolbarDidRequestTogglePunctuation:nil]; }
- (void)toggleCandidateTranslations:(id)sender {
    (void)sender;
    [self ensureAppearance];
    _appearance.candidateTranslations = !_appearance.candidateTranslations;
}
- (void)selectInputScheme:(id)sender {
    MSIMEInputController *focused = MSIMEFocusedController;
    if (focused && focused != self) { [focused selectInputScheme:sender]; return; }
    [self ensureAppearance];
    NSString *scheme = [sender respondsToSelector:@selector(representedObject)] ? [sender representedObject] : nil;
    if (![MSIMEInputSchemeNames() containsObject:scheme] || [_appearance.inputScheme isEqual:scheme]) return;
    // The composition was typed under the old scheme; commit it rather than reinterpret its keys.
    if (_session && _activeClient && [_view[@"editing_text"] length]) {
        NSDictionary *finished = [_session command:MSIME_FINISH_COMPOSITION error:nil];
        if (!finished) return;
        [self apply:finished];
    }
    _appearance.inputScheme = scheme;
    [self applyPreferencesToSessionNow];
}
- (void)selectGlobalTheme:(id)sender {
    [self ensureAppearance];
    NSString *theme = [sender respondsToSelector:@selector(representedObject)] ? [sender representedObject] : nil;
    if ([theme isKindOfClass:NSString.class]) _appearance.globalTheme = theme;
}
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
            if (!launched) {
                NSBeep();
                return;
            }
            [self flushKeyPressesWaitingUntilWritten:YES];
            [NSApp terminate:nil];
        });
}

- (void)terminateCurrentInputMethod {
    [self flushKeyPressesWaitingUntilWritten:YES];
    [NSApp terminate:nil];
}

// Whether word-to-character is enabled and bound to the pair this event belongs to. Shared by the paging
// exclusion and the branch that acts on it so the two can never disagree about who owns the key.
- (BOOL)wordCharacterClaimsEvent:(NSEvent *)event {
    NSDictionary *wordCharacter = [_appearance wordCharacterOptions];
    if (![wordCharacter[@"enabled"] boolValue]) return NO;
    // A scheme whose letters build the written text directly (a Korean syllable or Hanja, a Vietnamese word) has no word to take a character from: the pair is punctuation there (see candidateListMark in handleEvent:client:).
    if (MSIMESchemeTrait(_view, msime::mac::scheme::LetterComposition)) return NO;
    NSString *characters = event.charactersIgnoringModifiers;
    if (characters.length != 1) return NO;
    const unichar character = [characters characterAtIndex:0];
    // A key the Engine spells with, such as expression mode's '-', is input rather than an edge pick.
    if (MSIMESpellingSymbol(_view, character)) return NO;
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
//
// A Korean Hanja list always keeps a line for the 훈음, which every Hanja row draws on its gloss line whatever the translation switches say, with the translation lines, when they are on, under it.
- (CGFloat)reservedGlossHeightForFont:(NSFont *)glossFont {
    if (_appearance.vertical) return 0;
    const NSUInteger readingLines = MSIMEKoreanHanjaListOpen(_view) ? 1 : 0;
    const BOOL translations = (_appearance.candidateTranslations || _appearance.candidateEnglishGloss) &&
        !(_glossEnabled && !_glossEnabled.boolValue && !_appearance.candidateEnglishGloss);
    const NSUInteger lines = readingLines + (translations ? MIN(MAX(_glossTargetLanguages.count, (NSUInteger)1), (NSUInteger)2) : 0);
    if (lines == 0) return 0;
    NSMutableArray<NSString *> *placeholder = [NSMutableArray arrayWithCapacity:lines];
    for (NSUInteger line = 0; line < lines; ++line) [placeholder addObject:@"X"];
    return MSIMETranslationTextSize([placeholder componentsJoinedByString:@"\n"], glossFont).height + MSIMECandidateGlossPadding * MSIMECandidateScale(_appearance);
}

// Background music plays while one controller of this process is the active input method. IMK does not promise that the previous client's deactivateServer: comes before the next one's activateServer:, so only the controller that last let music play may stop it.
static __weak MSIMEInputController *MSIMEMusicOwner;

// Secure event input is on while a password field, or a terminal's secure keyboard entry, has the keyboard. It is window-server state shared by every process, so an application that leaves it on also silences this one; that errs the right way, because a click per keystroke tells anyone listening how long a password is.
- (BOOL)secureEventInputActive { return IsSecureEventInputEnabled(); }

// Become the controller music follows: it may play unless secure event input is on. Also called after a preference update, because music switched on while nothing else was sounding only starts once the player is told the input method is active.
- (void)claimBackgroundMusic {
    _secureEventInput = [self secureEventInputActive];
    MSIMEMusicOwner = self;
    [_session setMusicActive:!_secureEventInput];
}

- (void)releaseBackgroundMusic {
    if (MSIMEMusicOwner != self) return;
    MSIMEMusicOwner = nil;
    [_session setMusicActive:NO];
}

// Every key press this input method is given makes its key sound, handled or passed on to the application, except auto-repeat (a held key is one press), a Command or Control shortcut (silent on Linux and Windows too; Option stays audible, it types characters on macOS), anything typed while secure event input is on, and English mode, which is silent on every desktop host (Windows only hears the keys it composes). The session only queues the request, so this costs the key nothing when sound is off.
- (void)playKeySound:(NSEvent *)event {
    const BOOL secure = [self secureEventInputActive];
    if (secure != _secureEventInput) {
        _secureEventInput = secure;
        if (MSIMEMusicOwner == self) [_session setMusicActive:!secure];
    }
    const BOOL shortcut = (event.modifierFlags & (NSEventModifierFlagCommand | NSEventModifierFlagControl)) != 0;
    if (secure || shortcut || _appearance.englishMode) return;
    const uint32_t keyClass = msime::mac::PhysicalKeySoundClass(event.keyCode);
    [_session keySound:keyClass];
    // The typing effect counts the same keys the key sound plays for.
    [self typingEffect:keyClass commit:NO];
}

// The foreground application's full-screen state, read on the main queue's next turn rather than on a key: it walks the on-screen window list. The previous answer stands until then.
- (void)refreshTypingEffectFullscreen {
    if (!MSIMETypingEffectPanel.sharedPanel.configured) {
        _typingEffectFullscreen = NO;
        return;
    }
    _typingEffectFullscreenCheckedAt = NSProcessInfo.processInfo.systemUptime;
    __weak MSIMEInputController *weakSelf = self;
    dispatch_async(dispatch_get_main_queue(), ^{
        MSIMEInputController *strongSelf = weakSelf;
        if (strongSelf && strongSelf->_activeClient) strongSelf->_typingEffectFullscreen = MetasequoiaFrontmostApplicationOwnsFullscreenDisplay();
    });
}

// Entering or leaving native full screen moves the foreground application to another space.
// The session's resolved typing effect for the panel: read after a focus-in and after a preference update, never on a key, since it may read the effect pack's manifest.
- (void)refreshTypingEffectSettings {
    [[MSIMETypingEffectPanel sharedPanel] applySettings:_session ? [_session typingEffectSettingsWithError:nil] : nil];
}

- (void)typingEffectSpaceChanged:(NSNotification *)notification {
    (void)notification;
    if (_activeClient) [self refreshTypingEffectFullscreen];
}

// One key or commit for the typing effect. The library call is integer arithmetic on the session; nothing is drawn here. A full-screen foreground application keeps the tier-up sound quiet and gets nothing drawn over it, while the combo still counts.
- (void)typingEffect:(uint32_t)event commit:(BOOL)commit {
    if (!_session || !MSIMETypingEffectPanel.sharedPanel.configured) return;
    if (NSProcessInfo.processInfo.systemUptime - _typingEffectFullscreenCheckedAt >= 1.0) [self refreshTypingEffectFullscreen];
    const uint32_t packed = [_session typingEffect:event | (_typingEffectFullscreen ? MSIMETypingEffectEventMuted : 0)];
    if (_typingEffectFullscreen) return;
    // A tier reached by an earlier key still undrawn is kept, so its bounce is not lost to the key after it.
    _typingEffectPacked = packed | (_typingEffectScheduled ? (_typingEffectPacked & MSIMETypingEffectTierUp) : 0);
    _typingEffectCommit = commit || (_typingEffectScheduled && _typingEffectCommit);
    if (_typingEffectScheduled) return;
    _typingEffectScheduled = YES;
    __weak MSIMEInputController *weakSelf = self;
    dispatch_async(dispatch_get_main_queue(), ^{ [weakSelf presentTypingEffect]; });
}

- (void)presentTypingEffect {
    const uint32_t packed = _typingEffectPacked;
    const BOOL commit = _typingEffectCommit;
    _typingEffectScheduled = NO;
    _typingEffectPacked = 0;
    _typingEffectCommit = NO;
    if (!_activeClient || _secureEventInput) return;
    MSIMETypingEffectPanel *panel = MSIMETypingEffectPanel.sharedPanel;
    const MSIMETypingEffect effect = MSIMETypingEffectDecode(packed);
    // Nothing to draw, as after a backspace with only the counter on: the panel only takes a stale badge down, so the client is not asked for its caret.
    if (effect.style == MSIMETypingEffectStyleOff && MSIMETypingEffectComboText(effect.combo) == nil) {
        [panel presentEffect:packed commit:commit caretRect:NSZeroRect candidateView:nil cardRect:NSZeroRect cornerRadius:0];
        return;
    }
    NSRect caret = NSZeroRect;
    [(id<IMKTextInput>)_activeClient attributesForCharacterIndex:0 lineHeightRectangle:&caret];
    if (!MSIMEValidCaret(caret)) caret = NSZeroRect;
    MSIMECandidateChromeView *card = _panel.isVisible && [_panel.contentView isKindOfClass:MSIMECandidateChromeView.class]
        ? (MSIMECandidateChromeView *)_panel.contentView
        : nil;
    const NSRect bounds = card.bounds;
    const NSRect cardRect = card ? NSMakeRect(NSMinX(bounds), NSMinY(bounds), NSWidth(bounds), MAX(0.0, NSHeight(bounds) - MAX(0.0, card.cardTopInset))) : NSZeroRect;
    [panel presentEffect:packed commit:commit caretRect:caret candidateView:card cardRect:cardRect cornerRadius:card.cornerRadius];
}

// Every key down leaves through here, so the smart punctuation shadow sees each one exactly once, after it has been handled and with what became of it. Events this host posted itself (the voice sendinput route and the smart punctuation rewrite) are skipped: whoever posted them has already recorded what they carry.
- (BOOL)handleEvent:(NSEvent *)event client:(id)sender {
    const bool timed = msime_macos_diagnostic_enabled();
    const uint64_t started = timed ? clock_gettime_nsec_np(CLOCK_UPTIME_RAW) : 0;
    const NSEventType type = event.type;
    const char *label = type == NSEventTypeKeyDown ? "down" : type == NSEventTypeKeyUp ? "up" : type == NSEventTypeFlagsChanged ? "flags" : "other";
    CGEventRef nativeEvent = event.CGEvent;
    const BOOL selfPosted = nativeEvent && CGEventGetIntegerValueField(nativeEvent, kCGEventSourceUserData) == MSIMEVoiceCommitEventTag;
    if (timed && event.timestamp > 0 && !selfPosted) {
        // NSEvent.timestamp and systemUptime share the uptime clock, so this is how long the key waited before reaching us. It includes WindowServer and IMK delivery as well as time queued behind work on our main thread, such as a redraw, so it is an upper bound on our own queueing, unlike Windows' stage=queue which starts at the server's own enqueue.
        const double queueMs = (NSProcessInfo.processInfo.systemUptime - event.timestamp) * 1000.0;
        if (queueMs >= 8.0) msime_macos_diagnostic_writef("[key-latency] stage=queue type=%s elapsed_ms=%.3f", label, queueMs);
    }
    _smartPunctuationShadowWritten = NO;
    if (event.type == NSEventTypeKeyDown && sender) {
        [self ensureAppearance];
        if (_appearance.floatingToolbarEnabled)
            [[MSIMEFloatingToolbarPanel sharedPanel] wakeForInputDelegate:self];
    }
    if (event.type == NSEventTypeKeyDown && sender && !selfPosted && !event.isARepeat) [self playKeySound:event];
    const BOOL handled = [self handleKeyEvent:event client:sender];
    if (event.type == NSEventTypeKeyDown && sender && !selfPosted) {
        // A key event is the wake-up edge: restore the toolbar before the next event arrives, even if
        // a preceding focus or preference callback left its requested visibility stale.
        if (_appearance.floatingToolbarEnabled) {
            _toolbar = [MSIMEFloatingToolbarPanel sharedPanel];
            [_toolbar wakeForInputDelegate:self];
        }
        [self noteKeyForSmartPunctuationShadow:event eaten:handled];
    }
    if (!selfPosted) [self recordKeyPress:event client:sender];
    if (!handled) [self recordPassthroughKey:event client:sender];
    const double elapsedMs = timed ? static_cast<double>(clock_gettime_nsec_np(CLOCK_UPTIME_RAW) - started) / 1e6 : 0;
    if (timed && elapsedMs >= 8.0) {
        msime_macos_diagnostic_writef("[key-latency] stage=handle type=%s handled=%d elapsed_ms=%.3f", label, handled ? 1 : 0, elapsedMs);
    }
    return handled;
}

- (void)recordPassthroughKey:(NSEvent *)event client:(id)sender {
    if (!MSIMETypingStatisticsEnabled.load(std::memory_order_relaxed)) return;
    if (event.type != NSEventTypeKeyDown || !sender) return;
    // A password field turns on secure event input; what is typed there is never counted.
    if (IsSecureEventInputEnabled()) return;
    CGEventRef nativeEvent = event.CGEvent;
    if (nativeEvent && CGEventGetIntegerValueField(nativeEvent, kCGEventSourceUserData) == MSIMEVoiceCommitEventTag) return;
    NSString *characters = event.characters;
    if (characters.length != 1) return;
    if (!msime::mac::ShouldCountPassthroughCharacter([characters characterAtIndex:0], (event.modifierFlags & NSEventModifierFlagControl) != 0, (event.modifierFlags & NSEventModifierFlagCommand) != 0)) return;
    const msime::mac::TypingSource source = _appearance.englishMode ? msime::mac::TypingSource::English : MSIMEResolveTypingSource(_view, _view, MSIMEStatisticsHostOptions(_session), NO);
    MSIMERecordTypingStatistics(_preferencesDirectory ?: MSIMEStatisticsHostOptions(_session)[@"preferences_directory"], characters, source);
}

// Counts one physical key press for the key heatmap, whether the input method consumes the key or hands it to the application. Only each key's press count per local day is kept, never the order or what was typed. A held key counts once, as does a modifier, which reports both edges as FlagsChanged. Nothing is collected while statistics are off or while a password field holds secure event input.
- (void)recordKeyPress:(NSEvent *)event client:(id)sender {
    if (!MSIMETypingStatisticsEnabled.load(std::memory_order_relaxed)) {
        _keyPressBatch.clear();
        return;
    }
    if (!sender) return;
    const unsigned short keyCode = event.keyCode;
    if (event.type == NSEventTypeKeyDown) {
        if (event.isARepeat) return;
    } else if (event.type != NSEventTypeFlagsChanged || !msime::mac::IsModifierPress(keyCode, event.modifierFlags)) {
        return;
    }
    if (IsSecureEventInputEnabled()) return;
    const std::string_view keyId = msime::mac::KeyIdForVirtualKeyCode(keyCode);
    if (keyId.empty()) return;
    // The day is taken at the press, so counts collected before midnight are written under the day they belong to.
    for (msime::mac::KeyPressFlush &flush : _keyPressBatch.record(MSIMETypingStatisticsLocalDay().UTF8String, keyId))
        MSIMERecordKeyPresses([self keyPressStatisticsDirectory], std::move(flush), false);
    if (_keyPressBatch.empty()) {
        [_keyPressFlushTimer invalidate];
        _keyPressFlushTimer = nil;
    } else if (!_keyPressFlushTimer) {
        __weak MSIMEInputController *weakSelf = self;
        _keyPressFlushTimer = [NSTimer timerWithTimeInterval:30 repeats:NO block:^(NSTimer *) {
            MSIMEInputController *controller = weakSelf;
            if (!controller) return;
            controller->_keyPressFlushTimer = nil;
            [controller flushKeyPresses];
        }];
        [NSRunLoop.mainRunLoop addTimer:_keyPressFlushTimer forMode:NSRunLoopCommonModes];
    }
}

- (NSString *)keyPressStatisticsDirectory {
    return _preferencesDirectory ?: MSIMEStatisticsHostOptions(_session)[@"preferences_directory"];
}

- (void)flushKeyPresses {
    [self flushKeyPressesWaitingUntilWritten:NO];
}

// Exit runs no queued blocks and never reaches dealloc, so the paths that terminate the process write the pending counts first and wait for them; blocking a few milliseconds is acceptable there.
- (void)flushKeyPressesWaitingUntilWritten:(BOOL)wait {
    [_keyPressFlushTimer invalidate];
    _keyPressFlushTimer = nil;
    if (std::optional<msime::mac::KeyPressFlush> flush = _keyPressBatch.drain())
        MSIMERecordKeyPresses([self keyPressStatisticsDirectory], std::move(*flush), wait);
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
        MSIMEFocusedController = self;
        // Whatever the previous client was last given says nothing about this one.
        [self invalidateSmartPunctuationShadow];
        [_appearance activateInputModeForApplication:[sender respondsToSelector:@selector(bundleIdentifier)] ? [sender bundleIdentifier] : nil];
        [self syncSystemInputModeForClient:sender];
        // Punctuation and width are per app, so the new client's values reach the Engine before it types.
        [self syncPunctuation];
        [self syncCharacterWidth];
        [self refreshFloatingToolbarState];
        _focusPending = _appearance.englishMode;
        if (!_appearance.englishMode) {
            [self apply:[_session setFocused:YES error:nil]];
            [self refreshTypingEffectSettings];
        }
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
    // Korean letters take their case from Shift alone (KoreanKeyLetter) and Vietnamese composes an uppercase letter as written, so Caps Lock does not hand them to the application either.
    if (!MSIMESchemeTrait(_view, msime::mac::scheme::CapsLockBypassExempt) && MSIMECapsLockFreshUppercaseBypass(event, _view)) return NO;
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
    const BOOL digitIsSpelling = MSIMESpellingSymbol(_view, (unichar)msime::mac::PhysicalCandidateDigitCharacter(physicalDigit));
    if (msime::mac::ShouldRoutePhysicalCandidateDigit(
            _panel.isVisible, [_view[@"nine_key"] boolValue], digitIsSpelling,
            (event.modifierFlags & candidateDigitModifiers) != 0) ||
        msime::mac::ShouldRouteSpellingShiftCandidateDigit(
            _panel.isVisible, digitIsSpelling,
            (event.modifierFlags & candidateDigitModifiers) == NSEventModifierFlagShift,
            MSIMESpellingSymbolString(_view, event.characters))) {
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
    // A keypad mark the view lists among its spelling symbols (Zhuyin's bopomofo keys, an expression operator) is input like the keypad digits and goes on as a character.
    const char keypadPunctuation = msime::mac::KeypadPunctuation(event.keyCode);
    if (keypadPunctuation && !MSIMESpellingSymbol(_view, (unichar)keypadPunctuation) &&
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
    // Ctrl+Backspace deletes a segmentation unit and Ctrl+Left / Ctrl+Right move the caret by one, which is what the reference's composition editor does (`IsSegmentBackspaceKey` and `IsSegmentCaretKey` in its input_key_policy.h) and what both Linux front ends and the Windows host already route. It has to be decided before the rule below, which hands every Ctrl, Option and Command chord back to the application after finishing the composition - that rule is what left this host without segment editing.
    //
    // Only the bare Ctrl chord is the input method's: with Shift, Option or Command also held, or with nothing being composed, the key stays the application's. A scheme that locks the caret (`locks_caret`: Korean, Zhuyin, Vietnamese) has no segments, so there the chord finishes the composition below and the application edits by word as usual.
    if ((event.modifierFlags & (NSEventModifierFlagControl | NSEventModifierFlagShift |
                                NSEventModifierFlagOption | NSEventModifierFlagCommand)) ==
            NSEventModifierFlagControl &&
        !MSIMESchemeTrait(_view, msime::mac::scheme::LocksCaret) && _session && _activeClient && ([_view[@"editing_text"] length] || [_view[@"candidates"] count] ||
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
    // Option+Return opens the scheme's candidate list (the Hanja of the composing Korean syllable, the Zhuyin list), the key the system's Korean input method uses, and closes the list again while it is open. It is decided before the rule below, which would finish the syllable and hand the chord to the application as a line break. While a syllable composes the chord stays the input method's whatever the Engine answers: a lone jamo has no Hanja, and passing the chord on would write the jamo out and break the line. With nothing composing it is the application's as before. A repeat is swallowed so that holding the chord does not flicker the list open and shut.
    if (MSIMECandidateListTrigger(event, _view)) {
        if (event.isARepeat) return YES;
        NSDictionary *transition = [_session command:MSIME_OPEN_CANDIDATE_LIST error:nil];
        if (transition) [self apply:transition];
        return YES;
    }
    // Down opens a closed Zhuyin list (MSIMECandidateListDownKey). It is decided before the arrow navigation below, which with its binding off would finish the conversion and hand the key to the application.
    if (MSIMECandidateListDownKey(event, _view)) {
        NSDictionary *transition = [_session command:MSIME_OPEN_CANDIDATE_LIST error:nil];
        if (transition) [self apply:transition];
        return YES;
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
    // text at all). Unicode '+' is an Engine code-sequence character, the
    // Japanese minus/equal keys remain composition input, and so does any key
    // the Engine spells with (expression mode's '-' and '.').
    const int physicalPageDirection = msime::mac::PhysicalCandidatePageDirection(event.keyCode);
    const BOOL japaneseMinusEqual = msime::mac::IsJapaneseMinusEqualKey(
        [_view[@"scheme"] intValue], [_view[@"local_mode"] isEqual:@"temporary_japanese"],
        event.keyCode, 0);
    const BOOL engineInputKey = ([_view[@"local_mode"] isEqual:@"unicode"] &&
        [event.charactersIgnoringModifiers isEqual:@"+"]) || MSIMESpellingSymbolString(_view, event.charactersIgnoringModifiers);
    // Word-to-character owns whichever pair it is bound to, and paging does not get to take it. The two are
    // alternatives, which applyCloudSettingsSnapshot: already says by refusing a snapshot whose paging
    // preset collides - but that only guards the cloud path, so a locally enabled bracket or minus paging
    // shortcut quietly won here and left 以词定字 doing nothing at all, with nothing to say why.
    //
    // The test is the one the word-to-character branch below applies, so the key is excluded from paging
    // exactly when that branch will claim it: whichever way it goes, the keystroke has an owner.
    const BOOL wordCharacterOwnsKey = [self wordCharacterClaimsEvent:event];
    // The marks among the paging keys (- = [ ] , .) are punctuation while an opened candidate list (the Korean Hanja list) shows, as they are with no list: the Engine closes it and writes the composition with the mark, so a mark typed after a syllable never turns a page instead. Page Up and Page Down still page.
    const BOOL candidateListMark = MSIMECandidateListOpen(_view) && event.keyCode != 116 && event.keyCode != 121;
    if (_panel.isVisible && !(event.modifierFlags & NSEventModifierFlagShift) &&
        physicalPageDirection != 0 && !japaneseMinusEqual && !engineInputKey && !wordCharacterOwnsKey && !candidateListMark) {
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
        // An opened candidate list (the Korean Hanja list) belongs to a composition with no caret inside it, so the caret move would only write the composition out and leave the key to the application. Across a vertical list Left and Right turn the page instead.
        if (MSIMECandidateListOpen(_view)) {
            [self apply:[_session command:event.keyCode == 123 ? MSIME_PREVIOUS_PAGE : MSIME_NEXT_PAGE error:nil]];
            return YES;
        }
    }
    // A scheme that spells with Space (Zhuyin's first tone while a syllable is pending) gets it as a character: the key is input, not the selection of a highlighted item.
    const BOOL spaceIsSpelling = event.keyCode == 49 && MSIMESpellingSymbol(_view, ' ');
    if (_panel.isVisible && event.keyCode == 49 && !spaceIsSpelling &&
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
        !MSIMESpellingSymbolString(_view, event.characters) &&
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
    // A scheme outside the Chinese punctuation table (`uses_chinese_punctuation`: Korean, Vietnamese) ignores the Chinese punctuation switch, so its `{` is the Engine's plain ASCII mark. Zhuyin's Shift overlay turns `{` into 『 in the Engine whatever the switches say, so its `{` is the Engine's as well.
    if ([event.characters isEqualToString:@"{"] &&
        !(MSIMESchemeRulesApply(_view) && (!msime::mac::scheme::UsesChinesePunctuation(MSIMEViewScheme(_view)) ||
                                           MSIMEViewScheme(_view) == msime::mac::scheme::Zhuyin)) &&
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
    // With its navigation binding off, a paging or arrow key is the application's and moves the caret, so it ends a composition that commits on blur (a Korean syllable, a Zhuyin or Vietnamese composition) the way Tab does below. A candidate list on screen keeps the key instead, as every candidate list does (the switch below swallows it), so the composition is not written out.
    const BOOL applicationNavigationKey = ((event.keyCode == 116 || event.keyCode == 121) && ![_appearance navigationEnabled:@"page_up_down"]) ||
        ((event.keyCode == 125 || event.keyCode == 126) && ![_appearance navigationEnabled:@"arrows"]);
    if (MSIMESchemeTrait(_view, msime::mac::scheme::CommitsOnBlur) && applicationNavigationKey && !_panel.isVisible && [_view[@"editing_text"] length]) [self apply:[_session command:MSIME_FINISH_COMPOSITION error:nil]];
    switch (event.keyCode) {
        case 48:
            // With no candidate panel on screen (the caret rect was invalid or there is no screen to show it on) Tab and Shift+Tab are the application's, whatever navigation.tab says. A composition still being edited is finished first, for every scheme and not only a Korean syllable, so the key does not move focus away from marked text that would then dangle in the old field. With nothing composed the session is left alone.
            if ([_view[@"editing_text"] length] || [_view[@"candidates"] count] || [_view[@"phrase_prefix"] length])
                [self apply:[_session command:MSIME_FINISH_COMPOSITION error:nil]];
            return NO;
        case 51: command = MSIME_BACKSPACE; break;
        // With an opened candidate list (the Korean Hanja list) showing, Return chooses the highlighted Hanja, as Space does; only the session knows the highlight, so the command is the candidate one (msime_client.h). Otherwise Return writes the syllable out and breaks the line.
        case 36: case 76: command = MSIMECandidateListOpen(_view) ? MSIME_COMMIT_CANDIDATE : MSIME_COMMIT_RAW; break;
        case 53: [self flushPendingPairedClosing]; _pairedPunctuation.clear(); command = MSIME_CANCEL; break;
        case 49: if (!spaceIsSpelling) command = MSIME_COMMIT_CANDIDATE; break;
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
        const BOOL shift = (event.modifierFlags & NSEventModifierFlagShift) != 0;
        unichar typed = [event.characters characterAtIndex:0];
        if (MSIMESchemeTrait(_view, msime::mac::scheme::FoldsLetterCase)) typed = (unichar)msime::mac::KoreanKeyLetter((char)typed, shift);
        _punctuationKeyInFlight = MSIMEASCIIPunctuation(typed) ? typed : 0;
        transition = [_session typeASCII:(uint8_t)typed shift:shift error:nil];
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
    // A scheme that does not widen (`widens_full_width`: Korean, Vietnamese) writes half-width digits and punctuation whatever the width switch says, as host-api does for the Engine's own commits.
    if (!(MSIMESchemeRulesApply(_view) && !msime::mac::scheme::WidensFullWidth(MSIMEViewScheme(_view))) && _appearance.runtimeFullWidthInput && [_view[@"editing_text"] isKindOfClass:NSString.class] &&
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
    const uint64_t applySequence = ++_applySequence;
    const uint64_t glossViewSequence = _glossViewSequence;
    NSString *pendingClosing = _pendingPairedClosing;
    // A Korean, Zhuyin or Vietnamese composition is always drawn inline, whatever the preedit display preference says: until Option+Return opens a candidate list there is no candidate window to show it in, and hidden it would be text the user cannot see being written.
    const MSIMEInlinePreeditStyle preeditStyle = MSIMESchemeTrait(displayTransition[@"view"], msime::mac::scheme::AlwaysInlinePreedit)
        ? MSIMEInlinePreeditStylePinyin : _appearance.inlinePreeditStyle;
    MSIMEApplyTransitionTrackingMarkedText(displayTransition, (id<MSIMETextClient>)_activeClient, preeditStyle, pendingClosing, &_clientHasMarkedText);
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
        _clientHasMarkedText = YES;
        [(id<MSIMETextClient>)_activeClient setMarkedText:openedClosing
                                          selectionRange:NSMakeRange(0, 0)
                                        replacementRange:NSMakeRange(NSNotFound, NSNotFound)];
    }
    if ([displayTransition[@"commit"] isKindOfClass:NSString.class] && [displayTransition[@"commit"] length]) {
        const auto source = sourceOverride == msime::mac::TypingSource::Unknown
            ? MSIMEResolveTypingSource(transition[@"commit_context"], previousView, MSIMEStatisticsHostOptions(_session), _appearance.englishMode)
            : sourceOverride;
        if (MSIMECommitCountsAsTyping(transition[@"commit_context"]))
            MSIMERecordTypingStatistics(_preferencesDirectory ?: MSIMEStatisticsHostOptions(_session)[@"preferences_directory"],
                                        displayTransition[@"commit"], source);
        // Dictated text is not typing, and the melody follows the keyboard.
        if (source != msime::mac::TypingSource::Voice && !_secureEventInput) {
            [_session commitSound];
            [self typingEffect:MSIMETypingEffectEventCommit commit:YES];
        }
    }
    // A key handled while this transition's text was being written has already put the newer view on screen; this one is older and must not replace it.
    if (applySequence != _applySequence) return;
    // A gloss that arrived during the write was merged into this same page, and the transition's copy of the page predates it.
    if (glossViewSequence == _glossViewSequence || ![_view[@"session"] isEqual:transition[@"view"][@"session"]] ||
        ![_view[@"generation"] isEqual:transition[@"view"][@"generation"]])
        _view = transition[@"view"];
    if (![_view[@"candidates"] isKindOfClass:NSArray.class] || ![_view[@"candidates"] count])
        _armedGlossColumn = 0;
    [self refreshFloatingToolbarState];
    [self renderCandidates];
    [self synchronizeCandidateServices];
}

// Everything that follows the candidates on screen. Each one returns at once when its request has not changed.
- (void)synchronizeCandidateServices {
    // Every refresh clears the session's translations and moves its generation, even when the page shows the same words, and the requests below no longer change with the generation, so none of them would put the glosses back. Re-apply what is held once per generation instead, as Windows re-applies its cached glosses on every redraw (event_listener.cpp ApplyCandidateTranslations); the successful apply comes back here with the generation recorded.
    NSNumber *generation = _view[@"generation"];
    if ([generation isKindOfClass:NSNumber.class] && ![generation isEqual:_translationAppliedGeneration] &&
        (_glossResults.count || _targetGlossResults.count || _customResults.count || _accountGlossResults.count || _onDeviceGlossRequest)) {
        // Recorded before the attempt, so a refused apply falls through to the synchronization below rather than retrying.
        _translationAppliedGeneration = generation;
        if ([self applyCandidateTranslationResults]) return;
    }
    const BOOL nested = _serviceSnapshotActive;
    if (!nested) {
        _serviceSnapshotActive = YES;
        _serviceSnapshotQueryLoaded = NO;
        _serviceSnapshotViewLoaded = NO;
        _serviceSnapshotQuery = nil;
        _serviceSnapshotView = nil;
    }
    [self synchronizeCloudCandidates];
    [self scheduleSettledRerank];
    [self synchronizeCandidateGloss];
    [self synchronizeTargetGloss];
    [self synchronizeOnDeviceGloss];
    [self synchronizeAccountGloss:[self currentAccountGlossRequest]];
    [self synchronizeCustomTranslations];
    [self synchronizeAITranslations];
    if (!nested) [self invalidateServiceSnapshots];
}

// The card is at most half the screen's visible width, as the Windows card is at most half its work area, and at least seven times the candidate font size, as the Windows card and the source skins' `min-width: 7em` are. Every row is laid out at the width it then gets: text, 辅助码 and gloss wider than their column wrap inside it and the row takes their height (CandidateItemLayout.h).
- (MSIMECandidatePageGeometry)candidatePageGeometry:(NSArray *)candidates font:(NSFont *)font glossFont:(NSFont *)glossFont
                                     showSelectedBar:(BOOL)showSelectedBar inset:(CGFloat)inset paging:(BOOL)paging
                                             visible:(NSRect)visible preeditWidth:(CGFloat)preeditWidth
                                        minimumWidth:(CGFloat)minimumWidth {
    MSIMECandidatePageGeometry geometry;
    const BOOL vertical = _appearance.vertical;
    const BOOL traditional = _appearance.traditionalOutput && MSIMEScriptConversionApplies(_view);
    // The fonts arrive at the window's scale already; the row's own paddings follow them.
    const CGFloat scale = MSIMECandidateScale(_appearance);
    NSFont *numberFont = MSIMECandidateNumberFont(font);
    NSMutableArray<NSString *> *texts = [NSMutableArray arrayWithCapacity:candidates.count];
    NSMutableArray<NSString *> *annotations = [NSMutableArray arrayWithCapacity:candidates.count];
    NSMutableArray<NSString *> *displays = [NSMutableArray arrayWithCapacity:candidates.count];
    NSMutableArray<NSString *> *translations = [NSMutableArray arrayWithCapacity:candidates.count];
    NSMutableArray<NSString *> *readings = [NSMutableArray arrayWithCapacity:candidates.count];
    NSMutableArray<NSString *> *tooltips = [NSMutableArray arrayWithCapacity:candidates.count];
    // A Korean Hanja's 훈음 (나라 이름 한) arrives as the row's annotation. Drawn inline at the candidate size it made every row several times as wide as its one character, so it goes on the small gloss line under the Hanja instead, always, with any translation on the lines after it. It stays out of `translation`, which is what the gloss commit chords write.
    const BOOL hanjaReadings = MSIMEKoreanHanjaListOpen(_view);
    CGFloat candidateRow = MSIMECandidateTextHeight(@"", font) + MSIMECandidateRowPadding * scale;
    CGFloat numberWidth = 0;
    NSUInteger index = 0;
    for (NSDictionary *candidate in candidates) {
        NSString *hint = MSIMEWubiCodeHint(candidate, _view, _wubiCodeHintEnabled);
        NSString *text = CandidateTextRun(candidate, traditional);
        NSString *annotation = CandidateAnnotationRun(candidate, traditional, hint);
        NSString *reading = @"";
        if (hanjaReadings) {
            reading = annotation;
            annotation = @"";
        }
        NSString *gloss = MSIMECandidateGlossRun(reading, CandidateTranslation(candidate));
        // The tooltip and the accessibility label keep the reading apart from the Hanja rather than running the two together.
        NSString *display = hanjaReadings ? (reading.length ? [NSString stringWithFormat:@"%@ %@", text, reading] : text)
                                          : CandidateDisplayWithWubiHint(candidate, traditional, hint);
        [texts addObject:text];
        [annotations addObject:annotation];
        [displays addObject:display];
        [readings addObject:reading];
        [translations addObject:gloss];
        [tooltips addObject:gloss.length ? [NSString stringWithFormat:@"%@\n%@", hanjaReadings ? text : display, gloss] : display];
        NSString *number = [NSString stringWithFormat:@"%lu", (unsigned long)++index];
        numberWidth = MAX(numberWidth, [number sizeWithAttributes:@{NSFontAttributeName: numberFont}].width);
        // Fallback glyphs can stand taller than the primary font; a one-line row is as tall as its tallest.
        candidateRow = MAX(candidateRow, MSIMECandidateTextHeight([text stringByAppendingString:annotation], font) + MSIMECandidateRowPadding * scale);
    }
    geometry.texts = texts;
    geometry.annotations = annotations;
    geometry.displays = displays;
    geometry.readings = readings;
    geometry.tooltips = tooltips;
    geometry.contentLeft = MSIMECandidateTextLeft(showSelectedBar, scale) + ceil(numberWidth) + MSIMECandidateNumberGap * scale;
    const msime::mac::CandidateLayoutMetrics metrics =
        MSIMECandidateLayoutMetrics(font, glossFont, candidateRow, geometry.contentLeft + MSIMECandidateTextRight * scale, scale);
    std::vector<msime::mac::CandidateItemWidths> items;
    items.reserve(candidates.count);
    CGFloat natural = 0;
    for (NSUInteger i = 0; i < texts.count; ++i) {
        items.push_back(MSIMECandidateItemWidths(texts[i], annotations[i], translations[i], font, glossFont, scale));
        const CGFloat itemWidth = ceil(msime::mac::CandidateItemNaturalWidth(items.back(), metrics, !vertical));
        natural = vertical ? MAX(natural, itemWidth) : natural + itemWidth;
    }
    // The page arrows sit in the card's top row beside the reading, so they take no width from the candidate line.
    CGFloat width = MAX(20, natural + 2 * inset);
    width = MAX(width, preeditWidth);
    // Half the screen caps the card, except that a horizontal page whose candidate lines fit on one screen-wide line grows past it as far as its candidates need with nothing wrapped, glosses included, up to the screen less a margin on each side. Only a page wider than that squeezes its glosses to wrap under their text (SingleLineColumns). A page whose candidate lines alone do not fit breaks onto more lines anyway, so it keeps the half-screen card rather than a screen-wide one.
    CGFloat widthCap = MAX(80, floor(visible.size.width * 0.5));
    if (!vertical) {
        const CGFloat screenCap = MAX(widthCap, floor(visible.size.width - 2 * MSIMECandidateScreenMargin));
        if (ceil(msime::mac::SingleLineMinimumWidth(items, metrics)) + 2 * inset <= screenCap)
            widthCap = MAX(widthCap, MIN(screenCap, natural + 2 * inset));
    }
    width = MIN(width, widthCap);
    if (paging) width = MAX(width, 76 * scale);
    // At least 7em of the candidate font, raised by the skin's floor; the 7em part stays within the half-screen cap (CandidateItemLayout.h).
    width = MAX(width, msime::mac::CandidateCardMinimumWidth(font.pointSize, minimumWidth, widthCap));
    geometry.width = width;
    geometry.lineWidth = MAX(1, width - 2 * inset);
    const msime::mac::CandidatePageMeasure measure = [texts, annotations, translations, font, glossFont, scale](std::size_t row, msime::mac::CandidateRun run, double runWidth) {
        return MSIMECandidateRunMeasure(texts[row], annotations[row], translations[row], font, glossFont, scale)(run, runWidth);
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
    // The keymap opens beside the candidate window, so it is drawn in the candidate window's mode and marks the current key in the theme's accent.
    _keymapPanel.appearance = [_appearance candidateAppearanceOverride];
    [_keymapPanel setAccentColor:MSIMEThemedSkinColor(@"MSIMEKeymapAccent", [_appearance resolvedSkinForDark:NO].tokens.accent,
                                                      [_appearance resolvedSkinForDark:YES].tokens.accent)];
    [_keymapPanel updateHighlightedKey:MSIMEShuangpinKeymapHighlightedKey(_view)];
    const CGFloat scale = MSIMECandidateScale(_appearance);
    CGFloat clearance = (_appearance.fontSize + 42.0) * scale;
    if (_appearance.vertical) clearance = ((_appearance.fontSize + 10.0) * MIN([_view[@"candidates"] count], _appearance.pageSize) + 24.0) * scale;
    NSArray *candidates = MSIMEReorderedPinnedCandidates(_view[@"candidates"], MSIMECandidatePinCode(_view));
    if ([candidates isKindOfClass:NSArray.class] && candidates.count) {
        // The same rows the candidate panel lays out, so a card that wraps a long candidate onto several lines is kept clear of in full.
        NSScreen *screen = nil;
        for (NSScreen *candidateScreen in NSScreen.screens)
            if (NSPointInRect(NSMakePoint(NSMinX(cursor), NSMidY(cursor)), candidateScreen.frame)) { screen = candidateScreen; break; }
        screen = screen ?: NSScreen.mainScreen;
        NSFont *font = [_appearance candidateFontOfSize:_appearance.fontSize * scale englishFirst:YES];
        NSFont *glossFont = [_appearance candidateFontOfSize:MSIMECandidateTranslationPointSize * scale englishFirst:YES];
        const MSIMECandidatePageGeometry geometry =
            [self candidatePageGeometry:candidates font:font glossFont:glossFont showSelectedBar:_skinShowsSelectedBar inset:12 * scale
                                 paging:[_view[@"page_count"] unsignedIntegerValue] > 1
                                visible:screen ? screen.visibleFrame : NSMakeRect(0, 0, 1440, 900) preeditWidth:0 minimumWidth:0];
        clearance = MAX(clearance, geometry.rowsHeight + 24 * scale);
    }
    id preedit = [_view[@"preedit"] isKindOfClass:NSString.class] ? _view[@"preedit"] : editing;
    if ([_view[@"candidates"] count]) {
        // The card's top row: the brand mark, the reading when it is shown, and the page indicator whenever there is more than one page.
        CGFloat header = [_view[@"page_count"] unsignedIntegerValue] > 1 || MSIMECandidateLogoImage() ? MSIMECandidateHeaderHeight * scale : 0;
        if (_appearance.showsCandidatePreedit && [preedit length]) {
            NSFont *preeditFont = MSIMECandidatePreeditFont(_appearance);
            header = MAX(header, MAX(22.0 * scale, MSIMECandidateTextHeight(preedit, preeditFont) + 6.0 * scale));
        }
        clearance += header;
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
    NSDictionary *previousRenderedView = _renderedCandidateView;
    _candidateMenuToken = [NSObject new];
    [self updateKeymapPanel];
    if (_appearance.englishMode) { _renderedCandidateView = nil; [self resetCandidateAnchor]; [self hideCandidatePanel:"english_mode"]; return; }
    NSArray *candidates = MSIMEReorderedPinnedCandidates(_view[@"candidates"], MSIMECandidatePinCode(_view));
    if (![candidates isKindOfClass:NSArray.class] || candidates.count == 0) {
        _armedGlossColumn = 0;
        _renderedCandidateView = nil;
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
    if (!MSIMEValidCaret(cursor)) { _renderedCandidateView = nil; [self hideCandidatePanel:"invalid_caret"]; return; }
    NSScreen *screen = nil;
    for (NSScreen *candidate in NSScreen.screens) {
        if (NSPointInRect(NSMakePoint(NSMinX(cursor), NSMidY(cursor)), candidate.frame)) { screen = candidate; break; }
    }
    screen = screen ?: NSScreen.mainScreen;
    if (!screen) { _renderedCandidateView = nil; [self hideCandidatePanel:"no_screen"]; return; }
    NSRect visible = screen.visibleFrame;
    [self ensureAppearance];
    NSAppearance *candidateAppearance = [_appearance candidateAppearanceOverride];
    if (_appearance.candidateAppearanceOverrideConfigured) _panel.appearance = candidateAppearance;
    const BOOL vertical = _appearance.vertical;
    NSAppearance *currentAppearance = candidateAppearance ?: _panel.effectiveAppearance ?: NSApp.effectiveAppearance;
    NSString *currentTheme = [currentAppearance bestMatchFromAppearancesWithNames:@[NSAppearanceNameAqua, NSAppearanceNameDarkAqua]];
    // The styled skin carries the window's scale in its pad, radii, decoration and minimum width; the fonts and the fixed lengths below take it from `scale`.
    const auto skin = [_appearance candidateWindowSkinForDark:[currentTheme isEqual:NSAppearanceNameDarkAqua]];
    const auto geometry = skin.tokens;
    _skinShowsSelectedBar = geometry.showSelectedBar;
    const CGFloat scale = MSIMECandidateScale(_appearance);
    const CGFloat inset = MAX(2.0 * scale, geometry.pad);
    NSFont *font = [_appearance candidateFontOfSize:_appearance.fontSize * scale englishFirst:YES];
    id preeditValue = _view[@"preedit"];
    if (![preeditValue isKindOfClass:NSString.class]) preeditValue = _view[@"editing_text"];
    NSString *preedit = _appearance.showsCandidatePreedit && [preeditValue isKindOfClass:NSString.class] ? preeditValue : @"";
    NSFont *preeditFont = MSIMECandidatePreeditFont(_appearance);
    CGFloat preeditHeight = preedit.length ? MAX(22.0 * scale, MSIMECandidateTextHeight(preedit, preeditFont) + 6.0 * scale) : 0;
    const NSUInteger page = [_view[@"page"] unsignedIntegerValue];
    const NSUInteger pageCount = [_view[@"page_count"] unsignedIntegerValue];
    const BOOL paging = pageCount > 1;
    // The top row leads with the brand mark, as the floating toolbar and the mode HUD do, then the reading; 「1 / 3」 with ‹ › sit on the right.
    NSImage *logo = MSIMECandidateLogoImage();
    const CGFloat logoSide = MSIMECandidateLogoSide * scale;
    const CGFloat logoWidth = logo ? logoSide + MSIMECandidateLogoGap * scale : 0;
    const CGFloat headerRow = MSIMECandidateHeaderHeight * scale;
    const CGFloat arrowWidth = MSIMECandidatePageArrowWidth * scale;
    const CGFloat indicatorGap = MSIMECandidatePageIndicatorGap * scale;
    NSFont *pageIndicatorFont = [NSFont monospacedDigitSystemFontOfSize:MSIMECandidatePageIndicatorPointSize * scale weight:NSFontWeightRegular];
    NSString *pageIndicator = paging ? [NSString stringWithFormat:@"%lu / %lu", (unsigned long)(page + 1), (unsigned long)pageCount] : @"";
    const CGFloat pageIndicatorWidth = paging ? ceil([pageIndicator sizeWithAttributes:@{NSFontAttributeName: pageIndicatorFont}].width) : 0;
    const CGFloat pageControlsWidth = paging ? pageIndicatorWidth + indicatorGap + 2 * arrowWidth : 0;
    const CGFloat headerHeight = MAX(preeditHeight, paging || logo ? headerRow : 0);
    NSFont *numberFont = MSIMECandidateNumberFont(font);
    NSFont *glossFont = [_appearance candidateFontOfSize:MSIMECandidateTranslationPointSize * scale englishFirst:YES];
    const CGFloat preeditWidth = preedit.length ? ceil([preedit sizeWithAttributes:@{NSFontAttributeName:preeditFont}].width) + 4 * scale + MSIMEPreeditCaretGap : 0;
    const CGFloat headerWidth = preedit.length || paging || logo ? 2 * inset + logoWidth + preeditWidth + (preedit.length && paging ? indicatorGap : 0) + pageControlsWidth : 0;
    const MSIMECandidatePageGeometry pageGeometry =
        [self candidatePageGeometry:candidates font:font glossFont:glossFont showSelectedBar:geometry.showSelectedBar inset:inset
                             paging:paging visible:visible preeditWidth:headerWidth
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
    // The band above the card that the decoration stands in, transparent; none without an image to put there, as on Windows.
    const CGFloat decorationHeight = _appearance.decorationImage ? MAX(0.0, skin.decorationTopDip) : 0.0;
    const CGFloat height = pageGeometry.rowsHeight + 2 * inset + decorationHeight + headerHeight;
    const CGFloat headerBottom = height - inset - decorationHeight - headerHeight;
    // Gloss replies keep the candidate IDs and all panel structure stable. Repaint those rows in
    // place when their geometry is unchanged; a page or layout change still takes the full rebuild
    // below, which deliberately detaches every old button.
    if (MSIMEOnlyCandidateTranslationsChanged(previousRenderedView, _view) &&
        [_panel.contentView isKindOfClass:MSIMECandidateChromeView.class] &&
        NSEqualSizes(_panel.frame.size, NSMakeSize(width, height))) {
        MSIMECandidateChromeView *content = (id)_panel.contentView;
        BOOL reusable = YES;
        for (NSUInteger index = 0; index < candidates.count && reusable; ++index) {
            MSIMECandidateButton *button = nil;
            for (NSView *subview in content.subviews)
                if ([subview isKindOfClass:MSIMECandidateButton.class] && subview.tag == (NSInteger)index) {
                    if (button) { reusable = NO; break; }
                    button = (id)subview;
                }
            if (!button) { reusable = NO; break; }
            NSDictionary *candidate = candidates[index];
            const msime::mac::CandidateRowLayout &row = pageGeometry.rows[index];
            NSString *title = [NSString stringWithFormat:@"%lu  %@", (unsigned long)(index + 1), pageGeometry.texts[index]];
            if (![button.candidateID isEqual:candidate[@"id"]] || ![button.title isEqual:title] ||
                ![button.annotation isEqual:pageGeometry.annotations[index]] ||
                button.candidateHighlighted != [candidate[@"highlighted"] boolValue] ||
                !NSEqualRects(button.frame, NSMakeRect(inset + row.x, headerBottom - row.y - row.height,
                                                       row.width, row.height))) {
                reusable = NO;
                break;
            }
            button.menu = [self menuForCandidate:candidate];
            button.frame = NSMakeRect(inset + row.x, headerBottom - row.y - row.height,
                                      row.width, row.height);
            button.toolTip = pageGeometry.tooltips[index];
            button.translation = CandidateTranslation(candidate);
            button.glossReading = pageGeometry.readings[index];
            button.armedGlossColumn = _armedGlossColumn;
            button.translationFont = glossFont;
            button.itemLayout = row.item;
            button.hasItemLayout = YES;
            button.contentLeft = pageGeometry.contentLeft;
            button.translationBelow = MSIMECandidateGlossRun(button.glossReading, button.translation).length ? row.item.translation.below : !vertical;
            button.needsDisplay = YES;
        }
        if (reusable) {
            for (NSView *subview in content.subviews)
                if ([subview isKindOfClass:MSIMECandidateButton.class] && subview.tag < 0) {
                    MSIMECandidateButton *button = (id)subview;
                    button.candidateID = _view;
                    button.enabled = button.tag == -1 ? page > 0 : page + 1 < pageCount;
                }
            content.needsDisplay = YES;
            _renderedCandidateView = [_view copy];
            [self refreshCandidateSkin];
            const NSSize panelSize = _panel.frame.size;
            _tallestVerticalCandidateHeight = MSIMETallestCandidateHeight(_tallestVerticalCandidateHeight, panelSize.height, vertical, _panel.isVisible);
            [_panel setFrameOrigin:MSIMECandidateOrigin(cursor, panelSize, visible, vertical ? _tallestVerticalCandidateHeight : 0)];
            [self claimCandidatePanel];
            [_panel orderFrontRegardless];
            return;
        }
    }
    [_panel setContentSize:NSMakeSize(width, height)];
    MSIMECandidateChromeView *content = [[MSIMECandidateChromeView alloc] initWithFrame:NSMakeRect(0, 0, width, height)];
    NSUInteger slot = 0;
    const CGFloat rowsTop = headerBottom;
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
        button.chromeScale = scale;
        button.lineBreakMode = NSLineBreakByWordWrapping;
        button.annotation = row.item.annotation.width > 0 ? pageGeometry.annotations[slot - 1] : @"";
        button.translation = CandidateTranslation(candidate);
        button.glossReading = pageGeometry.readings[slot - 1];
        button.armedGlossColumn = _armedGlossColumn;
        button.translationFont = glossFont;
        button.itemLayout = row.item;
        button.hasItemLayout = YES;
        button.contentLeft = pageGeometry.contentLeft;
        button.translationBelow = MSIMECandidateGlossRun(button.glossReading, button.translation).length ? row.item.translation.below : !vertical;
        button.toolTip = pageGeometry.tooltips[slot - 1];
        button.bordered = NO;
        button.candidateHighlighted = [candidate[@"highlighted"] boolValue];
        id fixed = candidate[@"fixed_position"];
        button.candidateFixed = MSIMEUnsignedCandidateIdentityValue(fixed) &&
            [fixed compare:@0] == NSOrderedDescending && [fixed compare:@255] != NSOrderedDescending;
        button.alignment = NSTextAlignmentLeft;
        [content addSubview:button];
    }
    if (paging) {
        // Right-aligned in the top row, centred on the reading when that is taller than the arrows.
        const CGFloat controlsBottom = headerBottom + floor((headerHeight - headerRow) / 2);
        const CGFloat arrowsLeft = width - inset - 2 * arrowWidth;
        NSTextField *indicator = [NSTextField labelWithString:pageIndicator];
        indicator.identifier = @"candidate-page-indicator";
        indicator.accessibilityLabel = [NSString stringWithFormat:@"第 %lu 页，共 %lu 页", (unsigned long)(page + 1), (unsigned long)pageCount];
        indicator.font = pageIndicatorFont;
        indicator.alignment = NSTextAlignmentRight;
        const CGFloat indicatorHeight = ceil([pageIndicator sizeWithAttributes:@{NSFontAttributeName: pageIndicatorFont}].height);
        indicator.frame = NSMakeRect(arrowsLeft - indicatorGap - pageIndicatorWidth,
                                     controlsBottom + floor((headerRow - indicatorHeight) / 2), pageIndicatorWidth, indicatorHeight);
        [content addSubview:indicator];
        for (NSUInteger direction = 0; direction < 2; ++direction) {
            MSIMECandidateButton *button = [MSIMECandidateButton buttonWithTitle:direction == 0 ? @"‹" : @"›" target:self action:@selector(changeCandidatePage:)];
            button.frame = NSMakeRect(arrowsLeft + direction * arrowWidth, controlsBottom, arrowWidth, headerRow);
            button.font = [NSFont systemFontOfSize:NSFont.systemFontSize * scale];
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
        label.frame = NSMakeRect(inset + logoWidth, headerBottom + floor((headerHeight - preeditHeight) / 2),
                                 width - 2 * inset - logoWidth - (paging ? pageControlsWidth + indicatorGap : 0), preeditHeight);
        [content addSubview:label];
    }
    if (logo) {
        NSImageView *mark = [NSImageView imageViewWithImage:logo];
        mark.identifier = @"candidate-logo";
        mark.accessibilityLabel = @"水杉输入法";
        mark.imageScaling = NSImageScaleProportionallyUpOrDown;
        mark.frame = NSMakeRect(inset + 2 * scale, headerBottom + floor((headerHeight - logoSide) / 2), logoSide, logoSide);
        [content addSubview:mark];
    }
    content.cardTopInset = decorationHeight;
    const NSSize decorationSize = _appearance.decorationImage.size;
    if (const auto placed = msime::mac::DecorationPlacement(skin.decorationAlign, width, inset, decorationHeight, skin.decorationWidthDip,
                                                            decorationSize.width, decorationSize.height)) {
        // Added last so it sits over the card's top edge and the top row, which is what the overlap is for. It takes no clicks.
        NSImageView *decoration = [[NSImageView alloc] initWithFrame:NSMakeRect(placed->x, height - placed->top - placed->height, placed->width, placed->height)];
        decoration.identifier = @"candidate-decoration";
        decoration.image = _appearance.decorationImage;
        decoration.imageScaling = NSImageScaleAxesIndependently;
        decoration.wantsLayer = YES;
        [content addSubview:decoration];
    }
    _panel.contentView = content;
    _renderedCandidateView = [_view copy];
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
    const auto skin = [_appearance candidateWindowSkinForDark:[match isEqual:NSAppearanceNameDarkAqua]];
    const auto &tokens = skin.tokens;
    if (tokens.showSelectedBar != _skinShowsSelectedBar) { [self renderCandidates]; return; }
    // The card's own opacity is already in the surface and border alpha and in backgroundOpacity, rather than in the panel's alphaValue, which would fade the candidates along with the card.
    content.fillColor = SkinColor(tokens.surface);
    content.strokeColor = SkinColor(tokens.border);
    // tokens.radius is the user's radius when one is set, else the package's card radius when it sets one, at the window's scale.
    content.cornerRadius = tokens.radius;
    content.lineWidth = tokens.borderWidth;
    // Only in a mode whose resolution draws the package: one that supports a single mode draws no background in the other.
    content.backgroundImage = skin.backgroundPath.empty() ? nil : _appearance.backgroundImage;
    content.backgroundFit = skin.backgroundFit;
    content.backgroundOpacity = skin.backgroundOpacity;
    NSArray<MSIMECandidateButton *> *candidateButtons = [content.subviews filteredArrayUsingPredicate:
        [NSPredicate predicateWithBlock:^BOOL(NSView *view, NSDictionary *_) {
            return ([view isKindOfClass:MSIMECandidateButton.class] ||
                    [view isKindOfClass:MSIMECandidatePreeditField.class]) &&
                view.tag >= 0;
        }]];
    // The top row's page controls take the theme's colours too: labelColor would follow the system appearance, not a theme that fixes its own mode. The design draws 「1 / 3」 and ‹ › in the secondary colour (dc.html L1324); an arrow with no page behind it fades further.
    for (NSView *child in content.subviews) {
        if ([child.identifier isEqual:@"candidate-page-indicator"] && [child isKindOfClass:NSTextField.class])
            ((NSTextField *)child).textColor = SkinColor(tokens.number);
        if (![child isKindOfClass:MSIMECandidateButton.class] || (child.tag != -1 && child.tag != -2)) continue;
        MSIMECandidateButton *arrow = (id)child;
        arrow.titleColor = arrow.enabled ? SkinColor(tokens.number) : [SkinColor(tokens.number) colorWithAlphaComponent:tokens.number.a * 0.4];
        arrow.hoverColor = SkinColor(tokens.hover);
        arrow.cornerRadius = tokens.candidateRadius;
        arrow.needsDisplay = YES;
    }
    for (MSIMECandidateButton *button in candidateButtons) {
        if ([button.identifier isEqual:@"candidate-preedit"] && [button isKindOfClass:NSTextField.class]) {
            ((NSTextField *)(id)button).textColor = SkinColor(tokens.accent);
            if ([button isKindOfClass:MSIMECandidatePreeditField.class])
                ((MSIMECandidatePreeditField *)(id)button).caretColor = SkinColor(tokens.accent);
        }
        if (![button isKindOfClass:MSIMECandidateButton.class]) continue;
        button.fillColor = SkinColor(tokens.selected);
        button.hoverColor = SkinColor(tokens.hover);
        button.titleColor = button.candidateHighlighted ? SkinColor(tokens.selectedText) : SkinColor(tokens.text);
        // Windows fixed-position span overrides candidate text, not its number.
        if (button.candidateFixed) button.titleColor = [NSColor colorWithSRGBRed:55.0/255 green:154.0/255 blue:211.0/255 alpha:1];
        // The translation is the theme's secondary colour, which always equals the number colour: the design draws a plain row's gloss in cSub (dc.html L2150, L2183), the theme's kb.sub or the platform's sub, and on the selected fill in candSelTr (dc.html L1533), the selected number's colour. A fixed-position row keeps the Windows renderer's rule instead, where the translation is a child of the candidate text and takes its fixed-position colour at reduced opacity.
        // A package's own translation colour is its secondary colour, drawn on every row as Windows does.
        if (tokens.translation) button.translationColor = SkinColor(*tokens.translation);
        else if (button.candidateFixed) button.translationColor = [button.titleColor colorWithAlphaComponent:MSIMECandidateTranslationOpacity];
        else button.translationColor = button.candidateHighlighted ? SkinColor(tokens.selectedNumber) : SkinColor(tokens.number);
        button.numberColor = button.candidateHighlighted ? SkinColor(tokens.selectedNumber) : SkinColor(tokens.number);
        button.barColor = SkinColor(tokens.accent);
        button.showSelectedBar = tokens.showSelectedBar;
        const BOOL first = button == candidateButtons.firstObject;
        const BOOL last = button == candidateButtons.lastObject;
        button.cornerRadius = msime::mac::CandidateRowRadius(tokens, button.candidateHighlighted, first, last);
        button.selectionLeftInset = MSIMECandidateScale(_appearance);
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
    // The rows of an opened candidate list (the Korean Hanja table compiled into the Engine) are ones the Engine pins, fixes and removes none of, and the list learns nothing; the local pin below would still reorder the list under the composition. So they get no menu.
    if (MSIMECandidateListOpen(_view)) return nil;
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
    ApplyMetasequoiaMenuTheme(menu, [self resolvedMenuThemePreferences]);
    NSString *pinTitle = MSIMECandidateIsPinned(MSIMECandidatePinCode(_view), text) ? @"取消置顶" : @"置顶";
    [menu addItem:item(pinTitle, 0)];
    NSMenuItem *fixed = [[NSMenuItem alloc] initWithTitle:@"固定排位" action:nil keyEquivalent:@""];
    NSMenu *positions = [[NSMenu alloc] initWithTitle:@"固定排位"];
    positions.autoenablesItems = NO;
    ApplyMetasequoiaMenuTheme(positions, [self resolvedMenuThemePreferences]);
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
