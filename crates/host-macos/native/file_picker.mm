#import <AppKit/AppKit.h>
#import <UniformTypeIdentifiers/UniformTypeIdentifiers.h>
#include <cstdlib>
#include <cstring>

// Choosing a local speech model by typing its absolute path is not a real option on macOS: the path is long, Finder does not show it, and a typo surfaces as a recognizer that fails at the moment the user speaks. The reference host puts an NSOpenPanel behind a 选择… button; so does this, exposed as a host capability the shared settings page can ask for when the platform offers one. The model is an installed directory holding msime-model.json, so that button opens MSIMEDefaultVoiceModelDirectoryPicker.
//
// The panel itself cannot run headless, so the choice is injectable: tests drive the same path handling with their own picker, the way input source registration is tested.
extern "C" const char *MSIMEDefaultFilePicker(void) {
    NSOpenPanel *panel = [NSOpenPanel openPanel];
    panel.canChooseDirectories = NO;
    panel.canChooseFiles = YES;
    panel.allowsMultipleSelection = NO;
    panel.resolvesAliases = YES;
    if ([panel runModal] != NSModalResponseOK) return nullptr;
    // A security-scoped or aliased URL still answers .path; the recognizer opens by path.
    return panel.URL.path.UTF8String;
}

extern "C" const char *MSIMEDefaultDirectoryPicker(void) {
    NSOpenPanel *panel = [NSOpenPanel openPanel];
    panel.canChooseDirectories = YES;
    panel.canChooseFiles = NO;
    panel.canCreateDirectories = YES;
    panel.allowsMultipleSelection = NO;
    panel.resolvesAliases = YES;
    panel.message = @"请选择一个空文件夹存放水杉输入法的词库、学习记录和设置。";
    panel.prompt = @"选择";
    if ([panel runModal] != NSModalResponseOK) return nullptr;
    return panel.URL.path.UTF8String;
}

// An installed voice model is an existing directory, so the panel neither offers files nor creates folders, and it carries none of the data-directory panel's wording.
extern "C" const char *MSIMEDefaultVoiceModelDirectoryPicker(void) {
    NSOpenPanel *panel = [NSOpenPanel openPanel];
    panel.canChooseDirectories = YES;
    panel.canChooseFiles = NO;
    panel.canCreateDirectories = NO;
    panel.allowsMultipleSelection = NO;
    panel.resolvesAliases = YES;
    if ([panel runModal] != NSModalResponseOK) return nullptr;
    return panel.URL.path.UTF8String;
}

extern "C" char *msime_macos_pick_file_with(const char *(*picker)(void)) {
    if (picker == nullptr) return nullptr;
    @autoreleasepool {
        const char *chosen = picker();
        // Copy out of the autorelease pool: the caller owns the result and frees it below.
        if (chosen == nullptr || chosen[0] == '\0') return nullptr;
        return strdup(chosen);
    }
}

extern "C" char *msime_macos_pick_file(void) {
    if (!NSThread.isMainThread) return nullptr;
    return msime_macos_pick_file_with(MSIMEDefaultFilePicker);
}

extern "C" char *msime_macos_pick_directory(void) {
    if (!NSThread.isMainThread) return nullptr;
    return msime_macos_pick_file_with(MSIMEDefaultDirectoryPicker);
}

extern "C" char *msime_macos_pick_voice_model_directory(void) {
    if (!NSThread.isMainThread) return nullptr;
    return msime_macos_pick_file_with(MSIMEDefaultVoiceModelDirectoryPicker);
}

// 应用例外按 Bundle ID 记规则，可用户不知道、也不该去查 Bundle ID。设置页的「添加应用…」由宿主弹出这个只能选应用程序的面板，默认打开 /Applications，和原生设置窗口的「添加应用…」一样；交回所选应用的路径，Bundle ID 和名称再由下面的函数从路径读出。
extern "C" const char *MSIMEDefaultApplicationPicker(void) {
    NSOpenPanel *panel = [NSOpenPanel openPanel];
    panel.canChooseFiles = YES;
    panel.canChooseDirectories = NO;
    panel.allowsMultipleSelection = NO;
    panel.resolvesAliases = YES;
    // 应用程序是一个包；当成目录的话用户能走进包里选到别的文件。
    panel.treatsFilePackagesAsDirectories = NO;
    panel.allowedContentTypes = @[ UTTypeApplicationBundle ];
    panel.directoryURL = [NSURL fileURLWithPath:@"/Applications" isDirectory:YES];
    panel.prompt = @"添加";
    panel.message = @"选择一个应用程序，为它指定启动时的输入模式。";
    if ([panel runModal] != NSModalResponseOK) return nullptr;
    return panel.URL.path.UTF8String;
}

extern "C" char *msime_macos_pick_application(void) {
    if (!NSThread.isMainThread) return nullptr;
    return msime_macos_pick_file_with(MSIMEDefaultApplicationPicker);
}

static NSString *MSIMEStringFromUTF8(const char *value) {
    return value == nullptr ? nil : [NSString stringWithUTF8String:value];
}

// 调用方拥有返回值，用 msime_macos_free_picked_path 释放；空串当作没有。
static char *MSIMECopyString(NSString *value) {
    return value.length ? strdup(value.UTF8String) : nullptr;
}

// 路径上那个应用程序的 Bundle ID。不是应用程序（文档、脚本、只是名字以 .app 结尾的文件夹）或路径不存在时为 null。
extern "C" char *msime_macos_bundle_identifier_at(const char *path) {
    @autoreleasepool {
        NSString *location = MSIMEStringFromUTF8(path);
        if (location.length == 0) return nullptr;
        return MSIMECopyString([NSBundle bundleWithPath:location].bundleIdentifier);
    }
}

// 用户在 Finder 里看到的应用名称（本地化，按 Finder 的设置隐藏 .app）。
extern "C" char *msime_macos_display_name_at(const char *path) {
    @autoreleasepool {
        NSString *location = MSIMEStringFromUTF8(path);
        if (location.length == 0 || ![NSFileManager.defaultManager fileExistsAtPath:location]) return nullptr;
        return MSIMECopyString([NSFileManager.defaultManager displayNameAtPath:location]);
    }
}

// 本机已安装应用按 Bundle ID 查到的名称，规则列表靠它显示应用名而不是 Bundle ID。没装（别的 Mac 同步来的规则、已经删掉的应用）时为 null，列表就照旧显示 Bundle ID。
extern "C" char *msime_macos_application_name_for_bundle_identifier(const char *identifier) {
    @autoreleasepool {
        NSString *bundleIdentifier = MSIMEStringFromUTF8(identifier);
        if (bundleIdentifier.length == 0) return nullptr;
        NSURL *url = [NSWorkspace.sharedWorkspace URLForApplicationWithBundleIdentifier:bundleIdentifier];
        if (url == nil) return nullptr;
        return MSIMECopyString([NSFileManager.defaultManager displayNameAtPath:url.path]);
    }
}

extern "C" void msime_macos_free_picked_path(char *path) { free(path); }
