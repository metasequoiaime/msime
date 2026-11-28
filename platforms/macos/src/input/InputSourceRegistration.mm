#import "InputSourceRegistration.h"
#import "InputModeIdentifiers.h"
#include "../core/DiagnosticLog.h"
#include <atomic>
#include <cstring>
#include <time.h>

static std::atomic<uint64_t> lastInputMethodActivation{0};

void MSIMENoteInputMethodActivated(void) {
    lastInputMethodActivation.store(clock_gettime_nsec_np(CLOCK_UPTIME_RAW), std::memory_order_relaxed);
}

MSIMEInputSourceSwitchContext MSIMEInputSourceSwitchContextNow(NSString *mode) {
    MSIMEInputSourceSwitchContext context;
    context.secureInput = IsSecureEventInputEnabled();
    // The session dictionary names the process holding secure event input; the key is not in the public headers, but it is the one `ioreg -l -w 0 | grep SecureInput` shows.
    CFDictionaryRef session = CGSessionCopyCurrentDictionary();
    if (session) {
        CFTypeRef pid = CFDictionaryGetValue(session, CFSTR("kCGSSessionSecureInputPID"));
        int value = 0;
        if (pid && CFGetTypeID(pid) == CFNumberGetTypeID() && CFNumberGetValue((CFNumberRef)pid, kCFNumberIntType, &value))
            context.secureInputPID = value;
        CFRelease(session);
    }
    if (context.secureInputPID > 0) {
        NSRunningApplication *owner = [NSRunningApplication runningApplicationWithProcessIdentifier:context.secureInputPID];
        context.secureInputOwner = owner.bundleIdentifier ?: owner.executableURL.lastPathComponent;
    }
    context.frontmostBundle = NSWorkspace.sharedWorkspace.frontmostApplication.bundleIdentifier;
    if (mode.length) {
        NSDictionary *filter = @{(__bridge NSString *)kTISPropertyInputSourceID: mode};
        CFArrayRef sources = TISCreateInputSourceList((__bridge CFDictionaryRef)filter, true);
        if (sources && CFArrayGetCount(sources) > 0) {
            TISInputSourceRef source = (TISInputSourceRef)CFArrayGetValueAtIndex(sources, 0);
            auto flag = [&](CFStringRef key) {
                CFTypeRef value = (CFTypeRef)TISGetInputSourceProperty(source, key);
                return value && CFGetTypeID(value) == CFBooleanGetTypeID() ? (CFBooleanGetValue((CFBooleanRef)value) ? 1 : 0) : -1;
            };
            context.modeEnabled = flag(kTISPropertyInputSourceIsEnabled);
            context.modeSelectable = flag(kTISPropertyInputSourceIsSelectCapable);
        }
        if (sources) CFRelease(sources);
    }
    const uint64_t activated = lastInputMethodActivation.load(std::memory_order_relaxed);
    if (activated) context.msSinceActivation = (long long)((clock_gettime_nsec_np(CLOCK_UPTIME_RAW) - activated) / NSEC_PER_MSEC);
    return context;
}

@implementation MSIMEInputSourceMonitor {
    NSNotificationCenter *_center;
    NSString *_identifier;
    MSIMEInputSourceCopier _copier;
    MSIMEInputSourcePropertyGetter _getter;
    void (^_action)(void);
    // The last of this method's own modes the selection was seen on, for the switched_away line.
    NSString *_lastOwnMode;
}
- (instancetype)initWithCenter:(NSNotificationCenter *)center bundleIdentifier:(NSString *)identifier
                    copySource:(MSIMEInputSourceCopier)copier propertyGetter:(MSIMEInputSourcePropertyGetter)getter
                    switchedAway:(void (^)(void))action {
    self = [super init];
    if (self) {
        _center = center; _identifier = [identifier copy]; _copier = copier; _getter = getter; _action = [action copy];
        _switchContextReader = MSIMEInputSourceSwitchContextNow;
        if (!center || !identifier.length || !copier || !getter || !action) return nil;
        NSString *name = (__bridge NSString *)kTISNotifySelectedKeyboardInputSourceChanged;
        if ([center isKindOfClass:NSDistributedNotificationCenter.class])
            [(NSDistributedNotificationCenter *)center addObserver:self selector:@selector(sourceChanged:) name:name object:nil suspensionBehavior:NSNotificationSuspensionBehaviorDeliverImmediately];
        else [center addObserver:self selector:@selector(sourceChanged:) name:name object:nil];
    }
    return self;
}
- (void)sourceChanged:(NSNotification *)notification {
    (void)notification;
    if (!NSThread.isMainThread) {
        __weak MSIMEInputSourceMonitor *weakSelf = self;
        dispatch_async(dispatch_get_main_queue(), ^{ [weakSelf sourceChanged:nil]; });
        return;
    }
    if (!_center) return;
    // Query current state on delivery; delayed notifications carry no authority.
    TISInputSourceRef source = _copier();
    if (!source) return;
    auto stringProperty = [&](CFStringRef key) -> NSString * {
        void *value = _getter(source, key);
        return value && CFGetTypeID(value) == CFStringGetTypeID() ? (__bridge NSString *)value : nil;
    };
    NSString *bundle = stringProperty(kTISPropertyBundleID);
    NSString *identifier = stringProperty(kTISPropertyInputSourceID);
    // Some keyboard layouts have no bundle identifier. A missing or malformed
    // source is unknown, not evidence that the user switched input methods.
    BOOL away = bundle.length ? ![bundle isEqual:_identifier] :
        (identifier.length && ![identifier isEqual:_identifier] &&
         ![identifier hasPrefix:[_identifier stringByAppendingString:@"."]]);
    NSString *modePrefix = [_identifier stringByAppendingString:@"."];
    if (!away && [identifier hasPrefix:modePrefix]) _lastOwnMode = [identifier copy];
    // Name the source the system moved to, so a report that the input method keeps switching to another one shows in diagnostic.log which source took over and when; the ID is a state label like the others, and the log replaces control bytes and caps its length.
    // The rest tells apart the two causes left open in #11592 when the system keeps moving the selection to ABC: another process holding secure event input (secure=1 and the owner it names), or this method's mode no longer enabled or selectable in the TIS registry (enabled=0 or selectable=0 with secure=0). since_activate_ms shows whether the switch follows our own activation. Each event is capped at 192 bytes, so the app bundle identifiers, whose length is not ours to bound, go on a second line.
    if (away && msime_macos_diagnostic_enabled()) {
        const MSIMEInputSourceSwitchContext context = _switchContextReader ? _switchContextReader(_lastOwnMode) : MSIMEInputSourceSwitchContext{};
        auto tri = [](int value) { return value < 0 ? "-" : value ? "1" : "0"; };
        NSString *mode = [_lastOwnMode hasPrefix:modePrefix] ? [_lastOwnMode substringFromIndex:modePrefix.length] : nil;
        NSString *since = context.msSinceActivation < 0 ? @"-" : [NSString stringWithFormat:@"%lld", context.msSinceActivation];
        msime_macos_diagnostic_writef("input_source switched_away to=%s secure=%d secure_pid=%d mode=%s enabled=%s selectable=%s since_activate_ms=%s",
            (identifier.length ? identifier : bundle).UTF8String, context.secureInput ? 1 : 0, (int)context.secureInputPID,
            mode.length ? mode.UTF8String : "-", tri(context.modeEnabled), tri(context.modeSelectable), since.UTF8String);
        msime_macos_diagnostic_writef("input_source switched_away_apps front=%s secure_owner=%s",
            context.frontmostBundle.length ? context.frontmostBundle.UTF8String : "-",
            context.secureInputOwner.length ? context.secureInputOwner.UTF8String : "-");
    }
    CFRelease(source);
    if (away) _action();
}
- (void)stop { [_center removeObserver:self]; _center = nil; }
- (void)dealloc { [self stop]; }
@end

bool MSIMEShouldRegisterInputSource(int argc, const char *argv[]) {
    return argc == 2 && argv && argv[1] &&
        (std::strcmp(argv[1], "--register-input-source") == 0 ||
         std::strcmp(argv[1], "--reregister-input-source") == 0);
}

bool MSIMEShouldReportInputSourceRegistration(int argc, const char *argv[]) {
    return argc == 2 && argv && argv[1] && std::strcmp(argv[1], "--input-source-registered") == 0;
}

MSIMEInputSourceRegistryState MSIMEInputSourceRegistryStateFor(NSString *bundleIdentifier, MSIMEInputSourceLister lister) {
    if (!bundleIdentifier.length || !lister) return MSIMEInputSourceRegistryStateUnknown;
    NSDictionary *filter = @{(__bridge NSString *)kTISPropertyBundleID: bundleIdentifier,
                             (__bridge NSString *)kTISPropertyInputSourceIsEnableCapable: @YES};
    CFArrayRef sources = lister((__bridge CFDictionaryRef)filter, true);
    // 与登记后的查找一样，NULL 和空列表都算没有。
    const bool listed = sources && CFArrayGetCount(sources) > 0;
    if (sources) CFRelease(sources);
    return listed ? MSIMEInputSourceRegistryStateListed : MSIMEInputSourceRegistryStateMissing;
}

int MSIMEInputSourceRegistryExitCode(MSIMEInputSourceRegistryState state) {
    switch (state) {
    case MSIMEInputSourceRegistryStateListed: return 0;
    case MSIMEInputSourceRegistryStateMissing: return 3;
    case MSIMEInputSourceRegistryStateUnknown: return 1;
    }
    return 1;
}

bool MSIMEShouldReportEnabledInputModes(int argc, const char *argv[]) {
    return argc == 2 && argv && argv[1] && std::strcmp(argv[1], "--enabled-input-modes") == 0;
}

bool MSIMEShouldReportInputSourceEnabled(int argc, const char *argv[]) {
    return argc == 2 && argv && argv[1] && std::strcmp(argv[1], "--input-source-enabled") == 0;
}

NSArray<NSString *> *MSIMEEnabledInputModeIDs(NSString *bundleIdentifier, MSIMEInputSourceLister lister,
                                              MSIMEInputSourcePropertyGetter propertyGetter) {
    if (!bundleIdentifier.length || !lister || !propertyGetter) return nil;
    NSDictionary *filter = @{(__bridge NSString *)kTISPropertyBundleID: bundleIdentifier};
    // Every installed source, then each one's own enabled flag, the way scripts/check_input_source.swift reads it: the list without includeAllInstalled came back without any of this bundle's modes from a command-line run on macOS 27.0.1, while the flag reported the six that are in the input menu.
    CFArrayRef sources = lister((__bridge CFDictionaryRef)filter, true);
    if (!sources) return nil;
    NSString *modePrefix = [bundleIdentifier stringByAppendingString:@"."];
    NSMutableArray<NSString *> *modes = [NSMutableArray array];
    for (CFIndex i = 0; i < CFArrayGetCount(sources); ++i) {
        TISInputSourceRef source = (TISInputSourceRef)CFArrayGetValueAtIndex(sources, i);
        void *property = propertyGetter(source, kTISPropertyInputSourceID);
        void *enabled = propertyGetter(source, kTISPropertyInputSourceIsEnabled);
        if (property && CFGetTypeID(property) == CFStringGetTypeID() &&
            [(__bridge NSString *)property hasPrefix:modePrefix] &&
            enabled && CFGetTypeID(enabled) == CFBooleanGetTypeID() && CFBooleanGetValue((CFBooleanRef)enabled))
            [modes addObject:[(__bridge NSString *)property copy]];
    }
    CFRelease(sources);
    return modes;
}

OSStatus MSIMERegisterInputSource(NSURL *bundleURL, MSIMEInputSourceRegistrar registrar) {
    if (!bundleURL || !registrar) return paramErr;
    return registrar((__bridge CFURLRef)bundleURL);
}

OSStatus MSIMERegisterAndEnableInputSources(NSURL *bundleURL, NSString *bundleIdentifier,
                                            MSIMEInputSourceRegistrar registrar,
                                            MSIMEInputSourceLister lister,
                                            MSIMEInputSourcePropertyGetter propertyGetter,
                                            MSIMEInputSourceEnabler enabler) {
    if (!bundleIdentifier.length || !lister || !propertyGetter || !enabler) return paramErr;
    NSDictionary *filter = @{(__bridge NSString *)kTISPropertyBundleID: bundleIdentifier,
                             (__bridge NSString *)kTISPropertyInputSourceIsEnableCapable: @YES};
    // 登记前已有模式时，它们的启用状态就是用户选择；全部移除也不等于首次安装。
    CFArrayRef before = lister((__bridge CFDictionaryRef)filter, true);
    const BOOL previouslyInstalled = before && CFArrayGetCount(before) > 0;
    if (before) CFRelease(before);
    OSStatus status = MSIMERegisterInputSource(bundleURL, registrar);
    if (status != noErr) return status;
    CFArrayRef sources = lister((__bridge CFDictionaryRef)filter, true);
    if (!sources || CFArrayGetCount(sources) == 0) { if (sources) CFRelease(sources); return fnfErr; }
    if (previouslyInstalled) {
        // 重装要保留用户移除的模式，但顶层输入源是输入法本身。重复安装或旧版本登记留下的顶层源可能已经被系统标成关闭，不能因为它仍在注册表里就静默返回。
        for (CFIndex index = 0; index < CFArrayGetCount(sources); ++index) {
            TISInputSourceRef source = (TISInputSourceRef)CFArrayGetValueAtIndex(sources, index);
            void *identifier = propertyGetter(source, kTISPropertyInputSourceID);
            if (!identifier || CFGetTypeID(identifier) != CFStringGetTypeID() ||
                ![(__bridge NSString *)identifier isEqualToString:bundleIdentifier]) continue;
            void *isEnabled = propertyGetter(source, kTISPropertyInputSourceIsEnabled);
            if (!isEnabled || CFGetTypeID(isEnabled) != CFBooleanGetTypeID() ||
                CFBooleanGetValue((CFBooleanRef)isEnabled)) break;
            status = enabler(source);
            if (status != noErr) { CFRelease(sources); return status; }
            break;
        }
        CFRelease(sources);
        return noErr;
    }
    // 首次安装只提供主模式，其他模式由用户手动添加，避免一次塞进整个输入菜单。
    NSString *primaryID = [bundleIdentifier stringByAppendingString:@".Hans"];
    BOOL enabled = NO;
    for (NSString *identifier in @[bundleIdentifier, primaryID]) {
        for (CFIndex index = 0; index < CFArrayGetCount(sources); ++index) {
            TISInputSourceRef source = (TISInputSourceRef)CFArrayGetValueAtIndex(sources, index);
            void *property = propertyGetter(source, kTISPropertyInputSourceID);
            if (!property || CFGetTypeID(property) != CFStringGetTypeID() ||
                ![(__bridge NSString *)property isEqualToString:identifier]) continue;
            status = enabler(source);
            if (status != noErr) { CFRelease(sources); return status; }
            enabled = YES;
        }
    }
    CFRelease(sources);
    return enabled ? noErr : fnfErr;
}

NSArray<NSString *> *MSIMEEnableNewInputModes(NSString *bundleIdentifier, NSArray<NSString *> *offered,
                                              MSIMEInputSourceLister lister,
                                              MSIMEInputSourcePropertyGetter propertyGetter,
                                              MSIMEInputSourceEnabler enabler) {
    // 保留旧记录的兼容种子；所有新增模式只记录、不自动加入用户的输入源列表。
    NSArray<NSString *> *legacy = MSIMEEditionIsFull() ? @[
        MSIMEChineseInputModeID, MSIMEEnglishInputModeID, MSIMEJapaneseInputModeID, MSIMEKoreanInputModeID
    ] : @[];
    NSMutableOrderedSet<NSString *> *record = [NSMutableOrderedSet orderedSetWithArray:offered ?: legacy];
    if (!bundleIdentifier.length || !lister || !propertyGetter || !enabler) return record.array;
    NSDictionary *filter = @{(__bridge NSString *)kTISPropertyBundleID: bundleIdentifier,
                             (__bridge NSString *)kTISPropertyInputSourceIsEnableCapable: @YES};
    CFArrayRef sources = lister((__bridge CFDictionaryRef)filter, true);
    if (!sources) return record.array;
    NSString *modePrefix = [bundleIdentifier stringByAppendingString:@"."];
    for (CFIndex i = 0; i < CFArrayGetCount(sources); ++i) {
        TISInputSourceRef source = (TISInputSourceRef)CFArrayGetValueAtIndex(sources, i);
        void *property = propertyGetter(source, kTISPropertyInputSourceID);
        if (!property || CFGetTypeID(property) != CFStringGetTypeID()) continue;
        NSString *identifier = (__bridge NSString *)property;
        if (![identifier hasPrefix:modePrefix] || [record containsObject:identifier]) continue;
        // 启动只记下已提供的模式，不补加模式。用户移除过的和升级新增的都由用户自行添加。
        [record addObject:identifier];
    }
    CFRelease(sources);
    return record.array;
}

OSStatus MSIMEEnableInputMode(NSString *identifier, MSIMEInputSourceLister lister, MSIMEInputSourceEnabler enabler) {
    if (!identifier.length || !lister || !enabler) return paramErr;
    // includeAllInstalled, because the mode being enabled is by definition not in the enabled list yet.
    NSDictionary *filter = @{(__bridge NSString *)kTISPropertyInputSourceID: identifier};
    CFArrayRef sources = lister((__bridge CFDictionaryRef)filter, true);
    if (!sources) return fnfErr;
    const OSStatus status = CFArrayGetCount(sources) > 0 ? enabler((TISInputSourceRef)CFArrayGetValueAtIndex(sources, 0)) : fnfErr;
    CFRelease(sources);
    return status;
}

BOOL MSIMEInputSourceIsEnabled(NSString *identifier) {
    if (!identifier.length) return NO;
    // Without includeAllInstalled the list holds only enabled sources.
    NSDictionary *filter = @{(__bridge NSString *)kTISPropertyInputSourceID: identifier};
    CFArrayRef sources = TISCreateInputSourceList((__bridge CFDictionaryRef)filter, false);
    if (!sources) return NO;
    const BOOL enabled = CFArrayGetCount(sources) > 0;
    CFRelease(sources);
    return enabled;
}

void MSIMELaunchInputSourceReregistration(NSURL *bundleURL, NSWorkspace *workspace,
                                          void (^completion)(BOOL launched)) {
    if (!completion) return;
    if (!bundleURL || !bundleURL.isFileURL || !workspace) {
        completion(NO);
        return;
    }
    NSWorkspaceOpenConfiguration *configuration = [NSWorkspaceOpenConfiguration configuration];
    configuration.arguments = @[@"--reregister-input-source"];
    configuration.activates = NO;
    configuration.createsNewApplicationInstance = YES;
    [workspace openApplicationAtURL:bundleURL configuration:configuration
                 completionHandler:^(NSRunningApplication *application, NSError *error) {
        BOOL launched = application != nil && error == nil;
        if (NSThread.isMainThread) completion(launched);
        else dispatch_async(dispatch_get_main_queue(), ^{ completion(launched); });
    }];
}
