#import "DictionaryCollectionsWindow.h"
#import "../core/BoundedFileReader.h"
#import "../core/WindowPresentation.h"
#import "../core/EditionIdentity.h"
#include "../../../common/DictionaryCollectionsModel.h"
#include <array>
#include <set>

namespace {
using Json = nlohmann::json;
NSString *text(const std::string &value) { return [[NSString alloc] initWithBytes:value.data() length:value.size() encoding:NSUTF8StringEncoding] ?: @""; }
std::string bytes(NSString *value) { return value.UTF8String ?: ""; }
const std::array<const char *, 7> kinds{{"pinyin", "wubi", "wubi98", "wubixsj", "english", "quickPhrase", "cantonese"}};
const std::array<const char *, 5> formats{{"standard", "windows", "rime", "hans", "scel"}};
}
@interface MSIMEDictionaryCollectionsWindow () <NSTableViewDataSource, NSTableViewDelegate>
@end
@implementation MSIMEDictionaryCollectionsWindow {
    std::unique_ptr<msime::dictionary_collections::Model> _model;
    NSTableView *_table;
    NSTextField *_name, *_status;
    NSPopUpButton *_kind, *_format, *_target;
    NSArray<NSButton *> *_buttons;
    NSMutableArray<NSString *> *_rows, *_targets, *_titles;
    NSTimer *_timer;
}
+ (void)showWithOptions:(NSDictionary *)options {
    NSData *data = [NSJSONSerialization dataWithJSONObject:options options:0 error:nil];
    if (!data) return;
    static MSIMEDictionaryCollectionsWindow *controller;
    if (!controller) controller = [[self alloc] initWithWindow:nil];
    if (controller->_model && controller->_model->busy()) { [controller showWindow:nil]; return; }
    try { controller->_model = std::make_unique<msime::dictionary_collections::Model>(Json::parse(std::string(static_cast<const char *>(data.bytes), data.length))); }
    catch (...) { return; }
    if (!controller.window) [controller loadWindow];
    [controller showWindow:nil]; MSIMEPresentWindow(controller.window);
    [controller update:YES];
    if (!controller->_timer) {
        __weak MSIMEDictionaryCollectionsWindow *weak = controller;
        controller->_timer = [NSTimer scheduledTimerWithTimeInterval:0.1 repeats:YES block:^(NSTimer *) {
            MSIMEDictionaryCollectionsWindow *strong = weak;
            if (!strong || !strong->_model) return;
            try {
                if (strong->_model->busy())
                    [NSNotificationCenter.defaultCenter postNotificationName:MSIMEEditionNotificationName(@"MSIMEDictionaryMaintenanceWillBeginNotification") object:nil];
                if (strong->_model->poll()) [strong update:YES];
            } catch (...) { strong->_status.stringValue = @"操作失败，请重试"; }
        }];
    }
}
- (void)loadWindow {
    self.window = [[NSWindow alloc] initWithContentRect:NSMakeRect(0, 0, 730, 550)
        styleMask:NSWindowStyleMaskTitled | NSWindowStyleMaskClosable | NSWindowStyleMaskResizable
        backing:NSBackingStoreBuffered defer:NO];
    self.window.title = @"词库分组"; self.window.minSize = NSMakeSize(690, 480);
    _rows = [NSMutableArray new]; _titles = [NSMutableArray new]; _targets = [NSMutableArray new];
    _table = [[NSTableView alloc] initWithFrame:NSZeroRect];
    NSTableColumn *column = [[NSTableColumn alloc] initWithIdentifier:@"group"];
    column.width = 680; column.resizingMask = NSTableColumnAutoresizingMask;
    _table.columnAutoresizingStyle = NSTableViewLastColumnOnlyAutoresizingStyle;
    [_table addTableColumn:column]; _table.headerView = nil; _table.rowHeight = 38;
    _table.allowsMultipleSelection = YES; _table.dataSource = self; _table.delegate = self;
    [_table setAccessibilityLabel:@"词库分组"];
    NSScrollView *scroll = [[NSScrollView alloc] initWithFrame:NSZeroRect];
    scroll.documentView = _table; scroll.hasVerticalScroller = YES;
    _name = [NSTextField textFieldWithString:@""]; _name.placeholderString = @"导入时默认使用文件名";
    [_name setAccessibilityLabel:@"分组名称"];
    _kind = [[NSPopUpButton alloc] initWithFrame:NSZeroRect];
    [_kind addItemsWithTitles:@[@"拼音", @"86 五笔", @"98 五笔", @"新世纪五笔", @"英文", @"快捷短语", @"粤语"]];
    _kind.target = self; _kind.action = @selector(kindChanged:); [_kind setAccessibilityLabel:@"词库类型"];
    _format = [[NSPopUpButton alloc] initWithFrame:NSZeroRect];
    [_format addItemsWithTitles:@[@"词在前（TSV）", @"编码在前（TSV）", @"Rime", @"汉字词表", @"搜狗细胞词库"]];
    [_format setAccessibilityLabel:@"文件格式"];
    _target = [[NSPopUpButton alloc] initWithFrame:NSZeroRect]; [_target setAccessibilityLabel:@"导入到"];
    NSGridView *fields = [NSGridView gridViewWithViews:@[
        @[[NSTextField labelWithString:@"分组名称"], _name],
        @[[NSTextField labelWithString:@"词库类型"], _kind],
        @[[NSTextField labelWithString:@"文件格式"], _format],
        @[[NSTextField labelWithString:@"导入到"], _target]]];
    fields.rowSpacing = 8; fields.columnSpacing = 12;
    NSMutableArray<NSButton *> *buttons = [NSMutableArray new];
    for (NSArray<NSString *> *entry in @[@[@"新建", @"createGroup:"], @[@"改名", @"renameGroup:"], @[@"导入…", @"importFile:"],
        @[@"启用", @"enableGroups:"], @[@"停用", @"disableGroups:"], @[@"删除…", @"deleteGroups:"], @[@"刷新", @"refresh:"]])
        [buttons addObject:[NSButton buttonWithTitle:entry[0] target:self action:NSSelectorFromString(entry[1])]];
    _buttons = buttons;
    NSStackView *actions = [NSStackView stackViewWithViews:buttons]; actions.spacing = 8;
    _status = [NSTextField wrappingLabelWithString:@"正在读取…"];
    NSStackView *root = [NSStackView stackViewWithViews:@[scroll, fields, actions, _status]];
    root.orientation = NSUserInterfaceLayoutOrientationVertical; root.spacing = 14; root.alignment = NSLayoutAttributeLeading;
    root.translatesAutoresizingMaskIntoConstraints = NO; [self.window.contentView addSubview:root];
    [NSLayoutConstraint activateConstraints:@[
        [root.leadingAnchor constraintEqualToAnchor:self.window.contentView.leadingAnchor constant:20],
        [root.trailingAnchor constraintEqualToAnchor:self.window.contentView.trailingAnchor constant:-20],
        [root.topAnchor constraintEqualToAnchor:self.window.contentView.topAnchor constant:20],
        [root.bottomAnchor constraintEqualToAnchor:self.window.contentView.bottomAnchor constant:-20],
        [scroll.widthAnchor constraintEqualToAnchor:root.widthAnchor], [scroll.heightAnchor constraintGreaterThanOrEqualToConstant:200],
        [fields.widthAnchor constraintEqualToAnchor:root.widthAnchor], [_status.widthAnchor constraintEqualToAnchor:root.widthAnchor]]];
    [self.window center];
}
- (std::vector<std::string>)selected {
    std::vector<std::string> ids;
    NSUInteger index = _table.selectedRowIndexes.firstIndex;
    while (index != NSNotFound) {
        if (index < _rows.count) ids.push_back(bytes(_rows[index]));
        index = [_table.selectedRowIndexes indexGreaterThanIndex:index];
    }
    return ids;
}
- (NSInteger)numberOfRowsInTableView:(NSTableView *)tableView { (void)tableView; return _titles.count; }
- (NSView *)tableView:(NSTableView *)tableView viewForTableColumn:(NSTableColumn *)column row:(NSInteger)row {
    (void)tableView; (void)column;
    return [NSTextField labelWithString:row >= 0 && static_cast<NSUInteger>(row) < _titles.count ? _titles[static_cast<NSUInteger>(row)] : @""];
}
- (void)update:(BOOL)rebuild {
    if (!_model) return;
    if (rebuild) {
        const auto selected = [self selected]; const std::set<std::string> preserved(selected.begin(), selected.end());
        [_rows removeAllObjects]; [_titles removeAllObjects]; [_targets removeAllObjects];
        [_target removeAllItems]; [_target addItemWithTitle:@"新建分组"]; [_targets addObject:@""];
        NSMutableIndexSet *selection = [NSMutableIndexSet new];
        for (const auto *source : {"user", "import", "community"}) for (const auto &item : _model->view.at("collections")) {
            if (item.at("source").at("type") != source) continue;
            const auto id = item.at("id").get<std::string>();
            if (preserved.count(id)) [selection addIndex:_rows.count];
            [_rows addObject:text(id)];
            [_titles addObject:[NSString stringWithFormat:@"%@ · %@ · %u 条 · %@",
                text(msime::dictionary_collections::Model::source_title(source)), text(item.at("name").get<std::string>()),
                item.at("entry_count").get<unsigned>(), item.at("enabled").get<bool>() ? @"已启用" : @"已停用"]];
            if (item.at("kind") == kinds.at(static_cast<size_t>(_kind.indexOfSelectedItem)) && std::string(source) != "community") {
                [_target addItemWithTitle:text(item.at("name").get<std::string>())]; [_targets addObject:text(id)];
            }
        }
        [_table reloadData]; [_table selectRowIndexes:selection byExtendingSelection:NO];
    }
    const BOOL enabled = !_model->busy();
    for (NSControl *control in _buttons) control.enabled = enabled;
    _name.enabled = enabled; _kind.enabled = enabled; _format.enabled = enabled; _target.enabled = enabled;
    NSString *message = enabled ? text(msime::dictionary_collections::Model::message(_model->error)) : @"正在处理…";
    if (enabled && _model->error.empty() && _model->report.is_object()) {
        message = [NSString stringWithFormat:@"导入 %u 条，跳过重复 %u 条，失败 %u 条%@",
            _model->report.value("imported", 0u), _model->report.value("duplicates", 0u), _model->report.value("failed", 0u),
            _model->report.value("truncated", false) ? @"；分组已满" : @""];
    }
    _status.stringValue = message;
}
- (void)perform:(Json)action { _model->perform(std::move(action)); [self update:NO]; }
- (void)kindChanged:(id)sender { (void)sender; [self update:YES]; }
- (void)refresh:(id)sender { (void)sender; [self perform:{{"operation", "load"}}]; }
- (void)createGroup:(id)sender { (void)sender; [self perform:{{"operation", "create"}, {"name", bytes(_name.stringValue)}, {"kind", kinds.at(static_cast<size_t>(_kind.indexOfSelectedItem))}}]; }
- (void)renameGroup:(id)sender {
    (void)sender; const auto ids = [self selected];
    if (ids.size() != 1) { _status.stringValue = @"改名时请选择一个词库"; return; }
    [self perform:{{"operation", "rename"}, {"id", ids[0]}, {"name", bytes(_name.stringValue)}}];
}
- (void)enableGroups:(id)sender { (void)sender; [self setGroupsEnabled:YES]; }
- (void)disableGroups:(id)sender { (void)sender; [self setGroupsEnabled:NO]; }
- (void)setGroupsEnabled:(BOOL)enabled {
    const auto ids = [self selected];
    if (ids.empty()) { _status.stringValue = @"请先选择词库"; return; }
    [self perform:{{"operation", "set_enabled_many"}, {"ids", ids}, {"enabled", static_cast<bool>(enabled)}}];
}
- (void)deleteGroups:(id)sender {
    (void)sender; const auto ids = [self selected]; if (ids.empty()) { _status.stringValue = @"请先选择词库"; return; }
    NSAlert *alert = [NSAlert new]; alert.messageText = @"删除所选词库？";
    alert.informativeText = @"删除分组及其词条；其他已启用分组和原有个人词条会保留。";
    [alert addButtonWithTitle:@"取消"]; [alert addButtonWithTitle:@"删除"];
    if ([alert runModal] == NSAlertSecondButtonReturn) [self perform:{{"operation", "delete_many"}, {"ids", ids}}];
}
- (void)importFile:(id)sender {
    (void)sender; NSOpenPanel *panel = [NSOpenPanel openPanel]; panel.canChooseDirectories = NO; panel.allowsMultipleSelection = NO;
    if ([panel runModal] != NSModalResponseOK || !panel.URL) return;
    const auto format = formats.at(static_cast<size_t>(_format.indexOfSelectedItem));
    NSData *data = MSIMEReadFileUpTo(panel.URL, std::string(format) == "scel" ? 8 * 1024 * 1024 : 16 * 1024 * 1024, nil);
    if (!data) { _status.stringValue = @"文件无法读取或超出容量限制"; return; }
    NSString *name = _name.stringValue.length ? _name.stringValue : panel.URL.URLByDeletingPathExtension.lastPathComponent;
    Json action{{"operation", "import"}, {"name", bytes(name)}, {"kind", kinds.at(static_cast<size_t>(_kind.indexOfSelectedItem))}, {"format", format}};
    const auto target = static_cast<NSUInteger>(_target.indexOfSelectedItem);
    if (target < _targets.count && _targets[target].length) action["id"] = bytes(_targets[target]);
    if (std::string(format) == "scel") action["bytes_base64"] = bytes([data base64EncodedStringWithOptions:0]);
    else {
        NSString *contents = [[NSString alloc] initWithData:data encoding:NSUTF8StringEncoding];
        if (!contents) { _status.stringValue = @"请选择 UTF-8 编码的词库文件"; return; }
        action["text"] = bytes(contents);
    }
    [self perform:std::move(action)];
}
@end
