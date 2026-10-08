#import "CloudCandidateRequest.h"

#include "msime_client.h"
#import "../core/AIEndpointPolicy.h"

static BOOL MSIMEIsAllowedDescriptorURL(NSURL *url) {
    if ([url.scheme isEqual:@"https"]) return YES;
    if (![url.scheme isEqual:@"http"]) return NO;
    NSString *host = url.host.lowercaseString;
    return [host isEqual:@"localhost"] || [host isEqual:@"127.0.0.1"] || [host isEqual:@"::1"];
}

@implementation MSIMECloudCandidateRequest {
    NSURL *_url;
    NSURLSessionConfiguration *_configuration;
    NSURLSession *_session;
    NSURLSessionDataTask *_task;
    NSMutableData *_body;
    void (^_completion)(NSData *);
    BOOL _started;
    BOOL _accepted;
    NSURLRequest *_translationRequest;
    NSUInteger _maximumBodyBytes;
    NSTimeInterval _timeout;
}
- (instancetype)initWithURL:(NSURL *)url configuration:(NSURLSessionConfiguration *)configuration
                 completion:(void (^)(NSData *))completion {
    if ((self = [super init])) {
        _url = [url copy];
        _configuration = [configuration copy];
        _completion = [completion copy];
        _maximumBodyBytes = 262144;
        // NSURLSession has no separate connect budget, so the total is what can be honoured here;
        // the connect half of the shared pair is a subset of it.
        _timeout = MSIME_CLOUD_REQUEST_TIMEOUT_MS / 1000.0;
    }
    return self;
}
- (instancetype)initWithTranslationDescriptor:(NSDictionary *)descriptor configuration:(NSURLSessionConfiguration *)configuration
                                   completion:(void (^)(NSData *))completion {
    self = [self initWithURL:nil configuration:configuration completion:completion];
    if (!self) return nil;
    if (![descriptor isKindOfClass:NSDictionary.class] || ![descriptor[@"url"] isKindOfClass:NSString.class] ||
        ![descriptor[@"method"] isEqual:@"POST"] || ![descriptor[@"timeout_ms"] isEqual:@2500] ||
        ![descriptor[@"max_response_bytes"] isEqual:@1048576]) return self;
    NSString *address = descriptor[@"url"];
    if ([address lengthOfBytesUsingEncoding:NSUTF8StringEncoding] > 2048 ||
        [address rangeOfCharacterFromSet:NSCharacterSet.controlCharacterSet].location != NSNotFound) return self;
    NSURL *url = [NSURL URLWithString:address];
    if (!MSIMEIsAllowedDescriptorURL(url) || !url.host.length || url.user || url.password || url.fragment) return self;
    NSDictionary *headers = descriptor[@"headers"];
    if (![headers isKindOfClass:NSDictionary.class] || headers.count > 2 || ![headers[@"Content-Type"] isEqual:@"application/json"]) return self;
    for (id key in headers) {
        id value = headers[key];
        if (![@[@"Content-Type", @"Authorization"] containsObject:key] || ![value isKindOfClass:NSString.class] ||
            [value lengthOfBytesUsingEncoding:NSUTF8StringEncoding] > 4103 ||
            [value rangeOfCharacterFromSet:NSCharacterSet.controlCharacterSet].location != NSNotFound) return self;
    }
    if (![descriptor[@"body"] isKindOfClass:NSDictionary.class] || ![NSJSONSerialization isValidJSONObject:descriptor[@"body"]]) return self;
    NSData *body = [NSJSONSerialization dataWithJSONObject:descriptor[@"body"] options:0 error:nil];
    if (!body || body.length > 16384) return self;
    NSMutableURLRequest *request = [NSMutableURLRequest requestWithURL:url];
    request.HTTPMethod = @"POST";
    request.allHTTPHeaderFields = headers;
    request.HTTPBody = body;
    request.HTTPShouldHandleCookies = NO;
    request.timeoutInterval = 2.5;
    _translationRequest = [request copy];
    _maximumBodyBytes = 1048576;
    _timeout = 2.5;
    return self;
}
- (instancetype)initWithTencentDescriptor:(NSDictionary *)descriptor configuration:(NSURLSessionConfiguration *)configuration
                               completion:(void (^)(NSData *))completion {
    self = [self initWithURL:nil configuration:configuration completion:completion];
    if (!self) return nil;
    if (![descriptor isKindOfClass:NSDictionary.class] || ![descriptor[@"url"] isEqual:@"https://tmt.tencentcloudapi.com"] ||
        ![descriptor[@"method"] isEqual:@"POST"] || ![descriptor[@"timeout_ms"] isEqual:@2500] ||
        ![descriptor[@"max_response_bytes"] isEqual:@1048576] || ![descriptor[@"body_utf8"] isKindOfClass:NSString.class]) return self;
    NSDictionary *headers = descriptor[@"headers"];
    NSArray *allowed = @[@"Content-Type", @"Host", @"X-TC-Action", @"X-TC-Timestamp", @"X-TC-Version", @"X-TC-Region", @"Authorization"];
    if (![headers isKindOfClass:NSDictionary.class] || headers.count != allowed.count) return self;
    for (id name in headers) {
        id value = headers[name];
        if (![allowed containsObject:name] || ![value isKindOfClass:NSString.class] || ![value length] ||
            [value lengthOfBytesUsingEncoding:NSUTF8StringEncoding] > 8192 ||
            [value rangeOfCharacterFromSet:NSCharacterSet.controlCharacterSet].location != NSNotFound) return self;
    }
    if (![headers[@"Content-Type"] isEqual:@"application/json; charset=utf-8"] ||
        ![headers[@"Host"] isEqual:@"tmt.tencentcloudapi.com"] || ![headers[@"X-TC-Action"] isEqual:@"TextTranslateBatch"] ||
        ![headers[@"X-TC-Version"] isEqual:@"2018-03-21"] || ![headers[@"Authorization"] hasPrefix:@"TC3-HMAC-SHA256 "]) return self;
    NSString *timestamp = headers[@"X-TC-Timestamp"], *region = headers[@"X-TC-Region"];
    if (timestamp.length > 12 || [timestamp rangeOfCharacterFromSet:[[NSCharacterSet characterSetWithCharactersInString:@"0123456789"] invertedSet]].location != NSNotFound ||
        region.length > 64 || [region rangeOfCharacterFromSet:[[NSCharacterSet characterSetWithCharactersInString:@"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-"] invertedSet]].location != NSNotFound) return self;
    NSData *body = [descriptor[@"body_utf8"] dataUsingEncoding:NSUTF8StringEncoding];
    if (!body.length || body.length > 16384) return self;
    // Do not deserialize/reserialize: the TC3 signature binds these exact bytes.
    NSMutableURLRequest *request = [NSMutableURLRequest requestWithURL:[NSURL URLWithString:descriptor[@"url"]]];
    request.HTTPMethod = @"POST"; request.allHTTPHeaderFields = headers; request.HTTPBody = body;
    request.HTTPShouldHandleCookies = NO; request.timeoutInterval = 2.5;
    _translationRequest = [request copy]; _maximumBodyBytes = 1048576; _timeout = 2.5;
    return self;
}
- (instancetype)initWithNiuTransDescriptor:(NSDictionary *)descriptor configuration:(NSURLSessionConfiguration *)configuration
                                completion:(void (^)(NSData *))completion {
    self = [self initWithURL:nil configuration:configuration completion:completion];
    if (!self) return nil;
    if (![descriptor isKindOfClass:NSDictionary.class] ||
        ![descriptor[@"url"] isEqual:@"https://api.niutrans.com/v2/text/translate"] ||
        ![descriptor[@"method"] isEqual:@"POST"] || ![descriptor[@"timeout_ms"] isEqual:@2500] ||
        ![descriptor[@"max_response_bytes"] isEqual:@1048576] ||
        ![descriptor[@"body_utf8"] isKindOfClass:NSString.class]) return self;
    NSDictionary *headers = descriptor[@"headers"];
    if (![headers isKindOfClass:NSDictionary.class] || headers.count != 1 ||
        ![headers[@"Content-Type"] isEqual:@"application/x-www-form-urlencoded; charset=utf-8"]) return self;
    NSData *body = [descriptor[@"body_utf8"] dataUsingEncoding:NSUTF8StringEncoding];
    if (!body.length || body.length > 16384) return self;
    NSMutableURLRequest *request = [NSMutableURLRequest requestWithURL:[NSURL URLWithString:descriptor[@"url"]]];
    request.HTTPMethod = @"POST"; request.allHTTPHeaderFields = headers; request.HTTPBody = body;
    request.HTTPShouldHandleCookies = NO; request.timeoutInterval = 2.5;
    _translationRequest = [request copy]; _maximumBodyBytes = 1048576; _timeout = 2.5;
    return self;
}
- (instancetype)initWithAITranslationDescriptor:(NSDictionary *)descriptor configuration:(NSURLSessionConfiguration *)configuration
                                       completion:(void (^)(NSData *))completion {
    self = [self initWithURL:nil configuration:configuration completion:completion];
    if (!self) return nil;
    if (![descriptor isKindOfClass:NSDictionary.class] || ![descriptor[@"url"] isKindOfClass:NSString.class] ||
        ![descriptor[@"method"] isEqual:@"POST"] || ![descriptor[@"timeout_ms"] isEqual:@8000] ||
        ![descriptor[@"connect_timeout_ms"] isEqual:@2500] || ![descriptor[@"max_response_bytes"] isEqual:@1048576] ||
        ![descriptor[@"body"] isKindOfClass:NSDictionary.class] || ![NSJSONSerialization isValidJSONObject:descriptor[@"body"]]) return self;
    NSString *address = descriptor[@"url"];
    NSURL *url = [NSURL URLWithString:address];
    NSDictionary *headers = descriptor[@"headers"];
    // AI 描述符按 AIEndpointPolicy.h 检查：https 不限主机，http 只能指向本机或局域网（如 LM Studio）；翻译描述符仍用上面只放行回环地址的规则。
    if (MSIMEAIEndpointCheck(address, nil) != MSIMEAIEndpointProblemNone || !url.host.length || ![headers isKindOfClass:NSDictionary.class] ||
        headers.count != 2 || ![headers[@"Content-Type"] isEqual:@"application/json"] ||
        ![headers[@"Authorization"] isKindOfClass:NSString.class] || ![headers[@"Authorization"] hasPrefix:@"Bearer "]) return self;
    for (NSString *name in headers) {
        NSString *value = headers[name];
        if (![value isKindOfClass:NSString.class] || !value.length || [value lengthOfBytesUsingEncoding:NSUTF8StringEncoding] > 8192 ||
            [value rangeOfCharacterFromSet:NSCharacterSet.controlCharacterSet].location != NSNotFound) return self;
    }
    NSData *body = [NSJSONSerialization dataWithJSONObject:descriptor[@"body"] options:0 error:nil];
    if (!body || body.length > 65536) return self;
    NSMutableURLRequest *request = [NSMutableURLRequest requestWithURL:url];
    request.HTTPMethod = @"POST"; request.allHTTPHeaderFields = headers; request.HTTPBody = body;
    request.HTTPShouldHandleCookies = NO; request.timeoutInterval = 8;
    _translationRequest = [request copy]; _maximumBodyBytes = 1048576; _timeout = 8;
    return self;
}
- (void)start {
    NSAssert(NSThread.isMainThread, @"Cloud transport must run on main thread");
    if (_started || !_completion) return;
    _started = YES;
    if (!_translationRequest && (![_url.scheme isEqual:@"https"] || ![_url.host isEqual:@"inputtools.google.com"] ||
        _url.user || _url.password || (_url.port && _url.port.integerValue != 443))) {
        [self finish:nil]; return;
    }
    _configuration.URLCache = nil;
    _configuration.HTTPCookieStorage = nil;
    _configuration.URLCredentialStorage = nil;
    _configuration.HTTPShouldSetCookies = NO;
    _configuration.requestCachePolicy = NSURLRequestReloadIgnoringLocalCacheData;
    _configuration.timeoutIntervalForRequest = _timeout;
    _configuration.timeoutIntervalForResource = _timeout;
    // 明文 http 只会发往本机或局域网（AI 见 AIEndpointPolicy.h，翻译只放行回环地址），要直连：走系统代理的话，Token 会明文交给代理，代理还可能在公网上。空字典表示不用任何代理；https 仍按系统设置。
    if ([_translationRequest.URL.scheme.lowercaseString isEqualToString:@"http"]) _configuration.connectionProxyDictionary = @{};
    _body = [NSMutableData data];
    _session = [NSURLSession sessionWithConfiguration:_configuration delegate:self delegateQueue:NSOperationQueue.mainQueue];
    _task = _translationRequest ? [_session dataTaskWithRequest:_translationRequest] : [_session dataTaskWithURL:_url];
    [_task resume];
}
- (void)startInSession:(NSURLSession *)session {
    NSAssert(NSThread.isMainThread, @"Cloud transport must run on main thread");
    if (_started || !_completion) return;
    _started = YES;
    // Only a translation descriptor has passed validation; the owner of a shared session configured it, so this request neither owns nor invalidates it.
    if (!_translationRequest || !session) { [self finish:nil]; return; }
    _body = [NSMutableData data];
    _task = [session dataTaskWithRequest:_translationRequest];
    // A per-task delegate keeps the status, body-limit and redirect checks on this request even though the session is shared and has no delegate of its own.
    _task.delegate = self;
    [_task resume];
}
- (void)cancel {
    _completion = nil;
    [_task cancel];
    _task = nil;
    [_session invalidateAndCancel];
    _session = nil;
    _body = nil;
    _translationRequest = nil;
}
- (void)finish:(NSData *)body {
    void (^completion)(NSData *) = _completion;
    [self cancel];
    if (completion) completion(body);
}
- (void)URLSession:(NSURLSession *)session dataTask:(NSURLSessionDataTask *)task
 didReceiveResponse:(NSURLResponse *)response completionHandler:(void (^)(NSURLSessionResponseDisposition))completionHandler {
    (void)session; (void)task;
    NSInteger status = [response isKindOfClass:NSHTTPURLResponse.class] ? [(NSHTTPURLResponse *)response statusCode] : 0;
    _accepted = _completion && (status == 200 || (_translationRequest && status > 200 && status < 300)) &&
        response.expectedContentLength <= (int64_t)_maximumBodyBytes;
    completionHandler(_accepted ? NSURLSessionResponseAllow : NSURLSessionResponseCancel);
    if (!_accepted) [self finish:nil];
}
- (void)URLSession:(NSURLSession *)session dataTask:(NSURLSessionDataTask *)task didReceiveData:(NSData *)data {
    (void)session; (void)task;
    if (!_completion || !_accepted) return;
    if (data.length > _maximumBodyBytes - _body.length) { [self finish:nil]; return; }
    [_body appendData:data];
}
- (void)URLSession:(NSURLSession *)session task:(NSURLSessionTask *)task didCompleteWithError:(NSError *)error {
    (void)session; (void)task;
    [self finish:!error && _accepted && _body.length ? [_body copy] : nil];
}
- (void)URLSession:(NSURLSession *)session task:(NSURLSessionTask *)task
 willPerformHTTPRedirection:(NSHTTPURLResponse *)response newRequest:(NSURLRequest *)request
 completionHandler:(void (^)(NSURLRequest *))completionHandler {
    (void)session; (void)task; (void)response; (void)request;
    completionHandler(nil);
    [self finish:nil];
}
@end
