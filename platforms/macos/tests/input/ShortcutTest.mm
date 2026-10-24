#import "../../src/input/InputController.mm"
#include "../../src/candidate/CandidatePageSize.h"
#import "../../src/input/InputSourceRegistration.h"
#import "../../src/candidate/SkinSettingsView.h"
#include <cassert>
#include <fstream>
#include <sqlite3.h>
#include <sys/resource.h>
#import <objc/runtime.h>
#import "../settings/TestPreferenceSuite.h"
#import "../settings/PreferenceViewLookup.h"

// 既有候选文字回归使用出厂徽标；包装只属于本测试，不让其他引入控制器的测试产生未使用函数。
static NSString *CandidateDisplay(NSDictionary *candidate, BOOL traditional) {
    return CandidateDisplayWithBadges(candidate, traditional,
        MSIMECandidateBadgeString(msime::mac::candidate_ai_badge("robot")),
        MSIMECandidateBadgeString(msime::mac::candidate_fixed_badge(true, "paperclip")));
}

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
    // "" is a separator and "*" a submenu row, the scheme and the theme, whose action AppKit sets to submenuAction:.
    NSArray<NSString *> *actions = @[
        @"selectChineseMode:", @"selectEnglishMode:", @"toggleDedicatedEnglishMode:", @"",
        @"toggleTraditionalOutput:", @"toggleFullWidthInput:", @"toggleChinesePunctuation:", @"toggleCandidateTranslations:", @"",
        @"*", @"*", @"",
        @"toggleFloatingToolbar:", @"showEmoji:", @"showCloudClipboard:", @"showCloudDictionary:", @"showScreenKeyboard:", @"showHandwriting:",
        @"showVoicePanel", @"", @"showAppearance:", @"showAbout:"
    ];
    assert(menu.numberOfItems == (NSInteger)actions.count && !menu.autoenablesItems);
    for (NSUInteger index = 0; index < actions.count; ++index) {
        NSMenuItem *item = [menu itemAtIndex:index];
        if (actions[index].length == 0) assert(item.separatorItem);
        else if ([actions[index] isEqual:@"*"]) assert(!item.separatorItem && item.hasSubmenu);
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
@property(nonatomic) BOOL capsLock;
@property(nonatomic) NSUInteger capsLockCalls;
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
// 旧的声音调用。生命周期核心接管之后控制器一次都不该再调它们，否则同一个键会响两遍；记下来好断言它们一直是空的。
@property(nonatomic, strong) NSMutableArray<NSNumber *> *keySounds;
@property(nonatomic) NSUInteger commitSounds;
@property(nonatomic, strong) NSMutableArray<NSNumber *> *musicStates;
// 生命周期调用和派发按发生顺序记在一起："traits:<十六进制>"、"key:<值>"、"active:<0|1>"、"focus:<0|1>"、"ascii"、"command:<值>"、"select"、"maintenance"，用来断言契约的调用顺序。
@property(nonatomic, strong) NSMutableArray<NSString *> *calls;
// lifecycleKey: 报告的物理按下（按键类别或 MSIME_KEY_POINTER），以及它要返回的打字特效答复。
@property(nonatomic, strong) NSMutableArray<NSNumber *> *keys;
@property(nonatomic) uint32_t keyEffect;
@property(nonatomic, strong) NSMutableArray<NSNumber *> *reportedTraits;
@property(nonatomic, strong) NSMutableArray<NSNumber *> *activeStates;
@end
@implementation ShortcutSession
- (void)recordCall:(NSString *)call {
    if (!self.calls) self.calls = [NSMutableArray array];
    [self.calls addObject:call];
}
- (uint32_t)lifecycleKey:(uint32_t)key {
    if (!self.keys) self.keys = [NSMutableArray array];
    [self.keys addObject:@(key)];
    [self recordCall:[NSString stringWithFormat:@"key:%u", key]];
    return self.keyEffect;
}
- (uint32_t)setFieldTraits:(uint32_t)traits {
    if (!self.reportedTraits) self.reportedTraits = [NSMutableArray array];
    [self.reportedTraits addObject:@(traits)];
    [self recordCall:[NSString stringWithFormat:@"traits:%x", traits]];
    return traits;
}
- (BOOL)setLifecycleActive:(BOOL)active {
    if (!self.activeStates) self.activeStates = [NSMutableArray array];
    [self.activeStates addObject:@(active)];
    [self recordCall:[NSString stringWithFormat:@"active:%d", active ? 1 : 0]];
    return YES;
}
- (BOOL)keySound:(uint32_t)keyClass {
    if (!self.keySounds) self.keySounds = [NSMutableArray array];
    [self.keySounds addObject:@(keyClass)];
    return YES;
}
- (BOOL)commitSound { ++self.commitSounds; return YES; }
- (BOOL)setMusicActive:(BOOL)active {
    if (!self.musicStates) self.musicStates = [NSMutableArray array];
    [self.musicStates addObject:@(active)];
    return YES;
}
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
// 停顿计时器调用这两个入口；回答已完成且顺序未变，避免计时器在其他用例中触发时重排候选。
- (NSDictionary *)startSettledRerankWithError:(NSError **)error {
    (void)error; ++self.settledRerankCalls; return @{@"status":@"done", @"moved":@NO, @"current":@YES};
}
- (NSDictionary *)pollSettledRerank:(uint64_t)token accept:(BOOL)accept error:(NSError **)error {
    (void)token; (void)accept; (void)error;
    return @{@"status":@"done", @"moved":@NO, @"current":@YES};
}
- (NSDictionary *)translationQueryWithError:(NSError **)error { (void)error; return nil; }
- (NSDictionary *)onlineQueryWithError:(NSError **)error { (void)error; return nil; }
- (NSDictionary *)typingEffectSettingsWithError:(NSError **)error { (void)error; return nil; }
- (NSDictionary *)setCharacterWidthFull:(BOOL)fullwidth error:(NSError **)error {
    (void)error; self.fullwidth = fullwidth; ++self.widthCalls; return nil;
}
- (BOOL)setCapsLockEnabled:(BOOL)enabled error:(NSError **)error {
    (void)error; self.capsLock = enabled; ++self.capsLockCalls; return YES;
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
    (void)error; [self recordCall:@"maintenance"]; ++self.maintenanceCalls; self.maintenanceAction = 0; self.selectedGeneration = generation; self.selectedIndex = index; return nil;
}
- (NSDictionary *)removeGeneration:(uint64_t)generation index:(NSUInteger)index error:(NSError **)error {
    (void)error; [self recordCall:@"maintenance"]; ++self.maintenanceCalls; self.maintenanceAction = 1; self.selectedGeneration = generation; self.selectedIndex = index; return nil;
}
- (NSDictionary *)clearPositionGeneration:(uint64_t)generation index:(NSUInteger)index error:(NSError **)error {
    (void)error; [self recordCall:@"maintenance"]; ++self.maintenanceCalls; self.maintenanceAction = 2; self.selectedGeneration = generation; self.selectedIndex = index; return nil;
}
- (NSDictionary *)fixGeneration:(uint64_t)generation index:(NSUInteger)index position:(uint8_t)position error:(NSError **)error {
    (void)error; [self recordCall:@"maintenance"]; ++self.maintenanceCalls; self.maintenanceAction = 10 + position; self.selectedGeneration = generation; self.selectedIndex = index; return nil;
}
- (NSDictionary *)selectGeneration:(uint64_t)generation index:(NSUInteger)index error:(NSError **)error {
    (void)error; ++self.selectCalls; self.selectedGeneration = generation; self.selectedIndex = index;
    [self recordCall:@"select"];
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
    [self recordCall:focused ? @"focus:1" : @"focus:0"];
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
    [self recordCall:@"ascii"];
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
    [self recordCall:[NSString stringWithFormat:@"command:%u", command]];
    if (command == MSIME_COMMIT_RAW) ++self.rawCommitCalls;
    // 命令 20 对会话同 MSIME_FINISH_COMPOSITION，只在生命周期里按失焦上屏处理。
    const BOOL finish = command == MSIME_FINISH_COMPOSITION || command == MSIME_FINISH_ON_FOCUS_LOSS;
    if (self.failFinish && finish) return nil;
    // 命令 19 对会话同 MSIME_CANCEL，只在生命周期里把结束原因记成 HostCommit。
    const BOOL cancel = command == MSIME_CANCEL || command == MSIME_CANCEL_AFTER_HOST_COMMIT;
    if (self.failCancel && cancel) return nil;
    if (self.finishTransition && finish) return self.finishTransition;
    if (self.cancelTransition && cancel) return self.cancelTransition;
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
// 对文档的同步读取次数：`selectedRange` 和 `attributedSubstringFromRange:`，每一次都要到应用里来回一趟。
@property(nonatomic) NSUInteger documentReads;
@property(nonatomic) NSUInteger attributeReads;
@end

@interface ModeController : MSIMEInputController
@property(nonatomic) NSUInteger preparationCalls;
// Plays secure event input, which is window-server state a test cannot turn on. Left at NO it also keeps a test from inheriting the machine's state: a locked screen or another process's password field turns it on for everyone.
@property(nonatomic) BOOL secureInput;
@property(nonatomic) NSUInteger paletteCalls;
@property(nonatomic) NSUInteger screenKeyboardCalls;
@property(nonatomic) NSUInteger restartCalls;
@property(nonatomic) NSUInteger terminationCalls;
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
    // 上面的换客户端按键让这个控制器成了焦点控制器（MSIMEFocusedController）。它在 main 的自动释放池里一直活着，635195ce0 起系统的模式报告和方案菜单项都转交给焦点控制器，不清掉的话后面 TestSystemInputModeReport 与 TestOptInSchemeModes 的报告都会落到这里。
    if (MSIMEFocusedController == controller) MSIMEFocusedController = nil;
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
    // Secure event input skips counting; ModeController keeps it off whatever the CI machine has on.
    ModeController *controller = [ModeController alloc];
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
- (NSRange)selectedRange { ++self.documentReads; return self.selection; }
- (NSAttributedString *)attributedSubstringFromRange:(NSRange)range {
    ++self.documentReads;
    if (range.location == NSNotFound || range.location > self.document.length || range.length > self.document.length - range.location) return nil;
    return [[NSAttributedString alloc] initWithString:[self.document substringWithRange:range]];
}
- (NSDictionary *)attributesForCharacterIndex:(NSUInteger)index lineHeightRectangle:(NSRect *)rect {
    (void)index;
    ++self.attributeReads;
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
    // 路由用例只关心文字；属性与光标由 TextClientTest 的原生客户端回归覆盖。
    self.marked = [text isKindOfClass:NSAttributedString.class] ? [text string] : text;
}
@end

// 控制器隐藏和显示候选窗时会设透明度（皮肤的进出场动效），所以假窗口也接住 alphaValue、animator 和 contentView；没有内容视图，动效也就无处可画。
@interface TestCandidatePanel : NSObject
@property(nonatomic, getter=isVisible) BOOL visible;
@property(nonatomic) CGFloat alphaValue;
@property(nonatomic, readonly) NSView *contentView;
@end
@implementation TestCandidatePanel
- (void)orderOut:(id)sender { (void)sender; self.visible = NO; }
- (instancetype)animator { return self; }
@end

@interface HiddenCandidatePanel : MSIMECandidatePanel
@property(nonatomic) BOOL requestedVisible;
@end
@implementation HiddenCandidatePanel
- (BOOL)isVisible { return self.requestedVisible; }
- (void)orderFrontRegardless { self.requestedVisible = YES; }
- (void)orderOut:(id)sender { (void)sender; self.requestedVisible = NO; }
@end

@implementation ModeController
- (void)prepareSession {
    ++self.preparationCalls;
    if ([self valueForKey:@"session"]) [super prepareSession];
}
- (BOOL)secureEventInputActive { return self.secureInput; }
- (void)showSystemCharacterPalette { ++self.paletteCalls; }
- (void)showScreenKeyboard:(id)sender { (void)sender; ++self.screenKeyboardCalls; }
- (void)restartCurrentInputMethod { ++self.restartCalls; }
- (void)terminateCurrentInputMethod { ++self.terminationCalls; }
@end

// 布局字符由夹具注入，避免测试依赖开发机的当前输入源。
@interface ShortcutLayoutEvent : NSEvent
@property(nonatomic, strong) NSEvent *baseEvent;
@property(nonatomic, copy) NSString *layoutCharacters;
@end
@implementation ShortcutLayoutEvent
- (NSEventType)type { return self.baseEvent.type; }
- (unsigned short)keyCode { return self.baseEvent.keyCode; }
- (NSEventModifierFlags)modifierFlags { return self.baseEvent.modifierFlags; }
- (NSString *)characters { return self.baseEvent.characters; }
- (NSString *)charactersIgnoringModifiers { return self.baseEvent.charactersIgnoringModifiers; }
- (BOOL)isARepeat { return self.baseEvent.isARepeat; }
- (NSTimeInterval)timestamp { return self.baseEvent.timestamp; }
- (CGEventRef)CGEvent { return self.baseEvent.CGEvent; }
- (NSString *)charactersByApplyingModifiers:(NSEventModifierFlags)modifiers {
    assert(modifiers == 0);
    return self.layoutCharacters;
}
@end

static NSEvent *WithLayoutCharacters(NSEvent *base, NSString *characters) {
    ShortcutLayoutEvent *event = [ShortcutLayoutEvent new];
    event.baseEvent = base;
    event.layoutCharacters = characters;
    return event;
}

static NSEvent *TapEvent(NSEventType type, unsigned short key, NSEventModifierFlags flags, double time);

static NSEvent *ModeKey(unsigned short code, NSEventModifierFlags flags, BOOL repeat) {
    // 字母键带上它在美式布局上打出的字母，和真实事件一样：字母快捷键按这个字符认键。
    const char letter = msime::mac::PhysicalAnsiLetter(code);
    NSString *characters = code == 49 ? @" " : letter ? [NSString stringWithFormat:@"%c", letter] : @"a";
    NSEvent *event = [NSEvent keyEventWithType:NSEventTypeKeyDown location:NSZeroPoint modifierFlags:flags timestamp:0 windowNumber:0 context:nil characters:characters charactersIgnoringModifiers:characters isARepeat:repeat keyCode:code];
    NSString *unmodified = @"";
    for (const auto &entry : msime::mac::detail::kToggleChordKeyCodes)
        if (entry.keyCode == code && entry.character) unmodified = [NSString stringWithFormat:@"%c", entry.character];
    return WithLayoutCharacters(event, unmodified);
}

@interface VimModeClient : ShortcutClient
@property(nonatomic, copy) NSString *bundleIdentifier;
@end
@implementation VimModeClient
@end

// 经真实事件路由核对：空闲 Esc 交给应用并切英文，组字中的第一次 Esc 只取消。
static void TestVimModeEscapeRouting(void) {
    for (NSUInteger scenario = 0; scenario < 8; ++scenario) {
        NSString *suite = [@"msime.vim-mode." stringByAppendingString:NSUUID.UUID.UUIDString];
        NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
        MSIMEAppearancePreferences *appearance = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
        [appearance applySharedInputPreferences:@{@"vim_mode_apps": @[@"org.example.Editor"]}];
        ModeController *controller = [ModeController alloc];
        VimModeClient *client = [VimModeClient new];
        client.bundleIdentifier = scenario == 1 ? @"org.example.Other" : @"ORG.EXAMPLE.EDITOR";
        ShortcutSession *session = [ShortcutSession new];
        NSDictionary *empty = @{@"editing_text": @"", @"candidates": @[]};
        session.nextTransition = @{@"handled": @NO, @"view": empty};
        [controller setValue:appearance forKey:@"appearance"];
        [controller setValue:client forKey:@"activeClient"];
        [controller setValue:session forKey:@"session"];
        [controller setValue:scenario == 7 ? @{@"editing_text": @"ni", @"candidates": @[]} : empty forKey:@"view"];
        appearance.englishMode = NO;
        const NSEventModifierFlags flags[] = {0, 0, NSEventModifierFlagShift,
            NSEventModifierFlagControl, NSEventModifierFlagOption, NSEventModifierFlagCommand, 0, 0};
        if (scenario == 7) session.cancelTransition = @{@"handled": @YES, @"view": empty};
        const BOOL handled = [controller handleEvent:ModeKey(53, flags[scenario], scenario == 6) client:client];
        assert(appearance.englishMode == (scenario == 0));
        if (scenario == 0) assert(!handled && client.committed.length == 0);
        if (scenario == 7) {
            assert(handled);
            session.cancelTransition = nil;
            assert(![controller handleEvent:ModeKey(53, 0, NO) client:client]);
            assert(appearance.englishMode);
        }
        MSIMERemoveTestPreferenceSuite(defaults, suite);
    }
}

// 非美式键盘布局上的按键：`code` 是物理位置，`characters` 是当前布局在这个键上打出的字符。
static NSEvent *LayoutKey(unsigned short code, NSString *characters, NSEventModifierFlags flags) {
    return WithLayoutCharacters([NSEvent keyEventWithType:NSEventTypeKeyDown location:NSZeroPoint modifierFlags:flags timestamp:0 windowNumber:0 context:nil characters:characters charactersIgnoringModifiers:characters isARepeat:NO keyCode:code], characters);
}

// 字母快捷键按当前键盘布局在这个键上打出的字母认键（#5552）。
static void TestShortcutLetterFollowsKeyboardLayout(void) {
    for (unsigned short code = 0; code < 128; ++code) {
        const char letter = msime::mac::PhysicalAnsiLetter(code);
        if (!letter) continue;
        // 美式布局：字符和物理位置一致，大写（Shift）折成小写。
        assert(msime::mac::ShortcutLetter(code, static_cast<unsigned short>(letter)) == letter);
        assert(msime::mac::ShortcutLetter(code, static_cast<unsigned short>(letter - 'a' + 'A')) == letter);
        // 没有字符的合成事件、布局给出非拉丁字母时按物理位置认。
        assert(msime::mac::ShortcutLetter(code, 0) == letter);
        assert(msime::mac::ShortcutLetter(code, 0x0430) == letter);
    }
    // Dvorak：物理 Y 键打出 f，物理 F 键打出 u。
    assert(msime::mac::ShortcutLetter(16, 'f') == 'f' && msime::mac::ShortcutLetter(16, 'F') == 'f');
    assert(msime::mac::ShortcutLetter(3, 'u') == 'u');
    // 布局在字母位置上放的是 ASCII 标点或数字时它不是字母快捷键，不能再按物理位置认成 E。
    assert(msime::mac::ShortcutLetter(14, '.') == '\0');
    assert(msime::mac::ShortcutLetter(12, '\'') == '\0');
    assert(msime::mac::ShortcutLetter(49, ' ') == '\0');
    assert(msime::mac::ShortcutLetter(18, '1') == '\0');
    const NSEventModifierFlags optionShift = NSEventModifierFlagOption | NSEventModifierFlagShift;
    assert(msime::mac::FullWidthInputToggleArm(38, msime::mac::ShortcutLetter(38, 'H'), optionShift,
                                             msime::mac::FactoryFullWidthToggleChord()) == msime::mac::ToggleFullWidthArm::optionShiftH);
    assert(msime::mac::FullWidthInputToggleArm(4, msime::mac::ShortcutLetter(4, 'D'), optionShift,
                                             msime::mac::FactoryFullWidthToggleChord()) == msime::mac::ToggleFullWidthArm::none);
    assert(msime::mac::MaintenanceShortcut(msime::mac::ShortcutLetter(34, 'C'), true, true, true, false) ==
           msime::mac::MaintenanceShortcutAction::ClearCache);
    assert(msime::mac::MaintenanceShortcut(msime::mac::ShortcutLetter(8, 'J'), true, true, true, false) ==
           msime::mac::MaintenanceShortcutAction::None);
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

// 原生窗口的「智能标点」开关被用户切换时，数字后直出和字母后直出跟着写成同一个值，和共享设置页的总开关一样；在载入的文档里看到两半之前每次保存都带上，之后不再写，别处单独设过的一半不会被冲掉。
static void TestSmartPunctuationWritesBothHalves() {
    NSString *suite = [@"msime.smart-punctuation." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    MSIMEAppearancePreferences *prefs = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    assert([prefs sharedPreferencesByMerging:@{}][@"smart_punctuation_direct_digit"] == nil);
    NSSwitch *smart = (id)PreferenceControl(prefs, @selector(smartPunctuationChanged:));
    assert(smart);
    smart.state = NSControlStateValueOn;
    [NSApp sendAction:smart.action to:smart.target from:smart];
    NSDictionary *merged = [prefs sharedPreferencesByMerging:@{}];
    assert([merged[@"smart_punctuation"] isEqual:@YES]);
    assert([merged[@"smart_punctuation_direct_digit"] isEqual:@YES] && [merged[@"smart_punctuation_direct_letter"] isEqual:@YES]);
    // 载入的文档还没有这次保存的两半（例如保存失败），下一次保存照样带上。
    [prefs applySharedInputPreferences:@{@"smart_punctuation": @YES, @"smart_punctuation_direct_digit": @NO, @"smart_punctuation_direct_letter": @NO}];
    assert([[prefs sharedPreferencesByMerging:@{}][@"smart_punctuation_direct_letter"] isEqual:@YES]);
    // 文档里两半都已写好，就不再写。
    [prefs applySharedInputPreferences:@{@"smart_punctuation": @YES, @"smart_punctuation_direct_digit": @YES, @"smart_punctuation_direct_letter": @YES}];
    merged = [prefs sharedPreferencesByMerging:@{}];
    assert(merged[@"smart_punctuation_direct_digit"] == nil && merged[@"smart_punctuation_direct_letter"] == nil);
    // 之后共享设置页单独关掉字母后直出，原生端的保存不会把它冲回去。
    [prefs applySharedInputPreferences:@{@"smart_punctuation": @YES, @"smart_punctuation_direct_digit": @YES, @"smart_punctuation_direct_letter": @NO}];
    assert([prefs sharedPreferencesByMerging:@{}][@"smart_punctuation_direct_letter"] == nil);
    // 关掉总开关时两半一起关。
    smart.state = NSControlStateValueOff;
    [NSApp sendAction:smart.action to:smart.target from:smart];
    merged = [prefs sharedPreferencesByMerging:@{}];
    assert([merged[@"smart_punctuation"] isEqual:@NO]);
    assert([merged[@"smart_punctuation_direct_digit"] isEqual:@NO] && [merged[@"smart_punctuation_direct_letter"] isEqual:@NO]);
    MSIMERemoveTestPreferenceSuite(defaults, suite);
}

// 原生窗口各节「恢复默认值」的入口：删掉本机值，也清掉共享值。
@interface MSIMEAppearancePreferences (SectionRestore)
- (void)removeStoredKeys:(NSArray<NSString *> *)keys;
@end

// `show_candidate_page_number` 在 macOS 上的读写：缺省显示；共享文档里的值优先于本机值；「恢复默认值」清掉两者后写回文档的是默认的显示（#7316）。
static void TestCandidatePageNumberPreference() {
    NSString *suite = [@"msime.page-number." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    MSIMEAppearancePreferences *prefs = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    assert(prefs.showsCandidatePageNumber);
    assert([[prefs sharedPreferencesByMerging:@{}][@"show_candidate_page_number"] isEqual:@YES]);
    for (id invalid in @[NSNull.null, @1, @"false"]) {
        [prefs applySharedCandidatePreferences:@{@"show_candidate_page_number": invalid}];
        assert(prefs.showsCandidatePageNumber);
    }
    [prefs applySharedCandidatePreferences:@{@"show_candidate_page_number": @NO}];
    assert(!prefs.showsCandidatePageNumber && [defaults objectForKey:@"MSIMEClientCandidatePageNumber"] == nil);
    assert([[prefs sharedPreferencesByMerging:@{}][@"show_candidate_page_number"] isEqual:@NO]);
    prefs.showsCandidatePageNumber = YES;
    assert(prefs.showsCandidatePageNumber && [defaults objectForKey:@"MSIMEClientCandidatePageNumber"] != nil);
    assert([[prefs sharedPreferencesByMerging:@{@"show_candidate_page_number": @NO}][@"show_candidate_page_number"] isEqual:@YES]);
    prefs.showsCandidatePageNumber = NO;
    assert(![[[MSIMEAppearancePreferences alloc] initWithDefaults:defaults] showsCandidatePageNumber]);
    [prefs applySharedCandidatePreferences:@{@"show_candidate_page_number": @NO}];
    [prefs removeStoredKeys:@[@"MSIMEClientCandidatePageNumber"]];
    assert(prefs.showsCandidatePageNumber && [defaults objectForKey:@"MSIMEClientCandidatePageNumber"] == nil);
    assert([[prefs sharedPreferencesByMerging:@{@"show_candidate_page_number": @NO}][@"show_candidate_page_number"] isEqual:@YES]);
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
        assert([controller.menu itemAtIndex:4].state == (enabled.boolValue ? NSControlStateValueOn : NSControlStateValueOff));
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
    MSIMEAppearancePreferences *prefs = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    assert(prefs.quanpinHelpcodeEnabled && prefs.shuangpinHelpcodeEnabled);
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
    NSDictionary *shared = @{@"quanpin_helpcode": @{@"enabled": @YES, @"auto_display": @NO}, @"shuangpin_helpcode": @{@"enabled": @NO, @"future_field": @7}};
    [controller applySharedToolbarPreferences:shared];
    assert(prefs.quanpinHelpcodeEnabled && !prefs.shuangpinHelpcodeEnabled && saves == 0);
    assert(quanpin.state == NSControlStateValueOn && shuangpin.state == NSControlStateValueOff && autocorrect.state == NSControlStateValueOn);
    for (NSString *key in shared) assert([[prefs sharedPreferencesByMerging:shared][key] isEqual:shared[key]]);
    [controller applySharedToolbarPreferences:@{@"quanpin_helpcode": @{@"enabled": @0}, @"shuangpin_helpcode": NSNull.null}];
    assert(prefs.quanpinHelpcodeEnabled && !prefs.shuangpinHelpcodeEnabled && saves == 0);
    quanpin.state = NSControlStateValueOff;
    [NSApp sendAction:quanpin.action to:quanpin.target from:quanpin];
    assert(!prefs.quanpinHelpcodeEnabled && !prefs.shuangpinHelpcodeEnabled && saves == 1);
    shuangpin.state = NSControlStateValueOn;
    [NSApp sendAction:shuangpin.action to:shuangpin.target from:shuangpin];
    assert(!prefs.quanpinHelpcodeEnabled && prefs.shuangpinHelpcodeEnabled && saves == 2);
    MSIMEAppearancePreferences *reopened = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    assert(!reopened.quanpinHelpcodeEnabled && reopened.shuangpinHelpcodeEnabled);
    NSDictionary *edited = [prefs sharedPreferencesByMerging:shared];
    assert([edited[@"quanpin_helpcode"][@"enabled"] isEqual:@NO] && [edited[@"shuangpin_helpcode"][@"enabled"] isEqual:@YES]);
    assert([edited[@"quanpin_helpcode"][@"auto_display"] isEqual:@NO] && [edited[@"shuangpin_helpcode"][@"future_field"] isEqual:@7]);
    [controller applySharedToolbarPreferences:shared];
    assert(prefs.quanpinHelpcodeEnabled && !prefs.shuangpinHelpcodeEnabled && saves == 2);
    NSMutableDictionary *schemaControls = [NSMutableDictionary dictionary];
    NSMutableDictionary *displayControls = [NSMutableDictionary dictionary];
    for (NSControl *control in MSIMEFindPreferenceControls(prefs.window.contentView, @selector(helpcodeSchemaChanged:)))
        schemaControls[control.identifier] = control;
    for (NSControl *control in MSIMEFindPreferenceControls(prefs.window.contentView, @selector(helpcodeDisplayChanged:)))
        displayControls[control.identifier] = control;
    assert(schemaControls.count == 2 && displayControls.count == 2);
    NSDictionary *options = @{@"quanpin_helpcode": @{@"schema": @"shouyou2_0", @"show_in_candidate_window": @NO}, @"shuangpin_helpcode": @{@"schema": @"xiaohe", @"show_in_candidate_window": @YES}};
    [prefs applySharedAssistancePreferences:options];
    assert(saves == 2 && [defaults objectForKey:@"MSIMEClientHelpcodeOptions"] == nil);
    assert([(NSPopUpButton *)schemaControls[@"quanpin"] indexOfSelectedItem] == 2);
    assert([(NSButton *)displayControls[@"quanpin"] state] == NSControlStateValueOff);
    NSArray *identifiers = @[@"lantian", @"ziranma", @"shouyou2_0", @"shouyouplus", @"xiaohe", @"jiajia", @"wubi86"];
    // 下拉框的项数由内置方案数推出：没选插件时就是这些方案，选了插件再多一项，新增方案不必再改下面的断言。
    const NSInteger builtInSchemas = (NSInteger)identifiers.count;
    const NSInteger withPack = builtInSchemas + 1;
    for (NSString *scheme in @[@"quanpin", @"shuangpin"]) {
        NSPopUpButton *schemas = schemaControls[scheme];
        NSButton *display = displayControls[scheme];
        assert(([schemas.itemTitles isEqual:@[@"蓝天小雨点", @"自然码", @"首右2.0", @"首右plus", @"小鹤", @"加加", @"五笔 86"]]));
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
    // 选了辅助码表插件时，方案下拉框末尾多一项代表插件并选中它；再选这一项什么都不改，选一个方案就不再使用插件并把清空写进共享文档，与共享设置页一致。另一族的插件和其他插件设置原样保留。
    NSPopUpButton *quanpinSchemas = schemaControls[@"quanpin"];
    [prefs applySharedAssistancePreferences:@{@"plugins": @{@"helpcode_pack_quanpin": @"radicals", @"helpcode_pack_shuangpin": @"strokes"}}];
    assert([[prefs helpcodePackForScheme:@"quanpin"] isEqual:@"radicals"] && [[prefs helpcodePackForScheme:@"shuangpin"] isEqual:@"strokes"]);
    assert(quanpinSchemas.numberOfItems == withPack && [quanpinSchemas.titleOfSelectedItem isEqual:@"radicals（插件）"]);
    NSUInteger beforePack = saves;
    [NSApp sendAction:quanpinSchemas.action to:quanpinSchemas.target from:quanpinSchemas];
    assert(saves == beforePack && [[prefs helpcodePackForScheme:@"quanpin"] isEqual:@"radicals"]);
    assert([prefs sharedPreferencesByMerging:@{}][@"plugins"] == nil);
    [quanpinSchemas selectItemAtIndex:1];
    [NSApp sendAction:quanpinSchemas.action to:quanpinSchemas.target from:quanpinSchemas];
    assert(saves > beforePack && ![prefs helpcodePackForScheme:@"quanpin"]);
    assert(quanpinSchemas.numberOfItems == builtInSchemas && quanpinSchemas.indexOfSelectedItem == 1);
    NSDictionary *cleared = [prefs sharedPreferencesByMerging:@{@"plugins": @{@"helpcode_pack_quanpin": @"radicals", @"helpcode_pack_shuangpin": @"strokes", @"sound_pack": @"twinkle"}}];
    assert(([cleared[@"plugins"] isEqual:@{@"helpcode_pack_quanpin": @"", @"helpcode_pack_shuangpin": @"strokes", @"sound_pack": @"twinkle"}]));
    assert([cleared[@"quanpin_helpcode"][@"schema"] isEqual:@"ziranma"]);
    // 保存落盘之前读到的文档仍是旧插件，不能把它带回来；文档写进清空之后，别处再选的插件照常显示。
    [prefs applySharedAssistancePreferences:@{@"plugins": @{@"helpcode_pack_quanpin": @"radicals"}}];
    assert(![prefs helpcodePackForScheme:@"quanpin"] && quanpinSchemas.numberOfItems == builtInSchemas);
    [prefs applySharedAssistancePreferences:@{@"plugins": @{@"helpcode_pack_quanpin": @""}}];
    assert([prefs sharedPreferencesByMerging:@{}][@"plugins"] == nil);
    [prefs applySharedAssistancePreferences:@{@"plugins": @{@"helpcode_pack_quanpin": @"radicals"}}];
    assert([[prefs helpcodePackForScheme:@"quanpin"] isEqual:@"radicals"] && quanpinSchemas.numberOfItems == withPack);
    [prefs applySharedAssistancePreferences:@{@"plugins": @{@"helpcode_pack_quanpin": @"", @"helpcode_pack_shuangpin": @""}}];
    assert(quanpinSchemas.numberOfItems == builtInSchemas && [schemaControls[@"shuangpin"] numberOfItems] == builtInSchemas);
    // 清空没能写进文档（保存失败）时，共享设置页另选的插件不是那个旧 id：照常显示，之后的保存也不把它写成空串。
    [prefs applySharedAssistancePreferences:@{@"plugins": @{@"helpcode_pack_quanpin": @"radicals"}}];
    [quanpinSchemas selectItemAtIndex:1];
    [NSApp sendAction:quanpinSchemas.action to:quanpinSchemas.target from:quanpinSchemas];
    assert(![prefs helpcodePackForScheme:@"quanpin"]);
    [prefs applySharedAssistancePreferences:@{@"plugins": @{@"helpcode_pack_quanpin": @"strokes"}}];
    assert([[prefs helpcodePackForScheme:@"quanpin"] isEqual:@"strokes"]);
    assert(quanpinSchemas.numberOfItems == withPack && [quanpinSchemas.titleOfSelectedItem isEqual:@"strokes（插件）"]);
    assert([prefs sharedPreferencesByMerging:@{}][@"plugins"] == nil);
    [prefs applySharedAssistancePreferences:@{@"plugins": @{@"helpcode_pack_quanpin": @""}}];
    assert(quanpinSchemas.numberOfItems == builtInSchemas);
    // 共享偏好落盘时不写空的辅助码表包（空串的键省略，全为默认的 plugins 整个省略），所以清空写进文档后读回来的是没有 plugins 的文档：它与空串一样了结这次清空，之后在共享设置页再选同一个包照常显示，也不会被下一次保存写回空串。
    [prefs applySharedAssistancePreferences:@{@"plugins": @{@"helpcode_pack_quanpin": @"radicals"}}];
    [quanpinSchemas selectItemAtIndex:1];
    [NSApp sendAction:quanpinSchemas.action to:quanpinSchemas.target from:quanpinSchemas];
    assert(![prefs helpcodePackForScheme:@"quanpin"]);
    [prefs applySharedAssistancePreferences:@{}];
    assert([prefs sharedPreferencesByMerging:@{}][@"plugins"] == nil);
    [prefs applySharedAssistancePreferences:@{@"plugins": @{@"helpcode_pack_quanpin": @"radicals"}}];
    assert([[prefs helpcodePackForScheme:@"quanpin"] isEqual:@"radicals"] && quanpinSchemas.numberOfItems == withPack);
    assert([prefs sharedPreferencesByMerging:@{}][@"plugins"] == nil);
    // plugins 还有别的设置、只是没有这个键时同样是空包；共享设置页清掉插件也是这样回来的，下拉框随之去掉插件项。
    [quanpinSchemas selectItemAtIndex:1];
    [NSApp sendAction:quanpinSchemas.action to:quanpinSchemas.target from:quanpinSchemas];
    [prefs applySharedAssistancePreferences:@{@"plugins": @{@"sound_pack": @"twinkle"}}];
    assert([prefs sharedPreferencesByMerging:@{}][@"plugins"] == nil);
    [prefs applySharedAssistancePreferences:@{@"plugins": @{@"helpcode_pack_quanpin": @"radicals"}}];
    assert([[prefs helpcodePackForScheme:@"quanpin"] isEqual:@"radicals"] && quanpinSchemas.numberOfItems == withPack);
    [prefs applySharedAssistancePreferences:@{@"plugins": @{@"sound_pack": @"twinkle"}}];
    assert(![prefs helpcodePackForScheme:@"quanpin"] && quanpinSchemas.numberOfItems == builtInSchemas);
    [prefs applySharedAssistancePreferences:@{@"plugins": @{@"helpcode_pack_quanpin": @"radicals"}}];
    [prefs applySharedAssistancePreferences:@{@"plugins": @{@"helpcode_pack_quanpin": @7}}];
    assert(![prefs helpcodePackForScheme:@"quanpin"] && quanpinSchemas.numberOfItems == builtInSchemas);
    NSButton *neighbor = (id)PreferenceControl(prefs, @selector(neighborChanged:));
    [prefs applySharedAssistancePreferences:@{@"quanpin": @{@"autocorrect_transposition": @YES, @"autocorrect_neighbor": @NO}}];
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
    [prefs applySharedAssistancePreferences:@{@"quanpin": @{}}];
    assert(prefs.autocorrectTransposition && prefs.autocorrectNeighbor && saves == beforeCorrectionRefresh);
    assert([prefs sharedPreferencesByMerging:@{}][@"quanpin"][@"autocorrect_neighbor"] == NSNull.null);
    [prefs applySharedAssistancePreferences:@{@"quanpin": @{@"autocorrect_neighbor": @1}}];
    assert(prefs.autocorrectNeighbor);
    [NSNotificationCenter.defaultCenter removeObserver:observer];
    MSIMERemoveTestPreferenceSuite(defaults, suite);
}

// 五笔版本和双拼键位一样随共享文档往返：`wubi_profile` 读进来驱动弹出菜单，菜单选择写回去，未知值不改变当前版本。
static void TestSharedWubiProfile() {
    NSString *suite = [@"msime.shared-wubi-profile." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    MSIMEAppearancePreferences *prefs = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    assert([prefs.wubiProfile isEqual:@"wubi86"] && [[prefs sharedPreferencesByMerging:@{}][@"wubi_profile"] isEqual:@"wubi86"]);
    ModeController *controller = [ModeController alloc];
    [controller setValue:prefs forKey:@"appearance"];
    [controller applySharedToolbarPreferences:@{@"scheme": @"wubi", @"wubi_profile": @"wubi98"}];
    NSPopUpButton *profile = (id)PreferenceControl(prefs, @selector(wubiProfileChanged:));
    assert([prefs.wubiProfile isEqual:@"wubi98"] && profile.numberOfItems == 2 && profile.indexOfSelectedItem == 1);
    assert([[profile itemAtIndex:0].title isEqual:@"86 五笔"] && [[profile itemAtIndex:1].title isEqual:@"98 五笔"]);
    assert([[prefs sharedPreferencesByMerging:@{@"wubi_profile": @"wubi86"}][@"wubi_profile"] isEqual:@"wubi98"]);
    [controller applySharedToolbarPreferences:@{@"wubi_profile": @"wubi06"}];
    [controller applySharedToolbarPreferences:@{@"wubi_profile": @98}];
    assert([prefs.wubiProfile isEqual:@"wubi98"]);
    [profile selectItemAtIndex:0];
    [NSApp sendAction:profile.action to:profile.target from:profile];
    assert([prefs.wubiProfile isEqual:@"wubi86"] && [[defaults stringForKey:@"MSIMEClientWubiProfile"] isEqual:@"wubi86"]);
    assert([[prefs sharedPreferencesByMerging:@{@"wubi_profile": @"wubi98"}][@"wubi_profile"] isEqual:@"wubi86"]);
    prefs.wubiProfile = @"wubi06";
    assert([prefs.wubiProfile isEqual:@"wubi86"]);
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
    [controller applySharedToolbarPreferences:@{@"scheme": @"korean"}];
    assert([prefs.inputScheme isEqual:@"korean"] && SelectedSchemeIndex(prefs) == 4 && saves == 0);
    assert([[prefs sharedPreferencesByMerging:shared][@"scheme"] isEqual:@"korean"]);
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
    NSArray *identifiers = @[@"xiaohe", @"ziranma", @"shoudao", @"microsoft", @"smart_abc"];
    NSArray *titles = @[@"小鹤双拼", @"自然码双拼", @"首道双拼", @"微软双拼", @"智能 ABC 双拼"];
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
    [appearance applySharedInputPreferences:@{@"shuangpin_profile": @"smart_abc"}];
    assert([appearance.shuangpinProfile isEqual:@"smart_abc"]);
    assert([[appearance sharedPreferencesByMerging:@{}][@"shuangpin_profile"] isEqual:@"smart_abc"]);
    assert(!appearance.shuangpinKeymap);
    NSButton *toggle = (id)PreferenceControl(appearance, @selector(keymapChanged:));
    toggle.state = NSControlStateValueOn;
    [NSApp sendAction:toggle.action to:toggle.target from:toggle];
    assert(appearance.shuangpinKeymap);
    assert([[[MSIMEAppearancePreferences alloc] initWithDefaults:defaults skinsRoot:appearance.skinsRoot] shuangpinKeymap]);
    // 文档还没有 shuangpin_keymap_hint 时退回 defaults 里升级前的选择，并把它写进文档。
    [appearance applySharedInputPreferences:@{}];
    assert(appearance.shuangpinKeymap);
    assert([[appearance sharedPreferencesByMerging:@{}][@"shuangpin_keymap_hint"] isEqual:@YES]);
    // 文档带着这一项载入后它优先于 defaults，保存时原样写回，本机旧键随即删除。
    [appearance applySharedInputPreferences:@{@"shuangpin_keymap_hint": @NO}];
    assert(!appearance.shuangpinKeymap);
    assert([defaults objectForKey:@"MSIMEClientShuangpinKeymap"] == nil);
    assert([[appearance sharedPreferencesByMerging:@{}][@"shuangpin_keymap_hint"] isEqual:@NO]);
    // 之后文档又没有这一项（共享设置页的恢复默认设置、导入不带这一项的设置文件）就是关，升级前的旧选择不会复活。
    [appearance applySharedInputPreferences:@{}];
    assert(!appearance.shuangpinKeymap);
    assert([[appearance sharedPreferencesByMerging:@{}][@"shuangpin_keymap_hint"] isEqual:@NO]);
    // 「恢复默认值」经 SharedOverrideProperties() 用 KVC 清掉文档缓存、再删掉 defaults；载入过文档之后合并写回关，文档里的旧值不会在下次载入时回来。
    [appearance applySharedInputPreferences:@{@"shuangpin_keymap_hint": @YES}];
    [appearance setValue:nil forKey:@"sharedShuangpinKeymap"];
    [defaults removeObjectForKey:@"MSIMEClientShuangpinKeymap"];
    assert(!appearance.shuangpinKeymap);
    assert([[appearance sharedPreferencesByMerging:@{}][@"shuangpin_keymap_hint"] isEqual:@NO]);
    [defaults setBool:YES forKey:@"MSIMEClientShuangpinKeymap"];
    HiddenKeymapPanel *panel = [[HiddenKeymapPanel alloc] init];
    assert(panel.ignoresMouseEvents && panel.floatingPanel);
    for (NSString *profile in @[@"xiaohe", @"ziranma", @"shoudao", @"microsoft", @"smart_abc"]) {
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
    NSFont *clearanceFont = [appearance candidateFontOfSize:32];
    CGFloat measuredHeight = ceil([@"合成" sizeWithAttributes:@{NSFontAttributeName:clearanceFont}].height);
    HiddenCandidatePanel *candidatePanel = [[HiddenCandidatePanel alloc] initWithContentRect:NSZeroRect
        styleMask:NSWindowStyleMaskBorderless | NSWindowStyleMaskNonactivatingPanel backing:NSBackingStoreBuffered defer:NO];
    [controller setValue:candidatePanel forKey:@"panel"];
    NSNumber *savedPadding = appearance.candidatePadding;
    // 让开的距离覆盖真实卡片，既检查后备字形高度，也覆盖皮肤默认、零边距和大边距。
    for (id padding in @[NSNull.null, @0, @24]) {
        appearance.candidatePadding = padding == NSNull.null ? nil : padding;
        [controller updateKeymapPanel];
        [controller renderCandidates];
        assert(candidatePanel.requestedVisible);
        assert(candidatePanel.frame.size.height >= measuredHeight);
        assert(panel.clearance >= NSHeight(((MSIMECandidateChromeView *)candidatePanel.contentView).cardBounds));
    }
    appearance.candidatePadding = savedPadding;
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
    for (id mode in @[@"unicode", @"date_time", @"quick_phrase", @"emoji", @"kaomoji", @"super_jianpin", @"temporary_english", @"temporary_japanese", @"chaizi", @"unknown", @"", NSNull.null, @1]) {
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

// 预编辑外边距撑高候选窗时，键位图的避让高度必须覆盖最终卡片。
static void TestKeymapPreeditMargins()
{
    char temporary[] = "/tmp/msime-keymap-preedit-margin-XXXXXX";
    assert(mkdtemp(temporary));
    const std::filesystem::path root(temporary);
    std::filesystem::create_directory(root / "margin");
    {
        std::ofstream manifest(root / "margin" / "skin.toml");
        manifest << R"toml(schema_version = 2
id = "margin"
name = "Synthetic Keymap Margin"
version = "1"
base = "microsoft"
[supports]
layouts = ["horizontal", "vertical"]
themes = ["light", "dark"]
[candidate_window]
preedit_margin_dip = [70, 11, 80, 17]
)toml";
        assert(manifest.good());
    }
    NSString *suite = [@"app.msime.test.keymap-preedit-margin." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    [defaults setObject:@"custom" forKey:@"MSIMEClientGlobalTheme"];
    [defaults setObject:@"margin" forKey:@"MSIMEClientCustomCandidateSkin"];
    MSIMEAppearancePreferences *appearance = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults
        skinsRoot:[NSURL fileURLWithPath:@(root.c_str()) isDirectory:YES]];
    appearance.shuangpinKeymap = YES;
    appearance.showsAppLogo = NO;
    appearance.showsCandidatePageNumber = NO;
    appearance.showsCandidateExpandButton = NO;
    appearance.preeditFontSize = 12;
    appearance.themeMode = @"light";
    ModeController *controller = [ModeController alloc];
    ShortcutClient *client = [ShortcutClient new];
    client.caret = NSMakeRect(20, 400, 1, 20);
    HiddenKeymapPanel *keymap = [[HiddenKeymapPanel alloc] init];
    HiddenCandidatePanel *panel = [[HiddenCandidatePanel alloc] initWithContentRect:NSZeroRect
        styleMask:NSWindowStyleMaskBorderless | NSWindowStyleMaskNonactivatingPanel backing:NSBackingStoreBuffered defer:NO];
    [controller setValue:appearance forKey:@"appearance"];
    [controller setValue:client forKey:@"activeClient"];
    [controller setValue:keymap forKey:@"keymapPanel"];
    [controller setValue:panel forKey:@"panel"];
    [controller setValue:[ShortcutSession new] forKey:@"session"];
    [controller setValue:@{@"scheme": @1, @"local_mode": @"none", @"dedicated_english": @NO,
        @"shuangpin_profile": @"microsoft", @"editing_text": @"b;", @"preedit": @"bing", @"focused": @YES,
        @"page": @0, @"page_count": @1, @"candidates": @[@{@"text": @"合成"}]} forKey:@"view"];
    for (NSNumber *percent in @[@100, @150]) for (NSNumber *vertical in @[@NO, @YES]) {
        appearance.candidateScalePercent = percent.integerValue;
        appearance.vertical = vertical.boolValue;
        for (NSNumber *visible in @[@NO, @YES]) {
            appearance.showsCandidatePreedit = visible.boolValue;
            [controller updateKeymapPanel];
            [controller renderCandidates];
            assert(keymap.requestedVisible && panel.requestedVisible);
            assert(keymap.clearance >= NSHeight(((MSIMECandidateChromeView *)panel.contentView).cardBounds));
        }
    }
    MSIMERemoveTestPreferenceSuite(defaults, suite);
    std::filesystem::remove_all(root);
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
        assert(!MSIMEToggleChordMatches(msime::mac::FactoryPunctuationToggleChord(), toggleEvent(47, flags.unsignedIntegerValue, NO)));
    assert(!MSIMEToggleChordMatches(msime::mac::FactoryPunctuationToggleChord(), toggleEvent(65, NSEventModifierFlagControl, NO))); // 小键盘句号。
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
    // 锁定英文标点时运行时标点就是英文，即使起始标点还是中文。
    assert(preferences.chinesePunctuation && !preferences.runtimeChinesePunctuation);
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
    assert(preferences.runtimeChinesePunctuation);
    assert([defaults boolForKey:@"MSIMEClientPairedPunctuation"] && [[defaults stringForKey:@"MSIMEClientPunctuationLock"] isEqual:@"chinese"]);
    MSIMEAppearancePreferences *reopened = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    assert(reopened.pairedPunctuation && [reopened.punctuationLock isEqual:@"chinese"]);
    [preferences applySharedInputPreferences:@{ @"paired_punctuation": @NO, @"punctuation_lock": @"follow" }];
    assert(!preferences.pairedPunctuation && [preferences.punctuationLock isEqual:@"follow"]);
    assert(preferences.runtimeChinesePunctuation == preferences.chinesePunctuation);
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
    // 直角引号与空心方头括号（quote_style、bracket_style）照样补全，引号键出的后半个同样改回前半个。
    [controller setValue:@('\'') forKey:@"punctuationKeyInFlight"];
    [controller apply:@{@"commit": @"」", @"view": @{@"editing_text": @"", @"caret_position": @0}}];
    assert([client.committed isEqual:@"「"] && [client.marked isEqual:@"」"]);
    [controller flushPendingPairedClosing];
    [controller setValue:@('"') forKey:@"punctuationKeyInFlight"];
    [controller apply:@{@"commit": @"』", @"view": @{@"editing_text": @"", @"caret_position": @0}}];
    assert([client.committed isEqual:@"『"] && [client.marked isEqual:@"』"]);
    [controller flushPendingPairedClosing];
    [controller setValue:@('[') forKey:@"punctuationKeyInFlight"];
    [controller apply:@{@"commit": @"〖", @"view": @{@"editing_text": @"", @"caret_position": @0}}];
    assert([client.committed isEqual:@"〖"] && [client.marked isEqual:@"〗"]);
    [controller flushPendingPairedClosing];
    // 注音的大千键位 `[` `]` `{` `}` 自己出「」『』：`]` 出的」是用户要打的后半个，原样上屏；`[` `{` 出的「『不补后半个，因为这个宿主只在 `'` `"` 上跨过」』。
    client.marked = nil;
    [controller setValue:@{@"scheme": @(msime::mac::scheme::Zhuyin)} forKey:@"view"];
    [controller setValue:@(']') forKey:@"punctuationKeyInFlight"];
    [controller apply:@{@"commit": @"」", @"view": @{@"scheme": @(msime::mac::scheme::Zhuyin), @"editing_text": @"", @"caret_position": @0}}];
    assert([client.committed isEqual:@"」"] && !client.marked.length);
    [controller setValue:@('}') forKey:@"punctuationKeyInFlight"];
    [controller apply:@{@"commit": @"』", @"view": @{@"scheme": @(msime::mac::scheme::Zhuyin), @"editing_text": @"", @"caret_position": @0}}];
    assert([client.committed isEqual:@"』"] && !client.marked.length);
    [controller setValue:@('[') forKey:@"punctuationKeyInFlight"];
    [controller apply:@{@"commit": @"「", @"view": @{@"scheme": @(msime::mac::scheme::Zhuyin), @"editing_text": @"", @"caret_position": @0}}];
    assert([client.committed isEqual:@"「"] && !client.marked.length);
    [controller setValue:@('{') forKey:@"punctuationKeyInFlight"];
    [controller apply:@{@"commit": @"你好『", @"view": @{@"scheme": @(msime::mac::scheme::Zhuyin), @"editing_text": @"", @"caret_position": @0}}];
    assert([client.committed isEqual:@"你好『"] && !client.marked.length);
    // 没有按键的整段上屏（例如候选）照旧按字补，后半个不改回前半个。
    [controller apply:@{@"commit": @"「", @"view": @{@"editing_text": @"", @"caret_position": @0}}];
    assert([client.committed isEqual:@"「"] && [client.marked isEqual:@"」"]);
    [controller flushPendingPairedClosing];
    client.marked = nil;
    [controller apply:@{@"commit": @"」", @"view": @{@"editing_text": @"", @"caret_position": @0}}];
    assert([client.committed isEqual:@"」"] && !client.marked.length);

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
    // 独立英文模式保留大括号的成对补全；普通中文方案的 Shift+[ / Shift+] 另测直角引号。
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
    NSDictionary *idle = @{ @"dedicated_english": @YES, @"focused": @YES, @"editing_text": @"", @"candidates": @[] };
    NSEvent *brace = [NSEvent keyEventWithType:NSEventTypeKeyDown location:NSZeroPoint modifierFlags:NSEventModifierFlagShift
                                      timestamp:0 windowNumber:0 context:nil characters:@"{" charactersIgnoringModifiers:@"["
                                      isARepeat:NO keyCode:33];
    NSEvent *closeBrace = [NSEvent keyEventWithType:NSEventTypeKeyDown location:NSZeroPoint modifierFlags:NSEventModifierFlagShift
                                           timestamp:0 windowNumber:0 context:nil characters:@"}" charactersIgnoringModifiers:@"]"
                                           isARepeat:NO keyCode:30];

    // 没有自己补过的后半个可跨时，普通键不读文档：每次读取都要到应用里同步来回一趟，而这时读回来什么也改变不了。
    NSEvent *letter = [NSEvent keyEventWithType:NSEventTypeKeyDown location:NSZeroPoint modifierFlags:0 timestamp:0 windowNumber:0
                                        context:nil characters:@"q" charactersIgnoringModifiers:@"q" isARepeat:NO keyCode:12];
    [controller setValue:idle forKey:@"view"];
    session.nextTransition = @{ @"handled": @NO, @"commit": NSNull.null, @"view": idle };
    [controller handleEvent:letter client:client];
    assert(client.documentReads == 0);
    session.nextTransition = nil;
    session.asciiCalls = 0;

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
    [controller setValue:@{ @"dedicated_english": @YES, @"focused": @YES, @"editing_text": @"ni", @"candidates": @[] } forKey:@"view"];
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
    [controller setValue:@{ @"dedicated_english": @YES, @"focused": @YES, @"editing_text": @"ni", @"candidates": @[] } forKey:@"view"];
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

// 从原生控制器到真实 Rust 会话验证键位，使用合成词库和合成输入，不依赖本机词库。
static void AssertShiftCornerQuotes(NSDictionary *overrides, NSString *typed, NSString *expected,
                                   NSEventModifierFlags extraFlags = 0) {
    NSString *root = [NSTemporaryDirectory() stringByAppendingPathComponent:NSUUID.UUID.UUIDString];
    NSMutableDictionary *preferences = [@{@"scheme": @"quanpin", @"default_ime_mode": @"chinese", @"candidate_page_size": @5,
        @"learning": @NO, @"chinese_punctuation": @YES, @"paired_punctuation": @YES,
        @"smart_punctuation": @NO, @"cloud_candidates": @NO} mutableCopy];
    [preferences addEntriesFromDictionary:overrides];
    NSMutableDictionary *options = [@{@"api_version": @1, @"preferences": preferences} mutableCopy];
    for (NSString *name in @[@"resources", @"user_data", @"cache", @"dictionaries"]) {
        NSString *path = [root stringByAppendingPathComponent:name];
        assert([NSFileManager.defaultManager createDirectoryAtPath:path withIntermediateDirectories:YES attributes:nil error:nil]);
        options[name] = path;
    }
    // 可选方案必须有对应资源才能启用；用合成空库，避免「请求五笔、实际回退全拼」的假覆盖。
    options[@"language_dictionaries"] = options[@"resources"];
    for (NSString *name in @[@"msime-wubi.db", @"msime-zhuyin.db"]) {
        sqlite3 *database = nullptr;
        NSString *path = [options[@"resources"] stringByAppendingPathComponent:name];
        assert(sqlite3_open(path.fileSystemRepresentation, &database) == SQLITE_OK);
        assert(sqlite3_exec(database,
            "CREATE TABLE metadata(name TEXT PRIMARY KEY, value TEXT NOT NULL) WITHOUT ROWID;"
            "INSERT INTO metadata VALUES('format_version','1');"
            "CREATE TABLE syllables(syllable TEXT PRIMARY KEY) WITHOUT ROWID;"
            "CREATE TABLE entries(key TEXT NOT NULL, text TEXT NOT NULL, weight INTEGER NOT NULL, PRIMARY KEY(key,text)) WITHOUT ROWID;"
            "CREATE TABLE wubi86(key TEXT,value TEXT,weight INTEGER);", nullptr, nullptr, nullptr) == SQLITE_OK);
        assert(sqlite3_close(database) == SQLITE_OK);
    }
    NSError *error = nil;
    MSIMEClientSession *session = [[MSIMEClientSession alloc] initWithOptions:options error:&error];
    if (!session || error) fprintf(stderr, "corner quote session: %s\n", error.localizedDescription.UTF8String);
    assert(session && !error);
    NSDictionary *view = [session setFocused:YES error:&error][@"view"];
    assert(view && !error);
    NSDictionary *schemes = @{@"quanpin": @0, @"shuangpin": @1, @"wubi": @2, @"korean": @4,
        @"zhuyin": @6, @"vietnamese": @7};
    assert([view[@"scheme"] isEqual:schemes[preferences[@"scheme"]]]);
    NSString *suite = [@"msime.shift-corner-quotes." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    MSIMEAppearancePreferences *prefs = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults
        skinsRoot:[NSURL fileURLWithPath:root]];
    [prefs applySharedInputPreferences:preferences];
    prefs.cloudCandidates = NO;
    ShortcutClient *client = [ShortcutClient new];
    client.document = @"";
    client.selection = NSMakeRange(0, 0);
    ModeController *controller = [ModeController alloc];
    [controller setValue:prefs forKey:@"appearance"];
    [controller setValue:session forKey:@"session"];
    [controller setValue:client forKey:@"activeClient"];
    [controller setValue:view forKey:@"view"];
    [controller setValue:[[HiddenCandidatePanel alloc] init] forKey:@"panel"];
    for (NSUInteger index = 0; index < typed.length; ++index) {
        NSString *character = [typed substringWithRange:NSMakeRange(index, 1)];
        const unichar ascii = [character characterAtIndex:0];
        const unsigned short code = ascii == '{' || ascii == '[' ? 33 : ascii == '}' || ascii == ']' ? 30 :
            ascii == '\'' || ascii == '"' ? 39 : ascii == 'n' ? 45 : ascii == 'i' ? 34 : 49;
        const BOOL shift = ascii == '{' || ascii == '}' || ascii == '"';
        if (![controller handleEvent:LayoutKey(code, character, (shift ? NSEventModifierFlagShift : 0) | extraFlags) client:client])
            [client insertText:character replacementRange:NSMakeRange(NSNotFound, NSNotFound)];
    }
    [controller flushPendingPairedClosing];
    if (![client.document isEqual:expected])
        fprintf(stderr, "shift corner quotes: requested=%s initial=%d current=%d document=%s expected=%s\n", [preferences[@"scheme"] UTF8String], MSIMEViewScheme(view), MSIMEViewScheme([controller valueForKey:@"view"]), client.document.UTF8String, expected.UTF8String);
    assert([client.document isEqual:expected]);
    assert([session closeWithError:&error] && !error);
    MSIMERemoveTestPreferenceSuite(defaults, suite);
    [NSFileManager.defaultManager removeItemAtPath:root error:nil];
}

static void TestShiftCornerQuotesThroughRealSession() {
    for (NSString *scheme in @[@"quanpin", @"shuangpin", @"wubi"]) {
        for (NSNumber *paired in @[@NO, @YES]) {
            for (NSString *width in @[@"halfwidth", @"fullwidth"]) {
                NSDictionary *preferences = @{@"scheme": scheme, @"paired_punctuation": paired, @"character_width": width};
                AssertShiftCornerQuotes(preferences, @"{}", @"「」");
                // 普通方头括号仍走原来的中文标点映射。
                if (!paired.boolValue) AssertShiftCornerQuotes(preferences, @"{}[]", @"「」【】");
            }
        }
    }
    AssertShiftCornerQuotes(@{@"paired_punctuation": @NO}, @"\"\"{}''", @"“”「」‘’");
    AssertShiftCornerQuotes(@{@"paired_punctuation": @NO, @"quote_style": @"corner"}, @"\"\"{}''", @"『』「」「」");
    // 空词库仍有正在编辑的拼音：标点必须保留原始组字，不能吞掉它。
    AssertShiftCornerQuotes(@{}, @"ni{}", @"ni「」");
    AssertShiftCornerQuotes(@{}, @"{ni}", @"「ni」");
    AssertShiftCornerQuotes(@{@"paired_punctuation": @NO}, @"{ni}", @"「ni」");
    AssertShiftCornerQuotes(@{@"chinese_punctuation": @NO}, @"{}", @"{}");
    AssertShiftCornerQuotes(@{@"punctuation_lock": @"english"}, @"{}", @"{}");
    AssertShiftCornerQuotes(@{@"caps_lock_ascii_punctuation": @YES, @"paired_punctuation": @NO},
        @"{}", @"{}", NSEventModifierFlagCapsLock);
    AssertShiftCornerQuotes(@{@"caps_lock_ascii_punctuation": @NO}, @"{}", @"「」", NSEventModifierFlagCapsLock);
    AssertShiftCornerQuotes(@{@"scheme": @"korean"}, @"{}", @"{}");
    AssertShiftCornerQuotes(@{@"scheme": @"vietnamese"}, @"{}", @"{}");
    AssertShiftCornerQuotes(@{@"scheme": @"zhuyin"}, @"{}", @"『』");

    // 文档里已有自动补出的右引号时，只有 tracker 和光标后文字同时匹配才跨过。
    NSString *suite = [@"msime.corner-quote-skip." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    MSIMEAppearancePreferences *prefs = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    ShortcutSession *session = [ShortcutSession new];
    ModeController *controller = [ModeController alloc];
    ShortcutClient *client = [ShortcutClient new];
    client.document = @"「合成」";
    client.selection = NSMakeRange(3, 0);
    NSDictionary *idle = @{@"focused": @YES, @"editing_text": @"", @"candidates": @[]};
    [controller setValue:prefs forKey:@"appearance"];
    [controller setValue:session forKey:@"session"];
    [controller setValue:client forKey:@"activeClient"];
    [controller setValue:idle forKey:@"view"];
    // 通过既有成对上屏路线登记，避免直接修改 tracker 的内部栈。
    [controller apply:@{@"commit": @"「合成」", @"view": idle}];
    client.document = @"「合成」";
    client.selection = NSMakeRange(3, 0);
    assert([controller handleEvent:LayoutKey(30, @"}", NSEventModifierFlagShift) client:client]);
    assert([client.document isEqual:@"「合成」"] && client.selection.location == 4);
    assert(session.punctuationASCIICalls == 0);
    // 原有右引号没有登记过，照常插入一个，不得把文档里的字跳掉。
    client.selection = NSMakeRange(3, 0);
    session.punctuationASCIITransition = @{@"handled": @NO, @"commit": NSNull.null, @"view": idle};
    assert([controller handleEvent:LayoutKey(30, @"}", NSEventModifierFlagShift) client:client]);
    assert([client.document isEqual:@"「合成」」"] && session.punctuationASCIICalls == 1);
    // 会话失败保留正在编辑的拼音，不插入标点。
    session.punctuationASCIITransition = nil;
    [controller setValue:@{@"editing_text": @"ni", @"candidates": @[]} forKey:@"view"];
    assert([controller handleEvent:LayoutKey(33, @"{", NSEventModifierFlagShift) client:client]);
    assert([client.document isEqual:@"「合成」」"] && [[[controller valueForKey:@"view"] objectForKey:@"editing_text"] isEqual:@"ni"]);
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
    assert(preferences.mixedEnglishInput && preferences.mixedEmojiInput && !preferences.mixedKaomojiInput);
    // Symbol mixed input starts on; like client-core, the merged object only carries `symbols` once it is off.
    assert(preferences.mixedSymbolInput);
    // 触发字符数在 macOS 上固定为 5（host-api 不读它），窗口里没有它的控件，合并结果也不带 `minimum_prefix`，文档原有的值按键合并保留。
    NSDictionary *initial = [preferences sharedPreferencesByMerging:@{}][@"mixed_input"];
    assert([initial[@"english"] isEqual:@YES] && initial[@"minimum_prefix"] == nil &&
           [initial[@"emoji"] isEqual:@YES] && [initial[@"kaomoji"] isEqual:@NO] && initial[@"symbols"] == nil);
    [preferences applySharedInputPreferences:@{ @"mixed_input": @{
        @"english": @NO, @"minimum_prefix": @7, @"emoji": @YES, @"kaomoji": @YES, @"symbols": @NO } }];
    assert(!preferences.mixedEnglishInput && preferences.mixedEmojiInput && preferences.mixedKaomojiInput &&
           !preferences.mixedSymbolInput);
    assert([preferences sharedPreferencesByMerging:@{}][@"mixed_input"][@"minimum_prefix"] == nil);
    // 合并进一份完整文档时整个 `mixed_input` 被替换，文档原有的触发字符数要带过来：client-core 里它是必填字段，丢了整份文档就读不回来。
    NSDictionary *storedMixedInput = @{ @"mixed_input": @{ @"english": @YES, @"minimum_prefix": @7 } };
    assert([[preferences sharedPreferencesByMerging:storedMixedInput][@"mixed_input"][@"minimum_prefix"] isEqual:@7]);
    assert([defaults objectForKey:@"MSIMEClientMixedInput"] == nil);
    [preferences applySharedInputPreferences:@{ @"mixed_input": @{
        @"english": @1, @"minimum_prefix": @9, @"emoji": @"true", @"kaomoji": NSNull.null, @"symbols": @"true" } }];
    assert(!preferences.mixedEnglishInput && preferences.mixedEmojiInput && preferences.mixedKaomojiInput &&
           !preferences.mixedSymbolInput);
    assert(MSIMEFindPreferenceControl(preferences.window.contentView, NSSelectorFromString(@"mixedEnglishPrefixChanged:")) == nil);
    NSButton *english = (id)PreferenceControl(preferences, @selector(mixedEnglishChanged:));
    NSButton *emoji = (id)PreferenceControl(preferences, @selector(mixedEmojiChanged:));
    NSButton *kaomoji = (id)PreferenceControl(preferences, @selector(mixedKaomojiChanged:));
    NSButton *symbols = (id)PreferenceControl(preferences, @selector(mixedSymbolChanged:));
    assert(english.state == NSControlStateValueOff && emoji.state == NSControlStateValueOn &&
           kaomoji.state == NSControlStateValueOn && symbols.state == NSControlStateValueOff);
    english.state = NSControlStateValueOn;
    [NSApp sendAction:english.action to:english.target from:english];
    emoji.state = NSControlStateValueOff;
    [NSApp sendAction:emoji.action to:emoji.target from:emoji];
    kaomoji.state = NSControlStateValueOff;
    [NSApp sendAction:kaomoji.action to:kaomoji.target from:kaomoji];
    assert(preferences.mixedEnglishInput && !preferences.mixedEmojiInput && !preferences.mixedKaomojiInput);
    // Turned off in the shared page, symbol mixed input stays off after the other switches change here: the object is rewritten whole.
    assert(!preferences.mixedSymbolInput);
    NSDictionary *edited = [preferences sharedPreferencesByMerging:@{}][@"mixed_input"];
    assert(([edited isEqual:@{ @"english": @YES, @"emoji": @NO, @"kaomoji": @NO, @"symbols": @NO }]));
    assert([defaults dictionaryForKey:@"MSIMEClientMixedInput"] != nil);
    // An explicitly stored NO for emoji survives reopening instead of falling back to the on default.
    MSIMEAppearancePreferences *reopened = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    assert(reopened.mixedEnglishInput && !reopened.mixedEmojiInput && !reopened.mixedKaomojiInput &&
           !reopened.mixedSymbolInput);
    assert([[reopened sharedPreferencesByMerging:@{}][@"mixed_input"][@"emoji"] isEqual:@NO]);
    // Turning symbols back on here drops the key again; a shared object without the key reads as on.
    symbols.state = NSControlStateValueOn;
    [NSApp sendAction:symbols.action to:symbols.target from:symbols];
    assert(preferences.mixedSymbolInput && [preferences sharedPreferencesByMerging:@{}][@"mixed_input"][@"symbols"] == nil);
    [preferences applySharedInputPreferences:@{ @"mixed_input": @{
        @"english": @YES, @"minimum_prefix": @5, @"emoji": @NO, @"kaomoji": @NO, @"symbols": @NO } }];
    assert(!preferences.mixedSymbolInput);
    [preferences applySharedInputPreferences:@{ @"mixed_input": @{
        @"english": @YES, @"minimum_prefix": @5, @"emoji": @NO, @"kaomoji": @NO, @"symbols": NSNull.null } }];
    assert(!preferences.mixedSymbolInput);
    [preferences applySharedInputPreferences:@{ @"mixed_input": @{
        @"english": @YES, @"minimum_prefix": @5, @"emoji": @NO, @"kaomoji": @NO } }];
    assert(preferences.mixedSymbolInput);
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
    // Dvorak：标着 F 的键在美式布局的 Y 位置（keyCode 16），物理 F 键打出 u。英文模式下只占用不切换。
    assert([controller handleEvent:LayoutKey(16, @"F", flags) client:client]);
    assert(appearance.traditionalOutput == traditional);
    assert(![controller handleEvent:LayoutKey(3, @"U", flags) client:client]);
    appearance.englishMode = NO;
    assert([controller handleEvent:LayoutKey(16, @"F", flags) client:client]);
    assert(appearance.traditionalOutput != traditional);
    assert([controller handleEvent:LayoutKey(16, @"f", flags) client:client]);
    assert(appearance.traditionalOutput == traditional);
    // 没有字符的合成事件、给出非拉丁字母的布局仍按物理位置认键。
    assert([controller handleEvent:LayoutKey(3, @"", flags) client:client]);
    assert(appearance.traditionalOutput != traditional);
    assert([controller handleEvent:LayoutKey(3, @"\u0430", flags) client:client]);
    assert(appearance.traditionalOutput == traditional);
    appearance.englishMode = YES;
    [appearance applySharedInputPreferences:@{@"keybindings":keys}];
    assert(![controller handleEvent:ModeKey(3, flags, NO) client:client]);
    appearance.characterSetShortcut = NO;
    assert(![[[MSIMEAppearancePreferences alloc] initWithDefaults:defaults skinsRoot:appearance.skinsRoot] characterSetShortcut]);
    appearance.characterSetShortcut = YES;
    appearance.englishMode = english;
    assert(appearance.fullWidthInput == fullWidth && appearance.chinesePunctuation == punctuation);
    MSIMERemoveTestPreferenceSuite(defaults, suite);
}

// 三个可重绑切换热键按共享偏好 `keybindings.toggle_*` 认键：改成新键后按新键命中、
// 按旧键不再命中，空串不绑定；Option+Shift+H 不随 `toggle_full_width` 改动。
static void TestToggleHotkeyRebinding(void) {
    NSString *suite = [@"msime.toggle-hotkeys." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    MSIMEAppearancePreferences *appearance = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    ModeController *controller = [ModeController alloc];
    ShortcutClient *client = [ShortcutClient new];
    ShortcutSession *session = [ShortcutSession new];
    [controller setValue:appearance forKey:@"appearance"];
    [controller setValue:client forKey:@"activeClient"];
    [controller setValue:session forKey:@"session"];
    [controller setValue:@{@"editing_text":@"test", @"candidates":@[]} forKey:@"view"];
    appearance.englishMode = NO;
    const NSEventModifierFlags ctrl = NSEventModifierFlagControl;
    const NSEventModifierFlags ctrlShift = NSEventModifierFlagControl | NSEventModifierFlagShift;
    // 标点：出厂 Ctrl+. 命中；改成 Ctrl+, 后按 , 命中、按 . 不再命中；空串不绑定。
    NSUInteger calls = session.punctuationCalls;
    assert([controller handleEvent:ModeKey(47, ctrl, NO) client:client]);
    assert(session.punctuationCalls == calls + 1);
    calls = session.punctuationCalls;
    [appearance applySharedInputPreferences:@{@"keybindings":@{@"toggle_punctuation":@"Ctrl+Comma"}}];
    assert([controller handleEvent:ModeKey(43, ctrl, NO) client:client]);
    assert(session.punctuationCalls == calls + 1);
    calls = session.punctuationCalls;
    [controller handleEvent:ModeKey(47, ctrl, NO) client:client];
    assert(session.punctuationCalls == calls);
    [appearance applySharedInputPreferences:@{@"keybindings":@{@"toggle_punctuation":@""}}];
    [controller handleEvent:ModeKey(43, ctrl, NO) client:client];
    assert(session.punctuationCalls == calls);
    // 中英：改成 Ctrl+Shift+R 后按 R 命中、按 E 不再命中；字母主键按布局字母认。
    const NSUInteger ecalls = session.englishCandidateCalls;
    [appearance applySharedInputPreferences:@{@"keybindings":@{@"toggle_english_candidates":@"Ctrl+Shift+KeyR"}}];
    assert([controller handleEvent:ModeKey(15, ctrlShift, NO) client:client]);
    assert(session.englishCandidateCalls == ecalls + 1);
    [controller handleEvent:ModeKey(14, ctrlShift, NO) client:client];
    assert(session.englishCandidateCalls == ecalls + 1);
    // 全半角：改成 Ctrl+Shift+, 后按 , 命中、Ctrl+Shift+Space 不再命中；Option+Shift+H 照常。
    [appearance applySharedInputPreferences:@{@"keybindings":@{@"toggle_full_width":@"Ctrl+Shift+Comma"}}];
    BOOL width = appearance.runtimeFullWidthInput;
    NSUInteger wcalls = session.widthCalls;
    assert([controller handleEvent:ModeKey(43, ctrlShift, NO) client:client]);
    assert(appearance.runtimeFullWidthInput != width && session.widthCalls > wcalls);
    width = appearance.runtimeFullWidthInput;
    wcalls = session.widthCalls;
    [controller handleEvent:ModeKey(49, ctrlShift, NO) client:client];
    assert(appearance.runtimeFullWidthInput == width && session.widthCalls == wcalls);
    assert([controller handleEvent:ModeKey(4, NSEventModifierFlagOption | NSEventModifierFlagShift, NO) client:client]);
    assert(appearance.runtimeFullWidthInput != width && session.widthCalls > wcalls);
    // Dvorak：句号位输出 v 时只触发 KeyV，E 位输出句号时只触发 Period。
    [appearance applySharedInputPreferences:@{@"keybindings":@{
        @"toggle_punctuation":@"Ctrl+Period", @"toggle_english_candidates":@"Ctrl+KeyV"}}];
    calls = session.punctuationCalls;
    NSUInteger englishCalls = session.englishCandidateCalls;
    assert([controller handleEvent:LayoutKey(47, @"v", ctrl) client:client]);
    assert(session.punctuationCalls == calls && session.englishCandidateCalls == englishCalls + 1);
    assert([controller handleEvent:LayoutKey(14, @".", ctrl) client:client]);
    assert(session.punctuationCalls == calls + 1 && session.englishCandidateCalls == englishCalls + 1);
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
    // 上一组 Ctrl+Shift+E 已经结束；下一次 Shift+空格属于新一轮按下。
    assert(![controller handleEvent:TapEvent(NSEventTypeFlagsChanged, 56, 0, 1.0) client:client]);
    assert(![controller handleEvent:TapEvent(NSEventTypeFlagsChanged, 56, NSEventModifierFlagShift, 1.1) client:client]);
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
    // Dvorak：E 在美式布局的 D 位置（keyCode 2），物理 E 键打出句点，不再切换。
    const BOOL dedicated = session.dedicatedEnglish;
    [controller handleEvent:LayoutKey(14, @".", flags) client:client];
    assert(session.dedicatedEnglish == dedicated && session.englishCandidateCalls == calls);
    assert([controller handleEvent:LayoutKey(2, @"E", flags) client:client]);
    assert(session.dedicatedEnglish != dedicated);
    assert([controller handleEvent:LayoutKey(2, @"E", flags) client:client]);
    assert(session.dedicatedEnglish == dedicated);
}

static void TestPunctuationAfterCompositionKeepsRawInput(MSIMEAppearancePreferences *appearance) {
    ModeController *controller = [ModeController alloc];
    ShortcutSession *session = [ShortcutSession new];
    ShortcutClient *client = [ShortcutClient new];
    NSDictionary *composing = @{
        @"focused": @YES, @"scheme": @0, @"editing_text": @"Observable",
        @"candidates": @[@{@"text": @"白色", @"highlighted": @YES}],
    };
    NSDictionary *idle = @{@"focused": @YES, @"scheme": @0, @"editing_text": @"", @"candidates": @[]};
    [controller setValue:appearance forKey:@"appearance"];
    [controller setValue:session forKey:@"session"];
    [controller setValue:client forKey:@"activeClient"];
    [controller setValue:composing forKey:@"view"];
    appearance.englishMode = NO;
    session.nextTransition = @{@"handled": @NO, @"view": composing};
    session.rawTransition = @{@"handled": @YES, @"commit": @"Observable", @"view": idle};
    NSEvent *less = [NSEvent keyEventWithType:NSEventTypeKeyDown location:NSZeroPoint
        modifierFlags:NSEventModifierFlagShift timestamp:0 windowNumber:0 context:nil
        characters:@"<" charactersIgnoringModifiers:@"<" isARepeat:NO keyCode:33];

    // 标点由应用接收，组字则用原文提交，不能选择高亮的「白色」。
    assert(![controller handleEvent:less client:client]);
    assert(session.rawCommitCalls == 1 && session.lastCommand == MSIME_COMMIT_RAW);
    assert([client.committed isEqual:@"Observable"]);
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
        assert(msime::mac::FullWidthInputToggleArm(49, '\0', windowsChord | extra,
            msime::mac::FactoryFullWidthToggleChord()) == msime::mac::ToggleFullWidthArm::none);
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
    // Dvorak：H 在美式布局的 J 位置（keyCode 38）。
    assert([controller handleEvent:LayoutKey(38, @"H", chord) client:client]);
    assert(appearance.runtimeFullWidthInput != fullWidthBefore);
    assert([controller handleEvent:LayoutKey(38, @"H", chord) client:client]);
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
    assert(!MSIMESessionOptions(nil, nil));
    assert(!MSIMESessionOptions((NSDictionary *)@"not a dictionary", @"/synthetic/sound-packs"));

    NSDictionary *file = @{@"api_version":@1, @"resources":@"/synthetic/resources",
        @"preferences":@{@"scheme":@"quanpin"}};
    NSDictionary *requested = MSIMESessionOptions(file, nil);
    assert([requested[@"phrase_preedit"] isEqual:@YES]);
    assert([requested[@"quick_phrase_candidates"] isEqual:@YES]);
    assert([requested[@"korean_romaja"] isEqual:@YES]);
    // 原配置中的字段及嵌套对象保持原样，宿主额外声明三项运行时能力。
    for (NSString *key in file) assert([requested[key] isEqual:file[key]]);
    assert(requested.count == file.count + 3);
    // 不修改调用方的字典，原配置仍供其他读取方使用。
    assert(!file[@"phrase_preedit"]);
    assert(!file[@"korean_romaja"]);

    // An options file that already says something about it does not get to say no: this host draws
    // the field, and a stale file predates the behaviour entirely.
    NSDictionary *stale = MSIMESessionOptions(@{@"api_version":@1, @"phrase_preedit":@NO, @"korean_romaja":@NO}, nil);
    assert([stale[@"phrase_preedit"] isEqual:@YES]);
    assert([stale[@"quick_phrase_candidates"] isEqual:@YES]);
    assert([stale[@"korean_romaja"] isEqual:@YES]);

    // The built-in sound packs live in the bundle, while resources is EngineResources in Application Support, so the host library cannot find them beside it: the bundle's directory is named, unless the options file names one itself.
    NSDictionary *sounds = MSIMESessionOptions(file, @"/synthetic/bundle/Contents/Resources/sound-packs");
    assert([sounds[@"sound_packs"] isEqual:@"/synthetic/bundle/Contents/Resources/sound-packs"]);
    assert(sounds.count == file.count + 4 && !file[@"sound_packs"]);
    NSDictionary *named = MSIMESessionOptions(@{@"api_version":@1, @"sound_packs":@"/configured/sound-packs"}, @"/synthetic/sound-packs");
    assert([named[@"sound_packs"] isEqual:@"/configured/sound-packs"]);

    // 下载词库缺少辅助码时，会话从当前输入法包回退；原始配置继续供旧宿主读取。
    NSDictionary *helpcodes = MSIMESessionOptions(file, nil, @"/synthetic/bundle/Contents/Resources");
    assert([helpcodes[@"helpcode_fallback_resources"] isEqual:@"/synthetic/bundle/Contents/Resources"]);
    assert(!file[@"helpcode_fallback_resources"] && helpcodes.count == file.count + 4);
    assert(!MSIMESessionOptions(file, nil, @"relative/resources")[@"helpcode_fallback_resources"]);
    NSDictionary *explicitHelpcodes = MSIMESessionOptions(@{@"helpcode_fallback_resources":@"/configured/resources"},
        nil, @"/synthetic/bundle/Contents/Resources");
    assert([explicitHelpcodes[@"helpcode_fallback_resources"] isEqual:@"/configured/resources"]);

    NSString *root = [NSTemporaryDirectory() stringByAppendingPathComponent:NSUUID.UUID.UUIDString];
    NSString *bundlePath = [root stringByAppendingPathComponent:@"Synthetic.bundle"];
    NSString *resources = [bundlePath stringByAppendingPathComponent:@"Contents/Resources"];
    assert([NSFileManager.defaultManager createDirectoryAtPath:resources withIntermediateDirectories:YES attributes:nil error:nil]);
    assert([@{@"CFBundleIdentifier": @"app.msime.synthetic"} writeToURL:[NSURL fileURLWithPath:[bundlePath stringByAppendingPathComponent:@"Contents/Info.plist"]] error:nil]);
    assert(!MSIMEBundleSoundPacks([NSBundle bundleWithPath:bundlePath]));
    assert([NSFileManager.defaultManager createDirectoryAtPath:[resources stringByAppendingPathComponent:@"sound-packs"] withIntermediateDirectories:NO attributes:nil error:nil]);
    NSString *found = MSIMEBundleSoundPacks([NSBundle bundleWithPath:bundlePath]);
    assert([found.lastPathComponent isEqual:@"sound-packs"] && found.isAbsolutePath);
    [NSFileManager.defaultManager removeItemAtPath:root error:nil];
}

// A helpcode or sound failure can be reported while the first HostSession is
// being constructed. The live preference document is authoritative, so the
// log must be configured before that construction rather than only after the
// first asynchronous preference reload.
static void TestDiagnosticLogConfiguredBeforeSession() {
    NSString *root = [NSTemporaryDirectory() stringByAppendingPathComponent:NSUUID.UUID.UUIDString];
    assert([NSFileManager.defaultManager createDirectoryAtPath:root withIntermediateDirectories:YES attributes:nil error:nil]);
    NSError *error = nil;
    NSDictionary *initial = [MSIMEClientSession loadPreferencesInDirectory:root error:&error];
    assert(initial && !error);
    NSMutableDictionary *preferences = [initial[@"preferences"] mutableCopy];
    NSMutableDictionary *diagnostic = [preferences[@"diagnostic_log"] mutableCopy];
    diagnostic[@"server"] = @YES;
    preferences[@"diagnostic_log"] = diagnostic;
    NSDictionary *saved = [MSIMEClientSession savePreferencesInDirectory:root
        expectedRevision:[initial[@"revision"] unsignedLongLongValue]
        snapshot:@{ @"format_version": @1, @"revision": initial[@"revision"], @"preferences": preferences }
        error:&error];
    assert(saved && !error);
    NSDictionary *options = @{
        @"preferences_directory": root,
        // The runtime-options copy is stale and still says diagnostics are off;
        // the live preferences document above is authoritative at startup.
        @"preferences": @{ @"diagnostic_log": @{ @"server": @NO } },
    };
    msime_macos_diagnostic_configure(std::string(), false);
    assert(!msime_macos_diagnostic_enabled());
    MSIMEConfigureDiagnosticLogBeforeSession(options);
    assert(msime_macos_diagnostic_enabled());
    msime_macos_diagnostic_write("startup_failure");
    NSString *contents = [NSString stringWithContentsOfFile:[root stringByAppendingPathComponent:@"diagnostic.log"]
                                                    encoding:NSUTF8StringEncoding error:nil];
    assert([contents containsString:@"startup_failure"]);
    msime_macos_diagnostic_configure(std::string(), false);
    [NSFileManager.defaultManager removeItemAtPath:root error:nil];
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

@interface RestoredInputModeController : ModeController
@property(nonatomic, copy) NSString *selectedSource;
@property(nonatomic) NSUInteger synchronizationCalls;
@end
@implementation RestoredInputModeController
- (NSDictionary *)runtimeOptions { return @{}; }
- (NSString *)selectedSystemInputModeID { return self.selectedSource; }
- (void)syncSystemInputModeForClient:(id)client {
    ++self.synchronizationCalls;
    // 可用性由合成源决定，CI 不需要安装或启用真实输入源。
    MSIMESelectSystemInputMode(MSIMESharedSystemInputModeState(), [self currentInputModeID], client,
        +[](NSString *identifier) -> BOOL { return MSIMEIsInputModeID(identifier); });
}
@end

static void DrainMainQueue();

// 系统从 ASCII 恢复输入法时只校准已经选中的模式；排队期间换客户端、失焦和后续激活都不能留下旧请求。
static void TestRestoredInputModeActivation() {
    NSString *suite = [@"msime.source-restore." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    MSIMEAppearancePreferences *appearance = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    appearance.inputScheme = @"quanpin";
    appearance.englishMode = NO;
    appearance.inputModeStatusItem = NO;
    auto makeController = [&](ModeSelectingClient *client) {
        RestoredInputModeController *controller = [RestoredInputModeController alloc];
        [controller setValue:appearance forKey:@"appearance"];
        [controller setValue:client forKey:@"activeClient"];
        controller.selectedSource = @"com.apple.keylayout.US";
        MSIMEResetSystemInputModeState(MSIMESharedSystemInputModeState());
        MSIMEFocusedController = controller;
        return controller;
    };
    ModeSelectingClient *client = [ModeSelectingClient new];
    RestoredInputModeController *controller = makeController(client);
    [controller scheduleSystemInputModeSyncForClient:client];
    assert(controller.synchronizationCalls == 0 && client.selectedModes.count == 0);
    controller.selectedSource = MSIMEChineseInputModeID;
    DrainMainQueue();
    assert(controller.synchronizationCalls == 1 && client.selectedModes.count == 0);
    assert([MSIMESharedSystemInputModeState().current isEqual:MSIMEChineseInputModeID]);

    for (id selected in @[@"com.apple.keylayout.US", NSNull.null]) {
        controller = makeController(client);
        controller.selectedSource = selected == NSNull.null ? nil : selected;
        [controller scheduleSystemInputModeSyncForClient:client];
        DrainMainQueue();
        assert(controller.synchronizationCalls == 0 && client.selectedModes.count == 0);
    }
    // 当前已稳定在本输入法的另一模式时，仍按当前应用的状态同步。
    controller = makeController(client);
    controller.selectedSource = MSIMEEnglishInputModeID;
    [controller scheduleSystemInputModeSyncForClient:client];
    DrainMainQueue();
    assert(controller.synchronizationCalls == 1 && client.selectedModes.count == 1);
    assert([client.selectedModes.lastObject isEqual:MSIMEChineseInputModeID]);
    [client.selectedModes removeAllObjects];

    controller = makeController(client);
    controller.selectedSource = MSIMEChineseInputModeID;
    [controller scheduleSystemInputModeSyncForClient:client];
    [controller scheduleSystemInputModeSyncForClient:client];
    DrainMainQueue();
    assert(controller.synchronizationCalls == 1 && client.selectedModes.count == 0);
    for (NSUInteger change = 0; change < 2; ++change) {
        controller = makeController(client);
        controller.selectedSource = MSIMEChineseInputModeID;
        [controller scheduleSystemInputModeSyncForClient:client];
        if (change == 0) [controller setValue:[ModeSelectingClient new] forKey:@"activeClient"];
        else MSIMEFocusedController = nil;
        DrainMainQueue();
        assert(controller.synchronizationCalls == 0 && client.selectedModes.count == 0);
    }
    // 排队后状态变化时读取最新目标，不使用激活时捕获的旧目标。
    controller = makeController(client);
    controller.selectedSource = MSIMEChineseInputModeID;
    [controller scheduleSystemInputModeSyncForClient:client];
    appearance.englishMode = YES;
    DrainMainQueue();
    assert(controller.synchronizationCalls == 1 && client.selectedModes.count == 1);
    assert([client.selectedModes.lastObject isEqual:MSIMEEnglishInputModeID]);
    [client.selectedModes removeAllObjects];
    controller = makeController(client);
    controller.selectedSource = MSIMEChineseInputModeID;
    [controller scheduleSystemInputModeSyncForClient:client];
    // 显式同步已选中英文，但 TIS 仍报告中文时，旧激活校准不能重复请求。
    MSIMESharedSystemInputModeState().current = MSIMEEnglishInputModeID;
    SEL selector = @selector(syncSystemInputModeForClient:);
    auto synchronize = reinterpret_cast<void (*)(id, SEL, id)>(
        method_getImplementation(class_getInstanceMethod(MSIMEInputController.class, selector)));
    synchronize(controller, selector, client);
    DrainMainQueue();
    assert(controller.synchronizationCalls == 0 && client.selectedModes.count == 0);
    assert([MSIMESharedSystemInputModeState().current isEqual:MSIMEEnglishInputModeID]);
    MSIMEFocusedController = nil;
    MSIMEResetSystemInputModeState(MSIMESharedSystemInputModeState());
    MSIMERemoveTestPreferenceSuite(defaults, suite);
}

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
    // 回到的方案取决于系统是否启用了「双」模式（只读查询 TIS）：启用时选「中」表示全拼（见 MSIMESchemeForReportedInputMode）。装过本输入法的开发机上「双」通常已启用，CI 上没有；两条分支由 InputModeIdentifiersTest 用桩函数确定地覆盖，这里只验证控制器把报告接到了这条规则上。
    NSString *returned = MSIMEInputSourceIsEnabled(MSIMEShuangpinInputModeID) ? @"quanpin" : @"shuangpin";
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
    assert([appearance.inputScheme isEqual:returned] && !appearance.englishMode && client.selectedModes.count == 0);

    // 한 does the same for korean, straight from 日 too, and 中 still goes back to the Chinese scheme both were entered from.
    [controller systemDidReportInputMode:MSIMEKoreanInputModeID client:client];
    assert([appearance.inputScheme isEqual:@"korean"] && [appearance.lastChineseScheme isEqual:returned] &&
           !appearance.englishMode && client.selectedModes.count == 0);
    [controller systemDidReportInputMode:MSIMEJapaneseInputModeID client:client];
    [controller systemDidReportInputMode:MSIMEKoreanInputModeID client:client];
    assert([appearance.inputScheme isEqual:@"korean"] && [appearance.lastChineseScheme isEqual:returned] &&
           client.selectedModes.count == 0);
    [controller systemDidReportInputMode:MSIMEEnglishInputModeID client:client];
    assert([appearance.inputScheme isEqual:@"korean"] && appearance.englishMode && client.selectedModes.count == 0);
    [controller systemDidReportInputMode:MSIMEChineseInputModeID client:client];
    assert([appearance.inputScheme isEqual:returned] && !appearance.englishMode && client.selectedModes.count == 0);
    appearance.inputScheme = scheme;
    appearance.englishMode = english;
}

@interface SchemeHostSession : ShortcutSession
@property(nonatomic, copy) NSDictionary *hostOptions;
@end
@implementation SchemeHostSession
@end

// Cantonese, Zhuyin and Stroke are offered only where their dictionary is installed: the input menu leaves them out, its check falls on the scheme the Engine falls back to, and the settings radios are disabled. An opt-in mode is enabled when the scheme running moves to its scheme - in this process, while the input method was not running, or by the dictionary arriving after the scheme was picked - and never for the scheme the last sync already showed.
static void TestOptInSchemeModes() {
    NSString *suite = [@"msime.opt-in-modes." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    NSString *dictionaries = [NSTemporaryDirectory() stringByAppendingPathComponent:[@"msime-language-dictionaries-" stringByAppendingString:NSUUID.UUID.UUIDString]];
    assert([NSFileManager.defaultManager createDirectoryAtPath:dictionaries withIntermediateDirectories:YES attributes:nil error:nil]);
    assert([NSData.data writeToFile:[dictionaries stringByAppendingPathComponent:@"msime-cantonese.db"] atomically:YES]);
    NSMutableArray<NSString *> *enabled = [NSMutableArray array];
    auto makeController = [&](MSIMEAppearancePreferences *appearance, BOOL withEnabler) {
        MSIMEInputController *controller = [MSIMEInputController alloc];
        SchemeHostSession *session = [SchemeHostSession new];
        session.hostOptions = @{@"language_dictionaries": dictionaries};
        [controller setValue:appearance forKey:@"appearance"];
        [controller setValue:session forKey:@"session"];
        [controller setValue:[ModeSelectingClient new] forKey:@"activeClient"];
        if (withEnabler) [controller setValue:^OSStatus(NSString *identifier) { [enabled addObject:identifier]; return noErr; } forKey:@"optInInputModeEnabler"];
        return controller;
    };
    MSIMEAppearancePreferences *appearance = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    appearance.inputScheme = @"quanpin";
    MSIMEInputController *controller = makeController(appearance, YES);
    id client = [controller valueForKey:@"activeClient"];

    // 只有 msime-cantonese.db 时菜单按引擎顺序正好列出八个方案，没有注音或笔画。
    NSMenuItem *schemeItem = [controller.menu itemAtIndex:9];
    NSMutableArray<NSString *> *listed = [NSMutableArray array];
    for (NSMenuItem *item in schemeItem.submenu.itemArray) [listed addObject:item.representedObject];
    assert(([listed isEqualToArray:@[@"quanpin", @"shuangpin", @"wubi", @"japanese", @"korean", @"cantonese", @"vietnamese", @"tibetan"]]));

    // 第一次同步落在全拼上只记录方案。
    assert(!appearance.lastSyncedInputScheme);
    [controller syncSystemInputModeForClient:client];
    assert([appearance.lastSyncedInputScheme isEqual:@"quanpin"] && enabled.count == 0);

    // Picking vietnamese from the menu enables its mode once; syncing again on the same scheme does not repeat it.
    [NSApp sendAction:@selector(selectInputScheme:) to:controller from:[schemeItem.submenu itemAtIndex:6]];
    [controller syncSystemInputModeForClient:client];
    [controller syncSystemInputModeForClient:client];
    assert(([enabled isEqualToArray:@[MSIMEVietnameseInputModeID]]) && [appearance.lastSyncedInputScheme isEqual:@"vietnamese"]);

    // Zhuyin without its dictionary runs as quanpin: the menu checks 全拼, and nothing is enabled.
    appearance.inputScheme = @"zhuyin";
    [controller syncSystemInputModeForClient:client];
    assert(enabled.count == 1 && [appearance.lastSyncedInputScheme isEqual:@"quanpin"]);
    schemeItem = [controller.menu itemAtIndex:9];
    assert([schemeItem.title isEqual:@"输入方案（全拼）"] && [schemeItem.submenu itemAtIndex:0].state == NSControlStateValueOn);
    // Installing the dictionary afterwards makes zhuyin the scheme running, which enables its mode.
    assert([NSData.data writeToFile:[dictionaries stringByAppendingPathComponent:@"msime-zhuyin.db"] atomically:YES]);
    [controller syncSystemInputModeForClient:client];
    assert(([enabled isEqualToArray:@[MSIMEVietnameseInputModeID, MSIMEZhuyinInputModeID]]));
    schemeItem = [controller.menu itemAtIndex:9];
    assert(schemeItem.submenu.numberOfItems == 9 && [schemeItem.title isEqual:@"输入方案（注音）"]);

    // A later process starting on the scheme the last sync showed enables nothing, so a mode removed from the input menu stays removed.
    MSIMEAppearancePreferences *relaunched = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    MSIMEInputController *next = makeController(relaunched, YES);
    [next syncSystemInputModeForClient:[next valueForKey:@"activeClient"]];
    assert(enabled.count == 2);
    // A scheme picked while the input method was not running is still a change when it next syncs.
    [defaults setObject:@"cantonese" forKey:@"MSIMEClientInputScheme"];
    relaunched = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    next = makeController(relaunched, YES);
    [next syncSystemInputModeForClient:[next valueForKey:@"activeClient"]];
    assert(([enabled.lastObject isEqual:MSIMECantoneseInputModeID]) && enabled.count == 3);

    // 第一次同步就落在粤拼上时同样启用「粤」：持久记录出现之前就在用粤拼的人，否则永远等不到这一次切换。
    NSString *firstSuite = [@"msime.opt-in-first-sync." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *firstDefaults = [[NSUserDefaults alloc] initWithSuiteName:firstSuite];
    [firstDefaults setObject:@"cantonese" forKey:@"MSIMEClientInputScheme"];
    MSIMEAppearancePreferences *first = [[MSIMEAppearancePreferences alloc] initWithDefaults:firstDefaults];
    assert(!first.lastSyncedInputScheme);
    next = makeController(first, YES);
    [next syncSystemInputModeForClient:[next valueForKey:@"activeClient"]];
    assert(enabled.count == 4 && [enabled.lastObject isEqual:MSIMECantoneseInputModeID] && [first.lastSyncedInputScheme isEqual:@"cantonese"]);
    MSIMERemoveTestPreferenceSuite(firstDefaults, firstSuite);

    // 系统没能启用模式时不记录这次切换，下一次同步再试，成功后才记录。
    [defaults setObject:@"quanpin" forKey:@"MSIMEClientLastSyncedInputScheme"];
    relaunched = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    next = makeController(relaunched, NO);
    __block OSStatus enableStatus = paramErr;
    [next setValue:^OSStatus(NSString *identifier) { [enabled addObject:identifier]; return enableStatus; } forKey:@"optInInputModeEnabler"];
    [next syncSystemInputModeForClient:[next valueForKey:@"activeClient"]];
    assert(enabled.count == 5 && [relaunched.lastSyncedInputScheme isEqual:@"quanpin"]);
    enableStatus = noErr;
    [next syncSystemInputModeForClient:[next valueForKey:@"activeClient"]];
    assert(enabled.count == 6 && [enabled.lastObject isEqual:MSIMECantoneseInputModeID] && [relaunched.lastSyncedInputScheme isEqual:@"cantonese"]);

    // A stand-in session with no enabler never reaches TIS, and the change stays unrecorded so a real session would still enable it.
    [defaults setObject:@"quanpin" forKey:@"MSIMEClientLastSyncedInputScheme"];
    relaunched = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    next = makeController(relaunched, NO);
    [next syncSystemInputModeForClient:[next valueForKey:@"activeClient"]];
    assert(enabled.count == 6 && [relaunched.lastSyncedInputScheme isEqual:@"quanpin"]);

    // 笔画没有 msime-stroke.db 时按全拼运行、不启用模式；词库到位后成为实际方案，启用「笔」，菜单最后一项是笔画并打勾。
    [defaults setObject:@"stroke" forKey:@"MSIMEClientInputScheme"];
    relaunched = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    next = makeController(relaunched, YES);
    [next syncSystemInputModeForClient:[next valueForKey:@"activeClient"]];
    assert(enabled.count == 6 && [relaunched.lastSyncedInputScheme isEqual:@"quanpin"]);
    assert([NSData.data writeToFile:[dictionaries stringByAppendingPathComponent:@"msime-stroke.db"] atomically:YES]);
    [next syncSystemInputModeForClient:[next valueForKey:@"activeClient"]];
    assert(enabled.count == 7 && [enabled.lastObject isEqual:MSIMEStrokeInputModeID] && [relaunched.lastSyncedInputScheme isEqual:@"stroke"]);
    schemeItem = [next.menu itemAtIndex:9];
    assert(schemeItem.submenu.numberOfItems == 10 && [schemeItem.title isEqual:@"输入方案（笔画）"]);
    NSMenuItem *strokeItem = [schemeItem.submenu itemAtIndex:9];
    assert([strokeItem.representedObject isEqual:@"stroke"] && [strokeItem.title isEqual:@"笔画"] && strokeItem.state == NSControlStateValueOn);
    assert([NSFileManager.defaultManager removeItemAtPath:[dictionaries stringByAppendingPathComponent:@"msime-stroke.db"] error:nil]);
    [defaults setObject:@"cantonese" forKey:@"MSIMEClientInputScheme"];

    // The settings radios read the runtime options on disk: with only msime-cantonese.db named there, 注音 and 笔画 are disabled and say why, and the rest are enabled.
    assert([NSFileManager.defaultManager removeItemAtPath:[dictionaries stringByAppendingPathComponent:@"msime-zhuyin.db"] error:nil]);
    NSString *optionsPath = MSIMEDefaultRuntimeOptionsPath(NSFileManager.defaultManager);
    assert(![NSFileManager.defaultManager fileExistsAtPath:optionsPath]);
    assert([NSFileManager.defaultManager createDirectoryAtPath:optionsPath.stringByDeletingLastPathComponent withIntermediateDirectories:YES attributes:nil error:nil]);
    assert([[NSJSONSerialization dataWithJSONObject:@{@"language_dictionaries": dictionaries} options:0 error:nil] writeToFile:optionsPath atomically:YES]);
    MSIMEAppearancePreferences *settings = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    NSArray<NSControl *> *radios = MSIMEFindPreferenceControls(settings.window.contentView, @selector(schemeRadioChanged:));
    assert(radios.count == 10);
    for (NSControl *radio in radios) {
        const BOOL missing = radio.tag == 6 || radio.tag == 9;
        assert(radio.enabled == !missing && (missing ? [radio.toolTip isEqual:@"未安装该方案的词库，暂不可用"] : radio.toolTip == nil));
    }
    // 选中粤拼而「粤」不在输入法列表里时，输入方式卡片说明去「粤语」下添加；没有探针时不显示，加进去之后也不显示。
    auto hintRow = [](MSIMEAppearancePreferences *preferences) {
        return MSIMEFindPreferenceView(preferences.window.contentView, ^BOOL(NSView *view) {
            return [view.accessibilityLabel isEqual:@"菜单栏入口提示"];
        });
    };
    [defaults setObject:@"cantonese" forKey:@"MSIMEClientInputScheme"];
    settings = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    assert(hintRow(settings).hidden);
    static BOOL cantoneseAdded = NO;
    MSIMEInputModeEnabledProbe = [](NSString *identifier) -> BOOL {
        return cantoneseAdded || ![identifier isEqual:MSIMECantoneseInputModeID];
    };
    settings = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    NSView *hint = hintRow(settings);
    assert(hint && !hint.hidden);
    NSTextField *hintText = (NSTextField *)MSIMEFindPreferenceViewOfClass(hint, NSTextField.class);
    assert([hintText.stringValue containsString:@"「水杉输入法 · 粤」"] && [hintText.stringValue containsString:@"「粤语」"]);
    assert(MSIMEFindPreferenceControl(hint, @selector(openInputSourceSettings:)));
    cantoneseAdded = YES;
    [NSNotificationCenter.defaultCenter postNotificationName:NSWindowDidBecomeKeyNotification object:settings.window];
    assert(hint.hidden);
    MSIMEInputModeEnabledProbe = nullptr;
    [defaults setObject:@"quanpin" forKey:@"MSIMEClientInputScheme"];

    assert([NSFileManager.defaultManager removeItemAtPath:optionsPath error:nil]);
    settings = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    radios = MSIMEFindPreferenceControls(settings.window.contentView, @selector(schemeRadioChanged:));
    for (NSControl *radio in radios) assert(radio.enabled == (radio.tag != 5 && radio.tag != 6 && radio.tag != 9));

    // 藏文和越南文一样按需启用，而且不需要词库：从菜单选中藏文时启用「藏」一次，再同步同一方案不重复。
    NSString *tibetanSuite = [@"msime.opt-in-tibetan." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *tibetanDefaults = [[NSUserDefaults alloc] initWithSuiteName:tibetanSuite];
    MSIMEAppearancePreferences *tibetanAppearance = [[MSIMEAppearancePreferences alloc] initWithDefaults:tibetanDefaults];
    tibetanAppearance.inputScheme = @"quanpin";
    MSIMEInputController *tibetanController = makeController(tibetanAppearance, YES);
    id tibetanClient = [tibetanController valueForKey:@"activeClient"];
    [tibetanController syncSystemInputModeForClient:tibetanClient];
    const NSUInteger enabledBefore = enabled.count;
    NSMenuItem *tibetanSchemeItem = [tibetanController.menu itemAtIndex:9];
    NSMenuItem *tibetanItem = nil;
    for (NSMenuItem *item in tibetanSchemeItem.submenu.itemArray)
        if ([item.representedObject isEqual:@"tibetan"]) tibetanItem = item;
    assert(tibetanItem && [tibetanItem.title isEqual:@"藏文"] && tibetanSchemeItem.submenu.itemArray.lastObject == tibetanItem);
    [NSApp sendAction:@selector(selectInputScheme:) to:tibetanController from:tibetanItem];
    [tibetanController syncSystemInputModeForClient:tibetanClient];
    [tibetanController syncSystemInputModeForClient:tibetanClient];
    assert(enabled.count == enabledBefore + 1 && [enabled.lastObject isEqual:MSIMETibetanInputModeID]);
    assert([tibetanAppearance.lastSyncedInputScheme isEqual:@"tibetan"] && [tibetanAppearance.lastChineseScheme isEqual:@"quanpin"]);
    assert([[tibetanController.menu itemAtIndex:9].title isEqual:@"输入方案（藏文）"]);
    MSIMERemoveTestPreferenceSuite(tibetanDefaults, tibetanSuite);

    // Picking 粤 where msime-cantonese.db is missing leaves the scheme alone. Cantonese cannot run here, so switching to it only made the Engine fall back to the Chinese scheme used before it: the menu bar said 粤 and the keys typed shuangpin. With the dictionary in place the same report switches to cantonese as before.
    NSString *reportSuite = [@"msime.opt-in-report." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *reportDefaults = [[NSUserDefaults alloc] initWithSuiteName:reportSuite];
    NSString *emptyDictionaries = [NSTemporaryDirectory() stringByAppendingPathComponent:[@"msime-no-language-dictionaries-" stringByAppendingString:NSUUID.UUID.UUIDString]];
    assert([NSFileManager.defaultManager createDirectoryAtPath:emptyDictionaries withIntermediateDirectories:YES attributes:nil error:nil]);
    auto reportCantonese = [&](NSString *dictionaryDirectory) {
        MSIMEAppearancePreferences *reportAppearance = [[MSIMEAppearancePreferences alloc] initWithDefaults:reportDefaults];
        reportAppearance.inputScheme = @"quanpin";
        MSIMEInputController *reporter = makeController(reportAppearance, YES);
        ((SchemeHostSession *)[reporter valueForKey:@"session"]).hostOptions = @{@"language_dictionaries": dictionaryDirectory};
        [reporter setValue:emptyDictionaries forKey:@"preferencesDirectory"];
        [reporter setValue:@{@"editing_text":@"", @"candidates":@[]} forKey:@"view"];
        MSIMEResetSystemInputModeState(MSIMESharedSystemInputModeState());
        [reporter systemDidReportInputMode:MSIMECantoneseInputModeID client:[reporter valueForKey:@"activeClient"]];
        return reportAppearance.inputScheme;
    };
    assert([reportCantonese(emptyDictionaries) isEqual:@"quanpin"]);
    assert([reportCantonese(dictionaries) isEqual:@"cantonese"]);
    MSIMEResetSystemInputModeState(MSIMESharedSystemInputModeState());
    [NSFileManager.defaultManager removeItemAtPath:emptyDictionaries error:nil];
    MSIMERemoveTestPreferenceSuite(reportDefaults, reportSuite);

    [NSFileManager.defaultManager removeItemAtPath:dictionaries error:nil];
    MSIMERemoveTestPreferenceSuite(defaults, suite);
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
@property(nonatomic, copy) void (^onInsertText)(void);
@property(nonatomic) NSUInteger markedWrites;
@end
@implementation NestingClient
- (void)insertText:(id)text replacementRange:(NSRange)range {
    [super insertText:text replacementRange:range];
    void (^nested)(void) = self.onInsertText;
    self.onInsertText = nil;
    if (nested) nested();
}
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

static void TestNestedCommitKeepsTheNewerComposition() {
    NSString *suite = [@"msime.nested-commit." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    MSIMEAppearancePreferences *appearance = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    appearance.englishMode = NO;
    appearance.pairedPunctuation = YES;
    for (NSDictionary *scenario in @[
        @{@"commit": @"甲", @"expected": @"ne"},
        @{@"commit": @"甲", @"remaining": @"tail", @"expected": @"ne"},
        @{@"commit": @"（", @"expected": @"ne）", @"pendingAfter": @"）"},
        @{@"commit": @"甲", @"pendingBefore": @"）", @"expected": @"ne"},
        @{@"commit": @"甲", @"nestedCommit": @"乙", @"expected": @"ne"},
        @{@"commit": @"甲", @"switchClient": @YES}
    ]) {
        @autoreleasepool {
            ModeController *controller = [ModeController alloc];
            NestingClient *client = [NestingClient new];
            [controller setValue:appearance forKey:@"appearance"];
            [controller setValue:[ShortcutSession new] forKey:@"session"];
            [controller setValue:client forKey:@"activeClient"];
            [controller setValue:[[HiddenCandidatePanel alloc] init] forKey:@"panel"];
            [controller setValue:scenario[@"pendingBefore"] forKey:@"pendingPairedClosing"];
            NSDictionary *newer = @{@"session": @1, @"generation": @5, @"editing_text": @"ne",
                                     @"preedit": @"ne", @"caret_position": @2, @"candidates": @[]};
            __weak ModeController *weakController = controller;
            ShortcutClient *replacement = [ShortcutClient new];
            replacement.marked = @"external";
            client.onInsertText = ^{
                // 同步回调中的下一键必须已经能读到外层刚上屏的末字。
                NSString *landed = scenario[@"pendingBefore"] ?: scenario[@"commit"];
                assert([weakController precedingForSmartPunctuation:client] ==
                    [landed characterAtIndex:landed.length - 1]);
                if ([scenario[@"switchClient"] boolValue]) {
                    [weakController setValue:replacement forKey:@"activeClient"];
                    [weakController setValue:newer forKey:@"view"];
                    [weakController invalidateSmartPunctuationShadow];
                    return;
                }
                NSMutableDictionary *nested = [@{@"view": newer} mutableCopy];
                if (scenario[@"nestedCommit"]) nested[@"commit"] = scenario[@"nestedCommit"];
                [weakController apply:nested];
            };
            NSString *remaining = scenario[@"remaining"] ?: @"";
            [controller apply:@{@"commit": scenario[@"commit"],
                @"view": @{@"editing_text": remaining, @"caret_position": @(remaining.length), @"candidates": @[]}}];
            if ([scenario[@"switchClient"] boolValue]) {
                assert([replacement.marked isEqual:@"external"] && client.marked == nil);
            } else {
                assert([client.marked isEqual:scenario[@"expected"]]);
            }
            assert([[controller valueForKey:@"view"] isEqual:newer]);
            id pending = [controller valueForKey:@"pendingPairedClosing"];
            assert(scenario[@"pendingAfter"] ? [pending isEqual:scenario[@"pendingAfter"]] : pending == nil);
            NSString *outerCommit = [scenario[@"commit"] stringByAppendingString:(scenario[@"pendingBefore"] ?: @"")];
            assert([client.insertions.firstObject isEqual:outerCommit]);
            assert(client.insertions.count == (scenario[@"nestedCommit"] ? 2u : 1u));
            if ([scenario[@"switchClient"] boolValue]) {
                assert([controller precedingForSmartPunctuation:replacement] == 0);
            } else {
                NSString *lastCommit = scenario[@"nestedCommit"] ?: outerCommit;
                assert([controller precedingForSmartPunctuation:client] ==
                    [lastCommit characterAtIndex:lastCommit.length - 1]);
            }
        }
    }
    [defaults removePersistentDomainForName:suite];
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

// #7616：连续按住 Shift 打大写时，只有这一串的第一枚字母能进快捷模式，规则在共享引擎里（`crates/engine/src/session/tests.rs`）。
// 宿主这一侧的第二枚因此必须原样交给应用：既不能自己插进去，也不能留下组字。
static void TestConsecutiveUppercaseGoesToTheClient(MSIMEAppearancePreferences *appearance) {
    ModeController *controller = [ModeController alloc];
    ShortcutClient *client = [ShortcutClient new];
    ShortcutSession *session = [ShortcutSession new];
    [controller setValue:appearance forKey:@"appearance"];
    [controller setValue:client forKey:@"activeClient"];
    [controller setValue:session forKey:@"session"];
    [controller setValue:@{ @"scheme": @0, @"local_mode": @"none", @"editing_text": @"", @"candidates": @[] }
                   forKey:@"view"];
    session.nextTransition = @{ @"handled": @NO,
                                @"view": @{ @"scheme": @0, @"local_mode": @"none",
                                             @"editing_text": @"", @"candidates": @[] } };

    assert(![controller handleEvent:LayoutKey(46, @"M", NSEventModifierFlagShift) client:client]);
    NSUInteger asciiCalls = session.asciiCalls;
    assert(![controller handleEvent:LayoutKey(15, @"R", NSEventModifierFlagShift) client:client]);
    assert(session.asciiCalls == asciiCalls + 1);
    assert(client.insertions.count == 0);
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
    // 释义是宿主自己写进文档的，丢弃组字用 MSIME_CANCEL_AFTER_HOST_COMMIT：对会话同 MSIME_CANCEL，生命周期把结束原因记成 HostCommit。
    assert(session.lastCommand == MSIME_CANCEL_AFTER_HOST_COMMIT);

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
    assert(session.lastCommand == MSIME_CANCEL_AFTER_HOST_COMMIT);
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
        // The runtime reports conversion for quanpin outside the unicode and temporary Japanese modes.
        view[@"script_conversion"] = @(localMode == nil);
        [controller setValue:view forKey:@"view"];
        panel.visible = YES;
    };

    // A single-sense Ctrl+Enter commits the Traditional form and still clears the composition.
    compose(@"苹果", nil);
    client.committed = nil;
    session.lastCommand = UINT32_MAX;
    assert([controller handleEvent:ModeKey(36, NSEventModifierFlagControl, NO) client:client]);
    assert([client.committed isEqual:@"蘋果"]);
    assert(session.lastCommand == MSIME_CANCEL_AFTER_HOST_COMMIT);

    // The senses page picks the sense through the same conversion it is drawn with.
    compose(@"头发; 苹果", nil);
    client.committed = nil;
    assert([controller handleEvent:ModeKey(36, NSEventModifierFlagControl, NO) client:client]);
    assert(!client.committed);
    session.lastCommand = UINT32_MAX;
    assert([controller handleEvent:ModeKey(19, 0, NO) client:client]); // 2
    assert([client.committed isEqual:@"蘋果"]);
    assert(session.lastCommand == MSIME_CANCEL_AFTER_HOST_COMMIT);

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

// 展开候选网格的会话替身：一代候选是 total 个「<prefix><序号>」，每页 pageSize 个，视图是第 0 页、第一个高亮。msime_client_all_candidates 答这一代的全部候选，msime_client_select_any_candidate 只接受这一代。
@interface ExpandedGridSession : ShortcutSession
@property(nonatomic) uint64_t generation;
@property(nonatomic) NSUInteger total;
@property(nonatomic) NSUInteger pageSize;
@property(nonatomic) NSUInteger highlighted;
@property(nonatomic, copy) NSString *prefix;
@property(nonatomic, copy) NSString *editing;
@property(nonatomic) NSUInteger allCandidateCalls;
@property(nonatomic) NSUInteger selectAnyCalls;
@property(nonatomic) uint64_t selectAnyGeneration;
@property(nonatomic) NSUInteger selectAnyIndex;
@property(nonatomic) NSUInteger tallIndex;
@end
@implementation ExpandedGridSession
- (NSDictionary *)gridCandidate:(NSUInteger)index highlighted:(BOOL)highlighted {
    NSString *text = [NSString stringWithFormat:@"%@%lu", self.prefix, (unsigned long)index];
    if (self.tallIndex != 0 && index == self.tallIndex) text = [text stringByAppendingString:@"\n第二行"];
    return @{@"text": text,
             @"id": @{@"session": @1, @"generation": @(self.generation), @"index": @(index)},
             @"highlighted": @(highlighted)};
}
- (NSDictionary *)gridView {
    NSMutableArray *page = [NSMutableArray array];
    const NSUInteger start = self.highlighted / self.pageSize * self.pageSize;
    for (NSUInteger index = start; index < MIN(start + self.pageSize, self.total); ++index)
        [page addObject:[self gridCandidate:index highlighted:index == self.highlighted]];
    return @{@"focused": @YES, @"session": @1, @"generation": @(self.generation), @"editing_text": self.editing,
             @"preedit": self.editing, @"caret_position": @(self.editing.length), @"page": @(start / self.pageSize),
             @"page_count": @((self.total + self.pageSize - 1) / self.pageSize), @"candidates": page};
}
- (NSDictionary *)command:(uint32_t)command error:(NSError **)error {
    switch (command) {
        case MSIME_NEXT_CANDIDATE: self.highlighted = MIN(self.highlighted + 1, self.total - 1); break;
        case MSIME_PREVIOUS_CANDIDATE: self.highlighted = self.highlighted ? self.highlighted - 1 : 0; break;
        case MSIME_NEXT_PAGE:
            self.highlighted = MIN(self.highlighted / self.pageSize + 1, (self.total - 1) / self.pageSize) * self.pageSize;
            break;
        case MSIME_PREVIOUS_PAGE:
            self.highlighted = self.highlighted >= self.pageSize ? (self.highlighted / self.pageSize - 1) * self.pageSize : 0;
            break;
        default: return [super command:command error:error];
    }
    self.lastCommand = command;
    ++self.commandCalls;
    ++self.generation;
    return @{@"handled": @YES, @"view": [self gridView]};
}
- (NSDictionary *)allCandidatesWithError:(NSError **)error {
    (void)error;
    ++self.allCandidateCalls;
    NSMutableArray *all = [NSMutableArray array];
    for (NSUInteger index = 0; index < self.total; ++index) [all addObject:[self gridCandidate:index highlighted:NO]];
    return @{@"session": @1, @"generation": @(self.generation), @"preedit": self.editing, @"candidates": all};
}
- (NSDictionary *)selectAnyCandidateGeneration:(uint64_t)generation index:(NSUInteger)index error:(NSError **)error {
    (void)error;
    ++self.selectAnyCalls;
    self.selectAnyGeneration = generation;
    self.selectAnyIndex = index;
    [self recordCall:@"select_any"];
    if (generation != self.generation || index >= self.total) return nil;
    return @{@"handled": @YES, @"commit": [NSString stringWithFormat:@"%@%lu", self.prefix, (unsigned long)index],
             @"view": @{@"focused": @YES, @"session": @1, @"generation": @(self.generation + 1), @"editing_text": @"",
                        @"caret_position": @0, @"candidates": @[]}};
}
@end

static MSIMECandidateButton *ExpandedGridSubview(NSPanel *panel, NSInteger tag) {
    for (NSView *view in MSIMECandidateControlSubviews(panel.contentView))
        if ([view isKindOfClass:MSIMECandidateButton.class] && view.tag == tag) return (MSIMECandidateButton *)view;
    return nil;
}

static NSRect ExpandedGridFrame(NSPanel *panel, NSInteger tag) {
    MSIMECandidateButton *button = ExpandedGridSubview(panel, tag);
    return [button convertRect:button.bounds toView:MSIMECandidateControlsView(panel.contentView)];
}

static MSIMEExpandedCandidateScrollView *ExpandedGridScrollView(NSPanel *panel) {
    for (NSView *view in MSIMECandidateControlsView(panel.contentView).subviews)
        if ([view isKindOfClass:MSIMEExpandedCandidateScrollView.class]) return (id)view;
    return nil;
}

static NSUInteger ExpandedGridHighlightedTag(NSPanel *panel) {
    for (NSView *view in MSIMECandidateControlSubviews(panel.contentView))
        if ([view isKindOfClass:MSIMECandidateButton.class] && view.tag >= 0 && ((MSIMECandidateButton *)view).candidateHighlighted)
            return (NSUInteger)view.tag;
    return NSNotFound;
}

static NSEvent *ExpandedGridScrollEvent(CGFloat delta, NSEventPhase phase = NSEventPhaseChanged,
                                        NSEventPhase momentum = NSEventPhaseNone) {
    CGEventRef cg = CGEventCreateScrollWheelEvent(nullptr, kCGScrollEventUnitPixel, 1, (int32_t)delta);
    CGEventSetIntegerValueField(cg, kCGScrollWheelEventIsContinuous, 1);
    CGEventSetDoubleValueField(cg, kCGScrollWheelEventPointDeltaAxis1, delta);
    const int64_t scrollPhase = phase == NSEventPhaseBegan ? 1 : phase == NSEventPhaseChanged ? 2 :
        phase == NSEventPhaseEnded ? 4 : 0;
    const int64_t momentumPhase = momentum == NSEventPhaseBegan ? 1 : momentum == NSEventPhaseChanged ? 2 :
        momentum == NSEventPhaseEnded ? 3 : 0;
    CGEventSetIntegerValueField(cg, kCGScrollWheelEventScrollPhase, scrollPhase);
    CGEventSetIntegerValueField(cg, kCGScrollWheelEventMomentumPhase, momentumPhase);
    NSEvent *event = [NSEvent eventWithCGEvent:cg];
    CFRelease(cg);
    assert(event.hasPreciseScrollingDeltas && event.scrollingDeltaY == delta);
    assert(event.phase == phase && event.momentumPhase == momentum);
    return event;
}

static void ExpandedGridScroll(NSPanel *panel, CGFloat delta, NSEventPhase phase = NSEventPhaseChanged,
                               NSEventPhase momentum = NSEventPhaseNone) {
    if (phase == NSEventPhaseBegan) {
        [panel scrollWheel:ExpandedGridScrollEvent(0, NSEventPhaseBegan)];
        [NSRunLoop.currentRunLoop runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.05]];
        phase = NSEventPhaseChanged;
    }
    [panel scrollWheel:ExpandedGridScrollEvent(delta, phase, momentum)];
    [NSRunLoop.currentRunLoop runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.25]];
}

static void ExpandedGridFinishScroll(NSPanel *panel) {
    ExpandedGridScroll(panel, 0, NSEventPhaseEnded);
    NSScrollView *viewport = ExpandedGridScrollView(panel);
    const CGFloat maximum = MAX(0.0, NSHeight(viewport.documentView.bounds) - NSHeight(viewport.contentView.bounds));
    NSDate *deadline = [NSDate dateWithTimeIntervalSinceNow:1.5];
    while (deadline.timeIntervalSinceNow > 0 &&
        (NSMinY(viewport.contentView.bounds) < -0.01 || NSMinY(viewport.contentView.bounds) > maximum + 0.01))
        [NSRunLoop.currentRunLoop runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.02]];
    assert(NSMinY(viewport.contentView.bounds) >= -0.01 && NSMinY(viewport.contentView.bounds) <= maximum + 0.01);
}

// 普通候选把拼音编辑与选择分开，网格、列表和修饰键各走已有的入口。
static void TestPinyinArrowCandidateNavigation() {
    NSString *suite = [@"msime.pinyin-arrows." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    MSIMEAppearancePreferences *prefs = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    prefs.englishMode = NO;
    ModeController *controller = [ModeController alloc];
    ShortcutSession *session = [ShortcutSession new];
    ShortcutClient *client = [ShortcutClient new];
    TestCandidatePanel *panel = [TestCandidatePanel new];
    [controller setValue:prefs forKey:@"appearance"];
    [controller setValue:session forKey:@"session"];
    [controller setValue:client forKey:@"activeClient"];
    [controller setValue:panel forKey:@"panel"];
    auto check = [&](int scheme, NSUInteger caret, NSUInteger selected, unsigned short key, uint32_t expected, BOOL list = NO, NSUInteger page = 0) {
        [controller setValue:@{@"focused": @YES, @"scheme": @(scheme), @"editing_text": @"ceshi", @"caret_position": @(caret),
                              @"page": @(page), @"candidate_list_open": @(list), @"candidates": @[
                                  @{@"text": @"甲", @"highlighted": @(selected == 0)},
                                  @{@"text": @"乙", @"highlighted": @(selected == 1)}]} forKey:@"view"];
        panel.visible = YES;
        session.lastCommand = UINT32_MAX;
        session.nextTransition = @{@"handled": @YES};
        client.committed = nil;
        assert([controller handleEvent:ModeKey(key, 0, NO) client:client]);
        assert(session.lastCommand == expected && !client.committed);
    };
    // 默认交给 runtime；关闭后横排由宿主按同一条规则分工：首选上 ← 编辑、→ 先回到末尾，其余候选上左右键只移动候选。竖排时左右键与候选方向垂直，只编辑光标。
    for (NSNumber *leftRight in @[@YES, @NO]) {
        [prefs applySharedCandidatePreferences:@{@"navigation": @{@"left_right": leftRight}}];
        for (BOOL vertical : {NO, YES}) {
            prefs.vertical = vertical;
            const BOOL host = !leftRight.boolValue && !vertical;
            for (BOOL expand : {NO, YES}) {
                prefs.expandCandidates = expand;
                check(0, 5, 0, 123, MSIME_MOVE_LEFT);
                check(0, 5, 1, 123, host ? MSIME_PREVIOUS_CANDIDATE : MSIME_MOVE_LEFT);
                check(0, 2, 0, 124, MSIME_MOVE_RIGHT);
                check(0, 2, 1, 124, host ? MSIME_NEXT_CANDIDATE : MSIME_MOVE_RIGHT);
                // 第二页的第一个不是首选，左右键都移动候选。
                check(0, 5, 0, 123, host ? MSIME_PREVIOUS_CANDIDATE : MSIME_MOVE_LEFT, NO, 1);
                check(0, 2, 0, 124, host ? MSIME_NEXT_CANDIDATE : MSIME_MOVE_RIGHT, NO, 1);
                check(0, 5, 0, 124, host ? MSIME_NEXT_CANDIDATE : MSIME_MOVE_RIGHT);
                check(0, 5, 1, 124, host ? MSIME_NEXT_CANDIDATE : MSIME_MOVE_RIGHT);
            }
        }
    }
    // 五笔的左右键仍编辑光标；韩文汉字列表四方向都选择，不误用锁定的光标。
    check(2, 5, 1, 123, MSIME_MOVE_LEFT);
    check(2, 5, 0, 124, MSIME_MOVE_RIGHT);
    check(4, 5, 0, 123, MSIME_PREVIOUS_CANDIDATE, YES);
    check(4, 2, 0, 124, MSIME_NEXT_CANDIDATE, YES);
    [defaults removePersistentDomainForName:suite];
}

// 「展开候选」（#7307）：横排候选窗按 ↓ 展开多行网格，网格里 ←/→ 行内、↑/↓ 行间、数字选高亮行、空格和回车选高亮、Esc 或第一行 ↑ 收起而组字保留，网格不认的键先收起再照常处理；偏好关闭、竖排、只有一页时 ↓ 和 ←/→ 和以前一样；选择带着复制时的一代，过期的一代什么也不选。
static void TestExpandedCandidateGrid() {
    NSString *suite = [@"msime.expanded-grid." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    MSIMEAppearancePreferences *prefs = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    prefs.vertical = NO;
    prefs.pageSize = 5;
    prefs.englishMode = NO;
    assert(!prefs.expandCandidates);
    // 共享文档读写同一个值：缺字段是关，打开后写回 expand_candidates。
    assert([[prefs sharedPreferencesByMerging:@{}][@"expand_candidates"] isEqual:@NO]);
    [prefs applySharedCandidatePreferences:@{@"expand_candidates": @YES}];
    assert(prefs.expandCandidates);
    [prefs applySharedCandidatePreferences:@{@"expand_candidates": @"yes"}];
    assert(prefs.expandCandidates);
    [prefs applySharedCandidatePreferences:@{@"expand_candidates": @NO}];
    assert(!prefs.expandCandidates);
    NSButton *toggle = (id)PreferenceControl(prefs, NSSelectorFromString(@"expandCandidatesChanged:"));
    assert(toggle && toggle.state == NSControlStateValueOff);

    ModeController *controller = [ModeController alloc];
    ExpandedGridSession *session = [ExpandedGridSession new];
    session.generation = 7;
    session.total = 23;
    session.pageSize = 5;
    session.prefix = @"词";
    session.editing = @"shi";
    ShortcutClient *client = [ShortcutClient new];
    client.caret = NSMakeRect(200, 400, 1, 18);
    HiddenCandidatePanel *panel = [[HiddenCandidatePanel alloc] init];
    [controller setValue:prefs forKey:@"appearance"];
    [controller setValue:session forKey:@"session"];
    [controller setValue:client forKey:@"activeClient"];
    [controller setValue:panel forKey:@"panel"];
    void (^compose)(void) = ^{
        session.generation = 7;
        session.highlighted = 0;
        client.committed = nil;
        [controller setValue:[session gridView] forKey:@"view"];
        [controller renderCandidates];
        assert(panel.isVisible);
    };
    BOOL (^expanded)(void) = ^BOOL { return [controller valueForKey:@"expandedCandidates"] != nil; };
    NSUInteger (^cursor)(void) = ^NSUInteger { return [[controller valueForKey:@"expandedCursor"] unsignedIntegerValue]; };
    auto press = [&](unsigned short code) { return [controller handleEvent:ModeKey(code, 0, NO) client:client]; };

    // 偏好关闭：↓ 照旧移高亮，←/→ 按拼音光标和候选位置分工，不读完整候选，候选窗上也没有展开按钮。
    compose();
    assert(!ExpandedGridSubview(panel, -3));
    session.lastCommand = UINT32_MAX;
    assert(press(125));
    assert(session.lastCommand == MSIME_NEXT_CANDIDATE && session.allCandidateCalls == 0 && !expanded());
    compose();
    assert(press(123));
    assert(session.lastCommand == MSIME_MOVE_LEFT && !expanded());

    // 打开偏好但候选窗是竖排：↓ 照旧，没有展开按钮。
    prefs.expandCandidates = YES;
    prefs.vertical = YES;
    compose();
    assert(!ExpandedGridSubview(panel, -3));
    session.lastCommand = UINT32_MAX;
    assert(press(125));
    assert(session.lastCommand == MSIME_NEXT_CANDIDATE && session.allCandidateCalls == 0 && !expanded());
    prefs.vertical = NO;

    // 只有一页：没有可展开的，↓ 照旧。
    session.total = 4;
    compose();
    assert(!ExpandedGridSubview(panel, -3));
    session.lastCommand = UINT32_MAX;
    assert(press(125));
    assert(session.lastCommand == MSIME_NEXT_CANDIDATE && !expanded());
    session.total = 23;

    // 横排、多页：Tab 直接展开完整候选网格，Shift+Tab 仍保留上一页语义；光标在末尾时 → 选择下一个候选，首行有展开按钮。
    compose();
    MSIMECandidateButton *expandButton = ExpandedGridSubview(panel, -3);
    assert(expandButton && [expandButton.identifier isEqual:@"candidate-expand"] && [expandButton.accessibilityLabel isEqual:@"展开候选"]);
    session.lastCommand = UINT32_MAX;
    const NSUInteger tabReads = session.allCandidateCalls;
    assert(press(48) && expanded() && session.allCandidateCalls == tabReads + 1 && session.lastCommand == UINT32_MAX);
    [controller closeExpandedCandidates];
    session.lastCommand = UINT32_MAX;
    assert(press(124));
    assert(session.lastCommand == MSIME_MOVE_RIGHT && !expanded());

    compose();
    session.highlighted = 21;
    NSMutableDictionary *lastPage = [[session gridView] mutableCopy];
    lastPage[@"page"] = @4;
    lastPage[@"candidates"] = @[[session gridCandidate:20 highlighted:NO], [session gridCandidate:21 highlighted:YES], [session gridCandidate:22 highlighted:NO]];
    [controller setValue:lastPage forKey:@"view"];
    [controller renderCandidates];
    assert(press(125) && expanded());
    assert(cursor() == 21 && [[controller valueForKey:@"expandedFirstRow"] unsignedIntegerValue] == 4);
    assert(ExpandedGridSubview(panel, 21) && !ExpandedGridSubview(panel, 19));
    assert(press(126) && cursor() == 16);
    assert([[controller valueForKey:@"expandedFirstRow"] unsignedIntegerValue] == 3);
    [controller closeExpandedCandidates];

    // 页码隐藏仍能用鼠标展开；全隐藏首行时按钮留在候选行，不增加高度。
    prefs.showsCandidatePageNumber = NO;
    prefs.showsAppLogo = NO;
    prefs.showsCandidatePreedit = NO;
    assert(prefs.showsCandidateExpandButton);
    compose();
    MSIMECandidateButton *compactButton = ExpandedGridSubview(panel, -3);
    assert(compactButton && !ExpandedGridSubview(panel, -1) && !ExpandedGridSubview(panel, -2));
    MSIMECandidateButton *first = ExpandedGridSubview(panel, 0);
    assert(first && NSMinY(compactButton.frame) >= NSMinY(first.frame) && NSMaxY(compactButton.frame) <= NSMaxY(first.frame));
    const CGFloat compactHeight = panel.frame.size.height;
    [controller toggleExpandedCandidates:compactButton];
    assert(expanded());
    [controller closeExpandedCandidates];
    prefs.showsCandidateExpandButton = NO;
    compose();
    assert(!ExpandedGridSubview(panel, -3) && panel.frame.size.height == compactHeight);
    assert(press(125) && expanded());
    [controller closeExpandedCandidates];
    prefs.showsCandidateExpandButton = YES;
    prefs.showsCandidatePageNumber = YES;
    prefs.showsCandidatePreedit = YES;

    // ↓ 展开：读一次完整候选，不发任何 Engine 命令；23 个候选排成 5 行，高亮停在原来高亮的第一个。
    compose();
    session.lastCommand = UINT32_MAX;
    const NSUInteger reads = session.allCandidateCalls;
    assert(press(125));
    assert(expanded() && session.allCandidateCalls == reads + 1 && session.lastCommand == UINT32_MAX);
    assert(cursor() == 0 && ExpandedGridHighlightedTag(panel) == 0);
    assert(ExpandedGridSubview(panel, 22) && !ExpandedGridSubview(panel, 23));
    MSIMECandidateButton *collapse = ExpandedGridSubview(panel, -3);
    assert(collapse && [collapse.identifier isEqual:@"candidate-collapse"]);
    // 只有高亮那一行标序号，数字键选的就是那一行。
    assert([ExpandedGridSubview(panel, 1).title hasPrefix:@"2  "] && [ExpandedGridSubview(panel, 6).title hasPrefix:@"  "]);

    // 触控板的精确滚动保留行内像素偏移，滚回时位置恢复；用六行网格覆盖虚拟窗口边界。
    [controller closeExpandedCandidates];
    session.total = 24;
    session.pageSize = 4;
    prefs.pageSize = 4;
    compose();
    assert(press(125) && expanded());
    const CGFloat firstCandidateY = NSMinY(ExpandedGridFrame(panel, 0));
    const CGFloat viewportHeight = panel.frame.size.height;
    MSIMEExpandedCandidateScrollView *originalScrollView = ExpandedGridScrollView(panel);
    MSIMECandidateButton *originalFirst = ExpandedGridSubview(panel, 0);
    NSView *originalContent = panel.contentView;
    const NSUInteger caretReads = client.attributeReads;
    ExpandedGridScroll(panel, -8, NSEventPhaseBegan);
    assert([[controller valueForKey:@"expandedScrollOffset"] doubleValue] > 0.0);
    assert(fabs(NSMinY(ExpandedGridFrame(panel, 0)) - (firstCandidateY + 8.0)) < 0.01);
    assert(originalScrollView && ExpandedGridScrollView(panel) == originalScrollView);
    assert(panel.contentView == originalContent && ExpandedGridSubview(panel, 0) == originalFirst);
    assert(client.attributeReads == caretReads);
    assert(ExpandedGridSubview(panel, 20));
    NSScrollView *viewport = ExpandedGridScrollView(panel);
    assert(viewport.contentView.clipsToBounds && !viewport.acceptsFirstResponder);
    assert(viewport.verticalScrollElasticity == NSScrollElasticityAllowed);
    assert(viewport.horizontalScrollElasticity == NSScrollElasticityNone);
    const CGFloat scrollerPadding = MSIMEExpandedCandidateScrollerPadding(prefs.candidateScalePercent / 100.0);
    assert(scrollerPadding >= [NSScroller scrollerWidthForControlSize:NSControlSizeRegular
        scrollerStyle:NSScrollerStyleOverlay] + 4 * prefs.candidateScalePercent / 100.0);
    const auto rightClearance = [&](NSUInteger tag) {
        MSIMECandidateButton *button = ExpandedGridSubview(panel, tag);
        NSScrollView *scrollView = ExpandedGridScrollView(panel);
        assert(button && scrollView);
        const NSRect cell = [button convertRect:button.bounds toView:scrollView];
        assert(NSMaxX(cell) <= NSWidth(scrollView.bounds) - scrollerPadding + 0.01);
        const NSRect scroller = [scrollView.verticalScroller convertRect:scrollView.verticalScroller.bounds toView:scrollView];
        if (!NSIsEmptyRect(scroller)) assert(NSMaxX(cell) <= NSMinX(scroller) - 4 * prefs.candidateScalePercent / 100.0 + 0.01);
    };
    rightClearance(3);
    NSView *controls = MSIMECandidateControlsView(panel.contentView);
    MSIMECandidateButton *top = ExpandedGridSubview(panel, 0);
    const NSRect topRect = [top convertRect:top.bounds toView:controls];
    const NSRect viewportRect = [viewport convertRect:viewport.bounds toView:controls];
    assert(NSMaxY(topRect) > NSMaxY(viewportRect));
    const NSPoint clippedPoint = NSMakePoint(NSMidX(topRect), NSMaxY(viewportRect) + 1);
    assert([controls hitTest:[controls convertPoint:clippedPoint toView:controls.superview]] != top);
    ExpandedGridScroll(panel, 8);
    assert(fabs([[controller valueForKey:@"expandedScrollOffset"] doubleValue]) < 0.01);
    assert(fabs(NSMinY(ExpandedGridFrame(panel, 0)) - firstCandidateY) < 0.01);
    ExpandedGridScroll(panel, -10000);
    assert([[controller valueForKey:@"expandedFirstRow"] unsignedIntegerValue] == 1);
    assert(fabs([[controller valueForKey:@"expandedScrollOffset"] doubleValue]) < 0.01);
    rightClearance(23);
    assert(panel.frame.size.height == viewportHeight);
    ExpandedGridFinishScroll(panel);
    assert([[controller valueForKey:@"expandedFirstRow"] unsignedIntegerValue] == 1);
    // 停在末尾后新开的向下手势仍交给 AppKit，不能按 hasNextPage 拦截。
    MSIMEExpandedCandidateScrollView *bottomViewport = ExpandedGridScrollView(panel);
    void (^manualScroll)(void) = bottomViewport.manualScrollBegan;
    __block NSUInteger bottomEvents = 0;
    bottomViewport.manualScrollBegan = ^{
        ++bottomEvents;
        manualScroll();
    };
    ExpandedGridScroll(panel, -8, NSEventPhaseBegan);
    assert(bottomEvents >= 2 && ExpandedGridScrollView(panel) == bottomViewport);
    assert(NSMinY(bottomViewport.contentView.bounds) < 0);
    assert([[controller valueForKey:@"expandedFirstRow"] unsignedIntegerValue] == 1);
    assert(!panel.hasNextPage && panel.frame.size.height == viewportHeight);
    ExpandedGridFinishScroll(panel);
    bottomViewport.manualScrollBegan = manualScroll;
    [controller renderCandidates];
    assert([[controller valueForKey:@"expandedFirstRow"] unsignedIntegerValue] == 1);
    ExpandedGridScroll(panel, 0, NSEventPhaseNone, NSEventPhaseEnded);
    assert([[controller valueForKey:@"expandedFirstRow"] unsignedIntegerValue] == 1);
    ExpandedGridScroll(panel, 10000);
    assert([[controller valueForKey:@"expandedFirstRow"] unsignedIntegerValue] == 0);
    assert(fabs([[controller valueForKey:@"expandedScrollOffset"] doubleValue]) < 0.01);
    ExpandedGridScroll(panel, 10000);
    ExpandedGridFinishScroll(panel);
    assert(fabs(NSMinY(ExpandedGridFrame(panel, 0)) - firstCandidateY) < 0.01);

    const auto lineScroll = [&](BOOL backwards) {
        CGEventRef cg = CGEventCreateScrollWheelEvent(nullptr, kCGScrollEventUnitLine, 1, backwards ? 1 : -1);
        NSEvent *event = [NSEvent eventWithCGEvent:cg];
        CFRelease(cg);
        [panel scrollWheel:event];
    };
    // 到底后的无效普通滚轮不能丢掉手动视口，之后重画也不能拉回旧高亮。
    ExpandedGridScroll(panel, -10000, NSEventPhaseBegan);
    ExpandedGridFinishScroll(panel);
    assert(cursor() == 0 && [[controller valueForKey:@"expandedFirstRow"] unsignedIntegerValue] == 1);
    const CGFloat bottomY = NSMinY(ExpandedGridFrame(panel, 20));
    MSIMEExpandedCandidateScrollView *bottomScroll = ExpandedGridScrollView(panel);
    lineScroll(NO);
    assert(ExpandedGridScrollView(panel) == bottomScroll);
    assert([[controller valueForKey:@"expandedScrollsViewport"] boolValue]);
    [controller renderCandidates];
    assert([[controller valueForKey:@"expandedFirstRow"] unsignedIntegerValue] == 1);
    assert(fabs(NSMinY(ExpandedGridFrame(panel, 20)) - bottomY) < 0.01);

    // 末行特别高时，旧可见行数会误报到底；普通滚轮按真实行高继续，且不改变视口高度。
    [controller closeExpandedCandidates];
    session.tallIndex = 20;
    compose();
    assert(press(125) && expanded());
    NSMutableArray<NSDictionary *> *tallLastRows = [[controller valueForKey:@"expandedCandidates"] mutableCopy];
    NSMutableDictionary *tallLast = [tallLastRows[20] mutableCopy];
    tallLast[@"text"] = [@"合成" stringByPaddingToLength:400 withString:@"合成" startingAtIndex:0];
    tallLastRows[20] = tallLast;
    [controller setValue:tallLastRows forKey:@"expandedCandidates"];
    [controller renderCandidates];
    const CGFloat fixedHeight = panel.frame.size.height;
    const CGFloat firstHeight = NSMinY(ExpandedGridFrame(panel, 0)) - NSMinY(ExpandedGridFrame(panel, 4));
    ExpandedGridScroll(panel, -firstHeight, NSEventPhaseBegan);
    ExpandedGridFinishScroll(panel);
    assert([[controller valueForKey:@"expandedFirstRow"] unsignedIntegerValue] == 1 && panel.hasNextPage);
    lineScroll(NO);
    assert([[controller valueForKey:@"expandedFirstRow"] unsignedIntegerValue] == 2);
    assert(cursor() >= 8 && panel.frame.size.height == fixedHeight);
    const CGFloat secondRowY = NSMinY(ExpandedGridFrame(panel, 8));
    [controller renderCandidates];
    assert(fabs(NSMinY(ExpandedGridFrame(panel, 8)) - secondRowY) < 0.01);
    for (NSUInteger step = 0; step < 10 && panel.hasNextPage; ++step) lineScroll(NO);
    assert(!panel.hasNextPage && panel.frame.size.height == fixedHeight);
    const CGFloat lastBottomY = NSMinY(ExpandedGridFrame(panel, 20));
    lineScroll(NO);
    [controller renderCandidates];
    assert(fabs(NSMinY(ExpandedGridFrame(panel, 20)) - lastBottomY) < 0.01);
    lineScroll(YES);
    assert(panel.hasNextPage && panel.frame.size.height == fixedHeight);
    assert(press(116) && cursor() == 0);
    session.tallIndex = 0;

    // 跨行前后同一候选的位置连续；反向滚动沿用同一像素视口。
    [controller closeExpandedCandidates];
    session.total = 40;
    compose();
    assert(press(125) && expanded());
    const CGFloat rowHeight = NSMinY(ExpandedGridFrame(panel, 0)) - NSMinY(ExpandedGridFrame(panel, 4));
    ExpandedGridScroll(panel, -(rowHeight - 1), NSEventPhaseBegan);
    const CGFloat beforeCrossing = NSMinY(ExpandedGridFrame(panel, 4));
    MSIMECandidateButton *reused = ExpandedGridSubview(panel, 4);
    ExpandedGridScroll(panel, -2);
    assert([[controller valueForKey:@"expandedFirstRow"] unsignedIntegerValue] == 1);
    assert(ExpandedGridSubview(panel, 4) == reused);
    assert(fabs(NSMinY(ExpandedGridFrame(panel, 4)) - beforeCrossing - 2) < 0.01);
    assert(fabs([[controller valueForKey:@"expandedScrollOffset"] doubleValue] - 1) < 0.01);
    ExpandedGridScroll(panel, 2);
    assert([[controller valueForKey:@"expandedFirstRow"] unsignedIntegerValue] == 0);
    assert(fabs(NSMinY(ExpandedGridFrame(panel, 4)) - beforeCrossing) < 0.01);
    assert(panel.frame.size.height == viewportHeight);
    assert(!ExpandedGridSubview(panel, 24));
    assert(press(125) && cursor() == 4);
    assert(fabs([[controller valueForKey:@"expandedScrollOffset"] doubleValue]) < 0.01);

    // 首行折行后高于其它行，跨行仍使用该行真实高度，不假设等高。
    [controller closeExpandedCandidates];
    session.total = 200;
    compose();
    assert(press(125) && expanded());
    NSMutableArray<NSDictionary *> *uneven = [[controller valueForKey:@"expandedCandidates"] mutableCopy];
    NSMutableDictionary *tall = [uneven[0] mutableCopy];
    tall[@"text"] = [@"合成" stringByPaddingToLength:400 withString:@"合成" startingAtIndex:0];
    uneven[0] = tall;
    [controller setValue:uneven forKey:@"expandedCandidates"];
    [controller renderCandidates];
    const CGFloat tallHeight = NSMaxY(ExpandedGridFrame(panel, 0)) - NSMaxY(ExpandedGridFrame(panel, 4));
    const CGFloat normalHeight = NSMaxY(ExpandedGridFrame(panel, 4)) - NSMaxY(ExpandedGridFrame(panel, 8));
    assert(tallHeight > normalHeight);
    assert(ExpandedGridSubview(panel, 0).itemLayout.textWrapped);
    for (NSUInteger slot = 0; slot < 4; ++slot)
        assert(NSMaxX(ExpandedGridFrame(panel, slot)) <= NSWidth(ExpandedGridScrollView(panel).bounds) + 0.01);
    assert(panel.frame.size.width <= NSScreen.mainScreen.visibleFrame.size.width);
    assert(ExpandedGridScrollView(panel).documentView.subviews.count < session.total);
    ExpandedGridScroll(panel, -(tallHeight - 1), NSEventPhaseBegan);
    const CGFloat beforeUnevenCrossing = NSMinY(ExpandedGridFrame(panel, 4));
    ExpandedGridScroll(panel, -2);
    assert([[controller valueForKey:@"expandedFirstRow"] unsignedIntegerValue] == 1);
    assert(fabs(NSMinY(ExpandedGridFrame(panel, 4)) - beforeUnevenCrossing - 2) < 0.01);
    ExpandedGridScroll(panel, 2);
    assert([[controller valueForKey:@"expandedFirstRow"] unsignedIntegerValue] == 0);
    assert(fabs(NSMinY(ExpandedGridFrame(panel, 4)) - beforeUnevenCrossing) < 0.01);

    // 每列跨所有行取共享宽度；最长候选可以在尚未进入视口的行，短末行仍沿用列边界。
    [controller closeExpandedCandidates];
    session.total = 39;
    compose();
    assert(press(125) && expanded());
    NSMutableArray<NSDictionary *> *aligned = [[controller valueForKey:@"expandedCandidates"] mutableCopy];
    for (NSUInteger index = 0; index < aligned.count; ++index) {
        NSMutableDictionary *entry = [aligned[index] mutableCopy];
        entry[@"text"] = [@"合成" stringByPaddingToLength:(index == 29 ? 20 : 2 + index % 5)
            withString:@"合成" startingAtIndex:0];
        aligned[index] = entry;
    }
    [controller setValue:aligned forKey:@"expandedCandidates"];
    [controller renderCandidates];
    std::vector<NSRect> columns;
    for (NSUInteger slot = 0; slot < 4; ++slot) {
        columns.push_back(ExpandedGridFrame(panel, slot));
        for (NSUInteger row = 1; row < 3; ++row) {
            const NSRect cell = ExpandedGridFrame(panel, row * 4 + slot);
            assert(fabs(NSMinX(cell) - NSMinX(columns[slot])) < 0.01);
            assert(fabs(NSWidth(cell) - NSWidth(columns[slot])) < 0.01);
            assert(fabs(ExpandedGridSubview(panel, row * 4 + slot).contentLeft -
                ExpandedGridSubview(panel, slot).contentLeft) < 0.01);
            assert(NSMaxX(cell) <= NSWidth(ExpandedGridScrollView(panel).bounds) + 0.01);
        }
    }
    assert(NSWidth(columns[1]) > NSWidth(columns[0]));
    const CGFloat alignedWidth = panel.frame.size.width;
    ExpandedGridScroll(panel, -10000, NSEventPhaseBegan);
    ExpandedGridFinishScroll(panel);
    for (NSUInteger slot = 0; slot < 3; ++slot) {
        const NSRect cell = ExpandedGridFrame(panel, 36 + slot);
        assert(ExpandedGridSubview(panel, 36 + slot));
        assert(fabs(NSMinX(cell) - NSMinX(columns[slot])) < 0.01);
        assert(fabs(NSWidth(cell) - NSWidth(columns[slot])) < 0.01);
    }
    assert(panel.frame.size.width == alignedWidth && !ExpandedGridSubview(panel, 39));
    rightClearance(35);
    assert(fabs(ExpandedGridSubview(panel, 36).contentLeft - ExpandedGridSubview(panel, 32).contentLeft) < 0.01);

    // 从触控板切换到普通滚轮仍按行移动，回到顶部时也清掉剩余像素。
    [controller closeExpandedCandidates];
    session.total = 24;
    compose();
    assert(press(125) && expanded());
    ExpandedGridScroll(panel, -8, NSEventPhaseBegan);
    CGEventRef lineCG = CGEventCreateScrollWheelEvent(nullptr, kCGScrollEventUnitLine, 1, 1);
    NSEvent *lineEvent = [NSEvent eventWithCGEvent:lineCG];
    CFRelease(lineCG);
    assert(!lineEvent.hasPreciseScrollingDeltas);
    [panel scrollWheel:lineEvent];
    assert([[controller valueForKey:@"expandedFirstRow"] unsignedIntegerValue] == 0);
    assert(fabs([[controller valueForKey:@"expandedScrollOffset"] doubleValue]) < 0.01);
    assert(fabs(NSMinY(ExpandedGridFrame(panel, 0)) - firstCandidateY) < 0.01);

    // 从短末页打开也固定当前像素高度，回滚八点不能把窗口扩高一整行。
    [controller closeExpandedCandidates];
    session.total = 24;
    session.highlighted = 20;
    [controller setValue:[session gridView] forKey:@"view"];
    [controller renderCandidates];
    assert(press(125) && expanded());
    const CGFloat shortHeight = panel.frame.size.height;
    const CGFloat lastY = NSMinY(ExpandedGridFrame(panel, 20));
    ExpandedGridScroll(panel, 8, NSEventPhaseBegan);
    assert(fabs(panel.frame.size.height - shortHeight) < 0.01);
    assert(fabs(NSMinY(ExpandedGridFrame(panel, 20)) - lastY + 8) < 0.01);
    assert([[controller valueForKey:@"expandedFirstRow"] unsignedIntegerValue] == 4);
    ExpandedGridScroll(panel, 0, NSEventPhaseEnded);
    assert([[controller valueForKey:@"expandedVisibleRows"] unsignedIntegerValue] == 1);
    ExpandedGridScroll(panel, -8, NSEventPhaseBegan);
    assert([[controller valueForKey:@"expandedFirstRow"] unsignedIntegerValue] == 5);
    assert(fabs(NSMinY(ExpandedGridFrame(panel, 20)) - lastY) < 0.01);
    MSIMECandidateButton *stale = ExpandedGridSubview(panel, 20);
    [controller renderCandidates];
    const NSUInteger selectedBeforeStale = session.selectAnyCalls;
    [controller selectExpandedCandidate:stale];
    assert(session.selectAnyCalls == selectedBeforeStale && expanded());
    [controller closeExpandedCandidates];
    session.total = 20;
    compose();
    assert(press(125) && expanded());
    const CGFloat nonScrollingY = NSMinY(ExpandedGridFrame(panel, 0));
    ExpandedGridScroll(panel, -8, NSEventPhaseBegan);
    assert(!panel.hasPreviousPage && !panel.hasNextPage);
    ExpandedGridFinishScroll(panel);
    assert(fabs(NSMinY(ExpandedGridFrame(panel, 0)) - nonScrollingY) < 0.01);
    [controller closeExpandedCandidates];
    // 原生滚动条同样经裁剪视口定位，重画不能又跳回高亮行。
    session.total = 40;
    compose();
    assert(press(125) && expanded());
    assert(![[controller valueForKey:@"expandedScrollsViewport"] boolValue]);
    MSIMEExpandedCandidateScrollView *scroller = ExpandedGridScrollView(panel);
    NSPoint origin = scroller.contentView.bounds.origin;
    origin.y -= 8;
    [scroller.contentView scrollToPoint:origin];
    [scroller reflectScrolledClipView:scroller.contentView];
    [NSRunLoop.currentRunLoop runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.05]];
    assert([[controller valueForKey:@"expandedScrollsViewport"] boolValue]);
    assert(fabs([[controller valueForKey:@"expandedScrollOffset"] doubleValue] - 8) < 0.01);
    [controller renderCandidates];
    assert(fabs([[controller valueForKey:@"expandedScrollOffset"] doubleValue] - 8) < 0.01);
    // 字号变更开启新的布局预算，后续像素滚动仍保持这个新高度。
    ExpandedGridFinishScroll(panel);
    const CGFloat viewportRowsHeight = NSHeight(((MSIMECandidateChromeView *)panel.contentView).candidateRowsRect);
    prefs.candidateScalePercent = 150;
    [controller appearanceChanged:nil];
    const CGFloat scaledViewportHeight = panel.frame.size.height;
    assert(NSHeight(((MSIMECandidateChromeView *)panel.contentView).candidateRowsRect) > viewportRowsHeight * 1.4);
    ExpandedGridScroll(panel, -8, NSEventPhaseBegan);
    ExpandedGridFinishScroll(panel);
    assert(fabs(panel.frame.size.height - scaledViewportHeight) < 0.01);
    prefs.candidateScalePercent = 100;
    [controller appearanceChanged:nil];
    [controller closeExpandedCandidates];
    session.total = 23;
    session.pageSize = 5;
    prefs.pageSize = 5;
    session.lastCommand = UINT32_MAX;
    compose();
    assert(press(125) && expanded());

    // →、↓ 移动；最后一行短时停在它的最后一个；← 在行内；↑ 回到上一行同一列。
    assert(press(124) && cursor() == 1);
    assert(press(125) && cursor() == 6);
    assert(press(125) && press(125) && cursor() == 16);
    assert(press(125) && cursor() == 21);
    assert(press(125) && cursor() == 21);
    assert(press(123) && cursor() == 20);
    assert(press(123) && cursor() == 20);
    assert(press(126) && cursor() == 15 && ExpandedGridHighlightedTag(panel) == 15);
    assert([ExpandedGridSubview(panel, 16).title hasPrefix:@"2  "] && [ExpandedGridSubview(panel, 1).title hasPrefix:@"  "]);
    // 翻页键一次滚过可见的几行（这里 5 行都可见），到头为止、不收起。
    assert(press(116) && cursor() == 0 && expanded());
    assert(press(121) && cursor() == 20);
    // 网格里的移动一个 Engine 命令也没发。
    assert(session.lastCommand == UINT32_MAX && session.selectCalls == 0);

    // 数字键选高亮那一行里的第几个，带着复制时的一代走 select_any。
    assert(press(126) && cursor() == 15);
    client.committed = nil;
    assert(press(20)); // 3
    assert(session.selectAnyCalls == 1 && session.selectAnyGeneration == 7 && session.selectAnyIndex == 17);
    assert([client.committed isEqual:@"词17"] && !expanded() && session.selectCalls == 0);

    // 空格和回车选高亮的那一个。
    compose();
    assert(press(125) && press(125) && cursor() == 5);
    assert(press(49));
    assert(session.selectAnyCalls == 2 && session.selectAnyIndex == 5 && !expanded());
    compose();
    assert(press(125) && press(124) && cursor() == 1);
    assert(press(36));
    assert(session.selectAnyCalls == 3 && session.selectAnyIndex == 1 && session.rawCommitCalls == 0 && !expanded());

    // Esc 收起网格，组字和候选都在，也没有取消组字。
    compose();
    session.lastCommand = UINT32_MAX;
    assert(press(125) && expanded());
    assert(press(53));
    assert(!expanded() && session.lastCommand == UINT32_MAX && panel.isVisible);
    assert([[controller valueForKey:@"view"][@"editing_text"] isEqual:@"shi"]);
    // 第一行再按 ↑ 同样只收起；收起后 ↑ 回到原来的规则。
    assert(press(125) && expanded());
    assert(press(126));
    assert(!expanded() && session.lastCommand == UINT32_MAX && ExpandedGridSubview(panel, -3));
    assert(press(126));
    assert(session.lastCommand == MSIME_PREVIOUS_CANDIDATE);

    // #9206：第三个候选展开后左移到第一个，↑ 收起仍高亮第一个；再次展开不回跳。
    compose();
    session.highlighted = 2;
    [controller setValue:[session gridView] forKey:@"view"];
    [controller renderCandidates];
    assert(press(125) && cursor() == 2);
    assert(press(123) && press(123) && cursor() == 0);
    assert(press(126) && !expanded());
    assert(session.highlighted == 0 && ExpandedGridHighlightedTag(panel) == 0 && !client.committed);
    assert(press(125) && cursor() == 0);
    // Esc 和鼠标收起也同步跨页、短末页上的高亮。
    assert(press(125) && press(124) && cursor() == 6);
    assert(press(53) && !expanded());
    assert(session.highlighted == 6 && [[[controller valueForKey:@"view"] objectForKey:@"page"] isEqual:@1]);
    assert(press(125) && cursor() == 6);
    [controller setValue:@22 forKey:@"expandedCursor"];
    [controller toggleExpandedCandidates:ExpandedGridSubview(panel, -3)];
    assert(!expanded() && session.highlighted == 22 && !client.committed);
    assert(press(125) && cursor() == 22);
    assert(press(126) && cursor() == 17);
    [controller closeExpandedCandidates];
    assert(session.highlighted == 17 && !client.committed);

    // 网格不认的键先收起网格，再照常交给 Engine。
    compose();
    assert(press(125) && expanded());
    const NSUInteger letters = session.asciiCalls;
    [controller handleEvent:ModeKey(0, 0, NO) client:client]; // 'a'
    assert(!expanded() && session.asciiCalls == letters + 1);

    // 鼠标：首行的展开按钮打开网格，点网格里的候选先报告指针再选，收起按钮收起。
    compose();
    [controller toggleExpandedCandidates:ExpandedGridSubview(panel, -3)];
    assert(expanded() && cursor() == 0);
    [controller toggleExpandedCandidates:ExpandedGridSubview(panel, -3)];
    assert(!expanded());
    [controller toggleExpandedCandidates:ExpandedGridSubview(panel, -3)];
    assert(expanded());
    session.calls = nil;
    [controller selectExpandedCandidate:ExpandedGridSubview(panel, 12)];
    assert(session.selectAnyCalls == 4 && session.selectAnyIndex == 12 && !expanded());
    NSString *pointerKey = [NSString stringWithFormat:@"key:%u", MSIME_KEY_POINTER];
    assert([session.calls containsObject:@"select_any"] &&
           [session.calls indexOfObject:pointerKey] < [session.calls indexOfObject:@"select_any"]);

    // 关掉偏好或改成竖排后，下一次重画收起网格。
    compose();
    assert(press(125) && expanded());
    prefs.expandCandidates = NO;
    [controller renderCandidates];
    assert(!expanded());
    prefs.expandCandidates = YES;
    assert(press(125) && expanded());
    prefs.vertical = YES;
    [controller renderCandidates];
    assert(!expanded());
    prefs.vertical = NO;
    MSIMERemoveTestPreferenceSuite(defaults, suite);
}

// 展开的网格只认复制时的那一代（#7307）：组字不变而候选换了一代（云候选、AI 回答、整句重排）时按新的一代重载，高亮留在原来那个词上；组字变了就收起；视图已经换了一代而网格还没重画时点到的候选什么也不选。
static void TestExpandedCandidateGridGeneration() {
    NSString *suite = [@"msime.expanded-grid-generation." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    MSIMEAppearancePreferences *prefs = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    prefs.vertical = NO;
    prefs.pageSize = 5;
    prefs.englishMode = NO;
    prefs.expandCandidates = YES;
    ModeController *controller = [ModeController alloc];
    ExpandedGridSession *session = [ExpandedGridSession new];
    session.generation = 3;
    session.total = 12;
    session.pageSize = 5;
    session.prefix = @"甲";
    session.editing = @"jia";
    ShortcutClient *client = [ShortcutClient new];
    client.caret = NSMakeRect(200, 400, 1, 18);
    HiddenCandidatePanel *panel = [[HiddenCandidatePanel alloc] init];
    [controller setValue:prefs forKey:@"appearance"];
    [controller setValue:session forKey:@"session"];
    [controller setValue:client forKey:@"activeClient"];
    [controller setValue:panel forKey:@"panel"];
    BOOL (^expanded)(void) = ^BOOL { return [controller valueForKey:@"expandedCandidates"] != nil; };
    NSUInteger (^cursor)(void) = ^NSUInteger { return [[controller valueForKey:@"expandedCursor"] unsignedIntegerValue]; };
    auto press = [&](unsigned short code) { return [controller handleEvent:ModeKey(code, 0, NO) client:client]; };
    [controller setValue:[session gridView] forKey:@"view"];
    [controller renderCandidates];
    assert(press(125) && press(125) && cursor() == 5);
    assert([[controller valueForKey:@"expandedGeneration"] isEqual:@3]);

    // 同一组字的新一代到达（异步结果经 apply:）：网格留着、换成新的一代，高亮还是「甲5」。新的一代多了两个候选。
    session.generation = 4;
    session.total = 14;
    const NSUInteger reads = session.allCandidateCalls;
    [controller apply:@{@"view": [session gridView]}];
    assert(expanded() && session.allCandidateCalls == reads + 1);
    assert([[controller valueForKey:@"expandedGeneration"] isEqual:@4] && cursor() == 5);
    assert(ExpandedGridSubview(panel, 13) && ExpandedGridHighlightedTag(panel) == 5);
    // 网格里的移动不再读完整候选。
    assert(press(124) && cursor() == 6 && session.allCandidateCalls == reads + 1);
    // 读屏跟着网格的光标走：光标那一格报告被选中，刚离开的那一格不再报告，并请求朗读新的那一格。
    assert(ExpandedGridSubview(panel, 6).isAccessibilitySelected && !ExpandedGridSubview(panel, 5).isAccessibilitySelected);
    assert([[controller valueForKey:@"announcedCandidate"] isEqual:ExpandedGridSubview(panel, 6).accessibilityLabel]);
    // 选择带着新的一代。
    assert(press(49));
    assert(session.selectAnyCalls == 1 && session.selectAnyGeneration == 4 && session.selectAnyIndex == 6 && !expanded());

    // 新的一代里原来的词没了（换了一批文字）：高亮夹在范围内。
    session.generation = 5;
    session.total = 12;
    [controller setValue:[session gridView] forKey:@"view"];
    [controller renderCandidates];
    assert(press(125) && press(125) && press(125) && press(124) && cursor() == 11);
    session.generation = 6;
    session.total = 9;
    session.prefix = @"乙";
    [controller apply:@{@"view": [session gridView]}];
    assert(expanded() && cursor() == 8 && [[controller valueForKey:@"expandedGeneration"] isEqual:@6]);

    // 组字变了（比如又打了一个字母）：网格收起，普通候选窗照常显示新的一页。
    session.generation = 7;
    session.editing = @"jian";
    [controller apply:@{@"view": [session gridView]}];
    assert(!expanded() && panel.isVisible && ExpandedGridSubview(panel, -3));

    // 视图已经是新的一代、网格还没重画时点网格里的候选：不调 select_any，网格收起。
    [controller renderCandidates];
    [controller toggleExpandedCandidates:ExpandedGridSubview(panel, -3)];
    assert(expanded());
    MSIMECandidateButton *stale = ExpandedGridSubview(panel, 7);
    assert(stale);
    session.generation = 8;
    session.editing = @"jiang";
    [controller setValue:[session gridView] forKey:@"view"];
    const NSUInteger picks = session.selectAnyCalls;
    [controller selectExpandedCandidate:stale];
    assert(session.selectAnyCalls == picks && !expanded());

    // Engine 自己拒绝过期的一代（复制之后它又换了一代）：什么也不上屏，网格收起，组字还在。
    [controller renderCandidates];
    assert(press(125) && expanded());
    session.generation = 9;
    client.committed = nil;
    assert(press(49));
    assert(session.selectAnyCalls == picks + 1 && session.selectAnyGeneration == 8 && !client.committed && !expanded());
    assert([[controller valueForKey:@"view"][@"editing_text"] isEqual:@"jiang"]);

    // 焦点换到另一个客户端：旧客户端的组字随失焦结束，网格跟着收起。
    session.generation = 10;
    [controller setValue:[session gridView] forKey:@"view"];
    [controller renderCandidates];
    assert(press(125) && expanded());
    ShortcutClient *other = [ShortcutClient new];
    other.caret = NSMakeRect(300, 400, 1, 18);
    [controller handleEvent:ModeKey(0, 0, NO) client:other];
    assert(!expanded());
    MSIMERemoveTestPreferenceSuite(defaults, suite);
}

static void TestSegmentEditingChords(MSIMEAppearancePreferences *appearance) {
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
    NSDictionary *composing = @{ @"focused": @YES, @"editing_text": @"nihao", @"caret_position": @5,
                                 @"candidates": @[@{@"text": @"你好"}] };
    [controller setValue:composing forKey:@"view"];

    // 全拼（视图不带方案时按全拼读）里 Ctrl+左右和 Shift+左右都按音节移动拼音光标，发分段命令：逐个字母的光标命令在横排候选窗里会被共享 runtime 换成移动候选。
    for (NSArray *entry in @[@[@51, @(MSIME_BACKSPACE_SEGMENT)], @[@123, @(MSIME_MOVE_LEFT_SEGMENT)],
                             @[@124, @(MSIME_MOVE_RIGHT_SEGMENT)]]) {
        const unsigned short code = [entry[0] unsignedShortValue];
        session.lastCommand = UINT32_MAX;
        assert([controller handleEvent:ModeKey(code, NSEventModifierFlagControl, NO) client:client]);
        assert(session.lastCommand == [entry[1] unsignedIntValue]);
        [controller setValue:composing forKey:@"view"];
    }
    for (NSArray *entry in @[@[@123, @(MSIME_MOVE_LEFT_SEGMENT)], @[@124, @(MSIME_MOVE_RIGHT_SEGMENT)]]) {
        session.lastCommand = UINT32_MAX;
        assert([controller handleEvent:ModeKey([entry[0] unsignedShortValue], NSEventModifierFlagShift, NO) client:client]);
        assert(session.lastCommand == [entry[1] unsignedIntValue]);
        [controller setValue:composing forKey:@"view"];
    }
    // Shift+Backspace 不是分段编辑，仍是普通退格。
    session.lastCommand = UINT32_MAX;
    [controller handleEvent:ModeKey(51, NSEventModifierFlagShift, NO) client:client];
    assert(session.lastCommand == MSIME_BACKSPACE);
    [controller setValue:composing forKey:@"view"];
    // 默认左右键交给 runtime，关闭该偏好时保留宿主按音节移动的旧路径。
    NSMutableDictionary *shuangpin = [composing mutableCopy];
    shuangpin[@"scheme"] = @1;
    const BOOL savedLeftRight = [appearance navigationEnabled:@"left_right"];
    for (NSNumber *leftRight in @[@YES, @NO]) {
        [appearance applySharedCandidatePreferences:@{@"navigation": @{@"left_right": leftRight}}];
        for (NSArray *entry in @[@[@123, @(leftRight.boolValue ? MSIME_MOVE_LEFT : MSIME_MOVE_LEFT_SEGMENT)],
                                 @[@124, @(leftRight.boolValue ? MSIME_MOVE_RIGHT : MSIME_MOVE_RIGHT_SEGMENT)]]) {
            [controller setValue:shuangpin forKey:@"view"];
            panel.visible = YES;
            session.lastCommand = UINT32_MAX;
            assert([controller handleEvent:ModeKey([entry[0] unsignedShortValue], 0, NO) client:client]);
            assert(session.lastCommand == [entry[1] unsignedIntValue]);
        }
    }
    [appearance applySharedCandidatePreferences:@{@"navigation": @{@"left_right": @(savedLeftRight)}}];
    // 五笔里 Ctrl+左右和 Shift+左右同样按分段移动。
    NSMutableDictionary *wubi = [composing mutableCopy];
    wubi[@"scheme"] = @2;
    for (NSNumber *chord in @[@(NSEventModifierFlagControl), @(NSEventModifierFlagShift)]) {
        for (NSArray *entry in @[@[@123, @(MSIME_MOVE_LEFT_SEGMENT)], @[@124, @(MSIME_MOVE_RIGHT_SEGMENT)]]) {
            [controller setValue:wubi forKey:@"view"];
            session.lastCommand = UINT32_MAX;
            assert([controller handleEvent:ModeKey([entry[0] unsignedShortValue], chord.unsignedIntegerValue, NO) client:client]);
            assert(session.lastCommand == [entry[1] unsignedIntValue]);
        }
    }
    [controller setValue:composing forKey:@"view"];

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
    // Shift+左右也一样：没有组字时是应用里的选择文字，不发分段命令。
    [controller setValue:@{ @"focused": @YES, @"editing_text": @"", @"candidates": @[] } forKey:@"view"];
    session.lastCommand = UINT32_MAX;
    [controller handleEvent:ModeKey(123, NSEventModifierFlagShift, NO) client:client];
    assert(session.lastCommand != MSIME_MOVE_LEFT_SEGMENT);

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

// The scheme behaviour the controller used to infer from "scheme == 4" is read from the view and from msime::mac::scheme traits, so Zhuyin and Vietnamese get the rules of the scheme they behave like rather than quanpin's. The views here are what the runtime would publish; the real Korean session in TestRealSessionComposition covers scheme 4 end to end.
static void TestSchemeTraitsFromView(MSIMEAppearancePreferences *appearance) {
    // View fields the runtime publishes are read as they are, not recomputed from the scheme.
    assert(MSIMEScriptConversionApplies(@{@"scheme": @5, @"local_mode": @"none", @"script_conversion": @YES}));
    assert(!MSIMEScriptConversionApplies(@{@"scheme": @0, @"local_mode": @"none", @"script_conversion": @NO}));
    assert(!MSIMEScriptConversionApplies(@{@"scheme": @0, @"local_mode": @"none"}));
    assert(!MSIMEScriptConversionApplies(nil));
    assert(MSIMECandidateListOpen(@{@"scheme": @6, @"candidate_list_open": @YES, @"candidates": @[@{@"text": @"中"}]}));
    assert(!MSIMECandidateListOpen(@{@"scheme": @4, @"candidates": @[@{@"text": @"韓"}]}));
    assert(!MSIMECandidateListOpen(@{@"scheme": @0, @"candidate_list_open": @NO, @"candidates": @[@{@"text": @"中"}]}));
    // Glosses: the three Chinese schemes that always had them, not temporary Japanese inside them, and none of the schemes after them. A view without a scheme is quanpin.
    for (NSNumber *scheme in @[@0, @1, @2, @4]) assert(MSIMEViewShowsGlosses(@{@"scheme": scheme}));
    for (NSNumber *scheme in @[@3, @5, @6, @7, @8, @9, @99]) assert(!MSIMEViewShowsGlosses(@{@"scheme": scheme}));
    assert(!MSIMEViewShowsGlosses(@{@"scheme": @0, @"local_mode": @"temporary_japanese"}));
    assert(MSIMEViewShowsGlosses(@{}));
    // Scheme traits hold only while the scheme's own rules do: dedicated English and the local modes keep theirs.
    assert(MSIMESchemeTrait(@{@"scheme": @6}, msime::mac::scheme::LocksCaret));
    assert(MSIMESchemeTrait(@{@"scheme": @7, @"local_mode": @"none"}, msime::mac::scheme::LocksCaret));
    assert(!MSIMESchemeTrait(@{@"scheme": @7, @"dedicated_english": @YES}, msime::mac::scheme::LocksCaret));
    assert(MSIMESchemeTrait(@{@"scheme": @8, @"local_mode": @"none"}, msime::mac::scheme::LocksCaret));
    assert(!MSIMESchemeTrait(@{@"scheme": @6, @"local_mode": @"emoji"}, msime::mac::scheme::LocksCaret));
    assert(!MSIMESchemeTrait(nil, msime::mac::scheme::LocksCaret));

    ModeController *controller = [ModeController alloc];
    ShortcutSession *session = [ShortcutSession new];
    ShortcutClient *client = [ShortcutClient new];
    [controller setValue:appearance forKey:@"appearance"];
    [controller setValue:session forKey:@"session"];
    [controller setValue:client forKey:@"activeClient"];
    appearance.englishMode = NO;
    NSDictionary *(^composing)(NSNumber *) = ^NSDictionary *(NSNumber *scheme) {
        return @{@"focused": @YES, @"scheme": scheme, @"local_mode": @"none", @"editing_text": @"abc", @"caret_position": @3,
                 @"candidates": @[]};
    };

    // 只有光标不锁在组字末尾时 Ctrl+Backspace 才编辑分段；注音、越南文和藏文与韩文一样结束组字，把这个组合键留给应用。
    for (NSNumber *scheme in @[@0, @1, @2, @3, @5, @4, @6, @7, @8, @9]) {
        const BOOL locked = scheme.intValue == 4 || scheme.intValue == 6 || scheme.intValue == 7 || scheme.intValue == 8;
        [controller setValue:composing(scheme) forKey:@"view"];
        session.lastCommand = UINT32_MAX;
        assert([controller handleEvent:ModeKey(51, NSEventModifierFlagControl, NO) client:client] == !locked);
        assert(session.lastCommand == (locked ? MSIME_FINISH_COMPOSITION : MSIME_BACKSPACE_SEGMENT));
    }

    // Option+Return opens the candidate list of a scheme that has one to open (Korean, Zhuyin) and is swallowed while composing; elsewhere it finishes the composition and goes to the application as before.
    for (NSNumber *scheme in @[@0, @3, @5, @7, @8, @9, @4, @6]) {
        const BOOL opens = scheme.intValue == 4 || scheme.intValue == 6;
        [controller setValue:composing(scheme) forKey:@"view"];
        session.lastCommand = UINT32_MAX;
        assert([controller handleEvent:ModeKey(36, NSEventModifierFlagOption, NO) client:client] == opens);
        assert(session.lastCommand == (opens ? MSIME_OPEN_CANDIDATE_LIST : MSIME_FINISH_COMPOSITION));
    }
    // Dedicated English keeps its own rules inside Zhuyin: no list to open there.
    NSMutableDictionary *dedicated = [composing(@6) mutableCopy];
    dedicated[@"dedicated_english"] = @YES;
    [controller setValue:dedicated forKey:@"view"];
    session.lastCommand = UINT32_MAX;
    assert(![controller handleEvent:ModeKey(36, NSEventModifierFlagOption, NO) client:client]);
    assert(session.lastCommand == MSIME_FINISH_COMPOSITION);

    // 没有组字时，大写锁定下的大写字母交还给应用，除非方案自己组它：韩文折成不带 Shift 的字母键，越南文和藏文打出大写字母（威利转写里大写是另一个字母）。
    NSEvent *capsA = [NSEvent keyEventWithType:NSEventTypeKeyDown location:NSZeroPoint modifierFlags:NSEventModifierFlagCapsLock timestamp:0
                                  windowNumber:0 context:nil characters:@"A" charactersIgnoringModifiers:@"a" isARepeat:NO keyCode:0];
    session.nextTransition = @{@"handled": @YES, @"view": @{@"editing_text": @"", @"caret_position": @0, @"candidates": @[]}};
    session.capsLock = NO;
    const NSUInteger capsLockReports = session.capsLockCalls;
    for (NSNumber *scheme in @[@0, @5, @6, @9, @4, @7, @8]) {
        const int value = scheme.intValue;
        [controller setValue:@{@"focused": @YES, @"scheme": scheme, @"local_mode": @"none", @"editing_text": @"", @"caret_position": @0,
                               @"candidates": @[]} forKey:@"view"];
        session.asciiCalls = 0;
        session.lastASCII = 0;
        const BOOL composes = value == 4 || value == 7 || value == 8;
        assert([controller handleEvent:capsA client:client] == composes);
        assert(session.asciiCalls == (composes ? 1u : 0u));
        if (value == 4) assert(session.lastASCII == 'a');
        if (value == 7 || value == 8) assert(session.lastASCII == 'A');
    }
    // 大写锁定一变就报告给会话，只报告变化的那一次：「大写锁定时使用英文标点」由共享层按这个状态决定标点去向（#6370）。
    assert(session.capsLock && session.capsLockCalls == capsLockReports + 1);
    [controller setValue:@{@"focused": @YES, @"scheme": @0, @"local_mode": @"none", @"editing_text": @"", @"caret_position": @0,
                           @"candidates": @[]} forKey:@"view"];
    NSEvent *lowerA = [NSEvent keyEventWithType:NSEventTypeKeyDown location:NSZeroPoint modifierFlags:0 timestamp:0
                                   windowNumber:0 context:nil characters:@"a" charactersIgnoringModifiers:@"a" isARepeat:NO keyCode:0];
    [controller handleEvent:lowerA client:client];
    assert(!session.capsLock && session.capsLockCalls == capsLockReports + 2);
    session.nextTransition = nil;
}

// Keys a scheme spells with reach the Engine as characters before the host reads them as a candidate digit, a paging key, Space's selection or a paired mark, and Down opens a closed Zhuyin list. The views are what the runtime publishes for each state (design 3.3: Zhuyin's spelling symbols idle, composing and with the list open).
static void TestSchemeKeyRouting() {
    NSString *suite = [@"app.msime.test.scheme-key-routing." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    MSIMEAppearancePreferences *appearance = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    appearance.englishMode = NO;
    [appearance applySharedCandidatePreferences:@{@"navigation": @{@"minus_equal": @YES, @"arrows": @NO}}];
    assert([appearance navigationEnabled:@"minus_equal"] && ![appearance navigationEnabled:@"arrows"]);
    assert(appearance.pairedPunctuation && appearance.runtimeChinesePunctuation);
    ModeController *controller = [ModeController alloc];
    ShortcutSession *session = [ShortcutSession new];
    ShortcutClient *client = [ShortcutClient new];
    client.document = @"";
    client.insertions = [NSMutableArray array];
    HiddenCandidatePanel *panel = [[HiddenCandidatePanel alloc] init];
    [controller setValue:appearance forKey:@"appearance"];
    [controller setValue:session forKey:@"session"];
    [controller setValue:client forKey:@"activeClient"];
    [controller setValue:panel forKey:@"panel"];

    NSEvent *(^key)(unsigned short, NSString *, NSEventModifierFlags) = ^NSEvent *(unsigned short code, NSString *characters, NSEventModifierFlags flags) {
        return [NSEvent keyEventWithType:NSEventTypeKeyDown location:NSZeroPoint modifierFlags:flags timestamp:0 windowNumber:0 context:nil
                              characters:characters charactersIgnoringModifiers:characters isARepeat:NO keyCode:code];
    };
    NSDictionary *(^view)(NSNumber *, NSString *, NSString *, BOOL) = ^NSDictionary *(NSNumber *scheme, NSString *editing, NSString *symbols, BOOL listOpen) {
        NSArray *candidates = listOpen ? @[@{@"text": @"中", @"highlighted": @YES, @"id": @{@"session": @3, @"generation": @9, @"index": @0}}] : @[];
        return @{@"focused": @YES, @"scheme": scheme, @"local_mode": @"none", @"editing_text": editing, @"caret_position": @(editing.length),
                 @"spelling_symbols": symbols, @"candidate_list_open": @(listOpen), @"session": @3, @"generation": @9, @"candidates": candidates};
    };
    // Sends one key from a view and reports whether the host claimed it.
    __block NSUInteger asciiBefore = 0;
    BOOL (^handle)(NSDictionary *, NSEvent *, BOOL) = ^BOOL(NSDictionary *state, NSEvent *event, BOOL panelVisible) {
        [controller setValue:state forKey:@"view"];
        panel.requestedVisible = panelVisible;
        session.nextTransition = @{@"handled": @YES, @"view": state};
        session.lastCommand = UINT32_MAX;
        asciiBefore = session.asciiCalls;
        return [controller handleEvent:event client:client];
    };
    // Sends one key from a view and reports whether the host handed it to the Engine as a character.
    BOOL (^send)(NSDictionary *, NSEvent *, BOOL) = ^BOOL(NSDictionary *state, NSEvent *event, BOOL panelVisible) {
        return handle(state, event, panelVisible) && session.asciiCalls == asciiBefore + 1;
    };
    NSString *const zhuyinIdle = @"125890,./;-";
    NSString *const zhuyinComposing = @"1234567890,./;- ";
    NSString *const zhuyinListOpen = @"0,./;-";
    const NSUInteger punctuationRoutes = 0;

    // Idle Zhuyin: `,` and `1` are bopomofo, typed into the Engine rather than written as a mark or a digit.
    assert(send(view(@6, @"", zhuyinIdle, NO), key(43, @",", 0), NO) && session.lastASCII == ',');
    assert(send(view(@6, @"", zhuyinIdle, NO), key(18, @"1", 0), NO) && session.lastASCII == '1');
    // With the list open, `-` spells ㄦ instead of turning a page and `0` spells ㄢ instead of picking a row.
    assert(send(view(@6, @"su3", zhuyinListOpen, YES), key(27, @"-", 0), YES) && session.lastASCII == '-');
    assert(session.lastCommand != MSIME_PREVIOUS_PAGE);
    const NSUInteger selectsBefore = session.selectCalls;
    assert(send(view(@6, @"su3", zhuyinListOpen, YES), key(29, @"0", 0), YES) && session.lastASCII == '0');
    assert(session.selectCalls == selectsBefore);
    // Space is the first tone while a syllable is pending, panel or not: a character, not the commit command.
    for (NSNumber *visible in @[@NO, @YES]) {
        assert(send(view(@6, @"su", zhuyinComposing, NO), key(49, @" ", 0), visible.boolValue) && session.lastASCII == ' ');
        assert(session.lastCommand == UINT32_MAX);
    }
    // Shift+`{` is the Engine's 『 in Zhuyin, so the host does not open a `{` pair of its own.
    assert(send(view(@6, @"su3", zhuyinComposing, NO), key(33, @"{", NSEventModifierFlagShift), NO) && session.lastASCII == '{');
    assert(session.punctuationASCIICalls == punctuationRoutes && client.committed == nil && client.insertions.count == 0);
    // Keypad `-` and `.` spell ㄦ and ㄡ like the keypad digits do, rather than taking the keypad punctuation route that would finish the conversion and append the ASCII mark.
    assert(send(view(@6, @"su3", zhuyinComposing, NO), KeypadKey(78, @"-", 0, NO), NO) && session.lastASCII == '-');
    assert(send(view(@6, @"", zhuyinIdle, NO), KeypadKey(65, @".", 0, NO), NO) && session.lastASCII == '.');
    assert(session.punctuationASCIICalls == punctuationRoutes && session.enginePunctuationCalls == 0 && client.insertions.count == 0);
    // Shift+`:` after an ASCII letter is the Engine's overlay mark, not the host's contextual smart punctuation, which Quanpin still takes.
    appearance.smartPunctuation = YES;
    client.document = @"a";
    client.selection = NSMakeRange(1, 0);
    NSEvent *colon = key(41, @":", NSEventModifierFlagShift);
    assert(send(view(@6, @"", zhuyinIdle, NO), colon, NO) && session.lastASCII == ':' && session.lastShift);
    assert(session.contextualPunctuationCalls == 0 && session.enginePunctuationCalls == 0);
    assert(handle(view(@0, @"", @"", NO), colon, NO) && session.contextualPunctuationCalls == 1 && session.lastPrecedingScalar == 'a');
    appearance.smartPunctuation = NO;
    session.contextualPunctuationCalls = 0;
    session.enginePunctuationCalls = 0;
    client.document = @"";
    client.selection = NSMakeRange(0, 0);
    client.insertions = [NSMutableArray array];
    client.committed = nil;

    // Down opens a closed Zhuyin list whatever the arrow binding says (it is off here) and is never handed to the application.
    const NSEventModifierFlags arrowFlags = NSEventModifierFlagFunction | NSEventModifierFlagNumericPad;
    NSString *down = [NSString stringWithFormat:@"%C", (unichar)NSDownArrowFunctionKey];
    assert(handle(view(@6, @"su3", zhuyinComposing, NO), key(125, down, arrowFlags), NO));
    assert(session.lastCommand == MSIME_OPEN_CANDIDATE_LIST && session.asciiCalls == asciiBefore);
    [appearance applySharedCandidatePreferences:@{@"navigation": @{@"arrows": @YES}}];
    assert(handle(view(@6, @"su3", zhuyinComposing, NO), key(125, down, arrowFlags), NO));
    assert(session.lastCommand == MSIME_OPEN_CANDIDATE_LIST && session.asciiCalls == asciiBefore);
    // With the list open Down moves the highlight as in any list, and with Shift held or nothing composing it is not the list key.
    appearance.vertical = YES;
    assert(handle(view(@6, @"su3", zhuyinListOpen, YES), key(125, down, arrowFlags), YES));
    assert(session.lastCommand == MSIME_NEXT_CANDIDATE && session.asciiCalls == asciiBefore);
    appearance.vertical = NO;
    send(view(@6, @"su3", zhuyinComposing, NO), key(125, down, arrowFlags | NSEventModifierFlagShift), NO);
    assert(session.lastCommand != MSIME_OPEN_CANDIDATE_LIST);
    send(view(@6, @"", zhuyinIdle, NO), key(125, down, arrowFlags), NO);
    assert(session.lastCommand != MSIME_OPEN_CANDIDATE_LIST);
    // Down in a scheme without a Down-opened list keeps its arrow meaning.
    for (NSNumber *scheme in @[@0, @4, @5, @7, @9]) {
        send(view(scheme, @"abc", @"", NO), key(125, down, arrowFlags), NO);
        assert(session.lastCommand == MSIME_NEXT_CANDIDATE);
    }
    [appearance applySharedCandidatePreferences:@{@"navigation": @{@"arrows": @NO}}];

    // Cantonese: the syllable apostrophe goes to the Engine as input, not to the quote pairing or punctuation routes.
    assert(send(view(@5, @"ngo", @"'", NO), key(39, @"'", 0), NO) && session.lastASCII == '\'');
    assert(session.punctuationASCIICalls == punctuationRoutes && session.enginePunctuationCalls == 0 && session.contextualPunctuationCalls == 0);
    assert(client.insertions.count == 0 && client.committed == nil);

    // Vietnamese keeps the letter's case while composing, with Shift or with Caps Lock.
    assert(send(view(@7, @"vie", @"", NO), key(9, @"V", NSEventModifierFlagShift), NO) && session.lastASCII == 'V' && session.lastShift);
    assert(send(view(@7, @"vie", @"", NO), key(9, @"V", NSEventModifierFlagCapsLock), NO) && session.lastASCII == 'V' && !session.lastShift);

    // 藏文（威利转写）：空闲时 `'` 开始一个 achung 音节，`/` 由引擎单独写出垂符，都作为字符交给引擎，不走引号配对或标点路由。
    NSString *const tibetanIdle = @"'/";
    NSString *const tibetanComposing = @"'+-./";
    assert(send(view(@8, @"", tibetanIdle, NO), key(39, @"'", 0), NO) && session.lastASCII == '\'');
    assert(send(view(@8, @"", tibetanIdle, NO), key(44, @"/", 0), NO) && session.lastASCII == '/');
    // 组字时叠加的 `+`、消歧的 `.`、`-`、`'` 和写垂符的 `/` 都是拼写，开着减号翻页也不翻页。
    assert(send(view(@8, @"pad", tibetanComposing, NO), key(24, @"+", NSEventModifierFlagShift), NO) && session.lastASCII == '+');
    assert(send(view(@8, @"g", tibetanComposing, NO), key(47, @".", 0), NO) && session.lastASCII == '.');
    assert(send(view(@8, @"k", tibetanComposing, NO), key(27, @"-", 0), NO) && session.lastASCII == '-');
    assert(session.lastCommand != MSIME_PREVIOUS_PAGE);
    assert(send(view(@8, @"bsgra", tibetanComposing, NO), key(39, @"'", 0), NO) && session.lastASCII == '\'');
    assert(send(view(@8, @"bkra shis", tibetanComposing, NO), key(44, @"/", 0), NO) && session.lastASCII == '/');
    assert(session.punctuationASCIICalls == punctuationRoutes && session.enginePunctuationCalls == 0 && session.contextualPunctuationCalls == 0);
    assert(client.insertions.count == 0 && client.committed == nil);
    // 威利转写区分大小写：Shift 和大写锁定打出的大写字母都原样送进引擎。
    assert(send(view(@8, @"", tibetanIdle, NO), key(17, @"T", NSEventModifierFlagShift), NO) && session.lastASCII == 'T' && session.lastShift);
    assert(send(view(@8, @"k", tibetanComposing, NO), key(0, @"A", NSEventModifierFlagCapsLock), NO) && session.lastASCII == 'A' && !session.lastShift);
    // 空格上屏藏文加音节点、回车只上屏藏文、Esc 先退回威利原文，都是引擎命令，不是字符。
    assert(handle(view(@8, @"bkra", tibetanComposing, NO), key(49, @" ", 0), NO) && session.lastCommand == MSIME_COMMIT_CANDIDATE && session.asciiCalls == asciiBefore);
    assert(handle(view(@8, @"bkra", tibetanComposing, NO), key(36, @"\r", 0), NO) && session.lastCommand == MSIME_COMMIT_RAW && session.asciiCalls == asciiBefore);
    assert(handle(view(@8, @"bkra", tibetanComposing, NO), key(53, @"\x1b", 0), NO) && session.lastCommand == MSIME_CANCEL && session.asciiCalls == asciiBefore);

    // Stroke spells with the letters h s p n z and the wildcard x, so it publishes no spelling symbols: every stroke key reaches the Engine as a plain letter, idle or composing, and the Engine decides which of them start a composition.
    const struct { unsigned short code; char letter; } strokeKeys[] = {{4, 'h'}, {1, 's'}, {35, 'p'}, {45, 'n'}, {6, 'z'}, {7, 'x'}};
    for (const auto &stroke : strokeKeys) {
        NSString *characters = [NSString stringWithFormat:@"%c", stroke.letter];
        assert(send(view(@9, @"", @"", NO), key(stroke.code, characters, 0), NO) && session.lastASCII == stroke.letter && !session.lastShift);
        assert(send(view(@9, @"hs", @"", NO), key(stroke.code, characters, 0), NO) && session.lastASCII == stroke.letter);
    }
    assert(session.punctuationASCIICalls == punctuationRoutes && session.enginePunctuationCalls == 0 && client.insertions.count == 0);
    // With the stroke candidates up the physical digits still pick a row and Space still commits the highlighted one; strokes are never digits.
    NSMutableDictionary *stroke = [view(@9, @"hs", @"", YES) mutableCopy];
    stroke[@"candidate_list_open"] = @NO;
    stroke[@"preedit"] = @"一丨";
    stroke[@"reading"] = @"一丨";
    [controller setValue:stroke forKey:@"view"];
    panel.requestedVisible = YES;
    asciiBefore = session.asciiCalls;
    assert([controller handleEvent:key(18, @"1", 0) client:client] && session.asciiCalls == asciiBefore);
    assert(!send(stroke, key(49, @" ", 0), YES) && session.lastCommand == MSIME_COMMIT_CANDIDATE);
    assert(session.asciiCalls == asciiBefore);

    // Quanpin is unchanged: with the panel up a digit picks a row, `-` turns the page and Space commits the candidate.
    // An ordinary conversion's panel: candidates shown with no opened list, which would make the paging marks punctuation.
    NSMutableDictionary *quanpin = [view(@0, @"zhong", @"", YES) mutableCopy];
    quanpin[@"candidate_list_open"] = @NO;
    [controller setValue:quanpin forKey:@"view"];
    panel.requestedVisible = YES;
    asciiBefore = session.asciiCalls;
    assert([controller handleEvent:key(18, @"1", 0) client:client] && session.asciiCalls == asciiBefore);
    assert(!send(quanpin, key(27, @"-", 0), YES) && session.lastCommand == MSIME_PREVIOUS_PAGE);
    assert(!send(quanpin, key(49, @" ", 0), YES) && session.lastCommand == MSIME_COMMIT_CANDIDATE);
    assert(session.asciiCalls == asciiBefore);
    session.nextTransition = nil;
    MSIMERemoveTestPreferenceSuite(defaults, suite);
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
    // 智能标点和重复标点转中文默认开：全新的 macOS 配置不用先找到开关，数字后的句点就是英文句点。
    assert(appearance.smartPunctuation && appearance.smartPunctuationRepeatToChinese);
    assert([[appearance sharedPreferencesByMerging:@{}][@"smart_punctuation"] isEqual:@YES]);
    assert([[appearance sharedPreferencesByMerging:@{}][@"smart_punctuation_repeat"] isEqual:@YES]);
    NSButton *smartButton = (id)PreferenceControl(appearance, @selector(smartPunctuationChanged:));
    NSButton *smartRepeatButton = (id)PreferenceControl(appearance, @selector(smartPunctuationRepeatChanged:));
    assert(smartButton.state == NSControlStateValueOn && smartRepeatButton.state == NSControlStateValueOn);
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
    // 回到什么都没存过的状态（setter 清掉共享值，再删掉本机值）：下面的控制器路径按默认值走，不另外打开。
    appearance.smartPunctuation = NO;
    appearance.smartPunctuationRepeatToChinese = NO;
    [defaults removeObjectForKey:@"MSIMEClientSmartPunctuation"];
    [defaults removeObjectForKey:@"MSIMEClientSmartPunctuationRepeatToChinese"];
    assert(appearance.smartPunctuation && appearance.smartPunctuationRepeatToChinese);

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

    // 数字后同样按默认值走：没有组字时前文取自客户端，`0.10` 后的句点交给带前文的标点入口（由 client-core 的 route 判成 ASCII），不会变成「0.10。」。
    ModeController *digitController = [ModeController alloc];
    ShortcutSession *digitSession = [ShortcutSession new];
    ShortcutClient *digitClient = [ShortcutClient new];
    digitClient.document = @"0.10";
    digitClient.selection = NSMakeRange(4, 0);
    [digitController setValue:appearance forKey:@"appearance"];
    [digitController setValue:digitSession forKey:@"session"];
    [digitController setValue:digitClient forKey:@"activeClient"];
    [digitController setValue:@{@"focused": @YES, @"editing_text": @"", @"candidates": @[]} forKey:@"view"];
    digitSession.contextualPunctuationTransition = @{@"handled": @YES, @"commit": @".",
        @"view": @{@"focused": @YES, @"editing_text": @"", @"candidates": @[]}};
    assert([digitController handleEvent:period client:digitClient]);
    assert(digitSession.contextualPunctuationCalls == 1 && digitSession.lastContextualPunctuation == '.' && digitSession.lastPrecedingScalar == '0');
    assert([digitClient.document isEqual:@"0.10."]);

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
    // Dvorak：K 在美式布局的 V 位置（keyCode 9），物理 K 键打出 t。
    assert([controller handleEvent:LayoutKey(9, @"K", chord) client:client]);
    assert(controller.screenKeyboardCalls == 4);
    assert(![controller handleEvent:LayoutKey(40, @"T", chord) client:client]);
    assert(controller.screenKeyboardCalls == 4);
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
    // Dvorak：C、R、T 分别在美式布局的 I、O、K 位置，物理 C 键打出 j。
    assert([controller handleEvent:LayoutKey(34, @"C", chord) client:client]);
    assert([controller handleEvent:LayoutKey(31, @"R", chord) client:client]);
    assert([controller handleEvent:LayoutKey(40, @"T", chord) client:client]);
    assert(![controller handleEvent:LayoutKey(8, @"J", chord) client:client]);
    assert(session.resetCacheCalls == 3 && controller.restartCalls == 3 && controller.terminationCalls == 2);
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
                        scheme:(NSString *)scheme
                   schemeTitle:(NSString *)schemeTitle
                      capsLock:(BOOL)capsLock
          chinesePunctuationEnabled:(BOOL)chinesePunctuationEnabled
                   fullWidthEnabled:(BOOL)fullWidthEnabled
    traditionalChineseOutputEnabled:(BOOL)traditionalChineseOutputEnabled {
    (void)englishInputMode;
    (void)englishCandidateMode;
    (void)scheme;
    (void)schemeTitle;
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

// IMK 不保证上一个客户端的 deactivateServer: 先于下一个客户端的 activateServer: 到达。activateServer: 已经丢掉了上一个客户端欠着的闭合符，所以迟到的那次回调到来时，控制器里待补的闭合符和跳过记录都属于当前客户端：它们既不能被提前写进当前客户端（成对标点被提前合上，组字中的 marked text 也会被整段替换掉），也不能被清掉。正常的 deactivate 仍在自己的客户端里补上闭合符。
static void TestStaleDeactivationLeavesTheCurrentPairOpen() {
    NSString *suite = [@"msime.stale-deactivation-pair." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    MSIMEAppearancePreferences *appearance = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    assert(appearance.pairedPunctuation && appearance.runtimeChinesePunctuation && !appearance.runtimeFullWidthInput);
    ModeController *controller = [ModeController alloc];
    ShortcutSession *session = [ShortcutSession new];
    ShortcutClient *previous = [ShortcutClient new], *current = [ShortcutClient new];
    previous.document = current.document = @"";
    [controller setValue:appearance forKey:@"appearance"];
    [controller setValue:session forKey:@"session"];
    [controller setValue:current forKey:@"activeClient"];
    NSDictionary *idle = @{ @"focused": @YES, @"editing_text": @"", @"candidates": @[] };
    [controller setValue:idle forKey:@"view"];
    NSEvent *brace = [NSEvent keyEventWithType:NSEventTypeKeyDown location:NSZeroPoint modifierFlags:NSEventModifierFlagShift
                                      timestamp:0 windowNumber:0 context:nil characters:@"{" charactersIgnoringModifiers:@"["
                                      isARepeat:NO keyCode:33];
    NSEvent *closeBrace = [NSEvent keyEventWithType:NSEventTypeKeyDown location:NSZeroPoint modifierFlags:NSEventModifierFlagShift
                                           timestamp:0 windowNumber:0 context:nil characters:@"}" charactersIgnoringModifiers:@"]"
                                           isARepeat:NO keyCode:30];
    Method base = class_getInstanceMethod(IMKInputController.class, @selector(deactivateServer:));
    assert(base);
    baseDeactivationCalls = 0;
    IMP original = method_setImplementation(base, (IMP)RecordBaseDeactivation);

    // 当前客户端打开了一对 `「」`，`」` 作为 marked text 的尾巴等着补上。
    session.punctuationASCIITransition = @{ @"handled": @NO, @"commit": NSNull.null, @"view": idle };
    assert([controller handleEvent:brace client:current]);
    assert([current.committed isEqual:@"「"] && [current.marked isEqual:@"」"]);
    for (id stale in @[previous, NSNull.null]) {
        [controller deactivateServer:stale == NSNull.null ? nil : stale];
        assert([current.committed isEqual:@"「"] && [current.marked isEqual:@"」"] && current.insertions.count == 1);
        assert(previous.insertions.count == 0 && previous.marked == nil);
        assert([[controller valueForKey:@"pendingPairedClosing"] isEqual:@"」"]);
        assert([controller valueForKey:@"activeClient"] == current && baseDeactivationCalls == 0);
    }

    // 这一对仍归当前客户端：下一次上屏把 `」` 一起带走，之后在它前面敲的 `」` 被跨过去。
    [controller apply:@{ @"commit": @"a", @"view": idle }];
    assert([current.committed isEqual:@"a」"] && [current.document isEqual:@"「a」"]);
    for (id stale in @[previous, NSNull.null]) [controller deactivateServer:stale == NSNull.null ? nil : stale];
    current.selection = NSMakeRange(2, 0);
    const NSUInteger asciiBeforeStep = session.asciiCalls;
    assert([controller handleEvent:closeBrace client:current]);
    assert(session.asciiCalls == asciiBeforeStep && [current.document isEqual:@"「a」"] && current.selection.location == 3);
    assert(previous.insertions.count == 0 && previous.marked == nil);

    // 正常的 deactivate 照旧在自己的客户端里把这一对合上。
    current.selection = NSMakeRange(current.document.length, 0);
    assert([controller handleEvent:brace client:current]);
    assert([current.committed isEqual:@"「"] && [current.marked isEqual:@"」"]);
    [controller deactivateServer:current];
    assert([current.committed isEqual:@"」"] && [current.document isEqual:@"「a」「」"]);
    assert([controller valueForKey:@"pendingPairedClosing"] == nil && [controller valueForKey:@"activeClient"] == nil);
    assert(baseDeactivationCalls == 1 && previous.insertions.count == 0);
    method_setImplementation(base, original);
    MSIMERemoveTestPreferenceSuite(defaults, suite);
}

// 记下控制器要画的每个打字特效答复，不碰共享的特效面板。
@interface EffectRecordingController : ModeController
@property(nonatomic, strong) NSMutableArray<NSArray *> *effects;
@end
@implementation EffectRecordingController
- (void)drawTypingEffect:(uint32_t)packed commit:(BOOL)commit {
    if (!self.effects) self.effects = [NSMutableArray array];
    [self.effects addObject:@[@(packed), @(commit)]];
}
@end

// 按键音、上屏音、打字特效和背景音乐都由生命周期核心驱动，控制器只按 msime_client.h 的契约报告。这里钉住的是：每个键先比较字段属性、变了才报告，再报告这次按下，然后才派发；每个键报告的按键类别；自动重复、松开、英文模式不报告按下；安全输入只改字段属性，按下照样报告但答复不画；上屏特效取自派发回复的 feedback，没有它的上屏（语音转写、失焦）不画；Engine 算出来的结果不算打字；音乐跟随真正处于激活的控制器；旧的 keySound:、commitSound、setMusicActive: 一次都不调，否则同一个键会响两遍。
static void TestSoundsFollowKeysCommitsAndActivation() {
    NSString *root = [NSTemporaryDirectory() stringByAppendingPathComponent:NSUUID.UUID.UUIDString];
    assert([NSFileManager.defaultManager createDirectoryAtPath:root withIntermediateDirectories:YES attributes:nil error:nil]);
    NSString *suite = [@"msime.sounds." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    MSIMEAppearancePreferences *appearance = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    appearance.fullWidthInput = NO;
    EffectRecordingController *controller = [EffectRecordingController alloc];
    ShortcutSession *session = [ShortcutSession new];
    ShortcutClient *client = [ShortcutClient new];
    client.insertions = [NSMutableArray array];
    [controller setValue:appearance forKey:@"appearance"];
    [controller setValue:session forKey:@"session"];
    [controller setValue:client forKey:@"activeClient"];
    [controller setValue:root forKey:@"preferencesDirectory"];
    NSDictionary *idle = @{ @"focused": @YES, @"editing_text": @"", @"candidates": @[], @"scheme": @0 };
    [controller setValue:idle forKey:@"view"];
    session.nextTransition = @{ @"handled": @YES, @"commit": NSNull.null, @"view": idle };
    NSEvent *(^key)(NSEventType, unsigned short, NSString *, BOOL) = ^NSEvent *(NSEventType type, unsigned short code, NSString *characters, BOOL repeat) {
        return [NSEvent keyEventWithType:type location:NSZeroPoint modifierFlags:0 timestamp:0 windowNumber:0
                                 context:nil characters:characters charactersIgnoringModifiers:characters isARepeat:repeat keyCode:code];
    };
    NSString *knownTraits = [NSString stringWithFormat:@"traits:%x", MSIME_TRAIT_KNOWN];

    // 第一个键之前字段属性还没报告过：先报告属性，再报告这次按下，最后才派发。
    [controller handleEvent:key(NSEventTypeKeyDown, 0, @"a", NO) client:client];
    assert(([session.calls isEqual:@[knownTraits, @"key:0", @"ascii"]]));
    // 空格、两个回车键和退格各有自己的类别，字母和数字是默认类别。属性没变就不再报告。
    for (NSArray *press in @[@[@49, @" "], @[@36, @"\r"], @[@76, @"\x03"], @[@51, @"\x7f"], @[@18, @"1"]])
        [controller handleEvent:key(NSEventTypeKeyDown, [press[0] unsignedShortValue], press[1], NO) client:client];
    assert(([session.keys isEqual:@[@0, @1, @2, @2, @3, @0]]));
    assert(([session.reportedTraits isEqual:@[@(MSIME_TRAIT_KNOWN)]]));
    // 按住一个键算一次按下，松开也不算。
    [controller handleEvent:key(NSEventTypeKeyDown, 0, @"a", YES) client:client];
    [controller handleEvent:key(NSEventTypeKeyUp, 0, @"a", NO) client:client];
    assert(session.keys.count == 6);
    // 英文模式不出声，与 Windows、Linux 一致：不报告按下，但字段属性先报告了 HOST_ENGLISH。
    appearance.englishMode = YES;
    [controller handleEvent:key(NSEventTypeKeyDown, 0, @"a", NO) client:client];
    assert(session.keys.count == 6);
    assert([session.reportedTraits.lastObject isEqual:@(MSIME_TRAIT_KNOWN | MSIME_TRAIT_HOST_ENGLISH)]);
    appearance.englishMode = NO;
    // 没有会话（会话不可用，或词库维护期间）时没有人可报告：不报告按下，也不去画特效，否则一个 0 答复会把还在屏幕上的连击徽标撤掉。
    [controller setValue:nil forKey:@"session"];
    const NSUInteger effectsWithoutSession = controller.effects.count;
    [controller handleEvent:key(NSEventTypeKeyDown, 0, @"a", NO) client:client];
    assert(controller.effects.count == effectsWithoutSession && session.keys.count == 6);
    [controller setValue:session forKey:@"session"];

    // 激活把音乐交给核心仲裁，激活前先报告字段属性。
    [controller claimBackgroundMusic];
    assert(([session.activeStates isEqual:@[@YES]]));
    assert(([[session.calls subarrayWithRange:NSMakeRange(session.calls.count - 2, 2)] isEqual:@[knownTraits, @"active:1"]]));
    // 安全输入只是一项字段属性：核心看到 SECURE 就不出声、不出特效、停掉音乐，控制器不再自己开关音乐。按下照样报告，答复不画。
    [controller.effects removeAllObjects];
    session.keyEffect = (1u << 17) | 3;
    controller.secureInput = YES;
    session.nextTransition = @{ @"handled": @YES, @"commit": @"密码", @"view": idle, @"feedback": @{ @"typing_effect": @0 } };
    [controller handleEvent:key(NSEventTypeKeyDown, 0, @"a", NO) client:client];
    [controller handleEvent:key(NSEventTypeKeyDown, 11, @"b", NO) client:client];
    assert(session.keys.count == 8);
    assert(session.reportedTraits.count == 4 && [session.reportedTraits.lastObject isEqual:@(MSIME_TRAIT_KNOWN | MSIME_TRAIT_SECURE)]);
    assert(([session.activeStates isEqual:@[@YES]]));
    for (NSArray *effect in controller.effects) assert([effect[0] isEqual:@0]);
    controller.secureInput = NO;
    [controller.effects removeAllObjects];
    session.nextTransition = @{ @"handled": @YES, @"commit": NSNull.null, @"view": idle };
    [controller handleEvent:key(NSEventTypeKeyDown, 8, @"c", NO) client:client];
    assert(session.keys.count == 9 && [session.reportedTraits.lastObject isEqual:@(MSIME_TRAIT_KNOWN)]);
    // 按下的特效答复来自 lifecycleKey:，原样交去画。
    assert(([controller.effects isEqual:@[@[@((1u << 17) | 3), @NO]]]));
    session.keyEffect = 0;

    // 上屏特效取自派发回复的 feedback；Engine 算出来的结果（表达式、命令、提及）照样有上屏特效，但不算打字，普通文字两者都算。
    PassthroughStatisticsCall(root, @{@"operation": @"set_enabled", @"enabled": @YES});
    MSIMEReloadTypingStatisticsEnabled(root);
    assert(MSIMETypingStatisticsEnabled.load(std::memory_order_relaxed));
    [controller.effects removeAllObjects];
    [controller apply:@{ @"handled": @YES, @"commit": @"一百二十三", @"view": idle, @"feedback": @{ @"typing_effect": @((1u << 17) | 4) },
                         @"commit_context": @{ @"scheme": @0, @"local_mode": @"expression", @"typing_statistics": @NO } }];
    assert(([controller.effects isEqual:@[@[@((1u << 17) | 4), @YES]]]));
    assert([PassthroughStatisticsDetail(root)[@"characters"][@"han"] integerValue] == 0);
    [controller apply:@{ @"handled": @YES, @"commit": @"你好", @"view": idle, @"feedback": @{ @"typing_effect": @((1u << 17) | 5) },
                         @"commit_context": @{ @"scheme": @0, @"local_mode": @"none", @"typing_statistics": @YES } }];
    assert(controller.effects.count == 2 && [controller.effects[1] isEqual:(@[@((1u << 17) | 5), @YES])]);
    assert([PassthroughStatisticsDetail(root)[@"characters"][@"han"] integerValue] == 2);
    MSIMETypingStatisticsEnabled.store(false, std::memory_order_relaxed);
    // 没有 feedback 的上屏不画：语音转写、失焦上屏都不是派发的回复。不是非负整数或超出 32 位的 feedback 也不画，没有上屏的派发带的 feedback 同样不画。
    [controller setValue:@(static_cast<NSInteger>(msime::mac::TypingSource::Voice)) forKey:@"typingSourceOverride"];
    [controller apply:@{ @"handled": @YES, @"commit": @"语音", @"view": idle }];
    [controller apply:@{ @"handled": @NO, @"commit": @"失焦", @"view": idle }];
    for (id invalid in @[@"1", @(-1), @1.5, @YES, @(1ull << 32), NSNull.null])
        [controller apply:@{ @"handled": @YES, @"commit": @"坏", @"view": idle, @"feedback": @{ @"typing_effect": invalid } }];
    [controller apply:@{ @"handled": @YES, @"commit": @"坏", @"view": idle, @"feedback": @7 }];
    [controller apply:@{ @"handled": @YES, @"commit": NSNull.null, @"view": idle, @"feedback": @{ @"typing_effect": @0 } }];
    // 有上屏但答 0（失焦上屏、安全输入、特效关闭）也不画，否则会撤掉还在屏幕上的连击徽标。
    [controller apply:@{ @"handled": @YES, @"commit": @"零", @"view": idle, @"feedback": @{ @"typing_effect": @0 } }];
    assert(controller.effects.count == 2);

    // 只有音乐最后跟随的控制器能停它：下一个客户端的激活可能先于上一个的停用到达。核心另外按会话仲裁所有者。
    ModeController *next = [ModeController alloc];
    ShortcutSession *nextSession = [ShortcutSession new];
    [next setValue:appearance forKey:@"appearance"];
    [next setValue:nextSession forKey:@"session"];
    [next claimBackgroundMusic];
    assert(([nextSession.calls isEqual:@[knownTraits, @"active:1"]]));
    Method base = class_getInstanceMethod(IMKInputController.class, @selector(deactivateServer:));
    IMP original = method_setImplementation(base, (IMP)RecordBaseDeactivation);
    [controller deactivateServer:client];
    assert(session.activeStates.count == 1 && nextSession.activeStates.count == 1);
    // 不再属于这个控制器的客户端迟到的停用也碰不到音乐。
    ShortcutClient *stale = [ShortcutClient new];
    [next setValue:client forKey:@"activeClient"];
    [next deactivateServer:stale];
    assert(nextSession.activeStates.count == 1);
    [next deactivateServer:client];
    assert(([nextSession.activeStates isEqual:@[@YES, @NO]]));
    // 没有会话的控制器（英文模式下激活）认领音乐：核心里的所有者仍是上一个控制器的会话，宿主替它报告停用；它迟到的停用因所有权已转走不再报告，音乐也不会一直放着。
    [next setValue:client forKey:@"activeClient"];
    [next claimBackgroundMusic];
    ModeController *sessionless = [ModeController alloc];
    [sessionless setValue:appearance forKey:@"appearance"];
    [sessionless claimBackgroundMusic];
    assert(([nextSession.activeStates isEqual:@[@YES, @NO, @YES, @NO]]));
    [next deactivateServer:client];
    assert(nextSession.activeStates.count == 4);
    // 再认领一次（偏好更新）不会去碰别的会话。
    [sessionless claimBackgroundMusic];
    assert(nextSession.activeStates.count == 4);
    [sessionless releaseBackgroundMusic];
    method_setImplementation(base, original);

    // 旧的声音调用一次都没有。
    assert(!session.keySounds.count && session.commitSounds == 0 && !session.musicStates.count);
    assert(!nextSession.keySounds.count && nextSession.commitSounds == 0 && !nextSession.musicStates.count);

    MSIMERemoveTestPreferenceSuite(defaults, suite);
    [NSFileManager.defaultManager removeItemAtPath:root error:nil];
}

// 脚本运行时在每次 msime_client_focus(true) 之后要先确认字段属性才投递事件，同值的报告也算确认。控制器平时只在属性变了时报告，所以每个 setFocused:YES 之前都要不论变没变再报告一次：复用会话的聚焦（快照替换或英文模式之后 _focusPending 的 prepareSession）和按键换了客户端时的聚焦都是这样，报告要紧挨在 focus(true) 之前。
static void TestFieldTraitsAreConfirmedBeforeEveryFocusIn() {
    NSString *suite = [@"msime.focus-traits." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    MSIMEAppearancePreferences *appearance = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    appearance.fullWidthInput = NO;
    ModeController *controller = [ModeController alloc];
    ShortcutSession *session = [ShortcutSession new];
    ShortcutClient *client = [ShortcutClient new];
    [controller setValue:appearance forKey:@"appearance"];
    [controller setValue:session forKey:@"session"];
    [controller setValue:client forKey:@"activeClient"];
    NSDictionary *idle = @{ @"focused": @YES, @"editing_text": @"", @"candidates": @[], @"scheme": @0 };
    [controller setValue:idle forKey:@"view"];
    session.nextTransition = @{ @"handled": @YES, @"commit": NSNull.null, @"view": idle };
    NSEvent *(^letter)(unsigned short, NSString *) = ^NSEvent *(unsigned short code, NSString *characters) {
        return [NSEvent keyEventWithType:NSEventTypeKeyDown location:NSZeroPoint modifierFlags:0 timestamp:0 windowNumber:0
                                 context:nil characters:characters charactersIgnoringModifiers:characters isARepeat:NO keyCode:code];
    };
    NSString *knownTraits = [NSString stringWithFormat:@"traits:%x", MSIME_TRAIT_KNOWN];
    // 第一个键报告了属性，属性不变的下一个键不再报告。
    [controller handleEvent:letter(0, @"a") client:client];
    [controller handleEvent:letter(11, @"b") client:client];
    assert(([session.calls isEqual:@[knownTraits, @"key:0", @"ascii", @"key:0", @"ascii"]]));
    // 复用的会话重新聚焦：属性没变，也紧挨着 focus(true) 之前再报告一次。
    [session.calls removeAllObjects];
    [controller setValue:@YES forKey:@"focusPending"];
    [controller prepareSession];
    const NSUInteger focusIn = [session.calls indexOfObject:@"focus:1"];
    assert(focusIn != NSNotFound && focusIn > 0 && [session.calls[focusIn - 1] isEqual:knownTraits]);
    assert([session.reportedTraits.lastObject isEqual:@(MSIME_TRAIT_KNOWN)]);
    // 按键换了客户端：这个键先报告，旧客户端失焦，新客户端聚焦之前再报告一次属性，最后才为这个键派发。
    [session.calls removeAllObjects];
    ShortcutClient *other = [ShortcutClient new];
    [controller handleEvent:letter(8, @"c") client:other];
    assert(([session.calls isEqual:@[@"key:0", @"focus:0", knownTraits, @"focus:1", @"ascii"]]));
    // 没有聚焦的键照旧不重复报告。
    [session.calls removeAllObjects];
    [controller handleEvent:letter(2, @"d") client:other];
    assert(([session.calls isEqual:@[@"key:0", @"ascii"]]));
    MSIMERemoveTestPreferenceSuite(defaults, suite);
}

// IMK 在焦点移走或用户点进文档时调 commitComposition:，这是 macOS 失焦上屏最常走的一条路：用 MSIME_FINISH_ON_FOCUS_LOSS 结束组字，核心按失焦上屏处理，不出提交音也不画；回复里 feedback 为 0，控制器也就不画上屏特效。不是当前客户端的调用什么都不做。
static void TestCommitCompositionFinishesAsAFocusLoss() {
    NSString *suite = [@"msime.commit-composition." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    MSIMEAppearancePreferences *appearance = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    appearance.fullWidthInput = NO;
    EffectRecordingController *controller = [EffectRecordingController alloc];
    ShortcutSession *session = [ShortcutSession new];
    ShortcutClient *client = [ShortcutClient new];
    client.insertions = [NSMutableArray array];
    [controller setValue:appearance forKey:@"appearance"];
    [controller setValue:session forKey:@"session"];
    [controller setValue:client forKey:@"activeClient"];
    NSDictionary *idle = @{ @"focused": @YES, @"editing_text": @"", @"candidates": @[], @"scheme": @0 };
    [controller setValue:@{ @"focused": @YES, @"editing_text": @"nihao", @"candidates": @[], @"scheme": @0 } forKey:@"view"];
    session.finishTransition = @{ @"handled": @YES, @"commit": @"你好", @"view": idle, @"feedback": @{ @"typing_effect": @0 } };
    [controller commitComposition:[ShortcutClient new]];
    assert(session.commandCalls == 0);
    [controller commitComposition:client];
    assert(session.commandCalls == 1 && session.lastCommand == MSIME_FINISH_ON_FOCUS_LOSS);
    assert([client.committed isEqual:@"你好"]);
    // 核心对失焦上屏答 0，控制器什么都不画，屏幕上的连击徽标不会被撤掉。
    assert(!controller.effects.count);
    // 它不是一次按下，不开新的一轮。
    assert(!session.keys.count);
    MSIMERemoveTestPreferenceSuite(defaults, suite);
}

// 会话在 handleKeyEvent: 里才建好的控制器：词库维护结束后的第一个键就是这样，按下经过 handleEvent: 时还没有会话。这里用假会话代替真的新建，并照 prepareSession 在聚焦前强制报告的样子把上次报告的字段属性清零。
@interface SessionOpeningController : EffectRecordingController
@property(nonatomic, strong) ShortcutSession *opening;
@end
@implementation SessionOpeningController
- (void)prepareSession {
    if (![self valueForKey:@"session"] && self.opening) {
        [self setValue:self.opening forKey:@"session"];
        [self setValue:@0 forKey:@"fieldTraits"];
        self.opening = nil;
        return;
    }
    [super prepareSession];
}
@end

// 打开会话的那个键也要先于为它做的派发报告给生命周期核心：handleEvent: 那时没有会话可报告，会话建好之后补报一次。之后的键照常只报告一次。
static void TestTheKeyThatOpensTheSessionIsReportedBeforeItsDispatch() {
    NSString *suite = [@"msime.session-opening-key." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    MSIMEAppearancePreferences *appearance = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    appearance.fullWidthInput = NO;
    SessionOpeningController *controller = [SessionOpeningController alloc];
    ShortcutSession *session = [ShortcutSession new];
    ShortcutClient *client = [ShortcutClient new];
    [controller setValue:appearance forKey:@"appearance"];
    [controller setValue:client forKey:@"activeClient"];
    NSDictionary *idle = @{ @"focused": @YES, @"editing_text": @"", @"candidates": @[], @"scheme": @0 };
    [controller setValue:idle forKey:@"view"];
    session.nextTransition = @{ @"handled": @YES, @"commit": NSNull.null, @"view": idle };
    controller.opening = session;
    NSEvent *(^letter)(unsigned short, NSString *) = ^NSEvent *(unsigned short code, NSString *characters) {
        return [NSEvent keyEventWithType:NSEventTypeKeyDown location:NSZeroPoint modifierFlags:0 timestamp:0 windowNumber:0
                                 context:nil characters:characters charactersIgnoringModifiers:characters isARepeat:NO keyCode:code];
    };
    [controller handleEvent:letter(0, @"a") client:client];
    assert([controller valueForKey:@"session"] == session);
    assert(([session.calls isEqual:@[[NSString stringWithFormat:@"traits:%x", MSIME_TRAIT_KNOWN], @"key:0", @"ascii"]]));
    [controller handleEvent:letter(11, @"b") client:client];
    assert(([session.keys isEqual:@[@0, @0]]) && session.asciiCalls == 2);
    MSIMERemoveTestPreferenceSuite(defaults, suite);
}

// Opens a real session from its own options and records, for every music claim, whether a session was there to hear it.
@interface MusicClaimController : MSIMEInputController
@property(nonatomic, copy) NSDictionary *options;
@property(nonatomic, strong) NSMutableArray<NSNumber *> *claims;
@end
@implementation MusicClaimController
- (NSDictionary *)runtimeOptions { return self.options; }
- (BOOL)secureEventInputActive { return NO; }
- (void)claimBackgroundMusic {
    if (!self.claims) self.claims = [NSMutableArray array];
    [self.claims addObject:@([self valueForKey:@"session"] != nil)];
    [super claimBackgroundMusic];
}
@end

// 在英文模式下激活的控制器还没有会话，激活时那次 claimBackgroundMusic 发给的是 nil，播放器从没听到「输入法处于活动状态」。之后切回中文、按键或重新打开会话时建好会话，音乐的归属者要在那时补报一次；不是归属者的控制器、以及已经有会话的再次准备都不报。
static void TestMusicIsClaimedOnceTheSessionOpens() {
    NSString *root = [NSTemporaryDirectory() stringByAppendingPathComponent:NSUUID.UUID.UUIDString];
    NSMutableDictionary *options = [@{@"api_version": @1,
        @"preferences": @{@"scheme": @"quanpin", @"default_ime_mode": @"chinese", @"candidate_page_size": @5,
                          @"learning": @NO, @"chinese_punctuation": @YES}} mutableCopy];
    for (NSString *name in @[@"resources", @"user_data", @"cache", @"dictionaries"]) {
        NSString *path = [root stringByAppendingPathComponent:name];
        assert([NSFileManager.defaultManager createDirectoryAtPath:path withIntermediateDirectories:YES attributes:nil error:nil]);
        options[name] = path;
    }
    NSString *suite = [@"msime.music-claim." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    MSIMEAppearancePreferences *appearance = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    appearance.englishMode = YES;
    auto controllerFor = ^MusicClaimController *(ShortcutClient *client) {
        MusicClaimController *controller = [MusicClaimController alloc];
        controller.options = options;
        [controller setValue:appearance forKey:@"appearance"];
        [controller setValue:client forKey:@"activeClient"];
        [controller setValue:[[HiddenCandidatePanel alloc] init] forKey:@"panel"];
        return controller;
    };
    MusicClaimController *owner = controllerFor([ShortcutClient new]);
    MusicClaimController *other = controllerFor([ShortcutClient new]);

    // activateServer: in English mode: the claim finds no session.
    [owner claimBackgroundMusic];
    assert(([owner.claims isEqual:@[@NO]]));
    // The session opens later (back to Chinese, a key, a menu action): the owner claims again, now with a session to tell.
    [owner prepareSession];
    assert([owner valueForKey:@"session"]);
    assert(([owner.claims isEqual:@[@NO, @YES]]));
    // 新会话一建好就选择加入生命周期核心，由它出声、出特效、仲裁音乐；字段属性也已报告（这个控制器处于英文模式）。选择加入后旧的声音调用变成空操作，由 session-smoke 在声音开着的会话上确认，这里的偏好全关，那样的断言分不出来。
    MSIMEClientSession *opened = [owner valueForKey:@"session"];
    assert(opened.lifecycleFlags == MSIME_LIFECYCLE_FEEDBACK);
    assert([[owner valueForKey:@"fieldTraits"] unsignedIntValue] == (MSIME_TRAIT_KNOWN | MSIME_TRAIT_HOST_ENGLISH));
    // Preparing a session that is already open claims nothing more.
    [owner prepareSession];
    assert(owner.claims.count == 2);
    // A controller music does not follow opens its session without taking music over.
    [other prepareSession];
    assert([other valueForKey:@"session"] && other.claims.count == 0);
    assert(((MSIMEClientSession *)[other valueForKey:@"session"]).lifecycleFlags == MSIME_LIFECYCLE_FEEDBACK);

    [owner releaseBackgroundMusic];
    MSIMERemoveTestPreferenceSuite(defaults, suite);
    [NSFileManager.defaultManager removeItemAtPath:root error:nil];
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
@property(nonatomic) NSUInteger failedUpdatesRemaining;
@property(nonatomic, copy) NSDictionary *lastPreferences;
@end
@implementation AsyncPreferenceSession
- (NSDictionary *)updatePreferencesSnapshot:(NSDictionary *)snapshot error:(NSError **)error {
    self.lastPreferences = snapshot; ++self.updates;
    if (self.failedUpdatesRemaining) {
        --self.failedUpdatesRemaining;
        if (error) *error = [NSError errorWithDomain:@"synthetic-preferences" code:1 userInfo:nil];
        return nil;
    }
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
// 这些读取代次测试手动放行每次后台读取，监视生命周期由独立的按键恢复回归覆盖。
- (void)startPreferencesMonitoring {}
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

// 记录控制器释放时在哪个线程收起浮动工具栏。真实的工具栏是 NSPanel，在后台线程 orderOut: 会让 AppKit 直接终止进程。
@interface ThreadRecordingToolbar : NSObject
@property(atomic) NSUInteger deactivations;
@property(atomic) BOOL deactivatedOffMain;
@property(atomic) NSUInteger completions;
@end
@implementation ThreadRecordingToolbar
- (void)deactivateForDelegate:(id)delegate {
    (void)delegate;
    if (!NSThread.isMainThread) self.deactivatedOffMain = YES;
    self.deactivations = self.deactivations + 1;
}
@end

// 偏好读取完成时把次数记到工具栏桩上：控制器释放后它自己的计数读不到。
@interface DroppedPreferencesController : AsyncPreferencesController
@end
@implementation DroppedPreferencesController
- (void)completePreferenceLoad:(NSDictionary *)snapshot error:(NSError *)error generation:(uint64_t)generation
                       session:(MSIMEClientSession *)session client:(id)client {
    (void)snapshot; (void)error; (void)generation; (void)session; (void)client;
    ThreadRecordingToolbar *toolbar = [self valueForKey:@"toolbar"];
    toolbar.completions = toolbar.completions + 1;
}
@end

// IMK 放掉控制器时，后台的偏好读取可能还握着它。最后一个强引用若在读取线程上释放，dealloc 就在那条线程上收起浮动工具栏，AppKit 以「Must only be used from the main thread」终止输入法进程（0.52.0 的崩溃报告，#6667）。
static void DrainMainQueue();
static void TestPreferenceReadDoesNotDeallocControllerOffMain() {
    ThreadRecordingToolbar *toolbar = [ThreadRecordingToolbar new];
    ControlledPreferenceRead *read = [ControlledPreferenceRead new];
    read.snapshot = @{@"preferences":@{}};
    @autoreleasepool {
        DroppedPreferencesController *controller = [DroppedPreferencesController alloc];
        controller.reads = @[read];
        controller.appliedPreferences = [NSMutableArray array];
        [controller setValue:[ShortcutClient new] forKey:@"activeClient"];
        [controller setValue:@"/synthetic-preferences" forKey:@"preferencesDirectory"];
        [controller setValue:toolbar forKey:@"toolbar"];
        [controller reloadPreferences];
        assert(dispatch_semaphore_wait(read.started, dispatch_time(DISPATCH_TIME_NOW, NSEC_PER_SEC)) == 0);
    }
    // 主线程这一侧的引用已全部放掉，读取还在进行：此后控制器只由后台读取持有。这里不再碰控制器本身，免得主线程上多出一个自动释放的引用把它留到读取之后。
    assert(toolbar.deactivations == 0);
    dispatch_semaphore_signal(read.released);
    SettleWindowLayout();
    NSDate *deadline = [NSDate dateWithTimeIntervalSinceNow:2];
    while (toolbar.deactivations == 0 && deadline.timeIntervalSinceNow > 0)
        [NSRunLoop.currentRunLoop runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.005]];
    assert(toolbar.deactivations == 1);
    assert(!toolbar.deactivatedOffMain);
    // 释放排在完成块之前：完成块按弱引用取控制器，取不到就不应用这份偏好，和读取开始前控制器就已释放时一样。
    DrainMainQueue();
    assert(toolbar.completions == 0);
}

// 打开设置应用的完成回调由 NSWorkspace 在并发队列上调用并释放。它持有的回退块若强引用控制器，IMK 在设置应用启动期间放掉控制器时，最后一次释放就落在那条队列上。这里把 NSWorkspace 换成只收下完成回调的桩，再在后台队列上放掉它。
static void TestDesktopLaunchDoesNotDeallocControllerOffMain() {
    Method locate = class_getInstanceMethod(NSWorkspace.class, @selector(URLForApplicationWithBundleIdentifier:));
    Method open = class_getInstanceMethod(NSWorkspace.class, @selector(openApplicationAtURL:configuration:completionHandler:));
    __block id pendingHandler = nil;
    IMP originalLocate = method_setImplementation(locate, imp_implementationWithBlock(^NSURL *(id workspace, NSString *identifier) {
        (void)workspace; (void)identifier;
        return [NSURL fileURLWithPath:@"/Applications/synthetic.app"];
    }));
    IMP originalOpen = method_setImplementation(open, imp_implementationWithBlock(^(id workspace, NSURL *url, id configuration, id handler) {
        (void)workspace; (void)url; (void)configuration;
        // 调用方传进来的是栈上的块，按 id 收下只会 retain 不会拷贝，要显式 copy。
        pendingHandler = [handler copy];
    }));
    void (^launches[])(MSIMEInputController *) = {
        ^(MSIMEInputController *controller) { [controller showDictionary:nil]; },
        ^(MSIMEInputController *controller) { [controller restartCurrentInputMethod]; },
    };
    for (auto launch : launches) {
        ThreadRecordingToolbar *toolbar = [ThreadRecordingToolbar new];
        @autoreleasepool {
            MSIMEInputController *controller = [MSIMEInputController alloc];
            [controller setValue:toolbar forKey:@"toolbar"];
            launch(controller);
        }
        assert(pendingHandler);
        // 设置应用还在启动，主线程这一侧已经放掉控制器，再转一圈主队列，让主线程上自动释放的引用都先放掉。
        DrainMainQueue();
        dispatch_semaphore_t released = dispatch_semaphore_create(0);
        dispatch_async(dispatch_get_global_queue(QOS_CLASS_UTILITY, 0), ^{
            pendingHandler = nil;
            dispatch_semaphore_signal(released);
        });
        assert(dispatch_semaphore_wait(released, dispatch_time(DISPATCH_TIME_NOW, 5 * NSEC_PER_SEC)) == 0);
        NSDate *deadline = [NSDate dateWithTimeIntervalSinceNow:2];
        while (toolbar.deactivations == 0 && deadline.timeIntervalSinceNow > 0)
            [NSRunLoop.currentRunLoop runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.005]];
        assert(toolbar.deactivations == 1);
        assert(!toolbar.deactivatedOffMain);
    }
    method_setImplementation(locate, originalLocate);
    method_setImplementation(open, originalOpen);
}

@interface ReloadCountingController : ModeController
@property(nonatomic) NSUInteger reloads;
@end
@implementation ReloadCountingController
- (void)reloadPreferences {
    ++self.reloads;
    [super reloadPreferences];
}
@end

// 每个控制器都观察同一份外观设置，所以改动由最先注册的那个保存，而它往往是早已失去焦点的控制器。保存后只重新载入它，持有焦点的会话就停留在原来的设置上：从 한 切到 中 要等下一次每秒一次的轮询才到达 Engine，在此之前敲的键按韩文组字，音节又把新方案挡到它结束。保存完成后，持有焦点的控制器应立即载入保存的文档。
static void TestSavedPreferencesReachTheFocusedController() {
    NSString *suite = [@"msime.preference-focus-save." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    MSIMEAppearancePreferences *prefs = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    NSString *root = [NSTemporaryDirectory() stringByAppendingPathComponent:NSUUID.UUID.UUIDString];
    ReloadCountingController *saver = [ReloadCountingController alloc];
    [saver setValue:prefs forKey:@"appearance"];
    [saver setValue:root forKey:@"preferencesDirectory"];
    ReloadCountingController *focused = [ReloadCountingController alloc];
    [focused setValue:prefs forKey:@"appearance"];
    ShortcutClient *client = [ShortcutClient new];
    assert(![focused handleEvent:TapEvent(NSEventTypeFlagsChanged, 56, 0, 1) client:client]);
    focused.reloads = 0;
    saver.reloads = 0;
    [saver persistAppearancePreferences];
    SettleWindowLayout();
    NSDate *deadline = [NSDate dateWithTimeIntervalSinceNow:2];
    while (focused.reloads == 0 && deadline.timeIntervalSinceNow > 0)
        [NSRunLoop.currentRunLoop runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.005]];
    assert(saver.reloads == 1 && focused.reloads == 1);
    NSError *error = nil;
    assert([MSIMEClientSession loadPreferencesInDirectory:root error:&error] && !error);
    [NSFileManager.defaultManager removeItemAtPath:root error:nil];
    MSIMERemoveTestPreferenceSuite(defaults, suite);
}

// 切换输入模式就是切换方案，而方案只能通过共享文档到达 Engine。Ctrl+Space 之后敲的第一个键必须按新方案组字，此时后台保存和轮询都还没轮到运行循环：切换当场写入文档并应用到会话。用真实会话逐个方向检查，也检查输入法菜单，并且随后的保存不能改动这次切换。
static void TestModeSwitchReachesTheSessionBeforeTheNextKey() {
    NSString *root = [NSTemporaryDirectory() stringByAppendingPathComponent:NSUUID.UUID.UUIDString];
    NSMutableDictionary *options = [@{@"api_version": @1,
        @"preferences": @{@"scheme": @"quanpin", @"default_ime_mode": @"chinese", @"candidate_page_size": @5,
                          @"learning": @NO, @"chinese_punctuation": @YES}} mutableCopy];
    for (NSString *name in @[@"resources", @"user_data", @"cache", @"dictionaries", @"preferences"]) {
        NSString *path = [root stringByAppendingPathComponent:name];
        assert([NSFileManager.defaultManager createDirectoryAtPath:path withIntermediateDirectories:YES attributes:nil error:nil]);
        if (![name isEqual:@"preferences"]) options[name] = path;
    }
    NSError *error = nil;
    MSIMEClientSession *session = [[MSIMEClientSession alloc] initWithOptions:options error:&error];
    assert(session && !error);
    assert([session setFocused:YES error:&error] && !error);
    NSString *suite = [@"msime.mode-switch-session." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    MSIMEAppearancePreferences *prefs =
        [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults skinsRoot:[NSURL fileURLWithPath:root]];
    prefs.inputScheme = @"quanpin";
    prefs.englishMode = NO;
    ShortcutClient *client = [ShortcutClient new];
    client.document = @"";
    client.caret = NSMakeRect(100, 100, 1, 16);
    ModeController *controller = [ModeController alloc];
    [controller setValue:prefs forKey:@"appearance"];
    [controller setValue:session forKey:@"session"];
    [controller setValue:client forKey:@"activeClient"];
    [controller setValue:[root stringByAppendingPathComponent:@"preferences"] forKey:@"preferencesDirectory"];
    [controller setValue:[[HiddenCandidatePanel alloc] init] forKey:@"panel"];
    MSIMEFocusedController = controller;
    MSIMEResetSystemInputModeState(MSIMESharedSystemInputModeState());
    auto scheme = ^NSInteger { return [[controller valueForKey:@"view"][@"scheme"] integerValue]; };
    auto type = ^(unsigned short keyCode, NSString *letter) {
        assert([controller handleEvent:KeypadKey(keyCode, letter, 0, NO) client:client]);
    };

    // 中 切到 한：紧接着的字母就是韩文字母（자모）。
    [controller systemDidReportInputMode:MSIMEKoreanInputModeID client:client];
    type(15, @"r");
    assert(scheme() == msime::mac::KoreanScheme && [client.marked isEqual:@"\u3131"]);
    // 音节未完成时从 한 切到 中：先把音节上屏，下一个字母按拼音处理。
    [controller systemDidReportInputMode:MSIMEChineseInputModeID client:client];
    assert([client.insertions.lastObject isEqual:@"\u3131"]);
    type(45, @"n");
    assert(scheme() == 0 && [client.marked isEqual:@"n"]);
    // 中 切到 日 再切回来。
    [controller systemDidReportInputMode:MSIMEJapaneseInputModeID client:client];
    type(40, @"k");
    assert(scheme() == 3);
    [controller systemDidReportInputMode:MSIMEChineseInputModeID client:client];
    type(45, @"n");
    assert(scheme() == 0 && [client.marked isEqual:@"n"]);
    // 输入法菜单里的方案项走同一条路径。
    NSMenuItem *korean = [[NSMenuItem alloc] initWithTitle:@"韩语" action:@selector(selectInputScheme:) keyEquivalent:@""];
    korean.representedObject = @"korean";
    [controller selectInputScheme:korean];
    type(15, @"r");
    assert(scheme() == msime::mac::KoreanScheme && [client.marked isEqual:@"\u3131"]);

    // 外观改动触发的后台保存会再写一遍相同的偏好，之后的重新载入既不会再切换一次，也不会切回去。
    [controller persistAppearancePreferences];
    SettleWindowLayout();
    NSDate *deadline = [NSDate dateWithTimeIntervalSinceNow:1.5];
    while (deadline.timeIntervalSinceNow > 0) [NSRunLoop.currentRunLoop runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.01]];
    type(15, @"r");
    assert(scheme() == msime::mac::KoreanScheme && [prefs.inputScheme isEqual:@"korean"]);
    NSDictionary *stored = [MSIMEClientSession loadPreferencesInDirectory:[root stringByAppendingPathComponent:@"preferences"] error:&error];
    assert(!error && [stored[@"preferences"][@"scheme"] isEqual:@"korean"]);

    MSIMEFocusedController = nil;
    MSIMEResetSystemInputModeState(MSIMESharedSystemInputModeState());
    [NSFileManager.defaultManager removeItemAtPath:root error:nil];
    MSIMERemoveTestPreferenceSuite(defaults, suite);
}

// #4288：用户在设置页选了全拼，下次再用又回到双拼。方案在宿主里有两份：共享偏好文档里的 `scheme` 是所有界面（设置页、`msime config set`、云端同步和本输入法自己）都写的那份，NSUserDefaults 的 `MSIMEClientInputScheme` 只有本输入法自己写。外观设置读文档里的值，没读到时退回 NSUserDefaults，而文档要等激活之后在后台载入。所以输入法进程重新启动后（重新登录、重启、更新），控制器在载入文档之前就按 NSUserDefaults 里本输入法上次写下的旧方案行事：把菜单栏选到「双」、把旧方案记为已同步，这期间的任何一次保存都会把旧方案写回文档，用户选的全拼就此丢失。这里用一个新的外观对象模拟新启动的进程：NSUserDefaults 里留着旧的双拼，文档里是用户后来选的全拼。
static void TestFreshProcessActsOnTheSharedSchemeNotTheStaleLocalOne() {
    NSString *root = [NSTemporaryDirectory() stringByAppendingPathComponent:NSUUID.UUID.UUIDString];
    NSError *error = nil;
    NSDictionary *snapshot = [MSIMEClientSession loadPreferencesInDirectory:root error:&error];
    assert(snapshot && !error);
    NSMutableDictionary *document = [snapshot[@"preferences"] mutableCopy];
    document[@"scheme"] = @"quanpin";
    document[@"last_chinese_scheme"] = @"quanpin";
    NSDictionary *saved = [MSIMEClientSession savePreferencesInDirectory:root expectedRevision:[snapshot[@"revision"] unsignedLongLongValue]
        snapshot:@{@"format_version": @1, @"revision": snapshot[@"revision"], @"preferences": document} error:&error];
    assert(saved && !error);
    auto freshProcess = ^(NSUserDefaults *defaults) {
        [defaults setObject:@"shuangpin" forKey:@"MSIMEClientInputScheme"];
        [defaults setObject:@"shuangpin" forKey:@"MSIMEClientLastSyncedInputScheme"];
        MSIMEAppearancePreferences *prefs = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
        prefs.englishMode = NO;
        ModeController *controller = [ModeController alloc];
        [controller setValue:prefs forKey:@"appearance"];
        [controller setValue:root forKey:@"preferencesDirectory"];
        return controller;
    };

    // 客户端获得焦点：菜单栏按文档里的全拼对齐，而不是 NSUserDefaults 里的双拼。
    NSString *suite = [@"msime.fresh-process-scheme." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    ModeController *controller = freshProcess(defaults);
    MSIMEAppearancePreferences *prefs = [controller valueForKey:@"appearance"];
    MSIMEResetSystemInputModeState(MSIMESharedSystemInputModeState());
    ModeSelectingClient *client = [ModeSelectingClient new];
    assert(![controller handleEvent:TapEvent(NSEventTypeFlagsChanged, 56, 0, 1) client:client]);
    assert([prefs.inputScheme isEqual:@"quanpin"] && [prefs.lastSyncedInputScheme isEqual:@"quanpin"]);
    assert(![client.selectedModes containsObject:MSIMEShuangpinInputModeID]);
    // 之后的任何一次保存都不能把旧方案写回文档。
    [controller persistAppearancePreferences];
    NSDate *deadline = [NSDate dateWithTimeIntervalSinceNow:2];
    while (MSIMESharedPreferenceSaveState.saving && deadline.timeIntervalSinceNow > 0)
        [NSRunLoop.currentRunLoop runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.005]];
    SettleWindowLayout();
    NSDictionary *stored = [MSIMEClientSession loadPreferencesInDirectory:root error:&error];
    assert(!error && [stored[@"preferences"][@"scheme"] isEqual:@"quanpin"]);
    MSIMEFocusedController = nil;
    MSIMERemoveTestPreferenceSuite(defaults, suite);

    // 系统的模式报告可能先于激活到达：切到「日」时记下的回程方案同样是文档里的全拼。
    suite = [@"msime.fresh-process-report." stringByAppendingString:NSUUID.UUID.UUIDString];
    defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    controller = freshProcess(defaults);
    prefs = [controller valueForKey:@"appearance"];
    MSIMEResetSystemInputModeState(MSIMESharedSystemInputModeState());
    client = [ModeSelectingClient new];
    [controller setValue:client forKey:@"activeClient"];
    [controller systemDidReportInputMode:MSIMEJapaneseInputModeID client:client];
    assert([prefs.inputScheme isEqual:@"japanese"] && [prefs.lastChineseScheme isEqual:@"quanpin"]);

    MSIMEFocusedController = nil;
    MSIMEResetSystemInputModeState(MSIMESharedSystemInputModeState());
    [NSFileManager.defaultManager removeItemAtPath:root error:nil];
    MSIMERemoveTestPreferenceSuite(defaults, suite);
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
    // 没有别的声音时打开的音乐要等播放器听到输入法处于激活才响，所以每份应用了的文档都经 msime_client_set_active 再报告一次；没变的文档不应用，也不报告。
    [controller claimBackgroundMusic];
    assert(([session.activeStates isEqual:@[@YES]]));

    [controller reloadPreferences];
    assert(dispatch_semaphore_wait(controller.reads[0].started, dispatch_time(DISPATCH_TIME_NOW, NSEC_PER_SEC)) == 0);
    dispatch_semaphore_signal(controller.reads[0].released);
    WaitForPreferenceCompletions(controller, 1);
    assert(controller.appliedPreferences.count == 1 && session.updates == 1);
    assert(([session.activeStates isEqual:@[@YES, @YES]]));

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
    assert(session.activeStates.count == 3 && !session.musicStates.count);
    [controller releaseBackgroundMusic];

    // A local edit invalidating what was applied - so a document rolled back to a revision this
    // session already saw is applied again - is the load state's own rule, pinned next to it in
    // preference-load-state rather than here: appearanceChanged: reaches half the controller.
    MSIMERemoveTestPreferenceSuite(defaults, suite);
}

static void TestPreferenceRevisionRetriesFailedUpdate() {
    NSString *suite = [@"msime.preference-retry." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    MSIMEAppearancePreferences *prefs = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    AsyncPreferencesController *controller = [AsyncPreferencesController alloc];
    AsyncPreferenceSession *session = [AsyncPreferenceSession new];
    session.failedUpdatesRemaining = 1;
    NSMutableArray *reads = [NSMutableArray array];
    for (NSUInteger index = 0; index < 2; ++index) {
        ControlledPreferenceRead *read = [ControlledPreferenceRead new];
        read.snapshot = @{@"revision":@7, @"preferences":@{@"chinese_punctuation":@YES}};
        [reads addObject:read];
    }
    controller.reads = reads;
    controller.appliedPreferences = [NSMutableArray array];
    [controller setValue:prefs forKey:@"appearance"];
    [controller setValue:[ShortcutClient new] forKey:@"activeClient"];
    [controller setValue:session forKey:@"session"];
    [controller setValue:@"/synthetic-preferences" forKey:@"preferencesDirectory"];

    [controller reloadPreferences];
    assert(dispatch_semaphore_wait(controller.reads[0].started, dispatch_time(DISPATCH_TIME_NOW, NSEC_PER_SEC)) == 0);
    dispatch_semaphore_signal(controller.reads[0].released);
    WaitForPreferenceCompletions(controller, 1);
    assert(session.updates == 1 && controller.appliedPreferences.count == 0);

    // 第一次会话更新失败后，同一份修订号仍要在下一次轮询时交给会话。
    [controller reloadPreferences];
    assert(dispatch_semaphore_wait(controller.reads[1].started, dispatch_time(DISPATCH_TIME_NOW, NSEC_PER_SEC)) == 0);
    dispatch_semaphore_signal(controller.reads[1].released);
    WaitForPreferenceCompletions(controller, 2);
    assert(session.updates == 2 && controller.appliedPreferences.count == 1);
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

// 失焦保留会话后，应用可以只发按键而不再发 activateServer:。
static void TestKeyFocusRestoresPreferenceMonitoring() {
    Method base = class_getInstanceMethod(IMKInputController.class, @selector(deactivateServer:));
    IMP original = method_setImplementation(base, (IMP)RecordBaseDeactivation);
    for (NSNumber *english in @[@NO, @YES]) {
        NSString *suite = [@"msime.preference-key-focus." stringByAppendingString:NSUUID.UUID.UUIDString];
        NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
        MSIMEAppearancePreferences *prefs = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
        [prefs applySharedInputPreferences:@{@"chinese_punctuation":@NO}];
        prefs.englishMode = english.boolValue;
        RecoveringPreferencesController *controller = [RecoveringPreferencesController alloc];
        AsyncPreferenceSession *session = [AsyncPreferenceSession new];
        ShortcutClient *client = [ShortcutClient new];
        controller.appliedPreferences = [NSMutableArray array];
        controller.readSnapshot = @{@"revision":@1, @"preferences":@{@"chinese_punctuation":@NO}};
        [controller setValue:prefs forKey:@"appearance"];
        [controller setValue:session forKey:@"session"];
        [controller setValue:client forKey:@"activeClient"];
        [controller setValue:@"/synthetic-key-focus-preferences" forKey:@"preferencesDirectory"];
        [controller startPreferencesMonitoring];
        WaitForRecoveringCompletions(controller, 1);
        NSTimer *oldTimer = [controller valueForKey:@"preferencesTimer"];
        [controller deactivateServer:client];
        assert(!oldTimer.valid && ![controller valueForKey:@"preferencesTimer"]);
        assert([controller valueForKey:@"session"] == session && ![controller valueForKey:@"activeClient"]);

        controller.readSnapshot = @{@"revision":@2, @"preferences":@{@"chinese_punctuation":@YES}};
        // 修饰键释放不会准备会话；监视必须由接管客户端的路径恢复。
        assert(![controller handleEvent:TapEvent(NSEventTypeFlagsChanged, 56, 0, 1) client:client]);
        assert([[controller valueForKey:@"preferencesTimer"] isValid]);
        WaitForRecoveringCompletions(controller, 2);
        assert(session.updates == 2 && prefs.chinesePunctuation);
        assert([session.lastPreferences isEqual:controller.readSnapshot]);
        assert(controller.preparationCalls == 0 && [controller valueForKey:@"session"] == session);

        // 不显式 reload 或触发 timer，验证下一次保存能由真实轮询应用。
        controller.readSnapshot = @{@"revision":@3, @"preferences":@{@"chinese_punctuation":@NO}};
        WaitForRecoveringCompletions(controller, 3);
        assert(session.updates == 3 && !prefs.chinesePunctuation);
        assert([session.lastPreferences isEqual:controller.readSnapshot]);
        [controller deactivateServer:client];
        MSIMERemoveTestPreferenceSuite(defaults, suite);
    }
    method_setImplementation(base, original);
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

    // 进程长期运行时，旧的偏好目录应当从恢复去重表中淘汰，不能让用户切换配置目录的次数决定常驻内存。
    RecoveringPreferencesController *evicted = [RecoveringPreferencesController alloc];
    evicted.appliedPreferences = [NSMutableArray array];
    [evicted setValue:prefs forKey:@"appearance"];
    [evicted setValue:[ShortcutClient new] forKey:@"activeClient"];
    NSString *evictedDirectory = [@"/synthetic-recovery-eviction-" stringByAppendingString:NSUUID.UUID.UUIDString];
    [evicted setValue:evictedDirectory forKey:@"preferencesDirectory"];
    [evicted reloadPreferences];
    WaitForRecoveringCompletions(evicted, 1);
    assert(evicted.recoverCalls == 1);

    for (NSUInteger index = 0; index < 65; ++index) {
        RecoveringPreferencesController *other = [RecoveringPreferencesController alloc];
        other.appliedPreferences = [NSMutableArray array];
        [other setValue:prefs forKey:@"appearance"];
        [other setValue:[ShortcutClient new] forKey:@"activeClient"];
        NSString *directory = [NSString stringWithFormat:@"/synthetic-recovery-capacity-%@-%lu",
                               NSUUID.UUID.UUIDString, (unsigned long)index];
        [other setValue:directory forKey:@"preferencesDirectory"];
        [other reloadPreferences];
        WaitForRecoveringCompletions(other, 1);
        assert(other.recoverCalls == 1);
    }

    [evicted reloadPreferences];
    WaitForRecoveringCompletions(evicted, 2);
    assert(evicted.recoverCalls == 2);
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

// A Chinese/English switch is not part of the shared document, so it asks no controller to save it. Saving on every Shift tap wrote the whole in-memory view of the settings over whatever another writer had just saved.
static void TestInputModeSwitchDoesNotSaveSharedPreferences() {
    NSString *suite = [@"msime.mode-switch-save." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    MSIMEAppearancePreferences *prefs = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    VoiceSettingsPersistenceController *controller = [VoiceSettingsPersistenceController alloc];
    [controller setValue:prefs forKey:@"appearance"];
    [NSNotificationCenter.defaultCenter addObserver:controller selector:@selector(appearanceChanged:)
                                               name:MSIMEAppearanceDidChangeNotification object:prefs];
    __block NSUInteger announcements = 0;
    id observer = [NSNotificationCenter.defaultCenter addObserverForName:MSIMEAppearanceDidChangeNotification object:prefs queue:nil usingBlock:^(NSNotification *note) { (void)note; ++announcements; }];
    prefs.englishMode = YES;
    prefs.englishMode = NO;
    // Observers still hear about the switch; only the save is skipped.
    assert(announcements == 2 && controller.persistenceRequests == 0);
    prefs.traditionalOutput = YES;
    assert(announcements == 3 && controller.persistenceRequests == 1);
    [NSNotificationCenter.defaultCenter removeObserver:observer];
    [NSNotificationCenter.defaultCenter removeObserver:controller name:MSIMEAppearanceDidChangeNotification object:prefs];
    MSIMERemoveTestPreferenceSuite(defaults, suite);
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

static void TestShiftChordRecoversWithoutFlagsChanged() {
    for (const auto type : {NSEventTypeKeyDown, NSEventTypeKeyUp}) {
        MSIMEModifierTap tap;
        gHeldKeys.clear();
        tap.setKeyHeldProbe(&SyntheticKeyHeld);
        assert(!tap.observe(TapEvent(NSEventTypeFlagsChanged, 56, NSEventModifierFlagShift, 1.0), true, true));
        assert(!tap.observe(TapEvent(NSEventTypeKeyDown, 0, NSEventModifierFlagShift, 1.1), true, true));
        assert(tap.shiftChordInterrupted());
        // 双侧 Shift 中只松开一侧，同一轮的取消状态仍保留。
        assert(!tap.observe(TapEvent(NSEventTypeFlagsChanged, 60, NSEventModifierFlagShift, 1.2), true, true));
        assert(!tap.observe(TapEvent(NSEventTypeFlagsChanged, 56, NSEventModifierFlagShift, 1.3), true, true));
        assert(tap.shiftChordInterrupted());
        // 最后一侧释放通知丢失，但普通按键已经报告 Shift 不再按住。
        assert(!tap.observe(TapEvent(type, 0, 0, 1.4), true, true));
        assert(!tap.shiftChordInterrupted());
        assert(!tap.observe(TapEvent(NSEventTypeFlagsChanged, 60, NSEventModifierFlagShift, 2.0), true, true));
        assert(!tap.observe(TapEvent(NSEventTypeKeyDown, 49, NSEventModifierFlagShift, 2.1), true, true));
        assert(!tap.shiftChordInterrupted());
        assert(!tap.observe(TapEvent(NSEventTypeKeyDown, 49, NSEventModifierFlagShift, 2.2), true, true));
        assert(!tap.shiftChordInterrupted());
        assert(!tap.observe(TapEvent(NSEventTypeKeyDown, 0, NSEventModifierFlagShift, 2.3), true, true));
        assert(tap.shiftChordInterrupted());
        tap.reset();
        assert(!tap.shiftChordInterrupted());
    }
}

// 焦点和鼠标监视器是系统边界，用替身驱动真实控制器，CI 无需辅助功能授权。
@interface EditableShiftController : ModeController
@property(nonatomic, strong) id focusIdentity;
@property(nonatomic) pid_t focusPID;
@property(nonatomic) NSUInteger focusCaptures;
@property(nonatomic) NSUInteger modeSwitchCalls;
@property(nonatomic) NSUInteger monitorStarts;
@property(nonatomic) BOOL monitoring;
@property(nonatomic) BOOL allowMonitoring;
@end
@implementation EditableShiftController
- (msime::mac::EditableShiftFocus)editableShiftFocusForClient:(id)sender {
    (void)sender;
    ++self.focusCaptures;
    return {self.focusPID, self.focusIdentity};
}
- (BOOL)startEditableShiftMouseMonitoring {
    ++self.monitorStarts;
    self.monitoring = self.allowMonitoring;
    return self.allowMonitoring;
}
- (void)stopEditableShiftMouseMonitoring { self.monitoring = NO; }
- (void)toggleInputModeFromModifierTap {
    ++self.modeSwitchCalls;
    [super toggleInputModeFromModifierTap];
}
@end

static void TestEditableShiftController() {
    NSString *suite = [@"msime.editable-shift." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    MSIMEAppearancePreferences *prefs = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    assert(!prefs.shiftTapRequiresEditable);
    NSDictionary *restricted = @{@"keybindings":@{@"switch_language_shift_requires_editable":@YES}};
    [prefs applySharedInputPreferences:restricted];
    assert(prefs.shiftTapRequiresEditable);
    assert([[prefs sharedPreferencesByMerging:restricted][@"keybindings"][@"switch_language_shift_requires_editable"] boolValue]);
    for (NSDictionary *removed : @[@{}, @{@"keybindings":@{}}, @{@"keybindings":NSNull.null},
                                   @{@"keybindings":@{@"switch_language_shift_requires_editable":@"true"}}]) {
        [prefs applySharedInputPreferences:removed];
        assert(!prefs.shiftTapRequiresEditable);
        [prefs applySharedInputPreferences:restricted];
    }
    MSIMEAppearancePreferences *reopened = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    [reopened applySharedInputPreferences:restricted];
    assert(reopened.shiftTapRequiresEditable);
    EditableShiftController *controller = [EditableShiftController alloc];
    ShortcutClient *client = [ShortcutClient new];
    [controller setValue:prefs forKey:@"appearance"];
    [controller setValue:client forKey:@"activeClient"];
    controller.allowMonitoring = YES;
    controller.focusPID = 1;
    controller.focusIdentity = @"synthetic-empty-field";
    prefs.englishMode = NO;
    auto down = TapEvent(NSEventTypeFlagsChanged, 56, NSEventModifierFlagShift, 1.0);
    auto up = TapEvent(NSEventTypeFlagsChanged, 56, 0, 1.1);
    assert(![controller handleEvent:down client:client]);
    assert(controller.monitoring);
    assert([controller handleEvent:up client:client]);
    assert(prefs.englishMode && controller.modeSwitchCalls == 1 && !controller.monitoring);
    for (int failure = 0; failure < 6; ++failure) {
        [controller resetModifierTap];
        controller.focusPID = 1;
        controller.focusIdentity = failure == 0 ? nil : @"synthetic-empty-field";
        controller.allowMonitoring = failure != 5;
        assert(![controller handleEvent:down client:client]);
        if (failure == 1) controller.focusIdentity = nil; // 权限撤回或焦点未知。
        if (failure == 2) controller.focusIdentity = @"other-field";
        if (failure == 3) controller.focusPID = 2;
        if (failure == 4) [controller cancelEditableShiftTap]; // 鼠标事件原样放行，但取消本轮。
        assert(![controller handleEvent:up client:client]);
        assert(prefs.englishMode && controller.modeSwitchCalls == 1 && !controller.monitoring);
    }
    // 焦点无法确认不会进入模式切换，自然也不会走切换 HUD。
    controller.focusIdentity = nil;
    controller.allowMonitoring = YES;
    const NSUInteger captures = controller.focusCaptures;
    [prefs applySharedInputPreferences:@{}];
    assert(![controller handleEvent:down client:client]);
    assert([controller handleEvent:up client:client]);
    assert(!prefs.englishMode && controller.modeSwitchCalls == 2);
    assert(controller.focusCaptures == captures && !controller.monitoring);
    [prefs applySharedInputPreferences:restricted];
    prefs.controlTapShortcut = YES;
    assert(![controller handleEvent:TapEvent(NSEventTypeFlagsChanged, 59, NSEventModifierFlagControl, 2.0) client:client]);
    assert([controller handleEvent:TapEvent(NSEventTypeFlagsChanged, 59, 0, 2.1) client:client]);
    assert(prefs.englishMode && controller.modeSwitchCalls == 3);
    controller.focusIdentity = @"synthetic-empty-field";
    assert(![controller handleEvent:down client:client]);
    [controller resetModifierTap];
    assert(!controller.monitoring && ![controller handleEvent:up client:client]);
    assert(controller.modeSwitchCalls == 3);
    [defaults removePersistentDomainForName:suite];
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
        if (flag == NSEventModifierFlagShift) {
            tap.reset();
            assert(!tap.observe(down, true, true));
            assert(!tap.observe(TapEvent(NSEventTypeKeyDown, 0, flag, 1.02), true, true));
            assert(tap.shiftChordInterrupted());
            // 空格本身不应改变状态，控制器随后会把它交给正常输入路径。
            assert(!tap.observe(TapEvent(NSEventTypeKeyDown, 49, flag, 1.03), true, true));
            assert(tap.shiftChordInterrupted());
            assert(!tap.observe(up, true, true));
            assert(!tap.shiftChordInterrupted());
        }
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
    const NSUInteger tapKeys = session.keys.count;
    assert(![controller handleEvent:TapEvent(NSEventTypeFlagsChanged, 56, NSEventModifierFlagShift, 3.0) client:client]);
    assert([controller handleEvent:TapEvent(NSEventTypeFlagsChanged, 56, 0, 3.1) client:client]);
    assert(session.rawCommitCalls == rawBefore + 1);
    assert([client.committed isEqual:@"msime"] && prefs.englishMode);
    // 单按修饰键是一次物理按下：原样上屏之前先按指针报告（不出按键音），这次派发才属于它自己的一轮。
    assert(session.keys.count == tapKeys + 1 && [session.keys.lastObject isEqual:@(MSIME_KEY_POINTER)]);
    const NSUInteger tapKey = [session.calls indexOfObject:[NSString stringWithFormat:@"key:%u", MSIME_KEY_POINTER]];
    assert((tapKey != NSNotFound && [session.calls[tapKey + 1] isEqual:[NSString stringWithFormat:@"command:%u", MSIME_COMMIT_RAW]]));

    // Nothing composing, nothing to send out: the tap is only the mode switch.
    prefs.englishMode = NO;
    [controller setValue:@{@"editing_text":@"", @"caret_position":@0, @"candidates":@[]} forKey:@"view"];
    rawBefore = session.rawCommitCalls;
    assert(![controller handleEvent:TapEvent(NSEventTypeFlagsChanged, 56, NSEventModifierFlagShift, 4.0) client:client]);
    assert([controller handleEvent:TapEvent(NSEventTypeFlagsChanged, 56, 0, 4.1) client:client]);
    assert(session.rawCommitCalls == rawBefore && prefs.englishMode);
    // 没有派发就不开一轮。
    assert(session.keys.count == tapKeys + 1);
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

    // Shift+字符后仍按住 Shift 按空格：这次空格必须走英文正常输入，不能再次切换中英文。
    for (const BOOL shiftTap : {NO, YES}) {
        NSString *spaceSuite = [@"msime.shift-space-chord." stringByAppendingString:NSUUID.UUID.UUIDString];
        NSUserDefaults *spaceDefaults = [[NSUserDefaults alloc] initWithSuiteName:spaceSuite];
        MSIMEAppearancePreferences *spacePrefs = [[MSIMEAppearancePreferences alloc] initWithDefaults:spaceDefaults];
        spacePrefs.inputModeShortcut = YES;
        spacePrefs.shiftTapShortcut = shiftTap;
        spacePrefs.englishMode = YES;
        ModeController *spaceController = [ModeController alloc];
        ShortcutClient *spaceClient = [ShortcutClient new];
        ShortcutSession *spaceSession = [ShortcutSession new];
        [spaceController setValue:spacePrefs forKey:@"appearance"];
        [spaceController setValue:spaceClient forKey:@"activeClient"];
        [spaceController setValue:spaceSession forKey:@"session"];
        [spaceController setValue:@{ @"editing_text": @"", @"candidates": @[] } forKey:@"view"];
        assert(![spaceController handleEvent:TapEvent(NSEventTypeFlagsChanged, 56, NSEventModifierFlagShift, 5.0) client:spaceClient]);
        (void)[spaceController handleEvent:ModeKey(0, NSEventModifierFlagShift, NO) client:spaceClient];
        assert(![spaceController handleEvent:TapEvent(NSEventTypeKeyUp, 0, NSEventModifierFlagShift, 5.1) client:spaceClient]);
        assert(![spaceController handleEvent:ModeKey(49, NSEventModifierFlagShift, NO) client:spaceClient]);
        assert(spacePrefs.englishMode);
        // 漏掉 Shift 释放通知后，字符的 KeyUp 已确认松开；新一轮快捷键仍要切换一次。
        assert(![spaceController handleEvent:TapEvent(NSEventTypeKeyUp, 0, 0, 5.2) client:spaceClient]);
        assert(![spaceController handleEvent:TapEvent(NSEventTypeFlagsChanged, 56, NSEventModifierFlagShift, 5.3) client:spaceClient]);
        assert([spaceController handleEvent:ModeKey(49, NSEventModifierFlagShift, NO) client:spaceClient]);
        assert(!spacePrefs.englishMode);
        assert(![spaceController handleEvent:TapEvent(NSEventTypeFlagsChanged, 56, 0, 5.4) client:spaceClient]);
        assert(!spacePrefs.englishMode);
        MSIMERemoveTestPreferenceSuite(spaceDefaults, spaceSuite);
    }
    MSIMERemoveTestPreferenceSuite(defaults, suite);
}

// 菜单栏状态项（#7066）：共享偏好 input_mode_status_item 默认关闭；打开后每次切换中英文都把状态项摆成控制器算出的模式，换方案时显示真实方案，⌃⇧E 的英文候选模式仍是「中」；载入的共享文档里关上时立即隐藏，打开时立即按当前模式显示，不等下一次切换；原生设置窗口的开关关上时，没有客户端的控制器也立即隐藏。状态项换成记录用的替身，测试进程不往菜单栏放东西。
static void TestInputModeStatusItem() {
    NSString *suite = [@"msime.mode-status-item." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    MSIMEAppearancePreferences *prefs = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    assert(!prefs.inputModeStatusItem);
    assert([[prefs sharedPreferencesByMerging:@{}][@"input_mode_status_item"] isEqual:@NO]);
    [prefs applySharedCandidatePreferences:@{@"input_mode_status_item": @YES}];
    assert(prefs.inputModeStatusItem);
    // 不是布尔值的字段不算数，沿用已有的值。
    [prefs applySharedCandidatePreferences:@{@"input_mode_status_item": @"false"}];
    assert(prefs.inputModeStatusItem);
    [prefs applySharedCandidatePreferences:@{@"input_mode_status_item": @NO}];
    assert(!prefs.inputModeStatusItem);
    // 原生设置窗口的开关写同一个偏好，重开后仍在。
    NSButton *toggle = (id)PreferenceControl(prefs, NSSelectorFromString(@"inputModeStatusItemChanged:"));
    assert(toggle.state == NSControlStateValueOff);
    toggle.state = NSControlStateValueOn;
    assert([NSApp sendAction:toggle.action to:toggle.target from:toggle]);
    assert(prefs.inputModeStatusItem && [[[MSIMEAppearancePreferences alloc] initWithDefaults:defaults] inputModeStatusItem]);
    assert([[prefs sharedPreferencesByMerging:@{}][@"input_mode_status_item"] isEqual:@YES]);

    prefs.inputModeStatusItem = NO;
    prefs.inputScheme = @"quanpin";
    prefs.englishMode = NO;
    ModeController *controller = [ModeController alloc];
    ShortcutClient *client = [ShortcutClient new];
    ShortcutSession *session = [ShortcutSession new];
    [controller setValue:prefs forKey:@"appearance"];
    [controller setValue:client forKey:@"activeClient"];
    [controller setValue:session forKey:@"session"];
    [controller setValue:@{@"editing_text":@"", @"caret_position":@0, @"candidates":@[]} forKey:@"view"];
    NSMutableArray *shown = [NSMutableArray array];
    [controller setValue:^(NSString *mode) { [shown addObject:mode ?: NSNull.null]; } forKey:@"inputModeStatusItemPresenter"];
    auto tap = [&](double time) {
        assert(![controller handleEvent:TapEvent(NSEventTypeFlagsChanged, 56, NSEventModifierFlagShift, time) client:client]);
        assert([controller handleEvent:TapEvent(NSEventTypeFlagsChanged, 56, 0, time + 0.1) client:client]);
    };

    // 关着时切换只会确认它隐藏着，从不显示。
    tap(1.0);
    assert(prefs.englishMode && shown.count > 0 && [shown.lastObject isEqual:NSNull.null]);
    assert(![shown containsObject:MSIMEEnglishInputModeID] && ![shown containsObject:MSIMEChineseInputModeID]);

    // 打开的共享文档立即按当前模式显示，不等下一次切换。
    [controller applySharedToolbarPreferences:@{@"input_mode_status_item": @YES}];
    assert(prefs.inputModeStatusItem && [shown.lastObject isEqual:MSIMEEnglishInputModeID]);
    tap(2.0);
    assert(!prefs.englishMode && [shown.lastObject isEqual:MSIMEChineseInputModeID]);
    tap(3.0);
    assert(prefs.englishMode && [shown.lastObject isEqual:MSIMEEnglishInputModeID]);
    tap(4.0);
    assert(!prefs.englishMode && [shown.lastObject isEqual:MSIMEChineseInputModeID]);

    // 换到双拼时显示「双」，不论系统里有没有加入「双」这个输入源。
    prefs.inputScheme = @"shuangpin";
    [controller applySharedToolbarPreferences:@{@"input_mode_status_item": @YES}];
    assert([shown.lastObject isEqual:MSIMEShuangpinInputModeID]);
    tap(5.0);
    assert([shown.lastObject isEqual:MSIMEEnglishInputModeID]);
    tap(6.0);
    assert([shown.lastObject isEqual:MSIMEShuangpinInputModeID]);
    prefs.inputScheme = @"quanpin";

    // ⌃⇧E 的英文候选模式不是英文模式：键仍经过引擎，从「英」进入它时状态项和系统图标一样回到「中」。
    [controller applySharedToolbarPreferences:@{@"input_mode_status_item": @YES}];
    tap(6.5);
    assert(prefs.englishMode && [shown.lastObject isEqual:MSIMEEnglishInputModeID]);
    assert([controller handleEvent:ModeKey(14, NSEventModifierFlagControl | NSEventModifierFlagShift, NO) client:client]);
    assert(session.dedicatedEnglish && !prefs.englishMode && [shown.lastObject isEqual:MSIMEChineseInputModeID]);
    assert([controller handleEvent:ModeKey(14, NSEventModifierFlagControl | NSEventModifierFlagShift, NO) client:client]);
    assert(!session.dedicatedEnglish);
    // 松开 E 和 ⌃⇧，像真实键盘那样，下一次单按 Shift 才从没有修饰键的状态开始。
    assert(![controller handleEvent:TapEvent(NSEventTypeKeyUp, 14, NSEventModifierFlagControl | NSEventModifierFlagShift, 6.8) client:client]);
    assert(![controller handleEvent:TapEvent(NSEventTypeFlagsChanged, 59, 0, 6.9) client:client]);

    // 关上的共享文档立即隐藏，之后的切换也不再显示。
    [controller applySharedToolbarPreferences:@{@"input_mode_status_item": @NO}];
    assert(!prefs.inputModeStatusItem && [shown.lastObject isEqual:NSNull.null]);
    const NSUInteger hidden = shown.count;
    tap(7.0);
    assert(prefs.englishMode && shown.count > hidden);
    for (NSUInteger index = hidden; index < shown.count; ++index) assert([shown[index] isEqual:NSNull.null]);

    // 没有客户端的控制器（输入法不在前台时收到通知）只隐藏，不把状态项放回菜单栏。
    [controller setValue:nil forKey:@"activeClient"];
    const NSUInteger before = shown.count;
    [controller applySharedToolbarPreferences:@{@"input_mode_status_item": @YES}];
    assert(shown.count == before);

    // 原生设置窗口在输入法进程里，拨它的开关时没有控制器持有客户端，文档也不会被重新载入：关上经 appearanceChanged: 立即隐藏，打开不把状态项放回菜单栏，等下一次获得焦点。
    [controller setValue:nil forKey:@"session"];
    [NSNotificationCenter.defaultCenter addObserver:controller selector:@selector(appearanceChanged:)
                                               name:MSIMEAppearanceDidChangeNotification object:prefs];
    toggle.state = NSControlStateValueOff;
    assert([NSApp sendAction:toggle.action to:toggle.target from:toggle]);
    assert(!prefs.inputModeStatusItem && shown.count == before + 1 && [shown.lastObject isEqual:NSNull.null]);
    toggle.state = NSControlStateValueOn;
    assert([NSApp sendAction:toggle.action to:toggle.target from:toggle]);
    assert(prefs.inputModeStatusItem && shown.count == before + 1);
    [NSNotificationCenter.defaultCenter removeObserver:controller name:MSIMEAppearanceDidChangeNotification object:prefs];
    prefs.englishMode = NO;
    MSIMERemoveTestPreferenceSuite(defaults, suite);
}

// 单按 Caps Lock 切换中英文。合成的时钟、按键状态和撤回大写锁定：单测不能动跑测试那台机器的真实大写锁定。
static std::int64_t gCapsNow = 0;
static bool gCapsHeld = false;
static bool gCapsHIDKnown = false;
static bool gCapsHIDHeld = false;
static bool gCapsReleaseWorks = true;
static NSUInteger gCapsReleases = 0;
static std::int64_t SyntheticCapsNow() { return gCapsNow; }
static bool SyntheticCapsHID(bool *known) {
    if (known) *known = gCapsHIDKnown;
    return gCapsHIDHeld;
}
static CGEventFlags SyntheticCapsFlags(CGEventSourceStateID state) {
    assert(state == kCGEventSourceStateHIDSystemState);
    // 松手后逻辑锁定位仍为真，只有实体按住位清除；不能把锁定当成长按。
    return NX_ALPHASHIFTMASK | (gCapsHeld ? NX_ALPHASHIFT_STATELESS_MASK : 0);
}
static bool SyntheticCapsRelease() { ++gCapsReleases; return gCapsReleaseWorks; }

static void TestCapsLockTap() {
    const auto realRelease = MSIMECapsLockRelease;
    const auto realHID = MSIMECapsLockHIDState;
    const auto realFlags = MSIMEPhysicalModifierFlagsState;
    const auto realNow = MSIMECapsLockNow;
    MSIMECapsLockRelease = &SyntheticCapsRelease;
    MSIMECapsLockHIDState = &SyntheticCapsHID;
    MSIMEPhysicalModifierFlagsState = &SyntheticCapsFlags;
    MSIMECapsLockNow = &SyntheticCapsNow;
    gCapsReleases = 0;
    gCapsReleaseWorks = true;
    gCapsHIDKnown = true;
    gCapsHIDHeld = true;
    assert(MSIMECapsLockPhysicallyHeld());
    gCapsHIDHeld = false;
    assert(!MSIMECapsLockPhysicallyHeld());
    gCapsHIDKnown = false;

    NSString *suite = [@"msime.caps-lock-tap." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    MSIMEAppearancePreferences *prefs = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    // 缺省关，共享偏好的键读得进来也写得回去。
    assert(!prefs.capsLockTapShortcut);
    [prefs applySharedInputPreferences:@{@"keybindings":@{@"switch_language_caps_lock":@YES}}];
    assert(prefs.capsLockTapShortcut);
    prefs.capsLockTapShortcut = YES;
    assert([[prefs sharedPreferencesByMerging:@{}][@"keybindings"][@"switch_language_caps_lock"] isEqual:@YES]);
    NSSwitch *toggle = (id)PreferenceControl(prefs, NSSelectorFromString(@"capsLockTapShortcutChanged:"));
    assert(toggle.state == NSControlStateValueOn);

    ModeController *controller = [ModeController alloc];
    ShortcutClient *client = [ShortcutClient new];
    ShortcutSession *session = [ShortcutSession new];
    [controller setValue:prefs forKey:@"appearance"];
    [controller setValue:client forKey:@"activeClient"];
    [controller setValue:session forKey:@"session"];
    session.finishTransition = @{@"handled":@YES, @"view":@{@"editing_text":@"", @"caret_position":@0, @"candidates":@[]}};
    session.cancelTransition = session.finishTransition;
    prefs.englishMode = NO;
    session.capsLock = NO;
    auto capsOn = [](double seconds) { return TapEvent(NSEventTypeFlagsChanged, 0x39, NSEventModifierFlagCapsLock, seconds); };
    auto capsOff = [](double seconds) { return TapEvent(NSEventTypeFlagsChanged, 0x39, 0, seconds); };

    // 轻点：大写锁定打开的那一刻不报告，松开后撤回大写锁定并切到英文；键交还应用。
    NSUInteger reports = session.capsLockCalls;
    gCapsNow = 1000; gCapsHeld = true;
    assert(![controller handleEvent:capsOn(1.0) client:client]);
    assert(session.capsLockCalls == reports && !session.capsLock);
    gCapsNow = 1100;
    [controller pollCapsLockTap];
    assert(gCapsReleases == 0 && !prefs.englishMode);
    gCapsNow = 1150; gCapsHeld = false;
    [controller pollCapsLockTap];
    assert(gCapsReleases == 1 && prefs.englishMode);
    assert(session.capsLockCalls == reports && !session.capsLock);
    // 系统撤回锁定后紧跟着送来的旧 Caps Lock 修饰位不能让英文重新变成大写。
    assert([controller handleEvent:LayoutKey(0, @"A", NSEventModifierFlagCapsLock) client:client]);
    assert([client.insertions.lastObject isEqual:@"a"] && !session.capsLock);
    // 撤回之后系统报来的「大写锁定关」不会再切换，也不用报告。
    assert(![controller handleEvent:capsOff(1.2) client:client]);
    assert(prefs.englishMode && session.capsLockCalls == reports);
    // 再轻点一次切回中文。
    gCapsNow = 2000; gCapsHeld = true;
    assert(![controller handleEvent:capsOn(2.0) client:client]);
    gCapsNow = 2050; gCapsHeld = false;
    [controller pollCapsLockTap];
    assert(gCapsReleases == 2 && !prefs.englishMode);

    // 长按：超过 300 毫秒还按着，大写锁定照常生效并报告给会话，不切换。
    gCapsNow = 3000; gCapsHeld = true;
    assert(![controller handleEvent:capsOn(3.0) client:client]);
    gCapsNow = 3301;
    [controller pollCapsLockTap];
    assert(session.capsLock && session.capsLockCalls == reports + 1 && !prefs.englishMode && gCapsReleases == 2);
    gCapsHeld = false;
    [controller pollCapsLockTap];
    assert(gCapsReleases == 2 && !prefs.englishMode);
    // 大写锁定开着时按它是关掉大写锁定，照常处理，不切换。
    assert(![controller handleEvent:capsOff(3.5) client:client]);
    assert(!session.capsLock && session.capsLockCalls == reports + 2 && !prefs.englishMode);

    // 两次轮询之间松开、但已经过了分界：仍然算长按。
    gCapsNow = 4000; gCapsHeld = true;
    assert(![controller handleEvent:capsOn(4.0) client:client]);
    gCapsNow = 4350; gCapsHeld = false;
    [controller pollCapsLockTap];
    assert(session.capsLock && !prefs.englishMode && gCapsReleases == 2);
    assert(![controller handleEvent:capsOff(4.5) client:client]);

    // 按着 Caps Lock 打字是在用大写锁定：大写锁定立即报告，松开也不切换。
    gCapsNow = 5000; gCapsHeld = true;
    assert(![controller handleEvent:capsOn(5.0) client:client]);
    (void)[controller handleEvent:ModeKey(0, NSEventModifierFlagCapsLock, NO) client:client];
    assert(session.capsLock);
    gCapsNow = 5050; gCapsHeld = false;
    [controller pollCapsLockTap];
    assert(!prefs.englishMode && gCapsReleases == 2);
    assert(![controller handleEvent:capsOff(5.5) client:client]);

    // 和别的修饰键一起按是组合键，不布防。
    gCapsNow = 6000; gCapsHeld = true;
    assert(![controller handleEvent:TapEvent(NSEventTypeFlagsChanged, 0x39, NSEventModifierFlagCapsLock | NSEventModifierFlagShift, 6.0) client:client]);
    assert(session.capsLock);
    gCapsHeld = false;
    [controller pollCapsLockTap];
    assert(!prefs.englishMode && gCapsReleases == 2);
    assert(![controller handleEvent:capsOff(6.5) client:client]);

    // 撤不回大写锁定时不切换，大写锁定照实报告。
    gCapsReleaseWorks = false;
    gCapsNow = 7000; gCapsHeld = true;
    assert(![controller handleEvent:capsOn(7.0) client:client]);
    gCapsNow = 7050; gCapsHeld = false;
    [controller pollCapsLockTap];
    assert(gCapsReleases == 3 && session.capsLock && !prefs.englishMode);
    gCapsReleaseWorks = true;
    assert(![controller handleEvent:capsOff(7.5) client:client]);

    // 组字中轻点：先上屏原文再切换，和单按 Shift 一样。
    session.rawTransition = @{@"handled":@YES, @"commit":@"msime",
        @"view":@{@"editing_text":@"", @"caret_position":@0, @"candidates":@[]}};
    [controller setValue:@{@"editing_text":@"msime", @"caret_position":@5, @"candidates":@[]} forKey:@"view"];
    const NSUInteger rawBefore = session.rawCommitCalls;
    gCapsNow = 8000; gCapsHeld = true;
    assert(![controller handleEvent:capsOn(8.0) client:client]);
    gCapsNow = 8080; gCapsHeld = false;
    [controller pollCapsLockTap];
    assert(session.rawCommitCalls == rawBefore + 1 && [client.committed isEqual:@"msime"] && prefs.englishMode);
    assert(!session.capsLock && gCapsReleases == 4);
    session.rawTransition = nil;
    [controller setValue:nil forKey:@"view"];
    assert(![controller handleEvent:capsOff(8.5) client:client]);

    // 开关关掉：大写锁定照常，立即报告。
    prefs.capsLockTapShortcut = NO;
    gCapsNow = 9000; gCapsHeld = true;
    assert(![controller handleEvent:capsOn(9.0) client:client]);
    assert(session.capsLock);
    gCapsHeld = false;
    [controller pollCapsLockTap];
    assert(prefs.englishMode && gCapsReleases == 4);

    // 真正推进运行循环：直接调用 pollCapsLockTap 看不出默认模式的定时器会在事件跟踪或模态面板期间停住。
    prefs.capsLockTapShortcut = YES;
    for (NSRunLoopMode mode in @[NSDefaultRunLoopMode, NSEventTrackingRunLoopMode, NSModalPanelRunLoopMode]) {
        CFRunLoopAddCommonMode(CFRunLoopGetMain(), (__bridge CFStringRef)mode);
        assert(![controller handleEvent:capsOff(10.0) client:client]);
        prefs.englishMode = NO;
        const NSUInteger releasesBefore = gCapsReleases;
        gCapsNow = 10000; gCapsHeld = true;
        assert(![controller handleEvent:capsOn(10.0) client:client]);
        gCapsNow = 10050; gCapsHeld = false;
        NSDate *deadline = [NSDate dateWithTimeIntervalSinceNow:1.0];
        while (gCapsReleases == releasesBefore && deadline.timeIntervalSinceNow > 0)
            [NSRunLoop.mainRunLoop runMode:mode beforeDate:deadline];
        if (gCapsReleases != releasesBefore + 1 || !prefs.englishMode || session.capsLock)
            fprintf(stderr, "Caps Lock tap mode=%s releases=%lu expected=%lu english=%d caps=%d timer_valid=%d timer_present=%d\n",
                mode.UTF8String, (unsigned long)gCapsReleases, (unsigned long)(releasesBefore + 1),
                (int)prefs.englishMode, (int)session.capsLock,
                (int)[[controller valueForKey:@"capsLockTapTimer"] isValid],
                (int)([controller valueForKey:@"capsLockTapTimer"] != nil));
        assert(gCapsReleases == releasesBefore + 1 && prefs.englishMode && !session.capsLock);

        // 同一运行模式下超过阈值仍保留大写锁定，不能为了让轻点响应而把长按也切换掉。
        assert(![controller handleEvent:capsOff(10.1) client:client]);
        gCapsNow = 11000; gCapsHeld = true;
        assert(![controller handleEvent:capsOn(11.0) client:client]);
        gCapsNow = 11301;
        deadline = [NSDate dateWithTimeIntervalSinceNow:1.0];
        while (!session.capsLock && deadline.timeIntervalSinceNow > 0)
            [NSRunLoop.mainRunLoop runMode:mode beforeDate:deadline];
        assert(session.capsLock && prefs.englishMode && gCapsReleases == releasesBefore + 1);
        gCapsHeld = false;
    }

    // 实体松手的 FlagsChanged 可能先于下一次轮询抵达；锁定位仍开时不能取消这次轻点，也不能重新计时。
    for (unsigned short code : {static_cast<unsigned short>(0x39), static_cast<unsigned short>(0xff)}) {
        assert(![controller handleEvent:capsOff(12.0) client:client]);
        prefs.englishMode = NO;
        const NSUInteger releasesBefore = gCapsReleases;
        gCapsNow = 12000; gCapsHeld = true;
        assert(![controller handleEvent:capsOn(12.0) client:client]);
        gCapsNow = 12050; gCapsHeld = false;
        assert(![controller handleEvent:TapEvent(NSEventTypeFlagsChanged, code, NSEventModifierFlagCapsLock, 12.05) client:client]);
        assert(gCapsReleases == releasesBefore + 1 && prefs.englishMode && !session.capsLock);
    }

    // 初始事件和 HID 状态都没有实体按住位时不布防，避免不支持该位的设备连长按也被切换。
    assert(![controller handleEvent:capsOff(13.0) client:client]);
    prefs.englishMode = NO;
    const NSUInteger releasesBefore = gCapsReleases;
    gCapsNow = 13000; gCapsHeld = false;
    assert(![controller handleEvent:capsOn(13.0) client:client]);
    gCapsNow = 13050;
    [controller pollCapsLockTap];
    assert(session.capsLock && !prefs.englishMode && gCapsReleases == releasesBefore);

    MSIMECapsLockRelease = realRelease;
    MSIMECapsLockHIDState = realHID;
    MSIMEPhysicalModifierFlagsState = realFlags;
    MSIMECapsLockNow = realNow;
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

// 不回读文档的客户端：Electron/Chromium 一类聊天应用常常对 attributedSubstringFromRange: 什么也不给，智能标点只能靠影子记下的前一个字符。
@interface UnreadableShortcutClient : ShortcutClient
@end
@implementation UnreadableShortcutClient
- (NSAttributedString *)attributedSubstringFromRange:(NSRange)range {
    (void)range;
    ++self.documentReads;
    return nil;
}
@end

// 用户在五笔下连着打 `0.10.1`，智能标点开着却得到 `0。10。1`。这里按用户的偏好建真会话，从 handleEvent: 一路打进去：控制器不收的键由应用自己写进文档，和真实宿主一样。全拼做对照。`typed` 只含数字和句号。
static NSString *TypeThroughRealSession(NSString *scheme, ShortcutClient *client, NSString *typed, BOOL chinesePunctuation) {
    NSString *root = [NSTemporaryDirectory() stringByAppendingPathComponent:NSUUID.UUID.UUIDString];
    NSDictionary *preferences = @{
        @"scheme": scheme, @"wubi_profile": @"wubi86", @"default_ime_mode": @"chinese", @"candidate_page_size": @6,
        @"learning": @NO, @"chinese_punctuation": @(chinesePunctuation), @"punctuation_lock": @"follow",
        @"wubi_mixed_pinyin": @YES, @"wubi_auto_commit_unique": @YES, @"wubi_code_hint": @YES,
        @"number_row_selection": @YES, @"smart_punctuation": @YES, @"smart_punctuation_direct_digit": @YES,
        @"smart_punctuation_direct_letter": @YES, @"smart_punctuation_repeat": @YES,
        @"smart_punctuation_space_convert": @NO, @"paired_punctuation": @YES, @"character_width": @"halfwidth"};
    NSMutableDictionary *options = [@{@"api_version": @1, @"preferences": preferences} mutableCopy];
    for (NSString *name in @[@"resources", @"user_data", @"cache", @"dictionaries"]) {
        NSString *path = [root stringByAppendingPathComponent:name];
        assert([NSFileManager.defaultManager createDirectoryAtPath:path withIntermediateDirectories:YES attributes:nil error:nil]);
        options[name] = path;
    }
    NSError *error = nil;
    MSIMEClientSession *session = [[MSIMEClientSession alloc] initWithOptions:options error:&error];
    assert(session && !error);
    NSDictionary *view = [session setFocused:YES error:&error][@"view"];
    assert(view && !error);

    NSString *suite = [@"msime.smart-punctuation-version." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    MSIMEAppearancePreferences *prefs =
        [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults skinsRoot:[NSURL fileURLWithPath:root]];
    [prefs applySharedInputPreferences:preferences];
    assert(prefs.smartPunctuation && prefs.smartPunctuationRepeatToChinese && prefs.runtimeChinesePunctuation == chinesePunctuation);
    client.document = @"";
    client.selection = NSMakeRange(0, 0);
    client.caret = NSMakeRect(100, 100, 1, 16);
    ModeController *controller = [ModeController alloc];
    [controller setValue:prefs forKey:@"appearance"];
    [controller setValue:session forKey:@"session"];
    [controller setValue:client forKey:@"activeClient"];
    [controller setValue:view forKey:@"view"];
    [controller setValue:[[HiddenCandidatePanel alloc] init] forKey:@"panel"];

    for (NSUInteger index = 0; index < typed.length; ++index) {
        NSString *character = [typed substringWithRange:NSMakeRange(index, 1)];
        static NSDictionary<NSString *, NSNumber *> *const codes = @{@"0": @29, @"1": @18, @".": @47};
        assert(codes[character]);
        // 控制器不收的键由应用照原样写进文档。
        if (![controller handleEvent:KeypadKey(codes[character].unsignedShortValue, character, 0, NO) client:client])
            [client insertText:character replacementRange:NSMakeRange(NSNotFound, NSNotFound)];
    }
    assert(client.marked.length == 0);
    assert([session closeWithError:&error] && !error);
    return client.document;
}

static void AssertTypedThroughRealSession(NSString *scheme, ShortcutClient *client, NSString *typed, BOOL chinesePunctuation, NSString *expected) {
    NSString *document = TypeThroughRealSession(scheme, client, typed, chinesePunctuation);
    if (![document isEqual:expected])
        fprintf(stderr, "smart punctuation: scheme=%s client=%s typed=%s document=%s expected=%s\n", scheme.UTF8String,
                NSStringFromClass(client.class).UTF8String, typed.UTF8String, document.UTF8String, expected.UTF8String);
    assert([document isEqual:expected]);
}

// 没有组字时智能标点判为 ASCII，Engine 把键交回宿主；宿主要自己写出字面标点，不能再把键交给后面的普通标点路线，否则 Engine 照常写成中文标点。读不回文档的客户端靠影子判断，结果一样。
static void TestSmartPunctuationVersionNumberThroughRealSession() {
    for (NSString *scheme in @[@"quanpin", @"shuangpin", @"wubi"]) {
        for (Class type in @[ShortcutClient.class, UnreadableShortcutClient.class]) {
            AssertTypedThroughRealSession(scheme, [type new], @"0.10.1", YES, @"0.10.1");
            // 前面没有数字或字母的句号仍是中文句号。
            AssertTypedThroughRealSession(scheme, [type new], @".", YES, @"。");
            // 中文标点关着时 Engine 同样把键交回，打出来的本来就是 ASCII，连按不武装改中文的手势。
            AssertTypedThroughRealSession(scheme, [type new], @"0..", NO, @"0..");
        }
        // 两秒内紧接着再按同一个键，刚写出的 ASCII 句号改成中文句号；中间打过别的键（上面的 `0.10.1`）就不算连按。
        AssertTypedThroughRealSession(scheme, [ShortcutClient new], @"0..", YES, @"0。");
    }

    // 诊断日志开着时每次判断记一行：前一个字符的来源和类别、判断结果，不记字符本身。
    NSString *root = [NSTemporaryDirectory() stringByAppendingPathComponent:NSUUID.UUID.UUIDString];
    assert([NSFileManager.defaultManager createDirectoryAtPath:root withIntermediateDirectories:YES attributes:nil error:nil]);
    msime_macos_diagnostic_configure(root.fileSystemRepresentation, true);
    AssertTypedThroughRealSession(@"wubi", [UnreadableShortcutClient new], @"0.10.", YES, @"0.10.");
    AssertTypedThroughRealSession(@"wubi", [ShortcutClient new], @".", YES, @"。");
    msime_macos_diagnostic_configure(root.fileSystemRepresentation, false);
    NSString *log = [NSString stringWithContentsOfFile:[root stringByAppendingPathComponent:@"diagnostic.log"]
                                              encoding:NSUTF8StringEncoding error:nil];
    NSArray<NSString *> *lines = [log componentsSeparatedByString:@"\n"];
    NSPredicate *smart = [NSPredicate predicateWithFormat:@"SELF CONTAINS '[smart-punctuation]'"];
    NSArray<NSString *> *decisions = [lines filteredArrayUsingPredicate:smart];
    assert(decisions.count == 3);
    assert([decisions[0] hasSuffix:@"[smart-punctuation] composing=0 shadow=digit preceding=shadow:digit decision=ascii_literal"]);
    assert([decisions[1] hasSuffix:@"[smart-punctuation] composing=0 shadow=digit preceding=shadow:digit decision=ascii_literal"]);
    assert([decisions[2] hasSuffix:@"[smart-punctuation] composing=0 shadow=invalid preceding=document:none decision=engine"]);
    [NSFileManager.defaultManager removeItemAtPath:root error:nil];
}

// 使用真实 Rust 会话走原生按键入口，覆盖减号翻页开关的两种状态。
static void TestTemporaryJapaneseLongVowelThroughRealSession(BOOL paging) {
    // 复用引擎 golden 的微型模型，让真实 Host API 保持临时日文可用，不依赖本机安装的词库。
    NSString *fixturePath = [[@(__FILE__).stringByDeletingLastPathComponent
        stringByAppendingPathComponent:@"../../../../crates/engine/tests/golden/scenarios/ri_japanese_model_a.json"] stringByStandardizingPath];
    NSData *fixtureData = [NSData dataWithContentsOfFile:fixturePath];
    assert(fixtureData);
    NSDictionary *fixture = [NSJSONSerialization JSONObjectWithData:fixtureData options:0 error:nil];
    NSData *model = [fixture[@"fixture"][@"files"][@"msime-japanese.dat"] dataUsingEncoding:NSUTF8StringEncoding];
    assert(model.length);
    for (NSString *scheme in @[@"quanpin", @"shuangpin"]) {
        NSString *root = [NSTemporaryDirectory() stringByAppendingPathComponent:NSUUID.UUID.UUIDString];
        NSDictionary *preferences = @{@"scheme":scheme, @"learning":@NO, @"cloud_candidates":@NO,
            @"candidate_page_size":@6, @"default_ime_mode":@"chinese", @"chinese_punctuation":@YES};
        NSMutableDictionary *options = [@{@"api_version":@1, @"preferences":preferences} mutableCopy];
        for (NSString *name in @[@"resources", @"user_data", @"cache", @"dictionaries"]) {
            NSString *path = [root stringByAppendingPathComponent:name];
            assert([NSFileManager.defaultManager createDirectoryAtPath:path withIntermediateDirectories:YES attributes:nil error:nil]);
            options[name] = path;
        }
        assert([model writeToFile:[options[@"resources"] stringByAppendingPathComponent:@"msime-japanese.dat"] atomically:YES]);
        NSError *error = nil;
        MSIMEClientSession *session = [[MSIMEClientSession alloc] initWithOptions:options error:&error];
        assert(session && !error);
        NSDictionary *view = [session setFocused:YES error:&error][@"view"];
        assert(view && !error);
        NSString *suite = [@"msime.temporary-japanese." stringByAppendingString:NSUUID.UUID.UUIDString];
        NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
        MSIMEAppearancePreferences *prefs = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults skinsRoot:[NSURL fileURLWithPath:root]];
        [prefs applySharedInputPreferences:preferences];
        [prefs applySharedCandidatePreferences:@{@"navigation":@{@"minus_equal":@(paging)},
            @"word_character":@{@"enabled":@NO, @"keys":@"minus_equal"}}];
        prefs.englishMode = NO;
        ShortcutClient *client = [ShortcutClient new];
        client.document = @"";
        client.selection = NSMakeRange(0, 0);
        client.caret = NSMakeRect(100, 100, 1, 16);
        ModeController *controller = [ModeController alloc];
        [controller setValue:prefs forKey:@"appearance"];
        [controller setValue:session forKey:@"session"];
        [controller setValue:client forKey:@"activeClient"];
        [controller setValue:view forKey:@"view"];
        [controller setValue:[[HiddenCandidatePanel alloc] init] forKey:@"panel"];
        for (NSArray *stroke in @[@[@"R", @15], @[@"k", @40], @[@"o", @31], @[@"-", @27], @[@"h", @4], @[@"i", @34], @[@"-", @27]]) {
            NSString *character = stroke[0];
            NSEventModifierFlags flags = [character isEqual:@"R"] ? NSEventModifierFlagShift : 0;
            assert([controller handleEvent:KeypadKey([stroke[1] unsignedShortValue], character, flags, NO) client:client]);
            assert(client.insertions.count == 0);
        }
        NSDictionary *composed = [controller valueForKey:@"view"];
        assert([composed[@"editing_text"] isEqual:@"Rko-hi-"]);
        assert([composed[@"local_mode"] isEqual:@"temporary_japanese"]);
        NSDictionary *katakana = nil;
        for (NSDictionary *row in composed[@"candidates"]) if ([row[@"text"] isEqual:@"コーヒー"]) katakana = row;
        assert(katakana);
        [controller apply:[session selectGeneration:[composed[@"generation"] unsignedLongLongValue]
            index:[katakana[@"id"][@"index"] unsignedIntegerValue] error:&error]];
        assert(!error && [client.committed isEqual:@"コーヒー"]);
        assert([session closeWithError:&error] && !error);
        MSIMERemoveTestPreferenceSuite(defaults, suite);
        assert([NSFileManager.defaultManager removeItemAtPath:root error:&error] && !error);
    }
}

// 用真实 NSEvent、控制器和 Rust 会话检查调频，客户端仅记录合成上屏。
static void TestInitialLearningThroughRealKeys(BOOL numeric, BOOL priorContext) {
    NSString *root = [NSTemporaryDirectory() stringByAppendingPathComponent:NSUUID.UUID.UUIDString];
    NSError *error = nil;
    NSDictionary *snapshot = [MSIMEClientSession loadPreferencesInDirectory:[root stringByAppendingPathComponent:@"preferences"] error:&error];
    assert(snapshot && !error);
    NSMutableDictionary *preferences = [snapshot[@"preferences"] mutableCopy];
    preferences[@"scheme"] = @"shuangpin";
    preferences[@"shuangpin_profile"] = @"xiaohe";
    preferences[@"default_ime_mode"] = @"chinese";
    preferences[@"learning"] = @YES;
    preferences[@"cloud_candidates"] = @NO;
    preferences[@"frequency"] = @{@"mode": @"promote", @"trigger_count": @1, @"linear_step": @1};
    NSMutableDictionary *options = [@{@"api_version": @1, @"preferences": preferences,
        @"quick_phrase_candidates": @YES, @"phrase_preedit": @YES} mutableCopy];
    for (NSString *name in @[@"resources", @"user_data", @"cache", @"dictionaries"]) {
        NSString *path = [root stringByAppendingPathComponent:name];
        assert([NSFileManager.defaultManager createDirectoryAtPath:path withIntermediateDirectories:YES attributes:nil error:nil]);
        options[name] = path;
    }
    for (NSString *name in @[@"resources", @"dictionaries"]) {
        sqlite3 *database = nullptr;
        NSString *path = [options[name] stringByAppendingPathComponent:@"msime-pinyin.db"];
        assert(sqlite3_open(path.fileSystemRepresentation, &database) == SQLITE_OK);
        assert(sqlite3_exec(database,
            "CREATE TABLE tbl_1_l(key TEXT,jp TEXT,value TEXT,weight INTEGER);"
            "INSERT INTO tbl_1_l VALUES('lai','l','甲',14837007),('li','l','丁',7329328),"
            "('lao','l','戊',3762961),('la','l','己',3655999),('le','l','乙',3481243),"
            "('liang','l','庚',3302466),('li','l','辛',3298612),('liao','l','乙',999);"
            "CREATE TABLE tbl_2_l(key TEXT,jp TEXT,value TEXT,weight INTEGER);"
            "CREATE TABLE tbl_1_w(key TEXT,jp TEXT,value TEXT,weight INTEGER);"
            "INSERT INTO tbl_1_w VALUES('wan','w','丙',100);"
            "CREATE TABLE quick_parases(key TEXT,value TEXT,weight INTEGER);", nullptr, nullptr, nullptr) == SQLITE_OK);
        assert(sqlite3_close(database) == SQLITE_OK);
    }
    auto query = [&](NSString *path, const char *sql) -> sqlite3_int64 {
        sqlite3 *database = nullptr;
        assert(sqlite3_open_v2(path.fileSystemRepresentation, &database, SQLITE_OPEN_READONLY, nullptr) == SQLITE_OK);
        sqlite3_stmt *statement = nullptr;
        assert(sqlite3_prepare_v2(database, sql, -1, &statement, nullptr) == SQLITE_OK);
        assert(sqlite3_step(statement) == SQLITE_ROW);
        sqlite3_int64 result = sqlite3_column_int64(statement, 0);
        assert(sqlite3_finalize(statement) == SQLITE_OK);
        assert(sqlite3_close(database) == SQLITE_OK);
        return result;
    };
    NSString *working = [options[@"dictionaries"] stringByAppendingPathComponent:@"msime-pinyin.db"];
    NSString *journal = [options[@"user_data"] stringByAppendingPathComponent:@"msime_user.db"];
    if (priorContext) {
        // 首次创建真实会话前预置合成历史，检验旧上下文不能推翻全局调频首选。
        sqlite3 *database = nullptr;
        assert(sqlite3_open(journal.fileSystemRepresentation, &database) == SQLITE_OK);
        assert(sqlite3_exec(database,
            "CREATE TABLE personal_bigram(previous TEXT NOT NULL,word TEXT NOT NULL,count INTEGER NOT NULL CHECK(count>0),PRIMARY KEY(previous,word)) WITHOUT ROWID;"
            "CREATE TABLE personal_trigram(earlier TEXT NOT NULL,previous TEXT NOT NULL,word TEXT NOT NULL,count INTEGER NOT NULL CHECK(count>0),PRIMARY KEY(earlier,previous,word)) WITHOUT ROWID;"
            "INSERT INTO personal_bigram VALUES('丙','甲',10);", nullptr, nullptr, nullptr) == SQLITE_OK);
        assert(sqlite3_close(database) == SQLITE_OK);
    }
    const char *weightSQL = "SELECT weight FROM tbl_1_l WHERE key='le' AND value='乙'";
    MSIMEClientSession *session = [[MSIMEClientSession alloc] initWithOptions:options error:&error];
    assert(session && !error);
    NSDictionary *view = [session setFocused:YES error:&error][@"view"];
    assert(view && !error);
    NSString *suite = [@"msime.initial-learning." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    MSIMEAppearancePreferences *prefs = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults skinsRoot:[NSURL fileURLWithPath:root]];
    [prefs applySharedInputPreferences:preferences];
    prefs.englishMode = NO;
    prefs.expandCandidates = NO;
    ShortcutClient *client = [ShortcutClient new];
    client.document = @"";
    client.selection = NSMakeRange(0, 0);
    client.caret = NSMakeRect(100, 100, 1, 16);
    ModeController *controller = [ModeController alloc];
    [controller setValue:prefs forKey:@"appearance"];
    [controller setValue:session forKey:@"session"];
    [controller setValue:client forKey:@"activeClient"];
    [controller setValue:view forKey:@"view"];
    [controller setValue:[[HiddenCandidatePanel alloc] init] forKey:@"panel"];
    BOOL displayedBeforeGlobal = NO;
    for (NSUInteger round = 0; round < 5; ++round) {
        assert([controller handleEvent:KeypadKey(37, @"l", 0, NO) client:client]);
        NSArray *rows = [controller valueForKey:@"view"][@"candidates"];
        NSUInteger index = [rows indexOfObjectPassingTest:^BOOL(NSDictionary *row, NSUInteger offset, BOOL *stop) {
            (void)offset; (void)stop;
            return [row[@"text"] isEqual:@"乙"];
        }];
        assert(index != NSNotFound && index < 6);
        sqlite3_int64 before = query(working, weightSQL);
        if (index == 0 && before < 14837007) displayedBeforeGlobal = YES;
        if (numeric) {
            static const unsigned short codes[] = {18, 19, 20, 21, 23, 22};
            NSString *digit = [NSString stringWithFormat:@"%lu", (unsigned long)index + 1];
            assert([controller handleEvent:KeypadKey(codes[index], digit, 0, NO) client:client]);
        } else {
            // 双拼的左右键编辑音节，使用向下键逐项选择候选。
            for (NSUInteger move = 0; move < index; ++move)
                assert([controller handleEvent:ModeKey(125, 0, NO) client:client]);
            NSArray *highlighted = [controller valueForKey:@"view"][@"candidates"];
            assert([highlighted[index][@"highlighted"] boolValue]);
            assert([controller handleEvent:ModeKey(49, 0, NO) client:client]);
        }
        assert([client.committed isEqual:@"乙"] && client.insertions.count == round + 1);
        sqlite3_int64 after = query(working, weightSQL);
        if (before < 14837007) assert(after > before);
        assert(query(journal, "SELECT weight FROM user_dictionary_operations WHERE dictionary='pinyin' AND key='le' AND value='乙' AND operation='upsert'") == after);
    }
    assert(displayedBeforeGlobal && query(working, weightSQL) > 14837007);
    assert([controller handleEvent:KeypadKey(13, @"w", 0, NO) client:client]);
    assert([controller handleEvent:KeypadKey(38, @"j", 0, NO) client:client]);
    assert([controller handleEvent:ModeKey(49, 0, NO) client:client]);
    assert([client.committed isEqual:@"丙"]);
    assert([controller handleEvent:KeypadKey(37, @"l", 0, NO) client:client]);
    assert([[controller valueForKey:@"view"][@"candidates"][0][@"text"] isEqual:@"乙"]);
    assert([session closeWithError:&error] && !error);
    MSIMEClientSession *reopened = [[MSIMEClientSession alloc] initWithOptions:options error:&error];
    assert(reopened && !error);
    assert([reopened setFocused:YES error:&error] && !error);
    NSDictionary *result = [reopened typeASCII:'l' shift:NO error:&error];
    assert(result && !error && [result[@"view"][@"candidates"][0][@"text"] isEqual:@"乙"]);
    assert([reopened closeWithError:&error] && !error);
    assert(query([options[@"resources"] stringByAppendingPathComponent:@"msime-pinyin.db"], weightSQL) == 3481243);
    MSIMERemoveTestPreferenceSuite(defaults, suite);
    assert([NSFileManager.defaultManager removeItemAtPath:root error:&error] && !error);
}

// 只注入一次命令失败，其余按键、快照、偏好保存和方案切换均走真实引擎。
@interface CycleFailureSession : MSIMEClientSession
@property(nonatomic) BOOL failRaw;
@end
@implementation CycleFailureSession
- (NSDictionary *)command:(uint32_t)command error:(NSError **)error {
    if (self.failRaw && command == MSIME_COMMIT_RAW) return nil;
    return [super command:command error:error];
}
@end

static void TestRealTrilingualCycle() {
    NSString *root = [NSTemporaryDirectory() stringByAppendingPathComponent:NSUUID.UUID.UUIDString];
    NSString *directory = [root stringByAppendingPathComponent:@"preferences"];
    NSError *error = nil;
    NSDictionary *snapshot = [MSIMEClientSession loadPreferencesInDirectory:directory error:&error];
    assert(snapshot && !error);
    NSMutableDictionary *preferences = [snapshot[@"preferences"] mutableCopy];
    preferences[@"scheme"] = @"japanese";
    preferences[@"last_chinese_scheme"] = NSNull.null;
    preferences[@"learning"] = @NO;
    preferences[@"candidate_translations"] = @NO;
    preferences[@"cloud_candidates"] = @NO;
    NSMutableDictionary *keys = [preferences[@"keybindings"] mutableCopy];
    keys[@"trilingual_cycle"] = @YES;
    preferences[@"keybindings"] = keys;
    NSDictionary *saved = [MSIMEClientSession savePreferencesInDirectory:directory
        expectedRevision:[snapshot[@"revision"] unsignedLongLongValue]
        snapshot:@{@"format_version":@1, @"revision":snapshot[@"revision"], @"preferences":preferences} error:&error];
    assert(saved && !error);
    NSMutableDictionary *options = [@{@"api_version":@1, @"preferences_directory":directory} mutableCopy];
    for (NSString *name in @[@"resources", @"user_data", @"cache", @"dictionaries"]) {
        NSString *path = [root stringByAppendingPathComponent:name];
        assert([NSFileManager.defaultManager createDirectoryAtPath:path withIntermediateDirectories:YES attributes:nil error:nil]);
        options[name] = path;
    }
    CycleFailureSession *session = [[CycleFailureSession alloc] initWithOptions:options error:&error];
    assert(session && !error);
    NSString *suite = [@"msime.real-cycle." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    MSIMEAppearancePreferences *prefs = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults skinsRoot:[NSURL fileURLWithPath:root]];
    [prefs applySharedInputPreferences:saved[@"preferences"]];
    prefs.inputModeHUD = NO;
    prefs.controlOptionSpaceShortcut = YES;
    prefs.englishMode = YES;
    assert([prefs.lastChineseScheme isEqual:@"quanpin"]);
    ShortcutClient *client = [ShortcutClient new];
    client.document = @"";
    client.caret = NSMakeRect(100, 100, 1, 16);
    ModeController *controller = [ModeController alloc];
    [controller setValue:prefs forKey:@"appearance"];
    [controller setValue:session forKey:@"session"];
    [controller setValue:client forKey:@"activeClient"];
    [controller setValue:directory forKey:@"preferencesDirectory"];
    [controller setValue:[[HiddenCandidatePanel alloc] init] forKey:@"panel"];
    [controller setValue:[session setFocused:YES error:&error][@"view"] forKey:@"view"];
    assert(!error);
    auto scheme = [&]() { return ((NSDictionary *)[controller valueForKey:@"view"])[@"scheme"]; };
    auto toggle = [&]() {
        assert([controller handleEvent:ModeKey(49, NSEventModifierFlagControl | NSEventModifierFlagOption, NO) client:client]);
    };
    auto compose = [&]() {
        assert([controller handleEvent:KeypadKey(32, @"U", NSEventModifierFlagShift, NO) client:client]);
        for (NSArray *stroke in @[@[@21, @"4"], @[@14, @"e"], @[@19, @"2"], @[@2, @"d"]])
            assert([controller handleEvent:KeypadKey([stroke[0] unsignedShortValue], stroke[1], 0, NO) client:client]);
        assert([client.marked isEqual:@"U4e2d"]);
    };

    // 从未选过中文方案的日语用户，也能从英文回到真实的中文方案。
    toggle();
    assert(!prefs.englishMode && [prefs.inputScheme isEqual:@"quanpin"] && [scheme() isEqual:@0]);
    compose();
    session.failRaw = YES;
    toggle();
    assert(!prefs.englishMode && [scheme() isEqual:@0] && [client.marked isEqual:@"U4e2d"] && client.insertions.count == 0);
    session.failRaw = NO;
    toggle();
    assert(!prefs.englishMode && [scheme() isEqual:@3]);
    assert(client.marked.length == 0 && client.insertions.count == 1 && [client.insertions[0] isEqual:@"U4e2d"]);
    toggle();
    assert(prefs.englishMode && [scheme() isEqual:@3]);
    toggle();
    assert(!prefs.englishMode && [scheme() isEqual:@0]);

    // 关闭循环后，组合键仍取消组字；轻点修饰键仍提交原文。
    prefs.trilingualCycle = NO;
    compose();
    toggle();
    assert(prefs.englishMode && client.marked.length == 0 && client.insertions.count == 1);
    toggle();
    compose();
    [controller toggleInputModeFromModifierTap];
    assert(prefs.englishMode && client.marked.length == 0 && client.insertions.count == 2);
    assert([client.insertions[1] isEqual:@"U4e2d"]);
    assert([session closeWithError:&error] && !error);
    [defaults removePersistentDomainForName:suite];
    [NSFileManager.defaultManager removeItemAtPath:root error:nil];
}

// 按键从 handleEvent: 进入真实引擎，再把组字和提交写回合成编辑器，覆盖桩会话测不到的接缝。
// 尚未覆盖的只有系统 IMK 投递入口，那需要选中已安装的输入源。
static void TestRealSessionComposition() {
    NSString *root = [NSTemporaryDirectory() stringByAppendingPathComponent:NSUUID.UUID.UUIDString];
    NSMutableDictionary *options = [@{@"api_version": @1,
        @"preferences": @{@"scheme": @"quanpin", @"default_ime_mode": @"chinese", @"candidate_page_size": @5,
                          @"learning": @NO, @"chinese_punctuation": @YES, @"smart_punctuation": @NO}} mutableCopy];
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
    // 此处验证普通中文标点开关，显式关闭智能标点，避免上一段原文末尾的字母改变逗号路线。
    [prefs applySharedInputPreferences:options[@"preferences"]];
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

    // Shift+首字母也属于拼音组字：真实宿主不能先把 `O` 交给应用，再把剩下的字母送进引擎。
    [client.insertions removeAllObjects];
    client.marked = nil;
    assert([controller handleEvent:KeypadKey(31, @"O", NSEventModifierFlagShift, NO) client:client]);
    assert([client.marked isEqual:@"O"] && client.insertions.count == 0);
    for (NSArray *stroke in @[@[@11, @"b"], @[@1, @"s"], @[@14, @"e"], @[@15, @"r"], @[@9, @"v"], @[@0, @"a"], @[@11, @"b"], @[@37, @"l"], @[@14, @"e"]]) {
        assert([controller handleEvent:KeypadKey([stroke[0] unsignedShortValue], stroke[1], 0, NO) client:client]);
    }
    assert([client.marked isEqual:@"Observable"] && client.insertions.count == 0);
    assert([controller handleEvent:enter client:client]);
    assert(client.insertions.count == 1 && [client.insertions[0] isEqual:@"Observable"] && client.marked.length == 0);

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

    // Korean composes Dubeolsik syllables in the marked text through the real Engine. Each key is the ASCII letter the user typed, cased by Shift alone.
    NSMutableDictionary *koreanOptions = [options mutableCopy];
    NSMutableDictionary *koreanPreferences = [options[@"preferences"] mutableCopy];
    koreanPreferences[@"scheme"] = @"korean";
    koreanOptions[@"preferences"] = koreanPreferences;
    MSIMEClientSession *koreanSession = [[MSIMEClientSession alloc] initWithOptions:koreanOptions error:&error];
    assert(koreanSession && !error);
    // Focus answers with a transition; the controller keeps the view inside it, which is where the scheme it reads lives.
    NSDictionary *koreanView = [koreanSession setFocused:YES error:&error][@"view"];
    assert(koreanView && !error && [koreanView[@"scheme"] isEqual:@4]);
    [controller setValue:koreanSession forKey:@"session"];
    [controller setValue:koreanView forKey:@"view"];
    // The syllable is drawn inline even when the preedit display preference hides the composition: there is no candidate window to show it instead.
    [prefs applySharedInputPreferences:@{@"tsf_preedit_style": @"empty"}];
    [client.insertions removeAllObjects];
    client.document = @"";
    client.selection = NSMakeRange(0, 0);
    NSEvent *(^letter)(NSString *, unsigned short, NSEventModifierFlags) = ^NSEvent *(NSString *typed, unsigned short code, NSEventModifierFlags flags) {
        return [NSEvent keyEventWithType:NSEventTypeKeyDown location:NSZeroPoint modifierFlags:flags timestamp:0 windowNumber:0
                                 context:nil characters:typed charactersIgnoringModifiers:typed.lowercaseString isARepeat:NO keyCode:code];
    };
    for (NSArray *stroke in @[@[@"d", @2], @[@"k", @40], @[@"s", @1]])
        assert([controller handleEvent:letter(stroke[0], [stroke[1] unsignedShortValue], 0) client:client]);
    assert([client.marked isEqual:@"안"] && client.insertions.count == 0);
    // The fourth key starts a new syllable, so the finished one is written out while the next composes.
    assert([controller handleEvent:letter(@"s", 1, 0) client:client]);
    assert(client.insertions.count == 1 && [client.insertions[0] isEqual:@"안"] && [client.marked isEqual:@"ㄴ"]);
    assert([controller handleEvent:letter(@"u", 32, 0) client:client]);
    assert([controller handleEvent:letter(@"d", 2, 0) client:client]);
    assert([client.marked isEqual:@"녕"]);
    // Caps Lock types the unshifted jamo, ㄱ rather than ㄲ, and does not hand the letter to the application.
    assert([controller handleEvent:letter(@"R", 15, NSEventModifierFlagCapsLock) client:client]);
    assert(client.insertions.count == 2 && [client.insertions[1] isEqual:@"녕"] && [client.marked isEqual:@"ㄱ"]);
    // Space writes the syllable out and is still the application's to insert.
    assert(![controller handleEvent:ModeKey(49, 0, NO) client:client]);
    assert(client.insertions.count == 3 && [client.insertions[2] isEqual:@"ㄱ"] && client.marked.length == 0);
    // Shift+R is ㄲ, not a temporary Japanese entry, and punctuation follows the syllable in one half-width commit.
    assert([controller handleEvent:letter(@"R", 15, NSEventModifierFlagShift) client:client]);
    assert([controller handleEvent:letter(@"k", 40, 0) client:client]);
    assert([client.marked isEqual:@"까"]);
    assert([controller handleEvent:comma client:client]);
    assert(client.insertions.count == 4 && [client.insertions[3] isEqual:@"까,"] && client.marked.length == 0);
    // With nothing composing the mark is the application's, ASCII whatever the Chinese punctuation switch says.
    assert(![controller handleEvent:comma client:client]);
    assert(client.insertions.count == 4);
    // Backspace takes one jamo, Escape drops the syllable, and an idle Backspace belongs to the application.
    assert([controller handleEvent:letter(@"r", 15, 0) client:client]);
    assert([controller handleEvent:letter(@"k", 40, 0) client:client]);
    NSEvent *backspace = [NSEvent keyEventWithType:NSEventTypeKeyDown location:NSZeroPoint modifierFlags:0 timestamp:0 windowNumber:0
                                           context:nil characters:@"\b" charactersIgnoringModifiers:@"\b" isARepeat:NO keyCode:51];
    assert([controller handleEvent:backspace client:client]);
    assert([client.marked isEqual:@"ㄱ"]);
    NSEvent *escape = [NSEvent keyEventWithType:NSEventTypeKeyDown location:NSZeroPoint modifierFlags:0 timestamp:0 windowNumber:0
                                        context:nil characters:@"\e" charactersIgnoringModifiers:@"\e" isARepeat:NO keyCode:53];
    assert([controller handleEvent:escape client:client]);
    assert(client.marked.length == 0 && client.insertions.count == 4);
    assert(![controller handleEvent:backspace client:client]);
    // Enter writes the syllable out and still reaches the application as a newline.
    assert([controller handleEvent:letter(@"r", 15, 0) client:client]);
    assert([controller handleEvent:letter(@"k", 40, 0) client:client]);
    assert(![controller handleEvent:enter client:client]);
    assert(client.insertions.count == 5 && [client.insertions[4] isEqual:@"가"] && client.marked.length == 0);
    // Leaving the client commits the open syllable to it instead of dropping it.
    assert([controller handleEvent:letter(@"r", 15, 0) client:client]);
    [controller apply:[koreanSession setFocused:NO error:&error]];
    assert(!error && client.insertions.count == 6 && [client.insertions[5] isEqual:@"ㄱ"] && client.marked.length == 0);
    [controller apply:[koreanSession setFocused:YES error:&error]];
    assert(!error);

    // Option+Return converts the composing syllable to Hanja: the list opens in the candidate window, the syllable stays marked, and nothing reaches the document.
    [client.insertions removeAllObjects];
    NSEvent *(^chord)(NSString *, unsigned short, NSEventModifierFlags) = ^NSEvent *(NSString *typed, unsigned short code, NSEventModifierFlags flags) {
        return [NSEvent keyEventWithType:NSEventTypeKeyDown location:NSZeroPoint modifierFlags:flags timestamp:0 windowNumber:0
                                 context:nil characters:typed charactersIgnoringModifiers:typed isARepeat:NO keyCode:code];
    };
    NSEvent *hanja = chord(@"\r", 36, NSEventModifierFlagOption);
    NSEvent *keypadHanja = chord(@"\x03", 76, NSEventModifierFlagOption | NSEventModifierFlagNumericPad);
    void (^typeHan)(void) = ^{
        for (NSArray *stroke in @[@[@"g", @5], @[@"k", @40], @[@"s", @1]])
            assert([controller handleEvent:letter(stroke[0], [stroke[1] unsignedShortValue], 0) client:client]);
        assert([client.marked isEqual:@"한"]);
    };
    NSDictionary *(^currentView)(void) = ^NSDictionary *{ return [controller valueForKey:@"view"]; };
    typeHan();
    assert(!panel.isVisible && !MSIMECandidateListOpen(currentView()));
    assert([controller handleEvent:hanja client:client]);
    assert(MSIMECandidateListOpen(currentView()) && panel.isVisible);
    assert([currentView()[@"candidates"][0][@"text"] isEqual:@"韓"] && [currentView()[@"candidates"][1][@"text"] isEqual:@"漢"]);
    assert([client.marked isEqual:@"한"] && client.insertions.count == 0);
    // Hanja rows take no pin, fixed position or removal, so a right click offers no menu for them.
    assert([controller menuForCandidate:currentView()[@"candidates"][0]] == nil);
    // The row draws the Hanja alone on its line and the 훈음 the Engine annotated it with on the small gloss line under it, never as the row's translation.
    NSString *hanReading = currentView()[@"candidates"][0][@"annotation"];
    assert([hanReading isKindOfClass:NSString.class] && hanReading.length);
    MSIMECandidateButton *hanRow = nil;
    for (NSView *view in MSIMECandidateControlsView(panel.contentView).subviews)
        if ([view isKindOfClass:MSIMECandidateButton.class] && view.tag == 0) hanRow = (MSIMECandidateButton *)view;
    assert(hanRow && [hanRow.title isEqual:@"1  韓"] && hanRow.annotation.length == 0 && hanRow.itemLayout.annotation.width == 0);
    assert([hanRow.glossReading isEqual:hanReading] && hanRow.translation.length == 0 && hanRow.itemLayout.translation.width > 0);
    assert(![hanRow.title containsString:hanReading] && [hanRow.toolTip containsString:hanReading]);
    // Return chooses the highlighted Hanja instead of writing the syllable out and breaking the line.
    assert([controller handleEvent:enter client:client]);
    assert(client.insertions.count == 1 && [client.insertions[0] isEqual:@"韓"] && client.marked.length == 0 && !panel.isVisible);

    // Right moves the highlight in the list, and a digit chooses from the page.
    const BOOL savedVertical = prefs.vertical;
    prefs.vertical = NO;
    typeHan();
    assert([controller handleEvent:keypadHanja client:client]);
    assert(MSIMECandidateListOpen(currentView()));
    assert([controller handleEvent:KeypadKey(124, @"\uF703", NSEventModifierFlagFunction | NSEventModifierFlagNumericPad, NO) client:client]);
    assert([currentView()[@"candidates"][1][@"highlighted"] isEqual:@YES] && [client.marked isEqual:@"한"]);
    assert([controller handleEvent:enter client:client]);
    assert(client.insertions.count == 2 && [client.insertions[1] isEqual:@"漢"]);
    typeHan();
    assert([controller handleEvent:hanja client:client]);
    assert([controller handleEvent:KeypadKey(19, @"2", 0, NO) client:client]);
    assert(client.insertions.count == 3 && [client.insertions[2] isEqual:@"漢"] && client.marked.length == 0);

    // 竖排列表也一样：汉字列表里没有光标，与 Windows 的 korean_hanja_key 相同，←↑ 移到上一个、→↓ 移到下一个，不翻页，音节保留。
    prefs.vertical = YES;
    typeHan();
    assert([controller handleEvent:hanja client:client]);
    assert([currentView()[@"page"] isEqual:@0]);
    assert([controller handleEvent:KeypadKey(124, @"\uF703", NSEventModifierFlagFunction | NSEventModifierFlagNumericPad, NO) client:client]);
    assert([currentView()[@"page"] isEqual:@0] && [currentView()[@"candidates"][1][@"highlighted"] isEqual:@YES] &&
           [client.marked isEqual:@"한"] && client.insertions.count == 3);
    assert([controller handleEvent:KeypadKey(123, @"\uF702", NSEventModifierFlagFunction | NSEventModifierFlagNumericPad, NO) client:client]);
    assert([currentView()[@"candidates"][0][@"highlighted"] isEqual:@YES] && MSIMECandidateListOpen(currentView()));
    assert([controller handleEvent:KeypadKey(125, @"\uF701", NSEventModifierFlagFunction | NSEventModifierFlagNumericPad, NO) client:client]);
    assert([currentView()[@"candidates"][1][@"highlighted"] isEqual:@YES]);
    prefs.vertical = savedVertical;

    // Escape closes the list and keeps the syllable composing; the trigger reopens it, and a second press closes it again.
    assert([controller handleEvent:escape client:client]);
    assert(!MSIMECandidateListOpen(currentView()) && !panel.isVisible && [client.marked isEqual:@"한"] && client.insertions.count == 3);
    assert([controller handleEvent:hanja client:client]);
    assert(MSIMECandidateListOpen(currentView()));
    assert([controller handleEvent:hanja client:client]);
    assert(!MSIMECandidateListOpen(currentView()) && [client.marked isEqual:@"한"] && client.insertions.count == 3);

    // The paging marks and the word-to-character pair are punctuation while the list is open: the Hangul is written with the mark, never a page turned or a Hanja picked.
    assert([prefs navigationEnabled:@"comma_period"] && [prefs navigationEnabled:@"minus_equal"]);
    [prefs setWordCharacterEnabled:YES keys:@"brackets"];
    assert([controller handleEvent:hanja client:client]);
    assert([controller handleEvent:KeypadKey(47, @".", 0, NO) client:client]);
    assert(client.insertions.count == 4 && [client.insertions[3] isEqual:@"한."] && client.marked.length == 0);
    typeHan();
    assert([controller handleEvent:hanja client:client]);
    assert([controller handleEvent:KeypadKey(27, @"-", 0, NO) client:client]);
    assert(client.insertions.count == 5 && [client.insertions[4] isEqual:@"한-"]);
    typeHan();
    assert([controller handleEvent:hanja client:client]);
    assert([controller handleEvent:KeypadKey(33, @"[", 0, NO) client:client]);
    assert(client.insertions.count == 6 && [client.insertions[5] isEqual:@"한["] && !panel.isVisible);
    [prefs setWordCharacterEnabled:NO keys:@"brackets"];

    // A lone jamo has no Hanja. The chord is still swallowed while it composes, so the jamo is not written out with a line break after it; with nothing composing the chord is the application's.
    assert([controller handleEvent:letter(@"r", 15, 0) client:client]);
    assert([controller handleEvent:hanja client:client]);
    assert([client.marked isEqual:@"ㄱ"] && client.insertions.count == 6 && !MSIMECandidateListOpen(currentView()));
    assert([controller handleEvent:escape client:client]);
    assert(client.marked.length == 0);
    assert(![controller handleEvent:hanja client:client]);
    assert(client.insertions.count == 6);
    // The 훈음 is shown, not committable: with translations on, Ctrl+Enter and Option/Ctrl+digit write only a real translation, and a Hanja row has none here, so neither the 훈음 nor anything glued to it reaches the document.
    const BOOL savedTranslations = prefs.candidateTranslations;
    prefs.candidateTranslations = YES;
    typeHan();
    assert([controller handleEvent:hanja client:client]);
    assert(MSIMEKoreanHanjaListOpen(currentView()));
    assert([controller sensesForHighlightedCandidate].count == 0);
    assert(![controller commitHighlightedGlossColumn:1 client:client] && ![controller commitHighlightedGlossColumn:2 client:client]);
    assert(client.insertions.count == 6 && MSIMEKoreanHanjaListOpen(currentView()));
    for (NSEvent *glossChord in @[chord(@"1", 18, NSEventModifierFlagOption), chord(@"1", 18, NSEventModifierFlagControl),
                                  chord(@"\r", 36, NSEventModifierFlagControl)]) {
        if (!MSIMEKoreanHanjaListOpen(currentView())) {
            if (!client.marked.length) typeHan();
            assert([controller handleEvent:hanja client:client]);
        }
        [controller handleEvent:glossChord client:client];
        for (NSString *inserted in client.insertions) assert(![inserted containsString:hanReading]);
    }
    if (MSIMEKoreanHanjaListOpen(currentView())) assert([controller handleEvent:escape client:client]);
    if (client.marked.length) assert([controller handleEvent:escape client:client]);
    prefs.candidateTranslations = savedTranslations;
    [prefs applySharedInputPreferences:@{@"tsf_preedit_style": @"raw"}];
    assert([koreanSession closeWithError:&error] && !error);

    // 藏文经真实引擎把威利转写（EWTS）组成藏文：组字区显示转换后的藏文，空格上屏时加音节点 ་，`/` 上屏时加垂符 །，回车只上屏藏文，三者都不再交给应用。
    NSMutableDictionary *tibetanOptions = [options mutableCopy];
    NSMutableDictionary *tibetanPreferences = [options[@"preferences"] mutableCopy];
    tibetanPreferences[@"scheme"] = @"tibetan";
    tibetanOptions[@"preferences"] = tibetanPreferences;
    MSIMEClientSession *tibetanSession = [[MSIMEClientSession alloc] initWithOptions:tibetanOptions error:&error];
    assert(tibetanSession && !error);
    NSDictionary *tibetanView = [tibetanSession setFocused:YES error:&error][@"view"];
    assert(tibetanView && !error && [tibetanView[@"scheme"] isEqual:@8]);
    [controller setValue:tibetanSession forKey:@"session"];
    [controller setValue:tibetanView forKey:@"view"];
    // 与韩文一样没有候选窗，组字总是画在行内，即使预编辑显示偏好把它藏起来。
    [prefs applySharedInputPreferences:@{@"tsf_preedit_style": @"empty"}];
    [client.insertions removeAllObjects];
    client.document = @"";
    client.selection = NSMakeRange(0, 0);
    NSEvent *tibetanSpace = chord(@" ", 49, 0);
    NSEvent *tibetanSlash = chord(@"/", 44, 0);
    void (^typeWylie)(NSArray *) = ^(NSArray *strokes) {
        for (NSArray *stroke in strokes)
            assert([controller handleEvent:chord(stroke[0], [stroke[1] unsignedShortValue], [stroke[2] unsignedIntegerValue]) client:client]);
    };
    typeWylie(@[@[@"b", @11, @0], @[@"k", @40, @0], @[@"r", @15, @0], @[@"a", @0, @0]]);
    assert([client.marked isEqual:@"བཀྲ"] && client.insertions.count == 0 && !panel.isVisible);
    assert([controller handleEvent:tibetanSpace client:client]);
    assert(client.insertions.count == 1 && [client.insertions[0] isEqual:@"བཀྲ་"] && client.marked.length == 0);
    typeWylie(@[@[@"s", @1, @0], @[@"h", @4, @0], @[@"i", @34, @0], @[@"s", @1, @0]]);
    assert([client.marked isEqual:@"ཤིས"]);
    assert([controller handleEvent:tibetanSlash client:client]);
    assert(client.insertions.count == 2 && [client.insertions[1] isEqual:@"ཤིས།"] && client.marked.length == 0);
    // 没有组字时 `/` 单独写出垂符，空格仍是应用的。
    assert([controller handleEvent:tibetanSlash client:client]);
    assert(client.insertions.count == 3 && [client.insertions[2] isEqual:@"།"]);
    assert(![controller handleEvent:tibetanSpace client:client]);
    assert(client.insertions.count == 3);
    // 大写字母是另一个威利字母：Shift+T 是反写的 ཊ，不是临时英文或日文。叠加用的 `+` 也作为拼写送进引擎。
    typeWylie(@[@[@"T", @17, @(NSEventModifierFlagShift)], @[@"a", @0, @0]]);
    assert([client.marked isEqual:@"ཊ"]);
    assert([controller handleEvent:enter client:client]);
    assert(client.insertions.count == 4 && [client.insertions[3] isEqual:@"ཊ"] && client.marked.length == 0);
    typeWylie(@[@[@"p", @35, @0], @[@"a", @0, @0], @[@"d", @2, @0], @[@"+", @24, @(NSEventModifierFlagShift)], @[@"m", @46, @0], @[@"a", @0, @0]]);
    assert([client.marked isEqual:@"པདྨ"] && client.insertions.count == 4);
    // 退格删一个威利按键；第一次 Esc 把组字退回威利原文，第二次丢弃，什么都不上屏。
    assert([controller handleEvent:backspace client:client]);
    assert([client.marked isEqual:@"པདྨ"] && client.insertions.count == 4);
    assert([controller handleEvent:escape client:client]);
    assert([client.marked isEqual:@"pad+m"] && client.insertions.count == 4);
    assert([controller handleEvent:escape client:client]);
    assert(client.marked.length == 0 && client.insertions.count == 4);
    // 离开客户端时把组字上屏，而不是丢掉。
    typeWylie(@[@[@"k", @40, @0], @[@"a", @0, @0]]);
    [controller apply:[tibetanSession setFocused:NO error:&error]];
    assert(!error && client.insertions.count == 5 && [client.insertions[4] isEqual:@"ཀ"] && client.marked.length == 0);
    [prefs applySharedInputPreferences:@{@"tsf_preedit_style": @"raw"}];
    assert([tibetanSession closeWithError:&error] && !error);

    MSIMERemoveTestPreferenceSuite(defaults, suite);
    assert([NSFileManager.defaultManager removeItemAtPath:root error:nil]);
}

// 应用例外迁到共享文档：升级前的本地规则在文档还没有这个键时照样生效并被发布；文档带着规则载入后成为权威来源、本地旧键删除；文档清空规则后旧规则不会回来；本窗口写的规则经合并整张写进文档。
static void TestApplicationInputModeRulesFollowSharedDocument() {
    NSString *suite = [@"msime.app-rules." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    [defaults setObject:@{@"org.example.legacy": @"english", @"org.example.bad": @"global"} forKey:@"MSIMEClientAppInputModeRules"];
    MSIMEAppearancePreferences *prefs = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    assert(!prefs.applicationInputModeRulesAwaitPublication);
    [prefs applySharedInputPreferences:@{@"default_ime_mode": @"chinese"}];
    [prefs activateInputModeForApplication:@"org.example.legacy"];
    assert(prefs.englishMode);
    // 文档还没有规则表、本机有升级前的规则：输入法载入后要立即保存一次把它们发布出去，共享设置页才看得到。
    assert(prefs.applicationInputModeRulesAwaitPublication);
    assert([[prefs sharedPreferencesByMerging:@{}][@"app_input_mode_rules"] isEqual:(@{@"org.example.legacy": @"english"})]);
    // 升级前的规则还没发布，共享设置页就先写了规则表：旧规则并进来、旧键留着，下一次保存把两边一起写进文档，不会被悄悄丢掉。
    [prefs applySharedInputPreferences:@{@"default_ime_mode": @"chinese", @"app_input_mode_rules": @{@"org.example.shared": @"english"}}];
    assert([defaults objectForKey:@"MSIMEClientAppInputModeRules"] != nil);
    assert(prefs.applicationInputModeRulesAwaitPublication);
    [prefs activateInputModeForApplication:@"org.example.shared"];
    assert(prefs.englishMode);
    [prefs activateInputModeForApplication:@"org.example.legacy"];
    assert(prefs.englishMode);
    assert([[prefs sharedPreferencesByMerging:@{}][@"app_input_mode_rules"] isEqual:(@{@"org.example.shared": @"english", @"org.example.legacy": @"english"})]);
    // 文档已经包含旧键里的全部规则：迁移完成，旧键删除，文档是唯一来源。
    [prefs applySharedInputPreferences:@{@"default_ime_mode": @"chinese", @"app_input_mode_rules": @{@"org.example.shared": @"english"}}];
    assert([defaults objectForKey:@"MSIMEClientAppInputModeRules"] != nil);
    [prefs applySharedInputPreferences:@{@"default_ime_mode": @"chinese", @"app_input_mode_rules": @{@"org.example.shared": @"english", @"org.example.legacy": @"english"}}];
    assert([defaults objectForKey:@"MSIMEClientAppInputModeRules"] == nil);
    assert(!prefs.applicationInputModeRulesAwaitPublication);
    [prefs applySharedInputPreferences:@{@"default_ime_mode": @"chinese", @"app_input_mode_rules": @{@"org.example.shared": @"english"}}];
    [prefs activateInputModeForApplication:@"org.example.shared"];
    assert(prefs.englishMode);
    [prefs activateInputModeForApplication:@"org.example.legacy"];
    assert(!prefs.englishMode);
    assert([[prefs sharedPreferencesByMerging:@{}][@"app_input_mode_rules"] isEqual:(@{@"org.example.shared": @"english"})]);
    // 偏好库收不下的规则不发布：不合法的标识、只差大小写的重复和第 33 条以后的规则都留在本机，整份保存才不会被拒。
    NSMutableDictionary *crowded = [NSMutableDictionary dictionary];
    for (int index = 0; index < 40; ++index) crowded[[NSString stringWithFormat:@"org.example.app%02d", index]] = @"chinese";
    crowded[@"org.example.APP00"] = @"english";
    crowded[@" org.example.padded"] = @"english";
    crowded[@"org/example"] = @"english";
    crowded[[@"org.example." stringByPaddingToLength:65 withString:@"x" startingAtIndex:0]] = @"english";
    [defaults setObject:crowded forKey:@"MSIMEClientAppInputModeRules"];
    [prefs applySharedInputPreferences:@{@"default_ime_mode": @"chinese"}];
    NSDictionary *published = [prefs sharedPreferencesByMerging:@{}][@"app_input_mode_rules"];
    assert(published.count == 32);
    for (NSString *identifier in published) assert([identifier.lowercaseString hasPrefix:@"org.example.app"]);
    assert(published[@"org.example.APP00"] == nil || published[@"org.example.app00"] == nil);
    // 文档载入了发布出去的 32 条之后：已发布的从旧键里删掉，与已发布的只差大小写的那条是同一个 bundle id，以文档为准也删掉；收不下的（不合法的标识、第 33 条以后）留在旧键里，只在本机生效，不发布，也不再要求保存。
    NSString *longIdentifier = [@"org.example." stringByPaddingToLength:65 withString:@"x" startingAtIndex:0];
    [prefs applySharedInputPreferences:@{@"default_ime_mode": @"chinese", @"app_input_mode_rules": published}];
    assert(!prefs.applicationInputModeRulesAwaitPublication);
    NSDictionary *leftover = [defaults dictionaryForKey:@"MSIMEClientAppInputModeRules"];
    assert(leftover.count == crowded.count - 33);
    assert(leftover[@"org.example.app00"] == nil && leftover[@"org.example.APP00"] == nil);
    for (NSString *identifier in published) assert(leftover[identifier] == nil);
    assert([leftover[longIdentifier] isEqual:@"english"] && [leftover[@"org/example"] isEqual:@"english"]);
    [prefs activateInputModeForApplication:longIdentifier];
    assert(prefs.englishMode);
    [prefs activateInputModeForApplication:@"org/example"];
    assert(prefs.englishMode);
    assert([[prefs sharedPreferencesByMerging:@{}][@"app_input_mode_rules"] isEqual:published]);
    // 文档空出位置后，收得下的旧规则并进来、等下一次保存发布。
    NSMutableDictionary *roomy = [published mutableCopy];
    [roomy removeObjectForKey:roomy.allKeys.firstObject];
    [prefs applySharedInputPreferences:@{@"default_ime_mode": @"chinese", @"app_input_mode_rules": roomy}];
    assert(prefs.applicationInputModeRulesAwaitPublication);
    assert([[prefs sharedPreferencesByMerging:@{}][@"app_input_mode_rules"] count] == 32);
    [defaults removeObjectForKey:@"MSIMEClientAppInputModeRules"];
    [prefs applySharedInputPreferences:@{@"default_ime_mode": @"chinese"}];
    assert(!prefs.applicationInputModeRulesAwaitPublication);
    [prefs activateInputModeForApplication:@"org.example.shared"];
    assert(!prefs.englishMode);
    assert([[prefs sharedPreferencesByMerging:@{}][@"app_input_mode_rules"] isEqual:@{}]);
    MSIMERemoveTestPreferenceSuite(defaults, suite);
}

// 原生窗口的规则表编辑入口，「添加应用…」、移除和行内的模式选择都经由它。
@interface MSIMEAppearancePreferences (InputModeRuleEditing)
- (void)setInputMode:(NSString *)mode forApplication:(NSString *)identifier;
@end

// 迁移之后在原生窗口改规则：只改那一条所在的一边，不把已经在文档里的规则挤出去换成只在本机生效的旧规则；输入法重新启动、还没载入文档时不写规则表，免得本地键里剩下的那一小部分替换掉文档的整张表；按应用查规则时不分 ASCII 大小写，和偏好库的去重一致。
static void TestApplicationInputModeRuleEditsKeepDocumentTable() {
    NSString *suite = [@"msime.app-rule-edits." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    NSMutableDictionary *legacy = [NSMutableDictionary dictionary];
    for (int index = 0; index < 40; ++index) legacy[[NSString stringWithFormat:@"org.example.app%02d", index]] = @"chinese";
    NSString *longIdentifier = [@"org.example." stringByPaddingToLength:65 withString:@"x" startingAtIndex:0];
    legacy[longIdentifier] = @"english";
    [defaults setObject:legacy forKey:@"MSIMEClientAppInputModeRules"];
    MSIMEAppearancePreferences *prefs = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    assert([prefs sharedPreferencesByMerging:@{}][@"app_input_mode_rules"] == nil);
    [prefs applySharedInputPreferences:@{@"default_ime_mode": @"chinese"}];
    NSDictionary *first = [prefs sharedPreferencesByMerging:@{}][@"app_input_mode_rules"];
    assert(first.count == 32 && first[@"org.example.app31"] != nil && first[@"org.example.app32"] == nil);
    [prefs applySharedInputPreferences:@{@"default_ime_mode": @"chinese", @"app_input_mode_rules": first}];
    assert([[defaults dictionaryForKey:@"MSIMEClientAppInputModeRules"] count] == 9);
    // 共享设置页一次保存里删掉 app01、加上 org.zeta，文档仍是满的 32 条，app32 以后的旧规则继续只在本机生效。
    NSMutableDictionary *document = [first mutableCopy];
    [document removeObjectForKey:@"org.example.app01"];
    document[@"org.zeta"] = @"english";
    [prefs applySharedInputPreferences:@{@"default_ime_mode": @"chinese", @"app_input_mode_rules": document}];
    assert(!prefs.applicationInputModeRulesAwaitPublication);
    // 在原生窗口改一条文档里的规则：写出去的仍是文档那张表加上这次改动，org.zeta 不会被按标识排在前面的 app32 挤掉。
    [prefs setInputMode:@"english" forApplication:@"org.example.app05"];
    NSMutableDictionary *expected = [document mutableCopy];
    expected[@"org.example.app05"] = @"english";
    assert([[prefs sharedPreferencesByMerging:@{}][@"app_input_mode_rules"] isEqual:expected]);
    [prefs activateInputModeForApplication:@"org.example.app05"];
    assert(prefs.englishMode);
    // 改只在本机生效的旧规则只动本机那一边，文档那张表不变。
    [prefs setInputMode:@"chinese" forApplication:longIdentifier];
    assert([[prefs sharedPreferencesByMerging:@{}][@"app_input_mode_rules"] isEqual:expected]);
    [prefs activateInputModeForApplication:longIdentifier];
    assert(!prefs.englishMode);
    // 保存后重新载入：文档已经有的改动从本地键里删掉，收不下的旧规则带着新模式留在本地键。
    [prefs applySharedInputPreferences:@{@"default_ime_mode": @"chinese", @"app_input_mode_rules": expected}];
    NSDictionary *leftover = [defaults dictionaryForKey:@"MSIMEClientAppInputModeRules"];
    assert(leftover.count == 9 && [leftover[longIdentifier] isEqual:@"chinese"] && leftover[@"org.example.app05"] == nil);
    // 输入法重新启动、文档还没载入：本地键只剩收不下的旧规则，不能拿它替换文档的整张表。
    MSIMEAppearancePreferences *restarted = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    assert([restarted sharedPreferencesByMerging:@{}][@"app_input_mode_rules"] == nil);
    [restarted applySharedInputPreferences:@{@"default_ime_mode": @"chinese", @"app_input_mode_rules": expected}];
    assert([[restarted sharedPreferencesByMerging:@{}][@"app_input_mode_rules"] isEqual:expected]);
    // 移除文档里的一条规则：文档那张表少这一条，空出的位置在下一次载入时由收得下的旧规则补上，等待发布。
    [prefs setInputMode:nil forApplication:@"org.zeta"];
    [expected removeObjectForKey:@"org.zeta"];
    assert([[prefs sharedPreferencesByMerging:@{}][@"app_input_mode_rules"] isEqual:expected]);
    [prefs applySharedInputPreferences:@{@"default_ime_mode": @"chinese", @"app_input_mode_rules": expected}];
    assert(prefs.applicationInputModeRulesAwaitPublication);
    NSDictionary *refilled = [prefs sharedPreferencesByMerging:@{}][@"app_input_mode_rules"];
    assert(refilled.count == 32 && [refilled[@"org.example.app32"] isEqual:@"chinese"] && refilled[@"org.zeta"] == nil);
    // 共享设置页写下的标识大小写和 bundle id 不同：按应用查规则仍然找得到，手动切换照样让位，再次进入应用回到规则。
    [defaults setObject:@{@"com.apple.Terminal": @"chinese"} forKey:@"MSIMEClientAppInputModeRules"];
    [prefs applySharedInputPreferences:@{@"default_ime_mode": @"chinese", @"app_input_mode_rules": @{@"com.apple.terminal": @"english"}}];
    assert([defaults objectForKey:@"MSIMEClientAppInputModeRules"] == nil);
    [prefs activateInputModeForApplication:@"com.apple.Terminal"];
    assert(prefs.englishMode);
    prefs.englishMode = NO;
    assert(!prefs.englishMode);
    [prefs activateInputModeForApplication:@"org.example.other"];
    [prefs activateInputModeForApplication:@"com.apple.Terminal"];
    assert(prefs.englishMode);
    // 按应用的 bundle id 改规则，改的是文档里那一条，不会多出一条只差大小写的规则让整份保存被拒。
    [prefs setInputMode:@"chinese" forApplication:@"com.apple.Terminal"];
    assert([[prefs sharedPreferencesByMerging:@{}][@"app_input_mode_rules"] isEqual:(@{@"com.apple.terminal": @"chinese"})]);
    assert(!prefs.englishMode);
    // 升级前的表里有两条只差大小写的规则：一条并进文档那边，另一条留在本机那边。改本机那一条要真的生效，不能只改到文档里那条。
    [defaults setObject:@{@"com.example.Dup": @"chinese", @"com.example.dup": @"chinese"} forKey:@"MSIMEClientAppInputModeRules"];
    [prefs applySharedInputPreferences:@{@"default_ime_mode": @"chinese", @"app_input_mode_rules": @{@"com.apple.terminal": @"chinese"}}];
    [prefs setInputMode:@"english" forApplication:@"com.example.dup"];
    [prefs activateInputModeForApplication:@"com.example.dup"];
    assert(prefs.englishMode);
    MSIMERemoveTestPreferenceSuite(defaults, suite);
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

    // 默认英文直接进入新应用时，不必先来回切换一次中英文才能得到英文标点。
    [prefs applySharedInputPreferences:@{@"default_ime_mode":@"english", @"ime_mode_scope":@"app",
                                        @"chinese_punctuation":@YES, @"punctuation_lock":@"follow"}];
    [prefs activateInputModeForApplication:client.bundleIdentifier];
    assert(prefs.englishMode && !prefs.runtimeChinesePunctuation);
    assert(type(comma) == nil && type(letter) == nil);
    assert(!prefs.runtimeFullWidthInput && prefs.chinesePunctuation);

    // 手动覆盖仍按应用保留；新应用从英文标点开始，返回原应用不抹掉手动选择。
    prefs.runtimeChinesePunctuation = YES;
    [prefs activateInputModeForApplication:@"org.example.english-output-other"];
    assert(prefs.englishMode && !prefs.runtimeChinesePunctuation);
    [prefs activateInputModeForApplication:client.bundleIdentifier];
    assert(prefs.englishMode && prefs.runtimeChinesePunctuation);
    [prefs resetAllRuntimeInputState];
    assert(!prefs.runtimeChinesePunctuation);

    // 切回中文恢复保存的中文标点，回到英文重新跟随模式；固定英文标点不受模式影响。
    [controller setEnglishInputMode:NO];
    assert(!prefs.englishMode && prefs.runtimeChinesePunctuation);
    [controller setEnglishInputMode:YES];
    assert(prefs.englishMode && !prefs.runtimeChinesePunctuation && type(comma) == nil);
    prefs.punctuationLock = @"english";
    [controller setEnglishInputMode:NO];
    assert(!prefs.runtimeChinesePunctuation);

    // 固定中文标点时，英文模式仍按引擎正向表转换标点。
    prefs.punctuationLock = @"chinese";
    [controller setEnglishInputMode:YES];
    assert(prefs.englishMode && prefs.runtimeChinesePunctuation);
    // 人民币跟随本次全半角状态；英文模式锁定中文标点也走同一条规则。
    NSEvent *currency = EnglishKey(@"$", 21, NSEventModifierFlagShift);
    assert([type(currency) isEqual:@"¥"]);
    assert([controller handleEvent:widthToggle client:client] && prefs.runtimeFullWidthInput);
    assert([type(currency) isEqual:@"￥"]);
    assert([controller handleEvent:widthToggle client:client] && !prefs.runtimeFullWidthInput);
    assert([type(currency) isEqual:@"¥"]);
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
    // 引号与方头括号按 View 报告的样式出字（host-api 已解析好 corner_when_traditional）。
    [controller setEnglishInputMode:YES];
    [controller setValue:@{@"editing_text":@"", @"candidates":@[],
                           @"punctuation_marks":@{@"quotes":@"corner", @"brackets":@"hollow"}} forKey:@"view"];
    assert([type(quote) isEqual:@"『"] && [type(quote) isEqual:@"』"]);
    assert([type(EnglishKey(@"[", 33, 0)) isEqual:@"〖"] && [type(EnglishKey(@"]", 30, 0)) isEqual:@"〗"]);
    assert([type(comma) isEqual:@"，"]);
    [controller setValue:@{@"editing_text":@"", @"candidates":@[]} forKey:@"view"];
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

// 按住录音的三个组合键在这些测试期间关掉、测试后恢复。空格锁定在 macOS 上固定为开（不再读 `MSIMEClientVoiceHotkeyHoldSpace`），组合键开着时按住修饰键再按空格会被当作锁定录音，拦下 Control+Option+Space 等快捷键；只有关掉组合键才能把语音快捷键隔离出去。
static NSArray<NSString *> *MSIMETestVoiceHoldKeys() {
    return @[@"MSIMEClientVoiceHotkeyRightAlt", @"MSIMEClientVoiceHotkeyCtrlOption", @"MSIMEClientVoiceHotkeyCtrlCommand"];
}

static NSDictionary<NSString *, id> *MSIMEDisableVoiceHoldKeys(NSUserDefaults *defaults) {
    NSMutableDictionary<NSString *, id> *previous = [NSMutableDictionary dictionary];
    for (NSString *key in MSIMETestVoiceHoldKeys()) {
        id value = [defaults objectForKey:key];
        if (value) previous[key] = value;
        [defaults setBool:NO forKey:key];
    }
    return previous;
}

static void MSIMERestoreVoiceHoldKeys(NSUserDefaults *defaults, NSDictionary<NSString *, id> *previous) {
    for (NSString *key in MSIMETestVoiceHoldKeys()) {
        if (previous[key]) [defaults setObject:previous[key] forKey:key];
        else [defaults removeObjectForKey:key];
    }
}

static void TestControlOptionSpace() {
    NSUserDefaults *standardDefaults = NSUserDefaults.standardUserDefaults;
    NSDictionary *previousHoldKeys = MSIMEDisableVoiceHoldKeys(standardDefaults);
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
    // The controller clears marked text only where it knows it wrote some (MSIMEApplyTransitionTrackingMarkedText), so the composition set on the fake client is recorded as its own.
    [controller setValue:@YES forKey:@"clientHasMarkedText"];
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
    MSIMERestoreVoiceHoldKeys(standardDefaults, previousHoldKeys);
}

// 从控制器的偏好与修饰键事件入口验证 HUD，不能只检查孤立面板里有玻璃。
static void TestGlassInputModeHUD(BOOL visual) {
    NSString *suite = [@"msime.glass-hud." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    MSIMEAppearancePreferences *appearance = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    ModeController *controller = [ModeController alloc];
    ShortcutClient *client = [ShortcutClient new];
    client.caret = NSMakeRect(700, 500, 2, 24);
    MSIMEFloatingToolbarPanel *toolbar = [MSIMEFloatingToolbarPanel new];
    [toolbar setFrameAutosaveName:@""];
    [controller setValue:appearance forKey:@"appearance"];
    [controller setValue:client forKey:@"activeClient"];
    [controller setValue:toolbar forKey:@"toolbar"];
    [controller applySharedToolbarPreferences:@{@"global_theme":@"system", @"input_mode_hud":@YES, @"theme":@"system"}];
    MSIMEInputModeHUDPanel *hud = MSIMEInputModeHUDPanel.sharedPanel;
    assert(toolbar.liquidGlassEnabled && hud.liquidGlassEnabled);
    [appearance applySharedInputPreferences:@{@"keybindings":@{@"switch_language_shift":@YES}}];
    appearance.englishMode = NO;
    for (NSNumber *english in @[@YES, @NO]) {
        const unsigned short key = english.boolValue ? 56 : 60;
        const double pressedAt = english.boolValue ? 1.0 : 2.0;
        assert(![controller handleEvent:TapEvent(NSEventTypeFlagsChanged, key, NSEventModifierFlagShift, pressedAt) client:client]);
        assert([controller handleEvent:TapEvent(NSEventTypeFlagsChanged, key, 0, pressedAt + 0.1) client:client]);
        assert(appearance.englishMode == english.boolValue);
        assert(hud.visible && [hud.displayedText isEqual:english.boolValue ? @"英" : @"中"]);
        NSTextField *label = [hud valueForKey:@"label"];
        if (@available(macOS 26.0, *)) {
            if (!NSWorkspace.sharedWorkspace.accessibilityDisplayShouldReduceTransparency) {
                NSGlassEffectView *glass = [hud valueForKey:@"glassBackground"];
                assert(glass && [label isDescendantOf:glass.contentView]);
            }
        }
        if (visual) {
            printf("HUD visual window=%ld mode=%s\n", (long)hud.windowNumber, english.boolValue ? "english" : "chinese");
            fflush(stdout);
            NSDate *deadline = [NSDate dateWithTimeIntervalSinceNow:3];
            while (deadline.timeIntervalSinceNow > 0) {
                [hud showEnglishInputMode:english.boolValue nearCaretRect:client.caret];
                [NSRunLoop.currentRunLoop runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.1]];
            }
        }
    }
    [controller applySharedToolbarPreferences:@{@"input_mode_hud":@NO}];
    [hud orderOut:nil];
    assert(![controller handleEvent:TapEvent(NSEventTypeFlagsChanged, 56, NSEventModifierFlagShift, 3.0) client:client]);
    assert([controller handleEvent:TapEvent(NSEventTypeFlagsChanged, 56, 0, 3.1) client:client]);
    assert(appearance.englishMode && !hud.visible);
    [controller applySharedToolbarPreferences:@{@"global_theme":@"paper"}];
    assert(!toolbar.liquidGlassEnabled && !hud.liquidGlassEnabled);
    assert([hud valueForKey:@"glassBackground"] == nil);
    [hud orderOut:nil];
    [controller setValue:nil forKey:@"activeClient"];
    MSIMERemoveTestPreferenceSuite(defaults, suite);
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
    assert(menu.numberOfItems == 22);
    assert([[menu itemAtIndex:12].title isEqual:@"悬浮工具栏"]);
    NSArray<NSString *> *toolTitles = @[@"水杉表情面板…", @"云剪贴板…", @"云词库…", @"水杉屏幕键盘…", @"手写输入…", @"开始/结束语音输入"];
    NSArray<NSString *> *toolActions = @[@"showEmoji:", @"showCloudClipboard:", @"showCloudDictionary:", @"showScreenKeyboard:", @"showHandwriting:", @"showVoicePanel"];
    for (NSUInteger index = 0; index < toolTitles.count; ++index) {
        NSMenuItem *tool = [menu itemAtIndex:13 + index];
        assert([tool.title isEqual:toolTitles[index]] && tool.action == NSSelectorFromString(toolActions[index]));
    }
    assert([menu itemAtIndex:19].separatorItem);
    assert([[menu itemAtIndex:20].title isEqual:@"水杉输入法设置…"] && [menu itemAtIndex:20].action == @selector(showAppearance:));
    assert([[menu itemAtIndex:21].title isEqual:@"关于水杉输入法…"] && [menu itemAtIndex:21].action == @selector(showAbout:));
    // Simplified output is the off state of the one 繁体输出 toggle.
    const BOOL traditionalOutput = appearance.traditionalOutput;
    appearance.traditionalOutput = NO;
    NSMenuItem *traditional = [controller.menu itemAtIndex:4];
    assert([traditional.title isEqual:@"繁体输出"] && traditional.state == NSControlStateValueOff);
    [NSApp sendAction:traditional.action to:traditional.target from:traditional];
    assert(appearance.traditionalOutput && [controller.menu itemAtIndex:4].state == NSControlStateValueOn);
    [NSApp sendAction:traditional.action to:traditional.target from:traditional];
    assert(!appearance.traditionalOutput);
    appearance.traditionalOutput = traditionalOutput;
    // The typing toggles mirror the toolbar's runtime state, show the chords handleEvent claims, and flip through the same paths.
    NSMenuItem *fullWidth = [menu itemAtIndex:5], *punctuation = [menu itemAtIndex:6], *translations = [menu itemAtIndex:7];
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
    assert([controller.menu itemAtIndex:5].state == (runtimeWidth ? NSControlStateValueOff : NSControlStateValueOn));
    [NSApp sendAction:fullWidth.action to:fullWidth.target from:fullWidth];
    assert(appearance.runtimeFullWidthInput == runtimeWidth);
    NSString *lock = appearance.punctuationLock;
    appearance.punctuationLock = @"follow";
    const BOOL runtimePunctuation = appearance.runtimeChinesePunctuation;
    punctuation = [controller.menu itemAtIndex:6];
    assert(punctuation.enabled && punctuation.state == (runtimePunctuation ? NSControlStateValueOn : NSControlStateValueOff));
    [NSApp sendAction:punctuation.action to:punctuation.target from:punctuation];
    assert(appearance.runtimeChinesePunctuation == !runtimePunctuation && session.chinesePunctuation == !runtimePunctuation);
    assert([controller.menu itemAtIndex:6].state == (runtimePunctuation ? NSControlStateValueOff : NSControlStateValueOn));
    [NSApp sendAction:punctuation.action to:punctuation.target from:punctuation];
    assert(appearance.runtimeChinesePunctuation == runtimePunctuation);
    appearance.punctuationLock = @"english";
    assert(![controller.menu itemAtIndex:6].enabled);
    appearance.punctuationLock = lock;
    const BOOL showTranslations = appearance.candidateTranslations;
    assert(translations.state == (showTranslations ? NSControlStateValueOn : NSControlStateValueOff));
    [NSApp sendAction:translations.action to:translations.target from:translations];
    assert(appearance.candidateTranslations == !showTranslations);
    assert([controller.menu itemAtIndex:7].state == (showTranslations ? NSControlStateValueOff : NSControlStateValueOn));
    appearance.candidateTranslations = showTranslations;
    // The scheme is a radio submenu whose parent row names the selected scheme, the 双拼 row naming the selected profile.
    NSString *scheme = appearance.inputScheme, *profile = appearance.shuangpinProfile;
    appearance.inputScheme = @"quanpin";
    appearance.shuangpinProfile = @"ziranma";
    NSMenuItem *schemeItem = [controller.menu itemAtIndex:9];
    // 粤拼、注音和笔画只在装了词库时列出，隔离的 home 里三个词库都没有，所以按引擎顺序列出其余七个；笔画排在最后，下面的表不列它。词库在场的情形由 TestOptInSchemeModes 覆盖。
    assert([schemeItem.title isEqual:@"输入方案（全拼）"] && schemeItem.submenu.numberOfItems == 7);
    NSArray<NSString *> *schemeTitles = @[@"全拼", @"双拼（自然码）", @"五笔 86", @"日语", @"韩语", @"粤拼", @"注音", @"越南语", @"藏文"];
    NSArray<NSString *> *schemeIDs = @[@"quanpin", @"shuangpin", @"wubi", @"japanese", @"korean", @"cantonese", @"zhuyin", @"vietnamese", @"tibetan"];
    NSUInteger expected = 0;
    for (NSInteger index = 0; index < schemeItem.submenu.numberOfItems; ++index) {
        NSMenuItem *item = [schemeItem.submenu itemAtIndex:index];
        while (expected < schemeIDs.count && ![item.representedObject isEqual:schemeIDs[expected]]) {
            assert(expected == 5 || expected == 6);
            ++expected;
        }
        assert(expected < schemeIDs.count && [item.title isEqual:schemeTitles[expected]]);
        assert(item.action == @selector(selectInputScheme:) && item.target == controller);
        assert(item.state == (expected == 0 ? NSControlStateValueOn : NSControlStateValueOff));
        ++expected;
    }
    assert(expected == schemeIDs.count);
    [NSApp sendAction:@selector(selectInputScheme:) to:controller from:[schemeItem.submenu itemAtIndex:2]];
    assert([appearance.inputScheme isEqual:@"wubi"]);
    schemeItem = [controller.menu itemAtIndex:9];
    assert([schemeItem.title isEqual:@"输入方案（五笔 86）"]);
    // 98 五笔时菜单项和上一级标题都写出版本。
    NSString *wubiProfile = appearance.wubiProfile;
    appearance.wubiProfile = @"wubi98";
    schemeItem = [controller.menu itemAtIndex:9];
    assert([schemeItem.title isEqual:@"输入方案（五笔 98）"] && [[schemeItem.submenu itemAtIndex:2].title isEqual:@"五笔 98"]);
    appearance.wubiProfile = wubiProfile;
    schemeItem = [controller.menu itemAtIndex:9];
    assert([schemeItem.submenu itemAtIndex:2].state == NSControlStateValueOn && [schemeItem.submenu itemAtIndex:0].state == NSControlStateValueOff);
    [NSApp sendAction:@selector(selectInputScheme:) to:controller from:[schemeItem.submenu itemAtIndex:4]];
    assert([appearance.inputScheme isEqual:@"korean"] && [appearance.lastChineseScheme isEqual:@"wubi"]);
    schemeItem = [controller.menu itemAtIndex:9];
    assert([schemeItem.submenu itemAtIndex:4].state == NSControlStateValueOn && [schemeItem.submenu itemAtIndex:2].state == NSControlStateValueOff);
    appearance.inputScheme = scheme;
    appearance.shuangpinProfile = profile;
    // The theme submenu is the shared catalog, ticked at the current theme, whose name the parent row carries.
    NSString *globalTheme = appearance.globalTheme;
    appearance.globalTheme = @"system";
    NSMenuItem *themeItem = [controller.menu itemAtIndex:10];
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
    themeItem = [controller.menu itemAtIndex:10];
    assert(([themeItem.title isEqual:[NSString stringWithFormat:@"主题（%@）", @(catalog[1].title.c_str())]]));
    assert([themeItem.submenu itemAtIndex:1].state == NSControlStateValueOn);
    // A theme with a mode of its own fixes the menus' mode as it fixes the candidate window's and the toolbar's, over an explicit menu theme.
    [controller setValue:@{@"theme": @"light", @"menu_theme": @"light"} forKey:@"menuThemePreferences"];
    assert([controller.menu.appearance.name isEqual:NSAppearanceNameDarkAqua]);
    appearance.globalTheme = @"paper";
    assert([controller.menu.appearance.name isEqual:NSAppearanceNameAqua]);
    appearance.globalTheme = @"system";
    // 跟随系统的主题下菜单按颜色模式着色，菜单单独的明暗设置在 macOS 上不读。
    [controller setValue:@{@"theme": @"light", @"menu_theme": @"dark"} forKey:@"menuThemePreferences"];
    assert([controller.menu.appearance.name isEqual:NSAppearanceNameAqua]);
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
    for (NSView *child in MSIMECandidateControlsView(container).subviews)
        if ([child isKindOfClass:MSIMECandidateButton.class] &&
            [((MSIMECandidateButton *)child).candidateID isEqual:identifier])
            return (MSIMECandidateButton *)child;
    return nil;
}
static MSIMECandidateButton *PageButton(NSView *content, NSInteger tag) {
    for (NSView *view in MSIMECandidateControlsView(content).subviews) {
        if ([view isKindOfClass:MSIMECandidateButton.class] && view.tag == tag) return (id)view;
    }
    return nil;
}

@interface TranslationFontSession : ExpandedGridSession
@end
@implementation TranslationFontSession
- (NSDictionary *)gridCandidate:(NSUInteger)index highlighted:(BOOL)highlighted {
    NSMutableDictionary *candidate = [[super gridCandidate:index highlighted:highlighted] mutableCopy];
    candidate[@"translation"] = @"synthetic glossary";
    return candidate;
}
@end

static void TestCandidateTranslationFontSize() {
    NSString *suite = [@"msime.translation-font." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    MSIMEAppearancePreferences *prefs = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    prefs.englishMode = NO;
    prefs.candidateTranslations = YES;
    prefs.expandCandidates = YES;
    prefs.pageSize = 5;
    ModeController *controller = [ModeController alloc];
    TranslationFontSession *session = [TranslationFontSession new];
    session.generation = 7;
    session.total = 23;
    session.pageSize = 5;
    session.prefix = @"词";
    session.editing = @"shi";
    ShortcutClient *client = [ShortcutClient new];
    client.caret = NSMakeRect(200, 400, 1, 18);
    HiddenCandidatePanel *panel = [[HiddenCandidatePanel alloc] init];
    [controller setValue:prefs forKey:@"appearance"];
    [controller setValue:session forKey:@"session"];
    [controller setValue:client forKey:@"activeClient"];
    [controller setValue:panel forKey:@"panel"];
    [controller setValue:[session gridView] forKey:@"view"];
    NSMutableDictionary *preferences = [[prefs sharedPreferencesByMerging:@{}] mutableCopy];
    for (NSNumber *vertical in @[@NO, @YES]) {
        preferences[@"candidate_layout"] = vertical.boolValue ? @"vertical" : @"horizontal";
        for (NSNumber *scale in @[@100, @150]) {
            for (id size in @[@12, @21, @32, NSNull.null]) {
                preferences[@"candidate_translation_font_size"] = size;
                preferences[@"candidate_scale_percent"] = scale;
                [prefs applySharedCandidatePreferences:preferences];
                [controller renderCandidates];
                MSIMECandidateButton *button = PageButton(panel.contentView, 0);
                CGFloat expected = (size == NSNull.null ? MSIMECandidateTranslationPointSize : [size doubleValue]) * scale.doubleValue / 100;
                assert(button && [button.translation isEqual:@"synthetic glossary"]);
                assert(button.translationBelow == !vertical.boolValue);
                assert(fabs(button.translationFont.pointSize - expected) < 0.01);
                assert(button.frame.size.height >= MSIMETranslationTextSize(button.translation, button.translationFont).height);
                NSBitmapImageRep *bitmap = [button bitmapImageRepForCachingDisplayInRect:button.bounds];
                assert(bitmap);
                [button cacheDisplayInRect:button.bounds toBitmapImageRep:bitmap];
            }
        }
    }
    preferences[@"candidate_layout"] = @"horizontal";
    [prefs applySharedCandidatePreferences:preferences];
    [controller renderCandidates];
    assert([controller handleEvent:ModeKey(125, 0, NO) client:client]);
    assert([controller valueForKey:@"expandedCandidates"]);
    for (id size in @[@21, @32, NSNull.null]) {
        preferences[@"candidate_translation_font_size"] = size;
        [prefs applySharedCandidatePreferences:preferences];
        [controller renderCandidates];
        MSIMECandidateButton *button = ExpandedGridSubview(panel, 0);
        CGFloat expected = (size == NSNull.null ? MSIMECandidateTranslationPointSize : [size doubleValue]) * 1.5;
        assert(button && [button.translation isEqual:@"synthetic glossary"]);
        assert(fabs(button.translationFont.pointSize - expected) < 0.01);
        assert(button.frame.size.height >= MSIMETranslationTextSize(button.translation, button.translationFont).height);
    }
    [controller closeExpandedCandidates];
    [defaults removePersistentDomainForName:suite];
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
    assert([external.globalTheme isEqual:@"custom"] && [external.customCandidateSkin isEqual:@"synthetic"] && [external decorationImageForDark:NO]);
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
    assert([external.globalTheme isEqual:@"system"] && ![external decorationImageForDark:NO]);
    [NSApp sendAction:skinSwitches.lastObject.action to:skinSwitches.lastObject.target from:skinSwitches.lastObject];
    assert([external.globalTheme isEqual:@"custom"] && [external.customCandidateSkin isEqual:@"synthetic"] && [external decorationImageForDark:NO]);
    assert(skinSwitches.lastObject.state == NSControlStateValueOn);
    assert([panel.contentView.subviews.lastObject isKindOfClass:MSIMECandidateDecorationView.class]);
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
            MSIMECandidateDecorationView *decoration = (id)chrome.subviews.lastObject;
            assert([decoration isKindOfClass:MSIMECandidateDecorationView.class] && decoration.image);
            assert(panel.frame.size.width >= 240);
            // The 48pt band above the card is transparent and the square fixture, too tall at 120pt wide for the band plus the card's inset it overhangs, shrinks whole to that room: flush with the window's top, the inset in from the card's right edge.
            const CGFloat overhang = decoration.frame.size.height - 48;
            assert(chrome.cardTopInset == 48 && overhang > 0 && decoration.frame.size.width == decoration.frame.size.height);
            assert(NSMaxX(decoration.frame) == chrome.bounds.size.width - overhang && NSMaxY(decoration.frame) == chrome.bounds.size.height);
            MSIMECandidateButton *first = PageButton(chrome, 0);
            assert(NSMaxY(first.frame) <= NSMinY(decoration.frame));
            // 光标在屏幕中间，卡片放在下方：贴着光标的是卡片顶边（窗口顶边减 48pt 装饰带），不是窗口顶边；装饰带朝上伸过光标这一行。
            const NSRect caret = ((ShortcutClient *)[controller valueForKey:@"activeClient"]).caret;
            assert(NSMinY(caret) - 4 - (NSHeight(panel.frame) - 48) >= NSMinY(MSIMECandidateScreen(caret).visibleFrame));
            assert(fabs(NSMaxY(panel.frame) - 48 - (NSMinY(caret) - 4)) <= 0.5 && NSMaxY(panel.frame) > NSMaxY(caret));
            assert(first.showSelectedBar == [theme isEqual:NSAppearanceNameDarkAqua]);
            const auto tokens = [external resolvedSkinForDark:[theme isEqual:NSAppearanceNameDarkAqua]].tokens;
            assert([chrome.fillColor isEqual:SkinColor(tokens.surface)]);
            assert([first.titleColor isEqual:SkinColor(tokens.selectedText)]);
            NSBitmapImageRep *bitmap = [chrome bitmapImageRepForCachingDisplayInRect:chrome.bounds];
            [chrome cacheDisplayInRect:chrome.bounds toBitmapImageRep:bitmap];
            assert(bitmap && [[controller valueForKey:@"view"] isEqual:before]);
            // Left of the right-aligned mascot the band is see-through; the card under it is filled.
            assert([bitmap colorAtX:2 y:2].alphaComponent < 0.01);
            assert([bitmap colorAtX:2 y:(NSInteger)(bitmap.pixelsHigh * 0.8)].alphaComponent > 0.99);
        }
    }
    external.vertical = NO;
    assert(![external resolvedSkinForDark:YES].fitContent);
    for (NSNumber *paged in @[@NO, @YES]) {
        NSMutableDictionary *widthView = [@{
            @"focused": @YES, @"editing_text": @"ceshi", @"page": @0,
            @"page_count": paged.boolValue ? @2 : @1,
            @"candidates": @[@{@"text": @"测试", @"highlighted": @YES}]
        } mutableCopy];
        [controller setValue:[widthView copy] forKey:@"view"];
        [controller renderCandidates];
        const CGFloat narrowWidth = panel.frame.size.width;
        assert(panel.requestedVisible && narrowWidth >= 240);
        widthView[@"candidates"] = @[@{@"text": @"合成候选窗口宽度回归测试用长文本", @"highlighted": @YES}];
        [controller setValue:[widthView copy] forKey:@"view"];
        [controller renderCandidates];
        assert(panel.requestedVisible && panel.frame.size.width > narrowWidth);
        widthView[@"page"] = paged.boolValue ? @1 : @0;
        widthView[@"candidates"] = @[@{@"text": @"测试", @"highlighted": @YES}];
        [controller setValue:[widthView copy] forKey:@"view"];
        [controller renderCandidates];
        assert(panel.requestedVisible && fabs(panel.frame.size.width - narrowWidth) < 0.01);
    }
    [controller setValue:before forKey:@"view"];
    // No disk reads while typing/rendering: removal takes effect only on explicit reload.
    std::filesystem::remove_all(root / "synthetic");
    [controller renderCandidates];
    assert([external resolvedSkinForDark:NO].candidateSkin == "synthetic" && [external decorationImageForDark:NO]);
    // Rescanning is 刷新皮肤 on the skin page, which rebuilds the cards from the directory; the
    // accessor performs the same reload the button does.
    MetasequoiaSkinSettingsView *rescanned = (id)[external skinSettingsView];
    // The choice is kept for when the package comes back; meanwhile the custom theme is drawn without it.
    assert([external.customCandidateSkin isEqual:@"synthetic"]);
    assert([external resolvedSkinForDark:NO].id == "custom" && [external resolvedSkinForDark:NO].candidateSkin.empty() &&
           ![external decorationImageForDark:NO]);
    NSArray<NSSwitch *> *rescannedSwitches = [rescanned valueForKey:@"switches"];
    assert(rescannedSwitches.count == 7 && [rescannedSwitches.firstObject.identifier isEqual:@"system"]);
    [controller appearanceChanged:nil];
    for (NSView *view in panel.contentView.subviews) assert(![view isKindOfClass:MSIMECandidateDecorationView.class]);
    panel.appearance = nil;
    std::filesystem::remove_all(root);
}

static void WriteSolidPNG(const std::filesystem::path &path, unsigned char red, unsigned char green, unsigned char blue) {
    NSBitmapImageRep *image = [[NSBitmapImageRep alloc] initWithBitmapDataPlanes:nullptr pixelsWide:4 pixelsHigh:4 bitsPerSample:8 samplesPerPixel:4 hasAlpha:YES isPlanar:NO colorSpaceName:NSDeviceRGBColorSpace bytesPerRow:0 bitsPerPixel:0];
    for (NSInteger y = 0; y < 4; ++y) for (NSInteger x = 0; x < 4; ++x) {
        unsigned char *pixel = image.bitmapData + y * image.bytesPerRow + x * 4;
        pixel[0] = red; pixel[1] = green; pixel[2] = blue; pixel[3] = 255;
    }
    image = [image bitmapImageRepByRetaggingWithColorSpace:NSColorSpace.sRGBColorSpace];
    assert(image);
    assert([[image representationUsingType:NSBitmapImageFileTypePNG properties:@{}] writeToFile:@(path.c_str()) atomically:YES]);
}

// The msime-skins keys on the live panel: decoration.image over the preview and its alignment, the package card radius, the background drawn under the rows and clipped to the card, the translation colour, and the toolbar's own radius and colours.
static void TestStyledExternalSkin(MSIMEInputController *controller, HiddenCandidatePanel *panel, NSUserDefaults *defaults) {
    char temporary[] = "/tmp/msime-native-styled-skin-XXXXXX";
    assert(mkdtemp(temporary));
    const std::filesystem::path root(temporary);
    std::filesystem::create_directories(root / "styled" / "assets");
    auto writeManifest = [&](const char *align, const char *lightSurface = "#FFF7FA") {
        std::ofstream manifest(root / "styled" / "skin.toml");
        manifest << R"toml(schema_version = 1
id = "styled"
name = "Styled Skin"
version = "1.0"
base = "system"
preview = "assets/preview.png"
[supports]
layouts = ["horizontal", "vertical"]
themes = ["dark", "light"]
[candidate_window]
min_width_dip = 240
corner_radius_dip = 30
[candidate_window.decoration]
image = "assets/character.png"
top_inset_dip = 48
width_dip = 60
align = ")toml" << align << R"toml("
[candidate_window.background]
image = "assets/background.png"
fit = "stretch"
[candidate.dark]
surface = "#121314"
translation = "#9FB4E0"
[candidate.light]
surface = ")toml" << lightSurface << R"toml("
[toolbar]
corner_radius_dip = 5
[toolbar.dark]
background = "#141B33"
divider = "#224466"
[toolbar.light]
background = "#F4F8FF"
)toml";
        assert(manifest.good());
    };
    writeManifest("left");
    WriteSolidPNG(root / "styled" / "assets" / "character.png", 255, 0, 0);
    WriteSolidPNG(root / "styled" / "assets" / "preview.png", 0, 255, 0);
    WriteSolidPNG(root / "styled" / "assets" / "background.png", 0, 0, 255);
    MSIMEAppearancePreferences *styled = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults skinsRoot:[NSURL fileURLWithPath:@(root.c_str()) isDirectory:YES]];
    [styled selectExternalSkin:@"styled" base:@"system"];
    assert([styled resolvedSkinForDark:NO].candidateSkin == "styled" && [styled decorationImageForDark:NO] && [styled backgroundImageForDark:NO] && [styled decorationImageForDark:YES] == [styled decorationImageForDark:NO]);
    assert([styled resolvedSkinForDark:YES].decorationPath.find("character.png") != std::string::npos);
    NSDictionary *before = [[controller valueForKey:@"view"] copy];
    [controller setValue:styled forKey:@"appearance"];
    MSIMEFloatingToolbarPanel *toolbar = [MSIMEFloatingToolbarPanel new];
    [toolbar setFrameAutosaveName:@""];
    [controller setValue:toolbar forKey:@"toolbar"];
    styled.vertical = NO;
    for (NSString *theme in @[NSAppearanceNameAqua, NSAppearanceNameDarkAqua]) {
        const BOOL dark = [theme isEqual:NSAppearanceNameDarkAqua];
        panel.appearance = [NSAppearance appearanceNamed:theme];
        [controller appearanceChanged:nil];
        const auto skin = [styled resolvedSkinForDark:dark];
        MSIMECandidateChromeView *chrome = (id)panel.contentView;
        assert(chrome.cornerRadius == 30.0 && skin.tokens.radius == 30.0f);
        assert(chrome.backgroundImage == [styled backgroundImageForDark:dark] && chrome.backgroundFit == msime::mac::BackgroundFit::stretch && chrome.backgroundOpacity == 1.0);
        MSIMECandidateDecorationView *decoration = (id)chrome.subviews.lastObject;
        assert([decoration isKindOfClass:MSIMECandidateDecorationView.class] && decoration.image == [styled decorationImageForDark:dark]);
        // Left-aligned: in from the card's left edge by the same inset its bottom overhangs the card's top edge (the card starts 48pt down, under the band).
        const CGFloat overhang = NSHeight(chrome.bounds) - 48 - NSMinY(decoration.frame);
        assert(overhang > 0 && NSMinX(decoration.frame) == overhang && decoration.frame.size.width == decoration.frame.size.height);
        // The translation colour is the package's on every row in the mode that declares one, except the highlighted row, which takes the one derived for the selected fill (SkinTokens::selectedTranslation); elsewhere the rows keep the number colour.
        for (NSView *view in chrome.candidateContentView.subviews) {
            if (![view isKindOfClass:MSIMECandidateButton.class] || view.tag < 0) continue;
            MSIMECandidateButton *button = (id)view;
            if (dark)
                assert(skin.tokens.translation && skin.tokens.selectedTranslation &&
                       [button.translationColor isEqual:SkinColor(button.candidateHighlighted ? *skin.tokens.selectedTranslation : *skin.tokens.translation)]);
            else if (!button.candidateFixed)
                assert(!skin.tokens.translation && [button.translationColor isEqual:SkinColor(button.candidateHighlighted ? skin.tokens.selectedNumber : skin.tokens.number)]);
        }
        // The background shows through the empty inset under the rows and is clipped off the rounded corner (the bottom one: the left-aligned decoration covers the top left).
        NSBitmapImageRep *bitmap = [chrome bitmapImageRepForCachingDisplayInRect:chrome.bounds];
        [chrome cacheDisplayInRect:chrome.bounds toBitmapImageRep:bitmap];
        const CGFloat scale = bitmap.pixelsWide / NSWidth(chrome.bounds);
        NSColor *inside = [[bitmap colorAtX:(NSInteger)(NSWidth(chrome.bounds) / 2 * scale) y:(NSInteger)((NSHeight(chrome.bounds) - 3) * scale)] colorUsingColorSpace:NSColorSpace.sRGBColorSpace];
        assert(inside.blueComponent > .8 && inside.blueComponent - inside.redComponent > .5 && inside.blueComponent - inside.greenComponent > .5);
        NSColor *corner = [[bitmap colorAtX:0 y:bitmap.pixelsHigh - 1] colorUsingColorSpace:NSColorSpace.sRGBColorSpace];
        assert(corner.alphaComponent < .5);
        // The toolbar takes the package's toolbar radius and colours, never its card radius.
        [toolbar applyThemePreferences:@{@"theme": dark ? @"dark" : @"light"}];
        NSView *toolbarChrome = [toolbar valueForKey:@"chrome"];
        const auto toolbarTokens = [styled toolbarSkinForDark:dark];
        assert(toolbarTokens.radius == 5.0f && toolbarChrome.layer.cornerRadius == 5.0);
        assert([[toolbarChrome valueForKey:@"fillColor"] isEqual:SkinColor(toolbarTokens.surface)]);
        NSColor *expectedFill = dark ? [NSColor colorWithSRGBRed:0x14 / 255.0 green:0x1B / 255.0 blue:0x33 / 255.0 alpha:1]
                                     : [NSColor colorWithSRGBRed:0xF4 / 255.0 green:0xF8 / 255.0 blue:0xFF / 255.0 alpha:1];
        assert([SkinColor(toolbarTokens.surface) isEqual:expectedFill]);
        assert(dark ? toolbarTokens.divider.has_value() : !toolbarTokens.divider.has_value());
        assert([[controller valueForKey:@"view"] isEqual:before]);
    }
    // Center alignment after an explicit reload.
    writeManifest("center");
    [styled reloadSkins];
    [controller appearanceChanged:nil];
    MSIMECandidateChromeView *chrome = (id)panel.contentView;
    MSIMECandidateDecorationView *decoration = (id)chrome.subviews.lastObject;
    assert([decoration isKindOfClass:MSIMECandidateDecorationView.class]);
    assert(std::abs(NSMidX(decoration.frame) - NSWidth(chrome.bounds) / 2) < 0.01);
    for (NSView *child in chrome.subviews) assert(![child isKindOfClass:NSVisualEffectView.class]);
    // 旧版皮肤格式没有材质字段；半透明表面只改变颜色的 alpha，不能隐式开启模糊。
    writeManifest("center", "#D9D9DB99");
    panel.appearance = [NSAppearance appearanceNamed:NSAppearanceNameAqua];
    [styled reloadSkins];
    [controller appearanceChanged:nil];
    chrome = (id)panel.contentView;
    NSVisualEffectView *material = nil;
    for (NSView *child in chrome.subviews)
        if ([child isKindOfClass:NSVisualEffectView.class]) material = (id)child;
    assert(material == nil);
    // 纯透明表面同样不创建材质视图。
    writeManifest("center", "#D9D9DB00");
    [styled reloadSkins];
    [controller appearanceChanged:nil];
    chrome = (id)panel.contentView;
    for (NSView *child in chrome.subviews) assert(![child isKindOfClass:NSVisualEffectView.class]);
    panel.appearance = nil;
    std::filesystem::remove_all(root);
}

// 浅色、深色两个槽位放两个包：每种明暗画自己那个包的装饰图和背景图，明暗切换时候选窗换成另一个包的图，而不是一张图两种明暗共用。
static void TestSkinSlotsPerMode(MSIMEInputController *controller, HiddenCandidatePanel *panel, NSUserDefaults *defaults) {
    char temporary[] = "/tmp/msime-native-slot-skins-XXXXXX";
    assert(mkdtemp(temporary));
    const std::filesystem::path root(temporary);
    auto writePackage = [&](const char *id, const char *base, const char *mode, const char *surface) {
        std::filesystem::create_directories(root / id / "assets");
        std::ofstream manifest(root / id / "skin.toml");
        manifest << "schema_version = 1\nid = \"" << id << "\"\nname = \"" << id << "\"\nversion = \"1.0\"\nbase = \"" << base
                 << "\"\n[supports]\nlayouts = [\"horizontal\", \"vertical\"]\nthemes = [\"" << mode
                 << "\"]\n[candidate_window.decoration]\nimage = \"assets/character.png\"\ntop_inset_dip = 48\nwidth_dip = 60\n"
                 << "[candidate_window.background]\nimage = \"assets/background.png\"\nfit = \"stretch\"\n[candidate." << mode
                 << "]\nsurface = \"" << surface << "\"\n";
        assert(manifest.good());
    };
    writePackage("dawn", "paper", "light", "#FFF7FA");
    writePackage("dusk", "night", "dark", "#121314");
    WriteSolidPNG(root / "dawn" / "assets" / "character.png", 255, 0, 0);
    WriteSolidPNG(root / "dawn" / "assets" / "background.png", 255, 255, 0);
    WriteSolidPNG(root / "dusk" / "assets" / "character.png", 0, 0, 255);
    WriteSolidPNG(root / "dusk" / "assets" / "background.png", 0, 255, 255);
    MSIMEAppearancePreferences *slots = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults skinsRoot:[NSURL fileURLWithPath:@(root.c_str()) isDirectory:YES]];
    // 旧文档的浅色槽位里放着一款已经不在目录里的皮肤：应用浅色皮肤时直接覆盖，不挪进深色槽位。
    [slots clearCustomCandidateSkin];
    [defaults setObject:@"gone" forKey:@"MSIMEClientCustomCandidateSkin"];
    [slots selectExternalSkin:@"dawn" base:@"paper"];
    assert([slots.customCandidateSkin isEqual:@"dawn"] && slots.customCandidateSkinDark == nil);
    // 旧文档只有一款深色皮肤、存在浅色槽位里：应用浅色皮肤时它挪进深色槽位，两款都留着。
    [slots clearCustomCandidateSkin];
    [defaults setObject:@"dusk" forKey:@"MSIMEClientCustomCandidateSkin"];
    [slots selectExternalSkin:@"dawn" base:@"paper"];
    assert([slots.customCandidateSkin isEqual:@"dawn"] && [slots.customCandidateSkinDark isEqual:@"dusk"] &&
           [slots.customThemeBase isEqual:@"paper"]);
    [slots selectExternalSkin:@"dusk" base:@"night"];
    assert([slots.customCandidateSkin isEqual:@"dawn"] && [slots.customCandidateSkinDark isEqual:@"dusk"] &&
           [slots.customThemeBase isEqual:@"night"]);
    assert([slots resolvedSkinForDark:NO].candidateSkin == "dawn" && [slots resolvedSkinForDark:YES].candidateSkin == "dusk");
    // 两个槽位各画各自明暗的包：主题不固定明暗，候选窗跟随宿主。
    assert(![slots fixedThemeMode]);
    NSImage *lightDecoration = [slots decorationImageForDark:NO], *darkDecoration = [slots decorationImageForDark:YES];
    NSImage *lightBackground = [slots backgroundImageForDark:NO], *darkBackground = [slots backgroundImageForDark:YES];
    assert(lightDecoration && darkDecoration && lightDecoration != darkDecoration);
    assert(lightBackground && darkBackground && lightBackground != darkBackground);
    [controller setValue:slots forKey:@"appearance"];
    MSIMEFloatingToolbarPanel *toolbar = [MSIMEFloatingToolbarPanel new];
    [toolbar setFrameAutosaveName:@""];
    [controller setValue:toolbar forKey:@"toolbar"];
    slots.vertical = NO;
    for (NSString *theme in @[NSAppearanceNameAqua, NSAppearanceNameDarkAqua]) {
        const BOOL dark = [theme isEqual:NSAppearanceNameDarkAqua];
        panel.appearance = [NSAppearance appearanceNamed:theme];
        [controller appearanceChanged:nil];
        MSIMECandidateChromeView *chrome = (id)panel.contentView;
        MSIMECandidateDecorationView *decoration = (id)chrome.subviews.lastObject;
        assert([decoration isKindOfClass:MSIMECandidateDecorationView.class] && decoration.image == [slots decorationImageForDark:dark]);
        assert(chrome.backgroundImage == [slots backgroundImageForDark:dark]);
        assert([chrome.fillColor isEqual:SkinColor([slots resolvedSkinForDark:dark].tokens.surface)]);
    }
    // 只是外观变了、没有整页重排：画的包换了，候选窗也要重排成另一个包的装饰。
    panel.appearance = [NSAppearance appearanceNamed:NSAppearanceNameAqua];
    [controller appearanceChanged:nil];
    assert(((MSIMECandidateDecorationView *)panel.contentView.subviews.lastObject).image == lightDecoration);
    panel.appearance = [NSAppearance appearanceNamed:NSAppearanceNameDarkAqua];
    [controller refreshCandidateSkin];
    MSIMECandidateChromeView *switched = (id)panel.contentView;
    assert(((MSIMECandidateDecorationView *)switched.subviews.lastObject).image == darkDecoration && switched.backgroundImage == darkBackground);
    // 停用深色皮肤只清深色槽位；深色模式回落到浅色槽位，但浅色底的包不在深色模式画。
    [slots removeCustomCandidateSkin:@"dusk"];
    assert([slots.customCandidateSkin isEqual:@"dawn"] && slots.customCandidateSkinDark == nil);
    assert([slots resolvedSkinForDark:YES].candidateSkin.empty() && ![slots decorationImageForDark:YES] && [slots decorationImageForDark:NO]);
    [slots clearCustomCandidateSkin];
    assert(slots.customCandidateSkin == nil && slots.customCandidateSkinDark == nil);
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
@property(nonatomic, copy) NSString *expectedCloudMethod;
@end
@implementation CloudShortcutController
- (MSIMECloudCandidateRequest *)cloudRequestForURL:(NSURL *)url completion:(void (^)(NSData *))completion {
    assert([url.host isEqual:@"inputtools.google.com"]);
    ControlledCloudRequest *request = [ControlledCloudRequest new];
    request.reply = completion;
    [self.requests addObject:request];
    return request;
}
- (MSIMECloudCandidateRequest *)cloudRequestForDescriptor:(NSDictionary *)descriptor completion:(void (^)(NSData *))completion {
    assert(([@[@"GET", @"POST"] containsObject:descriptor[@"method"]]));
    if (self.expectedCloudMethod) assert([descriptor[@"method"] isEqual:self.expectedCloudMethod]);
    ControlledCloudRequest *request = [ControlledCloudRequest new];
    request.reply = completion;
    [self.requests addObject:request];
    return request;
}
- (void)renderCandidates {} // Keep the test independent of real panel placement.
@end

static void TestCloudCandidateScheduling() {
    CloudShortcutController *controller = [CloudShortcutController alloc];
    controller.expectedCloudMethod = @"POST";
    controller.requests = [NSMutableArray array];
    CloudShortcutSession *session = [CloudShortcutSession new];
    session.query = @{@"scheme":@0, @"generation":@1, @"identity":@"synthetic", @"query_text":@"nihao", @"cache_key":@"nihao", @"pinyin_segments":@[@"ni", @"hao"], @"cloud_eligible":@YES, @"ai_eligible":@NO, @"cloud_candidates":@YES, @"session_id":@1};
    NSMutableDictionary *customQuery = [session.query mutableCopy];
    customQuery[@"cloud_request"] = @{ @"url": @"https://cloud.example.invalid/api", @"method": @"POST" };
    session.query = customQuery;
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
            @"tokens":@{@"https://synthetic.invalid:443":@"synthetic-secret"}, @"candidate_limit":@3,
            @"prompt_id":@"custom_2", @"prompt_custom_2":@"synthetic prompt"},
        @"input":@{@"segmented_pinyin":@[@"ni", @"hao"], @"context":@"", @"candidate_limit":@3}} error:&bridgeError];
    if (!descriptor || bridgeError || ![descriptor[@"timeout_ms"] isEqual:@8000]) {
        // 只打印固定格式的结果：描述符里还带着 bearer token，不能整个打出来。
        fprintf(stderr, "AI descriptor present=%d error=%s timeout=%s\n", descriptor != nil,
            (bridgeError.localizedDescription ?: @"").UTF8String,
            ([descriptor[@"timeout_ms"] description] ?: @"").UTF8String);
    }
    assert(descriptor);
    assert(!bridgeError);
    assert([descriptor[@"timeout_ms"] isEqual:@8000]);
    assert([descriptor[@"headers"][@"Authorization"] isEqual:@"Bearer synthetic-secret"]);
    // 标签是 deepseek、接口地址在别家：厂商字段不外发。
    assert(descriptor[@"body"][@"thinking"] == nil);
    // 真实 DeepSeek 地址：默认档写 disabled；开 high 档写 enabled 加 reasoning_effort，预算放大到 30 秒。
    descriptor = [MSIMEClientSession aiHTTPRequest:@{
        @"config":@{@"enabled":@YES, @"provider":@"deepseek",
            @"endpoint":@"https://api.deepseek.com/chat/completions", @"model":@"synthetic",
            @"tokens":@{@"https://api.deepseek.com:443":@"synthetic-secret"}, @"candidate_limit":@3},
        @"input":@{@"segmented_pinyin":@[@"ni"], @"context":@"", @"candidate_limit":@3}} error:&bridgeError];
    assert(descriptor && !bridgeError);
    assert([descriptor[@"body"][@"thinking"][@"type"] isEqual:@"disabled"]);
    assert([descriptor[@"timeout_ms"] isEqual:@8000]);
    descriptor = [MSIMEClientSession aiHTTPRequest:@{
        @"config":@{@"enabled":@YES, @"provider":@"deepseek", @"thinking":@"high",
            @"endpoint":@"https://api.deepseek.com/chat/completions", @"model":@"synthetic",
            @"tokens":@{@"https://api.deepseek.com:443":@"synthetic-secret"}, @"candidate_limit":@3},
        @"input":@{@"segmented_pinyin":@[@"ni"], @"context":@"", @"candidate_limit":@3}} error:&bridgeError];
    assert(descriptor && !bridgeError);
    assert([descriptor[@"body"][@"thinking"][@"type"] isEqual:@"enabled"]);
    assert([descriptor[@"body"][@"reasoning_effort"] isEqual:@"high"]);
    assert([descriptor[@"body"][@"max_tokens"] isEqual:@8192]);
    assert([descriptor[@"timeout_ms"] isEqual:@30000]);
    NSData *response = [NSJSONSerialization dataWithJSONObject:@{@"choices":@[@{@"message":@{@"content":
        @"{\"candidates\":[{\"text\":\"合成候选甲\"},{\"text\":\"合成候选乙\"}]}"}}]} options:0 error:nil];
    NSArray *parsed = [MSIMEClientSession parseAIResponse:response limit:3 error:&bridgeError];
    assert(!bridgeError && parsed.count == 2);
    NSString *root = [NSTemporaryDirectory() stringByAppendingPathComponent:NSUUID.UUID.UUIDString];
    NSMutableDictionary *options = [@{@"api_version":@1, @"preferences":@{@"scheme":@"quanpin", @"learning":@NO, @"cloud_candidates":@NO,
        @"candidate_page_size":@5, @"chinese_punctuation":@YES, @"default_ime_mode":@"chinese",
        @"ai_assistant":@{@"enabled":@YES, @"provider":@"openai", @"endpoint":@"https://synthetic.invalid/chat",
            @"model":@"synthetic", @"tokens":@{@"https://synthetic.invalid:443":@"synthetic-private"},
            @"candidate_limit":@3}}} mutableCopy];
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
    assert(!query[@"ai_assistant"][@"tokens"]);
    NSDictionary *sessionDescriptor = [session aiRequestForQuery:query error:&error];
    assert(sessionDescriptor && !error &&
        [sessionDescriptor[@"headers"][@"Authorization"] isEqual:@"Bearer synthetic-private"]);
    NSDictionary *applied = [session applyOnlineCandidates:parsed source:1 query:query error:&error];
    assert(!error && [applied[@"applied"] boolValue]);
    NSArray *candidates = applied[@"view"][@"candidates"];
    NSPredicate *isAI = [NSPredicate predicateWithBlock:^BOOL(NSDictionary *candidate, NSDictionary *_) {
        return [candidate[@"source"] isEqual:@3];
    }];
    NSArray *aiCandidates = [candidates filteredArrayUsingPredicate:isAI];
    assert(aiCandidates.count == 2 && [aiCandidates[0][@"text"] isEqual:@"合成候选甲"]
        && [aiCandidates[1][@"text"] isEqual:@"合成候选乙"]);
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
    largeQuery[@"synthetic"] = [@"x" stringByPaddingToLength:MSIME_ONLINE_QUERY_MAX_BYTES + 1 withString:@"x" startingAtIndex:0];
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
    // 云候选在本地没有词时也保留固定席位；回包不能让候选栏从无到有或改变本地候选的编号。
    NSArray *candidates = view[@"candidates"];
    assert([candidates isKindOfClass:NSArray.class] && candidates.count == 1);
    assert([candidates[0][@"text"] isEqual:@"云端测试候选"] && [candidates[0][@"source"] isEqual:@2]);
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
    assert(!prefs.cloudCandidates && toggle.state == NSControlStateValueOff);
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
    assert(!fresh.cloudCandidates);
    [fresh applySharedInputPreferences:@{@"cloud_candidates":@YES}];
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

    // 没有偏好目录、从未判断过的配置算已回答，但没有任何已存选择时和共享默认值一样不发送。
    NSString *suite = [@"msime.cloud.consent." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    MSIMEAppearancePreferences *prefs = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    [prefs resolveCloudCandidatesConsentWithPreferencesDirectory:nil userDataDirectory:nil];
    assert(prefs.cloudCandidatesAnswered && !prefs.cloudCandidates && !prefs.cloudCandidatesEnabled);

    // Fresh profile: nothing is sent and the prompt is requested exactly once.
    [prefs resolveCloudCandidatesConsentWithPreferencesDirectory:root userDataDirectory:nil];
    assert(!prefs.cloudCandidatesAnswered && !prefs.cloudCandidates && !prefs.cloudCandidatesEnabled);
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

    // 打过字但从没改过设置的升级配置：没有 preferences.json，也没有已存选择，只有 Engine 用户数据。算已回答、不再询问，没有可沿用的值，按共享默认值关闭。
    assert([NSData.data writeToFile:[userData stringByAppendingPathComponent:@"msime_user.db"] atomically:YES]);
    suite = [@"msime.cloud.consent." stringByAppendingString:NSUUID.UUID.UUIDString];
    defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    prefs = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    [prefs resolveCloudCandidatesConsentWithPreferencesDirectory:bare userDataDirectory:userData];
    assert(prefs.cloudCandidatesAnswered && !prefs.cloudCandidates && !prefs.cloudCandidatesEnabled);
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
// The generation of the same page; 0 reads as 1. A highlight move or a late cloud candidate advances it without changing the words.
@property(nonatomic) uint64_t pageGeneration;
@end
@implementation GlossSession
- (NSDictionary *)translationQueryWithError:(NSError **)error { (void)error; return self.enabled ? @{@"generation":@(self.pageGeneration ?: 1), @"target_language":self.targetLanguage ?: @"en"} : nil; }
- (NSDictionary *)viewWithError:(NSError **)error { (void)error; return @{@"generation":@(self.pageGeneration ?: 1), @"scheme":@(self.scheme), @"local_mode":self.localMode ?: @"none", @"candidates":@[@{@"text":@"hello", @"source":@4}]}; }
- (NSDictionary *)hostOptions { return @{@"resources":@"/synthetic"}; }
- (NSDictionary *)applyTranslations:(NSArray *)translations generation:(uint64_t)generation error:(NSError **)error {
    (void)error; assert(NSThread.isMainThread && generation == (self.pageGeneration ?: 1) && translations.count <= 1);
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

@interface StaleGlossSnapshotSession : ServiceSnapshotSession
@end
@implementation StaleGlossSnapshotSession
- (NSDictionary *)translationQueryWithError:(NSError **)error {
    (void)error;
    return @{ @"generation": @3, @"target_language": @"en", @"target_languages": @[ @"en" ],
        @"translation_account": @NO };
}
- (NSDictionary *)viewWithError:(NSError **)error {
    (void)error;
    return @{ @"generation": @3, @"scheme": @0, @"local_mode": @"none",
        @"candidates": @[ @{ @"text": @"hello", @"source": @4 } ] };
}
- (NSDictionary *)applyTranslations:(NSArray *)translations generation:(uint64_t)generation error:(NSError **)error {
    (void)translations; (void)generation; (void)error;
    ++self.applications;
    return @{ @"applied": @YES, @"view": [self viewWithError:nil] };
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

static void TestEmptyGlossDoesNotApplyAStaleServiceGeneration() {
    NSString *suite = [@"msime.empty-gloss-generation." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    MSIMEAppearancePreferences *appearance = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    appearance.candidateTranslations = YES;
    appearance.candidateEnglishGloss = YES;
    StaleGlossSnapshotSession *session = [StaleGlossSnapshotSession new];
    MSIMEInputController *controller = [MSIMEInputController alloc];
    [controller setValue:appearance forKey:@"appearance"];
    [controller setValue:session forKey:@"session"];
    [controller setValue:[ShortcutClient new] forKey:@"activeClient"];
    [controller setValue:@NO forKey:@"glossEnabled"];
    [controller setValue:@{ @"generation": @2, @"scheme": @0, @"local_mode": @"none",
        @"candidates": @[ @{ @"text": @"hello", @"source": @4 } ] } forKey:@"view"];
    [controller applyCandidateTranslationResults];
    assert(session.applications == 0);
    NSDictionary *view = [controller valueForKey:@"view"];
    assert([view[@"generation"] integerValue] == 2);
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
// Every generation an apply reached, when set.
@property(nonatomic, strong) NSMutableArray<NSNumber *> *appliedGenerations;
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
    // 和真实会话（msime_client_apply_translations）一样：任何一条释义带控制字符（Unicode Cc，"\n" 也算）就整批拒收，什么也不交付。
    NSMutableCharacterSet *controls = [NSMutableCharacterSet characterSetWithRange:NSMakeRange(0, 0x20)];
    [controls addCharactersInRange:NSMakeRange(0x7F, 0x21)];
    for (NSDictionary *entry in translations)
        if ([entry[@"translation"] rangeOfCharacterFromSet:controls].location != NSNotFound) return nil;
    [self.appliedGenerations addObject:@(generation)];
    self.delivered = translations;
    return @{@"applied":@YES, @"view":[self viewWithError:nil]};
}
@end
@interface ControlledTranslationBatch : MSIMECustomTranslationBatch
// Stores the completion; reading it back returns a block that replays one response the way the real batch does: onReply for every item, then the completion, or the detach block once detached.
@property(nonatomic, copy) void (^reply)(NSArray *);
@property(nonatomic, copy) NSArray *items;
@property(nonatomic, copy) NSDictionary *tencentConfig;
@property(nonatomic, copy) NSDictionary *niuTransConfig;
@property(nonatomic) BOOL started;
@property(nonatomic) BOOL cancelled;
@property(nonatomic) BOOL detached;
@property(nonatomic) BOOL finished;
@property(nonatomic, copy) void (^detachedCompletion)(void);
@end
@implementation ControlledTranslationBatch
- (void)start { assert(!self.started); self.started = YES; }
// Keeps onReply deliberately, to simulate a reply already enqueued when the controller cancelled.
- (void)cancel { self.cancelled = YES; }
- (BOOL)detachWithCompletion:(void (^)(void))completion {
    if (!self.started || self.cancelled || self.finished) { self.cancelled = YES; return NO; }
    self.detached = YES;
    self.detachedCompletion = completion;
    return YES;
}
- (void (^)(NSArray *))reply {
    void (^completion)(NSArray *) = _reply;
    __weak ControlledTranslationBatch *weakSelf = self;
    return ^(NSArray *results) {
        ControlledTranslationBatch *batch = weakSelf;
        batch.finished = YES;
        if (batch.onReply) batch.onReply(results, [batch.items valueForKey:@"text"] ?: @[]);
        if (!batch.detached) { if (completion) completion(results); }
        else if (batch.detachedCompletion) batch.detachedCompletion();
    };
}
@end
@interface AIShortcutSession : ShortcutSession
@property(nonatomic, copy) NSDictionary *query;
@property(nonatomic) NSUInteger applications;
@property(nonatomic) NSUInteger descriptorRequests;
@property(nonatomic) BOOL rejectNextAI;
@property(nonatomic, copy) NSString *authToken;
// resolveOnlineCandidatesForSource: 被调用的次数。共享内核已经不放 AI 占位行，Host API 对它总是答 applied:NO，这里也一样。
@property(nonatomic) NSUInteger resolutions;
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
        @"headers": @{ @"Content-Type": @"application/json", @"Authorization":
            [@"Bearer " stringByAppendingString:self.authToken ?: @"synthetic"] },
        @"body": @{ @"messages": @[@{ @"role": @"system", @"content": @"synthetic" },
            @{ @"role": @"user", @"content": context }] }, @"timeout_ms": @8000,
        @"connect_timeout_ms": @2500, @"max_response_bytes": @1048576 };
}
- (NSDictionary *)applyOnlineCandidates:(NSArray<NSString *> *)candidates source:(uint8_t)source
                                  query:(NSDictionary *)query error:(NSError **)error {
    (void)error;
    assert(source == 1 && [query isEqual:self.query] && candidates.count <= 1);
    // Host API 不收空批次。
    if (!candidates.count) return @{ @"applied": @NO, @"view": @{} };
    if (self.rejectNextAI) {
        self.rejectNextAI = NO;
        return @{ @"applied": @NO, @"view": @{} };
    }
    ++self.applications;
    NSMutableDictionary *updated = [self.query mutableCopy];
    updated[@"generation"] = @([updated[@"generation"] unsignedLongLongValue] + 1);
    self.query = updated;
    return @{ @"applied": @YES, @"view": @{ @"focused": @YES, @"editing_text": @"synthetic",
        @"candidates": @[@{ @"text": @"合成候选", @"source": @3 }] } };
}
- (NSDictionary *)resolveOnlineCandidatesForSource:(NSUInteger)source query:(NSDictionary *)query error:(NSError **)error {
    (void)error;
    assert(source == 1 && [query isEqual:self.query]);
    ++self.resolutions;
    return @{ @"applied": @NO, @"view": @{} };
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
// 真实控制器、Rust 会话与合成编辑器覆盖普通英文、专用英文和临时英文；只替换网络传输。
static void TestEnglishOnlineCandidatesThroughRealSession() {
    for (NSUInteger entry = 0; entry < 3; ++entry) {
        @autoreleasepool {
            NSString *root = [NSTemporaryDirectory() stringByAppendingPathComponent:NSUUID.UUID.UUIDString];
            NSMutableDictionary *preferences = [@{@"scheme": @"quanpin", @"default_ime_mode": @"chinese",
                @"candidate_page_size": @5, @"learning": @NO, @"chinese_punctuation": @NO,
                @"cloud_candidates": @YES, @"candidate_translations": @NO,
                @"english_online_candidates": @(entry != 0),
                @"ai_assistant": @{@"enabled": @YES, @"provider": @"openai", @"model": @"synthetic",
                    @"endpoint": @"https://synthetic.invalid/chat", @"candidate_limit": @3,
                    @"tokens": @{@"https://synthetic.invalid:443": @"synthetic-token"}}} mutableCopy];
            NSMutableDictionary *options = [@{@"api_version": @1, @"preferences": preferences} mutableCopy];
            for (NSString *name in @[@"resources", @"user_data", @"cache", @"dictionaries"]) {
                NSString *path = [root stringByAppendingPathComponent:name];
                assert([NSFileManager.defaultManager createDirectoryAtPath:path withIntermediateDirectories:YES attributes:nil error:nil]);
                options[name] = path;
                if ([name isEqual:@"resources"] || [name isEqual:@"dictionaries"]) {
                    sqlite3 *database = nullptr;
                    assert(sqlite3_open([[path stringByAppendingPathComponent:@"msime-english.db"] fileSystemRepresentation], &database) == SQLITE_OK);
                    assert(sqlite3_exec(database, "CREATE TABLE english_words(word TEXT,display TEXT,weight INTEGER);", nullptr, nullptr, nullptr) == SQLITE_OK);
                    assert(sqlite3_exec(database, "INSERT INTO english_words VALUES('wording','wording',100);", nullptr, nullptr, nullptr) == SQLITE_OK);
                    sqlite3_close(database);
                }
            }
            NSError *error = nil;
            MSIMEClientSession *session = [[MSIMEClientSession alloc] initWithOptions:options error:&error];
            assert(session && !error);
            NSDictionary *view = [session setFocused:YES error:&error][@"view"];
            assert(view && !error);
            NSString *suite = [@"msime.english-online." stringByAppendingString:NSUUID.UUID.UUIDString];
            NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
            MSIMEAppearancePreferences *appearance = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults skinsRoot:[NSURL fileURLWithPath:root]];
            [appearance applySharedInputPreferences:preferences];
            appearance.cloudCandidates = YES;
            ShortcutClient *client = [ShortcutClient new];
            client.document = @"";
            client.selection = NSMakeRange(0, 0);
            AIShortcutController *controller = [AIShortcutController alloc];
            controller.aiBatches = [NSMutableArray array];
            controller.requests = [NSMutableArray array];
            [controller setValue:appearance forKey:@"appearance"];
            [controller setValue:session forKey:@"session"];
            [controller setValue:client forKey:@"activeClient"];
            [controller setValue:view forKey:@"view"];
            [controller setValue:[[HiddenCandidatePanel alloc] init] forKey:@"panel"];
            [controller applySharedToolbarPreferences:preferences];
            auto type = [&](NSString *text) {
                for (NSUInteger index = 0; index < text.length; ++index) {
                    NSString *character = [text substringWithRange:NSMakeRange(index, 1)];
                    assert([controller handleEvent:LayoutKey(0, character, 0) client:client]);
                }
            };
            if (entry == 0) {
                [controller setEnglishInputMode:YES];
                assert(![controller handleEvent:LayoutKey(0, @"x", 0) client:client]);
                assert(![controller valueForKey:@"aiTimer"] && ![session onlineQueryWithError:nil]);
                preferences[@"english_online_candidates"] = @YES;
                assert(([session updatePreferencesSnapshot:@{@"format_version": @1, @"revision": @1, @"preferences": preferences} error:&error] && !error));
                [controller applySharedToolbarPreferences:preferences];
                assert([[controller valueForKey:@"managedEnglishComposition"] boolValue]);
            } else if (entry == 1) {
                [controller setDedicatedEnglishInputMode:YES];
            } else {
                assert([controller handleEvent:LayoutKey(16, @"Y", NSEventModifierFlagShift) client:client]);
            }
            type(@"alpha");
            [controller apply:[session command:MSIME_COMMIT_RAW error:nil]];
            if (entry == 2) assert([controller handleEvent:LayoutKey(16, @"Y", NSEventModifierFlagShift) client:client]);
            type(@"accessability");
            NSDictionary *query = [session onlineQueryWithError:&error];
            assert(!error && [query[@"mode"] isEqual:@"english"] && [query[@"query_text"] isEqual:@"accessability"]);
            assert([query[@"ai_context"] containsString:@"alpha"]);
            assert(![query[@"cloud_candidates"] boolValue] && ![query[@"cloud_request"] isKindOfClass:NSDictionary.class]);
            assert(![controller valueForKey:@"cloudTimer"] && controller.requests.count == 0);
            NSTimer *timer = [controller valueForKey:@"aiTimer"];
            assert(timer && controller.aiBatches.count == 0);
            controller.secureInput = YES;
            [controller synchronizeAITranslations];
            [timer fire];
            assert(!timer.valid && controller.aiBatches.count == 0);
            controller.secureInput = NO;
            [controller synchronizeAITranslations];
            timer = [controller valueForKey:@"aiTimer"];
            assert(timer);
            [timer fire]; [timer invalidate];
            assert(controller.aiBatches.count == 1 && controller.aiBatches[0].started);
            NSDictionary *body = controller.aiBatches[0].items[0][@"request"][@"body"];
            NSData *payloadBytes = [body[@"messages"][1][@"content"] dataUsingEncoding:NSUTF8StringEncoding];
            NSDictionary *payload = [NSJSONSerialization JSONObjectWithData:payloadBytes options:0 error:nil];
            assert([payload[@"mode"] isEqual:@"english"] && [payload[@"input"] isEqual:@"accessability"]);
            controller.aiBatches[0].reply(@[@{@"translation": @"accessibility"}]);
            NSDictionary *after = [session viewWithError:nil];
            NSArray *rows = after[@"candidates"];
            NSUInteger answer = [rows indexOfObjectPassingTest:^BOOL(NSDictionary *row, NSUInteger index, BOOL *stop) {
                (void)index; (void)stop; return [row[@"text"] isEqual:@"accessibility"];
            }];
            assert(answer != NSNotFound && answer == MIN((NSUInteger)4, rows.count - 1));
            [controller apply:[session selectGeneration:[after[@"generation"] unsignedLongLongValue] index:answer error:&error]];
            assert(!error && [client.document hasSuffix:@"accessibility"]);
            if (entry == 0) {
                type(@"word");
                if (![controller handleEvent:LayoutKey(49, @" ", 0) client:client])
                    [client insertText:@" " replacementRange:NSMakeRange(NSNotFound, NSNotFound)];
                assert([client.document hasSuffix:@"wording "]);
                type(@"word");
                [controller commitComposition:client];
                assert([client.document hasSuffix:@"word"] && client.marked.length == 0);
                type(@"word");
                Method base = class_getInstanceMethod(IMKInputController.class, @selector(deactivateServer:));
                IMP original = method_setImplementation(base, (IMP)RecordBaseDeactivation);
                [controller deactivateServer:client];
                method_setImplementation(base, original);
                assert([client.document hasSuffix:@"wordword"] && client.marked.length == 0);
                [controller setValue:client forKey:@"activeClient"];
                [controller apply:[session setFocused:YES error:nil]];
                type(@"token");
                if (![controller handleEvent:LayoutKey(47, @".", 0) client:client])
                    [client insertText:@"." replacementRange:NSMakeRange(NSNotFound, NSNotFound)];
                assert([client.document hasSuffix:@"token."]);
            }
            if (entry == 2) assert([controller handleEvent:LayoutKey(16, @"Y", NSEventModifierFlagShift) client:client]);
            type(@"accessability");
            timer = [controller valueForKey:@"aiTimer"];
            assert(timer);
            preferences[@"english_online_candidates"] = @NO;
            assert(([session updatePreferencesSnapshot:@{@"format_version": @1, @"revision": @2, @"preferences": preferences} error:&error] && !error));
            [controller applySharedToolbarPreferences:preferences];
            assert(![session onlineQueryWithError:nil] && !timer.valid);
            [timer fire];
            assert(controller.aiBatches.count == 1);
            if (entry == 0) {
                assert([client.document hasSuffix:@"accessability"] && client.marked.length == 0);
                assert(![[controller valueForKey:@"managedEnglishComposition"] boolValue]);
                assert(![controller handleEvent:LayoutKey(0, @"x", 0) client:client]);
            }
            [controller cancelAITranslations];
            [controller cancelCloudCandidates];
            if (entry == 0) {
                preferences[@"english_online_candidates"] = @YES;
                assert(([session updatePreferencesSnapshot:@{@"format_version": @1, @"revision": @3, @"preferences": preferences} error:&error] && !error));
                [controller applySharedToolbarPreferences:preferences];
                type(@"pending");
                std::string lease;
                const std::string userData = [options[@"user_data"] fileSystemRepresentation];
                assert(msime::dictionary_lease::raise_dictionary_quiesce_lease(userData, lease));
                [controller releaseDictionarySessionIfQuiesced];
                assert(![controller valueForKey:@"session"]);
                assert(![[controller valueForKey:@"resumeDedicatedEnglish"] boolValue]);
                assert(![[controller valueForKey:@"managedEnglishComposition"] boolValue]);
                assert([client.document hasSuffix:@"pending"] && !client.marked.length);
                msime::dictionary_lease::lower_dictionary_quiesce_lease(userData, lease);
            } else {
                assert([session closeWithError:&error] && !error);
            }
            MSIMERemoveTestPreferenceSuite(defaults, suite);
            [NSFileManager.defaultManager removeItemAtPath:root error:nil];
        }
    }
}

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

// 输入法在事件跟踪模式中停留时也要启动 AI 请求，不能让 AI 回答一直等一个未发出的请求。
static void TestAiCandidateTimerFiresWhileTracking() {
    AIShortcutController *controller = [AIShortcutController alloc];
    controller.aiBatches = [NSMutableArray array];
    AIShortcutSession *session = [AIShortcutSession new];
    session.query = @{ @"scheme": @0, @"generation": @1, @"identity": @"synthetic-ai-tracking",
        @"query_text": @"nihao", @"cache_key": @"nihao-tracking", @"pinyin_segments": @[@"ni", @"hao"],
        @"ai_context": @"", @"cloud_eligible": @NO, @"ai_eligible": @YES, @"cloud_candidates": @YES,
        @"ai_assistant": @{ @"enabled": @YES, @"candidate_limit": @3 }, @"session_id": @1 };
    [controller setValue:session forKey:@"session"];
    [controller setValue:[ShortcutClient new] forKey:@"activeClient"];
    [controller synchronizeAITranslations];
    NSTimer *timer = [controller valueForKey:@"aiTimer"];
    assert(timer);
    timer.fireDate = [NSDate dateWithTimeIntervalSinceNow:0.01];
    CFRunLoopAddCommonMode(CFRunLoopGetMain(), (__bridge CFStringRef)NSEventTrackingRunLoopMode);
    NSDate *deadline = [NSDate dateWithTimeIntervalSinceNow:1];
    while (!controller.aiBatches.count && deadline.timeIntervalSinceNow > 0)
        [NSRunLoop.mainRunLoop runMode:NSEventTrackingRunLoopMode beforeDate:deadline];
    assert(controller.aiBatches.count == 1 && controller.aiBatches[0].started);
    assert([controller valueForKey:@"aiTimer"] == nil);
    controller.aiBatches[0].reply(@[]);
    assert(session.resolutions == 1);
    [controller synchronizeAITranslations];
    assert([controller valueForKey:@"aiTimer"] == nil && controller.aiBatches.count == 1);
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

// #6888：回答为空时宿主照旧调用 resolve，Host API 答 applied:NO，候选不变；同一组字不再重发请求，组字变了才再问。
static void TestAnEmptyAiAnswerIsNotAskedAgain() {
    AIShortcutController *controller = [AIShortcutController alloc];
    controller.aiBatches = [NSMutableArray array];
    AIShortcutSession *session = [AIShortcutSession new];
    session.query = @{ @"scheme": @0, @"generation": @1, @"identity": @"synthetic-ai-empty",
        @"query_text": @"nihao", @"cache_key": @"nihao-empty", @"pinyin_segments": @[@"ni", @"hao"],
        @"ai_context": @"", @"cloud_eligible": @NO, @"ai_eligible": @YES, @"cloud_candidates": @YES,
        @"ai_assistant": @{ @"enabled": @YES, @"candidate_limit": @3 }, @"session_id": @1 };
    [controller setValue:session forKey:@"session"];
    [controller setValue:[ShortcutClient new] forKey:@"activeClient"];
    NSDictionary *local = @{ @"candidates": @[@{ @"text": @"你好", @"source": @0 }, @{ @"text": @"倪好", @"source": @0 }] };
    [controller setValue:local forKey:@"view"];
    [controller synchronizeAITranslations];
    [(NSTimer *)[controller valueForKey:@"aiTimer"] fire];
    assert(controller.aiBatches.count == 1);
    controller.aiBatches[0].reply(@[]);
    assert(session.applications == 0 && session.resolutions == 1);
    assert([[controller valueForKey:@"view"] isEqual:local]);
    // 同一组字：查询身份还在，不再发请求。
    assert([controller valueForKey:@"aiQuery"] != nil);
    [controller synchronizeAITranslations];
    assert([controller valueForKey:@"aiTimer"] == nil && session.descriptorRequests == 1);
    // 组字变了：重新问。
    NSMutableDictionary *next = [session.query mutableCopy];
    next[@"generation"] = @2;
    next[@"cache_key"] = @"nihaoa-empty";
    session.query = next;
    [controller synchronizeAITranslations];
    assert([controller valueForKey:@"aiTimer"] != nil);
    [controller cancelAITranslations];
}

// 视图里的 source 是 Engine 的 CandidateSource：用户词库的词是 1，AI 候选才是 3。候选里有用户词时缓存的 AI 回答照样立刻换上（#6888）。
static void TestAiCacheIgnoresUserDictionaryRows() {
    AIShortcutController *controller = [AIShortcutController alloc];
    controller.aiBatches = [NSMutableArray array];
    AIShortcutSession *session = [AIShortcutSession new];
    session.query = @{ @"scheme": @0, @"generation": @1, @"identity": @"synthetic-ai-user-row",
        @"query_text": @"nihao", @"cache_key": @"nihao-user-row", @"pinyin_segments": @[@"ni", @"hao"],
        @"ai_context": @"", @"cloud_eligible": @NO, @"ai_eligible": @YES, @"cloud_candidates": @YES,
        @"ai_assistant": @{ @"enabled": @YES, @"candidate_limit": @3 }, @"session_id": @1 };
    [controller setValue:session forKey:@"session"];
    [controller setValue:[ShortcutClient new] forKey:@"activeClient"];
    [controller synchronizeAITranslations];
    [(NSTimer *)[controller valueForKey:@"aiTimer"] fire];
    controller.aiBatches[0].reply(@[@{ @"translation": @"缓存候选" }]);
    assert(session.applications == 1);
    NSMutableDictionary *next = [session.query mutableCopy];
    next[@"generation"] = @([next[@"generation"] unsignedLongLongValue] + 1);
    session.query = next;
    [controller setValue:@{ @"candidates": @[@{ @"text": @"用户词", @"source": @1 },
        @{ @"text": @"本地", @"source": @0 }] } forKey:@"view"];
    [controller synchronizeAITranslations];
    assert(session.applications == 2 && session.descriptorRequests == 2);
    // 已经坐着 AI 候选时不再重放缓存。
    next = [session.query mutableCopy];
    next[@"generation"] = @([next[@"generation"] unsignedLongLongValue] + 1);
    session.query = next;
    [controller setValue:@{ @"candidates": @[@{ @"text": @"缓存候选", @"source": @3 }] } forKey:@"view"];
    [controller synchronizeAITranslations];
    assert(session.applications == 2);
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
    assert(session.applications == 2 && session.descriptorRequests == 2 && controller.aiBatches.count == 1);
    NSMutableDictionary *changedContext = [session.query mutableCopy];
    changedContext[@"generation"] = @([changedContext[@"generation"] unsignedLongLongValue] + 1);
    changedContext[@"ai_context"] = @"另一个上下文";
    session.query = changedContext;
    [controller setValue:@{ @"candidates": @[@{ @"text": @"普通候选", @"source": @0 }] } forKey:@"view"];
    [controller synchronizeAITranslations];
    timer = [controller valueForKey:@"aiTimer"];
    assert(timer && controller.aiBatches.count == 1);
    [timer fire];
    assert(controller.aiBatches.count == 2 && session.descriptorRequests == 3);
    [controller cancelAITranslations];
}

static void TestAiCandidateCacheDoesNotSurviveCredentialRotation() {
    AIShortcutController *controller = [AIShortcutController alloc];
    controller.aiBatches = [NSMutableArray array];
    AIShortcutSession *session = [AIShortcutSession new];
    session.authToken = @"synthetic-first";
    session.query = @{ @"scheme": @0, @"generation": @1, @"identity": @"synthetic-ai-rotation",
        @"query_text": @"nihao", @"cache_key": @"nihao-rotation", @"pinyin_segments": @[@"ni", @"hao"],
        @"ai_context": @"", @"cloud_eligible": @NO, @"ai_eligible": @YES,
        @"cloud_candidates": @YES, @"ai_assistant": @{ @"enabled": @YES, @"candidate_limit": @3 },
        @"session_id": @1 };
    [controller setValue:session forKey:@"session"];
    [controller setValue:[ShortcutClient new] forKey:@"activeClient"];
    [controller synchronizeAITranslations];
    [(NSTimer *)[controller valueForKey:@"aiTimer"] fire];
    controller.aiBatches[0].reply(@[@{ @"translation": @"旧凭据候选" }]);
    assert(session.applications == 1 && session.descriptorRequests == 1);

    session.authToken = @"synthetic-second";
    NSMutableDictionary *next = [session.query mutableCopy];
    next[@"generation"] = @([next[@"generation"] unsignedLongLongValue] + 1);
    session.query = next;
    [controller setValue:@{ @"candidates": @[@{ @"text": @"本地候选", @"source": @0 }] } forKey:@"view"];
    [controller synchronizeAITranslations];
    assert(session.applications == 1 && session.descriptorRequests == 2);
    NSTimer *timer = [controller valueForKey:@"aiTimer"];
    assert(timer && controller.aiBatches.count == 1);
    [timer fire];
    assert(controller.aiBatches.count == 2);
    [controller cancelAITranslations];
}

static void TestAiCandidateCacheTracksProviderSettings() {
    AIShortcutController *controller = [AIShortcutController alloc];
    controller.aiBatches = [NSMutableArray array];
    AIShortcutSession *session = [AIShortcutSession new];
    session.query = @{ @"scheme": @0, @"generation": @1, @"identity": @"synthetic-ai-settings",
        @"query_text": @"nihao", @"cache_key": @"nihao-settings", @"pinyin_segments": @[@"ni", @"hao"],
        @"ai_context": @"设置缓存", @"cloud_eligible": @NO, @"ai_eligible": @YES,
        @"cloud_candidates": @YES, @"ai_assistant": @{ @"enabled": @YES, @"provider": @"openai",
            @"endpoint": @"https://synthetic.invalid/chat", @"model": @"synthetic",
            @"candidate_limit": @3, @"prompt_custom_1": @"旧提示" }, @"session_id": @1 };
    [controller setValue:session forKey:@"session"];
    [controller setValue:[ShortcutClient new] forKey:@"activeClient"];
    [controller synchronizeAITranslations];
    NSTimer *timer = [controller valueForKey:@"aiTimer"];
    [timer fire];
    controller.aiBatches[0].reply(@[@{ @"translation": @"缓存候选" }]);
    assert(session.applications == 1 && session.descriptorRequests == 1);

    NSMutableDictionary *next = [session.query mutableCopy];
    next[@"generation"] = @([next[@"generation"] unsignedLongLongValue] + 1);
    next[@"ai_assistant"] = @{ @"enabled": @YES, @"provider": @"openai",
        @"endpoint": @"https://synthetic.invalid/chat", @"model": @"synthetic",
        @"candidate_limit": @5, @"prompt_custom_1": @"新提示" };
    session.query = next;
    [controller setValue:@{ @"candidates": @[@{ @"text": @"普通候选", @"source": @0 }] } forKey:@"view"];
    [controller synchronizeAITranslations];
    timer = [controller valueForKey:@"aiTimer"];
    assert(timer && controller.aiBatches.count == 1);
    [timer fire];
    assert(controller.aiBatches.count == 2 && session.descriptorRequests == 2);
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
@property(nonatomic, strong) NSMutableArray<NSArray *> *accountFetches;
// Times the controller dropped the backend's waiting account page.
@property(nonatomic) NSUInteger accountQueueCancels;
// Packaged English dictionary answers beyond Hello's.
@property(nonatomic, copy) NSDictionary<NSString *, NSString *> *extraEnglishGlosses;
@property(nonatomic) BOOL useRealDelay;
// A real timer that never comes due on its own, for a test that fires it by hand: asserting that a 0.5 s timer has not fired yet depends on how busy the machine is.
@property(nonatomic) BOOL holdDelay;
@end
@implementation CustomTranslationController
- (void)fetchOnDeviceGlosses:(NSArray<NSString *> *)words targets:(NSArray<NSString *> *)targets {
    assert(NSThread.isMainThread);
    [self.onDeviceFetches addObject:@[words, targets]];
}
- (void)fetchAccountGlosses:(NSArray<NSString *> *)words primary:(NSString *)primary secondary:(NSString *)secondary
                 generation:(uint64_t)generation {
    assert(NSThread.isMainThread);
    [self.accountFetches addObject:@[words, primary, secondary, @(generation)]];
}
- (void)cancelQueuedAccountGlosses {
    assert(NSThread.isMainThread);
    self.accountQueueCancels++;
}
- (MSIMECustomTranslationBatch *)niuTransBatchForItems:(NSArray<NSDictionary *> *)items config:(NSDictionary *)config
                                           completion:(void (^)(NSArray<NSDictionary *> *))completion {
    ControlledTranslationBatch *batch = (ControlledTranslationBatch *)[self customBatchForItems:items completion:completion];
    batch.niuTransConfig = config; return batch;
}
- (NSTimer *)customTranslationTimerWithBlock:(void (^)(NSTimer *))block {
    if (self.holdDelay) {
        NSTimer *timer = [NSTimer timerWithTimeInterval:3600 repeats:NO block:block];
        [NSRunLoop.mainRunLoop addTimer:timer forMode:NSRunLoopCommonModes];
        return timer;
    }
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
    // A reply that lands after its page has moved on is still saved, as Windows persists every fetch that completes, but it does not touch the page now on screen. A reply from work cancelled outright is not saved.
    session.targetLanguage = @"en"; session.targetLanguages = @[@"en"]; session.tencent = TencentConfig();
    session.page = @[@{@"text":@"late", @"source":@4}];
    [writer synchronizeCandidateGloss]; WaitForGloss(writer); [writer synchronizeCustomTranslations];
    ControlledTranslationBatch *late = writer.batches.lastObject;
    session.generation++; session.page = @[@{@"text":@"stale", @"source":@4}];
    [writer synchronizeCandidateGloss]; WaitForGloss(writer); [writer synchronizeCustomTranslations];
    ControlledTranslationBatch *pending = writer.batches.lastObject;
    assert(pending != late && late.detached && !late.cancelled);
    NSArray *visible = session.delivered;
    late.reply(@[@{@"text":@"late", @"translation":@"迟到释义"}]);
    assert([session.delivered isEqual:visible]);
    [writer cancelCandidateTranslations];
    assert(pending.cancelled);
    pending.reply(@[@{@"text":@"stale", @"translation":@"不应保存"}]);
    dispatch_sync([MSIMEInputController learnedTranslationQueue], ^{});
    session.tencent = nil; session.delivered = @[];
    session.page = @[@{@"text":@"late", @"source":@4}, @{@"text":@"stale", @"source":@4}];
    [reader synchronizeCandidateGloss]; WaitForGloss(reader);
    assert(([session.delivered isEqual:@[@{@"text":@"late", @"translation":@"迟到释义"}]]));
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
    // A generation that moved over the same words is not a rejection: that page still shows what the reply answers (TestGlossSurvivesHighlightMove).
    for (NSString *change in @[@"page", @"client", @"session", @"focus", @"japanese", @"disabled"]) {
        [[MSIMETranslationCache sharedCache] clear];
        [controller cancelCandidateTranslations]; session.delivered = @[];
        [controller synchronizeCustomTranslations];
        ControlledTranslationBatch *pending = controller.batches.lastObject;
        NSArray *page = session.page;
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
    NSUInteger cachedBatchCount = controller.batches.count;
    session.generation++;
    [controller synchronizeCustomTranslations];
    assert(controller.batches.count == cachedBatchCount && [session.delivered.lastObject[@"translation"] isEqual:@"test"]);
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
    // The provider changed while the request was in flight. Its late reply must
    // leave neither a gloss nor a negative entry behind.
    session.tencent = disabled; assert(![controller currentCustomTranslationRequest]);
    session.tencent = TencentConfig();
    [controller applySharedToolbarPreferences:@{@"tencent_tmt":session.tencent}];
    [controller synchronizeCandidateGloss];
    NSUInteger beforeReenable = controller.batches.count;
    [controller synchronizeCustomTranslations]; pending = controller.batches.lastObject;
    assert(controller.batches.count == beforeReenable + 1);
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
    [[MSIMETranslationCache sharedCache] clear];
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

    // A reply captured under a different login must not enter the process-wide cache after the
    // settings app switches accounts while the request is in flight.
    [fetcher accountCandidateTranslationsDidArrive:
        [NSNotification notificationWithName:@"MSIMEBackendCandidateTranslationsDidArrive" object:nil
            userInfo:@{@"generation":@1, @"marker":@1, @"target":@"en", @"translations":@{@"\u6d4b\u8bd5":@"old-account"}}]];
    assert(([[MSIMETranslationCache sharedCache] valueForIdentity:@[@"account", @"0", @"en", @"测试"]] == nil));

    // The reply carries the same payload the Swift backend posts. Delivered straight to the instance that
    // asked, because these controllers are built without the activation that registers the observer.
    [fetcher accountCandidateTranslationsDidArrive:
        [NSNotification notificationWithName:@"MSIMEBackendCandidateTranslationsDidArrive" object:nil
            userInfo:@{@"generation":@1, @"marker":@0, @"target":@"en", @"translations":@{@"\u6d4b\u8bd5":@"test"}}]];
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
            userInfo:@{@"generation":@1, @"marker":@0, @"target":@"en", @"translations":@{@"\u6d4b\u8bd5":@"ignored"}}]];
    assert(fetcherSession.applied == nil);

    // A reply for an earlier generation still answers the same word on the current page. The cache key is language and word, as on Windows, so the generation it was asked under does not make the answer wrong.
    [[MSIMETranslationCache sharedCache] clear];
    fetcherSession.generation = 2;
    [fetcher synchronizeAccountGloss:[fetcher currentAccountGlossRequest]];
    fetcherSession.applied = nil;
    [fetcher accountCandidateTranslationsDidArrive:
        [NSNotification notificationWithName:@"MSIMEBackendCandidateTranslationsDidArrive" object:nil
            userInfo:@{@"generation":@1, @"marker":@0, @"target":@"en", @"translations":@{@"\u6d4b\u8bd5":@"stale"}}]];
    assert([[[fetcher valueForKey:@"accountGlossResults"] firstObject][@"translation"] isEqual:@"stale"]);
    [fetcher accountCandidateTranslationsDidArrive:
        [NSNotification notificationWithName:@"MSIMEBackendCandidateTranslationsDidArrive" object:nil
            userInfo:@{@"generation":@2, @"marker":@0, @"target":@"en", @"translations":@{@"\u6d4b\u8bd5":@"fresh"}}]];
    assert([[[fetcher valueForKey:@"accountGlossResults"] firstObject][@"translation"] isEqual:@"fresh"]);
}

// An account reply is cached before the controller checks whether it still shows the page that asked, as Windows caches every completed fetch before its staleness check (cloud_translation.cpp), so typing past a page no longer throws its answers away. A word the account had nothing for is cached as a negative entry for eight minutes and not asked about again; a word whose request failed gets no reply at all and is asked about again.
static void TestAccountGlossLateReplyAndNegatives() {
    [[MSIMETranslationCache sharedCache] clear];
    NSString *suite = [@"msime.account-gloss-late." stringByAppendingString:NSUUID.UUID.UUIDString];
    MSIMEAppearancePreferences *prefs =
        [[MSIMEAppearancePreferences alloc] initWithDefaults:[[NSUserDefaults alloc] initWithSuiteName:suite]];
    CustomTranslationController *controller = [CustomTranslationController alloc];
    controller.batches = [NSMutableArray array];
    controller.accountFetches = [NSMutableArray array];
    AccountGlossSession *session = [AccountGlossSession new];
    [controller setValue:[ShortcutClient new] forKey:@"activeClient"];
    [controller setValue:prefs forKey:@"appearance"];
    [controller setValue:session forKey:@"session"];
    void (^reply)(NSUInteger, NSDictionary *) = ^(NSUInteger generation, NSDictionary *translations) {
        [controller accountCandidateTranslationsDidArrive:[NSNotification notificationWithName:@"MSIMEBackendCandidateTranslationsDidArrive"
            object:nil userInfo:@{@"generation":@(generation), @"marker":@0, @"target":@"en", @"translations":translations}]];
    };
    void (^show)(NSUInteger, NSArray<NSString *> *) = ^(NSUInteger generation, NSArray<NSString *> *texts) {
        NSMutableArray *candidates = [NSMutableArray array];
        for (NSString *text in texts) [candidates addObject:@{@"text":text, @"online_gloss":@YES}];
        session.generation = generation;
        session.candidates = candidates;
        [controller synchronizeAccountGloss:[controller currentAccountGlossRequest]];
    };

    show(1, @[@"测试"]);
    show(2, @[@"你好"]);
    assert(controller.accountFetches.count == 2);
    // The answer for the page the user typed past lands in the cache, but the current page, which does not show that word, is left alone.
    session.applied = nil;
    reply(1, @{@"测试":@"test"});
    assert(([[MSIMETranslationCache sharedCache] valueForIdentity:@[@"account", @"0", @"en", @"测试"]]));
    assert([[controller valueForKey:@"accountGlossResults"] count] == 0 && session.applied == nil);
    // The account had nothing for 你好: remembered as a negative entry, with nothing shown.
    reply(2, @{@"你好":@""});
    assert(([[MSIMETranslationCache sharedCache] valueForIdentity:@[@"account", @"0", @"en", @"你好"]] == NSNull.null));
    assert([[controller valueForKey:@"accountGlossResults"] count] == 0);
    // A later empty answer for a word that already has a gloss does not evict it.
    reply(1, @{@"测试":@""});
    assert(([[[MSIMETranslationCache sharedCache] valueForIdentity:@[@"account", @"0", @"en", @"测试"]] isEqual:@"test"]));

    // Backing up to a page with both words asks about neither: one is answered from the cache, the other is known to have no answer.
    show(3, @[@"测试", @"你好"]);
    assert(controller.accountFetches.count == 2);
    NSArray *results = [controller valueForKey:@"accountGlossResults"];
    assert(results.count == 1 && [results[0][@"text"] isEqual:@"测试"] && [results[0][@"translation"] isEqual:@"test"]);

    // A request that failed posts no reply, so its word stays unknown and the next page showing it asks about it again. The page in between differs, since the same words again are the same request and ask nothing.
    show(4, @[@"再见"]);
    show(5, @[@"测试"]);
    show(6, @[@"再见"]);
    assert((controller.accountFetches.count == 4 && [controller.accountFetches[3][0] isEqual:@[@"再见"]]));
    [[NSUserDefaults new] removePersistentDomainForName:suite];
    [[MSIMETranslationCache sharedCache] clear];
}

// The account is asked once typing has been idle for 500 ms, as Windows' cloud worker waits kIdleDelay after the latest job (cloud_translation.cpp): a page the user types past cancels its pending request, so fast typing costs one request rather than one per keystroke, while a cached answer still shows at once.
static void TestAccountGlossIdleDelay() {
    [[MSIMETranslationCache sharedCache] clear];
    NSString *suite = [@"msime.account-gloss-idle." stringByAppendingString:NSUUID.UUID.UUIDString];
    MSIMEAppearancePreferences *prefs =
        [[MSIMEAppearancePreferences alloc] initWithDefaults:[[NSUserDefaults alloc] initWithSuiteName:suite]];
    CustomTranslationController *controller = [CustomTranslationController alloc];
    controller.useRealDelay = YES;
    controller.batches = [NSMutableArray array];
    controller.accountFetches = [NSMutableArray array];
    AccountGlossSession *session = [AccountGlossSession new];
    [controller setValue:[ShortcutClient new] forKey:@"activeClient"];
    [controller setValue:prefs forKey:@"appearance"];
    [controller setValue:session forKey:@"session"];
    void (^show)(NSUInteger, NSArray<NSString *> *) = ^(NSUInteger generation, NSArray<NSString *> *texts) {
        NSMutableArray *candidates = [NSMutableArray array];
        for (NSString *text in texts) [candidates addObject:@{@"text":text, @"online_gloss":@YES}];
        session.generation = generation;
        session.candidates = candidates;
        [controller synchronizeAccountGloss:[controller currentAccountGlossRequest]];
    };

    // Scheduling sends nothing, and syncing the same page again keeps the timer rather than restarting it. The upper bound is the contract; the lower bound only says the timer has not fired, for the reason TestCustomTranslationIdleDelay gives.
    show(1, @[@"测试"]);
    NSTimer *first = [controller valueForKey:@"accountGlossTimer"];
    assert(first.valid && first.fireDate.timeIntervalSinceNow > 0 && first.fireDate.timeIntervalSinceNow <= 0.5);
    [controller synchronizeAccountGloss:[controller currentAccountGlossRequest]];
    assert(first == [controller valueForKey:@"accountGlossTimer"] && controller.accountFetches.count == 0);
    // The next page cancels the first page's timer; firing the old one sends nothing.
    show(2, @[@"你好"]);
    NSTimer *second = [controller valueForKey:@"accountGlossTimer"];
    assert(!first.valid && second.valid && second != first);
    [first fire];
    assert(controller.accountFetches.count == 0);
    // Only the page the user stopped on goes out, once.
    NSDate *deadline = [NSDate dateWithTimeIntervalSinceNow:2];
    while (!controller.accountFetches.count && deadline.timeIntervalSinceNow > 0)
        [NSRunLoop.mainRunLoop runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.005]];
    assert(controller.accountFetches.count == 1 && ([controller.accountFetches[0] isEqual:@[@[@"你好"], @"en", @"", @2]]));
    assert(!second.valid && ![controller valueForKey:@"accountGlossTimer"]);

    // A page whose words are all cached shows them at once and schedules nothing.
    [controller accountCandidateTranslationsDidArrive:[NSNotification notificationWithName:@"MSIMEBackendCandidateTranslationsDidArrive"
        object:nil userInfo:@{@"generation":@2, @"marker":@0, @"target":@"en", @"translations":@{@"你好":@"hello"}}]];
    show(3, @[@"你好"]);
    assert(![controller valueForKey:@"accountGlossTimer"] && controller.accountFetches.count == 1);
    NSArray *results = [controller valueForKey:@"accountGlossResults"];
    assert(results.count == 1 && [results[0][@"translation"] isEqual:@"hello"]);

    // A timer whose page lost focus or was cancelled before it fired sends nothing.
    show(4, @[@"再见"]);
    NSTimer *unfocused = [controller valueForKey:@"accountGlossTimer"];
    assert(unfocused.valid);
    [controller setValue:@YES forKey:@"focusPending"];
    [unfocused fire];
    assert(controller.accountFetches.count == 1 && ![controller valueForKey:@"accountGlossTimer"]);
    [controller setValue:@NO forKey:@"focusPending"];
    // Once focus returns, the same page is asked again rather than taken as already sent.
    [controller synchronizeAccountGloss:[controller currentAccountGlossRequest]];
    NSTimer *refocused = [controller valueForKey:@"accountGlossTimer"];
    assert(refocused.valid && refocused != unfocused);
    [refocused fire];
    assert(controller.accountFetches.count == 2 && ([controller.accountFetches[1] isEqual:@[@[@"再见"], @"en", @"", @4]]));
    // A newer page only replaces the backend's waiting page; it never drops it.
    assert(controller.accountQueueCancels == 0);
    show(5, @[@"再见"]);
    NSTimer *cancelled = [controller valueForKey:@"accountGlossTimer"];
    [controller cancelCandidateTranslations];
    [cancelled fire];
    assert(!cancelled.valid && controller.accountFetches.count == 2);
    // Leaving the field, or the account no longer being the user's choice, drops the page still waiting behind the request in flight, so it is not sent after the user opted out.
    assert(controller.accountQueueCancels == 1);
    show(6, @[@"谢谢"]);
    session.choice = @{@"translation_account":@NO, @"provider":@"none"};
    [controller synchronizeAccountGloss:[controller currentAccountGlossRequest]];
    assert(controller.accountQueueCancels == 2 && ![controller valueForKey:@"accountGlossTimer"]);
    [[NSUserDefaults new] removePersistentDomainForName:suite];
    [[MSIMETranslationCache sharedCache] clear];
}

// The account is asked only after the local dictionaries have answered, and only for words they left empty, as Windows hands only the dictionary misses to RequestMisses (event_listener.cpp ApplyCandidateTranslations). A word the English dictionary answers costs no account quota when English is the only target, but it still goes out while another target lacks a local answer, because one word list serves every target.
static void TestAccountGlossWaitsForDictionary() {
    [[MSIMETranslationCache sharedCache] clear];
    NSString *suite = [@"msime.account-gloss-wait." stringByAppendingString:NSUUID.UUID.UUIDString];
    MSIMEAppearancePreferences *prefs =
        [[MSIMEAppearancePreferences alloc] initWithDefaults:[[NSUserDefaults alloc] initWithSuiteName:suite]];
    CustomTranslationController *controller = [CustomTranslationController alloc];
    controller.batches = [NSMutableArray array];
    controller.accountFetches = [NSMutableArray array];
    controller.extraEnglishGlosses = @{@"你好":@"hello"};
    CustomTranslationSession *session = [CustomTranslationSession new];
    session.enabled = YES; session.generation = 1; session.offline = YES; session.account = YES;
    session.targetLanguage = @"en"; session.targetLanguages = @[@"en"];
    session.page = @[@{@"text":@"Hello", @"source":@4}, @{@"text":@"你好", @"source":@0}, @{@"text":@"再见", @"source":@0}];
    session.queryCandidates = @[@{@"text":@"Hello", @"online_gloss":@NO}, @{@"text":@"你好", @"online_gloss":@YES},
                                @{@"text":@"再见", @"online_gloss":@YES}];
    [controller setValue:session forKey:@"session"];
    [controller setValue:[ShortcutClient new] forKey:@"activeClient"];
    [controller setValue:prefs forKey:@"appearance"];
    // Only marks which words' English replies to save; no reply is posted here, so nothing is written.
    [controller setValue:[NSTemporaryDirectory() stringByAppendingPathComponent:NSUUID.UUID.UUIDString] forKey:@"preferencesDirectory"];
    void (^settle)(void) = ^{
        [controller synchronizeCandidateGloss];
        [controller synchronizeTargetGloss];
        [controller synchronizeAccountGloss:[controller currentAccountGlossRequest]];
        [(NSOperationQueue *)[controller valueForKey:@"glossQueue"] waitUntilAllOperationsAreFinished];
        [(NSOperationQueue *)[controller valueForKey:@"targetGlossQueue"] waitUntilAllOperationsAreFinished];
        // Each worker ends by dispatching its completion to the main queue, so a block enqueued now runs after both.
        __block BOOL drained = NO;
        dispatch_async(dispatch_get_main_queue(), ^{ drained = YES; });
        NSDate *deadline = [NSDate dateWithTimeIntervalSinceNow:5.0];
        while (!drained && deadline.timeIntervalSinceNow > 0)
            [NSRunLoop.mainRunLoop runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.005]];
        assert(drained);
    };
    // Nothing is asked while the English dictionary is still reading; its completion asks, and only for the word it could not answer.
    [controller synchronizeCandidateGloss];
    [controller synchronizeAccountGloss:[controller currentAccountGlossRequest]];
    assert(controller.accountFetches.count == 0);
    WaitForGloss(controller);
    [NSRunLoop.mainRunLoop runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.05]];
    assert(controller.accountFetches.count == 1 && ([controller.accountFetches[0] isEqual:@[@[@"再见"], @"en", @"", @1]]));
    // The same page again asks nothing more.
    settle();
    assert(controller.accountFetches.count == 1);
    // With Japanese as well, the word the English dictionary answered still has an empty Japanese row, so it goes out.
    session.generation++; session.targetLanguages = @[@"en", @"ja"];
    settle();
    assert(controller.accountFetches.count == 2 && ([controller.accountFetches[1] isEqual:@[@[@"再见", @"你好"], @"en", @"ja", @2]]));
    // The account's English reply for the word the English dictionary answered is not saved to the learned glossary, where it would override the packaged gloss; only the word the dictionary missed is marked to save.
    assert([[NSSet setWithArray:[[controller valueForKey:@"accountEnglishQueries"] allKeys]] isEqual:[NSSet setWithObject:@"再见"]]);
    // 账号回复的日文行要和词典的英文行并到同一个候选下。以前按词去重，词典已经列出了 你好，账号那条就被丢掉，于是只显示英文释义。
    session.delivered = nil;
    [controller accountCandidateTranslationsDidArrive:[NSNotification notificationWithName:@"MSIMEBackendCandidateTranslationsDidArrive"
        object:nil userInfo:@{@"generation":@2, @"marker":@0, @"target":@"ja", @"translations":@{@"你好":@"こんにちは"}}]];
    NSDictionary *greeting = nil;
    for (NSDictionary *entry in session.delivered) if ([entry[@"text"] isEqual:@"你好"]) greeting = entry;
    assert([greeting[@"translation"] isEqual:@"hello\u2028こんにちは"]);
    // Once the Japanese dictionary answers it too, it is left out, while a word that dictionary answered only in Japanese goes out for its English row.
    session.generation++; session.offlineGlossLanguages = @[@"ja"];
    session.page = @[@{@"text":@"你好", @"source":@0}, @{@"text":@"测试", @"source":@0}];
    session.queryCandidates = @[@{@"text":@"你好", @"online_gloss":@YES}, @{@"text":@"测试", @"online_gloss":@YES}];
    settle();
    assert(controller.accountFetches.count == 3 && ([controller.accountFetches[2] isEqual:@[@[@"测试"], @"en", @"ja", @3]]));
    [controller cancelCandidateTranslations];
    [[NSUserDefaults new] removePersistentDomainForName:suite];
    [[MSIMETranslationCache sharedCache] clear];
}

// An English account gloss is saved to the user glossary when it arrives, as Windows saves a fetched English gloss (cloud_translation.cpp PersistGloss), so the next page and the next launch answer it offline. It used to wait for the commit, where the account rows, which carry no Engine source, were rejected by the plan and nothing was ever saved.
static void TestAccountGlossPersistsOnArrival() {
    [[MSIMETranslationCache sharedCache] clear];
    NSString *root = [NSTemporaryDirectory() stringByAppendingPathComponent:NSUUID.UUID.UUIDString];
    NSString *elsewhere = [root stringByAppendingPathComponent:@"another-profile"];
    // The glossary store writes into an existing preferences directory; it does not create one.
    assert([NSFileManager.defaultManager createDirectoryAtPath:elsewhere withIntermediateDirectories:YES attributes:nil error:nil]);
    NSString *suite = [@"msime.account-gloss-persist." stringByAppendingString:NSUUID.UUID.UUIDString];
    MSIMEAppearancePreferences *prefs =
        [[MSIMEAppearancePreferences alloc] initWithDefaults:[[NSUserDefaults alloc] initWithSuiteName:suite]];
    CustomTranslationController *(^attach)(AccountGlossSession *, NSString *) = ^(AccountGlossSession *session, NSString *directory) {
        CustomTranslationController *controller = [CustomTranslationController alloc];
        controller.batches = [NSMutableArray array];
        controller.accountFetches = [NSMutableArray array];
        [controller setValue:[ShortcutClient new] forKey:@"activeClient"];
        [controller setValue:prefs forKey:@"appearance"];
        [controller setValue:session forKey:@"session"];
        [controller setValue:directory forKey:@"preferencesDirectory"];
        return controller;
    };
    void (^reply)(CustomTranslationController *, NSUInteger, NSString *, NSDictionary *) =
        ^(CustomTranslationController *controller, NSUInteger generation, NSString *target, NSDictionary *translations) {
        [controller accountCandidateTranslationsDidArrive:[NSNotification notificationWithName:@"MSIMEBackendCandidateTranslationsDidArrive"
            object:nil userInfo:@{@"generation":@(generation), @"marker":@0, @"target":target, @"translations":translations}]];
    };
    NSArray *(^lookup)(NSString *) = ^(NSString *directory) {
        __block NSDictionary *found;
        dispatch_sync([MSIMEInputController learnedTranslationQueue], ^{
            found = [MSIMEClientSession learnedTranslationRequest:@{@"directory":directory, @"action":@"lookup", @"target_language":@"en",
                @"generation":@1, @"items":@[@{@"text":@"测试", @"direction":@"chinese_to_english"},
                                               @{@"text":@"再见", @"direction":@"chinese_to_english"},
                                               @{@"text":@"你好", @"direction":@"chinese_to_english"}]} error:nil];
        });
        return found[@"translations"];
    };
    AccountGlossSession *session = [AccountGlossSession new];
    session.candidates = @[@{@"text":@"测试", @"online_gloss":@YES}, @{@"text":@"再见", @"online_gloss":@YES}];
    CustomTranslationController *writer = attach(session, root);
    [writer synchronizeAccountGloss:[writer currentAccountGlossRequest]];
    assert(writer.accountFetches.count == 1 && ([writer.accountFetches[0][0] isEqual:@[@"测试", @"再见"]]));
    // Every controller hears every reply, so one that did not ask saves nothing, even into a directory of its own.
    AccountGlossSession *bystanderSession = [AccountGlossSession new];
    bystanderSession.candidates = session.candidates;
    CustomTranslationController *bystander = attach(bystanderSession, elsewhere);
    // The user types on before the account answers: the reply lands after the page it was asked for has gone, and is saved all the same, as Windows saves every fetch that completes.
    session.generation = 2;
    session.candidates = @[@{@"text":@"你好", @"online_gloss":@YES}];
    [writer synchronizeAccountGloss:[writer currentAccountGlossRequest]];
    reply(writer, 1, @"en", @{@"测试":@"test"});
    reply(bystander, 1, @"en", @{@"测试":@"test"});
    // Only the English reply is a glossary entry; a Japanese one for a word that was asked about stays in the process cache.
    reply(writer, 1, @"ja", @{@"再见":@"さようなら"});
    // A candidate committed before its gloss arrived is the usual case, and its gloss is saved too.
    [writer cancelCandidateTranslations];
    reply(writer, 2, @"en", @{@"你好":@"hello"});
    dispatch_sync([MSIMEInputController learnedTranslationQueue], ^{});
    assert(([lookup(root) isEqual:@[@{@"text":@"测试", @"translation":@"test"}, @{@"text":@"你好", @"translation":@"hello"}]]));
    assert([lookup(elsewhere) count] == 0);
    [[NSUserDefaults new] removePersistentDomainForName:suite];
    NSError *error = nil;
    assert([NSFileManager.defaultManager removeItemAtPath:root error:&error] && !error);
    [[MSIMETranslationCache sharedCache] clear];
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
    assert(([session.delivered isEqual:@[@{@"text":@"Hello", @"translation":@"本地释义"}, @{@"text":@"测试", @"translation":@"\u2028essai"}]]));
    session.generation++; session.targetLanguage = @"ja"; session.targetLanguages = @[@"ja", @"en"];
    settle();
    assert(([session.delivered isEqual:@[@{@"text":@"Hello", @"translation":@"\u2028本地释义"}, @{@"text":@"测试", @"translation":@"テスト"},
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
        // 英文词典和目标语言词典在后台队列结束时把结果投递到主队列，端侧请求要等这些结果落地才会发出，这条路径上没有计时器。这里排空主队列，而不是转一段固定 0.2 秒的 run loop：完整的 shortcut 运行里前面用例留在主线程的回调在慢机器和 AddressSanitizer 上能占满这段时间，结果还没处理时 onDeviceFetches 就会少一条。
        DrainMainQueue();
    };
    void (^reply)(NSString *, NSDictionary *) = ^(NSString *target, NSDictionary *translations) {
        [controller onDeviceCandidateTranslationsDidArrive:[NSNotification notificationWithName:@"MSIMEBackendOnDeviceTranslationsDidArrive"
            object:nil userInfo:@{@"target":target, @"translations":translations}]];
    };
    // Nothing is asked before the target's offline dictionary has answered; its completion asks.
    [controller synchronizeTargetGloss];
    [controller synchronizeOnDeviceGloss];
    assert(!controller.onDeviceFetches.count);
    // Only Chinese candidates are asked about, and only those the target's dictionary left empty: 测试 has a French entry, so the model never spends time on it.
    settle();
    assert(controller.onDeviceFetches.count == 1 && ([controller.onDeviceFetches[0] isEqual:@[@[@"你好"], @[@"fr"]]]));
    assert(([session.delivered isEqual:@[@{@"text":@"测试", @"translation":@"essai"}]]));
    // Every controller hears every on-device reply, so one can carry a word this controller skipped; the target's dictionary still keeps the rows it answered.
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
    // A model answer over several lines is formatted to one, so it stays on the second target's row instead of opening a third.
    reply(@"de", @{@"测试":@"Test\nfoo "});
    assert(([session.delivered isEqual:@[@{@"text":@"Hello", @"translation":@"本地释义"}, @{@"text":@"测试", @"translation":@"\u2028Test foo"}]]));
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
        // 英文词典在后台队列结束时把结果投递到主队列，端侧请求要等它落地才会发出，这条路径上没有计时器。这里排空主队列，而不是转一段固定 0.2 秒的 run loop：完整的 shortcut 运行里前面用例留在主线程的回调在慢机器和 AddressSanitizer 上能占满这段时间。
        DrainMainQueue();
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
    // English first, so a German reply that reached the glossary would overwrite it. The model's line break and trailing space are formatted away before the gloss is cached or saved.
    reply(writer, @"en", @{@"测试":@"test\nfoo "});
    assert([[[MSIMETranslationCache sharedCache] valueForIdentity:MSIMEOnDeviceGlossIdentity(@"en", @"测试")] isEqual:@"test foo"]);
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
    assert(([lookup()[@"translations"] isEqual:@[@{@"text":@"测试", @"translation":@"test foo"}, @{@"text":@"你好", @"translation":@"hello"}]]));
    // A fresh controller with an empty process cache, as after a restart, answers the saved word from the glossary and only asks the model about the rest.
    [writer cancelCandidateTranslations]; [[MSIMETranslationCache sharedCache] clear];
    session.page = askedPage; session.queryCandidates = askedQuery;
    session.generation++; session.targetLanguages = @[@"en"]; session.delivered = nil;
    CustomTranslationController *reader = attach();
    settle(reader);
    assert(reader.onDeviceFetches.count == 1 && ([reader.onDeviceFetches[0] isEqual:@[@[@"再见"], @[@"en"]]]));
    assert(([session.delivered isEqual:@[@{@"text":@"测试", @"translation":@"test foo"}]]));
    [reader cancelCandidateTranslations];
    [[NSUserDefaults new] removePersistentDomainForName:suite];
    NSError *error = nil;
    assert([NSFileManager.defaultManager removeItemAtPath:root error:&error] && !error);
    [[MSIMETranslationCache sharedCache] clear];
}

// 同一控制器先后为同一个词发起两个端侧英文释义请求时，晚到的第一个回复仍须使用它自己的 Engine 来源写入词库；不能被后一个页面的同名词覆盖。
static void TestOnDeviceGlossDuplicateTextKeepsRequestMetadata() {
    [[MSIMETranslationCache sharedCache] clear];
    NSString *root = [NSTemporaryDirectory() stringByAppendingPathComponent:NSUUID.UUID.UUIDString];
    assert([NSFileManager.defaultManager createDirectoryAtPath:root withIntermediateDirectories:YES attributes:nil error:nil]);
    NSString *suite = [@"msime.on-device-gloss-duplicate." stringByAppendingString:NSUUID.UUID.UUIDString];
    MSIMEAppearancePreferences *prefs =
        [[MSIMEAppearancePreferences alloc] initWithDefaults:[[NSUserDefaults alloc] initWithSuiteName:suite]];
    CustomTranslationSession *session = [CustomTranslationSession new];
    session.enabled = YES; session.targetLanguage = @"en"; session.generation = 1;
    session.targetLanguages = @[@"en"];
    session.page = @[@{ @"text": @"同名", @"source": @0 }];
    session.queryCandidates = @[@{ @"text": @"同名", @"online_gloss": @YES }];
    CustomTranslationController *controller = [CustomTranslationController alloc];
    controller.onDeviceFetches = [NSMutableArray array];
    [controller setValue:session forKey:@"session"];
    [controller setValue:[ShortcutClient new] forKey:@"activeClient"];
    [controller setValue:prefs forKey:@"appearance"];
    [controller setValue:root forKey:@"preferencesDirectory"];
    void (^settle)(void) = ^{
        [controller synchronizeCandidateGloss];
        [(NSOperationQueue *)[controller valueForKey:@"glossQueue"] waitUntilAllOperationsAreFinished];
        DrainMainQueue();
        [controller synchronizeOnDeviceGloss];
    };
    settle();
    assert(controller.onDeviceFetches.count == 1);

    // The next page asks for the same text with a different Engine source and target set,
    // replacing the visible request while the first model answer is still in flight.
    session.generation = 2;
    session.targetLanguages = @[@"en", @"de"];
    session.page = @[@{ @"text": @"同名", @"source": @6 }];
    settle();
    assert(controller.onDeviceFetches.count == 3);

    [controller onDeviceCandidateTranslationsDidArrive:[NSNotification notificationWithName:@"MSIMEBackendOnDeviceTranslationsDidArrive"
        object:nil userInfo:@{@"target": @"en", @"translations": @{@"同名": @"old reply"}}]];
    dispatch_sync([MSIMEInputController learnedTranslationQueue], ^{});
    __block NSDictionary *lookup;
    dispatch_sync([MSIMEInputController learnedTranslationQueue], ^{
        lookup = [MSIMEClientSession learnedTranslationRequest:@{@"directory": root, @"action": @"lookup", @"target_language": @"en",
            @"generation": @1, @"items": @[@{@"text": @"同名", @"direction": @"chinese_to_english"},
                                               @{@"text": @"同名", @"direction": @"english_to_chinese"}]} error:nil];
    });
    assert(([lookup[@"translations"] isEqual:@[@{@"text": @"同名", @"translation": @"old reply"}]]));
    [[NSUserDefaults new] removePersistentDomainForName:suite];
    assert([NSFileManager.defaultManager removeItemAtPath:root error:nil]);
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
    // Candidate identity, client, session, focus and mode guards all reject a callback even before the next synchronization cancels transport. A generation that moved over the same words is not among them: that page still shows what the reply answers (TestGlossSurvivesHighlightMove).
    for (NSString *change in @[@"page", @"client", @"session", @"focus", @"japanese", @"disabled"]) {
        // The rejected reply still reaches the cache, so clear it to make the next round ask again.
        [[MSIMETranslationCache sharedCache] clear];
        [controller cancelCandidateTranslations];
        [controller synchronizeCustomTranslations];
        ControlledTranslationBatch *batch = controller.batches.lastObject;
        NSArray *page = session.page;
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
// #5598：设了第二释义语言后，每个候选的两行释义要经过会话再读回来。会话拒收 "\n"，所以交给它时行之间用 U+2028 连接，读回时拆回原来的行；来源自带的 U+2028、U+2029 折成空格，不额外占一行。
static void TestGlossLinesSurviveSession() {
    NSArray<NSDictionary *> *encoded = MSIMESessionTranslations(@[
        @{@"text":@"国家", @"translation":@"country; state\n国家の"},
        @{@"text":@"你好", @"translation":@"\nこんにちは"},
        @{@"text":@"测试", @"translation":@"te\u2028st\u2029x\nテスト"},
        @{@"text":@"hello", @"translation":@"你好"}]);
    NSCharacterSet *controls = [NSCharacterSet characterSetWithRange:NSMakeRange(0, 0x20)];
    for (NSDictionary *entry in encoded) assert([entry[@"translation"] rangeOfCharacterFromSet:controls].location == NSNotFound);
    assert([encoded[0][@"translation"] isEqual:@"country; state\u2028国家の"]);
    assert(([encoded[0][@"text"] isEqual:@"国家"] && [encoded[3] isEqual:@{@"text":@"hello", @"translation":@"你好"}]));
    assert([CandidateTranslation(encoded[0]) isEqual:@"country; state\n国家の"]);
    assert([MSIMECandidateTranslationColumn(encoded[0], 1) isEqual:@"country; state"]);
    assert([MSIMECandidateTranslationColumn(encoded[0], 2) isEqual:@"国家の"]);
    assert([MSIMECandidateTranslationColumn(encoded[1], 1) isEqual:@""]);
    assert([MSIMECandidateTranslationColumn(encoded[1], 2) isEqual:@"こんにちは"]);
    assert([CandidateTranslation(encoded[2]) isEqual:@"te st x\nテスト"]);
    assert([MSIMECandidateTranslationColumn(encoded[2], 2) isEqual:@"テスト"]);
    assert([CandidateTranslation(encoded[3]) isEqual:@"你好"]);
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
    assert(([session.delivered isEqual:@[@{@"text":@"你好", @"translation":@"bonjour\u2028こんにちは"}]]));
    session.generation++;
    [controller synchronizeCustomTranslations];
    assert(controller.batches.count == 2 && [session.delivered[0][@"translation"] isEqual:@"bonjour\u2028こんにちは"]);
    session.generation++;
    session.targetLanguage = @"ja";
    session.targetLanguages = @[@"ja", @"fr"];
    [controller synchronizeCustomTranslations];
    assert(controller.batches.count == 2 && [session.delivered[0][@"translation"] isEqual:@"こんにちは\u2028bonjour"]);
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
    // Typing past a page leaves its request in flight rather than cancelling it: the answer lands in the cache, not on the page now showing, and the page it was for shows it straight from the cache when it comes back.
    ControlledTranslationBatch *left = controller.batches[3];
    session.generation++; session.page = @[@{@"text":@"other", @"source":@4}];
    [controller synchronizeCustomTranslations]; assert(controller.batches.count == 5);
    assert(left.detached && !left.cancelled && [[controller valueForKey:@"detachedCustomBatches"] containsObject:left]);
    NSArray *visible = session.delivered;
    left.reply(@[@{@"text":@"HELLO", @"translation":@"Hallo"}, @{@"text":@"missing", @"translation":@"gefunden"}]);
    assert([session.delivered isEqual:visible] && ![[controller valueForKey:@"detachedCustomBatches"] count]);
    session.generation++; session.page = @[@{@"text":@"HELLO", @"source":@4}, @{@"text":@"missing", @"source":@4}];
    [controller synchronizeCustomTranslations];
    assert(controller.batches.count == 5 && controller.batches[4].detached);
    assert(([session.delivered isEqual:@[@{@"text":@"HELLO", @"translation":@"Hallo"}, @{@"text":@"missing", @"translation":@"gefunden"}]]));
    // Cancelling outright reaches detached work as well, and its late reply is not cached.
    session.generation++; session.page = @[@{@"text":@"third", @"source":@4}];
    [controller synchronizeCustomTranslations]; assert(controller.batches.count == 6);
    ControlledTranslationBatch *third = controller.batches[5];
    session.generation++; session.page = @[@{@"text":@"fourth", @"source":@4}];
    [controller synchronizeCustomTranslations];
    assert(third.detached && !third.cancelled);
    [controller cancelCandidateTranslations];
    assert(third.cancelled && controller.batches[4].cancelled && ![[controller valueForKey:@"detachedCustomBatches"] count]);
    third.reply(@[@{@"text":@"third", @"translation":@"dritte"}]);
    session.generation++; session.page = @[@{@"text":@"third", @"source":@4}];
    [controller synchronizeCustomTranslations]; assert(controller.batches.count == 8);
    [controller cancelCandidateTranslations];
    [[MSIMETranslationCache sharedCache] clear];
}
static void TestCustomTranslationCacheSeparatesCredentialsAcrossControllers() {
    [[MSIMETranslationCache sharedCache] clear];
    CustomTranslationController *first = [CustomTranslationController alloc];
    first.batches = [NSMutableArray array];
    CustomTranslationSession *oldSession = [CustomTranslationSession new];
    oldSession.enabled = YES; oldSession.generation = 1; oldSession.targetLanguage = @"fr";
    oldSession.custom = @{@"enabled":@YES, @"endpoint":@"https://cache-source.invalid/api", @"api_key":@"synthetic-old"};
    oldSession.page = @[@{@"text":@"Hello", @"source":@4}];
    [first setValue:oldSession forKey:@"session"];
    [first setValue:[ShortcutClient new] forKey:@"activeClient"];
    [first applySharedToolbarPreferences:@{@"custom_translation":oldSession.custom}];
    [first synchronizeCustomTranslations];
    assert(first.batches.count == 1);
    first.batches[0].reply(@[@{@"text":@"Hello", @"translation":@"旧译文"}]);
    [first cancelCandidateTranslations];
    first = nil;

    CustomTranslationController *second = [CustomTranslationController alloc];
    second.batches = [NSMutableArray array];
    CustomTranslationSession *newSession = [CustomTranslationSession new];
    newSession.enabled = YES; newSession.generation = 1; newSession.targetLanguage = @"fr";
    newSession.custom = @{@"enabled":@YES, @"endpoint":@"https://cache-source.invalid/api", @"api_key":@"synthetic-new"};
    newSession.page = @[@{@"text":@"Hello", @"source":@4}];
    [second setValue:newSession forKey:@"session"];
    [second setValue:[ShortcutClient new] forKey:@"activeClient"];
    [second applySharedToolbarPreferences:@{@"custom_translation":newSession.custom}];
    [second synchronizeCustomTranslations];
    assert(second.batches.count == 1 && newSession.delivered.count == 0);
    [second cancelCandidateTranslations];
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
    // A generation that moved over the same words is the same request, so the dictionary is not read again.
    session.pageGeneration = 2;
    [controller synchronizeCandidateGloss];
    [NSRunLoop.mainRunLoop runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.02]];
    assert(controller.lookups == 2 && [controller valueForKey:@"glossResults"]);
    [controller cancelCandidateGloss];
}

// Moving the highlight, or a cloud candidate landing late, advances the generation without changing the words on the page. Windows keys its translation signature on the words alone and does nothing then (event_listener.cpp ApplyCandidateTranslations); here the requests leave the generation out for the same reason, so nothing is cancelled, read again or queued again, and what is held is applied once to the new generation, since the session dropped it when the generation moved.
static void TestGlossSurvivesHighlightMove() {
    [[MSIMETranslationCache sharedCache] clear];
    NSString *suite = [@"msime.highlight-move." stringByAppendingString:NSUUID.UUID.UUIDString];
    MSIMEAppearancePreferences *prefs =
        [[MSIMEAppearancePreferences alloc] initWithDefaults:[[NSUserDefaults alloc] initWithSuiteName:suite]];
    CustomTranslationController *controller = [CustomTranslationController alloc];
    controller.batches = [NSMutableArray array];
    controller.onDeviceFetches = [NSMutableArray array];
    controller.accountFetches = [NSMutableArray array];
    controller.holdDelay = YES;
    CustomTranslationSession *session = [CustomTranslationSession new];
    session.appliedGenerations = [NSMutableArray array];
    [controller setValue:session forKey:@"session"];
    [controller setValue:[ShortcutClient new] forKey:@"activeClient"];
    [controller setValue:prefs forKey:@"appearance"];
    void (^settle)(void) = ^{
        [controller setValue:[session viewWithError:nil] forKey:@"view"];
        [controller synchronizeCandidateServices];
        [(NSOperationQueue *)[controller valueForKey:@"glossQueue"] waitUntilAllOperationsAreFinished];
        [(NSOperationQueue *)[controller valueForKey:@"targetGlossQueue"] waitUntilAllOperationsAreFinished];
        // 词典查询在后台队列结束时把结果投递到主队列，自定义翻译要等这条结果落地才会排上计时器。这里排空主队列，而不是转一段固定 0.1 秒的 run loop：完整的 shortcut 运行里前面的用例会在主线程留下别的回调，它在 x86_64 和 AddressSanitizer 上一次就能占满这 0.1 秒，run loop 超时返回时词典结果还没处理，customTimer 因此是 nil。
        DrainMainQueue();
    };

    // The user's own service, with its request in flight while the dictionary's answer for Hello is on screen.
    session.enabled = YES; session.generation = 1; session.offline = YES;
    session.targetLanguage = @"en"; session.targetLanguages = @[@"en"];
    session.custom = @{@"enabled":@YES, @"endpoint":@"https://translation.invalid/api", @"api_key":@""};
    [controller applySharedToolbarPreferences:@{@"custom_translation":session.custom}];
    session.page = @[@{@"text":@"Hello", @"source":@4}, @{@"text":@"测试", @"source":@0}];
    settle();
    NSTimer *customTimer = [controller valueForKey:@"customTimer"];
    assert(customTimer.valid);
    [customTimer fire];
    assert(controller.batches.count == 1);
    ControlledTranslationBatch *batch = controller.batches[0];
    assert(batch.started && [[batch.items valueForKey:@"text"] isEqual:@[@"测试"]]);
    NSDictionary *glossRequest = [controller valueForKey:@"glossRequest"];
    [session.appliedGenerations removeAllObjects];
    session.generation = 2;
    settle();
    assert(!batch.cancelled && !batch.detached && controller.batches.count == 1);
    assert([controller valueForKey:@"glossRequest"] == glossRequest);
    assert([session.appliedGenerations isEqual:@[@2]]);
    assert(([session.delivered isEqual:@[@{@"text":@"Hello", @"translation":@"本地释义"}]]));
    // The reply lands on the page it was asked for, now at generation 2.
    batch.reply(@[@{@"text":@"测试", @"translation":@"test"}]);
    assert(([session.delivered isEqual:@[@{@"text":@"Hello", @"translation":@"本地释义"}, @{@"text":@"测试", @"translation":@"test"}]]));
    [controller cancelCandidateTranslations];
    [controller applySharedToolbarPreferences:@{@"custom_translation":@{@"enabled":@NO, @"endpoint":@"", @"api_key":@""}}];
    session.custom = nil;

    // On-device translation: a word already queued is not queued again.
    session.generation = 3; session.offline = NO;
    session.page = @[@{@"text":@"测试", @"source":@0}, @{@"text":@"你好", @"source":@0}];
    session.queryCandidates = @[@{@"text":@"测试", @"online_gloss":@YES}, @{@"text":@"你好", @"online_gloss":@YES}];
    settle();
    assert(controller.onDeviceFetches.count == 1 && ([controller.onDeviceFetches[0] isEqual:@[@[@"测试", @"你好"], @[@"en"]]]));
    [controller onDeviceCandidateTranslationsDidArrive:[NSNotification notificationWithName:@"MSIMEBackendOnDeviceTranslationsDidArrive"
        object:nil userInfo:@{@"target":@"en", @"translations":@{@"测试":@"test"}}]];
    assert(([session.delivered isEqual:@[@{@"text":@"测试", @"translation":@"test"}]]));
    [session.appliedGenerations removeAllObjects];
    session.generation = 4;
    settle();
    assert(controller.onDeviceFetches.count == 1 && [session.appliedGenerations isEqual:@[@4]]);
    assert(([session.delivered isEqual:@[@{@"text":@"测试", @"translation":@"test"}]]));
    [controller cancelCandidateTranslations];

    // The account: the pending idle timer is kept, not restarted, and the cached gloss is applied to the new generation.
    [[MSIMETranslationCache sharedCache] rememberTranslation:@"hello" identity:@[@"account", @"0", @"en", @"你好"]];
    session.account = YES; session.generation = 5;
    settle();
    NSTimer *accountTimer = [controller valueForKey:@"accountGlossTimer"];
    assert(accountTimer.valid && !controller.accountFetches.count);
    assert(([session.delivered isEqual:@[@{@"text":@"你好", @"translation":@"hello"}]]));
    [session.appliedGenerations removeAllObjects];
    session.generation = 6;
    settle();
    assert([controller valueForKey:@"accountGlossTimer"] == accountTimer && accountTimer.valid);
    assert([session.appliedGenerations isEqual:@[@6]] && !controller.accountFetches.count);
    assert(([session.delivered isEqual:@[@{@"text":@"你好", @"translation":@"hello"}]]));
    [accountTimer fire];
    assert(controller.accountFetches.count == 1 && ([controller.accountFetches[0] isEqual:@[@[@"测试"], @"en", @"", @6]]));
    [controller cancelCandidateTranslations];
    [[NSUserDefaults new] removePersistentDomainForName:suite];
    [[MSIMETranslationCache sharedCache] clear];
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

    for (NSString *category in @[@"dictionary access unavailable", @"dictionary maintenance busy", @"invalid options document"]) {
        NSError *error = [NSError errorWithDomain:@"synthetic" code:1 userInfo:@{NSLocalizedDescriptionKey:category}];
        assert([MSIMESessionUnavailableReason(@{}, error) isEqual:category]);
    }
    NSError *privateError = [NSError errorWithDomain:@"synthetic" code:1
        userInfo:@{NSLocalizedDescriptionKey:@"/synthetic/private-path synthetic-secret"}];
    assert([MSIMESessionUnavailableReason(@{}, privateError) isEqual:@"session refused the runtime options"]);
}

@interface SessionRecoveryController : MSIMEInputController
@property(strong) NSDictionary *testOptions;
@property NSUInteger optionReads;
@property NSUInteger creations;
@property NSUInteger failures;
@property BOOL missingFirstOptions;
@end
@implementation SessionRecoveryController
- (NSDictionary *)runtimeOptions {
    ++self.optionReads;
    return self.missingFirstOptions && self.optionReads == 1 ? nil : self.testOptions;
}
- (MSIMEClientSession *)createSessionWithOptions:(NSDictionary *)options error:(NSError **)error {
    ++self.creations;
    if (self.creations <= self.failures) {
        if (error) *error = [NSError errorWithDomain:@"synthetic" code:1
            userInfo:@{NSLocalizedDescriptionKey:@"dictionary access unavailable"}];
        return nil;
    }
    return [super createSessionWithOptions:options error:error];
}
// 本专项只验证原生控制器与真实 Rust 会话的生命周期，不启动后台偏好轮询或联网授权。
- (void)startPreferencesMonitoring {}
- (void)resolveCloudCandidatesConsentWithOptions:(NSDictionary *)options { (void)options; }
- (void)confirmFieldTraitsBeforeFocus {}
- (void)refreshTypingEffectSettings {}
@end

@interface SessionRecoveryNoticeController : SessionRecoveryController
@property BOOL secureInput;
@property NSUInteger restartRequests;
@property NSUInteger terminations;
@property(copy) void (^restartCompletion)(BOOL);
@end
@implementation SessionRecoveryNoticeController
- (BOOL)secureEventInputActive { return self.secureInput; }
- (void)launchInputMethodRestart:(void (^)(BOOL))completion {
    ++self.restartRequests;
    self.restartCompletion = completion;
}
- (void)terminateCurrentInputMethod { ++self.terminations; }
@end

static void TestSessionRecoveryNotice(NSDictionary *options) {
    MSIMEInputController *previousFocus = MSIMEFocusedController;
    MSIMESessionRecoveryLastNotice = 0;
    [MSIMEInputSessionRecoveryPanel dismissForDelegate:nil];
    NSString *suite = [@"msime.session-recovery-notice." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    MSIMEAppearancePreferences *appearance = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults
        skinsRoot:[NSURL fileURLWithPath:options[@"resources"]]];
    appearance.englishMode = NO;
    auto makeController = ^SessionRecoveryNoticeController * {
        SessionRecoveryNoticeController *controller = [SessionRecoveryNoticeController alloc];
        controller.testOptions = options;
        controller.failures = NSUIntegerMax;
        [controller setValue:appearance forKey:@"appearance"];
        [controller setValue:[ShortcutClient new] forKey:@"activeClient"];
        return controller;
    };
    SessionRecoveryNoticeController *controller = makeController();
    MSIMEFocusedController = controller;
    NSWindow *previousKey = NSApp.keyWindow;
    [controller prepareSession];
    assert(!MSIMESessionRecoveryPanel.isVisible);
    DrainMainQueue();
    MSIMEInputSessionRecoveryPanel *panel = MSIMESessionRecoveryPanel;
    assert(panel.isVisible && panel.recoveryDelegate == controller);
    assert(!panel.canBecomeKeyWindow && !panel.canBecomeMainWindow && !panel.isKeyWindow);
    assert(NSApp.keyWindow == previousKey);
    assert((panel.styleMask & NSWindowStyleMaskNonactivatingPanel) != 0);
    assert([panel.message.stringValue containsString:@"直接输入字母"]);
    assert(panel.retryButton.keyEquivalent.length == 0 && panel.restartButton.keyEquivalent.length == 0);
    uint64_t shown = MSIMESessionRecoveryLastNotice;
    [controller prepareSession];
    DrainMainQueue();
    assert(MSIMESessionRecoveryLastNotice == shown);
    [panel.laterButton performClick:nil];
    assert(!panel.isVisible && !panel.recoveryDelegate);
    [controller prepareSession];
    DrainMainQueue();
    assert(!panel.isVisible && MSIMESessionRecoveryLastNotice == shown);
    NSMenu *menu = [controller menu];
    assert([menu itemWithTitle:@"重试中文输入"].action == @selector(retryUnavailableSession:));
    assert([menu itemWithTitle:@"重启输入法…"].action == @selector(restartUnavailableInputMethod:));

    // 菜单先显示重启影响，提示中的明确点击才启动；失败后仍保留当前进程和可重试按钮。
    [controller restartUnavailableInputMethod:nil];
    assert(panel.isVisible && controller.restartRequests == 0);
    [controller restartUnavailableInputMethod:[menu itemWithTitle:@"重启输入法…"]];
    assert(panel.isVisible && controller.restartRequests == 0);
    [panel.retryButton performClick:nil];
    assert([panel.message.stringValue containsString:@"重试后仍无法恢复"]);
    [panel.restartButton performClick:nil];
    assert(controller.restartRequests == 1 && !panel.retryButton.enabled && !panel.restartButton.enabled);
    [controller restartUnavailableInputMethod:nil];
    assert(controller.restartRequests == 1);
    SessionRecoveryNoticeController *peer = makeController();
    MSIMEFocusedController = peer;
    [peer prepareSession];
    [peer restartUnavailableInputMethod:nil];
    assert(peer.restartRequests == 0);
    MSIMEFocusedController = controller;
    controller.restartCompletion(NO);
    assert(controller.terminations == 0 && panel.retryButton.enabled && panel.restartButton.enabled);
    assert([panel.message.stringValue containsString:@"未能启动"]);

    controller.failures = 0;
    [panel.retryButton performClick:nil];
    assert([controller valueForKey:@"session"] && ![controller valueForKey:@"sessionUnavailableReason"]);
    assert(!panel.isVisible && !panel.recoveryDelegate);
    assert(![[controller menu] itemWithTitle:@"重试中文输入"]);
    [controller releaseDictionarySession];
    controller.failures = NSUIntegerMax;

    // 排队期间失焦、恢复或进入安全输入时，过时提示不能显示。
    MSIMESessionRecoveryLastNotice = 0;
    [controller prepareSession];
    assert(MSIMESessionRecoveryNoticeScheduled);
    [controller setValue:nil forKey:@"activeClient"];
    DrainMainQueue();
    assert(!panel.isVisible);
    [controller setValue:[ShortcutClient new] forKey:@"activeClient"];
    controller.secureInput = YES;
    [controller prepareSession];
    DrainMainQueue();
    assert(!panel.isVisible);
    controller.secureInput = NO;
    appearance.englishMode = YES;
    [controller prepareSession];
    DrainMainQueue();
    assert(!panel.isVisible);
    appearance.englishMode = NO;
    [controller prepareSession];
    controller.failures = 0;
    [controller prepareSession];
    DrainMainQueue();
    assert(!panel.isVisible && [controller valueForKey:@"session"]);
    [controller releaseDictionarySession];
    controller.failures = NSUIntegerMax;
    [controller prepareSession];
    DrainMainQueue();
    assert(panel.isVisible);
    controller.secureInput = YES;
    [controller syncFieldTraits];
    assert(!panel.isVisible);
    controller.secureInput = NO;

    // 提醒频率按进程限制；另一个控制器不能因切换输入框而立即再弹。
    SessionRecoveryNoticeController *other = makeController();
    MSIMEFocusedController = other;
    [other prepareSession];
    DrainMainQueue();
    assert(!panel.isVisible);
    MSIMESessionRecoveryLastNotice = clock_gettime_nsec_np(CLOCK_UPTIME_RAW) - MSIMESessionRecoveryNoticeInterval - 1;
    [other prepareSession];
    DrainMainQueue();
    assert(panel.isVisible && panel.recoveryDelegate == other);
    NSUInteger creations = controller.creations;
    [controller retryUnavailableSession:nil];
    assert(controller.creations == creations && panel.recoveryDelegate == other);
    Method base = class_getInstanceMethod(IMKInputController.class, @selector(deactivateServer:));
    IMP original = method_setImplementation(base, (IMP)RecordBaseDeactivation);
    [other deactivateServer:[other valueForKey:@"activeClient"]];
    method_setImplementation(base, original);
    assert(!panel.isVisible && !panel.recoveryDelegate);

    MSIMEFocusedController = controller;
    [controller restartUnavailableInputMethod:nil];
    [panel.restartButton performClick:nil];
    assert(controller.restartRequests == 2);
    controller.restartCompletion(YES);
    assert(controller.terminations == 1 && !panel.isVisible);
    controller.restartCompletion = nil;
    [controller setValue:nil forKey:@"activeClient"];
    MSIMEFocusedController = previousFocus;
    MSIMESessionRecoveryLastNotice = 0;
    DrainMainQueue();
    [defaults removePersistentDomainForName:suite];
}

static void TestIdleSessionRecovery() {
    NSString *root = [NSTemporaryDirectory() stringByAppendingPathComponent:NSUUID.UUID.UUIDString];
    NSError *error = nil;
    NSDictionary *snapshot = [MSIMEClientSession loadPreferencesInDirectory:root error:&error];
    assert(snapshot && !error);
    NSMutableDictionary *preferences = [snapshot[@"preferences"] mutableCopy];
    preferences[@"scheme"] = @"wubi";
    preferences[@"learning"] = @NO;
    preferences[@"wubi_mixed_pinyin"] = @NO;
    NSMutableDictionary *options = [@{@"api_version":@1, @"preferences":preferences} mutableCopy];
    for (NSString *name in @[@"resources", @"user_data", @"cache", @"dictionaries"]) {
        NSString *path = [root stringByAppendingPathComponent:name];
        assert([NSFileManager.defaultManager createDirectoryAtPath:path withIntermediateDirectories:YES attributes:nil error:nil]);
        options[name] = path;
    }
    for (NSString *path in @[[options[@"resources"] stringByAppendingPathComponent:@"msime-wubi.db"],
                             [options[@"dictionaries"] stringByAppendingPathComponent:@"msime-pinyin.db"]]) {
        sqlite3 *database = nullptr;
        assert(sqlite3_open(path.fileSystemRepresentation, &database) == SQLITE_OK);
        assert(sqlite3_exec(database,
            "CREATE TABLE wubi86(key TEXT,value TEXT,weight INTEGER);"
            "CREATE TABLE wubi98(key TEXT,value TEXT,weight INTEGER);"
            "INSERT INTO wubi86 VALUES('wq','你',10);", nullptr, nullptr, nullptr) == SQLITE_OK);
        assert(sqlite3_close(database) == SQLITE_OK);
    }
    auto makeController = ^SessionRecoveryController * {
        SessionRecoveryController *controller = [SessionRecoveryController alloc];
        controller.testOptions = options;
        [controller setValue:[ShortcutClient new] forKey:@"activeClient"];
        return controller;
    };
    auto sessionOf = ^MSIMEClientSession *(SessionRecoveryController *controller) {
        return [controller valueForKey:@"session"];
    };
    struct rlimit previous = {};
    assert(getrlimit(RLIMIT_NOFILE, &previous) == 0);
    struct rlimit limited = previous;
    limited.rlim_cur = MIN(previous.rlim_cur, (rlim_t)256);
    assert(setrlimit(RLIMIT_NOFILE, &limited) == 0);
    SessionRecoveryController *active = makeController();
    [active prepareSession];
    MSIMEClientSession *activeSession = sessionOf(active);
    assert(activeSession);
    [activeSession typeASCII:'w' shift:NO error:&error];
    [activeSession typeASCII:'q' shift:NO error:&error];
    assert(!error);
    SessionRecoveryController *english = makeController();
    [english prepareSession];
    [english setValue:[sessionOf(english) setDedicatedEnglishEnabled:YES error:&error] forKey:@"view"];
    assert(!error);
    [english setValue:nil forKey:@"activeClient"];
    NSMutableArray<SessionRecoveryController *> *controllers = [NSMutableArray arrayWithObject:english];
    Method base = class_getInstanceMethod(IMKInputController.class, @selector(deactivateServer:));
    IMP original = method_setImplementation(base, (IMP)RecordBaseDeactivation);
    for (NSUInteger index = 0; index < 64; ++index) {
        @autoreleasepool {
            SessionRecoveryController *controller = makeController();
            [controllers addObject:controller];
            [controller prepareSession];
            MSIMEClientSession *session = sessionOf(controller);
            assert(session && controller.creations == 1);
            [session typeASCII:'w' shift:NO error:&error];
            NSDictionary *typed = [session typeASCII:'q' shift:NO error:&error];
            assert(!error && [typed[@"view"][@"scheme"] isEqual:@2]);
            assert([typed[@"view"][@"candidates"][0][@"text"] isEqual:@"你"]);
            [controller apply:[session command:MSIME_CANCEL error:&error]];
            [controller deactivateServer:[controller valueForKey:@"activeClient"]];
            NSUInteger idle = 0;
            for (SessionRecoveryController *retained in controllers) if (sessionOf(retained)) ++idle;
            assert(idle <= MSIMEIdleDictionarySessionLimit);
            assert(sessionOf(active) == activeSession);
            assert([[activeSession viewWithError:&error][@"editing_text"] isEqual:@"wq"] && !error);
        }
    }
    method_setImplementation(base, original);
    assert(!sessionOf(english) && [[english valueForKey:@"resumeDedicatedEnglish"] boolValue]);
    [english setValue:[ShortcutClient new] forKey:@"activeClient"];
    [english prepareSession];
    assert([sessionOf(english) viewWithError:&error][@"dedicated_english"] == (__bridge id)kCFBooleanTrue && !error);
    [english setValue:nil forKey:@"activeClient"];
    [MSIMEInputController releaseIdleDictionarySessionsKeeping:0];

    for (NSUInteger scenario = 0; scenario < 3; ++scenario) {
        SessionRecoveryController *idle = makeController();
        [idle prepareSession];
        assert(sessionOf(idle));
        [idle setValue:nil forKey:@"activeClient"];
        SessionRecoveryController *retry = makeController();
        retry.failures = scenario == 0 ? 1 : (scenario == 1 ? 2 : 0);
        retry.missingFirstOptions = scenario == 2;
        [retry prepareSession];
        assert(!sessionOf(idle));
        assert(retry.optionReads == 2 && retry.creations == (scenario == 2 ? 1 : 2));
        assert((sessionOf(retry) != nil) == (scenario != 1));
        if (scenario == 1) assert([[retry valueForKey:@"sessionUnavailableReason"] isEqual:@"dictionary access unavailable"]);
        [retry setValue:nil forKey:@"activeClient"];
        [MSIMEInputController releaseIdleDictionarySessionsKeeping:0];
    }
    [active setValue:nil forKey:@"activeClient"];
    [MSIMEInputController releaseIdleDictionarySessionsKeeping:0];
    assert([MSIMEInputController dictionarySessionHolders].count == 0);
    assert(setrlimit(RLIMIT_NOFILE, &previous) == 0);
    TestSessionRecoveryNotice(options);
    assert([NSFileManager.defaultManager removeItemAtPath:root error:&error] && !error);
}

// Every preference object built here without an explicit skins root reads msime::mac::DefaultSkinsRoot(), which is otherwise the user's real skins folder: an external skin installed there adds a card to the skin page and breaks the counts below. The whole run gets an empty folder of its own, removed when main returns.
struct IsolatedDefaultSkinsRoot {
    std::filesystem::path path;
    IsolatedDefaultSkinsRoot() {
        std::string pattern = (std::filesystem::temp_directory_path() / "msime-shortcut-skins.XXXXXX").string();
        assert(mkdtemp(pattern.data()) != nullptr);
        path = pattern;
        msime::mac::SetDefaultSkinsRoot(path);
    }
    ~IsolatedDefaultSkinsRoot() {
        msime::mac::SetDefaultSkinsRoot({});
        std::error_code error;
        std::filesystem::remove_all(path, error);
    }
};

// The controllers here read runtime-options.json from Application Support and save preferences into the directory it names. CTest points CFFIXED_USER_HOME at an empty home, but a direct run of this binary had none, and one of these cases once switched the developer's own input method to full width by saving into their real preferences. HOME does not move Application Support; CFFIXED_USER_HOME does, and Foundation reads it on first use, so it is set before anything touches Foundation, and the run stops if Application Support is still outside it.
struct IsolatedUserHome {
    std::filesystem::path path;
    bool created = false;
    IsolatedUserHome() {
        const char *given = std::getenv("CFFIXED_USER_HOME");
        if (given != nullptr && given[0] != '\0') {
            path = given;
        } else {
            std::string pattern = (std::filesystem::temp_directory_path() / "msime-shortcut-home.XXXXXX").string();
            assert(mkdtemp(pattern.data()) != nullptr);
            path = pattern;
            created = true;
            assert(setenv("CFFIXED_USER_HOME", path.c_str(), 1) == 0);
        }
        NSString *support = [NSFileManager.defaultManager URLsForDirectory:NSApplicationSupportDirectory inDomains:NSUserDomainMask].firstObject.path;
        const std::filesystem::path home = std::filesystem::weakly_canonical(path);
        if (support == nil || std::filesystem::weakly_canonical(support.fileSystemRepresentation).string().rfind(home.string(), 0) != 0) {
            std::fprintf(stderr, "refusing to run: Application Support (%s) is not inside the isolated home %s\n", support.UTF8String ?: "none", home.c_str());
            std::abort();
        }
    }
    ~IsolatedUserHome() {
        if (!created) return;
        std::error_code error;
        std::filesystem::remove_all(path, error);
    }
};

@interface SpacingQueryShortcutClient : ApplicationShortcutClient
@property(nonatomic, copy) void (^duringContextRead)(void);
@property(nonatomic, copy) void (^duringBundleRead)(void);
@property(nonatomic, copy) void (^afterInsert)(void);
@property(nonatomic, copy) void (^duringReplacement)(void);
@end
@implementation SpacingQueryShortcutClient
- (void)insertText:(id)text replacementRange:(NSRange)range {
    [super insertText:text replacementRange:range];
    void (^callback)(void) = self.afterInsert;
    self.afterInsert = nil;
    if (callback) callback();
    if (range.location != NSNotFound) {
        void (^replacement)(void) = self.duringReplacement;
        self.duringReplacement = nil;
        if (replacement) replacement();
    }
}
- (NSRange)selectedRange {
    void (^callback)(void) = self.duringContextRead;
    self.duringContextRead = nil;
    if (callback) callback();
    return [super selectedRange];
}
- (NSString *)bundleIdentifier {
    void (^callback)(void) = self.duringBundleRead;
    self.duringBundleRead = nil;
    if (callback) callback();
    return [super bundleIdentifier];
}
@end

static void TestCrossCommitSpacingReentrancy() {
    NSString *suite = [@"msime.cross-commit-spacing-reentrant." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    MSIMEAppearancePreferences *prefs = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    [prefs activateInputModeForApplication:@"com.example.Writer"];
    [prefs applySharedInputPreferences:@{@"cross_commit_spacing":@YES, @"cross_commit_spacing_excluded_apps":@[]}];
    prefs.inputScheme = @"quanpin";
    prefs.englishMode = YES;
    prefs.fullWidthInput = NO;
    SpacingQueryShortcutClient *client = [SpacingQueryShortcutClient new];
    client.bundleIdentifier = @"com.example.Writer";
    client.document = @"用";
    client.selection = NSMakeRange(1, 0);
    ModeController *controller = [ModeController alloc];
    [controller setValue:prefs forKey:@"appearance"];
    [controller setValue:client forKey:@"activeClient"];
    __weak SpacingQueryShortcutClient *weakClient = client;
    // 查询发生在原字落地后；新事件无需排队或重放。
    client.duringContextRead = ^{
        SpacingQueryShortcutClient *client = weakClient;
        assert([client.document isEqual:@"用R"]);
        assert([controller handleEvent:LayoutKey(32, @"u", 0) client:client]);
        assert(![controller handleEvent:LayoutKey(49, @" ", 0) client:client]);
        [client insertText:@" " replacementRange:NSMakeRange(NSNotFound, NSNotFound)];
    };
    assert([controller handleEvent:LayoutKey(15, @"R", NSEventModifierFlagShift) client:client]);
    assert([client.document isEqual:@"用Ru "]);
    // 原始写入本身重入，也不能让外层事后改写较新的文字。
    client.document = @"用";
    client.selection = NSMakeRange(1, 0);
    client.afterInsert = ^{
        SpacingQueryShortcutClient *client = weakClient;
        assert([controller handleEvent:LayoutKey(32, @"u", 0) client:client]);
        assert([controller handleEvent:LayoutKey(1, @"s", 0) client:client]);
    };
    assert([controller handleEvent:LayoutKey(15, @"R", NSEventModifierFlagShift) client:client]);
    assert([client.document isEqual:@"用Rus"]);
    // 退格和导航维持原来的 NO；查询过期后不补写被删掉的 R。
    client.document = @"用";
    client.selection = NSMakeRange(1, 0);
    client.duringContextRead = ^{
        SpacingQueryShortcutClient *client = weakClient;
        assert(![controller handleEvent:LayoutKey(51, @"\x7f", 0) client:client]);
        [client insertText:@"" replacementRange:NSMakeRange(client.selection.location - 1, 1)];
        assert(![controller handleEvent:LayoutKey(123, @"\uf702", 0) client:client]);
        client.selection = NSMakeRange(0, 0);
    };
    assert([controller handleEvent:LayoutKey(15, @"R", NSEventModifierFlagShift) client:client]);
    assert([client.document isEqual:@"用"] && client.selection.location == 0);
    // 普通上屏查询里重入新组字，不让旧上屏清掉它的标记或视图。
    client.document = @"Rust";
    client.selection = NSMakeRange(4, 0);
    prefs.englishMode = NO;
    NSDictionary *nextView = @{@"scheme":@0, @"editing_text":@"n", @"preedit":@"n", @"candidates":@[]};
    client.duringContextRead = ^{ [controller apply:@{@"view":nextView}]; };
    [controller apply:@{@"commit":@"写", @"commit_context":@{@"scheme":@0, @"local_mode":@"none"},
                        @"view":@{@"editing_text":@"", @"candidates":@[]}}];
    assert([client.document isEqual:@"Rust写"] && [client.marked isEqual:@"n"]);
    assert([[controller valueForKey:@"view"] isEqual:nextView]);
    // 最后的显式替换同样可能重入；替换完成后不能再画旧空标记。
    client.document = @"Rust";
    client.selection = NSMakeRange(4, 0);
    client.duringReplacement = ^{ [controller apply:@{@"view":nextView}]; };
    [controller apply:@{@"commit":@"写", @"commit_context":@{@"scheme":@0, @"local_mode":@"none"},
                        @"view":@{@"editing_text":@"", @"candidates":@[]}}];
    assert([client.document isEqual:@"Rust 写"] && [client.marked isEqual:@"n"]);
    assert([[controller valueForKey:@"view"] isEqual:nextView]);
    // 无副作用的抬键只取消格式化，不能阻止已完成上屏清理旧候选。
    NSDictionary *idleAfterCommit = @{@"scheme":@0, @"editing_text":@"", @"candidates":@[]};
    [controller setValue:nextView forKey:@"view"];
    client.afterInsert = ^{
        SpacingQueryShortcutClient *client = weakClient;
        NSEvent *up = [NSEvent keyEventWithType:NSEventTypeKeyUp location:NSZeroPoint modifierFlags:0 timestamp:0
            windowNumber:0 context:nil characters:@"r" charactersIgnoringModifiers:@"r" isARepeat:NO keyCode:15];
        [controller handleEvent:up client:client];
    };
    [controller apply:@{@"commit":@"写", @"commit_context":@{@"scheme":@0, @"local_mode":@"none"}, @"view":idleAfterCommit}];
    assert([[controller valueForKey:@"view"] isEqual:idleAfterCommit]);
    assert([client.marked isEqual:@""]);
    // 查询回调里变更开关也取消剩余读取及格式化。
    client.document = @"Rust";
    client.selection = NSMakeRange(4, 0);
    client.duringContextRead = ^{ [prefs applySharedInputPreferences:@{}]; };
    [controller apply:@{@"commit":@"写", @"commit_context":@{@"scheme":@0, @"local_mode":@"none"}, @"view":idleAfterCommit}];
    assert([client.document isEqual:@"Rust写"]);
    [prefs applySharedInputPreferences:@{@"cross_commit_spacing":@YES, @"cross_commit_spacing_excluded_apps":@[]}];
    // 安全或焦点变化只取消格式化，已经落地的原文保留。
    client.document = @"Rust";
    client.selection = NSMakeRange(4, 0);
    prefs.englishMode = NO;
    client.duringContextRead = ^{ controller.secureInput = YES; };
    NSDictionary *idle = @{@"editing_text":@"", @"candidates":@[]};
    [controller apply:@{@"commit":@"写", @"commit_context":@{@"scheme":@0, @"local_mode":@"none"}, @"view":idle}];
    assert([client.document isEqual:@"Rust写"]);
    controller.secureInput = NO;
    prefs.englishMode = YES;
    ShortcutClient *other = [ShortcutClient new];
    client.duringBundleRead = ^{ [controller setValue:other forKey:@"activeClient"]; };
    assert([controller handleEvent:LayoutKey(15, @"R", NSEventModifierFlagShift) client:client]);
    assert([client.document isEqual:@"Rust写R"]);
    [controller setValue:client forKey:@"activeClient"];
    [controller setValue:nil forKey:@"view"];
    prefs.inputScheme = @"japanese";
    NSUInteger reads = client.documentReads;
    assert(![controller handleEvent:LayoutKey(15, @"R", NSEventModifierFlagShift) client:client]);
    assert(client.documentReads == reads);
    MSIMERemoveTestPreferenceSuite(defaults, suite);
}

@interface SpacingTextView : NSTextView <MSIMETextClient>
@end
@implementation SpacingTextView
- (void)setMarkedText:(id)text selectionRange:(NSRange)selection replacementRange:(NSRange)replacement {
    [self setMarkedText:text selectedRange:selection replacementRange:replacement];
}
- (NSString *)bundleIdentifier { return @"com.example.Writer"; }
- (NSAttributedString *)attributedSubstringFromRange:(NSRange)range {
    return [self attributedSubstringForProposedRange:range actualRange:NULL];
}
@end

static void TestCrossCommitSpacingInTextView() {
    NSString *suite = [@"msime.cross-commit-spacing-appkit." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    MSIMEAppearancePreferences *prefs = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    [prefs activateInputModeForApplication:@"com.example.Writer"];
    [prefs applySharedInputPreferences:@{@"cross_commit_spacing":@YES, @"cross_commit_spacing_excluded_apps":@[]}];
    prefs.inputScheme = @"quanpin";
    prefs.englishMode = YES;
    prefs.fullWidthInput = NO;
    SpacingTextView *textView = [[SpacingTextView alloc] initWithFrame:NSMakeRect(0, 0, 300, 100)];
    textView.string = @"用";
    textView.selectedRange = NSMakeRange(1, 0);
    ModeController *controller = [ModeController alloc];
    [controller setValue:prefs forKey:@"appearance"];
    [controller setValue:textView forKey:@"activeClient"];
    for (NSString *letter in @[@"R", @"u", @"s", @"t"])
        assert([controller handleEvent:LayoutKey(32, letter, 0) client:textView]);
    prefs.englishMode = NO;
    NSDictionary *idle = @{@"scheme":@0, @"editing_text":@"", @"candidates":@[]};
    [controller apply:@{@"commit":@"写", @"commit_context":@{@"scheme":@0, @"local_mode":@"none"}, @"view":idle}];
    assert([textView.string isEqual:@"用 Rust 写"] && textView.selectedRange.location == textView.string.length);
    // 真实 marked text 被替换后读取刚上屏部分，既不读拼音，也不改右侧文本。
    textView.string = @"Rust右";
    textView.selectedRange = NSMakeRange(4, 0);
    [textView setMarkedText:@"xie" selectionRange:NSMakeRange(3, 0) replacementRange:NSMakeRange(NSNotFound, NSNotFound)];
    [controller setValue:@YES forKey:@"clientHasMarkedText"];
    [controller apply:@{@"commit":@"写", @"commit_context":@{@"scheme":@0, @"local_mode":@"none"}, @"view":idle}];
    assert([textView.string isEqual:@"Rust 写右"] && textView.selectedRange.location == 6);
    MSIMERemoveTestPreferenceSuite(defaults, suite);
}

static void TestCrossCommitSpacing() {
    NSString *suite = [@"msime.cross-commit-spacing." stringByAppendingString:NSUUID.UUID.UUIDString];
    NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
    MSIMEAppearancePreferences *prefs = [[MSIMEAppearancePreferences alloc] initWithDefaults:defaults];
    [prefs activateInputModeForApplication:@"com.example.Writer"];
    [prefs applySharedInputPreferences:@{@"cross_commit_spacing":@YES, @"cross_commit_spacing_excluded_apps":@[]}];
    prefs.englishMode = YES;
    prefs.fullWidthInput = NO;
    ApplicationShortcutClient *client = [ApplicationShortcutClient new];
    client.bundleIdentifier = @"com.example.Writer";
    client.document = @"用";
    client.selection = NSMakeRange(1, 0);
    ModeController *controller = [ModeController alloc];
    [controller setValue:prefs forKey:@"appearance"];
    [controller setValue:client forKey:@"activeClient"];
    NSDictionary *idle = @{@"scheme":@0, @"editing_text":@"", @"candidates":@[]};
    [controller setValue:idle forKey:@"view"];
    assert([controller handleEvent:LayoutKey(15, @"R", NSEventModifierFlagShift) client:client]);
    assert([client.document isEqual:@"用 R"]);
    assert(![controller valueForKey:@"session"]);
    for (NSString *letter in @[@"u", @"s", @"t"]) {
        assert([controller handleEvent:LayoutKey(32, letter, 0) client:client]);
    }
    prefs.englishMode = NO;
    [controller apply:@{@"commit":@"写", @"commit_context":@{@"scheme":@0, @"local_mode":@"none"}, @"view":idle}];
    assert([client.document isEqual:@"用 Rust 写"]);
    [controller apply:@{@"commit":@"Rust", @"commit_context":@{@"scheme":@0, @"local_mode":@"temporary_english"}, @"view":idle}];
    assert([client.document isEqual:@"用 Rust 写 Rust"]);
    [prefs applySharedInputPreferences:@{@"cross_commit_spacing":@YES, @"cross_commit_spacing_excluded_apps":@[@"COM.EXAMPLE.WRITER"]}];
    NSUInteger reads = client.documentReads;
    [controller apply:@{@"commit":@"字", @"commit_context":@{@"scheme":@0, @"local_mode":@"none"}, @"view":idle}];
    assert([client.document hasSuffix:@"Rust字"] && client.documentReads == reads);
    prefs.englishMode = YES;
    assert(![controller handleEvent:LayoutKey(15, @"R", NSEventModifierFlagShift) client:client]);
    assert(client.documentReads == reads);
    prefs.englishMode = NO;
    [prefs applySharedInputPreferences:@{@"cross_commit_spacing":@YES, @"cross_commit_spacing_excluded_apps":@[]}];
    for (NSString *mode in @[@"url", @"date_time", @"calculator", @"temporary_japanese"]) {
        reads = client.documentReads;
        [controller apply:@{@"commit":@"A", @"commit_context":@{@"scheme":@0, @"local_mode":mode}, @"view":idle}];
        assert([client.committed isEqual:@"A"] && client.documentReads == reads);
    }
    controller.secureInput = YES;
    reads = client.documentReads;
    [controller apply:@{@"commit":@"字", @"commit_context":@{@"scheme":@0, @"local_mode":@"none"}, @"view":idle}];
    assert([client.committed isEqual:@"字"] && client.documentReads == reads);
    controller.secureInput = NO;
    prefs.fullWidthInput = YES;
    reads = client.documentReads;
    [controller apply:@{@"commit":@"A", @"commit_context":@{@"scheme":@0, @"local_mode":@"none"}, @"view":idle}];
    assert([client.committed isEqual:@"A"] && client.documentReads == reads);
    prefs.fullWidthInput = NO;
    // 鼠标选择替换已有文字，使用替换起点之前的实时前文，不使用上次上屏的影子。
    client.document = @"Rust旧";
    client.selection = NSMakeRange(4, 1);
    [controller apply:@{@"commit":@"新", @"commit_context":@{@"scheme":@0, @"local_mode":@"none"}, @"view":idle}];
    assert([client.document isEqual:@"Rust 新"]);
    [prefs applySharedInputPreferences:@{}];
    reads = client.documentReads;
    [controller apply:@{@"commit":@"A", @"commit_context":@{@"scheme":@0, @"local_mode":@"none"}, @"view":idle}];
    assert([client.committed isEqual:@"A"] && client.documentReads == reads);
    MSIMERemoveTestPreferenceSuite(defaults, suite);
}

int main(int argc, char **argv) {
    const IsolatedUserHome userHome;
    const IsolatedDefaultSkinsRoot skinsRoot;
    assert(msime::mac::DefaultSkinsRoot() == skinsRoot.path);
    assert(!MSIMEShouldRegisterInputSource(1, nullptr));
    const char *registerArguments[] = {"test", "--register-input-source"};
    assert(MSIMEShouldRegisterInputSource(2, registerArguments));
    @autoreleasepool {
        [NSApplication sharedApplication];
        if (argc == 2 && std::string(argv[1]) == "--cross-commit-spacing") {
            TestCrossCommitSpacing();
            TestCrossCommitSpacingReentrancy();
            TestCrossCommitSpacingInTextView();
            return 0;
        }
        if (argc == 2 && std::string(argv[1]) == "--expanded-scroll") {
            TestExpandedCandidateGrid();
            TestExpandedCandidateGridGeneration();
            return 0;
        }
        if (argc == 2 && std::string(argv[1]) == "--session-recovery") {
            TestSessionUnavailableIsReported();
            TestIdleSessionRecovery();
            return 0;
        }
        if (argc == 2 && std::string(argv[1]) == "--english-punctuation") {
            @autoreleasepool { TestPerApplicationPunctuationAndWidth(); }
            @autoreleasepool { TestEnglishModePunctuationAndWidthOutput(); }
            return 0;
        }
        if (argc == 2 && std::string(argv[1]) == "--caps-lock") {
            TestCapsLockTap();
            return 0;
        }
        // Each case drains its own pool. No run loop turns between cases, so under one outer pool every autoreleased preferences window lived to the end of main: about a hundred piled up, every later window paid for all of their notification observers and weak references, and the run grew quadratically until the sanitizer leg crossed its 240 s ceiling.
        if (argc == 2 && std::string(argv[1]) == "--input-source-restore") {
            TestRestoredInputModeActivation();
            return 0;
        }
        if (argc == 2 && std::string(argv[1]) == "--shift-corner-quotes") {
            TestShiftCornerQuotesThroughRealSession();
            return 0;
        }
        if (argc == 2 && std::string(argv[1]) == "--temporary-japanese-long-vowel") {
            @autoreleasepool { TestTemporaryJapaneseLongVowelThroughRealSession(NO); }
            @autoreleasepool { TestTemporaryJapaneseLongVowelThroughRealSession(YES); }
            return 0;
        }
        // 可重绑切换热键的重绑路径单独可跑：全量 suite 里皮肤颜色断言在无图形会话的机器上先于它中止。
        if (argc == 2 && std::string(argv[1]) == "--editable-shift") {
            TestEditableShiftController();
            return 0;
        }
        if (argc == 2 && std::string(argv[1]) == "--toggle-hotkeys") {
            TestToggleHotkeyRebinding();
            return 0;
        }
        if (argc == 2 && std::string(argv[1]) == "--vim-mode") {
            TestVimModeEscapeRouting();
            return 0;
        }
        if (argc == 2 && std::string(argv[1]) == "--glass-visual") {
            TestGlassInputModeHUD(YES);
            return 0;
        }
        if (argc == 2 && std::string(argv[1]) == "--key-focus-preferences") {
            TestKeyFocusRestoresPreferenceMonitoring();
            return 0;
        }
        if (argc == 2 && std::string(argv[1]) == "--english-online") {
            TestEnglishOnlineCandidatesThroughRealSession();
            return 0;
        }
        if (argc == 2 && std::string(argv[1]) == "--nested-commit") {
            TestNestedCommitKeepsTheNewerComposition();
            return 0;
        }
        if (argc == 2 && std::string(argv[1]) == "--translation-font-size") {
            TestCandidateTranslationFontSize();
            return 0;
        }
        if (argc == 2 && std::string(argv[1]) == "--translations") {
            @autoreleasepool { TestPreferenceLoadReusesCandidateServiceSnapshots(); }
            @autoreleasepool { TestCandidateServiceSnapshotsAreReused(); }
            @autoreleasepool { TestApplyCandidateTranslationSnapshotsAreReused(); }
            @autoreleasepool { TestEmptyGlossDoesNotApplyAStaleServiceGeneration(); }
            @autoreleasepool { TestGlossScheduling(); }
            @autoreleasepool { TestGlossSurvivesHighlightMove(); }
            @autoreleasepool { TestAccountGlossSkipsNonChineseCandidates(); }
            @autoreleasepool { TestAccountGlossRequiresExplicitChoice(); }
            @autoreleasepool { TestAccountGlossCacheIsSharedAcrossControllers(); }
            @autoreleasepool { TestAccountGlossLateReplyAndNegatives(); }
            @autoreleasepool { TestAccountGlossIdleDelay(); }
            @autoreleasepool { TestAccountGlossPersistsOnArrival(); }
            @autoreleasepool { TestAccountGlossWaitsForDictionary(); }
            @autoreleasepool { TestOfflineTargetGlosses(); }
            @autoreleasepool { TestOnDeviceGlosses(); }
            @autoreleasepool { TestOnDeviceGlossPersistence(); }
            @autoreleasepool { TestOnDeviceGlossDuplicateTextKeepsRequestMetadata(); }
            @autoreleasepool { TestCustomTranslationController(); }
            @autoreleasepool { TestGlossLinesSurviveSession(); }
            @autoreleasepool { TestSecondaryTranslationScheduling(); }
            @autoreleasepool { TestCustomTranslationCacheDelivery(); }
            @autoreleasepool { TestCustomTranslationCacheSeparatesCredentialsAcrossControllers(); }
            @autoreleasepool { TestCustomTranslationIdleDelay(NO); }
            @autoreleasepool { TestCustomTranslationIdleDelay(YES); }
            @autoreleasepool { TestTencentCandidateScheduling(); }
            @autoreleasepool { TestNiuTransCandidateScheduling(); }
            @autoreleasepool { TestLearnedGlossRuntime(); }
            @autoreleasepool { TestCandidateTranslationPreference(); }
            @autoreleasepool { TestGlossModePolicy(); }
            return 0;
        }
        NSUserDefaults *standardDefaults = NSUserDefaults.standardUserDefaults;
        NSDictionary *previousVoiceHoldKeys = MSIMEDisableVoiceHoldKeys(standardDefaults);
        @autoreleasepool { TestSessionUnavailableIsReported(); }
        @autoreleasepool { TestIdleSessionRecovery(); }
        @autoreleasepool { TestCloudCandidateScheduling(); }
        @autoreleasepool { TestCloudCandidateRetryAfterRejectedResponse(); }
        @autoreleasepool { TestCloudCandidateEngineDelivery(); }
        @autoreleasepool { TestAiCandidateScheduling(); }
        @autoreleasepool { TestAiCandidateTimerFiresWhileTracking(); }
        @autoreleasepool { TestAiCandidatesIgnoreGlossSwitch(); }
        @autoreleasepool { TestAiCandidateRetryAfterRejectedResponse(); }
        @autoreleasepool { TestAnEmptyAiAnswerIsNotAskedAgain(); }
        @autoreleasepool { TestAiCacheIgnoresUserDictionaryRows(); }
        @autoreleasepool { TestAiCandidateCacheAcrossGenerations(); }
        @autoreleasepool { TestAiCandidateCacheDoesNotSurviveCredentialRotation(); }
        @autoreleasepool { TestAiCandidateCacheTracksProviderSettings(); }
        @autoreleasepool { TestAiCandidateDescriptorFailureIsRetryable(); }
        @autoreleasepool { TestAiCandidateEngineDelivery(); }
        @autoreleasepool { TestCloudCandidatePreference(); }
        @autoreleasepool { TestCloudCandidateConsent(); }
        @autoreleasepool { TestGlossScheduling(); }
        @autoreleasepool { TestGlossSurvivesHighlightMove(); }
        @autoreleasepool { TestAccountGlossSkipsNonChineseCandidates(); }
        @autoreleasepool { TestAccountGlossRequiresExplicitChoice(); }
        @autoreleasepool { TestAccountGlossCacheIsSharedAcrossControllers(); }
        @autoreleasepool { TestAccountGlossLateReplyAndNegatives(); }
        @autoreleasepool { TestAccountGlossIdleDelay(); }
        @autoreleasepool { TestAccountGlossWaitsForDictionary(); }
        @autoreleasepool { TestOfflineTargetGlosses(); }
        @autoreleasepool { TestOnDeviceGlosses(); }
        @autoreleasepool { TestOnDeviceGlossPersistence(); }
        @autoreleasepool { TestOnDeviceGlossDuplicateTextKeepsRequestMetadata(); }
        @autoreleasepool { TestCustomTranslationController(); }
        @autoreleasepool { TestGlossLinesSurviveSession(); }
        @autoreleasepool { TestSecondaryTranslationScheduling(); }
        @autoreleasepool { TestCustomTranslationCacheDelivery(); }
        @autoreleasepool { TestCustomTranslationCacheSeparatesCredentialsAcrossControllers(); }
        @autoreleasepool { TestCustomTranslationIdleDelay(NO); }
        @autoreleasepool { TestCustomTranslationIdleDelay(YES); }
        @autoreleasepool { TestTencentCandidateScheduling(); }
        @autoreleasepool { TestNiuTransCandidateScheduling(); }
        @autoreleasepool { TestLearnedGlossRuntime(); }
        @autoreleasepool { TestCandidateTranslationPreference(); }
        @autoreleasepool { TestGlossModePolicy(); }
        @autoreleasepool { TestSharedInputPreferences(); }
        @autoreleasepool { TestSharedWubiProfile(); }
        @autoreleasepool { TestIndependentAssistancePreferences(); }
        @autoreleasepool { TestSharedPunctuation(); }
        @autoreleasepool { TestSmartPunctuationWritesBothHalves(); }
        @autoreleasepool { TestSharedTraditionalOutput(); }
        @autoreleasepool { TestCandidatePageNumberPreference(); }
        @autoreleasepool { TestPageSizeCache(); }
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
        appearance.pageSize = 11;
        assert(appearance.pageSize == 10);
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
        // 三到十：参考实现的三到九，加上 0 键选第十个的 10（#6679）。
        assert(sizeControl.numberOfItems == (NSInteger)msime::mac::kOfferedCandidatePageSizes);
        NSArray *pageSizes = @[@3, @4, @5, @6, @7, @8, @9, @10];
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
        // 夹具直接注入外观，跳过 ensureAppearance；经生产偏好入口补齐候选显示的出厂状态。
        NSDictionary *candidateDisplayDefaults = @{
            @"candidate_translation_show_pos": @YES, @"candidate_translation_single_sense": @NO,
            @"candidate_fixed_badge": @YES, @"candidate_fixed_badge_style": @"paperclip",
            @"candidate_ai_badge_style": @"robot"
        };
        [controller applySharedToolbarPreferences:candidateDisplayDefaults];
        [NSUserDefaults.standardUserDefaults removeObjectForKey:@"MSIMEClientPinnedCandidates"];
        [controller syncPageSize];
        // 上面逐项点过每页候选的弹出菜单，停在最后一项 10。
        assert(session.requestedPageSize == 10);
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
        // 清掉前一组快捷键的上屏答复，光标命令的替身不返回上屏。
        session.nextTransition = @{@"handled": @YES, @"view": @{@"focused": @YES, @"editing_text": @"ceshi", @"caret_position": @3, @"candidates": @[]}};
        // macOS 普通左右键编辑拼音，候选窗可见或隐藏时都不进入整句改字，也不上屏。
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
            assert(client.committed == nil);
            panel.visible = NO;
            session.lastCommand = UINT32_MAX;
            assert([controller handleEvent:event client:client]);
            assert(session.lastCommand == (key.unsignedShortValue == 123 ? MSIME_MOVE_LEFT : MSIME_MOVE_RIGHT));
            assert(client.committed == nil);
        }
        session.nextTransition = nil;
        appearance.vertical = arrowVertical;
        HiddenCandidatePanel *layoutPanel = [[HiddenCandidatePanel alloc] initWithContentRect:NSZeroRect styleMask:NSWindowStyleMaskBorderless | NSWindowStyleMaskNonactivatingPanel backing:NSBackingStoreBuffered defer:NO];
        for (NSNumber *key in @[@115, @119]) {
            NSEvent *event = [NSEvent keyEventWithType:NSEventTypeKeyDown location:NSZeroPoint modifierFlags:0 timestamp:0 windowNumber:0 context:nil characters:@"" charactersIgnoringModifiers:@"" isARepeat:NO keyCode:key.unsignedShortValue];
            panel.visible = YES;
            assert([controller handleEvent:event client:client]);
            assert(session.lastCommand == (key.unsignedShortValue == 115 ? MSIME_FIRST_CANDIDATE_OR_HOME : MSIME_LAST_CANDIDATE));
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
        NSButton *rendered = (NSButton *)MSIMECandidateControlsView(layoutPanel.contentView).subviews.firstObject;
        assert(rendered.font.pointSize == 18);
        assert(fabs(((MSIMECandidateButton *)rendered).numberFont.pointSize - 14.4) < 0.01);
        assert([rendered.toolTip isEqualToString:@"合成候选布局测试文本"]);
        // Candidates wrap inside their column instead of being cut off with an ellipsis.
        assert(rendered.lineBreakMode == NSLineBreakByWordWrapping);
        [controller setValue:@{@"candidates": @[@{@"text": @"测试", @"highlighted": @YES}]} forKey:@"view"];
        [controller renderCandidates];
        assert(layoutPanel.requestedVisible && fabs(layoutPanel.frame.size.width - shortWidth) < 0.01);
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
        // 面板不隐藏时，翻到短候选页和同页候选变短都必须收窄；翻回长候选页仍能变宽。
        const CGFloat shortPageWidth = layoutPanel.frame.size.width;
        NSMutableDictionary *widthView = [pageView mutableCopy];
        widthView[@"candidates"] = @[@{@"text": @"合成候选布局测试文本", @"highlighted": @YES}];
        [controller setValue:[widthView copy] forKey:@"view"];
        [controller renderCandidates];
        const CGFloat longPageWidth = layoutPanel.frame.size.width;
        assert(longPageWidth > shortPageWidth && layoutPanel.requestedVisible);
        for (NSNumber *pageIndex in @[@1, @0]) {
            widthView[@"page"] = pageIndex;
            widthView[@"candidates"] = pageView[@"candidates"];
            [controller setValue:[widthView copy] forKey:@"view"];
            [controller renderCandidates];
            assert(layoutPanel.requestedVisible && fabs(layoutPanel.frame.size.width - shortPageWidth) < 0.01);
            MSIMECandidateButton *widthNext = (id)PageButton(layoutPanel.contentView, -2);
            assert(NSMaxX(widthNext.frame) <= layoutPanel.contentView.bounds.size.width);
            widthView[@"candidates"] = @[@{@"text": @"合成候选布局测试文本", @"highlighted": @YES}];
            [controller setValue:[widthView copy] forKey:@"view"];
            [controller renderCandidates];
            assert(layoutPanel.requestedVisible && fabs(layoutPanel.frame.size.width - longPageWidth) < 0.01);
        }
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
        for (NSView *child in MSIMECandidateControlsView(layoutPanel.contentView).subviews)
            if ([child.identifier isEqual:@"candidate-page-indicator"]) pageIndicator = (id)child;
        assert(pageIndicator && [pageIndicator.stringValue isEqual:@"1 / 3"]);
        assert([pageIndicator.accessibilityLabel isEqual:@"第 1 页，共 3 页"]);
        MSIMECandidateButton *pagedCandidate = PageButton(layoutPanel.contentView, 0);
        assert(NSMinY(previous.frame) >= NSMaxY(pagedCandidate.frame) && NSMinY(next.frame) == NSMinY(previous.frame));
        assert(NSMaxX(next.frame) <= layoutPanel.contentView.bounds.size.width && NSMaxX(pageIndicator.frame) <= NSMinX(previous.frame));
        // 页码与箭头共用 13pt 次要色，玻璃模式使用同一份高对比覆盖。
        auto pageTokens = [appearance resolvedSkinForDark:[[layoutPanel.contentView.effectiveAppearance bestMatchFromAppearancesWithNames:@[NSAppearanceNameAqua, NSAppearanceNameDarkAqua]] isEqual:NSAppearanceNameDarkAqua]].tokens;
        if (((MSIMECandidateChromeView *)layoutPanel.contentView).liquidGlassActive)
            msime::mac::ApplyNativeGlassCandidateContrast(pageTokens, [[layoutPanel.contentView.effectiveAppearance bestMatchFromAppearancesWithNames:@[NSAppearanceNameAqua, NSAppearanceNameDarkAqua]] isEqual:NSAppearanceNameDarkAqua]);
        assert(next.titleColor && [next.titleColor isEqual:SkinColor(pageTokens.number)]);
        assert([pageIndicator.textColor isEqual:SkinColor(pageTokens.number)] && pageIndicator.font.pointSize == 13);
        {
            // 关掉 `show_candidate_page_number` 时「1 / 3」和 ‹ › 一起不画；logo 和拼音也隐藏时首行不占高度，多页卡片和只有一页的一样高（#7316）。滚轮翻页照常。
            const BOOL savedPreedit = appearance.showsCandidatePreedit;
            const BOOL savedLogo = appearance.showsAppLogo;
            appearance.showsCandidatePreedit = NO;
            appearance.showsAppLogo = NO;
            NSMutableDictionary *singlePageView = [pageView mutableCopy];
            singlePageView[@"page_count"] = @1;
            [controller setValue:[singlePageView copy] forKey:@"view"];
            [controller renderCandidates];
            const CGFloat singlePageHeight = layoutPanel.frame.size.height;
            [controller setValue:[pageView copy] forKey:@"view"];
            [controller renderCandidates];
            assert(PageButton(layoutPanel.contentView, -1) && layoutPanel.frame.size.height > singlePageHeight);
            appearance.showsCandidatePageNumber = NO;
            [controller renderCandidates];
            assert(!PageButton(layoutPanel.contentView, -1) && !PageButton(layoutPanel.contentView, -2));
            for (NSView *child in MSIMECandidateControlsView(layoutPanel.contentView).subviews) assert(![child.identifier isEqual:@"candidate-page-indicator"]);
            assert(layoutPanel.frame.size.height == singlePageHeight);
            assert(!layoutPanel.hasPreviousPage && layoutPanel.hasNextPage);
            appearance.showsCandidatePageNumber = YES;
            [controller renderCandidates];
            assert(PageButton(layoutPanel.contentView, -1));
            appearance.showsCandidatePreedit = savedPreedit;
            appearance.showsAppLogo = savedLogo;
            [controller renderCandidates];
            previous = (id)PageButton(layoutPanel.contentView, -1);
            next = (id)PageButton(layoutPanel.contentView, -2);
        }
        // Fractional page counts are malformed JSON integers. Rendering must not truncate 2.5
        // into a usable two-page card and expose navigation for it.
        NSMutableDictionary *fractionalPageView = [pageView mutableCopy];
        fractionalPageView[@"page_count"] = @2.5;
        layoutPanel.contentView = nil;
        [controller setValue:fractionalPageView forKey:@"view"];
        [controller renderCandidates];
        assert(!PageButton(layoutPanel.contentView, -1) && !PageButton(layoutPanel.contentView, -2));
        [controller setValue:[pageView copy] forKey:@"view"];
        [controller renderCandidates];
        // A layout-only render can keep all Engine IDs unchanged. The detached
        // button must still be rejected, just like detached candidate buttons.
        MSIMECandidateButton *oldPageButton = next;
        [controller renderCandidates];
        // 被拒绝的翻页点击不派发，也不报告 MSIME_KEY_POINTER：报告和派发的条件必须一致。
        NSUInteger pageKeys = session.keys.count;
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
        for (NSNumber *invalidCount in @[@0, @1, @2.5]) {
            NSMutableDictionary *bad = [pageView mutableCopy];
            bad[@"page"] = @1; bad[@"page_count"] = invalidCount;
            next.candidateID = bad;
            [controller setValue:bad forKey:@"view"];
            session.lastCommand = UINT32_MAX;
            [controller changeCandidatePage:next];
            assert(session.lastCommand == UINT32_MAX);
        }
        assert(session.keys.count == pageKeys);
        next.candidateID = pageIdentity;
        [controller setValue:[pageView copy] forKey:@"view"];
        pageView[@"page"] = @1;
        session.nextTransition = @{@"handled": @YES, @"commit": NSNull.null, @"view": [pageView copy]};
        client.committed = nil;
        [controller setValue:@0 forKey:@"fieldTraits"];
        [next performClick:nil];
        assert(session.lastCommand == MSIME_NEXT_PAGE && client.committed == nil);
        // 指针翻页同样先报告变了的字段属性和 MSIME_KEY_POINTER。
        NSString *pageTraits = [NSString stringWithFormat:@"traits:%x", [[controller valueForKey:@"fieldTraits"] unsignedIntValue]];
        assert(([[session.calls subarrayWithRange:NSMakeRange(session.calls.count - 3, 3)] isEqual:@[pageTraits, [NSString stringWithFormat:@"key:%u", MSIME_KEY_POINTER], [NSString stringWithFormat:@"command:%u", MSIME_NEXT_PAGE]]]));
        assert([client.marked isEqual:@"ceshi"]);
        pageKeys = session.keys.count;
        // A previous page's retained button must not advance the current page.
        session.lastCommand = UINT32_MAX;
        [controller changeCandidatePage:next];
        assert(session.lastCommand == UINT32_MAX && session.keys.count == pageKeys);
        previous = (id)PageButton(layoutPanel.contentView, -1);
        next = (id)PageButton(layoutPanel.contentView, -2);
        assert(previous.enabled && next.enabled);
        pageView[@"page"] = @0;
        session.nextTransition = @{@"handled": @YES, @"commit": NSNull.null, @"view": [pageView copy]};
        [previous performClick:nil];
        assert(session.lastCommand == MSIME_PREVIOUS_PAGE);
        // 面板的 pageHandler（滚轮和触控板翻页）同样先报告 MSIME_KEY_POINTER 再派发。它只挂在控制器自己建的面板上，所以让控制器建一次面板把它取出来，再换回测试面板。
        [controller setValue:[pageView copy] forKey:@"view"];
        [controller setValue:nil forKey:@"panel"];
        [controller renderCandidates];
        MSIMECandidatePanel *ownPanel = [controller valueForKey:@"panel"];
        // 候选窗开在截图工具遮罩和屏保层级之上，仍在光标之下（#7326）。
        assert(ownPanel.level == MSIMECandidateWindowLevel() && ownPanel.level > NSScreenSaverWindowLevel &&
               ownPanel.level < CGWindowLevelForKey(kCGCursorWindowLevelKey));
        void (^pageHandler)(BOOL) = ownPanel.pageHandler;
        [ownPanel orderOut:nil];
        [controller setValue:layoutPanel forKey:@"panel"];
        assert(pageHandler && ownPanel != layoutPanel);
        NSMutableDictionary *secondPage = [pageView mutableCopy];
        secondPage[@"page"] = @1;
        session.nextTransition = @{@"handled": @YES, @"commit": NSNull.null, @"view": [secondPage copy]};
        session.lastCommand = UINT32_MAX;
        pageHandler(NO);
        assert(session.lastCommand == MSIME_NEXT_PAGE);
        assert(([[session.calls subarrayWithRange:NSMakeRange(session.calls.count - 2, 2)] isEqual:@[[NSString stringWithFormat:@"key:%u", MSIME_KEY_POINTER], [NSString stringWithFormat:@"command:%u", MSIME_NEXT_PAGE]]]));
        // 面板不可见时它什么都不做，也不报告。
        pageKeys = session.keys.count;
        layoutPanel.requestedVisible = NO;
        session.lastCommand = UINT32_MAX;
        pageHandler(NO);
        assert(session.lastCommand == UINT32_MAX && session.keys.count == pageKeys);
        layoutPanel.requestedVisible = YES;
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
        assert(session.keys.count == pageKeys);
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
            for (NSView *child in MSIMECandidateControlsView(layoutPanel.contentView).subviews) {
                if ([child.identifier isEqual:@"candidate-preedit"]) assert(child.frame.size.height >= fallbackHeight + 6);
                if ([child isKindOfClass:MSIMECandidateButton.class]) assert(child.frame.size.height >= fallbackHeight + 12);
            }
        }
        appearance.fontSize = 18;
        {
            // 首行：拼音后面竖线隔开输入状态，右侧依次是云候选预览和小装饰，首行和候选行之间一条细线。
            assert([MSIMECandidateStatusText(@"小鹤双拼", @{@"script_conversion": @YES}, YES, YES) isEqual:@"小鹤双拼 · 中 · 繁 · 全角"]);
            assert([MSIMECandidateStatusText(@"全拼", @{@"script_conversion": @YES, @"dedicated_english": @YES}, NO, NO) isEqual:@"全拼 · 英"]);
            assert([MSIMECandidateStatusText(@"日语", @{@"script_conversion": @NO}, NO, NO) isEqual:@"日语 · 中"]);
            NSDictionary *cloudRow = @{@"text": @"一行白鹭", @"source": @2, @"id": @{@"session": @1, @"generation": @2, @"index": @2}};
            NSDictionary *aiRow = @{@"text": @"一行代码", @"source": @3, @"id": @{@"session": @1, @"generation": @2, @"index": @1}};
            NSDictionary *placeholder = @{@"text": @"…", @"source": @12, @"id": @{@"session": @1, @"generation": @2, @"index": @3}};
            assert(MSIMECandidateOnlinePreview(@[aiRow, cloudRow]) == cloudRow);
            assert(MSIMECandidateOnlinePreview(@[placeholder, aiRow]) == aiRow);
            assert(!MSIMECandidateOnlinePreview(@[placeholder, @{@"text": @"一行", @"source": @0}]));

            const BOOL savedVertical = appearance.vertical;
            appearance.vertical = NO;
            appearance.showsCandidatePreedit = YES;
            NSMutableDictionary *headerView = [pageView mutableCopy];
            headerView[@"page_count"] = @1;
            headerView[@"scheme"] = @0;
            headerView[@"script_conversion"] = @YES;
            headerView[@"editing_text"] = @"yihang";
            headerView[@"caret_position"] = @6;
            const CGFloat savedPreeditSize = appearance.preeditFontSize;
            appearance.preeditFontSize = 16;
            // 一整页候选，卡片宽度由候选行定，首行放得下全部内容。
            NSMutableArray *headerCandidates = [@[
                @{@"text": @"一样", @"highlighted": @YES, @"source": @0, @"id": @{@"session": @1, @"generation": @2, @"index": @0}},
                aiRow, cloudRow] mutableCopy];
            NSArray<NSString *> *fillers = @[@"移行", @"一行字", @"一行行", @"一行业", @"医行", @"一航"];
            for (NSUInteger index = 0; index < fillers.count; ++index)
                [headerCandidates addObject:@{@"text": fillers[index], @"source": @0, @"id": @{@"session": @1, @"generation": @2, @"index": @(index + 3)}}];
            headerView[@"candidates"] = headerCandidates;
            [controller setValue:[headerView copy] forKey:@"view"];
            [controller renderCandidates];
            NSView *preeditField = nil, *divider = nil, *ornament = nil, *rule = nil;
            NSTextField *status = nil;
            MSIMECandidateButton *preview = PageButton(layoutPanel.contentView, -4), *first = PageButton(layoutPanel.contentView, 0);
            for (NSView *child in MSIMECandidateControlsView(layoutPanel.contentView).subviews) {
                if ([child.identifier isEqual:@"candidate-preedit"]) preeditField = child;
                if ([child.identifier isEqual:@"candidate-status-divider"]) divider = child;
                if ([child.identifier isEqual:@"candidate-status"]) status = (id)child;
                if ([child.identifier isEqual:@"candidate-header-ornament"]) ornament = child;
                if ([child.identifier isEqual:@"candidate-header-rule"]) rule = child;
            }
            assert(preeditField && divider && status && ornament && rule && preview && first);
            assert([status.stringValue isEqual:@"全拼 · 中 · 简"]);
            assert([preview.title isEqual:@"一行白鹭"] && [preview.candidateID isEqual:cloudRow[@"id"]]);
            assert(preview.action == @selector(selectCandidate:) && preview.target == controller);
            assert([preview.accessibilityLabel isEqual:@"云候选：一行白鹭"]);
            // 从左到右：拼音、竖线、状态、预览、装饰，互不重叠，都在卡片里；细线在首行和候选行之间。
            const CGFloat cardWidth = layoutPanel.contentView.bounds.size.width;
            assert(NSMaxX(preeditField.frame) <= NSMinX(divider.frame) && NSMaxX(divider.frame) <= NSMinX(status.frame));
            assert(NSMaxX(status.frame) <= NSMinX(preview.frame) && NSMaxX(preview.frame) <= NSMinX(ornament.frame));
            assert(NSMaxX(ornament.frame) <= cardWidth);
            assert(NSMinY(rule.frame) >= NSMaxY(first.frame) && NSMaxY(rule.frame) <= NSMinY(preeditField.frame) + 1);
            layoutPanel.contentView.appearance = [NSAppearance appearanceNamed:NSAppearanceNameAqua];
            [controller refreshCandidateSkin];
            auto headerTokens = [appearance resolvedSkinForDark:NO].tokens;
            if (((MSIMECandidateChromeView *)layoutPanel.contentView).liquidGlassActive)
                msime::mac::ApplyNativeGlassCandidateContrast(headerTokens, NO);
            assert([status.textColor isEqual:[SkinColor(headerTokens.number) colorWithAlphaComponent:headerTokens.number.a * 0.8]]);
            assert([((MSIMECandidateHeaderOrnamentView *)ornament).tintColor isEqual:SkinColor(headerTokens.accent)]);
            assert([preview.contentTintColor isEqual:SkinColor(headerTokens.accent)]);

            // 没有云候选和 AI 候选时不画预览；英文候选时状态是「英」，不写简繁。首行的内容不改变卡片宽度，中英切换时卡片不伸缩。
            NSMutableArray *plainCandidates = [headerCandidates mutableCopy];
            plainCandidates[1] = @{@"text": @"一行代码", @"source": @0, @"id": aiRow[@"id"]};
            plainCandidates[2] = @{@"text": @"一行白鹭", @"source": @0, @"id": cloudRow[@"id"]};
            headerView[@"candidates"] = plainCandidates;
            [controller setValue:[headerView copy] forKey:@"view"];
            [controller renderCandidates];
            assert(!PageButton(layoutPanel.contentView, -4));
            const CGFloat chineseWidth = layoutPanel.frame.size.width;
            headerView[@"dedicated_english"] = @YES;
            [controller setValue:[headerView copy] forKey:@"view"];
            [controller renderCandidates];
            assert(layoutPanel.frame.size.width == chineseWidth);
            status = nil;
            for (NSView *child in MSIMECandidateControlsView(layoutPanel.contentView).subviews)
                if ([child.identifier isEqual:@"candidate-status"]) status = (id)child;
            assert([status.stringValue isEqual:@"全拼 · 英"]);

            // 关掉候选窗拼音时，状态、预览和装饰随拼音一起不画。
            appearance.showsCandidatePreedit = NO;
            headerView[@"candidates"] = headerCandidates;
            [controller setValue:[headerView copy] forKey:@"view"];
            [controller renderCandidates];
            assert(!PageButton(layoutPanel.contentView, -4));
            for (NSView *child in MSIMECandidateControlsView(layoutPanel.contentView).subviews)
                assert(![child.identifier isEqual:@"candidate-status"] && ![child.identifier isEqual:@"candidate-header-ornament"]);
            appearance.showsCandidatePreedit = YES;
            appearance.preeditFontSize = savedPreeditSize;
            appearance.vertical = savedVertical;

            // 卡片窄到放不下时先让出装饰和状态：只有一个短候选、拼音很长时，拼音拿到首行余下的全部宽度。
            NSMutableDictionary *narrowView = [headerView mutableCopy];
            narrowView[@"dedicated_english"] = @NO;
            narrowView[@"preedit"] = @"yi'hang'bai'lu'shang'qing'tian";
            narrowView[@"editing_text"] = @"yihangbailushangqingtian";
            narrowView[@"caret_position"] = @24;
            narrowView[@"candidates"] = @[headerCandidates[0]];
            [controller setValue:[narrowView copy] forKey:@"view"];
            [controller renderCandidates];
            for (NSView *child in MSIMECandidateControlsView(layoutPanel.contentView).subviews) {
                assert(![child.identifier isEqual:@"candidate-status"] && ![child.identifier isEqual:@"candidate-header-ornament"]);
                if ([child.identifier isEqual:@"candidate-preedit"])
                    assert(NSMaxX(child.frame) <= layoutPanel.contentView.bounds.size.width);
            }
        }
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
            for (NSView *child in MSIMECandidateControlsView(layoutPanel.contentView).subviews)
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
                // 取色器属于自定义主题，选中它时才画。文档里的编号色和边框色在 macOS 上不读，下面比的是解析出的颜色，所以照样成立。
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
                for (NSView *child in chrome.candidateContentView.subviews)
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
                for (NSView *child in MSIMECandidateControlsView(layoutPanel.contentView).subviews)
                    if ([child.identifier isEqual:@"candidate-preedit"]) preeditLabel = (id)child;
                assert(preeditLabel && [preeditLabel isKindOfClass:MSIMECandidatePreeditField.class]);
                caretLabel = (id)preeditLabel;
            }
            assert(NSContainsRect(layoutPanel.contentView.bounds, preeditLabel.frame));
            for (NSView *child in MSIMECandidateControlsView(layoutPanel.contentView).subviews)
                if ([child isKindOfClass:MSIMECandidateButton.class]) {
                    assert(!NSIntersectsRect(child.frame, preeditLabel.frame));
                    assert([((NSButton *)child).font.familyName isEqual:installedFamily]);
                }
            CGFloat shownHeight = layoutPanel.frame.size.height;
            appearance.showsCandidatePreedit = NO;
            [controller renderCandidates];
            assert(layoutPanel.frame.size.height < shownHeight);
            for (NSView *child in MSIMECandidateControlsView(layoutPanel.contentView).subviews) assert(![child.identifier isEqual:@"candidate-preedit"]);
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
            for (NSView *child in MSIMECandidateControlsView(layoutPanel.contentView).subviews)
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
        assert(clickCandidate.menu == nil);
        NSEvent *rightClick = [NSEvent mouseEventWithType:NSEventTypeRightMouseDown location:NSZeroPoint modifierFlags:0 timestamp:0 windowNumber:0 context:nil eventNumber:1 clickCount:1 pressure:1];
        NSMenu *candidateMenu = [clickCandidate menuForEvent:rightClick];
        assert([clickCandidate menuForEvent:rightClick] == candidateMenu);
        // AppKit 把右键菜单开在来源窗口层级加一，候选窗开着菜单时必须留在原层级。降回弹出菜单层级会让候选窗和菜单一起掉到截图遮罩下面（#7326）。
        layoutPanel.level = MSIMECandidateWindowLevel();
        id<NSMenuDelegate> candidateMenuDelegate = candidateMenu.delegate;
        if ([candidateMenuDelegate respondsToSelector:@selector(menuWillOpen:)]) [candidateMenuDelegate menuWillOpen:candidateMenu];
        assert(layoutPanel.level == MSIMECandidateWindowLevel());
        if ([candidateMenuDelegate respondsToSelector:@selector(menuDidClose:)]) [candidateMenuDelegate menuDidClose:candidateMenu];
        assert(layoutPanel.level == MSIMECandidateWindowLevel());
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
        candidateMenu = [clickCandidate menuForEvent:rightClick];
        positionMenu = [candidateMenu itemAtIndex:1].submenu;
        NSMutableArray *operations = [NSMutableArray arrayWithObjects:[candidateMenu itemAtIndex:0], [candidateMenu itemAtIndex:2], nil];
        [operations addObjectsFromArray:[positionMenu.itemArray subarrayWithRange:NSMakeRange(0, 5)]];
        [operations addObject:[positionMenu itemAtIndex:6]];
        for (NSMenuItem *operation in operations) {
            NSUInteger calls = session.maintenanceCalls;
            [NSApp sendAction:operation.action to:operation.target from:operation];
            assert(session.maintenanceCalls == calls + 1 && session.maintenanceAction == operation.tag);
            assert(session.selectedGeneration == 2 && session.selectedIndex == 1);
            // 右键菜单的每一项和点选一样先报告 MSIME_KEY_POINTER 开新的一轮，再派发。
            assert(([[session.calls subarrayWithRange:NSMakeRange(session.calls.count - 2, 2)] isEqual:@[[NSString stringWithFormat:@"key:%u", MSIME_KEY_POINTER], @"maintenance"]]));
        }
        // 置顶 is one of the operations above, so the loop leaves 布局 pinned. That reorders every later
        // render in this function - including the external-skin block, which reads the first button as the
        // highlighted one - so undo it here rather than at the end. The loop is checking that each menu
        // item dispatches maintenance, not setting up state for anybody else.
        [NSUserDefaults.standardUserDefaults removeObjectForKey:@"MSIMEClientPinnedCandidates"];
        [controller renderCandidates];
        // renderCandidates rebuilds the buttons, so everything captured from the previous panel is stale.
        clickCandidate = (id)PageButton(layoutPanel.contentView, 1);
        candidateMenu = [clickCandidate menuForEvent:rightClick];
        assert(clickCandidate && candidateMenu.numberOfItems == 3);
        NSMenuItem *retainedOperation = [candidateMenu itemAtIndex:0];
        NSDictionary *validContext = retainedOperation.representedObject;
        NSUInteger maintenanceCalls = session.maintenanceCalls;
        // 被拒绝的菜单操作不派发，也不报告。
        const NSUInteger menuKeys = session.keys.count;
        const NSInteger validTag = retainedOperation.tag;
        for (NSInteger unknownTag : {-1, 3, 10, 16}) {
            retainedOperation.tag = unknownTag;
            [controller candidateMenuAction:retainedOperation];
        }
        retainedOperation.tag = validTag;
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
        assert(session.maintenanceCalls == maintenanceCalls && session.keys.count == menuKeys);
        for (NSString *text in @[@"中", @"𠀀", @"测试"]) {
            NSMenu *menu = [controller menuForCandidate:@{@"id":clickCandidate.candidateID, @"text":text}];
            assert(menu.numberOfItems == ([text isEqual:@"测试"] ? 3 : 2));
        }
        NSDictionary *validClickID = clickCandidate.candidateID;
        NSUInteger selectedCalls = session.selectCalls;
        // 让上次报告的字段属性作废：指针点选和按键一样，先比较属性、变了先报告。
        [controller setValue:@0 forKey:@"fieldTraits"];
        [controller selectCandidate:clickCandidate];
        assert(session.selectCalls == selectedCalls + 1 && session.selectedGeneration == 2 && session.selectedIndex == 1);
        // 指针点选先报告属性和一次 MSIME_KEY_POINTER 开新的一轮，再派发；被拒绝的点选什么都不报告。
        NSString *clickTraits = [NSString stringWithFormat:@"traits:%x", [[controller valueForKey:@"fieldTraits"] unsignedIntValue]];
        assert(([[session.calls subarrayWithRange:NSMakeRange(session.calls.count - 3, 3)] isEqual:@[clickTraits, [NSString stringWithFormat:@"key:%u", MSIME_KEY_POINTER], @"select"]]));
        const NSUInteger pointerKeys = session.keys.count;
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
        assert(session.keys.count == pointerKeys);
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
        {
            // 列表 A 画在窗上，同一会话里新一代的列表 B 接着画出来，同一位置上换成了别的词。A 留下的第 2 行按钮点下去什么也不选，更不能把 B 的第 2 个候选上屏：两份列表的第 2 行 index 都是 1，只有代次不同。
            NSDictionary *listA = @{@"session": @1, @"generation": @2, @"focused": @YES, @"page": @0, @"page_count": @1, @"editing_text": @"ceshi",
                                    @"candidates": @[@{@"text": @"测试", @"highlighted": @YES, @"id": @{@"session": @1, @"generation": @2, @"index": @0}},
                                                     @{@"text": @"布局", @"highlighted": @NO, @"id": @{@"session": @1, @"generation": @2, @"index": @1}}]};
            NSMutableDictionary *listB = [listA mutableCopy];
            listB[@"generation"] = @3;
            listB[@"candidates"] = @[@{@"text": @"侧视", @"highlighted": @YES, @"id": @{@"session": @1, @"generation": @3, @"index": @0}},
                                     @{@"text": @"策士", @"highlighted": @NO, @"id": @{@"session": @1, @"generation": @3, @"index": @1}}];
            [controller setValue:listA forKey:@"view"];
            [controller renderCandidates];
            MSIMECandidateButton *listAButton = PageButton(layoutPanel.contentView, 1);
            assert(listAButton && [listAButton.title containsString:@"布局"]);
            const NSUInteger listSelectCalls = session.selectCalls;
            const NSUInteger listKeys = session.keys.count;
            // B 已经是控制器的视图、窗上还是 A（AppKit 先递来点击、后重画）：按钮还挂在窗上，靠的是它记着的旧一代身份。
            [controller setValue:[listB copy] forKey:@"view"];
            assert(listAButton.superview == MSIMECandidateControlsView(layoutPanel.contentView));
            [controller selectCandidate:listAButton];
            assert(session.selectCalls == listSelectCalls && session.keys.count == listKeys);
            // B 画出来之后：A 的按钮整个被换掉，点它同样什么也不选。
            [controller renderCandidates];
            MSIMECandidateButton *listBButton = PageButton(layoutPanel.contentView, 1);
            assert(listBButton && listBButton != listAButton && [listBButton.title containsString:@"策士"]);
            [controller selectCandidate:listAButton];
            assert(session.selectCalls == listSelectCalls && session.keys.count == listKeys);
            // 原地重画：只有释义变了的同一代视图沿用窗上的按钮，A 的按钮还在窗上，点它选的是它自己那个候选。
            [controller setValue:listA forKey:@"view"];
            [controller renderCandidates];
            listAButton = PageButton(layoutPanel.contentView, 1);
            NSMutableDictionary *glossA = [listA mutableCopy];
            glossA[@"candidates"] = @[listA[@"candidates"][0], @{@"text": @"布局", @"highlighted": @NO, @"translation": @"x", @"id": @{@"session": @1, @"generation": @2, @"index": @1}}];
            [controller setValue:[glossA copy] forKey:@"view"];
            [controller renderCandidates];
            assert(PageButton(layoutPanel.contentView, 1) == listAButton && [listAButton.translation isEqual:@"x"]);
            [controller selectCandidate:listAButton];
            assert(session.selectCalls == listSelectCalls + 1 && session.selectedGeneration == 2 && session.selectedIndex == 1);
            const NSUInteger reusedKeys = session.keys.count;
            // 文字和 A 一样、只换了一代再带上释义的 B，看上去也能原地重画，但代次不同就整页重建：A 的按钮不会留在窗上冒充 B 的候选。
            NSMutableDictionary *glossB = [glossA mutableCopy];
            glossB[@"generation"] = @3;
            glossB[@"candidates"] = @[@{@"text": @"测试", @"highlighted": @YES, @"id": @{@"session": @1, @"generation": @3, @"index": @0}},
                                      @{@"text": @"布局", @"highlighted": @NO, @"translation": @"y", @"id": @{@"session": @1, @"generation": @3, @"index": @1}}];
            [controller setValue:[glossB copy] forKey:@"view"];
            [controller renderCandidates];
            assert(PageButton(layoutPanel.contentView, 1) != listAButton && listAButton.superview != layoutPanel.contentView);
            [controller selectCandidate:listAButton];
            assert(session.selectCalls == listSelectCalls + 1 && session.keys.count == reusedKeys);
            // 对照：B 自己的按钮照常选中 B 的那一代。
            [controller selectCandidate:PageButton(layoutPanel.contentView, 1)];
            assert(session.selectCalls == listSelectCalls + 2 && session.selectedGeneration == 3 && session.selectedIndex == 1);
            [controller setValue:pageView forKey:@"view"];
            [controller renderCandidates];
        }
        appearance.vertical = NO;
        session.lastCommand = UINT32_MAX;
        [controller appearanceChanged:nil];
        assert(session.lastCommand == UINT32_MAX);
        assert(layoutPanel.frame.size.height < verticalHeight);
        NSView *first = MSIMECandidateControlsView(layoutPanel.contentView).subviews[0];
        NSView *second = MSIMECandidateControlsView(layoutPanel.contentView).subviews[1];
        assert(first.frame.origin.y == second.frame.origin.y);
        assert(NSMaxX(first.frame) == NSMinX(second.frame));
        CGFloat normalHeight = layoutPanel.frame.size.height;
        appearance.fontSize = 20;
        [controller appearanceChanged:nil];
        assert(layoutPanel.frame.size.height > normalHeight);
        // 整体大小 multiplies the candidate fonts and the window's lengths together; 不透明度 fades the card and nothing on it, through the fill rather than the panel's alphaValue; 圆角大小 replaces the card radius at the window's scale and pulls the rows in with it.
        {
            MSIMECandidateButton *(^candidateRow)(BOOL) = ^MSIMECandidateButton *(BOOL highlighted) {
                for (NSView *view in MSIMECandidateControlsView(layoutPanel.contentView).subviews)
                    if ([view isKindOfClass:MSIMECandidateButton.class] && view.tag >= 0 &&
                        (!highlighted || ((MSIMECandidateButton *)view).candidateHighlighted)) return (id)view;
                return nil;
            };
            const NSSize unscaled = layoutPanel.frame.size;
            assert(candidateRow(NO) && candidateRow(NO).font.pointSize == 20.0);
            appearance.candidateScalePercent = 150;
            [controller appearanceChanged:nil];
            MSIMECandidateButton *scaledRow = candidateRow(NO);
            assert(scaledRow && std::abs(scaledRow.font.pointSize - 30.0) < 0.01 && scaledRow.chromeScale == 1.5);
            assert(std::abs(scaledRow.numberFont.pointSize - 30.0 * MSIMECandidateNumberScale) < 0.01);
            assert(layoutPanel.frame.size.height > unscaled.height * 1.3 && layoutPanel.frame.size.width >= unscaled.width);
            appearance.candidateOpacityPercent = 50;
            appearance.candidateCornerRadius = @4;
            [controller appearanceChanged:nil];
            MSIMECandidateChromeView *styledChrome = (id)layoutPanel.contentView;
            NSString *match = [styledChrome.effectiveAppearance bestMatchFromAppearancesWithNames:@[NSAppearanceNameAqua, NSAppearanceNameDarkAqua]];
            const auto plain = [appearance resolvedSkinForDark:[match isEqual:NSAppearanceNameDarkAqua]].tokens;
            assert(styledChrome.cornerRadius == 6.0 && layoutPanel.alphaValue == 1.0);
            assert(std::abs(styledChrome.fillColor.alphaComponent - plain.surface.a * 0.5) < 0.002);
            assert(std::abs(styledChrome.strokeColor.alphaComponent - plain.border.a * 0.5) < 0.002);
            for (NSView *child in styledChrome.subviews) assert(![child isKindOfClass:NSVisualEffectView.class]);
            scaledRow = candidateRow(NO);
            assert([scaledRow.fillColor isEqual:SkinColor(plain.selected)] && scaledRow.cornerRadius <= styledChrome.cornerRadius);
            if (MSIMECandidateButton *highlighted = candidateRow(YES)) assert([highlighted.titleColor isEqual:SkinColor(plain.selectedText)]);
            appearance.candidateScalePercent = 100;
            appearance.candidateOpacityPercent = 100;
            appearance.candidateCornerRadius = nil;
            [controller appearanceChanged:nil];
            assert(NSEqualSizes(layoutPanel.frame.size, unscaled));
            // 内边距 (#7316) moves the candidates toward the card's edge on every side: 2pt instead of the skin's own pad takes twice the difference off the card's height and never widens it (the 7em minimum width may hold it), and 跟随皮肤 restores it.
            {
                const CGFloat skinPad = [appearance candidateWindowSkinForDark:[match isEqual:NSAppearanceNameDarkAqua]].tokens.pad;
                assert(skinPad > 2.0);
                const NSRect rowBefore = candidateRow(NO).frame;
                appearance.candidatePadding = @2;
                [controller appearanceChanged:nil];
                const NSSize compact = layoutPanel.frame.size;
                assert(std::abs(compact.height - (unscaled.height - 2 * (skinPad - 2.0))) < 0.5);
                assert(compact.width <= unscaled.width);
                assert(std::abs(candidateRow(NO).frame.origin.x - (rowBefore.origin.x - (skinPad - 2.0))) < 0.5);
                // 低于 2pt 时候选行继续靠近边缘；边框不再让 0pt 与 1pt 的布局相同。
                const CGFloat border = [appearance candidateWindowSkinForDark:[match isEqual:NSAppearanceNameDarkAqua]].tokens.borderWidth;
                assert(border > 0.0 && border < 2.0);
                appearance.candidatePadding = @1;
                [controller appearanceChanged:nil];
                const NSSize hairline = layoutPanel.frame.size;
                assert(std::abs(hairline.height - (unscaled.height - 2 * (skinPad - 1.0))) < 0.5);
                appearance.candidatePadding = @0;
                [controller appearanceChanged:nil];
                assert(std::abs(layoutPanel.frame.size.height - (unscaled.height - 2 * skinPad)) < 0.5);
                assert(std::abs(candidateRow(NO).frame.origin.x - (rowBefore.origin.x - skinPad)) < 0.5);
                appearance.candidatePadding = nil;
                [controller appearanceChanged:nil];
                assert(NSEqualSizes(layoutPanel.frame.size, unscaled));
            }
            assert([((MSIMECandidateChromeView *)layoutPanel.contentView).fillColor isEqual:SkinColor(plain.surface)]);
        }
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
                    auto tokens = [appearance resolvedSkinForDark:dark].tokens;
                    if (chrome.liquidGlassActive) msime::mac::ApplyNativeGlassCandidateContrast(tokens, dark);
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
                        const unsigned expected = [skinID isEqual:@"shuishan"] ? 0x9FB5A3 : chrome.liquidGlassActive ? (dark ? 0xD1D1D6 : 0x202024) : (dark ? 0x98989D : 0x6E6E73);
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
                    for (NSView *child in chrome.candidateContentView.subviews)
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
        @autoreleasepool { TestExternalSkin(controller, layoutPanel, defaults); }
        @autoreleasepool { TestStyledExternalSkin(controller, layoutPanel, defaults); }
        @autoreleasepool { TestSkinSlotsPerMode(controller, layoutPanel, defaults); }
        [controller setValue:appearance forKey:@"appearance"];
        appearance.vertical = NO;
        appearance.globalTheme = @"system";
        [controller appearanceChanged:nil];
        NSDictionary *beforeArrowTransition = session.nextTransition;
        const BOOL beforeLeftRight = [appearance navigationEnabled:@"left_right"];
        assert(beforeLeftRight);
        for (NSNumber *leftRight in @[@YES, @NO]) {
            [appearance applySharedCandidatePreferences:@{@"navigation": @{@"left_right": leftRight}}];
            assert([appearance navigationEnabled:@"left_right"] == leftRight.boolValue);
            for (NSNumber *key in @[@123, @124, @125, @126]) {
                [controller setValue:@{@"focused": @YES, @"scheme": @0, @"editing_text": @"ceshi", @"caret_position": @5,
                                      @"candidates": @[@{@"text": @"甲", @"highlighted": @YES}]} forKey:@"view"];
                session.nextTransition = @{@"handled": @YES};
                layoutPanel.requestedVisible = YES;
                session.lastCommand = UINT32_MAX;
                NSEvent *event = [NSEvent keyEventWithType:NSEventTypeKeyDown location:NSZeroPoint modifierFlags:0 timestamp:0 windowNumber:0 context:nil characters:@"" charactersIgnoringModifiers:@"" isARepeat:NO keyCode:key.unsignedShortValue];
                assert([controller handleEvent:event client:client]);
                // 默认由 runtime 判断左右键端点；显式关闭时仍走宿主旧分支，上下始终移动高亮。
                const unsigned short code = key.unsignedShortValue;
                uint32_t expected = code == 123 ? MSIME_MOVE_LEFT : code == 124 ? (leftRight.boolValue ? MSIME_MOVE_RIGHT : MSIME_NEXT_CANDIDATE)
                                    : code == 126 ? MSIME_PREVIOUS_CANDIDATE : MSIME_NEXT_CANDIDATE;
                assert(session.lastCommand == expected);
            }
        }
        [appearance applySharedCandidatePreferences:@{@"navigation": @{@"left_right": @(beforeLeftRight)}}];
        session.nextTransition = beforeArrowTransition;
        // 后面的字符翻页测试从普通空组字开始，不继承皮肤测试的本地模式。
        [controller setValue:@{@"editing_text": @"", @"caret_position": @0, @"candidates": @[]} forKey:@"view"];
        {
            // 方向键与 Windows、Linux 同一套规则，不看方案和候选窗朝向：五笔的横排候选也是 ↑/↓ 移高亮、←/→ 移光标。
            NSDictionary *saved = [controller valueForKey:@"view"];
            NSMutableDictionary *wubi = [saved mutableCopy];
            wubi[@"scheme"] = @2;
            // 候选列表打开时（韩文汉字列表）组字里没有光标，四个方向键都移高亮，横排竖排都一样。
            NSMutableDictionary *hanja = [saved mutableCopy];
            hanja[@"scheme"] = @4;
            hanja[@"candidate_list_open"] = @YES;
            const BOOL savedVertical = appearance.vertical;
            for (NSNumber *vertical in @[@NO, @YES]) {
                appearance.vertical = vertical.boolValue;
                for (NSNumber *key in @[@123, @124, @125, @126]) {
                    const unsigned short code = key.unsignedShortValue;
                    NSEvent *event = [NSEvent keyEventWithType:NSEventTypeKeyDown location:NSZeroPoint modifierFlags:0 timestamp:0 windowNumber:0 context:nil characters:@"" charactersIgnoringModifiers:@"" isARepeat:NO keyCode:code];
                    [controller setValue:wubi forKey:@"view"];
                    layoutPanel.requestedVisible = YES;
                    session.lastCommand = UINT32_MAX;
                    assert([controller handleEvent:event client:client]);
                    uint32_t expected = code == 123 ? MSIME_MOVE_LEFT : code == 124 ? MSIME_MOVE_RIGHT
                                        : code == 126 ? MSIME_PREVIOUS_CANDIDATE : MSIME_NEXT_CANDIDATE;
                    assert(session.lastCommand == expected);
                    [controller setValue:hanja forKey:@"view"];
                    layoutPanel.requestedVisible = YES;
                    session.lastCommand = UINT32_MAX;
                    assert([controller handleEvent:event client:client]);
                    expected = code == 123 || code == 126 ? MSIME_PREVIOUS_CANDIDATE : MSIME_NEXT_CANDIDATE;
                    assert(session.lastCommand == expected);
                }
            }
            appearance.vertical = savedVertical;
            [controller setValue:saved forKey:@"view"];
        }
        CheckMenu([controller menu], controller);
        // Shift+[ / Shift+] 是直角引号键，仍不参与候选翻页；其它未翻页字符走普通输入。
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
                        } else if (shift.boolValue && i >= 2) {
                            assert(session.lastCommand == UINT32_MAX && session.asciiCalls == 0);
                            assert(session.lastPunctuationASCII == [characters characterAtIndex:0]);
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
        // What the Engine reports in Unicode mode; the digit routes below read this, not the mode's name.
        unicodePagingView[@"spelling_symbols"] = @"0123456789";
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
        // Expression mode (V) spells with digits and the operators + - * / . ( ) % ^, which the view lists in spelling_symbols. Every one of them has to reach the Engine even where this host reads the same key as something else: '-' is the default minus/equal page key, '.' the comma/period one, and on a US layout Shift+5, 6, 8, 9 and 0 are % ^ * ( ). A Shift+digit that types something the mode does not spell with still picks a candidate.
        {
            NSMutableDictionary *expressionView = [pageView mutableCopy];
            expressionView[@"local_mode"] = @"expression";
            expressionView[@"spelling_symbols"] = @"0123456789+-*/.()%^";
            const NSInteger pageShortcutBeforeExpression = appearance.pageShortcut;
            NSDictionary *transitionBeforeExpression = session.nextTransition;
            NSEvent *(^expressionKey)(unsigned short, NSString *, NSString *, NSEventModifierFlags) =
                ^NSEvent *(unsigned short code, NSString *characters, NSString *ignoring, NSEventModifierFlags flags) {
                    return [NSEvent keyEventWithType:NSEventTypeKeyDown location:NSZeroPoint modifierFlags:flags timestamp:0 windowNumber:0 context:nil characters:characters charactersIgnoringModifiers:ignoring isARepeat:NO keyCode:code];
                };
            for (NSNumber *shortcut in @[@0, @2]) {
                appearance.pageShortcut = shortcut.integerValue;
                for (NSArray *key in @[@[@27, @"-", @"-", @0], @[@47, @".", @".", @0], @[@18, @"1", @"1", @0], @[@25, @"(", @"9", @(NSEventModifierFlagShift)],
                                       @[@23, @"%", @"5", @(NSEventModifierFlagShift)], @[@44, @"/", @"/", @0]]) {
                    [controller setValue:expressionView forKey:@"view"];
                    [controller renderCandidates];
                    layoutPanel.requestedVisible = YES;
                    session.nextTransition = @{ @"handled": @YES, @"commit": NSNull.null, @"view": expressionView };
                    session.lastCommand = UINT32_MAX;
                    const NSUInteger selects = session.selectCalls, ascii = session.asciiCalls;
                    NSString *characters = key[1];
                    assert([controller handleEvent:expressionKey([key[0] unsignedShortValue], characters, key[2], [key[3] unsignedIntegerValue]) client:client]);
                    assert(session.lastCommand == UINT32_MAX && session.selectCalls == selects);
                    assert(session.asciiCalls == ascii + 1 && session.lastASCII == [characters characterAtIndex:0]);
                }
            }
            [controller setValue:expressionView forKey:@"view"];
            [controller renderCandidates];
            layoutPanel.requestedVisible = YES;
            const NSUInteger selectsBeforeShiftedDigit = session.selectCalls, asciiBeforeShiftedDigit = session.asciiCalls;
            assert([controller handleEvent:expressionKey(19, @"@", @"2", NSEventModifierFlagShift) client:client]);
            assert(session.selectCalls == selectsBeforeShiftedDigit + 1 && session.selectedIndex == 1);
            assert(session.asciiCalls == asciiBeforeShiftedDigit);
            appearance.pageShortcut = pageShortcutBeforeExpression;
            session.nextTransition = transitionBeforeExpression;
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
        NSDictionary *chineseContext = @{@"scheme": @0, @"local_mode": @"none", @"script_conversion": @YES};
        NSDictionary *emptyView = @{@"editing_text": @"", @"candidates": @[]};
        NSEvent *(^bracket)(BOOL) = ^NSEvent *(BOOL last) {
            NSString *glyph = last ? @"]" : @"[";
            return [NSEvent keyEventWithType:NSEventTypeKeyDown location:NSZeroPoint modifierFlags:0 timestamp:0 windowNumber:0 context:nil characters:glyph charactersIgnoringModifiers:glyph isARepeat:NO keyCode:last ? 30 : 33];
        };
        NSString *(^pressHeldEdge)(NSString *, NSString *, BOOL, NSString *, NSDictionary *) = ^NSString *(NSString *held, NSString *word, BOOL last, NSString *engineCommit, NSDictionary *context) {
            NSMutableDictionary *wordView = [@{@"session": @71, @"generation": @72, @"focused": @YES, @"editing_text": @"synthetic", @"scheme": @0, @"local_mode": @"none", @"script_conversion": @YES,
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
        assert([pressEdge(@"头发", YES, @"发", @{@"scheme": @0, @"local_mode": @"temporary_japanese", @"script_conversion": @NO}) isEqual:@"发"]);
        assert([pressEdge(@"头发", YES, @"发", @{@"scheme": @0, @"local_mode": @"unicode", @"script_conversion": @NO}) isEqual:@"发"]);
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
        // 最后一项是快捷键关掉后这个键做什么，各键不一样：逗号和句号是普通字符，交给 Engine；上下方向键是导航键，Windows 在候选显示时一律把它们交给 Server（CompositionProcessorEngine.cpp），关掉的键得到 NavigationIgnored，所以面板显示时它们被吞掉，既不翻页也不发 FINISH_COMPOSITION，组字保持不变。Tab 和 Page Up / Page Down 在 macOS 上固定开启，文档里存的关不起作用，下面单独断言。
        for (NSArray *entry in @[@[@"comma_period", @",", @43, @(MSIME_PREVIOUS_PAGE), @1],
                                 @[@"comma_period", @".", @47, @(MSIME_NEXT_PAGE), @1],
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
        // 引擎声明 Tab 是辅助码触发键时，普通 Tab 先送字符；Shift+Tab 仍翻上一页。
        NSMutableDictionary *manualHelpcodeView = [pageView mutableCopy];
        manualHelpcodeView[@"scheme"] = @1;
        manualHelpcodeView[@"spelling_symbols"] = @"\t";
        [controller setValue:manualHelpcodeView forKey:@"view"];
        session.nextTransition = @{@"handled": @YES, @"view": manualHelpcodeView};
        session.lastCommand = UINT32_MAX;
        session.asciiCalls = 0;
        NSEvent *manualTab = [NSEvent keyEventWithType:NSEventTypeKeyDown location:NSZeroPoint modifierFlags:0 timestamp:0 windowNumber:0 context:nil characters:@"\t" charactersIgnoringModifiers:@"\t" isARepeat:NO keyCode:48];
        assert([controller handleEvent:manualTab client:client]);
        assert(session.asciiCalls == 1 && session.lastASCII == '\t' && session.lastCommand == UINT32_MAX);
        assert([controller handleEvent:reverseTab client:client] && session.lastCommand == MSIME_PREVIOUS_PAGE);
        session.nextTransition = nil;
        [controller setValue:pageView forKey:@"view"];
        // 文档里存着 Tab 和 Page Up / Page Down 翻页为关时，它们照样翻页：macOS 固定开启这两组键。
        [appearance applySharedCandidatePreferences:@{@"navigation": @{@"tab": @NO, @"page_up_down": @NO}}];
        for (NSArray *entry in @[@[@"\t", @48, @0, @(MSIME_NEXT_PAGE)],
                                 @[@"\t", @48, @(NSEventModifierFlagShift), @(MSIME_PREVIOUS_PAGE)],
                                 @[@"", @116, @0, @(MSIME_PREVIOUS_PAGE)],
                                 @[@"", @121, @0, @(MSIME_NEXT_PAGE)]]) {
            layoutPanel.requestedVisible = YES;
            appearance.vertical = YES;
            session.lastCommand = UINT32_MAX;
            NSEvent *event = [NSEvent keyEventWithType:NSEventTypeKeyDown location:NSZeroPoint modifierFlags:(NSEventModifierFlags)[entry[2] unsignedIntegerValue] timestamp:0 windowNumber:0 context:nil characters:entry[0] charactersIgnoringModifiers:entry[0] isARepeat:NO keyCode:[entry[1] unsignedShortValue]];
            assert([controller handleEvent:event client:client] && session.lastCommand == [entry[3] unsignedIntValue]);
        }
        // With no candidates showing (an invalid caret, no screen) Tab and Shift+Tab go back to the application whatever navigation.tab says. A composition still on the line is finished first, for every scheme and not only a Korean syllable, so the key does not move focus away from marked text left dangling in the old field; with nothing composed the session is not touched at all.
        NSEvent *plainTab = [NSEvent keyEventWithType:NSEventTypeKeyDown location:NSZeroPoint modifierFlags:0 timestamp:0 windowNumber:0 context:nil characters:@"\t" charactersIgnoringModifiers:@"\t" isARepeat:NO keyCode:48];
        NSDictionary *hiddenComposition = @{@"session": @73, @"generation": @74, @"focused": @YES, @"editing_text": @"synthetic", @"scheme": @0, @"local_mode": @"none",
            @"candidates": @[@{@"text": @"合成", @"highlighted": @YES, @"id": @{@"session": @73, @"generation": @74, @"index": @0}}]};
        for (NSNumber *tabPaging in @[@YES, @NO]) {
            [appearance applySharedCandidatePreferences:@{@"navigation": @{@"tab": tabPaging}}];
            for (NSEvent *tab in @[plainTab, reverseTab]) {
                [controller setValue:hiddenComposition forKey:@"view"];
                layoutPanel.requestedVisible = NO;
                session.lastCommand = UINT32_MAX;
                session.nextTransition = @{@"handled": @YES, @"commit": NSNull.null, @"view": emptyView};
                assert(![controller handleEvent:tab client:client]);
                assert(session.lastCommand == MSIME_FINISH_COMPOSITION);
                [controller setValue:emptyView forKey:@"view"];
                layoutPanel.requestedVisible = NO;
                session.lastCommand = UINT32_MAX;
                NSUInteger commandsBeforeIdleTab = session.commandCalls;
                assert(![controller handleEvent:tab client:client]);
                assert(session.lastCommand == UINT32_MAX && session.commandCalls == commandsBeforeIdleTab);
            }
        }
        // A highlighted candidate that carries a gloss takes Tab first: Tab steps the armed gloss column 0 -> 1 -> 2 -> 0 and Shift+Tab steps it back, with no Engine command and whether or not Tab paging is on. A candidate without a gloss still pages.
        NSDictionary *glossView = @{@"session": @75, @"generation": @76, @"focused": @YES, @"page": @0, @"page_count": @2, @"editing_text": @"hello", @"caret_position": @5, @"scheme": @0, @"local_mode": @"none",
            @"candidates": @[@{@"text": @"你好", @"translation": @"hello\nhola", @"highlighted": @YES, @"id": @{@"session": @75, @"generation": @76, @"index": @0}}]};
        for (NSNumber *tabPaging in @[@YES, @NO]) {
            [appearance applySharedCandidatePreferences:@{@"navigation": @{@"tab": tabPaging}}];
            [controller setValue:glossView forKey:@"view"];
            [controller setValue:@0 forKey:@"armedGlossColumn"];
            NSUInteger commandsBeforeGloss = session.commandCalls;
            for (NSArray *step in @[@[plainTab, @1], @[plainTab, @2], @[plainTab, @0], @[reverseTab, @2], @[reverseTab, @1], @[reverseTab, @0]]) {
                layoutPanel.requestedVisible = YES;
                assert([controller handleEvent:step[0] client:client]);
                assert([[controller valueForKey:@"armedGlossColumn"] integerValue] == [step[1] integerValue]);
            }
            assert(session.commandCalls == commandsBeforeGloss);
        }
        // 从独立释义页按 Tab 返回时沿用释义列循环，不顺便触发手动辅助码。
        NSMutableDictionary *manualGlossView = [glossView mutableCopy];
        manualGlossView[@"scheme"] = @1;
        manualGlossView[@"candidates"] = @[@{@"text": @"你好", @"translation": @"1. hello; 2. hola", @"highlighted": @YES, @"id": @{@"session": @75, @"generation": @76, @"index": @0}}];
        manualGlossView[@"spelling_symbols"] = @"\t";
        [controller setValue:manualGlossView forKey:@"view"];
        [controller setValue:@0 forKey:@"armedGlossColumn"];
        appearance.candidateTranslations = YES;
        layoutPanel.requestedVisible = YES;
        NSEvent *openSenses = [NSEvent keyEventWithType:NSEventTypeKeyDown location:NSZeroPoint modifierFlags:NSEventModifierFlagControl timestamp:0 windowNumber:0 context:nil characters:@"\r" charactersIgnoringModifiers:@"\r" isARepeat:NO keyCode:36];
        assert([controller handleEvent:openSenses client:client]);
        assert([[controller valueForKey:@"glossSenses"] count] > 1);
        session.nextTransition = @{@"handled": @YES, @"view": manualGlossView};
        session.asciiCalls = 0;
        session.lastCommand = UINT32_MAX;
        assert([controller handleEvent:plainTab client:client]);
        assert(session.asciiCalls == 0 && session.lastCommand == UINT32_MAX);
        assert([[controller valueForKey:@"glossSenses"] count] == 0);
        assert([[controller valueForKey:@"armedGlossColumn"] integerValue] == 1);
        session.nextTransition = nil;
        [controller setValue:@0 forKey:@"armedGlossColumn"];
        [appearance applySharedCandidatePreferences:@{@"navigation": @{@"tab": @YES}}];
        NSMutableDictionary *unglossedView = [glossView mutableCopy];
        unglossedView[@"candidates"] = @[@{@"text": @"你好", @"highlighted": @YES, @"id": @{@"session": @75, @"generation": @76, @"index": @0}}];
        for (NSEvent *tab in @[plainTab, reverseTab]) {
            [controller setValue:[unglossedView copy] forKey:@"view"];
            [controller setValue:@0 forKey:@"armedGlossColumn"];
            layoutPanel.requestedVisible = YES;
            session.lastCommand = UINT32_MAX;
            assert([controller handleEvent:tab client:client]);
            assert(session.lastCommand == (tab == reverseTab ? MSIME_PREVIOUS_PAGE : MSIME_NEXT_PAGE));
        }
        session.nextTransition = beforeWordTransition;
        // Script selection changes only native display/commit strings, not Engine state or IDs.
        assert(!appearance.traditionalOutput);
        assert([MSIMEChineseOutputString(@"汉语", YES) isEqual:@"漢語"]);
        assert([MSIMEChineseOutputString(@"汉语", NO) isEqual:@"汉语"]);
        assert([CandidateDisplay(@{@"text": @"汉语", @"annotation": @"(aB)"}, YES) isEqual:@"漢語(aB)"]);
        assert([CandidateDisplay(@{@"text": @"汉语", @"annotation": NSNull.null}, NO) isEqual:@"汉语"]);
        // 注释一律包在括号里、紧贴候选字：裸的辅助码加括号，已带半角或全角括号的不重复加，首尾空白去掉，空白注释不显示。
        assert([CandidateDisplay(@{@"text": @"你好", @"annotation": @"wqvb"}, NO) isEqual:@"你好(wqvb)"]);
        assert([CandidateDisplay(@{@"text": @"你好", @"annotation": @" wqvb wqv "}, NO) isEqual:@"你好(wqvb wqv)"]);
        assert([CandidateDisplay(@{@"text": @"深圳市", @"annotation": @"（广东省）"}, NO) isEqual:@"深圳市（广东省）"]);
        assert([CandidateDisplay(@{@"text": @"你好", @"annotation": @"  "}, NO) isEqual:@"你好"]);
        assert(([CandidateDisplay(@{@"text":@"汉语", @"corrected":@YES, @"annotation":@"(aB)", @"source":@2}, YES) isEqual:@"漢語*(aB) ☁️"]));
        for (id corrected in @[@NO, @1, @"true", NSNull.null])
            assert(([CandidateDisplay(@{@"text":@"汉语", @"corrected":corrected}, NO) isEqual:@"汉语"]));
        for (id source in @[@0, @1, @4, @255, @(-1), @YES, @2.0, @"2", NSNull.null])
            assert(([CandidateDisplay(@{@"text":@"汉语", @"source":source}, NO) isEqual:@"汉语"]));
        assert(([CandidateDisplay(@{@"text":@"汉语", @"annotation":@"(aB)", @"source":@2}, YES) isEqual:@"漢語(aB) ☁️"]));
        assert(([CandidateDisplay(@{@"text":@"汉语", @"annotation":@"(aB)", @"source":@3}, NO) isEqual:@"汉语(aB) 🤖"]));
        // nil 表示尚未提供缓存，空串是显式隐藏；两者不能合并成同一个回退行为。
        NSDictionary *aiFixed = @{@"text": @"汉语", @"source": @3, @"fixed_position": @1};
        assert([CandidateDisplayWithBadges(aiFixed, NO, nil, nil) isEqual:@"汉语 🤖 📎"]);
        assert([CandidateDisplayWithBadges(aiFixed, NO, @"", @"") isEqual:@"汉语"]);
        assert([CandidateDisplayWithBadges(@{@"source": @3}, NO, nil, @"") isEqual:@" 🤖"]);
        NSMutableDictionary *scriptView = [@{@"scheme": @0, @"local_mode": @"none", @"script_conversion": @YES, @"session": @1, @"generation": @20, @"editing_text": @"hanyu", @"caret_position": @5, @"candidates": @[@{@"text": @"汉语", @"highlighted": @YES, @"id": @{@"session": @1, @"generation": @20, @"index": @0}}]} mutableCopy];
        [controller setValue:[scriptView copy] forKey:@"view"];
        NSDictionary *preserved = [[controller valueForKey:@"view"] copy];
        [controller selectTraditionalOutput:nil];
        [controller appearanceChanged:nil];
        assert([controller.menu itemAtIndex:4].state == NSControlStateValueOn);
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
        // 走实际偏好缓存和候选按钮，确认两类徽标可以独立切换，固定状态与候选身份保持不变。
        word[@"source"] = @3;
        word[@"fixed_position"] = @1;
        for (NSDictionary *badgeCase in @[
            @{@"ai": @"robot", @"fixed": @"paperclip", @"show": @YES, @"suffix": @" 🤖 📎"},
            @{@"ai": @"whale", @"fixed": @"pushpin", @"show": @YES, @"suffix": @" 🐳 📌"},
            @{@"ai": @"whale", @"fixed": @"pushpin", @"show": @NO, @"suffix": @" 🐳"},
            @{@"ai": @"robot", @"fixed": @"dot", @"show": @YES, @"suffix": @" 🤖 ·"}
        ]) {
            NSDictionary *unchanged = [word copy];
            [controller applySharedToolbarPreferences:@{
                @"candidate_ai_badge_style": badgeCase[@"ai"],
                @"candidate_fixed_badge_style": badgeCase[@"fixed"],
                @"candidate_fixed_badge": badgeCase[@"show"]
            }];
            [controller renderCandidates];
            scriptButton = PageButton(layoutPanel.contentView, 0);
            assert([scriptButton.toolTip isEqual:[@"漢語(aB)" stringByAppendingString:badgeCase[@"suffix"]]]);
            assert([scriptButton.title containsString:[@"漢語" stringByAppendingString:badgeCase[@"suffix"]]]);
            assert(scriptButton.candidateFixed && [scriptButton.candidateID isEqual:word[@"id"]]);
            assert([word isEqual:unchanged]);
        }
        [word removeObjectForKey:@"fixed_position"];
        [controller applySharedToolbarPreferences:candidateDisplayDefaults];
        NSUInteger contextIndex = 0;
        // The view and the commit context say whether conversion applies (`script_conversion`), as the runtime decides it from the scheme and local mode; the host no longer infers it from those. A view or context without the field converts nothing.
        for (NSDictionary *context in @[@{@"scheme": @0, @"local_mode": @"none", @"script_conversion": @YES}, @{@"scheme": @1, @"local_mode": @"quick_phrase", @"script_conversion": @YES}, @{@"scheme": @3, @"local_mode": @"none", @"script_conversion": @NO}, @{@"scheme": @0, @"local_mode": @"unicode", @"script_conversion": @NO}, @{@"scheme": @0, @"local_mode": @"temporary_japanese", @"script_conversion": @NO}, @{@"scheme": @1, @"local_mode": @"temporary_japanese", @"script_conversion": @NO}, @{@"scheme": @0, @"local_mode": @"none"}, @{}]) {
            BOOL convert = contextIndex++ < 2;
            assert(MSIMEScriptConversionApplies(context) == convert);
            NSMutableDictionary *candidateView = [scriptView mutableCopy];
            [candidateView removeObjectForKey:@"script_conversion"];
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
        japaneseView[@"script_conversion"] = @NO;
        japaneseView[@"candidates"] = @[@{@"text": @"日本国", @"highlighted": @YES, @"id": word[@"id"]}];
        [controller setValue:japaneseView forKey:@"view"];
        [controller renderCandidates];
        assert([PageButton(layoutPanel.contentView, 0).toolTip isEqual:@"日本国"]);
        assert([PageButton(layoutPanel.contentView, 0).candidateID isEqual:word[@"id"]]);
        [controller apply:@{@"commit": @"日本国", @"commit_context": @{@"scheme": @0, @"local_mode": @"temporary_japanese", @"script_conversion": @NO},
                            @"view": @{@"scheme": @0, @"local_mode": @"none", @"editing_text": @"", @"candidates": @[]}}];
        assert([client.committed isEqual:@"日本国"]);
        [controller selectSimplifiedOutput:nil];
        assert([controller.menu itemAtIndex:4].state == NSControlStateValueOff);
        [controller apply:@{@"commit": @"汉语", @"commit_context": @{@"scheme": @0, @"local_mode": @"none", @"script_conversion": @YES}, @"view": @{@"editing_text": @"", @"candidates": @[]}}];
        assert([client.committed isEqual:@"汉语"]);
        @autoreleasepool { TestGlassInputModeHUD(NO); }
        @autoreleasepool { TestInputMode(defaults, appearance); }
        @autoreleasepool { TestControlOptionSpace(); }
        @autoreleasepool { TestInputModePolicy(); }
        @autoreleasepool { TestPerApplicationPunctuationAndWidth(); }
        @autoreleasepool { TestEnglishModePunctuationAndWidthOutput(); }
        @autoreleasepool { TestApplicationInputModeRulesFollowSharedDocument(); }
        @autoreleasepool { TestApplicationInputModeRuleEditsKeepDocumentTable(); }
        @autoreleasepool { TestInputSourceModeReset(); }
        @autoreleasepool { TestTemporaryJapaneseLongVowelThroughRealSession(NO); }
        @autoreleasepool { TestTemporaryJapaneseLongVowelThroughRealSession(YES); }
        @autoreleasepool { TestRealSessionComposition(); }
        @autoreleasepool { TestEnglishOnlineCandidatesThroughRealSession(); }
        @autoreleasepool { TestRealTrilingualCycle(); }
        @autoreleasepool { TestInitialLearningThroughRealKeys(YES, NO); }
        @autoreleasepool { TestInitialLearningThroughRealKeys(YES, YES); }
        @autoreleasepool { TestInitialLearningThroughRealKeys(NO, NO); }
        @autoreleasepool { TestInitialLearningThroughRealKeys(NO, YES); }
        @autoreleasepool { TestSmartPunctuationVersionNumberThroughRealSession(); }
        @autoreleasepool { TestEditableShiftController(); }
        @autoreleasepool { TestModifierTaps(); }
        @autoreleasepool { TestShiftChordRecoversWithoutFlagsChanged(); }
        @autoreleasepool { TestInputModeStatusItem(); }
        @autoreleasepool { TestModifierTapSurvivesALostRelease(); }
        @autoreleasepool { TestCapsLockTap(); }
        @autoreleasepool { TestPunctuationAfterCompositionKeepsRawInput(appearance); }
        @autoreleasepool { TestStaleClientDeactivation(); }
        @autoreleasepool { TestStaleDeactivationLeavesTheCurrentPairOpen(); }
        @autoreleasepool { TestSoundsFollowKeysCommitsAndActivation(); }
        @autoreleasepool { TestTheKeyThatOpensTheSessionIsReportedBeforeItsDispatch(); }
        @autoreleasepool { TestCommitCompositionFinishesAsAFocusLoss(); }
        @autoreleasepool { TestFieldTraitsAreConfirmedBeforeEveryFocusIn(); }
        @autoreleasepool { TestMusicIsClaimedOnceTheSessionOpens(); }
        @autoreleasepool { TestPreferenceClientGeneration(); }
        @autoreleasepool { TestPreferenceReadDoesNotDeallocControllerOffMain(); }
        @autoreleasepool { TestDesktopLaunchDoesNotDeallocControllerOffMain(); }
        @autoreleasepool { TestSavedPreferencesReachTheFocusedController(); }
        @autoreleasepool { TestModeSwitchReachesTheSessionBeforeTheNextKey(); }
        @autoreleasepool { TestFreshProcessActsOnTheSharedSchemeNotTheStaleLocalOne(); }
        @autoreleasepool { TestPreferenceRevisionSkipsUnchangedDocuments(); }
        @autoreleasepool { TestPreferenceRevisionRetriesFailedUpdate(); }
        @autoreleasepool { TestKeyFocusRestoresPreferenceMonitoring(); }
        @autoreleasepool { TestUnreadablePreferencesAreRecoveredOnce(); }
        @autoreleasepool { TestProviderSettingsPersistTheSharedSnapshot(); }
        @autoreleasepool { TestInputModeSwitchDoesNotSaveSharedPreferences(); }
        @autoreleasepool { TestFullWidth(defaults, appearance); }
        @autoreleasepool { TestSessionOptions(); }
        @autoreleasepool { TestDiagnosticLogConfiguredBeforeSession(); }
        @autoreleasepool { TestKeypadDecimal(appearance); }
        @autoreleasepool { TestFloatingToolbarMenuToggle(appearance); }
        @autoreleasepool { TestCandidatePanelSingleOwner(); }
        @autoreleasepool { TestGlossArrivalRedrawsOnlyTheCard(appearance); }
        @autoreleasepool { TestNestedApplyKeepsTheNewerView(appearance); }
        @autoreleasepool { TestJapaneseConversionKeys(appearance); }
        @autoreleasepool { TestGlossSensePage(appearance); }
        @autoreleasepool { TestGlossSenseTraditionalOutput(appearance); }
        @autoreleasepool { TestPinyinArrowCandidateNavigation(); }
        @autoreleasepool { TestExpandedCandidateGrid(); }
        @autoreleasepool { TestExpandedCandidateGridGeneration(); }
        @autoreleasepool { TestSegmentEditingChords(appearance); }
        @autoreleasepool { TestSchemeTraitsFromView(appearance); }
        @autoreleasepool { TestSchemeKeyRouting(); }
        @autoreleasepool { TestBackspaceHoldDoesNotEscapeComposition(); }
        @autoreleasepool { TestPassthroughKeysAreCounted(); }
        @autoreleasepool { TestKeyLatencyIsLoggedWithoutTheKey(); }
        @autoreleasepool { TestKeypadOperators(appearance); }
        @autoreleasepool { TestSmartPunctuationPreferences(); }
        @autoreleasepool { TestSharedCharacterWidth(); }
        @autoreleasepool { TestScreenKeyboardShortcut(appearance); }
        @autoreleasepool { TestMaintenanceShortcuts(appearance); }
        @autoreleasepool { TestShortcutLetterFollowsKeyboardLayout(); }
        @autoreleasepool { TestPunctuation(defaults, appearance); }
        @autoreleasepool { TestPairedPunctuationPreferences(); }
        @autoreleasepool { TestPairedPunctuationHostExclusion(); }
        @autoreleasepool { TestPairedPunctuationClosesThePair(); }
        @autoreleasepool { TestPairedPunctuationClosesBrace(); }
        @autoreleasepool { TestShiftCornerQuotesThroughRealSession(); }
        @autoreleasepool { TestEmojiBridgeFallback(); }
        @autoreleasepool { TestMixedInputPreferences(); }
        @autoreleasepool { TestCharacterSetShortcut(); }
        @autoreleasepool { TestToggleHotkeyRebinding(); }
        @autoreleasepool { TestVimModeEscapeRouting(); }
        @autoreleasepool { TestDedicatedEnglish(appearance); }
        @autoreleasepool { TestConsecutiveUppercaseGoesToTheClient(appearance); }
        @autoreleasepool { TestSystemInputModeReport(appearance); }
        @autoreleasepool { TestOptInSchemeModes(); }
        @autoreleasepool { TestRestoredInputModeActivation(); }
        @autoreleasepool { TestKeymap(defaults, appearance); }
        @autoreleasepool { TestKeymapPreeditMargins(); }
        Method fontMethod = class_getClassMethod(NSFont.class, @selector(monospacedSystemFontOfSize:weight:));
        assert(fontMethod);
        originalMonospacedFont = method_setImplementation(fontMethod, (IMP)MissingKeyFont);
        @autoreleasepool { TestKeymap(defaults, appearance); }
        method_setImplementation(fontMethod, originalMonospacedFont);
        assert(missingKeyFontCalls > 0);
        [NSUserDefaults.standardUserDefaults removeObjectForKey:@"MSIMEClientPinnedCandidates"];
        MSIMERemoveTestPreferenceSuite(defaults, suite);
        MSIMERestoreVoiceHoldKeys(standardDefaults, previousVoiceHoldKeys);
    }
    return 0;
}
