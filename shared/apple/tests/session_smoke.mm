#import "MSIMEClientSession.h"
#import "TextClient.h"
#include "msime_client.h"
#include <cassert>
#include <initializer_list>

static BOOL gSnapshotIntegerProbeCalled;
static uint64_t gSnapshotIntegerProbeHandle;

@interface SnapshotIntegerProbe : MSIMEClientSession
@end

@implementation SnapshotIntegerProbe
+ (BOOL)discardSnapshotHandle:(uint64_t)handle error:(NSError **)error {
    (void)error;
    (void)self;
    gSnapshotIntegerProbeHandle = handle;
    gSnapshotIntegerProbeCalled = YES;
    return YES;
}
+ (BOOL)applySnapshotHandle:(uint64_t)handle expectedVersion:(NSString *)version error:(NSError **)error {
    (void)version;
    (void)error;
    (void)self;
    gSnapshotIntegerProbeHandle = handle;
    gSnapshotIntegerProbeCalled = YES;
    return YES;
}
@end

static NSDictionary *reload(MSIMEClientSession *session, NSString *directory, BOOL expectError) {
    __block BOOL done = NO;
    __block NSDictionary *loaded = nil;
    [session reloadPreferencesDirectory:directory completion:^(NSDictionary *result, NSError *error) {
        assert([NSThread isMainThread]);
        assert((error != nil) == expectError);
        loaded = result;
        done = YES;
    }];
    NSDate *deadline = [NSDate dateWithTimeIntervalSinceNow:5];
    while (!done && deadline.timeIntervalSinceNow > 0) {
        [[NSRunLoop currentRunLoop] runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.01]];
    }
    assert(done);
    return loaded;
}

int main() {
    @autoreleasepool {
        MSIMEApplyTransition((NSDictionary *)@[], nil);
        for (id invalid in @[@YES, @1.5, @1e30, @(-1), @0, @"1", NSNull.null]) {
            gSnapshotIntegerProbeCalled = NO;
            NSDictionary *discarded = [SnapshotIntegerProbe discardSnapshot:@{ @"handle": invalid }];
            assert(discarded[@"error"] && ![discarded[@"discarded"] boolValue]);
            assert(!gSnapshotIntegerProbeCalled);
            gSnapshotIntegerProbeCalled = NO;
            NSDictionary *applied = [SnapshotIntegerProbe applySnapshot:@{ @"handle": invalid,
                @"expectedVersion": [@"0" stringByPaddingToLength:64 withString:@"0" startingAtIndex:0] }];
            assert(applied[@"error"] && ![applied[@"activated"] boolValue]);
            assert(!gSnapshotIntegerProbeCalled);
        }
        NSDictionary *invalidDiscardObject = [SnapshotIntegerProbe discardSnapshot:(NSDictionary *)@YES];
        assert(invalidDiscardObject[@"error"]);
        NSDictionary *invalidApplyObject = [SnapshotIntegerProbe applySnapshot:(NSDictionary *)@YES];
        assert(invalidApplyObject[@"error"]);
        NSDictionary *invalidPrepareObject = [SnapshotIntegerProbe prepareSnapshot:(NSDictionary *)@YES];
        assert(invalidPrepareObject[@"error"]);
        gSnapshotIntegerProbeCalled = NO;
        NSDictionary *missingDiscardHandle = [SnapshotIntegerProbe discardSnapshot:@{}];
        assert(missingDiscardHandle[@"error"] && !gSnapshotIntegerProbeCalled);
        gSnapshotIntegerProbeCalled = NO;
        NSDictionary *missingApplyHandle = [SnapshotIntegerProbe applySnapshot:@{
            @"expectedVersion": [@"0" stringByPaddingToLength:64 withString:@"0" startingAtIndex:0] }];
        assert(missingApplyHandle[@"error"] && !gSnapshotIntegerProbeCalled);
        gSnapshotIntegerProbeCalled = NO;
        NSDictionary *discarded = [SnapshotIntegerProbe discardSnapshot:@{ @"handle": @42 }];
        assert([discarded[@"discarded"] isEqual:@YES] && gSnapshotIntegerProbeCalled && gSnapshotIntegerProbeHandle == 42);
        gSnapshotIntegerProbeCalled = NO;
        NSDictionary *maxDiscarded = [SnapshotIntegerProbe discardSnapshot:@{ @"handle": @(UINT64_MAX) }];
        assert([maxDiscarded[@"discarded"] isEqual:@YES] && gSnapshotIntegerProbeCalled && gSnapshotIntegerProbeHandle == UINT64_MAX);
        NSString *root = [NSTemporaryDirectory() stringByAppendingPathComponent:NSUUID.UUID.UUIDString];
        NSMutableDictionary *options = [@{@"api_version": @1, @"preferences": @{@"scheme": @"quanpin", @"default_ime_mode": @"chinese", @"candidate_page_size": @5, @"learning": @NO, @"chinese_punctuation": @YES}} mutableCopy];
        for (NSString *name in @[@"resources", @"user_data", @"cache", @"dictionaries"]) {
            NSString *path = [root stringByAppendingPathComponent:name];
            BOOL created = [[NSFileManager defaultManager] createDirectoryAtPath:path withIntermediateDirectories:YES attributes:nil error:nil];
            assert(created);
            options[name] = path;
        }
        NSError *error = nil;
        assert(![MSIMEClientSession handwritingProviderRequest:(NSDictionary *)@[] error:&error] && error);
        error = nil;
        assert([MSIMEClientSession emojiCatalogRequest:(NSDictionary *)@[]][@"error"]);
        MSIMEClientSession *session = [[MSIMEClientSession alloc] initWithOptions:options error:&error];
        assert(session && !error);
        NSDictionary *activeOptions = [MSIMEClientSession activeHostOptions];
        assert([activeOptions[@"api_version"] isEqual:@1]);
        error = nil;
        assert(![MSIMEClientSession applySnapshotHandle:UINT64_MAX expectedVersion:[@"0" stringByPaddingToLength:64 withString:@"0" startingAtIndex:0] error:&error]);
        assert(error);
        error = nil;
        NSDictionary *initialPreferences = [MSIMEClientSession loadPreferencesInDirectory:root error:&error];
        assert(initialPreferences && !error && [initialPreferences[@"revision"] isEqual:@0]);
        error = nil;
        assert(![MSIMEClientSession prepareHostWithResourcesDirectory:@"relative/resources" stateRoot:root error:&error] && error);
        error = nil;
        assert(![MSIMEClientSession prepareHostWithResourcesDirectory:@"" stateRoot:root error:&error] && error);
        error = nil;
        NSDictionary *saved = [MSIMEClientSession savePreferencesInDirectory:root expectedRevision:0 snapshot:@{@"format_version": @1, @"revision": @0, @"preferences": options[@"preferences"]} error:&error];
        assert(saved && !error && [saved[@"revision"] isEqual:@1]);
        error = nil;
        NSDictionary *stale = @{ @"format_version": @1, @"revision": @0, @"preferences": options[@"preferences"] };
        assert(![MSIMEClientSession savePreferencesInDirectory:root expectedRevision:0 snapshot:stale error:&error] && error);
        error = nil;
        NSDictionary *unsupported = @{ @"format_version": @2, @"revision": @1, @"preferences": options[@"preferences"] };
        assert(![MSIMEClientSession savePreferencesInDirectory:root expectedRevision:1 snapshot:unsupported error:&error] && error);
        NSDictionary *invalidDictionary = [MSIMEClientSession dictionaryRequest:@{@"options": options, @"action": @{@"operation": @"unknown"}} error:&error];
        assert(!invalidDictionary && error);
        error = nil;
        NSDictionary *invalidHandwriting = [MSIMEClientSession handwritingProviderRequest:@{@"language": @"zh-CN", @"socket_path": @"relative/provider", @"strokes": @[]} error:&error];
        assert(!invalidHandwriting && error);
        error = nil;
        assert(![session voiceProviderCancelSocket:(NSString *)@YES generation:1 error:&error] && error);
        error = nil;
        assert(![session voiceProviderStopSocket:(NSString *)@YES generation:1 error:&error] && error);
        error = nil;
        assert(![session applyVoiceText:(NSString *)@YES generation:1 error:&error] && error);
        error = nil;
        NSString *oversizedPath = [@"/tmp/" stringByPaddingToLength:4100 withString:@"x" startingAtIndex:0];
        NSDictionary *oversizedHandwriting = [MSIMEClientSession handwritingProviderRequest:@{@"language": @"zh-CN", @"socket_path": oversizedPath, @"strokes": @[]} error:&error];
        assert((!oversizedHandwriting && error));
        error = nil;
        assert(![MSIMEClientSession prepareHostWithResourcesDirectory:(NSString *)@YES stateRoot:root error:&error] && error);
        error = nil;
        assert(![MSIMEClientSession loadPreferencesInDirectory:(NSString *)@YES error:&error] && error);
        error = nil;
        NSMutableDictionary *oversized = [@{} mutableCopy];
        oversized[@"padding"] = [@"x" stringByPaddingToLength:70000 withString:@"x" startingAtIndex:0];
        assert(![MSIMEClientSession dictionaryRequest:oversized error:&error] && error);
        assert([session setFocused:YES error:&error]);
        assert([session typeASCII:'U' shift:YES error:&error]);
        for (uint8_t key : {'4', 'e', '2', 'd'}) assert([session typeASCII:key shift:NO error:&error]);
        NSDictionary *snapshot = @{@"format_version": @1, @"revision": @1, @"preferences": @{@"scheme": @"quanpin", @"candidate_page_size": @2, @"learning": @NO, @"chinese_punctuation": @NO}};
        NSDictionary *queued = [session updatePreferencesSnapshot:snapshot error:&error];
        assert([queued[@"deferred"] isEqual:@YES]);
        NSDictionary *result = [session command:MSIME_COMMIT_CANDIDATE error:&error];
        assert([result[@"commit"] isEqual:@"中"]);
        assert([[session updatePreferencesSnapshot:snapshot error:&error][@"deferred"] isEqual:@NO]);
        assert([[session typeASCII:',' shift:NO error:&error][@"handled"] isEqual:@NO]);
        NSString *preferencesPath = [root stringByAppendingPathComponent:@"preferences.json"];
        NSDictionary *on = @{@"format_version": @1, @"revision": @2, @"preferences": options[@"preferences"]};
        assert([[NSJSONSerialization dataWithJSONObject:on options:0 error:nil] writeToFile:preferencesPath atomically:YES]);
        assert([session typeASCII:'U' shift:YES error:&error]);
        for (uint8_t key : {'4', 'e', '2', 'd'}) assert([session typeASCII:key shift:NO error:&error]);
        assert([reload(session, root, NO)[@"deferred"] isEqual:@YES]);
        assert([[session command:MSIME_COMMIT_CANDIDATE error:&error][@"commit"] isEqual:@"中"]);
        assert([[session typeASCII:',' shift:NO error:&error][@"commit"] isEqual:@"，"]);
        assert([[@"broken" dataUsingEncoding:NSUTF8StringEncoding] writeToFile:preferencesPath atomically:YES]);
        assert(!reload(session, root, YES));
        assert([[session typeASCII:',' shift:NO error:&error][@"commit"] isEqual:@"，"]);
        __block BOOL rejected = NO;
        assert([session setChinesePunctuationEnabled:NO error:&error]);
        assert([[session typeASCII:',' shift:NO error:&error][@"handled"] isEqual:@NO]);
        assert([session setChinesePunctuationEnabled:YES error:&error]);
        assert([[session typeASCII:',' shift:NO error:&error][@"commit"] isEqual:@"，"]);
        // With paired completion off (the macOS host also sends this for an excluded app) no host supplies the closing half, so the Engine alone alternates quotes and nests book titles, as the reference does regardless of the setting (crates/engine/src/punctuation.rs).
        assert([session setPairedPunctuationEnabled:NO error:&error]);
        const struct { uint8_t key; NSString *mark; } unpaired[] = {
            {'"', @"“"}, {'"', @"”"}, {'"', @"“"}, {'"', @"”"}, {'\'', @"‘"}, {'\'', @"’"},
            {'<', @"《"}, {'<', @"〈"}, {'>', @"〉"}, {'>', @"》"}, {'>', @"》"}, {'<', @"《"}, {'>', @"》"},
        };
        for (const auto &step : unpaired) assert([[session punctuation:step.key error:&error][@"commit"] isEqual:step.mark]);
        assert([session setPairedPunctuationEnabled:YES error:&error]);
        error = nil;
        assert([[session resetCacheWithError:&error][@"handled"] isEqual:@YES] && !error);
        dispatch_semaphore_t done = dispatch_semaphore_create(0);
        dispatch_async(dispatch_get_global_queue(QOS_CLASS_DEFAULT, 0), ^{
            NSError *threadError = nil;
            rejected = ![session viewWithError:&threadError] && threadError != nil;
            rejected = rejected && ![session setChinesePunctuationEnabled:NO error:&threadError] && threadError != nil;
            dispatch_semaphore_signal(done);
        });
        assert(dispatch_semaphore_wait(done, dispatch_time(DISPATCH_TIME_NOW, 5 * NSEC_PER_SEC)) == 0);
        assert(rejected);
        // Nothing under preferences.plugins is on, so the sound calls queue nothing and start no player; a closed session answers the same without reaching the library.
        assert(![session keySound:0] && ![session keySound:1] && ![session commitSound] && ![session setMusicActive:YES]);
        assert([session closeWithError:&error]);
        assert(![session keySound:0] && ![session commitSound] && ![session setMusicActive:NO]);
        assert(![session resetCacheWithError:&error]);
        assert(![session setChinesePunctuationEnabled:NO error:&error]);
        assert(![session viewWithError:&error]);
        assert(error);
        [[NSFileManager defaultManager] removeItemAtPath:root error:nil];

        // An incomplete or unmatched special-mode input shows its raw text as the one Fallback candidate (source 9), and Space commits it, bare Y/R included, as in the reference's PrepareCandidateList (add_local_fallback_candidate in crates/engine/src/session/candidates.rs). Temporary English and Japanese stay off unless their resource files exist, so this session gets placeholders.
        NSString *localRoot = [NSTemporaryDirectory() stringByAppendingPathComponent:NSUUID.UUID.UUIDString];
        NSMutableDictionary *localOptions = [options mutableCopy];
        for (NSString *name in @[@"resources", @"user_data", @"cache", @"dictionaries"]) {
            NSString *path = [localRoot stringByAppendingPathComponent:name];
            BOOL created = [[NSFileManager defaultManager] createDirectoryAtPath:path withIntermediateDirectories:YES attributes:nil error:nil];
            assert(created);
            localOptions[name] = path;
        }
        for (NSString *name in @[@"msime-english.db", @"msime-japanese.dat"]) {
            BOOL written = [[@"fixture" dataUsingEncoding:NSUTF8StringEncoding] writeToFile:[localOptions[@"resources"] stringByAppendingPathComponent:name] atomically:YES];
            assert(written);
        }
        error = nil;
        MSIMEClientSession *local = [[MSIMEClientSession alloc] initWithOptions:localOptions error:&error];
        assert(local && !error);
        assert([local setFocused:YES error:&error]);
        const auto showsOnlyFallback = [](NSDictionary *transition, NSString *text) {
            NSArray *candidates = transition[@"view"][@"candidates"];
            return [candidates isKindOfClass:NSArray.class] && candidates.count == 1 &&
                   [candidates[0][@"text"] isEqual:text] && [candidates[0][@"source"] isEqual:@9];
        };
        assert(showsOnlyFallback([local typeASCII:'Y' shift:YES error:&error], @"Y"));
        NSDictionary *bareEnglish = [local command:MSIME_COMMIT_CANDIDATE error:&error];
        assert([bareEnglish[@"commit"] isEqual:@"Y"] && [bareEnglish[@"view"][@"candidates"] count] == 0);
        NSDictionary *unmatched = nil;
        assert([local typeASCII:'T' shift:YES error:&error]);
        for (uint8_t key : {'x', 'i', 'n'}) unmatched = [local typeASCII:key shift:NO error:&error];
        assert(showsOnlyFallback(unmatched, @"Txin"));
        assert([[local command:MSIME_COMMIT_CANDIDATE error:&error][@"commit"] isEqual:@"Txin"]);
        assert(showsOnlyFallback([local typeASCII:'R' shift:YES error:&error], @"R"));
        assert([[local command:MSIME_COMMIT_CANDIDATE error:&error][@"commit"] isEqual:@"R"]);
        assert([local closeWithError:&error]);
        [[NSFileManager defaultManager] removeItemAtPath:localRoot error:nil];
        puts("Apple Foundation consumer: input, commit, thread and lifetime checks passed");
    }
    return 0;
}
