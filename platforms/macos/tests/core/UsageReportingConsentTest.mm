// 使用真实宿主适配和共用生命周期；替代 ABI 回复以控制在途发送，不连接产品网络。
#import "../../src/core/UsageReporting.mm"
#include "../../../common/CrashRecorder.h"
#include <atomic>
#include <cassert>
#include <chrono>
#include <condition_variable>
#include <cstdlib>
#include <cstring>
#include <functional>
#include <string>
#include <thread>

namespace {
std::atomic<bool> consent{true};
std::atomic<unsigned> epoch{1};
std::atomic<unsigned> begun{0};
std::atomic<unsigned> stackReads{0};
std::atomic<unsigned> chained{0};
std::atomic<unsigned> exceptionRecords{0};
std::string ended;
std::mutex sendingMutex;
std::condition_variable sendingChanged;
bool blockSending = false;
bool sendingEntered = false;
bool sendingReleased = false;
char *reply(const char *text) { return strdup(text); }
std::string currentEpoch() { return "synthetic-epoch-" + std::to_string(epoch.load()); }
void waitFor(const std::function<bool()> &condition) {
    const auto deadline = std::chrono::steady_clock::now() + std::chrono::seconds(10);
    while (!condition() && std::chrono::steady_clock::now() < deadline)
        std::this_thread::sleep_for(std::chrono::milliseconds(10));
    assert(condition());
}
NSDictionary *request(const uint8_t *body, size_t size) {
    return [NSJSONSerialization JSONObjectWithData:[NSData dataWithBytes:body length:size] options:0 error:nil];
}
void PreviousException(NSException *) { ++chained; }
}

@interface ConsentProbeException : NSException
@end
@implementation ConsentProbeException
- (NSArray<NSString *> *)callStackSymbols {
    ++stackReads;
    return @[@"synthetic-throw-site"];
}
@end

extern "C" char *msime_client_telemetry_consent(const uint8_t *, size_t) {
    if (!consent.load()) return reply(R"({"ok":true,"value":{"enabled":false}})");
    const auto body = R"({"ok":true,"value":{"enabled":true,"epoch":")" + currentEpoch() + R"("}})";
    return reply(body.c_str());
}
extern "C" char *msime_client_telemetry_clear(const uint8_t *, size_t) {
    return reply(R"({"ok":true,"value":true})");
}
extern "C" char *msime_client_telemetry_begin(const uint8_t *body, size_t size) {
    NSDictionary *value = request(body, size);
    assert(value[@"enabled"] == nil);
    assert([value[@"consent_epoch"] isEqualToString:@(currentEpoch().c_str())]);
    const auto count = ++begun;
    const auto id = std::string("11111111-1111-4111-8111-") + std::string(12 - std::to_string(count).size(), '0') + std::to_string(count);
    const auto response = std::string(R"({"ok":true,"value":{"enabled":true,"session_id":")") + id +
        R"(","crash_record_path":"/synthetic/)" + id + R"(.crash","crash_record_header":""}})";
    return reply(response.c_str());
}
extern "C" char *msime_client_telemetry_flush(const uint8_t *body, size_t size) {
    const auto value = request(body, size);
    assert(value[@"enabled"] == nil && value[@"consent_epoch"] != nil);
    std::unique_lock lock(sendingMutex);
    if (blockSending) {
        sendingEntered = true;
        sendingChanged.notify_all();
        sendingChanged.wait(lock, [] { return sendingReleased; });
        // 旧请求在关闭期间完成，结果晚于授权重开到达。
        return reply(R"({"ok":true,"value":{"enabled":false}})");
    }
    return reply(R"({"ok":true,"value":{"enabled":true}})");
}
extern "C" char *msime_client_telemetry_end(const uint8_t *body, size_t size) {
    NSDictionary *request = [NSJSONSerialization JSONObjectWithData:[NSData dataWithBytes:body length:size] options:0 error:nil];
    ended = [request[@"session"] UTF8String];
    return reply(R"({"ok":true,"value":true})");
}
extern "C" char *msime_client_telemetry_record_crash(const uint8_t *body, size_t size) {
    const auto value = request(body, size);
    assert([value[@"message"] isEqualToString:@"Uncaught SyntheticProbe"]);
    assert([value[@"stack"] isEqualToString:@"synthetic-throw-site"]);
    ++exceptionRecords;
    return reply(R"({"ok":true,"value":false})");
}
extern "C" void msime_client_string_free(char *value) { std::free(value); }

int main(int argc, char **) {
    @autoreleasepool {
        const bool initiallyDisabled = argc > 1;
        consent.store(!initiallyDisabled);
        NSSetUncaughtExceptionHandler(PreviousException);
        MSIMEUsageReportingStart(@"/synthetic/preferences");
        dispatch_sync(gQueue, ^{});
        assert(msime::crash::is_armed() == !initiallyDisabled);
        consent.store(false);
        waitFor([] { return !msime::crash::is_armed(); });
        NSException *probe = [[ConsentProbeException alloc] initWithName:@"SyntheticProbe"
                                                               reason:@"synthetic-private-input" userInfo:nil];
        UncaughtException(probe);
        assert(stackReads.load() == 0 && exceptionRecords.load() == 0 && chained.load() == 1);
        epoch.fetch_add(1);
        consent.store(true);
        waitFor([] { return msime::crash::is_armed(); });
        // 回调遇到观察线程的快照锁时允许放弃，重试只用于确保本测试实际覆盖已授权的抛出点。
        waitFor([probe] {
            if (exceptionRecords.load() == 0) UncaughtException(probe);
            return exceptionRecords.load() == 1;
        });
        assert(stackReads.load() >= 1 && chained.load() == stackReads.load() + 1);
        const auto previousBegun = begun.load();
        epoch.fetch_add(1);
        waitFor([previousBegun] { return begun.load() > previousBegun; });
        {
            std::lock_guard guard(sendingMutex);
            blockSending = true;
        }
        std::thread network([] { @autoreleasepool { FlushNow(); } });
        {
            std::unique_lock lock(sendingMutex);
            assert(sendingChanged.wait_for(lock, std::chrono::seconds(10), [] { return sendingEntered; }));
        }
        consent.store(false);
        waitFor([] { return !msime::crash::is_armed(); });
        const auto beforeReopen = begun.load();
        epoch.fetch_add(1);
        consent.store(true);
        waitFor([beforeReopen] { return msime::crash::is_armed() && begun.load() > beforeReopen; });
        const auto current = begun.load();
        {
            std::lock_guard guard(sendingMutex);
            sendingReleased = true;
        }
        sendingChanged.notify_all();
        network.join();
        assert(msime::crash::is_armed() && begun.load() == current);
        MSIMEUsageReportingStop();
        assert(!ended.empty());
        Flush();
        dispatch_sync(gQueue, ^{});
        assert(!msime::crash::is_armed() && begun.load() == current);
    }
}
