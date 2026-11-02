#import "InputModeStatusItem.h"

#import "InputModeIdentifiers.h"

// 固定的 autosaveName 让系统记住用户按住 ⌘ 拖到的位置，隐藏再显示、重启输入法进程后都回到原处。
static NSString *const MSIMEInputModeStatusItemAutosaveName = @"MSIMEInputModeStatusItem";

// 模式字形是为 16 点菜单栏槽位画的，四周各留约 2 点空白。状态项把它居中放进 18 点的画布，外沿描一圈圆角框：框与字形之间仍有一点多的空隙，框本身让它与旁边系统输入源那个光秃的同一字形区分开（#8548），字形则随模式变化（#10630）。
static const CGFloat MSIMEInputModeStatusItemSide = 18;
static const CGFloat MSIMEInputModeStatusItemGlyphSide = 16;
static const CGFloat MSIMEInputModeStatusItemFrameWidth = 1.25;
static const CGFloat MSIMEInputModeStatusItemFrameRadius = 4;

NSImage *MSIMEInputModeStatusItemImage(NSDictionary *info, NSString *modeID, NSBundle *bundle) {
    // 与系统输入法菜单取同一张字形；本版本没有声明这个模式时（五笔版把五笔放在 .Hans 上）取主模式的。
    NSImage *glyph = [bundle imageForResource:MSIMEInputModeMenuIconFileIn(info, modeID)];
    if (!glyph) {
        // 模式图标缺失时退回产品 logo，不加框；复制一份再标成模板图，不改动 bundle 缓存里的那张。
        NSImage *logo = [[bundle imageForResource:@"MSIMEClientInputMethodMenuIcon.tiff"] copy];
        [logo setTemplate:YES];
        return logo;
    }
    const CGFloat side = MSIMEInputModeStatusItemSide;
    NSImage *image = [NSImage imageWithSize:NSMakeSize(side, side) flipped:NO drawingHandler:^BOOL(NSRect bounds) {
        const CGFloat inset = MSIMEInputModeStatusItemFrameWidth / 2;
        NSBezierPath *frame = [NSBezierPath bezierPathWithRoundedRect:NSInsetRect(bounds, inset, inset) xRadius:MSIMEInputModeStatusItemFrameRadius yRadius:MSIMEInputModeStatusItemFrameRadius];
        frame.lineWidth = MSIMEInputModeStatusItemFrameWidth;
        [NSColor.blackColor setStroke];
        [frame stroke];
        const CGFloat offset = (side - MSIMEInputModeStatusItemGlyphSide) / 2;
        [glyph drawInRect:NSMakeRect(offset, offset, MSIMEInputModeStatusItemGlyphSide, MSIMEInputModeStatusItemGlyphSide) fromRect:NSZeroRect operation:NSCompositingOperationSourceOver fraction:1];
        return YES;
    }];
    [image setTemplate:YES];
    return image;
}

@implementation MSIMEInputModeStatusItem {
    NSStatusItem *_item;
    NSString *_shownMode;
}

+ (instancetype)sharedItem {
    static MSIMEInputModeStatusItem *item;
    static dispatch_once_t once;
    dispatch_once(&once, ^{ item = [[self alloc] init]; });
    return item;
}

- (NSString *)shownMode { return _shownMode; }

- (void)showMode:(NSString *)modeID {
    if (!modeID.length || (_item && [modeID isEqualToString:_shownMode])) return;
    NSDictionary *info = MSIMEEditionInfo();
    // 本版本没有声明这个模式时（五笔版把五笔的名字放在 .Hans 上），名称取主模式的，与系统输入法菜单显示的一致。
    NSString *declared = MSIMEInputModeDeclaredIn(info, modeID) ? modeID : MSIMEChineseInputModeID;
    NSImage *image = MSIMEInputModeStatusItemImage(info, modeID, NSBundle.mainBundle);
    if (!_item) {
        _item = [NSStatusBar.systemStatusBar statusItemWithLength:NSSquareStatusItemLength];
        _item.autosaveName = MSIMEInputModeStatusItemAutosaveName;
    }
    NSString *name = MSIMEInputModeMenuName(declared);
    _item.button.image = image;
    _item.button.toolTip = name;
    _item.button.accessibilityLabel = name;
    _shownMode = [modeID copy];
}

- (void)hide {
    if (!_item) return;
    // 整个移除而不是设 visible = NO：macOS 27.0.1 上实测把带 autosaveName 的状态项设成不可见会删掉记下的 `NSStatusItem Preferred Position <名字>`，removeStatusItem: 则保留它，下次用同一个名字新建时回到原处。
    [NSStatusBar.systemStatusBar removeStatusItem:_item];
    _item = nil;
    _shownMode = nil;
}

@end
