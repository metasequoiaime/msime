#import "SupportWindowController.h"
#import "WindowPresentation.h"

namespace {

NSString *const kIssuesURL = @"https://github.com/metasequoiaime/msime/issues";
NSString *const kTelegramURL = @"https://t.me/msimegroup";
NSString *const kWebsiteURL = @"https://msime.app/";
NSString *const kLicenseURL = @"https://github.com/metasequoiaime/msime/blob/develop/LICENSE";
NSString *const kPrivacyURL = @"https://msime.app/privacy/";
NSString *const kQQGroup = @"829919142";

NSTextField *Heading(NSString *text) {
    NSTextField *label = [NSTextField labelWithString:text];
    label.font = [NSFont systemFontOfSize:24.0 weight:NSFontWeightSemibold];
    return label;
}

NSTextField *Title(NSString *text) {
    NSTextField *label = [NSTextField labelWithString:text];
    label.font = [NSFont systemFontOfSize:16.0 weight:NSFontWeightSemibold];
    return label;
}

NSTextField *Body(NSString *text) {
    NSTextField *label = [NSTextField wrappingLabelWithString:text];
    label.font = [NSFont systemFontOfSize:13.0];
    label.textColor = NSColor.secondaryLabelColor;
    label.maximumNumberOfLines = 0;
    label.preferredMaxLayoutWidth = 540.0;
    return label;
}

NSButton *ActionButton(NSString *title, id target, SEL action, NSString *identifier) {
    NSButton *button = [NSButton buttonWithTitle:title target:target action:action];
    button.bezelStyle = NSBezelStyleRounded;
    button.accessibilityIdentifier = identifier;
    return button;
}

NSStackView *Stack(NSArray<NSView *> *views, CGFloat spacing) {
    NSStackView *stack = [NSStackView stackViewWithViews:views];
    stack.orientation = NSUserInterfaceLayoutOrientationVertical;
    stack.alignment = NSLayoutAttributeLeading;
    stack.spacing = spacing;
    stack.translatesAutoresizingMaskIntoConstraints = NO;
    return stack;
}

void OpenURL(NSString *url) {
    [[NSWorkspace sharedWorkspace] openURL:[NSURL URLWithString:url]];
}

void InvokeUpdateController(void) {
    // MSIMEUpdateController is a source-level alias; the Objective-C runtime name is the
    // Metasequoia-prefixed class.
    Class type = NSClassFromString(@"MetasequoiaUpdateController");
    if (type == Nil || ![type respondsToSelector:@selector(sharedController)]) return;
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Warc-performSelector-leaks"
    id controller = [type performSelector:@selector(sharedController)];
    if ([controller respondsToSelector:@selector(checkForUpdates:)]) [controller performSelector:@selector(checkForUpdates:) withObject:nil];
#pragma clang diagnostic pop
}

void OpenPreferences(void) {
    Class type = NSClassFromString(@"MSIMEPreferencesWindowController");
    if (type == Nil || ![type respondsToSelector:@selector(sharedController)]) return;
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Warc-performSelector-leaks"
    id controller = [type performSelector:@selector(sharedController)];
    // This button sits under the help page's line about candidate settings, input schemes and shortcuts being adjustable in the settings window, so the page it has to open is 输入. It used to call the argument-less -showAndActivate, which landed on whatever page the window happened to be showing.
    if ([controller respondsToSelector:@selector(showAndActivateWithPageIdentifier:)])
        [controller performSelector:@selector(showAndActivateWithPageIdentifier:) withObject:@"input"];
#pragma clang diagnostic pop
}

} // namespace

@interface MSIMESupportWindowController ()
@property(nonatomic, readwrite) MSIMESupportPage page;
@end

@implementation MSIMESupportWindowController

+ (instancetype)sharedController {
    static MSIMESupportWindowController *controller;
    static dispatch_once_t once;
    dispatch_once(&once, ^{ controller = [[self alloc] initWithWindow:nil]; });
    return controller;
}

- (instancetype)initWithWindow:(NSWindow *)window {
    NSWindow *supportWindow = window;
    if (supportWindow == nil) {
        supportWindow = [[NSWindow alloc] initWithContentRect:NSMakeRect(0, 0, 620, 500)
                                                     styleMask:NSWindowStyleMaskTitled | NSWindowStyleMaskClosable |
                                                               NSWindowStyleMaskMiniaturizable | NSWindowStyleMaskResizable
                                                       backing:NSBackingStoreBuffered
                                                         defer:NO];
        supportWindow.releasedWhenClosed = NO;
        supportWindow.minSize = NSMakeSize(500, 380);
    }
    self = [super initWithWindow:supportWindow];
    if (self != nil) {
        _page = MSIMESupportPageHelp;
    }
    return self;
}

- (void)showPage:(MSIMESupportPage)page {
    self.page = page;
    NSArray<NSView *> *views;
    NSString *title;
    switch (page) {
        case MSIMESupportPageAbout: {
            title = @"关于水杉输入法";
            NSString *version = NSBundle.mainBundle.infoDictionary[@"CFBundleShortVersionString"] ?: @"0.51.4";
            views = @[
                Heading(@"水杉 IME"),
                Body(@"为现代 macOS 桌面体验打造的开放中文输入法。"),
                Title(@"当前版本"),
                Body([NSString stringWithFormat:@"v%@", version]),
                ActionButton(@"检查更新…", self, @selector(checkForUpdates:), @"MSIMESupportCheckForUpdates"),
                ActionButton(@"访问官方网站", self, @selector(openWebsite:), @"MSIMESupportWebsite"),
                ActionButton(@"开源许可协议", self, @selector(openLicense:), @"MSIMESupportLicense"),
                ActionButton(@"隐私政策", self, @selector(openPrivacy:), @"MSIMESupportPrivacy"),
            ];
            break;
        }
        case MSIMESupportPageFeedback:
            title = @"反馈与交流";
            views = @[
                Heading(@"告诉我们你的想法"),
                Body(@"遇到问题或有功能建议时，可以通过以下渠道提交和交流。"),
                Title(@"GitHub Issues"),
                Body(@"适合提交可复现的问题、功能建议和开发讨论。"),
                ActionButton(@"查看 Issues", self, @selector(openIssues:), @"MSIMESupportIssues"),
                Title(@"QQ 交流群"),
                Body(@"适合中文用户进行日常交流、测试反馈和使用讨论。群号：829919142"),
                ActionButton(@"复制群号", self, @selector(copyQQGroup:), @"MSIMESupportQQ"),
                Title(@"Telegram 群组"),
                Body(@"面向国际用户和开发者的即时讨论频道。"),
                ActionButton(@"打开群组", self, @selector(openTelegram:), @"MSIMESupportTelegram"),
            ];
            break;
        case MSIMESupportPageHelp:
        default:
            title = @"水杉输入法帮助";
            views = @[
                Heading(@"帮助"),
                Body(@"水杉输入法是一款 macOS 平台的中文输入法。请先在系统设置的键盘输入法中启用水杉输入法，再使用系统配置的输入法切换快捷键。"),
                Title(@"快速上手"),
                Body(@"默认使用全拼输入法。输入拼音后按数字键选择候选词；候选设置、输入方案和快捷键可以在设置窗口中调整。"),
                Title(@"基本功能"),
                Body(@"支持全拼、双拼和五笔，以及辅助码、候选窗口、手写识别板、屏幕键盘和语音输入。更多功能可以从输入菜单或悬浮工具栏打开。"),
                ActionButton(@"打开设置…", self, @selector(openPreferences:), @"MSIMESupportPreferences"),
            ];
            break;
    }

    NSStackView *stack = Stack(views, 12.0);
    NSView *content = [[NSView alloc] initWithFrame:NSMakeRect(0, 0, 620, 500)];
    [content addSubview:stack];
    [NSLayoutConstraint activateConstraints:@[
        [stack.leadingAnchor constraintEqualToAnchor:content.leadingAnchor constant:28.0],
        [stack.trailingAnchor constraintEqualToAnchor:content.trailingAnchor constant:-28.0],
        [stack.topAnchor constraintEqualToAnchor:content.topAnchor constant:28.0],
        [stack.bottomAnchor constraintLessThanOrEqualToAnchor:content.bottomAnchor constant:-28.0],
    ]];
    self.window.title = title;
    self.window.contentView = content;
    [self.window center];
    [self showWindow:nil];
    MSIMEPresentWindow(self.window);
}

- (void)checkForUpdates:(id)sender { (void)sender; InvokeUpdateController(); }
- (void)openWebsite:(id)sender { (void)sender; OpenURL(kWebsiteURL); }
- (void)openLicense:(id)sender { (void)sender; OpenURL(kLicenseURL); }
- (void)openPrivacy:(id)sender { (void)sender; OpenURL(kPrivacyURL); }
- (void)openIssues:(id)sender { (void)sender; OpenURL(kIssuesURL); }
- (void)openTelegram:(id)sender { (void)sender; OpenURL(kTelegramURL); }
- (void)openPreferences:(id)sender { (void)sender; OpenPreferences(); }
- (void)copyQQGroup:(id)sender {
    (void)sender;
    NSPasteboard *pasteboard = NSPasteboard.generalPasteboard;
    [pasteboard clearContents];
    [pasteboard setString:kQQGroup forType:NSPasteboardTypeString];
}

@end
