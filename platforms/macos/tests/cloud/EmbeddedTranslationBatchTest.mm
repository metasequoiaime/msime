#import "../../src/cloud/EmbeddedTranslationBatch.h"
#include "msime_client.h"
#include <atomic>
#include <cassert>
#include <cstdlib>
#include <cstring>
#include <thread>

static std::atomic<int> calls{0};
static std::atomic<int> busyReplies{0};
static std::atomic<bool> unavailable{false};
static std::atomic<long long> previousBudget{0};
extern "C" char *msime_client_local_translation_query(const uint8_t *request, size_t length) {
    ++calls;
    if (previousBudget) {
        NSDictionary *query = [NSJSONSerialization JSONObjectWithData:[NSData dataWithBytes:request length:length] options:0 error:nil];
        const auto budget = [query[@"timeout_ms"] longLongValue];
        assert(budget > 0 && budget < previousBudget.exchange(budget));
    }
    std::this_thread::sleep_for(std::chrono::milliseconds(100));
    if (busyReplies.fetch_sub(1) > 0) return strdup(R"({"ok":false,"error":"local_translation_busy"})");
    if (unavailable) return strdup(R"({"ok":false,"error":"local_translation_unavailable"})");
    return strdup(R"({"ok":true,"value":{"translations":[{"text":"合成文本","translation":"synthetic text"}]}})");
}
extern "C" void msime_client_local_translation_cancel(const uint8_t *, size_t) {}
extern "C" void msime_client_string_free(char *raw) { std::free(raw); }

static void drain(NSTimeInterval seconds) {
    NSDate *end = [NSDate dateWithTimeIntervalSinceNow:seconds];
    while ([end timeIntervalSinceNow] > 0)
        [NSRunLoop.currentRunLoop runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.01]];
}
static bool awaitReply(BOOL (^completed)(void)) {
    // 忙碌重试包含后台线程和主队列交接，按完成条件等待，不把机器调度延迟当作产品超时。
    NSDate *deadline = [NSDate dateWithTimeIntervalSinceNow:5];
    while (!completed() && [deadline timeIntervalSinceNow] > 0) drain(0.01);
    return completed();
}
int main() {
    @autoreleasepool {
        NSDictionary *query = @{@"provider":@"local", @"generation":@1, @"target_language":@"yue",
            @"candidates":@[@{@"text":@"合成文本"}], @"state_root":@"/synthetic/state"};
        NSDictionary *identity = MSIMEEmbeddedTranslationIdentity(query, @{@"generation":@1});
        assert(identity && !identity[@"generation"]);
        assert(!MSIMEEmbeddedTranslationIdentity(query, @{@"generation":@2}));
        NSMutableDictionary *next = [query mutableCopy]; next[@"generation"] = @2;
        assert([identity isEqual:MSIMEEmbeddedTranslationIdentity(next, @{@"generation":@2})]);
        __block NSUInteger replies = 0;
        MSIMEEmbeddedTranslationBatch *old = [[MSIMEEmbeddedTranslationBatch alloc] initWithQuery:identity completion:^(NSArray *) { ++replies; }];
        [old start];
        while (!calls.load()) std::this_thread::yield();
        [old cancel];
        drain(0.2);
        assert(replies == 0);
        MSIMEEmbeddedTranslationBatch *current = [[MSIMEEmbeddedTranslationBatch alloc] initWithQuery:identity completion:^(NSArray *rows) {
            assert(NSThread.isMainThread);
            assert([rows.firstObject[@"translation"] isEqual:@"synthetic text"]);
            ++replies;
        }];
        [current start];
        drain(0.3);
        assert(replies == 1);

        // 旧请求取消尚未收尾时队列可能短暂满载；输入停在同一候选也必须最终得到结果。
        busyReplies = 2;
        previousBudget = 30001;
        MSIMEEmbeddedTranslationBatch *retry = [[MSIMEEmbeddedTranslationBatch alloc] initWithQuery:identity completion:^(NSArray *rows) {
            assert(NSThread.isMainThread);
            assert([rows.firstObject[@"translation"] isEqual:@"synthetic text"]);
            ++replies;
        }];
        [retry start];
        assert(awaitReply(^BOOL { return replies == 2; }));
        previousBudget = 0;

        // 非暂时失败也要结束批次，不能让控制器一直保留请求及其客户端。
        unavailable = true;
        const int beforeFailure = calls.load();
        MSIMEEmbeddedTranslationBatch *failure = [[MSIMEEmbeddedTranslationBatch alloc] initWithQuery:identity completion:^(NSArray *rows) {
            assert(NSThread.isMainThread && rows == nil);
            ++replies;
        }];
        [failure start];
        assert(awaitReply(^BOOL { return replies == 3; }));
        assert(calls.load() == beforeFailure + 1);
        unavailable = false;

        busyReplies = 100;
        const int before = calls.load();
        MSIMEEmbeddedTranslationBatch *cancelRetry = [[MSIMEEmbeddedTranslationBatch alloc] initWithQuery:identity completion:^(NSArray *) { ++replies; }];
        [cancelRetry start];
        while (calls.load() == before) std::this_thread::yield();
        [cancelRetry cancel];
        drain(0.3);
        const int stopped = calls.load();
        drain(0.3);
        assert(calls.load() == stopped && replies == 3);
    }
}
