#import <Foundation/Foundation.h>

/// One bounded ephemeral request. All methods and completion run on main thread.
@interface MSIMECloudCandidateRequest : NSObject <NSURLSessionDataDelegate>
/// Fixed NiuTrans HTTPS endpoint; signed form bytes are not reserialized.
- (instancetype)initWithNiuTransDescriptor:(NSDictionary *)descriptor configuration:(NSURLSessionConfiguration *)configuration
                                completion:(void (^)(NSData *body))completion;
- (instancetype)initWithURL:(NSURL *)url configuration:(NSURLSessionConfiguration *)configuration
                 completion:(void (^)(NSData *body))completion;
/// 执行共享层构造的云候选 GET/POST；缓存命中时不建立连接。
- (instancetype)initWithCloudDescriptor:(NSDictionary *)descriptor configuration:(NSURLSessionConfiguration *)configuration
                            completion:(void (^)(NSData *body))completion;
- (void)start;
/// Translation descriptors only: runs as one task in a session the caller owns and configured, so consecutive requests can reuse its connection. Cancelling cancels only this task.
- (void)startInSession:(NSURLSession *)session;
/// Consume the shared custom-translation descriptor. HTTP(S) only; no redirects.
- (instancetype)initWithTranslationDescriptor:(NSDictionary *)descriptor configuration:(NSURLSessionConfiguration *)configuration
                                   completion:(void (^)(NSData *body))completion;
/// Consume the shared AI chat descriptor. HTTP(S) only; no redirects.
- (instancetype)initWithAITranslationDescriptor:(NSDictionary *)descriptor configuration:(NSURLSessionConfiguration *)configuration
                                       completion:(void (^)(NSData *body))completion;
/// Cancel without delivering a result; safe after completion.
- (void)cancel;
/// Fixed Tencent HTTPS endpoint. Sends signed body_utf8 unchanged; no redirects.
- (instancetype)initWithTencentDescriptor:(NSDictionary *)descriptor configuration:(NSURLSessionConfiguration *)configuration
                               completion:(void (^)(NSData *body))completion;
@end
