#import "GlobalVoiceCoordinator.h"
#import "GlobalVoicePreferenceNotification.h"
#import "GlobalVoiceBackground.h"
#import "GlobalVoiceState.h"
#import "VoiceHoldShortcut.h"
#import "VoiceWaveOverlay.h"
#import "VoiceTextCommit.h"
#import "../core/ChineseTextConversion.h"
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
#import <os/log.h>

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
    if (!MSIMEGlobalVoiceInstalledHost(NSBundle.mainBundle)) { *exitCode = 1; return YES; }
    if (!strcmp(argv[1], "--configure-global-voice")) {
        if (argc != 3 || (strcmp(argv[2], "enable") && strcmp(argv[2], "disable"))) { *exitCode = 2; return YES; }
        SMAppService *service = [SMAppService agentServiceWithPlistName:AgentPlist];
        NSError *error = nil;
        BOOL enabled = !strcmp(argv[2], "enable");
        MSIMEGlobalVoiceBackground *registration = [MSIMEGlobalVoiceBackground new];
        [registration configureService:service enabled:enabled revision:MSIMEGlobalVoiceBackgroundRevision(NSBundle.mainBundle)
                              defaults:NSUserDefaults.standardUserDefaults changed:nil];
        NSDate *registrationDeadline = [NSDate dateWithTimeIntervalSinceNow:10];
        while (registration.busy && registrationDeadline.timeIntervalSinceNow > 0)
            [NSRunLoop.currentRunLoop runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.05]];
        error = registration.error;
        if (registration.busy || error) *exitCode = 1;
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

@interface MSIMEGlobalVoiceCoordinator ()
- (CGEventRef)observe:(CGEventRef)event type:(CGEventType)type;
- (void)toggle;
@end

@implementation MSIMEGlobalVoiceCoordinator {
    id<MSIMEGlobalVoiceTarget> (^_target)(void);
    id<MSIMEGlobalVoiceDeliveryTarget> (^_deliveryTarget)(void);
    id<MSIMEGlobalVoiceTarget> _owner;
    NSTimer *_poll;
    NSTimer *_sessionPoll;
    CFMachPortRef _tap;
    CFRunLoopSourceRef _tapSource;
    EventHandlerRef _hotkeyHandler;
    std::vector<EventHotKeyRef> _hotkeys;
    NSUInteger _keysDown;
    NSUInteger _registeredMask;
    NSMutableDictionary<NSNumber *, NSNumber *> *_hotkeyConflicts;
    NSMutableSet<NSNumber *> *_attemptedHotkeys;
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
    BOOL _reloadPending;
    BOOL _enabled;
    BOOL _permissionAsked;
    BOOL _holding;
    BOOL _armed;
    BOOL _spaceDown;
    BOOL _escapeDown;
    BOOL _locked;
    NSUInteger _holdGeneration;
    MSIMEVoiceHoldShortcut _gesture;
    msime::mac::GlobalVoiceState _state;
    MSIMEVoiceWaveOverlay *_failure;
    BOOL _serviceErrorShown;
    MSIMEGlobalVoiceBackground *_backgroundRegistration;
    SMAppServiceStatus _backgroundStatus;
    BOOL _holdStartedSession;
    BOOL _requiresHold;
    BOOL _trusted;
    BOOL _configurationPending;
    NSString *_layoutID;
    NSTimeInterval _nextListenerRetry;
    BOOL _listenerErrorShown;
    BOOL _snapshotErrorShown;
    BOOL _starting;
    NSInteger _waitReason;
    NSString *_pendingText;
    NSString *_commitMode;
    void (^_deliveryEnd)(void);
    NSTimeInterval _deliveryDeadline;
    BOOL _deliveryStarted;
    BOOL _deliveryIMKStarted;
    BOOL _deliveryAttempting;
    BOOL _deliveryRestore;
}
- (instancetype)initWithTarget:(id<MSIMEGlobalVoiceTarget> (^)(void))target {
    return [self initWithTarget:target deliveryTarget:^{ return (id<MSIMEGlobalVoiceDeliveryTarget>)target(); }];
}
- (instancetype)initWithTarget:(id<MSIMEGlobalVoiceTarget> (^)(void))target
                deliveryTarget:(id<MSIMEGlobalVoiceDeliveryTarget> (^)(void))deliveryTarget {
    if (!(self = [super init])) return nil;
    _hotkeyConflicts = [NSMutableDictionary dictionary];
    _target = [target copy];
    _deliveryTarget = [deliveryTarget copy];
    __weak MSIMEGlobalVoiceCoordinator *weakSelf = self;
    MSIMEGlobalVoiceHost().deliver = ^(NSString *text, NSString *mode, void (^completion)(void)) {
        [weakSelf deliver:text mode:mode completion:completion];
    };
    MSIMEGlobalVoiceHost().toggleCurrent = ^BOOL {
        MSIMEGlobalVoiceCoordinator *self = weakSelf;
        if (!self || !self->_owner || ![self->_owner globalVoiceBusy]) return NO;
        [self toggle]; return YES;
    };
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
        if (self && self->_owner && (self->_deliveryStarted || ![self->_owner globalVoiceBusy]))
            self->_state.hostSelectedSource(mode.UTF8String ?: "");
    };
    [self schedulePreferencePolling:30];
    [NSNotificationCenter.defaultCenter addObserver:self selector:@selector(preferencesChanged:)
        name:MSIMEGlobalVoicePreferencesDidChangeNotification() object:nil];
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
- (BOOL)accessibilityTrusted { return AXIsProcessTrustedWithOptions(nullptr); }
- (BOOL)keyboardListenerAvailable { return _tap && CFMachPortIsValid(_tap); }
- (NSUserDefaults *)backgroundDefaults { return NSUserDefaults.standardUserDefaults; }
- (NSString *)backgroundRevision { return MSIMEGlobalVoiceBackgroundRevision(NSBundle.mainBundle); }
- (BOOL)ownsBackgroundHost { return MSIMEGlobalVoiceInstalledHost(NSBundle.mainBundle); }
- (SMAppService *)backgroundService { return [SMAppService agentServiceWithPlistName:AgentPlist]; }
- (void)schedulePreferencePolling:(NSTimeInterval)interval {
    if (_poll.valid && _poll.timeInterval == interval) return;
    [_poll invalidate];
    __weak MSIMEGlobalVoiceCoordinator *weakSelf = self;
    _poll = [NSTimer scheduledTimerWithTimeInterval:interval repeats:YES block:^(NSTimer *timer) { (void)timer; [weakSelf reload]; }];
}
- (void)preferencesChanged:(NSNotification *)notification {
    (void)notification;
    if (NSThread.isMainThread) {
        if (_loading) _reloadPending = YES;
        else [self reload];
    }
    else {
        __weak MSIMEGlobalVoiceCoordinator *weakSelf = self;
        dispatch_async(dispatch_get_main_queue(), ^{ [weakSelf preferencesChanged:nil]; });
    }
}
- (void)applySnapshot:(NSDictionary *)snapshot {
    NSDictionary *voice = snapshot[@"preferences"][@"voice_input"];
    const BOOL wanted = [voice[@"hotkey_global"] isEqual:@YES] && ![voice[@"enabled"] isEqual:@NO];
    // 关闭时只保留外部写入的低频兜底；已启用时及时观察批准撤销与监听重试。
    if (snapshot) [self schedulePreferencePolling:wanted ? 1 : 30];
    const BOOL trusted = (_enabled || wanted) ? [self accessibilityTrusted] : _trusted;
    const BOOL trustChanged = _trusted != trusted;
    _trusted = trusted;
    if (!snapshot) {
        // 配置文件暂不可读也不能忽略系统撤销批准或辅助功能的状态变化。
        if (_enabled && (trustChanged || [self backgroundService].status != _backgroundStatus)) [self configure];
        if (!_snapshotErrorShown) os_log_error(OS_LOG_DEFAULT, "MSIME global voice: preferences unavailable");
        _snapshotErrorShown = YES;
        return;
    }
    _snapshotErrorShown = NO;
    if ([_revision isEqual:snapshot[@"revision"]]) {
        // 权限在设置页获准后可能仍有短暂的监听失败；偏好未变也要有界重试，不重复弹授权框。
        const BOOL retry = NSProcessInfo.processInfo.systemUptime >= _nextListenerRetry;
        if ((_backgroundRegistration.error && retry) || (_enabled && (trustChanged ||
            [self backgroundService].status != _backgroundStatus ||
            (trusted && ![self keyboardListenerAvailable] && retry)))) [self configure];
        return;
    }
    _revision = snapshot[@"revision"];
    _preferences = snapshot[@"preferences"];
    if (_enabled || [_preferences[@"voice_input"][@"hotkey_global"] isEqual:@YES])
        MSIMEApplySharedVoicePreferences(_preferences[@"voice_input"], NSUserDefaults.standardUserDefaults);
    [self configure];
}
- (void)completePreferenceReload:(NSDictionary *)snapshot {
    _loading = NO;
    [self applySnapshot:snapshot];
    // 保存可能发生在旧读取开始之后，保留一次追读，不能再等三十秒兜底。
    if (_reloadPending) { _reloadPending = NO; [self reload]; }
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
            [self completePreferenceReload:snapshot];
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
    if (![self ownsBackgroundHost]) {
        [self cancel]; [self removeKeyboard]; _enabled = NO;
        return;
    }
    NSDictionary *voice = _preferences[@"voice_input"];
    BOOL enabled = [voice[@"hotkey_global"] isEqual:@YES] && ![voice[@"enabled"] isEqual:@NO];
    const BOOL trusted = [self accessibilityTrusted];
    SMAppService *service = [self backgroundService];
    const BOOL approved = service.status == SMAppServiceStatusEnabled;
    os_log(OS_LOG_DEFAULT, "MSIME global voice: configure enabled=%{public}d accessibility=%{public}d", enabled, trusted);
    if (_state.phase != msime::mac::GlobalVoiceState::Phase::Idle) {
        // 识别中刷新偏好不能丢掉尚未到来的松手沿；这轮结束后再换注册与手势状态。
        if (enabled && trusted && approved) { _configurationPending = YES; return; }
        if (!trusted || !approved) [self cancel];
        else [self stop];
    }
    _configurationPending = NO;
    [self removeKeyboard];
    _enabled = enabled;
    _layoutID = KeyboardLayoutID();
    _trusted = [self accessibilityTrusted];
    _nextListenerRetry = NSProcessInfo.processInfo.systemUptime + 5;
    if (!_backgroundRegistration) _backgroundRegistration = [MSIMEGlobalVoiceBackground new];
    __weak MSIMEGlobalVoiceCoordinator *weakSelf = self;
    [_backgroundRegistration configureService:service enabled:enabled revision:[self backgroundRevision] defaults:[self backgroundDefaults]
        changed:^{
            MSIMEGlobalVoiceCoordinator *self = weakSelf;
            if (!self) return;
            self->_backgroundStatus = [self backgroundService].status;
            if (self->_backgroundRegistration.error) {
                self->_nextListenerRetry = NSProcessInfo.processInfo.systemUptime + 5;
                return;
            }
            [self configure];
        }];
    NSError *serviceError = _backgroundRegistration.error;
    _backgroundStatus = service.status;
    os_log(OS_LOG_DEFAULT, "MSIME global voice: background status=%{public}ld error=%{public}ld", (long)service.status, (long)serviceError.code);
    if (enabled && !_serviceErrorShown && (serviceError || service.status != SMAppServiceStatusEnabled)) {
        _serviceErrorShown = YES;
        [self showFailure:@"后台语音尚未获准。请在系统设置的「通用 > 登录项与扩展」允许水杉后台运行。"];
    }
    if (!enabled) { [_hotkeyConflicts removeAllObjects]; _permissionAsked = NO; _serviceErrorShown = NO; _listenerErrorShown = NO; _keysDown = 0; return; }
    // 注册失败、更新正在迁移或用户撤销后台批准时，全局入口都必须关闭；局部 IMK 快捷键继续可用。
    if (_backgroundRegistration.busy || serviceError || service.status != SMAppServiceStatusEnabled) return;
    _serviceErrorShown = NO;
    if (!_trusted) {
        if (!_permissionAsked) {
            _permissionAsked = YES;
            AXIsProcessTrustedWithOptions((__bridge CFDictionaryRef)@{(__bridge NSString *)kAXTrustedCheckOptionPrompt: @YES});
            [self showFailure:@"跨输入法语音需要辅助功能权限。请在系统设置中允许水杉输入法，授权后会自动重试。"];
        }
        return;
    }
    [self installKeyboardForVoice:voice];
}
- (void)installKeyboardForVoice:(NSDictionary *)voice {
    _tap = CGEventTapCreate(kCGSessionEventTap, kCGHeadInsertEventTap, kCGEventTapOptionDefault,
        CGEventMaskBit(kCGEventFlagsChanged) | CGEventMaskBit(kCGEventKeyDown) | CGEventMaskBit(kCGEventKeyUp),
        [](CGEventTapProxy proxy, CGEventType type, CGEventRef event, void *context) -> CGEventRef {
            (void)proxy;
            return [(__bridge MSIMEGlobalVoiceCoordinator *)context observe:event type:type];
        }, (__bridge void *)self);
    os_log(OS_LOG_DEFAULT, "MSIME global voice: listener created=%{public}d", _tap != nullptr);
    if (!_tap) {
        if (!_listenerErrorShown) [self showFailure:@"无法监听全局语音键。请检查辅助功能与输入监控权限。授权后会自动重试。"];
        _listenerErrorShown = YES;
        return;
    }
    _listenerErrorShown = NO;
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
    [self beginHotkeyRegistration];
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
    [self finishHotkeyRegistration];
    MSIMEGlobalVoiceHost().keyboard = true;
}
- (void)beginHotkeyRegistration { _attemptedHotkeys = [NSMutableSet set]; }
- (void)finishHotkeyRegistration {
    for (NSNumber *identifier in _hotkeyConflicts.allKeys)
        if (![_attemptedHotkeys containsObject:identifier]) [_hotkeyConflicts removeObjectForKey:identifier];
    _attemptedHotkeys = nil;
}
- (BOOL)systemHotkeyConflictForCode:(UInt32)code modifiers:(UInt32)modifiers {
    return SystemHotkeyConflict(code, modifiers);
}
- (OSStatus)registerSystemHotkeyCode:(UInt32)code modifiers:(UInt32)modifiers identifier:(UInt32)identifier key:(EventHotKeyRef *)key {
    return RegisterEventHotKey(code, modifiers, {'MSVG', identifier}, GetApplicationEventTarget(), 0, key);
}
- (void)registerCode:(UInt32)code modifiers:(UInt32)modifiers identifier:(UInt32)identifier {
    EventHotKeyRef key = nullptr;
    NSNumber *binding = @(((uint64_t)modifiers << 32) | code);
    [_attemptedHotkeys addObject:@(identifier)];
    if ([self systemHotkeyConflictForCode:code modifiers:modifiers] || [self registerSystemHotkeyCode:code modifiers:modifiers identifier:identifier key:&key] != noErr) {
        // 偏好、布局或权限重配仍要重试注册，但同一冲突只提示一次。
        if (![_hotkeyConflicts[@(identifier)] isEqual:binding])
            [self showFailure:@"语音组合键与系统或其他应用冲突。请在快捷键设置中更换现有语音绑定。"];
        _hotkeyConflicts[@(identifier)] = binding;
        return;
    }
    [_hotkeyConflicts removeObjectForKey:@(identifier)];
    _hotkeyCodes[identifier] = code; _hotkeyModifiers[identifier] = modifiers;
    if (key) _hotkeys.push_back(key);
    _registeredMask |= 1u << identifier;
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
    NSRunningApplication *application = [self frontApplication];
    if (![application.bundleIdentifier isEqual:MSIMESettingsBundleIdentifier()]) return NO;
    id focus = [self focusIdentity];
    if (!focus) return NO;
    CFTypeRef label = nullptr;
    AXUIElementCopyAttributeValue((__bridge AXUIElementRef)focus, kAXDescriptionAttribute, &label);
    BOOL recording = label && CFEqual(label, CFSTR("正在录制快捷键"));
    if (label) CFRelease(label);
    return recording;
}
- (void)replayKey:(UInt32)identifier down:(BOOL)down {
    NSRunningApplication *application = [self frontApplication];
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
        os_log(OS_LOG_DEFAULT, "MSIME global voice: listener was disabled, rearming");
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
    NSDictionary *voice = _preferences[@"voice_input"];
    MSIMEVoiceHoldShortcut::Options options = {![voice[@"hotkey_ralt"] isEqual:@NO], [voice[@"hotkey_ctrl_win"] isEqual:@YES] != NO, [voice[@"hotkey_rctrl_ralt"] isEqual:@YES] != NO, true};
    if (code == 49 && _holding && _owner && _state.phase == msime::mac::GlobalVoiceState::Phase::Active) {
        if (type == kCGEventKeyDown) {
            // 外层吞掉空格时，内部手势也必须收到锁定，才能保留真正的修饰键松手沿。
            _gesture.observe(native, options, true, CGEventSourceFlagsState(kCGEventSourceStateHIDSystemState));
            _spaceDown = YES; _locked = YES; [_owner lockGlobalVoice]; return nullptr;
        }
    }
    if (type == kCGEventKeyUp && code == 49 && _spaceDown) {
        _gesture.observe(native, options, _owner != nil, CGEventSourceFlagsState(kCGEventSourceStateHIDSystemState));
        _spaceDown = NO; return nullptr;
    }
    if (type == kCGEventKeyDown && _holding) {
        _armed = NO; ++_holdGeneration;
        if (!_locked) [self queueGestureAction:0];
        // 修饰键已按原样交给应用，此刻的普通组合键也原样交付。
    }
    auto result = _gesture.observe(native, options, _owner != nil, CGEventSourceFlagsState(kCGEventSourceStateHIDSystemState));
    if (result.action == MSIMEVoiceHoldShortcut::Action::Toggle && !result.onRelease) {
        _holding = YES;
        _holdStartedSession = !_owner;
        if (_owner) { [self queueGestureAction:2]; return event; }
        _armed = YES;
        os_log(OS_LOG_DEFAULT, "MSIME global voice: hold armed");
        const NSUInteger generation = ++_holdGeneration;
        __weak MSIMEGlobalVoiceCoordinator *weakSelf = self;
        dispatch_after(dispatch_time(DISPATCH_TIME_NOW, 250 * NSEC_PER_MSEC), dispatch_get_main_queue(), ^{
            MSIMEGlobalVoiceCoordinator *self = weakSelf;
            if (self && self->_enabled && self->_armed && generation == self->_holdGeneration) {
                os_log(OS_LOG_DEFAULT, "MSIME global voice: hold admitted");
                self->_armed = NO; [self begin:YES];
            }
        });
    }
    if (result.action == MSIMEVoiceHoldShortcut::Action::ChordCancel) {
        _holding = NO; _armed = NO; _holdStartedSession = NO; ++_holdGeneration;
    }
    if (result.onRelease) {
        os_log(OS_LOG_DEFAULT, "MSIME global voice: hold released");
        _holding = NO; _armed = NO; ++_holdGeneration;
        if (!_locked && (_holdStartedSession || result.action == MSIMEVoiceHoldShortcut::Action::Toggle)) [self queueGestureAction:1];
        _holdStartedSession = NO;
    }
    return event;
}
- (void)toggle {
    if (!_enabled && !_owner) return;
    if (_state.phase == msime::mac::GlobalVoiceState::Phase::Idle) [self begin:NO];
    else if (_state.phase == msime::mac::GlobalVoiceState::Phase::Finishing || [_owner globalVoiceProcessing]) [self cancel];
    else [self stop];
}
- (NSTimeInterval)monotonicTime { return NSProcessInfo.processInfo.systemUptime; }
- (BOOL)isOwnSourceIdentifier:(NSString *)identifier { return IsOurs(identifier); }
- (NSString *)currentSourceIdentifier { return CurrentSourceID(); }
- (NSString *)enabledSourceIdentifier { return EnabledSource(); }
- (BOOL)selectSourceIdentifier:(NSString *)identifier { return SelectSource(identifier); }
- (id)focusIdentity { return MSIMEGlobalVoiceFocusIdentity(); }
- (NSRunningApplication *)frontApplication { return NSWorkspace.sharedWorkspace.frontmostApplication; }
- (void)begin:(BOOL)holding {
    os_log(OS_LOG_DEFAULT, "MSIME global voice: begin requested phase=%{public}d", (int)_state.phase);
    if ([self shortcutRecorderFocused]) return;
    if (_starting || _state.phase != msime::mac::GlobalVoiceState::Phase::Idle) return;
    if (_backgroundRegistration && (_backgroundRegistration.busy || _backgroundRegistration.error ||
        [self backgroundService].status != SMAppServiceStatusEnabled)) return;
    id<MSIMEGlobalVoiceDeliveryTarget> local = _deliveryTarget();
    if ([local respondsToSelector:@selector(toggleLocalVoiceIfBusy)] && [local toggleLocalVoiceIfBusy]) return;
    NSString *source = [self currentSourceIdentifier] ?: @"";
    id<MSIMEGlobalVoiceTarget> target = _target();
    if ([target globalVoiceBusy] || [_deliveryTarget() globalVoiceBusy]) return;
    if (!_state.begin(source.UTF8String, [self frontApplication].processIdentifier, [self monotonicTime])) return;
    _holding = holding; _requiresHold = holding; _locked = NO; _deliveryRestore = NO;
    _waitReason = -1;
    // 开始录音不读取目的地；最终文字按当时活动的输入客户端交付。
    _state.selected = source.UTF8String;
    __weak MSIMEGlobalVoiceCoordinator *weakSelf = self;
    _sessionPoll = [NSTimer scheduledTimerWithTimeInterval:0.05 repeats:YES block:^(NSTimer *timer) { (void)timer; [weakSelf advance]; }];
    [self advance];
}
// 1 切源、2 控制器、5 忙；不记录元素身份或输入。
- (void)waitingFor:(NSInteger)reason {
    if (_waitReason == reason) return;
    _waitReason = reason;
    os_log(OS_LOG_DEFAULT, "MSIME global voice: waiting gate=%{public}ld", (long)reason);
}
- (void)advance {
    if (_starting) return;
    using Phase = msime::mac::GlobalVoiceState::Phase;
    NSString *source = [self currentSourceIdentifier];
    if (_state.phase == Phase::Waiting) {
        if (_requiresHold && !_holding && !_locked) { [self cancel]; return; }
        id<MSIMEGlobalVoiceTarget> target = _target();
        if (target && ![target globalVoiceBusy]) {
            const auto generation = _state.generation;
            _owner = target; MSIMEGlobalVoiceHost().owner = target;
            MSIMEGlobalVoiceHost().sessionToken = [NSObject new];
            _starting = YES;
            const auto result = [target startGlobalVoiceHolding:_holding];
            _starting = NO;
            os_log(OS_LOG_DEFAULT, "MSIME global voice: capture start outcome=%{public}ld", (long)result);
            if (_state.generation != generation || _state.phase != Phase::Waiting || _owner != target) return;
            if (result == MSIMEGlobalVoiceStartAccepted) {
                NSString *activeSource = [self currentSourceIdentifier];
                _state.activate(_state.targetPID, (activeSource ?: source ?: @"").UTF8String);
            } else if (result == MSIMEGlobalVoiceStartFailed) {
                [self complete];
            } else {
                [target endGlobalVoice];
                _owner = nil; MSIMEGlobalVoiceHost().owner = nil;
                MSIMEGlobalVoiceHost().sessionToken = nil;
                if (_state.timedOut([self monotonicTime])) { [self cancel]; [self showFailure:@"录音服务尚未就绪，请重试。"]; }
            }
        } else {
            [self waitingFor:!target ? 2 : 5];
            if (_state.timedOut([self monotonicTime])) {
                [self cancel]; [self showFailure:@"录音服务尚未就绪，请重试。"];
            }
        }
        return;
    }
    if (_pendingText) { [self advanceDelivery]; return; }
    if (_owner && ![_owner globalVoiceBusy]) { [self complete]; return; }
    if (_owner && [_owner globalVoiceProcessing]) _state.phase = Phase::Finishing;
}
- (void)sourceChanged:(NSNotification *)notification {
    (void)notification;
    NSString *source = [self currentSourceIdentifier];
    if (_deliveryStarted) {
        if (!_state.selectedObserved && _state.original == (source.UTF8String ?: "")) {
            // 切源请求被接受后仍可能先收到原源通知，等真实的 IMK 激活。
        } else if (MSIMESharedSystemInputModeState().selecting && _owner)
            _state.hostSelectedSource(source.UTF8String ?: "");
        else _state.sourceChanged(source.UTF8String ?: "");
    }
    if (_state.phase == msime::mac::GlobalVoiceState::Phase::Idle && _enabled && ![_layoutID isEqual:KeyboardLayoutID()]) [self configure];
}
- (void)deliver:(NSString *)text mode:(NSString *)mode completion:(void (^)(void))completion {
    if (!_owner || !text.length || text.length > 65536 || _pendingText) { if (completion) completion(); return; }
    _pendingText = [text copy]; _commitMode = MSIMEVoiceCommitMode(mode); _deliveryEnd = [completion copy];
    [self advanceDelivery];
}
- (void)finishDelivery:(BOOL)delivered {
    void (^completion)(void) = _deliveryEnd;
    _pendingText = nil; _commitMode = nil; _deliveryEnd = nil; _deliveryStarted = NO; _deliveryIMKStarted = NO; _deliveryDeadline = 0;
    if (completion) completion();
    [self complete];
    if (!delivered) [self showFailure:@"未能插入语音结果，请重试。"];
}
- (MSIMEVoiceCommitOutcome)postGlobalVoiceText:(NSString *)text route:(const MSIMEVoiceCommitRoute &)route {
    return route.deliver(text);
}
- (MSIMEVoiceCommitOutcome)postPendingVoiceUsingMode:(NSString *)mode token:(id)token {
    const auto generation = _state.generation;
    NSRunningApplication *application = [self frontApplication];
    MSIMEVoiceCommitRoute route;
    route.mode = mode; route.pid = application.processIdentifier;
    route.current = [self, token, generation, application] {
        return MSIMEGlobalVoiceHost().sessionToken == token && _state.generation == generation &&
            _owner && [_owner globalVoiceBusy] && !application.terminated && application.processIdentifier > 0 &&
            [self frontApplication].processIdentifier == application.processIdentifier;
    };
    id setting = _preferences[@"traditional_chinese_output"];
    const BOOL valid = [setting isKindOfClass:NSNumber.class] && CFGetTypeID((__bridge CFTypeRef)setting) == CFBooleanGetTypeID();
    const BOOL traditional = valid ? [setting boolValue] : [NSUserDefaults.standardUserDefaults boolForKey:@"MSIMEClientTraditionalOutput"];
    NSString *text = traditional ? MSIMEChineseOutputString(_pendingText, YES) : _pendingText;
    const auto outcome = [self postGlobalVoiceText:text route:route];
    if (_state.generation == generation && MSIMEGlobalVoiceHost().sessionToken == token && _pendingText &&
        outcome == MSIMEVoiceCommitOutcome::posted && MSIMEGlobalVoiceHost().didCommit)
        MSIMEGlobalVoiceHost().didCommit(text);
    return outcome;
}
- (void)finishUnavailableIMK {
    // 只为默认全局会话补上没有 IMK 客户端的通道；不越过现有组字或用户明确选择的策略。
    if (_state.manual || ![_commitMode isEqual:@"tsf"] || _deliveryTarget()) { [self finishDelivery:NO]; return; }
    const auto generation = _state.generation;
    id token = MSIMEGlobalVoiceHost().sessionToken;
    const auto outcome = [self postPendingVoiceUsingMode:@"sendinput" token:token];
    if (_state.generation != generation || MSIMEGlobalVoiceHost().sessionToken != token || !_pendingText) return;
    os_log(OS_LOG_DEFAULT, "MSIME global voice: missing IMK client fallback outcome=%{public}d", (int)outcome);
    [self finishDelivery:outcome == MSIMEVoiceCommitOutcome::posted];
}
- (void)advanceDelivery {
    if (!_pendingText || _deliveryAttempting) return;
    // 同步 IMK 调用可能泵送主循环；一次交付尝试结束前不重复准备或发键。
    struct DeliveryAttempt {
        BOOL &active;
        ~DeliveryAttempt() { active = NO; }
    } attempt{_deliveryAttempting};
    _deliveryAttempting = YES;
    if (![_owner globalVoiceBusy]) { [self complete]; return; }
    id token = MSIMEGlobalVoiceHost().sessionToken;
    const auto generation = _state.generation;
    if (_holding) return;
    if (_deliveryStarted && _state.manual) { [self finishDelivery:NO]; return; }
    if (!_deliveryStarted) {
        _deliveryStarted = YES;
        _deliveryDeadline = [self monotonicTime] + 3;
        NSString *source = [self currentSourceIdentifier] ?: @"";
        // 只恢复交付时临时替换的输入源，录音期间用户的新选择就是本轮恢复基线。
        _state.original = source.UTF8String; _state.selected = source.UTF8String;
        _state.manual = false; _state.selectedObserved = false;
    }
    if (!_deliveryIMKStarted) {
        NSString *source = [self currentSourceIdentifier] ?: @"";
        if (![_commitMode isEqual:@"tsf"]) {
            id<MSIMEGlobalVoiceDeliveryTarget> target = _deliveryTarget();
            const BOOL prepared = ![self isOwnSourceIdentifier:source] ||
                ([target respondsToSelector:@selector(prepareGlobalVoiceTextForCommit:)] && [target prepareGlobalVoiceTextForCommit:token]);
            if (_state.generation != generation || MSIMEGlobalVoiceHost().sessionToken != token || !_pendingText) return;
            if (!prepared) {
                // 准备暂不可用仍重试用户所选策略，不把它当作发键能力不可用。
                if ([self monotonicTime] >= _deliveryDeadline) [self finishDelivery:NO];
                return;
            }
            const auto outcome = [self postPendingVoiceUsingMode:_commitMode token:token];
            if (_state.generation != generation || MSIMEGlobalVoiceHost().sessionToken != token || !_pendingText) return;
            if (outcome != MSIMEVoiceCommitOutcome::unavailable) { [self finishDelivery:outcome == MSIMEVoiceCommitOutcome::posted]; return; }
        }
        _deliveryIMKStarted = YES;
        NSString *selected = [self enabledSourceIdentifier];
        if (!selected) { [self finishUnavailableIMK]; return; }
        MSIMEGlobalVoiceHost().delivering = true;
        _state.hostSelectedSource(selected.UTF8String);
        _deliveryRestore = ![source isEqual:selected];
        if (_deliveryRestore) {
            os_log(OS_LOG_DEFAULT, "MSIME global voice: delivery source requested");
            if (![self selectSourceIdentifier:selected]) { [self finishUnavailableIMK]; return; }
        }
        if (_state.generation != generation || MSIMEGlobalVoiceHost().sessionToken != token || !_pendingText) return;
    }
    NSString *source = [self currentSourceIdentifier];
    const BOOL arrived = [self isOwnSourceIdentifier:source];
    if (arrived) _state.selectedObserved = true;
    id<MSIMEGlobalVoiceDeliveryTarget> target = arrived ? _deliveryTarget() : nil;
    const auto result = [target respondsToSelector:@selector(commitGlobalVoiceText:token:)]
        ? [target commitGlobalVoiceText:_pendingText token:token] : MSIMEGlobalVoiceDeliveryRetry;
    if (_state.generation != generation || MSIMEGlobalVoiceHost().sessionToken != token || !_pendingText) return;
    if (result == MSIMEGlobalVoiceDeliveryAccepted) [self finishDelivery:YES];
    else if ([self monotonicTime] >= _deliveryDeadline) {
        os_log(OS_LOG_DEFAULT, "MSIME global voice: delivery target unavailable");
        [self finishUnavailableIMK];
    } else [self waitingFor:!arrived ? 1 : !target ? 2 : 5];
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
    os_log(OS_LOG_DEFAULT, "MSIME global voice: complete phase=%{public}d owner=%{public}d busy=%{public}d",
        (int)_state.phase, _owner != nil, _owner && [_owner globalVoiceBusy]);
    NSString *source = [self currentSourceIdentifier];
    NSString *original = [NSString stringWithUTF8String:_state.original.c_str()];
    BOOL restore = _deliveryRestore && _state.canRestore(source.UTF8String ?: "");
    _pendingText = nil; _commitMode = nil; _deliveryEnd = nil; _deliveryStarted = NO; _deliveryIMKStarted = NO; _deliveryDeadline = 0;
    MSIMEGlobalVoiceHost().delivering = false;
    [_sessionPoll invalidate]; _sessionPoll = nil;
    [_owner endGlobalVoice]; _owner = nil; MSIMEGlobalVoiceHost().owner = nil;
    MSIMEGlobalVoiceHost().sessionToken = nil;
    _state.clear(); _locked = NO; _requiresHold = NO; _deliveryRestore = NO;
    if (restore) [self selectSourceIdentifier:original];
    if (_configurationPending) {
        _configurationPending = NO;
        __weak MSIMEGlobalVoiceCoordinator *weakSelf = self;
        dispatch_async(dispatch_get_main_queue(), ^{ [weakSelf configure]; });
    }
}
- (void)suspended:(NSNotification *)notification { (void)notification; [self cancel]; }
- (void)dealloc {
    [_poll invalidate]; [_sessionPoll invalidate]; [self removeKeyboard];
    [NSWorkspace.sharedWorkspace.notificationCenter removeObserver:self];
    [NSDistributedNotificationCenter.defaultCenter removeObserver:self];
    [NSNotificationCenter.defaultCenter removeObserver:self];
}
@end
