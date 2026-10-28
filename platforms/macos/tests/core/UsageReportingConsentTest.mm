// 包含真实实现以主动触发定时刷新，不改变生产计时或增加测试专用入口。
#import "../../src/core/UsageReporting.mm"
#include <atomic>
#include <cassert>
#include <cstdlib>
#include <cstring>
#include <string>

namespace {
std::atomic<bool> consent{true};
std::atomic<unsigned> begun{0};
std::atomic<unsigned> generation{1};
std::atomic<bool> readable{true};
std::atomic<int> prepareChange{0};
std::string ended;
char *reply(const char *text) { return strdup(text); }
}

extern "C" char *msime_client_telemetry_consent(const uint8_t *, size_t) {
    if (!readable.load()) return reply(R"({"ok":false,"error":"synthetic-unavailable"})");
    if (!consent.load()) return reply(R"({"ok":true,"value":{"enabled":false}})");
    return reply((std::string(R"({"ok":true,"value":{"enabled":true,"epoch":")") + std::to_string(generation.load()) + R"("}})").c_str());
}
extern "C" char *msime_client_telemetry_clear(const uint8_t *, size_t) { return reply(R"({"ok":true,"value":true})"); }
extern "C" char *msime_client_telemetry_begin(const uint8_t *, size_t) {
    const auto count = ++begun;
    if (!consent.load()) return reply(R"({"ok":true,"value":{"enabled":false}})");
    const auto change = prepareChange.exchange(0);
    if (change == 1) generation.fetch_add(1);
    if (change == 2) consent.store(false);
    const auto id = std::string("11111111-1111-4111-8111-") + std::string(12 - std::to_string(count).size(), '0') + std::to_string(count);
    const auto body = std::string(R"({"ok":true,"value":{"enabled":true,"session_id":")") + id +
        R"(","crash_record_path":"/synthetic/)" + id + R"(.crash","crash_record_header":""}})";
    return reply(body.c_str());
}
extern "C" char *msime_client_telemetry_flush(const uint8_t *, size_t) {
    return reply(consent.load() ? R"({"ok":true,"value":{"enabled":true}})" : R"({"ok":true,"value":{"enabled":false}})");
}
extern "C" char *msime_client_telemetry_end(const uint8_t *body, size_t size) {
    NSDictionary *request = [NSJSONSerialization JSONObjectWithData:[NSData dataWithBytes:body length:size] options:0 error:nil];
    ended = [request[@"session"] UTF8String];
    return reply(R"({"ok":true,"value":true})");
}
extern "C" char *msime_client_telemetry_record_crash(const uint8_t *, size_t) { return reply(R"({"ok":true,"value":false})"); }
extern "C" void msime_client_string_free(char *value) { std::free(value); }

int main(int argc, char **) {
    @autoreleasepool {
        const bool initiallyDisabled = argc > 1;
        consent.store(!initiallyDisabled);
        MSIMEUsageReportingStart(@"/synthetic/preferences");
        dispatch_sync(gQueue, ^{});
        dispatch_source_cancel(gConsentTimer);
        dispatch_sync(gQueue, ^{});
        assert(msime::crash::is_armed() == !initiallyDisabled);
        const auto previousBegun = begun.load();
        consent.store(false);
        Flush();
        dispatch_sync(gQueue, ^{});
        assert(!msime::crash::is_armed());
        consent.store(true);
        Flush();
        dispatch_sync(gQueue, ^{});
        assert(msime::crash::is_armed());
        assert(begun.load() > previousBegun);
        const auto stable = begun.load();
        readable.store(false);
        Flush(); dispatch_sync(gQueue, ^{});
        assert(!msime::crash::is_armed());
        readable.store(true);
        Flush(); dispatch_sync(gQueue, ^{});
        assert(msime::crash::is_armed() && begun.load() == stable);
        // 两次观察之间关闭重开，布尔值不变也必须换身份。
        generation.fetch_add(1);
        Flush(); dispatch_sync(gQueue, ^{});
        assert(msime::crash::is_armed() && begun.load() == stable + 1);
        // begin 已创建 E1 会话，但回包之前授权变成 E2：不得重新武装旧头。
        generation.fetch_add(1);
        prepareChange.store(1);
        Flush(); dispatch_sync(gQueue, ^{});
        assert(!msime::crash::is_armed());
        Flush(); dispatch_sync(gQueue, ^{});
        assert(msime::crash::is_armed());
        // 同一慢回包边界撤回时也保持停用。
        generation.fetch_add(1);
        prepareChange.store(2);
        Flush(); dispatch_sync(gQueue, ^{});
        assert(!msime::crash::is_armed());
        consent.store(true);
        generation.fetch_add(1);
        Flush(); dispatch_sync(gQueue, ^{});
        assert(msime::crash::is_armed());
        const auto capture = std::atomic_load(&gCapture);
        assert(capture);
        const std::string expectedEnd = capture->session.UTF8String;
        MSIMEUsageReportingStop();
        assert(ended == expectedEnd);
        Flush();
        dispatch_sync(gQueue, ^{});
        assert(!msime::crash::is_armed());
    }
}
