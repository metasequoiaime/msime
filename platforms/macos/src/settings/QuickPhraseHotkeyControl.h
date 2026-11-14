#pragma once
#import <AppKit/AppKit.h>
#import "../input/ToggleHotkeyEvent.h"
#include "msime_client.h"
#include <cstring>

// 只由 AppearancePreferences.mm 引入；录键与错误显示独立于巨型设置窗口实现。
@interface MSIMEQuickPhraseHotkeyButton : NSButton
@property(nonatomic, copy) void (^recorded)(NSString *);
@property(nonatomic, copy) NSString *binding;
@property(nonatomic) BOOL recording;
@end

@implementation MSIMEQuickPhraseHotkeyButton
- (instancetype)initWithFrame:(NSRect)frame {
    if ((self = [super initWithFrame:frame])) {
        self.target = self;
        self.action = @selector(beginRecording:);
        self.bezelStyle = NSBezelStyleRounded;
        self.accessibilityLabel = @"快捷短语附加快捷键";
        self.binding = @"";
    }
    return self;
}
- (BOOL)acceptsFirstResponder { return YES; }
- (void)setBinding:(NSString *)value {
    _binding = [value copy] ?: @"";
    if (!self.recording) self.title = _binding.length ? _binding : @"未绑定";
}
- (void)beginRecording:(id)sender {
    (void)sender;
    if (![self.window makeFirstResponder:self]) return;
    self.recording = YES;
    self.title = @"按下组合键…";
}
- (BOOL)resignFirstResponder {
    self.recording = NO;
    self.binding = self.binding;
    return [super resignFirstResponder];
}
- (void)keyDown:(NSEvent *)event {
    if (!self.recording) { [super keyDown:event]; return; }
    NSString *chord = nil;
    if (event.keyCode == 53) { self.recording = NO; self.binding = self.binding; return; }
    if (event.keyCode == 51 || event.keyCode == 117) chord = @"";
    else {
        const auto flags = event.modifierFlags;
        if (!(flags & (NSEventModifierFlagControl | NSEventModifierFlagOption | NSEventModifierFlagCommand))) {
            NSBeep(); return;
        }
        const char *key = msime::mac::ToggleKeyNameForEvent(event.keyCode, msime::mac::ToggleHotkeyCharacter(event));
        if (!key) { NSBeep(); return; }
        NSMutableArray *parts = [NSMutableArray array];
        if (flags & NSEventModifierFlagControl) [parts addObject:@"Ctrl"];
        if (flags & NSEventModifierFlagOption) [parts addObject:@"Alt"];
        if (flags & NSEventModifierFlagShift) [parts addObject:@"Shift"];
        if (flags & NSEventModifierFlagCommand) [parts addObject:@"Meta"];
        [parts addObject:@(key)];
        chord = [parts componentsJoinedByString:@"+"];
    }
    self.recording = NO;
    self.binding = self.binding;
    if (self.recorded) self.recorded(chord);
}
@end

@interface MSIMEQuickPhraseHotkeyControl : NSStackView
@property(nonatomic, readonly) MSIMEQuickPhraseHotkeyButton *recorder;
- (instancetype)initWithSnapshot:(NSDictionary *(^)(void))snapshot apply:(void (^)(NSString *))apply;
@end

@implementation MSIMEQuickPhraseHotkeyControl {
    NSTextField *_error;
}
- (instancetype)initWithSnapshot:(NSDictionary *(^)(void))snapshot apply:(void (^)(NSString *))apply {
    if ((self = [super initWithFrame:NSZeroRect])) {
        self.orientation = NSUserInterfaceLayoutOrientationVertical;
        self.alignment = NSLayoutAttributeTrailing;
        self.spacing = 4;
        _recorder = [[MSIMEQuickPhraseHotkeyButton alloc] initWithFrame:NSZeroRect];
        _error = [NSTextField wrappingLabelWithString:@""];
        _error.textColor = NSColor.systemRedColor;
        _error.font = [NSFont systemFontOfSize:NSFont.smallSystemFontSize];
        _error.hidden = YES;
        [self addArrangedSubview:_recorder];
        [self addArrangedSubview:_error];
        __weak MSIMEQuickPhraseHotkeyControl *weakSelf = self;
        _recorder.recorded = ^(NSString *chord) {
            MSIMEQuickPhraseHotkeyControl *control = weakSelf;
            if (!control) return;
            NSMutableDictionary *request = [snapshot() mutableCopy];
            request[@"field"] = @"enter_quick_phrase";
            request[@"chord"] = chord;
            NSData *body = [NSJSONSerialization dataWithJSONObject:request options:0 error:nil];
            char *raw = body ? msime_client_check_toggle_hotkey((const uint8_t *)body.bytes, body.length) : nullptr;
            NSDictionary *response = raw ? [NSJSONSerialization JSONObjectWithData:[NSData dataWithBytes:raw length:strlen(raw)] options:0 error:nil] : nil;
            msime_client_string_free(raw);
            NSDictionary *result = [response[@"value"] isKindOfClass:NSDictionary.class] ? response[@"value"] : nil;
            if ([response[@"ok"] isEqual:@YES] && [result[@"ok"] isEqual:@YES]) {
                apply(result[@"chord"]);
                control.recorder.binding = result[@"chord"];
                control->_error.hidden = YES;
            } else {
                control->_error.stringValue = [result[@"kind"] isEqual:@"conflict"] ? @"与已有快捷键冲突" : @"无法设置快捷键";
                control->_error.hidden = NO;
            }
        };
    }
    return self;
}
@end
