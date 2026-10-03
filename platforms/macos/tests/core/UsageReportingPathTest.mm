#import "../../src/core/UsageReporting.h"

#include <cassert>
#include <cstdlib>
#include <cstring>
#include <filesystem>

extern "C" char *msime_client_telemetry_begin(const uint8_t *, size_t);
extern "C" char *msime_client_telemetry_end(const uint8_t *, size_t);
extern "C" char *msime_client_telemetry_record_crash(const uint8_t *, size_t);
extern "C" char *msime_client_telemetry_flush(const uint8_t *, size_t);
extern "C" void msime_client_string_free(char *value);

namespace {
char *response() {
    constexpr const char value[] = R"({"ok":true,"value":{"enabled":false}})";
    char *copy = static_cast<char *>(std::malloc(sizeof(value)));
    assert(copy);
    std::memcpy(copy, value, sizeof(value));
    return copy;
}
}

extern "C" char *msime_client_telemetry_begin(const uint8_t *, size_t) { return response(); }
extern "C" char *msime_client_telemetry_end(const uint8_t *, size_t) { return response(); }
extern "C" char *msime_client_telemetry_record_crash(const uint8_t *, size_t) { return response(); }
extern "C" char *msime_client_telemetry_flush(const uint8_t *, size_t) { return response(); }
extern "C" void msime_client_string_free(char *value) { std::free(value); }

int main() {
    @autoreleasepool {
        const auto root = std::filesystem::path("/tmp/msime-usage-reporting-path-test");
        std::filesystem::remove_all(root);
        std::filesystem::create_directories(root / "Library/Application Support");
        const auto outside = root / "outside";
        std::filesystem::create_directories(outside);
        NSString *telemetry = MSIMEUsageReportingDirectory();
        NSString *msime = [telemetry stringByDeletingLastPathComponent];
        NSError *linkError = nil;
        if (![[NSFileManager defaultManager] createSymbolicLinkAtPath:msime
                                                     withDestinationPath:@(outside.c_str())
                                                                  error:&linkError]) {
            NSLog(@"telemetry=%@ msime=%@ outside=%@ error=%@", telemetry, msime, @(outside.c_str()), linkError);
            return 1;
        }

        // 启动时不应在 Application Support 的符号链接祖先下预创建 telemetry 目录。
        MSIMEUsageReportingStart(nil);
        assert(!std::filesystem::exists(outside / "telemetry"));

        std::filesystem::remove_all(root);
    }
    return 0;
}
