#import <AppKit/AppKit.h>
#import <WebKit/WebKit.h>
#include <cassert>
#include "../../../../apps/desktop/src-tauri/macos/settings_glass.mm"

// 合成焦点和应用状态，避免测试抢走真实桌面的焦点。
@interface SettingsGlassTestWindow : NSWindow
@property(nonatomic) BOOL simulatedKey;
@end
@implementation SettingsGlassTestWindow
- (BOOL)isKeyWindow { return self.simulatedKey; }
@end

static BOOL ApplicationActive = YES;
static BOOL TestApplicationActive(id application, SEL selector) {
    (void)application;
    (void)selector;
    return ApplicationActive;
}

static void CheckAppearance(WKWebView *webview, NSWindow *window, NSString *mode, NSAppearanceName expected) {
    NSString *script = [NSString stringWithFormat:@"document.documentElement.dataset.settingsAppearance = '%@'", mode];
    [webview evaluateJavaScript:script completionHandler:nil];
    NSDate *deadline = [NSDate dateWithTimeIntervalSinceNow:8];
    while (deadline.timeIntervalSinceNow > 0) {
        [NSRunLoop.currentRunLoop runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.05]];
        if (expected ? [window.appearance.name isEqual:expected] : window.appearance == nil) return;
    }
    assert(false && "设置窗口未同步文档外观偏好");
}

static void TestSettingsGlass() {
    @autoreleasepool {
        [NSApplication sharedApplication];
        SettingsGlassTestWindow *window = [[SettingsGlassTestWindow alloc] initWithContentRect:NSMakeRect(0, 0, 1000, 780)
            styleMask:NSWindowStyleMaskTitled | NSWindowStyleMaskResizable
            backing:NSBackingStoreBuffered defer:NO];
        window.simulatedKey = YES;
        WKWebView *webview = [[WKWebView alloc] initWithFrame:window.contentView.bounds];
        webview.autoresizingMask = NSViewWidthSizable | NSViewHeightSizable;
        [window.contentView addSubview:webview];
        msime_install_settings_glass((__bridge void *)window, (__bridge void *)webview);
        if (@available(macOS 26.0, *)) {
            // 使用真实视图验证透明策略；显式开关避免测试依赖机器当前的辅助功能配置。
            MSIMESettingsGlass *controller = objc_getAssociatedObject(window, &MSIMESettingsGlassControllerKey);
            NSUInteger scripts = webview.configuration.userContentController.userScripts.count;
            msime_install_settings_glass((__bridge void *)window, (__bridge void *)webview);
            assert(webview.configuration.userContentController.userScripts.count == scripts);
            [controller updateEnabled:YES];
            assert(controller.glass.superview == window.contentView);
            assert(window.contentView.subviews.firstObject == controller.glass);
            assert(((NSGlassEffectView *)controller.glass).contentView == webview);
            assert([webview isDescendantOf:controller.glass]);
            assert(!NSIsEmptyRect(webview.frame));
            assert(NSEqualRects(controller.glass.frame, window.contentView.bounds));
            assert(![[webview valueForKey:@"drawsBackground"] boolValue]);
            assert(!window.opaque);
            assert(window.backgroundColor.alphaComponent == 0);
            [window setContentSize:NSMakeSize(360, 540)];
            assert(NSEqualRects(controller.glass.frame, window.contentView.bounds));
            [controller updateEnabled:NO];
            assert(controller.glass == nil);
            assert(window.opaque);
            assert(webview.superview == window.contentView);
            [controller updateEnabled:YES];
            assert(controller.glass.superview == window.contentView);
            assert(((NSGlassEffectView *)controller.glass).contentView == webview);
            assert([webview isDescendantOf:controller.glass]);
            assert(!NSIsEmptyRect(webview.frame));
            // 真正的通知入口要同步移除和恢复材质，并保留同一 WebView。
            for (NSNumber *active in @[@NO, @YES]) {
                ApplicationActive = active.boolValue;
                [NSNotificationCenter.defaultCenter postNotificationName:active.boolValue ? NSApplicationDidBecomeActiveNotification : NSApplicationDidResignActiveNotification object:NSApp];
                BOOL enabled = active.boolValue && !NSWorkspace.sharedWorkspace.accessibilityDisplayShouldReduceTransparency;
                assert((controller.glass != nil) == enabled);
                assert(window.opaque == !enabled);
                assert([webview isDescendantOf:window.contentView]);
            }
            for (NSNumber *key in @[@NO, @YES]) {
                window.simulatedKey = key.boolValue;
                [NSNotificationCenter.defaultCenter postNotificationName:key.boolValue ? NSWindowDidBecomeKeyNotification : NSWindowDidResignKeyNotification object:window];
                BOOL enabled = key.boolValue && !NSWorkspace.sharedWorkspace.accessibilityDisplayShouldReduceTransparency;
                assert((controller.glass != nil) == enabled);
                assert(window.opaque == !enabled);
                assert([webview isDescendantOf:window.contentView]);
            }
            [webview loadHTMLString:@"<!doctype html><html><body>synthetic settings</body></html>" baseURL:nil];
            // 页面重载通过消息读取实时状态；不能只验证原生视图存在却没有透出材质。
            NSDate *deadline = [NSDate dateWithTimeIntervalSinceNow:8];
            __block BOOL matched = NO;
            NSString *expected = NSWorkspace.sharedWorkspace.accessibilityDisplayShouldReduceTransparency ? @"false" : @"true";
            while (!matched && deadline.timeIntervalSinceNow > 0) {
                [webview evaluateJavaScript:@"document.documentElement.dataset.macosLiquidGlass + '/' + document.documentElement.dataset.macosSystemControls"
                    completionHandler:^(id value, NSError *error) {
                        if (!error && [value isEqual:[expected stringByAppendingString:@"/true"]]) matched = YES;
                    }];
                [NSRunLoop.currentRunLoop runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.05]];
            }
            assert(matched);
            // 减少透明度可以移除玻璃，系统控件仍由 WebKit 绘制。
            [controller updateEnabled:NO];
            deadline = [NSDate dateWithTimeIntervalSinceNow:8];
            matched = NO;
            while (!matched && deadline.timeIntervalSinceNow > 0) {
                [webview evaluateJavaScript:@"document.documentElement.dataset.macosLiquidGlass + '/' + document.documentElement.dataset.macosSystemControls + '/' + ('switch' in document.createElement('input'))"
                    completionHandler:^(id value, NSError *error) {
                        if (!error && [value isEqual:@"false/true/true"]) matched = YES;
                    }];
                [NSRunLoop.currentRunLoop runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.05]];
            }
            assert(matched);
            // 明暗必须到达原生材质；系统模式清除覆盖，不能锁在刚才的深色。
            CheckAppearance(webview, window, @"dark", NSAppearanceNameDarkAqua);
            CheckAppearance(webview, window, @"light", NSAppearanceNameAqua);
            CheckAppearance(webview, window, @"dark", NSAppearanceNameDarkAqua);
            CheckAppearance(webview, window, @"system", nil);
            CheckAppearance(webview, window, @"invalid", nil);
            [controller updateEnabled:YES];
            CheckAppearance(webview, window, @"light", NSAppearanceNameAqua);
            CheckAppearance(webview, window, @"dark", NSAppearanceNameDarkAqua);
            // 外观消息会刷新辅助功能策略；减少透明度时玻璃应当已被移除。
            assert((controller.glass == nil) == NSWorkspace.sharedWorkspace.accessibilityDisplayShouldReduceTransparency);
            // 单独验证存在的玻璃继承窗口深色，不能假定消息刷新后仍然启用玻璃。
            [controller updateEnabled:YES];
            assert(controller.glass.superview == window.contentView);
            assert([controller.glass.effectiveAppearance.name isEqual:NSAppearanceNameDarkAqua]);
            [webview loadHTMLString:@"<!doctype html><html><body>synthetic reload</body></html>" baseURL:nil];
            deadline = [NSDate dateWithTimeIntervalSinceNow:8];
            while (window.appearance != nil && deadline.timeIntervalSinceNow > 0) {
                [NSRunLoop.currentRunLoop runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.05]];
            }
            assert(window.appearance == nil);
        }
    }
}

// 只覆盖当前测试进程的系统读取，不修改机器的辅助功能偏好。
static BOOL ReducedTransparency = NO;
static BOOL TestReducedTransparency(id workspace, SEL selector) {
    (void)workspace;
    (void)selector;
    return ReducedTransparency;
}

int main() {
    @autoreleasepool {
        [NSApplication sharedApplication];
        Method activeMethod = class_getInstanceMethod(NSApplication.class, @selector(isActive));
        IMP originalActive = method_setImplementation(activeMethod, (IMP)TestApplicationActive);
        Method method = class_getInstanceMethod(NSWorkspace.class, @selector(accessibilityDisplayShouldReduceTransparency));
        IMP original = method_setImplementation(method, (IMP)TestReducedTransparency);
        for (NSNumber *reduced in @[@NO, @YES]) {
            ReducedTransparency = reduced.boolValue;
            TestSettingsGlass();
        }
        method_setImplementation(method, original);
        method_setImplementation(activeMethod, originalActive);
    }
    return 0;
}
