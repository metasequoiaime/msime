#import "../../src/core/UsageReporting.mm"

#include <cassert>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <filesystem>
#include <string>

extern "C" char *msime_client_telemetry_begin(const uint8_t *, size_t);
extern "C" char *msime_client_telemetry_end(const uint8_t *, size_t);
extern "C" char *msime_client_telemetry_record_crash(const uint8_t *, size_t);
extern "C" char *msime_client_telemetry_flush(const uint8_t *, size_t);
extern "C" void msime_client_string_free(char *value);

namespace {
// 无偏好目录的模式里任何宿主遥测调用都说明进程在没有用户授权来源时开始了会话或发送，直接失败。
bool gHostForbidden = false;

char *response() {
    constexpr const char value[] = R"({"ok":true,"value":{"enabled":false}})";
    char *copy = static_cast<char *>(std::malloc(sizeof(value)));
    assert(copy);
    std::memcpy(copy, value, sizeof(value));
    return copy;
}

char *host(const char *name) {
    if (gHostForbidden) {
        std::fprintf(stderr, "telemetry %s called without a preferences directory\n", name);
        std::abort();
    }
    return response();
}
}

extern "C" char *msime_client_telemetry_begin(const uint8_t *, size_t) { return host("begin"); }
extern "C" char *msime_client_telemetry_end(const uint8_t *, size_t) { return host("end"); }
extern "C" char *msime_client_telemetry_record_crash(const uint8_t *, size_t) { return host("record_crash"); }
extern "C" char *msime_client_telemetry_flush(const uint8_t *, size_t) { return host("flush"); }
extern "C" char *msime_client_telemetry_consent(const uint8_t *, size_t) { return host("consent"); }
extern "C" char *msime_client_telemetry_clear(const uint8_t *, size_t) { return host("clear"); }
extern "C" void msime_client_string_free(char *value) { std::free(value); }

namespace {
const std::filesystem::path kRoot("/tmp/msime-usage-reporting-path-test");

// 运行时配置缺少偏好目录（开发构建、配置尚未写出、被 O_NOFOLLOW 或大小上限拒绝、解析失败）时，输入法和原生设置窗口都不得按共用核心的旧协议默认开启：不调用任何宿主遥测入口、不武装记录器、不观察授权、不排发送。
int RunWithoutPreferences(bool settings) {
    std::filesystem::remove_all(kRoot);
    std::filesystem::create_directories(kRoot / "Library/Application Support");
    gHostForbidden = true;
    if (settings) MSIMESettingsCrashReportingStart(nil);
    else MSIMEUsageReportingStart(nil);
    // 排空发送队列：误排的 Flush 会在这里撞上桩并中止进程。
    dispatch_sync(gQueue, ^{});
    assert(!msime::crash::is_armed());
    assert(!gConsentObserver);
    assert(!gFlushTimer);
    assert(!std::filesystem::exists(std::filesystem::path(MSIMEUsageReportingDirectory().UTF8String)));
    MSIMEUsageReportingStop();
    assert(!msime::crash::is_armed());
    std::filesystem::remove_all(kRoot);
    return 0;
}

int RunSymlinkedSupport() {
    std::filesystem::remove_all(kRoot);
    std::filesystem::create_directories(kRoot / "Library/Application Support");
    const auto outside = kRoot / "outside";
    std::filesystem::create_directories(outside);
    const auto preferences = kRoot / "preferences";
    std::filesystem::create_directories(preferences);
    NSString *telemetry = MSIMEUsageReportingDirectory();
    NSString *msime = [telemetry stringByDeletingLastPathComponent];
    NSError *linkError = nil;
    if (![[NSFileManager defaultManager] createSymbolicLinkAtPath:msime
                                                 withDestinationPath:@(outside.c_str())
                                                              error:&linkError]) {
        NSLog(@"telemetry=%@ msime=%@ outside=%@ error=%@", telemetry, msime, @(outside.c_str()), linkError);
        return 1;
    }

    // 启动时不应在 Application Support 的符号链接祖先下预创建 telemetry 目录。给出真实偏好目录，确保走到开始会话的路径。
    MSIMEUsageReportingStart(@(preferences.c_str()));
    assert(!std::filesystem::exists(outside / "telemetry"));
    // 夹具不运行 AppKit 事件循环，主动结束并排空自己的发送队列，再析构共用状态。
    MSIMEUsageReportingStop();
    dispatch_sync(gQueue, ^{});

    std::filesystem::remove_all(kRoot);
    return 0;
}
}

int main(int argc, char **argv) {
    @autoreleasepool {
        const std::string mode = argc > 1 ? argv[1] : "";
        if (mode == "--nil-input") return RunWithoutPreferences(false);
        if (mode == "--nil-settings") return RunWithoutPreferences(true);
        return RunSymlinkedSupport();
    }
}
