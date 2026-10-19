#import <AppKit/AppKit.h>
#import <WebKit/WebKit.h>
#import <objc/runtime.h>

static char MSIMESettingsGlassControllerKey;

@interface MSIMESettingsGlass : NSObject <WKScriptMessageHandler>
@property(nonatomic, weak) NSWindow *window;
@property(nonatomic, weak) WKWebView *webview;
@property(nonatomic, strong) NSView *glass;
@property(nonatomic, weak) NSView *host;
@end

@implementation MSIMESettingsGlass
- (void)refresh {
    [self updateEnabled:!NSWorkspace.sharedWorkspace.accessibilityDisplayShouldReduceTransparency];
}
- (void)updateEnabled:(BOOL)enabled {
    BOOL systemControls = NO;
    if (@available(macOS 26.0, *)) {
        systemControls = YES;
        if (enabled && !self.glass) {
            NSView *content = self.host;
            NSGlassEffectView *glass = [[NSGlassEffectView alloc] initWithFrame:content.bounds];
            glass.style = NSGlassEffectViewStyleRegular;
            glass.cornerRadius = 20;
            glass.autoresizingMask = NSViewWidthSizable | NSViewHeightSizable;
            glass.contentView = self.webview;
            [content addSubview:glass];
            self.glass = glass;
        }
    } else {
        enabled = NO;
    }
    // 不保留窗口本身的实色垫底，否则玻璃只能采样到固定底色。
    self.window.opaque = !enabled;
    self.window.backgroundColor = enabled ? NSColor.clearColor : NSColor.windowBackgroundColor;
    if (!enabled) {
        if (@available(macOS 26.0, *)) {
            ((NSGlassEffectView *)self.glass).contentView = nil;
        }
        [self.glass removeFromSuperview];
        self.glass = nil;
        if (self.webview.superview != self.host) [self.host addSubview:self.webview];
        self.webview.translatesAutoresizingMaskIntoConstraints = YES;
        self.webview.autoresizingMask = NSViewWidthSizable | NSViewHeightSizable;
        self.webview.frame = self.host.bounds;
    }
    NSString *script = [NSString stringWithFormat:
        @"if (document.documentElement) { document.documentElement.dataset.macosLiquidGlass = '%@'; document.documentElement.dataset.macosSystemControls = '%@'; }",
        enabled ? @"true" : @"false", systemControls ? @"true" : @"false"];
    [self.webview evaluateJavaScript:script completionHandler:nil];
}
- (void)userContentController:(WKUserContentController *)controller didReceiveScriptMessage:(WKScriptMessage *)message {
    if (!message.frameInfo.mainFrame) return;
    if ([message.body isKindOfClass:NSDictionary.class]) {
        id appearance = message.body[@"appearance"];
        if ([appearance isEqual:@"system"]) self.window.appearance = nil;
        else if ([appearance isEqual:@"light"]) self.window.appearance = [NSAppearance appearanceNamed:NSAppearanceNameAqua];
        else if ([appearance isEqual:@"dark"]) self.window.appearance = [NSAppearance appearanceNamed:NSAppearanceNameDarkAqua];
    }
    [self refresh];
}
- (void)dealloc {
    [NSWorkspace.sharedWorkspace.notificationCenter removeObserver:self];
}
@end

extern "C" void msime_install_settings_glass(void *windowPointer, void *webviewPointer) {
    NSCAssert(NSThread.isMainThread, @"设置窗口材质必须在主线程安装");
    if (@available(macOS 26.0, *)) {
        NSWindow *window = (__bridge NSWindow *)windowPointer;
        WKWebView *webview = (__bridge WKWebView *)webviewPointer;
        MSIMESettingsGlass *existing = objc_getAssociatedObject(window, &MSIMESettingsGlassControllerKey);
        if (existing) {
            [existing refresh];
            return;
        }
        MSIMESettingsGlass *controller = [MSIMESettingsGlass new];
        controller.window = window;
        controller.webview = webview;
        controller.host = webview.superview;
        objc_setAssociatedObject(window, &MSIMESettingsGlassControllerKey, controller, OBJC_ASSOCIATION_RETAIN_NONATOMIC);
        // 与 Tauri 的透明 WebView 策略一致；不改变其他窗口的配置。
        [webview setValue:@NO forKey:@"drawsBackground"];
        webview.underPageBackgroundColor = NSColor.clearColor;
        // 页面重载读取实时能力；同步原始外观偏好，让玻璃与文档同色且系统模式不锁死。
        [webview.configuration.userContentController addScriptMessageHandler:controller name:@"msimeSettingsGlass"];
        [webview.configuration.userContentController addUserScript:
            [[WKUserScript alloc] initWithSource:
                @"(() => { const sync = () => window.webkit.messageHandlers.msimeSettingsGlass.postMessage({appearance: document.documentElement?.dataset.settingsAppearance ?? 'system'}); new MutationObserver(sync).observe(document, {subtree: true, attributes: true, attributeFilter: ['data-settings-appearance']}); sync(); })()"
                injectionTime:WKUserScriptInjectionTimeAtDocumentStart forMainFrameOnly:YES]];
        [NSWorkspace.sharedWorkspace.notificationCenter addObserver:controller selector:@selector(refresh)
            name:NSWorkspaceAccessibilityDisplayOptionsDidChangeNotification object:nil];
        [controller refresh];
    }
}
