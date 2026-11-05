#import "../../src/voice/GlobalVoiceScopeMenu.h"
#include <cassert>

@interface ScopeMenuTarget : NSObject
@property(nonatomic, copy) NSString *directory;
@property(nonatomic) BOOL saved;
- (void)choose:(id)sender;
@end
@implementation ScopeMenuTarget
- (void)choose:(id)sender { MSIMESelectGlobalVoiceScope(sender, ^{ self.saved = YES; }, ^{ self.saved = NO; }); }
@end

static void WaitUntil(BOOL (^done)(void)) {
    NSDate *deadline = [NSDate dateWithTimeIntervalSinceNow:5];
    while (!done() && deadline.timeIntervalSinceNow > 0)
        [NSRunLoop.currentRunLoop runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.005]];
    assert(done());
}

static NSDictionary *Snapshot(BOOL global) {
    return @{@"format_version":@1, @"revision":@1,
        @"preferences":@{@"learning":@NO, @"voice_input":@{@"hotkey_global":@(global), @"asr_provider":@"local"}}};
}

static void TestMenuStates(void) {
    for (NSNumber *value in @[@NO, @YES]) {
        NSMenuItem *root = MSIMEGlobalVoiceScopeMenuItem(Snapshot(value.boolValue), nil, @selector(choose:));
        assert([root.title isEqual:@"语音快捷键范围"] && root.enabled);
        assert(root.submenu.numberOfItems == 2 && !root.submenu.autoenablesItems);
        assert([[root.submenu itemAtIndex:0].title isEqual:@"仅水杉输入法"]);
        assert([[root.submenu itemAtIndex:1].title isEqual:@"任意输入法"]);
        for (NSInteger index = 0; index < 2; ++index) {
            NSMenuItem *choice = [root.submenu itemAtIndex:index];
            assert(choice.tag == index && choice.enabled);
            assert(choice.state == (value.boolValue == (index == 1) ? NSControlStateValueOn : NSControlStateValueOff));
        }
    }
    NSDictionary *old = @{@"revision":@0, @"preferences":@{@"voice_input":@{}}};
    assert([MSIMEGlobalVoiceScopeMenuItem(old, nil, @selector(choose:)).submenu itemAtIndex:0].state == NSControlStateValueOn);
    for (id invalid in @[NSNull.null, @{}, @{@"revision":@-1, @"preferences":@{@"voice_input":@{}}}]) {
        NSMenuItem *root = MSIMEGlobalVoiceScopeMenuItem(invalid == NSNull.null ? nil : invalid, nil, @selector(choose:));
        assert(!root.enabled);
        for (NSMenuItem *choice in root.submenu.itemArray) assert(!choice.enabled && choice.state == NSControlStateValueOff);
    }
}

static void TestConflictReload(void) {
    __block NSDictionary *current = Snapshot(NO);
    __block NSUInteger saves = 0;
    assert(MSIMESaveGlobalVoiceScope(YES, ^{ return current; }, ^NSDictionary *(uint64_t revision, NSDictionary *updated) {
        if (++saves == 1) {
            current = @{@"format_version":@1, @"revision":@2,
                @"preferences":@{@"learning":@YES, @"voice_input":@{@"asr_provider":@"openai", @"hotkey_ralt":@NO}}};
            return nil;
        }
        assert(revision == 2);
        current = updated;
        return updated;
    }));
    assert(saves == 2 && [current[@"preferences"][@"learning"] isEqual:@YES]);
    NSDictionary *voice = current[@"preferences"][@"voice_input"];
    assert([voice[@"hotkey_global"] isEqual:@YES] && [voice[@"asr_provider"] isEqual:@"openai"]);
    assert([voice[@"hotkey_ralt"] isEqual:@NO]);
    saves = 0;
    assert(!MSIMESaveGlobalVoiceScope(NO, ^{ return current; }, ^NSDictionary *(uint64_t revision, NSDictionary *updated) {
        (void)revision; (void)updated; ++saves; return nil;
    }));
    assert(saves == 2 && [current[@"preferences"][@"voice_input"][@"hotkey_global"] isEqual:@YES]);
    assert(!MSIMEStoreGlobalVoiceScope(nil, YES));
    assert(!MSIMELoadGlobalVoiceScope(@"relative-fixture"));
}

static void TestNativeActionPersists(void) {
    NSString *directory = [NSTemporaryDirectory() stringByAppendingPathComponent:NSUUID.UUID.UUIDString];
    ScopeMenuTarget *target = [ScopeMenuTarget new]; target.directory = directory;
    NSDictionary *initial = [MSIMEClientSession loadPreferencesInDirectory:directory error:nil];
    assert(initial);
    NSMutableDictionary *preferences = [initial[@"preferences"] mutableCopy];
    preferences[@"learning"] = @NO;
    NSMutableDictionary *voice = [preferences[@"voice_input"] mutableCopy];
    voice[@"hotkey_ralt"] = @NO;
    preferences[@"voice_input"] = voice;
    NSDictionary *saved = [MSIMEClientSession savePreferencesInDirectory:directory
        expectedRevision:[initial[@"revision"] unsignedLongLongValue]
        snapshot:@{@"format_version":@1, @"revision":initial[@"revision"], @"preferences":preferences} error:nil];
    assert(saved);
    for (NSNumber *value in @[@YES, @NO]) {
        NSMenuItem *root = MSIMEGlobalVoiceScopeMenuForDirectory(directory, target, @selector(choose:));
        if (value.boolValue) assert(!root.enabled);
        else assert(root.enabled);
        WaitUntil(^{ return root.enabled; });
        NSMenuItem *choice = [root.submenu itemAtIndex:value.boolValue ? 1 : 0];
        target.saved = NO;
        NSMenuItem *copied = [choice copy];
        assert([copied.representedObject isKindOfClass:NSString.class]);
        [target choose:@{kIMKCommandMenuItemName:copied}];
        WaitUntil(^{ return target.saved; });
        NSDictionary *after = MSIMELoadGlobalVoiceScope(directory);
        assert([after[@"preferences"][@"voice_input"][@"hotkey_global"] isEqual:value]);
        assert([after[@"preferences"][@"voice_input"][@"hotkey_ralt"] isEqual:@NO]);
        assert([after[@"preferences"][@"learning"] isEqual:@NO]);
        NSMenuItem *reopened = MSIMEGlobalVoiceScopeMenuItem(after, target, @selector(choose:));
        assert([reopened.submenu itemAtIndex:choice.tag].state == NSControlStateValueOn);
    }
    assert([NSFileManager.defaultManager removeItemAtPath:directory error:nil]);
}

static void TestReadDoesNotBlockMenu(void) {
    dispatch_semaphore_t gate = dispatch_semaphore_create(0);
    NSMenuItem *root = MSIMEGlobalVoiceScopeMenuForDirectory(@"/synthetic-fixture", nil, @selector(choose:),
        ^NSDictionary *(NSString *directory) {
            (void)directory;
            assert(!NSThread.isMainThread);
            dispatch_semaphore_wait(gate, DISPATCH_TIME_FOREVER);
            return Snapshot(NO);
        });
    assert(!root.enabled);
    __block BOOL mainQueueRan = NO;
    dispatch_async(dispatch_get_main_queue(), ^{ mainQueueRan = YES; });
    WaitUntil(^{ return mainQueueRan; });
    assert(!root.enabled);
    dispatch_semaphore_signal(gate);
    WaitUntil(^{ return root.enabled; });
    NSMenuItem *reopened = MSIMEGlobalVoiceScopeMenuForDirectory(@"/synthetic-fixture", nil, @selector(choose:),
        ^NSDictionary *(NSString *directory) { (void)directory; return Snapshot(NO); });
    assert(reopened.enabled && [reopened.submenu itemAtIndex:0].state == NSControlStateValueOn);
    __block BOOL saved = NO;
    MSIMESelectGlobalVoiceScope([root.submenu itemAtIndex:1], ^{ saved = YES; }, ^{ assert(false); },
        ^NSDictionary *(NSString *directory) { (void)directory; assert(!NSThread.isMainThread); return Snapshot(NO); },
        ^NSDictionary *(NSString *directory, uint64_t revision, NSDictionary *snapshot) {
            (void)directory; (void)revision; assert(!NSThread.isMainThread); return snapshot;
        });
    assert(!root.enabled);
    WaitUntil(^{ return saved; });
    assert(root.enabled && [root.submenu itemAtIndex:1].state == NSControlStateValueOn);
}

static void TestWarmSnapshotAndLateRead(void) {
    NSString *directory = [@"/synthetic-warm/" stringByAppendingString:NSUUID.UUID.UUIDString];
    MSIMERecordGlobalVoiceScopeSnapshot(directory, Snapshot(YES), MSIMEGlobalVoiceScopeReadEpoch(directory));
    dispatch_semaphore_t gate = dispatch_semaphore_create(0);
    NSMenuItem *root = MSIMEGlobalVoiceScopeMenuForDirectory(directory, nil, @selector(choose:),
        ^NSDictionary *(NSString *path) { (void)path; dispatch_semaphore_wait(gate, DISPATCH_TIME_FOREVER); return Snapshot(NO); });
    assert(root.enabled && [root.submenu itemAtIndex:1].state == NSControlStateValueOn);
    NSDictionary *newer = @{@"revision":@2,@"preferences":@{@"voice_input":@{@"hotkey_global":@YES}}};
    MSIMERecordGlobalVoiceScopeSnapshot(directory, newer, MSIMEGlobalVoiceScopeReadEpoch(directory));
    dispatch_semaphore_signal(gate);
    WaitUntil(^BOOL { return ![MSIMEVoiceScopeState(directory, NO)[@"loading"] boolValue]; });
    assert(root.enabled && [root.submenu itemAtIndex:1].state == NSControlStateValueOn);
    MSIMESelectGlobalVoiceScope(@{}, ^{ assert(false); }, ^{ assert(false); });
}

static void TestSaveAfterDocumentReset(void) {
    NSString *directory = [@"/synthetic-reset/" stringByAppendingString:NSUUID.UUID.UUIDString];
    NSMutableDictionary *old = [Snapshot(NO) mutableCopy]; old[@"revision"] = @100;
    const uint64_t oldReadEpoch = MSIMEGlobalVoiceScopeReadEpoch(directory);
    MSIMERecordGlobalVoiceScopeSnapshot(directory, old, oldReadEpoch);
    NSMenuItem *root = MSIMEGlobalVoiceScopeMenuItem(old, nil, @selector(choose:));
    NSMenuItem *choice = [root.submenu itemAtIndex:1]; choice.representedObject = directory;
    __block BOOL completed = NO;
    __block NSDictionary *stored = nil;
    MSIMESelectGlobalVoiceScope(choice, ^{ completed = YES; }, ^{ assert(false); },
        ^NSDictionary *(NSString *path) {
            (void)path; return @{@"format_version":@1,@"revision":@0,@"preferences":@{@"voice_input":@{@"hotkey_global":@NO}}};
        }, ^NSDictionary *(NSString *path, uint64_t revision, NSDictionary *snapshot) {
            (void)path; assert(revision == 0);
            NSMutableDictionary *updated = [snapshot mutableCopy]; updated[@"revision"] = @1; stored = updated; return updated;
        });
    WaitUntil(^{ return completed; });
    assert([stored[@"preferences"][@"voice_input"][@"hotkey_global"] isEqual:@YES]);
    NSDictionary *cached = MSIMEVoiceScopeState(directory, NO)[@"snapshot"];
    assert([cached[@"revision"] isEqual:@1] && [cached[@"preferences"][@"voice_input"][@"hotkey_global"] isEqual:@YES]);
    MSIMERecordGlobalVoiceScopeSnapshot(directory, old, oldReadEpoch);
    assert([MSIMEVoiceScopeState(directory, NO)[@"snapshot"] isEqual:cached]);
}

static void TestLateSavePreservesNewerSnapshot(void) {
    NSString *directory = [@"/synthetic-late-save/" stringByAppendingString:NSUUID.UUID.UUIDString];
    MSIMERecordGlobalVoiceScopeSnapshot(directory, Snapshot(NO), MSIMEGlobalVoiceScopeReadEpoch(directory));
    NSMenuItem *root = MSIMEGlobalVoiceScopeMenuItem(Snapshot(NO), nil, @selector(choose:));
    NSMenuItem *choice = [root.submenu itemAtIndex:1]; choice.representedObject = directory;
    dispatch_semaphore_t gate = dispatch_semaphore_create(0);
    __block BOOL completed = NO;
    MSIMESelectGlobalVoiceScope(choice, ^{ completed = YES; }, ^{ assert(false); },
        ^NSDictionary *(NSString *path) { (void)path; return Snapshot(NO); },
        ^NSDictionary *(NSString *path, uint64_t revision, NSDictionary *snapshot) {
            (void)path; (void)revision;
            dispatch_semaphore_wait(gate, DISPATCH_TIME_FOREVER);
            NSMutableDictionary *saved = [snapshot mutableCopy]; saved[@"revision"] = @1; return saved;
        });
    NSDictionary *newer = @{@"revision":@2,@"preferences":@{@"voice_input":@{@"hotkey_global":@NO}}};
    MSIMERecordGlobalVoiceScopeSnapshot(directory, newer, MSIMEGlobalVoiceScopeReadEpoch(directory));
    dispatch_semaphore_signal(gate);
    WaitUntil(^{ return completed; });
    assert([MSIMEVoiceScopeState(directory, NO)[@"snapshot"] isEqual:newer]);
}

static void TestResetRefreshRejectsPriorHostRead(void) {
    for (NSNumber *unreadable in @[@NO, @YES]) {
        NSString *directory = [@"/synthetic-refresh-reset/" stringByAppendingString:NSUUID.UUID.UUIDString];
        NSMutableDictionary *old = [Snapshot(YES) mutableCopy]; old[@"revision"] = @100;
        const uint64_t oldReadEpoch = MSIMEGlobalVoiceScopeReadEpoch(directory);
        MSIMERecordGlobalVoiceScopeSnapshot(directory, old, oldReadEpoch);
        MSIMEGlobalVoiceScopeMenuForDirectory(directory, nil, @selector(choose:),
            ^NSDictionary *(NSString *path) {
                (void)path;
                return unreadable.boolValue ? nil : @{@"revision":@0,@"preferences":@{@"voice_input":@{@"hotkey_global":@NO}}};
            });
        WaitUntil(^BOOL { return ![MSIMEVoiceScopeState(directory, NO)[@"loading"] boolValue]; });
        NSDictionary *accepted = MSIMEVoiceScopeState(directory, NO)[@"snapshot"];
        assert(unreadable.boolValue ? !accepted : [accepted[@"revision"] isEqual:@0]);
        MSIMERecordGlobalVoiceScopeSnapshot(directory, old, oldReadEpoch);
        assert(MSIMEVoiceScopeState(directory, NO)[@"snapshot"] == accepted);
    }
}

static void TestSaveNotification(void) {
    __block NSUInteger notifications = 0;
    id observer = [NSNotificationCenter.defaultCenter addObserverForName:MSIMEGlobalVoicePreferencesDidChangeNotification()
        object:nil queue:nil usingBlock:^(NSNotification *notification) { (void)notification; assert(NSThread.isMainThread); ++notifications; }];
    for (NSNumber *success in @[@NO, @YES]) {
        NSString *directory = [@"/synthetic-notify/" stringByAppendingString:NSUUID.UUID.UUIDString];
        MSIMERecordGlobalVoiceScopeSnapshot(directory, Snapshot(NO), MSIMEGlobalVoiceScopeReadEpoch(directory));
        NSMenuItem *item = [MSIMEGlobalVoiceScopeMenuItem(Snapshot(NO), nil, @selector(choose:)).submenu itemAtIndex:1];
        item.representedObject = directory;
        __block BOOL done = NO;
        MSIMESelectGlobalVoiceScope(item, ^{ assert(success.boolValue); done = YES; }, ^{ assert(!success.boolValue); done = YES; },
            ^NSDictionary *(NSString *path) { (void)path; return Snapshot(NO); },
            ^NSDictionary *(NSString *path, uint64_t revision, NSDictionary *snapshot) { (void)path; (void)revision; return success.boolValue ? snapshot : nil; });
        WaitUntil(^{ return done; });
        assert(notifications == (success.boolValue ? 1 : 0));
    }
    [NSNotificationCenter.defaultCenter removeObserver:observer];
}

int main(void) {
    @autoreleasepool {
        [NSApplication sharedApplication];
        TestSaveNotification(); TestMenuStates(); TestConflictReload(); TestNativeActionPersists(); TestReadDoesNotBlockMenu(); TestWarmSnapshotAndLateRead(); TestSaveAfterDocumentReset(); TestLateSavePreservesNewerSnapshot(); TestResetRefreshRejectsPriorHostRead();
    }
}
