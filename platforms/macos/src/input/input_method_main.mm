#import <AppKit/AppKit.h>
#import <InputMethodKit/InputMethodKit.h>
#import "InputModeIdentifiers.h"
#import "InputModeStatusItem.h"
#import "InputSourceRegistration.h"
#import "../settings/PreferencesWindowController.h"
#import "../settings/RuntimeOptions.h"
#import "../settings/RuntimeOptionsRefresh.h"
#import "../settings/AppearancePreferences.h"
#import "../core/FloatingToolbarPanel.h"
#import "../core/UsageReporting.h"
#import "../core/UpdateController.h"
#import "../candidate/CandidateSkin.h"
#import "../voice/VoiceAudioMuter.h"
#include "../core/DiagnosticLog.h"
#include "msime_client.h"
#include <cstdio>
#include <cstring>
#include <dlfcn.h>

static bool MSIMEShouldShowPreferences(int argc, const char *argv[]) {
    for (int index = 1; index < argc; ++index) {
        if (strcmp(argv[index], "--preferences") == 0) return true;
    }
    return false;
}

static void MSIMEConfigureMovableState(void) {
    NSDictionary *options = MSIMELoadRuntimeOptions();
    NSString *directory = [options[@"preferences_directory"] isKindOfClass:NSString.class]
        ? options[@"preferences_directory"] : nil;
    if (directory.length > 0 && directory.isAbsolutePath) {
        metasequoia::mac::SetDefaultSkinsRoot(
            std::filesystem::path(directory.fileSystemRepresentation) / "skins");
    } else if (!MSIMEEditionIsFull()) {
        // 没有配置状态目录时 CandidateSkin.cpp 退回 full 的状态目录；其他版本的皮肤在自己的状态目录下。
        NSURL *state = MSIMEDefaultClientStateDirectory(NSFileManager.defaultManager);
        if (state.path.length > 0)
            metasequoia::mac::SetDefaultSkinsRoot(std::filesystem::path(state.path.fileSystemRepresentation) / "skins");
    }
}

int main(int argc, const char *argv[]) {
    @autoreleasepool {
        if (MSIMEShouldRegisterInputSource(argc, argv)) {
            NSURL *bundleURL = NSBundle.mainBundle.bundleURL;
            NSString *identifier = NSBundle.mainBundle.bundleIdentifier;
            OSStatus status = MSIMERegisterAndEnableInputSources(bundleURL, identifier,
                TISRegisterInputSource, TISCreateInputSourceList,
                [](TISInputSourceRef source, CFStringRef key) -> void * {
                    return (void *)TISGetInputSourceProperty(source, key);
                }, TISEnableInputSource);
            return status == noErr ? 0 : 1;
        }
        if (MSIMEShouldReportInputSourceRegistration(argc, argv)) {
            return MSIMEInputSourceRegistryExitCode(
                MSIMEInputSourceRegistryStateFor(NSBundle.mainBundle.bundleIdentifier, TISCreateInputSourceList));
        }
        if (MSIMEShouldReportEnabledInputModes(argc, argv)) {
            NSArray<NSString *> *modes = MSIMEEnabledInputModeIDs(NSBundle.mainBundle.bundleIdentifier,
                TISCreateInputSourceList,
                [](TISInputSourceRef source, CFStringRef key) -> void * {
                    return (void *)TISGetInputSourceProperty(source, key);
                });
            if (!modes) return 1;
            for (NSString *mode in modes) std::printf("%s\n", mode.UTF8String);
            return 0;
        }
        [NSApplication sharedApplication];
        // Host API reports failures it recovers from by itself (a sound, music or helpcode pack that does not load, an audio device that does not open) as one line on stderr, which launchd points at /dev/null for an input method; this sends them to diagnostic.log instead. Before the first session, whose creation reports the helpcode fallback; lines that arrive before the log is configured or while it is off are dropped, not kept for later (DiagnosticLog.h).
        msime_client_set_diagnostic_sink(msime_macos_diagnostic_host_line);
        // After an app upgrade the options still point at the previous dictionary generation; bring them to the installed one (replaying the user dictionary) before any session, including the standalone preferences window's, reads them.
        // A missing file is the not-yet-configured state, which is not a refresh failure.
        NSString *optionsPath = MSIMERuntimeOptionsPath();
        switch (optionsPath && [NSFileManager.defaultManager fileExistsAtPath:optionsPath]
                    ? MSIMERefreshRuntimeOptions(optionsPath) : MSIMERuntimeOptionsRefreshCurrent) {
        case MSIMERuntimeOptionsRefreshUpdated:
            NSLog(@"MSIME dictionary updated to the installed generation");
            break;
        case MSIMERuntimeOptionsRefreshFailed:
            NSLog(@"MSIME cannot update the dictionary to the installed generation; keeping the current one");
            break;
        case MSIMERuntimeOptionsRefreshCurrent:
            break;
        }
        MSIMEConfigureMovableState();
        NSString *swiftBackend = [NSBundle.mainBundle.privateFrameworksPath stringByAppendingPathComponent:@"MSIMEBackend.dylib"];
        if (swiftBackend.length > 0) {
            void *backend = dlopen(swiftBackend.fileSystemRepresentation, RTLD_NOW | RTLD_GLOBAL);
            if (backend == nullptr) return 1;
            // 后端 dylib 不链接 Rust 库，云剪贴板的端到端加密驱动由这里把函数指针交给它；输入法和独立设置窗口都要先做这一步。
            using MSIMEInstallCloudClipboardDriver = void (*)(char *(*)(const uint8_t *, size_t), void (*)(char *));
            if (auto install = reinterpret_cast<MSIMEInstallCloudClipboardDriver>(dlsym(backend, "MSIMEInstallCloudClipboardDriver"))) {
                install(msime_client_cloud_clipboard_e2e, msime_client_string_free);
            }
        }
        // 独立设置窗口也要检查模式是否已加入输入法列表，所以在进入该分支前初始化探针。
        MSIMEInputModeEnabledProbe = MSIMEInputSourceIsEnabled;
        if (MSIMEShouldShowPreferences(argc, argv)) {
            [NSApp setActivationPolicy:NSApplicationActivationPolicyRegular];
            id closeObserver = [[NSNotificationCenter defaultCenter]
                addObserverForName:MSIMEStandalonePreferencesDidCloseNotification
                            object:nil
                             queue:NSOperationQueue.mainQueue
                        usingBlock:^(NSNotification *notification) {
                            (void)notification;
                            [NSApp terminate:nil];
                        }];
            [[MSIMEPreferencesWindowController sharedController] showAndActivateForStandaloneLaunch];
            [NSApp run];
            [[NSNotificationCenter defaultCenter] removeObserver:closeObserver];
            return 0;
        }
        // 脚本插件只在输入法进程里运行，上面的独立设置窗口已经返回。在第一个会话之前打开开关，开发构建和 release 构建一样（msime_client.h）。
        msime_client_scripts_enable(true);
        // One input method process is one usage session; the standalone preferences window above is not.
        NSDictionary *reportingOptions = MSIMELoadRuntimeOptions();
        id reportingPreferences = reportingOptions[@"preferences_directory"];
        MSIMEUsageReportingStart([reportingPreferences isKindOfClass:NSString.class] && [reportingPreferences isAbsolutePath] ? reportingPreferences : nil);
        // Recover a prior crashed capture before accepting new IMK sessions.
        // A running owner holds the journal lock, so this cannot undo its mute.
        [[[MSIMEVoiceAudioMuter alloc] init] restore];
        // imklaunchagent 用这个名字找到本 bundle；名字为什么必须是这种形式，见 Info.plist.in。
        NSString *connectionName = NSBundle.mainBundle.infoDictionary[@"InputMethodConnectionName"];
        if (![connectionName isKindOfClass:NSString.class] || connectionName.length == 0) return 1;
        __attribute__((objc_precise_lifetime)) IMKServer *server = [[IMKServer alloc] initWithName:connectionName bundleIdentifier:NSBundle.mainBundle.bundleIdentifier];
        if (!server) return 1;
        // 记录本版本的模式，不自动启用或关闭输入源，保留用户在系统设置中的选择。
        NSString *const offeredModesKey = @"MSIMEOfferedInputModes";
        NSArray *offeredModes = [NSUserDefaults.standardUserDefaults arrayForKey:offeredModesKey];
        NSArray<NSString *> *offered = MSIMEEnableNewInputModes(NSBundle.mainBundle.bundleIdentifier, offeredModes,
            TISCreateInputSourceList,
            [](TISInputSourceRef source, CFStringRef key) -> void * {
                return (void *)TISGetInputSourceProperty(source, key);
            }, TISEnableInputSource);
        if (![offered isEqualToArray:offeredModes]) [NSUserDefaults.standardUserDefaults setObject:offered forKey:offeredModesKey];
        __attribute__((objc_precise_lifetime)) MSIMEInputSourceMonitor *sourceMonitor =
            [[MSIMEInputSourceMonitor alloc] initWithCenter:NSDistributedNotificationCenter.defaultCenter
                bundleIdentifier:NSBundle.mainBundle.bundleIdentifier copySource:TISCopyCurrentKeyboardInputSource
                propertyGetter:[](TISInputSourceRef source, CFStringRef key) -> void * {
                    return (void *)TISGetInputSourceProperty(source, key);
                } switchedAway:^{
                    // Leaving this input method is the counterpart of TSF Deactivate, which clears the open/close, punctuation and width compartments together: every app, in either ime_mode_scope, starts from default_ime_mode and the saved punctuation and width when it comes back. The shared record of the shown mode is cleared too, so picking either mode entry on the way back is adopted as a choice. Focus changes between clients never get here.
                    MSIMEAppearancePreferences *preferences = [MSIMEAppearancePreferences sharedPreferences];
                    [preferences resetRememberedInputModes];
                    [preferences resetAllRuntimeInputState];
                    MSIMEResetSystemInputModeState(MSIMESharedSystemInputModeState());
                    [[MSIMEFloatingToolbarPanel sharedPanel] deactivateForInputSourceSwitch];
                    // 切到 ABC 等别的输入源后，菜单栏状态项不能留着一个过期的「中」；切回来时 activateServer: 的同步会按当前模式重新放上。焦点在客户端之间移动不会走到这里，所以 deactivateServer: 不隐藏它。
                    [[MSIMEInputModeStatusItem sharedItem] hide];
                    // 用户刚离开水杉，这时装后台下好的更新不会打断打字：进程退出，切回来时系统从新 bundle 拉起（UpdateController.mm）。
                    if (MSIMEUpdateHostUsesSparkle(NSBundle.mainBundle)) [[MSIMEUpdateController sharedController] installDownloadedUpdateIfReady];
                }];
        // 带 feed 的发布包在这里启动 Sparkle，它才会按 SUScheduledCheckInterval 在后台检查和下载；没有 feed 的构建不创建更新控制器，「检查更新…」用到时再建。
        if (MSIMEUpdateHostUsesSparkle(NSBundle.mainBundle)) (void)[MSIMEUpdateController sharedController];
        Class bridge = NSClassFromString(@"MSIMEBackendWindowBridge");
        id shared = [bridge respondsToSelector:@selector(shared)] ? [bridge performSelector:@selector(shared)] : nil;
        if ([shared respondsToSelector:@selector(startClipboardCaptureWithOptions:)]) {
            [shared performSelector:@selector(startClipboardCaptureWithOptions:) withObject:MSIMELoadRuntimeOptions() ?: @{}];
        }
        [NSApp run];
        if ([shared respondsToSelector:@selector(stopClipboardCapture)]) [shared performSelector:@selector(stopClipboardCapture)];
        [sourceMonitor stop];
        MSIMEUsageReportingStop();
        (void)server;
    }
    return 0;
}
