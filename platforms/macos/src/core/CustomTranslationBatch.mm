#import "CustomTranslationBatch.h"
#import "../cloud/CloudCandidateRequest.h"
#import "MSIMEClientSession.h"
#include "../candidate/CandidatePageSize.h"

@implementation MSIMECustomTranslationBatch {
    NSArray<NSDictionary *> *_items;
    NSURLSessionConfiguration *_configuration;
    void (^_completion)(NSArray<NSDictionary *> *);
    NSMutableArray<NSDictionary *> *_results;
    MSIMECloudCandidateRequest *_request;
    NSURLSession *_session;
    NSTimer *_timer;
    NSTimeInterval _deadline;
    NSUInteger _nextIndex;
    BOOL _started;
    BOOL _tencent;
    BOOL _ai;
    BOOL _niuTrans;
    BOOL _detached;
}
- (instancetype)initWithNiuTransItems:(NSArray<NSDictionary *> *)items config:(NSDictionary *)config
                        configuration:(NSURLSessionConfiguration *)configuration
                           completion:(void (^)(NSArray<NSDictionary *> *))completion {
    NSMutableArray *requests = [NSMutableArray array];
    BOOL valid = [items isKindOfClass:NSArray.class] && items.count <= msime::mac::kMaximumCandidatePageSize && [config isKindOfClass:NSDictionary.class];
    if (valid) for (id item in items) {
        if (![item isKindOfClass:NSDictionary.class]) { valid = NO; break; }
        for (NSString *key in @[@"text", @"key", @"source_language", @"target_language"])
            if (![item[key] isKindOfClass:NSString.class] || ![item[key] length]) valid = NO;
        if (!valid) break;
        [requests addObject:@{@"text":item[@"text"], @"request":@{@"config":config, @"text":item[@"key"],
            @"source_language":item[@"source_language"], @"target_language":item[@"target_language"]}}];
    }
    self = [self initWithItems:valid ? requests : @[] configuration:configuration completion:completion];
    if (self) _niuTrans = YES;
    return self;
}
- (MSIMECloudCandidateRequest *)niuTransRequestForDescriptor:(NSDictionary *)descriptor completion:(void (^)(NSData *))completion {
    return [[MSIMECloudCandidateRequest alloc] initWithNiuTransDescriptor:descriptor configuration:_configuration completion:completion];
}
- (instancetype)initWithAIItems:(NSArray<NSDictionary *> *)items
                   configuration:(NSURLSessionConfiguration *)configuration
                      completion:(void (^)(NSArray<NSDictionary *> *))completion {
    self = [self initWithItems:@[] configuration:configuration completion:completion];
    if (!self) return nil;
    _ai = YES;
    if (![items isKindOfClass:NSArray.class] || items.count > 10 || ![NSJSONSerialization isValidJSONObject:items]) return self;
    NSData *data = [NSJSONSerialization dataWithJSONObject:items options:0 error:nil];
    if (!data.length || data.length > 65536) return self;
    _items = [NSJSONSerialization JSONObjectWithData:data options:0 error:nil];
    for (NSDictionary *item in _items) {
        if (![item[@"text"] isKindOfClass:NSString.class] || ![item[@"text"] length] ||
            ![item[@"request"] isKindOfClass:NSDictionary.class]) { _items = nil; break; }
        NSNumber *limit = item[@"candidate_limit"];
        if (limit && (![limit isKindOfClass:NSNumber.class] ||
            CFGetTypeID((__bridge CFTypeRef)limit) == CFBooleanGetTypeID() ||
            CFNumberIsFloatType((__bridge CFNumberRef)limit) ||
            limit.integerValue < 1 || limit.integerValue > 10)) { _items = nil; break; }
    }
    return self;
}
- (instancetype)initWithItems:(NSArray<NSDictionary *> *)items
                configuration:(NSURLSessionConfiguration *)configuration
                   completion:(void (^)(NSArray<NSDictionary *> *))completion {
    if ((self = [super init])) {
        _completion = [completion copy];
        _configuration = [configuration copy];
        _results = [NSMutableArray array];
        if (![items isKindOfClass:NSArray.class] || items.count > msime::mac::kMaximumCandidatePageSize ||
            ![NSJSONSerialization isValidJSONObject:items]) return self;
        for (id item in items) {
            if (![item isKindOfClass:NSDictionary.class] || ![item[@"text"] isKindOfClass:NSString.class] ||
                ![item[@"text"] length] || [item[@"text"] lengthOfBytesUsingEncoding:NSUTF8StringEncoding] > 4096 ||
                ![item[@"request"] isKindOfClass:NSDictionary.class]) return self;
        }
        NSData *data = [NSJSONSerialization dataWithJSONObject:items options:0 error:nil];
        if (data.length && data.length <= 262144)
            _items = [NSJSONSerialization JSONObjectWithData:data options:0 error:nil];
    }
    return self;
}
- (instancetype)initWithTencentItems:(NSArray<NSDictionary *> *)items
                               config:(NSDictionary *)config
                        configuration:(NSURLSessionConfiguration *)configuration
                           completion:(void (^)(NSArray<NSDictionary *> *))completion {
    self = [self initWithItems:@[] configuration:configuration completion:completion];
    if (!self) return nil;
    _tencent = YES;
    if (![items isKindOfClass:NSArray.class] || items.count > msime::mac::kMaximumCandidatePageSize ||
        ![config isKindOfClass:NSDictionary.class]) return self;
    NSDictionary *input = @{@"items":items, @"config":config};
    if (![NSJSONSerialization isValidJSONObject:input]) return self;
    NSData *data = [NSJSONSerialization dataWithJSONObject:input options:0 error:nil];
    if (!data.length || data.length > 65536) return self;
    input = [NSJSONSerialization JSONObjectWithData:data options:0 error:nil];
    NSMutableArray<NSMutableDictionary *> *groups = [NSMutableArray array];
    for (id item in input[@"items"]) {
        if (![item isKindOfClass:NSDictionary.class]) return self;
        for (NSString *field in @[@"text", @"key", @"source_language", @"target_language"]) {
            if (![item[field] isKindOfClass:NSString.class] || ![item[field] length] ||
                [item[field] lengthOfBytesUsingEncoding:NSUTF8StringEncoding] > 4096) return self;
        }
        NSMutableDictionary *group = nil;
        for (NSMutableDictionary *candidate in groups) {
            if ([candidate[@"source_language"] isEqual:item[@"source_language"]] &&
                [candidate[@"target_language"] isEqual:item[@"target_language"]]) { group = candidate; break; }
        }
        if (!group) {
            group = [@{@"source_language":item[@"source_language"], @"target_language":item[@"target_language"],
                @"config":input[@"config"], @"texts":[NSMutableArray array], @"originals":[NSMutableArray array]} mutableCopy];
            [groups addObject:group];
        }
        [group[@"texts"] addObject:item[@"key"]];
        [group[@"originals"] addObject:item[@"text"]];
    }
    _items = [groups copy];
    return self;
}
- (NSTimeInterval)currentTime { return NSProcessInfo.processInfo.systemUptime; }
- (NSTimeInterval)unixTime { return NSDate.date.timeIntervalSince1970; }
- (MSIMECloudCandidateRequest *)tencentRequestForDescriptor:(NSDictionary *)descriptor completion:(void (^)(NSData *))completion {
    return [[MSIMECloudCandidateRequest alloc] initWithTencentDescriptor:descriptor
        configuration:_configuration completion:completion];
}
- (MSIMECloudCandidateRequest *)AIRequestForDescriptor:(NSDictionary *)descriptor completion:(void (^)(NSData *))completion {
    return [[MSIMECloudCandidateRequest alloc] initWithAITranslationDescriptor:descriptor
        configuration:_configuration completion:completion];
}
- (MSIMECloudCandidateRequest *)requestForDescriptor:(NSDictionary *)descriptor completion:(void (^)(NSData *))completion {
    return [[MSIMECloudCandidateRequest alloc] initWithTranslationDescriptor:descriptor
        configuration:_configuration completion:completion];
}
// One session per NiuTrans or custom batch, so the second word onwards reuses the first word's connection instead of paying another TCP and TLS handshake out of the six-second budget. It carries the same hardening as a request's own session, and no delegate: each task reports to its own request.
- (NSURLSession *)transportSession {
    if (_session || !_configuration) return _session;
    NSURLSessionConfiguration *configuration = [_configuration copy];
    configuration.URLCache = nil;
    configuration.HTTPCookieStorage = nil;
    configuration.URLCredentialStorage = nil;
    configuration.HTTPShouldSetCookies = NO;
    configuration.requestCachePolicy = NSURLRequestReloadIgnoringLocalCacheData;
    configuration.timeoutIntervalForRequest = 2.5;
    configuration.timeoutIntervalForResource = 2.5;
    _session = [NSURLSession sessionWithConfiguration:configuration delegate:nil delegateQueue:NSOperationQueue.mainQueue];
    return _session;
}
- (void)start {
    NSAssert(NSThread.isMainThread, @"Translation batch must run on main thread");
    if (_started || !_completion) return;
    _started = YES;
    _deadline = [self currentTime] + 6;
    __weak MSIMECustomTranslationBatch *weakSelf = self;
    _timer = [NSTimer timerWithTimeInterval:6 repeats:NO block:^(NSTimer *timer) {
        (void)timer;
        [weakSelf finish];
    }];
    [NSRunLoop.mainRunLoop addTimer:_timer forMode:NSRunLoopCommonModes];
    [self advance];
}
- (void)advance {
    if (!_completion) return;
    if (_detached || _nextIndex >= _items.count || [self currentTime] >= _deadline) { [self finish]; return; }
    NSDictionary *item = _items[_nextIndex++];
    NSString *text = item[@"text"];
    NSUInteger sequence = _nextIndex;
    __weak MSIMECustomTranslationBatch *weakSelf = self;
    // `answered` is NO for a transport failure (nothing came back, which includes HTTP errors such as 429) and for a body in which the provider reports a failure: Tencent's Response.Error, NiuTrans' errorCode, a DeepLX code other than 200, or a malformed body. Those items stay unanswered, so the caller asks again rather than hiding the gloss for eight minutes over a rate limit or an outage. A request that could not even be built is an answer: asking again builds the same invalid request.
    void (^handle)(NSData *, BOOL) = ^(NSData *body, BOOL answered) {
        MSIMECustomTranslationBatch *strongSelf = weakSelf;
        if (!strongSelf || !strongSelf->_completion || sequence != strongSelf->_nextIndex) return;
        NSMutableArray<NSDictionary *> *results = [NSMutableArray array];
        if (strongSelf->_niuTrans) {
            NSString *translation = body ? [MSIMEClientSession parseNiuTransTranslationResponse:body error:nil] : nil;
            if (translation.length) [results addObject:@{@"text":text, @"translation":translation}];
            if (body && [MSIMEClientSession niuTransTranslationReplyFailed:body]) answered = NO;
        } else if (strongSelf->_tencent) {
            NSArray *translations = body ? [MSIMEClientSession parseTencentTranslationResponse:body
                expectedCount:[item[@"originals"] count] error:nil] : nil;
            // Tencent's parser already tells the two apart: an answer is an array, with NSNull where a text got nothing.
            if (body && !translations) answered = NO;
            for (NSUInteger i = 0; i < translations.count; ++i) {
                id gloss = translations[i];
                if ([gloss isKindOfClass:NSString.class] && [gloss length])
                    [results addObject:@{@"text":item[@"originals"][i], @"translation":gloss}];
            }
        } else if (strongSelf->_ai) {
            NSUInteger limit = [item[@"candidate_limit"] isKindOfClass:NSNumber.class]
                ? [item[@"candidate_limit"] unsignedIntegerValue] : 10;
            NSArray *values = body ? [MSIMEClientSession parseAIResponse:body limit:limit error:nil] : nil;
            for (NSString *value in values) if ([value isKindOfClass:NSString.class] && value.length)
                [results addObject:@{@"text":text, @"translation":value}];
        } else {
            NSString *translation = body ? [MSIMEClientSession parseCustomTranslationResponse:body error:nil] : nil;
            if (translation.length) [results addObject:@{@"text":text, @"translation":translation}];
            if (body && [MSIMEClientSession customTranslationReplyFailed:body]) answered = NO;
        }
        // A Tencent item is a whole language group, answered by its original texts; every other item is one text.
        NSArray<NSString *> *answeredTexts = !answered ? @[] : strongSelf->_tencent ? item[@"originals"] : @[text];
        // The main queue may be busy when the deadline timer becomes due. A response that was already paid for still reaches onReply, so the caller can cache it, but completion keeps to what arrived in time.
        BOOL late = [strongSelf currentTime] >= strongSelf->_deadline;
        if (!late) [strongSelf->_results addObjectsFromArray:results];
        if (strongSelf->_onReply) strongSelf->_onReply([results copy], answeredTexts);
        if (!strongSelf->_completion) return;
        if (late) { [strongSelf finish]; return; }
        strongSelf->_request = nil;
        [strongSelf advance];
    };
    void (^reply)(NSData *) = ^(NSData *body) { handle(body, body != nil); };
    if (_niuTrans) {
        NSMutableDictionary *input = [item[@"request"] mutableCopy];
        input[@"timestamp"] = [NSString stringWithFormat:@"%lld", (long long)([self unixTime] * 1000)];
        NSDictionary *descriptor = [MSIMEClientSession niuTransTranslationHTTPRequest:input error:nil];
        if (!descriptor) { handle(nil, YES); return; }
        if ([self currentTime] >= _deadline) { [self finish]; return; }
        _request = [self niuTransRequestForDescriptor:descriptor completion:reply];
    } else if (_tencent) {
        NSDictionary *descriptor = [MSIMEClientSession tencentTranslationHTTPRequest:@{
            @"config":item[@"config"], @"texts":item[@"texts"],
            @"source_language":item[@"source_language"], @"target_language":item[@"target_language"],
            @"timestamp":@((long long)[self unixTime])} error:nil];
        if (!descriptor) { handle(nil, YES); return; }
        if ([self currentTime] >= _deadline) { [self finish]; return; }
        _request = [self tencentRequestForDescriptor:descriptor completion:reply];
    } else if (_ai) {
        _request = [self AIRequestForDescriptor:item[@"request"] completion:reply];
    } else {
        _request = [self requestForDescriptor:item[@"request"] completion:reply];
    }
    // NiuTrans and custom send one request per word within the 2.5-second budget the shared session enforces. Tencent sends one request per language group, and an AI request needs its own 8-second budget, so both keep a session of their own.
    if (!_tencent && !_ai) [_request startInSession:[self transportSession]];
    else [_request start];
}
- (void)finish {
    void (^completion)(NSArray<NSDictionary *> *) = _completion;
    NSArray<NSDictionary *> *results = [_results copy];
    [self cancel];
    if (completion) completion(results);
}
- (BOOL)detachWithCompletion:(void (^)(void))completion {
    NSAssert(NSThread.isMainThread, @"Translation batch must run on main thread");
    if (!_completion || !_request) { [self cancel]; return NO; }
    void (^ended)(void) = [completion copy];
    _detached = YES;
    _completion = ^(NSArray<NSDictionary *> *results) { (void)results; if (ended) ended(); };
    return YES;
}
- (void)cancel {
    NSAssert(NSThread.isMainThread, @"Translation batch must run on main thread");
    _completion = nil;
    _onReply = nil;
    [_timer invalidate];
    _timer = nil;
    [_request cancel];
    _request = nil;
    [_session invalidateAndCancel];
    _session = nil;
    _items = nil;
    _configuration = nil;
    _results = nil;
}
- (void)dealloc {
    [_timer invalidate];
    [_request cancel];
    [_session invalidateAndCancel];
}
@end
