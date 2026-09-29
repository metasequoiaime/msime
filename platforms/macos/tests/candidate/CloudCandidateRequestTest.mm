#import "../../src/cloud/CloudCandidateRequest.h"
#import "MSIMEClientSession.h"
#include <cassert>
#include "msime_client.h"

static NSInteger ResponseStatus = 200;
static NSUInteger ResponseBytes = 8;
static BOOL FailWithTimeout = NO;
static BOOL TranslationMode = NO;
static BOOL TencentMode = NO;
static NSData *TencentPayload;
static NSString *TencentAuthorization;
@interface SyntheticCloudProtocol : NSURLProtocol
@end
@implementation SyntheticCloudProtocol
+ (BOOL)canInitWithRequest:(NSURLRequest *)request { (void)request; return YES; }
+ (NSURLRequest *)canonicalRequestForRequest:(NSURLRequest *)request { return request; }
- (void)startLoading {
    if (TencentMode) {
        assert([self.request.URL.absoluteString isEqual:@"https://tmt.tencentcloudapi.com"]);
        assert([self.request.HTTPMethod isEqual:@"POST"]);
        assert([[self.request valueForHTTPHeaderField:@"Authorization"] isEqual:TencentAuthorization]);
        assert([[self.request valueForHTTPHeaderField:@"Content-Type"] isEqual:@"application/json; charset=utf-8"]);
        assert([[self.request valueForHTTPHeaderField:@"X-TC-Action"] isEqual:@"TextTranslateBatch"]);
        NSData *body = self.request.HTTPBody;
        if (!body) {
            NSInputStream *stream = self.request.HTTPBodyStream;
            assert(stream);
            NSMutableData *bytes = [NSMutableData data];
            [stream open];
            uint8_t buffer[1024]; NSInteger count;
            while ((count = [stream read:buffer maxLength:sizeof(buffer)]) > 0) [bytes appendBytes:buffer length:(NSUInteger)count];
            [stream close]; assert(count == 0); body = bytes;
        }
        assert([body isEqual:TencentPayload]);
    }
    if (TranslationMode) {
        assert([self.request.HTTPMethod isEqual:@"POST"]);
        assert([[self.request valueForHTTPHeaderField:@"Authorization"] isEqual:@"Bearer synthetic"]);
        assert([[self.request valueForHTTPHeaderField:@"Content-Type"] isEqual:@"application/json"]);
    }
    if (FailWithTimeout) {
        [self.client URLProtocol:self didFailWithError:[NSError errorWithDomain:NSURLErrorDomain code:NSURLErrorTimedOut userInfo:nil]];
        return;
    }
    NSHTTPURLResponse *response = [[NSHTTPURLResponse alloc] initWithURL:self.request.URL statusCode:ResponseStatus HTTPVersion:@"HTTP/1.1" headerFields:@{}];
    [self.client URLProtocol:self didReceiveResponse:response cacheStoragePolicy:NSURLCacheStorageNotAllowed];
    [self.client URLProtocol:self didLoadData:[NSMutableData dataWithLength:ResponseBytes / 2]];
    [self.client URLProtocol:self didLoadData:[NSMutableData dataWithLength:ResponseBytes - ResponseBytes / 2]];
    [self.client URLProtocolDidFinishLoading:self];
}
- (void)stopLoading {}
@end

static void Wait(BOOL (^done)(void)) {
    NSDate *deadline = [NSDate dateWithTimeIntervalSinceNow:3];
    while (!done() && deadline.timeIntervalSinceNow > 0)
        [NSRunLoop.mainRunLoop runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.005]];
    assert(done());
}

static void TestTranslationTransport() {
    NSDictionary *descriptor = @{@"url":@"https://translation.invalid/api", @"method":@"POST", @"headers":@{@"Content-Type":@"application/json", @"Authorization":@"Bearer synthetic"},
        @"body":@{@"text":@"hello", @"source_lang":@"EN", @"target_lang":@"ZH"}, @"timeout_ms":@2500, @"max_response_bytes":@1048576};
    TranslationMode = YES;
    for (NSNumber *bytes in @[@8, @1048576, @1048577]) {
        for (NSNumber *status in @[@200, @201, @503]) {
            ResponseBytes = bytes.unsignedIntegerValue;
            ResponseStatus = status.integerValue;
            __block BOOL done = NO;
            NSURLSessionConfiguration *configuration = NSURLSessionConfiguration.ephemeralSessionConfiguration;
            configuration.protocolClasses = @[SyntheticCloudProtocol.class];
            MSIMECloudCandidateRequest *request = [[MSIMECloudCandidateRequest alloc] initWithTranslationDescriptor:descriptor configuration:configuration completion:^(NSData *body) {
                assert(NSThread.isMainThread && !done);
                assert((body != nil) == (ResponseStatus < 300 && ResponseBytes <= 1048576));
                done = YES;
            }];
            NSURLRequest *prepared = [request valueForKey:@"translationRequest"];
            assert(prepared && [[NSJSONSerialization JSONObjectWithData:prepared.HTTPBody options:0 error:nil] isEqual:descriptor[@"body"]]);
            assert(prepared.timeoutInterval == 2.5);
            [request start];
            NSURLSessionConfiguration *effective = [(NSURLSession *)[request valueForKey:@"session"] configuration];
            assert(effective.timeoutIntervalForResource == 2.5 && !effective.URLCredentialStorage && !effective.HTTPCookieStorage && !effective.URLCache);
            Wait(^BOOL { return done; });
            assert(![request valueForKey:@"translationRequest"]);
        }
    }
    for (NSDictionary *override in @[@{@"url":@"file:///synthetic"}, @{@"url":@"http://translation.invalid/api"}, @{@"url":@"https://user:pass@translation.invalid/"},
        @{@"headers":@{@"Content-Type":@"application/json", @"Authorization":@"synthetic\r\nX: bad"}}, @{@"timeout_ms":@9999}, @{@"method":@"GET"}]) {
        NSMutableDictionary *invalid = [descriptor mutableCopy]; [invalid addEntriesFromDictionary:override];
        __block BOOL rejected = NO;
        MSIMECloudCandidateRequest *request = [[MSIMECloudCandidateRequest alloc] initWithTranslationDescriptor:invalid configuration:NSURLSessionConfiguration.ephemeralSessionConfiguration completion:^(NSData *body) { assert(!body); rejected = YES; }];
        [request start];
        assert(rejected && ![request valueForKey:@"session"]);
    }
    NSMutableDictionary *loopback = [descriptor mutableCopy];
    loopback[@"url"] = @"http://127.0.0.1:8765/translate";
    MSIMECloudCandidateRequest *local = [[MSIMECloudCandidateRequest alloc]
        initWithTranslationDescriptor:loopback
        configuration:NSURLSessionConfiguration.ephemeralSessionConfiguration
        completion:^(NSData *body) { assert(!body); }];
    assert([[(NSURLRequest *)[local valueForKey:@"translationRequest"] URL].host isEqual:@"127.0.0.1"]);
    NSURLSessionConfiguration *configuration = NSURLSessionConfiguration.ephemeralSessionConfiguration;
    configuration.protocolClasses = @[SyntheticCloudProtocol.class];
    MSIMECloudCandidateRequest *cancelled = [[MSIMECloudCandidateRequest alloc] initWithTranslationDescriptor:descriptor configuration:configuration completion:^(NSData *body) { (void)body; assert(false && "cancelled translation must not complete"); }];
    [cancelled start];
    [cancelled cancel];
    assert(![cancelled valueForKey:@"translationRequest"]);
    [NSRunLoop.mainRunLoop runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.05]];
    TranslationMode = NO;
}

static void TestAIRejectsPlainHTTP() {
    NSDictionary *descriptor = @{@"url":@"http://ai.invalid/chat", @"method":@"POST",
        @"headers":@{@"Content-Type":@"application/json", @"Authorization":@"Bearer synthetic"},
        @"body":@{@"model":@"synthetic"}, @"timeout_ms":@8000, @"connect_timeout_ms":@2500,
        @"max_response_bytes":@1048576};
    MSIMECloudCandidateRequest *request = [[MSIMECloudCandidateRequest alloc]
        initWithAITranslationDescriptor:descriptor
        configuration:NSURLSessionConfiguration.ephemeralSessionConfiguration
        completion:^(NSData *body) { assert(!body); }];
    assert(![request valueForKey:@"translationRequest"]);
    [request start];
    assert(![request valueForKey:@"session"]);
    descriptor = [descriptor mutableCopy];
    descriptor[@"url"] = @"http://localhost:8765/chat";
    request = [[MSIMECloudCandidateRequest alloc]
        initWithAITranslationDescriptor:descriptor
        configuration:NSURLSessionConfiguration.ephemeralSessionConfiguration
        completion:^(NSData *body) { assert(!body); }];
    assert([[(NSURLRequest *)[request valueForKey:@"translationRequest"] URL].host isEqual:@"localhost"]);
}

static void TestTencentTransport() {
    NSError *error = nil;
    NSDictionary *descriptor = [MSIMEClientSession tencentTranslationHTTPRequest:@{
        @"config":@{@"enabled":@YES, @"secret_id":@"AKIDsynthetic", @"secret_key":@"synthetic", @"region":@""},
        @"texts":@[@"测试", @"line quote\"😀"], @"source_language":@"zh", @"target_language":@"en", @"timestamp":@1704067200} error:&error];
    assert(descriptor && !error);
    TencentMode = YES;
    TencentPayload = [descriptor[@"body_utf8"] dataUsingEncoding:NSUTF8StringEncoding];
    TencentAuthorization = descriptor[@"headers"][@"Authorization"];
    NSURLSessionConfiguration *configuration = NSURLSessionConfiguration.ephemeralSessionConfiguration;
    configuration.protocolClasses = @[SyntheticCloudProtocol.class];
    for (NSNumber *bytes in @[@8, @1048576, @1048577]) {
        for (NSNumber *status in @[@200, @201, @503]) {
            ResponseBytes = bytes.unsignedIntegerValue; ResponseStatus = status.integerValue;
            __block BOOL done = NO;
            MSIMECloudCandidateRequest *request = [[MSIMECloudCandidateRequest alloc] initWithTencentDescriptor:descriptor configuration:configuration completion:^(NSData *body) {
                assert(NSThread.isMainThread && !done);
                assert((body != nil) == (ResponseStatus < 300 && ResponseBytes <= 1048576)); done = YES;
            }];
            NSURLRequest *prepared = [request valueForKey:@"translationRequest"];
            assert([prepared.HTTPBody isEqual:TencentPayload] && prepared.timeoutInterval == 2.5 && !prepared.HTTPShouldHandleCookies);
            [request start];
            NSURLSessionConfiguration *effective = [(NSURLSession *)[request valueForKey:@"session"] configuration];
            assert(effective.timeoutIntervalForResource == 2.5 && !effective.URLCache && !effective.HTTPCookieStorage && !effective.URLCredentialStorage);
            Wait(^BOOL { return done; });
            assert(![request valueForKey:@"translationRequest"]);
        }
    }
    for (NSDictionary *override in @[@{@"url":@"http://tmt.tencentcloudapi.com"}, @{@"url":@"https://elsewhere.invalid"},
        @{@"url":@"https://tmt.tencentcloudapi.com/extra"}, @{@"method":@"GET"}, @{@"body_utf8":@""},
        @{@"body_utf8":[@"x" stringByPaddingToLength:16385 withString:@"x" startingAtIndex:0]}, @{@"timeout_ms":@5000}]) {
        NSMutableDictionary *invalid = [descriptor mutableCopy]; [invalid addEntriesFromDictionary:override];
        __block BOOL done = NO;
        MSIMECloudCandidateRequest *request = [[MSIMECloudCandidateRequest alloc] initWithTencentDescriptor:invalid configuration:configuration completion:^(NSData *body) { assert(!body); done = YES; }];
        [request start]; assert(done && ![request valueForKey:@"session"]);
    }
    for (NSDictionary *override in @[@{@"Authorization":@"Bearer wrong"}, @{@"X-TC-Region":@"region\r\nInjected"},
        @{@"X-TC-Timestamp":@"not-a-time"}, @{@"Host":@"elsewhere.invalid"}, @{@"Extra":@"not-allowed"}]) {
        NSMutableDictionary *invalid = [descriptor mutableCopy], *headers = [descriptor[@"headers"] mutableCopy];
        [headers addEntriesFromDictionary:override]; invalid[@"headers"] = headers;
        __block BOOL done = NO;
        MSIMECloudCandidateRequest *request = [[MSIMECloudCandidateRequest alloc] initWithTencentDescriptor:invalid configuration:configuration completion:^(NSData *body) { assert(!body); done = YES; }];
        [request start]; assert(done && ![request valueForKey:@"session"]);
    }
    __block NSUInteger completions = 0;
    MSIMECloudCandidateRequest *redirected = [[MSIMECloudCandidateRequest alloc] initWithTencentDescriptor:descriptor configuration:configuration completion:^(NSData *body) { assert(!body); ++completions; }];
    [redirected start];
    NSURLSession *redirectSession = [redirected valueForKey:@"session"];
    NSURL *originalURL = [NSURL URLWithString:@"https://tmt.tencentcloudapi.com"];
    NSURLSessionDataTask *redirectTask = [redirectSession dataTaskWithURL:originalURL];
    NSHTTPURLResponse *redirectResponse = [[NSHTTPURLResponse alloc] initWithURL:originalURL statusCode:302 HTTPVersion:@"HTTP/1.1" headerFields:@{@"Location":@"https://elsewhere.invalid"}];
    [redirected URLSession:redirectSession task:redirectTask willPerformHTTPRedirection:redirectResponse newRequest:[NSURLRequest requestWithURL:[NSURL URLWithString:@"https://elsewhere.invalid"]]
        completionHandler:^(NSURLRequest *next) { assert(!next); }];
    [redirected start]; assert(completions == 1 && ![redirected valueForKey:@"translationRequest"]);
    FailWithTimeout = YES;
    __block BOOL timedOut = NO;
    MSIMECloudCandidateRequest *timeout = [[MSIMECloudCandidateRequest alloc] initWithTencentDescriptor:descriptor configuration:configuration completion:^(NSData *body) { assert(!body); timedOut = YES; }];
    [timeout start]; Wait(^BOOL { return timedOut; }); FailWithTimeout = NO;
    MSIMECloudCandidateRequest *cancelled = [[MSIMECloudCandidateRequest alloc] initWithTencentDescriptor:descriptor configuration:configuration completion:^(NSData *body) { (void)body; assert(false); }];
    [cancelled start]; [cancelled cancel];
    assert(![cancelled valueForKey:@"translationRequest"]);
    [NSRunLoop.mainRunLoop runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.05]];
    TencentMode = NO; TencentPayload = nil; TencentAuthorization = nil;
}
int main() {
    @autoreleasepool {
        TestTranslationTransport();
        TestAIRejectsPlainHTTP();
        TestTencentTransport();
        for (NSNumber *bytes in @[@8, @262144, @262145]) {
            for (NSNumber *status in @[@200, @503]) {
                ResponseBytes = bytes.unsignedIntegerValue;
                ResponseStatus = status.integerValue;
                __block BOOL done = NO;
                __block NSUInteger calls = 0;
                NSURLSessionConfiguration *configuration = NSURLSessionConfiguration.ephemeralSessionConfiguration;
                configuration.protocolClasses = @[SyntheticCloudProtocol.class];
                MSIMECloudCandidateRequest *request = [[MSIMECloudCandidateRequest alloc] initWithURL:[NSURL URLWithString:@"https://inputtools.google.com/synthetic"] configuration:configuration completion:^(NSData *body) {
                    assert(NSThread.isMainThread);
                    ++calls;
                    assert((body != nil) == (ResponseStatus == 200 && ResponseBytes <= 262144));
                    if (body) assert(body.length == ResponseBytes);
                    done = YES;
                }];
                [request start];
                NSURLSessionConfiguration *effective = [(NSURLSession *)[request valueForKey:@"session"] configuration];
                assert(!effective.URLCache && !effective.HTTPCookieStorage && !effective.URLCredentialStorage);
                // The deadline is the shared one, not this host's own: a cloud reply that arrives
                // inside the budget the reference allows is a candidate the user is meant to see.
                const NSTimeInterval budget = MSIME_CLOUD_REQUEST_TIMEOUT_MS / 1000.0;
                assert(!effective.HTTPShouldSetCookies && effective.timeoutIntervalForResource == budget &&
                       effective.timeoutIntervalForRequest == budget);
                Wait(^BOOL { return done; });
                assert(calls == 1);
                [request cancel];
            }
        }
        __block NSUInteger calls = 0;
        FailWithTimeout = YES;
        NSURLSessionConfiguration *timeoutConfiguration = NSURLSessionConfiguration.ephemeralSessionConfiguration;
        timeoutConfiguration.protocolClasses = @[SyntheticCloudProtocol.class];
        __block BOOL timedOut = NO;
        MSIMECloudCandidateRequest *timeout = [[MSIMECloudCandidateRequest alloc] initWithURL:[NSURL URLWithString:@"https://inputtools.google.com/synthetic"] configuration:timeoutConfiguration completion:^(NSData *body) { assert(!body); timedOut = YES; }];
        [timeout start];
        Wait(^BOOL { return timedOut; });
        FailWithTimeout = NO;
        MSIMECloudCandidateRequest *cancelled = [[MSIMECloudCandidateRequest alloc] initWithURL:[NSURL URLWithString:@"https://inputtools.google.com/synthetic"] configuration:NSURLSessionConfiguration.ephemeralSessionConfiguration completion:^(NSData *body) { (void)body; ++calls; }];
        [cancelled cancel];
        [cancelled start];
        NSURL *syntheticURL = [NSURL URLWithString:@"https://invalid.example/"];
        NSURLSession *unusedSession = [NSURLSession sessionWithConfiguration:NSURLSessionConfiguration.ephemeralSessionConfiguration];
        NSURLSessionDataTask *unusedTask = [unusedSession dataTaskWithURL:syntheticURL]; // Never resumed.
        [cancelled URLSession:unusedSession task:unusedTask didCompleteWithError:nil];
        assert(calls == 0);
        for (NSString *url in @[@"http://inputtools.google.com/", @"https://invalid.example/", @"https://user@inputtools.google.com/"]) {
            MSIMECloudCandidateRequest *invalid = [[MSIMECloudCandidateRequest alloc] initWithURL:[NSURL URLWithString:url] configuration:NSURLSessionConfiguration.ephemeralSessionConfiguration completion:^(NSData *body) { assert(!body); ++calls; }];
            [invalid start];
        }
        assert(calls == 3);
        MSIMECloudCandidateRequest *redirect = [[MSIMECloudCandidateRequest alloc] initWithURL:nil configuration:NSURLSessionConfiguration.ephemeralSessionConfiguration completion:^(NSData *body) { assert(!body); ++calls; }];
        NSHTTPURLResponse *redirectResponse = [[NSHTTPURLResponse alloc] initWithURL:syntheticURL statusCode:302 HTTPVersion:@"HTTP/1.1" headerFields:@{}];
        [redirect URLSession:unusedSession task:unusedTask willPerformHTTPRedirection:redirectResponse newRequest:[NSURLRequest requestWithURL:syntheticURL] completionHandler:^(NSURLRequest *next) { assert(!next); }];
        assert(calls == 4);
        [unusedSession invalidateAndCancel];
    }
}
