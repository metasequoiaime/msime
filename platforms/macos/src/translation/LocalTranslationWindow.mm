#import "LocalTranslationWindow.h"
#import "../core/WindowPresentation.h"
#include "msime_client.h"
#include "../../../common/HostApiString.h"
#include <atomic>
#include <cstring>

namespace {
using Call = char *(*)(const uint8_t *, size_t);
NSDictionary *call(Call function, NSDictionary *request) {
    NSData *data = [NSJSONSerialization dataWithJSONObject:request options:0 error:nil];
    if (!data) return nil;
    auto raw = msime::host_api::own_string(function(static_cast<const uint8_t *>(data.bytes), data.length));
    if (!raw) return nil;
    id value = [NSJSONSerialization JSONObjectWithData:[NSData dataWithBytes:raw.get() length:std::strlen(raw.get())] options:0 error:nil];
    return [value isKindOfClass:NSDictionary.class] ? value : nil;
}
id value(NSDictionary *envelope) { return [envelope[@"ok"] boolValue] ? envelope[@"value"] : nil; }
struct Progress {
    std::atomic<bool> cancelled{false};
    std::atomic<uint64_t> done{0}, total{0};
};
void progress(void *context, const char *, uint64_t done, uint64_t total) {
    auto &state = *static_cast<Progress *>(context);
    state.done = done; state.total = total;
    if (state.cancelled) {
        constexpr char pack[] = "local-translation";
        msime_client_resource_pack_cancel(reinterpret_cast<const uint8_t *>(pack), sizeof(pack) - 1);
    }
}
NSString *errorMessage(NSDictionary *envelope) {
    NSString *code = [envelope[@"error"] isKindOfClass:NSString.class] ? envelope[@"error"] : @"";
    if ([code containsString:@"cancelled"]) return @"已取消";
    if ([code containsString:@"checksum"] || [code containsString:@"size_mismatch"]) return @"模型文件校验失败，请重新下载或导入";
    if ([code containsString:@"preferences changed"]) return @"设置已在其他窗口更新，请重新选择";
    if ([code containsString:@"busy"]) return @"模型正在使用，请稍后重试";
    return @"操作失败，请重试";
}
}

@interface MSIMELocalTranslationWindow () <NSWindowDelegate>
@end
@implementation MSIMELocalTranslationWindow {
    NSString *_directory;
    NSButton *_enabled, *_download, *_import, *_remove, *_cancel;
    NSPopUpButton *_language;
    NSTextField *_status, *_model;
    NSProgressIndicator *_indicator;
    NSArray<NSDictionary *> *_languages;
    NSDictionary *_settings;
    BOOL _available, _installed, _busy;
    NSString *_statusOverride;
    std::shared_ptr<Progress> _progress;
    NSTimer *_timer;
}
+ (void)showForDirectory:(NSString *)directory {
    if (![directory isKindOfClass:NSString.class] || !directory.isAbsolutePath) return;
    static MSIMELocalTranslationWindow *controller;
    if (!controller) controller = [[self alloc] initWithWindow:nil];
    if (controller->_busy && ![controller->_directory isEqual:directory]) return;
    controller->_directory = [directory copy];
    if (!controller.window) [controller loadWindow];
    [controller showWindow:nil];
    MSIMEPresentWindow(controller.window);
    if (!controller->_busy) [controller refresh];
}
- (void)loadWindow {
    NSWindow *window = [[NSWindow alloc] initWithContentRect:NSMakeRect(0, 0, 480, 335)
        styleMask:NSWindowStyleMaskTitled | NSWindowStyleMaskClosable backing:NSBackingStoreBuffered defer:NO];
    window.title = @"本地模型翻译";
    window.delegate = self;
    self.window = window;
    _enabled = [NSButton checkboxWithTitle:@"启用本地翻译" target:self action:@selector(save:)];
    _language = [[NSPopUpButton alloc] initWithFrame:NSZeroRect pullsDown:NO];
    _language.target = self; _language.action = @selector(save:);
    [_language setAccessibilityLabel:@"目标语言"];
    _model = [NSTextField labelWithString:@"正在读取…"];
    _status = [NSTextField wrappingLabelWithString:@"正在读取…"];
    _indicator = [[NSProgressIndicator alloc] initWithFrame:NSZeroRect];
    _indicator.indeterminate = NO; _indicator.minValue = 0; _indicator.maxValue = 100;
    [_indicator setAccessibilityLabel:@"模型安装进度"];
    _download = [NSButton buttonWithTitle:@"下载模型" target:self action:@selector(download:)];
    _import = [NSButton buttonWithTitle:@"从文件导入…" target:self action:@selector(importModel:)];
    _remove = [NSButton buttonWithTitle:@"删除模型…" target:self action:@selector(removeModel:)];
    _cancel = [NSButton buttonWithTitle:@"取消" target:self action:@selector(cancel:)];
    NSStackView *actions = [NSStackView stackViewWithViews:@[_download, _import, _remove, _cancel]];
    actions.spacing = 8;
    NSGridView *grid = [NSGridView gridViewWithViews:@[
        @[[NSTextField labelWithString:@"翻译"], _enabled],
        @[[NSTextField labelWithString:@"目标语言"], _language],
        @[[NSTextField labelWithString:@"模型"], _model]]];
    grid.rowSpacing = 16;
    NSButton *source = [NSButton buttonWithTitle:@"来源与许可 · 腾讯 Hy-MT2 · Apache-2.0" target:self action:@selector(showSource:)];
    source.bordered = NO;
    NSStackView *stack = [NSStackView stackViewWithViews:@[grid, source, _status, _indicator, actions]];
    stack.orientation = NSUserInterfaceLayoutOrientationVertical;
    stack.alignment = NSLayoutAttributeLeading; stack.spacing = 20;
    stack.translatesAutoresizingMaskIntoConstraints = NO;
    [window.contentView addSubview:stack];
    [NSLayoutConstraint activateConstraints:@[
        [stack.leadingAnchor constraintEqualToAnchor:window.contentView.leadingAnchor constant:24],
        [stack.trailingAnchor constraintEqualToAnchor:window.contentView.trailingAnchor constant:-24],
        [stack.topAnchor constraintEqualToAnchor:window.contentView.topAnchor constant:24],
        [_indicator.widthAnchor constraintEqualToConstant:432],
        [_language.widthAnchor constraintEqualToConstant:220]]];
    [window center];
    [self updateControls];
}
- (void)showSource:(id)sender {
    (void)sender;
    [NSWorkspace.sharedWorkspace openURL:[NSURL URLWithString:@"https://huggingface.co/tencent/HY-MT2-1.8B-GGUF"]];
}
- (void)updateControls {
    const BOOL loaded = _settings != nil;
    _enabled.enabled = loaded && _available && !_busy && (_installed || _enabled.state == NSControlStateValueOn);
    _language.enabled = loaded && _available && !_busy;
    _download.enabled = loaded && _available && !_busy;
    _import.enabled = loaded && _available && !_busy;
    _remove.enabled = loaded && _installed && !_busy;
    _cancel.hidden = !_busy || !_progress;
    _indicator.hidden = !_progress;
}
- (void)refresh {
    if (_busy) return;
    _busy = YES; [self updateControls];
    NSString *directory = _directory;
    __weak MSIMELocalTranslationWindow *weakSelf = self;
    dispatch_async(dispatch_get_global_queue(QOS_CLASS_UTILITY, 0), ^{
        @autoreleasepool {
            NSDictionary *settingsEnvelope = call(msime_client_local_translation_settings, @{@"directory":directory});
            NSDictionary *settings = value(settingsEnvelope);
            auto raw = msime::host_api::own_string(msime_client_local_translation_catalog());
            NSDictionary *catalog = raw ? value([NSJSONSerialization JSONObjectWithData:[NSData dataWithBytes:raw.get() length:std::strlen(raw.get())] options:0 error:nil]) : nil;
            NSArray *packs = value(call(msime_client_resource_packs, @{@"state_root":directory}));
            BOOL installed = NO;
            for (NSDictionary *pack in packs) if ([pack[@"id"] isEqual:@"local-translation"]) installed = [pack[@"state"] isEqual:@"installed"];
            dispatch_async(dispatch_get_main_queue(), ^{
                MSIMELocalTranslationWindow *owner = weakSelf;
                if (!owner || ![owner->_directory isEqual:directory]) return;
                owner->_busy = NO;
                owner->_settings = settings;
                owner->_installed = installed;
                owner->_available = [catalog[@"available"] boolValue];
                owner->_languages = catalog[@"languages"];
                owner->_model.stringValue = [NSString stringWithFormat:@"%@ · %@", catalog[@"model"] ?: @"模型不可用",
                    [NSByteCountFormatter stringFromByteCount:[catalog[@"size"] longLongValue] countStyle:NSByteCountFormatterCountStyleFile]];
                [owner->_language removeAllItems];
                for (NSDictionary *language in owner->_languages) {
                    [owner->_language addItemWithTitle:language[@"name"]];
                    owner->_language.lastItem.representedObject = language[@"code"];
                    if ([language[@"code"] isEqual:settings[@"local_translation"][@"target_language"]])
                        [owner->_language selectItem:owner->_language.lastItem];
                }
                owner->_enabled.state = [settings[@"local_translation"][@"enabled"] boolValue] ? NSControlStateValueOn : NSControlStateValueOff;
                owner->_status.stringValue = !settings ? errorMessage(settingsEnvelope) : !owner->_available ? @"此构建未包含本地推理库" : installed ? @"已安装 · 翻译无需联网" : @"未安装";
                if (owner->_statusOverride) { owner->_status.stringValue = owner->_statusOverride; owner->_statusOverride = nil; }
                [owner updateControls];
            });
        }
    });
}
- (void)save:(id)sender {
    (void)sender;
    if (_busy || !_settings || !_language.selectedItem.representedObject) return;
    NSDictionary *request = @{@"directory":_directory, @"update":@{
        @"expected_revision":_settings[@"revision"], @"enabled":@(_enabled.state == NSControlStateValueOn),
        @"target_language":_language.selectedItem.representedObject}};
    [self perform:^{ return call(msime_client_local_translation_settings, request); }];
}
- (void)perform:(NSDictionary *(^)(void))operation {
    _busy = YES; [self updateControls];
    __weak MSIMELocalTranslationWindow *weakSelf = self;
    dispatch_async(dispatch_get_global_queue(QOS_CLASS_UTILITY, 0), ^{
        @autoreleasepool {
            NSDictionary *result = operation();
            dispatch_async(dispatch_get_main_queue(), ^{
                MSIMELocalTranslationWindow *owner = weakSelf;
                if (!owner) return;
                owner->_busy = NO;
                owner->_progress.reset();
                [owner->_timer invalidate]; owner->_timer = nil;
                if (!value(result)) owner->_statusOverride = errorMessage(result);
                [owner refresh];
            });
        }
    });
}
- (void)installFiles:(NSArray<NSString *> *)files {
    if (_busy) return;
    NSMutableDictionary *request = [@{@"state_root":_directory, @"pack":@"local-translation"} mutableCopy];
    if (files) request[@"files"] = files;
    _progress = std::make_shared<Progress>();
    const auto state = _progress;
    __weak MSIMELocalTranslationWindow *weakSelf = self;
    _timer = [NSTimer scheduledTimerWithTimeInterval:0.1 repeats:YES block:^(NSTimer *) {
        MSIMELocalTranslationWindow *owner = weakSelf;
        if (!owner) return;
        const uint64_t total = state->total.load(), done = state->done.load();
        owner->_indicator.doubleValue = total ? 100.0 * done / total : 0;
        owner->_status.stringValue = state->cancelled ? @"正在取消…" : [NSString stringWithFormat:@"安装中 %.0f%%", owner->_indicator.doubleValue];
    }];
    [self perform:^{
        NSData *data = [NSJSONSerialization dataWithJSONObject:request options:0 error:nil];
        auto raw = msime::host_api::own_string(msime_client_resource_pack_install(static_cast<const uint8_t *>(data.bytes), data.length, progress, state.get()));
        return raw ? [NSJSONSerialization JSONObjectWithData:[NSData dataWithBytes:raw.get() length:std::strlen(raw.get())] options:0 error:nil] : nil;
    }];
}
- (void)download:(id)sender { (void)sender; [self installFiles:nil]; }
- (void)importModel:(id)sender {
    (void)sender;
    if (_busy) return;
    NSOpenPanel *panel = [NSOpenPanel openPanel];
    panel.canChooseDirectories = NO; panel.allowsMultipleSelection = NO;
    panel.title = @"导入 Hy-MT2 Q4_K_M 模型";
    [panel beginSheetModalForWindow:self.window completionHandler:^(NSModalResponse result) {
        if (result == NSModalResponseOK && panel.URL.isFileURL) [self installFiles:@[panel.URL.path]];
    }];
}
- (void)removeModel:(id)sender {
    (void)sender;
    if (_busy) return;
    NSAlert *alert = [[NSAlert alloc] init];
    alert.messageText = @"删除本地翻译模型？";
    alert.informativeText = @"删除后需重新下载或导入才能离线翻译。";
    [alert addButtonWithTitle:@"取消"]; [alert addButtonWithTitle:@"删除"];
    [alert beginSheetModalForWindow:self.window completionHandler:^(NSModalResponse result) {
        if (result != NSAlertSecondButtonReturn) return;
        NSDictionary *request = @{@"state_root":self->_directory, @"pack":@"local-translation"};
        msime_client_local_translation_cancel(nullptr, 0);
        NSDictionary *disable = @{@"directory":self->_directory, @"update":@{
            @"expected_revision":self->_settings[@"revision"], @"enabled":@NO,
            @"target_language":self->_settings[@"local_translation"][@"target_language"]}};
        [self perform:^{
            NSDictionary *saved = call(msime_client_local_translation_settings, disable);
            return value(saved) ? call(msime_client_resource_pack_remove, request) : saved;
        }];
    }];
}
- (void)cancel:(id)sender {
    (void)sender;
    if (_progress) {
        _progress->cancelled = true;
        constexpr char pack[] = "local-translation";
        msime_client_resource_pack_cancel(reinterpret_cast<const uint8_t *>(pack), sizeof(pack) - 1);
    }
}
- (void)windowWillClose:(NSNotification *)notification {
    (void)notification;
    [self cancel:nil];
}
@end
