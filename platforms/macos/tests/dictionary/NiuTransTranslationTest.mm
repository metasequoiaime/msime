#import "../../src/core/CustomTranslationBatch.h"
#import "../../src/cloud/CloudCandidateRequest.h"
#import "MSIMEClientSession.h"
#include <cassert>

static NSData *Response(NSString *text) {
    return [NSJSONSerialization dataWithJSONObject:@{@"tgtText":text} options:0 error:nil];
}
static NSData *ExpectedPayload;
static NSUInteger TransportCalls;
// Requests after this many are held open, so a test can end the batch while one is in flight.
static NSUInteger AnsweredCalls = NSUIntegerMax;
static NSUInteger StoppedCalls;
static NSMutableArray<NSURLSessionTask *> *Tasks;
@interface NiuTransProtocol : NSURLProtocol
@end
@implementation NiuTransProtocol {
    BOOL _held;
}
+ (BOOL)canInitWithRequest:(NSURLRequest *)request { (void)request; return YES; }
+ (NSURLRequest *)canonicalRequestForRequest:(NSURLRequest *)request { return request; }
- (void)startLoading {
    @synchronized(NiuTransProtocol.class) { ++TransportCalls; if (self.task) [Tasks addObject:self.task]; }
    assert([self.request.URL.absoluteString isEqual:@"https://api.niutrans.com/v2/text/translate"]);
    assert([self.request.HTTPMethod isEqual:@"POST"] && !self.request.HTTPShouldHandleCookies);
    assert([[self.request valueForHTTPHeaderField:@"Content-Type"] isEqual:@"application/x-www-form-urlencoded; charset=utf-8"]);
    NSData *body = self.request.HTTPBody;
    if (!body) {
        NSInputStream *stream = self.request.HTTPBodyStream; assert(stream);
        NSMutableData *bytes = [NSMutableData data]; [stream open];
        uint8_t buffer[1024]; NSInteger count;
        while ((count = [stream read:buffer maxLength:sizeof(buffer)]) > 0) [bytes appendBytes:buffer length:(NSUInteger)count];
        [stream close]; assert(count == 0); body = bytes;
    }
    // A batch signs each item with its own timestamp, so the batch cases leave the payload unchecked.
    assert(!ExpectedPayload || [body isEqual:ExpectedPayload]);
    @synchronized(NiuTransProtocol.class) { _held = TransportCalls > AnsweredCalls; }
    if (_held) return;
    NSHTTPURLResponse *response = [[NSHTTPURLResponse alloc] initWithURL:self.request.URL statusCode:200 HTTPVersion:@"HTTP/1.1" headerFields:@{}];
    [self.client URLProtocol:self didReceiveResponse:response cacheStoragePolicy:NSURLCacheStorageNotAllowed];
    [self.client URLProtocol:self didLoadData:Response(@"synthetic gloss")];
    [self.client URLProtocolDidFinishLoading:self];
}
// Loading also stops after a finished reply; only a held request stopping means it was cancelled.
- (void)stopLoading { @synchronized(NiuTransProtocol.class) { if (_held) ++StoppedCalls; } }
@end

@interface NiuTransFakeRequest : MSIMECloudCandidateRequest
@property(copy) void (^reply)(NSData *);
@property BOOL cancelled;
@end
@implementation NiuTransFakeRequest
- (void)start {}
- (void)startInSession:(NSURLSession *)session { assert(session); }
- (void)cancel { self.cancelled = YES; }
@end
@interface NiuTransFakeBatch : MSIMECustomTranslationBatch
@property NSTimeInterval now;
@property NSTimeInterval wall;
@property NSMutableArray<NiuTransFakeRequest *> *requests;
@property NSMutableArray<NSDictionary *> *descriptors;
@end
@implementation NiuTransFakeBatch
- (NSTimeInterval)currentTime { return self.now; }
- (NSTimeInterval)unixTime { return self.wall; }
- (MSIMECloudCandidateRequest *)niuTransRequestForDescriptor:(NSDictionary *)descriptor completion:(void (^)(NSData *))completion {
    if (!self.requests) { self.requests = [NSMutableArray array]; self.descriptors = [NSMutableArray array]; }
    NiuTransFakeRequest *request = [NiuTransFakeRequest new]; request.reply = completion;
    [self.requests addObject:request]; [self.descriptors addObject:descriptor]; return request;
}
@end
@interface NiuTransClockBatch : MSIMECustomTranslationBatch
@property NSTimeInterval now;
@end
@implementation NiuTransClockBatch
- (NSTimeInterval)currentTime { return self.now; }
@end

static void Spin(BOOL (^done)(void)) {
    NSDate *deadline = [NSDate dateWithTimeIntervalSinceNow:3];
    while (!done() && deadline.timeIntervalSinceNow > 0) [NSRunLoop.mainRunLoop runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.005]];
}
static NSUInteger Calls() { @synchronized(NiuTransProtocol.class) { return TransportCalls; } }
static NSUInteger Stops() { @synchronized(NiuTransProtocol.class) { return StoppedCalls; } }

// Real transport through the batch: every word's task comes from the one session the batch owns, and that session goes away with the batch whichever way it ends.
static void TestBatchReusesOneSession(NSDictionary *config, NSURLSessionConfiguration *configuration) {
    ExpectedPayload = nil;
    NSArray *items = @[@{@"text":@"一", @"key":@"一", @"source_language":@"zh", @"target_language":@"en"},
        @{@"text":@"二", @"key":@"二", @"source_language":@"zh", @"target_language":@"en"},
        @{@"text":@"三", @"key":@"三", @"source_language":@"zh", @"target_language":@"en"}];
    Tasks = [NSMutableArray array]; TransportCalls = 0; StoppedCalls = 0; AnsweredCalls = NSUIntegerMax;
    __block NSUInteger calls = 0;
    NiuTransClockBatch *batch = [[NiuTransClockBatch alloc] initWithNiuTransItems:items config:config configuration:configuration completion:^(NSArray *results) {
        assert(++calls == 1 && results.count == 3);
    }];
    [batch start];
    NSURLSession *session = [batch valueForKey:@"session"];
    assert(session && !session.delegate);
    NSURLSessionConfiguration *effective = session.configuration;
    assert(!effective.URLCache && !effective.URLCredentialStorage && !effective.HTTPCookieStorage && !effective.HTTPShouldSetCookies);
    assert(effective.timeoutIntervalForRequest == 2.5 && effective.timeoutIntervalForResource == 2.5);
    // The session has no delegate, so three glosses arriving means each request's own task delegate received its task's events.
    Spin(^BOOL { return calls > 0; });
    assert(calls == 1 && Calls() == 3 && ![batch valueForKey:@"session"]);
    // Task identifiers are unique only within a session and start again in a fresh one, so three different identifiers mean one session carried all three words.
    assert([[NSSet setWithArray:[Tasks valueForKey:@"taskIdentifier"]] count] == 3);

    for (NSString *ending in @[@"cancel", @"deadline"]) {
        Tasks = [NSMutableArray array]; TransportCalls = 0; StoppedCalls = 0; AnsweredCalls = 1;
        __block NSUInteger finished = 0;
        BOOL deadline = [ending isEqual:@"deadline"];
        NiuTransClockBatch *held = [[NiuTransClockBatch alloc] initWithNiuTransItems:items config:config configuration:configuration completion:^(NSArray *results) {
            assert(deadline && ++finished == 1 && results.count == 1);
        }];
        NSMutableArray *answered = [NSMutableArray array];
        held.onReply = ^(NSArray *results, NSArray *texts) { (void)results; [answered addObjectsFromArray:texts]; };
        held.now = 10; [held start];
        Spin(^BOOL { return Calls() == 2; });
        assert(Calls() == 2 && [answered isEqual:@[@"一"]]);
        if (deadline) { held.now = 16; [(NSTimer *)[held valueForKey:@"timer"] fire]; }
        else [held cancel];
        assert(![held valueForKey:@"session"] && finished == (deadline ? 1u : 0u));
        // The held task is cancelled with the session, and nothing further is sent or reported.
        Spin(^BOOL { return Stops() == 1; });
        [NSRunLoop.mainRunLoop runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.05]];
        assert(Stops() == 1 && Calls() == 2 && finished == (deadline ? 1u : 0u) && [answered isEqual:@[@"一"]]);
    }
    Tasks = nil; AnsweredCalls = NSUIntegerMax;
}

int main() {
    @autoreleasepool {
        NSMutableDictionary *config = [@{@"enabled":@YES, @"app_id":@"app-id", @"apikey":@"api-key"} mutableCopy];
        NSDictionary *input = @{@"config":config, @"text":@"hello", @"source_language":@"en", @"target_language":@"zh", @"timestamp":@"1704067200000"};
        NSDictionary *descriptor = [MSIMEClientSession niuTransTranslationHTTPRequest:input error:nil];
        assert([descriptor[@"body_utf8"] containsString:@"authStr=6da3515e010ef871b66e4e31ff5ba580"]);
        assert(![descriptor[@"body_utf8"] containsString:@"api-key"]);
        assert([[MSIMEClientSession parseNiuTransTranslationResponse:Response(@" hello\nworld ") error:nil] isEqual:@"hello world"]);
        assert(![MSIMEClientSession parseNiuTransTranslationResponse:[@"{\"errorCode\":\"401\",\"tgtText\":\"invalid\"}" dataUsingEncoding:NSUTF8StringEncoding] error:nil]);
        assert(![MSIMEClientSession parseNiuTransTranslationResponse:[NSMutableData dataWithLength:1048577] error:nil]);
        ExpectedPayload = [descriptor[@"body_utf8"] dataUsingEncoding:NSUTF8StringEncoding];
        NSURLSessionConfiguration *configuration = NSURLSessionConfiguration.ephemeralSessionConfiguration;
        configuration.protocolClasses = @[NiuTransProtocol.class];
        __block BOOL done = NO;
        MSIMECloudCandidateRequest *transport = [[MSIMECloudCandidateRequest alloc] initWithNiuTransDescriptor:descriptor configuration:configuration completion:^(NSData *body) {
            assert([body isEqual:Response(@"synthetic gloss")]); done = YES;
        }];
        assert([(NSURLRequest *)[transport valueForKey:@"translationRequest"] timeoutInterval] == 2.5);
        [transport start];
        NSURLSessionConfiguration *effective = [(NSURLSession *)[transport valueForKey:@"session"] configuration];
        assert(!effective.URLCache && !effective.URLCredentialStorage && !effective.HTTPCookieStorage);
        NSDate *deadline = [NSDate dateWithTimeIntervalSinceNow:3];
        while (!done && deadline.timeIntervalSinceNow > 0) [NSRunLoop.mainRunLoop runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.005]];
        assert(done && TransportCalls == 1 && ![transport valueForKey:@"translationRequest"]);
        __block BOOL refused = NO;
        MSIMECloudCandidateRequest *redirected = [[MSIMECloudCandidateRequest alloc] initWithNiuTransDescriptor:descriptor configuration:configuration completion:^(NSData *body) { assert(!body); refused = YES; }];
        NSURLSession *unusedSession = [NSURLSession sessionWithConfiguration:configuration];
        NSURL *original = [NSURL URLWithString:descriptor[@"url"]];
        NSURLSessionDataTask *unusedTask = [unusedSession dataTaskWithURL:original];
        NSHTTPURLResponse *redirectResponse = [[NSHTTPURLResponse alloc] initWithURL:original statusCode:302 HTTPVersion:@"HTTP/1.1" headerFields:@{}];
        [redirected URLSession:unusedSession task:unusedTask willPerformHTTPRedirection:redirectResponse newRequest:[NSURLRequest requestWithURL:[NSURL URLWithString:@"https://untrusted.invalid/"]] completionHandler:^(NSURLRequest *next) { assert(!next); }];
        assert(refused && ![redirected valueForKey:@"translationRequest"]);
        [unusedSession invalidateAndCancel];
        for (NSDictionary *change in @[@{@"url":@"https://untrusted.invalid/"}, @{@"method":@"GET"},
            @{@"timeout_ms":@9000}, @{@"max_response_bytes":@99999999}, @{@"headers":@{@"Content-Type":@"application/json"}},
            @{@"body_utf8":@42}, @{@"body_utf8":[@"x" stringByPaddingToLength:16385 withString:@"x" startingAtIndex:0]}]) {
            NSMutableDictionary *invalid = [descriptor mutableCopy]; [invalid addEntriesFromDictionary:change];
            __block BOOL rejected = NO;
            MSIMECloudCandidateRequest *request = [[MSIMECloudCandidateRequest alloc] initWithNiuTransDescriptor:invalid configuration:configuration completion:^(NSData *body) { assert(!body); rejected = YES; }];
            [request start]; assert(rejected && TransportCalls == 1 && ![request valueForKey:@"session"]);
        }
        NSArray *items = @[@{@"text":@"HELLO", @"key":@"hello", @"source_language":@"en", @"target_language":@"zh"},
            @{@"text":@"世界", @"key":@"世界", @"source_language":@"zh", @"target_language":@"fr"}];
        __block NSUInteger calls = 0;
        NiuTransFakeBatch *batch = [[NiuTransFakeBatch alloc] initWithNiuTransItems:items config:config configuration:configuration completion:^(NSArray *results) {
            assert(++calls == 1 && results.count == 1 && [results[0][@"text"] isEqual:@"HELLO"]);
        }];
        NSMutableArray *answered = [NSMutableArray array];
        batch.onReply = ^(NSArray *results, NSArray *texts) { (void)results; [answered addObjectsFromArray:texts]; };
        config[@"app_id"] = @"mutated"; config[@"apikey"] = @"mutated";
        batch.wall = 1704067200; batch.now = 10; [batch start];
        assert(batch.requests.count == 1 && [batch.descriptors[0][@"body_utf8"] isEqual:descriptor[@"body_utf8"]]);
        batch.wall += 1; batch.requests[0].reply(Response(@"synthetic gloss"));
        assert(batch.requests.count == 2 && [batch.descriptors[1][@"body_utf8"] containsString:@"timestamp=1704067201000"]);
        batch.requests[0].reply(Response(@"duplicate")); assert(batch.requests.count == 2);
        batch.now = 16; [(NSTimer *)[batch valueForKey:@"timer"] fire];
        assert(calls == 1 && batch.requests[1].cancelled && ![batch valueForKey:@"items"]);
        batch.requests[1].reply(Response(@"late")); assert(calls == 1);
        assert([answered isEqual:@[@"HELLO"]]); // The item the deadline cut off stays unanswered.
        NiuTransFakeBatch *cancelled = [[NiuTransFakeBatch alloc] initWithNiuTransItems:items config:config configuration:configuration completion:^(NSArray *results) { (void)results; assert(false); }];
        cancelled.wall = 1704067200; [cancelled start]; [cancelled cancel];
        assert(cancelled.requests[0].cancelled && ![cancelled valueForKey:@"items"]);
        cancelled.requests[0].reply(Response(@"late"));
        __block BOOL invalidDone = NO;
        NiuTransFakeBatch *invalid = [[NiuTransFakeBatch alloc] initWithNiuTransItems:items config:@{@"enabled":@YES, @"app_id":@"", @"apikey":@""} configuration:configuration completion:^(NSArray *results) { assert(!results.count); invalidDone = YES; }];
        NSMutableArray *unsendable = [NSMutableArray array];
        invalid.onReply = ^(NSArray *results, NSArray *texts) { assert(!results.count); [unsendable addObjectsFromArray:texts]; };
        [invalid start]; assert(invalidDone && !invalid.requests.count);
        // A request that cannot be built will not improve by asking again, so it counts as answered.
        assert(([unsendable isEqual:@[@"HELLO", @"世界"]]));
        // 服务报错、缺少译文字段或字段类型错误都不能记成空回答；合法空译文仍然计入已回答。
        for (NSString *body in @[@"{\"errorCode\":\"13001\",\"errorMsg\":\"rate limited\"}",
            @"{}", @"{\"message\":\"temporarily unavailable\"}", @"{\"tgtText\":null}",
            @"{\"tgtText\":42}", @"{\"tgtText\":false}", @"{\"tgtText\":[]}",
            @"{\"tgtText\":{\"text\":\"synthetic\"}}"]) {
            NiuTransFakeBatch *failing = [[NiuTransFakeBatch alloc] initWithNiuTransItems:items config:@{@"enabled":@YES, @"app_id":@"synthetic-app", @"apikey":@"synthetic-key"}
                configuration:configuration completion:^(NSArray *results) { assert(!results.count); }];
            NSMutableArray *failingAnswered = [NSMutableArray array];
            failing.onReply = ^(NSArray *results, NSArray *texts) { assert(!results.count); [failingAnswered addObject:texts]; };
            failing.wall = 1704067200; failing.now = 10; [failing start];
            failing.requests[0].reply([body dataUsingEncoding:NSUTF8StringEncoding]);
            failing.requests[1].reply([@"{\"tgtText\":\"\"}" dataUsingEncoding:NSUTF8StringEncoding]);
            assert(([failingAnswered isEqual:@[@[], @[@"世界"]]]));
        }
        TestBatchReusesOneSession(config, configuration);
    }
}
