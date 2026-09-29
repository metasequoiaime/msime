#import "ScreenKeyboardPanel.h"
#import "WindowPresentationLog.h"
#import <Carbon/Carbon.h>
#import <CoreGraphics/CoreGraphics.h>
#include <algorithm>
#include <vector>
#include "ScreenKeyboardTargetPolicy.h"

namespace {
NSColor *KeyboardColor(NSAppearance *appearance, unsigned light, unsigned dark) {
    NSString *match = [appearance bestMatchFromAppearancesWithNames:@[NSAppearanceNameAqua, NSAppearanceNameDarkAqua]];
    const unsigned rgb = [match isEqualToString:NSAppearanceNameDarkAqua] ? dark : light;
    return [NSColor colorWithSRGBRed:((rgb >> 16) & 255) / 255.0 green:((rgb >> 8) & 255) / 255.0 blue:(rgb & 255) / 255.0 alpha:1];
}
struct Key {
    const char *normal;
    const char *shifted;
    double weight;
    unsigned short code;
    NSEventModifierFlags modifier = 0;
};
// Windows baseline: 04a8df56f86312474a069f4335a1b58da7afaa9e,
// server/src/keyboard-panel/KeyboardPanel.cpp. macOS uses ANSI hardware key codes.
const std::vector<std::vector<Key>> &Rows() {
    static const std::vector<std::vector<Key>> rows = {
        {{"`","~",1,kVK_ANSI_Grave},{"1","!",1,kVK_ANSI_1},{"2","@",1,kVK_ANSI_2},
         {"3","#",1,kVK_ANSI_3},{"4","$",1,kVK_ANSI_4},{"5","%",1,kVK_ANSI_5},
         {"6","^",1,kVK_ANSI_6},{"7","&",1,kVK_ANSI_7},{"8","*",1,kVK_ANSI_8},
         {"9","(",1,kVK_ANSI_9},{"0",")",1,kVK_ANSI_0},{"-","_",1,kVK_ANSI_Minus},
         {"=","+",1,kVK_ANSI_Equal},{"Backspace","",1.9,kVK_Delete}},
        {{"Tab","",1.5,kVK_Tab},{"q","Q",1,kVK_ANSI_Q},{"w","W",1,kVK_ANSI_W},
         {"e","E",1,kVK_ANSI_E},{"r","R",1,kVK_ANSI_R},{"t","T",1,kVK_ANSI_T},
         {"y","Y",1,kVK_ANSI_Y},{"u","U",1,kVK_ANSI_U},{"i","I",1,kVK_ANSI_I},
         {"o","O",1,kVK_ANSI_O},{"p","P",1,kVK_ANSI_P},{"[","{",1,kVK_ANSI_LeftBracket},
         {"]","}",1,kVK_ANSI_RightBracket},{"\\","|",1.4,kVK_ANSI_Backslash}},
        {{"Caps Lock","",1.85,kVK_CapsLock,NSEventModifierFlagCapsLock},
         {"a","A",1,kVK_ANSI_A},{"s","S",1,kVK_ANSI_S},{"d","D",1,kVK_ANSI_D},
         {"f","F",1,kVK_ANSI_F},{"g","G",1,kVK_ANSI_G},{"h","H",1,kVK_ANSI_H},
         {"j","J",1,kVK_ANSI_J},{"k","K",1,kVK_ANSI_K},{"l","L",1,kVK_ANSI_L},
         {";",":",1,kVK_ANSI_Semicolon},{"'","\"",1,kVK_ANSI_Quote},{"Enter","",2,kVK_Return}},
        {{"Shift","",2.35,kVK_Shift,NSEventModifierFlagShift},
         {"z","Z",1,kVK_ANSI_Z},{"x","X",1,kVK_ANSI_X},{"c","C",1,kVK_ANSI_C},
         {"v","V",1,kVK_ANSI_V},{"b","B",1,kVK_ANSI_B},{"n","N",1,kVK_ANSI_N},
         {"m","M",1,kVK_ANSI_M},{",","<",1,kVK_ANSI_Comma},{".",">",1,kVK_ANSI_Period},
         {"/","?",1,kVK_ANSI_Slash},{"Shift","",2.15,kVK_Shift,NSEventModifierFlagShift}},
        {{"Ctrl","",1.25,kVK_Control,NSEventModifierFlagControl},
         {"Command","",1.25,kVK_Command,NSEventModifierFlagCommand},
         {"Option","",1.25,kVK_Option,NSEventModifierFlagOption},{"Space"," ",6.7,kVK_Space},
         {"Option","",1.25,kVK_Option,NSEventModifierFlagOption},
         {"Command","",1.25,kVK_Command,NSEventModifierFlagCommand},{"Del","",1.25,kVK_ForwardDelete},
         {"Ctrl","",1.25,kVK_Control,NSEventModifierFlagControl}}
    };
    return rows;
}
bool Letter(const Key &key) { return key.normal[0] >= 'a' && key.normal[0] <= 'z' && key.normal[1] == '\0'; }
bool CommitKey(const Key &key) {
    return (key.normal[0] >= '0' && key.normal[0] <= '9' && key.normal[1] == '\0') ||
        key.code == kVK_Space || key.code == kVK_Return || key.code == kVK_Tab ||
        key.code == kVK_Delete || key.code == kVK_ForwardDelete;
}
bool RepeatableKey(const Key &key) { return key.modifier == 0; }
BOOL PostKey(unsigned short code, NSEventModifierFlags flags, pid_t targetPID) {
    (void)targetPID;
    // The panel is a non-activating utility surface. Never request permission
    // from a key click: the prompt can change focus and the user must opt in
    // through the host's normal Accessibility settings flow instead.
    if (!CGPreflightPostEventAccess()) return NO;
    // Resolve the destination for every stroke. The panel is non-activating,
    // so the user may switch editors while it remains visible; a cached PID
    // from showKeyboard must never send a later key to the old editor.
    const pid_t ownProcess = NSProcessInfo.processInfo.processIdentifier;
    const pid_t foreground = NSWorkspace.sharedWorkspace.frontmostApplication.processIdentifier;
    const pid_t destination = msime::mac::LiveScreenKeyboardTarget(foreground, ownProcess);
    NSRunningApplication *target = destination > 0
        ? [NSRunningApplication runningApplicationWithProcessIdentifier:destination]
        : nil;
    if (!target || target.terminated) return NO;
    CGEventRef down = CGEventCreateKeyboardEvent(nullptr, code, true);
    CGEventRef up = CGEventCreateKeyboardEvent(nullptr, code, false);
    if (!down || !up) {
        if (down) CFRelease(down);
        if (up) CFRelease(up);
        return NO;
    }
    CGEventFlags eventFlags = 0;
    if (flags & NSEventModifierFlagShift) eventFlags |= kCGEventFlagMaskShift;
    if (flags & NSEventModifierFlagControl) eventFlags |= kCGEventFlagMaskControl;
    if (flags & NSEventModifierFlagOption) eventFlags |= kCGEventFlagMaskAlternate;
    if (flags & NSEventModifierFlagCommand) eventFlags |= kCGEventFlagMaskCommand;
    CGEventSetFlags(down, eventFlags);
    CGEventSetFlags(up, eventFlags);
    // Do not deliver a pending click after a focus change while events were
    // being allocated.
    const pid_t currentForeground = NSWorkspace.sharedWorkspace.frontmostApplication.processIdentifier;
    const BOOL valid = currentForeground == destination && !target.terminated;
    if (valid) {
        CGEventPostToPid(destination, down);
        CGEventPostToPid(destination, up);
    }
    CFRelease(down);
    CFRelease(up);
    return valid;
}
}

@interface MSIMEScreenKeyboardButton : NSButton
@property(nonatomic) BOOL keyboardHovered;
@property(nonatomic) BOOL closeButton;
@end
@implementation MSIMEScreenKeyboardButton {
    NSTrackingArea *_keyboardTracking;
}
- (void)updateTrackingAreas {
    [super updateTrackingAreas];
    if (_keyboardTracking) [self removeTrackingArea:_keyboardTracking];
    _keyboardTracking = [[NSTrackingArea alloc] initWithRect:NSZeroRect
        options:NSTrackingMouseEnteredAndExited | NSTrackingActiveAlways | NSTrackingInVisibleRect owner:self userInfo:nil];
    [self addTrackingArea:_keyboardTracking];
}
- (void)mouseEntered:(NSEvent *)event { (void)event; self.keyboardHovered = YES; self.needsDisplay = YES; }
- (void)mouseExited:(NSEvent *)event { (void)event; self.keyboardHovered = NO; self.needsDisplay = YES; }
- (void)viewDidChangeEffectiveAppearance { [super viewDidChangeEffectiveAppearance]; self.needsDisplay = YES; }
- (void)drawRect:(NSRect)dirtyRect {
    (void)dirtyRect;
    NSColor *fill = KeyboardColor(self.effectiveAppearance, 0xFFFFFF, 0x2B2D34);
    if (self.closeButton) fill = KeyboardColor(self.effectiveAppearance, 0xECEEF2, 0x17181D);
    if (self.keyboardHovered) fill = KeyboardColor(self.effectiveAppearance, 0xE1E4EA, 0x41434D);
    // A latched modifier takes a solid tint of the brand accent (#2C7A4B at 20% over white, #5FBF84 at 24% over the dark key), replacing the reference's lilac so the panel shares the global accent.
    if (!self.closeButton && self.state == NSControlStateValueOn) fill = KeyboardColor(self.effectiveAppearance, 0xD5E4DB, 0x375047);
    if (self.cell.isHighlighted) fill = KeyboardColor(self.effectiveAppearance, self.closeButton ? 0xE1E4EA : 0xC7C9D0, self.closeButton ? 0x41434D : 0x666A77);
    [fill setFill];
    [[NSBezierPath bezierPathWithRoundedRect:self.bounds xRadius:self.closeButton ? 4 : 5 yRadius:self.closeButton ? 4 : 5] fill];
    NSDictionary *attributes = @{NSFontAttributeName:self.font,
        NSForegroundColorAttributeName:KeyboardColor(self.effectiveAppearance, 0x202124, 0xF1F1F3)};
    NSSize size = [self.title sizeWithAttributes:attributes];
    [self.title drawAtPoint:NSMakePoint(NSMidX(self.bounds) - size.width / 2, NSMidY(self.bounds) - size.height / 2) withAttributes:attributes];
}
@end

@interface MSIMEScreenKeyboardContent : NSView
@property(nonatomic, copy) void (^layoutKeys)(NSSize);
@end
@implementation MSIMEScreenKeyboardContent
- (BOOL)isFlipped { return YES; }
- (void)layout { [super layout]; if (self.layoutKeys) self.layoutKeys(self.bounds.size); }
- (void)updateHeaderColors {
    for (NSView *view in self.subviews) if ([view isKindOfClass:NSTextField.class])
        ((NSTextField *)view).textColor = KeyboardColor(self.effectiveAppearance, 0x555861, 0xD7D8DD);
}
- (void)didAddSubview:(NSView *)subview { [super didAddSubview:subview]; [self updateHeaderColors]; }
- (void)viewDidChangeEffectiveAppearance { [super viewDidChangeEffectiveAppearance]; [self updateHeaderColors]; self.needsDisplay = YES; }
- (void)drawRect:(NSRect)dirtyRect {
    (void)dirtyRect;
    [KeyboardColor(self.effectiveAppearance, 0xECEEF2, 0x17181D) setFill];
    [[NSBezierPath bezierPathWithRoundedRect:self.bounds xRadius:8 yRadius:8] fill];
}
@end

@implementation MSIMEScreenKeyboardPanel {
    MSIMEScreenKeyboardSender _sender;
    MSIMEScreenKeyboardTargetProvider _targetProvider;
    pid_t _inputTargetPID;
    NSMutableArray<NSButton *> *_buttons;
    std::vector<Key> _keys;
    NSEventModifierFlags _modifiers;
    NSTextField *_status;
}
+ (instancetype)sharedPanel {
    static MSIMEScreenKeyboardPanel *panel;
    static dispatch_once_t once;
    dispatch_once(&once, ^{ panel = [[self alloc] init]; });
    return panel;
}
- (instancetype)init {
    self = [self initWithKeySender:nil targetProvider:^pid_t {
        NSRunningApplication *frontmost = NSWorkspace.sharedWorkspace.frontmostApplication;
        return frontmost ? frontmost.processIdentifier : 0;
    }];
    if (self) {
        __weak MSIMEScreenKeyboardPanel *weakSelf = self;
        _sender = ^BOOL(unsigned short code, NSEventModifierFlags flags) {
            MSIMEScreenKeyboardPanel *strongSelf = weakSelf;
            return strongSelf ? PostKey(code, flags, strongSelf->_inputTargetPID) : NO;
        };
    }
    return self;
}
- (instancetype)initWithKeySender:(MSIMEScreenKeyboardSender)sender {
    return [self initWithKeySender:sender targetProvider:^pid_t {
        NSRunningApplication *frontmost = NSWorkspace.sharedWorkspace.frontmostApplication;
        return frontmost ? frontmost.processIdentifier : 0;
    }];
}
- (instancetype)initWithKeySender:(MSIMEScreenKeyboardSender)sender
                    targetProvider:(MSIMEScreenKeyboardTargetProvider)targetProvider {
    self = [super initWithContentRect:NSMakeRect(0, 0, 1100, 400)
        styleMask:NSWindowStyleMaskBorderless | NSWindowStyleMaskNonactivatingPanel backing:NSBackingStoreBuffered defer:NO];
    if (!self) return nil;
    _sender = [sender copy];
    _targetProvider = [targetProvider copy];
    _inputTargetPID = 0;
    _buttons = [NSMutableArray new];
    self.releasedWhenClosed = NO;
    self.opaque = NO;
    self.backgroundColor = NSColor.clearColor;
    self.level = NSFloatingWindowLevel;
    self.hidesOnDeactivate = NO;
    self.movableByWindowBackground = YES;
    self.collectionBehavior = NSWindowCollectionBehaviorCanJoinAllSpaces | NSWindowCollectionBehaviorFullScreenAuxiliary;
    MSIMEScreenKeyboardContent *content = [[MSIMEScreenKeyboardContent alloc] initWithFrame:NSMakeRect(0, 0, 1100, 400)];
    content.wantsLayer = YES;
    self.contentView = content;
    _status = [NSTextField labelWithString:@"水杉屏幕键盘"];
    _status.font = [NSFont systemFontOfSize:12];
    _status.frame = NSMakeRect(10, 4, 950, 20);
    _status.autoresizingMask = NSViewWidthSizable;
    [content addSubview:_status];
    MSIMEScreenKeyboardButton *close = [MSIMEScreenKeyboardButton buttonWithTitle:@"×" target:self action:@selector(closeKeyboard:)];
    close.closeButton = YES;
    close.accessibilityLabel = @"关闭屏幕键盘";
    close.frame = NSMakeRect(1066, 2, 28, 24);
    close.autoresizingMask = NSViewMinXMargin;
    [content addSubview:close];
    for (const auto &row : Rows()) for (const Key &key : row) {
        NSButton *button = [MSIMEScreenKeyboardButton buttonWithTitle:@(key.normal) target:self action:@selector(pressKey:)];
        button.tag = _keys.size();
        button.accessibilityIdentifier = [NSString stringWithFormat:@"MSIMEScreenKeyboardKey%ld", (long)button.tag];
        button.accessibilityLabel = @(key.normal);
        button.font = [NSFont systemFontOfSize:key.normal[1] == '\0' ? 15 : 12];
        button.bezelStyle = NSBezelStyleRegularSquare;
        button.buttonType = NSButtonTypePushOnPushOff;
        // Use AppKit's native press-and-hold tracking so character, editing and
        // commit keys repeat while the pointer remains down. Sticky modifiers
        // stay one-shot toggles and must never oscillate during a long press.
        button.continuous = RepeatableKey(key);
        if (button.continuous) [button setPeriodicDelay:0.45 interval:0.075];
        [_buttons addObject:button];
        _keys.push_back(key);
        [content addSubview:button];
    }
    __weak MSIMEScreenKeyboardPanel *weakSelf = self;
    content.layoutKeys = ^(NSSize size) { [weakSelf layoutKeys:size]; };
    [self layoutKeys:content.bounds.size];
    return self;
}
- (BOOL)canBecomeKeyWindow { return NO; }
- (void)applyThemePreferences:(NSDictionary *)preferences {
    id surface = preferences[@"screen_keyboard_theme"];
    id global = preferences[@"theme"];
    id resolved = ([surface isEqual:@"dark"] || [surface isEqual:@"light"]) ? surface : global;
    if ([resolved isEqual:@"light"]) self.appearance = [NSAppearance appearanceNamed:NSAppearanceNameAqua];
    else if (resolved == nil || [resolved isEqual:@"system"]) self.appearance = nil;
    else self.appearance = [NSAppearance appearanceNamed:NSAppearanceNameDarkAqua];
}
- (BOOL)canBecomeMainWindow { return NO; }
- (void)layoutKeys:(NSSize)size {
    NSUInteger index = 0;
    const double height = std::max(1.0, (size.height - 28.0 - 7.0 - 16.0) / 5.0);
    NSUInteger rowIndex = 0;
    for (const auto &row : Rows()) {
        double weight = 0;
        for (const auto &key : row) weight += key.weight;
        const double available = std::max(1.0, size.width - 14.0 - 4.0 * (row.size() - 1));
        double x = 7;
        for (const auto &key : row) {
            double width = available * key.weight / weight;
            _buttons[index++].frame = NSMakeRect(x, 28 + rowIndex * (height + 4), width, height);
            x += width + 4;
        }
        ++rowIndex;
    }
}
- (void)refreshKeys {
    const bool shift = (_modifiers & NSEventModifierFlagShift) != 0;
    for (NSUInteger index = 0; index < _keys.size(); ++index) {
        const Key &key = _keys[index];
        // Upstream Caps affects posting, while the key face only reflects Shift.
        const bool shifted = shift && key.normal[1] == '\0';
        _buttons[index].title = @(shifted && key.shifted[0] ? key.shifted : key.normal);
        _buttons[index].state = key.modifier && (_modifiers & key.modifier) ? NSControlStateValueOn : NSControlStateValueOff;
    }
}
- (void)pressKey:(NSButton *)button {
    if (button.tag < 0 || (NSUInteger)button.tag >= _keys.size() || _buttons[button.tag] != button) return;
    const Key &key = _keys[button.tag];
    if (key.modifier) _modifiers ^= key.modifier;
    else {
        NSEventModifierFlags flags = _modifiers & ~NSEventModifierFlagCapsLock;
        if (Letter(key) && (_modifiers & NSEventModifierFlagCapsLock)) flags |= NSEventModifierFlagShift;
        if (CommitKey(key)) flags = 0; // Preserve Engine candidate selection with sticky modifiers.
        if (_sender && _sender(key.code, flags)) {
            _modifiers &= ~NSEventModifierFlagShift;
            _status.stringValue = @"水杉屏幕键盘";
        } else _status.stringValue = @"未发送：请检查辅助功能权限，并聚焦输入窗口后重试";
    }
    [self refreshKeys];
}
- (void)closeKeyboard:(id)sender { (void)sender; _modifiers = 0; _inputTargetPID = 0; [self refreshKeys]; [self orderOut:nil]; }
- (void)showKeyboard {
    pid_t candidate = _targetProvider ? _targetProvider() : 0;
    _inputTargetPID = msime::mac::CapturedScreenKeyboardTarget(candidate, NSProcessInfo.processInfo.processIdentifier);
    if (!self.visible) {
        NSRect visible = (NSScreen.mainScreen ?: NSScreen.screens.firstObject).visibleFrame;
        if (!NSIsEmptyRect(visible)) {
            const NSSize size = NSMakeSize(std::min(1100.0, visible.size.width), std::min(400.0, visible.size.height));
            [self setFrame:NSMakeRect(NSMidX(visible) - size.width / 2, NSMinY(visible), size.width, size.height) display:NO];
        }
    }
    [self orderFrontRegardless];
    // The pid decides where the keys go, so whether one was captured is the first thing to know when they go nowhere.
    os_log(MSIMEUILog(), "screen_keyboard_shown target_captured=%d", _inputTargetPID != 0);
    MSIMELogWindowState("screen_keyboard_present", self);
}
@end
