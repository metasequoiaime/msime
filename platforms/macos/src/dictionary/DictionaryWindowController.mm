#import "DictionaryWindowController.h"
#import "MSIMEClientSession.h"
#import "../core/ClientDictionaryRuntime.h"
#import "../core/BoundedFileReader.h"
#import "../core/EditionIdentity.h"
#import <UniformTypeIdentifiers/UniformTypeIdentifiers.h>
#include "../../../common/DictionaryQuiesceLease.h"

// The same budget the settings window gives the input hosts (crates/client-core/src/dictionary/quiesce.rs).
static const NSTimeInterval MSIMEDictionaryQuiesceBudget = 2.5;
static const useconds_t MSIMEDictionaryQuiesceRetryMicroseconds = 50000;

// A mutation from this window runs inside the input method, whose own controllers hold the dictionary lock through their sessions. Only the lock failure is retried, so a completed write is never replayed: the lease goes up beside the lock, the controllers are told on the main thread and release before this continues, and the request is retried until it gets through or the budget runs out. Removing the lease is the resume; the next key reopens a session. Called off the main thread.
static NSDictionary *MSIMEQuiescedDictionaryRequest(NSDictionary *request, NSError **error) {
    NSError *failure = nil;
    NSDictionary *result = [MSIMEClientSession dictionaryRequest:request error:&failure];
    NSString *userData = [request[@"options"] isKindOfClass:NSDictionary.class] ? request[@"options"][@"user_data"] : nil;
    if (!result && [failure.localizedDescription isEqualToString:@"dictionary maintenance busy"] &&
        [userData isKindOfClass:NSString.class] && userData.isAbsolutePath) {
        const std::string root(userData.fileSystemRepresentation);
        std::string lease;
        if (msime::dictionary_lease::raise_dictionary_quiesce_lease(root, lease)) {
            void (^announce)(void) = ^{
                [NSNotificationCenter.defaultCenter postNotificationName:MSIMEEditionNotificationName(@"MSIMEDictionaryMaintenanceWillBeginNotification") object:nil];
            };
            if (NSThread.isMainThread) announce();
            else dispatch_sync(dispatch_get_main_queue(), announce);
            const NSTimeInterval deadline = NSProcessInfo.processInfo.systemUptime + MSIMEDictionaryQuiesceBudget;
            while (!result && [failure.localizedDescription isEqualToString:@"dictionary maintenance busy"] &&
                   NSProcessInfo.processInfo.systemUptime < deadline) {
                usleep(MSIMEDictionaryQuiesceRetryMicroseconds);
                failure = nil;
                result = [MSIMEClientSession dictionaryRequest:request error:&failure];
            }
            msime::dictionary_lease::lower_dictionary_quiesce_lease(root, lease);
        }
    }
    if (error) *error = failure;
    return result;
}

@interface MSIMEDictionaryWindowController () <NSTableViewDataSource, NSTableViewDelegate>
@property(nonatomic, copy) NSDictionary *options;
@property(nonatomic, strong) NSTableView *table;
@property(nonatomic, copy) NSArray<NSDictionary *> *entries;
@property(nonatomic) NSUInteger offset;
@property(nonatomic, strong) NSButton *previous;
@property(nonatomic, strong) NSButton *next;
@property(nonatomic, strong) NSTextField *pageLabel;
@property(nonatomic, strong) NSPopUpButton *kind;
@property(nonatomic, strong) NSPopUpButton *format;
@property(nonatomic, strong) NSButton *importButton;
@property(nonatomic, strong) NSButton *exportButton;
@property(nonatomic, strong) NSButton *addButton;
@property(nonatomic, strong) NSButton *editButton;
@property(nonatomic, strong) NSButton *removeButton;
@property(nonatomic, strong) NSTextField *status;
@property(nonatomic, copy) NSString *runtimeError;
@property(nonatomic) NSUInteger refreshGeneration;
@end
@implementation MSIMEDictionaryWindowController
- (instancetype)initWithOptions:(NSDictionary *)options {
    NSWindow *window = [[NSWindow alloc] initWithContentRect:NSMakeRect(0, 0, 620, 500) styleMask:NSWindowStyleMaskTitled | NSWindowStyleMaskClosable backing:NSBackingStoreBuffered defer:NO];
    self = [super initWithWindow:window];
    if (self) {
        _options = [options copy];
        NSError *error = nil;
        if (![[MSIMEDictionaryRuntime alloc] initWithHostOptions:_options error:&error]) _runtimeError = error.localizedDescription ?: @"词典运行目录配置无效";
        window.title = @"个人词典"; window.releasedWhenClosed = NO; [self loadWindow];
    }
    return self;
}
- (void)loadWindow {
    _kind = [[NSPopUpButton alloc] initWithFrame:NSZeroRect pullsDown:NO];
    for (NSArray<NSString *> *item in @[@[@"拼音词库", @"pinyin"], @[@"86 五笔词库", @"wubi"], @[@"98 五笔词库", @"wubi98"], @[@"快捷短语", @"quick_phrase"], @[@"英文词库", @"english"]]) {
        [_kind addItemWithTitle:item[0]];
        _kind.lastItem.representedObject = item[1];
    }
    _kind.translatesAutoresizingMaskIntoConstraints = NO;
    _kind.target = self;
    _kind.action = @selector(dictionaryKindChanged:);
    _format = [[NSPopUpButton alloc] initWithFrame:NSZeroRect pullsDown:NO];
    for (NSArray<NSString *> *item in @[@[@"词在前（标准 TSV）", @"standard"], @[@"编码在前（Windows TSV）", @"windows"], @[@"Rime userdb / dict.yaml", @"rime"], @[@"汉字自动注音（仅导入）", @"hans"]]) {
        [_format addItemWithTitle:item[0]];
        _format.lastItem.representedObject = item[1];
    }
    _format.translatesAutoresizingMaskIntoConstraints = NO;
    NSButton *refresh = [NSButton buttonWithTitle:@"刷新" target:self action:@selector(refresh:)];
    refresh.translatesAutoresizingMaskIntoConstraints = NO;
    _addButton = [NSButton buttonWithTitle:@"新增…" target:self action:@selector(addEntry:)];
    _editButton = [NSButton buttonWithTitle:@"编辑…" target:self action:@selector(editEntry:)];
    _removeButton = [NSButton buttonWithTitle:@"删除" target:self action:@selector(removeEntry:)];
    _importButton = [NSButton buttonWithTitle:@"导入…" target:self action:@selector(importFile:)];
    _importButton.translatesAutoresizingMaskIntoConstraints = NO;
    _exportButton = [NSButton buttonWithTitle:@"导出…" target:self action:@selector(exportFile:)];
    _exportButton.translatesAutoresizingMaskIntoConstraints = NO;
    _previous = [NSButton buttonWithTitle:@"上一页" target:self action:@selector(previousPage:)];
    _next = [NSButton buttonWithTitle:@"下一页" target:self action:@selector(nextPage:)];
    _previous.translatesAutoresizingMaskIntoConstraints = _next.translatesAutoresizingMaskIntoConstraints = NO;
    _pageLabel = [NSTextField labelWithString:@"第 1 页"]; _pageLabel.translatesAutoresizingMaskIntoConstraints = NO;
    _status = [NSTextField labelWithString:@""]; _status.translatesAutoresizingMaskIntoConstraints = NO;
    _entries = @[];
    _table = [[NSTableView alloc] initWithFrame:NSZeroRect];
    _table.dataSource = self; _table.delegate = self; _table.usesAlternatingRowBackgroundColors = YES;
    _table.allowsMultipleSelection = NO; _table.rowHeight = 22.0;
    for (NSArray<NSString *> *definition in @[@[@"key", @"编码", @"190"], @[@"value", @"词条", @"270"], @[@"weight", @"权重", @"90"]]) {
        NSTableColumn *column = [[NSTableColumn alloc] initWithIdentifier:definition[0]];
        column.title = definition[1]; column.width = definition[2].doubleValue; [_table addTableColumn:column];
    }
    NSScrollView *scroll = [[NSScrollView alloc] initWithFrame:NSZeroRect];
    scroll.hasVerticalScroller = YES; scroll.borderType = NSBezelBorder; scroll.documentView = _table; scroll.translatesAutoresizingMaskIntoConstraints = NO;
    NSArray<NSButton *> *buttons = @[_addButton, _editButton, _removeButton, refresh, _importButton, _exportButton];
    for (NSButton *button in buttons) { button.translatesAutoresizingMaskIntoConstraints = NO; button.bezelStyle = NSBezelStyleRounded; }
    _removeButton.hasDestructiveAction = YES;
    [self.window.contentView addSubview:_previous]; [self.window.contentView addSubview:_next]; [self.window.contentView addSubview:_pageLabel]; [self.window.contentView addSubview:_status]; [self.window.contentView addSubview:scroll];
    NSStackView *toolbar = [NSStackView stackViewWithViews:@[_kind, _format, _addButton, _editButton, _removeButton, refresh, _importButton, _exportButton]];
    toolbar.orientation = NSUserInterfaceLayoutOrientationHorizontal; toolbar.spacing = 8.0; toolbar.translatesAutoresizingMaskIntoConstraints = NO;
    [self.window.contentView addSubview:toolbar];
    [NSLayoutConstraint activateConstraints:@[[toolbar.topAnchor constraintEqualToAnchor:self.window.contentView.topAnchor constant:12], [toolbar.leadingAnchor constraintEqualToAnchor:self.window.contentView.leadingAnchor constant:12], [toolbar.trailingAnchor constraintEqualToAnchor:self.window.contentView.trailingAnchor constant:-12], [_next.topAnchor constraintEqualToAnchor:toolbar.bottomAnchor constant:8], [_next.trailingAnchor constraintEqualToAnchor:self.window.contentView.trailingAnchor constant:-12], [_previous.topAnchor constraintEqualToAnchor:_next.topAnchor], [_previous.trailingAnchor constraintEqualToAnchor:_next.leadingAnchor constant:-8], [_pageLabel.centerYAnchor constraintEqualToAnchor:_next.centerYAnchor], [_pageLabel.trailingAnchor constraintEqualToAnchor:_previous.leadingAnchor constant:-12], [_status.topAnchor constraintEqualToAnchor:_next.bottomAnchor constant:8], [_status.leadingAnchor constraintEqualToAnchor:self.window.contentView.leadingAnchor constant:12], [_status.trailingAnchor constraintEqualToAnchor:self.window.contentView.trailingAnchor constant:-12], [scroll.topAnchor constraintEqualToAnchor:_status.bottomAnchor constant:8], [scroll.leadingAnchor constraintEqualToAnchor:self.window.contentView.leadingAnchor constant:12], [scroll.trailingAnchor constraintEqualToAnchor:self.window.contentView.trailingAnchor constant:-12], [scroll.bottomAnchor constraintEqualToAnchor:self.window.contentView.bottomAnchor constant:-12]]];
    [self updateEditButtons];
    [self refresh:nil];
}
- (NSString *)selectedKind { return self.kind.selectedItem.representedObject ?: @"pinyin"; }
- (NSString *)selectedFormat { return self.format.selectedItem.representedObject ?: @"standard"; }
- (void)updateEditButtons {
    const BOOL selected = self.table.selectedRow >= 0 && self.table.selectedRow < (NSInteger)self.entries.count;
    self.editButton.enabled = selected;
    self.removeButton.enabled = selected;
}
- (void)showMessage:(NSString *)message {
    self.status.stringValue = message.length ? message : @"操作失败";
}
- (BOOL)isCurrentDictionaryMutation:(NSUInteger)generation kind:(NSString *)kind offset:(NSUInteger)offset {
    return self.refreshGeneration == generation && self.offset == offset && [[self selectedKind] isEqualToString:kind];
}
- (void)dictionaryKindChanged:(id)sender {
    (void)sender;
    self.offset = 0;
    [self refresh:nil];
}
- (NSInteger)numberOfRowsInTableView:(NSTableView *)tableView {
    (void)tableView;
    return (NSInteger)self.entries.count;
}
- (NSView *)tableView:(NSTableView *)tableView viewForTableColumn:(NSTableColumn *)tableColumn row:(NSInteger)row {
    (void)tableView;
    if (row < 0 || row >= (NSInteger)self.entries.count) return nil;
    NSDictionary *entry = self.entries[(NSUInteger)row];
    NSString *value = [tableColumn.identifier isEqualToString:@"key"] ? entry[@"key"] :
        [tableColumn.identifier isEqualToString:@"value"] ? entry[@"value"] : [entry[@"weight"] stringValue];
    NSTextField *cell = [NSTextField labelWithString:[value isKindOfClass:NSString.class] ? value : @""];
    cell.lineBreakMode = NSLineBreakByTruncatingTail;
    return cell;
}
- (void)tableViewSelectionDidChange:(NSNotification *)notification {
    (void)notification;
    [self updateEditButtons];
}
- (void)refresh:(id)sender {
    (void)sender;
    NSUInteger offset = self.offset;
    NSUInteger generation = ++self.refreshGeneration;
    NSString *kind = [[self selectedKind] copy];
    if (self.runtimeError) { [self showMessage:self.runtimeError]; self.entries = @[]; [self.table reloadData]; self.previous.enabled = NO; self.next.enabled = NO; return; }
    NSDictionary *request = @{ @"options": self.options, @"action": @{ @"operation": @"list", @"offset": @(offset), @"limit": @100, @"kind": kind } };
    __weak MSIMEDictionaryWindowController *weakSelf = self;
    dispatch_async(dispatch_get_global_queue(QOS_CLASS_UTILITY, 0), ^{
        NSError *error = nil;
        NSDictionary *result = [MSIMEClientSession dictionaryRequest:request error:&error];
        NSArray *entries = [result[@"entries"] isKindOfClass:NSArray.class] ? result[@"entries"] : @[];
        NSString *message = nil;
        BOOL hasMore = [result[@"has_more"] boolValue];
        if (!result) message = error.localizedDescription ?: @"词典读取失败";
        else message = entries.count ? [NSString stringWithFormat:@"第 %lu 页，共显示 %lu 条%@", (unsigned long)(offset / 100 + 1), (unsigned long)entries.count, hasMore ? @"，还有更多" : @""] : @"暂无个人词条";
        dispatch_async(dispatch_get_main_queue(), ^{
            MSIMEDictionaryWindowController *controller = weakSelf;
            if (controller && controller.refreshGeneration == generation && controller.offset == offset &&
                [[controller selectedKind] isEqualToString:kind]) {
                controller.entries = entries;
                [controller.table reloadData];
                [controller updateEditButtons];
                [controller showMessage:message];
                controller.previous.enabled = offset >= 100;
                controller.next.enabled = hasMore;
                controller.pageLabel.stringValue = [NSString stringWithFormat:@"第 %lu 页", (unsigned long)(offset / 100 + 1)];
            }
        });
    });
}
- (void)presentEditorForEntry:(NSDictionary *)existing {
    NSTextField *key = [NSTextField textFieldWithString:existing[@"key"] ?: @""];
    NSTextField *value = [NSTextField textFieldWithString:existing[@"value"] ?: @""];
    NSTextField *weight = [NSTextField textFieldWithString:[existing[@"weight"] stringValue] ?: @"100000"];
    key.placeholderString = @"编码"; value.placeholderString = @"上屏内容"; weight.placeholderString = @"权重";
    for (NSTextField *field in @[key, value, weight]) { field.translatesAutoresizingMaskIntoConstraints = NO; [field.widthAnchor constraintEqualToConstant:280.0].active = YES; }
    NSGridView *grid = [NSGridView gridViewWithViews:@[
        @[[NSTextField labelWithString:@"编码"], key], @[[NSTextField labelWithString:@"词条"], value], @[[NSTextField labelWithString:@"权重"], weight]
    ]];
    grid.rowSpacing = 8.0; grid.columnSpacing = 10.0; [grid layoutSubtreeIfNeeded]; grid.frame = NSMakeRect(0, 0, grid.fittingSize.width, grid.fittingSize.height);
    NSAlert *alert = [NSAlert new]; alert.messageText = existing ? @"编辑词条" : @"新增词条"; alert.informativeText = @"编码和词条格式由输入法引擎校验。"; alert.accessoryView = grid;
    [alert addButtonWithTitle:existing ? @"保存" : @"添加"]; [alert addButtonWithTitle:@"取消"];
    __weak MSIMEDictionaryWindowController *weakSelf = self;
    [alert beginSheetModalForWindow:self.window completionHandler:^(NSModalResponse response) {
        if (response != NSAlertFirstButtonReturn) return;
        MSIMEDictionaryWindowController *controller = weakSelf; if (!controller) return;
        NSUInteger mutationGeneration = controller.refreshGeneration;
        NSUInteger mutationOffset = controller.offset;
        NSString *mutationKind = [[controller selectedKind] copy];
        NSDictionary *options = [controller.options copy];
        NSDictionary *replacement = @{ @"kind": mutationKind, @"key": key.stringValue, @"value": value.stringValue, @"weight": @([weight.stringValue longLongValue]) };
        NSDictionary *action = @{ @"operation": @"edit", @"previous": existing ?: [NSNull null], @"replacement": replacement, @"request_id": NSUUID.UUID.UUIDString };
        NSMutableDictionary *mutableAction = [action mutableCopy];
        if (!existing) mutableAction[@"previous"] = [NSNull null];
        dispatch_async(dispatch_get_global_queue(QOS_CLASS_UTILITY, 0), ^{
            NSError *error = nil; NSDictionary *result = MSIMEQuiescedDictionaryRequest(@{ @"options": options, @"action": mutableAction }, &error);
            dispatch_async(dispatch_get_main_queue(), ^{
                MSIMEDictionaryWindowController *current = weakSelf;
                if (!current || ![current isCurrentDictionaryMutation:mutationGeneration kind:mutationKind offset:mutationOffset]) return;
                if (!result) [current showMessage:error.localizedDescription ?: (existing ? @"词条保存失败。" : @"词条添加失败。")];
                else { current.offset = existing ? mutationOffset : 0; [current refresh:nil]; }
            });
        });
    }];
}
- (void)addEntry:(id)sender { (void)sender; if (!self.runtimeError) [self presentEditorForEntry:nil]; }
- (void)editEntry:(id)sender {
    (void)sender; NSInteger row = self.table.selectedRow;
    if (row >= 0 && row < (NSInteger)self.entries.count) [self presentEditorForEntry:self.entries[(NSUInteger)row]];
}
- (void)removeEntry:(id)sender {
    (void)sender; NSInteger row = self.table.selectedRow;
    if (row < 0 || row >= (NSInteger)self.entries.count) return;
    NSDictionary *entry = self.entries[(NSUInteger)row]; NSAlert *alert = [NSAlert new]; alert.alertStyle = NSAlertStyleWarning;
    alert.messageText = [NSString stringWithFormat:@"删除「%@」？", entry[@"value"] ?: @""]; alert.informativeText = @"此操作无法撤销。";
    [alert addButtonWithTitle:@"取消"]; [alert addButtonWithTitle:@"删除"]; alert.buttons[1].hasDestructiveAction = YES;
    __weak MSIMEDictionaryWindowController *weakSelf = self;
    [alert beginSheetModalForWindow:self.window completionHandler:^(NSModalResponse response) {
        if (response != NSAlertSecondButtonReturn) return;
        MSIMEDictionaryWindowController *controller = weakSelf; if (!controller) return;
        NSUInteger mutationGeneration = controller.refreshGeneration;
        NSUInteger mutationOffset = controller.offset;
        NSString *mutationKind = [[controller selectedKind] copy];
        NSDictionary *options = [controller.options copy];
        NSDictionary *action = @{ @"operation": @"edit", @"previous": entry, @"replacement": [NSNull null], @"request_id": NSUUID.UUID.UUIDString };
        dispatch_async(dispatch_get_global_queue(QOS_CLASS_UTILITY, 0), ^{
            NSError *error = nil; NSDictionary *result = MSIMEQuiescedDictionaryRequest(@{ @"options": options, @"action": action }, &error);
            dispatch_async(dispatch_get_main_queue(), ^{
                MSIMEDictionaryWindowController *current = weakSelf;
                if (!current || ![current isCurrentDictionaryMutation:mutationGeneration kind:mutationKind offset:mutationOffset]) return;
                if (!result) [current showMessage:error.localizedDescription ?: @"词条删除失败。"];
                else { current.offset = current.entries.count == 1 && mutationOffset >= 100 ? mutationOffset - 100 : mutationOffset; [current refresh:nil]; }
            });
        });
    }];
}
- (void)importFile:(id)sender {
    (void)sender;
    if (self.runtimeError) { [self showMessage:self.runtimeError]; return; }
    if ([[self selectedFormat] isEqualToString:@"hans"] && ![[self selectedKind] isEqualToString:@"pinyin"]) {
        [self showMessage:@"汉字自动注音格式只支持拼音词库。"];
        return;
    }
    NSOpenPanel *panel = [NSOpenPanel openPanel];
    panel.allowsMultipleSelection = NO;
    panel.canChooseDirectories = NO;
    panel.allowedContentTypes = @[UTTypePlainText, UTTypeTabSeparatedText, UTTypeYAML];
    __weak MSIMEDictionaryWindowController *weakSelf = self;
    [panel beginWithCompletionHandler:^(NSModalResponse response) {
        if (response != NSModalResponseOK || !panel.URL) return;
        MSIMEDictionaryWindowController *controller = weakSelf;
        if (!controller) return;
        NSURL *sourceURL = [panel.URL copy];
        NSError *error = nil;
        NSData *data = MSIMEReadFileUpTo(sourceURL, 65536, &error);
        if (!data || data.length == 0 || data.length > 65536) {
            [controller showMessage:@"文件需为不超过 64 KiB 的 UTF-8 文本。"];
            return;
        }
        NSString *text = [[NSString alloc] initWithData:data encoding:NSUTF8StringEncoding];
        if (!text || [text rangeOfString:@"\0"].location != NSNotFound) {
            [controller showMessage:@"文件需为不超过 64 KiB 的 UTF-8 文本。"];
            return;
        }
        NSUInteger mutationGeneration = controller.refreshGeneration;
        NSUInteger mutationOffset = controller.offset;
        NSString *mutationKind = [[controller selectedKind] copy];
        NSDictionary *options = [controller.options copy];
        NSDictionary *request = @{ @"options": options, @"action": @{ @"operation": @"import", @"kind": mutationKind, @"format": [controller selectedFormat], @"text": text, @"request_id": NSUUID.UUID.UUIDString } };
        dispatch_async(dispatch_get_global_queue(QOS_CLASS_UTILITY, 0), ^{
            NSError *requestError = nil;
            NSDictionary *result = MSIMEQuiescedDictionaryRequest(request, &requestError);
            NSString *message = nil;
            if (!result) message = requestError.localizedDescription ?: @"词典导入失败";
            else {
                NSUInteger applied = [result[@"applied"] unsignedIntegerValue];
                NSUInteger failed = [result[@"failed"] unsignedIntegerValue];
                BOOL truncated = [result[@"truncated"] boolValue];
                message = failed || truncated
                    ? [NSString stringWithFormat:@"已导入 %lu 个词条，跳过 %lu 个无效条目%@。", (unsigned long)applied, (unsigned long)failed, truncated ? @"（达到上限）" : @""]
                    : [NSString stringWithFormat:@"已导入 %lu 个词条。", (unsigned long)applied];
            }
            dispatch_async(dispatch_get_main_queue(), ^{
                MSIMEDictionaryWindowController *current = weakSelf;
                if (!current || ![current isCurrentDictionaryMutation:mutationGeneration kind:mutationKind offset:mutationOffset]) return;
                [current showMessage:message]; current.offset = 0; [current refresh:nil];
            });
        });
    }];
}
- (void)exportFile:(id)sender {
    (void)sender;
    if (self.runtimeError) { [self showMessage:self.runtimeError]; return; }
    NSString *format = [self selectedFormat];
    if ([format isEqualToString:@"hans"] || [format isEqualToString:@"rime"]) {
        [self showMessage:@"当前格式仅支持导入，请选择「词在前」或「编码在前」。"];
        return;
    }
    NSSavePanel *panel = [NSSavePanel savePanel];
    panel.allowedContentTypes = @[UTTypeTabSeparatedText];
    panel.nameFieldStringValue = [NSString stringWithFormat:@"msime-%@.tsv", [self selectedKind]];
    __weak MSIMEDictionaryWindowController *weakSelf = self;
    [panel beginWithCompletionHandler:^(NSModalResponse response) {
        if (response != NSModalResponseOK || !panel.URL) return;
        MSIMEDictionaryWindowController *controller = weakSelf;
        if (!controller) return;
        NSURL *destinationURL = [panel.URL copy];
        NSUInteger mutationGeneration = controller.refreshGeneration;
        NSUInteger mutationOffset = controller.offset;
        NSDictionary *options = [controller.options copy];
        NSString *mutationKind = [[controller selectedKind] copy];
        NSString *kind = mutationKind;
        dispatch_async(dispatch_get_global_queue(QOS_CLASS_UTILITY, 0), ^{
            NSMutableString *text = [NSMutableString string];
            NSUInteger offset = 0;
            BOOL hasMore = YES;
            NSString *message = nil;
            while (hasMore && offset <= 1000000) {
                NSDictionary *request = @{ @"options": options, @"action": @{ @"operation": @"export", @"kind": kind, @"format": format, @"offset": @(offset), @"limit": @1000 } };
                NSError *error = nil;
                NSDictionary *result = [MSIMEClientSession dictionaryRequest:request error:&error];
                if (!result) { message = error.localizedDescription ?: @"词典导出失败"; break; }
                NSString *chunk = [result[@"text"] isKindOfClass:NSString.class] ? result[@"text"] : @"";
                [text appendString:chunk];
                NSUInteger count = 0;
                for (NSString *line in [chunk componentsSeparatedByString:@"\n"]) if (line.length) ++count;
                hasMore = [result[@"has_more"] boolValue];
                if (hasMore && count == 0) { message = @"词典导出返回了无效分页。"; break; }
                offset += count;
            }
            if (!message && hasMore) message = @"词典导出超过了支持的条目上限。";
            if (!message) {
                NSError *error = nil;
                NSData *data = [text dataUsingEncoding:NSUTF8StringEncoding];
                if (![data writeToURL:destinationURL options:NSDataWritingAtomic error:&error]) message = error.localizedDescription ?: @"词典文件写入失败";
                else message = [NSString stringWithFormat:@"已导出至 %@。", destinationURL.lastPathComponent];
            }
            dispatch_async(dispatch_get_main_queue(), ^{
                MSIMEDictionaryWindowController *current = weakSelf;
                if (!current || ![current isCurrentDictionaryMutation:mutationGeneration kind:mutationKind offset:mutationOffset]) return;
                [current showMessage:message];
            });
        });
    }];
}
- (void)previousPage:(id)sender { (void)sender; if (_offset >= 100) { _offset -= 100; [self refresh:nil]; } }
- (void)nextPage:(id)sender { (void)sender; _offset += 100; [self refresh:nil]; }
@end
