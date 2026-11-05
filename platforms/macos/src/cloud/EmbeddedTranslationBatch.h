#import <Foundation/Foundation.h>

NS_ASSUME_NONNULL_BEGIN

// 保留稳定查询身份，去掉只负责本次应用的代次；运行时仍在交回结果时核对当前会话。
NSDictionary * _Nullable MSIMEEmbeddedTranslationIdentity(NSDictionary *query, NSDictionary *view);

@interface MSIMEEmbeddedTranslationBatch : NSObject
- (instancetype)initWithQuery:(NSDictionary *)query completion:(void (^)(NSArray<NSDictionary *> *))completion;
- (void)start;
- (void)cancel;
@end

NS_ASSUME_NONNULL_END
