#import "CloudClipboardWindowController.h"
#import "CloudClipboardClient.h"
@implementation MSIMECloudClipboardWindowController { NSString *_token; NSTextView *_editor; NSTextField *_search; NSTextView *_items; NSTextField *_status; NSArray *_entries; NSArray<NSValue *> *_entryRanges; NSUInteger _refreshGeneration; }
+ (instancetype)sharedController { static id c; static dispatch_once_t once; dispatch_once(&once, ^{ c=[self new]; }); return c; }
- (void)showWithToken:(NSString *)token { _token=[token copy]; if(!self.window){ self.window=[[NSWindow alloc] initWithContentRect:NSMakeRect(0,0,480,420) styleMask:(NSWindowStyleMaskTitled|NSWindowStyleMaskClosable) backing:NSBackingStoreBuffered defer:NO]; self.window.title=@"云剪贴板"; } _items=[[NSTextView alloc] initWithFrame:NSMakeRect(20,180,440,210)]; _items.editable=NO; _editor=[[NSTextView alloc] initWithFrame:NSMakeRect(20,100,440,70)]; _search=[[NSTextField alloc] initWithFrame:NSMakeRect(20,65,300,24)]; _search.placeholderString=@"搜索云端历史"; NSButton *r=[NSButton buttonWithTitle:@"刷新" target:self action:@selector(refresh:)]; r.frame=NSMakeRect(330,62,70,30); NSButton *d=[NSButton buttonWithTitle:@"删除选中条目" target:self action:@selector(deleteItem:)]; d.frame=NSMakeRect(330,25,120,32); NSButton *u=[NSButton buttonWithTitle:@"上传明确选择的文本" target:self action:@selector(upload:)]; u.frame=NSMakeRect(20,25,180,32); _status=[[NSTextField alloc] initWithFrame:NSMakeRect(20,0,440,24)]; _status.editable=NO; _status.bezeled=NO; _status.drawsBackground=NO; NSView *v=[NSView new]; for(NSView *x in @[_items,_editor,_search,r,d,u,_status]) [v addSubview:x]; self.window.contentView=v; [self.window center]; [self showWindow:nil]; [self refresh:nil]; }
- (void)deleteItem:(id)sender {
    (void)sender;
    NSRange selection = _items.selectedRange;
    if (selection.length > 0 && selection.location <= _items.string.length &&
        selection.length <= _items.string.length - selection.location) {
        for (NSUInteger index = 0; index < _entryRanges.count; ++index) {
            NSRange range = _entryRanges[index].rangeValue;
            if (selection.location < range.location || NSMaxRange(selection) > NSMaxRange(range)) continue;
            NSString *itemID = _entries[index][@"id"];
            NSString *token = [_token copy];
            NSUInteger generation = _refreshGeneration;
            MSIMERemoveCloudClipboard(itemID, token, ^(NSData *data, NSInteger status, NSError *error) {
                (void)data;
                if (generation != self->_refreshGeneration || ![token isEqualToString:self->_token]) return;
                if (!error && status >= 200 && status < 300) [self refresh:nil];
                else self->_status.stringValue = @"删除失败";
            });
            return;
        }
    }
    _status.stringValue = @"请仅选择一个条目中的文本";
}
- (void)upload:(id)sender {
    (void)sender;
    NSString *t = _editor.string;
    if (!t.length || t.length > 4000) return;
    NSString *token = [_token copy];
    NSUInteger generation = _refreshGeneration;
    MSIMEAddCloudClipboard(t, token, ^(NSData *data, NSInteger status, NSError *error) {
        (void)data;
        if (generation != self->_refreshGeneration || ![token isEqualToString:self->_token]) return;
        if (!error && status >= 200 && status < 300) [self refresh:nil];
        else self->_status.stringValue = @"上传失败";
    });
}
- (void)refresh:(id)sender {
    (void)sender;
    NSUInteger generation = ++_refreshGeneration;
    _entries = @[];
    _entryRanges = @[];
    _items.string = @"";
    MSIMEFetchCloudClipboard(_search.stringValue ?: @"", _token, ^(NSData *data, NSInteger status, NSError *error) {
        if (generation != self->_refreshGeneration) return;
        id response = data && !error && status >= 200 && status < 300
            ? [NSJSONSerialization JSONObjectWithData:data options:0 error:nil] : nil;
        if (![response isKindOfClass:NSDictionary.class] || ![response[@"items"] isKindOfClass:NSArray.class]) {
            self->_status.stringValue = @"刷新失败";
            return;
        }
        NSMutableArray *entries = [NSMutableArray array];
        NSMutableArray *ranges = [NSMutableArray array];
        NSMutableString *text = [NSMutableString string];
        for (id entry in response[@"items"]) {
            if (![entry isKindOfClass:NSDictionary.class] ||
                ![entry[@"id"] isKindOfClass:NSString.class] || ![entry[@"id"] length] ||
                ![entry[@"text"] isKindOfClass:NSString.class] || ![entry[@"text"] length]) continue;
            [entries addObject:entry];
            [ranges addObject:[NSValue valueWithRange:NSMakeRange(text.length, [entry[@"text"] length])]];
            [text appendFormat:@"%@\n\n", entry[@"text"]];
        }
        self->_entries = entries;
        self->_entryRanges = ranges;
        self->_items.string = text.length ? text : @"暂无云端历史";
        self->_items.selectedRange = NSMakeRange(0, 0);
        self->_status.stringValue = @"已刷新";
    });
}
@end
