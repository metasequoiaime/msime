#import "../../src/cloud/EmbeddedTranslationBatch.h"
#include "msime_client.h"
#include <atomic>
#include <cassert>
#include <cstdlib>
#include <cstring>
#include <thread>

static std::atomic<int> calls{0};
extern "C" char *msime_client_local_translation_query(const uint8_t *, size_t) {
    ++calls;
    std::this_thread::sleep_for(std::chrono::milliseconds(100));
    return strdup(R"({"ok":true,"value":{"translations":[{"text":"合成文本","translation":"synthetic text"}]}})");
}
extern "C" void msime_client_local_translation_cancel(const uint8_t *, size_t) {}
extern "C" void msime_client_string_free(char *raw) { std::free(raw); }

static void drain(NSTimeInterval seconds) {
    NSDate *end = [NSDate dateWithTimeIntervalSinceNow:seconds];
    while ([end timeIntervalSinceNow] > 0)
        [NSRunLoop.currentRunLoop runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.01]];
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
    }
}
