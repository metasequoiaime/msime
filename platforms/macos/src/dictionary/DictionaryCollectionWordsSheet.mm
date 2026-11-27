#import "DictionaryCollectionWordsSheet.h"

@implementation MSIMEDictionaryCollectionWordsSheet {
    MSIMECollectionWordsPageRequest _request;
    MSIMECollectionWordsRemoval _removal;
    nlohmann::json _entries;
    NSUInteger _page, _pageCount;
    BOOL _busy;
    NSTableView *_table;
    NSTextField *_status;
    NSButton *_previous, *_next, *_remove;
}

- (instancetype)initWithPageRequest:(MSIMECollectionWordsPageRequest)request
                            removal:(MSIMECollectionWordsRemoval)removal {
    self = [super initWithWindow:nil];
    if (self) {
        _request = [request copy];
        _removal = [removal copy];
        _entries = nlohmann::json::array();
        [self loadWindow];
    }
    return self;
}

- (void)loadWindow {
    self.window = [[NSWindow alloc] initWithContentRect:NSMakeRect(0, 0, 570, 375)
        styleMask:NSWindowStyleMaskTitled backing:NSBackingStoreBuffered defer:NO];
    self.window.title = @"分组词条";
    _table = [[NSTableView alloc] initWithFrame:NSZeroRect];
    for (NSArray<NSString *> *field in @[@[@"value", @"词条"], @[@"key", @"编码"]]) {
        NSTableColumn *column = [[NSTableColumn alloc] initWithIdentifier:field[0]];
        column.title = field[1];
        column.width = [field[0] isEqualToString:@"value"] ? 330 : 170;
        [_table addTableColumn:column];
    }
    _table.columnAutoresizingStyle = NSTableViewLastColumnOnlyAutoresizingStyle;
    _table.dataSource = self;
    _table.delegate = self;
    _table.allowsMultipleSelection = YES;
    _table.usesAlternatingRowBackgroundColors = YES;
    [_table setAccessibilityLabel:@"分组词条"];
    NSScrollView *scroll = [[NSScrollView alloc] initWithFrame:NSZeroRect];
    scroll.hasVerticalScroller = YES;
    scroll.documentView = _table;
    _previous = [NSButton buttonWithTitle:@"上一页" target:self action:@selector(previous:)];
    _next = [NSButton buttonWithTitle:@"下一页" target:self action:@selector(next:)];
    _remove = [NSButton buttonWithTitle:@"删除所选…" target:self action:@selector(remove:)];
    NSButton *close = [NSButton buttonWithTitle:@"完成" target:self action:@selector(closeSheet:)];
    close.keyEquivalent = @"\r";
    NSStackView *actions = [NSStackView stackViewWithViews:@[_previous, _next, _remove, close]];
    _status = [NSTextField labelWithString:@""];
    NSStackView *body = [NSStackView stackViewWithViews:@[scroll, _status, actions]];
    body.orientation = NSUserInterfaceLayoutOrientationVertical;
    body.alignment = NSLayoutAttributeLeading;
    body.spacing = 12;
    body.translatesAutoresizingMaskIntoConstraints = NO;
    NSView *content = self.window.contentView;
    [content addSubview:body];
    [NSLayoutConstraint activateConstraints:@[
        [body.leadingAnchor constraintEqualToAnchor:content.leadingAnchor constant:20],
        [body.trailingAnchor constraintEqualToAnchor:content.trailingAnchor constant:-20],
        [body.topAnchor constraintEqualToAnchor:content.topAnchor constant:20],
        [body.bottomAnchor constraintEqualToAnchor:content.bottomAnchor constant:-20],
        [scroll.widthAnchor constraintEqualToAnchor:body.widthAnchor],
        [scroll.heightAnchor constraintGreaterThanOrEqualToConstant:260],
        [_status.widthAnchor constraintEqualToAnchor:body.widthAnchor]]];
    [self updateButtons];
}

- (void)showPage:(const nlohmann::json &)page {
    _entries = page.at("entries");
    _page = page.at("page").get<NSUInteger>();
    _pageCount = page.at("page_count").get<NSUInteger>();
    const auto name = page.at("name").get<std::string>();
    self.window.title = [NSString stringWithUTF8String:name.c_str()] ?: @"分组词条";
    [_table deselectAll:nil];
    [_table reloadData];
    if (!_entries.empty()) [_table scrollRowToVisible:0];
    [self setBusy:NO message:_pageCount
        ? [NSString stringWithFormat:@"第 %lu 页，共 %lu 页", _page + 1, _pageCount]
        : @"此分组没有词条"];
}

- (void)setBusy:(BOOL)busy message:(NSString *)message {
    _busy = busy;
    _status.stringValue = message;
    [self updateButtons];
}

- (void)updateButtons {
    _previous.enabled = !_busy && _page > 0;
    _next.enabled = !_busy && _page + 1 < _pageCount;
    _remove.enabled = !_busy && _table.numberOfSelectedRows > 0;
}
- (NSInteger)numberOfRowsInTableView:(NSTableView *)tableView {
    (void)tableView;
    return static_cast<NSInteger>(_entries.size());
}
- (NSView *)tableView:(NSTableView *)tableView viewForTableColumn:(NSTableColumn *)column row:(NSInteger)row {
    (void)tableView;
    const auto &value = _entries.at(static_cast<size_t>(row)).at(column.identifier.UTF8String).get_ref<const std::string &>();
    return [NSTextField labelWithString:[NSString stringWithUTF8String:value.c_str()] ?: @""];
}
- (void)tableViewSelectionDidChange:(NSNotification *)notification { (void)notification; [self updateButtons]; }
- (void)previous:(id)sender { (void)sender; if (_previous.enabled) _request(_page - 1); }
- (void)next:(id)sender { (void)sender; if (_next.enabled) _request(_page + 1); }
- (void)closeSheet:(id)sender { (void)sender; [self.window.sheetParent endSheet:self.window]; }
- (void)remove:(id)sender {
    (void)sender;
    if (!_remove.enabled) return;
    auto words = nlohmann::json::array();
    NSIndexSet *rows = _table.selectedRowIndexes;
    for (NSUInteger row = rows.firstIndex; row != NSNotFound; row = [rows indexGreaterThanIndex:row]) {
        if (row < _entries.size()) words.push_back(_entries[row]);
    }
    if (words.empty()) return;
    NSAlert *alert = [NSAlert new];
    alert.messageText = [NSString stringWithFormat:@"从此分组删除 %lu 条词条？", words.size()];
    alert.informativeText = @"其他分组与原有个人词条会保留。";
    [alert addButtonWithTitle:@"取消"];
    [alert addButtonWithTitle:@"删除"];
    if ([alert runModal] == NSAlertSecondButtonReturn) _removal(words);
}
@end
