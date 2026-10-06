#import "AISettingsWindow.h"
#import "MSIMEClientSession.h"
#import "AISettingsSnapshot.h"

static BOOL MSIMEAIStrictRevision(id value, uint64_t *result) {
    if (![value isKindOfClass:NSNumber.class] || CFGetTypeID((__bridge CFTypeRef)value) == CFBooleanGetTypeID() || CFNumberIsFloatType((__bridge CFNumberRef)value)) return NO;
    NSNumber *number = (NSNumber *)value;
    if ([number compare:@0] == NSOrderedAscending) return NO;
    uint64_t revision = number.unsignedLongLongValue;
    if ([number compare:@(revision)] != NSOrderedSame) return NO;
    if (result) *result = revision;
    return YES;
}

static BOOL SafeAIEndpoint(NSString *value) {
    NSURLComponents *url = [NSURLComponents componentsWithString:value ?: @""];
    if (!url.host.length || url.user != nil || url.password != nil || url.fragment != nil) return NO;
    if ([url.scheme.lowercaseString isEqualToString:@"https"]) return YES;
    if (![url.scheme.lowercaseString isEqualToString:@"http"]) return NO;
    // 与共享请求层一致，明文 HTTP 只允许本机回环服务。
    NSString *host = url.host.lowercaseString;
    return [@[@"localhost", @"127.0.0.1", @"[::1]", @"::1"] containsObject:host];
}

/// Saves `edits` (a subset of the AI keys) over `snapshot`; when another writer saved first, they are merged onto its revision and written once more so its other changes survive.
static NSDictionary *SaveAIEdits(NSString *directory, NSDictionary *snapshot, NSDictionary *edits) {
    NSMutableDictionary *next = [snapshot mutableCopy]; next[@"preferences"] = MSIMEAISettingsMerge(snapshot[@"preferences"], edits);
    uint64_t revision = 0;
    if (!MSIMEAIStrictRevision(snapshot[@"revision"], &revision)) return nil;
    NSDictionary *saved = [MSIMEClientSession savePreferencesInDirectory:directory expectedRevision:revision snapshot:next error:nil];
    if (saved) return saved;
    NSDictionary *latest = [MSIMEClientSession loadPreferencesInDirectory:directory error:nil];
    // The same revision means the document itself was refused, which another attempt cannot fix.
    uint64_t latestRevision = 0;
    if (!latest || !MSIMEAIStrictRevision(latest[@"revision"], &latestRevision) || latestRevision == revision) return nil;
    next = [latest mutableCopy]; next[@"preferences"] = MSIMEAISettingsMerge(latest[@"preferences"], edits);
    return [MSIMEClientSession savePreferencesInDirectory:directory expectedRevision:latestRevision snapshot:next error:nil];
}

@interface MSIMEAISettingsWindow () <NSWindowDelegate, NSTextFieldDelegate>
@end

// Saves as each control is set, like the voice settings window: the checkbox and provider at once, a field when it loses focus, and any edit still in progress when the window closes. There is no 保存 or 重新加载 button; the window reads the stored settings each time it opens.
@implementation MSIMEAISettingsWindow {
    NSString *_directory;
    void (^_saved)(NSDictionary *);
    NSDictionary *_snapshot;
    /// The AI keys as last written (or as loaded); a commit writes only the keys that differ from these.
    NSDictionary *_committed;
    NSButton *_enabled;
    NSPopUpButton *_provider;
    NSTextField *_model, *_endpoint, *_limit, *_status;
    NSArray<NSTextField *> *_prompts;
}

- (instancetype)initWithDirectory:(NSString *)directory saved:(void (^)(NSDictionary *))saved {
    if ((self = [super initWithWindow:nil])) { _directory = [directory copy]; _saved = [saved copy]; }
    return self;
}

- (void)loadWindow {
    NSWindow *window = [[NSWindow alloc] initWithContentRect:NSMakeRect(0, 0, 600, 620) styleMask:NSWindowStyleMaskTitled | NSWindowStyleMaskClosable backing:NSBackingStoreBuffered defer:NO];
    window.title = @"AI 联想设置"; window.delegate = self; self.window = window;
    _enabled = [NSButton checkboxWithTitle:@"启用 AI 联想" target:self action:@selector(commit:)];
    _provider = [[NSPopUpButton alloc] initWithFrame:NSZeroRect]; [_provider addItemsWithTitles:@[@"DeepSeek", @"OpenAI", @"SiliconFlow", @"Groq"]]; _provider.target = self; _provider.action = @selector(commit:);
    _model = [NSTextField textFieldWithString:@""]; _endpoint = [NSTextField textFieldWithString:@""]; _limit = [NSTextField textFieldWithString:@"3"];
    NSMutableArray *prompts = [NSMutableArray array]; for (NSUInteger i = 0; i < 3; ++i) [prompts addObject:[NSTextField textFieldWithString:@""]]; _prompts = prompts;
    NSGridView *grid = [NSGridView gridViewWithViews:@[@[[NSTextField labelWithString:@"状态"], _enabled], @[[NSTextField labelWithString:@"提供商"], _provider], @[[NSTextField labelWithString:@"模型"], _model], @[[NSTextField labelWithString:@"接口地址"], _endpoint], @[[NSTextField labelWithString:@"候选数量"], _limit], @[[NSTextField labelWithString:@"自定义提示词 1"], _prompts[0]], @[[NSTextField labelWithString:@"自定义提示词 2"], _prompts[1]], @[[NSTextField labelWithString:@"自定义提示词 3"], _prompts[2]]]];
    grid.rowSpacing = 12; for (NSView *view in @[_model, _endpoint, _limit, _prompts[0], _prompts[1], _prompts[2]]) { view.translatesAutoresizingMaskIntoConstraints = NO; [view.widthAnchor constraintEqualToConstant:390].active = YES; ((NSTextField *)view).delegate = self; }
    _status = [NSTextField wrappingLabelWithString:@""];
    NSStackView *stack = [NSStackView stackViewWithViews:@[grid, _status]]; stack.orientation = NSUserInterfaceLayoutOrientationVertical; stack.spacing = 16; stack.translatesAutoresizingMaskIntoConstraints = NO; [window.contentView addSubview:stack]; [NSLayoutConstraint activateConstraints:@[[stack.leadingAnchor constraintEqualToAnchor:window.contentView.leadingAnchor constant:20], [stack.trailingAnchor constraintEqualToAnchor:window.contentView.trailingAnchor constant:-20], [stack.topAnchor constraintEqualToAnchor:window.contentView.topAnchor constant:20]]]; [window center];
}
- (void)showWindow:(id)sender { if (!self.window) [self loadWindow]; [super showWindow:sender]; [self reload:nil]; }
- (void)reload:(id)sender { (void)sender; if (!_directory.isAbsolutePath) { _status.stringValue = @"请先激活输入法。"; return; } _snapshot = [MSIMEClientSession loadPreferencesInDirectory:_directory error:nil]; NSDictionary *ai = _snapshot[@"preferences"][@"ai_assistant"]; _enabled.state = [ai[@"enabled"] boolValue] ? NSControlStateValueOn : NSControlStateValueOff; NSArray *providers = @[@"deepseek", @"openai", @"siliconflow", @"groq"]; NSUInteger index = [providers indexOfObject:ai[@"provider"]]; [_provider selectItemAtIndex:index == NSNotFound ? 0 : index]; _model.stringValue = ai[@"model"] ?: @""; _endpoint.stringValue = ai[@"endpoint"] ?: @""; _limit.stringValue = [ai[@"candidate_limit"] stringValue] ?: @"3"; for (NSUInteger i = 0; i < 3; ++i) _prompts[i].stringValue = ai[[NSString stringWithFormat:@"prompt_custom_%lu", (unsigned long)i + 1]] ?: @""; _committed = _snapshot ? [self form] : nil; _status.stringValue = _snapshot ? @"修改会自动保存。" : @"加载失败。"; }
/// The AI keys as the controls now read.
- (NSDictionary *)form { NSArray *providers = @[@"deepseek", @"openai", @"siliconflow", @"groq"]; return @{ @"enabled": @(_enabled.state == NSControlStateValueOn), @"provider": providers[_provider.indexOfSelectedItem], @"model": _model.stringValue, @"endpoint": _endpoint.stringValue, @"candidate_limit": @(_limit.integerValue), @"prompt_custom_1": _prompts[0].stringValue, @"prompt_custom_2": _prompts[1].stringValue, @"prompt_custom_3": _prompts[2].stringValue }; }
/// Writes the keys that changed since the last save; an invalid value is reported and nothing is written until it is corrected.
- (void)commit:(id)sender {
    (void)sender; if (!_snapshot) return;
    NSDictionary *form = [self form];
    NSMutableDictionary *edits = [NSMutableDictionary dictionary]; for (NSString *key in form) if (![form[key] isEqual:_committed[key]]) edits[key] = form[key];
    if (!edits.count) return;
    NSInteger limit = [form[@"candidate_limit"] integerValue]; if (limit < 1 || limit > 10) { _status.stringValue = @"候选数量必须为 1～10；修改尚未保存。"; return; }
    if ([form[@"enabled"] boolValue] && !SafeAIEndpoint(form[@"endpoint"])) { _status.stringValue = @"启用 AI 时请输入有效的 HTTPS 接口地址（本机回环地址可用 HTTP）；修改尚未保存。"; return; }
    NSDictionary *saved = SaveAIEdits(_directory, _snapshot, edits);
    if (saved) { _snapshot = saved; _committed = form; _status.stringValue = @"已保存。"; if (_saved) _saved(saved[@"preferences"]); }
    else _status.stringValue = @"保存失败，修改尚未写入；再次修改或关闭窗口时会重试。";
}
- (void)controlTextDidEndEditing:(NSNotification *)notification { (void)notification; [self commit:nil]; }
// Ending the edit in progress commits it through controlTextDidEndEditing; the explicit commit covers a field edited without ever taking focus.
- (void)windowWillClose:(NSNotification *)notification { (void)notification; [self.window makeFirstResponder:nil]; [self commit:nil]; }
@end
