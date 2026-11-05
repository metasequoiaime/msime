#import "EmbeddedTranslationBatch.h"
#include "../../../common/EmbeddedTranslation.h"
#include <cstring>

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
    NSData *data = [NSJSONSerialization dataWithJSONObject:query options:0 error:nil];
    if (!data) return;
    const std::string request(static_cast<const char *>(data.bytes), data.length);
    void (^completion)(NSArray<NSDictionary *> *) = [_completion copy];
    dispatch_async(dispatch_get_global_queue(QOS_CLASS_UTILITY, 0), ^{
        @autoreleasepool {
            try {
                auto raw = msime::translation::embedded(request, requestID, [cancelled] { return cancelled->load(); });
                if (!raw || cancelled->load()) return;
                NSData *result = [NSData dataWithBytes:raw.get() length:std::strlen(raw.get())];
                NSDictionary *envelope = [NSJSONSerialization JSONObjectWithData:result options:0 error:nil];
                if (![envelope isKindOfClass:NSDictionary.class] || ![envelope[@"ok"] boolValue] ||
                    ![envelope[@"value"] isKindOfClass:NSDictionary.class]) return;
                NSArray *translations = envelope[@"value"][@"translations"];
                if (![translations isKindOfClass:NSArray.class]) return;
                dispatch_async(dispatch_get_main_queue(), ^{
                    if (!cancelled->load()) completion(translations);
                });
            } catch (...) {
                // 推理失败不启动其他翻译服务；下一次查询仍可重试。
            }
        }
    });
}
@end
