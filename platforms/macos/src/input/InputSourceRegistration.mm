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
    OSStatus status = MSIMERegisterInputSource(bundleURL, registrar);
    if (status != noErr) return status;
    NSDictionary *filter = @{(__bridge NSString *)kTISPropertyBundleID: bundleIdentifier,
                             (__bridge NSString *)kTISPropertyInputSourceIsEnableCapable: @YES};
    CFArrayRef sources = lister((__bridge CFDictionaryRef)filter, true);
    if (!sources || CFArrayGetCount(sources) == 0) { if (sources) CFRelease(sources); return fnfErr; }
    bool enabled = false;
    TISInputSourceRef primary = nullptr;
    for (CFIndex i = 0; i < CFArrayGetCount(sources); ++i) {
        TISInputSourceRef source = (TISInputSourceRef)CFArrayGetValueAtIndex(sources, i);
        void *property = propertyGetter(source, kTISPropertyInputSourceID);
        if (property && CFGetTypeID(property) == CFStringGetTypeID() &&
            [(__bridge NSString *)property isEqualToString:bundleIdentifier]) {
            status = enabler(source); if (status != noErr) { CFRelease(sources); return status; }
            if (!primary) primary = source;
            enabled = true;
        }
    }
    // A bundle with visible ComponentInputModeDict entries does not need a separate top-level
    // TISInputSourceID. In that shape macOS exposes the mode as the selectable source, which avoids
    // showing the bundle and its only mode as two identically named menu entries.
    if (!enabled) {
        NSString *modePrefix = [bundleIdentifier stringByAppendingString:@"."];
        for (CFIndex i = 0; i < CFArrayGetCount(sources); ++i) {
            TISInputSourceRef source = (TISInputSourceRef)CFArrayGetValueAtIndex(sources, i);
            void *property = propertyGetter(source, kTISPropertyInputSourceID);
            if (!property || CFGetTypeID(property) != CFStringGetTypeID() ||
                ![(__bridge NSString *)property hasPrefix:modePrefix] ||
                MSIMEIsOptInInputModeID((__bridge NSString *)property)) continue;
            status = enabler(source); if (status != noErr) { CFRelease(sources); return status; }
            primary = source;
            enabled = true;
            break;
        }
    }
    if (!enabled) { CFRelease(sources); return fnfErr; }
    // The opt-in modes are left as registration found them: off on a fresh install, and on only where the user picked their scheme before.
    for (CFIndex i = 0; i < CFArrayGetCount(sources); ++i) {
        TISInputSourceRef source = (TISInputSourceRef)CFArrayGetValueAtIndex(sources, i);
        if (source == primary) continue;
        void *property = propertyGetter(source, kTISPropertyInputSourceID);
        const BOOL identified = property && CFGetTypeID(property) == CFStringGetTypeID();
        if (identified && MSIMEIsOptInInputModeID((__bridge NSString *)property)) continue;
        if (!identified || ![(__bridge NSString *)property isEqualToString:bundleIdentifier]) {
            status = enabler(source); if (status != noErr) { CFRelease(sources); return status; }
        }
    }
    CFRelease(sources); return noErr;
}

NSArray<NSString *> *MSIMEEnableNewInputModes(NSString *bundleIdentifier, NSArray<NSString *> *offered,
                                              MSIMEInputSourceLister lister,
                                              MSIMEInputSourcePropertyGetter propertyGetter,
                                              MSIMEInputSourceEnabler enabler) {
    // 没有记录说明这是第一次留记录的启动。full 在开始记录之前的每次安装都登记并启用过中、英、日、韩，它们算作已经提供过，用户移除过的就保持移除；之后新增的模式才是没重新登记的更新漏掉的。其他版本从一开始就留记录，没有这样的历史，第一次启动把本版本的模式各启用一次。
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
        void *enabled = propertyGetter(source, kTISPropertyInputSourceIsEnabled);
        const BOOL alreadyEnabled = enabled && CFGetTypeID(enabled) == CFBooleanGetTypeID() && CFBooleanGetValue((CFBooleanRef)enabled);
        // 按需模式只记录、不启用，之后的启动也不会启用它。已经启用的同样只记录，不关掉：进程启用不了输入法本身，用户第一次加入输入法只能经系统设置的「添加」对话框，所以第一次启动时看到的已启用按需模式，多半就是用户刚加的那个（「简体中文」下排第一的正是「笔」），也正是此刻要切换过去的模式。在 macOS 15.7.9 上实测，这时关掉它会让系统丢掉这次切换、退回原来的输入源，用户看到的是从菜单里选了水杉却没有反应。
        if (MSIMEIsOptInInputModeID(identifier)) {
            [record addObject:identifier];
            continue;
        }
        // A mode that could not be enabled stays unrecorded, so the next launch tries again.
        if (alreadyEnabled || enabler(source) == noErr) [record addObject:identifier];
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
