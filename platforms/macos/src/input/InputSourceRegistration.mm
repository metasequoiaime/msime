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
    for (CFIndex i = 0; i < CFArrayGetCount(sources); ++i) {
        TISInputSourceRef source = (TISInputSourceRef)CFArrayGetValueAtIndex(sources, i);
        if (source == primary) continue;
        void *property = propertyGetter(source, kTISPropertyInputSourceID);
        // Shuangpin and Wubi stay off until the user adds them; enabling every mode here would override the plist's default state on each install.
        if (property && CFGetTypeID(property) == CFStringGetTypeID() && MSIMEIsOptInInputModeID((__bridge NSString *)property)) continue;
        if (!property || CFGetTypeID(property) != CFStringGetTypeID() ||
            ![(__bridge NSString *)property isEqualToString:bundleIdentifier]) {
            status = enabler(source); if (status != noErr) { CFRelease(sources); return status; }
        }
    }
    CFRelease(sources); return noErr;
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
