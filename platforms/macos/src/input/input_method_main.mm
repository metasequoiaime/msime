#import <AppKit/AppKit.h>
#import <InputMethodKit/InputMethodKit.h>
#import "InputModeIdentifiers.h"
#import "InputModeStatusItem.h"
#import "InputSourceRegistration.h"
#import "InputControllerActivity.h"
#import "../settings/PreferencesWindowController.h"
#import "../settings/RuntimeOptions.h"
#import "../settings/RuntimeOptionsRefresh.h"
#import "../settings/AppearancePreferences.h"
#import "../core/FloatingToolbarPanel.h"
#import "../core/UsageReporting.h"
#import "../core/UpdateController.h"
#import "MSIMEClientSession.h"
#import "../candidate/CandidateSkin.h"
#import "../voice/VoiceAudioMuter.h"
#include "../core/DiagnosticLog.h"
#include "msime_client.h"
#include "MSIMERustClient.h"
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

// 「我的词条」的网盘自动同步在 Rust 的后台线程里跑（msime_client.h 的 msime_client_dictionary_sync_start），这里只交出运行时配置文件和与设置应用共用的状态目录：设置应用的 dictionary_sync 命令用的是同一个 Application Support/<设置应用 bundle id>/dictionary-sync（apps/desktop/src-tauri/src/platform/dictionary_sync.rs）。后台线程每一轮重读配置和偏好，没配置网盘、设置应用还没生成设备身份时什么也不做，从不自己生成身份。起它只是开一个线程，不在主线程上读写文件或联网。
static void MSIMEStartDictionarySync(void) {
    NSString *optionsPath = MSIMERuntimeOptionsPath();
    NSString *state = MSIMEDefaultClientStateDirectory(NSFileManager.defaultManager).path;
    if (optionsPath.length == 0 || !optionsPath.isAbsolutePath || state.length == 0) return;
    NSDictionary *request = @{
        @"options_path": optionsPath,
        @"directory": [state stringByAppendingPathComponent:@"dictionary-sync"],
    };
    NSData *body = [NSJSONSerialization dataWithJSONObject:request options:0 error:nil];
    if (!body) return;
    char *response = msime_client_dictionary_sync_start(static_cast<const uint8_t *>(body.bytes), body.length);
    if (response && !strstr(response, "\"ok\":true")) NSLog(@"MSIME dictionary sync did not start: %s", response);
    if (response) msime_client_string_free(response);
}

// 当前登录会话的屏幕是否锁着（锁屏或开盖后还没解锁）。读不到会话信息时按没锁处理。
static BOOL MSIMEScreenIsLocked(void) {
    CFDictionaryRef session = CGSessionCopyCurrentDictionary();
    if (!session) return NO;
    const void *locked = CFDictionaryGetValue(session, CFSTR("CGSSessionScreenIsLocked"));
    const BOOL result = locked && CFGetTypeID(locked) == CFBooleanGetTypeID() && CFBooleanGetValue((CFBooleanRef)locked);
    CFRelease(session);
    return result;
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
        if (MSIMEShouldReportInputSourceEnabled(argc, argv)) {
            std::printf("%s\n", MSIMEInputSourceIsEnabled(NSBundle.mainBundle.bundleIdentifier) ? "true" : "false");
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
            // 后端 dylib 不链接 Rust 库，账号、云端业务和云剪贴板的端到端加密驱动都经这张带版本号的函数表交给它（shared/backend/rust/MSIMERustClient.h）；输入法和独立设置窗口都要先做这一步。dylib 不收这张表时它的每个 Rust 入口都报 rust_client_unavailable，这里只记一条日志。
            using MSIMEInstallRustClient = bool (*)(const msime_rust_client *);
            const msime_rust_client table = msime_rust_client_linked();
            auto install = reinterpret_cast<MSIMEInstallRustClient>(dlsym(backend, "MSIMEInstallRustClient"));
            if (install == nullptr || !install(&table)) NSLog(@"MSIME backend did not accept the Rust client table");
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
        // 同样只在输入法进程里：独立设置窗口已经在上面返回。
        MSIMEStartDictionarySync();
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
                    // 离开本输入法对应 TSF 的 Deactivate：它把开关、标点和全半角三个 compartment 一起清掉，所以不论 ime_mode_scope 是哪种，每个应用切回来时都从 default_ime_mode 和保存的标点、全半角开始。共享的模式记录也一并清空并标记为刚切进，切回时选哪个模式入口都当作用户的选择采纳；只有一个例外，切回后、按键之前报告的 中 只改中英状态，不把双拼等方案改成全拼，因为系统可能自己落在 中 入口上（#10199，见 MSIMESystemInputModeState 的 switchedIn）。焦点在客户端之间移动不会走到这里。
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
        // 合盖睡眠、锁屏到解锁之后几秒，系统会自己恢复输入源并报告一个模式，有用户每次开盖方案都被改成注音。这段时间里的报告不当作用户的选择（MSIMESystemInputModeState 的 restoringUntil）。没锁屏的唤醒没有解锁通知，所以唤醒时屏幕没锁就直接进入解锁后的那几秒。
        NSNotificationCenter *workspaceCenter = NSWorkspace.sharedWorkspace.notificationCenter;
        NSDistributedNotificationCenter *distributedCenter = NSDistributedNotificationCenter.defaultCenter;
        __attribute__((objc_precise_lifetime)) NSArray<id> *workspaceObservers = @[
            [workspaceCenter addObserverForName:NSWorkspaceWillSleepNotification object:nil
                queue:NSOperationQueue.mainQueue usingBlock:^(NSNotification *) {
                    MSIMEHoldSystemInputModeRestore(MSIMESharedSystemInputModeState());
                    msime_macos_diagnostic_write("input_mode_restore event=sleep");
                }],
            [workspaceCenter addObserverForName:NSWorkspaceDidWakeNotification object:nil
                queue:NSOperationQueue.mainQueue usingBlock:^(NSNotification *) {
                    if (MSIMEScreenIsLocked()) return;
                    MSIMEBeginSystemInputModeRestore(MSIMESharedSystemInputModeState(), NSProcessInfo.processInfo.systemUptime);
                    msime_macos_diagnostic_write("input_mode_restore event=wake");
                }],
        ];
        __attribute__((objc_precise_lifetime)) NSArray<id> *distributedObservers = @[
            [distributedCenter addObserverForName:@"com.apple.screenIsLocked" object:nil
                queue:NSOperationQueue.mainQueue usingBlock:^(NSNotification *) {
                    MSIMEHoldSystemInputModeRestore(MSIMESharedSystemInputModeState());
                    msime_macos_diagnostic_write("input_mode_restore event=lock");
                }],
            [distributedCenter addObserverForName:@"com.apple.screenIsUnlocked" object:nil
                queue:NSOperationQueue.mainQueue usingBlock:^(NSNotification *) {
                    MSIMEBeginSystemInputModeRestore(MSIMESharedSystemInputModeState(), NSProcessInfo.processInfo.systemUptime);
                    msime_macos_diagnostic_write("input_mode_restore event=unlock");
                }],
        ];
        // 带 feed 的发布包在这里启动 Sparkle，它才会按 SUScheduledCheckInterval 在后台检查和下载；没有 feed 的构建不创建更新控制器，「检查更新…」用到时再建。
        // 先在后台队列读出偏好里的更新通道（update_channel），回到主线程再带着它创建控制器：Sparkle 启动后安排的第一次检查就取用户所选通道的 appcast，读偏好文件也不占用处理按键的主线程。偏好读不出来时按稳定版。之后的通道变化由 InputController 读偏好时交给控制器。
        if (MSIMEUpdateHostUsesSparkle(NSBundle.mainBundle)) {
            NSString *updatePreferences = [reportingPreferences isKindOfClass:NSString.class] && [reportingPreferences isAbsolutePath]
                ? [reportingPreferences copy] : nil;
            dispatch_async(dispatch_get_global_queue(QOS_CLASS_UTILITY, 0), ^{
                id channel = nil;
                if (updatePreferences) {
                    NSDictionary *snapshot = [MSIMEClientSession loadPreferencesInDirectory:updatePreferences error:nil];
                    NSDictionary *preferences = [snapshot[@"preferences"] isKindOfClass:NSDictionary.class] ? snapshot[@"preferences"] : nil;
                    channel = preferences[@"update_channel"];
                }
                dispatch_async(dispatch_get_main_queue(), ^{
                    MSIMEUpdateController *updates = [MSIMEUpdateController sharedControllerWithUpdateChannelPreference:channel];
                    // 切换输入源和离开电脑时都先看用户手上有没有没完成的东西，有就这次不装（UpdateController.h）。
                    updates.unfinishedWork = ^BOOL {
                        return MSIMEInputControllerHasUnfinishedInput() || MSIMEUpdateWindowsHoldUserWork(NSApp.windows);
                    };
                    // 从不切走水杉的用户，在熄屏、切走登录会话或半小时没有任何输入时装后台下好的更新（UpdateController.h）。闲置时长取整个登录会话的输入，不只是打给水杉的按键。
                    [updates installDownloadedUpdateWhenAwayWithWorkspaceCenter:NSWorkspace.sharedWorkspace.notificationCenter
                                                                      idleClock:^CFTimeInterval {
                                                                          return CGEventSourceSecondsSinceLastEventType(kCGEventSourceStateCombinedSessionState, kCGAnyInputEventType);
                                                                      }];
                });
            });
        }
        Class bridge = NSClassFromString(@"MSIMEBackendWindowBridge");
        id shared = [bridge respondsToSelector:@selector(shared)] ? [bridge performSelector:@selector(shared)] : nil;
        if ([shared respondsToSelector:@selector(startClipboardCaptureWithOptions:)]) {
            [shared performSelector:@selector(startClipboardCaptureWithOptions:) withObject:MSIMELoadRuntimeOptions() ?: @{}];
        }
        [NSApp run];
        if ([shared respondsToSelector:@selector(stopClipboardCapture)]) [shared performSelector:@selector(stopClipboardCapture)];
        [sourceMonitor stop];
        for (id observer in workspaceObservers) [workspaceCenter removeObserver:observer];
        for (id observer in distributedObservers) [distributedCenter removeObserver:observer];
        // 正在跑的那一次同步最多等两秒，与使用统计收尾的宽限相同；没等完也不要紧，每一步都已写盘。被 SIGTERM 结束时不经过这里，进程直接退出，道理相同。
        (void)msime_client_dictionary_sync_stop(2000);
        MSIMEUsageReportingStop();
        (void)server;
    }
    return 0;
}
