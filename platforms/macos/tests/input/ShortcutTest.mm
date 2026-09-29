#import "../../src/input/InputController.mm"
#include "../../src/candidate/CandidatePageSize.h"
#import "../../src/input/InputSourceRegistration.h"
#import "../../src/candidate/SkinSettingsView.h"
#include <cassert>
#include <fstream>
#include <sqlite3.h>
#import <objc/runtime.h>
#import "../settings/TestPreferenceSuite.h"
#import "../settings/PreferenceViewLookup.h"

static NSUInteger missingKeyFontCalls;
static IMP originalMonospacedFont;
static NSFont *MissingKeyFont(id cls, SEL selector, CGFloat size, NSFontWeight weight) {
    if (size == 11.0 && weight == NSFontWeightBold) {
        ++missingKeyFontCalls;
        return nil;
    }
    return ((NSFont *(*)(id, SEL, CGFloat, NSFontWeight))originalMonospacedFont)(cls, selector, size, weight);
}

// The scheme is a radio group now, not a popup: report the checked one, and check one by
// sending the action the way a click does.
static NSInteger SelectedSchemeIndex(MSIMEAppearancePreferences *preferences) {
    for (NSControl *control in MSIMEFindPreferenceControls(preferences.window.contentView,
                                                           @selector(schemeRadioChanged:)))
        if (((NSButton *)control).state == NSControlStateValueOn) return control.tag;
    return -1;
}

static void SelectScheme(MSIMEAppearancePreferences *preferences, NSInteger index) {
    for (NSControl *control in MSIMEFindPreferenceControls(preferences.window.contentView,
                                                           @selector(schemeRadioChanged:)))
        if (control.tag == index) {
            ((NSButton *)control).state = NSControlStateValueOn;
            [NSApp sendAction:control.action to:control.target from:control];
            return;
        }
    assert(false && "Missing scheme radio");
}

static BOOL MSIMEViewHasComposition(NSDictionary *view) {
    if (![view isKindOfClass:NSDictionary.class]) return NO;
    NSString *editing = [view[@"editing_text"] isKindOfClass:NSString.class] ? view[@"editing_text"] : @"";
    NSArray *candidates = [view[@"candidates"] isKindOfClass:NSArray.class] ? view[@"candidates"] : @[];
    NSString *phrase = [view[@"phrase_prefix"] isKindOfClass:NSString.class] ? view[@"phrase_prefix"] : @"";
    return editing.length || candidates.count || phrase.length;
}

static NSControl *PreferenceControl(MSIMEAppearancePreferences *preferences, SEL action) {
    NSControl *control = MSIMEFindPreferenceControl(preferences.window.contentView, action);
    assert(control && "Missing preference action");
    return control;
}

static void CheckMenu(NSMenu *menu, id controller) {
    // "" is a separator and "*" a row without a controller action: the disabled scheme header and the theme submenu, whose action AppKit sets to submenuAction:.
    NSArray<NSString *> *actions = @[
        @"selectChineseMode:", @"selectEnglishMode:", @"toggleDedicatedEnglishMode:", @"",
        @"selectSimplifiedOutput:", @"selectTraditionalOutput:", @"",
        @"toggleFullWidthInput:", @"toggleChinesePunctuation:", @"toggleCandidateTranslations:", @"",
        @"*", @"selectInputScheme:", @"selectInputScheme:", @"selectInputScheme:", @"selectInputScheme:", @"",
        @"toggleFloatingToolbar:", @"showEmoji:", @"showScreenKeyboard:", @"showHandwriting:",
        @"showVoicePanel", @"", @"*", @"showDictionary:", @"showAppearance:", @"showAbout:"
    ];
    assert(menu.numberOfItems == (NSInteger)actions.count && !menu.autoenablesItems);
    for (NSUInteger index = 0; index < actions.count; ++index) {
        NSMenuItem *item = [menu itemAtIndex:index];
        if (actions[index].length == 0) assert(item.separatorItem);
        else if ([actions[index] isEqual:@"*"]) assert(!item.separatorItem && ((item.action == nil && item.target == nil) || item.hasSubmenu));
        else {
            assert(item.action == NSSelectorFromString(actions[index]));
            assert(item.target == controller && [controller respondsToSelector:item.action]);
        }
    }
}

@interface ShortcutSession : NSObject
@property(nonatomic) uint32_t lastCommand;
@property(nonatomic) NSUInteger edgeCalls;
@property(nonatomic) uint8_t lastEdge;
@property(nonatomic) uint64_t edgeGeneration;
@property(nonatomic) NSUInteger edgeIndex;
@property(nonatomic) NSUInteger selectCalls;
@property(nonatomic) uint64_t selectedGeneration;
@property(nonatomic) NSUInteger selectedIndex;
@property(nonatomic) NSUInteger maintenanceCalls;
@property(nonatomic) NSInteger maintenanceAction;
@property(nonatomic) NSUInteger resetCacheCalls;
@property(nonatomic, copy) NSDictionary *resetCacheTransition;
@property(nonatomic) NSUInteger englishCandidateCalls;
@property(nonatomic) BOOL dedicatedEnglish;
@property(nonatomic, copy) NSDictionary *nextTransition;
// Holds the engine call for this long, so a test can make one key slow enough to be logged.
@property(nonatomic) useconds_t stall;
@property(nonatomic) NSUInteger asciiCalls;
@property(nonatomic) uint8_t lastASCII;
@property(nonatomic) BOOL lastShift;
@property(nonatomic) NSUInteger punctuationASCIICalls;
@property(nonatomic) uint8_t lastPunctuationASCII;
@property(nonatomic, copy) NSDictionary *punctuationASCIITransition;
@property(nonatomic) NSUInteger contextualPunctuationCalls;
@property(nonatomic) uint8_t lastContextualPunctuation;
@property(nonatomic) uint32_t lastPrecedingScalar;
@property(nonatomic, copy) NSDictionary *contextualPunctuationTransition;
@property(nonatomic) NSUInteger enginePunctuationCalls;
@property(nonatomic) uint8_t lastEnginePunctuation;
@property(nonatomic, copy) NSDictionary *enginePunctuationTransition;
@property(nonatomic) uint8_t requestedPageSize;
@property(nonatomic) BOOL failFinish;
// Switching into English cancels the composition rather than finishing it, so the tests that check
// "an Engine failure must not switch the mode" have to be able to fail a cancel too.
@property(nonatomic) BOOL failCancel;
@property(nonatomic) NSUInteger focusCalls;
@property(nonatomic) BOOL chinesePunctuation;
@property(nonatomic) NSUInteger punctuationCalls;
@property(nonatomic, copy) NSDictionary *punctuationView;
@property(nonatomic) BOOL pairedPunctuation;
@property(nonatomic) NSUInteger pairedPunctuationCalls;
@property(nonatomic) NSUInteger balanceCalls;
@property(nonatomic) uint8_t lastBalanceOpening;
@property(nonatomic) uint8_t punctuationLock;
@property(nonatomic) NSUInteger punctuationLockCalls;
@property(nonatomic) BOOL fullwidth;
@property(nonatomic) NSUInteger widthCalls;
@property(nonatomic, copy) NSDictionary *finishTransition;
@property(nonatomic, copy) NSDictionary *cancelTransition;
@property(nonatomic) NSUInteger snapshotCalls;
@property(nonatomic, copy) NSDictionary *lastSnapshot;
@property(nonatomic) NSUInteger settledRerankCalls;
@property(nonatomic) NSUInteger rawCommitCalls;
@property(nonatomic) NSUInteger commandCalls;
@property(nonatomic, copy) NSDictionary *rawTransition;
@end
@implementation ShortcutSession
// The controller defers a preference snapshot to the main queue while a composition is live, so a block
// scheduled by one test can land in another test's run loop. Without this the fake raises an unrecognized
// selector from a completely unrelated test, which is how it surfaced.
- (NSDictionary *)updatePreferencesSnapshot:(NSDictionary *)snapshot error:(NSError **)error {
    (void)error;
    ++self.snapshotCalls;
    self.lastSnapshot = snapshot;
    return @{@"deferred":@NO, @"view":[self viewWithError:nil]};
}
// The settled rerank runs off a timer, so it fires inside whichever test happens to be draining the run
// loop when the delay elapses - the same way the deferred preference snapshot above does. Answering "nothing
// moved" keeps it from perturbing the test it lands in.
- (NSDictionary *)rerankSettledWithError:(NSError **)error {
    (void)error; ++self.settledRerankCalls; return @{@"moved":@NO};
}
- (NSDictionary *)translationQueryWithError:(NSError **)error { (void)error; return nil; }
- (NSDictionary *)onlineQueryWithError:(NSError **)error { (void)error; return nil; }
- (NSDictionary *)setCharacterWidthFull:(BOOL)fullwidth error:(NSError **)error {
    (void)error; self.fullwidth = fullwidth; ++self.widthCalls; return nil;
}
- (NSDictionary *)viewWithError:(NSError **)error {
    (void)error;
    return @{@"focused":@NO, @"editing_text":@"", @"candidates":@[], @"dedicated_english":@(self.dedicatedEnglish)};
}
- (NSDictionary *)setDedicatedEnglishEnabled:(BOOL)enabled error:(NSError **)error {
    (void)error; ++self.englishCandidateCalls; self.dedicatedEnglish = enabled;
    return @{@"focused":@YES, @"editing_text":@"", @"preedit":@"", @"candidates":@[], @"dedicated_english":@(enabled)};
}
- (NSDictionary *)pinGeneration:(uint64_t)generation index:(NSUInteger)index error:(NSError **)error {
    (void)error; ++self.maintenanceCalls; self.maintenanceAction = 0; self.selectedGeneration = generation; self.selectedIndex = index; return nil;
}
- (NSDictionary *)removeGeneration:(uint64_t)generation index:(NSUInteger)index error:(NSError **)error {
    (void)error; ++self.maintenanceCalls; self.maintenanceAction = 1; self.selectedGeneration = generation; self.selectedIndex = index; return nil;
}
- (NSDictionary *)clearPositionGeneration:(uint64_t)generation index:(NSUInteger)index error:(NSError **)error {
    (void)error; ++self.maintenanceCalls; self.maintenanceAction = 2; self.selectedGeneration = generation; self.selectedIndex = index; return nil;
}
- (NSDictionary *)fixGeneration:(uint64_t)generation index:(NSUInteger)index position:(uint8_t)position error:(NSError **)error {
    (void)error; ++self.maintenanceCalls; self.maintenanceAction = 10 + position; self.selectedGeneration = generation; self.selectedIndex = index; return nil;
}
- (NSDictionary *)selectGeneration:(uint64_t)generation index:(NSUInteger)index error:(NSError **)error {
    (void)error; ++self.selectCalls; self.selectedGeneration = generation; self.selectedIndex = index;
    return nil;
}
- (NSDictionary *)selectEdgeGeneration:(uint64_t)generation index:(NSUInteger)index edge:(uint8_t)edge error:(NSError **)error {
    (void)error; ++self.edgeCalls; self.lastEdge = edge; self.edgeGeneration = generation; self.edgeIndex = index;
    return self.nextTransition;
}
- (NSDictionary *)setChinesePunctuationEnabled:(BOOL)enabled error:(NSError **)error {
    (void)error;
    self.chinesePunctuation = enabled;
    ++self.punctuationCalls;
    return self.punctuationView;
}
- (BOOL)balancePairedPunctuationAfterAutoClose:(uint8_t)opening error:(NSError **)error {
    (void)error;
    ++self.balanceCalls;
    self.lastBalanceOpening = opening;
    return YES;
}
- (NSDictionary *)setPairedPunctuationEnabled:(BOOL)enabled error:(NSError **)error {
    (void)error;
    self.pairedPunctuation = enabled;
    ++self.pairedPunctuationCalls;
    return self.punctuationView;
}
- (NSDictionary *)setPunctuationLock:(NSString *)lock error:(NSError **)error {
    (void)error;
    if ([lock isEqual:@"chinese"]) self.punctuationLock = 1;
    else if ([lock isEqual:@"english"]) self.punctuationLock = 2;
    else self.punctuationLock = 0;
    ++self.punctuationLockCalls;
    return self.punctuationView;
}
- (NSDictionary *)setFocused:(BOOL)focused error:(NSError **)error {
    (void)error;
    ++self.focusCalls;
    return @{@"handled": @NO, @"commit": NSNull.null, @"view": @{@"focused": @(focused), @"editing_text": @"", @"candidates": @[]}};
}
- (NSDictionary *)setCandidatePageSize:(uint8_t)size error:(NSError **)error {
    (void)error;
    self.requestedPageSize = size;
    return nil;
}
- (NSDictionary *)typeASCII:(uint8_t)ascii shift:(BOOL)shift error:(NSError **)error {
    (void)error;
    ++self.asciiCalls;
    self.lastASCII = ascii;
    self.lastShift = shift;
    if (self.stall) usleep(self.stall);
    return self.nextTransition;
}
- (NSDictionary *)punctuationASCII:(uint8_t)ascii error:(NSError **)error {
    (void)error;
    ++self.punctuationASCIICalls;
    self.lastPunctuationASCII = ascii;
    return self.punctuationASCIITransition ?: self.nextTransition;
}
- (NSDictionary *)punctuation:(uint8_t)ascii preceding:(uint32_t)preceding error:(NSError **)error {
    (void)error;
    ++self.contextualPunctuationCalls;
    self.lastContextualPunctuation = ascii;
    self.lastPrecedingScalar = preceding;
    return self.contextualPunctuationTransition ?: self.nextTransition;
}
- (NSDictionary *)punctuation:(uint8_t)ascii error:(NSError **)error {
    (void)error;
    ++self.enginePunctuationCalls;
    self.lastEnginePunctuation = ascii;
    return self.enginePunctuationTransition ?: self.nextTransition;
}
- (NSDictionary *)command:(uint32_t)command error:(NSError **)error {
    (void)error;
    ++self.commandCalls;
    self.lastCommand = command;
    if (command == MSIME_COMMIT_RAW) ++self.rawCommitCalls;
    if (self.failFinish && command == MSIME_FINISH_COMPOSITION) return nil;
    if (self.failCancel && command == MSIME_CANCEL) return nil;
    if (self.finishTransition && command == MSIME_FINISH_COMPOSITION) return self.finishTransition;
    if (self.cancelTransition && command == MSIME_CANCEL) return self.cancelTransition;
    if (self.rawTransition && command == MSIME_COMMIT_RAW) return self.rawTransition;
    if (self.nextTransition) return self.nextTransition;
    return @{@"handled": @YES, @"commit": @"测试", @"view": @{@"editing_text": @"", @"caret_position": @0, @"candidates": @[]}};
}
- (NSDictionary *)resetCacheWithError:(NSError **)error {
    (void)error;
    ++self.resetCacheCalls;
    return self.resetCacheTransition;
}
@end

@interface ShortcutClient : NSObject <MSIMETextClient>
@property(nonatomic, copy) NSString *committed;
@property(nonatomic, copy) NSString *marked;
@property(nonatomic, copy) NSString *document;
@property(nonatomic) NSRange selection;
@property(nonatomic) NSRect caret;
@property(nonatomic, strong) NSMutableArray<NSString *> *insertions;
@end

static void TestBackspaceHoldDoesNotEscapeComposition() {
    NSString *suite = [@"msime.backspace-hold." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    MSIMEAppearancePreferences *appearance = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    ShortcutSession *session = [ShortcutSession new];
    ShortcutClient *client = [ShortcutClient new];
    MSIMEInputController *controller = [MSIMEInputController alloc];
    [controller setValue:appearance forKey:@"appearance"];
    [controller setValue:session forKey:@"session"];
    [controller setValue:client forKey:@"activeClient"];
    [controller setValue:@{ @"focused": @YES, @"editing_text": @"n", @"candidates": @[] } forKey:@"view"];
    session.nextTransition = @{ @"handled": @YES, @"commit": NSNull.null,
                                @"view": @{ @"focused": @YES, @"editing_text": @"", @"candidates": @[] } };
    NSEvent *(^backspace)(BOOL) = ^NSEvent *(BOOL repeat) {
        return [NSEvent keyEventWithType:NSEventTypeKeyDown location:NSZeroPoint modifierFlags:0 timestamp:0
                            windowNumber:0 context:nil characters:@"\b" charactersIgnoringModifiers:@"\b"
                               isARepeat:repeat keyCode:51];
    };

    // Backspace disarms the same-key revert that follows a space conversion, so only the character just converted can be taken back.
    [controller setValue:@((unichar)',') forKey:@"spaceRevertKey"];
    [controller setValue:@((unichar)0xFF0C) forKey:@"spaceRevertChinese"];
    [controller setValue:@(NSProcessInfo.processInfo.systemUptime) forKey:@"spaceRevertTime"];
    [controller setValue:client forKey:@"spaceRevertClient"];
    assert([controller handleEvent:backspace(NO) client:client]);
    assert(session.commandCalls == 1 && session.lastCommand == MSIME_BACKSPACE);
    assert([[controller valueForKey:@"spaceRevertKey"] integerValue] == 0);
    assert(![controller valueForKey:@"spaceRevertClient"]);
    assert([controller handleEvent:backspace(YES) client:client]);
    assert(session.commandCalls == 1); // Repeat is swallowed without touching Engine or document text.

    // Releasing and pressing Backspace again starts a new hold. With no
    // composition it belongs to the client, so the old guard must not claim it.
    session.nextTransition = @{ @"handled": @NO, @"commit": NSNull.null,
                                @"view": @{ @"focused": @YES, @"editing_text": @"", @"candidates": @[] } };
    assert(![controller handleEvent:backspace(NO) client:client]);
    assert(session.commandCalls == 2);

    // Focus loss also ends ownership; a repeat must never carry into another
    // text client even if AppKit delivers it after the switch.
    [controller setValue:@YES forKey:@"backspaceHoldArmed"];
    ShortcutClient *nextClient = [ShortcutClient new];
    NSUInteger callsBeforeSwitch = session.commandCalls;
    assert(![controller handleEvent:backspace(YES) client:nextClient]);
    assert(session.commandCalls == callsBeforeSwitch + 1);
    assert(![[controller valueForKey:@"backspaceHoldArmed"] boolValue]);
    MSIMERemoveTestPreferenceSuite(defaults, suite);
}
static NSDictionary *PassthroughStatisticsCall(NSString *root, NSDictionary *action) {
    NSData *request = [NSJSONSerialization dataWithJSONObject:@{@"directory": root, @"action": action} options:0 error:nil];
    char *raw = msime_client_typing_statistics(static_cast<const uint8_t *>(request.bytes), request.length);
    assert(raw);
    NSDictionary *response = [NSJSONSerialization JSONObjectWithData:[NSData dataWithBytes:raw length:strlen(raw)] options:0 error:nil];
    msime_client_string_free(raw);
    assert([response[@"ok"] boolValue]);
    return response[@"value"];
}

static NSDictionary *PassthroughStatisticsDetail(NSString *root) {
    // Records are written on the statistics worker; an empty block behind them drains it.
    dispatch_sync(MSIMETypingStatisticsQueue(), ^{});
    return PassthroughStatisticsCall(root, @{@"operation": @"load"})[@"detail"];
}

// Keys the input method hands back to the application are typed by the application, so they are counted at the exit of handleEvent:client: like MSIME-Windows counts uneaten OnTestKeyDown keys. Counting must never change the routing decision.
static void TestPassthroughKeysAreCounted() {
    NSString *root = [NSTemporaryDirectory() stringByAppendingPathComponent:NSUUID.UUID.UUIDString];
    assert([NSFileManager.defaultManager createDirectoryAtPath:root withIntermediateDirectories:YES attributes:nil error:nil]);
    NSString *suite = [@"msime.passthrough-statistics." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    MSIMEAppearancePreferences *appearance = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    appearance.fullWidthInput = NO;
    ShortcutSession *session = [ShortcutSession new];
    ShortcutClient *client = [ShortcutClient new];
    MSIMEInputController *controller = [MSIMEInputController alloc];
    [controller setValue:appearance forKey:@"appearance"];
    [controller setValue:session forKey:@"session"];
    [controller setValue:client forKey:@"activeClient"];
    [controller setValue:root forKey:@"preferencesDirectory"];
    NSDictionary *idle = @{ @"focused": @YES, @"editing_text": @"", @"candidates": @[], @"scheme": @0 };
    [controller setValue:idle forKey:@"view"];
    session.nextTransition = @{ @"handled": @NO, @"commit": NSNull.null, @"view": idle };
    NSEvent *(^key)(NSString *, unsigned short, NSEventModifierFlags) = ^NSEvent *(NSString *characters, unsigned short code, NSEventModifierFlags flags) {
        return [NSEvent keyEventWithType:NSEventTypeKeyDown location:NSZeroPoint modifierFlags:flags timestamp:0 windowNumber:0
                                 context:nil characters:characters charactersIgnoringModifiers:characters isARepeat:NO keyCode:code];
    };
    NSString *upArrow = [NSString stringWithFormat:@"%C", (unichar)NSUpArrowFunctionKey];

    // With statistics off nothing is recorded, and the opt-in gate is the controller's, not only the store's.
    PassthroughStatisticsCall(root, @{@"operation": @"set_enabled", @"enabled": @YES});
    MSIMETypingStatisticsEnabled.store(false, std::memory_order_relaxed);
    appearance.englishMode = YES;
    assert(![controller handleEvent:key(@"a", 0, 0) client:client]);
    assert([PassthroughStatisticsDetail(root)[@"characters"][@"latin"] integerValue] == 0);

    MSIMEReloadTypingStatisticsEnabled(root);
    assert(MSIMETypingStatisticsEnabled.load(std::memory_order_relaxed));
    assert(![controller handleEvent:key(@"a", 0, 0) client:client]);
    NSDictionary *detail = PassthroughStatisticsDetail(root);
    assert([detail[@"characters"][@"latin"] integerValue] == 1);
    assert([detail[@"sources"][@"english"] integerValue] == 1);

    // Shortcut chords and AppKit function keys reach the application but type nothing.
    assert(![controller handleEvent:key(@"c", 8, NSEventModifierFlagCommand) client:client]);
    assert(![controller handleEvent:key(upArrow, 126, NSEventModifierFlagFunction | NSEventModifierFlagNumericPad) client:client]);
    detail = PassthroughStatisticsDetail(root);
    assert([detail[@"characters"][@"latin"] integerValue] == 1);
    assert([detail[@"characters"][@"symbol"] integerValue] == 0 && [detail[@"characters"][@"other"] integerValue] == 0);

    // A Chinese-mode digit the Engine declines is typed by the application and takes the current scheme's source.
    appearance.englishMode = NO;
    const NSUInteger asciiCalls = session.asciiCalls;
    assert(![controller handleEvent:key(@"1", 18, 0) client:client]);
    assert(session.asciiCalls == asciiCalls + 1 && session.lastASCII == '1');
    assert(client.insertions.count == 0);
    detail = PassthroughStatisticsDetail(root);
    assert([detail[@"characters"][@"number"] integerValue] == 1);
    assert([detail[@"sources"][@"quanpin"] integerValue] == 1);
    // A key the Engine consumes is not a passthrough.
    session.nextTransition = @{ @"handled": @YES, @"commit": NSNull.null, @"view": idle };
    assert([controller handleEvent:key(@"2", 19, 0) client:client]);
    assert([PassthroughStatisticsDetail(root)[@"characters"][@"number"] integerValue] == 1);

    MSIMETypingStatisticsEnabled.store(false, std::memory_order_relaxed);
    MSIMERemoveTestPreferenceSuite(defaults, suite);
    [NSFileManager.defaultManager removeItemAtPath:root error:nil];
}

// The diagnostic log times every key through handleEvent:client: and, like MSIME-Windows' [key-latency] stage=handle lines, writes only a key that took at least 8 ms, recording the event type and outcome but never the key itself.
static void TestKeyLatencyIsLoggedWithoutTheKey() {
    NSString *root = [NSTemporaryDirectory() stringByAppendingPathComponent:NSUUID.UUID.UUIDString];
    assert([NSFileManager.defaultManager createDirectoryAtPath:root withIntermediateDirectories:YES attributes:nil error:nil]);
    NSString *suite = [@"msime.key-latency-log." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    MSIMEAppearancePreferences *appearance = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    appearance.fullWidthInput = NO;
    appearance.englishMode = YES;
    ShortcutSession *session = [ShortcutSession new];
    ShortcutClient *client = [ShortcutClient new];
    MSIMEInputController *controller = [MSIMEInputController alloc];
    [controller setValue:appearance forKey:@"appearance"];
    [controller setValue:session forKey:@"session"];
    [controller setValue:client forKey:@"activeClient"];
    [controller setValue:root forKey:@"preferencesDirectory"];
    NSDictionary *idle = @{ @"focused": @YES, @"editing_text": @"", @"candidates": @[], @"scheme": @0 };
    [controller setValue:idle forKey:@"view"];
    session.nextTransition = @{ @"handled": @NO, @"commit": NSNull.null, @"view": idle };
    NSEvent *key = [NSEvent keyEventWithType:NSEventTypeKeyDown location:NSZeroPoint modifierFlags:0 timestamp:0 windowNumber:0
                                     context:nil characters:@"q" charactersIgnoringModifiers:@"q" isARepeat:NO keyCode:12];
    NSString *logPath = [root stringByAppendingPathComponent:@"diagnostic.log"];

    // Off: no timing line and no file.
    msime_macos_diagnostic_configure(root.fileSystemRepresentation, false);
    assert(![controller handleEvent:key client:client]);
    assert(![NSFileManager.defaultManager fileExistsAtPath:logPath]);

    msime_macos_diagnostic_configure(root.fileSystemRepresentation, true);
    assert(![controller handleEvent:key client:client]);
    NSString *fast = [NSString stringWithContentsOfFile:logPath encoding:NSUTF8StringEncoding error:nil];
    assert(![fast containsString:@"[key-latency]"]); // A key handled well under 8 ms is not written.
    appearance.englishMode = NO;
    session.stall = 12000;
    assert(![controller handleEvent:key client:client]);
    assert(session.asciiCalls > 0);
    session.stall = 0;
    msime_macos_diagnostic_configure(root.fileSystemRepresentation, false);
    NSString *contents = [NSString stringWithContentsOfFile:logPath encoding:NSUTF8StringEncoding error:nil];
    assert([contents containsString:@"[key-latency] stage=handle type=down handled=0 elapsed_ms="]);
    // Timestamps, pids and labels never contain a q, so any q would be the typed key leaking.
    assert(![contents containsString:@"q"] && ![contents containsString:@"keycode"]);

    MSIMERemoveTestPreferenceSuite(defaults, suite);
    [NSFileManager.defaultManager removeItemAtPath:root error:nil];
}

@implementation ShortcutClient
- (NSRange)selectedRange { return self.selection; }
- (NSAttributedString *)attributedSubstringFromRange:(NSRange)range {
    if (range.location == NSNotFound || range.location > self.document.length || range.length > self.document.length - range.location) return nil;
    return [[NSAttributedString alloc] initWithString:[self.document substringWithRange:range]];
}
- (NSDictionary *)attributesForCharacterIndex:(NSUInteger)index lineHeightRectangle:(NSRect *)rect {
    (void)index;
    *rect = self.caret;
    return @{};
}
- (void)insertText:(id)text replacementRange:(NSRange)range {
    self.committed = text;
    if ([text isKindOfClass:NSString.class]) {
        NSRange target = range.location == NSNotFound ? self.selection : range;
        if (target.location != NSNotFound && target.location <= self.document.length && target.length <= self.document.length - target.location) {
            NSString *string = text;
            self.document = [self.document stringByReplacingCharactersInRange:target withString:string];
            self.selection = NSMakeRange(target.location + string.length, 0);
        }
        if (!self.insertions) self.insertions = [NSMutableArray array];
        [self.insertions addObject:text];
    }
}
- (void)setMarkedText:(id)text selectionRange:(NSRange)selection replacementRange:(NSRange)range {
    (void)selection;
    (void)range;
    self.marked = text;
}
@end

@interface TestCandidatePanel : NSObject
@property(nonatomic, getter=isVisible) BOOL visible;
@end
@implementation TestCandidatePanel
- (void)orderOut:(id)sender { (void)sender; self.visible = NO; }
@end

@interface HiddenCandidatePanel : MSIMECandidatePanel
@property(nonatomic) BOOL requestedVisible;
@end
@implementation HiddenCandidatePanel
- (BOOL)isVisible { return self.requestedVisible; }
- (void)orderFrontRegardless { self.requestedVisible = YES; }
- (void)orderOut:(id)sender { (void)sender; self.requestedVisible = NO; }
@end

@interface ModeController : MSIMEInputController
@property(nonatomic) NSUInteger preparationCalls;
@property(nonatomic) NSUInteger paletteCalls;
@property(nonatomic) NSUInteger screenKeyboardCalls;
@property(nonatomic) NSUInteger restartCalls;
@property(nonatomic) NSUInteger terminationCalls;
@end
@implementation ModeController
- (void)prepareSession {
    ++self.preparationCalls;
    if ([self valueForKey:@"session"]) [super prepareSession];
}
- (void)showSystemCharacterPalette { ++self.paletteCalls; }
- (void)showScreenKeyboard:(id)sender { (void)sender; ++self.screenKeyboardCalls; }
- (void)restartCurrentInputMethod { ++self.restartCalls; }
- (void)terminateCurrentInputMethod { ++self.terminationCalls; }
@end

static NSEvent *ModeKey(unsigned short code, NSEventModifierFlags flags, BOOL repeat) {
    return [NSEvent keyEventWithType:NSEventTypeKeyDown location:NSZeroPoint modifierFlags:flags timestamp:0 windowNumber:0 context:nil characters:code == 49 ? @" " : @"a" charactersIgnoringModifiers:code == 49 ? @" " : @"a" isARepeat:repeat keyCode:code];
}

static NSEvent *KeypadKey(unsigned short code, NSString *characters, NSEventModifierFlags flags, BOOL repeat) {
    return [NSEvent keyEventWithType:NSEventTypeKeyDown location:NSZeroPoint modifierFlags:flags timestamp:0 windowNumber:0 context:nil characters:characters charactersIgnoringModifiers:characters isARepeat:repeat keyCode:code];
}

@interface HiddenKeymapPanel : MSIMEShuangpinKeymapPanel
@property(nonatomic) BOOL requestedVisible;
@property(nonatomic) CGFloat clearance;
@end
@implementation HiddenKeymapPanel
- (void)showNearCaretRect:(NSRect)rect candidateClearance:(CGFloat)clearance {
    (void)rect; self.requestedVisible = YES; self.clearance = clearance;
}
- (void)orderOut:(id)sender { (void)sender; self.requestedVisible = NO; }
@end

@interface SuccessfulPageSession : ShortcutSession
@property(nonatomic) NSUInteger pageSizeCalls;
@end
@implementation SuccessfulPageSession
- (NSDictionary *)setCandidatePageSize:(uint8_t)size error:(NSError **)error {
    (void)error;
    self.requestedPageSize = size;
    ++self.pageSizeCalls;
    return @{@"view": @{@"editing_text": @"", @"candidates": @[]}};
}
@end

static void TestPageSizeCache() {
    NSString *suite = [@"msime.page-cache." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    MSIMEAppearancePreferences *prefs = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    ModeController *controller = [ModeController alloc];
    SuccessfulPageSession *session = [SuccessfulPageSession new];
    [controller setValue:prefs forKey:@"appearance"];
    [controller setValue:session forKey:@"session"];
    [controller syncPageSize];
    [controller syncPageSize];
    // Nothing stored yet, so this is the shared default rather than the top of the range.
    assert(session.pageSizeCalls == 1 && session.requestedPageSize == 6);
    [controller applySharedToolbarPreferences:@{@"candidate_page_size": @5}];
    // The shared snapshot changed Engine independently; a local return to 9
    // must not be skipped just because the last direct request was also 9.
    prefs.pageSize = 9;
    [controller syncPageSize];
    assert(session.pageSizeCalls == 2 && session.requestedPageSize == 9);
    [controller applySharedToolbarPreferences:@{@"candidate_page_size": @9}];
    [controller syncPageSize];
    assert(session.pageSizeCalls == 2);
    ShortcutClient *client = [ShortcutClient new];
    client.marked = @"synthetic";
    [controller setValue:client forKey:@"activeClient"];
    HiddenKeymapPanel *keymap = [[HiddenKeymapPanel alloc] init];
    keymap.requestedVisible = YES;
    [controller setValue:keymap forKey:@"keymapPanel"];
    HiddenCandidatePanel *panel = [[HiddenCandidatePanel alloc] init];
    panel.requestedVisible = YES;
    [controller setValue:panel forKey:@"panel"];
    [controller snapshotSessionReplaced:[NSNotification notificationWithName:@"synthetic" object:[NSObject new]]];
    [controller syncPageSize];
    assert(session.pageSizeCalls == 2 && [client.marked isEqual:@"synthetic"] && keymap.requestedVisible && panel.requestedVisible);
    session.dedicatedEnglish = YES;
    [controller snapshotSessionReplaced:[NSNotification notificationWithName:@"synthetic" object:session]];
    assert([[[controller valueForKey:@"view"] objectForKey:@"dedicated_english"] isEqual:@YES]);
    assert(client.marked.length == 0 && client.committed == nil && !keymap.requestedVisible && !panel.requestedVisible);
    assert([[controller valueForKey:@"focusPending"] boolValue]);
    [controller syncPageSize];
    assert(session.pageSizeCalls == 3 && session.requestedPageSize == 9);
    [controller prepareSession];
    assert(session.focusCalls == 1 && ![[controller valueForKey:@"focusPending"] boolValue]);
    MSIMERemoveTestPreferenceSuite(defaults, suite);
}

static void TestSharedPunctuation() {
    NSString *suite = [@"msime.punctuation." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    MSIMEAppearancePreferences *prefs = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    ModeController *controller = [ModeController alloc];
    MSIMEFloatingToolbarPanel *toolbar = [[MSIMEFloatingToolbarPanel alloc] init];
    [controller setValue:prefs forKey:@"appearance"];
    [controller setValue:toolbar forKey:@"toolbar"];
    NSButton *toggle = (id)PreferenceControl(prefs, @selector(punctuationChanged:));
    NSButton *toolbarToggle = [toolbar valueForKey:@"punctuationButton"];
    __block NSUInteger saves = 0;
    id observer = [NSNotificationCenter.defaultCenter addObserverForName:MSIMEAppearanceDidChangeNotification object:prefs queue:nil usingBlock:^(NSNotification *note) { (void)note; ++saves; }];
    for (NSNumber *enabled in @[@NO, @YES, @NO]) {
        [controller applySharedToolbarPreferences:@{@"chinese_punctuation": enabled}];
        assert(prefs.chinesePunctuation == enabled.boolValue && toggle.state == (enabled.boolValue ? NSControlStateValueOn : NSControlStateValueOff));
        assert([toolbarToggle.title isEqual:enabled.boolValue ? @"。" : @"."]);
        assert([toolbarToggle.accessibilityLabel isEqual:enabled.boolValue ? @"切换到西文标点" : @"切换到中文标点"]);
        assert([[prefs sharedPreferencesByMerging:@{}][@"chinese_punctuation"] isEqual:enabled] && saves == 0);
    }
    assert([defaults objectForKey:@"MSIMEClientChinesePunctuation"] == nil);
    for (id invalid in @[NSNull.null, @1, @"true"]) {
        [controller applySharedToolbarPreferences:@{@"chinese_punctuation": invalid}];
        assert(!prefs.chinesePunctuation && [toolbarToggle.title isEqual:@"."] && saves == 0);
    }
    // The toolbar switches this app's punctuation only: nothing is saved and the settings checkbox keeps the starting value.
    [controller floatingToolbarDidRequestTogglePunctuation:toolbar];
    assert(prefs.runtimeChinesePunctuation && [toolbarToggle.title isEqual:@"。"] && saves == 0);
    assert(!prefs.chinesePunctuation && toggle.state == NSControlStateValueOff);
    assert([defaults objectForKey:@"MSIMEClientChinesePunctuation"] == nil);
    assert([[prefs sharedPreferencesByMerging:@{}][@"chinese_punctuation"] isEqual:@NO]);
    MSIMEAppearancePreferences *reopened = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    assert(reopened.chinesePunctuation && reopened.runtimeChinesePunctuation); // Nothing saved, so the built-in default.
    // Re-applying the same shared value keeps the toggle; a new one is a new starting value for every app.
    [controller applySharedToolbarPreferences:@{@"chinese_punctuation": @NO}];
    assert(prefs.runtimeChinesePunctuation && [toolbarToggle.title isEqual:@"。"]);
    [controller applySharedToolbarPreferences:@{@"chinese_punctuation": @YES}];
    [controller floatingToolbarDidRequestTogglePunctuation:toolbar];
    assert(!prefs.runtimeChinesePunctuation);
    [controller applySharedToolbarPreferences:@{@"chinese_punctuation": @NO}];
    assert(!prefs.chinesePunctuation && !prefs.runtimeChinesePunctuation && [toolbarToggle.title isEqual:@"."] && saves == 0);
    [NSNotificationCenter.defaultCenter removeObserver:observer];
    MSIMERemoveTestPreferenceSuite(defaults, suite);
}

static void TestSharedTraditionalOutput() {
    NSString *suite = [@"msime.traditional." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    MSIMEAppearancePreferences *prefs = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    ModeController *controller = [ModeController alloc];
    MSIMEFloatingToolbarPanel *toolbar = [[MSIMEFloatingToolbarPanel alloc] init];
    [controller setValue:prefs forKey:@"appearance"];
    [controller setValue:toolbar forKey:@"toolbar"];
    NSButton *toggle = [toolbar valueForKey:@"traditionalOutputButton"];
    __block NSUInteger saves = 0;
    id observer = [NSNotificationCenter.defaultCenter addObserverForName:MSIMEAppearanceDidChangeNotification object:prefs queue:nil usingBlock:^(NSNotification *note) { (void)note; ++saves; }];
    assert(!prefs.traditionalOutput);
    assert(![prefs sharedPreferencesByMerging:@{}][@"traditional_chinese_output"]);
    assert([[prefs sharedPreferencesByMerging:@{@"traditional_chinese_output":@YES}][@"traditional_chinese_output"] isEqual:@YES]);
    for (NSNumber *enabled in @[@YES, @NO, @YES]) {
        [controller applySharedToolbarPreferences:@{@"traditional_chinese_output":enabled}];
        assert(prefs.traditionalOutput == enabled.boolValue && saves == 0);
        assert([toggle.title isEqual:enabled.boolValue ? @"繁" : @"简"]);
        assert([controller.menu itemAtIndex:enabled.boolValue ? 5 : 4].state == NSControlStateValueOn);
        assert([[prefs cloudSettingsSnapshot][@"platform.macos.traditional_chinese_output"] isEqual:enabled]);
    }
    assert([defaults objectForKey:@"MSIMEClientTraditionalOutput"] == nil);
    for (id invalid in @[NSNull.null, @1, @"true"]) {
        [controller applySharedToolbarPreferences:@{@"traditional_chinese_output":invalid}];
        assert(prefs.traditionalOutput && saves == 0);
    }
    [controller selectSimplifiedOutput:nil];
    assert(!prefs.traditionalOutput && saves == 1);
    assert([[prefs sharedPreferencesByMerging:@{}][@"traditional_chinese_output"] isEqual:@NO]);
    [controller applySharedToolbarPreferences:@{@"traditional_chinese_output":@YES}];
    assert(prefs.traditionalOutput && saves == 1);
    assert([[prefs sharedPreferencesByMerging:@{}][@"traditional_chinese_output"] isEqual:@YES]);
    NSMutableDictionary *cloud = [[prefs cloudSettingsSnapshot] mutableCopy];
    cloud[@"platform.macos.traditional_chinese_output"] = @NO;
    assert([prefs applyCloudSettingsSnapshot:cloud]);
    assert(!prefs.traditionalOutput && saves == 2);
    assert([[prefs sharedPreferencesByMerging:@{}][@"traditional_chinese_output"] isEqual:@NO]);
    [controller selectTraditionalOutput:nil];
    assert(prefs.traditionalOutput && saves == 3);
    assert([[[MSIMEAppearancePreferences alloc] initWithDefaults:defaults] traditionalOutput]);

    // Persist through the actual shared store, then consume it with fresh local defaults.
    NSString *root = [NSTemporaryDirectory() stringByAppendingPathComponent:NSUUID.UUID.UUIDString];
    NSError *error = nil;
    NSDictionary *snapshot = [MSIMEClientSession loadPreferencesInDirectory:root error:&error];
    assert(snapshot && !error);
    NSDictionary *saved = [MSIMEClientSession savePreferencesInDirectory:root expectedRevision:[snapshot[@"revision"] unsignedLongLongValue]
        snapshot:@{@"format_version":@1, @"revision":snapshot[@"revision"], @"preferences":[prefs sharedPreferencesByMerging:snapshot[@"preferences"]]} error:&error];
    assert(saved && !error);
    NSDictionary *loaded = [MSIMEClientSession loadPreferencesInDirectory:root error:&error];
    assert(loaded && !error && [loaded[@"preferences"][@"traditional_chinese_output"] isEqual:@YES]);
    [NSNotificationCenter.defaultCenter removeObserver:observer];
    MSIMERemoveTestPreferenceSuite(defaults, suite);
    MSIMEAppearancePreferences *fresh = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    assert(!fresh.traditionalOutput);
    [fresh applySharedInputPreferences:loaded[@"preferences"]];
    assert(fresh.traditionalOutput);
    assert([NSFileManager.defaultManager removeItemAtPath:root error:&error] && !error);
}

static void TestSharedCharacterWidth() {
    NSString *suite = [@"msime.character-width." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    MSIMEAppearancePreferences *preferences = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    assert(!preferences.fullWidthInput);
    assert([[preferences sharedPreferencesByMerging:@{}][@"character_width"] isEqual:@"halfwidth"]);

    [preferences applySharedInputPreferences:@{@"character_width": @"fullwidth"}];
    assert(preferences.fullWidthInput);
    assert([[preferences sharedPreferencesByMerging:@{}][@"character_width"] isEqual:@"fullwidth"]);
    assert([defaults objectForKey:@"MSIMEClientFullWidthInput"] == nil);

    [preferences applySharedInputPreferences:@{@"character_width": @"halfwidth"}];
    assert(!preferences.fullWidthInput);
    for (id invalid in @[@YES, @1, @"wide", NSNull.null]) {
        [preferences applySharedInputPreferences:@{@"character_width": invalid}];
        assert(!preferences.fullWidthInput);
    }

    preferences.fullWidthInput = YES;
    assert(preferences.fullWidthInput);
    assert([[preferences sharedPreferencesByMerging:@{}][@"character_width"] isEqual:@"fullwidth"]);
    assert([defaults boolForKey:@"MSIMEClientFullWidthInput"]);
    MSIMERemoveTestPreferenceSuite(defaults, suite);
}

static void TestIndependentAssistancePreferences() {
    NSString *suite = [@"msime.assistance." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    [defaults setBool:NO forKey:@"MSIMEClientHelpcodeEnabled"];
    MSIMEAppearancePreferences *prefs = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    assert(!prefs.quanpinHelpcodeEnabled && !prefs.shuangpinHelpcodeEnabled);
    assert(prefs.autocorrectTransposition && prefs.autocorrectNeighbor);
    assert([[prefs helpcodeOptionsForScheme:@"quanpin"] isEqual:
        (@{@"schema": @"ziranma", @"show_in_candidate_window": @NO})]);
    assert([[prefs helpcodeOptionsForScheme:@"shuangpin"] isEqual:
        (@{@"schema": @"lantian", @"show_in_candidate_window": @YES})]);
    ModeController *controller = [ModeController alloc];
    [controller setValue:prefs forKey:@"appearance"];
    NSButton *quanpin = (id)PreferenceControl(prefs, @selector(quanpinHelpcodeChanged:));
    NSButton *shuangpin = (id)PreferenceControl(prefs, @selector(shuangpinHelpcodeChanged:));
    NSButton *autocorrect = (id)PreferenceControl(prefs, @selector(transpositionChanged:));
    __block NSUInteger saves = 0;
    id observer = [NSNotificationCenter.defaultCenter addObserverForName:MSIMEAppearanceDidChangeNotification object:prefs queue:nil usingBlock:^(NSNotification *note) { (void)note; ++saves; }];
    NSDictionary *shared = @{@"autocorrect": @NO, @"quanpin_helpcode": @{@"enabled": @YES, @"auto_display": @NO}, @"shuangpin_helpcode": @{@"enabled": @NO, @"future_field": @7}};
    [controller applySharedToolbarPreferences:shared];
    assert(prefs.quanpinHelpcodeEnabled && !prefs.shuangpinHelpcodeEnabled && !prefs.autocorrect && saves == 0);
    assert(quanpin.state == NSControlStateValueOn && shuangpin.state == NSControlStateValueOff && autocorrect.state == NSControlStateValueOn);
    for (NSString *key in shared) assert([[prefs sharedPreferencesByMerging:shared][key] isEqual:shared[key]]);
    [controller applySharedToolbarPreferences:@{@"autocorrect": @1, @"quanpin_helpcode": @{@"enabled": @0}, @"shuangpin_helpcode": NSNull.null}];
    assert(prefs.quanpinHelpcodeEnabled && !prefs.shuangpinHelpcodeEnabled && !prefs.autocorrect && saves == 0);
    quanpin.state = NSControlStateValueOff;
    [NSApp sendAction:quanpin.action to:quanpin.target from:quanpin];
    assert(!prefs.quanpinHelpcodeEnabled && !prefs.shuangpinHelpcodeEnabled && saves == 1);
    shuangpin.state = NSControlStateValueOn;
    [NSApp sendAction:shuangpin.action to:shuangpin.target from:shuangpin];
    prefs.autocorrect = YES;
    assert(!prefs.quanpinHelpcodeEnabled && prefs.shuangpinHelpcodeEnabled && prefs.autocorrect && saves == 3);
    MSIMEAppearancePreferences *reopened = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    assert(!reopened.quanpinHelpcodeEnabled && reopened.shuangpinHelpcodeEnabled && reopened.autocorrect);
    NSDictionary *edited = [prefs sharedPreferencesByMerging:shared];
    assert([edited[@"quanpin_helpcode"][@"enabled"] isEqual:@NO] && [edited[@"shuangpin_helpcode"][@"enabled"] isEqual:@YES]);
    assert([edited[@"quanpin_helpcode"][@"auto_display"] isEqual:@NO] && [edited[@"shuangpin_helpcode"][@"future_field"] isEqual:@7]);
    [controller applySharedToolbarPreferences:shared];
    assert(prefs.quanpinHelpcodeEnabled && !prefs.shuangpinHelpcodeEnabled && !prefs.autocorrect && saves == 3);
    NSMutableDictionary *schemaControls = [NSMutableDictionary dictionary];
    NSMutableDictionary *displayControls = [NSMutableDictionary dictionary];
    for (NSControl *control in MSIMEFindPreferenceControls(prefs.window.contentView, @selector(helpcodeSchemaChanged:)))
        schemaControls[control.identifier] = control;
    for (NSControl *control in MSIMEFindPreferenceControls(prefs.window.contentView, @selector(helpcodeDisplayChanged:)))
        displayControls[control.identifier] = control;
    assert(schemaControls.count == 2 && displayControls.count == 2);
    NSDictionary *options = @{@"quanpin_helpcode": @{@"schema": @"shouyou2_0", @"show_in_candidate_window": @NO}, @"shuangpin_helpcode": @{@"schema": @"xiaohe", @"show_in_candidate_window": @YES}};
    [prefs applySharedAssistancePreferences:options];
    assert(saves == 3 && [defaults objectForKey:@"MSIMEClientHelpcodeOptions"] == nil);
    assert([(NSPopUpButton *)schemaControls[@"quanpin"] indexOfSelectedItem] == 2);
    assert([(NSButton *)displayControls[@"quanpin"] state] == NSControlStateValueOff);
    for (NSString *scheme in @[@"quanpin", @"shuangpin"]) {
        NSPopUpButton *schemas = schemaControls[scheme];
        NSButton *display = displayControls[scheme];
        NSArray *identifiers = @[@"lantian", @"ziranma", @"shouyou2_0", @"shouyouplus", @"xiaohe", @"jiajia"];
        assert(([schemas.itemTitles isEqual:@[@"蓝天小雨点", @"自然码", @"首右2.0", @"首右plus", @"小鹤", @"加加"]]));
        for (NSUInteger index = 0; index < identifiers.count; ++index) {
            [schemas selectItemAtIndex:index];
            [NSApp sendAction:schemas.action to:schemas.target from:schemas];
            display.state = index % 2 ? NSControlStateValueOn : NSControlStateValueOff;
            [NSApp sendAction:display.action to:display.target from:display];
            NSDictionary *merged = [prefs sharedPreferencesByMerging:options][[scheme stringByAppendingString:@"_helpcode"]];
            assert([merged[@"schema"] isEqual:identifiers[index]] && [merged[@"show_in_candidate_window"] boolValue] == (index % 2 == 1));
            MSIMEAppearancePreferences *restored = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
            assert([[restored helpcodeOptionsForScheme:scheme][@"schema"] isEqual:identifiers[index]]);
            assert([[restored helpcodeOptionsForScheme:scheme][@"show_in_candidate_window"] boolValue] == (index % 2 == 1));
        }
    }
    NSUInteger beforeRefresh = saves;
    [prefs applySharedAssistancePreferences:options];
    [prefs applySharedAssistancePreferences:@{@"quanpin_helpcode": @{@"schema": @"invalid", @"show_in_candidate_window": @1}}];
    assert(saves == beforeRefresh);
    assert([[prefs helpcodeOptionsForScheme:@"quanpin"] isEqual:options[@"quanpin_helpcode"]]);
    assert([[prefs helpcodeOptionsForScheme:@"shuangpin"] isEqual:options[@"shuangpin_helpcode"]]);
    NSButton *neighbor = (id)PreferenceControl(prefs, @selector(neighborChanged:));
    [prefs applySharedAssistancePreferences:@{@"autocorrect": @NO, @"quanpin": @{@"autocorrect_transposition": @YES, @"autocorrect_neighbor": @NO}}];
    assert(autocorrect.state == NSControlStateValueOn && neighbor.state == NSControlStateValueOff);
    assert(prefs.autocorrectTransposition && !prefs.autocorrectNeighbor);
    autocorrect.state = NSControlStateValueOff;
    [NSApp sendAction:autocorrect.action to:autocorrect.target from:autocorrect];
    neighbor.state = NSControlStateValueOn;
    [NSApp sendAction:neighbor.action to:neighbor.target from:neighbor];
    MSIMEAppearancePreferences *correctionRestored = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    assert(!correctionRestored.autocorrectTransposition && correctionRestored.autocorrectNeighbor);
    NSDictionary *correctionMerged = [prefs sharedPreferencesByMerging:@{@"quanpin": @{@"future": @7}}][@"quanpin"];
    assert([correctionMerged[@"autocorrect_transposition"] isEqual:@NO] && [correctionMerged[@"autocorrect_neighbor"] isEqual:@YES] && [correctionMerged[@"future"] isEqual:@7]);
    NSUInteger beforeCorrectionRefresh = saves;
    [prefs applySharedAssistancePreferences:@{@"autocorrect": @NO, @"quanpin": @{}}];
    assert(prefs.autocorrectTransposition && prefs.autocorrectNeighbor && saves == beforeCorrectionRefresh);
    assert([prefs sharedPreferencesByMerging:@{}][@"quanpin"][@"autocorrect_neighbor"] == NSNull.null);
    [prefs applySharedAssistancePreferences:@{@"quanpin": @{@"autocorrect_neighbor": @1}}];
    assert(prefs.autocorrectNeighbor);
    [NSNotificationCenter.defaultCenter removeObserver:observer];
    MSIMERemoveTestPreferenceSuite(defaults, suite);
}

static void TestSharedInputPreferences() {
    NSString *suite = [@"msime.shared-input." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    MSIMEAppearancePreferences *prefs = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    prefs.inputScheme = @"quanpin";
    prefs.shuangpinProfile = @"xiaohe";
    prefs.shuangpinPreeditUsesRaw = YES;
    ModeController *controller = [ModeController alloc];
    [controller setValue:prefs forKey:@"appearance"];
    __block NSUInteger saves = 0;
    id observer = [NSNotificationCenter.defaultCenter addObserverForName:MSIMEAppearanceDidChangeNotification object:prefs queue:nil usingBlock:^(NSNotification *note) { (void)note; ++saves; }];
    NSDictionary *shared = @{@"scheme": @"shuangpin", @"shuangpin_profile": @"microsoft", @"shuangpin_preedit_uses_raw": @NO};
    [controller applySharedToolbarPreferences:shared];
    assert([prefs.inputScheme isEqual:@"shuangpin"] && [prefs.shuangpinProfile isEqual:@"microsoft"] && !prefs.shuangpinPreeditUsesRaw);
    NSPopUpButton *profile = (id)PreferenceControl(prefs, @selector(profileChanged:));
    NSPopUpButton *preedit = (id)PreferenceControl(prefs, @selector(preeditChanged:));
    assert(SelectedSchemeIndex(prefs) == 1 && profile.indexOfSelectedItem == 3 && preedit.indexOfSelectedItem == 0);
    assert(saves == 0);
    assert([[defaults stringForKey:@"MSIMEClientInputScheme"] isEqual:@"quanpin"]);
    for (NSString *key in shared) assert([[prefs sharedPreferencesByMerging:shared][key] isEqual:shared[key]]);
    [controller applySharedToolbarPreferences:@{@"scheme": NSNull.null, @"shuangpin_profile": @42, @"shuangpin_preedit_uses_raw": @1}];
    [controller applySharedToolbarPreferences:@{}];
    assert([prefs.inputScheme isEqual:@"shuangpin"] && [prefs.shuangpinProfile isEqual:@"microsoft"] && !prefs.shuangpinPreeditUsesRaw && saves == 0);
    [controller applySharedToolbarPreferences:@{@"scheme": @"japanese"}];
    assert([prefs.inputScheme isEqual:@"japanese"] && SelectedSchemeIndex(prefs) == 3 && saves == 0);
    assert([[prefs sharedPreferencesByMerging:shared][@"scheme"] isEqual:@"japanese"]);
    [controller applySharedToolbarPreferences:shared];
    assert([prefs.inputScheme isEqual:@"shuangpin"] && SelectedSchemeIndex(prefs) == 1);
    SelectScheme(prefs, 2);
    [profile selectItemAtIndex:2];
    [NSApp sendAction:profile.action to:profile.target from:profile];
    [preedit selectItemAtIndex:1];
    [NSApp sendAction:preedit.action to:preedit.target from:preedit];
    NSDictionary *edited = [prefs sharedPreferencesByMerging:shared];
    assert([edited[@"scheme"] isEqual:@"wubi"] && [edited[@"shuangpin_profile"] isEqual:@"shoudao"] && [edited[@"shuangpin_preedit_uses_raw"] isEqual:@YES] && saves == 3);
    [controller applySharedToolbarPreferences:shared];
    assert(SelectedSchemeIndex(prefs) == 1 && profile.indexOfSelectedItem == 3 && preedit.indexOfSelectedItem == 0 && saves == 3);
    NSPopUpButton *layout = (id)PreferenceControl(prefs, @selector(layoutChanged:));
    NSPopUpButton *font = (id)PreferenceControl(prefs, @selector(fontChanged:));
    NSPopUpButton *page = (id)PreferenceControl(prefs, @selector(pageSizeChanged:));
    // Six is in here because it is the shared default, and this platform used to rewrite it to nine.
    for (NSUInteger size = 12; size <= 32; ++size) {
        for (NSNumber *count in @[@5, @6, @7, @9]) {
            NSDictionary *candidate = @{@"candidate_layout": count.integerValue == 7 ? @"horizontal" : @"vertical", @"candidate_font_size": @(size), @"candidate_page_size": count};
            [controller applySharedToolbarPreferences:candidate];
            assert(prefs.vertical == (count.integerValue != 7) && prefs.fontSize == size && prefs.pageSize == count.unsignedIntegerValue);
            assert(layout.indexOfSelectedItem == (NSInteger)(count.integerValue == 7 ? 0 : 1) && font.indexOfSelectedItem == (NSInteger)size - 12 && page.indexOfSelectedItem == (NSInteger)msime::mac::CandidatePageSizeOptionIndex(count.unsignedIntegerValue));
            for (NSString *key in candidate) assert([[prefs sharedPreferencesByMerging:candidate][key] isEqual:candidate[key]]);
        }
    }
    assert(saves == 3 && [defaults objectForKey:@"MSIMEClientCandidateFontSize"] == nil);
    for (id invalid in @[NSNull.null, @YES, @0, @99, @1.5, @"18"]) {
        [controller applySharedToolbarPreferences:@{@"candidate_layout": invalid, @"candidate_font_size": invalid, @"candidate_page_size": invalid}];
        assert(prefs.vertical && prefs.fontSize == 32 && prefs.pageSize == 9 && saves == 3);
    }
    [layout selectItemAtIndex:0];
    [NSApp sendAction:layout.action to:layout.target from:layout];
    [font selectItemAtIndex:1];
    [NSApp sendAction:font.action to:font.target from:font];
    [page selectItemAtIndex:1];
    [NSApp sendAction:page.action to:page.target from:page];
    // The page-size control lists the reference's three through nine, so the second item is four.
    assert(!prefs.vertical && prefs.fontSize == 13 && prefs.pageSize == 4 && saves == 6);
    NSDictionary *candidateEdited = [prefs sharedPreferencesByMerging:@{}];
    assert([candidateEdited[@"candidate_layout"] isEqual:@"horizontal"] && [candidateEdited[@"candidate_font_size"] isEqual:@13] && [candidateEdited[@"candidate_page_size"] isEqual:@4]);
    [NSNotificationCenter.defaultCenter removeObserver:observer];
    MSIMERemoveTestPreferenceSuite(defaults, suite);
}

static void TestKeymap(NSUserDefaults *defaults, MSIMEAppearancePreferences *appearance) {
    NSPopUpButton *profiles = (id)PreferenceControl(appearance, @selector(profileChanged:));
    NSArray *identifiers = @[@"xiaohe", @"ziranma", @"shoudao", @"microsoft"];
    NSArray *titles = @[@"小鹤双拼", @"自然码双拼", @"首道双拼", @"微软双拼"];
    assert([profiles.itemTitles isEqual:titles]);
    for (NSUInteger index = 0; index < identifiers.count; ++index) {
        [profiles selectItemAtIndex:index];
        assert([NSApp sendAction:profiles.action to:profiles.target from:profiles]);
        assert([appearance.shuangpinProfile isEqual:identifiers[index]]);
        MSIMEAppearancePreferences *restored = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults skinsRoot:appearance.skinsRoot];
        assert([restored.shuangpinProfile isEqual:identifiers[index]]);
        NSPopUpButton *restoredProfiles = (id)PreferenceControl(restored, @selector(profileChanged:));
        assert([restoredProfiles.titleOfSelectedItem isEqual:titles[index]]);
    }
    assert(!appearance.shuangpinKeymap);
    NSButton *toggle = (id)PreferenceControl(appearance, @selector(keymapChanged:));
    toggle.state = NSControlStateValueOn;
    [NSApp sendAction:toggle.action to:toggle.target from:toggle];
    assert(appearance.shuangpinKeymap);
    assert([[[MSIMEAppearancePreferences alloc] initWithDefaults:defaults skinsRoot:appearance.skinsRoot] shuangpinKeymap]);
    HiddenKeymapPanel *panel = [[HiddenKeymapPanel alloc] init];
    assert(panel.ignoresMouseEvents && panel.floatingPanel);
    for (NSString *profile in @[@"xiaohe", @"ziranma", @"shoudao", @"microsoft"]) {
        NSArray *rows = MSIMEShuangpinKeymapRows(profile);
        assert(rows.count == 3 && [rows[0] count] == 10 && [rows[2] count] == 7);
        assert([rows[1] count] == ([profile isEqual:@"microsoft"] ? 10 : 9));
        [panel setProfileName:profile];
        [panel.contentView layoutSubtreeIfNeeded];
        assert([panel.contentView.accessibilityLabel containsString:@"键位提示"]);
        assert([MSIMEShuangpinZeroInitialText(profile) containsString:@"零声母"]);
        if ([profile isEqual:@"microsoft"]) {
            assert([rows[1][9][@"key"] isEqual:@";"]);
            assert([rows[1][9][@"codes"] containsString:@"ing"]);
        }
        [panel updateHighlightedKey:@"a"];
        assert([panel.contentView.accessibilityValue containsString:@"当前按键 A"]);
        for (NSAppearanceName theme in @[NSAppearanceNameAqua, NSAppearanceNameDarkAqua]) {
            panel.contentView.appearance = [NSAppearance appearanceNamed:theme];
            NSBitmapImageRep *bitmap = [panel.contentView bitmapImageRepForCachingDisplayInRect:panel.contentView.bounds];
            assert(bitmap != nil);
            [panel.contentView cacheDisplayInRect:panel.contentView.bounds toBitmapImageRep:bitmap];
            assert(bitmap.pixelsWide >= 620 && bitmap.pixelsHigh >= 203);
            NSString *directory = NSProcessInfo.processInfo.environment[@"MSIME_KEYMAP_SNAPSHOT_DIR"];
            if (directory) {
                NSString *file = [directory stringByAppendingPathComponent:[NSString stringWithFormat:@"%@-%@.png", profile, theme]];
                assert([[bitmap representationUsingType:NSBitmapImageFileTypePNG properties:@{}] writeToFile:file atomically:YES]);
            }
        }
        [panel updateHighlightedKey:@""];
        assert(![panel.contentView.accessibilityValue containsString:@"当前按键"]);
    }
    NSRect frame = MSIMEShuangpinKeymapPanelFrame(NSMakeRect(30, 400, 1, 20), NSMakeSize(620, 203), 60, NSMakeRect(0, 0, 1000, 800));
    assert(frame.origin.x == 30 && frame.origin.y == 129);
    frame = MSIMEShuangpinKeymapPanelFrame(NSMakeRect(990, 20, 1, 20), NSMakeSize(620, 203), 60, NSMakeRect(0, 0, 1000, 800));
    assert(frame.origin.x == 364 && frame.origin.y == 108);
    ModeController *controller = [ModeController alloc];
    ShortcutClient *client = [ShortcutClient new];
    client.caret = NSMakeRect(20, 400, 1, 20);
    [controller setValue:appearance forKey:@"appearance"];
    [controller setValue:client forKey:@"activeClient"];
    [controller setValue:panel forKey:@"keymapPanel"];
    [controller setValue:[ShortcutSession new] forKey:@"session"];
    NSDictionary *view = @{@"scheme": @1, @"local_mode": @"none", @"dedicated_english": @NO, @"shuangpin_profile": @"microsoft", @"editing_text": @"b;", @"preedit": @"bing", @"candidates": @[]};
    [controller setValue:view forKey:@"view"];
    [controller updateKeymapPanel];
    assert(panel.requestedVisible && [panel.contentView.accessibilityValue containsString:@"当前按键 ;"]);
    assert(panel.clearance == (appearance.vertical ? 24 : appearance.fontSize + 42));
    NSMutableDictionary *withCandidates = [view mutableCopy];
    withCandidates[@"candidates"] = @[@{@"text": @"合成"}];
    [controller setValue:withCandidates forKey:@"view"];
    appearance.showsCandidatePreedit = NO;
    [controller updateKeymapPanel];
    CGFloat hiddenClearance = panel.clearance;
    appearance.showsCandidatePreedit = YES;
    [controller updateKeymapPanel];
    assert(panel.clearance > hiddenClearance);
    NSUInteger savedFontSize = appearance.fontSize;
    BOOL savedVertical = appearance.vertical;
    NSString *savedFamily = appearance.fontFamily;
    appearance.fontFamily = @"Helvetica";
    appearance.fontSize = 32;
    appearance.vertical = YES;
    appearance.showsCandidatePreedit = NO;
    [controller updateKeymapPanel];
    NSFont *clearanceFont = [appearance candidateFontOfSize:32];
    CGFloat measuredHeight = ceil([@"合成" sizeWithAttributes:@{NSFontAttributeName:clearanceFont}].height);
    assert(panel.clearance >= measuredHeight + 12 + 24);
    appearance.fontSize = savedFontSize;
    appearance.fontFamily = savedFamily;
    appearance.vertical = savedVertical;
    appearance.showsCandidatePreedit = YES;
    for (NSString *display in @[@"b;", @"bing", @""]) {
        NSMutableDictionary *next = [view mutableCopy];
        next[@"preedit"] = display;
        [controller setValue:next forKey:@"view"];
        [controller updateKeymapPanel];
        assert(panel.requestedVisible && [panel.contentView.accessibilityValue containsString:@"当前按键 ;"]);
    }
    for (id mode in @[@"unicode", @"date_time", @"quick_phrase", @"emoji", @"kaomoji", @"super_jianpin", @"temporary_english", @"temporary_japanese", @"unknown", @"", NSNull.null, @1]) {
        NSMutableDictionary *next = [view mutableCopy];
        next[@"local_mode"] = mode;
        [controller setValue:next forKey:@"view"];
        [controller updateKeymapPanel];
        assert(!panel.requestedVisible);
        [controller setValue:view forKey:@"view"];
        [controller updateKeymapPanel];
        assert(panel.requestedVisible && [panel.contentView.accessibilityValue containsString:@"当前按键 ;"]);
    }
    NSMutableDictionary *missingMode = [view mutableCopy];
    missingMode[@"dedicated_english"] = @YES;
    [controller setValue:missingMode forKey:@"view"];
    [controller updateKeymapPanel];
    assert(!panel.requestedVisible);
    [controller setValue:view forKey:@"view"];
    [controller updateKeymapPanel];
    assert(panel.requestedVisible);
    missingMode[@"dedicated_english"] = @NO;
    [missingMode removeObjectForKey:@"local_mode"];
    [controller setValue:missingMode forKey:@"view"];
    [controller updateKeymapPanel];
    assert(!panel.requestedVisible);
    for (NSDictionary *excluded in @[@{}, @{@"scheme": @0}, @{@"scheme": @3}, @{@"editing_text": @""}, @{@"editing_text": NSNull.null}, @{@"editing_text": @42}, @{@"shuangpin_profile": @""}]) {
        NSMutableDictionary *next = [view mutableCopy];
        [next addEntriesFromDictionary:excluded];
        if (!excluded.count) [next removeObjectForKey:@"scheme"];
        [controller setValue:next forKey:@"view"];
        [controller updateKeymapPanel];
        assert(!panel.requestedVisible);
    }
    [controller setValue:view forKey:@"view"];
    client.caret = NSZeroRect;
    [controller updateKeymapPanel];
    assert(!panel.requestedVisible);
    client.caret = NSMakeRect(20, 400, 1, 20);
    appearance.englishMode = YES;
    [controller updateKeymapPanel];
    assert(!panel.requestedVisible);
    appearance.englishMode = NO;
    appearance.shuangpinKeymap = NO;
    [controller updateKeymapPanel];
    assert(!panel.requestedVisible && toggle.state == NSControlStateValueOff);
}

static void TestPunctuation(NSUserDefaults *defaults, MSIMEAppearancePreferences *appearance) {
    assert(appearance.chinesePunctuation);
    ModeController *controller = [ModeController alloc];
    ShortcutClient *client = [ShortcutClient new];
    ShortcutSession *session = [ShortcutSession new];
    [controller setValue:appearance forKey:@"appearance"];
    [controller setValue:client forKey:@"activeClient"];
    [controller setValue:session forKey:@"session"];
    [controller setValue:@{@"editing_text": @"test", @"candidates": @[]} forKey:@"view"];
    const BOOL english = appearance.englishMode;
    const BOOL fullWidth = appearance.fullWidthInput;
    const BOOL traditional = appearance.traditionalOutput;
    session.failFinish = YES;
    [controller floatingToolbarDidRequestTogglePunctuation:nil];
    assert(appearance.runtimeChinesePunctuation && session.punctuationCalls == 0);
    session.failFinish = NO;
    [controller floatingToolbarDidRequestTogglePunctuation:nil];
    assert(session.lastCommand == MSIME_FINISH_COMPOSITION);
    assert([client.committed isEqual:@"测试"]);
    assert(!appearance.runtimeChinesePunctuation && !session.chinesePunctuation);
    assert(appearance.chinesePunctuation && [defaults objectForKey:@"MSIMEClientChinesePunctuation"] == nil);
    assert([[[MSIMEAppearancePreferences alloc] initWithDefaults:defaults skinsRoot:appearance.skinsRoot] chinesePunctuation]);
    session.chinesePunctuation = YES;
    [controller prepareSession];
    assert(!session.chinesePunctuation);
    assert(appearance.englishMode == english && appearance.fullWidthInput == fullWidth && appearance.traditionalOutput == traditional);
    [controller floatingToolbarDidRequestTogglePunctuation:nil];
    assert(appearance.runtimeChinesePunctuation && session.chinesePunctuation);
    NSEvent *(^toggleEvent)(unsigned short, NSEventModifierFlags, BOOL) = ^NSEvent *(unsigned short key, NSEventModifierFlags flags, BOOL repeat) {
        return [NSEvent keyEventWithType:NSEventTypeKeyDown location:NSZeroPoint modifierFlags:flags timestamp:0 windowNumber:0 context:nil characters:@"." charactersIgnoringModifiers:@"." isARepeat:repeat keyCode:key];
    };
    NSEvent *toggle = toggleEvent(47, NSEventModifierFlagControl, NO);
    [controller setValue:@{@"editing_text":@"test", @"candidates":@[]} forKey:@"view"];
    session.failFinish = YES;
    NSUInteger calls = session.punctuationCalls;
    assert([controller handleEvent:toggle client:client]);
    assert(appearance.runtimeChinesePunctuation && session.punctuationCalls == calls);
    session.failFinish = NO;
    assert([controller handleEvent:toggle client:client]);
    assert(!appearance.runtimeChinesePunctuation && !session.chinesePunctuation && session.punctuationCalls > calls);
    assert(session.lastCommand == MSIME_FINISH_COMPOSITION && [client.committed isEqual:@"测试"]);
    assert(appearance.chinesePunctuation && [defaults objectForKey:@"MSIMEClientChinesePunctuation"] == nil);
    assert([[[MSIMEAppearancePreferences alloc] initWithDefaults:defaults skinsRoot:appearance.skinsRoot] chinesePunctuation]);
    calls = session.punctuationCalls;
    assert([controller handleEvent:toggleEvent(47, NSEventModifierFlagControl, YES) client:client]);
    assert(!appearance.runtimeChinesePunctuation && session.punctuationCalls == calls);
    for (NSNumber *flags in @[@0, @(NSEventModifierFlagCommand), @(NSEventModifierFlagControl | NSEventModifierFlagShift), @(NSEventModifierFlagControl | NSEventModifierFlagOption), @(NSEventModifierFlagControl | NSEventModifierFlagCommand)])
        assert(!MSIMEPunctuationToggle(toggleEvent(47, flags.unsignedIntegerValue, NO)));
    assert(!MSIMEPunctuationToggle(toggleEvent(65, NSEventModifierFlagControl, NO))); // Keypad decimal.
    // The punctuation preference can be changed while English passthrough is active.
    appearance.englishMode = YES;
    assert([controller handleEvent:toggle client:client]);
    assert(appearance.englishMode && appearance.runtimeChinesePunctuation && session.chinesePunctuation);
    appearance.englishMode = english;
    assert(appearance.fullWidthInput == fullWidth && appearance.traditionalOutput == traditional);
    // The punctuation API returns a bare View, not {view: ...} like key input.
    NSDictionary *punctuationView = @{@"session":@9, @"generation":@4, @"focused":@YES,
        @"editing_text":@"nini", @"preedit":@"ni'ni", @"caret_position":@2,
        @"dedicated_english":@YES, @"candidates":@[]};
    session.punctuationView = punctuationView;
    // The inline preedit style decides whether the client is marked with the segmented preedit or the raw
    // editing text, and it defaults to raw - so the distinct preedit in this fixture was never what reached
    // the client. Ask for the style this assertion is about instead of inheriting whatever ran before.
    [appearance applySharedInputPreferences:@{@"tsf_preedit_style":@"pinyin"}];
    NSString *previousCommit = client.committed;
    session.lastCommand = UINT32_MAX;
    [controller syncPunctuation];
    assert([[controller valueForKey:@"view"] isEqual:punctuationView]);
    assert([client.marked isEqual:@"ni'ni"] && [client.committed isEqual:previousCommit]);
    assert(session.lastCommand == UINT32_MAX);
    session.punctuationView = nil;
    [controller syncPunctuation];
    assert([[controller valueForKey:@"view"] isEqual:punctuationView]);
}

static void TestPairedPunctuationPreferences() {
    NSString *suite = [@"msime.paired-punctuation." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    MSIMEAppearancePreferences *preferences = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    assert(preferences.pairedPunctuation && [preferences.punctuationLock isEqual:@"follow"]);
    NSDictionary *initial = [preferences sharedPreferencesByMerging:@{}];
    assert([initial[@"paired_punctuation"] isEqual:@YES] && [initial[@"punctuation_lock"] isEqual:@"follow"]);
    [preferences applySharedInputPreferences:@{ @"paired_punctuation": @NO, @"punctuation_lock": @"english" }];
    assert(!preferences.pairedPunctuation && [preferences.punctuationLock isEqual:@"english"]);
    assert([defaults objectForKey:@"MSIMEClientPairedPunctuation"] == nil && [defaults objectForKey:@"MSIMEClientPunctuationLock"] == nil);
    [preferences applySharedInputPreferences:@{ @"paired_punctuation": @1, @"punctuation_lock": @"invalid" }];
    assert(!preferences.pairedPunctuation && [preferences.punctuationLock isEqual:@"english"]);
    NSButton *paired = (id)PreferenceControl(preferences, @selector(pairedPunctuationChanged:));
    NSPopUpButton *lock = (id)PreferenceControl(preferences, @selector(punctuationLockChanged:));
    assert(paired.state == NSControlStateValueOff && lock.indexOfSelectedItem == 2);
    paired.state = NSControlStateValueOn;
    [NSApp sendAction:paired.action to:paired.target from:paired];
    [lock selectItemAtIndex:1];
    [NSApp sendAction:lock.action to:lock.target from:lock];
    assert(preferences.pairedPunctuation && [preferences.punctuationLock isEqual:@"chinese"]);
    assert([defaults boolForKey:@"MSIMEClientPairedPunctuation"] && [[defaults stringForKey:@"MSIMEClientPunctuationLock"] isEqual:@"chinese"]);
    MSIMEAppearancePreferences *reopened = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    assert(reopened.pairedPunctuation && [reopened.punctuationLock isEqual:@"chinese"]);
    [preferences applySharedInputPreferences:@{ @"paired_punctuation": @NO, @"punctuation_lock": @"follow" }];
    assert(!preferences.pairedPunctuation && [preferences.punctuationLock isEqual:@"follow"]);
    ShortcutSession *session = [ShortcutSession new];
    ShortcutClient *client = [ShortcutClient new];
    MSIMEInputController *controller = [MSIMEInputController alloc];
    [controller setValue:preferences forKey:@"appearance"];
    [controller setValue:session forKey:@"session"];
    [controller setValue:client forKey:@"activeClient"];
    [controller setValue:@{ @"editing_text": @"", @"candidates": @[] } forKey:@"view"];
    session.punctuationView = @{ @"editing_text": @"", @"candidates": @[] };
    [controller syncPunctuation];
    assert(!session.pairedPunctuation && session.punctuationLock == 0 && session.pairedPunctuationCalls == 1 && session.punctuationLockCalls == 1);
    MSIMERemoveTestPreferenceSuite(defaults, suite);
}

static void TestPairedPunctuationHostExclusion() {
    assert(MSIMEPairedPunctuationExcludedBundleIdentifier(@"com.microsoft.Excel"));
    assert(MSIMEPairedPunctuationExcludedBundleIdentifier(@"COM.MICROSOFT.EXCEL"));
    assert(!MSIMEPairedPunctuationExcludedBundleIdentifier(@"com.apple.TextEdit"));
    assert(!MSIMEPairedPunctuationExcludedBundleIdentifier(nil));
}

static void TestPairedPunctuationClosesThePair() {
    // The Engine commits the opening mark alone - the Windows TIP and the Linux host each append
    // their own closing mark, and this host used to append nothing at all, so the switch was on and
    // the page promised a pair that never arrived. IMK cannot move the client's caret, so the
    // closing mark rides in the marked text after it until the composition ends.
    NSString *suite = [@"app.msime.test.paired." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    MSIMEAppearancePreferences *appearance = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    assert(appearance.pairedPunctuation);
    ModeController *controller = [ModeController alloc];
    ShortcutClient *client = [ShortcutClient new];
    client.document = @"";
    [controller setValue:appearance forKey:@"appearance"];
    [controller setValue:client forKey:@"activeClient"];

    [controller apply:@{@"commit": @"（", @"view": @{@"editing_text": @"", @"caret_position": @0}}];
    assert([client.committed isEqual:@"（"]);
    assert([client.marked isEqual:@"）"]);

    // Composing inside the pair keeps the closing mark visible and after the caret.
    [controller apply:@{@"commit": NSNull.null, @"view": @{@"editing_text": @"ni", @"caret_position": @2}}];
    assert([client.marked isEqual:@"ni）"]);

    // Committing takes the closing mark with it, and the pair is done.
    [controller apply:@{@"commit": @"你好", @"view": @{@"editing_text": @"", @"caret_position": @0}}];
    assert([client.committed isEqual:@"你好）"]);
    [controller apply:@{@"commit": @"吗", @"view": @{@"editing_text": @"", @"caret_position": @0}}];
    assert([client.committed isEqual:@"吗"]);

    // A pair left open when the composition is torn down is closed rather than dropped.
    [controller apply:@{@"commit": @"【", @"view": @{@"editing_text": @"", @"caret_position": @0}}];
    assert([client.marked isEqual:@"】"]);
    [controller flushPendingPairedClosing];
    assert([client.committed isEqual:@"】"]);
    [controller flushPendingPairedClosing];
    assert([client.committed isEqual:@"】"]);

    // Book title marks nest, so a pair this host closed has to be reported to the Engine: it counts
    // how many 《 are open and answers 〈 inside one, and the `>` that would unwind the count is
    // never typed here. Without the call the next pair the user opens comes back 〈〉.
    ShortcutSession *session = [ShortcutSession new];
    [controller setValue:session forKey:@"session"];
    [controller apply:@{@"commit": @"（", @"view": @{@"editing_text": @"", @"caret_position": @0}}];
    assert(session.balanceCalls == 0);
    [controller flushPendingPairedClosing];
    [controller apply:@{@"commit": @"《", @"view": @{@"editing_text": @"", @"caret_position": @0}}];
    assert([client.marked isEqual:@"》"]);
    assert(session.balanceCalls == 1 && session.lastBalanceOpening == '<');
    [controller flushPendingPairedClosing];
    // The inner half of the same nesting comes from the same key and unwinds the same count.
    [controller apply:@{@"commit": @"〈", @"view": @{@"editing_text": @"", @"caret_position": @0}}];
    assert([client.marked isEqual:@"〉"]);
    assert(session.balanceCalls == 2 && session.lastBalanceOpening == '<');
    [controller flushPendingPairedClosing];

    // A quote key alternates in the Engine because a host without pairing needs it to. This host
    // supplies the closing half, so the press that would have flipped it back never happens: with
    // pairing on, a closing quote is read as the start of a fresh pair.
    [controller apply:@{@"commit": @"”", @"view": @{@"editing_text": @"", @"caret_position": @0}}];
    assert([client.committed isEqual:@"“"]);
    assert([client.marked isEqual:@"”"]);
    [controller flushPendingPairedClosing];
    [controller apply:@{@"commit": @"’", @"view": @{@"editing_text": @"", @"caret_position": @0}}];
    assert([client.committed isEqual:@"‘"] && [client.marked isEqual:@"’"]);
    [controller flushPendingPairedClosing];

    // A punctuation key typed during a composition finishes it, and the Engine commits the candidate and the mark as one string. The pair is read from the last character of that commit, the way the reference reads `punctuationStr.back()`.
    [controller setValue:@('(') forKey:@"punctuationKeyInFlight"];
    [controller apply:@{@"commit": @"你好（", @"view": @{@"editing_text": @"", @"caret_position": @0}}];
    assert([client.committed isEqual:@"你好（"]);
    assert([client.marked isEqual:@"）"]);
    [controller apply:@{@"commit": @"吗", @"view": @{@"editing_text": @"", @"caret_position": @0}}];
    assert([client.committed isEqual:@"吗）"]);
    // The quote toggle is reopened on the last character only, and only for a quote key.
    [controller setValue:@('"') forKey:@"punctuationKeyInFlight"];
    [controller apply:@{@"commit": @"你好”", @"view": @{@"editing_text": @"", @"caret_position": @0}}];
    assert([client.committed isEqual:@"你好“"] && [client.marked isEqual:@"”"]);
    [controller flushPendingPairedClosing];
    const NSUInteger balancedBeforeTail = session.balanceCalls;
    [controller setValue:@('<') forKey:@"punctuationKeyInFlight"];
    [controller apply:@{@"commit": @"你好《", @"view": @{@"editing_text": @"", @"caret_position": @0}}];
    assert([client.committed isEqual:@"你好《"] && [client.marked isEqual:@"》"]);
    assert(session.balanceCalls == balancedBeforeTail + 1 && session.lastBalanceOpening == '<');
    [controller flushPendingPairedClosing];
    // A candidate or phrase that merely ends in a mark is not a punctuation key: it is committed as it is and nothing is owed.
    [controller apply:@{@"commit": @"“好”", @"view": @{@"editing_text": @"", @"caret_position": @0}}];
    assert([client.committed isEqual:@"“好”"] && client.marked.length == 0);
    [controller apply:@{@"commit": @"你好（", @"view": @{@"editing_text": @"", @"caret_position": @0}}];
    assert([client.committed isEqual:@"你好（"] && client.marked.length == 0);
    // The key is consumed by the apply it belonged to, so it never reaches a later candidate commit.
    [controller setValue:@('(') forKey:@"punctuationKeyInFlight"];
    [controller apply:@{@"commit": NSNull.null, @"view": @{@"editing_text": @"ni", @"caret_position": @2}}];
    [controller apply:@{@"commit": @"你好（", @"view": @{@"editing_text": @"", @"caret_position": @0}}];
    assert([client.committed isEqual:@"你好（"] && client.marked.length == 0);

    // With the preference off nothing is owed, the commit is what the Engine said, and the Engine
    // is told nothing - its own alternation and nesting are what a host without pairing wants.
    appearance.pairedPunctuation = NO;
    const NSUInteger balancedSoFar = session.balanceCalls;
    [controller apply:@{@"commit": @"（", @"view": @{@"editing_text": @"", @"caret_position": @0}}];
    assert([client.committed isEqual:@"（"] && client.marked.length == 0);
    [controller apply:@{@"commit": @"《", @"view": @{@"editing_text": @"", @"caret_position": @0}}];
    assert([client.committed isEqual:@"《"] && client.marked.length == 0);
    [controller apply:@{@"commit": @"”", @"view": @{@"editing_text": @"", @"caret_position": @0}}];
    assert([client.committed isEqual:@"”"]);
    [controller setValue:@('(') forKey:@"punctuationKeyInFlight"];
    [controller apply:@{@"commit": @"你好（", @"view": @{@"editing_text": @"", @"caret_position": @0}}];
    assert([client.committed isEqual:@"你好（"] && client.marked.length == 0);
    assert(session.balanceCalls == balancedSoFar);
    MSIMERemoveTestPreferenceSuite(defaults, suite);
}

static void TestPairedPunctuationClosesBrace() {
    // The reference routes `{` through its punctuation path and appends `}` (`_GetPairedPunctuationClosingFor`), with or without a live composition. The Engine answers `{` as ASCII, so the host opens the pair itself and the closing mark rides after the caret like the other pairs.
    NSString *suite = [@"app.msime.test.paired-brace." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    MSIMEAppearancePreferences *appearance = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    assert(appearance.pairedPunctuation && appearance.runtimeChinesePunctuation && !appearance.runtimeFullWidthInput);
    ModeController *controller = [ModeController alloc];
    ShortcutSession *session = [ShortcutSession new];
    ShortcutClient *client = [ShortcutClient new];
    client.document = @"";
    [controller setValue:appearance forKey:@"appearance"];
    [controller setValue:session forKey:@"session"];
    [controller setValue:client forKey:@"activeClient"];
    NSDictionary *idle = @{ @"focused": @YES, @"editing_text": @"", @"candidates": @[] };
    NSEvent *brace = [NSEvent keyEventWithType:NSEventTypeKeyDown location:NSZeroPoint modifierFlags:NSEventModifierFlagShift
                                      timestamp:0 windowNumber:0 context:nil characters:@"{" charactersIgnoringModifiers:@"["
                                      isARepeat:NO keyCode:33];
    NSEvent *closeBrace = [NSEvent keyEventWithType:NSEventTypeKeyDown location:NSZeroPoint modifierFlags:NSEventModifierFlagShift
                                           timestamp:0 windowNumber:0 context:nil characters:@"}" charactersIgnoringModifiers:@"]"
                                           isARepeat:NO keyCode:30];

    // Idle: the Engine leaves `{` alone, so the host commits it and owes `}`.
    [controller setValue:idle forKey:@"view"];
    session.punctuationASCIITransition = @{ @"handled": @NO, @"commit": NSNull.null, @"view": idle };
    assert([controller handleEvent:brace client:client]);
    assert(session.punctuationASCIICalls == 1 && session.lastPunctuationASCII == '{' && session.asciiCalls == 0);
    assert([client.committed isEqual:@"{"] && [client.marked isEqual:@"}"]);

    // The next commit takes the closing mark with it, and a `}` typed before it is stepped over.
    [controller apply:@{ @"commit": @"a", @"view": idle }];
    assert([client.committed isEqual:@"a}"] && [client.document isEqual:@"{a}"]);
    client.selection = NSMakeRange(2, 0);
    const NSUInteger asciiBeforeStep = session.asciiCalls;
    assert([controller handleEvent:closeBrace client:client]);
    assert(session.asciiCalls == asciiBeforeStep && [client.document isEqual:@"{a}"] && client.selection.location == 3);

    // Composing: the Engine commits the highlighted candidate followed by `{`, and the pair is still opened.
    client.document = @"";
    client.selection = NSMakeRange(0, 0);
    [controller setValue:@{ @"focused": @YES, @"editing_text": @"ni", @"candidates": @[] } forKey:@"view"];
    session.punctuationASCIITransition = @{ @"handled": @YES, @"commit": @"你{", @"view": idle };
    assert([controller handleEvent:brace client:client]);
    assert([client.committed isEqual:@"你{"] && [client.marked isEqual:@"}"] && session.asciiCalls == asciiBeforeStep);
    [controller flushPendingPairedClosing];
    assert([client.committed isEqual:@"}"]);

    // Full-width input pairs the full-width marks, idle and composing alike.
    appearance.runtimeFullWidthInput = YES;
    session.punctuationASCIITransition = @{ @"handled": @NO, @"commit": NSNull.null, @"view": idle };
    [controller setValue:idle forKey:@"view"];
    assert([controller handleEvent:brace client:client]);
    assert([client.committed isEqual:@"｛"] && [client.marked isEqual:@"｝"]);
    [controller flushPendingPairedClosing];
    [controller setValue:@{ @"focused": @YES, @"editing_text": @"ni", @"candidates": @[] } forKey:@"view"];
    session.punctuationASCIITransition = @{ @"handled": @YES, @"commit": @"你{", @"view": idle };
    assert([controller handleEvent:brace client:client]);
    assert([client.committed isEqual:@"你｛"] && [client.marked isEqual:@"｝"]);
    [controller flushPendingPairedClosing];
    appearance.runtimeFullWidthInput = NO;

    // With pairing or Chinese punctuation off, `{` is an ordinary key for the Engine and nothing is owed.
    session.punctuationASCIITransition = nil;
    session.nextTransition = @{ @"handled": @YES, @"commit": @"{", @"view": idle };
    for (NSUInteger variant = 0; variant < 2; ++variant) {
        appearance.pairedPunctuation = variant != 0;
        appearance.runtimeChinesePunctuation = variant == 0;
        [controller setValue:idle forKey:@"view"];
        client.marked = nil;
        session.asciiCalls = 0;
        const NSUInteger punctuationCalls = session.punctuationASCIICalls;
        assert([controller handleEvent:brace client:client]);
        assert(session.asciiCalls == 1 && session.lastASCII == '{' && session.punctuationASCIICalls == punctuationCalls);
        assert([client.committed isEqual:@"{"] && client.marked.length == 0);
    }
    MSIMERemoveTestPreferenceSuite(defaults, suite);
}

static void TestEmojiBridgeFallback() {
    ModeController *controller = [ModeController alloc];
    [controller showEmoji:nil];
    // The standalone input source can outlive the Swift/Tauri bridge during
    // launch or bundle repair; the menu must still open macOS Character Viewer.
    assert(controller.paletteCalls == 1);
}

static void TestMixedInputPreferences() {
    NSString *suite = [@"msime.mixed-input." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    MSIMEAppearancePreferences *preferences = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    // Emoji mixed-input starts on like the source's `emoji_mixed_input = true`; kaomoji starts off.
    assert(preferences.mixedEnglishInput && preferences.mixedEnglishMinimumPrefix == 5 &&
           preferences.mixedEmojiInput && !preferences.mixedKaomojiInput);
    NSDictionary *initial = [preferences sharedPreferencesByMerging:@{}][@"mixed_input"];
    assert([initial[@"english"] isEqual:@YES] && [initial[@"minimum_prefix"] isEqual:@5] &&
           [initial[@"emoji"] isEqual:@YES] && [initial[@"kaomoji"] isEqual:@NO]);
    [preferences applySharedInputPreferences:@{ @"mixed_input": @{
        @"english": @NO, @"minimum_prefix": @7, @"emoji": @YES, @"kaomoji": @YES } }];
    assert(!preferences.mixedEnglishInput && preferences.mixedEnglishMinimumPrefix == 7 &&
           preferences.mixedEmojiInput && preferences.mixedKaomojiInput);
    assert([defaults objectForKey:@"MSIMEClientMixedInput"] == nil);
    [preferences applySharedInputPreferences:@{ @"mixed_input": @{
        @"english": @1, @"minimum_prefix": @9, @"emoji": @"true", @"kaomoji": NSNull.null } }];
    assert(!preferences.mixedEnglishInput && preferences.mixedEnglishMinimumPrefix == 7 &&
           preferences.mixedEmojiInput && preferences.mixedKaomojiInput);
    NSButton *english = (id)PreferenceControl(preferences, @selector(mixedEnglishChanged:));
    NSPopUpButton *prefix = (id)PreferenceControl(preferences, @selector(mixedEnglishPrefixChanged:));
    NSButton *emoji = (id)PreferenceControl(preferences, @selector(mixedEmojiChanged:));
    NSButton *kaomoji = (id)PreferenceControl(preferences, @selector(mixedKaomojiChanged:));
    assert(english.state == NSControlStateValueOff && prefix.indexOfSelectedItem == 6 && !prefix.enabled &&
           emoji.state == NSControlStateValueOn && kaomoji.state == NSControlStateValueOn);
    english.state = NSControlStateValueOn;
    [NSApp sendAction:english.action to:english.target from:english];
    [prefix selectItemAtIndex:4];
    [NSApp sendAction:prefix.action to:prefix.target from:prefix];
    emoji.state = NSControlStateValueOff;
    [NSApp sendAction:emoji.action to:emoji.target from:emoji];
    kaomoji.state = NSControlStateValueOff;
    [NSApp sendAction:kaomoji.action to:kaomoji.target from:kaomoji];
    assert(preferences.mixedEnglishInput && preferences.mixedEnglishMinimumPrefix == 5 &&
           !preferences.mixedEmojiInput && !preferences.mixedKaomojiInput && prefix.enabled);
    NSDictionary *edited = [preferences sharedPreferencesByMerging:@{}][@"mixed_input"];
    assert(([edited isEqual:@{ @"english": @YES, @"minimum_prefix": @5, @"emoji": @NO, @"kaomoji": @NO }]));
    assert([defaults dictionaryForKey:@"MSIMEClientMixedInput"] != nil);
    // An explicitly stored NO for emoji survives reopening instead of falling back to the on default.
    MSIMEAppearancePreferences *reopened = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    assert(reopened.mixedEnglishInput && reopened.mixedEnglishMinimumPrefix == 5 &&
           !reopened.mixedEmojiInput && !reopened.mixedKaomojiInput);
    assert([[reopened sharedPreferencesByMerging:@{}][@"mixed_input"][@"emoji"] isEqual:@NO]);
    MSIMERemoveTestPreferenceSuite(defaults, suite);
}

// Its own preference suite, not the shared one. Every keybinding here is published only once its native
// default has actually been written, so on an appearance other tests have already touched the merge stops
// being a pass-through and these assertions are about cross-test isolation that does not exist.
static void TestCharacterSetShortcut(void) {
    NSString *suite = [@"msime.character-set." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    MSIMEAppearancePreferences *appearance = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    assert(appearance.characterSetShortcut);
    NSDictionary *keys = @{@"toggle_character_set_ctrl_shift_f":@NO, @"switch_language_shift":@NO};
    assert([[appearance sharedPreferencesByMerging:@{@"keybindings":keys}][@"keybindings"] isEqual:keys]);
    assert(![appearance sharedPreferencesByMerging:@{}][@"keybindings"]);
    [appearance applySharedInputPreferences:@{@"keybindings":keys}];
    assert(!appearance.characterSetShortcut);
    [appearance applySharedInputPreferences:@{@"keybindings":@{@"toggle_character_set_ctrl_shift_f":@"invalid"}}];
    [appearance applySharedInputPreferences:@{@"keybindings":@"invalid"}];
    assert(!appearance.characterSetShortcut);
    NSButton *button = (id)PreferenceControl(appearance, NSSelectorFromString(@"characterSetShortcutChanged:"));
    assert(button.state == NSControlStateValueOff);
    button.state = NSControlStateValueOn;
    [NSApp sendAction:button.action to:button.target from:button];
    assert(appearance.characterSetShortcut);
    NSDictionary *merged = [appearance sharedPreferencesByMerging:@{@"keybindings":keys}];
    assert(([merged[@"keybindings"] isEqual:@{@"toggle_character_set_ctrl_shift_f":@YES, @"switch_language_shift":@NO}]));
    NSDictionary *patch = [appearance sharedPreferencesByMerging:@{}];
    assert([patch[@"keybindings"] isEqual:@{@"toggle_character_set_ctrl_shift_f":@YES}]);
    assert([MSIMEMergePreferenceSnapshot(@{@"keybindings":keys}, patch)[@"keybindings"] isEqual:merged[@"keybindings"]]);

    ModeController *controller = [ModeController alloc];
    ShortcutClient *client = [ShortcutClient new];
    ShortcutSession *session = [ShortcutSession new];
    [controller setValue:appearance forKey:@"appearance"];
    [controller setValue:client forKey:@"activeClient"];
    [controller setValue:session forKey:@"session"];
    NSDictionary *view = @{@"editing_text":@"test", @"candidates":@[]};
    [controller setValue:view forKey:@"view"];
    const BOOL english = appearance.englishMode, traditional = appearance.traditionalOutput;
    const BOOL fullWidth = appearance.fullWidthInput, punctuation = appearance.chinesePunctuation;
    appearance.englishMode = NO;
    session.lastCommand = UINT32_MAX;
    NSEventModifierFlags flags = NSEventModifierFlagControl | NSEventModifierFlagShift;
    assert([controller handleEvent:ModeKey(3, flags, NO) client:client]);
    assert(appearance.traditionalOutput != traditional);
    assert([controller handleEvent:ModeKey(3, flags, YES) client:client]);
    assert(appearance.traditionalOutput != traditional);
    assert([[controller valueForKey:@"view"] isEqual:view]);
    assert(session.lastCommand == UINT32_MAX && client.committed == nil);
    assert([controller handleEvent:ModeKey(3, flags | NSEventModifierFlagCapsLock, NO) client:client]);
    assert(appearance.traditionalOutput == traditional);
    appearance.englishMode = YES;
    assert([controller handleEvent:ModeKey(3, flags, NO) client:client]);
    assert(appearance.traditionalOutput == traditional && appearance.englishMode);
    for (NSUInteger mask = 0; mask < 16; ++mask) {
        NSEventModifierFlags mods = (mask & 1 ? NSEventModifierFlagControl : 0) |
            (mask & 2 ? NSEventModifierFlagShift : 0) | (mask & 4 ? NSEventModifierFlagOption : 0) |
            (mask & 8 ? NSEventModifierFlagCommand : 0);
        assert([controller handleEvent:ModeKey(3, mods, NO) client:client] == (mask == 3));
        assert(appearance.traditionalOutput == traditional);
    }
    assert(![controller handleEvent:ModeKey(2, flags, NO) client:client]);
    [appearance applySharedInputPreferences:@{@"keybindings":keys}];
    assert(![controller handleEvent:ModeKey(3, flags, NO) client:client]);
    appearance.characterSetShortcut = NO;
    assert(![[[MSIMEAppearancePreferences alloc] initWithDefaults:defaults skinsRoot:appearance.skinsRoot] characterSetShortcut]);
    appearance.characterSetShortcut = YES;
    appearance.englishMode = english;
    assert(appearance.fullWidthInput == fullWidth && appearance.chinesePunctuation == punctuation);
    MSIMERemoveTestPreferenceSuite(defaults, suite);
}

static void TestDedicatedEnglish(MSIMEAppearancePreferences *appearance) {
    ModeController *controller = [ModeController alloc];
    ShortcutClient *client = [ShortcutClient new];
    ShortcutSession *session = [ShortcutSession new];
    [controller setValue:appearance forKey:@"appearance"];
    [controller setValue:client forKey:@"activeClient"];
    [controller setValue:session forKey:@"session"];
    [controller setValue:@{@"editing_text":@"test", @"candidates":@[]} forKey:@"view"];
    NSEventModifierFlags flags = NSEventModifierFlagControl | NSEventModifierFlagShift;
    // Ctrl+Shift+E cancels the composition as the reference's FUNCTION_CANCEL does. The stub's default reply commits text, so a view-only cancel reply is what makes a stray commit visible.
    NSDictionary *previousCancel = session.cancelTransition;
    session.cancelTransition = @{ @"handled": @YES, @"commit": NSNull.null,
                                  @"view": @{ @"editing_text": @"", @"caret_position": @0, @"candidates": @[] } };
    session.failCancel = YES;
    assert([controller handleEvent:ModeKey(14, flags, NO) client:client]);
    assert(session.englishCandidateCalls == 0 && !session.dedicatedEnglish);
    session.failCancel = NO;
    assert([controller handleEvent:ModeKey(14, flags, NO) client:client]);
    assert(session.dedicatedEnglish && session.englishCandidateCalls == 1 && !appearance.englishMode);
    assert(session.lastCommand == MSIME_CANCEL && client.committed == nil && client.marked.length == 0);
    assert([[[controller menu] itemAtIndex:2] state] == NSControlStateValueOn);
    assert([[[controller menu] itemAtIndex:0] state] == NSControlStateValueOff);
    assert([controller handleEvent:ModeKey(14, flags, YES) client:client]);
    assert(session.englishCandidateCalls == 1);
    [controller selectChineseMode:nil];
    assert(!session.dedicatedEnglish && !appearance.englishMode);
    appearance.englishMode = YES;
    assert([controller handleEvent:ModeKey(14, flags, NO) client:client]);
    assert(session.dedicatedEnglish && !appearance.englishMode);
    assert([controller handleEvent:ModeKey(14, flags, NO) client:client]);
    assert(!session.dedicatedEnglish && !appearance.englishMode);
    // Leaving the mode with an English word being spelled drops the word too.
    assert([controller handleEvent:ModeKey(14, flags, NO) client:client]);
    assert(session.dedicatedEnglish);
    [controller setValue:@{@"editing_text":@"hello", @"candidates":@[], @"dedicated_english":@YES} forKey:@"view"];
    session.lastCommand = UINT32_MAX;
    assert([controller handleEvent:ModeKey(14, flags, NO) client:client]);
    assert(session.lastCommand == MSIME_CANCEL && client.committed == nil && client.marked.length == 0);
    assert(!session.dedicatedEnglish && !appearance.englishMode);
    // Switching to English by any route leaves the candidate mode, as the reference's status task calls SetEnglishInputMode(false) whenever the mode turns English, so switching back gives pinyin.
    assert([controller handleEvent:ModeKey(14, flags, NO) client:client]);
    assert(session.dedicatedEnglish && !appearance.englishMode);
    NSUInteger englishCalls = session.englishCandidateCalls;
    [controller selectEnglishMode:nil];
    assert(!session.dedicatedEnglish && appearance.englishMode && session.englishCandidateCalls == englishCalls + 1);
    [controller setEnglishInputMode:NO];
    assert(!session.dedicatedEnglish && !appearance.englishMode && session.englishCandidateCalls == englishCalls + 1);
    assert([[[controller menu] itemAtIndex:0] state] == NSControlStateValueOn);
    assert([[[controller menu] itemAtIndex:2] state] == NSControlStateValueOff);
    const BOOL previousInputModeShortcut = appearance.inputModeShortcut;
    appearance.inputModeShortcut = YES;
    assert([controller handleEvent:ModeKey(14, flags, NO) client:client]);
    assert(session.dedicatedEnglish && !appearance.englishMode);
    englishCalls = session.englishCandidateCalls;
    assert([controller handleEvent:ModeKey(49, NSEventModifierFlagShift, NO) client:client]);
    assert(!session.dedicatedEnglish && appearance.englishMode && session.englishCandidateCalls == englishCalls + 1);
    assert([controller handleEvent:ModeKey(49, NSEventModifierFlagShift, NO) client:client]);
    assert(!session.dedicatedEnglish && !appearance.englishMode && session.englishCandidateCalls == englishCalls + 1);
    appearance.inputModeShortcut = previousInputModeShortcut;
    session.cancelTransition = previousCancel;
    NSUInteger calls = session.englishCandidateCalls;
    for (NSNumber *extra in @[@(NSEventModifierFlagCommand), @(NSEventModifierFlagOption)]) {
        [controller handleEvent:ModeKey(14, flags | extra.unsignedIntegerValue, NO) client:client];
        assert(session.englishCandidateCalls == calls);
    }
}

static void TestFullWidth(NSUserDefaults *defaults, MSIMEAppearancePreferences *appearance) {
    assert(!appearance.fullWidthInput);
    NSButton *control = (id)PreferenceControl(appearance, @selector(fullWidthChanged:));
    control.state = NSControlStateValueOn;
    [NSApp sendAction:control.action to:control.target from:control];
    assert(appearance.fullWidthInput);
    assert([[[MSIMEAppearancePreferences alloc] initWithDefaults:defaults skinsRoot:appearance.skinsRoot] fullWidthInput]);
    ModeController *controller = [ModeController alloc];
    ShortcutClient *client = [ShortcutClient new];
    ShortcutSession *session = [ShortcutSession new];
    [controller setValue:appearance forKey:@"appearance"];
    [controller setValue:client forKey:@"activeClient"];
    [controller setValue:session forKey:@"session"];
    const NSEventModifierFlags chord = NSEventModifierFlagOption | NSEventModifierFlagShift;
    [controller prepareSession];
    assert(session.fullwidth && session.widthCalls > 0);
    assert([controller handleEvent:ModeKey(4, chord, NO) client:client]);
    // The chord switches the current app only and syncs the Engine at once; the saved starting value stays.
    assert(!appearance.runtimeFullWidthInput && !session.fullwidth);
    assert(appearance.fullWidthInput && [defaults boolForKey:@"MSIMEClientFullWidthInput"]);
    assert([[[MSIMEAppearancePreferences alloc] initWithDefaults:defaults skinsRoot:appearance.skinsRoot] fullWidthInput]);
    assert([controller handleEvent:ModeKey(4, chord, YES) client:client]);
    assert(!appearance.runtimeFullWidthInput);
    assert([controller handleEvent:ModeKey(4, chord, NO) client:client]);
    assert(appearance.runtimeFullWidthInput && session.asciiCalls == 0);
    const NSEventModifierFlags windowsChord = NSEventModifierFlagControl | NSEventModifierFlagShift;
    assert([controller handleEvent:ModeKey(49, windowsChord, NO) client:client]);
    assert(!appearance.runtimeFullWidthInput);
    [controller appearanceChanged:nil];
    assert(!session.fullwidth);
    NSUInteger widthCalls = session.widthCalls;
    assert([controller handleEvent:ModeKey(49, windowsChord, YES) client:client]);
    assert(!appearance.runtimeFullWidthInput && session.widthCalls == widthCalls);
    assert([controller handleEvent:ModeKey(49, windowsChord, NO) client:client]);
    [controller appearanceChanged:nil];
    assert(appearance.runtimeFullWidthInput && session.fullwidth);
    for (NSEventModifierFlags extra : {NSEventModifierFlagCommand, NSEventModifierFlagOption})
        assert(!msime::mac::IsFullWidthInputToggle(49, windowsChord | extra));
    for (NSEventModifierFlags extra : {NSEventModifierFlagCommand, NSEventModifierFlagControl}) {
        assert(![controller handleEvent:ModeKey(4, chord | extra, NO) client:client]);
        assert(appearance.runtimeFullWidthInput);
    }
    // Turning the preference off gives Option+Shift+H back to the application. Ctrl+Shift+Space is
    // not what the settings page names, so it keeps working either way.
    const BOOL restoreFullWidthShortcut = appearance.fullWidthShortcut;
    const BOOL fullWidthBefore = appearance.runtimeFullWidthInput;
    appearance.fullWidthShortcut = NO;
    assert(![controller handleEvent:ModeKey(4, chord, NO) client:client]);
    assert(appearance.runtimeFullWidthInput == fullWidthBefore);
    assert([controller handleEvent:ModeKey(49, windowsChord, NO) client:client]);
    assert(appearance.runtimeFullWidthInput != fullWidthBefore);
    assert([controller handleEvent:ModeKey(49, windowsChord, NO) client:client]);
    assert(appearance.runtimeFullWidthInput == fullWidthBefore);
    appearance.fullWidthShortcut = restoreFullWidthShortcut;
    assert([controller handleEvent:ModeKey(4, chord, NO) client:client]);
    assert(appearance.runtimeFullWidthInput != fullWidthBefore);
    assert([controller handleEvent:ModeKey(4, chord, NO) client:client]);
    assert(appearance.runtimeFullWidthInput == fullWidthBefore);
    appearance.englishMode = YES;
    assert(![controller handleEvent:ModeKey(4, chord, NO) client:client]);
    assert([controller handleEvent:ModeKey(49, windowsChord, NO) client:client]);
    assert(!appearance.runtimeFullWidthInput && appearance.englishMode);
    assert([controller handleEvent:ModeKey(49, windowsChord, NO) client:client]);
    client.committed = nil;
    assert([controller handleEvent:ModeKey(0, 0, NO) client:client] && [client.committed isEqual:@"ａ"]); // English mode applies full-width too.
    assert(appearance.runtimeFullWidthInput && appearance.fullWidthInput);
    appearance.englishMode = NO;
    NSDictionary *idle = @{@"handled": @NO, @"view": @{@"editing_text": @"", @"candidates": @[]}};
    session.nextTransition = idle;
    for (unichar character = 32; character <= 126; ++character) {
        NSString *text = [NSString stringWithCharacters:&character length:1];
        NSEvent *key = [NSEvent keyEventWithType:NSEventTypeKeyDown location:NSZeroPoint modifierFlags:0 timestamp:0 windowNumber:0 context:nil characters:text charactersIgnoringModifiers:text isARepeat:NO keyCode:character == 32 ? 49 : 0];
        assert([controller handleEvent:key client:client]);
        assert(client.committed.length == 1);
        assert([client.committed characterAtIndex:0] == (character == 32 ? 0x3000 : character + 0xFEE0));
        // With pairing on, `{` opens a full-width pair; close it so the next keys are checked on their own.
        if (character == '{') {
            assert([client.marked isEqual:@"｝"]);
            [controller flushPendingPairedClosing];
        }
    }
    client.committed = nil;
    session.nextTransition = @{@"handled": @YES, @"view": @{@"editing_text": @"a", @"candidates": @[]}};
    NSUInteger calls = session.asciiCalls;
    assert([controller handleEvent:ModeKey(0, 0, NO) client:client]);
    assert(session.asciiCalls == calls + 1 && client.committed == nil && [client.marked isEqual:@"a"]);
    session.nextTransition = @{@"handled": @NO, @"view": @{@"editing_text": @"a", @"candidates": @[]}};
    session.failFinish = YES;
    assert(![controller handleEvent:ModeKey(0, 0, NO) client:client]);
    assert(client.committed == nil);
    session.failFinish = NO;
    // A partial finish must not insert the fallback inside the remaining composition.
    assert(![controller handleEvent:ModeKey(0, 0, NO) client:client]);
    assert(client.committed == nil);
    session.finishTransition = @{@"handled": @YES, @"commit": @"测试", @"view": @{@"editing_text": @"", @"candidates": @[]}};
    client.insertions = [NSMutableArray array];
    assert([controller handleEvent:ModeKey(0, 0, NO) client:client]);
    assert([client.committed isEqual:@"ａ"] && client.marked.length == 0);
    assert(([client.insertions isEqual:@[@"测试", @"ａ"]]));
    session.nextTransition = idle;
    for (NSString *text in @[@"\t", @"汉", @"😀"]) {
        NSEvent *key = [NSEvent keyEventWithType:NSEventTypeKeyDown location:NSZeroPoint modifierFlags:0 timestamp:0 windowNumber:0 context:nil characters:text charactersIgnoringModifiers:text isARepeat:NO keyCode:0];
        client.committed = nil;
        assert(![controller handleEvent:key client:client]);
        assert(client.committed == nil);
    }
    session.nextTransition = nil;
    client.committed = nil;
    assert(![controller handleEvent:ModeKey(0, 0, NO) client:client]);
    assert(client.committed == nil);
    session.nextTransition = idle;
    appearance.fullWidthInput = NO;
    client.committed = nil;
    assert(![controller handleEvent:ModeKey(0, 0, NO) client:client]);
    assert(client.committed == nil && control.state == NSControlStateValueOff);
}

// What this host asks a session for, on top of the options file. The file is shared with hosts that
// render differently, so a behaviour this one draws has to be requested rather than written into it -
// and if that request is ever dropped, nothing else here notices: a phrase being assembled would go
// back to arriving in the document one piece at a time, which looks like ordinary typing.
static void TestSessionOptions() {
    assert(!MSIMESessionOptions(nil));
    assert(!MSIMESessionOptions((NSDictionary *)@"not a dictionary"));

    NSDictionary *file = @{@"api_version":@1, @"resources":@"/synthetic/resources",
        @"preferences":@{@"scheme":@"quanpin"}};
    NSDictionary *requested = MSIMESessionOptions(file);
    assert([requested[@"phrase_preedit"] isEqual:@YES]);
    // Everything the file carried is passed through untouched, including nested objects.
    for (NSString *key in file) assert([requested[key] isEqual:file[key]]);
    assert(requested.count == file.count + 1);
    // The caller's dictionary is not modified: prepareSession reads preferences_directory back out
    // of the result, and the options file dictionary is handed around elsewhere.
    assert(!file[@"phrase_preedit"]);

    // An options file that already says something about it does not get to say no: this host draws
    // the field, and a stale file predates the behaviour entirely.
    NSDictionary *stale = MSIMESessionOptions(@{@"api_version":@1, @"phrase_preedit":@NO});
    assert([stale[@"phrase_preedit"] isEqual:@YES]);
}

static void TestKeypadDecimal(MSIMEAppearancePreferences *appearance) {
    ModeController *controller = [ModeController alloc];
    ShortcutSession *session = [ShortcutSession new];
    ShortcutClient *client = [ShortcutClient new];
    client.insertions = [NSMutableArray array];
    [controller setValue:appearance forKey:@"appearance"];
    [controller setValue:session forKey:@"session"];
    [controller setValue:client forKey:@"activeClient"];
    [controller setValue:@{ @"focused": @YES, @"editing_text": @"", @"candidates": @[] } forKey:@"view"];
    NSEvent *key = [NSEvent keyEventWithType:NSEventTypeKeyDown location:NSZeroPoint modifierFlags:0 timestamp:0
                                      windowNumber:0 context:nil characters:@"." charactersIgnoringModifiers:@"."
                                      isARepeat:NO keyCode:65];
    session.punctuationASCIITransition = @{ @"handled": @YES, @"commit": @"候选.",
        @"view": @{ @"focused": @YES, @"editing_text": @"", @"candidates": @[] } };
    assert([controller handleEvent:key client:client]);
    assert(session.punctuationASCIICalls == 1 && session.lastPunctuationASCII == '.' && session.asciiCalls == 0);
    assert([client.committed isEqual:@"候选."]);

    session.punctuationASCIITransition = @{ @"handled": @NO, @"view": @{ @"focused": @YES,
        @"editing_text": @"", @"candidates": @[] } };
    client.committed = nil;
    [client.insertions removeAllObjects];
    assert([controller handleEvent:key client:client]);
    assert(session.punctuationASCIICalls == 2 && [client.committed isEqual:@"."]);
    assert([client.insertions isEqual:@[@"."]]);

    NSEvent *modified = [NSEvent keyEventWithType:NSEventTypeKeyDown location:NSZeroPoint
        modifierFlags:NSEventModifierFlagControl timestamp:0 windowNumber:0 context:nil characters:@"."
        charactersIgnoringModifiers:@"." isARepeat:NO keyCode:65];
    assert(![controller handleEvent:modified client:client]);
    assert(session.punctuationASCIICalls == 2);
}

// Editing a composition by segmentation unit rather than by character. The reference's composition
// editor gives Ctrl+Backspace and Ctrl+Left / Ctrl+Right this meaning, and both Linux front ends and
// the Windows host route them; this host handed every Ctrl chord back to the application after
// finishing the composition, so the three keys did nothing but commit what was being typed.
// Ctrl+Enter on a candidate whose gloss carries several senses.
//
// The reference turns the highlighted candidate's gloss into a page of its senses and lets the user
// pick one the way they pick a candidate; a single sense commits straight away. Both Linux front
// ends follow it. This host had neither: Ctrl+Enter fell into "any Ctrl chord finishes the
// composition and goes back to the application", so the only thing it did was commit the pinyin.
// Japanese converts with Space and commits with Enter.
//
// Romaji is not what the user typed; かな is. This host sent MSIME_COMMIT_RAW on Enter for every
// scheme, which in Japanese commits `nihon` where the user meant にほん - and Space committed the
// first conversion outright, so the second one could not be reached at all. Both are what every
// Japanese input method does differently, and both touch hosts here already did it correctly.
// The floating toolbar is one click from the input menu, with a tick showing the state.
//
// The reference puts it first in the menu its language bar opens; here it could only be reached by
// opening the settings window and finding a checkbox, which is a long way round for something the
// user turns on and off while typing. The item writes the same preference that checkbox writes, so
// the two cannot disagree and the choice survives a restart.
@interface ModeSelectingClient : ShortcutClient
@property(nonatomic, strong) NSMutableArray<NSString *> *selectedModes;
- (void)selectInputMode:(NSString *)identifier;
@end
@implementation ModeSelectingClient
- (void)selectInputMode:(NSString *)identifier {
    if (!self.selectedModes) self.selectedModes = [NSMutableArray array];
    [self.selectedModes addObject:identifier];
}
@end

// The selected input mode, 中, 英 or 日, follows the controller's state. A mode the system reports - picked from the input menu or reached with Ctrl+Space - sets the Chinese/English state and, for 中 and 日, the scheme, and is not selected back.
static void TestSystemInputModeReport(MSIMEAppearancePreferences *appearance) {
    ModeController *controller = [ModeController alloc];
    ModeSelectingClient *client = [ModeSelectingClient new];
    ShortcutSession *session = [ShortcutSession new];
    [controller setValue:appearance forKey:@"appearance"];
    [controller setValue:client forKey:@"activeClient"];
    [controller setValue:session forKey:@"session"];
    [controller setValue:@{@"editing_text":@"", @"candidates":@[]} forKey:@"view"];
    const BOOL english = appearance.englishMode;
    appearance.englishMode = NO;

    [controller systemDidReportInputMode:MSIMEEnglishInputModeID client:client];
    assert(appearance.englishMode && client.selectedModes.count == 0);
    [controller systemDidReportInputMode:MSIMEEnglishInputModeID client:client];
    assert(appearance.englishMode && client.selectedModes.count == 0);

    // Another source is not a mode switch of this input method.
    [controller systemDidReportInputMode:@"com.apple.keylayout.ABC" client:client];
    [controller systemDidReportInputMode:nil client:client];
    assert(appearance.englishMode && client.selectedModes.count == 0);

    [controller systemDidReportInputMode:MSIMEChineseInputModeID client:client];
    assert(!appearance.englishMode && client.selectedModes.count == 0);

    // 日 moves the scheme to japanese, 英 over it leaves the scheme alone, and 中 goes back to the Chinese scheme japanese was entered from.
    NSString *scheme = appearance.inputScheme;
    appearance.inputScheme = @"shuangpin";
    [controller systemDidReportInputMode:MSIMEJapaneseInputModeID client:client];
    assert([appearance.inputScheme isEqual:@"japanese"] && [appearance.lastChineseScheme isEqual:@"shuangpin"] &&
           !appearance.englishMode && client.selectedModes.count == 0);
    [controller systemDidReportInputMode:MSIMEEnglishInputModeID client:client];
    assert([appearance.inputScheme isEqual:@"japanese"] && appearance.englishMode && client.selectedModes.count == 0);
    [controller systemDidReportInputMode:MSIMEJapaneseInputModeID client:client];
    assert([appearance.inputScheme isEqual:@"japanese"] && !appearance.englishMode && client.selectedModes.count == 0);
    [controller systemDidReportInputMode:MSIMEChineseInputModeID client:client];
    assert([appearance.inputScheme isEqual:@"shuangpin"] && !appearance.englishMode && client.selectedModes.count == 0);
    appearance.inputScheme = scheme;
    appearance.englishMode = english;
}

static void TestFloatingToolbarMenuToggle(MSIMEAppearancePreferences *appearance) {
    ModeController *controller = [ModeController alloc];
    [controller setValue:appearance forKey:@"appearance"];
    appearance.floatingToolbarEnabled = YES;

    NSMenu *menu = [controller menu];
    NSMenuItem *item = nil;
    for (NSMenuItem *candidate in menu.itemArray)
        if (candidate.action == @selector(toggleFloatingToolbar:)) { item = candidate; break; }
    assert(item && [item.title isEqual:@"悬浮工具栏"]);
    assert(item.state == NSControlStateValueOn);

    // Choosing it turns the toolbar off, and the menu built next says so.
    [controller toggleFloatingToolbar:item];
    assert(!appearance.floatingToolbarEnabled);
    NSMenuItem *again = nil;
    for (NSMenuItem *candidate in [controller menu].itemArray)
        if (candidate.action == @selector(toggleFloatingToolbar:)) { again = candidate; break; }
    assert(again && again.state == NSControlStateValueOff);

    // And back on, which is the same preference the settings page reads.
    [controller toggleFloatingToolbar:again];
    assert(appearance.floatingToolbarEnabled);
}

// IMK gives each text input client its own controller and candidate window. A controller whose client went away without a matching deactivateServer: must not leave its last frame on screen once another controller shows candidates or takes focus.
@interface GlossArrivalSession : ShortcutSession
@property(nonatomic) BOOL translated;
@property(nonatomic) NSUInteger applications;
@end
@implementation GlossArrivalSession
- (NSDictionary *)translationQueryWithError:(NSError **)error { (void)error; return nil; }
- (NSDictionary *)viewWithError:(NSError **)error {
    (void)error;
    NSMutableDictionary *candidate = [@{@"text":@"你", @"source":@4, @"id":@{@"session":@1, @"generation":@3, @"index":@0}} mutableCopy];
    if (self.translated) candidate[@"translation"] = @"you";
    return @{@"session":@1, @"generation":@3, @"editing_text":@"ni", @"preedit":@"ni", @"caret_position":@2, @"candidates":@[candidate]};
}
- (NSDictionary *)applyTranslations:(NSArray *)translations generation:(uint64_t)generation error:(NSError **)error {
    (void)translations; (void)error;
    assert(generation == 3);
    ++self.applications;
    self.translated = YES;
    return @{@"applied":@YES, @"view":[self viewWithError:nil]};
}
@end

@interface NestingClient : ShortcutClient
@property(nonatomic, copy) void (^onMarkedText)(void);
@property(nonatomic) NSUInteger markedWrites;
@end
@implementation NestingClient
- (void)setMarkedText:(id)text selectionRange:(NSRange)selection replacementRange:(NSRange)range {
    ++self.markedWrites;
    [super setMarkedText:text selectionRange:selection replacementRange:range];
    void (^nested)(void) = self.onMarkedText;
    self.onMarkedText = nil;
    if (nested) nested();
}
@end

@interface ServiceCountingController : ModeController
@property(nonatomic) NSUInteger serviceSyncs;
@end
@implementation ServiceCountingController
- (void)synchronizeCandidateServices { ++self.serviceSyncs; [super synchronizeCandidateServices]; }
@end

// A gloss changes what the card shows, never the composition. Its arrival must not write the marked text again: IMK services the next key inside that synchronous call, which is how a keystroke, reranking and all, came to run nested in every gloss arrival.
static void TestGlossArrivalRedrawsOnlyTheCard(MSIMEAppearancePreferences *appearance) {
    ServiceCountingController *controller = [ServiceCountingController alloc];
    GlossArrivalSession *session = [GlossArrivalSession new];
    NestingClient *client = [NestingClient new];
    HiddenCandidatePanel *panel = [[HiddenCandidatePanel alloc] init];
    [controller setValue:appearance forKey:@"appearance"];
    [controller setValue:session forKey:@"session"];
    [controller setValue:client forKey:@"activeClient"];
    [controller setValue:panel forKey:@"panel"];
    [controller setValue:[session viewWithError:nil] forKey:@"view"];
    [controller applyCandidateTranslationResults];
    assert(session.applications == 1 && client.markedWrites == 0);
    NSDictionary *view = [controller valueForKey:@"view"];
    assert([view[@"candidates"][0][@"translation"] isEqual:@"you"]);
    // The same gloss arriving again leaves the card as it is, but what depends on the answer still follows it: an offline lookup that found nothing for this page is what lets the online fallback start.
    const NSUInteger syncs = controller.serviceSyncs;
    [controller applyCandidateTranslationResults];
    assert(session.applications == 2 && client.markedWrites == 0 && [controller valueForKey:@"view"] == view);
    assert(controller.serviceSyncs == syncs + 1);

    // A gloss merged while an apply: is writing the marked text belongs to the same page; the transition finishing afterwards carries the page from before it and must not take the gloss away.
    session.translated = NO;
    NSDictionary *untranslated = [session viewWithError:nil];
    [controller setValue:untranslated forKey:@"view"];
    __weak ServiceCountingController *weakController = controller;
    client.onMarkedText = ^{ [weakController applyCandidateTranslationResults]; };
    [controller apply:@{@"view":untranslated}];
    assert([[controller valueForKey:@"view"][@"candidates"][0][@"translation"] isEqual:@"you"]);
}

// An apply: that writes marked text can have a newer key handled inside that call; the older transition finishing afterwards must not put its view back over the newer one.
static void TestNestedApplyKeepsTheNewerView(MSIMEAppearancePreferences *appearance) {
    ModeController *controller = [ModeController alloc];
    ShortcutSession *session = [ShortcutSession new];
    NestingClient *client = [NestingClient new];
    HiddenCandidatePanel *panel = [[HiddenCandidatePanel alloc] init];
    [controller setValue:appearance forKey:@"appearance"];
    [controller setValue:session forKey:@"session"];
    [controller setValue:client forKey:@"activeClient"];
    [controller setValue:panel forKey:@"panel"];
    NSDictionary *older = @{@"session":@1, @"generation":@4, @"editing_text":@"jie", @"preedit":@"jie", @"caret_position":@3, @"candidates":@[]};
    NSDictionary *newer = @{@"session":@1, @"generation":@5, @"editing_text":@"", @"preedit":@"", @"caret_position":@0, @"candidates":@[]};
    __weak ModeController *weakController = controller;
    client.onMarkedText = ^{ [weakController apply:@{@"view":newer}]; };
    [controller apply:@{@"view":older}];
    assert(client.markedWrites >= 1);
    assert([[controller valueForKey:@"view"] isEqual:newer]);
}

static void TestCandidatePanelSingleOwner() {
    ModeController *stale = [ModeController alloc];
    ModeController *current = [ModeController alloc];
    HiddenCandidatePanel *stalePanel = [[HiddenCandidatePanel alloc] init];
    HiddenCandidatePanel *currentPanel = [[HiddenCandidatePanel alloc] init];
    [stale setValue:stalePanel forKey:@"panel"];
    [current setValue:currentPanel forKey:@"panel"];
    [stale claimCandidatePanel];
    [stalePanel orderFrontRegardless];
    // Re-claiming by the owner itself leaves its window alone.
    [stale claimCandidatePanel];
    assert(stalePanel.isVisible);
    [current claimCandidatePanel];
    [currentPanel orderFrontRegardless];
    assert(!stalePanel.isVisible && currentPanel.isVisible);
    // Focus moving back hands the window over the other way.
    [stale claimCandidatePanel];
    assert(!currentPanel.isVisible);
}

static void TestJapaneseConversionKeys(MSIMEAppearancePreferences *appearance) {
    ModeController *controller = [ModeController alloc];
    ShortcutSession *session = [ShortcutSession new];
    ShortcutClient *client = [ShortcutClient new];
    TestCandidatePanel *panel = [TestCandidatePanel new];
    panel.visible = YES;
    [controller setValue:appearance forKey:@"appearance"];
    [controller setValue:session forKey:@"session"];
    [controller setValue:client forKey:@"activeClient"];
    [controller setValue:panel forKey:@"panel"];
    appearance.englishMode = NO;

    NSDictionary *(^composing)(NSInteger) = ^(NSInteger scheme) {
        // The identities carry the session and generation the view is on; the host refuses any
        // candidate whose identity does not match what is on screen.
        return @{ @"focused": @YES, @"scheme": @(scheme), @"editing_text": @"nihon",
                  @"caret_position": @5, @"reading": @"にほん", @"session": @4, @"generation": @7,
                  @"candidates": @[@{ @"text": @"日本", @"highlighted": @YES,
                                      @"id": @{ @"session": @4, @"generation": @7, @"index": @0 } },
                                   @{ @"text": @"にほん",
                                      @"id": @{ @"session": @4, @"generation": @7, @"index": @1 } },
                                   @{ @"text": @"二本",
                                      @"id": @{ @"session": @4, @"generation": @7, @"index": @2 } }] };
    };
    // Stepping through candidates leaves the list on screen with the highlight moved, which is
    // what the Engine answers; a stub that dropped the candidates would take the identities the
    // next Enter has to name.
    session.nextTransition = @{ @"handled": @YES, @"view": composing(3) };

    // Enter with no conversion started commits the reading, not the romaji.
    [controller setValue:composing(3) forKey:@"view"];
    session.lastCommand = UINT32_MAX;
    assert([controller handleEvent:ModeKey(36, 0, NO) client:client]);
    assert(session.lastCommand == MSIME_COMMIT_READING);

    // Space starts the conversion instead of committing it: nothing reaches the Engine, because
    // the first candidate is already the highlighted one.
    [controller setValue:composing(3) forKey:@"view"];
    session.lastCommand = UINT32_MAX;
    NSUInteger selectsBefore = session.selectCalls;
    assert([controller handleEvent:ModeKey(49, 0, NO) client:client]);
    assert(session.lastCommand == UINT32_MAX && session.selectCalls == selectsBefore);

    // The next press steps to the following candidate.
    assert([controller handleEvent:ModeKey(49, 0, NO) client:client]);
    assert(session.lastCommand == MSIME_NEXT_CANDIDATE);

    // Enter now takes the candidate that was stepped to rather than the reading.
    assert([controller handleEvent:ModeKey(36, 0, NO) client:client]);
    assert(session.selectCalls == selectsBefore + 1);
    assert(session.selectedGeneration == 7 && session.selectedIndex == 1);

    // Stepping past the end comes back to the first candidate.
    [controller setValue:composing(3) forKey:@"view"];
    for (int press = 0; press < 3; ++press)
        assert([controller handleEvent:ModeKey(49, 0, NO) client:client]);
    session.lastCommand = UINT32_MAX;
    assert([controller handleEvent:ModeKey(49, 0, NO) client:client]);
    assert(session.lastCommand == MSIME_FIRST_CANDIDATE);

    // Editing the reading abandons the conversion: Enter is the kana again.
    [controller setValue:composing(3) forKey:@"view"];
    assert([controller handleEvent:ModeKey(49, 0, NO) client:client]);
    NSMutableDictionary *edited = [composing(3) mutableCopy];
    edited[@"editing_text"] = @"nihong";
    [controller setValue:edited forKey:@"view"];
    session.lastCommand = UINT32_MAX;
    assert([controller handleEvent:ModeKey(36, 0, NO) client:client]);
    assert(session.lastCommand == MSIME_COMMIT_READING);

    // Every other scheme keeps the keys it had: Space commits the candidate, Enter the raw input.
    [controller setValue:composing(0) forKey:@"view"];
    session.lastCommand = UINT32_MAX;
    assert([controller handleEvent:ModeKey(49, 0, NO) client:client]);
    assert(session.lastCommand == MSIME_COMMIT_CANDIDATE);
    [controller setValue:composing(0) forKey:@"view"];
    session.lastCommand = UINT32_MAX;
    assert([controller handleEvent:ModeKey(36, 0, NO) client:client]);
    assert(session.lastCommand == MSIME_COMMIT_RAW);

    // A Japanese composition with no candidates leaves Space alone, and a chord is never this.
    NSMutableDictionary *bare = [composing(3) mutableCopy];
    bare[@"candidates"] = @[];
    [controller setValue:bare forKey:@"view"];
    session.lastCommand = UINT32_MAX;
    assert([controller handleEvent:ModeKey(49, 0, NO) client:client]);
    assert(session.lastCommand == MSIME_COMMIT_CANDIDATE);

    // A bare Shift+R shows its raw prefix as the one Fallback candidate (source 9). Like Windows, the first Space commits it; there is nothing to convert.
    NSMutableDictionary *fallback = [composing(3) mutableCopy];
    fallback[@"editing_text"] = @"R";
    fallback[@"reading"] = @"";
    fallback[@"local_mode"] = @"temporary_japanese";
    fallback[@"candidates"] = @[@{ @"text": @"R", @"source": @9, @"highlighted": @YES,
                                   @"id": @{ @"session": @4, @"generation": @7, @"index": @0 } }];
    [controller setValue:fallback forKey:@"view"];
    session.lastCommand = UINT32_MAX;
    assert([controller handleEvent:ModeKey(49, 0, NO) client:client]);
    assert(session.lastCommand == MSIME_COMMIT_CANDIDATE);
    [controller setValue:composing(3) forKey:@"view"];
    session.lastCommand = UINT32_MAX;
    [controller handleEvent:ModeKey(36, NSEventModifierFlagControl, NO) client:client];
    assert(session.lastCommand != MSIME_COMMIT_READING);
}

static void TestGlossSensePage(MSIMEAppearancePreferences *appearance) {
    ModeController *controller = [ModeController alloc];
    ShortcutSession *session = [ShortcutSession new];
    ShortcutClient *client = [ShortcutClient new];
    TestCandidatePanel *panel = [TestCandidatePanel new];
    panel.visible = YES;
    [controller setValue:appearance forKey:@"appearance"];
    [controller setValue:session forKey:@"session"];
    [controller setValue:client forKey:@"activeClient"];
    [controller setValue:panel forKey:@"panel"];
    appearance.englishMode = NO;
    appearance.candidateTranslations = YES;
    // Committing a sense clears the composition rather than finishing it, and a cleared composition
    // commits nothing: the stub's default transition would otherwise put its own text in behind the
    // sense, which is exactly the bug this behaviour exists to avoid.
    session.cancelTransition = @{ @"handled": @YES,
                                  @"view": @{ @"editing_text": @"", @"caret_position": @0, @"candidates": @[] } };

    // Committing clears the composition, which takes the panel down with it, so each case puts a
    // fresh candidate list on screen the way the Engine would.
    void (^compose)(NSString *) = ^(NSString *gloss) {
        [controller setValue:@{ @"focused": @YES, @"editing_text": @"nihao", @"caret_position": @5,
                                @"candidates": @[@{@"text": @"你好", @"highlighted": @YES, @"translation": gloss},
                                                 @{@"text": @"泥好"}] }
                      forKey:@"view"];
        panel.visible = YES;
    };

    // A gloss with one sense commits it and takes the composition away with it: the sense is what
    // the user asked for, and finishing instead would put the Chinese word in behind it.
    compose(@"hello");
    client.committed = nil;
    session.lastCommand = UINT32_MAX;
    assert([controller handleEvent:ModeKey(36, NSEventModifierFlagControl, NO) client:client]);
    assert([client.committed isEqual:@"hello"]);
    assert(session.lastCommand == MSIME_CANCEL);

    // Several senses open the page instead. The candidates on screen are the senses, and the first
    // one is highlighted.
    compose(@"hello; hi; greetings");
    client.committed = nil;
    assert([controller handleEvent:ModeKey(36, NSEventModifierFlagControl, NO) client:client]);
    assert(!client.committed);
    NSArray *shown = [controller valueForKey:@"view"][@"candidates"];
    assert(shown.count == 3);
    assert([shown[0][@"text"] isEqual:@"hello"] && [shown[0][@"highlighted"] isEqual:@YES]);
    assert([shown[2][@"text"] isEqual:@"greetings"]);

    // A digit picks a sense by its position on the page.
    session.lastCommand = UINT32_MAX;
    assert([controller handleEvent:ModeKey(20, 0, NO) client:client]); // 3
    assert([client.committed isEqual:@"greetings"]);
    assert(session.lastCommand == MSIME_CANCEL);
    // The page is gone afterwards, together with the composition it was drawn from.
    assert([[controller valueForKey:@"view"][@"candidates"] count] == 0);

    // Arrow keys move inside the page and space takes what is highlighted.
    compose(@"hello; hi; greetings");
    assert([controller handleEvent:ModeKey(36, NSEventModifierFlagControl, NO) client:client]);
    assert([controller handleEvent:ModeKey(125, 0, NO) client:client]); // Down
    client.committed = nil;
    assert([controller handleEvent:ModeKey(49, 0, NO) client:client]); // Space
    assert([client.committed isEqual:@"hi"]);

    // A key the page does not claim leaves it and is routed as usual rather than being swallowed by
    // a mode the user has forgotten about.
    compose(@"hello; hi");
    assert([controller handleEvent:ModeKey(36, NSEventModifierFlagControl, NO) client:client]);
    client.committed = nil;
    [controller handleEvent:ModeKey(0, 0, NO) client:client]; // 'a'
    assert(!client.committed);
    assert([[controller valueForKey:@"view"][@"candidates"][0][@"text"] isEqual:@"你好"]);

    // Escape leaves the page and puts the candidates that were on screen back, without committing.
    compose(@"hello; hi");
    assert([controller handleEvent:ModeKey(36, NSEventModifierFlagControl, NO) client:client]);
    client.committed = nil;
    assert([controller handleEvent:ModeKey(53, 0, NO) client:client]);
    assert(!client.committed);
    assert([[controller valueForKey:@"view"][@"candidates"][0][@"text"] isEqual:@"你好"]);

    // A candidate with no gloss is not a page, and the chord keeps its old meaning there.
    [controller setValue:@{ @"focused": @YES, @"editing_text": @"nihao",
                            @"candidates": @[@{@"text": @"你好", @"highlighted": @YES}] } forKey:@"view"];
    panel.visible = YES;
    client.committed = nil;
    session.lastCommand = UINT32_MAX;
    assert(![controller handleEvent:ModeKey(36, NSEventModifierFlagControl, NO) client:client]);
    assert(session.lastCommand == MSIME_FINISH_COMPOSITION);

    // With candidate translations turned off there is nothing to offer.
    appearance.candidateTranslations = NO;
    compose(@"hello; hi");
    session.lastCommand = UINT32_MAX;
    assert(![controller handleEvent:ModeKey(36, NSEventModifierFlagControl, NO) client:client]);
    assert(session.lastCommand == MSIME_FINISH_COMPOSITION);
    appearance.candidateTranslations = YES;
}

// Ctrl+Enter translations, the senses page and the Option/Ctrl+digit gloss columns insert text through the same Traditional conversion as every other commit (the reference's CandidateTextForOutput), and the page that is drawn converted inserts what it shows.
static void TestGlossSenseTraditionalOutput(MSIMEAppearancePreferences *appearance) {
    ModeController *controller = [ModeController alloc];
    ShortcutSession *session = [ShortcutSession new];
    ShortcutClient *client = [ShortcutClient new];
    TestCandidatePanel *panel = [TestCandidatePanel new];
    panel.visible = YES;
    [controller setValue:appearance forKey:@"appearance"];
    [controller setValue:session forKey:@"session"];
    [controller setValue:client forKey:@"activeClient"];
    [controller setValue:panel forKey:@"panel"];
    const BOOL savedTraditional = appearance.traditionalOutput;
    appearance.englishMode = NO;
    appearance.candidateTranslations = YES;
    appearance.traditionalOutput = YES;
    session.cancelTransition = @{ @"handled": @YES,
                                  @"view": @{ @"editing_text": @"", @"caret_position": @0, @"candidates": @[] } };
    void (^compose)(NSString *, NSString *) = ^(NSString *gloss, NSString *localMode) {
        NSMutableDictionary *view = [@{ @"focused": @YES, @"scheme": @0, @"editing_text": @"apple", @"caret_position": @5,
                                        @"candidates": @[@{@"text": @"apple", @"highlighted": @YES, @"translation": gloss},
                                                         @{@"text": @"apply"}] } mutableCopy];
        if (localMode) view[@"local_mode"] = localMode;
        [controller setValue:view forKey:@"view"];
        panel.visible = YES;
    };

    // A single-sense Ctrl+Enter commits the Traditional form and still clears the composition.
    compose(@"苹果", nil);
    client.committed = nil;
    session.lastCommand = UINT32_MAX;
    assert([controller handleEvent:ModeKey(36, NSEventModifierFlagControl, NO) client:client]);
    assert([client.committed isEqual:@"蘋果"]);
    assert(session.lastCommand == MSIME_CANCEL);

    // The senses page picks the sense through the same conversion it is drawn with.
    compose(@"头发; 苹果", nil);
    client.committed = nil;
    assert([controller handleEvent:ModeKey(36, NSEventModifierFlagControl, NO) client:client]);
    assert(!client.committed);
    session.lastCommand = UINT32_MAX;
    assert([controller handleEvent:ModeKey(19, 0, NO) client:client]); // 2
    assert([client.committed isEqual:@"蘋果"]);
    assert(session.lastCommand == MSIME_CANCEL);

    // Option+digit takes the first gloss column of that candidate.
    compose(@"苹果", nil);
    client.committed = nil;
    assert([controller handleEvent:ModeKey(18, NSEventModifierFlagOption, NO) client:client]); // Option+1
    assert([client.committed isEqual:@"蘋果"]);

    // Temporary Japanese is exempt, as it is for Engine commits.
    compose(@"苹果", @"temporary_japanese");
    client.committed = nil;
    assert([controller handleEvent:ModeKey(36, NSEventModifierFlagControl, NO) client:client]);
    assert([client.committed isEqual:@"苹果"]);

    // With the switch off the gloss is inserted as the dictionary has it.
    appearance.traditionalOutput = NO;
    compose(@"头发; 苹果", nil);
    client.committed = nil;
    assert([controller handleEvent:ModeKey(36, NSEventModifierFlagControl, NO) client:client]);
    assert([controller handleEvent:ModeKey(19, 0, NO) client:client]);
    assert([client.committed isEqual:@"苹果"]);

    appearance.traditionalOutput = savedTraditional;
}

static void TestSegmentEditingChords(MSIMEAppearancePreferences *appearance) {
    ModeController *controller = [ModeController alloc];
    ShortcutSession *session = [ShortcutSession new];
    ShortcutClient *client = [ShortcutClient new];
    [controller setValue:appearance forKey:@"appearance"];
    [controller setValue:session forKey:@"session"];
    [controller setValue:client forKey:@"activeClient"];
    appearance.englishMode = NO;
    NSDictionary *composing = @{ @"focused": @YES, @"editing_text": @"nihao", @"caret_position": @5,
                                 @"candidates": @[@{@"text": @"你好"}] };
    [controller setValue:composing forKey:@"view"];

    for (NSArray *entry in @[@[@51, @(MSIME_BACKSPACE_SEGMENT)], @[@123, @(MSIME_MOVE_LEFT_SEGMENT)],
                             @[@124, @(MSIME_MOVE_RIGHT_SEGMENT)]]) {
        const unsigned short code = [entry[0] unsignedShortValue];
        session.lastCommand = UINT32_MAX;
        assert([controller handleEvent:ModeKey(code, NSEventModifierFlagControl, NO) client:client]);
        assert(session.lastCommand == [entry[1] unsignedIntValue]);
        [controller setValue:composing forKey:@"view"];
    }

    // Only the bare Ctrl chord. With anything else held the key is the application's, and this host
    // finishes the composition on the way out rather than editing it.
    for (NSNumber *extra in @[@(NSEventModifierFlagShift), @(NSEventModifierFlagOption),
                              @(NSEventModifierFlagCommand)]) {
        session.lastCommand = UINT32_MAX;
        assert(![controller handleEvent:ModeKey(51, NSEventModifierFlagControl | extra.unsignedIntegerValue, NO)
                                 client:client]);
        assert(session.lastCommand == MSIME_FINISH_COMPOSITION);
        [controller setValue:composing forKey:@"view"];
    }

    // With nothing being composed the chord stays the application's: Ctrl+Backspace deletes a word
    // in the editor, and an input method that swallowed it would be taking a shortcut nobody gave it.
    [controller setValue:@{ @"focused": @YES, @"editing_text": @"", @"candidates": @[] } forKey:@"view"];
    session.lastCommand = UINT32_MAX;
    assert(![controller handleEvent:ModeKey(51, NSEventModifierFlagControl, NO) client:client]);
    assert(session.lastCommand == MSIME_FINISH_COMPOSITION);

    // A candidate page with no editing text still counts as a composition: the pinyin has been
    // consumed by earlier selections and the page is what is left to edit.
    [controller setValue:@{ @"focused": @YES, @"editing_text": @"", @"candidates": @[@{@"text": @"你好"}] }
                  forKey:@"view"];
    session.lastCommand = UINT32_MAX;
    assert([controller handleEvent:ModeKey(123, NSEventModifierFlagControl, NO) client:client]);
    assert(session.lastCommand == MSIME_MOVE_LEFT_SEGMENT);

    // A Ctrl+Backspace that emptied the reading of a half-chosen phrase leaves only the chosen piece, as the reference's `keep_creating_word_after_empty_raw` does. That is still a composition: the next Ctrl+Backspace deletes the piece instead of going to the application.
    NSDictionary *heldOnly = @{ @"focused": @YES, @"editing_text": @"", @"candidates": @[], @"phrase_prefix": @"海滩" };
    assert(MSIMEViewHasComposition(heldOnly));
    assert(!MSIMEViewHasComposition(@{ @"focused": @YES, @"editing_text": @"", @"candidates": @[], @"phrase_prefix": @"" }));
    assert(!MSIMEViewHasComposition(@{}));
    [controller setValue:heldOnly forKey:@"view"];
    session.lastCommand = UINT32_MAX;
    assert([controller handleEvent:ModeKey(51, NSEventModifierFlagControl, NO) client:client]);
    assert(session.lastCommand == MSIME_BACKSPACE_SEGMENT);
}

static void TestKeypadOperators(MSIMEAppearancePreferences *appearance) {
    ModeController *controller = [ModeController alloc];
    ShortcutSession *session = [ShortcutSession new];
    ShortcutClient *client = [ShortcutClient new];
    client.insertions = [NSMutableArray array];
    [controller setValue:appearance forKey:@"appearance"];
    [controller setValue:session forKey:@"session"];
    [controller setValue:client forKey:@"activeClient"];
    appearance.englishMode = NO;
    [controller setValue:@{ @"focused": @YES, @"editing_text": @"", @"candidates": @[] } forKey:@"view"];

    NSArray *operators = @[
        @[@67, @"*"], @[@69, @"+"], @[@75, @"/"], @[@78, @"-"],
        @[@81, @"="], @[@95, @","]
    ];
    NSUInteger expectedCalls = 0;
    for (NSArray *entry in operators) {
        const unsigned short keyCode = [entry[0] unsignedShortValue];
        const uint8_t mark = (uint8_t)[entry[1] characterAtIndex:0];
        session.enginePunctuationTransition = @{ @"handled": @YES, @"commit": entry[1],
            @"view": @{ @"focused": @YES, @"editing_text": @"", @"candidates": @[] } };
        session.punctuationASCIITransition = nil;
        client.committed = nil;
        assert([controller handleEvent:KeypadKey(keyCode, entry[1], 0, NO) client:client]);
        assert(++expectedCalls == session.enginePunctuationCalls && session.lastEnginePunctuation == mark);
        assert(session.punctuationASCIICalls == 0 && session.asciiCalls == 0);
        assert([client.committed isEqual:entry[1]]);
    }

    // Any active composition uses the literal route, including keypad '-'
    // and '=', so the main-row paging bindings cannot intercept them.
    [controller setValue:@{ @"focused": @YES, @"editing_text": @"nihao",
        @"candidates": @[@{ @"highlighted": @YES }] } forKey:@"view"];
    session.enginePunctuationTransition = nil;
    session.punctuationASCIITransition = @{ @"handled": @YES, @"commit": @"候选-",
        @"view": @{ @"focused": @YES, @"editing_text": @"", @"candidates": @[] } };
    client.committed = nil;
    assert([controller handleEvent:KeypadKey(78, @"-", 0, NO) client:client]);
    assert(session.punctuationASCIICalls == 1 && session.lastPunctuationASCII == '-');
    assert(session.enginePunctuationCalls == expectedCalls && [client.committed isEqual:@"候选-"]);

    const NSUInteger asciiCalls = session.punctuationASCIICalls;
    const NSUInteger engineCalls = session.enginePunctuationCalls;
    assert(![controller handleEvent:KeypadKey(69, @"+", NSEventModifierFlagControl, NO) client:client]);
    assert(session.punctuationASCIICalls == asciiCalls && session.enginePunctuationCalls == engineCalls);
}

static void TestSmartPunctuationPreferences() {
    NSString *suite = [@"msime.smart-punctuation." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    MSIMEAppearancePreferences *appearance = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    // The source ships the whole smart-punctuation family off, and a fresh macOS profile follows it.
    assert(!appearance.smartPunctuation && !appearance.smartPunctuationRepeatToChinese);
    assert([[appearance sharedPreferencesByMerging:@{}][@"smart_punctuation"] isEqual:@NO]);
    NSButton *smartButton = (id)PreferenceControl(appearance, @selector(smartPunctuationChanged:));
    NSButton *smartRepeatButton = (id)PreferenceControl(appearance, @selector(smartPunctuationRepeatChanged:));
    assert(smartButton.state == NSControlStateValueOff && smartRepeatButton.state == NSControlStateValueOff);
    smartButton.state = NSControlStateValueOn;
    [NSApp sendAction:smartButton.action to:smartButton.target from:smartButton];
    smartRepeatButton.state = NSControlStateValueOn;
    [NSApp sendAction:smartRepeatButton.action to:smartRepeatButton.target from:smartRepeatButton];
    assert(appearance.smartPunctuation && appearance.smartPunctuationRepeatToChinese);
    assert([[appearance sharedPreferencesByMerging:@{}][@"smart_punctuation"] isEqual:@YES]);
    smartButton.state = NSControlStateValueOff;
    [NSApp sendAction:smartButton.action to:smartButton.target from:smartButton];
    smartRepeatButton.state = NSControlStateValueOff;
    [NSApp sendAction:smartRepeatButton.action to:smartRepeatButton.target from:smartRepeatButton];
    assert(!appearance.smartPunctuation && !appearance.smartPunctuationRepeatToChinese);
    smartButton.state = NSControlStateValueOn;
    [NSApp sendAction:smartButton.action to:smartButton.target from:smartButton];
    smartRepeatButton.state = NSControlStateValueOn;
    [NSApp sendAction:smartRepeatButton.action to:smartRepeatButton.target from:smartRepeatButton];
    assert(appearance.smartPunctuation && appearance.smartPunctuationRepeatToChinese);
    assert([[appearance sharedPreferencesByMerging:@{}][@"smart_punctuation"] isEqual:@YES]);
    [appearance applySharedInputPreferences:@{@"smart_punctuation": @NO, @"smart_punctuation_repeat": @NO}];
    assert(!appearance.smartPunctuation && !appearance.smartPunctuationRepeatToChinese);
    [appearance applySharedInputPreferences:@{@"smart_punctuation": @1, @"smart_punctuation_repeat": @"true"}];
    assert(!appearance.smartPunctuation && !appearance.smartPunctuationRepeatToChinese);
    appearance.smartPunctuation = YES;
    appearance.smartPunctuationRepeatToChinese = YES;

    ModeController *controller = [ModeController alloc];
    ShortcutSession *session = [ShortcutSession new];
    ShortcutClient *client = [ShortcutClient new];
    client.document = @"a";
    client.selection = NSMakeRange(1, 0);
    [controller setValue:appearance forKey:@"appearance"];
    [controller setValue:session forKey:@"session"];
    [controller setValue:client forKey:@"activeClient"];
    [controller setValue:@{@"focused": @YES, @"editing_text": @"", @"candidates": @[]} forKey:@"view"];
    session.contextualPunctuationTransition = @{@"handled": @YES, @"commit": @".",
        @"view": @{@"focused": @YES, @"editing_text": @"", @"candidates": @[]}};
    NSEvent *period = [NSEvent keyEventWithType:NSEventTypeKeyDown location:NSZeroPoint modifierFlags:0 timestamp:0
                                      windowNumber:0 context:nil characters:@"." charactersIgnoringModifiers:@"." isARepeat:NO keyCode:47];
    assert([controller handleEvent:period client:client]);
    assert(session.contextualPunctuationCalls == 1 && session.lastContextualPunctuation == '.' && session.lastPrecedingScalar == 'a');
    assert([client.document isEqual:@"a."]);
    client.selection = NSMakeRange(2, 0);
    session.contextualPunctuationTransition = @{@"handled": @YES, @"commit": @".",
        @"view": @{@"focused": @YES, @"editing_text": @"", @"candidates": @[]}};
    assert([controller handleEvent:period client:client]);
    assert([client.document isEqual:@"a。"] && [client.committed isEqual:@"。"]);

    ModeController *composingController = [ModeController alloc];
    ShortcutSession *composingSession = [ShortcutSession new];
    ShortcutClient *composingClient = [ShortcutClient new];
    composingClient.document = @"";
    [composingController setValue:appearance forKey:@"appearance"];
    [composingController setValue:composingSession forKey:@"session"];
    [composingController setValue:composingClient forKey:@"activeClient"];
    [composingController setValue:@{@"focused": @YES, @"editing_text": @"abc",
        @"candidates": @[@{@"text": @"abc", @"highlighted": @YES}]} forKey:@"view"];
    composingSession.punctuationASCIITransition = @{@"handled": @YES, @"commit": @"abc.",
        @"view": @{@"focused": @YES, @"editing_text": @"", @"candidates": @[]}};
    assert([composingController handleEvent:period client:composingClient]);
    assert(composingSession.punctuationASCIICalls == 1 && [composingClient.committed isEqual:@"abc."]);

    // A non-ASCII preceding scalar stays on the normal Engine route rather
    // than using the contextual ASCII fast path.
    ModeController *nonASCIIController = [ModeController alloc];
    ShortcutSession *nonASCIISession = [ShortcutSession new];
    ShortcutClient *nonASCIIClient = [ShortcutClient new];
    nonASCIIClient.document = @"中";
    nonASCIIClient.selection = NSMakeRange(1, 0);
    [nonASCIIController setValue:appearance forKey:@"appearance"];
    [nonASCIIController setValue:nonASCIISession forKey:@"session"];
    [nonASCIIController setValue:nonASCIIClient forKey:@"activeClient"];
    [nonASCIIController setValue:@{ @"focused": @YES, @"editing_text": @"", @"candidates": @[] } forKey:@"view"];
    nonASCIISession.nextTransition = @{ @"handled": @YES, @"commit": @"，",
        @"view": @{ @"focused": @YES, @"editing_text": @"", @"candidates": @[] } };
    assert([nonASCIIController handleEvent:period client:nonASCIIClient]);
    assert(nonASCIISession.contextualPunctuationCalls == 0 && nonASCIISession.asciiCalls == 1);

    // Full-width input applies to the idle smart-punctuation transition too.
    ModeController *fullWidthController = [ModeController alloc];
    ShortcutSession *fullWidthSession = [ShortcutSession new];
    ShortcutClient *fullWidthClient = [ShortcutClient new];
    fullWidthClient.document = @"a";
    fullWidthClient.selection = NSMakeRange(1, 0);
    appearance.fullWidthInput = YES;
    [fullWidthController setValue:appearance forKey:@"appearance"];
    [fullWidthController setValue:fullWidthSession forKey:@"session"];
    [fullWidthController setValue:fullWidthClient forKey:@"activeClient"];
    [fullWidthController setValue:@{ @"focused": @YES, @"editing_text": @"", @"candidates": @[] } forKey:@"view"];
    fullWidthSession.contextualPunctuationTransition = @{ @"handled": @YES, @"commit": @".",
        @"view": @{ @"focused": @YES, @"editing_text": @"", @"candidates": @[] } };
    assert([fullWidthController handleEvent:period client:fullWidthClient]);
    assert([fullWidthClient.document isEqual:@"a．"]);
    appearance.fullWidthInput = NO;
    appearance.smartPunctuation = NO;
    assert(![[appearance sharedPreferencesByMerging:@{}][@"smart_punctuation"] boolValue]);
    [defaults removePersistentDomainForName:suite];
}

static void TestScreenKeyboardShortcut(MSIMEAppearancePreferences *appearance) {
    ModeController *controller = [ModeController alloc];
    ShortcutClient *client = [ShortcutClient new];
    [controller setValue:appearance forKey:@"appearance"];
    [controller setValue:[ShortcutSession new] forKey:@"session"];
    [controller setValue:client forKey:@"activeClient"];
    const BOOL previousEnglishMode = appearance.englishMode;
    const NSEventModifierFlags chord = NSEventModifierFlagControl |
        NSEventModifierFlagShift | NSEventModifierFlagCommand;

    appearance.englishMode = NO;
    assert([controller handleEvent:ModeKey(40, chord, NO) client:client]);
    assert(controller.screenKeyboardCalls == 1);
    assert([controller handleEvent:ModeKey(40, chord, YES) client:client]);
    assert(controller.screenKeyboardCalls == 1);
    assert([controller handleEvent:ModeKey(40, chord | NSEventModifierFlagCapsLock, NO) client:client]);
    assert(controller.screenKeyboardCalls == 2);

    appearance.englishMode = YES;
    assert([controller handleEvent:ModeKey(40, chord, NO) client:client]);
    assert(controller.screenKeyboardCalls == 3);
    for (NSNumber *modifiers in @[
        @(chord & ~NSEventModifierFlagControl),
        @(chord & ~NSEventModifierFlagShift),
        @(chord & ~NSEventModifierFlagCommand),
        @(chord | NSEventModifierFlagOption)
    ]) {
        assert(![controller handleEvent:ModeKey(40, modifiers.unsignedIntegerValue, NO) client:client]);
        assert(controller.screenKeyboardCalls == 3);
    }
    assert(![controller handleEvent:ModeKey(39, chord, NO) client:client]);
    assert(controller.screenKeyboardCalls == 3);
    appearance.englishMode = previousEnglishMode;
}

static void TestMaintenanceShortcuts(MSIMEAppearancePreferences *appearance) {
    ModeController *controller = [ModeController alloc];
    ShortcutClient *client = [ShortcutClient new];
    ShortcutSession *session = [ShortcutSession new];
    session.resetCacheTransition = @{@"handled": @YES, @"view": @{
        @"focused": @YES, @"editing_text": @"", @"candidates": @[]
    }};
    [controller setValue:appearance forKey:@"appearance"];
    [controller setValue:session forKey:@"session"];
    [controller setValue:client forKey:@"activeClient"];
    const BOOL previousEnglishMode = appearance.englishMode;
    const NSEventModifierFlags chord = NSEventModifierFlagControl |
        NSEventModifierFlagShift | NSEventModifierFlagOption;

    appearance.englishMode = NO;
    assert([controller handleEvent:ModeKey(8, chord, NO) client:client]);
    assert(session.resetCacheCalls == 1);
    assert([controller handleEvent:ModeKey(15, chord, NO) client:client]);
    assert(controller.restartCalls == 1);
    assert([controller handleEvent:ModeKey(17, chord, NO) client:client]);
    assert(controller.terminationCalls == 1);

    for (NSNumber *code in @[@8, @15, @17])
        assert([controller handleEvent:ModeKey(code.unsignedShortValue, chord, YES) client:client]);
    assert(session.resetCacheCalls == 1 && controller.restartCalls == 1 && controller.terminationCalls == 1);

    assert([controller handleEvent:ModeKey(8, chord | NSEventModifierFlagCapsLock, NO) client:client]);
    assert(session.resetCacheCalls == 2);
    appearance.englishMode = YES;
    assert([controller handleEvent:ModeKey(15, chord, NO) client:client]);
    assert(controller.restartCalls == 2);

    for (NSNumber *modifiers in @[
        @(chord & ~NSEventModifierFlagControl),
        @(chord & ~NSEventModifierFlagShift),
        @(chord & ~NSEventModifierFlagOption),
        @(chord | NSEventModifierFlagCommand)
    ]) {
        assert(![controller handleEvent:ModeKey(8, modifiers.unsignedIntegerValue, NO) client:client]);
    }
    assert(![controller handleEvent:ModeKey(9, chord, NO) client:client]);
    assert(session.resetCacheCalls == 2 && controller.restartCalls == 2 && controller.terminationCalls == 1);
    appearance.englishMode = previousEnglishMode;
}

static NSEvent *TapEvent(NSEventType type, unsigned short key, NSEventModifierFlags flags, double time) {
    return [NSEvent keyEventWithType:type location:NSZeroPoint modifierFlags:flags timestamp:time windowNumber:0 context:nil characters:@"" charactersIgnoringModifiers:@"" isARepeat:NO keyCode:key];
}

static NSUInteger baseDeactivationCalls;
static void RecordBaseDeactivation(id object, SEL selector, id sender) {
    (void)object; (void)selector; (void)sender; ++baseDeactivationCalls;
}

@interface DeactivationToolbar : NSObject
@property(nonatomic) NSUInteger calls;
@end
@implementation DeactivationToolbar
- (void)deactivateForDelegate:(id)delegate { (void)delegate; ++self.calls; }
- (void)updateEnglishInputMode:(BOOL)englishInputMode
         englishCandidateMode:(BOOL)englishCandidateMode
             japaneseInputMode:(BOOL)japaneseInputMode
                      capsLock:(BOOL)capsLock
          chinesePunctuationEnabled:(BOOL)chinesePunctuationEnabled
                   fullWidthEnabled:(BOOL)fullWidthEnabled
    traditionalChineseOutputEnabled:(BOOL)traditionalChineseOutputEnabled {
    (void)englishInputMode;
    (void)englishCandidateMode;
    (void)japaneseInputMode;
    (void)capsLock;
    (void)chinesePunctuationEnabled;
    (void)fullWidthEnabled;
    (void)traditionalChineseOutputEnabled;
}
@end

static void TestStaleClientDeactivation() {
    NSString *suite = [@"msime.deactivation." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    MSIMEAppearancePreferences *prefs = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    ModeController *controller = [ModeController alloc];
    ShortcutClient *oldClient = [ShortcutClient new], *current = [ShortcutClient new];
    ShortcutSession *session = [ShortcutSession new];
    TestCandidatePanel *panel = [TestCandidatePanel new], *keymap = [TestCandidatePanel new];
    DeactivationToolbar *toolbar = [DeactivationToolbar new];
    NSTimer *timer = [NSTimer timerWithTimeInterval:1 repeats:YES block:^(NSTimer *unused) { (void)unused; }];
    NSDictionary *view = @{@"focused":@YES, @"editing_text":@"test", @"candidates":@[]};
    [controller setValue:prefs forKey:@"appearance"];
    [controller setValue:current forKey:@"activeClient"];
    [controller setValue:session forKey:@"session"];
    [controller setValue:panel forKey:@"panel"];
    [controller setValue:keymap forKey:@"keymapPanel"];
    [controller setValue:toolbar forKey:@"toolbar"];
    [controller setValue:timer forKey:@"preferencesTimer"];
    [controller setValue:view forKey:@"view"];
    panel.visible = keymap.visible = YES;
    current.marked = @"test";
    // Isolate superclass IPC in this controller test, while recording whether
    // stale callbacks reach it. Actual installed IMK delivery is a separate gate.
    Method base = class_getInstanceMethod(IMKInputController.class, @selector(deactivateServer:));
    assert(base);
    baseDeactivationCalls = 0;
    IMP original = method_setImplementation(base, (IMP)RecordBaseDeactivation);
    auto down = TapEvent(NSEventTypeFlagsChanged, 56, NSEventModifierFlagShift, 1.0);
    auto up = TapEvent(NSEventTypeFlagsChanged, 56, 0, 1.1);
    assert(![controller handleEvent:down client:current]);
    for (id stale in @[oldClient, NSNull.null]) {
        [controller deactivateServer:stale == NSNull.null ? nil : stale];
        assert([controller valueForKey:@"activeClient"] == current);
        assert([[controller valueForKey:@"view"] isEqual:view]);
        assert([current.marked isEqual:@"test"] && current.committed == nil);
        assert(panel.visible && keymap.visible && timer.valid);
        assert(session.focusCalls == 0 && toolbar.calls == 0 && baseDeactivationCalls == 0);
    }
    // The current client's pending tap must survive an unrelated deactivation.
    assert([controller handleEvent:up client:current] && prefs.englishMode);
    assert([current.committed isEqual:@"测试"]);
    panel.visible = keymap.visible = YES;
    [controller deactivateServer:current];
    assert([controller valueForKey:@"activeClient"] == nil);
    assert([controller valueForKey:@"preferencesTimer"] == nil && !timer.valid);
    assert(!panel.visible && !keymap.visible && current.marked.length == 0);
    // A focus-out is the reference's ClientSuspended: the floating toolbar keeps its owner and stays visible, and only an input-source switch (or the controller going away) releases it.
    assert(session.focusCalls == 1 && toolbar.calls == 0 && baseDeactivationCalls == 1);
    [controller deactivateServer:current];
    assert(session.focusCalls == 1 && toolbar.calls == 0 && baseDeactivationCalls == 1);
    method_setImplementation(base, original);
    MSIMERemoveTestPreferenceSuite(defaults, suite);
}

@interface ControlledPreferenceRead : NSObject
@property(nonatomic, strong) dispatch_semaphore_t started;
@property(nonatomic, strong) dispatch_semaphore_t released;
@property(nonatomic, copy) NSDictionary *snapshot;
@end
@implementation ControlledPreferenceRead
- (instancetype)init {
    self = [super init];
    if (self) { _started = dispatch_semaphore_create(0); _released = dispatch_semaphore_create(0); }
    return self;
}
@end

@interface AsyncPreferenceSession : ShortcutSession
@property(nonatomic) NSUInteger updates;
@end
@implementation AsyncPreferenceSession
- (NSDictionary *)updatePreferencesSnapshot:(NSDictionary *)snapshot error:(NSError **)error {
    (void)snapshot; (void)error; ++self.updates;
    return @{@"view":@{@"focused":@YES, @"editing_text":@"", @"candidates":@[]}};
}
@end

@interface AsyncPreferencesController : ModeController
@property(nonatomic, copy) NSArray<ControlledPreferenceRead *> *reads;
@property(nonatomic) NSUInteger readCalls;
@property(nonatomic) NSUInteger completions;
@property(nonatomic, strong) NSMutableArray<NSDictionary *> *appliedPreferences;
@end
@implementation AsyncPreferencesController
- (NSDictionary *)readPreferencesSnapshotInDirectory:(NSString *)directory error:(NSError **)error {
    (void)directory; (void)error;
    assert(!NSThread.isMainThread);
    ControlledPreferenceRead *read;
    @synchronized(self) { assert(self.readCalls < self.reads.count); read = self.reads[self.readCalls++]; }
    dispatch_semaphore_signal(read.started);
    assert(dispatch_semaphore_wait(read.released, dispatch_time(DISPATCH_TIME_NOW, 5 * NSEC_PER_SEC)) == 0);
    return read.snapshot;
}
- (void)completePreferenceLoad:(NSDictionary *)snapshot error:(NSError *)error generation:(uint64_t)generation
                       session:(MSIMEClientSession *)session client:(id)client {
    assert(NSThread.isMainThread);
    [super completePreferenceLoad:snapshot error:error generation:generation session:session client:client];
    ++self.completions;
}
- (void)applySharedToolbarPreferences:(NSDictionary *)preferences {
    [self.appliedPreferences addObject:preferences];
    [super applySharedToolbarPreferences:preferences];
}
@end

// Lays out every window whose layout is still owed, before a bounded wait starts its clock. A test that builds a settings window and never turns the run loop leaves that window's constraint and layout pass pending, and AppKit settles it from a run-loop observer on the next turn, whichever wait that happens to be: measured at about a second for the windows the earlier cases leave behind, several times that under AddressSanitizer, which is how DrainMainQueue and the other waits below timed out on CI without anything they wait for being late. Settled here, unbounded, the clock only measures what the wait is for.
static void SettleWindowLayout() {
    for (NSWindow *window in NSApp.windows) {
        [window updateConstraintsIfNeeded];
        [window layoutIfNeeded];
    }
}

static void WaitForPreferenceCompletions(AsyncPreferencesController *controller, NSUInteger count) {
    SettleWindowLayout();
    NSDate *deadline = [NSDate dateWithTimeIntervalSinceNow:2];
    while (controller.completions < count && deadline.timeIntervalSinceNow > 0)
        [NSRunLoop.currentRunLoop runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.005]];
    assert(controller.completions == count);
}

static void TestPreferenceClientGeneration() {
    for (NSNumber *returnToFirst in @[@NO, @YES]) {
        NSString *suite = [@"msime.preference-focus." stringByAppendingString:NSUUID.UUID.UUIDString];
        NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
        MSIMEAppearancePreferences *prefs = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
        prefs.englishMode = YES;
        AsyncPreferencesController *controller = [AsyncPreferencesController alloc];
        ShortcutClient *first = [ShortcutClient new], *second = [ShortcutClient new];
        AsyncPreferenceSession *session = returnToFirst.boolValue ? [AsyncPreferenceSession new] : nil;
        NSMutableArray *reads = [NSMutableArray array];
        for (NSNumber *enabled in @[@NO, @YES, @NO]) {
            ControlledPreferenceRead *read = [ControlledPreferenceRead new];
            read.snapshot = @{@"preferences":@{@"chinese_punctuation":enabled}};
            [reads addObject:read];
        }
        controller.reads = reads;
        controller.appliedPreferences = [NSMutableArray array];
        [controller setValue:prefs forKey:@"appearance"];
        [controller setValue:first forKey:@"activeClient"];
        [controller setValue:session forKey:@"session"];
        [controller setValue:@"/synthetic-preferences" forKey:@"preferencesDirectory"];
        [controller reloadPreferences];
        assert(dispatch_semaphore_wait(controller.reads[0].started, dispatch_time(DISPATCH_TIME_NOW, NSEC_PER_SEC)) == 0);
        NSEvent *release = TapEvent(NSEventTypeFlagsChanged, 56, 0, 1);
        assert(![controller handleEvent:release client:second]);
        if (returnToFirst.boolValue) assert(![controller handleEvent:release client:first]);
        [controller reloadPreferences];
        assert(dispatch_semaphore_wait(controller.reads[1].started, dispatch_time(DISPATCH_TIME_NOW, NSEC_PER_SEC)) == 0);
        dispatch_semaphore_signal(controller.reads[0].released);
        WaitForPreferenceCompletions(controller, 1);
        assert(controller.appliedPreferences.count == 0 && session.updates == 0);
        // Completing the old request must not release the newer request's gate.
        [controller reloadPreferences];
        dispatch_semaphore_signal(controller.reads[1].released);
        WaitForPreferenceCompletions(controller, 2);
        assert(controller.readCalls == 2 && controller.appliedPreferences.count == 1);
        assert([controller.appliedPreferences[0][@"chinese_punctuation"] isEqual:@YES]);
        assert(session.updates == (session ? 1 : 0));
        dispatch_semaphore_signal(controller.reads[2].released);
        [controller reloadPreferences];
        WaitForPreferenceCompletions(controller, 3);
        assert(controller.appliedPreferences.count == 2 && !prefs.chinesePunctuation);
        assert(session.updates == (session ? 2 : 0));
        MSIMERemoveTestPreferenceSuite(defaults, suite);
    }
}

// The poll reads the preferences document once a second, and most of those reads find exactly what
// was applied a second ago. Applying it again walks every preference, goes back into the Engine and
// writes a diagnostic line - once a second, for nothing. It also buried the diagnostic log under
// `preferences_applied`, which is how it was noticed at all.
static void TestPreferenceRevisionSkipsUnchangedDocuments() {
    NSString *suite = [@"msime.preference-revision." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    MSIMEAppearancePreferences *prefs = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    AsyncPreferencesController *controller = [AsyncPreferencesController alloc];
    ShortcutClient *client = [ShortcutClient new];
    AsyncPreferenceSession *session = [AsyncPreferenceSession new];
    NSMutableArray *reads = [NSMutableArray array];
    // Same revision twice, then a new one, then the first revision again after a local edit.
    for (NSArray *fixture in @[@[@7, @YES], @[@7, @YES], @[@8, @NO]]) {
        ControlledPreferenceRead *read = [ControlledPreferenceRead new];
        read.snapshot = @{@"revision":fixture[0], @"preferences":@{@"chinese_punctuation":fixture[1]}};
        [reads addObject:read];
    }
    controller.reads = reads;
    controller.appliedPreferences = [NSMutableArray array];
    [controller setValue:prefs forKey:@"appearance"];
    [controller setValue:client forKey:@"activeClient"];
    [controller setValue:session forKey:@"session"];
    [controller setValue:@"/synthetic-preferences" forKey:@"preferencesDirectory"];

    [controller reloadPreferences];
    assert(dispatch_semaphore_wait(controller.reads[0].started, dispatch_time(DISPATCH_TIME_NOW, NSEC_PER_SEC)) == 0);
    dispatch_semaphore_signal(controller.reads[0].released);
    WaitForPreferenceCompletions(controller, 1);
    assert(controller.appliedPreferences.count == 1 && session.updates == 1);

    // The second read finds the same revision: nothing is applied and the Engine is not disturbed.
    [controller reloadPreferences];
    assert(dispatch_semaphore_wait(controller.reads[1].started, dispatch_time(DISPATCH_TIME_NOW, NSEC_PER_SEC)) == 0);
    dispatch_semaphore_signal(controller.reads[1].released);
    WaitForPreferenceCompletions(controller, 2);
    assert(controller.readCalls == 2);
    assert(controller.appliedPreferences.count == 1 && session.updates == 1);

    // A document that actually changed is applied.
    [controller reloadPreferences];
    assert(dispatch_semaphore_wait(controller.reads[2].started, dispatch_time(DISPATCH_TIME_NOW, NSEC_PER_SEC)) == 0);
    dispatch_semaphore_signal(controller.reads[2].released);
    WaitForPreferenceCompletions(controller, 3);
    assert(controller.appliedPreferences.count == 2 && session.updates == 2);
    assert([controller.appliedPreferences[1][@"chinese_punctuation"] isEqual:@NO]);

    // A local edit invalidating what was applied - so a document rolled back to a revision this
    // session already saw is applied again - is the load state's own rule, pinned next to it in
    // preference-load-state rather than here: appearanceChanged: reaches half the controller.
    MSIMERemoveTestPreferenceSuite(defaults, suite);
}

// A preferences document that is not JSON is repaired by the host and the repaired snapshot applied, once per directory: a failure that repair cannot fix must not turn the one-second poll into a repair attempt every second.
@interface RecoveringPreferencesController : ModeController
@property(nonatomic, copy) NSDictionary *readSnapshot;
@property(nonatomic, copy) NSDictionary *recovery;
@property(nonatomic) NSUInteger readCalls;
@property(nonatomic) NSUInteger recoverCalls;
@property(nonatomic) NSUInteger completions;
@property(nonatomic, strong) NSMutableArray<NSDictionary *> *appliedPreferences;
@end
@implementation RecoveringPreferencesController
- (NSDictionary *)readPreferencesSnapshotInDirectory:(NSString *)directory error:(NSError **)error {
    (void)directory;
    assert(!NSThread.isMainThread);
    @synchronized(self) { ++self.readCalls; }
    if (self.readSnapshot) return self.readSnapshot;
    if (error) *error = [NSError errorWithDomain:@"test" code:1 userInfo:@{NSLocalizedDescriptionKey: @"invalid preferences document: EOF"}];
    return nil;
}
- (NSDictionary *)recoverPreferencesInDirectory:(NSString *)directory error:(NSError **)error {
    (void)directory; (void)error;
    assert(!NSThread.isMainThread);
    @synchronized(self) { ++self.recoverCalls; }
    return self.recovery;
}
- (void)completePreferenceLoad:(NSDictionary *)snapshot error:(NSError *)error generation:(uint64_t)generation
                       session:(MSIMEClientSession *)session client:(id)client {
    [super completePreferenceLoad:snapshot error:error generation:generation session:session client:client];
    ++self.completions;
}
- (void)applySharedToolbarPreferences:(NSDictionary *)preferences {
    [self.appliedPreferences addObject:preferences];
    [super applySharedToolbarPreferences:preferences];
}
@end

static void WaitForRecoveringCompletions(RecoveringPreferencesController *controller, NSUInteger count) {
    SettleWindowLayout();
    NSDate *deadline = [NSDate dateWithTimeIntervalSinceNow:2];
    while (controller.completions < count && deadline.timeIntervalSinceNow > 0)
        [NSRunLoop.currentRunLoop runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.005]];
    assert(controller.completions == count);
}

static void TestUnreadablePreferencesAreRecoveredOnce() {
    NSString *suite = [@"msime.preference-recovery." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    MSIMEAppearancePreferences *prefs = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    enum Case { Repaired, Refused, Readable };
    for (int kase : {Repaired, Refused, Readable}) {
        RecoveringPreferencesController *controller = [RecoveringPreferencesController alloc];
        controller.appliedPreferences = [NSMutableArray array];
        NSDictionary *repaired = @{@"format_version":@1, @"revision":@1758620000, @"preferences":@{@"chinese_punctuation":@NO}};
        if (kase == Readable) controller.readSnapshot = @{@"revision":@3, @"preferences":@{@"chinese_punctuation":@YES}};
        controller.recovery = kase == Repaired
            ? @{@"recovered":@YES, @"snapshot":repaired, @"backup_name":@"preferences.json.corrupt-20260923-101500", @"salvaged":@NO}
            : nil;
        [controller setValue:prefs forKey:@"appearance"];
        [controller setValue:[ShortcutClient new] forKey:@"activeClient"];
        [controller setValue:[@"/synthetic-recovery-" stringByAppendingString:NSUUID.UUID.UUIDString] forKey:@"preferencesDirectory"];

        [controller reloadPreferences];
        WaitForRecoveringCompletions(controller, 1);
        assert(controller.readCalls == 1);
        assert(controller.recoverCalls == (kase == Readable ? 0u : 1u));
        if (kase == Repaired) {
            assert(controller.appliedPreferences.count == 1);
            assert([controller.appliedPreferences[0][@"chinese_punctuation"] isEqual:@NO]);
        } else if (kase == Refused) {
            assert(controller.appliedPreferences.count == 0);
        } else {
            assert(controller.appliedPreferences.count == 1);
        }

        // The poll comes round with the document failing: a directory gets one repair attempt in all, whether or not the first one found anything to repair.
        controller.readSnapshot = nil;
        [controller reloadPreferences];
        WaitForRecoveringCompletions(controller, 2);
        assert(controller.readCalls == 2);
        assert(controller.recoverCalls == 1);
    }
    MSIMERemoveTestPreferenceSuite(defaults, suite);
}

@interface VoiceSettingsPersistenceController : AsyncPreferencesController
@property(nonatomic) NSUInteger persistenceRequests;
@end
@implementation VoiceSettingsPersistenceController
- (void)persistAppearancePreferences { ++self.persistenceRequests; }
@end

static void TestProviderSettingsPersistTheSharedSnapshot() {
    VoiceSettingsPersistenceController *controller = [VoiceSettingsPersistenceController alloc];
    ControlledPreferenceRead *read = [ControlledPreferenceRead new];
    read.snapshot = @{@"revision":@7, @"preferences":@{@"voice_input":@{@"asr_provider":@"openai"}}};
    controller.reads = @[read];
    controller.appliedPreferences = [NSMutableArray array];
    AsyncPreferenceSession *session = [AsyncPreferenceSession new];
    [controller setValue:session forKey:@"session"];
    [controller setValue:[ShortcutClient new] forKey:@"activeClient"];
    [controller setValue:@"/synthetic-preferences" forKey:@"preferencesDirectory"];
    [controller reloadPreferences];
    assert(dispatch_semaphore_wait(read.started, dispatch_time(DISPATCH_TIME_NOW, NSEC_PER_SEC)) == 0);
    [controller voiceProviderSettingsChanged:nil];
    assert(controller.persistenceRequests == 1);
    dispatch_semaphore_signal(read.released);
    WaitForPreferenceCompletions(controller, 1);
    assert(controller.appliedPreferences.count == 0 && session.updates == 0);
}

// The synthetic keyboard these cases drive: a key is held between its down and its up, which is what
// the detector now asks about instead of trusting its own record. The record can lose a release -
// focus moves while a key is down, or the host is told about fewer event kinds - and a stale entry
// used to refuse every tap for the rest of the session.
static std::set<unsigned short> gHeldKeys;
static bool SyntheticKeyHeld(unsigned short key) { return gHeldKeys.count(key) != 0; }

// A key whose release never arrived. The host can simply not be told: focus moves while the key is
// down, the application takes the release, or the event kinds the host asked for do not include it.
// The record then holds that key forever, and before the detector checked what is actually held,
// every tap for the rest of the session was refused - with nothing on screen to explain it.
static void TestModifierTapSurvivesALostRelease() {
    MSIMEModifierTap tap;
    gHeldKeys.clear();
    tap.setKeyHeldProbe(&SyntheticKeyHeld);
    auto down = TapEvent(NSEventTypeFlagsChanged, 56, NSEventModifierFlagShift, 2.0);
    auto up = TapEvent(NSEventTypeFlagsChanged, 56, 0, 2.1);

    // A letter is typed and its release is never delivered.
    gHeldKeys.insert(0);
    assert(!tap.observe(TapEvent(NSEventTypeKeyDown, 0, 0, 1.0), true, true));
    gHeldKeys.erase(0);

    // The tap works anyway, because the key is not being held.
    assert(!tap.observe(down, true, true));
    assert(tap.observe(up, true, true));

    // And a key that really is held still cancels it, which is the rule the record was there for.
    gHeldKeys.insert(0);
    assert(!tap.observe(TapEvent(NSEventTypeKeyDown, 0, 0, 3.0), true, true));
    assert(!tap.observe(TapEvent(NSEventTypeFlagsChanged, 56, NSEventModifierFlagShift, 3.1), true, true));
    assert(!tap.observe(TapEvent(NSEventTypeFlagsChanged, 56, 0, 3.2), true, true));
    gHeldKeys.clear();
}

static void TestModifierTaps() {
    for (NSNumber *key in @[@56, @60, @59, @62]) {
        const auto code = key.unsignedShortValue;
        const auto flag = code == 56 || code == 60 ? NSEventModifierFlagShift : NSEventModifierFlagControl;
        auto down = TapEvent(NSEventTypeFlagsChanged, code, flag, 1.0);
        auto up = TapEvent(NSEventTypeFlagsChanged, code, 0, 1.1);
        MSIMEModifierTap tap;
        gHeldKeys.clear();
        tap.setKeyHeldProbe(&SyntheticKeyHeld);
        assert(!tap.observe(up, true, true)); // A release after focus acquisition cannot toggle.
        assert(!tap.observe(down, true, true));
        assert(tap.observe(up, true, true));
        assert(!tap.observe(up, true, true));
        for (double time : {1.5, 2.0, 0.9}) {
            tap.reset();
            assert(!tap.observe(down, true, true));
            assert(!tap.observe(TapEvent(NSEventTypeFlagsChanged, code, 0, time), true, true));
        }
        tap.reset();
        assert(!tap.observe(down, false, false));
        assert(!tap.observe(up, true, true)); // Enabling while held does not arm.
        assert(!tap.observe(down, true, true));
        assert(!tap.observe(up, false, false));
        tap.reset();
        assert(!tap.observe(down, true, true));
        tap.reset();
        assert(!tap.observe(up, true, true));
        for (auto other : {NSEventModifierFlagCommand, NSEventModifierFlagOption, NSEventModifierFlagFunction,
                          flag == NSEventModifierFlagShift ? NSEventModifierFlagControl : NSEventModifierFlagShift}) {
            tap.reset();
            assert(!tap.observe(down, true, true));
            assert(!tap.observe(TapEvent(NSEventTypeFlagsChanged, 55, flag | other, 1.02), true, true));
            assert(!tap.observe(TapEvent(NSEventTypeFlagsChanged, 55, flag, 1.04), true, true));
            assert(!tap.observe(up, true, true));
        }
        // A held non-modifier and typing while held cancel.
        tap.reset();
        gHeldKeys.insert(0);
        assert(!tap.observe(TapEvent(NSEventTypeKeyDown, 0, 0, 0.9), true, true));
        assert(!tap.observe(down, true, true));
        assert(!tap.observe(up, true, true));
        gHeldKeys.erase(0);
        assert(!tap.observe(TapEvent(NSEventTypeKeyUp, 0, 0, 1.2), true, true));
        assert(!tap.observe(down, true, true));
        assert(tap.observe(up, true, true));
        assert(!tap.observe(down, true, true));
        gHeldKeys.insert(0);
        assert(!tap.observe(TapEvent(NSEventTypeKeyDown, 0, flag, 1.02), true, true));
        gHeldKeys.erase(0);
        assert(!tap.observe(TapEvent(NSEventTypeKeyUp, 0, flag, 1.04), true, true));
        assert(!tap.observe(up, true, true));
        assert(!tap.observe(down, true, true));
        unsigned short otherSide = code == 56 ? 60 : code == 60 ? 56 : code == 59 ? 62 : 59;
        assert(!tap.observe(TapEvent(NSEventTypeFlagsChanged, otherSide, flag, 1.02), true, true));
        assert(!tap.observe(TapEvent(NSEventTypeFlagsChanged, otherSide, flag, 1.04), true, true));
        assert(tap.observe(up, true, true));
        assert(!tap.observe(down, true, true));
        assert(!tap.observe(TapEvent(NSEventTypeFlagsChanged, otherSide, flag, 1.02), true, true));
        assert(!tap.observe(TapEvent(NSEventTypeFlagsChanged, code, flag, 1.04), true, true));
        assert(tap.observe(TapEvent(NSEventTypeFlagsChanged, otherSide, 0, 1.1), true, true));
    }

    NSString *suite = [@"msime.modifier-taps." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    MSIMEAppearancePreferences *prefs = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    assert(prefs.shiftTapShortcut && !prefs.controlTapShortcut);
    assert(![prefs sharedPreferencesByMerging:@{}][@"keybindings"]);
    NSDictionary *keys = @{@"switch_language_shift":@NO, @"switch_language_ctrl":@YES, @"switch_language_ctrl_alt_space":@NO};
    [prefs applySharedInputPreferences:@{@"keybindings":keys}];
    assert(!prefs.shiftTapShortcut && !prefs.inputModeShortcut && prefs.controlTapShortcut);
    assert([[prefs sharedPreferencesByMerging:@{@"keybindings":keys}][@"keybindings"] isEqual:keys]);
    [prefs applySharedInputPreferences:@{@"keybindings":@{@"switch_language_shift":@1, @"switch_language_ctrl":@"false"}}];
    assert(!prefs.shiftTapShortcut && !prefs.inputModeShortcut && prefs.controlTapShortcut);
    NSButton *shift = (id)PreferenceControl(prefs, NSSelectorFromString(@"shiftTapShortcutChanged:"));
    NSButton *control = (id)PreferenceControl(prefs, NSSelectorFromString(@"controlTapShortcutChanged:"));
    assert(shift.state == NSControlStateValueOff && control.state == NSControlStateValueOn);
    shift.state = NSControlStateValueOn;
    assert([NSApp sendAction:shift.action to:shift.target from:shift]);
    control.state = NSControlStateValueOff;
    assert([NSApp sendAction:control.action to:control.target from:control]);
    MSIMEAppearancePreferences *reopened = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    assert(reopened.shiftTapShortcut && !reopened.controlTapShortcut);
    NSDictionary *merged = [MSIMEMergePreferenceSnapshot(@{@"keybindings":keys}, [prefs sharedPreferencesByMerging:@{}]) objectForKey:@"keybindings"];
    assert(([merged isEqual:@{@"switch_language_shift":@YES, @"switch_language_ctrl":@NO, @"switch_language_ctrl_alt_space":@NO}]));

    ModeController *controller = [ModeController alloc];
    ShortcutClient *client = [ShortcutClient new];
    ShortcutSession *session = [ShortcutSession new];
    [controller setValue:prefs forKey:@"appearance"];
    [controller setValue:client forKey:@"activeClient"];
    [controller setValue:session forKey:@"session"];
    assert([controller recognizedEvents:client] == (NSEventMaskKeyDown | NSEventMaskKeyUp | NSEventMaskFlagsChanged));
    auto down = TapEvent(NSEventTypeFlagsChanged, 56, NSEventModifierFlagShift, 1.0);
    auto up = TapEvent(NSEventTypeFlagsChanged, 56, 0, 1.1);
    session.failFinish = YES;
    session.failCancel = YES;
    assert(![controller handleEvent:down client:client]);
    assert([controller handleEvent:up client:client]);
    assert(!prefs.englishMode);
    session.failFinish = NO;
    session.failCancel = NO;
    assert(![controller handleEvent:down client:client]);
    assert([controller handleEvent:up client:client]);
    assert(prefs.englishMode && [client.committed isEqual:@"测试"]);

    // With letters still being composed the tap sends those letters out, not the highlighted candidate.
    // That is the whole point of reaching for Shift mid-word: a name, a command or an acronym the
    // dictionary does not carry leaves as what was typed. The composition is committed before the switch,
    // because switching rebuilds the session and would take the letters with it.
    prefs.englishMode = NO;
    session.rawTransition = @{@"handled":@YES, @"commit":@"msime",
        @"view":@{@"editing_text":@"", @"caret_position":@0, @"candidates":@[]}};
    // After the raw commit there is no composition left, so the cancel that the mode switch performs has
    // nothing to drop: the real Engine answers handled with no commit, and the fake has to say the same or
    // it overwrites what the raw commit just inserted.
    NSDictionary *previousFinish = session.finishTransition;
    NSDictionary *previousCancel = session.cancelTransition;
    session.finishTransition = @{@"handled":@YES,
        @"view":@{@"editing_text":@"", @"caret_position":@0, @"candidates":@[]}};
    session.cancelTransition = session.finishTransition;
    [controller setValue:@{@"editing_text":@"msime", @"caret_position":@5, @"candidates":@[]} forKey:@"view"];
    NSUInteger rawBefore = session.rawCommitCalls;
    assert(![controller handleEvent:TapEvent(NSEventTypeFlagsChanged, 56, NSEventModifierFlagShift, 3.0) client:client]);
    assert([controller handleEvent:TapEvent(NSEventTypeFlagsChanged, 56, 0, 3.1) client:client]);
    assert(session.rawCommitCalls == rawBefore + 1);
    assert([client.committed isEqual:@"msime"] && prefs.englishMode);

    // Nothing composing, nothing to send out: the tap is only the mode switch.
    prefs.englishMode = NO;
    [controller setValue:@{@"editing_text":@"", @"caret_position":@0, @"candidates":@[]} forKey:@"view"];
    rawBefore = session.rawCommitCalls;
    assert(![controller handleEvent:TapEvent(NSEventTypeFlagsChanged, 56, NSEventModifierFlagShift, 4.0) client:client]);
    assert([controller handleEvent:TapEvent(NSEventTypeFlagsChanged, 56, 0, 4.1) client:client]);
    assert(session.rawCommitCalls == rawBefore && prefs.englishMode);
    session.rawTransition = nil;
    session.finishTransition = previousFinish;
    session.cancelTransition = previousCancel;
    // Hand the sequence below back the state it had before this block: English on, nothing composing.
    prefs.englishMode = YES;
    [controller setValue:nil forKey:@"view"];

    assert(![controller handleEvent:down client:client]);
    assert([controller handleEvent:up client:client]);
    assert(!prefs.englishMode);
    assert(![controller handleEvent:down client:client]);
    ShortcutClient *other = [ShortcutClient new];
    assert(![controller handleEvent:up client:other]);
    assert(!prefs.englishMode);
    prefs.controlTapShortcut = YES;
    assert(![controller handleEvent:TapEvent(NSEventTypeFlagsChanged, 62, NSEventModifierFlagControl, 2.0) client:other]);
    assert([controller handleEvent:TapEvent(NSEventTypeFlagsChanged, 62, 0, 2.1) client:other]);
    assert(prefs.englishMode);
    assert(![controller handleEvent:TapEvent(NSEventTypeKeyUp, 0, 0, 2.2) client:other]);
    assert(![controller handleEvent:down client:other]);
    [controller snapshotSessionReplaced:[NSNotification notificationWithName:@"synthetic" object:session]];
    assert(![controller handleEvent:up client:other] && prefs.englishMode);
    assert(![controller handleEvent:down client:other]);
    assert(![controller handleEvent:up client:nil]);
    assert(![controller handleEvent:up client:other] && prefs.englishMode);
    MSIMERemoveTestPreferenceSuite(defaults, suite);
}

@interface ApplicationShortcutClient : ShortcutClient
@property(nonatomic, copy) NSString *bundleIdentifier;
@end
@implementation ApplicationShortcutClient
@end

static NSDictionary *monitoredSource;
static NSUInteger monitoredSourceReads;
static TISInputSourceRef CopyMonitoredSource() {
    ++monitoredSourceReads;
    return monitoredSource ? (TISInputSourceRef)CFBridgingRetain(monitoredSource) : nullptr;
}
static void *MonitoredSourceProperty(TISInputSourceRef source, CFStringRef key) {
    return (__bridge void *)((__bridge NSDictionary *)source)[(__bridge NSString *)key];
}
static void DrainMainQueue();

// What an editor receives, driven through the real engine.
//
// Every other controller test here hands the controller a stand-in session, and the tests that drive a real
// session call it directly. Neither covers the path a typist exercises: a key event reaching handleEvent:,
// the engine deciding what the composition now is, and the transition being applied to the text client. A
// wrong preedit, a swallowed commit or punctuation in the wrong script would show up in that seam, and it
// had nothing under it.
//
// IMK delivering the event is the only part left out - that needs the input source selected. From
// handleEvent: down this is the real thing.
static void TestRealSessionComposition() {
    NSString *root = [NSTemporaryDirectory() stringByAppendingPathComponent:NSUUID.UUID.UUIDString];
    NSMutableDictionary *options = [@{@"api_version": @1,
        @"preferences": @{@"scheme": @"quanpin", @"default_ime_mode": @"chinese", @"candidate_page_size": @5,
                          @"learning": @NO, @"chinese_punctuation": @YES}} mutableCopy];
    for (NSString *name in @[@"resources", @"user_data", @"cache", @"dictionaries"]) {
        NSString *path = [root stringByAppendingPathComponent:name];
        assert([NSFileManager.defaultManager createDirectoryAtPath:path withIntermediateDirectories:YES attributes:nil error:nil]);
        options[name] = path;
    }
    NSError *error = nil;
    MSIMEClientSession *session = [[MSIMEClientSession alloc] initWithOptions:options error:&error];
    assert(session && !error);
    assert([session setFocused:YES error:&error] && !error);

    NSString *suite = [@"msime.real-composition." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    MSIMEAppearancePreferences *prefs =
        [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults skinsRoot:[NSURL fileURLWithPath:root]];
    ShortcutClient *client = [ShortcutClient new];
    client.document = @"";
    client.caret = NSMakeRect(100, 100, 1, 16);
    ModeController *controller = [ModeController alloc];
    [controller setValue:prefs forKey:@"appearance"];
    [controller setValue:session forKey:@"session"];
    [controller setValue:client forKey:@"activeClient"];
    HiddenCandidatePanel *panel = [[HiddenCandidatePanel alloc] init];
    [controller setValue:panel forKey:@"panel"];

    // Unicode mode needs no dictionary, so what comes back is the engine's own answer rather than a
    // fixture's. Shift+U opens it and the code point is typed into the buffer.
    assert([controller handleEvent:KeypadKey(32, @"U", NSEventModifierFlagShift, NO) client:client]);
    for (NSArray *stroke in @[@[@21, @"4"], @[@14, @"e"], @[@19, @"2"], @[@2, @"d"]])
        assert([controller handleEvent:KeypadKey([stroke[0] unsignedShortValue], stroke[1], 0, NO) client:client]);
    // The editor shows the composition while it is being typed, not only once it ends.
    assert([client.marked isEqual:@"U4e2d"]);
    assert(client.insertions.count == 0);

    // Space commits what the engine resolved the code point to, and the marked text goes with it.
    assert([controller handleEvent:ModeKey(49, 0, NO) client:client]);
    assert(client.insertions.count == 1 && [client.insertions[0] isEqual:@"\u4e2d"]);
    assert(client.marked.length == 0);

    // Return is the other half of the same contract: it commits the letters that were typed, not the
    // character they resolve to. Someone who meant to write the code point itself gets it.
    [client.insertions removeAllObjects];
    assert([controller handleEvent:KeypadKey(32, @"U", NSEventModifierFlagShift, NO) client:client]);
    for (NSArray *stroke in @[@[@21, @"4"], @[@14, @"e"], @[@19, @"2"], @[@2, @"d"]])
        assert([controller handleEvent:KeypadKey([stroke[0] unsignedShortValue], stroke[1], 0, NO) client:client]);
    NSEvent *enter = [NSEvent keyEventWithType:NSEventTypeKeyDown location:NSZeroPoint modifierFlags:0
        timestamp:0 windowNumber:0 context:nil characters:@"\r" charactersIgnoringModifiers:@"\r" isARepeat:NO keyCode:36];
    assert([controller handleEvent:enter client:client]);
    assert(client.insertions.count == 1 && [client.insertions[0] isEqual:@"U4e2d"]);
    assert(client.marked.length == 0);

    // Punctuation with nothing composing goes through the shared policy and reaches the editor converted.
    [client.insertions removeAllObjects];
    NSEvent *comma = [NSEvent keyEventWithType:NSEventTypeKeyDown location:NSZeroPoint modifierFlags:0
        timestamp:0 windowNumber:0 context:nil characters:@"," charactersIgnoringModifiers:@"," isARepeat:NO keyCode:43];
    assert([controller handleEvent:comma client:client]);
    assert(client.insertions.count == 1 && [client.insertions[0] isEqual:@"\uff0c"]);

    // With the preference off there is nothing to convert, so the key is declined rather than consumed and
    // the application types its own comma. Consuming it and inserting the ASCII one would look the same
    // here and be wrong in an editor that treats the two differently.
    [client.insertions removeAllObjects];
    assert([session setChinesePunctuationEnabled:NO error:&error] && !error);
    assert(![controller handleEvent:comma client:client]);
    assert(client.insertions.count == 0);

    assert([session closeWithError:&error] && !error);

    // The reference reserves the physical ANSI minus key for Japanese: it must pass through this
    // controller into the real Engine and become the long-vowel mark, even when minus/equal paging
    // is enabled by default.  The helper-only paging tests cannot prove the last half of that path.
    NSMutableDictionary *japaneseOptions = [options mutableCopy];
    NSMutableDictionary *japanesePreferences = [options[@"preferences"] mutableCopy];
    japanesePreferences[@"scheme"] = @"japanese";
    japaneseOptions[@"preferences"] = japanesePreferences;
    MSIMEClientSession *japaneseSession = [[MSIMEClientSession alloc] initWithOptions:japaneseOptions error:&error];
    assert(japaneseSession && !error);
    NSDictionary *japaneseView = [japaneseSession setFocused:YES error:&error];
    assert(japaneseView && !error);
    [controller setValue:japaneseSession forKey:@"session"];
    [controller setValue:japaneseView forKey:@"view"];
    [client.insertions removeAllObjects];

    assert([controller handleEvent:KeypadKey(27, @"-", 0, NO) client:client]);
    assert([client.marked isEqual:@"ー"]);
    assert([controller handleEvent:enter client:client]);
    assert(client.insertions.count == 1 && [client.insertions[0] isEqual:@"ー"]);
    assert(client.marked.length == 0);

    // A preceding bare n must settle to ん before the same key contributes ー. This is the converter
    // edge in the source change, exercised here through the product host instead of against the converter.
    [client.insertions removeAllObjects];
    assert([controller handleEvent:KeypadKey(45, @"n", 0, NO) client:client]);
    assert([controller handleEvent:KeypadKey(27, @"-", 0, NO) client:client]);
    assert([client.marked isEqual:@"んー"]);
    assert([controller handleEvent:enter client:client]);
    assert(client.insertions.count == 1 && [client.insertions[0] isEqual:@"んー"]);
    assert([japaneseSession closeWithError:&error] && !error);

    MSIMERemoveTestPreferenceSuite(defaults, suite);
    assert([NSFileManager.defaultManager removeItemAtPath:root error:nil]);
}

static void TestInputSourceModeReset() {
    NSString *suite = [@"msime.source-reset." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    MSIMEAppearancePreferences *prefs = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    [prefs activateInputModeForApplication:@"org.example.fixture"];
    prefs.englishMode = YES; // Keep an independent per-app choice.
    prefs.imeModeScope = @"global";
    [prefs activateInputModeForApplication:@"org.example.fixture"];
    prefs.englishMode = YES;
    NSNotificationCenter *center = [NSNotificationCenter new];
    NSString *bundleKey = (__bridge NSString *)kTISPropertyBundleID;
    NSString *sourceKey = (__bridge NSString *)kTISPropertyInputSourceID;
    NSString *notification = (__bridge NSString *)kTISNotifySelectedKeyboardInputSourceChanged;
    NSString *own = @"org.example.input-method";
    __block NSUInteger resets = 0, saves = 0;
    id saveObserver = [NSNotificationCenter.defaultCenter addObserverForName:MSIMEAppearanceDidChangeNotification object:prefs queue:nil usingBlock:^(NSNotification *note) { (void)note; ++saves; }];
    MSIMEInputSourceMonitor *monitor = [[MSIMEInputSourceMonitor alloc] initWithCenter:center bundleIdentifier:own
        copySource:CopyMonitoredSource propertyGetter:MonitoredSourceProperty switchedAway:^{ assert(NSThread.isMainThread); ++resets; [prefs resetRememberedInputModes]; }];
    assert(monitor);
    for (NSDictionary *source in @[@{}, @{sourceKey:@42}, @{bundleKey:own}, @{sourceKey:own},
                                   @{sourceKey:[own stringByAppendingString:@".mode"]}]) {
        monitoredSource = source;
        [center postNotificationName:notification object:nil];
        assert(resets == 0 && prefs.englishMode && saves == 0);
    }
    monitoredSource = nil;
    [center postNotificationName:notification object:nil];
    assert(resets == 0 && prefs.englishMode);
    for (NSDictionary *source in @[@{bundleKey:@"org.example.other-input"},
                                   @{sourceKey:@"com.apple.keylayout.US"},
                                   @{sourceKey:[own stringByAppendingString:@"-other"]}]) {
        prefs.englishMode = YES;
        NSUInteger beforeSaves = saves, beforeResets = resets;
        monitoredSource = source;
        [center postNotificationName:notification object:nil];
        assert(resets == beforeResets + 1 && !prefs.englishMode && saves == beforeSaves);
    }
    prefs.defaultImeMode = @"english";
    prefs.englishMode = NO;
    [center postNotificationName:notification object:nil];
    assert(prefs.englishMode); // Reset follows the configured default, not hardcoded Chinese.
    // App scope: switching between this method's own modes keeps each app's choice, and leaving for another source drops every app's choice so the next activation starts from default_ime_mode, as TIP re-activation does in the reference.
    prefs.imeModeScope = @"app";
    prefs.defaultImeMode = @"chinese";
    [prefs activateInputModeForApplication:@"org.example.other-app"];
    prefs.englishMode = YES;
    [prefs activateInputModeForApplication:@"org.example.fixture"];
    prefs.englishMode = YES;
    NSUInteger appResets = resets;
    for (NSDictionary *source in @[@{bundleKey:own}, @{sourceKey:[own stringByAppendingString:@".mode"]}]) {
        monitoredSource = source;
        [center postNotificationName:notification object:nil];
        [prefs activateInputModeForApplication:@"org.example.fixture"];
        assert(resets == appResets && prefs.englishMode);
    }
    monitoredSource = @{sourceKey:@"com.apple.keylayout.US"};
    [center postNotificationName:notification object:nil];
    assert(resets == appResets + 1);
    [prefs activateInputModeForApplication:@"org.example.fixture"];
    assert(!prefs.englishMode);
    [prefs activateInputModeForApplication:@"org.example.other-app"];
    assert(!prefs.englishMode); // Apps other than the one the user switched away in are cleared too.
    prefs.defaultImeMode = @"english";
    [prefs activateInputModeForApplication:@"org.example.fixture"];
    prefs.englishMode = NO;
    [center postNotificationName:notification object:nil];
    [prefs activateInputModeForApplication:@"org.example.fixture"];
    assert(prefs.englishMode); // The app-scope reset follows the configured default too.
    NSUInteger before = resets;
    // A delayed background notification must re-read the selected source on
    // the main thread, not reset from an obsolete source captured at receipt.
    dispatch_semaphore_t posted = dispatch_semaphore_create(0);
    NSUInteger reads = monitoredSourceReads;
    dispatch_async(dispatch_get_global_queue(QOS_CLASS_UTILITY, 0), ^{
        [center postNotificationName:notification object:nil];
        dispatch_semaphore_signal(posted);
    });
    // Both waits end as soon as the event happens; the bound is only there for a stuck run, and a loaded CI runner can take more than a second to give the main queue a turn.
    assert(dispatch_semaphore_wait(posted, dispatch_time(DISPATCH_TIME_NOW, 5 * NSEC_PER_SEC)) == 0);
    monitoredSource = @{bundleKey:own};
    // sourceChanged: marshals background notifications onto the main dispatch queue. Use the
    // same CoreFoundation run-loop pump as the other async tests; NSRunLoop's short slices can
    // leave the main dispatch queue unserviced under the sanitizer runtime.
    DrainMainQueue();
    assert(monitoredSourceReads > reads && resets == before);
    monitoredSource = @{sourceKey:@"com.apple.keylayout.US"};
    [monitor stop];
    [center postNotificationName:notification object:nil];
    assert(resets == before);
    __weak MSIMEInputSourceMonitor *weakMonitor = monitor;
    monitor = nil;
    assert(weakMonitor == nil);
    [NSNotificationCenter.defaultCenter removeObserver:saveObserver];
    MSIMERemoveTestPreferenceSuite(defaults, suite);
    monitoredSource = nil;
}

// Punctuation and width are runtime state of each app, like the reference's per-thread TSF compartments: the toggles never save, another app starts from the saved value, a Chinese/English switch puts punctuation back in step with the mode, and a new saved starting value applies everywhere.
static void TestPerApplicationPunctuationAndWidth() {
    NSString *suite = [@"msime.runtime-punctuation." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    MSIMEAppearancePreferences *prefs = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    prefs.imeModeScope = @"global"; // The toggles stay per app whatever the Chinese/English scope says.
    ModeController *controller = [ModeController alloc];
    ShortcutSession *session = [ShortcutSession new];
    ApplicationShortcutClient *a = [ApplicationShortcutClient new], *b = [ApplicationShortcutClient new];
    a.bundleIdentifier = @"org.example.runtime-a"; b.bundleIdentifier = @"org.example.runtime-b";
    [controller setValue:prefs forKey:@"appearance"];
    [controller setValue:session forKey:@"session"];
    [controller setValue:a forKey:@"activeClient"];
    [controller setValue:@{@"editing_text":@"", @"candidates":@[]} forKey:@"view"];
    [prefs activateInputModeForApplication:a.bundleIdentifier];
    assert(prefs.chinesePunctuation && !prefs.fullWidthInput && !prefs.englishMode);
    __block NSUInteger saves = 0;
    id observer = [NSNotificationCenter.defaultCenter addObserverForName:MSIMEAppearanceDidChangeNotification object:prefs queue:nil usingBlock:^(NSNotification *note) { (void)note; ++saves; }];
    NSEvent *period = [NSEvent keyEventWithType:NSEventTypeKeyDown location:NSZeroPoint modifierFlags:NSEventModifierFlagControl timestamp:0 windowNumber:0 context:nil characters:@"." charactersIgnoringModifiers:@"." isARepeat:NO keyCode:47];
    const NSEventModifierFlags widthChord = NSEventModifierFlagControl | NSEventModifierFlagShift;
    assert([controller handleEvent:period client:a]);
    assert([controller handleEvent:ModeKey(49, widthChord, NO) client:a]);
    assert(!prefs.runtimeChinesePunctuation && prefs.runtimeFullWidthInput);
    assert(!session.chinesePunctuation && session.fullwidth && saves == 0);
    assert([defaults objectForKey:@"MSIMEClientChinesePunctuation"] == nil && [defaults objectForKey:@"MSIMEClientFullWidthInput"] == nil);
    assert(prefs.chinesePunctuation && !prefs.fullWidthInput);
    NSDictionary *shared = [prefs sharedPreferencesByMerging:@{}];
    assert([shared[@"chinese_punctuation"] isEqual:@YES] && [shared[@"character_width"] isEqual:@"halfwidth"]);

    // Focus moving to another app gives it the saved values and tells the Engine before it types.
    session.nextTransition = @{@"handled": @NO, @"view": @{@"editing_text": @"", @"candidates": @[]}};
    [controller handleEvent:ModeKey(0, 0, NO) client:b];
    assert(prefs.runtimeChinesePunctuation && !prefs.runtimeFullWidthInput);
    assert(session.chinesePunctuation && !session.fullwidth);
    [controller handleEvent:ModeKey(0, 0, NO) client:a];
    assert(!prefs.runtimeChinesePunctuation && prefs.runtimeFullWidthInput);
    assert(!session.chinesePunctuation && session.fullwidth);
    session.nextTransition = nil;

    // A Chinese/English switch resolves punctuation from the new mode (SyncPunctuationWithImeMode); width is independent of the mode.
    [controller floatingToolbarDidRequestTogglePunctuation:nil];
    assert(prefs.runtimeChinesePunctuation);
    [controller setEnglishInputMode:YES];
    assert(prefs.englishMode && !prefs.runtimeChinesePunctuation && prefs.runtimeFullWidthInput);
    [controller setEnglishInputMode:NO]; // Back to Chinese: the saved starting value.
    assert(!prefs.englishMode && prefs.runtimeChinesePunctuation);
    [controller floatingToolbarDidRequestTogglePunctuation:nil];
    assert(!prefs.runtimeChinesePunctuation);
    [controller setEnglishInputMode:NO]; // Not a switch, so the toggle stays.
    assert(!prefs.runtimeChinesePunctuation);
    prefs.punctuationLock = @"chinese";
    [controller setEnglishInputMode:YES]; // A Chinese punctuation lock keeps Chinese punctuation in English mode.
    assert(prefs.runtimeChinesePunctuation);
    [controller setEnglishInputMode:NO];
    prefs.punctuationLock = @"follow";
    prefs.runtimeChinesePunctuation = NO;

    // Leaving the input method (the TSF Deactivate counterpart) clears the toggles of every app together, as it does the modes.
    [prefs activateInputModeForApplication:b.bundleIdentifier];
    prefs.runtimeFullWidthInput = YES;
    [prefs activateInputModeForApplication:a.bundleIdentifier];
    [prefs resetAllRuntimeInputState];
    assert(prefs.runtimeChinesePunctuation && !prefs.runtimeFullWidthInput);
    [prefs activateInputModeForApplication:b.bundleIdentifier];
    assert(prefs.runtimeChinesePunctuation && !prefs.runtimeFullWidthInput);
    prefs.runtimeFullWidthInput = YES;

    // Re-applying the shared document keeps the toggles; a changed starting value replaces them in every app.
    [prefs applySharedInputPreferences:@{@"chinese_punctuation":@YES, @"character_width":@"halfwidth"}];
    assert(prefs.runtimeFullWidthInput);
    prefs.runtimeChinesePunctuation = NO;
    [prefs applySharedInputPreferences:@{@"character_width":@"fullwidth"}];
    assert(prefs.runtimeFullWidthInput && !prefs.runtimeChinesePunctuation); // Width changed, punctuation did not.
    [prefs activateInputModeForApplication:a.bundleIdentifier];
    prefs.runtimeFullWidthInput = NO;
    [prefs applySharedInputPreferences:@{@"chinese_punctuation":@NO, @"character_width":@"halfwidth"}];
    assert(!prefs.runtimeChinesePunctuation && !prefs.runtimeFullWidthInput);
    [prefs activateInputModeForApplication:b.bundleIdentifier];
    assert(!prefs.runtimeChinesePunctuation && !prefs.runtimeFullWidthInput);
    prefs.runtimeFullWidthInput = YES;
    prefs.fullWidthInput = NO; // The settings checkbox is a new starting value too.
    assert(!prefs.runtimeFullWidthInput);
    [NSNotificationCenter.defaultCenter removeObserver:observer];
    MSIMERemoveTestPreferenceSuite(defaults, suite);
}

static NSEvent *EnglishKey(NSString *characters, unsigned short code, NSEventModifierFlags flags) {
    return [NSEvent keyEventWithType:NSEventTypeKeyDown location:NSZeroPoint modifierFlags:flags timestamp:0 windowNumber:0 context:nil characters:characters charactersIgnoringModifiers:characters isARepeat:NO keyCode:code];
}

// English mode is the reference's closed IME, whose punctuation and double/single-byte compartments still shape what is typed (KeyEventSink.cpp, ResolvePunctuationOpen).
static void TestEnglishModePunctuationAndWidthOutput() {
    NSString *suite = [@"msime.english-output." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    MSIMEAppearancePreferences *prefs = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    ModeController *controller = [ModeController alloc];
    ShortcutSession *session = [ShortcutSession new];
    ApplicationShortcutClient *client = [ApplicationShortcutClient new];
    client.bundleIdentifier = @"org.example.english-output";
    [controller setValue:prefs forKey:@"appearance"];
    [controller setValue:session forKey:@"session"];
    [controller setValue:client forKey:@"activeClient"];
    [controller setValue:@{@"editing_text":@"", @"candidates":@[]} forKey:@"view"];
    [prefs activateInputModeForApplication:client.bundleIdentifier];
    NSString *(^type)(NSEvent *) = ^NSString *(NSEvent *event) {
        client.committed = nil;
        return [controller handleEvent:event client:client] ? (client.committed ?: @"") : nil;
    };
    NSEvent *comma = EnglishKey(@",", 43, 0), *letter = EnglishKey(@"a", 0, 0), *space = EnglishKey(@" ", 49, 0);
    NSEvent *quote = EnglishKey(@"\"", 39, NSEventModifierFlagShift);
    NSEvent *less = EnglishKey(@"<", 43, NSEventModifierFlagShift), *greater = EnglishKey(@">", 47, NSEventModifierFlagShift);
    NSEvent *keypadPeriod = EnglishKey(@".", 65, NSEventModifierFlagNumericPad);
    NSEvent *toggle = EnglishKey(@".", 47, NSEventModifierFlagControl);
    NSEvent *widthToggle = EnglishKey(@" ", 49, NSEventModifierFlagControl | NSEventModifierFlagShift);

    // A Chinese lock converts English-mode punctuation with the Engine's forward table.
    prefs.punctuationLock = @"chinese";
    [controller setEnglishInputMode:YES];
    assert(prefs.englishMode && prefs.runtimeChinesePunctuation);
    assert([type(comma) isEqual:@"，"]);
    assert([type(quote) isEqual:@"“"] && [type(quote) isEqual:@"”"]);
    assert([type(less) isEqual:@"《"] && [type(less) isEqual:@"〈"] && [type(greater) isEqual:@"〉"] && [type(greater) isEqual:@"》"]);
    assert(type(letter) == nil && type(space) == nil);
    assert(type(keypadPeriod) == nil); // The keypad never turns Chinese.
    assert(type(EnglishKey(@",", 43, NSEventModifierFlagCommand)) == nil);
    assert(type(EnglishKey(@"a", 0, NSEventModifierFlagControl)) == nil);
    // A mode switch starts the quote pair over.
    assert([type(quote) isEqual:@"“"]);
    [controller setEnglishInputMode:NO];
    [controller setEnglishInputMode:YES];
    assert([type(quote) isEqual:@"“"]);
    [controller setEnglishInputMode:NO];

    // Follow: English mode starts with English punctuation, and Ctrl+. turns Chinese on for this app without saving it.
    prefs.punctuationLock = @"follow";
    [controller setEnglishInputMode:YES];
    assert(!prefs.runtimeChinesePunctuation && type(comma) == nil);
    assert([controller handleEvent:toggle client:client]);
    assert(prefs.runtimeChinesePunctuation && [defaults objectForKey:@"MSIMEClientChinesePunctuation"] == nil);
    assert([type(comma) isEqual:@"，"]);
    [controller floatingToolbarDidRequestTogglePunctuation:nil];
    assert(!prefs.runtimeChinesePunctuation && type(comma) == nil);
    [controller floatingToolbarDidRequestTogglePunctuation:nil];
    assert([type(comma) isEqual:@"，"]);
    [controller setEnglishInputMode:NO];
    [controller setEnglishInputMode:YES]; // The round trip drops the English-mode choice.
    assert(!prefs.runtimeChinesePunctuation && type(comma) == nil);
    [controller setEnglishInputMode:NO];

    // A pinned lock holds against Ctrl+. and the toolbar, as ResolvePunctuationOpen does.
    prefs.punctuationLock = @"english";
    [controller setEnglishInputMode:YES];
    assert([controller handleEvent:toggle client:client] && !prefs.runtimeChinesePunctuation);
    [controller floatingToolbarDidRequestTogglePunctuation:nil];
    assert(!prefs.runtimeChinesePunctuation && type(comma) == nil);
    prefs.punctuationLock = @"chinese";
    assert([controller handleEvent:toggle client:client] && prefs.runtimeChinesePunctuation);
    [controller floatingToolbarDidRequestTogglePunctuation:nil];
    assert(prefs.runtimeChinesePunctuation && [type(comma) isEqual:@"，"]);
    [controller setEnglishInputMode:NO];

    // Full width in English mode: printable ASCII widens, Space becomes U+3000, Chinese punctuation wins where both apply.
    prefs.punctuationLock = @"follow";
    [controller setEnglishInputMode:YES];
    assert([controller handleEvent:widthToggle client:client] && prefs.runtimeFullWidthInput && prefs.englishMode);
    assert([type(letter) isEqual:@"ａ"] && [type(space) isEqual:@"\u3000"] && [type(comma) isEqual:@"，"]);
    assert([type(keypadPeriod) isEqual:@"．"]);
    assert([controller handleEvent:toggle client:client] && prefs.runtimeChinesePunctuation);
    assert([type(comma) isEqual:@"，"] && [type(keypadPeriod) isEqual:@"．"]);
    assert(type(EnglishKey(@",", 43, NSEventModifierFlagCommand)) == nil);
    assert(type(EnglishKey(@"a", 0, NSEventModifierFlagControl)) == nil);
    assert(type(EnglishKey(@"\t", 48, 0)) == nil && type(EnglishKey(@"é", 14, 0)) == nil);
    assert([defaults objectForKey:@"MSIMEClientFullWidthInput"] == nil);
    assert([controller handleEvent:widthToggle client:client] && !prefs.runtimeFullWidthInput);
    [controller setEnglishInputMode:NO];
    MSIMERemoveTestPreferenceSuite(defaults, suite);
}

static void TestInputModePolicy() {
    NSString *suite = [@"msime.mode-policy." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    MSIMEAppearancePreferences *prefs = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    assert([prefs.defaultImeMode isEqual:@"chinese"] && [prefs.imeModeScope isEqual:@"app"]);
    assert(![prefs sharedPreferencesByMerging:@{}][@"default_ime_mode"]);
    NSDictionary *shared = @{@"default_ime_mode":@"english", @"ime_mode_scope":@"global"};
    assert([[prefs sharedPreferencesByMerging:shared][@"default_ime_mode"] isEqual:@"english"]);
    [prefs applySharedInputPreferences:shared];
    assert([prefs.defaultImeMode isEqual:@"english"] && [prefs.imeModeScope isEqual:@"global"]);
    [prefs applySharedInputPreferences:@{@"default_ime_mode":@YES, @"ime_mode_scope":@"invalid"}];
    assert([prefs.defaultImeMode isEqual:@"english"] && [prefs.imeModeScope isEqual:@"global"]);
    NSPopUpButton *mode = (id)PreferenceControl(prefs, NSSelectorFromString(@"defaultImeModeChanged:"));
    NSPopUpButton *scope = (id)PreferenceControl(prefs, NSSelectorFromString(@"imeModeScopeChanged:"));
    assert(mode.indexOfSelectedItem == 1 && scope.indexOfSelectedItem == 1);
    [mode selectItemAtIndex:0];
    assert([NSApp sendAction:mode.action to:mode.target from:mode]);
    [scope selectItemAtIndex:0];
    assert([NSApp sendAction:scope.action to:scope.target from:scope]);
    MSIMEAppearancePreferences *reopened = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    assert([reopened.defaultImeMode isEqual:@"chinese"] && [reopened.imeModeScope isEqual:@"app"]);
    NSDictionary *patch = [prefs sharedPreferencesByMerging:@{}];
    assert([patch[@"default_ime_mode"] isEqual:@"chinese"] && [patch[@"ime_mode_scope"] isEqual:@"app"]);
    prefs.defaultImeMode = @"invalid"; prefs.imeModeScope = @"invalid";
    assert([prefs.defaultImeMode isEqual:@"chinese"] && [prefs.imeModeScope isEqual:@"app"]);

    // Before the first key, the initial asynchronous preference load can seed
    // the default; after the first key, refreshes cannot change the active mode.
    [prefs activateInputModeForApplication:@"org.example.fixture-a"];
    [prefs applySharedInputPreferences:@{@"default_ime_mode":@"english"}];
    assert(prefs.englishMode);
    [prefs lockActiveInputMode];
    [prefs applySharedInputPreferences:@{@"default_ime_mode":@"chinese"}];
    assert(prefs.englishMode);
    [prefs activateInputModeForApplication:@"org.example.fixture-b"];
    assert(!prefs.englishMode);
    [prefs lockActiveInputMode];
    [prefs activateInputModeForApplication:@"org.example.fixture-a"];
    assert(prefs.englishMode);
    [prefs applySharedInputPreferences:@{@"ime_mode_scope":@"global"}];
    assert(prefs.englishMode); // Scope changes are deferred to activation.
    [prefs activateInputModeForApplication:@"org.example.fixture-a"];
    assert(!prefs.englishMode);
    prefs.englishMode = YES;
    [prefs activateInputModeForApplication:@"org.example.fixture-b"];
    assert(prefs.englishMode);
    NSMutableDictionary *cloud = [[prefs cloudSettingsSnapshot] mutableCopy];
    assert([cloud[@"platform.macos.english_input_mode"] isEqual:@YES]);
    cloud[@"platform.macos.english_input_mode"] = @NO;
    assert([prefs applyCloudSettingsSnapshot:cloud] && !prefs.englishMode);
    [prefs activateInputModeForApplication:@"org.example.fixture-a"];
    assert(!prefs.englishMode);
    // Per-app choices survive changing scope, but do not leak into fresh preferences.
    prefs.imeModeScope = @"app";
    [prefs activateInputModeForApplication:@"org.example.fixture-a"];
    assert(prefs.englishMode);
    [reopened activateInputModeForApplication:@"org.example.fixture-a"];
    assert(!reopened.englishMode);

    ModeController *controller = [ModeController alloc];
    ShortcutSession *session = [ShortcutSession new];
    ApplicationShortcutClient *a = [ApplicationShortcutClient new], *b = [ApplicationShortcutClient new];
    a.bundleIdentifier = @"org.example.fixture-a"; b.bundleIdentifier = @"org.example.fixture-b";
    [controller setValue:prefs forKey:@"appearance"];
    [controller setValue:session forKey:@"session"];
    assert(![controller handleEvent:ModeKey(0, 0, NO) client:a]);
    assert(prefs.englishMode && session.asciiCalls == 0);
    [controller handleEvent:ModeKey(0, 0, NO) client:b];
    assert(!prefs.englishMode && session.asciiCalls == 1);
    [prefs applySharedInputPreferences:@{@"default_ime_mode":@"english"}];
    assert(!prefs.englishMode);
    [controller handleEvent:ModeKey(0, 0, NO) client:a];
    assert(prefs.englishMode && session.asciiCalls == 1);
    // App identities are memory-only; no per-app state is exported or persisted.
    assert([defaults objectForKey:a.bundleIdentifier] == nil);
    assert(![prefs sharedPreferencesByMerging:@{}][a.bundleIdentifier]);
    MSIMERemoveTestPreferenceSuite(defaults, suite);
}

static void TestControlOptionSpace() {
    NSUserDefaults *standardDefaults = NSUserDefaults.standardUserDefaults;
    id previousHoldSpace = [standardDefaults objectForKey:@"MSIMEClientVoiceHotkeyHoldSpace"];
    [standardDefaults setBool:NO forKey:@"MSIMEClientVoiceHotkeyHoldSpace"];
    NSString *suite = [@"msime.control-option-space." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    MSIMEAppearancePreferences *prefs = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    assert(prefs.controlOptionSpaceShortcut);
    NSDictionary *keys = @{@"switch_language_ctrl_alt_space":@NO, @"switch_language_ctrl":@YES, @"toggle_character_set_ctrl_shift_f":@NO};
    assert([[prefs sharedPreferencesByMerging:@{@"keybindings":keys}][@"keybindings"] isEqual:keys]);
    assert(![prefs sharedPreferencesByMerging:@{}][@"keybindings"]);
    [prefs applySharedInputPreferences:@{@"keybindings":keys}];
    assert(!prefs.controlOptionSpaceShortcut && !prefs.characterSetShortcut);
    for (id invalid in @[NSNull.null, @1, @"true"])
        [prefs applySharedInputPreferences:@{@"keybindings":@{@"switch_language_ctrl_alt_space":invalid}}];
    assert(!prefs.controlOptionSpaceShortcut);
    NSButton *button = (id)PreferenceControl(prefs, NSSelectorFromString(@"controlOptionSpaceShortcutChanged:"));
    assert(button.state == NSControlStateValueOff);
    button.state = NSControlStateValueOn;
    assert([NSApp sendAction:button.action to:button.target from:button]);
    assert(prefs.controlOptionSpaceShortcut && !prefs.characterSetShortcut);
    NSMutableDictionary *expected = [keys mutableCopy];
    expected[@"switch_language_ctrl_alt_space"] = @YES;
    assert([[prefs sharedPreferencesByMerging:@{@"keybindings":keys}][@"keybindings"] isEqual:expected]);
    NSDictionary *patch = [prefs sharedPreferencesByMerging:@{}];
    assert([patch[@"keybindings"] isEqual:@{@"switch_language_ctrl_alt_space":@YES}]);
    assert([MSIMEMergePreferenceSnapshot(@{@"keybindings":keys}, patch)[@"keybindings"] isEqual:expected]);
    assert([[[MSIMEAppearancePreferences alloc] initWithDefaults:defaults] controlOptionSpaceShortcut]);

    ModeController *controller = [ModeController alloc];
    ShortcutClient *client = [ShortcutClient new];
    ShortcutSession *session = [ShortcutSession new];
    TestCandidatePanel *panel = [TestCandidatePanel new];
    [controller setValue:prefs forKey:@"appearance"];
    [controller setValue:client forKey:@"activeClient"];
    [controller setValue:session forKey:@"session"];
    [controller setValue:panel forKey:@"panel"];
    prefs.inputModeShortcut = NO; // Independent from the legacy Shift+Space switch.
    NSEventModifierFlags flags = NSEventModifierFlagControl | NSEventModifierFlagOption;
    session.failFinish = YES;
    session.failCancel = YES;
    panel.visible = YES;
    client.marked = @"test";
    assert([controller handleEvent:ModeKey(49, flags, NO) client:client]);
    assert(!prefs.englishMode && panel.visible && [client.marked isEqual:@"test"]);
    session.failFinish = NO;
    session.failCancel = NO;
    assert([controller handleEvent:ModeKey(49, flags, NO) client:client]);
    assert(prefs.englishMode && !panel.visible && client.marked.length == 0);
    assert(session.lastCommand == MSIME_CANCEL);

    // What the switch does to a composition in flight, which is the whole reason it sends a command
    // at all. The reference's English-mode switch is FUNCTION_CANCEL and its handler terminates the
    // composition without sending anything; committing the highlighted Chinese candidate instead -
    // which this host used to do - puts a word nobody chose into the document, and the user reached
    // for English precisely because the candidates on screen were wrong. The Shift tap keeps the
    // other rule and is checked in TestModifierTaps.
    prefs.englishMode = NO;
    client.committed = nil;
    client.marked = @"拼音";
    session.lastCommand = UINT32_MAX;
    session.nextTransition = @{@"handled":@YES,
        @"view":@{@"editing_text":@"", @"caret_position":@0, @"candidates":@[]}};
    [controller setValue:@{@"editing_text":@"nihao", @"caret_position":@5, @"candidates":@[]} forKey:@"view"];
    assert([controller handleEvent:ModeKey(49, flags, NO) client:client]);
    assert(prefs.englishMode && session.lastCommand == MSIME_CANCEL);
    assert(client.committed.length == 0 && client.marked.length == 0);
    session.nextTransition = nil;
    session.lastCommand = UINT32_MAX;
    assert([controller handleEvent:ModeKey(49, flags, YES) client:client]);
    assert(prefs.englishMode && session.lastCommand == UINT32_MAX);
    assert([controller handleEvent:ModeKey(49, flags | NSEventModifierFlagCapsLock, NO) client:client]);
    assert(!prefs.englishMode && session.lastCommand == UINT32_MAX);
    for (NSUInteger mask = 0; mask < 16; ++mask) {
        prefs.englishMode = YES;
        NSEventModifierFlags mods = (mask & 1 ? NSEventModifierFlagControl : 0) |
            (mask & 2 ? NSEventModifierFlagOption : 0) | (mask & 4 ? NSEventModifierFlagShift : 0) |
            (mask & 8 ? NSEventModifierFlagCommand : 0);
        assert([controller handleEvent:ModeKey(49, mods, NO) client:client] == (mask == 3 || mask == 5));
        assert(prefs.englishMode == (mask != 3));
    }
    assert(![controller handleEvent:ModeKey(0, flags, NO) client:client]);
    [prefs applySharedInputPreferences:@{@"keybindings":keys}];
    assert(![controller handleEvent:ModeKey(49, flags, NO) client:client]);
    assert(prefs.englishMode && button.state == NSControlStateValueOff);
    prefs.controlOptionSpaceShortcut = NO;
    assert(![[[MSIMEAppearancePreferences alloc] initWithDefaults:defaults] controlOptionSpaceShortcut]);
    MSIMERemoveTestPreferenceSuite(defaults, suite);
    if (previousHoldSpace) [standardDefaults setObject:previousHoldSpace forKey:@"MSIMEClientVoiceHotkeyHoldSpace"];
    else [standardDefaults removeObjectForKey:@"MSIMEClientVoiceHotkeyHoldSpace"];
}

static void TestInputMode(NSUserDefaults *defaults, MSIMEAppearancePreferences *appearance) {
    assert(!appearance.englishMode && appearance.inputModeShortcut);
    [appearance applySharedInputPreferences:@{@"keybindings":@{@"switch_language_shift":@NO}}];
    assert(!appearance.inputModeShortcut && !appearance.shiftTapShortcut);
    [appearance applySharedInputPreferences:@{@"keybindings":@{@"switch_language_shift":@YES}}];
    assert(appearance.inputModeShortcut && appearance.shiftTapShortcut);
    NSButton *shortcut = (id)PreferenceControl(appearance, @selector(inputModeShortcutChanged:));
    shortcut.state = NSControlStateValueOff;
    [NSApp sendAction:shortcut.action to:shortcut.target from:shortcut];
    MSIMEAppearancePreferences *reloaded = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults skinsRoot:appearance.skinsRoot];
    assert(!reloaded.inputModeShortcut);
    assert(appearance.shiftTapShortcut);
    [appearance applySharedInputPreferences:@{@"keybindings":@{@"switch_language_shift":@YES}}];
    NSButton *shift = (id)PreferenceControl(appearance, @selector(shiftTapShortcutChanged:));
    shift.state = NSControlStateValueOff;
    [NSApp sendAction:shift.action to:shift.target from:shift];
    assert(!appearance.inputModeShortcut && !appearance.shiftTapShortcut);
    appearance.inputModeShortcut = YES;
    ModeController *controller = [ModeController alloc];
    ShortcutClient *client = [ShortcutClient new];
    TestCandidatePanel *panel = [TestCandidatePanel new];
    ShortcutSession *session = [ShortcutSession new];
    [controller setValue:appearance forKey:@"appearance"];
    [controller setValue:client forKey:@"activeClient"];
    [controller setValue:panel forKey:@"panel"];
    [controller setValue:session forKey:@"session"];
    NSMenu *menu = controller.menu;
    CheckMenu(menu, controller);
    assert([menu itemAtIndex:0].state == NSControlStateValueOn);
    assert([menu itemAtIndex:1].state == NSControlStateValueOff);
    assert(menu.numberOfItems == 27);
    assert([[menu itemAtIndex:17].title isEqual:@"悬浮工具栏"]);
    NSArray<NSString *> *toolTitles = @[@"水杉表情面板…", @"水杉屏幕键盘…", @"手写输入…", @"开始/结束语音输入"];
    NSArray<NSString *> *toolActions = @[@"showEmoji:", @"showScreenKeyboard:", @"showHandwriting:", @"showVoicePanel"];
    for (NSUInteger index = 0; index < toolTitles.count; ++index) {
        NSMenuItem *tool = [menu itemAtIndex:18 + index];
        assert([tool.title isEqual:toolTitles[index]] && tool.action == NSSelectorFromString(toolActions[index]));
    }
    assert([menu itemAtIndex:22].separatorItem);
    assert([[menu itemAtIndex:24].title isEqual:@"词库…"] && [menu itemAtIndex:24].action == @selector(showDictionary:));
    assert([[menu itemAtIndex:25].title isEqual:@"水杉输入法设置…"] &&
           [menu itemAtIndex:25].action == @selector(showAppearance:));
    assert([[menu itemAtIndex:26].title isEqual:@"关于水杉输入法…"] &&
           [menu itemAtIndex:26].action == @selector(showAbout:));
    // The typing toggles mirror the toolbar's runtime state, show the chords handleEvent claims, and flip through the same paths.
    NSMenuItem *fullWidth = [menu itemAtIndex:7], *punctuation = [menu itemAtIndex:8], *translations = [menu itemAtIndex:9];
    assert([fullWidth.title isEqual:@"全角字符"] && [fullWidth.keyEquivalent isEqual:@" "] &&
           fullWidth.keyEquivalentModifierMask == (NSEventModifierFlagControl | NSEventModifierFlagShift));
    assert([punctuation.title isEqual:@"中文标点"] && [punctuation.keyEquivalent isEqual:@"."] &&
           punctuation.keyEquivalentModifierMask == NSEventModifierFlagControl);
    assert([translations.title isEqual:@"显示译文"]);
    const BOOL runtimeWidth = appearance.runtimeFullWidthInput;
    assert(fullWidth.state == (runtimeWidth ? NSControlStateValueOn : NSControlStateValueOff));
    session.widthCalls = 0;
    [NSApp sendAction:fullWidth.action to:fullWidth.target from:fullWidth];
    assert(appearance.runtimeFullWidthInput == !runtimeWidth && session.widthCalls == 1 && session.fullwidth == !runtimeWidth);
    assert([controller.menu itemAtIndex:7].state == (runtimeWidth ? NSControlStateValueOff : NSControlStateValueOn));
    [NSApp sendAction:fullWidth.action to:fullWidth.target from:fullWidth];
    assert(appearance.runtimeFullWidthInput == runtimeWidth);
    NSString *lock = appearance.punctuationLock;
    appearance.punctuationLock = @"follow";
    const BOOL runtimePunctuation = appearance.runtimeChinesePunctuation;
    punctuation = [controller.menu itemAtIndex:8];
    assert(punctuation.enabled && punctuation.state == (runtimePunctuation ? NSControlStateValueOn : NSControlStateValueOff));
    [NSApp sendAction:punctuation.action to:punctuation.target from:punctuation];
    assert(appearance.runtimeChinesePunctuation == !runtimePunctuation && session.chinesePunctuation == !runtimePunctuation);
    assert([controller.menu itemAtIndex:8].state == (runtimePunctuation ? NSControlStateValueOff : NSControlStateValueOn));
    [NSApp sendAction:punctuation.action to:punctuation.target from:punctuation];
    assert(appearance.runtimeChinesePunctuation == runtimePunctuation);
    appearance.punctuationLock = @"english";
    assert(![controller.menu itemAtIndex:8].enabled);
    appearance.punctuationLock = lock;
    const BOOL showTranslations = appearance.candidateTranslations;
    assert(translations.state == (showTranslations ? NSControlStateValueOn : NSControlStateValueOff));
    [NSApp sendAction:translations.action to:translations.target from:translations];
    assert(appearance.candidateTranslations == !showTranslations);
    assert([controller.menu itemAtIndex:9].state == (showTranslations ? NSControlStateValueOff : NSControlStateValueOn));
    appearance.candidateTranslations = showTranslations;
    // The scheme is a radio group under a disabled header, the 双拼 row naming the selected profile.
    NSString *scheme = appearance.inputScheme, *profile = appearance.shuangpinProfile;
    appearance.inputScheme = @"quanpin";
    appearance.shuangpinProfile = @"ziranma";
    menu = controller.menu;
    assert([[menu itemAtIndex:11].title isEqual:@"输入方案"] && ![menu itemAtIndex:11].enabled);
    NSArray<NSString *> *schemeTitles = @[@"全拼", @"双拼（自然码）", @"五笔 86", @"日语"];
    NSArray<NSString *> *schemeIDs = @[@"quanpin", @"shuangpin", @"wubi", @"japanese"];
    for (NSUInteger index = 0; index < schemeIDs.count; ++index) {
        NSMenuItem *item = [menu itemAtIndex:12 + index];
        assert([item.title isEqual:schemeTitles[index]] && [item.representedObject isEqual:schemeIDs[index]]);
        assert(item.state == (index == 0 ? NSControlStateValueOn : NSControlStateValueOff));
    }
    [NSApp sendAction:@selector(selectInputScheme:) to:controller from:[menu itemAtIndex:14]];
    assert([appearance.inputScheme isEqual:@"wubi"]);
    assert([controller.menu itemAtIndex:14].state == NSControlStateValueOn && [controller.menu itemAtIndex:12].state == NSControlStateValueOff);
    appearance.inputScheme = scheme;
    appearance.shuangpinProfile = profile;
    // The theme submenu is the shared catalog, ticked at the current theme, whose name the parent row carries.
    NSString *globalTheme = appearance.globalTheme;
    appearance.globalTheme = @"system";
    NSMenuItem *themeItem = [controller.menu itemAtIndex:23];
    const auto &catalog = msime::mac::ThemeCatalog();
    assert(themeItem.submenu.numberOfItems == (NSInteger)catalog.size());
    assert(([themeItem.title isEqual:[NSString stringWithFormat:@"主题（%@）", @(catalog[0].title.c_str())]]));
    for (size_t index = 0; index < catalog.size(); ++index) {
        NSMenuItem *item = [themeItem.submenu itemAtIndex:index];
        assert([item.representedObject isEqual:@(catalog[index].id.c_str())] && item.action == @selector(selectGlobalTheme:) && item.target == controller);
        assert(item.state == (index == 0 ? NSControlStateValueOn : NSControlStateValueOff));
    }
    [NSApp sendAction:@selector(selectGlobalTheme:) to:controller from:[themeItem.submenu itemAtIndex:1]];
    assert([appearance.globalTheme isEqual:@(catalog[1].id.c_str())]);
    themeItem = [controller.menu itemAtIndex:23];
    assert(([themeItem.title isEqual:[NSString stringWithFormat:@"主题（%@）", @(catalog[1].title.c_str())]]));
    assert([themeItem.submenu itemAtIndex:1].state == NSControlStateValueOn);
    // A theme with a mode of its own fixes the menus' mode as it fixes the candidate window's and the toolbar's, over an explicit menu theme.
    [controller setValue:@{@"theme": @"light", @"menu_theme": @"light"} forKey:@"menuThemePreferences"];
    assert([controller.menu.appearance.name isEqual:NSAppearanceNameDarkAqua]);
    appearance.globalTheme = @"paper";
    assert([controller.menu.appearance.name isEqual:NSAppearanceNameAqua]);
    appearance.globalTheme = @"system";
    [controller setValue:@{@"theme": @"system", @"menu_theme": @"follow"} forKey:@"menuThemePreferences"];
    assert(controller.menu.appearance == nil);
    [controller setValue:nil forKey:@"menuThemePreferences"];
    appearance.globalTheme = globalTheme;
    client.marked = @"ceshi";
    panel.visible = YES;
    [NSApp sendAction:[menu itemAtIndex:1].action to:controller from:[menu itemAtIndex:1]];
    assert(appearance.englishMode && !panel.visible);
    assert([client.committed isEqual:@"测试"] && client.marked.length == 0);
    assert([controller.menu itemAtIndex:1].state == NSControlStateValueOn);
    assert(reloaded.englishMode); // Persistence is shared, not held only in the controller.
    session.lastCommand = UINT32_MAX;
    session.asciiCalls = 0;
    for (NSNumber *flags in @[@0, @(NSEventModifierFlagCommand), @(NSEventModifierFlagOption)]) {
        assert(![controller handleEvent:ModeKey(0, flags.unsignedIntegerValue, NO) client:client]);
    }
    assert(session.lastCommand == UINT32_MAX && session.asciiCalls == 0);
    assert([controller handleEvent:ModeKey(49, NSEventModifierFlagShift, YES) client:client]);
    assert(appearance.englishMode);
    assert([controller handleEvent:ModeKey(49, NSEventModifierFlagShift, NO) client:client]);
    assert(!appearance.englishMode);
    assert([controller handleEvent:ModeKey(49, NSEventModifierFlagShift, YES) client:client]);
    assert(!appearance.englishMode);
    const BOOL beforeWidth = appearance.fullWidthInput;
    for (NSUInteger mask = 1; mask < 8; ++mask) {
        NSEventModifierFlags flags = NSEventModifierFlagShift;
        if (mask & 1) flags |= NSEventModifierFlagCommand;
        if (mask & 2) flags |= NSEventModifierFlagControl;
        if (mask & 4) flags |= NSEventModifierFlagOption;
        assert([controller handleEvent:ModeKey(49, flags, NO) client:client] == (mask == 2));
        assert(!appearance.englishMode);
    }
    appearance.fullWidthInput = beforeWidth;
    appearance.inputModeShortcut = NO;
    [controller handleEvent:ModeKey(49, NSEventModifierFlagShift, NO) client:client];
    assert(!appearance.englishMode && session.lastCommand == MSIME_COMMIT_CANDIDATE);
    appearance.inputModeShortcut = YES;
    session.failFinish = YES;
    session.failCancel = YES;
    panel.visible = YES;
    [controller selectEnglishMode:nil];
    [controller openCharacterPalette:nil];
    assert(!appearance.englishMode && panel.visible && controller.paletteCalls == 0);
    session.failFinish = NO;
    session.failCancel = NO;
    client.marked = @"ceshi";
    client.committed = nil;
    [controller openCharacterPalette:nil];
    assert(controller.paletteCalls == 1 && [client.committed isEqual:@"测试"] && client.marked.length == 0);
    [controller setValue:nil forKey:@"session"];
    [controller selectEnglishMode:nil];
    assert(![controller handleEvent:ModeKey(0, 0, NO) client:client]);
    assert(controller.preparationCalls == 0);
    [controller handleEvent:ModeKey(49, NSEventModifierFlagShift, NO) client:client];
    assert(!appearance.englishMode && controller.preparationCalls == 0);
    assert(![controller handleEvent:ModeKey(0, 0, NO) client:client]);
    assert(controller.preparationCalls == 1);
    [controller setValue:session forKey:@"session"];
    [controller setValue:@YES forKey:@"focusPending"];
    session.focusCalls = 0;
    [controller handleEvent:ModeKey(0, 0, NO) client:client];
    assert(session.focusCalls == 1);
    [controller handleEvent:ModeKey(0, 0, NO) client:client];
    assert(session.focusCalls == 1);
    appearance.englishMode = NO;
}

// The rendered order is not the candidate order: a pinned candidate is drawn first whatever its index.
static MSIMECandidateButton *CandidateButtonWithID(NSView *container, NSDictionary *identifier) {
    for (NSView *child in container.subviews)
        if ([child isKindOfClass:MSIMECandidateButton.class] &&
            [((MSIMECandidateButton *)child).candidateID isEqual:identifier])
            return (MSIMECandidateButton *)child;
    return nil;
}
static MSIMECandidateButton *PageButton(NSView *content, NSInteger tag) {
    for (NSView *view in content.subviews) {
        if ([view isKindOfClass:MSIMECandidateButton.class] && view.tag == tag) return (id)view;
    }
    return nil;
}

static void TestExternalSkin(MSIMEInputController *controller, HiddenCandidatePanel *panel, NSUserDefaults *defaults) {
    char temporary[] = "/tmp/msime-native-skin-XXXXXX";
    assert(mkdtemp(temporary));
    const std::filesystem::path root(temporary);
    std::filesystem::create_directory(root / "synthetic");
    {
        std::ofstream manifest(root / "synthetic" / "skin.toml");
        manifest << R"toml(schema_version = 1
id = "synthetic"
name = "Synthetic Skin"
version = "1.0"
base = "system"
preview = "decoration.png"
[supports]
layouts = ["horizontal", "vertical"]
themes = ["dark", "light"]
[candidate_window]
min_width_dip = 240
[candidate_window.decoration]
top_inset_dip = 48
width_dip = 120
[candidate.light]
surface = "#fff7fa"
selected = "#111111"
show_selected_bar = false
[candidate.dark]
surface = "#121314"
selected = "#ffffff"
show_selected_bar = true
)toml";
        assert(manifest.good());
    }
    NSBitmapImageRep *image = [[NSBitmapImageRep alloc] initWithBitmapDataPlanes:nullptr pixelsWide:4 pixelsHigh:4 bitsPerSample:8 samplesPerPixel:4 hasAlpha:YES isPlanar:NO colorSpaceName:NSDeviceRGBColorSpace bytesPerRow:0 bitsPerPixel:0];
    for (NSInteger y = 0; y < 4; ++y) for (NSInteger x = 0; x < 4; ++x) {
        unsigned char *pixel = image.bitmapData + y * image.bytesPerRow + x * 4;
        pixel[0] = 255; pixel[1] = 0; pixel[2] = 0; pixel[3] = 255;
    }
    image = [image bitmapImageRepByRetaggingWithColorSpace:NSColorSpace.sRGBColorSpace];
    assert(image);
    assert([[image representationUsingType:NSBitmapImageFileTypePNG properties:@{}] writeToFile:@((root / "synthetic" / "decoration.png").c_str()) atomically:YES]);
    MSIMEAppearancePreferences *external = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults skinsRoot:[NSURL fileURLWithPath:@(root.c_str()) isDirectory:YES]];
    // Skins are picked from the cards on the 皮肤 page, which is the browser itself now; the popup
    // of skin names it replaced could not show what any of them looked like.
    MetasequoiaSkinSettingsView *picker = (id)[external skinSettingsView];
    NSArray<NSSwitch *> *pickerSwitches = [picker valueForKey:@"switches"];
    // Seven global themes, then the package.
    assert(pickerSwitches.count == 8 && [pickerSwitches.lastObject.identifier isEqual:@"synthetic"]);
    [NSApp sendAction:pickerSwitches.lastObject.action to:pickerSwitches.lastObject.target from:pickerSwitches.lastObject];
    assert([external.globalTheme isEqual:@"custom"] && [external.customCandidateSkin isEqual:@"synthetic"] && external.decorationImage);
    MSIMEAppearancePreferences *loaded = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults skinsRoot:external.skinsRoot];
    assert([loaded.customCandidateSkin isEqual:@"synthetic"] && [loaded resolvedSkinForDark:NO].candidateSkin == "synthetic");
    NSDictionary *before = [[controller valueForKey:@"view"] copy];
    [controller setValue:external forKey:@"appearance"];
    MSIMEFloatingToolbarPanel *toolbar = [MSIMEFloatingToolbarPanel new];
    [toolbar setFrameAutosaveName:@""];
    [controller setValue:toolbar forKey:@"toolbar"];
    MetasequoiaSkinSettingsView *cards = (id)[external skinSettingsView];
    NSArray<NSSwitch *> *skinSwitches = [cards valueForKey:@"switches"];
    assert([skinSwitches.lastObject.identifier isEqual:@"synthetic"]);
    [NSNotificationCenter.defaultCenter addObserver:controller selector:@selector(appearanceChanged:)
                                              name:MSIMEAppearanceDidChangeNotification object:external];
    [NSApp sendAction:skinSwitches.firstObject.action to:skinSwitches.firstObject.target from:skinSwitches.firstObject];
    assert([external.globalTheme isEqual:@"system"] && !external.decorationImage);
    [NSApp sendAction:skinSwitches.lastObject.action to:skinSwitches.lastObject.target from:skinSwitches.lastObject];
    assert([external.globalTheme isEqual:@"custom"] && [external.customCandidateSkin isEqual:@"synthetic"] && external.decorationImage);
    assert(skinSwitches.lastObject.state == NSControlStateValueOn);
    assert([panel.contentView.subviews.lastObject isKindOfClass:NSImageView.class]);
    assert([[controller valueForKey:@"view"] isEqual:before]);
    [NSNotificationCenter.defaultCenter removeObserver:controller name:MSIMEAppearanceDidChangeNotification object:external];
    for (NSNumber *vertical in @[@NO, @YES]) {
        external.vertical = vertical.boolValue;
        for (NSString *theme in @[NSAppearanceNameAqua, NSAppearanceNameDarkAqua]) {
            panel.appearance = [NSAppearance appearanceNamed:theme];
            [controller appearanceChanged:nil];
            [toolbar applyThemePreferences:@{@"theme": [theme isEqual:NSAppearanceNameDarkAqua] ? @"dark" : @"light"}];
            NSColor *toolbarFill = [[toolbar valueForKey:@"chrome"] valueForKey:@"fillColor"];
            // The toolbar derives from the candidate palette of the theme on screen (THEME_CONTRACT §3), the package's colours included.
            const BOOL dark = [theme isEqual:NSAppearanceNameDarkAqua];
            const auto toolbarTokens = [external toolbarSkinForDark:dark];
            assert([toolbarFill isEqual:SkinColor(toolbarTokens.surface)]);
            assert([toolbarFill isEqual:SkinColor([external resolvedSkinForDark:dark].tokens.surface)]);
            MSIMECandidateChromeView *chrome = (id)panel.contentView;
            NSImageView *decoration = (id)chrome.subviews.lastObject;
            assert([decoration isKindOfClass:NSImageView.class] && decoration.image);
            assert(panel.frame.size.width >= 240);
            assert(decoration.frame.size.width == 120 && decoration.frame.size.height == 48);
            assert(NSMaxX(decoration.frame) == chrome.bounds.size.width && NSMaxY(decoration.frame) == chrome.bounds.size.height);
            MSIMECandidateButton *first = PageButton(chrome, 0);
            assert(NSMaxY(first.frame) <= NSMinY(decoration.frame));
            assert(first.showSelectedBar == [theme isEqual:NSAppearanceNameDarkAqua]);
            const auto tokens = [external resolvedSkinForDark:[theme isEqual:NSAppearanceNameDarkAqua]].tokens;
            assert([chrome.fillColor isEqual:SkinColor(tokens.surface)]);
            assert([first.titleColor isEqual:SkinColor(tokens.selectedText)]);
            NSBitmapImageRep *bitmap = [chrome bitmapImageRepForCachingDisplayInRect:chrome.bounds];
            [chrome cacheDisplayInRect:chrome.bounds toBitmapImageRep:bitmap];
            assert(bitmap && [[controller valueForKey:@"view"] isEqual:before]);
        }
    }
    // No disk reads while typing/rendering: removal takes effect only on explicit reload.
    std::filesystem::remove_all(root / "synthetic");
    [controller renderCandidates];
    assert([external resolvedSkinForDark:NO].candidateSkin == "synthetic" && external.decorationImage);
    // Rescanning is 刷新皮肤 on the skin page, which rebuilds the cards from the directory; the
    // accessor performs the same reload the button does.
    MetasequoiaSkinSettingsView *rescanned = (id)[external skinSettingsView];
    // The choice is kept for when the package comes back; meanwhile the custom theme is drawn without it.
    assert([external.customCandidateSkin isEqual:@"synthetic"]);
    assert([external resolvedSkinForDark:NO].id == "custom" && [external resolvedSkinForDark:NO].candidateSkin.empty() &&
           !external.decorationImage);
    NSArray<NSSwitch *> *rescannedSwitches = [rescanned valueForKey:@"switches"];
    assert(rescannedSwitches.count == 7 && [rescannedSwitches.firstObject.identifier isEqual:@"system"]);
    [controller appearanceChanged:nil];
    for (NSView *view in panel.contentView.subviews) assert(![view isKindOfClass:NSImageView.class]);
    panel.appearance = nil;
    std::filesystem::remove_all(root);
}

@interface CloudShortcutSession : ShortcutSession
@property(nonatomic, copy) NSDictionary *query;
@property(nonatomic) NSUInteger cloudApplications;
@property(nonatomic) BOOL rejectNextCloud;
@end
@implementation CloudShortcutSession
- (NSDictionary *)onlineQueryWithError:(NSError **)error { (void)error; return self.query; }
- (NSDictionary *)applyCloudResponse:(NSData *)body query:(NSDictionary *)query error:(NSError **)error {
    (void)body; (void)error;
    assert([query isEqual:self.query] && NSThread.isMainThread);
    if (self.rejectNextCloud) {
        self.rejectNextCloud = NO;
        return @{ @"applied": @NO, @"view": @{} };
    }
    ++self.cloudApplications;
    NSMutableDictionary *updated = [self.query mutableCopy];
    updated[@"generation"] = @([updated[@"generation"] unsignedLongLongValue] + 1);
    self.query = updated;
    return @{@"applied":@YES, @"view":@{@"focused":@YES, @"preedit":@"synthetic", @"editing_text":@"synthetic", @"caret_position":@0, @"candidates":@[]}};
}
@end
@interface ControlledCloudRequest : MSIMECloudCandidateRequest
@property(nonatomic, copy) void (^reply)(NSData *);
@property(nonatomic) BOOL started;
@property(nonatomic) BOOL cancelled;
@end
@implementation ControlledCloudRequest
- (void)start { self.started = YES; }
- (void)cancel { self.cancelled = YES; }
@end
@interface CloudShortcutController : ModeController
@property(nonatomic, strong) NSMutableArray<ControlledCloudRequest *> *requests;
@end
@implementation CloudShortcutController
- (MSIMECloudCandidateRequest *)cloudRequestForURL:(NSURL *)url completion:(void (^)(NSData *))completion {
    assert([url.host isEqual:@"inputtools.google.com"]);
    ControlledCloudRequest *request = [ControlledCloudRequest new];
    request.reply = completion;
    [self.requests addObject:request];
    return request;
}
- (void)renderCandidates {} // Keep the test independent of real panel placement.
@end

static void TestCloudCandidateScheduling() {
    CloudShortcutController *controller = [CloudShortcutController alloc];
    controller.requests = [NSMutableArray array];
    CloudShortcutSession *session = [CloudShortcutSession new];
    session.query = @{@"scheme":@0, @"generation":@1, @"identity":@"synthetic", @"query_text":@"nihao", @"cache_key":@"nihao", @"pinyin_segments":@[@"ni", @"hao"], @"cloud_eligible":@YES, @"ai_eligible":@NO, @"cloud_candidates":@YES, @"session_id":@1};
    ShortcutClient *client = [ShortcutClient new], *other = [ShortcutClient new];
    [controller setValue:session forKey:@"session"];
    [controller setValue:client forKey:@"activeClient"];
    [controller synchronizeCloudCandidates];
    NSTimer *timer = [controller valueForKey:@"cloudTimer"];
    assert(timer && timer.fireDate.timeIntervalSinceNow > 0.4 && controller.requests.count == 0);
    [controller synchronizeCloudCandidates];
    assert([controller valueForKey:@"cloudTimer"] == timer);
    [timer fire]; [timer invalidate];
    ControlledCloudRequest *first = controller.requests.lastObject;
    assert(first.started && controller.requests.count == 1);
    NSData *body = [@"synthetic" dataUsingEncoding:NSUTF8StringEncoding];
    first.reply(body);
    assert(session.cloudApplications == 1 && client.committed == nil && [client.marked isEqual:@"synthetic"]);
    assert([controller valueForKey:@"cloudTimer"] == nil); // No response-triggered request loop.
    [controller cancelCloudCandidates];
    [controller synchronizeCloudCandidates];
    timer = [controller valueForKey:@"cloudTimer"];
    [timer fire]; [timer invalidate];
    ControlledCloudRequest *stale = controller.requests.lastObject;
    [controller setValue:other forKey:@"activeClient"];
    stale.reply(body);
    assert(session.cloudApplications == 1);
    [controller cancelCloudCandidates];
    assert(stale.cancelled);
    [controller setValue:client forKey:@"activeClient"];
    [controller synchronizeCloudCandidates];
    timer = [controller valueForKey:@"cloudTimer"];
    [timer fire]; [timer invalidate];
    stale.reply(body); // A -> B -> A must still reject the old response.
    assert(session.cloudApplications == 1);
    ControlledCloudRequest *disabled = controller.requests.lastObject;
    NSMutableDictionary *query = [session.query mutableCopy];
    query[@"cloud_candidates"] = @NO;
    session.query = query;
    disabled.reply(body); // Recheck permission even before the next synchronization.
    assert(session.cloudApplications == 1);
    [controller synchronizeCloudCandidates];
    assert(disabled.cancelled && [controller valueForKey:@"cloudTimer"] == nil);
    query[@"cloud_candidates"] = @YES;
    session.query = query;
    [controller synchronizeCloudCandidates];
    timer = [controller valueForKey:@"cloudTimer"];
    session.query = nil; // Composition cancelled before debounce expires.
    [timer fire]; [timer invalidate];
    assert(controller.requests.count == 3);
    [controller cancelCloudCandidates];
    session.query = query;
    [controller synchronizeCloudCandidates];
    NSTimer *oldTimer = [controller valueForKey:@"cloudTimer"];
    NSMutableDictionary *newQuery = [query mutableCopy];
    newQuery[@"generation"] = @99;
    session.query = newQuery;
    [controller synchronizeCloudCandidates];
    assert(!oldTimer.valid);
    timer = [controller valueForKey:@"cloudTimer"];
    assert(timer != oldTimer);
    [timer fire]; [timer invalidate];
    ControlledCloudRequest *replaced = controller.requests.lastObject;
    [controller setValue:[CloudShortcutSession new] forKey:@"session"];
    replaced.reply(body);
    assert(session.cloudApplications == 1);
    [controller setValue:session forKey:@"session"];
    [controller snapshotSessionReplaced:[NSNotification notificationWithName:MSIMEClientSessionDidReplaceSnapshotNotification object:session]];
    assert(replaced.cancelled);
    replaced.reply(body);
    assert(session.cloudApplications == 1);
    [controller synchronizeCloudCandidates];
    assert([controller valueForKey:@"cloudTimer"] == nil);
    [controller setValue:@NO forKey:@"focusPending"];
    [controller synchronizeCloudCandidates];
    timer = [controller valueForKey:@"cloudTimer"];
    [timer fire]; [timer invalidate];
    ControlledCloudRequest *blurred = controller.requests.lastObject;
    Method base = class_getInstanceMethod(IMKInputController.class, @selector(deactivateServer:));
    IMP original = method_setImplementation(base, (IMP)RecordBaseDeactivation);
    [controller deactivateServer:other];
    assert(!blurred.cancelled);
    session.query = nil;
    [controller deactivateServer:client];
    assert(blurred.cancelled && [controller valueForKey:@"activeClient"] == nil);
    blurred.reply(body);
    assert(session.cloudApplications == 1);
    method_setImplementation(base, original);
}

static void TestCloudCandidateRetryAfterRejectedResponse() {
    CloudShortcutController *controller = [CloudShortcutController alloc];
    controller.requests = [NSMutableArray array];
    CloudShortcutSession *session = [CloudShortcutSession new];
    session.query = @{ @"scheme": @0, @"generation": @1, @"identity": @"synthetic-retry",
        @"query_text": @"nihao", @"cache_key": @"nihao", @"pinyin_segments": @[@"ni", @"hao"],
        @"cloud_eligible": @YES, @"ai_eligible": @NO, @"cloud_candidates": @YES, @"session_id": @1 };
    session.rejectNextCloud = YES;
    [controller setValue:session forKey:@"session"];
    [controller setValue:[ShortcutClient new] forKey:@"activeClient"];
    [controller synchronizeCloudCandidates];
    NSTimer *timer = [controller valueForKey:@"cloudTimer"];
    [timer fire]; [timer invalidate];
    assert(controller.requests.count == 1);
    ControlledCloudRequest *rejected = controller.requests.lastObject;
    rejected.reply([@"synthetic" dataUsingEncoding:NSUTF8StringEncoding]);
    assert(session.cloudApplications == 0);
    // A rejected response must not poison the query identity: the next render can retry
    // after the local candidate page has been rebuilt.
    assert([controller valueForKey:@"cloudQuery"] == nil);
    [controller synchronizeCloudCandidates];
    timer = [controller valueForKey:@"cloudTimer"];
    assert(timer);
    [timer fire]; [timer invalidate];
    ControlledCloudRequest *retry = controller.requests.lastObject;
    retry.reply([@"synthetic" dataUsingEncoding:NSUTF8StringEncoding]);
    assert(session.cloudApplications == 1);
    [controller cancelCloudCandidates];
}

static void TestAiCandidateEngineDelivery() {
    NSError *bridgeError = nil;
    NSDictionary *descriptor = [MSIMEClientSession aiHTTPRequest:@{
        @"config":@{@"enabled":@YES, @"provider":@"deepseek", @"endpoint":@"https://synthetic.invalid/chat", @"model":@"synthetic",
            @"token":@"synthetic-secret", @"candidate_limit":@3, @"prompt_id":@"custom_2", @"prompt_custom_2":@"synthetic prompt"},
        @"input":@{@"segmented_pinyin":@[@"ni", @"hao"], @"context":@"", @"candidate_limit":@3}} error:&bridgeError];
    assert(descriptor && !bridgeError && [descriptor[@"timeout_ms"] isEqual:@8000]);
    assert([descriptor[@"headers"][@"Authorization"] isEqual:@"Bearer synthetic-secret"]);
    assert([descriptor[@"body"][@"thinking"][@"type"] isEqual:@"disabled"]);
    NSData *response = [NSJSONSerialization dataWithJSONObject:@{@"choices":@[@{@"message":@{@"content":
        @"{\"candidates\":[{\"text\":\"合成候选甲\"},{\"text\":\"合成候选乙\"}]}"}}]} options:0 error:nil];
    NSArray *parsed = [MSIMEClientSession parseAIResponse:response limit:3 error:&bridgeError];
    assert(!bridgeError && parsed.count == 2);
    NSString *root = [NSTemporaryDirectory() stringByAppendingPathComponent:NSUUID.UUID.UUIDString];
    NSMutableDictionary *options = [@{@"api_version":@1, @"preferences":@{@"scheme":@"quanpin", @"learning":@NO,
        @"candidate_page_size":@5, @"chinese_punctuation":@YES, @"default_ime_mode":@"chinese",
        @"ai_assistant":@{@"enabled":@YES, @"provider":@"openai", @"endpoint":@"https://synthetic.invalid/chat",
            @"model":@"synthetic", @"token":@"synthetic-private", @"candidate_limit":@3}}} mutableCopy];
    for (NSString *name in @[@"resources", @"user_data", @"cache", @"dictionaries"]) {
        NSString *path = [root stringByAppendingPathComponent:name];
        assert([NSFileManager.defaultManager createDirectoryAtPath:path withIntermediateDirectories:YES attributes:nil error:nil]);
        options[name] = path;
    }
    NSError *error = nil;
    MSIMEClientSession *session = [[MSIMEClientSession alloc] initWithOptions:options error:&error];
    assert(session && !error);
    [session setFocused:YES error:&error];
    for (char byte : std::string("nihaoshijie")) [session typeASCII:byte shift:NO error:&error];
    NSDictionary *query = [session onlineQueryWithError:&error];
    assert(!error && [query[@"ai_eligible"] boolValue]);
    assert(!query[@"ai_assistant"][@"token"]); // Copied queries never expose credentials.
    NSDictionary *sessionDescriptor = [session aiRequestForQuery:query error:&error];
    assert(sessionDescriptor && !error &&
        [sessionDescriptor[@"headers"][@"Authorization"] isEqual:@"Bearer synthetic-private"]);
    NSDictionary *applied = [session applyOnlineCandidates:parsed source:1 query:query error:&error];
    assert(!error && [applied[@"applied"] boolValue]);
    NSArray *candidates = applied[@"view"][@"candidates"];
    assert(candidates.count >= 2 && [candidates[0][@"text"] isEqual:@"合成候选甲"] && [candidates[1][@"text"] isEqual:@"合成候选乙"]);
    assert([candidates[0][@"source"] isEqual:@3] && [candidates[1][@"source"] isEqual:@3]);
    assert([applied[@"view"][@"editing_text"] isEqual:@"nihaoshijie"]);
    [session typeASCII:'a' shift:NO error:&error];
    applied = [session applyOnlineCandidates:@[@"过期候选"] source:1 query:query error:&error];
    assert(!error && ![applied[@"applied"] boolValue]);
    query = [session onlineQueryWithError:&error];
    applied = [session applyOnlineCandidates:@[@"甲", @"乙", @"丙", @"丁"] source:1 query:query error:&error];
    assert(!error && ![applied[@"applied"] boolValue]); // Configured limit, not just ABI limit.
    assert(![session applyOnlineCandidates:(id)@[@1] source:1 query:query error:&error] && error);
    error = nil;
    assert(![session applyOnlineCandidates:@[@"合成"] source:2 query:query error:&error] && error);
    error = nil;
    NSString *oversized = [@"x" stringByPaddingToLength:4097 withString:@"x" startingAtIndex:0];
    assert(![session applyOnlineCandidates:@[oversized] source:1 query:query error:&error] && error);
    error = nil;
    NSMutableDictionary *largeQuery = [query mutableCopy];
    largeQuery[@"synthetic"] = [@"x" stringByPaddingToLength:16385 withString:@"x" startingAtIndex:0];
    assert(![session applyOnlineCandidates:@[@"合成"] source:1 query:largeQuery error:&error] && error);
    error = nil;
    applied = [session applyOnlineCandidates:@[] source:1 query:query error:&error];
    assert(!error && ![applied[@"applied"] boolValue]);
    assert([NSFileManager.defaultManager removeItemAtPath:root error:&error] && !error);
}
static void TestCloudCandidateEngineDelivery() {
    NSString *root = [NSTemporaryDirectory() stringByAppendingPathComponent:NSUUID.UUID.UUIDString];
    NSMutableDictionary *options = [@{@"api_version":@1, @"preferences":@{@"scheme":@"quanpin", @"candidate_page_size":@5, @"chinese_punctuation":@YES, @"cloud_candidates":@YES, @"learning":@NO, @"default_ime_mode":@"chinese"}} mutableCopy];
    for (NSString *name in @[@"resources", @"user_data", @"cache", @"dictionaries"]) {
        NSString *path = [root stringByAppendingPathComponent:name];
        assert([NSFileManager.defaultManager createDirectoryAtPath:path withIntermediateDirectories:YES attributes:nil error:nil]);
        options[name] = path;
    }
    NSError *error = nil;
    MSIMEClientSession *session = [[MSIMEClientSession alloc] initWithOptions:options error:&error];
    assert(session && !error);
    CloudShortcutController *controller = [CloudShortcutController alloc];
    controller.requests = [NSMutableArray array];
    ShortcutClient *client = [ShortcutClient new];
    [controller setValue:session forKey:@"session"];
    [controller setValue:client forKey:@"activeClient"];
    [controller apply:[session setFocused:YES error:&error]];
    for (char byte : std::string("nihao")) [controller apply:[session typeASCII:byte shift:NO error:&error]];
    assert(!error && controller.requests.count == 0);
    NSTimer *timer = [controller valueForKey:@"cloudTimer"];
    assert(timer);
    [timer fire]; [timer invalidate];
    assert(controller.requests.count == 1);
    controller.requests.lastObject.reply([@"[\"SUCCESS\", [[\"nihao\", [\"云端测试候选\"]]]]" dataUsingEncoding:NSUTF8StringEncoding]);
    NSDictionary *view = [session viewWithError:&error];
    assert(!error && [view[@"editing_text"] isEqual:@"nihao"]);
    // A cloud suggestion is merged into an existing candidate page and never manufactures one. This
    // session is built on empty resource directories, so the Engine offers nothing locally and the
    // suggestion is refused - which is the documented contract, matching Windows: a callback arriving
    // after the local page was cleared must not rebuild a page out of stale provider state.
    //
    // What this test is for is the controller's plumbing above: that typing schedules a request rather
    // than sending one per keystroke, that firing the timer sends exactly one, and that a reply is
    // accepted without error. The merge itself needs a real dictionary to have a page to merge into, and
    // is covered at the layer that has one - crates/host-api/src/tests.rs drives the same "nihao" query
    // against a host with resources and asserts the candidate reaches the view.
    assert([view[@"candidates"] isKindOfClass:NSArray.class] && [view[@"candidates"] count] == 0);
    assert(client.committed == nil && [controller valueForKey:@"cloudTimer"] == nil);
    [controller cancelCloudCandidates];
    assert([session closeWithError:&error] && !error);
    assert([NSFileManager.defaultManager removeItemAtPath:root error:nil]);
}

static void TestCloudCandidatePreference() {
    NSString *suite = [@"msime.cloud.preference." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    MSIMEAppearancePreferences *prefs = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    NSSwitch *toggle = (id)PreferenceControl(prefs, @selector(cloudCandidatesChanged:));
    assert(prefs.cloudCandidates && toggle.state == NSControlStateValueOn);
    // The switch has no title of its own — the wording naming where the query goes is on the row
    // label, which is also what the switch reports to VoiceOver. Still asserted: this is the one
    // control here that sends what is being typed off the machine, and it has to say so.
    assert([toggle.accessibilityLabel containsString:@"Google"]);
    assert(![prefs sharedPreferencesByMerging:@{}][@"cloud_candidates"]);
    assert([[prefs sharedPreferencesByMerging:@{@"cloud_candidates":@NO}][@"cloud_candidates"] isEqual:@NO]);
    __block NSUInteger saves = 0;
    id observer = [NSNotificationCenter.defaultCenter addObserverForName:MSIMEAppearanceDidChangeNotification object:prefs queue:nil usingBlock:^(NSNotification *note) { (void)note; ++saves; }];
    [prefs applySharedInputPreferences:@{@"cloud_candidates":@NO}];
    assert(!prefs.cloudCandidates && toggle.state == NSControlStateValueOff && saves == 0);
    for (id invalid in @[NSNull.null, @1, @"true"]) [prefs applySharedInputPreferences:@{@"cloud_candidates":invalid}];
    assert(!prefs.cloudCandidates && saves == 0);
    assert([defaults objectForKey:@"MSIMEClientCloudCandidates"] == nil);
    toggle.state = NSControlStateValueOn;
    [NSApp sendAction:toggle.action to:toggle.target from:toggle];
    assert(prefs.cloudCandidates && saves == 1);
    assert([[prefs sharedPreferencesByMerging:@{}][@"cloud_candidates"] isEqual:@YES]);
    CloudShortcutController *controller = [CloudShortcutController alloc];
    controller.requests = [NSMutableArray array];
    CloudShortcutSession *session = [CloudShortcutSession new];
    session.query = @{@"scheme":@0, @"generation":@1, @"identity":@"synthetic", @"query_text":@"nihao", @"cache_key":@"nihao", @"pinyin_segments":@[@"ni", @"hao"], @"cloud_eligible":@YES, @"ai_eligible":@NO, @"cloud_candidates":@YES, @"session_id":@1};
    [controller setValue:prefs forKey:@"appearance"];
    [controller setValue:session forKey:@"session"];
    [controller setValue:[ShortcutClient new] forKey:@"activeClient"];
    [controller synchronizeCloudCandidates];
    NSTimer *timer = [controller valueForKey:@"cloudTimer"];
    [timer fire]; [timer invalidate];
    assert(controller.requests.count == 1);
    ControlledCloudRequest *request = controller.requests.lastObject;
    toggle.state = NSControlStateValueOff;
    [NSApp sendAction:toggle.action to:toggle.target from:toggle];
    // Guard even before the notification handler runs or shared storage is saved.
    request.reply([@"synthetic" dataUsingEncoding:NSUTF8StringEncoding]);
    assert(session.cloudApplications == 0 && !prefs.cloudCandidates && saves == 2);
    [controller appearanceChanged:nil];
    assert(request.cancelled && [controller valueForKey:@"cloudTimer"] == nil);
    [controller synchronizeCloudCandidates];
    assert([controller valueForKey:@"cloudTimer"] == nil);
    assert(![[[MSIMEAppearancePreferences alloc] initWithDefaults:defaults] cloudCandidates]);
    NSString *root = [NSTemporaryDirectory() stringByAppendingPathComponent:NSUUID.UUID.UUIDString];
    NSError *error = nil;
    NSDictionary *snapshot = [MSIMEClientSession loadPreferencesInDirectory:root error:&error];
    assert(snapshot && !error);
    NSDictionary *merged = [prefs sharedPreferencesByMerging:snapshot[@"preferences"]];
    assert([merged[@"cloud_candidates"] isEqual:@NO]);
    assert([merged[@"ai_assistant"] isEqual:snapshot[@"preferences"][@"ai_assistant"]]);
    assert(([MSIMEClientSession savePreferencesInDirectory:root expectedRevision:[snapshot[@"revision"] unsignedLongLongValue]
        snapshot:@{@"format_version":@1, @"revision":snapshot[@"revision"], @"preferences":merged} error:&error] && !error));
    NSDictionary *loaded = [MSIMEClientSession loadPreferencesInDirectory:root error:&error];
    assert(loaded && !error && [loaded[@"preferences"][@"cloud_candidates"] isEqual:@NO]);
    [NSNotificationCenter.defaultCenter removeObserver:observer];
    MSIMERemoveTestPreferenceSuite(defaults, suite);
    MSIMEAppearancePreferences *fresh = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    assert(fresh.cloudCandidates);
    [fresh applySharedInputPreferences:loaded[@"preferences"]];
    assert(!fresh.cloudCandidates);
    [controller cancelCloudCandidates];
    [prefs.window close];
    assert([NSFileManager.defaultManager removeItemAtPath:root error:&error] && !error);
}

@interface ConsentCloudController : CloudShortcutController
@property(nonatomic) NSUInteger prompts;
@property(nonatomic, copy) void (^answer)(NSNumber *);
@end
@implementation ConsentCloudController
- (void)presentCloudConsent:(void (^)(NSNumber *))completion { ++self.prompts; self.answer = completion; }
@end

// Runs the main queue until everything already enqueued on it has run: the main queue is FIFO, so a sentinel enqueued now runs after them. A fixed run-loop slice was not enough on slow CI runners.
static void DrainMainQueue() {
    __block BOOL drained = NO;
    dispatch_async(dispatch_get_main_queue(), ^{ drained = YES; });
    SettleWindowLayout();
    NSDate *deadline = [NSDate dateWithTimeIntervalSinceNow:5];
    while (!drained && deadline.timeIntervalSinceNow > 0) CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.05, true);
    assert(drained);
}

static void TestCloudCandidateConsent() {
    NSDictionary *query = @{@"scheme":@0, @"generation":@1, @"identity":@"synthetic", @"query_text":@"nihao", @"cache_key":@"nihao", @"pinyin_segments":@[@"ni", @"hao"], @"cloud_eligible":@YES, @"ai_eligible":@NO, @"cloud_candidates":@YES, @"session_id":@1};
    NSString *root = [NSTemporaryDirectory() stringByAppendingPathComponent:NSUUID.UUID.UUIDString];
    assert([NSFileManager.defaultManager createDirectoryAtPath:root withIntermediateDirectories:YES attributes:nil error:nil]);
    NSString *preferencesFile = [root stringByAppendingPathComponent:@"preferences.json"];

    // A profile that was never resolved (no preferences directory known) keeps sending as before.
    NSString *suite = [@"msime.cloud.consent." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    MSIMEAppearancePreferences *prefs = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    [prefs resolveCloudCandidatesConsentWithPreferencesDirectory:nil userDataDirectory:nil];
    assert(prefs.cloudCandidatesAnswered && prefs.cloudCandidatesEnabled);

    // Fresh profile: nothing is sent and the prompt is requested exactly once.
    [prefs resolveCloudCandidatesConsentWithPreferencesDirectory:root userDataDirectory:nil];
    assert(!prefs.cloudCandidatesAnswered && prefs.cloudCandidates && !prefs.cloudCandidatesEnabled);
    ConsentCloudController *controller = [ConsentCloudController alloc];
    controller.requests = [NSMutableArray array];
    CloudShortcutSession *session = [CloudShortcutSession new];
    session.query = query;
    [controller setValue:prefs forKey:@"appearance"];
    [controller setValue:session forKey:@"session"];
    [controller setValue:[ShortcutClient new] forKey:@"activeClient"];
    [controller setValue:root forKey:@"preferencesDirectory"];
    [controller synchronizeCloudCandidates];
    assert([controller valueForKey:@"cloudTimer"] == nil && controller.requests.count == 0);
    [controller requestCloudCandidatesConsentIfNeeded];
    [controller requestCloudCandidatesConsentIfNeeded];
    assert(controller.prompts == 0); // Deferred off the activation path.
    DrainMainQueue();
    assert(controller.prompts == 1 && controller.answer);
    [controller requestCloudCandidatesConsentIfNeeded];
    DrainMainQueue();
    assert(controller.prompts == 1); // Still showing.

    // A preferences.json that appears later (this host writes one after any appearance change) does not turn pending into answered.
    assert([@"{}" writeToFile:preferencesFile atomically:YES encoding:NSUTF8StringEncoding error:nil]);
    [prefs resolveCloudCandidatesConsentWithPreferencesDirectory:root userDataDirectory:nil];
    assert(!prefs.cloudCandidatesAnswered);
    assert([NSFileManager.defaultManager removeItemAtPath:preferencesFile error:nil]);

    // Closed without an answer: still pending, asked again on the next activation.
    void (^dismiss)(NSNumber *) = controller.answer;
    dismiss(nil);
    assert(!prefs.cloudCandidatesAnswered);
    [controller requestCloudCandidatesConsentIfNeeded];
    DrainMainQueue();
    assert(controller.prompts == 2);

    // Declining stores cloud_candidates = false in the shared preferences and sends nothing.
    __block NSUInteger changes = 0;
    id observer = [NSNotificationCenter.defaultCenter addObserverForName:MSIMEAppearanceDidChangeNotification object:prefs queue:nil usingBlock:^(NSNotification *note) { (void)note; ++changes; }];
    void (^decline)(NSNumber *) = controller.answer;
    decline(@NO);
    assert(prefs.cloudCandidatesAnswered && !prefs.cloudCandidates && !prefs.cloudCandidatesEnabled && changes == 1);
    assert([[prefs sharedPreferencesByMerging:@{}][@"cloud_candidates"] isEqual:@NO]);
    [controller synchronizeCloudCandidates];
    assert([controller valueForKey:@"cloudTimer"] == nil && controller.requests.count == 0);
    [controller requestCloudCandidatesConsentIfNeeded];
    DrainMainQueue();
    assert(controller.prompts == 2);
    NSError *error = nil;
    NSDictionary *snapshot = [MSIMEClientSession loadPreferencesInDirectory:root error:&error];
    assert(snapshot && !error);
    NSDictionary *merged = [prefs sharedPreferencesByMerging:snapshot[@"preferences"]];
    assert(([MSIMEClientSession savePreferencesInDirectory:root expectedRevision:[snapshot[@"revision"] unsignedLongLongValue]
        snapshot:@{@"format_version":@1, @"revision":snapshot[@"revision"], @"preferences":merged} error:&error] && !error));
    NSDictionary *loaded = [MSIMEClientSession loadPreferencesInDirectory:root error:&error];
    assert(loaded && !error && [loaded[@"preferences"][@"cloud_candidates"] isEqual:@NO]);

    // Enabling afterwards resumes queries.
    [prefs answerCloudCandidates:YES];
    assert(prefs.cloudCandidatesEnabled);
    [controller synchronizeCloudCandidates];
    NSTimer *timer = [controller valueForKey:@"cloudTimer"];
    assert(timer);
    [timer fire]; [timer invalidate];
    assert(controller.requests.count == 1 && controller.requests.lastObject.started);
    [controller cancelCloudCandidates];
    [NSNotificationCenter.defaultCenter removeObserver:observer];
    MSIMERemoveTestPreferenceSuite(defaults, suite);

    // Accepting from the prompt enables queries straight away.
    suite = [@"msime.cloud.consent." stringByAppendingString:NSUUID.UUID.UUIDString];
    defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    NSString *empty = [NSTemporaryDirectory() stringByAppendingPathComponent:NSUUID.UUID.UUIDString];
    assert([NSFileManager.defaultManager createDirectoryAtPath:empty withIntermediateDirectories:YES attributes:nil error:nil]);
    prefs = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    [prefs resolveCloudCandidatesConsentWithPreferencesDirectory:empty userDataDirectory:nil];
    [controller setValue:prefs forKey:@"appearance"];
    [controller requestCloudCandidatesConsentIfNeeded];
    DrainMainQueue();
    assert(controller.prompts == 3);
    void (^accept)(NSNumber *) = controller.answer;
    accept(@YES);
    assert(prefs.cloudCandidatesEnabled && [[prefs sharedPreferencesByMerging:@{}][@"cloud_candidates"] isEqual:@YES]);
    // The native settings checkbox counts as an answer too.
    MSIMERemoveTestPreferenceSuite(defaults, suite);
    defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    prefs = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    [prefs resolveCloudCandidatesConsentWithPreferencesDirectory:empty userDataDirectory:nil];
    assert(!prefs.cloudCandidatesAnswered);
    NSButton *toggle = (id)PreferenceControl(prefs, @selector(cloudCandidatesChanged:));
    toggle.state = NSControlStateValueOff;
    [NSApp sendAction:toggle.action to:toggle.target from:toggle];
    assert(prefs.cloudCandidatesAnswered && !prefs.cloudCandidatesEnabled);
    [prefs.window close];
    MSIMERemoveTestPreferenceSuite(defaults, suite);

    // Upgrade with an existing preferences.json: answered, never asked, stored value kept (including false).
    assert([@"{}" writeToFile:[empty stringByAppendingPathComponent:@"preferences.json"] atomically:YES encoding:NSUTF8StringEncoding error:nil]);
    for (NSNumber *stored in @[@NO, @YES]) {
        suite = [@"msime.cloud.consent." stringByAppendingString:NSUUID.UUID.UUIDString];
        defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
        prefs = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
        [prefs applySharedInputPreferences:@{@"cloud_candidates":stored}];
        [prefs resolveCloudCandidatesConsentWithPreferencesDirectory:empty userDataDirectory:nil];
        assert(prefs.cloudCandidatesAnswered && prefs.cloudCandidates == stored.boolValue);
        assert(prefs.cloudCandidatesEnabled == stored.boolValue);
        [controller setValue:prefs forKey:@"appearance"];
        [controller requestCloudCandidatesConsentIfNeeded];
        DrainMainQueue();
        assert(controller.prompts == 3);
        MSIMERemoveTestPreferenceSuite(defaults, suite);
    }

    // Upgrade with an existing NSUserDefaults choice and no shared file: answered, value kept.
    NSString *bare = [NSTemporaryDirectory() stringByAppendingPathComponent:NSUUID.UUID.UUIDString];
    assert([NSFileManager.defaultManager createDirectoryAtPath:bare withIntermediateDirectories:YES attributes:nil error:nil]);
    suite = [@"msime.cloud.consent." stringByAppendingString:NSUUID.UUID.UUIDString];
    defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    [defaults setBool:NO forKey:@"MSIMEClientCloudCandidates"];
    prefs = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    [prefs resolveCloudCandidatesConsentWithPreferencesDirectory:bare userDataDirectory:nil];
    assert(prefs.cloudCandidatesAnswered && !prefs.cloudCandidates);
    MSIMERemoveTestPreferenceSuite(defaults, suite);

    // An empty Engine user-data directory is still a fresh profile.
    NSString *userData = [NSTemporaryDirectory() stringByAppendingPathComponent:NSUUID.UUID.UUIDString];
    assert([NSFileManager.defaultManager createDirectoryAtPath:userData withIntermediateDirectories:YES attributes:nil error:nil]);
    suite = [@"msime.cloud.consent." stringByAppendingString:NSUUID.UUID.UUIDString];
    defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    prefs = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    [prefs resolveCloudCandidatesConsentWithPreferencesDirectory:bare userDataDirectory:userData];
    assert(!prefs.cloudCandidatesAnswered && !prefs.cloudCandidatesEnabled);
    MSIMERemoveTestPreferenceSuite(defaults, suite);

    // Upgrade from a profile that typed but never changed a setting: no preferences.json and no stored choice, only Engine user data. Answered, never asked, default kept.
    assert([NSData.data writeToFile:[userData stringByAppendingPathComponent:@"msime_user.db"] atomically:YES]);
    suite = [@"msime.cloud.consent." stringByAppendingString:NSUUID.UUID.UUIDString];
    defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    prefs = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    [prefs resolveCloudCandidatesConsentWithPreferencesDirectory:bare userDataDirectory:userData];
    assert(prefs.cloudCandidatesAnswered && prefs.cloudCandidates && prefs.cloudCandidatesEnabled);
    [controller setValue:prefs forKey:@"appearance"];
    [controller requestCloudCandidatesConsentIfNeeded];
    DrainMainQueue();
    assert(controller.prompts == 3);
    MSIMERemoveTestPreferenceSuite(defaults, suite);

    [controller setValue:nil forKey:@"appearance"];
    for (NSString *path in @[root, empty, bare, userData]) assert([NSFileManager.defaultManager removeItemAtPath:path error:nil]);
}

@interface GlossSession : ShortcutSession
@property(nonatomic) NSUInteger applications;
@property(nonatomic) NSUInteger clears;
@property(nonatomic) BOOL enabled;
@property(nonatomic, copy) NSString *targetLanguage;
@property(nonatomic, copy) NSString *localMode;
@property(nonatomic) NSUInteger scheme;
@end
@implementation GlossSession
- (NSDictionary *)translationQueryWithError:(NSError **)error { (void)error; return self.enabled ? @{@"generation":@1, @"target_language":self.targetLanguage ?: @"en"} : nil; }
- (NSDictionary *)viewWithError:(NSError **)error { (void)error; return @{@"generation":@1, @"scheme":@(self.scheme), @"local_mode":self.localMode ?: @"none", @"candidates":@[@{@"text":@"hello", @"source":@4}]}; }
- (NSDictionary *)hostOptions { return @{@"resources":@"/synthetic"}; }
- (NSDictionary *)applyTranslations:(NSArray *)translations generation:(uint64_t)generation error:(NSError **)error {
    (void)error; assert(NSThread.isMainThread && generation == 1 && translations.count <= 1);
    if (translations.count) ++self.applications; else ++self.clears;
    return @{@"applied":@YES, @"view":[self viewWithError:nil]};
}
@end

@interface ServiceSnapshotSession : GlossSession
@property(nonatomic) NSUInteger translationQueryCalls;
@property(nonatomic) NSUInteger viewCalls;
@end
@implementation ServiceSnapshotSession
- (NSDictionary *)translationQueryWithError:(NSError **)error {
    ++self.translationQueryCalls;
    (void)error;
    return @{ @"generation": @1, @"target_language": @"en", @"target_languages": @[ @"en" ],
        @"translation_account": @NO };
}
- (NSDictionary *)viewWithError:(NSError **)error {
    ++self.viewCalls;
    return [super viewWithError:error];
}
- (NSDictionary *)hostOptions { return @{}; }
@end

@interface ApplySnapshotSession : ServiceSnapshotSession
@end
@implementation ApplySnapshotSession
- (NSDictionary *)applyTranslations:(NSArray *)translations generation:(uint64_t)generation error:(NSError **)error {
    (void)translations; (void)generation; (void)error;
    return @{ @"applied": @NO };
}
@end

@interface PreferenceSnapshotController : ModeController
@property(nonatomic, copy) NSDictionary *snapshot;
@property(nonatomic) NSUInteger completions;
@end
@implementation PreferenceSnapshotController
- (NSDictionary *)readPreferencesSnapshotInDirectory:(NSString *)directory error:(NSError **)error {
    (void)directory; (void)error;
    assert(!NSThread.isMainThread);
    return self.snapshot;
}
- (void)completePreferenceLoad:(NSDictionary *)snapshot error:(NSError *)error generation:(uint64_t)generation
                       session:(MSIMEClientSession *)session client:(id)client {
    assert(NSThread.isMainThread);
    [super completePreferenceLoad:snapshot error:error generation:generation session:session client:client];
    ++self.completions;
}
@end

static void TestPreferenceLoadReusesCandidateServiceSnapshots() {
    NSString *suite = [@"msime.preference-service-snapshot." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    MSIMEAppearancePreferences *appearance = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    appearance.candidateTranslations = YES;
    appearance.candidateEnglishGloss = YES;
    ServiceSnapshotSession *session = [ServiceSnapshotSession new];
    PreferenceSnapshotController *controller = [PreferenceSnapshotController alloc];
    controller.snapshot = @{ @"revision": @1, @"preferences": @{} };
    [controller setValue:appearance forKey:@"appearance"];
    [controller setValue:session forKey:@"session"];
    [controller setValue:[ShortcutClient new] forKey:@"activeClient"];
    [controller setValue:@"/synthetic-preferences" forKey:@"preferencesDirectory"];
    [controller reloadPreferences];
    NSDate *deadline = [NSDate dateWithTimeIntervalSinceNow:2];
    while (controller.completions < 1 && deadline.timeIntervalSinceNow > 0)
        [NSRunLoop.currentRunLoop runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.005]];
    assert(controller.completions == 1);
    assert(session.translationQueryCalls == 1 && session.viewCalls == 3);
    MSIMERemoveTestPreferenceSuite(defaults, suite);
}

static void TestCandidateServiceSnapshotsAreReused() {
    NSString *suite = [@"msime.service-snapshot." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    MSIMEAppearancePreferences *appearance = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    appearance.candidateTranslations = YES;
    appearance.candidateEnglishGloss = YES;
    MSIMEInputController *controller = [MSIMEInputController alloc];
    ServiceSnapshotSession *session = [ServiceSnapshotSession new];
    [controller setValue:appearance forKey:@"appearance"];
    [controller setValue:session forKey:@"session"];
    [controller setValue:[ShortcutClient new] forKey:@"activeClient"];
    [controller setValue:@NO forKey:@"glossEnabled"];
    [controller synchronizeCandidateServices];
    assert(session.translationQueryCalls == 1 && session.viewCalls == 1);
    MSIMERemoveTestPreferenceSuite(defaults, suite);
}

static void TestApplyCandidateTranslationSnapshotsAreReused() {
    NSString *suite = [@"msime.apply-service-snapshot." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    MSIMEAppearancePreferences *appearance = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    appearance.candidateTranslations = YES;
    appearance.candidateEnglishGloss = YES;
    ApplySnapshotSession *session = [ApplySnapshotSession new];
    MSIMEInputController *controller = [MSIMEInputController alloc];
    [controller setValue:appearance forKey:@"appearance"];
    [controller setValue:session forKey:@"session"];
    [controller setValue:[ShortcutClient new] forKey:@"activeClient"];
    [controller setValue:@NO forKey:@"glossEnabled"];
    NSDictionary *request = [controller currentGlossRequest];
    [controller setValue:request forKey:@"glossRequest"];
    [controller setValue:@[] forKey:@"glossResults"];
    session.translationQueryCalls = 0;
    session.viewCalls = 0;
    [controller applyCandidateTranslationResults];
    assert(session.translationQueryCalls == 1 && session.viewCalls == 1);
    MSIMERemoveTestPreferenceSuite(defaults, suite);
}

@interface GlossController : CloudShortcutController
@property(nonatomic, strong) dispatch_semaphore_t started;
@property(nonatomic, strong) dispatch_semaphore_t released;
@property(nonatomic) NSUInteger lookups;
@end
@implementation GlossController
- (NSDictionary *)readCandidateGloss:(NSDictionary *)request resources:(NSString *)resources {
    assert(!NSThread.isMainThread && [resources isEqual:@"/synthetic"]);
    ++self.lookups;
    dispatch_semaphore_signal(self.started);
    assert(dispatch_semaphore_wait(self.released, dispatch_time(DISPATCH_TIME_NOW, 5 * NSEC_PER_SEC)) == 0);
    return @{@"generation":request[@"generation"], @"translations":@[@{@"text":@"hello", @"translation":@"测试释义"}]};
}
@end
@interface CustomTranslationSession : GlossSession
@property(nonatomic, copy) NSDictionary *custom;
@property(nonatomic, copy) NSArray *queryCandidates;
@property(nonatomic) BOOL account;
@property(nonatomic, copy) NSDictionary *tencent;
@property(nonatomic, copy) NSDictionary *niuTrans;
@property(nonatomic, copy) NSArray *page;
@property(nonatomic, copy) NSArray *targetLanguages;
@property(nonatomic, copy) NSArray *offlineGlossLanguages;
@property(nonatomic, copy) NSArray *delivered;
@property(nonatomic) uint64_t generation;
@property(nonatomic) BOOL offline;
@end
@implementation CustomTranslationSession
- (NSDictionary *)translationQueryWithError:(NSError **)error {
    (void)error;
    return self.enabled ? @{@"generation":@(self.generation), @"target_language":self.targetLanguage ?: @"en",
        @"target_languages":self.targetLanguages ?: @[], @"offline_gloss_languages":self.offlineGlossLanguages ?: @[],
        @"custom_translation":self.custom ?: @{}, @"tencent_tmt":self.tencent ?: @{}, @"niutrans":self.niuTrans ?: @{},
        @"candidates":self.queryCandidates ?: @[], @"translation_account":@(self.account)} : nil;
}
- (NSDictionary *)viewWithError:(NSError **)error {
    (void)error;
    return @{@"generation":@(self.generation), @"scheme":@(self.scheme), @"local_mode":self.localMode ?: @"none", @"candidates":self.page ?: @[]};
}
- (NSDictionary *)hostOptions { return self.offline ? @{@"resources":@"/synthetic"} : @{}; }
- (NSDictionary *)applyTranslations:(NSArray *)translations generation:(uint64_t)generation error:(NSError **)error {
    (void)error;
    assert(NSThread.isMainThread && generation == self.generation);
    self.delivered = translations;
    return @{@"applied":@YES, @"view":[self viewWithError:nil]};
}
@end
@interface ControlledTranslationBatch : MSIMECustomTranslationBatch
@property(nonatomic, copy) void (^reply)(NSArray *);
@property(nonatomic, copy) NSArray *items;
@property(nonatomic, copy) NSDictionary *tencentConfig;
@property(nonatomic, copy) NSDictionary *niuTransConfig;
@property(nonatomic) BOOL started;
@property(nonatomic) BOOL cancelled;
@end
@implementation ControlledTranslationBatch
- (void)start { assert(!self.started); self.started = YES; }
- (void)cancel { self.cancelled = YES; }
@end
@interface AIShortcutSession : ShortcutSession
@property(nonatomic, copy) NSDictionary *query;
@property(nonatomic) NSUInteger applications;
@property(nonatomic) NSUInteger descriptorRequests;
@property(nonatomic) BOOL rejectNextAI;
@end
@implementation AIShortcutSession
- (NSDictionary *)onlineQueryWithError:(NSError **)error { (void)error; return self.query; }
// Turning both gloss switches off clears the translation column; the AI tests only need that call to be accepted.
- (NSDictionary *)applyTranslations:(NSArray *)translations generation:(uint64_t)generation error:(NSError **)error {
    (void)translations; (void)generation; (void)error;
    return @{ @"applied": @YES };
}
- (NSDictionary *)aiRequestForQuery:(NSDictionary *)query error:(NSError **)error {
    (void)error;
    assert([query isEqual:self.query]);
    ++self.descriptorRequests;
    if ([query[@"ai_assistant"][@"provider"] isEqual:@"invalid"]) return nil;
    NSString *context = [query[@"ai_context"] isKindOfClass:NSString.class] ? query[@"ai_context"] : @"";
    return @{ @"url": @"https://synthetic.invalid/chat", @"method": @"POST",
        @"headers": @{ @"Content-Type": @"application/json", @"Authorization": @"Bearer synthetic" },
        @"body": @{ @"messages": @[@{ @"role": @"system", @"content": @"synthetic" },
            @{ @"role": @"user", @"content": context }] }, @"timeout_ms": @8000,
        @"connect_timeout_ms": @2500, @"max_response_bytes": @1048576 };
}
- (NSDictionary *)applyOnlineCandidates:(NSArray<NSString *> *)candidates source:(uint8_t)source
                                  query:(NSDictionary *)query error:(NSError **)error {
    (void)error;
    assert(source == 1 && [query isEqual:self.query] && candidates.count == 1);
    if (self.rejectNextAI) {
        self.rejectNextAI = NO;
        return @{ @"applied": @NO, @"view": @{} };
    }
    ++self.applications;
    NSMutableDictionary *updated = [self.query mutableCopy];
    updated[@"generation"] = @([updated[@"generation"] unsignedLongLongValue] + 1);
    self.query = updated;
    return @{ @"applied": @YES, @"view": @{ @"focused": @YES, @"editing_text": @"synthetic",
        @"candidates": @[@{ @"text": @"合成候选", @"source": @1 }] } };
}
@end
@interface AIShortcutController : CloudShortcutController
@property(nonatomic, strong) NSMutableArray<ControlledTranslationBatch *> *aiBatches;
@end
@implementation AIShortcutController
- (MSIMECustomTranslationBatch *)aiBatchForItems:(NSArray<NSDictionary *> *)items
                                       completion:(void (^)(NSArray<NSDictionary *> *))completion {
    ControlledTranslationBatch *batch = [ControlledTranslationBatch new];
    batch.reply = completion;
    batch.items = items;
    [self.aiBatches addObject:batch];
    return batch;
}
@end
static void TestAiCandidateScheduling() {
    AIShortcutController *controller = [AIShortcutController alloc];
    controller.aiBatches = [NSMutableArray array];
    AIShortcutSession *session = [AIShortcutSession new];
    session.query = @{ @"scheme": @0, @"generation": @1, @"identity": @"synthetic-ai",
        @"query_text": @"nihao", @"cache_key": @"nihao", @"pinyin_segments": @[@"ni", @"hao"],
        @"ai_context": @"最近提交的上下文",
        @"cloud_eligible": @NO, @"ai_eligible": @YES, @"cloud_candidates": @YES,
        @"ai_assistant": @{ @"enabled": @YES, @"candidate_limit": @3 }, @"session_id": @1 };
    ShortcutClient *client = [ShortcutClient new];
    [controller setValue:session forKey:@"session"];
    [controller setValue:client forKey:@"activeClient"];
    [controller synchronizeAITranslations];
    NSTimer *timer = [controller valueForKey:@"aiTimer"];
    assert(timer);
    [timer fire];
    assert(session.descriptorRequests == 1 && controller.aiBatches.count == 1 && controller.aiBatches[0].started);
    NSDictionary *requestBody = controller.aiBatches[0].items[0][@"request"][@"body"];
    assert([requestBody[@"messages"][1][@"content"] containsString:@"最近提交的上下文"]);
    controller.aiBatches[0].reply(@[@{ @"translation": @"合成候选" }]);
    assert(session.applications == 1);
    NSDictionary *expectedAIQuery = @{ @"online": session.query, @"config": session.query[@"ai_assistant"] };
    assert([[controller valueForKey:@"aiQuery"] isEqual:expectedAIQuery]);
    [controller synchronizeAITranslations];
    assert(controller.aiBatches.count == 1);
    [controller cancelAITranslations];
}

// The source gates AI suggestions on ai_assistant.enabled alone (ai_eligible / UpdateAiInput); the gloss switch only governs glosses and translations, so turning it off must neither block nor cancel an AI request.
static void TestAiCandidatesIgnoreGlossSwitch() {
    NSString *suite = [@"msime.ai-gloss-off." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    MSIMEAppearancePreferences *appearance = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    appearance.candidateTranslations = NO;
    appearance.candidateEnglishGloss = NO;
    AIShortcutController *controller = [AIShortcutController alloc];
    controller.aiBatches = [NSMutableArray array];
    AIShortcutSession *session = [AIShortcutSession new];
    session.query = @{ @"scheme": @0, @"generation": @1, @"identity": @"synthetic-ai-gloss-off",
        @"query_text": @"nihao", @"cache_key": @"nihao-gloss-off", @"pinyin_segments": @[@"ni", @"hao"],
        @"ai_context": @"释义关闭", @"cloud_eligible": @NO, @"ai_eligible": @YES, @"cloud_candidates": @YES,
        @"ai_assistant": @{ @"enabled": @YES, @"candidate_limit": @3 }, @"session_id": @1 };
    [controller setValue:appearance forKey:@"appearance"];
    [controller setValue:session forKey:@"session"];
    [controller setValue:[ShortcutClient new] forKey:@"activeClient"];
    [controller synchronizeAITranslations];
    NSTimer *timer = [controller valueForKey:@"aiTimer"];
    assert(timer);
    [timer fire];
    assert(controller.aiBatches.count == 1 && controller.aiBatches[0].started);
    uint64_t epoch = [[controller valueForKey:@"aiEpoch"] unsignedLongLongValue];
    [controller appearanceChanged:nil];
    [controller applySharedToolbarPreferences:@{@"candidate_translations": @NO}];
    assert(!controller.aiBatches[0].cancelled);
    assert([[controller valueForKey:@"aiEpoch"] unsignedLongLongValue] == epoch);
    assert(controller.aiBatches.count == 1 && [controller valueForKey:@"aiQuery"]);
    // The AI assistant switch remains the one gate.
    NSMutableDictionary *disabled = [session.query mutableCopy];
    disabled[@"ai_assistant"] = @{ @"enabled": @NO, @"candidate_limit": @3 };
    session.query = disabled;
    [controller synchronizeAITranslations];
    assert(controller.aiBatches[0].cancelled && [controller valueForKey:@"aiQuery"] == nil);
    assert([[controller valueForKey:@"aiEpoch"] unsignedLongLongValue] > epoch);
    [controller cancelAITranslations];
    [defaults removePersistentDomainForName:suite];
}

static void TestAiCandidateRetryAfterRejectedResponse() {
    AIShortcutController *controller = [AIShortcutController alloc];
    controller.aiBatches = [NSMutableArray array];
    AIShortcutSession *session = [AIShortcutSession new];
    session.query = @{ @"scheme": @0, @"generation": @1, @"identity": @"synthetic-ai-retry",
        @"query_text": @"nihao", @"cache_key": @"nihao", @"pinyin_segments": @[@"ni", @"hao"],
        @"ai_context": @"重试上下文",
        @"cloud_eligible": @NO, @"ai_eligible": @YES, @"cloud_candidates": @YES,
        @"ai_assistant": @{ @"enabled": @YES, @"candidate_limit": @3 }, @"session_id": @1 };
    session.rejectNextAI = YES;
    [controller setValue:session forKey:@"session"];
    [controller setValue:[ShortcutClient new] forKey:@"activeClient"];
    [controller synchronizeAITranslations];
    NSTimer *timer = [controller valueForKey:@"aiTimer"];
    [timer fire];
    assert(controller.aiBatches.count == 1);
    controller.aiBatches[0].reply(@[@{ @"translation": @"被拒候选" }]);
    assert(session.applications == 0 && [controller valueForKey:@"aiQuery"] == nil);
    [controller synchronizeAITranslations];
    timer = [controller valueForKey:@"aiTimer"];
    assert(timer);
    [timer fire];
    assert(controller.aiBatches.count == 2);
    controller.aiBatches[1].reply(@[@{ @"translation": @"重试候选" }]);
    assert(session.applications == 1);
    [controller cancelAITranslations];
}

static void TestAiCandidateCacheAcrossGenerations() {
    AIShortcutController *controller = [AIShortcutController alloc];
    controller.aiBatches = [NSMutableArray array];
    AIShortcutSession *session = [AIShortcutSession new];
    session.query = @{ @"scheme": @0, @"generation": @1, @"identity": @"synthetic-ai-cache",
        @"query_text": @"nihao", @"cache_key": @"nihao", @"pinyin_segments": @[@"ni", @"hao"],
        @"ai_context": @"缓存上下文", @"cloud_eligible": @NO, @"ai_eligible": @YES,
        @"cloud_candidates": @YES, @"ai_assistant": @{ @"enabled": @YES, @"candidate_limit": @3 },
        @"session_id": @1 };
    [controller setValue:session forKey:@"session"];
    [controller setValue:[ShortcutClient new] forKey:@"activeClient"];
    [controller synchronizeAITranslations];
    NSTimer *timer = [controller valueForKey:@"aiTimer"];
    [timer fire];
    controller.aiBatches[0].reply(@[@{ @"translation": @"缓存候选" }]);
    assert(session.applications == 1 && controller.aiBatches.count == 1);
    NSMutableDictionary *next = [session.query mutableCopy];
    next[@"generation"] = @([next[@"generation"] unsignedLongLongValue] + 1);
    session.query = next;
    [controller setValue:@{ @"candidates": @[@{ @"text": @"普通候选", @"source": @0 }] } forKey:@"view"];
    [controller synchronizeAITranslations];
    assert(session.applications == 2 && session.descriptorRequests == 1 && controller.aiBatches.count == 1);
    [controller cancelAITranslations];
}

static void TestAiCandidateDescriptorFailureIsRetryable() {
    AIShortcutController *controller = [AIShortcutController alloc];
    controller.aiBatches = [NSMutableArray array];
    AIShortcutSession *session = [AIShortcutSession new];
    session.query = @{ @"scheme": @0, @"generation": @1, @"identity": @"synthetic-ai-descriptor",
        @"query_text": @"nihao", @"cache_key": @"nihao", @"pinyin_segments": @[@"ni", @"hao"],
        @"ai_context": @"descriptor retry", @"cloud_eligible": @NO, @"ai_eligible": @YES,
        @"cloud_candidates": @YES, @"ai_assistant": @{ @"enabled": @YES, @"provider": @"invalid",
            @"endpoint": @"file:///not-http", @"candidate_limit": @3 }, @"session_id": @1 };
    [controller setValue:session forKey:@"session"];
    [controller setValue:[ShortcutClient new] forKey:@"activeClient"];
    [controller synchronizeAITranslations];
    assert(controller.aiBatches.count == 0 && [controller valueForKey:@"aiQuery"] == nil);
    assert(session.descriptorRequests == 1);
    NSMutableDictionary *query = [session.query mutableCopy];
    query[@"ai_assistant"] = @{ @"enabled": @YES, @"provider": @"openai",
        @"endpoint": @"https://synthetic.invalid/chat", @"model": @"synthetic", @"candidate_limit": @3 };
    session.query = query;
    [controller synchronizeAITranslations];
    NSTimer *timer = [controller valueForKey:@"aiTimer"];
    assert(timer && session.descriptorRequests == 2);
    [timer fire];
    assert(controller.aiBatches.count == 1 && controller.aiBatches[0].started);
    [controller cancelAITranslations];
}
@interface CustomTranslationController : CloudShortcutController
@property(nonatomic, strong) NSMutableArray<ControlledTranslationBatch *> *batches;
@property(nonatomic, strong) NSMutableArray<NSArray *> *onDeviceFetches;
// Packaged English dictionary answers beyond Hello's.
@property(nonatomic, copy) NSDictionary<NSString *, NSString *> *extraEnglishGlosses;
@property(nonatomic) BOOL useRealDelay;
@end
@implementation CustomTranslationController
- (void)fetchOnDeviceGlosses:(NSArray<NSString *> *)words targets:(NSArray<NSString *> *)targets {
    assert(NSThread.isMainThread);
    [self.onDeviceFetches addObject:@[words, targets]];
}
- (MSIMECustomTranslationBatch *)niuTransBatchForItems:(NSArray<NSDictionary *> *)items config:(NSDictionary *)config
                                           completion:(void (^)(NSArray<NSDictionary *> *))completion {
    ControlledTranslationBatch *batch = (ControlledTranslationBatch *)[self customBatchForItems:items completion:completion];
    batch.niuTransConfig = config; return batch;
}
- (NSTimer *)customTranslationTimerWithBlock:(void (^)(NSTimer *))block {
    if (self.useRealDelay) return [super customTranslationTimerWithBlock:block];
    block(nil); return nil;
}
- (MSIMECustomTranslationBatch *)customBatchForItems:(NSArray<NSDictionary *> *)items completion:(void (^)(NSArray<NSDictionary *> *))completion {
    ControlledTranslationBatch *batch = [ControlledTranslationBatch new];
    batch.reply = completion;
    batch.items = items;
    [self.batches addObject:batch];
    return batch;
}
- (NSDictionary *)readCandidateGloss:(NSDictionary *)request resources:(NSString *)resources {
    assert(!NSThread.isMainThread && [resources isEqual:@"/synthetic"]);
    NSMutableArray *translations = [NSMutableArray arrayWithObject:@{@"text":@"Hello", @"translation":@"本地释义"}];
    for (NSString *text in self.extraEnglishGlosses) [translations addObject:@{@"text":text, @"translation":self.extraEnglishGlosses[text]}];
    return @{@"generation":request[@"generation"], @"translations":translations};
}
- (NSDictionary *)readTargetGloss:(NSDictionary *)request language:(NSString *)language resources:(NSString *)resources {
    assert(!NSThread.isMainThread && [resources isEqual:@"/synthetic"]);
    NSDictionary *glosses = @{@"fr":@{@"测试":@"essai"}, @"ja":@{@"测试":@"テスト", @"你好":@"こんにちは"}};
    NSMutableArray *translations = [NSMutableArray array];
    for (NSDictionary *candidate in request[@"candidates"])
        if (glosses[language][candidate[@"text"]])
            [translations addObject:@{@"text":candidate[@"text"], @"translation":glosses[language][candidate[@"text"]]}];
    return @{@"generation":request[@"generation"], @"translations":translations};
}
- (MSIMECustomTranslationBatch *)tencentBatchForItems:(NSArray<NSDictionary *> *)items config:(NSDictionary *)config
                                         completion:(void (^)(NSArray<NSDictionary *> *))completion {
    ControlledTranslationBatch *batch = (ControlledTranslationBatch *)[self customBatchForItems:items completion:completion];
    batch.tencentConfig = config;
    return batch;
}
@end
static NSDictionary *TencentConfig() {
    return @{@"enabled":@YES, @"secret_id":@"AKIDsynthetic", @"secret_key":@"synthetic", @"region":@"ap-guangzhou"};
}
static void WaitForGloss(CustomTranslationController *controller) {
    SettleWindowLayout();
    NSDate *deadline = [NSDate dateWithTimeIntervalSinceNow:3];
    while (![controller valueForKey:@"glossResults"] && deadline.timeIntervalSinceNow > 0)
        [NSRunLoop.mainRunLoop runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.005]];
    assert([controller valueForKey:@"glossResults"]);
}
static void TestLearnedGlossRuntime() {
    [[MSIMETranslationCache sharedCache] clear];
    NSString *root = [NSTemporaryDirectory() stringByAppendingPathComponent:NSUUID.UUID.UUIDString];
    // The glossary store writes into an existing preferences directory; it does not create one.
    assert([NSFileManager.defaultManager createDirectoryAtPath:root withIntermediateDirectories:YES attributes:nil error:nil]);
    CustomTranslationController *writer = [CustomTranslationController alloc]; writer.batches = [NSMutableArray array];
    CustomTranslationSession *session = [CustomTranslationSession new];
    session.enabled = YES; session.generation = 1; session.targetLanguage = @"en"; session.tencent = TencentConfig();
    // Only the English row is written to the glossary, so the request has to carry it: the fixture does not derive target_languages from targetLanguage the way the preference plan does.
    session.targetLanguages = @[@"en"];
    session.page = @[@{@"text":@"Hello", @"source":@4}, @{@"text":@"测试", @"source":@0}];
    [writer setValue:session forKey:@"session"]; [writer setValue:[ShortcutClient new] forKey:@"activeClient"];
    [writer setValue:root forKey:@"preferencesDirectory"];
    [writer synchronizeCandidateGloss]; WaitForGloss(writer);
    [writer synchronizeCustomTranslations];
    assert(writer.batches.count == 1);
    writer.batches[0].reply(@[@{@"text":@"Hello", @"translation":@"学习释义"}, @{@"text":@"测试", @"translation":@"test"}]);
    // Every successful English fetch is saved, as Windows PersistGloss does; nothing is committed here.
    dispatch_sync([MSIMEInputController learnedTranslationQueue], ^{});
    [writer cancelCandidateTranslations]; [[MSIMETranslationCache sharedCache] clear];
    // A new controller with both providers absent reuses the persisted glosses.
    CustomTranslationController *reader = [CustomTranslationController alloc]; reader.batches = [NSMutableArray array];
    session.tencent = nil; session.generation++;
    [reader setValue:session forKey:@"session"]; [reader setValue:[ShortcutClient new] forKey:@"activeClient"];
    [reader setValue:root forKey:@"preferencesDirectory"];
    [reader synchronizeCandidateGloss]; WaitForGloss(reader);
    assert(reader.batches.count == 0 && session.delivered.count == 2);
    assert([session.delivered[0][@"translation"] isEqual:@"学习释义"]);
    // Learned glossary entries override packaged values, matching Engine.
    [reader cancelCandidateTranslations]; session.offline = YES;
    [reader synchronizeCandidateGloss]; WaitForGloss(reader);
    assert(([session.delivered isEqual:@[@{@"text":@"Hello", @"translation":@"学习释义"}, @{@"text":@"测试", @"translation":@"test"}]]));
    [reader cancelCandidateTranslations]; session.offline = NO;
    [reader setValue:[root stringByAppendingPathComponent:@"another-profile"] forKey:@"preferencesDirectory"];
    [reader synchronizeCandidateGloss]; WaitForGloss(reader);
    assert(session.delivered.count == 0); // User directories cannot share learned words.
    [reader cancelCandidateTranslations]; [reader setValue:root forKey:@"preferencesDirectory"];
    session.targetLanguage = @"fr"; session.targetLanguages = @[@"fr"];
    assert(![reader currentGlossRequest]);
    // A stale online completion must not persist text, even if it is otherwise valid.
    session.targetLanguage = @"en"; session.targetLanguages = @[@"en"]; session.tencent = TencentConfig();
    session.page = @[@{@"text":@"stale", @"source":@4}];
    [writer synchronizeCandidateGloss]; WaitForGloss(writer); [writer synchronizeCustomTranslations];
    ControlledTranslationBatch *pending = writer.batches.lastObject;
    session.generation++;
    pending.reply(@[@{@"text":@"stale", @"translation":@"不应保存"}]);
    dispatch_sync([MSIMEInputController learnedTranslationQueue], ^{});
    [writer cancelCandidateTranslations];
    session.tencent = nil; session.delivered = @[];
    [reader synchronizeCandidateGloss]; WaitForGloss(reader);
    assert(session.delivered.count == 0);
    [reader cancelCandidateTranslations];
    // With French primary and English secondary, only the English row reaches the glossary.
    [[MSIMETranslationCache sharedCache] clear];
    session.targetLanguage = @"fr"; session.targetLanguages = @[@"fr", @"en"]; session.tencent = TencentConfig();
    session.page = @[@{@"text":@"世界", @"source":@0}]; session.generation++;
    [writer.batches removeAllObjects];
    [writer synchronizeCandidateGloss]; WaitForGloss(writer); [writer synchronizeCustomTranslations];
    assert(writer.batches.count == 2);
    // Reply to the English batch first so a saved French reply would overwrite it.
    writer.batches[1].reply(@[@{@"text":@"世界", @"translation":@"world"}]);
    writer.batches[0].reply(@[@{@"text":@"世界", @"translation":@"monde"}]);
    dispatch_sync([MSIMEInputController learnedTranslationQueue], ^{});
    [writer cancelCandidateTranslations]; [[MSIMETranslationCache sharedCache] clear];
    session.targetLanguage = @"en"; session.targetLanguages = @[@"en"]; session.tencent = nil; session.delivered = @[];
    [reader synchronizeCandidateGloss]; WaitForGloss(reader);
    assert(([session.delivered isEqual:@[@{@"text":@"世界", @"translation":@"world"}]]));
    [reader cancelCandidateTranslations];
    NSError *error = nil;
    assert([NSFileManager.defaultManager removeItemAtPath:root error:&error] && !error);
    [[MSIMETranslationCache sharedCache] clear];
}
static void TestNiuTransCandidateScheduling() {
    [[MSIMETranslationCache sharedCache] clear];
    CustomTranslationController *controller = [CustomTranslationController alloc]; controller.batches = [NSMutableArray array];
    CustomTranslationSession *session = [CustomTranslationSession new];
    session.enabled = YES; session.generation = 1; session.targetLanguage = @"fr";
    session.niuTrans = @{@"enabled":@YES, @"app_id":@"synthetic-app", @"apikey":@"synthetic-key"};
    session.custom = @{@"enabled":@YES, @"endpoint":@"https://synthetic.invalid", @"api_key":@""};
    session.tencent = TencentConfig();
    session.page = @[@{@"text":@"Hello", @"source":@4}, @{@"text":@"测试", @"source":@0}, @{@"text":@"smile", @"source":@6}];
    ShortcutClient *client = [ShortcutClient new];
    [controller setValue:session forKey:@"session"]; [controller setValue:client forKey:@"activeClient"];
    [controller applySharedToolbarPreferences:@{@"niutrans":session.niuTrans, @"custom_translation":session.custom, @"tencent_tmt":session.tencent}];
    [controller synchronizeCustomTranslations]; [controller synchronizeCustomTranslations];
    assert(controller.batches.count == 1);
    ControlledTranslationBatch *first = controller.batches[0];
    assert([first.niuTransConfig isEqual:session.niuTrans] && !first.tencentConfig && first.items.count == 2);
    assert([first.items[0][@"target_language"] isEqual:@"zh"] && [first.items[1][@"target_language"] isEqual:@"fr"]);
    first.reply(@[@{@"text":@"Hello", @"translation":@"合成释义"}]);
    session.generation++; [controller synchronizeCustomTranslations];
    assert(controller.batches.count == 1); // Positive and negative entries are reused within this provider.
    NSDictionary *updated = @{@"enabled":@YES, @"app_id":@"synthetic-app-2", @"apikey":@"synthetic-key-2"};
    [controller applySharedToolbarPreferences:@{@"niutrans":updated}];
    assert(![controller currentCustomTranslationRequest]); // Pending Engine snapshot must not use old credentials.
    session.niuTrans = updated; [controller synchronizeCustomTranslations];
    assert(controller.batches.count == 2 && controller.batches.lastObject.items.count == 2);
    ControlledTranslationBatch *pending = controller.batches.lastObject;
    NSDictionary *disabled = @{@"enabled":@NO, @"app_id":@"", @"apikey":@""};
    [controller applySharedToolbarPreferences:@{@"niutrans":disabled}];
    pending.reply(@[@{@"text":@"Hello", @"translation":@"stale"}]);
    assert(pending.cancelled && !session.delivered.count && ![controller currentCustomTranslationRequest]);
    session.niuTrans = disabled; [controller synchronizeCustomTranslations];
    assert(controller.batches.count == 3 && !controller.batches.lastObject.niuTransConfig); // Explicit opt-out selects custom.
    [controller cancelCustomTranslations];
    [controller applySharedToolbarPreferences:@{@"niutrans":updated}];
    assert(![controller currentCustomTranslationRequest]); // No fallback while NiuTrans enablement is pending.
    session.niuTrans = updated; [controller synchronizeCustomTranslations];
    pending = controller.batches.lastObject;
    [controller setValue:[ShortcutClient new] forKey:@"activeClient"];
    pending.reply(@[@{@"text":@"Hello", @"translation":@"stale focus"}]);
    assert(!session.delivered.count);
    [controller cancelCustomTranslations];
    [controller setValue:client forKey:@"activeClient"];
    session.localMode = @"temporary_japanese";
    assert(![controller currentCustomTranslationRequest]);
    [[MSIMETranslationCache sharedCache] clear];
}
static void TestTencentCandidateScheduling() {
    [[MSIMETranslationCache sharedCache] clear];
    CustomTranslationController *controller = [CustomTranslationController alloc];
    controller.batches = [NSMutableArray array];
    CustomTranslationSession *session = [CustomTranslationSession new];
    session.enabled = YES; session.generation = 1; session.targetLanguage = @"fr";
    session.tencent = TencentConfig();
    session.page = @[@{@"text":@"Hello", @"source":@4}, @{@"text":@"测试", @"source":@0}, @{@"text":@"smile", @"source":@6}];
    ShortcutClient *client = [ShortcutClient new];
    [controller setValue:session forKey:@"session"]; [controller setValue:client forKey:@"activeClient"];
    [controller applySharedToolbarPreferences:@{@"tencent_tmt":session.tencent}];
    [controller synchronizeCustomTranslations]; [controller synchronizeCustomTranslations];
    assert(controller.batches.count == 1 && controller.batches[0].started);
    ControlledTranslationBatch *first = controller.batches[0];
    assert([first.tencentConfig isEqual:session.tencent] && first.items.count == 2);
    assert(([first.items[0] isEqual:@{@"text":@"Hello", @"key":@"hello", @"source_language":@"en", @"target_language":@"zh"}]));
    assert([first.items[1][@"target_language"] isEqual:@"fr"]);
    NSArray *online = @[@{@"text":@"Hello", @"translation":@"你好"}];
    first.reply(online); assert([session.delivered isEqual:online]);
    session.generation++; [controller synchronizeCustomTranslations];
    assert(controller.batches.count == 1); // Positive and negative cache hits.
    // Custom remains authoritative, including a configured but invalid endpoint.
    session.custom = @{@"enabled":@YES, @"endpoint":@"", @"api_key":@""};
    [controller synchronizeCustomTranslations]; assert(controller.batches.count == 1);
    session.custom = @{@"enabled":@YES, @"endpoint":@"https://provider.invalid", @"api_key":@""};
    [controller synchronizeCustomTranslations]; assert(controller.batches.count == 2);
    assert(!controller.batches.lastObject.tencentConfig && controller.batches.lastObject.items.count == 2);
    controller.batches.lastObject.reply(@[@{@"text":@"Hello", @"translation":@"自定义"}]);
    session.custom = nil; [controller synchronizeCustomTranslations];
    assert(controller.batches.count == 2 && [session.delivered isEqual:online]); // Provider cache isolation.
    for (NSString *field in @[@"secret_id", @"secret_key", @"region"]) {
        NSMutableDictionary *edited = [session.tencent mutableCopy];
        edited[field] = [field isEqual:@"region"] ? @"ap-shanghai" : @"syntheticReplacement";
        [controller applySharedToolbarPreferences:@{@"tencent_tmt":edited}];
        assert(![controller currentCustomTranslationRequest] && session.delivered.count == 0);
        session.tencent = edited;
        NSUInteger before = controller.batches.count;
        [controller synchronizeCustomTranslations];
        assert(controller.batches.count == before + 1 && controller.batches.lastObject.items.count == 2);
        controller.batches.lastObject.reply(online);
    }
    for (NSString *change in @[@"generation", @"page", @"client", @"session", @"focus", @"japanese", @"disabled"]) {
        [[MSIMETranslationCache sharedCache] clear];
        [controller cancelCandidateTranslations]; session.delivered = @[];
        [controller synchronizeCustomTranslations];
        ControlledTranslationBatch *pending = controller.batches.lastObject;
        NSArray *page = session.page;
        if ([change isEqual:@"generation"]) session.generation++;
        if ([change isEqual:@"page"]) session.page = @[];
        if ([change isEqual:@"client"]) [controller setValue:[ShortcutClient new] forKey:@"activeClient"];
        if ([change isEqual:@"session"]) [controller setValue:[CustomTranslationSession new] forKey:@"session"];
        if ([change isEqual:@"focus"]) [controller setValue:@YES forKey:@"focusPending"];
        if ([change isEqual:@"japanese"]) session.localMode = @"temporary_japanese";
        if ([change isEqual:@"disabled"]) session.enabled = NO;
        pending.reply(online); assert(session.delivered.count == 0);
        [controller cancelCandidateTranslations]; assert(pending.cancelled);
        session.page = page; session.localMode = nil; session.enabled = YES;
        [controller setValue:session forKey:@"session"]; [controller setValue:client forKey:@"activeClient"];
        [controller setValue:@NO forKey:@"focusPending"];
    }
    session.targetLanguage = @"en"; session.offline = YES;
    [controller synchronizeCandidateGloss]; [controller synchronizeCustomTranslations];
    assert(![controller currentCustomTranslationRequest]);
    NSUInteger before = controller.batches.count;
    SettleWindowLayout();
    NSDate *deadline = [NSDate dateWithTimeIntervalSinceNow:2];
    while (controller.batches.count == before && deadline.timeIntervalSinceNow > 0)
        [NSRunLoop.mainRunLoop runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.005]];
    assert(controller.batches.count == before + 1);
    ControlledTranslationBatch *fallback = controller.batches.lastObject;
    assert(fallback.tencentConfig && fallback.items.count == 1 && [fallback.items[0][@"text"] isEqual:@"测试"]);
    fallback.reply(@[@{@"text":@"测试", @"translation":@"test"}]);
    assert(([session.delivered isEqual:@[@{@"text":@"Hello", @"translation":@"本地释义"}, @{@"text":@"测试", @"translation":@"test"}]]));
    [controller cancelCandidateTranslations]; [[MSIMETranslationCache sharedCache] clear];
    session.offline = NO;
    [controller synchronizeCandidateGloss];
    [controller synchronizeCustomTranslations];
    ControlledTranslationBatch *pending = controller.batches.lastObject;
    assert(pending != fallback && pending.started);
    NSMutableDictionary *disabled = [session.tencent mutableCopy]; disabled[@"enabled"] = @NO;
    [controller applySharedToolbarPreferences:@{@"tencent_tmt":disabled}];
    assert(pending.cancelled && ![controller currentCustomTranslationRequest] && session.delivered.count == 0);
    pending.reply(online); assert(session.delivered.count == 0);
    session.tencent = disabled; assert(![controller currentCustomTranslationRequest]);
    session.tencent = TencentConfig();
    [controller applySharedToolbarPreferences:@{@"tencent_tmt":session.tencent}];
    [controller synchronizeCandidateGloss];
    [controller synchronizeCustomTranslations]; pending = controller.batches.lastObject;
    // Pending custom enablement must block Tencent even before the query updates.
    NSDictionary *custom = @{@"enabled":@YES, @"endpoint":@"https://provider.invalid", @"api_key":@""};
    [controller applySharedToolbarPreferences:@{@"custom_translation":custom}];
    assert(pending.cancelled && ![controller currentCustomTranslationRequest]);
    pending.reply(online); assert(session.delivered.count == 0);
    [controller cancelCandidateTranslations]; [[MSIMETranslationCache sharedCache] clear];
}
@interface AccountGlossSession : ShortcutSession
@property(nonatomic, copy) NSArray *candidates;
@property(nonatomic, copy) NSArray *applied;
@property(nonatomic, assign) NSUInteger generation;
// Replaces the service choice the shared query reports; nil means the user explicitly chose the account.
@property(nonatomic, copy) NSDictionary *choice;
@end
@implementation AccountGlossSession
- (NSDictionary *)translationQueryWithError:(NSError **)error {
    (void)error;
    // The account endpoint needs an explicit choice: the shared query reports translation_account only when the user selected it and no service of their own takes precedence.
    NSMutableDictionary *query = [@{@"generation":@(self.generation ?: 1), @"target_languages":@[@"en"], @"candidates":self.candidates ?: @[]} mutableCopy];
    [query addEntriesFromDictionary:self.choice ?: @{@"translation_account":@YES, @"provider":@"none"}];
    return query;
}
- (NSDictionary *)viewWithError:(NSError **)error {
    (void)error; return @{@"generation":@(self.generation ?: 1), @"scheme":@0, @"local_mode":@"none", @"candidates":@[]};
}
// Reached once a gloss arrives and the controller pushes the merged results back into the view.
- (NSDictionary *)applyTranslations:(NSArray *)translations generation:(uint64_t)generation error:(NSError **)error {
    (void)error; (void)generation;
    self.applied = translations;
    return @{@"applied":@YES, @"view":[self viewWithError:nil]};
}
@end

// Only Chinese candidates reach the account gloss endpoint. The shared query answers this per candidate;
// the controller must honour that answer rather than sending the whole page. Asking about "cun", "123",
// "OpenAI", a punctuation candidate or an emoji spends the account's bounded quota to put noise under
// candidates that should carry no gloss, and a pinyin buffer candidate hands raw keystrokes to a service.
// A gloss fetched through one controller has to be visible to the next one. IMKit builds a controller per
// text input client, so the instance that fetched is rarely the instance composing a moment later; held per
// instance, the answers pile up in controllers that stopped composing and every new text field starts from
// nothing - which is the "it appears the second time, not the first" symptom, the second time happening to
// reuse the same instance.
static void TestAccountGlossCacheIsSharedAcrossControllers() {
    NSString *suite = [@"msime.account-gloss-shared." stringByAppendingString:NSUUID.UUID.UUIDString];
    MSIMEAppearancePreferences *prefs =
        [[MSIMEAppearancePreferences alloc] initWithDefaults:[[NSUserDefaults alloc] initWithSuiteName:suite]];
    NSArray *candidates = @[@{@"text":@"\u6d4b\u8bd5", @"online_gloss":@YES}];

    CustomTranslationController *fetcher = [CustomTranslationController alloc];
    fetcher.batches = [NSMutableArray array];
    AccountGlossSession *fetcherSession = [AccountGlossSession new];
    fetcherSession.candidates = candidates;
    [fetcher setValue:[ShortcutClient new] forKey:@"activeClient"];
    [fetcher setValue:prefs forKey:@"appearance"];
    [fetcher setValue:fetcherSession forKey:@"session"];
    [fetcher synchronizeAccountGloss:[fetcher currentAccountGlossRequest]];

    // The reply carries the same payload the Swift backend posts. Delivered straight to the instance that
    // asked, because these controllers are built without the activation that registers the observer.
    [fetcher accountCandidateTranslationsDidArrive:
        [NSNotification notificationWithName:@"MSIMEBackendCandidateTranslationsDidArrive" object:nil
            userInfo:@{@"generation":@1, @"translations":@{@"\u6d4b\u8bd5":@"test"}}]];
    assert([[[fetcher valueForKey:@"accountGlossResults"] valueForKey:@"translation"] containsObject:@"test"]);

    // A different controller, as a different text field would get, and it must not have to ask again.
    CustomTranslationController *reader = [CustomTranslationController alloc];
    reader.batches = [NSMutableArray array];
    AccountGlossSession *readerSession = [AccountGlossSession new];
    readerSession.candidates = candidates;
    [reader setValue:[ShortcutClient new] forKey:@"activeClient"];
    [reader setValue:prefs forKey:@"appearance"];
    [reader setValue:readerSession forKey:@"session"];
    NSDictionary *request = [reader currentAccountGlossRequest];
    assert(request);
    NSArray *results = [reader accountGlossResultsForRequest:request];
    assert(results.count == 1 && [results[0][@"translation"] isEqual:@"test"]);

    // A controller that has stopped composing must stop answering the broadcast. The reply reaches every
    // instance, and IMKit keeps one per text input client, so a request left behind after the client went
    // away means a dozen of them merging results into sessions nobody is typing in.
    assert([fetcher valueForKey:@"accountGlossRequest"] != nil);
    [fetcher cancelCandidateTranslations];
    assert([fetcher valueForKey:@"accountGlossRequest"] == nil);
    fetcherSession.applied = nil;
    [fetcher accountCandidateTranslationsDidArrive:
        [NSNotification notificationWithName:@"MSIMEBackendCandidateTranslationsDidArrive" object:nil
            userInfo:@{@"generation":@1, @"translations":@{@"\u6d4b\u8bd5":@"ignored"}}]];
    assert(fetcherSession.applied == nil);

    // A response for the previous candidate generation must not be accepted by a new request,
    // even when the text itself is unchanged. The generation is the request identity here.
    [[MSIMETranslationCache sharedCache] clear];
    fetcherSession.generation = 2;
    [fetcher synchronizeAccountGloss:[fetcher currentAccountGlossRequest]];
    fetcherSession.applied = nil;
    [fetcher accountCandidateTranslationsDidArrive:
        [NSNotification notificationWithName:@"MSIMEBackendCandidateTranslationsDidArrive" object:nil
            userInfo:@{@"generation":@1, @"translations":@{@"\u6d4b\u8bd5":@"stale"}}]];
    assert(fetcherSession.applied == nil);
    [fetcher accountCandidateTranslationsDidArrive:
        [NSNotification notificationWithName:@"MSIMEBackendCandidateTranslationsDidArrive" object:nil
            userInfo:@{@"generation":@2, @"translations":@{@"\u6d4b\u8bd5":@"fresh"}}]];
    assert([[[fetcher valueForKey:@"accountGlossResults"] firstObject][@"translation"] isEqual:@"fresh"]);
}

static void TestAccountGlossSkipsNonChineseCandidates() {
    NSString *suite = [@"msime.account-gloss." stringByAppendingString:NSUUID.UUID.UUIDString];
    MSIMEAppearancePreferences *prefs =
        [[MSIMEAppearancePreferences alloc] initWithDefaults:[[NSUserDefaults alloc] initWithSuiteName:suite]];
    CustomTranslationController *controller = [CustomTranslationController alloc];
    controller.batches = [NSMutableArray array];
    AccountGlossSession *session = [AccountGlossSession new];
    [controller setValue:[ShortcutClient new] forKey:@"activeClient"];
    [controller setValue:prefs forKey:@"appearance"];
    [controller setValue:session forKey:@"session"];

    session.candidates = @[@{@"text":@"\u6d4b\u8bd5", @"online_gloss":@YES},
                           @{@"text":@"cun", @"online_gloss":@NO},
                           @{@"text":@"123", @"online_gloss":@NO},
                           @{@"text":@"OpenAI", @"online_gloss":@NO},
                           @{@"text":@"\U0001F600", @"online_gloss":@NO},
                           @{@"text":@"\u4f60\u597d", @"online_gloss":@YES}];
    NSDictionary *request = [controller currentAccountGlossRequest];
    assert([[request[@"candidates"] valueForKey:@"text"] isEqual:(@[@"\u6d4b\u8bd5", @"\u4f60\u597d"])]);

    // A page with nothing to ask about produces no request at all, rather than an empty one that would
    // still cost a round trip and a signature.
    session.candidates = @[@{@"text":@"cun", @"online_gloss":@NO}, @{@"text":@"OpenAI", @"online_gloss":@NO}];
    assert(![controller currentAccountGlossRequest]);

    // A candidate the shared layer never answered for is not sent either: absent is not permission.
    session.candidates = @[@{@"text":@"\u6d4b\u8bd5"}];
    assert(![controller currentAccountGlossRequest]);
}

// Nothing reaches api.msime.app unless the user chose the MSIME account. A query without the flag, one that says no, and one whose service is the user's own (even with no usable credentials in it) all leave the account path idle: no request is formed and none is remembered.
static void TestAccountGlossRequiresExplicitChoice() {
    NSString *suite = [@"msime.account-gloss-choice." stringByAppendingString:NSUUID.UUID.UUIDString];
    MSIMEAppearancePreferences *prefs =
        [[MSIMEAppearancePreferences alloc] initWithDefaults:[[NSUserDefaults alloc] initWithSuiteName:suite]];
    CustomTranslationController *controller = [CustomTranslationController alloc];
    controller.batches = [NSMutableArray array];
    AccountGlossSession *session = [AccountGlossSession new];
    session.candidates = @[@{@"text":@"\u6d4b\u8bd5", @"online_gloss":@YES}];
    [controller setValue:[ShortcutClient new] forKey:@"activeClient"];
    [controller setValue:prefs forKey:@"appearance"];
    [controller setValue:session forKey:@"session"];
    for (NSDictionary *choice in @[@{},
                                   @{@"translation_account":@NO, @"provider":@"none"},
                                   @{@"provider":@"tencent", @"tencent_tmt":NSNull.null},
                                   @{@"provider":@"niutrans", @"niutrans":NSNull.null}]) {
        session.choice = choice;
        assert(![controller currentAccountGlossRequest]);
        [controller synchronizeAccountGloss:[controller currentAccountGlossRequest]];
        assert([controller valueForKey:@"accountGlossRequest"] == nil);
    }
    // The same page with the explicit choice does form a request, so the cases above fail for the reason they name.
    session.choice = nil;
    assert([controller currentAccountGlossRequest]);
}

static void TestOfflineTargetGlosses() {
    [[MSIMETranslationCache sharedCache] clear];
    CustomTranslationController *controller = [CustomTranslationController alloc];
    controller.batches = [NSMutableArray array];
    CustomTranslationSession *session = [CustomTranslationSession new];
    session.enabled = YES; session.generation = 1; session.offline = YES;
    session.targetLanguage = @"en"; session.targetLanguages = @[@"en", @"fr"]; session.offlineGlossLanguages = @[@"fr", @"ja"];
    session.page = @[@{@"text":@"Hello", @"source":@4}, @{@"text":@"测试", @"source":@0}, @{@"text":@"你好", @"source":@0}];
    [controller setValue:session forKey:@"session"];
    [controller setValue:[ShortcutClient new] forKey:@"activeClient"];
    void (^settle)(void) = ^{
        [controller synchronizeCandidateGloss];
        [controller synchronizeTargetGloss];
        NSOperationQueue *glossQueue = [controller valueForKey:@"glossQueue"];
        NSOperationQueue *targetGlossQueue = [controller valueForKey:@"targetGlossQueue"];
        [glossQueue waitUntilAllOperationsAreFinished];
        [targetGlossQueue waitUntilAllOperationsAreFinished];
        // Each worker ends by dispatching its apply to the main queue, so once both queues are
        // finished both of those blocks are already sitting in it, and a block enqueued now is
        // behind them: when this one runs, theirs have run. That is the whole condition, and it
        // needs no guess about how many applies a generation produces — waiting for a single
        // apply passed unloaded and failed under the sanitizer, where the English gloss landed
        // while the offline target one was still pending and the first assertion compared
        // against half a delivery. The deadline is only here so a genuine hang fails as a test
        // rather than as a timeout.
        __block BOOL drained = NO;
        dispatch_async(dispatch_get_main_queue(), ^{ drained = YES; });
        SettleWindowLayout();
        NSDate *deadline = [NSDate dateWithTimeIntervalSinceNow:5.0];
        while (!drained && deadline.timeIntervalSinceNow > 0)
            [NSRunLoop.mainRunLoop runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.005]];
        assert(drained);
    };
    // Only the selected targets are read: ja is installed but not chosen. Rows follow the target order, and a candidate the English dictionary cannot answer keeps an empty first row.
    settle();
    assert(([[controller currentTargetGlossRequest][@"offline_languages"] isEqual:@[@"fr"]]));
    assert(([session.delivered isEqual:@[@{@"text":@"Hello", @"translation":@"本地释义"}, @{@"text":@"测试", @"translation":@"\nessai"}]]));
    session.generation++; session.targetLanguage = @"ja"; session.targetLanguages = @[@"ja", @"en"];
    settle();
    assert(([session.delivered isEqual:@[@{@"text":@"Hello", @"translation":@"\n本地释义"}, @{@"text":@"测试", @"translation":@"テスト"},
        @{@"text":@"你好", @"translation":@"こんにちは"}]]));
    // Without English among the targets the offline dictionary is the only local source.
    session.generation++; session.targetLanguage = @"fr"; session.targetLanguages = @[@"fr"];
    settle();
    assert(([session.delivered isEqual:@[@{@"text":@"测试", @"translation":@"essai"}]]));
    // Nothing installed for the chosen targets leaves the English path exactly as it was.
    session.generation++; session.targetLanguage = @"en"; session.targetLanguages = @[@"en", @"de"];
    settle();
    assert(![controller currentTargetGlossRequest] && ([session.delivered isEqual:@[@{@"text":@"Hello", @"translation":@"本地释义"}]]));
    // The user's own translator still takes precedence over the offline dictionary.
    session.generation++; session.targetLanguage = @"fr"; session.targetLanguages = @[@"fr"];
    session.custom = @{@"enabled":@YES, @"endpoint":@"https://offline-gloss.invalid/api", @"api_key":@""};
    settle();
    [controller synchronizeCustomTranslations];
    assert(controller.batches.count == 1);
    controller.batches[0].reply(@[@{@"text":@"测试", @"translation":@"test en ligne"}]);
    assert(([session.delivered isEqual:@[@{@"text":@"测试", @"translation":@"test en ligne"}]]));
    [controller cancelCandidateTranslations];
    assert(![controller valueForKey:@"targetGlossRequest"] && ![controller valueForKey:@"targetGlossResults"]);
    [[MSIMETranslationCache sharedCache] clear];
}
// Apple's on-device translation fills only what the offline dictionaries leave empty, and only for a user without a service of their own. Replies are delivered straight to the controller, as in the account tests, because these controllers are built without the activation that registers the observer.
static void TestOnDeviceGlosses() {
    [[MSIMETranslationCache sharedCache] clear];
    NSString *suite = [@"msime.on-device-gloss." stringByAppendingString:NSUUID.UUID.UUIDString];
    MSIMEAppearancePreferences *prefs =
        [[MSIMEAppearancePreferences alloc] initWithDefaults:[[NSUserDefaults alloc] initWithSuiteName:suite]];
    CustomTranslationController *controller = [CustomTranslationController alloc];
    controller.batches = [NSMutableArray array];
    controller.onDeviceFetches = [NSMutableArray array];
    CustomTranslationSession *session = [CustomTranslationSession new];
    session.enabled = YES; session.generation = 1; session.offline = YES;
    session.targetLanguage = @"fr"; session.targetLanguages = @[@"fr"]; session.offlineGlossLanguages = @[@"fr"];
    session.page = @[@{@"text":@"Hello", @"source":@4}, @{@"text":@"测试", @"source":@0}, @{@"text":@"你好", @"source":@0}];
    session.queryCandidates = @[@{@"text":@"Hello", @"online_gloss":@NO}, @{@"text":@"测试", @"online_gloss":@YES},
                                @{@"text":@"你好", @"online_gloss":@YES}];
    [controller setValue:session forKey:@"session"];
    [controller setValue:[ShortcutClient new] forKey:@"activeClient"];
    [controller setValue:prefs forKey:@"appearance"];
    void (^settle)(void) = ^{
        [controller synchronizeCandidateGloss];
        [controller synchronizeTargetGloss];
        [controller synchronizeOnDeviceGloss];
        [(NSOperationQueue *)[controller valueForKey:@"glossQueue"] waitUntilAllOperationsAreFinished];
        [(NSOperationQueue *)[controller valueForKey:@"targetGlossQueue"] waitUntilAllOperationsAreFinished];
        [NSRunLoop.mainRunLoop runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.2]];
    };
    void (^reply)(NSString *, NSDictionary *) = ^(NSString *target, NSDictionary *translations) {
        [controller onDeviceCandidateTranslationsDidArrive:[NSNotification notificationWithName:@"MSIMEBackendOnDeviceTranslationsDidArrive"
            object:nil userInfo:@{@"target":target, @"translations":translations}]];
    };
    // Only Chinese candidates are asked about, and the dictionary keeps the rows it answered.
    settle();
    assert(controller.onDeviceFetches.count == 1 && ([controller.onDeviceFetches[0] isEqual:@[@[@"测试", @"你好"], @[@"fr"]]]));
    assert(([session.delivered isEqual:@[@{@"text":@"测试", @"translation":@"essai"}]]));
    reply(@"fr", @{@"测试":@"tester", @"你好":@"bonjour"});
    assert(([session.delivered isEqual:@[@{@"text":@"测试", @"translation":@"essai"}, @{@"text":@"你好", @"translation":@"bonjour"}]]));
    // A word already answered, even with nothing useful, is not asked about again.
    session.generation++; session.page = @[@{@"text":@"你好", @"source":@0}, @{@"text":@"世界", @"source":@0}];
    session.queryCandidates = @[@{@"text":@"你好", @"online_gloss":@YES}, @{@"text":@"世界", @"online_gloss":@YES}];
    settle();
    assert(controller.onDeviceFetches.count == 2 && ([controller.onDeviceFetches[1] isEqual:@[@[@"世界"], @[@"fr"]]]));
    reply(@"fr", @{@"世界":@""});
    session.generation++;
    settle();
    assert(controller.onDeviceFetches.count == 2 && ([session.delivered isEqual:@[@{@"text":@"你好", @"translation":@"bonjour"}]]));
    // Without a dictionary for the target it is the only offline source, and rows follow the target order beside the English dictionary.
    session.generation++; session.targetLanguage = @"en"; session.targetLanguages = @[@"en", @"de"]; session.offlineGlossLanguages = @[];
    session.page = @[@{@"text":@"Hello", @"source":@4}, @{@"text":@"测试", @"source":@0}];
    session.queryCandidates = @[@{@"text":@"Hello", @"online_gloss":@NO}, @{@"text":@"测试", @"online_gloss":@YES}];
    // Nothing is asked before the English dictionary has answered; its completion asks.
    [controller synchronizeCandidateGloss];
    [controller synchronizeOnDeviceGloss];
    assert(controller.onDeviceFetches.count == 2);
    settle();
    // Each target is asked for its own words, so one target's gap does not send the word through the other's line too.
    assert(controller.onDeviceFetches.count == 4 && ([controller.onDeviceFetches[2] isEqual:@[@[@"测试"], @[@"en"]]]) &&
           ([controller.onDeviceFetches[3] isEqual:@[@[@"测试"], @[@"de"]]]));
    reply(@"de", @{@"测试":@"Test"});
    assert(([session.delivered isEqual:@[@{@"text":@"Hello", @"translation":@"本地释义"}, @{@"text":@"测试", @"translation":@"\nTest"}]]));
    // A Chinese word the English dictionary answers keeps its place in the page but is not sent to the model for English, which spends half a second on every word it is given.
    controller.extraEnglishGlosses = @{@"你好":@"hello"};
    session.generation++; session.targetLanguages = @[@"en"];
    session.page = @[@{@"text":@"Hello", @"source":@4}, @{@"text":@"你好", @"source":@0}, @{@"text":@"再见", @"source":@0}];
    session.queryCandidates = @[@{@"text":@"Hello", @"online_gloss":@NO}, @{@"text":@"你好", @"online_gloss":@YES},
                                @{@"text":@"再见", @"online_gloss":@YES}];
    settle();
    assert(controller.onDeviceFetches.count == 5 && ([controller.onDeviceFetches[4] isEqual:@[@[@"再见"], @[@"en"]]]));
    reply(@"en", @{@"再见":@"goodbye"});
    assert(([session.delivered isEqual:@[@{@"text":@"Hello", @"translation":@"本地释义"}, @{@"text":@"你好", @"translation":@"hello"},
                                         @{@"text":@"再见", @"translation":@"goodbye"}]]));
    controller.extraEnglishGlosses = nil;
    session.targetLanguages = @[@"en", @"de"];
    // A service of the user's own, or the MSIME account, answers every candidate; this path stays idle for both.
    session.generation++; session.custom = @{@"enabled":@YES, @"endpoint":@"https://on-device.invalid/api", @"api_key":@""};
    assert(![controller currentOnDeviceGlossRequest]);
    session.custom = nil; session.account = YES;
    assert(![controller currentOnDeviceGlossRequest]);
    session.account = NO;
    assert([controller currentOnDeviceGlossRequest]);
    // Switching candidate translation off stops it too.
    [prefs setValue:@NO forKey:@"candidateTranslations"];
    assert(![controller currentOnDeviceGlossRequest]);
    [prefs setValue:@YES forKey:@"candidateTranslations"];
    // A controller that stopped composing ignores the broadcast.
    settle();
    [controller cancelCandidateTranslations];
    assert(![controller valueForKey:@"onDeviceGlossRequest"]);
    session.delivered = nil;
    reply(@"de", @{@"测试":@"ignoriert"});
    assert(session.delivered == nil);
    [[NSUserDefaults new] removePersistentDomainForName:suite];
    [[MSIMETranslationCache sharedCache] clear];
}
// An English on-device gloss is saved to the user glossary when it arrives, as Windows saves a fetched English gloss (cloud_translation.cpp PersistGloss), so the next page and the next launch read it from the learned overlay instead of waiting on the model again.
static void TestOnDeviceGlossPersistence() {
    [[MSIMETranslationCache sharedCache] clear];
    NSString *root = [NSTemporaryDirectory() stringByAppendingPathComponent:NSUUID.UUID.UUIDString];
    // The glossary store writes into an existing preferences directory; it does not create one.
    assert([NSFileManager.defaultManager createDirectoryAtPath:root withIntermediateDirectories:YES attributes:nil error:nil]);
    NSString *suite = [@"msime.on-device-gloss-persist." stringByAppendingString:NSUUID.UUID.UUIDString];
    MSIMEAppearancePreferences *prefs =
        [[MSIMEAppearancePreferences alloc] initWithDefaults:[[NSUserDefaults alloc] initWithSuiteName:suite]];
    CustomTranslationSession *session = [CustomTranslationSession new];
    session.enabled = YES; session.generation = 1;
    session.targetLanguage = @"en"; session.targetLanguages = @[@"en", @"de"];
    // 世界 is on the page with a source, so the English gloss request built off the view carries it and would save it, but the shared query does not offer it to the on-device model.
    session.page = @[@{@"text":@"测试", @"source":@0}, @{@"text":@"再见", @"source":@0}, @{@"text":@"世界", @"source":@0}];
    session.queryCandidates = @[@{@"text":@"测试", @"online_gloss":@YES}, @{@"text":@"再见", @"online_gloss":@YES},
                                @{@"text":@"世界", @"online_gloss":@NO}];
    CustomTranslationController *(^attach)(void) = ^{
        CustomTranslationController *controller = [CustomTranslationController alloc];
        controller.batches = [NSMutableArray array];
        controller.onDeviceFetches = [NSMutableArray array];
        [controller setValue:session forKey:@"session"];
        [controller setValue:[ShortcutClient new] forKey:@"activeClient"];
        [controller setValue:prefs forKey:@"appearance"];
        [controller setValue:root forKey:@"preferencesDirectory"];
        return controller;
    };
    void (^settle)(CustomTranslationController *) = ^(CustomTranslationController *controller) {
        [controller synchronizeCandidateGloss];
        [controller synchronizeOnDeviceGloss];
        [(NSOperationQueue *)[controller valueForKey:@"glossQueue"] waitUntilAllOperationsAreFinished];
        [NSRunLoop.mainRunLoop runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.2]];
    };
    void (^reply)(CustomTranslationController *, NSString *, NSDictionary *) =
        ^(CustomTranslationController *controller, NSString *target, NSDictionary *translations) {
        [controller onDeviceCandidateTranslationsDidArrive:[NSNotification notificationWithName:@"MSIMEBackendOnDeviceTranslationsDidArrive"
            object:nil userInfo:@{@"target":target, @"translations":translations}]];
    };
    NSDictionary *(^lookup)(void) = ^{
        __block NSDictionary *found;
        dispatch_sync([MSIMEInputController learnedTranslationQueue], ^{
            found = [MSIMEClientSession learnedTranslationRequest:@{@"directory":root, @"action":@"lookup", @"target_language":@"en",
                @"generation":@1, @"items":@[@{@"text":@"测试", @"direction":@"chinese_to_english"},
                                               @{@"text":@"再见", @"direction":@"chinese_to_english"},
                                               @{@"text":@"世界", @"direction":@"chinese_to_english"},
                                               @{@"text":@"你好", @"direction":@"chinese_to_english"}]} error:nil];
        });
        return found;
    };
    CustomTranslationController *writer = attach();
    settle(writer);
    assert(writer.onDeviceFetches.count == 2);
    // The user types on before the model answers, as a fast typist always does: the replies below land after the page they were asked for has gone, and are saved all the same, as Windows saves every fetch that completes.
    NSArray *askedPage = session.page, *askedQuery = session.queryCandidates;
    session.generation++;
    session.page = @[@{@"text":@"你好", @"source":@0}];
    session.queryCandidates = @[@{@"text":@"你好", @"online_gloss":@YES}];
    settle(writer);
    // English first, so a German reply that reached the glossary would overwrite it.
    reply(writer, @"en", @{@"测试":@"test"});
    reply(writer, @"de", @{@"测试":@"Prüfung"});
    // An empty answer is not a gloss and stays in the process cache only. The glossary store rejects it too, so this records the outcome rather than isolating the length check in persistOnDeviceGlosses.
    reply(writer, @"en", @{@"再见":@""});
    // Every controller hears every reply, so a word the on-device request did not ask about is not saved, even though the English gloss request for this page carries it with a source.
    reply(writer, @"en", @{@"世界":@"world"});
    // Nor does the page the user typed on to have to be composing still: a candidate committed before its gloss arrived is the usual case, and its gloss is saved too.
    [writer cancelCandidateTranslations];
    reply(writer, @"en", @{@"你好":@"hello"});
    dispatch_sync([MSIMEInputController learnedTranslationQueue], ^{});
    // Saved under the Chinese key in the Chinese-to-English direction; the lookup only finds it there.
    assert(([lookup()[@"translations"] isEqual:@[@{@"text":@"测试", @"translation":@"test"}, @{@"text":@"你好", @"translation":@"hello"}]]));
    // A fresh controller with an empty process cache, as after a restart, answers the saved word from the glossary and only asks the model about the rest.
    [writer cancelCandidateTranslations]; [[MSIMETranslationCache sharedCache] clear];
    session.page = askedPage; session.queryCandidates = askedQuery;
    session.generation++; session.targetLanguages = @[@"en"]; session.delivered = nil;
    CustomTranslationController *reader = attach();
    settle(reader);
    assert(reader.onDeviceFetches.count == 1 && ([reader.onDeviceFetches[0] isEqual:@[@[@"再见"], @[@"en"]]]));
    assert(([session.delivered isEqual:@[@{@"text":@"测试", @"translation":@"test"}]]));
    [reader cancelCandidateTranslations];
    [[NSUserDefaults new] removePersistentDomainForName:suite];
    NSError *error = nil;
    assert([NSFileManager.defaultManager removeItemAtPath:root error:&error] && !error);
    [[MSIMETranslationCache sharedCache] clear];
}
static void TestCustomTranslationController() {
    CustomTranslationController *controller = [CustomTranslationController alloc];
    controller.batches = [NSMutableArray array];
    CustomTranslationSession *session = [CustomTranslationSession new];
    session.enabled = YES; session.generation = 1; session.targetLanguage = @"fr";
    session.custom = @{@"enabled":@YES, @"endpoint":@"https://translation.invalid/api", @"api_key":@""};
    session.page = @[@{@"text":@"Hello", @"source":@4}, @{@"text":@"测试", @"source":@0}, @{@"text":@"smile", @"source":@6}];
    ShortcutClient *client = [ShortcutClient new];
    [controller setValue:client forKey:@"activeClient"];
    [controller setValue:session forKey:@"session"];
    [controller synchronizeCandidateGloss];
    [controller synchronizeCustomTranslations];
    [controller synchronizeCustomTranslations];
    assert(controller.batches.count == 1 && controller.batches[0].started);
    ControlledTranslationBatch *first = controller.batches[0];
    assert(first.items.count == 2);
    assert(([first.items[0][@"request"][@"body"] isEqual:@{@"text":@"hello", @"source_lang":@"EN", @"target_lang":@"ZH"}]));
    assert(([first.items[1][@"request"][@"body"] isEqual:@{@"text":@"测试", @"source_lang":@"ZH", @"target_lang":@"FR"}]));
    NSArray *online = @[@{@"text":@"Hello", @"translation":@"你好"}, @{@"text":@"测试", @"translation":@"essai"}];
    first.reply(online);
    assert([session.delivered isEqual:online] && controller.batches.count == 1);
    // A pending target change cancels work before Engine's applied snapshot changes.
    [controller applySharedToolbarPreferences:@{@"translation_target_language":@"de"}];
    assert(session.delivered.count == 0 && ![controller currentCustomTranslationRequest]);
    first.reply(online);
    assert(session.delivered.count == 0);
    session.targetLanguage = @"de";
    [controller synchronizeCustomTranslations];
    assert(controller.batches.count == 2);
    ControlledTranslationBatch *second = controller.batches.lastObject;
    assert([second.items[1][@"request"][@"body"][@"target_lang"] isEqual:@"DE"]);
    // Candidate identity, generation, client, session, focus and mode guards all
    // reject a callback even before the next synchronization cancels transport.
    for (NSString *change in @[@"generation", @"page", @"client", @"session", @"focus", @"japanese", @"disabled"]) {
        [controller cancelCandidateTranslations];
        [controller synchronizeCustomTranslations];
        ControlledTranslationBatch *batch = controller.batches.lastObject;
        NSArray *page = session.page;
        if ([change isEqual:@"generation"]) session.generation++;
        if ([change isEqual:@"page"]) session.page = @[@{@"text":@"different", @"source":@0}];
        if ([change isEqual:@"client"]) [controller setValue:[ShortcutClient new] forKey:@"activeClient"];
        if ([change isEqual:@"session"]) [controller setValue:[CustomTranslationSession new] forKey:@"session"];
        if ([change isEqual:@"focus"]) [controller setValue:@YES forKey:@"focusPending"];
        if ([change isEqual:@"japanese"]) session.localMode = @"temporary_japanese";
        if ([change isEqual:@"disabled"]) session.enabled = NO;
        batch.reply(online);
        assert(session.delivered.count == 0);
        [controller cancelCandidateTranslations];
        assert(batch.cancelled);
        session.page = page; session.localMode = nil; session.enabled = YES;
        [controller setValue:session forKey:@"session"];
        [controller setValue:client forKey:@"activeClient"];
        [controller setValue:@NO forKey:@"focusPending"];
    }
    [controller synchronizeCustomTranslations];
    ControlledTranslationBatch *pending = controller.batches.lastObject;
    NSDictionary *disabled = @{@"enabled":@NO, @"endpoint":@"https://translation.invalid/api", @"api_key":@""};
    [controller applySharedToolbarPreferences:@{@"custom_translation":disabled}];
    assert(pending.cancelled && ![controller currentCustomTranslationRequest]);
    pending.reply(online);
    assert(session.delivered.count == 0);
    [controller applySharedToolbarPreferences:@{@"custom_translation":session.custom, @"translation_target_language":@"en"}];
    session.targetLanguage = @"en"; session.offline = YES;
    [controller synchronizeCandidateGloss];
    [controller synchronizeCustomTranslations];
    assert(![controller currentCustomTranslationRequest]); // Wait for offline lookup.
    SettleWindowLayout();
    NSDate *deadline = [NSDate dateWithTimeIntervalSinceNow:2];
    NSUInteger previous = controller.batches.count;
    while (controller.batches.count == previous && deadline.timeIntervalSinceNow > 0)
        [NSRunLoop.mainRunLoop runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.005]];
    assert(controller.batches.count == previous + 1);
    ControlledTranslationBatch *fallback = controller.batches.lastObject;
    assert(fallback.items.count == 1 && [fallback.items[0][@"text"] isEqual:@"测试"]);
    fallback.reply(@[@{@"text":@"测试", @"translation":@"test"}]);
    assert(([session.delivered isEqual:@[@{@"text":@"Hello", @"translation":@"本地释义"}, @{@"text":@"测试", @"translation":@"test"}]]));
    [controller synchronizeCandidateGloss]; [controller synchronizeCustomTranslations];
    assert(controller.batches.lastObject == fallback);
    [controller cancelCandidateTranslations];
    // The native toggle is authoritative even before persistence/notification.
    session.offline = NO;
    [controller synchronizeCandidateGloss]; [controller synchronizeCustomTranslations];
    ControlledTranslationBatch *nativePending = controller.batches.lastObject;
    NSString *suite = [@"msime.custom.translation." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    MSIMEAppearancePreferences *prefs = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    [controller setValue:prefs forKey:@"appearance"];
    prefs.candidateTranslations = NO;
    assert(![controller currentCustomTranslationRequest]);
    [controller appearanceChanged:nil];
    assert(nativePending.cancelled && session.delivered.count == 0);
    nativePending.reply(online);
    assert(session.delivered.count == 0);
    [prefs.window close];
    MSIMERemoveTestPreferenceSuite(defaults, suite);
}
static void TestSecondaryTranslationScheduling() {
    [[MSIMETranslationCache sharedCache] clear];
    CustomTranslationController *controller = [CustomTranslationController alloc];
    controller.batches = [NSMutableArray array];
    CustomTranslationSession *session = [CustomTranslationSession new];
    session.enabled = YES; session.generation = 1; session.targetLanguage = @"fr";
    session.targetLanguages = @[@"fr", @"ja"];
    session.custom = @{@"enabled":@YES, @"endpoint":@"https://secondary.invalid/api", @"api_key":@""};
    // A Chinese candidate is the one the target language applies to. An English candidate is always glossed into Chinese no matter which languages are selected, so it cannot tell the two rows apart.
    session.page = @[@{@"text":@"你好", @"source":@0}];
    [controller setValue:session forKey:@"session"];
    [controller setValue:[ShortcutClient new] forKey:@"activeClient"];
    [controller synchronizeCustomTranslations];
    assert(controller.batches.count == 2 && controller.batches[0].items.count == 1 && controller.batches[1].items.count == 1);
    assert([controller.batches[0].items[0][@"request"][@"body"][@"target_lang"] isEqual:@"FR"]);
    assert([controller.batches[1].items[0][@"request"][@"body"][@"target_lang"] isEqual:@"JA"]);
    controller.batches[0].reply(@[@{@"text":@"你好", @"translation":@"bonjour"}]);
    controller.batches[1].reply(@[@{@"text":@"你好", @"translation":@"こんにちは"}]);
    assert(([session.delivered isEqual:@[@{@"text":@"你好", @"translation":@"bonjour\nこんにちは"}]]));
    session.generation++;
    [controller synchronizeCustomTranslations];
    assert(controller.batches.count == 2 && [session.delivered[0][@"translation"] isEqual:@"bonjour\nこんにちは"]);
    session.generation++;
    session.targetLanguage = @"ja";
    session.targetLanguages = @[@"ja", @"fr"];
    [controller synchronizeCustomTranslations];
    assert(controller.batches.count == 2 && [session.delivered[0][@"translation"] isEqual:@"こんにちは\nbonjour"]);
    [[MSIMETranslationCache sharedCache] clear];
    session.generation++;
    session.targetLanguage = @"fr";
    session.targetLanguages = @[@"fr", @"ja"];
    [controller synchronizeCustomTranslations];
    assert(controller.batches.count == 4);
    ControlledTranslationBatch *stale = controller.batches.lastObject;
    session.generation++;
    stale.reply(@[@{@"text":@"你好", @"translation":@"stale"}]);
    assert(![session.delivered[0][@"translation"] isEqual:@"stale"]);
    [controller cancelCandidateTranslations];
    [[MSIMETranslationCache sharedCache] clear];
}
static void TestCustomTranslationCacheDelivery() {
    [[MSIMETranslationCache sharedCache] clear];
    CustomTranslationController *controller = [CustomTranslationController alloc];
    controller.batches = [NSMutableArray array];
    CustomTranslationSession *session = [CustomTranslationSession new];
    session.enabled = YES; session.generation = 1; session.targetLanguage = @"fr";
    session.custom = @{@"enabled":@YES, @"endpoint":@"https://cache.invalid/api", @"api_key":@""};
    session.page = @[@{@"text":@"Hello", @"source":@4}];
    [controller setValue:session forKey:@"session"];
    [controller setValue:[ShortcutClient new] forKey:@"activeClient"];
    [controller applySharedToolbarPreferences:@{@"custom_translation":session.custom}];
    [controller synchronizeCustomTranslations];
    assert(controller.batches.count == 1);
    controller.batches[0].reply(@[@{@"text":@"Hello", @"translation":@"你好"}]);
    [controller cancelCandidateTranslations]; session.generation++;
    session.page = @[@{@"text":@"HELLO", @"source":@4}];
    [controller synchronizeCustomTranslations];
    assert(controller.batches.count == 1);
    assert(([session.delivered isEqual:@[@{@"text":@"HELLO", @"translation":@"你好"}]]));
    session.generation++; session.page = @[@{@"text":@"HELLO", @"source":@4}, @{@"text":@"missing", @"source":@4}];
    [controller synchronizeCustomTranslations]; assert(controller.batches.count == 2);
    assert(controller.batches[1].items.count == 1 && [controller.batches[1].items[0][@"text"] isEqual:@"missing"]);
    controller.batches[1].reply(@[]);
    assert(([session.delivered isEqual:@[@{@"text":@"HELLO", @"translation":@"你好"}]]));
    session.generation++; [controller synchronizeCustomTranslations];
    assert(controller.batches.count == 2);
    // A credential edit invalidates failure suppression without putting a key
    // in the cache identity; a subsequent generation may retry immediately.
    session.custom = @{@"enabled":@YES, @"endpoint":@"https://cache.invalid/api", @"api_key":@"synthetic"};
    [controller applySharedToolbarPreferences:@{@"custom_translation":session.custom}];
    [controller synchronizeCustomTranslations]; assert(controller.batches.count == 3);
    controller.batches[2].reply(@[@{@"text":@"missing", @"translation":@"找到"}]);
    session.targetLanguage = @"de"; session.generation++;
    [controller synchronizeCustomTranslations]; assert(controller.batches.count == 4);
    [controller cancelCandidateTranslations];
    [[MSIMETranslationCache sharedCache] clear];
}
static void TestCustomTranslationIdleDelay(BOOL tencent) {
    [[MSIMETranslationCache sharedCache] clear];
    CustomTranslationController *controller = [CustomTranslationController alloc];
    controller.useRealDelay = YES; controller.batches = [NSMutableArray array];
    CustomTranslationSession *session = [CustomTranslationSession new];
    session.enabled = YES; session.generation = 1; session.targetLanguage = @"fr";
    session.custom = @{@"enabled":@YES, @"endpoint":@"https://idle.invalid/api", @"api_key":@""};
    if (tencent) { session.custom = nil; session.tencent = TencentConfig(); }
    session.page = @[@{@"text":@"hello", @"source":@4}];
    [controller setValue:session forKey:@"session"];
    [controller setValue:[ShortcutClient new] forKey:@"activeClient"];
    [controller synchronizeCustomTranslations];
    NSTimer *first = [controller valueForKey:@"customTimer"];
    // The upper bound is the contract: a request is scheduled rather than sent, and no later than the
    // configured idle delay. The lower bound cannot be the same number - the clock runs between scheduling
    // and asserting, and on a loaded machine it runs far enough to fail a test about batching for a reason
    // that has nothing to do with batching. What has to hold is that the timer has not fired yet.
    assert(first.valid && first.fireDate.timeIntervalSinceNow > 0 && first.fireDate.timeIntervalSinceNow <= 0.5);
    [controller synchronizeCustomTranslations];
    assert(first == [controller valueForKey:@"customTimer"]);
    // Do not advance time at all. What has to hold is that scheduling a request does not send one, and
    // running the loop toward the edge of the delay only gave the timer a chance to fire and made the
    // assertion depend on how loaded the machine was - which is what it then had to be weakened for.
    assert(controller.batches.count == 0);
    session.generation++; session.page = @[@{@"text":@"newest", @"source":@4}];
    [controller synchronizeCustomTranslations];
    NSTimer *second = [controller valueForKey:@"customTimer"];
    assert(!first.valid && second.valid && second != first);
    [first fire]; assert(controller.batches.count == 0);
    [controller setValue:@YES forKey:@"focusPending"];
    [second fire];
    assert(controller.batches.count == 0 && ![controller valueForKey:@"customTimer"]);
    [controller cancelCandidateTranslations];
    [controller setValue:@NO forKey:@"focusPending"];
    [controller synchronizeCustomTranslations];
    SettleWindowLayout();
    NSDate *deadline = [NSDate dateWithTimeIntervalSinceNow:2];
    while (!controller.batches.count && deadline.timeIntervalSinceNow > 0)
        [NSRunLoop.mainRunLoop runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.005]];
    assert(controller.batches.count == 1 && [controller.batches[0].items[0][@"text"] isEqual:@"newest"]);
    controller.batches[0].reply(@[@{@"text":@"newest", @"translation":@"最新"}]);
    session.generation++;
    [controller synchronizeCustomTranslations];
    assert(![controller valueForKey:@"customTimer"] && controller.batches.count == 1);
    assert(session.delivered.count == 1); // Cache hits never incur the delay.
    session.generation++; session.page = @[@{@"text":@"cancelled", @"source":@4}];
    [controller synchronizeCustomTranslations];
    NSTimer *cancelled = [controller valueForKey:@"customTimer"];
    [controller cancelCandidateTranslations]; [cancelled fire];
    assert(!cancelled.valid && controller.batches.count == 1);
    [[MSIMETranslationCache sharedCache] clear];
}
static void TestGlossScheduling() {
    GlossController *controller = [GlossController alloc];
    controller.started = dispatch_semaphore_create(0);
    controller.released = dispatch_semaphore_create(0);
    GlossSession *session = [GlossSession new]; session.enabled = YES;
    ShortcutClient *a = [ShortcutClient new], *b = [ShortcutClient new];
    [controller setValue:session forKey:@"session"];
    [controller setValue:a forKey:@"activeClient"];
    [controller synchronizeCandidateGloss];
    assert(dispatch_semaphore_wait(controller.started, dispatch_time(DISPATCH_TIME_NOW, 5 * NSEC_PER_SEC)) == 0);
    [controller synchronizeCandidateGloss]; // Identical view must not duplicate work.
    [controller cancelCandidateGloss];
    [controller setValue:b forKey:@"activeClient"];
    [controller synchronizeCandidateGloss];
    dispatch_semaphore_signal(controller.released);
    assert(dispatch_semaphore_wait(controller.started, dispatch_time(DISPATCH_TIME_NOW, 5 * NSEC_PER_SEC)) == 0);
    [NSRunLoop.mainRunLoop runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.02]];
    assert(session.applications == 0);
    dispatch_semaphore_signal(controller.released);
    SettleWindowLayout();
    NSDate *deadline = [NSDate dateWithTimeIntervalSinceNow:2];
    while (session.applications == 0 && deadline.timeIntervalSinceNow > 0)
        [NSRunLoop.mainRunLoop runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.005]];
    assert(session.applications == 1 && controller.lookups == 2);
    [controller synchronizeCandidateGloss];
    assert(controller.lookups == 2);
    [controller cancelCandidateGloss];
}

@interface SettingsRouteWorkspace : NSWorkspace
@property BOOL installed;
@property(strong) NSWorkspaceOpenConfiguration *configuration;
@property(copy) void (^completion)(NSRunningApplication *, NSError *);
@end
@implementation SettingsRouteWorkspace
- (NSURL *)URLForApplicationWithBundleIdentifier:(NSString *)identifier {
    assert([identifier isEqual:@"app.msime.macos"]);
    return self.installed ? [NSURL fileURLWithPath:@"/synthetic/Settings.app"] : nil;
}
- (void)openApplicationAtURL:(NSURL *)url configuration:(NSWorkspaceOpenConfiguration *)configuration
          completionHandler:(void (^)(NSRunningApplication *, NSError *))completion {
    assert([url.path isEqual:@"/synthetic/Settings.app"]);
    self.configuration = configuration; self.completion = completion;
}
@end
@interface RoutedAppearancePreferences : MSIMEAppearancePreferences
@property(strong) SettingsRouteWorkspace *testWorkspace;
@end
@implementation RoutedAppearancePreferences
- (NSWorkspace *)desktopSettingsWorkspace { return self.testWorkspace; }
@end

static void TestCandidateTranslationPreference() {
    NSString *suite = [@"msime.gloss.preference." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    RoutedAppearancePreferences *prefs = [[RoutedAppearancePreferences alloc] initWithDefaults:defaults];
    prefs.testWorkspace = [SettingsRouteWorkspace new];
    NSButton *toggle = (id)PreferenceControl(prefs, @selector(candidateTranslationsChanged:));
    NSButton *offlineToggle = (id)PreferenceControl(prefs, @selector(candidateEnglishGlossChanged:));
    assert(prefs.candidateTranslations && toggle.state == NSControlStateValueOn);
    assert(!prefs.candidateEnglishGloss && offlineToggle.state == NSControlStateValueOff);
    assert(![prefs sharedPreferencesByMerging:@{}][@"candidate_translations"]);
    assert(![prefs sharedPreferencesByMerging:@{}][@"candidate_english_gloss"]);
    assert([[prefs sharedPreferencesByMerging:@{@"candidate_translations":@NO}][@"candidate_translations"] isEqual:@NO]);
    __block NSUInteger saves = 0;
    id observer = [NSNotificationCenter.defaultCenter addObserverForName:MSIMEAppearanceDidChangeNotification object:prefs queue:nil usingBlock:^(NSNotification *note) { (void)note; ++saves; }];
    [prefs applySharedInputPreferences:@{@"candidate_translations":@NO}];
    assert(!prefs.candidateTranslations && toggle.state == NSControlStateValueOff && saves == 0);
    for (id invalid in @[NSNull.null, @1, @"true"]) [prefs applySharedInputPreferences:@{@"candidate_translations":invalid}];
    assert(!prefs.candidateTranslations && saves == 0 && ![defaults objectForKey:@"MSIMEClientCandidateTranslations"]);
    toggle.state = NSControlStateValueOn;
    [NSApp sendAction:toggle.action to:toggle.target from:toggle];
    assert(prefs.candidateTranslations && saves == 1);
    [prefs applySharedInputPreferences:@{@"candidate_english_gloss":@YES}];
    assert(prefs.candidateEnglishGloss && offlineToggle.state == NSControlStateValueOn && saves == 1);
    for (id invalid in @[NSNull.null, @1, @"true"]) [prefs applySharedInputPreferences:@{@"candidate_english_gloss":invalid}];
    assert(prefs.candidateEnglishGloss && saves == 1 && ![defaults objectForKey:@"MSIMEClientCandidateEnglishGloss"]);
    offlineToggle.state = NSControlStateValueOff;
    [NSApp sendAction:offlineToggle.action to:offlineToggle.target from:offlineToggle];
    assert(!prefs.candidateEnglishGloss && saves == 2);
    offlineToggle.state = NSControlStateValueOn;
    [NSApp sendAction:offlineToggle.action to:offlineToggle.target from:offlineToggle];
    assert(prefs.candidateEnglishGloss && saves == 3);
    GlossController *controller = [GlossController alloc];
    controller.started = dispatch_semaphore_create(0);
    controller.released = dispatch_semaphore_create(0);
    GlossSession *session = [GlossSession new]; session.enabled = YES;
    [controller setValue:prefs forKey:@"appearance"];
    [controller setValue:session forKey:@"session"];
    [controller setValue:[ShortcutClient new] forKey:@"activeClient"];
    [controller synchronizeCandidateGloss];
    assert(dispatch_semaphore_wait(controller.started, dispatch_time(DISPATCH_TIME_NOW, 5 * NSEC_PER_SEC)) == 0);
    toggle.state = NSControlStateValueOff;
    [NSApp sendAction:toggle.action to:toggle.target from:toggle];
    assert([controller currentGlossRequest] && saves == 4);
    [controller appearanceChanged:nil];
    assert(session.clears == 0);
    offlineToggle.state = NSControlStateValueOff;
    [NSApp sendAction:offlineToggle.action to:offlineToggle.target from:offlineToggle];
    [controller appearanceChanged:nil];
    assert(![controller currentGlossRequest] && session.clears == 1 && saves == 5);
    dispatch_semaphore_signal(controller.released);
    [(NSOperationQueue *)[controller valueForKey:@"glossQueue"] waitUntilAllOperationsAreFinished];
    [NSRunLoop.mainRunLoop runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.02]];
    assert(session.applications == 0);
    assert(![[[MSIMEAppearancePreferences alloc] initWithDefaults:defaults] candidateTranslations]);
    assert(![[[MSIMEAppearancePreferences alloc] initWithDefaults:defaults] candidateEnglishGloss]);
    NSString *root = [NSTemporaryDirectory() stringByAppendingPathComponent:NSUUID.UUID.UUIDString];
    NSError *error = nil;
    NSDictionary *snapshot = [MSIMEClientSession loadPreferencesInDirectory:root error:&error];
    assert(snapshot && !error);
    NSDictionary *merged = [prefs sharedPreferencesByMerging:snapshot[@"preferences"]];
    assert([merged[@"candidate_translations"] isEqual:@NO]);
    assert([merged[@"candidate_english_gloss"] isEqual:@NO]);
    assert([merged[@"custom_translation"] isEqual:snapshot[@"preferences"][@"custom_translation"]]);
    NSDictionary *saved = [MSIMEClientSession savePreferencesInDirectory:root expectedRevision:[snapshot[@"revision"] unsignedLongLongValue]
        snapshot:@{@"format_version":@1, @"revision":snapshot[@"revision"], @"preferences":merged} error:&error];
    assert(saved && !error);
    NSDictionary *loaded = [MSIMEClientSession loadPreferencesInDirectory:root error:&error];
    assert(loaded && !error && [loaded[@"preferences"][@"candidate_translations"] isEqual:@NO]);
    [prefs setTranslationPreferencesDirectory:root];
    NSControl *translationEntry = PreferenceControl(prefs, @selector(showTranslationSettings:));
    assert(translationEntry);
    prefs.testWorkspace.installed = YES;
    [NSApp sendAction:translationEntry.action to:translationEntry.target from:translationEntry];
    assert([prefs.testWorkspace.configuration.arguments isEqual:@[@"--route=settings:expression"]]);
    prefs.testWorkspace.completion(NSRunningApplication.currentApplication, nil);
    assert(![prefs valueForKey:@"translationWindow"]);
    NSControl *aiEntry = PreferenceControl(prefs, @selector(showAISettings:));
    [NSApp sendAction:aiEntry.action to:aiEntry.target from:aiEntry];
    assert([prefs.testWorkspace.configuration.arguments isEqual:@[@"--route=settings:ai"]]);
    prefs.testWorkspace.completion(NSRunningApplication.currentApplication, nil);
    assert(![prefs valueForKey:@"aiWindow"]);
    NSControl *skinEntry = PreferenceControl(prefs, @selector(showSkinCatalog:));
    [NSApp sendAction:skinEntry.action to:skinEntry.target from:skinEntry];
    assert([prefs.testWorkspace.configuration.arguments isEqual:@[@"--route=settings:skin"]]);
    prefs.testWorkspace.completion(NSRunningApplication.currentApplication, nil);
    // The native fallback is the 皮肤 page rather than a window of its own, so a launch that
    // succeeded must leave the window on the page it was already showing.
    assert([[prefs valueForKey:@"selectedPageIndex"] integerValue] == 0);
    // A failed asynchronous launch still reaches the existing native editor.
    [NSApp sendAction:aiEntry.action to:aiEntry.target from:aiEntry];
    prefs.testWorkspace.completion(nil, [NSError errorWithDomain:@"SyntheticLaunchFailure" code:1 userInfo:nil]);
    SettleWindowLayout();
    NSDate *launchDeadline = [NSDate dateWithTimeIntervalSinceNow:2];
    while (![prefs valueForKey:@"aiWindow"] && launchDeadline.timeIntervalSinceNow > 0)
        [NSRunLoop.mainRunLoop runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.005]];
    NSWindowController *aiWindow = [prefs valueForKey:@"aiWindow"];
    assert(aiWindow.window.visible); [aiWindow close];
    prefs.testWorkspace.installed = NO;
    [NSApp sendAction:translationEntry.action to:translationEntry.target from:translationEntry];
    NSWindowController *translationWindow = [prefs valueForKey:@"translationWindow"];
    assert(translationWindow.window.visible);
    [translationWindow close];
    [NSNotificationCenter.defaultCenter removeObserver:observer];
    MSIMERemoveTestPreferenceSuite(defaults, suite);
    MSIMEAppearancePreferences *fresh = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    assert(fresh.candidateTranslations);
    [fresh applySharedInputPreferences:loaded[@"preferences"]];
    assert(!fresh.candidateTranslations);
    [prefs.window close];
    [controller cancelCandidateGloss];
    assert([NSFileManager.defaultManager removeItemAtPath:root error:&error] && !error);
}

static void TestGlossModePolicy() {
    GlossController *controller = [GlossController alloc];
    controller.started = dispatch_semaphore_create(0);
    controller.released = dispatch_semaphore_create(0);
    GlossSession *session = [GlossSession new]; session.enabled = YES;
    [controller setValue:session forKey:@"session"];
    [controller setValue:[ShortcutClient new] forKey:@"activeClient"];
    assert([controller currentGlossRequest]);
    for (NSString *target in @[@"fr", @"ja", @"es", @"ru", @"de", @"ko", @"unknown"]) {
        session.targetLanguage = target;
        assert(![controller currentGlossRequest]);
    }
    session.targetLanguage = @"en";
    session.scheme = 3;
    assert(![controller currentGlossRequest]);
    for (NSNumber *scheme in @[@0, @1]) {
        session.scheme = scheme.unsignedIntegerValue;
        session.localMode = @"temporary_japanese";
        assert(![controller currentGlossRequest]);
    }
    session.localMode = @"none";
    assert([controller currentGlossRequest]);
    [controller synchronizeCandidateGloss];
    assert(dispatch_semaphore_wait(controller.started, dispatch_time(DISPATCH_TIME_NOW, 5 * NSEC_PER_SEC)) == 0);
    // Requested settings take effect before the Engine applies its deferred snapshot.
    [controller applySharedToolbarPreferences:@{@"translation_target_language":@"fr"}];
    assert(![controller currentGlossRequest] && session.clears == 1);
    dispatch_semaphore_signal(controller.released);
    [(NSOperationQueue *)[controller valueForKey:@"glossQueue"] waitUntilAllOperationsAreFinished];
    [NSRunLoop.mainRunLoop runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.02]];
    assert(session.applications == 0);
    [controller applySharedToolbarPreferences:@{@"translation_target_language":@"en"}];
    assert([controller currentGlossRequest]);
    [controller synchronizeCandidateGloss];
    assert(dispatch_semaphore_wait(controller.started, dispatch_time(DISPATCH_TIME_NOW, 5 * NSEC_PER_SEC)) == 0);
    session.localMode = @"temporary_japanese";
    dispatch_semaphore_signal(controller.released);
    [(NSOperationQueue *)[controller valueForKey:@"glossQueue"] waitUntilAllOperationsAreFinished];
    [NSRunLoop.mainRunLoop runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.02]];
    assert(session.applications == 0);
    [controller cancelCandidateGloss];
}

// A controller that opens no session hands every key to the application, which is indistinguishable from the
// user having chosen English. So the reason has to be recorded, and recorded once: a dictionary that was
// never prepared otherwise looks like an input method bug, which is how #912 came to be diagnosed from
// silence and fixed in a file no target compiles.
//
// Driven through the reason and the report rather than through prepareSession, which would read whichever
// runtime options the machine running the test happens to have installed.
static void TestSessionUnavailableIsReported() {
    assert([MSIMESessionUnavailableReason(nil) isEqual:@"no usable runtime options"]);
    assert([MSIMESessionUnavailableReason(@{}) isEqual:@"session refused the runtime options"]);

    NSString *directory = [NSTemporaryDirectory() stringByAppendingPathComponent:
        [@"msime.session-unavailable." stringByAppendingString:NSUUID.UUID.UUIDString]];
    assert([NSFileManager.defaultManager createDirectoryAtPath:directory withIntermediateDirectories:YES
                                                    attributes:nil error:nil]);
    msime_macos_diagnostic_configure(directory.UTF8String, true);
    MSIMEInputController *controller = [MSIMEInputController alloc];
    [controller reportSessionUnavailable:MSIMESessionUnavailableReason(nil)];
    [controller reportSessionUnavailable:MSIMESessionUnavailableReason(nil)];
    [controller reportSessionUnavailable:MSIMESessionUnavailableReason(@{})];
    NSString *log = [NSString stringWithContentsOfFile:[directory stringByAppendingPathComponent:@"diagnostic.log"]
                                              encoding:NSUTF8StringEncoding error:nil];
    msime_macos_diagnostic_configure("", false);
    [NSFileManager.defaultManager removeItemAtPath:directory error:nil];
    assert(log);
    // The repeat says nothing new; without this every focus while the dictionary is missing writes a line.
    assert([log componentsSeparatedByString:@"reason=no usable runtime options"].count - 1 == 1);
    assert([log componentsSeparatedByString:@"reason=session refused the runtime options"].count - 1 == 1);
}

int main(int argc, char **argv) {
    assert(!MSIMEShouldRegisterInputSource(1, nullptr));
    const char *registerArguments[] = {"test", "--register-input-source"};
    assert(MSIMEShouldRegisterInputSource(2, registerArguments));
    @autoreleasepool {
        [NSApplication sharedApplication];
        if (argc == 2 && std::string(argv[1]) == "--translations") {
            TestPreferenceLoadReusesCandidateServiceSnapshots();
            TestCandidateServiceSnapshotsAreReused();
            TestApplyCandidateTranslationSnapshotsAreReused();
            TestGlossScheduling();
            TestAccountGlossSkipsNonChineseCandidates();
            TestAccountGlossRequiresExplicitChoice();
            TestAccountGlossCacheIsSharedAcrossControllers();
            TestOfflineTargetGlosses();
            TestOnDeviceGlosses();
            TestOnDeviceGlossPersistence();
            TestCustomTranslationController();
            TestSecondaryTranslationScheduling();
            TestCustomTranslationCacheDelivery();
            TestCustomTranslationIdleDelay(NO);
            TestCustomTranslationIdleDelay(YES);
            TestTencentCandidateScheduling();
            TestNiuTransCandidateScheduling();
            TestLearnedGlossRuntime();
            TestCandidateTranslationPreference();
            TestGlossModePolicy();
            return 0;
        }
        NSUserDefaults *standardDefaults = NSUserDefaults.standardUserDefaults;
        id previousVoiceHoldSpace = [standardDefaults objectForKey:@"MSIMEClientVoiceHotkeyHoldSpace"];
        [standardDefaults setBool:NO forKey:@"MSIMEClientVoiceHotkeyHoldSpace"];
        TestSessionUnavailableIsReported();
        TestCloudCandidateScheduling();
        TestCloudCandidateRetryAfterRejectedResponse();
        TestCloudCandidateEngineDelivery();
        TestAiCandidateScheduling();
        TestAiCandidatesIgnoreGlossSwitch();
        TestAiCandidateRetryAfterRejectedResponse();
        TestAiCandidateCacheAcrossGenerations();
        TestAiCandidateDescriptorFailureIsRetryable();
        TestAiCandidateEngineDelivery();
        TestCloudCandidatePreference();
        TestCloudCandidateConsent();
        TestGlossScheduling();
        TestAccountGlossSkipsNonChineseCandidates();
        TestAccountGlossRequiresExplicitChoice();
        TestAccountGlossCacheIsSharedAcrossControllers();
        TestOfflineTargetGlosses();
        TestOnDeviceGlosses();
        TestOnDeviceGlossPersistence();
        TestCustomTranslationController();
        TestSecondaryTranslationScheduling();
        TestCustomTranslationCacheDelivery();
        TestCustomTranslationIdleDelay(NO);
        TestCustomTranslationIdleDelay(YES);
        TestTencentCandidateScheduling();
        TestNiuTransCandidateScheduling();
        TestLearnedGlossRuntime();
        TestCandidateTranslationPreference();
        TestGlossModePolicy();
        TestSharedInputPreferences();
        TestIndependentAssistancePreferences();
        TestSharedPunctuation();
        TestSharedTraditionalOutput();
        TestPageSizeCache();
        NSString *suite = [@"app.msime.test.appearance." stringByAppendingString:NSUUID.UUID.UUIDString];
        NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
        MSIMEAppearancePreferences *appearance = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
        assert(!appearance.vertical && appearance.fontSize == 18);
        assert(appearance.candidateFollowCursor);
        assert(appearance.pageShortcut == 0);
        // Unset is the shared default, and a number past the end is pulled to the end rather than to
        // the top of a three-value set.
        assert(appearance.pageSize == 6);
        assert([appearance.globalTheme isEqual:@"system"]);
        // A retired or foreign id is refused rather than normalised: nothing is stored and the theme stays.
        for (NSString *invalid in @[@"../invalid", @"fluent", @"willow_green", @""]) {
            appearance.globalTheme = invalid;
            assert([appearance.globalTheme isEqual:@"system"]);
        }
        appearance.pageSize = 10;
        assert(appearance.pageSize == 9);
        appearance.pageSize = 4;
        assert(appearance.pageSize == 4);
        appearance.pageShortcut = 99;
        assert(appearance.pageShortcut == 0);
        appearance.fontSize = 99;
        assert(appearance.fontSize == 18);
        NSPopUpButton *layoutControl = (id)PreferenceControl(appearance, @selector(layoutChanged:));
        NSPopUpButton *fontControl = (id)PreferenceControl(appearance, @selector(fontChanged:));
        NSPopUpButton *shortcutControl = (id)PreferenceControl(appearance, @selector(pageShortcutChanged:));
        NSPopUpButton *sizeControl = (id)PreferenceControl(appearance, @selector(pageSizeChanged:));
        NSButton *followCursorControl = (id)PreferenceControl(appearance, @selector(candidateFollowCursorChanged:));
        assert(followCursorControl.state == NSControlStateValueOn);
        [followCursorControl setState:NSControlStateValueOff];
        [NSApp sendAction:followCursorControl.action to:followCursorControl.target from:followCursorControl];
        assert(!appearance.candidateFollowCursor);
        MSIMEAppearancePreferences *followCursorLoaded = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
        assert(!followCursorLoaded.candidateFollowCursor);
        assert([[[appearance sharedPreferencesByMerging:@{}] objectForKey:@"candidate_follow_cursor"] isEqual:@NO]);
        [appearance applySharedCandidatePreferences:@{@"candidate_follow_cursor": @YES}];
        assert(appearance.candidateFollowCursor);
        NSArray<NSString *> *skinIDs = @[@"system", @"shuishan", @"light", @"paper", @"night", @"ink", @"custom"];
        NSArray<NSSwitch *> *skinCards = [(id)[appearance skinSettingsView] valueForKey:@"switches"];
        assert(skinCards.count == skinIDs.count);
        for (NSUInteger option = 0; option < skinIDs.count; ++option) {
            assert([skinCards[option].identifier isEqual:skinIDs[option]]);
            [NSApp sendAction:skinCards[option].action to:skinCards[option].target from:skinCards[option]];
            MSIMEAppearancePreferences *loaded = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
            assert([loaded.globalTheme isEqual:skinIDs[option]]);
        }
        appearance.globalTheme = @"system";
        // The reference's set, three through nine.
        assert(sizeControl.numberOfItems == (NSInteger)msime::mac::kOfferedCandidatePageSizes);
        NSArray *pageSizes = @[@3, @4, @5, @6, @7, @8, @9];
        for (NSInteger option = 0; option < (NSInteger)msime::mac::kOfferedCandidatePageSizes; ++option) {
            assert(([sizeControl.itemTitles[option] isEqual:[NSString stringWithFormat:@"%@ 个", pageSizes[option]]]));
            [sizeControl selectItemAtIndex:option];
            [NSApp sendAction:sizeControl.action to:sizeControl.target from:sizeControl];
            MSIMEAppearancePreferences *loaded = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
            assert(loaded.pageSize == [pageSizes[option] unsignedIntegerValue]);
        }
        assert(([shortcutControl.itemTitles isEqual:@[@"- / =", @"[ / ]", @"Page Up / Page Down"]]));
        // Word-to-character owns a key group, and paging cannot take the same one: the setter refuses and
        // beeps rather than leaving both bound to the brackets. It ships enabled on the brackets, so the
        // bracket option is refused until it is turned off - which is what the control's tooltip tells the
        // user and what this loop has to turn off before every option can be selected.
        [appearance setWordCharacterEnabled:YES keys:@"brackets"];
        [shortcutControl selectItemAtIndex:1];
        [NSApp sendAction:shortcutControl.action to:shortcutControl.target from:shortcutControl];
        assert([[MSIMEAppearancePreferences alloc] initWithDefaults:defaults].pageShortcut != 1);

        [appearance setWordCharacterEnabled:NO keys:@"brackets"];
        for (NSInteger option = 0; option < 3; ++option) {
            [shortcutControl selectItemAtIndex:option];
            [NSApp sendAction:shortcutControl.action to:shortcutControl.target from:shortcutControl];
            MSIMEAppearancePreferences *loaded = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
            assert(loaded.pageShortcut == option);
        }
        appearance.pageShortcut = 0;
        assert(([layoutControl.itemTitles isEqual:@[@"横向排列", @"纵向列表"]]));
        [layoutControl selectItemAtIndex:1];
        [NSApp sendAction:layoutControl.action to:layoutControl.target from:layoutControl];
        assert(fontControl.numberOfItems == 21);
        for (NSInteger option = 0; option < 21; ++option) {
            [fontControl selectItemAtIndex:option];
            [NSApp sendAction:fontControl.action to:fontControl.target from:fontControl];
            MSIMEAppearancePreferences *loaded = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
            assert(loaded.fontSize == (NSUInteger)option + 12);
            assert(([fontControl.titleOfSelectedItem isEqual:[NSString stringWithFormat:@"%ld pt", option + 12]]));
        }
        [fontControl selectItemAtIndex:4];
        [NSApp sendAction:fontControl.action to:fontControl.target from:fontControl];
        MSIMEAppearancePreferences *reopened = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
        assert(reopened.vertical && reopened.fontSize == 16);
        appearance.fontSize = 18;
        MSIMECandidatePanel *focusPanel = [[MSIMECandidatePanel alloc] initWithContentRect:NSZeroRect styleMask:NSWindowStyleMaskBorderless | NSWindowStyleMaskNonactivatingPanel backing:NSBackingStoreBuffered defer:NO];
        assert(!focusPanel.canBecomeKeyWindow && !focusPanel.canBecomeMainWindow);
        MSIMECandidateButton *focusButton = [[MSIMECandidateButton alloc] initWithFrame:NSZeroRect];
        assert(!focusButton.acceptsFirstResponder);
        assert([focusButton acceptsFirstMouse:nil]);
        assert(!MSIMEValidCaret(NSZeroRect));
        assert(!MSIMEValidCaret(NSMakeRect(NAN, 0, 1, 20)));
        assert(!MSIMEValidCaret(NSMakeRect(0, INFINITY, 1, 20)));
        assert(!MSIMEValidCaret(NSMakeRect(0, 0, INFINITY, 20)));
        assert(!MSIMEValidCaret(NSMakeRect(0, 0, 1, -1)));
        assert(MSIMEValidCaret(NSMakeRect(-500, -200, 0, 20)));
        NSRect bounds = NSMakeRect(0, 0, 1000, 800);
        assert(NSEqualPoints(MSIMECandidateOrigin(NSMakeRect(100, 500, 1, 20), NSMakeSize(200, 100), bounds), NSMakePoint(100, 396)));
        assert(NSEqualPoints(MSIMECandidateOrigin(NSMakeRect(950, 20, 1, 20), NSMakeSize(200, 100), bounds), NSMakePoint(800, 44)));
        // Oversized content anchors at the visible origin, never outside both edges.
        assert(NSEqualPoints(MSIMECandidateOrigin(NSMakeRect(950, 20, 1, 20), NSMakeSize(1200, 900), bounds), NSZeroPoint));
        bounds = NSMakeRect(-1000, -800, 1000, 800);
        assert(NSEqualPoints(MSIMECandidateOrigin(NSMakeRect(-50, -780, 1, 20), NSMakeSize(200, 100), bounds), NSMakePoint(-200, -756)));
        // Inject a session and client without registering a system input source.
        MSIMEInputController *controller = [MSIMEInputController alloc];
        ShortcutSession *session = [ShortcutSession new];
        ShortcutClient *client = [ShortcutClient new];
        [controller setValue:session forKey:@"session"];
        [controller setValue:client forKey:@"activeClient"];
        [controller setValue:appearance forKey:@"appearance"];
        [NSUserDefaults.standardUserDefaults removeObjectForKey:@"MSIMEClientPinnedCandidates"];
        [controller syncPageSize];
        assert(session.requestedPageSize == 9);
        appearance.pageSize = 5;
        [controller appearanceChanged:nil];
        assert(session.requestedPageSize == 5);
        // Windows leaves an unshifted uppercase letter produced by CapsLock to
        // the host at the start of a composition, but routes the same key to
        // Engine once input is already in progress.
        [controller setValue:@{ @"focused": @YES, @"editing_text": @"", @"candidates": @[] } forKey:@"view"];
        session.asciiCalls = 0;
        NSEvent *capsFresh = [NSEvent keyEventWithType:NSEventTypeKeyDown location:NSZeroPoint
                                         modifierFlags:NSEventModifierFlagCapsLock timestamp:0 windowNumber:0
                                                context:nil characters:@"A" charactersIgnoringModifiers:@"a"
                                             isARepeat:NO keyCode:0];
        assert(![controller handleEvent:capsFresh client:client]);
        assert(session.asciiCalls == 0);
        [controller setValue:@{ @"focused": @YES, @"editing_text": @"n", @"candidates": @[] } forKey:@"view"];
        session.nextTransition = @{ @"handled": @YES, @"view": @{ @"focused": @YES, @"editing_text": @"nA", @"candidates": @[] } };
        session.asciiCalls = 0;
        assert([controller handleEvent:capsFresh client:client]);
        assert(session.asciiCalls == 1 && session.lastASCII == 'A' && !session.lastShift);
        session.nextTransition = nil;
        for (NSNumber *flags in @[@(NSEventModifierFlagCommand), @(NSEventModifierFlagControl), @(NSEventModifierFlagOption)]) {
            session.lastCommand = UINT32_MAX;
            client.committed = nil;
            client.marked = @"ceshi";
            NSEvent *event = [NSEvent keyEventWithType:NSEventTypeKeyDown location:NSZeroPoint modifierFlags:flags.unsignedIntegerValue timestamp:0 windowNumber:0 context:nil characters:@"a" charactersIgnoringModifiers:@"a" isARepeat:NO keyCode:0];
            assert(![controller handleEvent:event client:client]);
            assert(session.lastCommand == MSIME_FINISH_COMPOSITION);
            assert([client.committed isEqualToString:@"测试"]);
            assert(client.marked.length == 0);
        }
        TestCandidatePanel *panel = [TestCandidatePanel new];
        [controller setValue:panel forKey:@"panel"];
        // Windows maps Left/Right to the composition caret while candidates are shown, so a visible vertical panel sends the same caret move as a hidden one does and still consumes the key.
        const BOOL arrowVertical = appearance.vertical;
        appearance.vertical = YES;
        assert([appearance navigationEnabled:@"arrows"]);
        for (NSNumber *key in @[@123, @124]) {
            NSEvent *event = [NSEvent keyEventWithType:NSEventTypeKeyDown location:NSZeroPoint modifierFlags:0 timestamp:0 windowNumber:0 context:nil characters:@"" charactersIgnoringModifiers:@"" isARepeat:NO keyCode:key.unsignedShortValue];
            panel.visible = YES;
            session.lastCommand = UINT32_MAX;
            client.committed = nil;
            client.marked = @"ceshi";
            assert([controller handleEvent:event client:client]);
            assert(session.lastCommand == (key.unsignedShortValue == 123 ? MSIME_MOVE_LEFT : MSIME_MOVE_RIGHT));
            panel.visible = NO;
            session.lastCommand = UINT32_MAX;
            assert([controller handleEvent:event client:client]);
            assert(session.lastCommand == (key.unsignedShortValue == 123 ? MSIME_MOVE_LEFT : MSIME_MOVE_RIGHT));
        }
        appearance.vertical = arrowVertical;
        HiddenCandidatePanel *layoutPanel = [[HiddenCandidatePanel alloc] initWithContentRect:NSZeroRect styleMask:NSWindowStyleMaskBorderless | NSWindowStyleMaskNonactivatingPanel backing:NSBackingStoreBuffered defer:NO];
        for (NSNumber *key in @[@115, @119]) {
            NSEvent *event = [NSEvent keyEventWithType:NSEventTypeKeyDown location:NSZeroPoint modifierFlags:0 timestamp:0 windowNumber:0 context:nil characters:@"" charactersIgnoringModifiers:@"" isARepeat:NO keyCode:key.unsignedShortValue];
            panel.visible = YES;
            assert([controller handleEvent:event client:client]);
            assert(session.lastCommand == (key.unsignedShortValue == 115 ? MSIME_FIRST_CANDIDATE : MSIME_LAST_CANDIDATE));
            panel.visible = NO;
            assert([controller handleEvent:event client:client]);
            assert(session.lastCommand == (key.unsignedShortValue == 115 ? MSIME_MOVE_HOME : MSIME_MOVE_END));
        }
        [controller setValue:layoutPanel forKey:@"panel"];
        client.caret = NSMakeRect(NSMidX(NSScreen.mainScreen.visibleFrame), NSMidY(NSScreen.mainScreen.visibleFrame), 1, 20);
        appearance.candidateFollowCursor = NO;
        [controller setValue:@{@"candidates": @[@{@"text": @"测试", @"highlighted": @YES}]} forKey:@"view"];
        [controller renderCandidates];
        assert(layoutPanel.requestedVisible);
        NSPoint anchoredCandidateOrigin = layoutPanel.frame.origin;
        NSRect visibleFrame = NSScreen.mainScreen.visibleFrame;
        client.caret = NSMakeRect(NSMaxX(visibleFrame) - 120, NSMinY(visibleFrame) + 120, 1, 20);
        [controller renderCandidates];
        assert(NSEqualPoints(layoutPanel.frame.origin, anchoredCandidateOrigin));
        appearance.candidateFollowCursor = YES;
        [controller renderCandidates];
        assert(!NSEqualPoints(layoutPanel.frame.origin, anchoredCandidateOrigin));
        CGFloat shortWidth = layoutPanel.frame.size.width;
        [controller setValue:@{@"candidates": @[@{@"text": @"合成候选布局测试文本", @"highlighted": @YES}]} forKey:@"view"];
        [controller renderCandidates];
        assert(layoutPanel.frame.size.width > shortWidth);
        NSButton *rendered = (NSButton *)layoutPanel.contentView.subviews.firstObject;
        assert(rendered.font.pointSize == 18);
        assert(fabs(((MSIMECandidateButton *)rendered).numberFont.pointSize - 14.4) < 0.01);
        assert([rendered.toolTip isEqualToString:@"合成候选布局测试文本"]);
        // Candidates wrap inside their column instead of being cut off with an ellipsis.
        assert(rendered.lineBreakMode == NSLineBreakByWordWrapping);
        client.caret = NSZeroRect;
        [controller renderCandidates];
        assert(!layoutPanel.requestedVisible);
        client.caret = NSMakeRect(NSMidX(NSScreen.mainScreen.visibleFrame), NSMidY(NSScreen.mainScreen.visibleFrame), 1, 20);
        NSMutableDictionary *pageView = [@{@"session": @1, @"generation": @2, @"focused": @YES, @"page": @0, @"page_count": @3, @"editing_text": @"ceshi", @"caret_position": @5, @"candidates": @[@{@"text": @"测试", @"highlighted": @YES}]} mutableCopy];
        [controller setValue:[pageView copy] forKey:@"view"];
        [controller renderCandidates];
        MSIMECandidateButton *previous = (id)PageButton(layoutPanel.contentView, -1);
        MSIMECandidateButton *next = (id)PageButton(layoutPanel.contentView, -2);
        assert(previous && next && !previous.enabled && next.enabled);
        assert([previous.accessibilityLabel isEqual:@"上一页候选"]);
        assert([next.accessibilityLabel isEqual:@"下一页候选"]);
        assert(previous.frame.size.height == 26 && next.frame.size.width == 28);
        // A gloss-only update keeps the candidate identity and row geometry, so the visible row can
        // be updated in place without rebuilding its AppKit button.
        NSMutableDictionary *glossReuseView = [@{
            @"session": @1, @"generation": @2, @"focused": @YES, @"page": @0, @"page_count": @1,
            @"editing_text": @"ceshi",
            @"candidates": @[@{@"text": @"测试", @"highlighted": @YES,
                               @"id": @{ @"session": @1, @"generation": @2, @"index": @0 }}]
        } mutableCopy];
        NSDictionary *beforeGlossView = [glossReuseView copy];
        [controller setValue:beforeGlossView forKey:@"view"];
        [controller renderCandidates];
        MSIMECandidateButton *reusedBeforeGloss = PageButton(layoutPanel.contentView, 0);
        NSMutableDictionary *glossReuseCandidate = [glossReuseView[@"candidates"][0] mutableCopy];
        glossReuseCandidate[@"translation"] = @"x";
        glossReuseView[@"candidates"] = @[glossReuseCandidate];
        [controller setValue:[glossReuseView copy] forKey:@"view"];
        [controller renderCandidates];
        MSIMECandidateButton *reusedAfterGloss = PageButton(layoutPanel.contentView, 0);
        assert(reusedAfterGloss == reusedBeforeGloss && [reusedAfterGloss.translation isEqual:@"x"] &&
               [reusedAfterGloss.toolTip containsString:@"\nx"]);
        [controller setValue:[pageView copy] forKey:@"view"];
        [controller renderCandidates];
        previous = (id)PageButton(layoutPanel.contentView, -1);
        next = (id)PageButton(layoutPanel.contentView, -2);
        // The arrows and 「1 / 3」 share the card's top row, right-aligned above the candidates.
        NSTextField *pageIndicator = nil;
        for (NSView *child in layoutPanel.contentView.subviews)
            if ([child.identifier isEqual:@"candidate-page-indicator"]) pageIndicator = (id)child;
        assert(pageIndicator && [pageIndicator.stringValue isEqual:@"1 / 3"]);
        assert([pageIndicator.accessibilityLabel isEqual:@"第 1 页，共 3 页"]);
        MSIMECandidateButton *pagedCandidate = PageButton(layoutPanel.contentView, 0);
        assert(NSMinY(previous.frame) >= NSMaxY(pagedCandidate.frame) && NSMinY(next.frame) == NSMinY(previous.frame));
        assert(NSMaxX(next.frame) <= layoutPanel.contentView.bounds.size.width && NSMaxX(pageIndicator.frame) <= NSMinX(previous.frame));
        // 「1 / 3」 and ‹ › are the design's 13pt secondary run (dc.html L1324).
        const auto pageTokens = [appearance resolvedSkinForDark:[[layoutPanel.contentView.effectiveAppearance bestMatchFromAppearancesWithNames:@[NSAppearanceNameAqua, NSAppearanceNameDarkAqua]] isEqual:NSAppearanceNameDarkAqua]].tokens;
        assert(next.titleColor && [next.titleColor isEqual:SkinColor(pageTokens.number)]);
        assert([pageIndicator.textColor isEqual:SkinColor(pageTokens.number)] && pageIndicator.font.pointSize == 13);
        // A layout-only render can keep all Engine IDs unchanged. The detached
        // button must still be rejected, just like detached candidate buttons.
        MSIMECandidateButton *oldPageButton = next;
        [controller renderCandidates];
        session.lastCommand = UINT32_MAX;
        [controller changeCandidatePage:oldPageButton];
        assert(session.lastCommand == UINT32_MAX && oldPageButton.superview != layoutPanel.contentView);
        next = (id)PageButton(layoutPanel.contentView, -2);
        NSDictionary *pageIdentity = next.candidateID;
        for (NSString *field in @[@"session", @"generation", @"page", @"page_count", @"focused"]) {
            for (id invalid in @[NSNull.null, @YES, @(-1), @1.5, @"1"]) {
                NSMutableDictionary *bad = [pageView mutableCopy];
                bad[field] = [field isEqual:@"focused"] && [invalid isEqual:@YES] ? @1 : invalid;
                next.candidateID = bad;
                [controller setValue:bad forKey:@"view"];
                session.lastCommand = UINT32_MAX;
                [controller changeCandidatePage:next];
                assert(session.lastCommand == UINT32_MAX);
            }
        }
        for (NSNumber *invalidCount in @[@0, @1]) {
            NSMutableDictionary *bad = [pageView mutableCopy];
            bad[@"page"] = @1; bad[@"page_count"] = invalidCount;
            next.candidateID = bad;
            [controller setValue:bad forKey:@"view"];
            session.lastCommand = UINT32_MAX;
            [controller changeCandidatePage:next];
            assert(session.lastCommand == UINT32_MAX);
        }
        next.candidateID = pageIdentity;
        [controller setValue:[pageView copy] forKey:@"view"];
        pageView[@"page"] = @1;
        session.nextTransition = @{@"handled": @YES, @"commit": NSNull.null, @"view": [pageView copy]};
        client.committed = nil;
        [next performClick:nil];
        assert(session.lastCommand == MSIME_NEXT_PAGE && client.committed == nil);
        assert([client.marked isEqual:@"ceshi"]);
        // A previous page's retained button must not advance the current page.
        session.lastCommand = UINT32_MAX;
        [controller changeCandidatePage:next];
        assert(session.lastCommand == UINT32_MAX);
        previous = (id)PageButton(layoutPanel.contentView, -1);
        next = (id)PageButton(layoutPanel.contentView, -2);
        assert(previous.enabled && next.enabled);
        pageView[@"page"] = @0;
        session.nextTransition = @{@"handled": @YES, @"commit": NSNull.null, @"view": [pageView copy]};
        [previous performClick:nil];
        assert(session.lastCommand == MSIME_PREVIOUS_PAGE);
        for (NSString *changed in @[@"session", @"generation", @"focused"]) {
            [controller setValue:[pageView copy] forKey:@"view"];
            [controller renderCandidates];
            next = (id)PageButton(layoutPanel.contentView, -2);
            NSMutableDictionary *stale = [pageView mutableCopy];
            stale[changed] = [changed isEqual:@"focused"] ? @NO : @99;
            [controller setValue:stale forKey:@"view"];
            session.lastCommand = UINT32_MAX;
            [controller changeCandidatePage:next];
            assert(session.lastCommand == UINT32_MAX);
        }
        pageView[@"page"] = @2;
        [controller setValue:[pageView copy] forKey:@"view"];
        [controller renderCandidates];
        previous = (id)PageButton(layoutPanel.contentView, -1);
        next = (id)PageButton(layoutPanel.contentView, -2);
        assert(previous.enabled && !next.enabled);
        session.lastCommand = UINT32_MAX;
        [controller changeCandidatePage:next];
        assert(session.lastCommand == UINT32_MAX);
        [layoutPanel orderOut:nil];
        [controller changeCandidatePage:previous];
        assert(session.lastCommand == UINT32_MAX);
        pageView[@"page"] = @0;
        pageView[@"page_count"] = @1;
        // The candidate preedit uses Engine display text, independent of candidate font.
        pageView[@"preedit"] = @"ce'shi";
        pageView[@"caret_position"] = @2;
        [controller setValue:pageView forKey:@"view"];
        // Fallback text can be taller than the primary family at the same size.
        appearance.fontFamily = @"Helvetica";
        appearance.fontSize = 32;
        appearance.preeditFontSize = 32;
        appearance.showsCandidatePreedit = YES;
        NSFont *fallbackFont = [appearance candidateFontOfSize:32];
        CGFloat fallbackHeight = ceil([@"合😀" sizeWithAttributes:@{NSFontAttributeName:fallbackFont}].height);
        assert(fallbackHeight > ceil(fallbackFont.ascender - fallbackFont.descender + fallbackFont.leading));
        NSMutableDictionary *fallbackView = [pageView mutableCopy];
        fallbackView[@"preedit"] = @"合😀";
        fallbackView[@"candidates"] = @[@{@"text": @"合😀", @"highlighted": @YES}];
        for (NSNumber *vertical in @[@NO, @YES]) {
            appearance.vertical = vertical.boolValue;
            [controller setValue:fallbackView forKey:@"view"];
            [controller renderCandidates];
            for (NSView *child in layoutPanel.contentView.subviews) {
                if ([child.identifier isEqual:@"candidate-preedit"]) assert(child.frame.size.height >= fallbackHeight + 6);
                if ([child isKindOfClass:MSIMECandidateButton.class]) assert(child.frame.size.height >= fallbackHeight + 12);
            }
        }
        appearance.fontSize = 18;
        [controller setValue:pageView forKey:@"view"];
        NSString *installedFamily = [NSFont fontWithName:@"Menlo" size:18].familyName;
        assert(installedFamily);
        appearance.fontFamily = installedFamily;
        for (NSNumber *vertical in @[@NO, @YES]) {
            appearance.vertical = vertical.boolValue;
            appearance.showsCandidatePreedit = YES;
            appearance.preeditFontSize = 32;
            [controller renderCandidates];
            NSTextField *preeditLabel = nil;
            for (NSView *child in layoutPanel.contentView.subviews)
                if ([child.identifier isEqual:@"candidate-preedit"]) preeditLabel = (id)child;
            assert(preeditLabel && [preeditLabel.stringValue isEqual:@"ce'shi"] && preeditLabel.font.pointSize == 32);
            assert([preeditLabel isKindOfClass:MSIMECandidatePreeditField.class]);
            MSIMECandidatePreeditField *caretLabel = (id)preeditLabel;
            assert(caretLabel.caretIndex == 2 && caretLabel.showsCaret);
            NSRect middleCaret = caretLabel.caretRect;
            assert(!NSIsEmptyRect(middleCaret) && NSContainsRect(caretLabel.bounds, middleCaret));
            NSBitmapImageRep *withCaret = [caretLabel bitmapImageRepForCachingDisplayInRect:caretLabel.bounds];
            [caretLabel cacheDisplayInRect:caretLabel.bounds toBitmapImageRep:withCaret];
            caretLabel.showsCaret = NO;
            assert(NSIsEmptyRect(caretLabel.caretRect));
            NSBitmapImageRep *withoutCaret = [caretLabel bitmapImageRepForCachingDisplayInRect:caretLabel.bounds];
            [caretLabel cacheDisplayInRect:caretLabel.bounds toBitmapImageRep:withoutCaret];
            assert(![[withCaret TIFFRepresentation] isEqual:[withoutCaret TIFFRepresentation]]);
            caretLabel.showsCaret = YES;
            caretLabel.caretIndex = 0;
            assert(NSMinX(caretLabel.caretRect) < NSMinX(middleCaret));
            caretLabel.caretIndex = caretLabel.stringValue.length;
            assert(NSMinX(caretLabel.caretRect) > NSMinX(middleCaret));
            caretLabel.caretIndex = 2;
            assert([preeditLabel.font.familyName isEqual:installedFamily]);
            // The reading is the design's semibold accent run (dc.html L1325).
            assert([[preeditLabel.font.fontDescriptor objectForKey:NSFontTraitsAttribute][NSFontWeightTrait] doubleValue] > 0);
            const auto preeditTokens = [appearance resolvedSkinForDark:NO].tokens;
            layoutPanel.contentView.appearance = [NSAppearance appearanceNamed:NSAppearanceNameAqua];
            [controller refreshCandidateSkin];
            assert([preeditLabel.textColor isEqual:SkinColor(preeditTokens.accent)]);
            assert([caretLabel.caretColor isEqual:SkinColor(preeditTokens.accent)]);
            if (!vertical.boolValue) {
                // The seven pickers are the custom theme's, and draw while it is selected.
                [appearance applySharedCandidatePreferences:@{@"global_theme": @"custom", @"custom_theme": @{@"candidate_colors": @{
                    @"text": @"#102030", @"number": @"#203040", @"accent": @"#304050", @"selected": @"#405060",
                    @"hover": @"#506070", @"surface": @"#607080", @"border": @"#708090"}}}];
                const auto picked = [appearance resolvedSkinForDark:NO].tokens;
                assert([SkinColor(picked.surface) isEqual:[NSColor colorWithSRGBRed:0x60 / 255.0 green:0x70 / 255.0 blue:0x80 / 255.0 alpha:1]]);
                NSMutableDictionary *colorView = [pageView mutableCopy];
                colorView[@"candidates"] = @[@{@"text": @"selected", @"highlighted": @YES},
                    @{@"text": @"ordinary", @"highlighted": @NO}];
                [controller setValue:colorView forKey:@"view"];
                [controller renderCandidates];
                MSIMECandidateChromeView *chrome = (id)layoutPanel.contentView;
                MSIMECandidateButton *selectedButton = PageButton(chrome, 0);
                MSIMECandidateButton *customButton = PageButton(chrome, 1);
                preeditLabel = nil;
                for (NSView *child in chrome.subviews)
                    if ([child.identifier isEqual:@"candidate-preedit"]) preeditLabel = (id)child;
                assert(preeditLabel && [preeditLabel isKindOfClass:MSIMECandidatePreeditField.class]);
                caretLabel = (id)preeditLabel;
                assert([chrome.fillColor isEqual:SkinColor(picked.surface)]);
                assert([chrome.strokeColor isEqual:SkinColor(picked.border)]);
                assert([customButton.titleColor isEqual:SkinColor(picked.text)]);
                assert([customButton.numberColor isEqual:SkinColor(picked.number)]);
                assert([selectedButton.fillColor isEqual:SkinColor(picked.selected)]);
                assert([selectedButton.titleColor isEqual:SkinColor(picked.selectedText)]);
                assert([selectedButton.numberColor isEqual:SkinColor(picked.selectedNumber)]);
                assert([customButton.hoverColor isEqual:SkinColor(picked.hover)]);
                assert([customButton.barColor isEqual:SkinColor(picked.accent)]);
                assert([caretLabel.caretColor isEqual:customButton.barColor]);
                [appearance applySharedCandidatePreferences:@{@"global_theme": @"system"}];
                [controller setValue:pageView forKey:@"view"];
                [controller renderCandidates];
                preeditLabel = nil;
                for (NSView *child in layoutPanel.contentView.subviews)
                    if ([child.identifier isEqual:@"candidate-preedit"]) preeditLabel = (id)child;
                assert(preeditLabel && [preeditLabel isKindOfClass:MSIMECandidatePreeditField.class]);
                caretLabel = (id)preeditLabel;
            }
            assert(NSContainsRect(layoutPanel.contentView.bounds, preeditLabel.frame));
            for (NSView *child in layoutPanel.contentView.subviews)
                if ([child isKindOfClass:MSIMECandidateButton.class]) {
                    assert(!NSIntersectsRect(child.frame, preeditLabel.frame));
                    assert([((NSButton *)child).font.familyName isEqual:installedFamily]);
                }
            CGFloat shownHeight = layoutPanel.frame.size.height;
            appearance.showsCandidatePreedit = NO;
            [controller renderCandidates];
            assert(layoutPanel.frame.size.height < shownHeight);
            for (NSView *child in layoutPanel.contentView.subviews) assert(![child.identifier isEqual:@"candidate-preedit"]);
        }
        appearance.showsCandidatePreedit = YES;
        appearance.preeditFontSize = 16;
        // Long rows scroll rather than truncating away the insertion point.
        MSIMECandidatePreeditField *longPreedit = [MSIMECandidatePreeditField labelWithString:[@"ni'" stringByPaddingToLength:180 withString:@"ni'" startingAtIndex:0]];
        longPreedit.frame = NSMakeRect(0, 0, 80, 28);
        longPreedit.font = [NSFont systemFontOfSize:16];
        longPreedit.showsCaret = YES;
        for (NSNumber *offset in @[@0, @50, @180, @(NSUIntegerMax)]) {
            longPreedit.caretIndex = offset.unsignedIntegerValue;
            assert(NSContainsRect(longPreedit.bounds, longPreedit.caretRect));
        }
        longPreedit.stringValue = @"合😀";
        longPreedit.caretIndex = 3;
        assert(NSContainsRect(longPreedit.bounds, longPreedit.caretRect));
        longPreedit.stringValue = @"";
        assert(NSIsEmptyRect(longPreedit.caretRect));
        longPreedit.stringValue = @"iiii";
        longPreedit.caretIndex = 2;
        CTLineRef slotted = [longPreedit newPreeditLine];
        CGFloat beforeSlot = CTLineGetOffsetForStringIndex(slotted, 2, nullptr);
        CGFloat afterSlot = CTLineGetOffsetForStringIndex(slotted, 3, nullptr);
        assert(fabs(afterSlot - beforeSlot - 2.95) < 0.01);
        assert(fabs(NSMinX(longPreedit.caretRect) - (2 + beforeSlot + 0.85)) < 0.01);
        assert(fabs(NSWidth(longPreedit.caretRect) - 1.25) < 0.01);
        CFRelease(slotted);
        assert([longPreedit.stringValue isEqual:@"iiii"]); // The slot is display-only.
        longPreedit.showsCaret = NO;
        CTLineRef unslotted = [longPreedit newPreeditLine];
        CGFloat unslottedWidth = CTLineGetTypographicBounds(unslotted, nullptr, nullptr, nullptr);
        CFRelease(unslotted);
        longPreedit.showsCaret = YES;
        longPreedit.caretIndex = 4;
        assert(fabs(NSMinX(longPreedit.caretRect) - (2 + unslottedWidth + 1.5)) < 0.01);
        appearance.fontFamily = @"Segoe UI";
        for (id display in @[@"", NSNull.null]) {
            pageView[@"preedit"] = display;
            [controller setValue:pageView forKey:@"view"];
            [controller renderCandidates];
            NSTextField *label = nil;
            for (NSView *child in layoutPanel.contentView.subviews)
                if ([child.identifier isEqual:@"candidate-preedit"]) label = (id)child;
            if (display == NSNull.null) assert([label.stringValue isEqual:@"ceshi"]);
            else assert(label == nil);
        }
        pageView[@"preedit"] = @"ce'shi";
        [controller setValue:pageView forKey:@"view"];
        [controller renderCandidates];
        assert(PageButton(layoutPanel.contentView, -1) == nil);
        assert(PageButton(layoutPanel.contentView, -2) == nil);
        pageView[@"candidates"] = @[@{@"text": @"测试", @"highlighted": @YES, @"id": @{@"session": @1, @"generation": @2, @"index": @0}}, @{@"text": @"布局", @"highlighted": @NO, @"id": @{@"session": @1, @"generation": @2, @"index": @1}}];
        [controller setValue:pageView forKey:@"view"];
        [controller renderCandidates];
        CGFloat verticalHeight = layoutPanel.frame.size.height;
        MSIMECandidateButton *clickCandidate = (id)PageButton(layoutPanel.contentView, 1);
        assert(clickCandidate);
        // Keyboard selection is tied to the button identities in the rendered
        // panel, not a newer controller snapshot that AppKit has not painted.
        NSEvent *(^candidateKey)(unsigned short, NSString *) = ^NSEvent *(unsigned short code, NSString *characters) {
            return [NSEvent keyEventWithType:NSEventTypeKeyDown location:NSZeroPoint modifierFlags:0 timestamp:0 windowNumber:0 context:nil characters:characters charactersIgnoringModifiers:characters isARepeat:NO keyCode:code];
        };
        NSUInteger selectedCallsBeforeKeyboard = session.selectCalls;
        // Both baselines are relative, because this session is shared with everything above: the Caps Lock
        // block has already sent one ASCII 'A' through it, so an absolute asciiCalls == 0 here asserts
        // something about another test rather than about this key.
        NSUInteger asciiCallsBeforeKeyboard = session.asciiCalls;
        assert([controller handleEvent:candidateKey(18, @"1") client:client]);
        assert(session.selectCalls == selectedCallsBeforeKeyboard + 1 && session.selectedGeneration == 2 && session.selectedIndex == 0);
        // Slot 3 has no button in the painted panel. The key is consumed rather than selecting anything or
        // falling through to Engine numeric input, which would type a literal 3 into the composition.
        assert([controller handleEvent:candidateKey(20, @"3") client:client]);
        assert(session.selectCalls == selectedCallsBeforeKeyboard + 1 && session.asciiCalls == asciiCallsBeforeKeyboard);
        NSMutableDictionary *unpaintedView = [pageView mutableCopy];
        unpaintedView[@"generation"] = @99;
        [controller setValue:unpaintedView forKey:@"view"];
        assert([controller handleEvent:candidateKey(18, @"1") client:client]);
        assert(session.selectCalls == selectedCallsBeforeKeyboard + 1 && session.asciiCalls == asciiCallsBeforeKeyboard);
        assert([controller handleEvent:candidateKey(49, @" ") client:client]);
        assert(session.selectCalls == selectedCallsBeforeKeyboard + 1);
        [controller setValue:pageView forKey:@"view"];
        [controller renderCandidates];
        assert([controller handleEvent:candidateKey(49, @" ") client:client]);
        assert(session.selectCalls == selectedCallsBeforeKeyboard + 2 && session.selectedGeneration == 2 && session.selectedIndex == 0);
        const NSEventModifierFlags deleteModifiers = NSEventModifierFlagControl | NSEventModifierFlagOption | NSEventModifierFlagShift;
        NSEvent *(^deleteEvent)(unsigned short, NSEventModifierFlags, BOOL) = ^NSEvent *(unsigned short code, NSEventModifierFlags modifiers, BOOL repeat) {
            return [NSEvent keyEventWithType:NSEventTypeKeyDown location:NSZeroPoint modifierFlags:modifiers timestamp:0 windowNumber:0 context:nil characters:@"!" charactersIgnoringModifiers:@"!" isARepeat:repeat keyCode:code];
        };
        NSArray *digitCodes = @[@18, @19, @20, @21, @23, @22, @26, @28];
        NSMutableDictionary *deleteView = [pageView mutableCopy];
        NSMutableArray *deleteCandidates = [NSMutableArray array];
        for (NSUInteger slot = 0; slot < 8; ++slot)
            [deleteCandidates addObject:@{@"id":@{@"session":@1, @"generation":@2, @"index":@(40 + slot)}, @"text":@"合成测试"}];
        deleteView[@"candidates"] = deleteCandidates;
        [controller setValue:deleteView forKey:@"view"];
        [controller renderCandidates];
        for (NSUInteger slot = 0; slot < 8; ++slot) {
            unsigned short code = [digitCodes[slot] unsignedShortValue];
            NSUInteger calls = session.maintenanceCalls;
            session.lastCommand = UINT32_MAX;
            assert([controller handleEvent:deleteEvent(code, deleteModifiers, NO) client:client]);
            assert(session.maintenanceCalls == calls + 1 && session.maintenanceAction == 1);
            assert(session.selectedGeneration == 2 && session.selectedIndex == 40 + slot && session.lastCommand == UINT32_MAX);
            assert([controller handleEvent:deleteEvent(code, deleteModifiers, YES) client:client]);
            assert(session.maintenanceCalls == calls + 1);
            for (NSNumber *modifiers in @[@(deleteModifiers | NSEventModifierFlagCommand), @(deleteModifiers ^ NSEventModifierFlagControl), @(deleteModifiers ^ NSEventModifierFlagOption), @(deleteModifiers ^ NSEventModifierFlagShift)])
                assert(MSIMECandidateDeletionSlot(deleteEvent(code, modifiers.unsignedIntegerValue, NO)) == NSNotFound);
        }
        for (NSNumber *code in @[@25, @29, @83, @84, @85, @86, @87, @88, @89, @91])
            assert(MSIMECandidateDeletionSlot(deleteEvent(code.unsignedShortValue, deleteModifiers, NO)) == NSNotFound);
        NSUInteger deletionCalls = session.maintenanceCalls;
        for (id invalid in @[NSNull.null, @{}, @{@"id":@{@"session":@1, @"generation":@99, @"index":@0}}]) {
            NSMutableDictionary *invalidView = [deleteView mutableCopy];
            invalidView[@"generation"] = @99;
            invalidView[@"candidates"] = @[invalid];
            [controller setValue:invalidView forKey:@"view"];
            assert([controller handleEvent:deleteEvent(18, deleteModifiers, NO) client:client]);
        }
        [controller setValue:pageView forKey:@"view"];
        [controller renderCandidates];
        assert([controller handleEvent:deleteEvent(28, deleteModifiers, NO) client:client]); // Empty slot.
        assert(session.maintenanceCalls == deletionCalls && session.lastCommand == UINT32_MAX);
        clickCandidate = (id)PageButton(layoutPanel.contentView, 1);
        assert(clickCandidate);
        NSMenu *candidateMenu = clickCandidate.menu;
        NSEvent *rightClick = [NSEvent mouseEventWithType:NSEventTypeRightMouseDown location:NSZeroPoint modifierFlags:0 timestamp:0 windowNumber:0 context:nil eventNumber:1 clickCount:1 pressure:1];
        assert([clickCandidate menuForEvent:rightClick] == candidateMenu);
        assert(candidateMenu.numberOfItems == 3);
        assert([[candidateMenu itemAtIndex:0].title isEqual:@"置顶"]);
        assert([[candidateMenu itemAtIndex:1].title isEqual:@"固定排位"]);
        assert([[candidateMenu itemAtIndex:2].title isEqual:@"删除"]);
        assert([[candidateMenu itemAtIndex:2].keyEquivalent isEqual:@"2"]);
        assert([candidateMenu itemAtIndex:2].keyEquivalentModifierMask == deleteModifiers);
        NSMenu *positionMenu = [candidateMenu itemAtIndex:1].submenu;
        assert(positionMenu.numberOfItems == 7 && [positionMenu itemAtIndex:5].separatorItem);
        [NSUserDefaults.standardUserDefaults setObject:@{@"ce'shi": @[@"布局"]} forKey:@"MSIMEClientPinnedCandidates"];
        [controller renderCandidates];
        // Pinning moves the candidate to the first slot - that is what 置顶 does - while it keeps the Engine
        // index it had, so selecting it still commits the right word. Reading slot 1 here asserted the
        // opposite of the feature, and never ran because the suite aborted earlier.
        MSIMECandidateButton *pinnedCandidate = (id)PageButton(layoutPanel.contentView, 0);
        assert(pinnedCandidate && [pinnedCandidate.title containsString:@"布局"]);
        assert([pinnedCandidate.candidateID[@"index"] isEqual:@1]);
        assert([[[pinnedCandidate menuForEvent:rightClick] itemAtIndex:0].title isEqual:@"取消置顶"]);
        [NSUserDefaults.standardUserDefaults removeObjectForKey:@"MSIMEClientPinnedCandidates"];
        [controller renderCandidates];
        clickCandidate = (id)PageButton(layoutPanel.contentView, 1);
        candidateMenu = clickCandidate.menu;
        positionMenu = [candidateMenu itemAtIndex:1].submenu;
        NSMutableArray *operations = [NSMutableArray arrayWithObjects:[candidateMenu itemAtIndex:0], [candidateMenu itemAtIndex:2], nil];
        [operations addObjectsFromArray:[positionMenu.itemArray subarrayWithRange:NSMakeRange(0, 5)]];
        [operations addObject:[positionMenu itemAtIndex:6]];
        for (NSMenuItem *operation in operations) {
            NSUInteger calls = session.maintenanceCalls;
            [NSApp sendAction:operation.action to:operation.target from:operation];
            assert(session.maintenanceCalls == calls + 1 && session.maintenanceAction == operation.tag);
            assert(session.selectedGeneration == 2 && session.selectedIndex == 1);
        }
        // 置顶 is one of the operations above, so the loop leaves 布局 pinned. That reorders every later
        // render in this function - including the external-skin block, which reads the first button as the
        // highlighted one - so undo it here rather than at the end. The loop is checking that each menu
        // item dispatches maintenance, not setting up state for anybody else.
        [NSUserDefaults.standardUserDefaults removeObjectForKey:@"MSIMEClientPinnedCandidates"];
        [controller renderCandidates];
        // renderCandidates rebuilds the buttons, so everything captured from the previous panel is stale.
        clickCandidate = (id)PageButton(layoutPanel.contentView, 1);
        candidateMenu = clickCandidate.menu;
        assert(clickCandidate && candidateMenu.numberOfItems == 3);
        NSMenuItem *retainedOperation = [candidateMenu itemAtIndex:0];
        NSDictionary *validContext = retainedOperation.representedObject;
        NSUInteger maintenanceCalls = session.maintenanceCalls;
        retainedOperation.enabled = NO;
        [controller candidateMenuAction:retainedOperation];
        retainedOperation.enabled = YES;
        layoutPanel.requestedVisible = NO;
        [controller candidateMenuAction:retainedOperation];
        layoutPanel.requestedVisible = YES;
        for (NSString *field in @[@"session", @"generation", @"focused"]) {
            NSMutableDictionary *stale = [pageView mutableCopy];
            stale[field] = [field isEqual:@"focused"] ? @NO : @99;
            [controller setValue:stale forKey:@"view"];
            [controller candidateMenuAction:retainedOperation];
        }
        [controller setValue:pageView forKey:@"view"];
        for (NSString *field in @[@"session", @"generation", @"index"]) {
            NSMutableDictionary *badID = [validContext[@"id"] mutableCopy];
            [badID removeObjectForKey:field];
            retainedOperation.representedObject = @{@"id":badID, @"render":validContext[@"render"]};
            [controller candidateMenuAction:retainedOperation];
        }
        retainedOperation.representedObject = validContext;
        assert(session.maintenanceCalls == maintenanceCalls);
        for (NSString *text in @[@"中", @"𠀀", @"测试"]) {
            NSMenu *menu = [controller menuForCandidate:@{@"id":clickCandidate.candidateID, @"text":text}];
            assert(menu.numberOfItems == ([text isEqual:@"测试"] ? 3 : 2));
        }
        NSDictionary *validClickID = clickCandidate.candidateID;
        NSUInteger selectedCalls = session.selectCalls;
        [controller selectCandidate:clickCandidate];
        assert(session.selectCalls == selectedCalls + 1 && session.selectedGeneration == 2 && session.selectedIndex == 1);
        for (NSString *field in @[@"session", @"generation", @"index"]) {
            for (id invalid in @[@(-1), @YES, @1.5, @"1", NSNull.null, @"missing"]) {
                NSMutableDictionary *bad = [validClickID mutableCopy];
                bad[field] = invalid;
                if ([invalid isEqual:@"missing"]) [bad removeObjectForKey:field];
                clickCandidate.candidateID = bad;
                [controller selectCandidate:clickCandidate];
                assert(session.selectCalls == selectedCalls + 1);
            }
        }
        clickCandidate.candidateID = validClickID;
        for (NSString *field in @[@"session", @"generation", @"focused"]) {
            NSMutableDictionary *staleView = [pageView mutableCopy];
            staleView[field] = [field isEqual:@"focused"] ? @NO : @99;
            [controller setValue:staleView forKey:@"view"];
            [controller selectCandidate:clickCandidate];
            assert(session.selectCalls == selectedCalls + 1);
        }
        [controller setValue:pageView forKey:@"view"];
        layoutPanel.requestedVisible = NO;
        [controller selectCandidate:clickCandidate];
        assert(session.selectCalls == selectedCalls + 1);
        layoutPanel.requestedVisible = YES;
        clickCandidate.enabled = NO;
        [controller selectCandidate:clickCandidate];
        assert(session.selectCalls == selectedCalls + 1);
        clickCandidate.enabled = YES;
        [controller renderCandidates];
        [controller selectCandidate:clickCandidate];
        assert(session.selectCalls == selectedCalls + 1); // Detached button from an earlier render.
        [controller candidateMenuAction:retainedOperation];
        assert(session.maintenanceCalls == maintenanceCalls);
        appearance.vertical = NO;
        session.lastCommand = UINT32_MAX;
        [controller appearanceChanged:nil];
        assert(session.lastCommand == UINT32_MAX);
        assert(layoutPanel.frame.size.height < verticalHeight);
        NSView *first = layoutPanel.contentView.subviews[0];
        NSView *second = layoutPanel.contentView.subviews[1];
        assert(first.frame.origin.y == second.frame.origin.y);
        assert(NSMaxX(first.frame) == NSMinX(second.frame));
        CGFloat normalHeight = layoutPanel.frame.size.height;
        appearance.fontSize = 20;
        [controller appearanceChanged:nil];
        assert(layoutPanel.frame.size.height > normalHeight);
        // Palette and native drawing coverage: every global theme, two layouts and both appearances.
        NSDictionary *preservedView = [[controller valueForKey:@"view"] copy];
        session.lastCommand = UINT32_MAX;
        for (NSString *skinID in skinIDs) {
            appearance.globalTheme = skinID;
            for (NSNumber *vertical in @[@NO, @YES]) {
                appearance.vertical = vertical.boolValue;
                [controller appearanceChanged:nil];
                for (NSString *theme in @[NSAppearanceNameAqua, NSAppearanceNameDarkAqua]) {
                    MSIMECandidateChromeView *chrome = (id)layoutPanel.contentView;
                    chrome.appearance = [NSAppearance appearanceNamed:theme];
                    // Exercise the same callback AppKit uses for a system appearance change.
                    [chrome viewDidChangeEffectiveAppearance];
                    const BOOL dark = [theme isEqual:NSAppearanceNameDarkAqua];
                    const auto tokens = [appearance resolvedSkinForDark:dark].tokens;
                    assert([chrome.fillColor isEqual:SkinColor(tokens.surface)]);
                    assert([chrome.strokeColor isEqual:SkinColor(tokens.border)]);
                    assert(chrome.cornerRadius == tokens.radius && chrome.lineWidth == tokens.borderWidth);
                    assert(!chrome.isOpaque && !layoutPanel.isOpaque);
                    // By identity, not by position: a pinned candidate is drawn first whether or not it is
                    // the highlighted one, and 布局 is pinned by the menu loop above. Reading subviews[0]
                    // as "the selected one" asserted the ordering rather than the colouring.
                    MSIMECandidateButton *selected = CandidateButtonWithID(chrome, preservedView[@"candidates"][0][@"id"]);
                    MSIMECandidateButton *unselected = CandidateButtonWithID(chrome, preservedView[@"candidates"][1][@"id"]);
                    assert(selected && unselected);
                    assert(selected.candidateHighlighted && !unselected.candidateHighlighted);
                    assert([selected.fillColor isEqual:SkinColor(tokens.selected)]);
                    assert([selected.titleColor isEqual:SkinColor(tokens.selectedText)]);
                    assert([unselected.titleColor isEqual:SkinColor(tokens.text)]);
                    // On the selected fill the translation is the design's candSelTr, the selected number's colour.
                    assert([selected.translationColor isEqual:SkinColor(tokens.selectedNumber)]);
                    // A plain row's translation is the theme's secondary colour, which is the number colour (dc.html L2150, L2183): #9FB5A3 under 水杉, the platform's sub grey under 跟随系统.
                    assert([unselected.translationColor isEqual:SkinColor(tokens.number)]);
                    if ([skinID isEqual:@"shuishan"] || [skinID isEqual:@"system"]) {
                        NSColor *gloss = [unselected.translationColor colorUsingColorSpace:NSColorSpace.sRGBColorSpace];
                        const unsigned expected = [skinID isEqual:@"shuishan"] ? 0x9FB5A3 : dark ? 0x98989D : 0x6E6E73;
                        assert(fabs(gloss.redComponent - ((expected >> 16) & 0xFF) / 255.0) < 0.002);
                        assert(fabs(gloss.greenComponent - ((expected >> 8) & 0xFF) / 255.0) < 0.002);
                        assert(fabs(gloss.blueComponent - (expected & 0xFF) / 255.0) < 0.002);
                        assert(fabs(gloss.alphaComponent - 1.0) < 0.002);
                    }
                    // A text picker selects the custom theme over the one on screen, so every other slot stays.
                    appearance.candidateTextColor = @"#1234AB";
                    [controller refreshCandidateSkin];
                    assert([appearance.globalTheme isEqual:@"custom"]);
                    const auto picked = [appearance resolvedSkinForDark:dark].tokens;
                    NSColor *override = [NSColor colorWithSRGBRed:0x12 / 255.0 green:0x34 / 255.0 blue:0xAB / 255.0 alpha:1];
                    assert([SkinColor(picked.text) isEqual:override] && [unselected.titleColor isEqual:override]);
                    assert([unselected.translationColor isEqual:SkinColor(picked.number)]);
                    assert([selected.titleColor isEqual:SkinColor(picked.selectedText)]);
                    assert([chrome.fillColor isEqual:SkinColor(tokens.surface)]);
                    for (NSView *child in chrome.subviews)
                        if ([child.identifier isEqual:@"candidate-preedit"]) {
                            assert([((NSTextField *)child).textColor isEqual:SkinColor(picked.accent)]);
                            assert([((MSIMECandidatePreeditField *)child).caretColor isEqual:SkinColor(tokens.accent)]);
                        }
                    appearance.candidateTextColor = nil;
                    appearance.globalTheme = skinID;
                    [controller refreshCandidateSkin];
                    assert([unselected.titleColor isEqual:SkinColor(tokens.text)]);
                    assert([unselected.numberColor isEqual:SkinColor(tokens.number)]);
                    assert([unselected.translationColor isEqual:SkinColor(tokens.number)]);
                    assert(selected.showSelectedBar == tokens.showSelectedBar);
                    NSBitmapImageRep *bitmap = [chrome bitmapImageRepForCachingDisplayInRect:chrome.bounds];
                    assert(bitmap);
                    [chrome cacheDisplayInRect:chrome.bounds toBitmapImageRep:bitmap];
                    assert(bitmap.pixelsWide > 0 && bitmap.pixelsHigh > 0);
                    assert([[controller valueForKey:@"view"] isEqual:preservedView]);
                    assert(session.lastCommand == UINT32_MAX);
                    assert(!layoutPanel.canBecomeKeyWindow && !selected.acceptsFirstResponder);
                }
            }
        }
        TestExternalSkin(controller, layoutPanel, defaults);
        [controller setValue:appearance forKey:@"appearance"];
        appearance.vertical = NO;
        appearance.globalTheme = @"system";
        [controller appearanceChanged:nil];
        for (NSNumber *key in @[@123, @124, @125, @126]) {
            layoutPanel.requestedVisible = YES;
            session.lastCommand = UINT32_MAX;
            NSEvent *event = [NSEvent keyEventWithType:NSEventTypeKeyDown location:NSZeroPoint modifierFlags:0 timestamp:0 windowNumber:0 context:nil characters:@"" charactersIgnoringModifiers:@"" isARepeat:NO keyCode:key.unsignedShortValue];
            assert([controller handleEvent:event client:client]);
            uint32_t expected = key.unsignedShortValue == 123 ? MSIME_PREVIOUS_CANDIDATE : key.unsignedShortValue == 124 ? MSIME_NEXT_CANDIDATE : UINT32_MAX;
            assert(session.lastCommand == expected);
        }
        CheckMenu([controller menu], controller);
        // Pairing sends a shifted `{` down the paired-punctuation route; this loop is about page keys versus typed keys only.
        const BOOL pairedBeforePaging = appearance.pairedPunctuation;
        appearance.pairedPunctuation = NO;
        for (NSInteger option = 0; option < 3; ++option) {
            appearance.pageShortcut = option;
            NSArray *plain = @[@"-", @"=", @"[", @"]"];
            NSArray *shifted = @[@"_", @"+", @"{", @"}"];
            NSArray *physicalCodes = @[@27, @24, @33, @30];
            for (NSUInteger i = 0; i < plain.count; ++i) {
                for (NSNumber *visible in @[@NO, @YES]) {
                    for (NSNumber *shift in @[@NO, @YES]) {
                        layoutPanel.requestedVisible = visible.boolValue;
                        session.lastCommand = UINT32_MAX;
                        session.asciiCalls = 0;
                        NSString *characters = shift.boolValue ? shifted[i] : plain[i];
                        NSEvent *event = [NSEvent keyEventWithType:NSEventTypeKeyDown location:NSZeroPoint modifierFlags:shift.boolValue ? NSEventModifierFlagShift : 0 timestamp:0 windowNumber:0 context:nil characters:characters charactersIgnoringModifiers:plain[i] isARepeat:NO keyCode:[physicalCodes[i] unsignedShortValue]];
                        assert([controller handleEvent:event client:client]);
                        BOOL paging = visible.boolValue && !shift.boolValue && ((option == 0 && i < 2) || (option == 1 && i >= 2));
                        if (paging) {
                            assert(session.lastCommand == (i % 2 == 0 ? MSIME_PREVIOUS_PAGE : MSIME_NEXT_PAGE));
                            assert(session.asciiCalls == 0);
                        } else {
                            assert(session.lastCommand == UINT32_MAX && session.asciiCalls == 1);
                            assert(session.lastASCII == [characters characterAtIndex:0] && session.lastShift == shift.boolValue);
                        }
                    }
                }
            }
            for (NSNumber *key in @[@116, @121]) {
                layoutPanel.requestedVisible = YES;
                NSEvent *event = [NSEvent keyEventWithType:NSEventTypeKeyDown location:NSZeroPoint modifierFlags:NSEventModifierFlagShift timestamp:0 windowNumber:0 context:nil characters:@"" charactersIgnoringModifiers:@"" isARepeat:NO keyCode:key.unsignedShortValue];
                assert([controller handleEvent:event client:client]);
                assert(session.lastCommand == (key.unsignedShortValue == 116 ? MSIME_PREVIOUS_PAGE : MSIME_NEXT_PAGE));
            }
        }
        appearance.pairedPunctuation = pairedBeforePaging;
        // Japanese owns the physical ANSI minus/equal keys even when the
        // active layout reports different characters. They must reach Engine
        // instead of becoming candidate-page shortcuts.
        NSMutableDictionary *japanesePagingView = [pageView mutableCopy];
        japanesePagingView[@"scheme"] = @3;
        japanesePagingView[@"local_mode"] = @"none";
        [controller setValue:japanesePagingView forKey:@"view"];
        [controller renderCandidates];
        layoutPanel.requestedVisible = YES;
        for (NSDictionary *fixture in @[@{@"code": @27, @"character": @"-", @"layout": @"x"},
                                       @{@"code": @24, @"character": @"=", @"layout": @"x"}]) {
            session.lastCommand = UINT32_MAX;
            session.asciiCalls = 0;
            session.nextTransition = @{ @"handled": @YES, @"commit": NSNull.null, @"view": japanesePagingView };
            NSString *character = fixture[@"character"];
            NSEvent *event = [NSEvent keyEventWithType:NSEventTypeKeyDown location:NSZeroPoint modifierFlags:0 timestamp:0 windowNumber:0 context:nil characters:character charactersIgnoringModifiers:fixture[@"layout"] isARepeat:NO keyCode:[fixture[@"code"] unsignedShortValue]];
            assert([controller handleEvent:event client:client]);
            assert(session.lastCommand == UINT32_MAX && session.asciiCalls == 1 && session.lastASCII == [character characterAtIndex:0]);
        }
        // Unicode composition owns '+' even when it arrives from the
        // physical ANSI equal key; it must not become candidate paging.
        NSMutableDictionary *unicodePagingView = [pageView mutableCopy];
        unicodePagingView[@"local_mode"] = @"unicode";
        [controller setValue:unicodePagingView forKey:@"view"];
        [controller renderCandidates];
        layoutPanel.requestedVisible = YES;
        session.lastCommand = UINT32_MAX;
        session.asciiCalls = 0;
        NSEvent *unicodePlus = [NSEvent keyEventWithType:NSEventTypeKeyDown location:NSZeroPoint modifierFlags:0 timestamp:0 windowNumber:0 context:nil characters:@"+" charactersIgnoringModifiers:@"+" isARepeat:NO keyCode:24];
        // '+' is part of the U+ code-point sequence, so it must reach the Engine instead of paging the
        // panel. The contract is the next line - no paging command, one ASCII call carrying '+' - while the
        // return value just repeats whatever the Engine answered, and here it says it handled the key.
        assert([controller handleEvent:unicodePlus client:client]);
        assert(session.lastCommand == UINT32_MAX && session.asciiCalls == 1 && session.lastASCII == '+');
        // Selecting anything but the first candidate here is what Shift+digit is for: the unshifted
        // digits are the code point being typed. Without it the panel shows candidates the keyboard
        // cannot reach.
        {
            // The '+' above went through the Engine and left its answer in the view, so put the panel
            // back into Unicode composition before asking about its digits.
            [controller setValue:unicodePagingView forKey:@"view"];
            [controller renderCandidates];
            layoutPanel.requestedVisible = YES;
            NSUInteger selectCallsBeforeUnicode = session.selectCalls;
            NSUInteger asciiCallsBeforeUnicode = session.asciiCalls;
            NSEvent *shiftedDigit = [NSEvent keyEventWithType:NSEventTypeKeyDown location:NSZeroPoint modifierFlags:NSEventModifierFlagShift timestamp:0 windowNumber:0 context:nil characters:@"@" charactersIgnoringModifiers:@"2" isARepeat:NO keyCode:19];
            assert([controller handleEvent:shiftedDigit client:client]);
            assert(session.selectCalls == selectCallsBeforeUnicode + 1 && session.selectedIndex == 1);
            assert(session.asciiCalls == asciiCallsBeforeUnicode);
            // The unshifted digit stays hexadecimal input, which is the half that already worked.
            NSEvent *plainDigit = [NSEvent keyEventWithType:NSEventTypeKeyDown location:NSZeroPoint modifierFlags:0 timestamp:0 windowNumber:0 context:nil characters:@"2" charactersIgnoringModifiers:@"2" isARepeat:NO keyCode:19];
            assert([controller handleEvent:plainDigit client:client]);
            assert(session.selectCalls == selectCallsBeforeUnicode + 1);
            assert(session.asciiCalls == asciiCallsBeforeUnicode + 1 && session.lastASCII == '2');
        }
        [controller setValue:pageView forKey:@"view"];
        [controller renderCandidates];
        appearance.pageShortcut = 0;
        for (NSNumber *modifier in @[@(NSEventModifierFlagCommand), @(NSEventModifierFlagControl), @(NSEventModifierFlagOption)]) {
            layoutPanel.requestedVisible = YES;
            NSEvent *event = [NSEvent keyEventWithType:NSEventTypeKeyDown location:NSZeroPoint modifierFlags:modifier.unsignedIntegerValue timestamp:0 windowNumber:0 context:nil characters:@"=" charactersIgnoringModifiers:@"=" isARepeat:NO keyCode:0];
            assert(![controller handleEvent:event client:client]);
            assert(session.lastCommand == MSIME_FINISH_COMPOSITION);
        }
        NSDictionary *beforeWordView = [controller valueForKey:@"view"];
        NSDictionary *beforeWordTransition = session.nextTransition;
        // Both switches on at once, which the loop below deliberately avoids by turning paging off first.
        // They are alternatives - applyCloudSettingsSnapshot: refuses a cloud snapshot that puts paging on
        // the pair word-to-character holds - but nothing stopped a local toggle from setting both, and
        // paging used to win here and leave 以词定字 inert on the keys it was explicitly bound to.
        for (NSString *contested in @[@"brackets", @"minus_equal"]) {
            // Through the shared snapshot, not the setters. setWordCharacterEnabled:keys: and
            // setNavigation:enabled: each refuse a combination that collides, but
            // applySharedCandidatePreferences: takes both fields straight - and that is the path the Tauri
            // settings app writes through, so this is the state a user can actually end up in.
            [appearance applySharedCandidatePreferences:@{
                @"word_character": @{@"enabled": @YES, @"keys": contested},
                @"navigation": @{contested: @YES}}];
            assert([[appearance wordCharacterOptions][@"enabled"] boolValue] &&
                   [appearance navigationEnabled:contested]);
            NSDictionary *contestedView = @{@"session": @71, @"generation": @72, @"focused": @YES,
                @"editing_text": @"synthetic",
                @"candidates": @[@{@"text": @"合成", @"highlighted": @YES,
                    @"id": @{@"session": @71, @"generation": @72, @"index": @8}}]};
            for (NSUInteger edge = 0; edge < 2; ++edge) {
                [controller setValue:contestedView forKey:@"view"];
                layoutPanel.requestedVisible = YES;
                NSString *glyph = [contested isEqual:@"brackets"] ? (edge ? @"]" : @"[") : (edge ? @"=" : @"-");
                unsigned short code = [contested isEqual:@"brackets"] ? (edge ? 30 : 33) : (edge ? 24 : 27);
                NSEvent *event = [NSEvent keyEventWithType:NSEventTypeKeyDown location:NSZeroPoint modifierFlags:0 timestamp:0 windowNumber:0 context:nil characters:glyph charactersIgnoringModifiers:glyph isARepeat:NO keyCode:code];
                NSUInteger edges = session.edgeCalls;
                session.lastCommand = UINT32_MAX;
                session.nextTransition = @{@"handled": @NO, @"commit": NSNull.null, @"view": contestedView};
                assert([controller handleEvent:event client:client]);
                assert(session.edgeCalls == edges + 1 && session.lastEdge == edge);
                assert(session.lastCommand != MSIME_PREVIOUS_PAGE && session.lastCommand != MSIME_NEXT_PAGE);
            }
            // The other pair still pages, so the exclusion is about the keys word-to-character holds and
            // not about word-to-character being on at all.
            NSString *free = [contested isEqual:@"brackets"] ? @"minus_equal" : @"brackets";
            [appearance applySharedCandidatePreferences:@{@"navigation": @{free: @YES}}];
            [controller setValue:contestedView forKey:@"view"];
            layoutPanel.requestedVisible = YES;
            NSString *freeGlyph = [free isEqual:@"brackets"] ? @"[" : @"-";
            unsigned short freeCode = [free isEqual:@"brackets"] ? 33 : 27;
            NSUInteger edges = session.edgeCalls;
            session.lastCommand = UINT32_MAX;
            assert([controller handleEvent:[NSEvent keyEventWithType:NSEventTypeKeyDown location:NSZeroPoint modifierFlags:0 timestamp:0 windowNumber:0 context:nil characters:freeGlyph charactersIgnoringModifiers:freeGlyph isARepeat:NO keyCode:freeCode] client:client]);
            assert(session.lastCommand == MSIME_PREVIOUS_PAGE && session.edgeCalls == edges);
            [appearance applySharedCandidatePreferences:@{
                @"word_character": @{@"enabled": @NO, @"keys": contested},
                @"navigation": @{free: @NO, contested: @NO}}];
        }

        NSDictionary *savedEnginePunctuation = session.enginePunctuationTransition;
        NSDictionary *savedASCIIPunctuation = session.punctuationASCIITransition;
        for (NSString *keys in @[@"brackets", @"minus_equal"]) {
            [appearance setNavigation:keys enabled:NO];
            [appearance setWordCharacterEnabled:YES keys:keys];
            for (NSUInteger edge = 0; edge < 2; ++edge) {
                NSDictionary *edgeView = @{@"session": @71, @"generation": @72, @"focused": @YES, @"editing_text": @"synthetic",
                    @"candidates": @[@{@"text": @"合成", @"highlighted": @YES, @"id": @{@"session": @71, @"generation": @72, @"index": @8}}]};
                [controller setValue:edgeView forKey:@"view"];
                layoutPanel.requestedVisible = YES;
                [controller renderCandidates];
                NSString *character = [keys isEqual:@"brackets"] ? (edge ? @"]" : @"[") : (edge ? @"=" : @"-");
                unsigned short physicalKey = [keys isEqual:@"brackets"] ? (edge ? 30 : 33) : (edge ? 24 : 27);
                NSEvent *event = [NSEvent keyEventWithType:NSEventTypeKeyDown location:NSZeroPoint modifierFlags:0 timestamp:0 windowNumber:0 context:nil characters:character charactersIgnoringModifiers:character isARepeat:NO keyCode:physicalKey];
                NSUInteger calls = session.edgeCalls;
                NSUInteger punctuationCalls = session.enginePunctuationCalls;
                NSUInteger asciiPunctuationCalls = session.punctuationASCIICalls;
                // Word-to-character owns this key while enabled; selecting the Han edge is the
                // complete action and must not fall through to punctuation.
                session.punctuationASCIITransition = nil;
                client.committed = nil;
                session.nextTransition = @{@"handled": @NO, @"commit": NSNull.null, @"view": edgeView};
                assert([controller handleEvent:event client:client]);
                assert(client.committed == nil);
                assert(session.edgeCalls == calls + 1 && session.lastEdge == edge && session.edgeGeneration == 72 && session.edgeIndex == 8);
                // Word-to-character owns the key for both outcomes. Even when the
                // Engine declines the edge selection, the key must not fall through
                // to punctuation handling.
                assert(session.enginePunctuationCalls == punctuationCalls &&
                       session.punctuationASCIICalls == asciiPunctuationCalls);
                // A Han edge the Engine accepts is the whole answer; no punctuation follows it.
                [controller setValue:edgeView forKey:@"view"];
                layoutPanel.requestedVisible = YES;
                punctuationCalls = session.enginePunctuationCalls;
                asciiPunctuationCalls = session.punctuationASCIICalls;
                session.nextTransition = @{@"handled": @YES, @"commit": @"合", @"view": edgeView};
                assert([controller handleEvent:event client:client]);
                assert(session.edgeCalls == calls + 2);
                assert(session.enginePunctuationCalls == punctuationCalls && session.punctuationASCIICalls == asciiPunctuationCalls);
                // An Engine failure (stale generation, closed session) swallows the key without manufacturing punctuation.
                [controller setValue:edgeView forKey:@"view"];
                layoutPanel.requestedVisible = YES;
                session.nextTransition = nil;
                assert([controller handleEvent:event client:client]);
                assert(session.edgeCalls == calls + 3);
                assert(session.enginePunctuationCalls == punctuationCalls && session.punctuationASCIICalls == asciiPunctuationCalls);
                session.nextTransition = @{@"handled": @NO, @"commit": NSNull.null, @"view": edgeView};
                // Matching glyphs from an unrelated physical key must not
                // activate word-to-character; Windows checks both VK and WCH.
                NSEvent *wrongPhysical = [NSEvent keyEventWithType:NSEventTypeKeyDown location:NSZeroPoint modifierFlags:0 timestamp:0 windowNumber:0 context:nil characters:character charactersIgnoringModifiers:character isARepeat:NO keyCode:0];
                NSUInteger wrongCalls = session.edgeCalls;
                session.asciiCalls = 0;
                [controller handleEvent:wrongPhysical client:client];
                assert(session.edgeCalls == wrongCalls && session.asciiCalls == 1);
                for (NSString *field in @[@"session", @"generation", @"index"]) {
                    for (id invalid in @[@(-1), @YES, @1.5, @"8", NSNull.null, @"missing"]) {
                        NSMutableDictionary *badID = [@{@"session": @71, @"generation": @72, @"index": @8} mutableCopy];
                        badID[field] = invalid;
                        if ([invalid isEqual:@"missing"]) [badID removeObjectForKey:field];
                        NSMutableDictionary *badView = [edgeView mutableCopy];
                        badView[@"candidates"] = @[@{@"text": @"合成", @"highlighted": @YES, @"id": badID}];
                        [controller setValue:badView forKey:@"view"];
                        layoutPanel.requestedVisible = YES;
                        NSUInteger before = session.edgeCalls;
                        NSUInteger punctuationBefore = session.enginePunctuationCalls + session.punctuationASCIICalls;
                        assert([controller handleEvent:event client:client] && session.edgeCalls == before);
                        assert(session.enginePunctuationCalls + session.punctuationASCIICalls == punctuationBefore);
                    }
                }
                for (NSString *field in @[@"session", @"generation", @"focused"]) {
                    NSMutableDictionary *staleView = [edgeView mutableCopy];
                    staleView[field] = [field isEqual:@"focused"] ? @NO : @99;
                    [controller setValue:staleView forKey:@"view"];
                    layoutPanel.requestedVisible = YES;
                    NSUInteger before = session.edgeCalls;
                    NSUInteger punctuationBefore = session.enginePunctuationCalls + session.punctuationASCIICalls;
                    assert([controller handleEvent:event client:client] && session.edgeCalls == before);
                    assert(session.enginePunctuationCalls + session.punctuationASCIICalls == punctuationBefore);
                }
            }
            [appearance setWordCharacterEnabled:NO keys:keys];
        }
        session.enginePunctuationTransition = savedEnginePunctuation;
        session.punctuationASCIITransition = savedASCIIPunctuation;
        // With traditional output on, the edge character comes from the converted phrase, as Windows takes ExtractHanCharacter(CandidateTextForOutput(word)): 头发+] is 髮 and 皇后+] is 后, where s2t of the Engine's lone 发 or 后 would give 發 or 後.
        [appearance setNavigation:@"brackets" enabled:NO];
        [appearance setWordCharacterEnabled:YES keys:@"brackets"];
        NSDictionary *chineseContext = @{@"scheme": @0, @"local_mode": @"none"};
        NSDictionary *emptyView = @{@"editing_text": @"", @"candidates": @[]};
        NSEvent *(^bracket)(BOOL) = ^NSEvent *(BOOL last) {
            NSString *glyph = last ? @"]" : @"[";
            return [NSEvent keyEventWithType:NSEventTypeKeyDown location:NSZeroPoint modifierFlags:0 timestamp:0 windowNumber:0 context:nil characters:glyph charactersIgnoringModifiers:glyph isARepeat:NO keyCode:last ? 30 : 33];
        };
        NSString *(^pressHeldEdge)(NSString *, NSString *, BOOL, NSString *, NSDictionary *) = ^NSString *(NSString *held, NSString *word, BOOL last, NSString *engineCommit, NSDictionary *context) {
            NSMutableDictionary *wordView = [@{@"session": @71, @"generation": @72, @"focused": @YES, @"editing_text": @"synthetic", @"scheme": @0, @"local_mode": @"none",
                @"candidates": @[@{@"text": word, @"highlighted": @YES, @"id": @{@"session": @71, @"generation": @72, @"index": @8}}]} mutableCopy];
            if (held) wordView[@"phrase_prefix"] = held;
            [controller setValue:wordView forKey:@"view"];
            layoutPanel.requestedVisible = YES;
            client.committed = nil;
            NSUInteger calls = session.edgeCalls;
            NSUInteger punctuationCalls = session.enginePunctuationCalls;
            session.nextTransition = @{@"handled": @YES, @"commit": engineCommit, @"commit_context": context, @"view": emptyView};
            assert([controller handleEvent:bracket(last) client:client]);
            assert(session.edgeCalls == calls + 1 && session.lastEdge == (last ? MSIME_LAST_HAN : MSIME_FIRST_HAN));
            assert(session.enginePunctuationCalls == punctuationCalls);
            return client.committed;
        };
        NSString *(^pressEdge)(NSString *, BOOL, NSString *, NSDictionary *) = ^NSString *(NSString *word, BOOL last, NSString *engineCommit, NSDictionary *context) {
            return pressHeldEdge(nil, word, last, engineCommit, context);
        };
        [controller selectTraditionalOutput:nil];
        assert([pressEdge(@"头发", YES, @"发", chineseContext) isEqual:@"髮"]);
        // A phrase piece already chosen with phrase_preedit is held in phrase_prefix, and the runtime commits it in front of the edge (held + edge); the override keeps it and converts it with the edge: haitanpaobu, pick 海滩, ] on 跑步 gives 海灘步.
        assert([pressHeldEdge(@"海滩", @"跑步", YES, @"海滩步", chineseContext) isEqual:@"海灘步"]);
        assert([pressHeldEdge(@"你好", @"头发", YES, @"你好发", chineseContext) isEqual:@"你好髮"]);
        assert([pressEdge(@"皇后", YES, @"后", chineseContext) isEqual:@"后"]);
        assert([pressEdge(@"干杯", NO, @"干", chineseContext) isEqual:@"乾"]);
        assert([pressEdge(@"面条", NO, @"面", chineseContext) isEqual:@"麪"]);
        assert([pressEdge(@"头发", NO, @"头", chineseContext) isEqual:@"頭"]);
        // Japanese and Unicode commits keep the Engine's text untouched, the same rule apply: uses for every commit.
        assert([pressEdge(@"头发", YES, @"发", @{@"scheme": @0, @"local_mode": @"temporary_japanese"}) isEqual:@"发"]);
        assert([pressEdge(@"头发", YES, @"发", @{@"scheme": @0, @"local_mode": @"unicode"}) isEqual:@"发"]);
        [controller selectSimplifiedOutput:nil];
        assert([pressEdge(@"头发", YES, @"发", chineseContext) isEqual:@"发"]);
        assert([pressHeldEdge(@"你好", @"头发", YES, @"你好发", chineseContext) isEqual:@"你好发"]);
        assert([pressEdge(@"皇后", YES, @"后", chineseContext) isEqual:@"后"]);
        [appearance setWordCharacterEnabled:NO keys:@"brackets"];
        [controller setValue:beforeWordView forKey:@"view"];
        session.nextTransition = beforeWordTransition;
        appearance.pageShortcut = 0;
        // Paging is routed by physical key code, not by the glyph the layout produces, so comma and period
        // need theirs - 43 and 47. With 0 they could only ever fall through to ASCII, which is what the
        // disabled half of this loop asserts, so both halves were passing for the same wrong reason.
        // The last element is what the key does once its shortcut is off, which is not the same for all of them: comma and period are ordinary characters and go to the Engine, while Tab, Page Up/Page Down and Up/Down are navigation keys that Windows routes to the server whenever candidates are showing (CompositionProcessorEngine.cpp), where a disabled one gets NavigationIgnored - so with the panel up they are consumed with no command at all, neither a page move nor FINISH_COMPOSITION, and the composition stays.
        for (NSArray *entry in @[@[@"comma_period", @",", @43, @(MSIME_PREVIOUS_PAGE), @1],
                                 @[@"comma_period", @".", @47, @(MSIME_NEXT_PAGE), @1],
                                 @[@"tab", @"\t", @48, @(MSIME_NEXT_PAGE), @0],
                                 @[@"page_up_down", @"", @116, @(MSIME_PREVIOUS_PAGE), @0],
                                 @[@"page_up_down", @"", @121, @(MSIME_NEXT_PAGE), @0],
                                 @[@"arrows", @"", @126, @(MSIME_PREVIOUS_CANDIDATE), @0],
                                 @[@"arrows", @"", @125, @(MSIME_NEXT_CANDIDATE), @0]]) {
            for (NSNumber *enabled in @[@NO, @YES]) {
                [appearance applySharedCandidatePreferences:@{@"navigation": @{entry[0]:enabled}}];
                layoutPanel.requestedVisible = YES;
                appearance.vertical = YES;
                session.lastCommand = UINT32_MAX;
                session.asciiCalls = 0;
                NSEvent *event = [NSEvent keyEventWithType:NSEventTypeKeyDown location:NSZeroPoint modifierFlags:0 timestamp:0 windowNumber:0 context:nil characters:entry[1] charactersIgnoringModifiers:entry[1] isARepeat:NO keyCode:[entry[2] unsignedShortValue]];
                BOOL handled = [controller handleEvent:event client:client];
                if (enabled.boolValue) assert(handled && session.lastCommand == [entry[3] unsignedIntValue]);
                else {
                    // Turned off means no paging command, whatever else happens to the key.
                    assert(session.lastCommand == UINT32_MAX);
                    assert(session.asciiCalls == [entry[4] unsignedIntegerValue]);
                    if (![entry[4] unsignedIntegerValue]) assert(handled);
                }
            }
        }
        layoutPanel.requestedVisible = YES;
        NSEvent *reverseTab = [NSEvent keyEventWithType:NSEventTypeKeyDown location:NSZeroPoint modifierFlags:NSEventModifierFlagShift timestamp:0 windowNumber:0 context:nil characters:@"\t" charactersIgnoringModifiers:@"\t" isARepeat:NO keyCode:48];
        assert([controller handleEvent:reverseTab client:client] && session.lastCommand == MSIME_PREVIOUS_PAGE);
        // Shift+Tab with tab paging off is eaten while the panel is up, like plain Tab above.
        [appearance applySharedCandidatePreferences:@{@"navigation": @{@"tab": @NO}}];
        layoutPanel.requestedVisible = YES;
        session.lastCommand = UINT32_MAX;
        session.asciiCalls = 0;
        assert([controller handleEvent:reverseTab client:client] && session.lastCommand == UINT32_MAX && session.asciiCalls == 0);
        [appearance applySharedCandidatePreferences:@{@"navigation": @{@"tab": @YES}}];
        // With no candidates showing, Tab goes back to the application.
        layoutPanel.requestedVisible = NO;
        session.lastCommand = UINT32_MAX;
        assert(![controller handleEvent:reverseTab client:client] && session.lastCommand == UINT32_MAX);
        // Script selection changes only native display/commit strings, not Engine state or IDs.
        assert(!appearance.traditionalOutput);
        assert([MSIMEChineseOutputString(@"汉语", YES) isEqual:@"漢語"]);
        assert([MSIMEChineseOutputString(@"汉语", NO) isEqual:@"汉语"]);
        assert([CandidateDisplay(@{@"text": @"汉语", @"annotation": @"(aB)"}, YES) isEqual:@"漢語(aB)"]);
        assert([CandidateDisplay(@{@"text": @"汉语", @"annotation": NSNull.null}, NO) isEqual:@"汉语"]);
        assert(([CandidateDisplay(@{@"text":@"汉语", @"corrected":@YES, @"annotation":@"(aB)", @"source":@2}, YES) isEqual:@"漢語*(aB) ☁️"]));
        for (id corrected in @[@NO, @1, @"true", NSNull.null])
            assert(([CandidateDisplay(@{@"text":@"汉语", @"corrected":corrected}, NO) isEqual:@"汉语"]));
        for (id source in @[@0, @1, @4, @255, @(-1), @YES, @2.0, @"2", NSNull.null])
            assert(([CandidateDisplay(@{@"text":@"汉语", @"source":source}, NO) isEqual:@"汉语"]));
        assert(([CandidateDisplay(@{@"text":@"汉语", @"annotation":@"(aB)", @"source":@2}, YES) isEqual:@"漢語(aB) ☁️"]));
        assert(([CandidateDisplay(@{@"text":@"汉语", @"annotation":@"(aB)", @"source":@3}, NO) isEqual:@"汉语(aB) 🤖"]));
        NSMutableDictionary *scriptView = [@{@"scheme": @0, @"local_mode": @"none", @"session": @1, @"generation": @20, @"editing_text": @"hanyu", @"caret_position": @5, @"candidates": @[@{@"text": @"汉语", @"highlighted": @YES, @"id": @{@"session": @1, @"generation": @20, @"index": @0}}]} mutableCopy];
        [controller setValue:[scriptView copy] forKey:@"view"];
        NSDictionary *preserved = [[controller valueForKey:@"view"] copy];
        [controller selectTraditionalOutput:nil];
        [controller appearanceChanged:nil];
        assert([controller.menu itemAtIndex:5].state == NSControlStateValueOn);
        assert([[[MSIMEAppearancePreferences alloc] initWithDefaults:defaults skinsRoot:appearance.skinsRoot] traditionalOutput]);
        MSIMECandidateButton *scriptButton = PageButton(layoutPanel.contentView, 0);
        assert([scriptButton.toolTip isEqual:@"漢語"] && [scriptButton.title containsString:@"漢語"]);
        assert([scriptButton.candidateID isEqual:scriptView[@"candidates"][0][@"id"]]);
        assert([[controller valueForKey:@"view"] isEqual:preserved]);
        for (NSNumber *traditional in @[@NO, @YES]) {
            [controller applySharedToolbarPreferences:@{@"traditional_chinese_output":traditional}];
            [controller renderCandidates];
            scriptButton = PageButton(layoutPanel.contentView, 0);
            assert([scriptButton.toolTip isEqual:traditional.boolValue ? @"漢語" : @"汉语"]);
            assert([scriptButton.candidateID isEqual:scriptView[@"candidates"][0][@"id"]]);
            assert([[controller valueForKey:@"view"] isEqual:preserved]);
        }
        NSMutableDictionary *annotated = [scriptView mutableCopy];
        NSMutableDictionary *word = [scriptView[@"candidates"][0] mutableCopy];
        word[@"annotation"] = @"(aB)";
        annotated[@"candidates"] = @[word];
        [controller setValue:annotated forKey:@"view"];
        [controller renderCandidates];
        scriptButton = PageButton(layoutPanel.contentView, 0);
        // The annotation is drawn as its own run after the candidate text; the tooltip keeps the combined form.
        assert([scriptButton.title containsString:@"漢語"] && ![scriptButton.title containsString:@"(aB)"] && [scriptButton.annotation isEqual:@"(aB)"]);
        assert([scriptButton.toolTip isEqual:@"漢語(aB)"]);
        assert([scriptButton.candidateID isEqual:word[@"id"]]);
        assert([word[@"text"] isEqual:@"汉语"]);
        for (NSNumber *vertical in @[@NO, @YES]) {
            appearance.vertical = vertical.boolValue;
            [controller renderCandidates];
            NSSize originalSize = PageButton(layoutPanel.contentView, 0).frame.size;
            CGFloat originalPanelHeight = layoutPanel.frame.size.height;
            word[@"translation"] = @"synthetic glossary";
            [controller renderCandidates];
            MSIMECandidateButton *translated = PageButton(layoutPanel.contentView, 0);
            assert([translated.translation isEqual:@"synthetic glossary"] && translated.translationBelow == !vertical.boolValue);
            assert(fabs(translated.translationFont.pointSize - MSIMECandidateTranslationPointSize) < 0.01);
            assert([translated.toolTip containsString:@"\nsynthetic glossary"] && [translated.candidateID isEqual:word[@"id"]]);
            // Vertical puts the gloss on the candidate's own line, so it costs width and never height, and the row does not jump when the debounced gloss arrives (Windows test_layout.cpp vertical_candidate_translation_stays_on_the_same_line); horizontal stacks the gloss underneath, and that height is already reserved, so the row does not change either.
            if (vertical.boolValue) assert(translated.frame.size.width > originalSize.width && fabs(translated.frame.size.height - originalSize.height) < 0.01 && fabs(layoutPanel.frame.size.height - originalPanelHeight) < 0.01);
            else assert(fabs(translated.frame.size.height - originalSize.height) < 0.01);
            NSBitmapImageRep *bitmap = [translated bitmapImageRepForCachingDisplayInRect:translated.bounds];
            assert(bitmap);
            [translated cacheDisplayInRect:translated.bounds toBitmapImageRep:bitmap];
            [word removeObjectForKey:@"translation"];
        }
        // The reservation is real, not just an absent assertion: with the feature off the horizontal row is
        // shorter than with it on and nothing to show yet. That difference is the space the gloss lands in.
        appearance.vertical = NO;
        [controller renderCandidates];
        const CGFloat reservedHeight = PageButton(layoutPanel.contentView, 0).frame.size.height;
        const BOOL previousTranslations = appearance.candidateTranslations;
        const BOOL previousGloss = appearance.candidateEnglishGloss;
        appearance.candidateTranslations = NO;
        appearance.candidateEnglishGloss = NO;
        [controller renderCandidates];
        assert(PageButton(layoutPanel.contentView, 0).frame.size.height < reservedHeight);
        appearance.candidateTranslations = previousTranslations;
        appearance.candidateEnglishGloss = previousGloss;
        [controller renderCandidates];
        word[@"corrected"] = @YES;
        NSUInteger fixedCase = 0;
        for (id fixed in @[@0, @1, @5, @(-1), @256, @YES, @1.0, @"1", NSNull.null]) {
            word[@"fixed_position"] = fixed;
            const BOOL valid = fixedCase == 1 || fixedCase == 2;
            ++fixedCase;
            for (NSNumber *highlighted in @[@NO, @YES]) {
                word[@"highlighted"] = highlighted;
                [controller renderCandidates];
                scriptButton = PageButton(layoutPanel.contentView, 0);
                assert(scriptButton.candidateFixed == valid);
                assert([scriptButton.title containsString:@"漢語*"] && [scriptButton.annotation isEqual:@"(aB)"] && [scriptButton.candidateID isEqual:word[@"id"]]);
                if (valid) {
                    NSColor *color = [scriptButton.titleColor colorUsingColorSpace:NSColorSpace.sRGBColorSpace];
                    assert(fabs(color.redComponent - 55.0/255) < 0.001 && fabs(color.greenComponent - 154.0/255) < 0.001 && fabs(color.blueComponent - 211.0/255) < 0.001);
                    assert([scriptButton.translationColor isEqual:[color colorWithAlphaComponent:MSIMECandidateTranslationOpacity]]);
                    [controller refreshCandidateSkin];
                    assert([scriptButton.titleColor isEqual:color]);
                    assert([scriptButton.translationColor isEqual:[color colorWithAlphaComponent:MSIMECandidateTranslationOpacity]]);
                }
                assert([word[@"text"] isEqual:@"汉语"]);
            }
        }
        [word removeObjectForKey:@"corrected"];
        [word removeObjectForKey:@"fixed_position"];
        for (NSNumber *source in @[@2, @3]) {
            word[@"source"] = source;
            NSDictionary *unchanged = [word copy];
            for (NSNumber *vertical in @[@NO, @YES]) {
                appearance.vertical = vertical.boolValue;
                [controller renderCandidates];
                scriptButton = PageButton(layoutPanel.contentView, 0);
                NSString *expected = source.integerValue == 2 ? @"漢語(aB) ☁️" : @"漢語(aB) 🤖";
                // The badge belongs to the text run, as in the Windows presenter; the annotation follows it.
                NSString *textRun = source.integerValue == 2 ? @"漢語 ☁️" : @"漢語 🤖";
                assert([scriptButton.title containsString:textRun] && [scriptButton.annotation isEqual:@"(aB)"] && [scriptButton.toolTip isEqual:expected]);
                assert([scriptButton.candidateID isEqual:word[@"id"]] && [word isEqual:unchanged]);
                assert(scriptButton.frame.size.width > 0 && scriptButton.frame.size.height > 0);
            }
        }
        NSUInteger contextIndex = 0;
        for (NSDictionary *context in @[@{@"scheme": @0, @"local_mode": @"none"}, @{@"scheme": @1, @"local_mode": @"quick_phrase"}, @{@"scheme": @3, @"local_mode": @"none"}, @{@"scheme": @0, @"local_mode": @"unicode"}, @{@"scheme": @0, @"local_mode": @"temporary_japanese"}, @{@"scheme": @1, @"local_mode": @"temporary_japanese"}, @{}]) {
            BOOL convert = contextIndex++ < 2;
            assert(MSIMEScriptConversionApplies(context) == convert);
            NSMutableDictionary *candidateView = [scriptView mutableCopy];
            [candidateView addEntriesFromDictionary:context];
            if (context.count == 0) [candidateView removeObjectForKey:@"scheme"];
            [controller setValue:candidateView forKey:@"view"];
            [controller renderCandidates];
            scriptButton = PageButton(layoutPanel.contentView, 0);
            assert([scriptButton.toolTip isEqual:convert ? @"漢語" : @"汉语"]);
            // Post-commit view has already reset its mode and may have applied another scheme.
            NSDictionary *transition = @{@"handled": @YES, @"commit": @"汉语", @"commit_context": context, @"view": @{@"scheme": @0, @"local_mode": @"none", @"editing_text": @"", @"candidates": @[]}};
            [controller apply:transition];
            assert([client.committed isEqual:convert ? @"漢語" : @"汉语"]);
            assert([transition[@"commit"] isEqual:@"汉语"]);
        }
        NSMutableDictionary *japaneseView = [scriptView mutableCopy];
        japaneseView[@"local_mode"] = @"temporary_japanese";
        japaneseView[@"candidates"] = @[@{@"text": @"日本国", @"highlighted": @YES, @"id": word[@"id"]}];
        [controller setValue:japaneseView forKey:@"view"];
        [controller renderCandidates];
        assert([PageButton(layoutPanel.contentView, 0).toolTip isEqual:@"日本国"]);
        assert([PageButton(layoutPanel.contentView, 0).candidateID isEqual:word[@"id"]]);
        [controller apply:@{@"commit": @"日本国", @"commit_context": @{@"scheme": @0, @"local_mode": @"temporary_japanese"},
                            @"view": @{@"scheme": @0, @"local_mode": @"none", @"editing_text": @"", @"candidates": @[]}}];
        assert([client.committed isEqual:@"日本国"]);
        [controller selectSimplifiedOutput:nil];
        assert([controller.menu itemAtIndex:4].state == NSControlStateValueOn);
        [controller apply:@{@"commit": @"汉语", @"commit_context": @{@"scheme": @0, @"local_mode": @"none"}, @"view": @{@"editing_text": @"", @"candidates": @[]}}];
        assert([client.committed isEqual:@"汉语"]);
        TestInputMode(defaults, appearance);
        TestControlOptionSpace();
        TestInputModePolicy();
        TestPerApplicationPunctuationAndWidth();
        TestEnglishModePunctuationAndWidthOutput();
        TestInputSourceModeReset();
        TestRealSessionComposition();
        TestModifierTaps();
        TestModifierTapSurvivesALostRelease();
        TestStaleClientDeactivation();
        TestPreferenceClientGeneration();
        TestPreferenceRevisionSkipsUnchangedDocuments();
        TestUnreadablePreferencesAreRecoveredOnce();
        TestProviderSettingsPersistTheSharedSnapshot();
        TestFullWidth(defaults, appearance);
        TestSessionOptions();
        TestKeypadDecimal(appearance);
        TestFloatingToolbarMenuToggle(appearance);
        TestCandidatePanelSingleOwner();
        TestGlossArrivalRedrawsOnlyTheCard(appearance);
        TestNestedApplyKeepsTheNewerView(appearance);
        TestJapaneseConversionKeys(appearance);
        TestGlossSensePage(appearance);
        TestGlossSenseTraditionalOutput(appearance);
        TestSegmentEditingChords(appearance);
        TestBackspaceHoldDoesNotEscapeComposition();
        TestPassthroughKeysAreCounted();
        TestKeyLatencyIsLoggedWithoutTheKey();
        TestKeypadOperators(appearance);
        TestSmartPunctuationPreferences();
        TestSharedCharacterWidth();
        TestScreenKeyboardShortcut(appearance);
        TestMaintenanceShortcuts(appearance);
        TestPunctuation(defaults, appearance);
        TestPairedPunctuationPreferences();
        TestPairedPunctuationHostExclusion();
        TestPairedPunctuationClosesThePair();
        TestPairedPunctuationClosesBrace();
        TestEmojiBridgeFallback();
        TestMixedInputPreferences();
        TestCharacterSetShortcut();
        TestDedicatedEnglish(appearance);
        TestSystemInputModeReport(appearance);
        TestKeymap(defaults, appearance);
        Method fontMethod = class_getClassMethod(NSFont.class, @selector(monospacedSystemFontOfSize:weight:));
        assert(fontMethod);
        originalMonospacedFont = method_setImplementation(fontMethod, (IMP)MissingKeyFont);
        TestKeymap(defaults, appearance);
        method_setImplementation(fontMethod, originalMonospacedFont);
        assert(missingKeyFontCalls > 0);
        [NSUserDefaults.standardUserDefaults removeObjectForKey:@"MSIMEClientPinnedCandidates"];
        MSIMERemoveTestPreferenceSuite(defaults, suite);
        if (previousVoiceHoldSpace) [standardDefaults setObject:previousVoiceHoldSpace forKey:@"MSIMEClientVoiceHotkeyHoldSpace"];
        else [standardDefaults removeObjectForKey:@"MSIMEClientVoiceHotkeyHoldSpace"];
    }
    return 0;
}
