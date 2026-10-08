#import "../../src/core/AISettingsWindow.h"
#import "MSIMEClientSession.h"
#import "../../src/core/AIEndpointPolicy.h"
#include <cassert>

@interface MSIMEAISettingsWindow (TestActions)
- (void)commit:(id)sender;
- (void)controlTextDidEndEditing:(NSNotification *)notification;
@end

int main() {
    @autoreleasepool {
        [NSApplication sharedApplication];
        NSString *root = [NSTemporaryDirectory() stringByAppendingPathComponent:NSUUID.UUID.UUIDString];
        NSError *error = nil;
        NSDictionary *initial = [MSIMEClientSession loadPreferencesInDirectory:root error:&error];
        assert(initial && !error);
        __block NSUInteger saves = 0;
        MSIMEAISettingsWindow *window = [[MSIMEAISettingsWindow alloc] initWithDirectory:root saved:^(NSDictionary *preferences) {
            assert([preferences[@"ai_assistant"] isKindOfClass:NSDictionary.class]); ++saves;
        }];
        [window showWindow:nil];
        NSButton *enabled = [window valueForKey:@"enabled"];
        NSPopUpButton *provider = [window valueForKey:@"provider"];
        NSTextField *model = [window valueForKey:@"model"], *endpoint = [window valueForKey:@"endpoint"], *limit = [window valueForKey:@"limit"];
        NSNotification *endEditing = [NSNotification notificationWithName:NSControlTextDidEndEditingNotification object:model];
        // There is nothing to press: no 保存 or 重新加载 button.
        NSMutableArray<NSView *> *views = [NSMutableArray arrayWithObject:window.window.contentView];
        for (NSUInteger i = 0; i < views.count; ++i) {
            assert(![views[i] isKindOfClass:NSButton.class] || views[i] == enabled || views[i] == provider);
            [views addObjectsFromArray:views[i].subviews];
        }
        // Controls that reflect what is stored write nothing.
        [window controlTextDidEndEditing:endEditing]; assert(saves == 0);
        // A field is written when it loses focus.
        model.stringValue = @"synthetic-model"; [window controlTextDidEndEditing:endEditing]; assert(saves == 1);
        NSDictionary *stored = [MSIMEClientSession loadPreferencesInDirectory:root error:&error];
        assert([stored[@"preferences"][@"ai_assistant"][@"model"] isEqual:@"synthetic-model"]);
        // Invalid values are reported and not written.
        limit.stringValue = @"11"; [window controlTextDidEndEditing:endEditing]; assert(saves == 1);
        limit.stringValue = @"5";
        enabled.state = NSControlStateValueOn; endpoint.stringValue = @"file:///synthetic"; [window commit:enabled]; assert(saves == 1);
        // 远程 HTTP 与伪装成 localhost 的远程域名不能保存。
        for (NSString *invalid in @[@"http://remote.invalid/v1", @"http://localhost.example/v1", @"http://192.0.2.1/v1", @"http://[2001:db8::1]/v1", @"http://user:synthetic@localhost/v1", @"http://@:localhost/v1", @"http://localhost/v1#fragment"]) {
            endpoint.stringValue = invalid; [window commit:enabled]; assert(saves == 1);
        }
        // 公网 http 被拒绝时说明原因，而不是只说地址无效。
        endpoint.stringValue = @"http://remote.invalid/v1"; [window commit:enabled];
        assert([[(NSTextField *)[window valueForKey:@"status"] stringValue] hasPrefix:MSIMEAIEndpointCleartextMessage]);
        // Another writer saved in between: the edit is merged onto its revision and keeps its change.
        stored = [MSIMEClientSession loadPreferencesInDirectory:root error:&error];
        NSMutableDictionary *other = [stored mutableCopy], *otherPreferences = [stored[@"preferences"] mutableCopy];
        otherPreferences[@"candidate_page_size"] = @7; other[@"preferences"] = otherPreferences;
        assert([MSIMEClientSession savePreferencesInDirectory:root expectedRevision:[stored[@"revision"] unsignedLongLongValue] snapshot:other error:&error]);
        endpoint.stringValue = @"https://synthetic.invalid/v1"; [window controlTextDidEndEditing:endEditing]; assert(saves == 2);
        stored = [MSIMEClientSession loadPreferencesInDirectory:root error:&error];
        NSDictionary *ai = stored[@"preferences"][@"ai_assistant"];
        assert([stored[@"preferences"][@"candidate_page_size"] isEqual:@7]);
        assert([ai[@"enabled"] isEqual:@YES] && [ai[@"endpoint"] isEqual:@"https://synthetic.invalid/v1"] && [ai[@"candidate_limit"] isEqual:@5]);
        // The provider popup saves as soon as it changes.
        [provider selectItemAtIndex:2]; [window commit:provider]; assert(saves == 3);
        stored = [MSIMEClientSession loadPreferencesInDirectory:root error:&error];
        assert([stored[@"preferences"][@"ai_assistant"][@"provider"] isEqual:@"siliconflow"]);
        // Closing writes an edit that never lost focus instead of discarding it.
        model.stringValue = @"closing-model"; [window close]; assert(saves == 4);
        stored = [MSIMEClientSession loadPreferencesInDirectory:root error:&error];
        assert([stored[@"preferences"][@"ai_assistant"][@"model"] isEqual:@"closing-model"]);
        [window showWindow:nil]; assert([model.stringValue isEqual:@"closing-model"]);
        [window close]; assert(saves == 4);
        // 本机服务仍可使用 HTTP；写入后重读，确认并未只更新窗口。
        [window showWindow:nil];
        NSUInteger expectedSaves = saves;
        for (NSString *local in @[@"http://localhost:8080/v1", @"http://127.0.0.1:8080/v1", @"http://[::1]:8080/v1", @"http://192.168.1.20:1234/v1", @"http://studio.local:1234/v1"]) {
            endpoint.stringValue = local; [window controlTextDidEndEditing:endEditing]; assert(saves == ++expectedSaves);
            stored = [MSIMEClientSession loadPreferencesInDirectory:root error:&error];
            assert([stored[@"preferences"][@"ai_assistant"][@"endpoint"] isEqual:local]);
        }
        [window close]; assert(saves == expectedSaves);
        assert([NSFileManager.defaultManager removeItemAtPath:root error:&error] && !error);
    }
    return 0;
}
