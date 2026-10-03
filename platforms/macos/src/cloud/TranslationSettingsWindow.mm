#import "TranslationSettingsWindow.h"
#import "MSIMEClientSession.h"

static NSArray *TranslationLanguages() { return @[@"en", @"fr", @"ja", @"es", @"ru", @"de", @"ko"]; }

/// Mirrors `usable_credential` in crates/client-core/src/translation.rs.
static BOOL TencentCredentialUsable(id value) {
    if (![value isKindOfClass:NSString.class]) return NO;
    NSString *trimmed = [value stringByTrimmingCharactersInSet:[NSCharacterSet characterSetWithCharactersInString:@" \t\r\n"]];
    return trimmed.length && !([trimmed hasPrefix:@"<"] && [trimmed hasSuffix:@">"]) && ![trimmed hasPrefix:@"FAKESECRET_"];
}

/// Applies `edits` (owned preference keys; NSNull removes one) on top of `preferences`.
static NSDictionary *TranslationPreferencesApplying(NSDictionary *preferences, NSDictionary *edits) {
    NSMutableDictionary *result = [preferences mutableCopy] ?: [NSMutableDictionary dictionary];
    for (NSString *key in edits) {
        if (edits[key] == NSNull.null) [result removeObjectForKey:key];
        else result[key] = edits[key];
    }
    return result;
}

/// Writes `edits` over `snapshot`. When another writer saved first, the edits are moved onto its revision and written once more, so only the keys changed in this window replace what the other writer stored. Blocks on disk: call off the main thread.
static NSDictionary *SaveTranslationEdits(NSString *directory, NSDictionary *snapshot, NSDictionary *edits) {
    NSMutableDictionary *next = [snapshot mutableCopy];
    next[@"preferences"] = TranslationPreferencesApplying(snapshot[@"preferences"], edits);
    uint64_t revision = [snapshot[@"revision"] unsignedLongLongValue];
    NSDictionary *saved = [MSIMEClientSession savePreferencesInDirectory:directory expectedRevision:revision snapshot:next error:nil];
    if (saved) return saved;
    NSDictionary *latest = [MSIMEClientSession loadPreferencesInDirectory:directory error:nil];
    // The same revision means the document itself was refused (a malformed value), which another attempt cannot fix.
    if (!latest || [latest[@"revision"] unsignedLongLongValue] == revision) return nil;
    next = [latest mutableCopy];
    next[@"preferences"] = TranslationPreferencesApplying(latest[@"preferences"], edits);
    return [MSIMEClientSession savePreferencesInDirectory:directory expectedRevision:[latest[@"revision"] unsignedLongLongValue] snapshot:next error:nil];
}

@interface MSIMETranslationSettingsWindow () <NSTextFieldDelegate>
@end

// Every control saves as soon as it is set, the way the voice settings window does: a checkbox or popup the moment it changes, a field when it loses focus, and whatever is still being edited when the window closes. There is no 保存 or 重新加载 button; the window reads the stored settings each time it opens.
@implementation MSIMETranslationSettingsWindow {
    NSString *_directory;
    NSDictionary *_snapshot;
    /// The owned keys as last written (or as loaded); a commit writes only the keys that differ from these.
    NSDictionary *_committed;
    void (^_saved)(NSDictionary *);
    dispatch_queue_t _queue;
    NSButton *_enabled, *_offline, *_reveal;
    NSPopUpButton *_target, *_secondary, *_provider;
    NSGridView *_grid;
    NSTextField *_endpoint, *_plainKey, *_status;
    NSSecureTextField *_key;
    NSButton *_tencent, *_revealTencent, *_revealNiuTrans;
    NSTextField *_secretId, *_plainTencentKey, *_region;
    NSSecureTextField *_tencentKey, *_niuTransKey;
    NSTextField *_appId, *_plainNiuTransKey;
    BOOL _busy, _saving, _pending, _holdCommits;
    NSUInteger _epoch;
    NSUInteger _callbackGeneration;
}
- (instancetype)initWithDirectory:(NSString *)directory saved:(void (^)(NSDictionary *))saved {
    if ((self = [super initWithWindow:nil])) {
        _directory = [directory copy]; _saved = [saved copy];
        // Loads and saves run in order on one queue, so a save flushed on close lands before the next open reads the file.
        _queue = dispatch_queue_create("app.msime.translation-settings", DISPATCH_QUEUE_SERIAL);
    }
    return self;
}
- (void)loadWindow {
    NSWindow *window = [[NSWindow alloc] initWithContentRect:NSMakeRect(0, 0, 570, 840)
        styleMask:NSWindowStyleMaskTitled | NSWindowStyleMaskClosable backing:NSBackingStoreBuffered defer:NO];
    window.title = @"候选翻译设置"; window.delegate = self; self.window = window;
    _enabled = [NSButton checkboxWithTitle:@"显示候选释义" target:self action:@selector(controlChanged:)];
    _offline = [NSButton checkboxWithTitle:@"显示离线英文释义" target:self action:@selector(controlChanged:)];
    _target = [[NSPopUpButton alloc] initWithFrame:NSZeroRect pullsDown:NO];
    [_target addItemsWithTitles:@[@"英语", @"法语", @"日语", @"西班牙语", @"俄语", @"德语", @"韩语"]];
    _secondary = [[NSPopUpButton alloc] initWithFrame:NSZeroRect pullsDown:NO];
    [_secondary addItemsWithTitles:@[@"不显示", @"英语", @"法语", @"日语", @"西班牙语", @"俄语", @"德语", @"韩语"]];
    _target.target = self; _target.action = @selector(controlChanged:);
    _secondary.target = self; _secondary.action = @selector(controlChanged:);
    _provider = [[NSPopUpButton alloc] initWithFrame:NSZeroRect pullsDown:NO];
    [_provider addItemsWithTitles:@[@"腾讯云", @"小牛翻译（NiuTrans）", @"自定义 DeepLX", @"水杉账号（发送到 api.msime.app）"]];
    _provider.target = self; _provider.action = @selector(providerChanged:);
    _endpoint = [NSTextField textFieldWithString:@""]; _endpoint.placeholderString = @"https://example.com/translate";
    _key = [[NSSecureTextField alloc] initWithFrame:NSZeroRect]; _key.placeholderString = @"留空表示不鉴权";
    _plainKey = [NSTextField textFieldWithString:@""]; _plainKey.hidden = YES;
    _plainKey.allowsEditingTextAttributes = NO;
    _reveal = [NSButton checkboxWithTitle:@"显示 API Key" target:self action:@selector(revealKey:)];
    NSStackView *keys = [NSStackView stackViewWithViews:@[_key, _plainKey, _reveal]];
    keys.orientation = NSUserInterfaceLayoutOrientationVertical; keys.alignment = NSLayoutAttributeLeading;
    _tencent = [NSButton checkboxWithTitle:@"启用腾讯云在线翻译" target:self action:@selector(controlChanged:)];
    _secretId = [NSTextField textFieldWithString:@""]; _secretId.placeholderString = @"AKID...";
    _tencentKey = [[NSSecureTextField alloc] initWithFrame:NSZeroRect]; _tencentKey.placeholderString = @"SecretKey";
    _plainTencentKey = [NSTextField textFieldWithString:@""]; _plainTencentKey.hidden = YES;
    _plainTencentKey.allowsEditingTextAttributes = NO;
    _revealTencent = [NSButton checkboxWithTitle:@"显示 SecretKey" target:self action:@selector(revealTencentKey:)];
    _region = [NSTextField textFieldWithString:@""]; _region.placeholderString = @"ap-guangzhou（默认）";
    NSStackView *tencentKeys = [NSStackView stackViewWithViews:@[_tencentKey, _plainTencentKey, _revealTencent]];
    tencentKeys.orientation = NSUserInterfaceLayoutOrientationVertical; tencentKeys.alignment = NSLayoutAttributeLeading;
    _appId = [NSTextField textFieldWithString:@""]; _appId.placeholderString = @"App ID";
    _niuTransKey = [[NSSecureTextField alloc] initWithFrame:NSZeroRect]; _niuTransKey.placeholderString = @"API Key";
    _plainNiuTransKey = [NSTextField textFieldWithString:@""]; _plainNiuTransKey.hidden = YES;
    _plainNiuTransKey.allowsEditingTextAttributes = NO;
    _revealNiuTrans = [NSButton checkboxWithTitle:@"显示 API Key" target:self action:@selector(revealNiuTransKey:)];
    NSStackView *niuTransKeys = [NSStackView stackViewWithViews:@[_niuTransKey, _plainNiuTransKey, _revealNiuTrans]];
    niuTransKeys.orientation = NSUserInterfaceLayoutOrientationVertical; niuTransKeys.alignment = NSLayoutAttributeLeading;
    _grid = [NSGridView gridViewWithViews:@[
        @[[NSTextField labelWithString:@"候选释义"], _enabled],
        @[[NSTextField labelWithString:@"离线释义"], _offline],
        @[[NSTextField labelWithString:@"中文候选目标语言"], _target],
        @[[NSTextField labelWithString:@"第二种候选语言"], _secondary],
        @[[NSTextField labelWithString:@"在线翻译服务"], _provider],
        @[[NSTextField labelWithString:@"完整 POST 接口地址"], _endpoint],
        @[[NSTextField labelWithString:@"Bearer API Key（可选）"], keys],
        @[[NSTextField labelWithString:@"腾讯云服务"], _tencent],
        @[[NSTextField labelWithString:@"SecretId"], _secretId],
        @[[NSTextField labelWithString:@"SecretKey"], tencentKeys],
        @[[NSTextField labelWithString:@"腾讯云区域"], _region],
        @[[NSTextField labelWithString:@"小牛翻译 App ID"], _appId],
        @[[NSTextField labelWithString:@"小牛翻译 API Key"], niuTransKeys]]];
    _grid.rowSpacing = 14;
    for (NSTextField *field in @[_endpoint, _key, _plainKey, _secretId, _tencentKey, _plainTencentKey, _region, _appId, _niuTransKey, _plainNiuTransKey]) {
        [field.widthAnchor constraintEqualToConstant:310].active = YES;
        field.delegate = self;
    }
    NSTextField *notice = [NSTextField wrappingLabelWithString:@"可同时显示两种候选释义；相同语言会自动去重。离线英文释义使用随客户端打包的词库，不联网；英文候选译为中文，英语目标优先查本地词库。不选择翻译服务时不联网。自定义服务优先，未命中候选会发送到所填地址（建议 HTTPS）；也可选择腾讯云或小牛翻译。腾讯云须填写 SecretId 和 SecretKey，小牛翻译须填写 App ID 和 API Key。凭据仅保存在本机配置文件，不参与云端设置同步。选择「水杉账号」会把当前页的中文候选词发送到 api.msime.app，首次使用会创建匿名账号。"];
    _status = [NSTextField wrappingLabelWithString:@""];
    NSStackView *stack = [NSStackView stackViewWithViews:@[_grid, notice, _status]];
    stack.orientation = NSUserInterfaceLayoutOrientationVertical; stack.alignment = NSLayoutAttributeLeading; stack.spacing = 16;
    stack.translatesAutoresizingMaskIntoConstraints = NO; [window.contentView addSubview:stack];
    [NSLayoutConstraint activateConstraints:@[[stack.leadingAnchor constraintEqualToAnchor:window.contentView.leadingAnchor constant:20],
        [stack.trailingAnchor constraintEqualToAnchor:window.contentView.trailingAnchor constant:-20],
        [stack.topAnchor constraintEqualToAnchor:window.contentView.topAnchor constant:20]]];
    [window center]; [self updateControls:nil];
}
- (void)showWindow:(id)sender {
    if (!self.window) [self loadWindow];
    [super showWindow:sender]; [self reload:nil];
}
- (void)invalidatePendingCallbacks {
    ++_callbackGeneration;
}
- (void)updateControls:(id)sender {
    (void)sender;
    BOOL ready = !_busy && _snapshot != nil;
    _enabled.enabled = ready; _offline.enabled = ready; _provider.enabled = ready;
    _target.enabled = ready && (_enabled.state == NSControlStateValueOn || _offline.state == NSControlStateValueOn);
    _secondary.enabled = _target.enabled;
    BOOL selectedTencent = _provider.indexOfSelectedItem == 0;
    BOOL selectedNiuTrans = _provider.indexOfSelectedItem == 1;
    BOOL selectedCustom = _provider.indexOfSelectedItem == 2;
    BOOL selectedAccount = _provider.indexOfSelectedItem == 3;
    for (NSInteger row = 5; row <= 6; ++row) [_grid rowAtIndex:row].hidden = !selectedCustom;
    for (NSInteger row = 7; row <= 10; ++row) [_grid rowAtIndex:row].hidden = selectedCustom || selectedNiuTrans || selectedAccount;
    for (NSInteger row = 11; row <= 12; ++row) [_grid rowAtIndex:row].hidden = !selectedNiuTrans;
    BOOL custom = ready && selectedCustom;
    BOOL niuTrans = ready && selectedNiuTrans;
    _endpoint.enabled = custom; _key.enabled = custom; _plainKey.enabled = custom; _reveal.enabled = custom;
    _tencent.enabled = ready && selectedTencent;
    BOOL tencent = ready && selectedTencent && _tencent.state == NSControlStateValueOn;
    _secretId.enabled = tencent; _tencentKey.enabled = tencent; _plainTencentKey.enabled = tencent;
    _revealTencent.enabled = tencent; _region.enabled = tencent;
    _appId.enabled = niuTrans; _niuTransKey.enabled = niuTrans; _plainNiuTransKey.enabled = niuTrans; _revealNiuTrans.enabled = niuTrans;
}
- (void)controlChanged:(id)sender {
    [self updateControls:sender];
    [self commit:sender];
}
- (void)controlTextDidEndEditing:(NSNotification *)notification {
    (void)notification;
    [self commit:nil];
}
- (void)providerChanged:(id)sender {
    (void)sender;
    // Switching provider retains drafts but always remasks any revealed key.
    if (_reveal.state == NSControlStateValueOn) {
        _reveal.state = NSControlStateValueOff; [self revealKey:nil];
    }
    if (_revealTencent.state == NSControlStateValueOn) {
        _revealTencent.state = NSControlStateValueOff; [self revealTencentKey:nil];
    }
    if (_revealNiuTrans.state == NSControlStateValueOn) {
        _revealNiuTrans.state = NSControlStateValueOff; [self revealNiuTransKey:nil];
    }
    [self updateControls:nil];
    // A provider whose fields are still incomplete is not written; the one in force stays until they are filled in.
    [self commit:nil];
}
- (void)revealKey:(id)sender {
    (void)sender;
    // Ending the edit here would read the field being swapped out; the edit is committed once the text has moved.
    _holdCommits = YES; [self.window makeFirstResponder:nil]; _holdCommits = NO;
    BOOL reveal = _reveal.state == NSControlStateValueOn;
    if (reveal) { _plainKey.stringValue = _key.stringValue; _key.stringValue = @""; }
    else { _key.stringValue = _plainKey.stringValue; _plainKey.stringValue = @""; }
    _plainKey.hidden = !reveal; _key.hidden = reveal;
    [self commit:nil];
}
- (void)reload:(id)sender {
    (void)sender;
    if (_busy) return;
    if (!_directory.isAbsolutePath) { _status.stringValue = @"请先激活水杉输入法以加载本机配置。"; return; }
    _busy = YES; _snapshot = nil; _committed = nil; _pending = NO; _key.stringValue = @""; _plainKey.stringValue = @"";
    _secretId.stringValue = @""; _tencentKey.stringValue = @""; _plainTencentKey.stringValue = @"";
    _appId.stringValue = @""; _niuTransKey.stringValue = @""; _plainNiuTransKey.stringValue = @"";
    _status.stringValue = @"正在加载…"; [self updateControls:nil];
    NSUInteger epoch = ++_epoch;
    NSString *directory = _directory;
    __weak MSIMETranslationSettingsWindow *weakSelf = self;
    dispatch_async(_queue, ^{
        NSDictionary *snapshot = [MSIMEClientSession loadPreferencesInDirectory:directory error:nil];
        dispatch_async(dispatch_get_main_queue(), ^{
            MSIMETranslationSettingsWindow *current = weakSelf;
            if (!current || current->_epoch != epoch) return;
            current->_busy = NO; current->_snapshot = snapshot;
            NSDictionary *preferences = snapshot[@"preferences"], *custom = preferences[@"custom_translation"], *niutrans = preferences[@"niutrans"];
            if (snapshot) {
                current->_enabled.state = [preferences[@"candidate_translations"] boolValue] ? NSControlStateValueOn : NSControlStateValueOff;
                current->_offline.state = [preferences[@"candidate_english_gloss"] boolValue] ? NSControlStateValueOn : NSControlStateValueOff;
                NSUInteger index = [TranslationLanguages() indexOfObject:preferences[@"translation_target_language"] ?: @"en"];
                [current->_target selectItemAtIndex:index == NSNotFound ? 0 : index];
                NSUInteger secondary = [TranslationLanguages() indexOfObject:preferences[@"translation_secondary_language"] ?: @""];
                [current->_secondary selectItemAtIndex:secondary == NSNotFound ? 0 : secondary + 1];
                // Same precedence as `selected_translation_services` in crates/host-api/src/ffi/providers.rs: Tencent's default `enabled: true` without usable secrets does not shadow the account.
                NSDictionary *tencentPreferences = preferences[@"tencent_tmt"];
                BOOL tencentUsable = [tencentPreferences[@"enabled"] boolValue] && TencentCredentialUsable(tencentPreferences[@"secret_id"]) && TencentCredentialUsable(tencentPreferences[@"secret_key"]);
                NSUInteger provider = [niutrans[@"enabled"] boolValue] ? 1
                    : ([custom[@"enabled"] boolValue] ? 2 : ([preferences[@"translation_account"] boolValue] && !tencentUsable ? 3 : 0));
                [current->_provider selectItemAtIndex:provider];
                current->_endpoint.stringValue = custom[@"endpoint"] ?: @"";
                current->_key.stringValue = custom[@"api_key"] ?: @"";
                current->_plainKey.hidden = YES; current->_key.hidden = NO; current->_reveal.state = NSControlStateValueOff;
                NSDictionary *tencent = preferences[@"tencent_tmt"];
                current->_tencent.state = [tencent[@"enabled"] boolValue] ? NSControlStateValueOn : NSControlStateValueOff;
                current->_secretId.stringValue = tencent[@"secret_id"] ?: @"";
                current->_tencentKey.stringValue = tencent[@"secret_key"] ?: @"";
                current->_region.stringValue = tencent[@"region"] ?: @"ap-guangzhou";
                current->_plainTencentKey.hidden = YES; current->_tencentKey.hidden = NO;
                current->_revealTencent.state = NSControlStateValueOff;
                current->_appId.stringValue = niutrans[@"app_id"] ?: @"";
                current->_niuTransKey.stringValue = niutrans[@"apikey"] ?: @"";
                current->_plainNiuTransKey.hidden = YES; current->_niuTransKey.hidden = NO;
                current->_revealNiuTrans.state = NSControlStateValueOff;
                current->_committed = [current formPreferences];
            }
            current->_status.stringValue = snapshot ? @"修改会自动保存到本机配置。" : @"加载失败；未修改任何设置。";
            [current updateControls:nil];
        });
    });
}
/// The keys this window owns, as the controls now read; NSNull marks a key the document should not carry.
- (NSDictionary *)formPreferences {
    NSString *key = _reveal.state == NSControlStateValueOn ? _plainKey.stringValue : _key.stringValue;
    NSUInteger provider = _provider.indexOfSelectedItem;
    BOOL selectedNiuTrans = provider == 1;
    BOOL selectedCustom = provider == 2;
    BOOL selectedAccount = provider == 3;
    NSString *niuTransKey = _revealNiuTrans.state == NSControlStateValueOn ? _plainNiuTransKey.stringValue : _niuTransKey.stringValue;
    NSMutableDictionary *preferences = [NSMutableDictionary dictionary];
    preferences[@"custom_translation"] = @{@"enabled":@(selectedCustom), @"endpoint":_endpoint.stringValue, @"api_key":key};
    // Choosing the MSIME account turns Tencent off, so the account is never shadowed by a Tencent checkbox the popup no longer shows.
    preferences[@"tencent_tmt"] = @{@"enabled":@(!selectedAccount && _tencent.state == NSControlStateValueOn),
        @"secret_id":_secretId.stringValue, @"secret_key":_revealTencent.state == NSControlStateValueOn ? _plainTencentKey.stringValue : _tencentKey.stringValue,
        @"region":_region.stringValue};
    preferences[@"niutrans"] = @{@"enabled":@(selectedNiuTrans), @"app_id":_appId.stringValue, @"apikey":niuTransKey};
    // Written only when chosen, like translation_secondary_language, so a document that never chose the account stays readable by older strict parsers.
    preferences[@"translation_account"] = selectedAccount ? @YES : NSNull.null;
    preferences[@"candidate_translations"] = @(_enabled.state == NSControlStateValueOn);
    preferences[@"candidate_english_gloss"] = @(_offline.state == NSControlStateValueOn);
    preferences[@"translation_target_language"] = TranslationLanguages()[_target.indexOfSelectedItem];
    preferences[@"translation_secondary_language"] = _secondary.indexOfSelectedItem > 0
        ? TranslationLanguages()[_secondary.indexOfSelectedItem - 1] : NSNull.null;
    return preferences;
}
/// Why `form` cannot be written, or nil when it can.
- (NSString *)validationProblem:(NSDictionary *)form {
    NSDictionary *custom = form[@"custom_translation"];
    // Use the same descriptor validation as runtime for the provider that is selected.
    if ([custom[@"enabled"] boolValue]) {
        NSString *endpoint = custom[@"endpoint"];
        NSDictionary *request = [MSIMEClientSession customTranslationHTTPRequest:@{@"config":custom,
            @"text":@"validation", @"source_language":@"en", @"target_language":@"zh"} error:nil];
        NSURLComponents *url = [NSURLComponents componentsWithString:endpoint];
        if (!request || !url.host.length || url.user || url.password || url.fragment ||
            [endpoint rangeOfCharacterFromSet:NSCharacterSet.controlCharacterSet].location != NSNotFound)
            return @"请输入有效的 HTTP(S) 完整接口地址和不含控制字符的 API Key；修改尚未保存。";
    }
    NSDictionary *niutrans = form[@"niutrans"];
    if ([niutrans[@"enabled"] boolValue]) {
        NSDictionary *request = [MSIMEClientSession niuTransTranslationHTTPRequest:@{@"config":niutrans,
            @"text":@"validation", @"source_language":@"en", @"target_language":@"zh", @"timestamp":@"1704067200000"} error:nil];
        if (!request || ![request[@"url"] isKindOfClass:NSString.class]) return @"请输入有效的小牛翻译 App ID 和 API Key；修改尚未保存。";
    }
    return nil;
}
/// The owned keys whose value in `form` differs from what was last written.
- (NSDictionary *)editsInForm:(NSDictionary *)form {
    NSMutableDictionary *edits = [NSMutableDictionary dictionary];
    for (NSString *key in form) if (![form[key] isEqual:_committed[key]]) edits[key] = form[key];
    return edits;
}
/// Writes what changed since the last save. One save runs at a time; a commit that arrives meanwhile runs again once it finishes.
- (void)commit:(id)sender {
    (void)sender;
    if (_busy || _holdCommits || !_snapshot) return;
    if (_saving) { _pending = YES; return; }
    NSDictionary *form = [self formPreferences];
    NSDictionary *edits = [self editsInForm:form];
    if (!edits.count) return;
    NSString *problem = [self validationProblem:form];
    if (problem) { _status.stringValue = problem; return; }
    _saving = YES; _status.stringValue = @"正在保存…";
    NSUInteger epoch = _epoch;
    NSString *directory = _directory;
    NSDictionary *snapshot = _snapshot;
    void (^savedHandler)(NSDictionary *) = _saved;
    __weak MSIMETranslationSettingsWindow *weakSelf = self;
    dispatch_async(_queue, ^{
        NSDictionary *saved = SaveTranslationEdits(directory, snapshot, edits);
        dispatch_async(dispatch_get_main_queue(), ^{
            MSIMETranslationSettingsWindow *current = weakSelf;
            if (!current || current->_epoch != epoch) return;
            // 保存排队期间窗口可能已经关闭或开始新一轮加载；这个结果属于旧页面，不能通知当前宿主。
            if (saved && savedHandler) savedHandler(saved[@"preferences"]);
            current->_saving = NO;
            if (saved) { current->_snapshot = saved; current->_committed = form; }
            current->_status.stringValue = saved ? @"已保存到本机配置。" : @"保存失败，修改尚未写入；再次修改或关闭窗口时会重试。";
            if (current->_pending) { current->_pending = NO; [current commit:nil]; }
        });
    });
}
- (void)windowWillClose:(NSNotification *)notification {
    (void)notification;
    // Ending the edit in progress would commit it on its own; it is written below together with everything else still unsaved.
    _holdCommits = YES; [self.window makeFirstResponder:nil]; _holdCommits = NO;
    if (!_busy && _snapshot) {
        NSDictionary *form = [self formPreferences];
        NSDictionary *edits = [self editsInForm:form];
        // Queued behind a save still in flight; if that one moved the revision, the edits are moved onto it.
        if (edits.count && ![self validationProblem:form]) {
            NSString *directory = _directory;
            NSDictionary *snapshot = _snapshot;
            void (^savedHandler)(NSDictionary *) = _saved;
            NSUInteger callbackGeneration = _callbackGeneration;
            __weak MSIMETranslationSettingsWindow *weakSelf = self;
            dispatch_async(_queue, ^{
                NSDictionary *saved = SaveTranslationEdits(directory, snapshot, edits);
                if (saved && savedHandler) dispatch_async(dispatch_get_main_queue(), ^{
                    MSIMETranslationSettingsWindow *current = weakSelf;
                    if (current && current->_callbackGeneration == callbackGeneration) savedHandler(saved[@"preferences"]);
                });
            });
        }
    }
    ++_epoch; _busy = NO; _saving = NO; _pending = NO; _snapshot = nil; _committed = nil;
    _key.stringValue = @""; _plainKey.stringValue = @""; _endpoint.stringValue = @"";
    _secretId.stringValue = @""; _tencentKey.stringValue = @""; _plainTencentKey.stringValue = @"";
    _appId.stringValue = @""; _niuTransKey.stringValue = @""; _plainNiuTransKey.stringValue = @"";
}
- (void)revealTencentKey:(id)sender {
    (void)sender;
    // Ending the edit here would read the field being swapped out; the edit is committed once the text has moved.
    _holdCommits = YES; [self.window makeFirstResponder:nil]; _holdCommits = NO;
    BOOL reveal = _revealTencent.state == NSControlStateValueOn;
    if (reveal) { _plainTencentKey.stringValue = _tencentKey.stringValue; _tencentKey.stringValue = @""; }
    else { _tencentKey.stringValue = _plainTencentKey.stringValue; _plainTencentKey.stringValue = @""; }
    _plainTencentKey.hidden = !reveal; _tencentKey.hidden = reveal;
    [self commit:nil];
}
- (void)revealNiuTransKey:(id)sender {
    (void)sender;
    // Ending the edit here would read the field being swapped out; the edit is committed once the text has moved.
    _holdCommits = YES; [self.window makeFirstResponder:nil]; _holdCommits = NO;
    BOOL reveal = _revealNiuTrans.state == NSControlStateValueOn;
    if (reveal) { _plainNiuTransKey.stringValue = _niuTransKey.stringValue; _niuTransKey.stringValue = @""; }
    else { _niuTransKey.stringValue = _plainNiuTransKey.stringValue; _plainNiuTransKey.stringValue = @""; }
    _plainNiuTransKey.hidden = !reveal; _niuTransKey.hidden = reveal;
    [self commit:nil];
}
@end
