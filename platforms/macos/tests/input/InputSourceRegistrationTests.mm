#import "../../src/input/InputSourceRegistration.h"
#import "../../src/input/InputModeIdentifiers.h"

#include <algorithm>
#include <stdexcept>
#include <vector>

@interface RegistrationWorkspace : NSWorkspace
@property(nonatomic) NSUInteger launches;
@property(nonatomic, strong) NSURL *launchedURL;
@property(nonatomic, strong) NSWorkspaceOpenConfiguration *configuration;
@property(nonatomic, copy) void (^completion)(NSRunningApplication *, NSError *);
@end

@implementation RegistrationWorkspace
- (void)openApplicationAtURL:(NSURL *)url configuration:(NSWorkspaceOpenConfiguration *)configuration
           completionHandler:(void (^)(NSRunningApplication *, NSError *))completion {
    ++self.launches;
    self.launchedURL = url;
    self.configuration = configuration;
    self.completion = completion;
}
@end

namespace
{
NSURL *registeredURL = nil;
CFArrayRef sourceList = nullptr;
NSString *listedBundleIdentifier = nil;
Boolean includedAllInstalled = false;
BOOL enableCapableOnly = NO;
std::vector<TISInputSourceRef> enabledSources;
TISInputSourceRef rejectedSource = nullptr;
TISInputSourceRef parentSource = reinterpret_cast<TISInputSourceRef>(0x101);
TISInputSourceRef modeSource = reinterpret_cast<TISInputSourceRef>(0x102);
TISInputSourceRef englishModeSource = reinterpret_cast<TISInputSourceRef>(0x103);
TISInputSourceRef hansModeSource = reinterpret_cast<TISInputSourceRef>(0x104);
TISInputSourceRef shuangpinModeSource = reinterpret_cast<TISInputSourceRef>(0x105);
TISInputSourceRef wubiModeSource = reinterpret_cast<TISInputSourceRef>(0x106);
TISInputSourceRef koreanModeSource = reinterpret_cast<TISInputSourceRef>(0x107);
TISInputSourceRef japaneseModeSource = reinterpret_cast<TISInputSourceRef>(0x108);
TISInputSourceRef appParentSource = reinterpret_cast<TISInputSourceRef>(0x109);
TISInputSourceRef cantoneseModeSource = reinterpret_cast<TISInputSourceRef>(0x10a);
TISInputSourceRef zhuyinModeSource = reinterpret_cast<TISInputSourceRef>(0x10b);
TISInputSourceRef vietnameseModeSource = reinterpret_cast<TISInputSourceRef>(0x10c);
TISInputSourceRef tibetanModeSource = reinterpret_cast<TISInputSourceRef>(0x10d);
TISInputSourceRef strokeModeSource = reinterpret_cast<TISInputSourceRef>(0x10e);
std::vector<TISInputSourceRef> alreadyEnabledSources;
std::vector<TISInputSourceRef> disabledSources;
NSString *listedSourceIdentifier = nil;

void require(bool condition, const char *message)
{
    if (!condition)
    {
        throw std::runtime_error(message);
    }
}

OSStatus CaptureRegistration(CFURLRef location)
{
    registeredURL = (__bridge NSURL *)location;
    return noErr;
}

OSStatus RejectRegistration(CFURLRef location)
{
    (void)location;
    return -50;
}

CFArrayRef CopyInputSources(CFDictionaryRef properties, Boolean includeAllInstalled)
{
    NSDictionary *filter = (__bridge NSDictionary *)properties;
    listedBundleIdentifier = filter[(__bridge NSString *)kTISPropertyBundleID];
    listedSourceIdentifier = filter[(__bridge NSString *)kTISPropertyInputSourceID];
    enableCapableOnly = [filter[(__bridge NSString *)kTISPropertyInputSourceIsEnableCapable] boolValue];
    includedAllInstalled = includeAllInstalled;
    return sourceList == nullptr ? nullptr : (CFArrayRef)CFRetain(sourceList);
}

void *GetInputSourceProperty(TISInputSourceRef inputSource, CFStringRef propertyKey)
{
    if (propertyKey == kTISPropertyInputSourceIsEnabled)
    {
        const bool enabled = std::find(alreadyEnabledSources.begin(), alreadyEnabledSources.end(), inputSource) != alreadyEnabledSources.end();
        return const_cast<void *>(reinterpret_cast<const void *>(enabled ? kCFBooleanTrue : kCFBooleanFalse));
    }
    if (propertyKey != kTISPropertyInputSourceID)
    {
        return nullptr;
    }
    if (inputSource == koreanModeSource) return (__bridge void *)MSIMEKoreanInputModeID;
    if (inputSource == japaneseModeSource) return (__bridge void *)MSIMEJapaneseInputModeID;
    if (inputSource == appParentSource) return (__bridge void *)@"app.msime.inputmethod.MetasequoiaIME";
    if (inputSource == hansModeSource) return (__bridge void *)MSIMEChineseInputModeID;
    if (inputSource == shuangpinModeSource) return (__bridge void *)MSIMEShuangpinInputModeID;
    if (inputSource == wubiModeSource) return (__bridge void *)MSIMEWubiInputModeID;
    if (inputSource == cantoneseModeSource) return (__bridge void *)MSIMECantoneseInputModeID;
    if (inputSource == zhuyinModeSource) return (__bridge void *)MSIMEZhuyinInputModeID;
    if (inputSource == vietnameseModeSource) return (__bridge void *)MSIMEVietnameseInputModeID;
    if (inputSource == tibetanModeSource) return (__bridge void *)MSIMETibetanInputModeID;
    if (inputSource == strokeModeSource) return (__bridge void *)MSIMEStrokeInputModeID;
    CFStringRef identifier = inputSource == parentSource        ? CFSTR("com.houko.inputmethod.MetasequoiaIME")
                             : inputSource == englishModeSource ? CFSTR("com.houko.inputmethod.MetasequoiaIME.Roman")
                                                                : CFSTR("com.houko.inputmethod.MetasequoiaIME.Hans");
    return const_cast<void *>(reinterpret_cast<const void *>(identifier));
}

OSStatus EnableInputSource(TISInputSourceRef inputSource)
{
    enabledSources.push_back(inputSource);
    return inputSource == rejectedSource ? -50 : noErr;
}

OSStatus DisableInputSource(TISInputSourceRef inputSource)
{
    disabledSources.push_back(inputSource);
    return inputSource == rejectedSource ? -50 : noErr;
}
} // namespace

int main()
{
    @autoreleasepool
    {
        const char *registrationArguments[] = {"MetasequoiaIME", "--register-input-source"};
        const char *ordinaryArguments[] = {"MetasequoiaIME"};
        const char *unknownArguments[] = {"MetasequoiaIME", "--unknown"};
        require(MSIMEShouldRegisterInputSource(2, registrationArguments),
                "The registration command was not recognized.");
        const char *reregistrationArguments[] = {"MetasequoiaIME", "--reregister-input-source"};
        require(MSIMEShouldRegisterInputSource(2, reregistrationArguments),
                "The re-registration command was not recognized.");
        require(!MSIMEShouldRegisterInputSource(1, ordinaryArguments),
                "Ordinary InputMethodKit startup was treated as registration.");
        require(!MSIMEShouldRegisterInputSource(2, unknownArguments),
                "An unknown command was treated as registration.");

        NSURL *bundleURL = [NSURL fileURLWithPath:@"/tmp/MetasequoiaIME.app" isDirectory:YES];
        RegistrationWorkspace *workspace = [RegistrationWorkspace new];
        __block BOOL launchCompleted = NO;
        __block BOOL launchSucceeded = NO;
        MSIMELaunchInputSourceReregistration(bundleURL, workspace, ^(BOOL launched) {
            launchCompleted = YES;
            launchSucceeded = launched;
        });
        require(workspace.launches == 1 && [workspace.launchedURL isEqual:bundleURL],
                "Re-registration did not launch the current input method bundle.");
        require([workspace.configuration.arguments isEqual:@[@"--reregister-input-source"]] &&
                    !workspace.configuration.activates && workspace.configuration.createsNewApplicationInstance,
                "Re-registration used the wrong launch policy.");
        workspace.completion(NSRunningApplication.currentApplication, nil);
        require(launchCompleted && launchSucceeded, "A successful re-registration launch was not reported.");
        launchCompleted = NO;
        launchSucceeded = YES;
        workspace.completion(nil, [NSError errorWithDomain:@"SyntheticLaunchFailure" code:1 userInfo:nil]);
        require(launchCompleted && !launchSucceeded, "A failed re-registration launch was accepted.");
        launchCompleted = NO;
        MSIMELaunchInputSourceReregistration([NSURL URLWithString:@"https://invalid.example"], workspace,
                                             ^(BOOL launched) { launchCompleted = !launched; });
        require(launchCompleted && workspace.launches == 1, "A non-file bundle URL was launched.");

        require(MSIMERegisterInputSource(bundleURL, CaptureRegistration) == noErr,
                "A successful registration callback was reported as failed.");
        require([registeredURL isEqual:bundleURL], "Registration did not receive the installed bundle URL.");
        require(MSIMERegisterInputSource(bundleURL, RejectRegistration) == -50,
                "A registration callback failure was not preserved.");
        require(MSIMERegisterInputSource(nil, CaptureRegistration) == paramErr,
                "A missing bundle URL was accepted.");
        require(MSIMERegisterInputSource(bundleURL, nullptr) == paramErr,
                "A missing registration callback was accepted.");

        const void *sources[] = {parentSource, modeSource};
        sourceList = CFArrayCreate(nullptr, sources, 2, nullptr);
        NSString *bundleIdentifier = @"com.houko.inputmethod.MetasequoiaIME";
        require(MSIMERegisterAndEnableInputSources(bundleURL, bundleIdentifier, CaptureRegistration,
                                                         CopyInputSources, GetInputSourceProperty,
                                                         EnableInputSource) == noErr,
                "A registered input method was not enabled.");
        require([listedBundleIdentifier isEqualToString:bundleIdentifier] && enableCapableOnly && includedAllInstalled,
                "Input source discovery did not use the registered bundle identifier.");
        require(enabledSources.size() == 2 && enabledSources[0] == parentSource && enabledSources[1] == modeSource,
                "The parent input method was not enabled before its input mode.");

        enabledSources.clear();
        rejectedSource = modeSource;
        require(MSIMERegisterAndEnableInputSources(bundleURL, bundleIdentifier, CaptureRegistration,
                                                         CopyInputSources, GetInputSourceProperty,
                                                         EnableInputSource) == -50,
                "An input source enable failure was not preserved.");
        rejectedSource = nullptr;

        CFRelease(sourceList);
        sourceList = CFArrayCreate(nullptr, nullptr, 0, nullptr);
        require(MSIMERegisterAndEnableInputSources(bundleURL, bundleIdentifier, CaptureRegistration,
                                                         CopyInputSources, GetInputSourceProperty,
                                                         EnableInputSource) == fnfErr,
                "A registration with no discoverable input sources was accepted.");
        CFRelease(sourceList);
        sourceList = nullptr;

        const void *modeOnlySources[] = {modeSource};
        sourceList = CFArrayCreate(nullptr, modeOnlySources, 1, nullptr);
        enabledSources.clear();
        require(MSIMERegisterAndEnableInputSources(bundleURL, bundleIdentifier, CaptureRegistration,
                                                         CopyInputSources, GetInputSourceProperty,
                                                         EnableInputSource) == noErr,
                "An input mode without a top-level source was rejected.");
        require(enabledSources.size() == 1 && enabledSources[0] == modeSource,
                "The primary input mode was not enabled when the top-level source was absent.");
        CFRelease(sourceList);
        sourceList = nullptr;

        // The bundle declares a Chinese and an English mode; the menu bar can only show the 英 icon if registration enables both.
        const void *twoModeSources[] = {modeSource, englishModeSource};
        sourceList = CFArrayCreate(nullptr, twoModeSources, 2, nullptr);
        enabledSources.clear();
        require(MSIMERegisterAndEnableInputSources(bundleURL, bundleIdentifier, CaptureRegistration,
                                                         CopyInputSources, GetInputSourceProperty,
                                                         EnableInputSource) == noErr,
                "A bundle with a Chinese and an English mode was rejected.");
        require(enabledSources.size() == 2 && enabledSources[0] == modeSource && enabledSources[1] == englishModeSource,
                "Registration did not enable both the Chinese and the English input mode.");
        CFRelease(sourceList);
        sourceList = nullptr;

        // 登记会启用除按需模式以外的全部模式，双拼和五笔也在内，装好即可使用，不必让用户去系统设置的「添加」对话框里逐个找。
        const void *chineseSchemeSources[] = {shuangpinModeSource, wubiModeSource, hansModeSource};
        sourceList = CFArrayCreate(nullptr, chineseSchemeSources, 3, nullptr);
        enabledSources.clear();
        require(MSIMERegisterAndEnableInputSources(bundleURL, @"app.msime.inputmethod.MetasequoiaIME", CaptureRegistration,
                                                         CopyInputSources, GetInputSourceProperty,
                                                         EnableInputSource) == noErr,
                "A bundle with Shuangpin and Wubi modes was rejected.");
        require(enabledSources.size() == 3 && enabledSources[0] == shuangpinModeSource && enabledSources[1] == wubiModeSource &&
                    enabledSources[2] == hansModeSource,
                "Registration did not enable the Shuangpin and Wubi modes with the others.");
        CFRelease(sourceList);
        sourceList = nullptr;

        // 登记时粤拼、注音、越南文、藏文和笔画五个模式保持关闭：它们按需启用，用户选中对应方案时才打开，安装不能给输入菜单平白加上没人要的五项。排在最前面的也不会因此成为主模式。
        const void *optInSources[] = {cantoneseModeSource, hansModeSource, zhuyinModeSource, wubiModeSource, vietnameseModeSource, tibetanModeSource, strokeModeSource};
        sourceList = CFArrayCreate(nullptr, optInSources, 7, nullptr);
        enabledSources.clear();
        require(MSIMERegisterAndEnableInputSources(bundleURL, @"app.msime.inputmethod.MetasequoiaIME", CaptureRegistration,
                                                         CopyInputSources, GetInputSourceProperty,
                                                         EnableInputSource) == noErr,
                "A bundle with opt-in modes was rejected.");
        require(enabledSources.size() == 2 && enabledSources[0] == hansModeSource && enabledSources[1] == wubiModeSource,
                "Registration enabled an opt-in mode, or skipped one that is not opt-in.");
        CFRelease(sourceList);
        sourceList = nullptr;

        // An update that only replaced the bundle left the modes it added off. The first launch that keeps a record counts the modes every earlier install enabled as offered and turns on the rest, once each; the bundle-level source is not a mode.
        NSString *appBundle = @"app.msime.inputmethod.MetasequoiaIME";
        const void *installedSources[] = {appParentSource, hansModeSource, japaneseModeSource, koreanModeSource, shuangpinModeSource, wubiModeSource};
        sourceList = CFArrayCreate(nullptr, installedSources, 6, nullptr);
        alreadyEnabledSources = {appParentSource, hansModeSource, japaneseModeSource};
        enabledSources.clear();
        NSArray<NSString *> *offered = MSIMEEnableNewInputModes(appBundle, nil, CopyInputSources, GetInputSourceProperty, EnableInputSource, DisableInputSource);
        require([listedBundleIdentifier isEqualToString:appBundle] && enableCapableOnly && includedAllInstalled,
                "New-mode discovery did not list every installed source of the bundle.");
        // Korean is in the seed, since every install since it was added registered and enabled it.
        require(enabledSources.size() == 2 && enabledSources[0] == shuangpinModeSource && enabledSources[1] == wubiModeSource,
                "The first recorded launch did not enable exactly the modes added since the earlier installs.");
        require([offered isEqualToArray:@[MSIMEChineseInputModeID, MSIMEEnglishInputModeID, MSIMEJapaneseInputModeID,
                                          MSIMEKoreanInputModeID, MSIMEShuangpinInputModeID, MSIMEWubiInputModeID]],
                "The record does not name the earlier modes and the newly enabled ones.");

        // A mode the user removed afterwards stays removed.
        alreadyEnabledSources = {appParentSource, hansModeSource, japaneseModeSource, shuangpinModeSource, wubiModeSource};
        enabledSources.clear();
        require([MSIMEEnableNewInputModes(appBundle, offered, CopyInputSources, GetInputSourceProperty, EnableInputSource, DisableInputSource) isEqualToArray:offered] &&
                    enabledSources.empty(),
                "A recorded mode the user removed was enabled again.");

        // A Japanese or Korean mode the user removed before any record existed is not brought back either.
        alreadyEnabledSources = {appParentSource, hansModeSource};
        enabledSources.clear();
        offered = MSIMEEnableNewInputModes(appBundle, nil, CopyInputSources, GetInputSourceProperty, EnableInputSource, DisableInputSource);
        require(std::find(enabledSources.begin(), enabledSources.end(), japaneseModeSource) == enabledSources.end() &&
                    std::find(enabledSources.begin(), enabledSources.end(), koreanModeSource) == enabledSources.end() &&
                    [offered containsObject:MSIMEKoreanInputModeID],
                "A previously offered Japanese or Korean mode removed before the first record was enabled again.");

        // A new mode that is already on is only recorded, and one the system refuses stays unrecorded so the next launch retries it.
        alreadyEnabledSources = {appParentSource, hansModeSource, japaneseModeSource, koreanModeSource};
        rejectedSource = wubiModeSource;
        enabledSources.clear();
        offered = MSIMEEnableNewInputModes(appBundle, nil, CopyInputSources, GetInputSourceProperty, EnableInputSource, DisableInputSource);
        require(enabledSources.size() == 2 && enabledSources[0] == shuangpinModeSource && enabledSources[1] == wubiModeSource &&
                    [offered containsObject:MSIMEKoreanInputModeID] && [offered containsObject:MSIMEShuangpinInputModeID] &&
                    ![offered containsObject:MSIMEWubiInputModeID],
                "An enabled new mode was enabled again, or a refused one was recorded.");
        rejectedSource = nullptr;
        enabledSources.clear();
        offered = MSIMEEnableNewInputModes(appBundle, offered, CopyInputSources, GetInputSourceProperty, EnableInputSource, DisableInputSource);
        require(enabledSources.size() == 1 && enabledSources[0] == wubiModeSource && [offered containsObject:MSIMEWubiInputModeID],
                "A mode the system refused was not retried on the next launch.");
        alreadyEnabledSources.clear();
        CFRelease(sourceList);
        sourceList = nullptr;

        // Without the system calls nothing is enabled and the record is only seeded.
        require([MSIMEEnableNewInputModes(appBundle, nil, nullptr, GetInputSourceProperty, EnableInputSource, DisableInputSource)
                    isEqualToArray:@[MSIMEChineseInputModeID, MSIMEEnglishInputModeID, MSIMEJapaneseInputModeID, MSIMEKoreanInputModeID]],
                "A missing lister did not leave just the seeded record.");
        require([MSIMEEnableNewInputModes(appBundle, nil, CopyInputSources, GetInputSourceProperty, EnableInputSource, nullptr)
                    isEqualToArray:@[MSIMEChineseInputModeID, MSIMEEnglishInputModeID, MSIMEJapaneseInputModeID, MSIMEKoreanInputModeID]],
                "A missing disabler did not leave just the seeded record.");

        // The opt-in modes are recorded without being enabled, so neither this launch nor any later one turns them on. One the system enabled by itself despite tsInputModeDefaultStateKey is turned off once, when it is first recorded.
        const void *optInInstalledSources[] = {appParentSource, hansModeSource, cantoneseModeSource, zhuyinModeSource, vietnameseModeSource, tibetanModeSource, strokeModeSource};
        sourceList = CFArrayCreate(nullptr, optInInstalledSources, 7, nullptr);
        alreadyEnabledSources = {appParentSource, hansModeSource, zhuyinModeSource, tibetanModeSource, strokeModeSource};
        enabledSources.clear();
        disabledSources.clear();
        offered = MSIMEEnableNewInputModes(appBundle, @[MSIMEChineseInputModeID], CopyInputSources, GetInputSourceProperty, EnableInputSource, DisableInputSource);
        require(enabledSources.empty(), "An opt-in mode was enabled by an update.");
        require(disabledSources.size() == 3 && disabledSources[0] == zhuyinModeSource && disabledSources[1] == tibetanModeSource &&
                    disabledSources[2] == strokeModeSource,
                "An opt-in mode the system enabled by itself was not turned off, or one already off was disabled.");
        require([offered isEqualToArray:@[MSIMEChineseInputModeID, MSIMECantoneseInputModeID, MSIMEZhuyinInputModeID, MSIMEVietnameseInputModeID, MSIMETibetanInputModeID, MSIMEStrokeInputModeID]],
                "The opt-in modes were not recorded.");
        // Once recorded, an opt-in mode the user turned on by picking its scheme is left on.
        alreadyEnabledSources = {appParentSource, hansModeSource, cantoneseModeSource, zhuyinModeSource, strokeModeSource};
        enabledSources.clear();
        disabledSources.clear();
        require([MSIMEEnableNewInputModes(appBundle, offered, CopyInputSources, GetInputSourceProperty, EnableInputSource, DisableInputSource) isEqualToArray:offered] &&
                    enabledSources.empty() && disabledSources.empty(),
                "A recorded opt-in mode was enabled or disabled again.");
        // A disable the system refuses is still recorded: a later launch could not tell the system's doing from the user picking the scheme.
        alreadyEnabledSources = {appParentSource, hansModeSource, vietnameseModeSource};
        rejectedSource = vietnameseModeSource;
        disabledSources.clear();
        offered = MSIMEEnableNewInputModes(appBundle, @[MSIMEChineseInputModeID], CopyInputSources, GetInputSourceProperty, EnableInputSource, DisableInputSource);
        require(disabledSources.size() == 1 && disabledSources[0] == vietnameseModeSource && [offered containsObject:MSIMEVietnameseInputModeID],
                "A refused disable left the opt-in mode unrecorded.");
        rejectedSource = nullptr;
        alreadyEnabledSources.clear();
        CFRelease(sourceList);
        sourceList = nullptr;

        // Picking an opt-in scheme enables its mode by identifier, from every installed source, since the mode is not enabled yet.
        const void *cantoneseOnly[] = {cantoneseModeSource};
        sourceList = CFArrayCreate(nullptr, cantoneseOnly, 1, nullptr);
        enabledSources.clear();
        require(MSIMEEnableInputMode(MSIMECantoneseInputModeID, CopyInputSources, EnableInputSource) == noErr &&
                    [listedSourceIdentifier isEqualToString:MSIMECantoneseInputModeID] && includedAllInstalled &&
                    enabledSources.size() == 1 && enabledSources[0] == cantoneseModeSource,
                "Picking an opt-in scheme did not enable its installed mode.");
        rejectedSource = cantoneseModeSource;
        require(MSIMEEnableInputMode(MSIMECantoneseInputModeID, CopyInputSources, EnableInputSource) == -50,
                "A refused enable was not reported.");
        rejectedSource = nullptr;
        CFRelease(sourceList);
        sourceList = CFArrayCreate(nullptr, nullptr, 0, nullptr);
        require(MSIMEEnableInputMode(MSIMEZhuyinInputModeID, CopyInputSources, EnableInputSource) == fnfErr,
                "A mode that is not installed was reported as enabled.");
        CFRelease(sourceList);
        sourceList = nullptr;
        require(MSIMEEnableInputMode(nil, CopyInputSources, EnableInputSource) == paramErr &&
                    MSIMEEnableInputMode(MSIMEZhuyinInputModeID, nullptr, EnableInputSource) == paramErr,
                "Enabling a mode without an identifier or a lister was accepted.");

        require(MSIMERegisterAndEnableInputSources(bundleURL, bundleIdentifier, RejectRegistration,
                                                         CopyInputSources, GetInputSourceProperty,
                                                         EnableInputSource) == -50,
                "A registration failure was not returned before discovery.");
        require(MSIMERegisterAndEnableInputSources(bundleURL, nil, CaptureRegistration, CopyInputSources,
                                                         GetInputSourceProperty, EnableInputSource) == paramErr,
                "A missing bundle identifier was accepted.");
        require(MSIMERegisterAndEnableInputSources(bundleURL, bundleIdentifier, CaptureRegistration, nullptr,
                                                         GetInputSourceProperty, EnableInputSource) == paramErr,
                "A missing input source lister was accepted.");
    }
    return 0;
}
