#pragma once

#import <AppKit/AppKit.h>
#import "../settings/RuntimeOptions.h"
#import "WindowPresentationLog.h"

enum class MSIMEDesktopSettingsPage { Appearance, Voice, Translation, AI, Skin };

static inline void MSIMEOpenDesktopRouteWithContext(NSString *route, NSString *optionsPath,
    NSDictionary<NSString *, NSString *> *environment, NSWorkspace *workspace,
    void (^launched)(NSRunningApplication *), dispatch_block_t fallback) {
    os_log(MSIMEUILog(), "desktop_route_requested route=%{public}@ options=%{public}s", route,
           optionsPath ? (optionsPath.isAbsolutePath ? "absolute" : "relative") : "none");
    if (optionsPath && !optionsPath.isAbsolutePath) {
        os_log(MSIMEUILog(), "desktop_route_fallback route=%{public}@ reason=relative_options_path", route);
        fallback();
        return;
    }
    NSURL *url = [workspace URLForApplicationWithBundleIdentifier:MSIMEClientApplicationIdentifier];
    if (!url) {
        os_log(MSIMEUILog(), "desktop_route_fallback route=%{public}@ reason=settings_app_not_found", route);
        fallback();
        return;
    }
    NSWorkspaceOpenConfiguration *configuration = [NSWorkspaceOpenConfiguration configuration];
    configuration.arguments = @[[NSString stringWithFormat:@"--route=%@", route]];
    // The shared keyboard must leave the editor foreground, just like the
    // native nonactivating NSPanel. Settings pages still activate normally.
    configuration.activates = ![route isEqualToString:@"keyboard"];
    // LaunchServices does not reliably inherit the input method's environment.
    // Pass the selected file path and optional native session identity, never
    // file contents, credentials, or text being composed.
    NSMutableDictionary *launchEnvironment = [NSMutableDictionary dictionaryWithDictionary:environment ?: @{}];
    if (optionsPath) launchEnvironment[@"MSIME_CLIENT_HOST_OPTIONS"] = optionsPath;
    if (launchEnvironment.count) configuration.environment = launchEnvironment;
    // Always a new process, settings included. Without it LaunchServices would activate whichever app.msime.macos instance is running, possibly a hidden per-session panel process, and drop the arguments. A settings launch that finds a settings window already open hands its --route= to that window through the shell's single-instance socket and exits, so the existing window comes back on the requested page.
    configuration.createsNewApplicationInstance = YES;
    os_log(MSIMEUILog(), "desktop_route_launching route=%{public}@ activates=%d", route, configuration.activates);
    const uint64_t started = clock_gettime_nsec_np(CLOCK_UPTIME_RAW);
    [workspace openApplicationAtURL:url configuration:configuration
                 completionHandler:^(NSRunningApplication *application, NSError *error) {
        const unsigned long long elapsed = (clock_gettime_nsec_np(CLOCK_UPTIME_RAW) - started) / 1000000;
        if (error || !application) {
            os_log(MSIMEUILog(), "desktop_route_fallback route=%{public}@ reason=launch_failed error=%{public}@:%ld elapsed_ms=%llu",
                   route, error.domain ?: @"-", (long)error.code, elapsed);
            dispatch_async(dispatch_get_main_queue(), fallback);
        } else {
            os_log(MSIMEUILog(), "desktop_route_launched route=%{public}@ pid=%d active=%d elapsed_ms=%llu", route,
                   application.processIdentifier, application.isActive, elapsed);
            if (launched) dispatch_async(dispatch_get_main_queue(), ^{ launched(application); });
        }
    }];
}

static inline void MSIMEOpenDesktopRouteWithOptions(NSString *route, NSString *optionsPath,
                                                   NSWorkspace *workspace, dispatch_block_t fallback) {
    MSIMEOpenDesktopRouteWithContext(route, optionsPath, nil, workspace, nil, fallback);
}

static inline void MSIMEOpenDesktopRoute(NSString *route, NSWorkspace *workspace,
                                        dispatch_block_t fallback) {
    MSIMEOpenDesktopRouteWithOptions(route, MSIMERuntimeOptionsPath(), workspace, fallback);
}

// The update entry is a shared About page on desktop. Native Sparkle remains
// the platform fallback when the Tauri shell is not installed or cannot launch.
static inline void MSIMEOpenDesktopUpdateSettings(NSWorkspace *workspace,
                                                  dispatch_block_t fallback) {
    MSIMEOpenDesktopRoute(@"settings:about", workspace, fallback);
}

// These are settings categories from client-core, not input-panel routes.
static inline void MSIMEOpenDesktopSettings(MSIMEDesktopSettingsPage page,
                                           NSWorkspace *workspace,
                                           dispatch_block_t fallback) {
    NSString *route = @"settings:appearance";
    switch (page) {
        case MSIMEDesktopSettingsPage::Appearance: break;
        case MSIMEDesktopSettingsPage::Voice: route = @"settings:voice"; break;
        // Translation controls live in the shared Expression (表达) category.
        case MSIMEDesktopSettingsPage::Translation: route = @"settings:expression"; break;
        case MSIMEDesktopSettingsPage::AI: route = @"settings:ai"; break;
        case MSIMEDesktopSettingsPage::Skin: route = @"settings:skin"; break;
    }
    MSIMEOpenDesktopRoute(route, workspace, fallback);
}
