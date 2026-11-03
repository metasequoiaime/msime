#import "GlobalVoiceCoordinator.h"
#import "GlobalVoiceState.h"
#import "VoiceHoldShortcut.h"
#import "VoiceWaveOverlay.h"
#import "VoiceTextCommit.h"
#import "../core/SharedVoicePreferences.h"
#import "../settings/RuntimeOptions.h"
#import "../input/ToggleHotkeys.h"
#import "../input/InputModeIdentifiers.h"
#import "MSIMEClientSession.h"
#import <Carbon/Carbon.h>
#import <ServiceManagement/ServiceManagement.h>
#import <ApplicationServices/ApplicationServices.h>
#include "msime_client.h"
#include <vector>

namespace {
NSString *const AgentPlist = @"app.msime.global-voice.plist";
NSString *SourceID(TISInputSourceRef source) {
    return source ? (__bridge NSString *)TISGetInputSourceProperty(source, kTISPropertyInputSourceID) : nil;
}
NSString *CurrentSourceID() {
    TISInputSourceRef source = TISCopyCurrentKeyboardInputSource();
    NSString *identifier = [SourceID(source) copy];
    if (source) CFRelease(source);
    return identifier;
}
BOOL SelectSource(NSString *identifier) {
    CFArrayRef sources = TISCreateInputSourceList((__bridge CFDictionaryRef)@{
        (__bridge NSString *)kTISPropertyInputSourceID: identifier,
        (__bridge NSString *)kTISPropertyInputSourceIsEnabled: @YES,
        (__bridge NSString *)kTISPropertyInputSourceIsSelectCapable: @YES }, false);
    BOOL selected = sources && CFArrayGetCount(sources) &&
        TISSelectInputSource((TISInputSourceRef)CFArrayGetValueAtIndex(sources, 0)) == noErr;
    if (sources) CFRelease(sources);
    return selected;
}
NSString *KeyboardLayoutID() {
    TISInputSourceRef source = TISCopyCurrentKeyboardLayoutInputSource();
    NSString *identifier = [SourceID(source) copy];
    if (source) CFRelease(source);
    return identifier;
}
BOOL IsOurs(NSString *identifier) {
    return identifier && ([identifier isEqual:NSBundle.mainBundle.bundleIdentifier] ||
        [identifier hasPrefix:[NSBundle.mainBundle.bundleIdentifier stringByAppendingString:@"."]]);
}
NSString *EnabledSource() {
    TISInputSourceRef current = TISCopyCurrentKeyboardInputSource();
    NSString *identifier = [SourceID(current) copy];
    if (current) CFRelease(current);
    if (IsOurs(identifier)) return identifier;
    CFArrayRef sources = TISCreateInputSourceList((__bridge CFDictionaryRef)@{
        (__bridge NSString *)kTISPropertyBundleID: NSBundle.mainBundle.bundleIdentifier,
        (__bridge NSString *)kTISPropertyInputSourceIsEnabled: @YES,
        (__bridge NSString *)kTISPropertyInputSourceIsSelectCapable: @YES }, false);
    NSString *selected = nil;
    if (sources && CFArrayGetCount(sources)) selected = [SourceID((TISInputSourceRef)CFArrayGetValueAtIndex(sources, 0)) copy];
    if (sources) CFRelease(sources);
    return selected;
}
void OpenHost(void (^completion)(NSError *)) {
    NSWorkspaceOpenConfiguration *configuration = NSWorkspaceOpenConfiguration.configuration;
    configuration.activates = NO;
    configuration.createsNewApplicationInstance = NO;
    [NSWorkspace.sharedWorkspace openApplicationAtURL:NSBundle.mainBundle.bundleURL configuration:configuration
        completionHandler:^(NSRunningApplication *application, NSError *error) { (void)application; completion(error); }];
}
BOOL SystemHotkeyConflict(UInt32 code, UInt32 modifiers) {
    CFArrayRef keys = nullptr;
    if (CopySymbolicHotKeys(&keys) != noErr || !keys) return YES;
    BOOL conflict = NO;
    for (NSDictionary *key in (__bridge NSArray *)keys) {
        if ([key[(__bridge NSString *)kHISymbolicHotKeyEnabled] boolValue] &&
            [key[(__bridge NSString *)kHISymbolicHotKeyCode] unsignedIntValue] == code &&
            [key[(__bridge NSString *)kHISymbolicHotKeyModifiers] unsignedIntValue] == modifiers) { conflict = YES; break; }
    }
    CFRelease(keys);
    return conflict;
}
// 组合键沿用当前键盘布局的无修饰字符，不能把自定义字母硬编码成美式物理键。
int KeyCodeFor(msime::mac::ToggleChord chord) {
    if (!chord.bound) return -1;
    if (!chord.letter) return chord.keyCode;
    TISInputSourceRef source = TISCopyCurrentKeyboardLayoutInputSource();
    CFDataRef data = source ? (CFDataRef)TISGetInputSourceProperty(source, kTISPropertyUnicodeKeyLayoutData) : nullptr;
    int found = -1;
    if (data) for (UInt16 code = 0; code < 128; ++code) {
        UInt32 dead = 0; UniChar chars[4]; UniCharCount count = 0;
        if (UCKeyTranslate(reinterpret_cast<const UCKeyboardLayout *>(CFDataGetBytePtr(data)), code,
            kUCKeyActionDown, 0, LMGetKbdType(), kUCKeyTranslateNoDeadKeysBit, &dead, 4, &count, chars) == noErr &&
            count == 1 && chars[0] == chord.letter) { found = code; break; }
    }
    if (source) CFRelease(source);
    return found;
}
}

BOOL MSIMEHandleGlobalVoiceBootstrap(int argc, const char *argv[], int *exitCode) {
    if (argc < 2 || (strcmp(argv[1], "--global-voice-bootstrap") && strcmp(argv[1], "--configure-global-voice"))) return NO;
    *exitCode = 0;
    if (!strcmp(argv[1], "--configure-global-voice")) {
        if (argc != 3 || (strcmp(argv[2], "enable") && strcmp(argv[2], "disable"))) { *exitCode = 2; return YES; }
        SMAppService *service = [SMAppService agentServiceWithPlistName:AgentPlist];
        NSError *error = nil;
        BOOL enabled = !strcmp(argv[2], "enable");
        if (enabled && service.status == SMAppServiceStatusNotRegistered && ![service registerAndReturnError:&error]) *exitCode = 1;
        if (!enabled && service.status != SMAppServiceStatusNotRegistered && ![service unregisterAndReturnError:&error]) *exitCode = 1;
        if (!enabled) return YES;
    }
    // Launch Services 复用已经运行的 IMK 应用；引导进程不注册 IMK 连接、不监听键盘，也不录音。
    __block BOOL done = NO;
    __block int result = 0;
    OpenHost(^(NSError *error) { dispatch_async(dispatch_get_main_queue(), ^{ result = error ? 1 : 0; done = YES; }); });
    NSDate *deadline = [NSDate dateWithTimeIntervalSinceNow:10];
    while (!done && deadline.timeIntervalSinceNow > 0) [NSRunLoop.currentRunLoop runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.05]];
    *exitCode = *exitCode || !done || result ? 1 : 0;
    return YES;
}

@interface MSIMEGlobalVoiceCoordinator () <NSWindowDelegate>
- (CGEventRef)observe:(CGEventRef)event type:(CGEventType)type;
- (void)toggle;
@end

@implementation MSIMEGlobalVoiceCoordinator {
    id<MSIMEGlobalVoiceTarget> (^_target)(void);
    id<MSIMEGlobalVoiceTarget> _owner;
    NSTimer *_poll;
    NSTimer *_sessionPoll;
    CFMachPortRef _tap;
    CFRunLoopSourceRef _tapSource;
    EventHandlerRef _hotkeyHandler;
    std::vector<EventHotKeyRef> _hotkeys;
    NSUInteger _keysDown;
    NSUInteger _registeredMask;
    UInt32 _pressedCodes[2];
    NSUInteger _replayedKeys;
    pid_t _replayPID;
    UInt32 _hotkeyCodes[2];
    UInt32 _hotkeyModifiers[2];
    UInt32 _replayCodes[2];
    UInt32 _replayModifiers[2];
    NSDictionary *_preferences;
    id _revision;
    BOOL _loading;
    BOOL _enabled;
    BOOL _permissionAsked;
    BOOL _holding;
    BOOL _armed;
    BOOL _spaceDown;
    BOOL _escapeDown;
    BOOL _locked;
    NSUInteger _holdGeneration;
    NSRunningApplication *_application;
    MSIMEVoiceHoldShortcut _gesture;
    msime::mac::GlobalVoiceState _state;
    MSIMEVoiceWaveOverlay *_failure;
    NSPanel *_recovery;
    NSString *_recoveryText;
    id _focusIdentity;
    BOOL _serviceErrorShown;
    BOOL _holdStartedSession;
    BOOL _requiresHold;
    BOOL _trusted;
    BOOL _configurationPending;
    NSString *_layoutID;
}
- (instancetype)initWithTarget:(id<MSIMEGlobalVoiceTarget> (^)(void))target {
    if (!(self = [super init])) return nil;
    _target = [target copy];
    __weak MSIMEGlobalVoiceCoordinator *weakSelf = self;
    MSIMEGlobalVoiceHost().recover = ^(NSString *text) { [weakSelf recover:text]; };
    MSIMEGlobalVoiceHost().toggleHandled = ^BOOL(NSEvent *event) {
        MSIMEGlobalVoiceCoordinator *self = weakSelf;
        if (!self) return NO;
        const auto flags = event.modifierFlags;
        UInt32 modifiers = (flags & NSEventModifierFlagControl ? controlKey : 0) |
            (flags & NSEventModifierFlagOption ? optionKey : 0) | (flags & NSEventModifierFlagShift ? shiftKey : 0) |
            (flags & NSEventModifierFlagCommand ? cmdKey : 0);
        for (UInt32 identifier = 0; identifier < 2; ++identifier)
            if ((self->_registeredMask & (1u << identifier)) && event.keyCode == self->_hotkeyCodes[identifier] &&
                modifiers == self->_hotkeyModifiers[identifier]) return YES;
        return NO;
    };
    MSIMEGlobalVoiceHost().selectedMode = ^(NSString *mode) {
        MSIMEGlobalVoiceCoordinator *self = weakSelf;
        if (self && self->_owner && ![self->_owner globalVoiceBusy]) self->_state.hostSelectedSource(mode.UTF8String ?: "");
    };
    _poll = [NSTimer scheduledTimerWithTimeInterval:1 repeats:YES block:^(NSTimer *timer) { (void)timer; [weakSelf reload]; }];
    [self reload];
    NSNotificationCenter *workspace = NSWorkspace.sharedWorkspace.notificationCenter;
    [workspace addObserver:self selector:@selector(suspended:) name:NSWorkspaceWillSleepNotification object:nil];
    [workspace addObserver:self selector:@selector(suspended:) name:NSWorkspaceSessionDidResignActiveNotification object:nil];
    [NSDistributedNotificationCenter.defaultCenter addObserver:self selector:@selector(suspended:) name:@"com.apple.screenIsLocked" object:nil];
    [NSDistributedNotificationCenter.defaultCenter addObserver:self selector:@selector(sourceChanged:) name:(__bridge NSString *)kTISNotifySelectedKeyboardInputSourceChanged object:nil];
    return self;
}
- (void)showFailure:(NSString *)detail {
    if (!_failure) _failure = [MSIMEVoiceWaveOverlay new];
    [_failure showFailure:MSIMEVoiceFailureSession detail:detail];
}
- (void)recover:(NSString *)text {
    if (!text.length || text.length > 65536) return;
    _recoveryText = [text copy];
    if (!_recovery) {
        _recovery = [[NSPanel alloc] initWithContentRect:NSMakeRect(0, 0, 440, 180)
            styleMask:NSWindowStyleMaskTitled | NSWindowStyleMaskClosable backing:NSBackingStoreBuffered defer:NO];
        _recovery.title = @"语音结果尚未插入";
        _recovery.releasedWhenClosed = NO;
        _recovery.delegate = self;
        _recovery.level = NSFloatingWindowLevel;
        NSTextField *message = [NSTextField wrappingLabelWithString:@"输入位置已改变。复制后可自行粘贴，关闭会丢弃此结果。"];
        message.frame = NSMakeRect(20, 128, 400, 32); [_recovery.contentView addSubview:message];
        NSTextField *preview = [NSTextField wrappingLabelWithString:@""]; preview.tag = 1;
        preview.selectable = YES; preview.frame = NSMakeRect(20, 48, 400, 72); [_recovery.contentView addSubview:preview];
        NSButton *copy = [NSButton buttonWithTitle:@"复制结果" target:self action:@selector(copyRecovery:)];
        copy.frame = NSMakeRect(320, 10, 100, 30); [_recovery.contentView addSubview:copy];
        [_recovery center];
    }
    [(NSTextField *)[_recovery.contentView viewWithTag:1] setStringValue:text];
    [_recovery orderFrontRegardless];
}
- (void)windowWillClose:(NSNotification *)notification {
    if (notification.object == _recovery) { _recoveryText = nil; [(NSTextField *)[_recovery.contentView viewWithTag:1] setStringValue:@""]; }
}
- (void)copyRecovery:(id)sender {
    (void)sender;
    if (!_recoveryText.length) return;
    // 只有明确点击才写剪贴板，不自动粘贴，也不落盘转写内容。
    NSPasteboard *pasteboard = NSPasteboard.generalPasteboard;
    [pasteboard clearContents]; [pasteboard setString:_recoveryText forType:NSPasteboardTypeString];
    [pasteboard setString:@"" forType:@"org.nspasteboard.ConcealedType"];
    [pasteboard setString:@"" forType:@"org.nspasteboard.AutoGeneratedType"];
    _recoveryText = nil; [(NSTextField *)[_recovery.contentView viewWithTag:1] setStringValue:@""]; [_recovery close];
}
- (void)reload {
    if (_loading) return;
    _loading = YES;
    __weak MSIMEGlobalVoiceCoordinator *weakSelf = self;
    dispatch_async(dispatch_get_global_queue(QOS_CLASS_UTILITY, 0), ^{
        NSDictionary *options = MSIMELoadRuntimeOptions();
        NSString *directory = [options[@"preferences_directory"] isKindOfClass:NSString.class] ? options[@"preferences_directory"] : nil;
        NSDictionary *snapshot = directory.length ? [MSIMEClientSession loadPreferencesInDirectory:directory error:nil] : nil;
        dispatch_async(dispatch_get_main_queue(), ^{
            MSIMEGlobalVoiceCoordinator *self = weakSelf;
            if (!self) return;
            self->_loading = NO;
            const BOOL trusted = AXIsProcessTrusted();
            const BOOL trustChanged = self->_trusted != trusted;
            self->_trusted = trusted;
            if (!snapshot) return;
            if ([self->_revision isEqual:snapshot[@"revision"]]) {
                if (trustChanged && self->_enabled) [self configure];
                return;
            }
            self->_revision = snapshot[@"revision"];
            self->_preferences = snapshot[@"preferences"];
            [self configure];
        });
    });
}
- (void)removeKeyboard {
    MSIMEGlobalVoiceHost().keyboard = false;
    if (_tap) { CGEventTapEnable(_tap, false); CFMachPortInvalidate(_tap); CFRelease(_tap); _tap = nullptr; }
    if (_tapSource) { CFRunLoopRemoveSource(CFRunLoopGetMain(), _tapSource, kCFRunLoopCommonModes); CFRelease(_tapSource); _tapSource = nullptr; }
    for (auto key : _hotkeys) UnregisterEventHotKey(key);
    _hotkeys.clear(); _registeredMask = 0;
    if (_hotkeyHandler) { RemoveEventHandler(_hotkeyHandler); _hotkeyHandler = nullptr; }
    _gesture.reset(); _armed = NO; _holding = NO; _holdStartedSession = NO; ++_holdGeneration;
}
- (void)configure {
    NSDictionary *voice = _preferences[@"voice_input"];
    BOOL enabled = [voice[@"hotkey_global"] isEqual:@YES] && ![voice[@"enabled"] isEqual:@NO];
    const BOOL trusted = AXIsProcessTrusted();
    if (_state.phase != msime::mac::GlobalVoiceState::Phase::Idle) {
        // 识别中刷新偏好不能丢掉尚未到来的松手沿；这轮结束后再换注册与手势状态。
        if (enabled && trusted) { _configurationPending = YES; return; }
        if (!trusted) [self cancel];
        else [self stop];
    }
    _configurationPending = NO;
    [self removeKeyboard];
    _enabled = enabled;
    _layoutID = KeyboardLayoutID();
    _trusted = AXIsProcessTrusted();
    SMAppService *service = [SMAppService agentServiceWithPlistName:AgentPlist];
    NSError *serviceError = nil;
    if (enabled && service.status == SMAppServiceStatusNotRegistered) [service registerAndReturnError:&serviceError];
    if (!enabled && service.status != SMAppServiceStatusNotRegistered) [service unregisterAndReturnError:&serviceError];
    if (enabled && !_serviceErrorShown && (serviceError || service.status != SMAppServiceStatusEnabled)) {
        _serviceErrorShown = YES;
        [self showFailure:@"后台语音尚未获准。请在系统设置的「通用 > 登录项与扩展」允许水杉后台运行。"];
    }
    if (!enabled) { _permissionAsked = NO; _serviceErrorShown = NO; _keysDown = 0; return; }
    if (!_trusted) {
        if (!_permissionAsked) {
            _permissionAsked = YES;
            AXIsProcessTrustedWithOptions((__bridge CFDictionaryRef)@{(__bridge NSString *)kAXTrustedCheckOptionPrompt: @YES});
            [self showFailure:@"跨输入法语音需要辅助功能权限。请在系统设置中允许水杉输入法，然后重新启用快捷键范围。"];
        }
        return;
    }
    _tap = CGEventTapCreate(kCGSessionEventTap, kCGHeadInsertEventTap, kCGEventTapOptionDefault,
        CGEventMaskBit(kCGEventFlagsChanged) | CGEventMaskBit(kCGEventKeyDown) | CGEventMaskBit(kCGEventKeyUp),
        [](CGEventTapProxy proxy, CGEventType type, CGEventRef event, void *context) -> CGEventRef {
            (void)proxy;
            return [(__bridge MSIMEGlobalVoiceCoordinator *)context observe:event type:type];
        }, (__bridge void *)self);
    if (!_tap) { [self showFailure:@"无法监听全局语音键。请检查辅助功能与输入监控权限。水杉内的快捷键仍可使用。"] ; return; }
    _tapSource = CFMachPortCreateRunLoopSource(kCFAllocatorDefault, _tap, 0);
    CFRunLoopAddSource(CFRunLoopGetMain(), _tapSource, kCFRunLoopCommonModes);
    CGEventTapEnable(_tap, true);
    EventTypeSpec types[] = {{kEventClassKeyboard, kEventHotKeyPressed}, {kEventClassKeyboard, kEventHotKeyReleased}};
    const OSStatus handlerStatus = InstallApplicationEventHandler([](EventHandlerCallRef next, EventRef event, void *context) -> OSStatus {
        (void)next;
        EventHotKeyID key = {};
        if (GetEventParameter(event, kEventParamDirectObject, typeEventHotKeyID, nullptr, sizeof(key), nullptr, &key) != noErr || key.signature != 'MSVG') return eventNotHandledErr;
        MSIMEGlobalVoiceCoordinator *self = (__bridge MSIMEGlobalVoiceCoordinator *)context;
        if (key.id > 1) return eventNotHandledErr;
        [self handleHotkey:key.id down:GetEventKind(event) == kEventHotKeyPressed];
        return noErr;
    }, 2, types, (__bridge void *)self, &_hotkeyHandler);
    if (handlerStatus != noErr) { [self removeKeyboard]; [self showFailure:@"无法注册全局语音事件，请重启水杉输入法后重试。"]; return; }
    if (![voice[@"hotkey_ctrl_f9"] isEqual:@NO]) [self registerCode:101 modifiers:controlKey identifier:0];
    NSData *body = [NSJSONSerialization dataWithJSONObject:@{@"keybindings": _preferences[@"keybindings"] ?: NSNull.null} options:0 error:nil];
    char *raw = body ? msime_client_toggle_hotkey_chords((const uint8_t *)body.bytes, body.length) : nullptr;
    NSDictionary *response = raw ? [NSJSONSerialization JSONObjectWithData:[NSData dataWithBytes:raw length:strlen(raw)] options:0 error:nil] : nil;
    if (raw) msime_client_string_free(raw);
    NSDictionary *chord = response[@"value"][@"voice_input"];
    if ([chord[@"key"] isKindOfClass:NSString.class]) {
        auto decoded = msime::mac::ToggleChordForKeyName([chord[@"key"] UTF8String], [chord[@"ctrl"] boolValue],
            [chord[@"alt"] boolValue], [chord[@"shift"] boolValue], [chord[@"meta"] boolValue]);
        const int code = KeyCodeFor(decoded);
        UInt32 modifiers = (decoded.control ? controlKey : 0) | (decoded.option ? optionKey : 0) |
            (decoded.shift ? shiftKey : 0) | (decoded.command ? cmdKey : 0);
        if (code >= 0 && !(code == 101 && modifiers == controlKey && ![voice[@"hotkey_ctrl_f9"] isEqual:@NO]))
            [self registerCode:code modifiers:modifiers identifier:1];
    }
    MSIMEGlobalVoiceHost().keyboard = true;
}
- (void)registerCode:(UInt32)code modifiers:(UInt32)modifiers identifier:(UInt32)identifier {
    EventHotKeyRef key = nullptr;
    if (SystemHotkeyConflict(code, modifiers) || RegisterEventHotKey(code, modifiers, {'MSVG', identifier}, GetApplicationEventTarget(), 0, &key) != noErr) {
        [self showFailure:@"语音组合键与系统或其他应用冲突。请在快捷键设置中更换现有语音绑定。"];
        return;
    }
    _hotkeyCodes[identifier] = code; _hotkeyModifiers[identifier] = modifiers;
    _hotkeys.push_back(key); _registeredMask |= 1u << identifier;
}
- (void)handleHotkey:(UInt32)identifier down:(BOOL)down {
    if (identifier > 1) return;
    const NSUInteger mask = 1u << identifier;
    if (!down) {
        _keysDown &= ~mask;
        if (_replayedKeys & mask) { [self replayKey:identifier down:NO]; _replayedKeys &= ~mask; }
    } else if (!(_keysDown & mask)) {
        _keysDown |= mask; _pressedCodes[identifier] = _hotkeyCodes[identifier];
        // 已注册的全局组合键先到 Carbon。录键按钮持有焦点时交还设置窗口，不触发语音。
        if ([self shortcutRecorderFocused]) { _replayedKeys |= mask; [self replayKey:identifier down:YES]; }
        else [self toggle];
    }
}
- (BOOL)shortcutRecorderFocused {
    NSRunningApplication *application = NSWorkspace.sharedWorkspace.frontmostApplication;
    if (![application.bundleIdentifier isEqual:MSIMESettingsBundleIdentifier()]) return NO;
    id focus = MSIMEGlobalVoiceFocusIdentity();
    if (!focus) return NO;
    CFTypeRef label = nullptr;
    AXUIElementCopyAttributeValue((__bridge AXUIElementRef)focus, kAXDescriptionAttribute, &label);
    BOOL recording = label && CFEqual(label, CFSTR("正在录制快捷键"));
    if (label) CFRelease(label);
    return recording;
}
- (void)replayKey:(UInt32)identifier down:(BOOL)down {
    NSRunningApplication *application = NSWorkspace.sharedWorkspace.frontmostApplication;
    if (down) {
        _replayPID = application.processIdentifier;
        _replayCodes[identifier] = _hotkeyCodes[identifier];
        _replayModifiers[identifier] = _hotkeyModifiers[identifier];
    }
    if (application.processIdentifier != _replayPID || ![application.bundleIdentifier isEqual:MSIMESettingsBundleIdentifier()]) return;
    CGEventRef event = CGEventCreateKeyboardEvent(nullptr, _replayCodes[identifier], down);
    if (!event) return;
    UInt32 modifiers = _replayModifiers[identifier];
    CGEventSetFlags(event, (modifiers & controlKey ? kCGEventFlagMaskControl : 0) |
        (modifiers & optionKey ? kCGEventFlagMaskAlternate : 0) | (modifiers & shiftKey ? kCGEventFlagMaskShift : 0) |
        (modifiers & cmdKey ? kCGEventFlagMaskCommand : 0));
    CGEventSetIntegerValueField(event, kCGEventSourceUserData, MSIMEVoiceCommitEventTag);
    CGEventPostToPid(_replayPID, event);
    CFRelease(event);
}
- (void)queueGestureAction:(NSUInteger)action {
    const uint64_t generation = _state.generation;
    if (action == 0) { _armed = NO; ++_holdGeneration; }
    __weak MSIMEGlobalVoiceCoordinator *weakSelf = self;
    // 事件 tap 不等待麦克风停止、输入源 IPC 或后台项注册；晚到的动作也不能作用于下一轮。
    dispatch_async(dispatch_get_main_queue(), ^{
        MSIMEGlobalVoiceCoordinator *self = weakSelf;
        if (!self || self->_state.generation != generation) return;
        if (action == 0) [self cancel];
        else if (action == 1) [self stop];
        else [self toggle];
    });
}
- (CGEventRef)observe:(CGEventRef)event type:(CGEventType)type {
    if (type == kCGEventTapDisabledByTimeout || type == kCGEventTapDisabledByUserInput) {
        [self cancel];
        CGEventTapEnable(_tap, true);
        return event;
    }
    // 本宿主的上屏与录键回送不再次触发语音；获准发送按键的键盘重映射工具仍可使用现有绑定。
    if (CGEventGetIntegerValueField(event, kCGEventSourceUnixProcessID) == getpid() ||
        CGEventGetIntegerValueField(event, kCGEventSourceUserData) == MSIMEVoiceCommitEventTag) return event;
    NSEvent *native = [NSEvent eventWithCGEvent:event];
    if (!native) return event;
    const unsigned short code = native.keyCode;
    if (type == kCGEventKeyUp) for (UInt32 identifier = 0; identifier < 2; ++identifier) {
        const NSUInteger mask = 1u << identifier;
        if ((_replayedKeys & mask) && _replayCodes[identifier] == code) {
            [self replayKey:identifier down:NO]; _replayedKeys &= ~mask; _keysDown &= ~mask;
        } else if (_pressedCodes[identifier] == code) _keysDown &= ~mask;
    }
    if (type == kCGEventKeyDown && code == 53 && _state.phase != msime::mac::GlobalVoiceState::Phase::Idle) {
        _escapeDown = YES; [self queueGestureAction:0]; return nullptr;
    }
    if (type == kCGEventKeyUp && code == 53 && _escapeDown) { _escapeDown = NO; return nullptr; }
    if (code == 49 && _holding && _owner && _state.phase == msime::mac::GlobalVoiceState::Phase::Active) {
        if (type == kCGEventKeyDown) { _spaceDown = YES; _locked = YES; [_owner lockGlobalVoice]; return nullptr; }
    }
    if (type == kCGEventKeyUp && code == 49 && _spaceDown) { _spaceDown = NO; return nullptr; }
    if (type == kCGEventKeyDown && _holding) {
        _armed = NO; ++_holdGeneration;
        if (!_locked) [self queueGestureAction:0];
        // 修饰键已按原样交给应用，此刻的普通组合键也原样交付。
    }
    NSDictionary *voice = _preferences[@"voice_input"];
    MSIMEVoiceHoldShortcut::Options options = {![voice[@"hotkey_ralt"] isEqual:@NO], [voice[@"hotkey_ctrl_win"] isEqual:@YES] != NO, [voice[@"hotkey_rctrl_ralt"] isEqual:@YES] != NO, true};
    auto result = _gesture.observe(native, options, _owner != nil, CGEventSourceFlagsState(kCGEventSourceStateHIDSystemState));
    if (result.action == MSIMEVoiceHoldShortcut::Action::Toggle && !result.onRelease) {
        _holding = YES;
        _holdStartedSession = !_owner;
        if (_owner) { [self queueGestureAction:2]; return event; }
        _armed = YES;
        const NSUInteger generation = ++_holdGeneration;
        __weak MSIMEGlobalVoiceCoordinator *weakSelf = self;
        dispatch_after(dispatch_time(DISPATCH_TIME_NOW, 250 * NSEC_PER_MSEC), dispatch_get_main_queue(), ^{
            MSIMEGlobalVoiceCoordinator *self = weakSelf;
            if (self && self->_enabled && self->_armed && generation == self->_holdGeneration) { self->_armed = NO; [self begin:YES]; }
        });
    }
    if (result.onRelease) {
        _holding = NO; _armed = NO; ++_holdGeneration;
        if (!_locked && (_holdStartedSession || result.action == MSIMEVoiceHoldShortcut::Action::Toggle)) [self queueGestureAction:1];
        _holdStartedSession = NO;
    }
    return event;
}
- (void)toggle {
    if (!_enabled) return;
    if (_state.phase == msime::mac::GlobalVoiceState::Phase::Idle) [self begin:NO];
    else if (_state.phase == msime::mac::GlobalVoiceState::Phase::Finishing) [self cancel];
    else [self stop];
}
- (void)begin:(BOOL)holding {
    if ([self shortcutRecorderFocused]) return;
    if (_state.phase != msime::mac::GlobalVoiceState::Phase::Idle) return;
    if (_recoveryText.length) { [_recovery orderFrontRegardless]; return; }
    NSRunningApplication *application = NSWorkspace.sharedWorkspace.frontmostApplication;
    id focus = MSIMEGlobalVoiceFocusIdentity();
    if (!focus) { [self showFailure:@"当前输入位置不支持全局语音。请将光标放入普通可编辑文本框。"] ; return; }
    NSString *source = CurrentSourceID(), *selected = EnabledSource();
    if (!application || application.processIdentifier == getpid() || !source || !selected) { [self showFailure:@"请先启用水杉输入法，并将光标放入可编辑文本框。"] ; return; }
    id<MSIMEGlobalVoiceTarget> target = _target();
    if ([target globalVoiceBusy]) return;
    if (!_state.begin(source.UTF8String, application.processIdentifier, NSProcessInfo.processInfo.systemUptime)) return;
    _application = application; _focusIdentity = focus; _holding = holding; _requiresHold = holding; _locked = NO;
    if (!SelectSource(selected)) { _state.clear(); [self showFailure:@"无法切换到已启用的水杉输入法。"] ; return; }
    _state.selected = selected.UTF8String;
    __weak MSIMEGlobalVoiceCoordinator *weakSelf = self;
    _sessionPoll = [NSTimer scheduledTimerWithTimeInterval:0.05 repeats:YES block:^(NSTimer *timer) { (void)timer; [weakSelf advance]; }];
    [self advance];
}
- (void)advance {
    using Phase = msime::mac::GlobalVoiceState::Phase;
    NSString *source = CurrentSourceID();
    if (_state.phase == Phase::Waiting) {
        if (_requiresHold && !_holding && !_locked) { [self cancel]; return; }
        if (_application.terminated || NSWorkspace.sharedWorkspace.frontmostApplication.processIdentifier != _state.targetPID || !IsOurs(source)) { [self cancel]; return; }
        id<MSIMEGlobalVoiceTarget> target = _target();
        id focus = MSIMEGlobalVoiceFocusIdentity();
        if (focus && _focusIdentity && !CFEqual((__bridge CFTypeRef)focus, (__bridge CFTypeRef)_focusIdentity)) { [self cancel]; return; }
        if (target && focus && [target globalVoiceTargetPID] == _state.targetPID && ![target globalVoiceBusy]) {
            _state.activate(_state.targetPID, source.UTF8String);
            _owner = target; MSIMEGlobalVoiceHost().owner = target; MSIMEGlobalVoiceHost().focusIdentity = _focusIdentity;
            [_owner startGlobalVoiceHolding:_holding];
        } else if (_state.timedOut(NSProcessInfo.processInfo.systemUptime)) {
            [self cancel]; [self showFailure:@"没有获得可编辑的输入位置，已取消语音。"];
        }
        return;
    }
    id focus = MSIMEGlobalVoiceFocusIdentity();
    if (_owner && (_application.terminated || NSWorkspace.sharedWorkspace.frontmostApplication.processIdentifier != _state.targetPID ||
        !focus || !_focusIdentity || !CFEqual((__bridge CFTypeRef)focus, (__bridge CFTypeRef)_focusIdentity))) [_owner detachGlobalVoice];
    if (_owner && ![_owner globalVoiceBusy]) { [self complete]; return; }
    if (_owner && [_owner globalVoiceProcessing]) _state.phase = Phase::Finishing;
}
- (void)sourceChanged:(NSNotification *)notification {
    (void)notification;
    NSString *source = CurrentSourceID();
    if (_state.phase == msime::mac::GlobalVoiceState::Phase::Waiting) {
        if (!IsOurs(source)) { _state.manual = true; [self cancel]; }
    } else if (_state.phase != msime::mac::GlobalVoiceState::Phase::Idle) {
        if (MSIMESharedSystemInputModeState().selecting && _owner && ![_owner globalVoiceBusy])
            _state.hostSelectedSource(source.UTF8String ?: "");
        else _state.sourceChanged(source.UTF8String ?: "");
    }
    // 布局变化后重新解码字母键；录音中的改动等这一轮结束再应用。
    if (_state.phase == msime::mac::GlobalVoiceState::Phase::Idle && _enabled && ![_layoutID isEqual:KeyboardLayoutID()]) [self configure];
}
- (void)stop {
    if (_state.phase == msime::mac::GlobalVoiceState::Phase::Waiting) [self cancel];
    else if (_owner && ![_owner globalVoiceProcessing]) { [_owner stopGlobalVoice]; _state.phase = msime::mac::GlobalVoiceState::Phase::Finishing; }
}
- (void)cancel {
    _armed = NO; ++_holdGeneration;
    if (_state.phase == msime::mac::GlobalVoiceState::Phase::Idle && !_owner) return;
    [_owner cancelGlobalVoice];
    [self complete];
}
- (void)complete {
    NSString *source = CurrentSourceID();
    NSString *original = [NSString stringWithUTF8String:_state.original.c_str()];
    BOOL restore = _state.canRestore(source.UTF8String ?: "");
    [_sessionPoll invalidate]; _sessionPoll = nil;
    [_owner endGlobalVoice]; _owner = nil; MSIMEGlobalVoiceHost().owner = nil; MSIMEGlobalVoiceHost().focusIdentity = nil;
    _state.clear(); _application = nil; _focusIdentity = nil; _locked = NO; _requiresHold = NO;
    if (restore) SelectSource(original);
    if (_configurationPending) {
        _configurationPending = NO;
        __weak MSIMEGlobalVoiceCoordinator *weakSelf = self;
        dispatch_async(dispatch_get_main_queue(), ^{ [weakSelf configure]; });
    }
}
- (void)suspended:(NSNotification *)notification { (void)notification; _state.manual = true; [self cancel]; }
- (void)dealloc {
    [_poll invalidate]; [_sessionPoll invalidate]; [self removeKeyboard];
    [NSWorkspace.sharedWorkspace.notificationCenter removeObserver:self];
    [NSDistributedNotificationCenter.defaultCenter removeObserver:self];
}
@end
