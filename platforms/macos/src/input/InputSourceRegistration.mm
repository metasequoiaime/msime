#import "InputSourceRegistration.h"
#import "InputModeIdentifiers.h"
#include <cstring>

@implementation MSIMEInputSourceMonitor {
    NSNotificationCenter *_center;
    NSString *_identifier;
    MSIMEInputSourceCopier _copier;
    MSIMEInputSourcePropertyGetter _getter;
    void (^_action)(void);
}
- (instancetype)initWithCenter:(NSNotificationCenter *)center bundleIdentifier:(NSString *)identifier
                    copySource:(MSIMEInputSourceCopier)copier propertyGetter:(MSIMEInputSourcePropertyGetter)getter
                    switchedAway:(void (^)(void))action {
    self = [super init];
    if (self) {
        _center = center; _identifier = [identifier copy]; _copier = copier; _getter = getter; _action = [action copy];
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
    if (previouslyInstalled) { CFRelease(sources); return noErr; }
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
