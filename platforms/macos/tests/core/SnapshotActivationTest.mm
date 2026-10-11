#import "MSIMEClientSession.h"
#include "msime_client.h"
#include <cassert>

// 选择加入生命周期核心和最后报告的字段属性，要随会话替换恢复到新句柄上（restoreLiveModes:）。FEEDBACK 下旧的 typingEffect: 是空操作（没选择加入的会话会答出闪光样式和连击数），SECURE 下按键什么都不出；去掉 SECURE 之后按键才有特效，说明前面的 0 来自恢复的属性，而不是特效关着。最后把 SECURE 设回去，留给下一次替换。
static void AssertLifecycleRestored(MSIMEClientSession *session) {
    assert(session.lifecycleFlags == MSIME_LIFECYCLE_FEEDBACK);
    assert([session typingEffect:0] == 0);
    assert([session lifecycleKey:0] == 0);
    assert([session setFieldTraits:MSIME_TRAIT_KNOWN] == MSIME_TRAIT_KNOWN);
    assert([session lifecycleKey:0] != 0);
    assert([session setFieldTraits:MSIME_TRAIT_KNOWN | MSIME_TRAIT_SECURE] == (MSIME_TRAIT_KNOWN | MSIME_TRAIT_SECURE));
}

// Explicit integration test: pass the lock-verified desktop resource directory.
// Active and staged user data are always created beneath a fresh temporary root.
int main(int argc, const char *argv[]) {
    @autoreleasepool {
        if (argc != 2) { fputs("usage: snapshot-activation-test <desktop-resources>\n", stderr); return 2; }
        NSString *resources = [NSString stringWithUTF8String:argv[1]];
        NSString *root = [NSTemporaryDirectory() stringByAppendingPathComponent:NSUUID.UUID.UUIDString];
        NSString *staging = [root stringByAppendingPathComponent:@"staging"];
        assert([NSFileManager.defaultManager createDirectoryAtPath:staging withIntermediateDirectories:YES attributes:nil error:nil]);
        __block NSError *error = nil;
        __block NSDictionary *preparedHost = nil;
        dispatch_sync(dispatch_get_global_queue(QOS_CLASS_UTILITY, 0), ^{
            preparedHost = [MSIMEClientSession prepareHostWithResourcesDirectory:resources stateRoot:[root stringByAppendingPathComponent:@"active"] error:&error];
        });
        assert(preparedHost && !error);
        NSMutableDictionary *options = [preparedHost mutableCopy];
        NSMutableDictionary *preferences = [options[@"preferences"] mutableCopy];
        preferences[@"learning"] = @NO;
        // 打字特效开着，AssertLifecycleRestored 才分得出选择加入和 SECURE 有没有恢复。
        NSMutableDictionary *plugins = [preferences[@"plugins"] isKindOfClass:NSDictionary.class] ? [preferences[@"plugins"] mutableCopy] : [NSMutableDictionary dictionary];
        plugins[@"effect_style"] = @"flash";
        preferences[@"plugins"] = plugins;
        options[@"preferences"] = preferences;
        MSIMEClientSession *session = [[MSIMEClientSession alloc] initWithOptions:options error:&error];
        assert(session && !error && [session setFocused:YES error:&error]);
        assert([session setLifecycle:MSIME_LIFECYCLE_FEEDBACK] == MSIME_LIFECYCLE_FEEDBACK);
        assert([session setFieldTraits:MSIME_TRAIT_KNOWN | MSIME_TRAIT_SECURE] == (MSIME_TRAIT_KNOWN | MSIME_TRAIT_SECURE));
        for (NSNumber *enabled in @[@YES, @NO]) {
            assert([session setDedicatedEnglishEnabled:enabled.boolValue error:&error] && !error);
            assert([session setChinesePunctuationEnabled:!enabled.boolValue error:&error] && !error);
            assert([session setCharacterWidthFull:!enabled.boolValue error:&error] && !error);
            uint64_t oldSession = [[session viewWithError:&error][@"session"] unsignedLongLongValue];
            NSString *version = [MSIMEClientSession snapshotVersionForOptions:options error:&error];
            assert(version && !error);
            NSString *activationID = @"00112233-4455-6677-8899-aabbccddeeff";
            NSDictionary *record = @{@"type":@"overlay", @"deleted":@NO,
                @"data":@{@"kind":@"english", @"code":@"msimesnapshotfixture", @"word":@"msimesnapshotfixture", @"weight":@10000, @"user_inserted":@YES}};
            __block NSDictionary *prepared = nil;
            dispatch_sync(dispatch_get_global_queue(QOS_CLASS_UTILITY, 0), ^{
                __block BOOL sent = NO;
                prepared = [MSIMEClientSession prepareSnapshotRequest:@{@"options":options, @"staging_root":staging, @"expected_version":version, @"records":@1, @"activation_id":activationID}
                    nextRecord:^NSDictionary *(NSError **readerError) {
                        (void)readerError;
                        if (sent) return nil;
                        sent = YES;
                        return record;
                    } error:&error];
            });
            assert(prepared && !error && [prepared[@"source_version"] isEqual:version]);
            __block NSUInteger notifications = 0;
            id observer = [NSNotificationCenter.defaultCenter addObserverForName:MSIMEClientSessionDidReplaceSnapshotNotification object:session queue:nil usingBlock:^(NSNotification *note) {
                assert(note.object == session && NSThread.isMainThread);
                assert([[session viewWithError:nil][@"dedicated_english"] isEqual:enabled]);
                assert([[session viewWithError:nil][@"character_width"] isEqual:enabled.boolValue ? @"Halfwidth" : @"Fullwidth"]);
                ++notifications;
            }];
            assert([[MSIMEClientSession snapshotActivationReady][@"ready"] isEqual:@YES]);
            assert([session typeASCII:'U' shift:YES error:&error]);
            assert([[MSIMEClientSession snapshotActivationReady][@"ready"] isEqual:@NO]);
            error = nil;
            assert(![MSIMEClientSession applySnapshotHandle:[prepared[@"handle"] unsignedLongLongValue] expectedVersion:version error:&error]);
            assert(error && [[session viewWithError:nil][@"editing_text"] length] > 0);
            error = nil;
            assert([session command:MSIME_CANCEL error:&error]);
            assert([[MSIMEClientSession snapshotActivationReady][@"ready"] isEqual:@YES]);
            // 失败恢复：版本对不上时激活失败，旧会话已经销毁，包装层用原选项新建一个会话顶上，同样经 restoreLiveModes: 恢复模式、选择加入和字段属性，并发出替换通知。准备好的快照还在，下面照常激活。
            error = nil;
            NSString *staleVersion = [@"" stringByPaddingToLength:64 withString:@"0" startingAtIndex:0];
            assert(![staleVersion isEqual:version]);
            assert(![MSIMEClientSession applySnapshotHandle:[prepared[@"handle"] unsignedLongLongValue] expectedVersion:staleVersion error:&error]);
            assert(error && notifications == 1);
            AssertLifecycleRestored(session);
            error = nil;
            BOOL activated = [MSIMEClientSession applySnapshotHandle:[prepared[@"handle"] unsignedLongLongValue] expectedVersion:version error:&error];
            if (!activated) NSLog(@"Synthetic snapshot activation failed: %@", error.localizedDescription);
            assert(activated);
            assert(!error && notifications == 2);
            AssertLifecycleRestored(session);
            assert([[MSIMEClientSession snapshotVersion:options][@"generation"] isEqual:activationID]);
            [NSNotificationCenter.defaultCenter removeObserver:observer];
            NSDictionary *view = [session viewWithError:&error];
            assert([view[@"session"] unsignedLongLongValue] != oldSession && [view[@"editing_text"] length] == 0);
            assert([view[@"dedicated_english"] isEqual:enabled]);
            assert([session setFocused:YES error:&error]);
            if (enabled.boolValue) {
                NSString *text = @"msimesnapshotfixture";
                for (NSUInteger i = 0; i < text.length; ++i) assert([session typeASCII:[text characterAtIndex:i] shift:NO error:&error]);
                assert([[[session viewWithError:&error][@"candidates"] firstObject][@"text"] isEqual:text]);
                assert([[session command:MSIME_COMMIT_CANDIDATE error:&error][@"commit"] isEqual:text]);
            }
            // Isolate punctuation behavior from the restored English/width modes.
            assert([session setDedicatedEnglishEnabled:NO error:&error]);
            assert([session setCharacterWidthFull:NO error:&error]);
            NSDictionary *punctuation = [session typeASCII:',' shift:NO error:&error];
            assert(punctuation && !error && [punctuation[@"handled"] boolValue] == !enabled.boolValue);
            if (!enabled.boolValue) assert([punctuation[@"commit"] isEqual:@"，"]);
            else assert(![punctuation[@"commit"] isKindOfClass:NSString.class]);
        }
        assert([session closeWithError:&error] && !error);
        assert([NSFileManager.defaultManager removeItemAtPath:root error:nil]);
        puts("Packaged snapshot activation, English mode retention and imported candidate commit passed");
    }
    return 0;
}
