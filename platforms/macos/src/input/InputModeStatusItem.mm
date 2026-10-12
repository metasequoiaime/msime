#import "InputModeStatusItem.h"

#import "InputModeIdentifiers.h"

// 固定的 autosaveName 让系统记住用户按住 ⌘ 拖到的位置，隐藏再显示、重启输入法进程后都回到原处。
static NSString *const MSIMEInputModeStatusItemAutosaveName = @"MSIMEInputModeStatusItem";

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
    // 本版本没有声明这个模式时（五笔版把五笔的名字和图标放在 .Hans 上），名字和图标都取主模式的，与系统输入法菜单显示的一致。
    NSString *declared = MSIMEInputModeDeclaredIn(info, modeID) ? modeID : MSIMEChineseInputModeID;
    // imageForResource: 返回按名字缓存的共享图，复制一份再标成模板图，不改动缓存里的那张。
    NSImage *image = [[NSBundle.mainBundle imageForResource:MSIMEInputModeMenuIconFileIn(info, declared)] copy];
    [image setTemplate:YES];
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
