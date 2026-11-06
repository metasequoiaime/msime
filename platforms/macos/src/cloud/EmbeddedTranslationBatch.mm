#import "EmbeddedTranslationBatch.h"
#include "../../../common/EmbeddedTranslation.h"
#include <cstring>
#include <thread>

NSDictionary *MSIMEEmbeddedTranslationIdentity(NSDictionary *query, NSDictionary *view) {
    if (![query[@"provider"] isEqual:@"local"] ||
        ![query[@"generation"] isEqual:view[@"generation"]] ||
        ![query[@"candidates"] isKindOfClass:NSArray.class] || ![query[@"candidates"] count]) return nil;
    NSMutableDictionary *identity = [query mutableCopy];
    [identity removeObjectForKey:@"generation"];
    [identity removeObjectForKey:@"gloss_query"];
    return identity;
}

@implementation MSIMEEmbeddedTranslationBatch {
    NSDictionary *_query;
    void (^_completion)(NSArray<NSDictionary *> *);
    std::shared_ptr<std::atomic<bool>> _cancelled;
}
- (instancetype)initWithQuery:(NSDictionary *)query completion:(void (^)(NSArray<NSDictionary *> *))completion {
    if ((self = [super init])) {
        _query = [query copy];
        _completion = [completion copy];
        _cancelled = std::make_shared<std::atomic<bool>>(false);
    }
    return self;
}
- (void)cancel { _cancelled->store(true); }
- (void)dealloc { [self cancel]; }
- (void)start {
    const auto cancelled = _cancelled;
    const auto requestID = msime::translation::next_request_id();
    NSMutableDictionary *query = [_query mutableCopy];
    query[@"request_id"] = [NSString stringWithUTF8String:requestID.c_str()];
    const auto deadline = std::chrono::steady_clock::now() + std::chrono::seconds(30);
    void (^completion)(NSArray<NSDictionary *> *) = [_completion copy];
    dispatch_async(dispatch_get_global_queue(QOS_CLASS_UTILITY, 0), ^{
        @autoreleasepool {
            NSArray *translations = nil;
            try {
                while (!cancelled->load()) {
                    const auto remaining = std::chrono::duration_cast<std::chrono::milliseconds>(
                        deadline - std::chrono::steady_clock::now()).count();
                    if (remaining <= 0) break;
                    query[@"timeout_ms"] = @(remaining);
                    NSData *data = [NSJSONSerialization dataWithJSONObject:query options:0 error:nil];
                    if (!data) break;
                    const std::string request(static_cast<const char *>(data.bytes), data.length);
                    auto raw = msime::translation::embedded(request, requestID, [cancelled] { return cancelled->load(); });
                    if (!raw || cancelled->load()) break;
                    NSData *result = [NSData dataWithBytes:raw.get() length:std::strlen(raw.get())];
                    NSDictionary *envelope = [NSJSONSerialization JSONObjectWithData:result options:0 error:nil];
                    if (![envelope isKindOfClass:NSDictionary.class]) break;
                    // 旧批次取消后仍可能占着队列；仅重试忙碌，等待时间计入同一个截止时间。
                    if (![envelope[@"ok"] boolValue] && [envelope[@"error"] isEqual:@"local_translation_busy"]) {
                        std::this_thread::sleep_for(std::chrono::milliseconds(100));
                        continue;
                    }
                    if ([envelope[@"ok"] boolValue] && [envelope[@"value"] isKindOfClass:NSDictionary.class] &&
                        [envelope[@"value"][@"translations"] isKindOfClass:NSArray.class])
                        translations = envelope[@"value"][@"translations"];
                    break;
                }
            } catch (...) {
                // 失败也结束批次，不启动其他翻译服务。
            }
            dispatch_async(dispatch_get_main_queue(), ^{
                if (!cancelled->load()) completion(translations);
            });
        }
    });
}
@end
