#import "../../src/cloud/TranslationSettingsWindow.h"
#import "MSIMEClientSession.h"
#include <cassert>

@interface MSIMETranslationSettingsWindow (TestActions)
- (void)reload:(id)sender;
- (void)revealKey:(id)sender;
- (void)revealTencentKey:(id)sender;
- (void)controlChanged:(id)sender;
- (void)providerChanged:(id)sender;
- (void)revealNiuTransKey:(id)sender;
- (void)controlTextDidEndEditing:(NSNotification *)notification;
@end
static void Wait(MSIMETranslationSettingsWindow *window) {
    NSDate *deadline = [NSDate dateWithTimeIntervalSinceNow:3];
    while (([[window valueForKey:@"busy"] boolValue] || [[window valueForKey:@"saving"] boolValue]) && deadline.timeIntervalSinceNow > 0)
        [NSRunLoop.mainRunLoop runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.005]];
    assert(![[window valueForKey:@"busy"] boolValue] && ![[window valueForKey:@"saving"] boolValue]);
}
// A text field losing focus: whatever differs from what was last written is saved.
static void Commit(MSIMETranslationSettingsWindow *window) {
    [window controlTextDidEndEditing:[NSNotification notificationWithName:NSControlTextDidEndEditingNotification object:nil]];
    Wait(window);
}
int main() {
    @autoreleasepool {
        [NSApplication sharedApplication];
        NSString *root = [NSTemporaryDirectory() stringByAppendingPathComponent:NSUUID.UUID.UUIDString];
        NSError *error = nil;
        NSDictionary *initial = [MSIMEClientSession loadPreferencesInDirectory:root error:&error];
        assert(initial && !error);
        __block NSUInteger saves = 0;
        MSIMETranslationSettingsWindow *window = [[MSIMETranslationSettingsWindow alloc] initWithDirectory:root saved:^(NSDictionary *preferences) {
            assert(NSThread.isMainThread && [preferences[@"custom_translation"] isKindOfClass:NSDictionary.class]); ++saves;
        }];
        [window showWindow:nil]; Wait(window);
        NSButton *enabled = [window valueForKey:@"enabled"], *offline = [window valueForKey:@"offline"];
        NSButton *reveal = [window valueForKey:@"reveal"];
        NSPopUpButton *provider = [window valueForKey:@"provider"];
        NSGridView *grid = [window valueForKey:@"grid"];
        NSTextField *endpoint = [window valueForKey:@"endpoint"], *plain = [window valueForKey:@"plainKey"];
        NSSecureTextField *key = [window valueForKey:@"key"];
        NSPopUpButton *target = [window valueForKey:@"target"];
        NSPopUpButton *secondary = [window valueForKey:@"secondary"];
        NSButton *tencent = [window valueForKey:@"tencent"], *revealTencent = [window valueForKey:@"revealTencent"];
        NSTextField *secretId = [window valueForKey:@"secretId"], *region = [window valueForKey:@"region"], *plainTencent = [window valueForKey:@"plainTencentKey"];
        NSSecureTextField *tencentKey = [window valueForKey:@"tencentKey"];
        assert([tencentKey isKindOfClass:NSSecureTextField.class] && !tencentKey.hidden && plainTencent.hidden);
        // 新装默认选「水杉账号」（#2519）：腾讯云默认的 `enabled: true` 没有可用凭据，不算用户的选择，凭据行全部禁用。
        assert(provider.indexOfSelectedItem == 3 && !tencent.enabled && !secretId.enabled && !tencentKey.enabled);
        assert(tencent.state == NSControlStateValueOn && [region.stringValue isEqual:@"ap-guangzhou"]);
        // 下面的流程从一份没有选过账号的已有配置开始，即升级上来的用户：文档里没有 `translation_account`，按未选择读，服务落在腾讯云。
        NSMutableDictionary *existing = [initial mutableCopy], *existingPreferences = [initial[@"preferences"] mutableCopy];
        [existingPreferences removeObjectForKey:@"translation_account"]; existing[@"preferences"] = existingPreferences;
        assert([MSIMEClientSession savePreferencesInDirectory:root expectedRevision:[initial[@"revision"] unsignedLongLongValue] snapshot:existing error:&error] && !error);
        initial = [MSIMEClientSession loadPreferencesInDirectory:root error:&error];
        assert(initial && !error && ![initial[@"preferences"][@"translation_account"] boolValue]);
        [window reload:nil]; Wait(window); assert(saves == 0);
        assert(tencent.state == NSControlStateValueOn && secretId.enabled && [region.stringValue isEqual:@"ap-guangzhou"]);
        secretId.stringValue = @"AKIDsynthetic"; tencentKey.stringValue = @"synthetic-tencent";
        assert([key isKindOfClass:NSSecureTextField.class] && !key.hidden && plain.hidden);
        NSTextField *appId = [window valueForKey:@"appId"], *plainNiuTrans = [window valueForKey:@"plainNiuTransKey"];
        NSSecureTextField *niuTransKey = [window valueForKey:@"niuTransKey"];
        NSButton *revealNiuTrans = [window valueForKey:@"revealNiuTrans"];
        assert(target.numberOfItems == 7 && secondary.numberOfItems == 8 && secondary.indexOfSelectedItem == 0);
        assert(provider.indexOfSelectedItem == 0 && !endpoint.enabled);
        assert(([provider.itemTitles isEqual:@[@"腾讯云", @"小牛翻译（NiuTrans）", @"自定义 DeepLX", @"水杉账号（发送到 api.msime.app）"]]));
        assert(offline.state == NSControlStateValueOff && offline.enabled);
        assert([grid rowAtIndex:5].hidden && ![grid rowAtIndex:8].hidden);
        // A provider whose fields are incomplete is not written.
        [provider selectItemAtIndex:1]; [window providerChanged:nil]; Wait(window); assert(saves == 0);
        assert(![grid rowAtIndex:11].hidden && ![grid rowAtIndex:12].hidden && appId.enabled && niuTransKey.enabled);
        appId.stringValue = @"synthetic-niutrans-app"; niuTransKey.stringValue = @"synthetic-niutrans-key";
        appId.stringValue = @""; Commit(window); assert(saves == 0);
        appId.stringValue = @"synthetic-niutrans-app";
        // Revealing a key ends the edit only after the text has moved, so the key is never saved empty.
        revealNiuTrans.state = NSControlStateValueOn; [window revealNiuTransKey:nil]; Wait(window); assert(saves == 1);
        assert(niuTransKey.hidden && !plainNiuTrans.hidden && !niuTransKey.stringValue.length);
        NSDictionary *stored = [MSIMEClientSession loadPreferencesInDirectory:root error:&error];
        assert([stored[@"preferences"][@"niutrans"][@"enabled"] isEqual:@YES]);
        assert([stored[@"preferences"][@"niutrans"][@"apikey"] isEqual:@"synthetic-niutrans-key"]);
        assert([stored[@"preferences"][@"tencent_tmt"][@"secret_id"] isEqual:@"AKIDsynthetic"]);
        plainNiuTrans.stringValue = @"synthetic-niutrans-key-edited";
        revealNiuTrans.state = NSControlStateValueOff; [window revealNiuTransKey:nil]; Wait(window); assert(saves == 2);
        assert([niuTransKey.stringValue isEqual:@"synthetic-niutrans-key-edited"] && !plainNiuTrans.stringValue.length);
        [provider selectItemAtIndex:2]; [window providerChanged:nil]; Wait(window); assert(saves == 2);
        assert(![grid rowAtIndex:5].hidden && [grid rowAtIndex:8].hidden);
        assert(endpoint.enabled && key.enabled);
        assert(!tencent.enabled && !secretId.enabled && !tencentKey.enabled && !region.enabled);
        endpoint.stringValue = @"file:///synthetic";
        Commit(window);
        assert(saves == 2);
        endpoint.stringValue = @"https://translation.invalid/api"; key.stringValue = @"synthetic";
        reveal.state = NSControlStateValueOn; [window revealKey:nil];
        assert(key.hidden && !plain.hidden && !key.stringValue.length && [plain.stringValue isEqual:@"synthetic"]);
        // An edit made while a save is in flight is written once that save finishes.
        plain.stringValue = @"synthetic-edited";
        reveal.state = NSControlStateValueOff; [window revealKey:nil];
        assert([key.stringValue isEqual:@"synthetic-edited"] && !plain.stringValue.length);
        Wait(window); assert(saves == 4);
        [target selectItemAtIndex:1]; [window controlChanged:target]; Wait(window); assert(saves == 5);
        [secondary selectItemAtIndex:3]; [window controlChanged:secondary]; Wait(window); assert(saves == 6);
        stored = [MSIMEClientSession loadPreferencesInDirectory:root error:&error];
        assert(stored && !error);
        assert([stored[@"preferences"][@"translation_target_language"] isEqual:@"fr"]);
        assert([stored[@"preferences"][@"translation_secondary_language"] isEqual:@"ja"]);
        assert([stored[@"preferences"][@"custom_translation"][@"api_key"] isEqual:@"synthetic-edited"]);
        assert([stored[@"preferences"][@"custom_translation"][@"enabled"] isEqual:@YES]);
        assert([stored[@"preferences"][@"niutrans"][@"enabled"] isEqual:@NO]);
        assert([stored[@"preferences"][@"niutrans"][@"app_id"] isEqual:@"synthetic-niutrans-app"]);
        assert([stored[@"preferences"][@"niutrans"][@"apikey"] isEqual:@"synthetic-niutrans-key-edited"]);
        assert([stored[@"preferences"][@"tencent_tmt"][@"secret_key"] isEqual:@"synthetic-tencent"]);
        assert([stored[@"preferences"][@"cloud_candidates"] isEqual:initial[@"preferences"][@"cloud_candidates"]]);
        // A different writer advances the revision; the edit is moved onto it and keeps what that writer stored.
        NSMutableDictionary *other = [stored mutableCopy], *otherPreferences = [stored[@"preferences"] mutableCopy];
        otherPreferences[@"candidate_page_size"] = @7; other[@"preferences"] = otherPreferences;
        assert([MSIMEClientSession savePreferencesInDirectory:root expectedRevision:[stored[@"revision"] unsignedLongLongValue] snapshot:other error:&error] && !error);
        endpoint.stringValue = @"https://changed.invalid/api";
        Commit(window);
        assert(saves == 7);
        stored = [MSIMEClientSession loadPreferencesInDirectory:root error:&error];
        assert([stored[@"preferences"][@"candidate_page_size"] isEqual:@7]);
        assert([stored[@"preferences"][@"custom_translation"][@"endpoint"] isEqual:@"https://changed.invalid/api"]);
        [window reload:nil]; Wait(window);
        assert([endpoint.stringValue isEqual:@"https://changed.invalid/api"]);
        assert(provider.indexOfSelectedItem == 2 && ![grid rowAtIndex:5].hidden && [grid rowAtIndex:8].hidden);
        assert(secondary.indexOfSelectedItem == 3);
        assert(offline.state == NSControlStateValueOff);
        enabled.state = NSControlStateValueOff; [provider selectItemAtIndex:0];
        [window providerChanged:nil]; assert(!target.enabled && !secondary.enabled && !endpoint.enabled);
        offline.state = NSControlStateValueOn; [window controlChanged:offline];
        assert(target.enabled && secondary.enabled);
        Wait(window); assert(saves == 9);
        stored = [MSIMEClientSession loadPreferencesInDirectory:root error:&error];
        assert([stored[@"preferences"][@"candidate_page_size"] isEqual:@7]);
        assert([stored[@"preferences"][@"candidate_translations"] isEqual:@NO]);
        assert([stored[@"preferences"][@"candidate_english_gloss"] isEqual:@YES]);
        assert([stored[@"preferences"][@"custom_translation"][@"enabled"] isEqual:@NO]);
        assert([stored[@"preferences"][@"custom_translation"][@"api_key"] isEqual:@"synthetic-edited"]);
        assert(tencent.enabled && secretId.enabled && tencentKey.enabled);
        revealTencent.state = NSControlStateValueOn; [window revealTencentKey:nil]; Wait(window); assert(saves == 9);
        assert(tencentKey.hidden && !plainTencent.hidden && !tencentKey.stringValue.length);
        assert([plainTencent.stringValue isEqual:@"synthetic-tencent"]);
        [provider selectItemAtIndex:2]; [window providerChanged:nil]; Wait(window); assert(saves == 10);
        assert(revealTencent.state == NSControlStateValueOff && !plainTencent.stringValue.length);
        assert([tencentKey.stringValue isEqual:@"synthetic-tencent"] && ![grid rowAtIndex:5].hidden);
        reveal.state = NSControlStateValueOn; [window revealKey:nil];
        [provider selectItemAtIndex:0]; [window providerChanged:nil]; Wait(window); assert(saves == 11);
        assert(reveal.state == NSControlStateValueOff && !plain.stringValue.length);
        assert([key.stringValue isEqual:@"synthetic-edited"] && [grid rowAtIndex:5].hidden);
        [provider selectItemAtIndex:1]; [window providerChanged:nil]; Wait(window); assert(saves == 12);
        assert(appId.enabled && [appId.stringValue isEqual:@"synthetic-niutrans-app"]);
        stored = [MSIMEClientSession loadPreferencesInDirectory:root error:&error];
        assert([stored[@"preferences"][@"niutrans"][@"enabled"] isEqual:@YES]);
        [provider selectItemAtIndex:0]; [window providerChanged:nil]; Wait(window); assert(saves == 13);
        revealTencent.state = NSControlStateValueOn; [window revealTencentKey:nil];
        plainTencent.stringValue = @"synthetic-tencent-edited"; region.stringValue = @"ap-shanghai";
        Commit(window); assert(saves == 14);
        stored = [MSIMEClientSession loadPreferencesInDirectory:root error:&error];
        assert([stored[@"preferences"][@"tencent_tmt"][@"secret_key"] isEqual:@"synthetic-tencent-edited"]);
        assert([stored[@"preferences"][@"tencent_tmt"][@"region"] isEqual:@"ap-shanghai"]);
        revealTencent.state = NSControlStateValueOff; [window revealTencentKey:nil]; Wait(window); assert(saves == 14);
        assert(!plainTencent.stringValue.length && [tencentKey.stringValue isEqual:@"synthetic-tencent-edited"]);
        // Shared validation rejects malformed drafts without changing stored settings.
        region.stringValue = @"invalid\nregion"; Commit(window); assert(saves == 14);
        assert([[MSIMEClientSession loadPreferencesInDirectory:root error:&error][@"revision"] isEqual:stored[@"revision"]]);
        [window reload:nil]; Wait(window);
        assert([region.stringValue isEqual:@"ap-shanghai"] && !tencentKey.hidden && plainTencent.hidden);
        assert(provider.indexOfSelectedItem == 0 && [grid rowAtIndex:5].hidden && ![grid rowAtIndex:8].hidden);
        // A Tencent edit made over a concurrent write is moved onto it the same way.
        other = [stored mutableCopy]; otherPreferences = [stored[@"preferences"] mutableCopy];
        otherPreferences[@"candidate_page_size"] = @8; other[@"preferences"] = otherPreferences;
        assert([MSIMEClientSession savePreferencesInDirectory:root expectedRevision:[stored[@"revision"] unsignedLongLongValue] snapshot:other error:&error]);
        secretId.stringValue = @"AKIDchanged"; Commit(window); assert(saves == 15);
        [window reload:nil]; Wait(window); assert([secretId.stringValue isEqual:@"AKIDchanged"]);
        tencent.state = NSControlStateValueOff; [window controlChanged:tencent];
        assert(!secretId.enabled && !tencentKey.enabled && !region.enabled);
        Wait(window); assert(saves == 16);
        stored = [MSIMEClientSession loadPreferencesInDirectory:root error:&error];
        assert([stored[@"preferences"][@"tencent_tmt"][@"enabled"] isEqual:@NO]);
        assert([stored[@"preferences"][@"tencent_tmt"][@"secret_key"] isEqual:@"synthetic-tencent-edited"]);
        assert([stored[@"preferences"][@"candidate_page_size"] isEqual:@8]);
        [secondary selectItemAtIndex:0]; [window controlChanged:secondary]; Wait(window); assert(saves == 17);
        stored = [MSIMEClientSession loadPreferencesInDirectory:root error:&error];
        assert(!stored[@"preferences"][@"translation_secondary_language"]);
        offline.state = NSControlStateValueOff; [window controlChanged:offline];
        assert(!target.enabled && !secondary.enabled);
        Wait(window); assert(saves == 18);
        stored = [MSIMEClientSession loadPreferencesInDirectory:root error:&error];
        assert([stored[@"preferences"][@"candidate_english_gloss"] isEqual:@NO]);
        assert([stored[@"preferences"][@"translation_account"] isEqual:@NO]);
        // The MSIME account is an explicit choice: it hides every credential row, and saving it turns Tencent off even if its checkbox was left on.
        tencent.state = NSControlStateValueOn;
        [provider selectItemAtIndex:3]; [window providerChanged:nil];
        assert(!tencent.enabled && !secretId.enabled && !appId.enabled && !endpoint.enabled);
        assert([grid rowAtIndex:5].hidden && [grid rowAtIndex:7].hidden && [grid rowAtIndex:8].hidden && [grid rowAtIndex:11].hidden);
        Wait(window); assert(saves == 19);
        stored = [MSIMEClientSession loadPreferencesInDirectory:root error:&error];
        assert([stored[@"preferences"][@"translation_account"] isEqual:@YES]);
        assert([stored[@"preferences"][@"tencent_tmt"][@"enabled"] isEqual:@NO]);
        assert([stored[@"preferences"][@"tencent_tmt"][@"secret_key"] isEqual:@"synthetic-tencent-edited"]);
        assert([stored[@"preferences"][@"niutrans"][@"enabled"] isEqual:@NO]);
        assert([stored[@"preferences"][@"custom_translation"][@"enabled"] isEqual:@NO]);
        [window reload:nil]; Wait(window);
        assert(provider.indexOfSelectedItem == 3 && tencent.state == NSControlStateValueOff && !tencent.enabled);
        // Any other choice drops the key, so the document goes back to what an older strict parser can read.
        [provider selectItemAtIndex:0]; [window providerChanged:nil];
        assert(tencent.enabled && ![grid rowAtIndex:8].hidden);
        Wait(window); assert(saves == 20);
        stored = [MSIMEClientSession loadPreferencesInDirectory:root error:&error];
        assert([stored[@"preferences"][@"translation_account"] isEqual:@NO]);
        [window reload:nil]; Wait(window); assert(provider.indexOfSelectedItem == 0);
        // Controls that reflect what is stored write nothing.
        Commit(window); [window controlChanged:enabled]; Wait(window); assert(saves == 20);
        [window.window.contentView layoutSubtreeIfNeeded];
        NSView *stack = window.window.contentView.subviews.firstObject;
        assert(NSMinY(stack.frame) >= 0 && NSMaxY(stack.frame) <= NSHeight(window.window.contentView.bounds));
        assert(NSMinX(stack.frame) >= 0 && NSMaxX(stack.frame) <= NSWidth(window.window.contentView.bounds));
        for (NSView *field in @[secretId, tencentKey, region, tencent, appId, niuTransKey]) {
            NSRect rect = [field convertRect:field.bounds toView:window.window.contentView];
            assert(NSMinX(rect) >= 0 && NSMaxX(rect) <= NSWidth(window.window.contentView.bounds));
            assert(NSMinY(rect) >= 0 && NSMaxY(rect) <= NSHeight(window.window.contentView.bounds));
        }
        // Closing the window writes an edit that never lost focus instead of discarding it.
        region.stringValue = @"ap-beijing";
        [window close];
        assert(!key.stringValue.length && !plain.stringValue.length && ![window valueForKey:@"snapshot"]);
        assert(!secretId.stringValue.length && !tencentKey.stringValue.length && !plainTencent.stringValue.length);
        assert(!appId.stringValue.length && !niuTransKey.stringValue.length && !plainNiuTrans.stringValue.length);
        NSDate *deadline = [NSDate dateWithTimeIntervalSinceNow:3];
        while (saves == 20 && deadline.timeIntervalSinceNow > 0)
            [NSRunLoop.mainRunLoop runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.005]];
        assert(saves == 21);
        stored = [MSIMEClientSession loadPreferencesInDirectory:root error:&error];
        assert([stored[@"preferences"][@"tencent_tmt"][@"region"] isEqual:@"ap-beijing"]);
        assert([stored[@"preferences"][@"tencent_tmt"][@"secret_id"] isEqual:@"AKIDchanged"]);
        [window showWindow:nil]; [window close];
        [NSRunLoop.mainRunLoop runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.05]];
        assert(saves == 21);
        assert(!key.stringValue.length && ![window valueForKey:@"snapshot"]);
        assert(!secretId.stringValue.length && !tencentKey.stringValue.length && !plainTencent.stringValue.length);
        assert(!appId.stringValue.length && !niuTransKey.stringValue.length && !plainNiuTrans.stringValue.length);
        // Real Apple -> C -> Rust disk roundtrip, outside the main input thread.
        dispatch_semaphore_t done = dispatch_semaphore_create(0);
        dispatch_async(dispatch_get_global_queue(QOS_CLASS_UTILITY, 0), ^{
            assert(!NSThread.isMainThread);
            NSError *failure = nil;
            NSDictionary *write = @{@"directory":root, @"action":@"remember", @"target_language":@"en", @"generation":@19,
                @"items":@[@{@"text":@"Hello", @"direction":@"english_to_chinese", @"translation":@"你好"}]};
            NSDictionary *saved = [MSIMEClientSession learnedTranslationRequest:write error:&failure];
            assert(saved && !failure && [saved[@"saved"] isEqual:@1]);
            NSDictionary *read = @{@"directory":root, @"action":@"lookup", @"target_language":@"en", @"generation":@20,
                @"items":@[@{@"text":@"HELLO", @"direction":@"english_to_chinese"}]};
            NSDictionary *found = [MSIMEClientSession learnedTranslationRequest:read error:&failure];
            assert(found && !failure && [found[@"generation"] isEqual:@20]);
            assert(([found[@"translations"] isEqual:@[@{@"text":@"HELLO", @"translation":@"你好"}]]));
            NSMutableDictionary *bad = [read mutableCopy]; bad[@"directory"] = @"relative";
            assert(![MSIMEClientSession learnedTranslationRequest:bad error:&failure] && failure);
            dispatch_semaphore_signal(done);
        });
        assert(dispatch_semaphore_wait(done, dispatch_time(DISPATCH_TIME_NOW, 3 * NSEC_PER_SEC)) == 0);
        assert([NSFileManager.defaultManager removeItemAtPath:root error:&error] && !error);

        // 首次保存仍在排队时关闭窗口只能发布一次完成通知：关闭时的补写负责最终通知，旧保存已经过期。
        NSString *queuedRoot = [NSTemporaryDirectory() stringByAppendingPathComponent:NSUUID.UUID.UUIDString];
        __block NSUInteger queuedSaves = 0;
        MSIMETranslationSettingsWindow *queuedWindow = [[MSIMETranslationSettingsWindow alloc] initWithDirectory:queuedRoot saved:^(NSDictionary *preferences) {
            assert(NSThread.isMainThread && [preferences isKindOfClass:NSDictionary.class]); ++queuedSaves;
        }];
        [queuedWindow showWindow:nil]; Wait(queuedWindow);
        NSPopUpButton *queuedProvider = [queuedWindow valueForKey:@"provider"];
        NSTextField *queuedEndpoint = [queuedWindow valueForKey:@"endpoint"];
        NSSecureTextField *queuedKey = [queuedWindow valueForKey:@"key"];
        [queuedProvider selectItemAtIndex:2]; [queuedWindow providerChanged:nil]; Wait(queuedWindow);
        queuedEndpoint.stringValue = @"https://translation.invalid/api";
        queuedKey.stringValue = @"synthetic-queued-key";
        dispatch_queue_t queuedQueue = [queuedWindow valueForKey:@"queue"];
        dispatch_semaphore_t blockerStarted = dispatch_semaphore_create(0);
        dispatch_semaphore_t releaseBlocker = dispatch_semaphore_create(0);
        dispatch_async(queuedQueue, ^{
            dispatch_semaphore_signal(blockerStarted);
            dispatch_semaphore_wait(releaseBlocker, DISPATCH_TIME_FOREVER);
        });
        assert(dispatch_semaphore_wait(blockerStarted, dispatch_time(DISPATCH_TIME_NOW, 3 * NSEC_PER_SEC)) == 0);
        [queuedWindow controlTextDidEndEditing:[NSNotification notificationWithName:NSControlTextDidEndEditingNotification object:nil]];
        assert([[queuedWindow valueForKey:@"saving"] boolValue]);
        [queuedWindow close];
        dispatch_semaphore_signal(releaseBlocker);
        NSDate *queuedDeadline = [NSDate dateWithTimeIntervalSinceNow:3];
        while (queuedSaves == 0 && queuedDeadline.timeIntervalSinceNow > 0)
            [NSRunLoop.mainRunLoop runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.005]];
        [NSRunLoop.mainRunLoop runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.1]];
        assert(queuedSaves == 1);
        assert([NSFileManager.defaultManager removeItemAtPath:queuedRoot error:&error] && !error);
    }
    return 0;
}
