#import "../../src/translation/LocalTranslationWindow.h"
#include "msime_client.h"
#include <atomic>
#include <cassert>
#include <cstdlib>
#include <cstring>

@interface MSIMELocalTranslationWindow (Testing)
- (void)refresh;
- (void)save:(id)sender;
@end

static std::atomic<bool> installed{false}, enabled{false}, conflict{false};
static NSString *language = @"en";
static char *reply(id value) {
    NSData *data = [NSJSONSerialization dataWithJSONObject:@{@"ok":@YES, @"value":value} options:0 error:nil];
    return strndup(static_cast<const char *>(data.bytes), data.length);
}
extern "C" char *msime_client_local_translation_catalog() {
    assert(!NSThread.isMainThread);
    return reply(@{@"available":@YES, @"model":@"Hy-MT2 1.8B Q4_K_M", @"size":@1133080448,
        @"languages":@[@{@"code":@"en", @"name":@"英语"}, @{@"code":@"ja", @"name":@"日语"}]});
}
extern "C" char *msime_client_local_translation_settings(const uint8_t *request, size_t length) {
    assert(!NSThread.isMainThread);
    NSDictionary *input = [NSJSONSerialization JSONObjectWithData:[NSData dataWithBytes:request length:length] options:0 error:nil];
    if (input[@"update"]) {
        if (conflict) return strdup(R"({"ok":false,"error":"preferences changed"})");
        enabled = [input[@"update"][@"enabled"] boolValue];
        language = [input[@"update"][@"target_language"] copy];
    }
    return reply(@{@"revision":@1, @"local_translation":@{@"enabled":@(enabled.load()), @"target_language":language}});
}
extern "C" char *msime_client_resource_packs(const uint8_t *, size_t) {
    assert(!NSThread.isMainThread);
    return reply(@[@{@"id":@"local-translation", @"state":installed ? @"installed" : @"missing"}]);
}
extern "C" char *msime_client_resource_pack_install(const uint8_t *, size_t, msime_client_resource_pack_progress_callback, void *) { std::abort(); }
extern "C" char *msime_client_resource_pack_remove(const uint8_t *, size_t) { std::abort(); }
extern "C" void msime_client_resource_pack_cancel(const uint8_t *, size_t) {}
extern "C" void msime_client_local_translation_cancel(const uint8_t *, size_t) {}
extern "C" void msime_client_string_free(char *value) { std::free(value); }

static void settle(MSIMELocalTranslationWindow *window) {
    NSDate *deadline = [NSDate dateWithTimeIntervalSinceNow:5];
    while ([[window valueForKey:@"busy"] boolValue] && deadline.timeIntervalSinceNow > 0)
        [NSRunLoop.currentRunLoop runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.01]];
    assert(![[window valueForKey:@"busy"] boolValue]);
    [window.window.contentView layoutSubtreeIfNeeded];
}
static void capture(MSIMELocalTranslationWindow *window, NSString *name) {
    const char *directory = std::getenv("MSIME_TRANSLATION_WINDOW_CAPTURE");
    if (!directory) return;
    [window.window displayIfNeeded];
    NSView *view = window.window.contentView;
    NSBitmapImageRep *bitmap = [view bitmapImageRepForCachingDisplayInRect:view.bounds];
    [view cacheDisplayInRect:view.bounds toBitmapImageRep:bitmap];
    [NSGraphicsContext saveGraphicsState];
    NSGraphicsContext.currentContext = [NSGraphicsContext graphicsContextWithBitmapImageRep:bitmap];
    [window.window.backgroundColor setFill];
    NSRectFillUsingOperation(view.bounds, NSCompositingOperationDestinationOver);
    [NSGraphicsContext restoreGraphicsState];
    assert([[bitmap representationUsingType:NSBitmapImageFileTypePNG properties:@{}]
        writeToFile:[@(directory) stringByAppendingPathComponent:name] atomically:YES]);
}
int main() {
    @autoreleasepool {
        [NSApplication sharedApplication];
        // 只绘制真实控件，不读写个人设置，也不要求桌面解锁。
        auto *window = [[MSIMELocalTranslationWindow alloc] initWithWindow:nil];
        [window setValue:@"/synthetic/translation" forKey:@"directory"];
        [window loadWindow]; [window.window orderFront:nil]; [window refresh]; settle(window);
        NSButton *toggle = [window valueForKey:@"enabled"];
        NSPopUpButton *choices = [window valueForKey:@"language"];
        NSButton *remove = [window valueForKey:@"remove"];
        assert(!toggle.enabled && choices.enabled && !remove.enabled);
        capture(window, @"macos-uninstalled.png");
        installed = true; [window refresh]; settle(window);
        assert(toggle.enabled && remove.enabled);
        [choices selectItemAtIndex:1]; toggle.state = NSControlStateValueOn;
        [window save:toggle];
        assert(!toggle.enabled && !choices.enabled);
        settle(window);
        assert(enabled && [language isEqual:@"ja"] && toggle.state == NSControlStateValueOn);
        capture(window, @"macos-installed.png");
        // 并发保存失败后恢复共享快照，不能显示未提交的选择。
        conflict = true; [choices selectItemAtIndex:0]; [window save:choices]; settle(window);
        assert([choices.selectedItem.representedObject isEqual:@"ja"]);
        NSTextField *status = [window valueForKey:@"status"];
        assert([status.stringValue containsString:@"其他窗口"]);
        [window close];
    }
}
