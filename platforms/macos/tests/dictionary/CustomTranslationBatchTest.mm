#import "../../src/core/CustomTranslationBatch.h"
#import "../../src/cloud/CloudCandidateRequest.h"
#include <cassert>

@interface MSIMECustomTranslationBatch (TestSeams)
- (NSTimeInterval)currentTime;
- (MSIMECloudCandidateRequest *)requestForDescriptor:(NSDictionary *)descriptor completion:(void (^)(NSData *))completion;
- (MSIMECloudCandidateRequest *)AIRequestForDescriptor:(NSDictionary *)descriptor completion:(void (^)(NSData *))completion;
@end

@interface SyntheticTranslationRequest : MSIMECloudCandidateRequest
@property(nonatomic, copy) void (^reply)(NSData *);
@property(nonatomic) BOOL started;
@property(nonatomic) BOOL cancelled;
@end
@implementation SyntheticTranslationRequest
- (void)start { assert(!_started); _started = YES; }
- (void)startInSession:(NSURLSession *)session { assert(session); [self start]; }
// Keep the reply deliberately, to simulate an already-enqueued late callback.
- (void)cancel { _cancelled = YES; }
@end

@interface SyntheticTranslationBatch : MSIMECustomTranslationBatch
@property(nonatomic) NSTimeInterval now;
@property(nonatomic) NSTimeInterval wallTime;
@property(nonatomic, strong) NSMutableArray<SyntheticTranslationRequest *> *requests;
@property(nonatomic, strong) NSMutableArray<NSDictionary *> *descriptors;
@end
@implementation SyntheticTranslationBatch
- (NSTimeInterval)currentTime { return _now; }
- (NSTimeInterval)unixTime { return _wallTime; }
- (MSIMECloudCandidateRequest *)tencentRequestForDescriptor:(NSDictionary *)descriptor completion:(void (^)(NSData *))completion {
    return [self requestForDescriptor:descriptor completion:completion];
}
- (MSIMECloudCandidateRequest *)AIRequestForDescriptor:(NSDictionary *)descriptor completion:(void (^)(NSData *))completion {
    return [self requestForDescriptor:descriptor completion:completion];
}
- (MSIMECloudCandidateRequest *)requestForDescriptor:(NSDictionary *)descriptor completion:(void (^)(NSData *))completion {
    if (!_requests) _requests = [NSMutableArray array];
    if (!_descriptors) _descriptors = [NSMutableArray array];
    SyntheticTranslationRequest *request = [SyntheticTranslationRequest new];
    request.reply = completion;
    [_requests addObject:request];
    [_descriptors addObject:descriptor];
    return request;
}
@end

static NSData *Response(NSString *translation) {
    return [NSJSONSerialization dataWithJSONObject:@{@"translation":translation} options:0 error:nil];
}
static NSDictionary *Item(NSString *text) {
    return @{@"text":text, @"request":@{@"url":@"https://translation.invalid/api", @"method":@"POST",
        @"headers":@{@"Content-Type":@"application/json"}, @"body":@{@"text":text, @"source_lang":@"EN", @"target_lang":@"ZH"},
        @"timeout_ms":@2500, @"max_response_bytes":@1048576}};
}
static SyntheticTranslationBatch *Batch(NSArray *items, void (^completion)(NSArray *)) {
    return [[SyntheticTranslationBatch alloc] initWithItems:items configuration:NSURLSessionConfiguration.ephemeralSessionConfiguration completion:completion];
}
static void AssertReleased(MSIMECustomTranslationBatch *batch) {
    for (NSString *key in @[@"items", @"request", @"session", @"timer", @"configuration", @"results", @"completion", @"onReply"])
        assert(![batch valueForKey:key]);
}
static void TestSequentialResults() {
    __block NSUInteger calls = 0;
    SyntheticTranslationBatch *batch = Batch(@[Item(@"one"), Item(@"two"), Item(@"three"), Item(@"four")], ^(NSArray *results) {
        assert(NSThread.isMainThread && ++calls == 1);
        assert(([results isEqual:@[@{@"text":@"one", @"translation":@"一"}, @{@"text":@"four", @"translation":@"四"}]]));
    });
    NSMutableArray *replies = [NSMutableArray array];
    batch.onReply = ^(NSArray *results, NSArray *answered) { assert(calls == 0); [replies addObject:@[results, answered]]; };
    batch.now = 100;
    [batch start]; [batch start];
    assert(batch.requests.count == 1 && batch.requests[0].started);
    batch.requests[0].reply(Response(@"一"));
    assert(batch.requests.count == 2 && calls == 0);
    batch.requests[0].reply(Response(@"stale"));
    assert(batch.requests.count == 2);
    batch.requests[1].reply(nil);
    assert(batch.requests.count == 3);
    batch.requests[2].reply([@"malformed" dataUsingEncoding:NSUTF8StringEncoding]);
    assert(batch.requests.count == 4);
    batch.requests[3].reply(Response(@"四"));
    assert(calls == 1);
    // Neither a transport failure nor a malformed body answers anything, so "two" and "three" stay free to be asked again.
    assert(([replies isEqual:@[@[@[@{@"text":@"one", @"translation":@"一"}], @[@"one"]], @[@[], @[]], @[@[], @[]],
        @[@[@{@"text":@"four", @"translation":@"四"}], @[@"four"]]]]));
    [batch start]; [batch cancel];
    batch.requests[3].reply(Response(@"late"));
    assert(calls == 1 && batch.requests.count == 4);
    AssertReleased(batch);
}
static void TestDeadline() {
    for (NSNumber *useTimer in @[@NO, @YES]) {
        __block NSUInteger calls = 0;
        SyntheticTranslationBatch *batch = Batch(@[Item(@"one"), Item(@"two"), Item(@"three")], ^(NSArray *results) {
            assert(++calls == 1);
            assert(([results isEqual:@[@{@"text":@"one", @"translation":@"一"}]]));
        });
        NSMutableArray *answered = [NSMutableArray array];
        NSMutableArray *translated = [NSMutableArray array];
        batch.onReply = ^(NSArray *results, NSArray *texts) {
            [answered addObjectsFromArray:texts];
            [translated addObjectsFromArray:[results valueForKey:@"translation"]];
        };
        batch.now = 50;
        [batch start];
        NSTimer *timer = [batch valueForKey:@"timer"];
        // Non-repeating Foundation timers report a zero repeat interval.
        assert(timer.valid && timer.fireDate.timeIntervalSinceNow > 5 && timer.fireDate.timeIntervalSinceNow <= 6);
        batch.now = 55.9;
        batch.requests[0].reply(Response(@"一"));
        assert(batch.requests.count == 2);
        batch.now = 56;
        if (useTimer.boolValue) [timer fire];
        else batch.requests[1].reply(Response(@"too late"));
        assert(calls == 1 && batch.requests.count == 2 && batch.requests[1].cancelled && !timer.valid);
        batch.requests[1].reply(Response(@"late again"));
        assert(calls == 1);
        // "three" was never sent, so it is never reported as answered. A response the busy main queue delivered after the deadline was still paid for and still reaches onReply, but not completion.
        if (useTimer.boolValue) assert(([answered isEqual:@[@"one"]] && [translated isEqual:@[@"一"]]));
        else assert(([answered isEqual:@[@"one", @"two"]] && [translated isEqual:@[@"一", @"too late"]]));
        AssertReleased(batch);
    }
}
static void TestCancellationAndLifetime() {
    SyntheticTranslationBatch *before = Batch(@[Item(@"one")], ^(NSArray *results) { (void)results; assert(false); });
    [before cancel]; [before start];
    assert(before.requests.count == 0);
    AssertReleased(before);
    SyntheticTranslationBatch *during = Batch(@[Item(@"one"), Item(@"two")], ^(NSArray *results) { (void)results; assert(false); });
    [during start];
    during.requests[0].reply(Response(@"一"));
    NSTimer *timer = [during valueForKey:@"timer"];
    [during cancel]; [during cancel];
    assert(during.requests[1].cancelled && !timer.valid);
    during.requests[1].reply(Response(@"二"));
    [timer fire];
    AssertReleased(during);
    __weak SyntheticTranslationBatch *weakBatch;
    SyntheticTranslationRequest *request;
    @autoreleasepool {
        SyntheticTranslationBatch *released = Batch(@[Item(@"one")], ^(NSArray *results) { (void)results; assert(false); });
        weakBatch = released;
        [released start];
        request = released.requests[0];
    }
    assert(!weakBatch && request.cancelled);
    request.reply(Response(@"late"));
}
static void TestCopiedInput() {
    NSMutableString *text = [@"one" mutableCopy];
    NSMutableDictionary *body = [@{@"text":@"one"} mutableCopy];
    NSMutableDictionary *descriptor = [Item(text)[@"request"] mutableCopy];
    descriptor[@"body"] = body;
    NSMutableArray *items = [NSMutableArray arrayWithObject:@{@"text":text, @"request":descriptor}];
    __block BOOL done = NO;
    SyntheticTranslationBatch *batch = Batch(items, ^(NSArray *results) {
        assert(([results isEqual:@[@{@"text":@"one", @"translation":@"一"}]])); done = YES;
    });
    [text setString:@"mutated"]; body[@"text"] = @"mutated"; descriptor[@"url"] = @"https://changed.invalid/"; [items removeAllObjects];
    [batch start];
    assert([batch.descriptors[0][@"body"][@"text"] isEqual:@"one"]);
    assert([batch.descriptors[0][@"url"] isEqual:@"https://translation.invalid/api"]);
    batch.requests[0].reply(Response(@"一"));
    assert(done);
}
static void TestBoundsAndEmptyResults() {
    NSMutableArray *nine = [NSMutableArray array];
    for (NSUInteger i = 0; i < 9; ++i) [nine addObject:Item([NSString stringWithFormat:@"synthetic-%lu", (unsigned long)i])];
    __block BOOL done = NO;
    SyntheticTranslationBatch *batch = Batch(nine, ^(NSArray *results) { assert(results.count == 0); done = YES; });
    [batch start];
    for (NSUInteger i = 0; i < 9; ++i) {
        assert(batch.requests.count == i + 1);
        batch.requests[i].reply(Response(@""));
    }
    assert(done);
    // 一批是一页候选，最多十个（每页候选数上限，#6679）。
    NSArray *eleven = [[nine arrayByAddingObject:Item(@"ten")] arrayByAddingObject:Item(@"eleven")];
    NSString *oversized = [@"x" stringByPaddingToLength:262145 withString:@"x" startingAtIndex:0];
    for (NSArray *invalid in @[@[], eleven, @[@1], @[@{@"text":@"", @"request":@{}}],
        @[@{@"text":@"one", @"request":@1}], @[Item(oversized)], @[@{@"text":@"one", @"request":@{@"body":oversized}}]]) {
        __block NSUInteger calls = 0;
        SyntheticTranslationBatch *rejected = Batch(invalid, ^(NSArray *results) { assert(++calls == 1 && results.count == 0); });
        [rejected start];
        assert(calls == 1 && rejected.requests.count == 0);
        AssertReleased(rejected);
    }
    // Real transport rejects invalid descriptors synchronously; bounded recursion
    // must finish exactly once without ever creating a network session.
    __block NSUInteger calls = 0;
    MSIMECustomTranslationBatch *invalidTransport = [[MSIMECustomTranslationBatch alloc]
        initWithItems:@[@{@"text":@"one", @"request":@{}}, @{@"text":@"two", @"request":@{}}]
        configuration:NSURLSessionConfiguration.ephemeralSessionConfiguration completion:^(NSArray *results) {
            assert(++calls == 1 && results.count == 0);
        }];
    [invalidTransport start];
    assert(calls == 1);
    AssertReleased(invalidTransport);
}
// A provider that reports a failure in a well-formed body - DeepLX's non-200 code, as a rate limit or an outage comes back - has not answered: negative-caching it would hide the gloss for eight minutes over something asking again fixes. An answer with no translation is still an answer.
static void TestFailedRepliesAreNotAnswers() {
    SyntheticTranslationBatch *batch = Batch(@[Item(@"one"), Item(@"two"), Item(@"three")], ^(NSArray *results) { assert(results.count == 0); });
    NSMutableArray *answered = [NSMutableArray array];
    batch.onReply = ^(NSArray *results, NSArray *texts) { assert(results.count == 0); [answered addObject:texts]; };
    [batch start];
    batch.requests[0].reply([@"{\"code\":429,\"message\":\"rate limited\"}" dataUsingEncoding:NSUTF8StringEncoding]);
    batch.requests[1].reply([@"{\"code\":200,\"data\":\"\"}" dataUsingEncoding:NSUTF8StringEncoding]);
    batch.requests[2].reply([@"{\"code\":\"500\"}" dataUsingEncoding:NSUTF8StringEncoding]);
    assert(([answered isEqual:@[@[], @[@"two"], @[]]]));
    AssertReleased(batch);
}
static NSDictionary *TencentItem(NSString *text, NSString *key, NSString *source, NSString *target) {
    return @{@"text":text, @"key":key, @"source_language":source, @"target_language":target};
}
static SyntheticTranslationBatch *TencentBatch(NSArray *items, void (^completion)(NSArray *)) {
    return [[SyntheticTranslationBatch alloc] initWithTencentItems:items
        config:@{@"enabled":@YES, @"secret_id":@"AKIDsynthetic", @"secret_key":@"synthetic", @"region":@"ap-guangzhou"}
        configuration:NSURLSessionConfiguration.ephemeralSessionConfiguration completion:completion];
}
static NSData *TencentResponse(NSArray *texts) {
    return [NSJSONSerialization dataWithJSONObject:@{@"Response":@{@"TargetTextList":texts}} options:0 error:nil];
}
static void TestTencentGroups() {
    NSMutableString *original = [@"HELLO" mutableCopy];
    NSMutableArray *items = [@[TencentItem(original, @"hello", @"en", @"zh"),
        TencentItem(@"你好", @"你好", @"zh", @"ja"), TencentItem(@"World", @"world", @"en", @"zh")] mutableCopy];
    __block NSUInteger calls = 0;
    SyntheticTranslationBatch *batch = TencentBatch(items, ^(NSArray *results) {
        assert(++calls == 1);
        assert(([results isEqual:@[@{@"text":@"HELLO", @"translation":@"你好"},
            @{@"text":@"你好", @"translation":@"こんにちは"}]]));
    });
    [original setString:@"mutated"]; [items removeAllObjects];
    batch.wallTime = 1704067200;
    [batch start];
    assert(batch.requests.count == 1);
    NSDictionary *first = batch.descriptors[0];
    NSDictionary *body = [NSJSONSerialization JSONObjectWithData:[first[@"body_utf8"] dataUsingEncoding:NSUTF8StringEncoding] options:0 error:nil];
    assert(([body[@"SourceTextList"] isEqual:@[@"hello", @"world"]]));
    assert([body[@"Source"] isEqual:@"en"] && [body[@"Target"] isEqual:@"zh"]);
    assert([first[@"headers"][@"X-TC-Timestamp"] isEqual:@"1704067200"]);
    batch.wallTime += 2;
    batch.requests[0].reply(TencentResponse(@[@"  你好  ", @""]));
    assert(batch.requests.count == 2 && calls == 0);
    assert([batch.descriptors[1][@"headers"][@"X-TC-Timestamp"] isEqual:@"1704067202"]);
    batch.requests[0].reply(TencentResponse(@[@"stale", @"stale"]));
    assert(batch.requests.count == 2);
    batch.requests[1].reply(TencentResponse(@[@"こんにちは"]));
    assert(calls == 1);
    AssertReleased(batch);
}
static void TestTencentFailedRepliesAreNotAnswers() {
    NSArray *items = @[TencentItem(@"Hello", @"hello", @"en", @"zh"), TencentItem(@"你好", @"你好", @"zh", @"en")];
    SyntheticTranslationBatch *batch = TencentBatch(items, ^(NSArray *results) { (void)results; });
    NSMutableArray *answered = [NSMutableArray array];
    batch.onReply = ^(NSArray *results, NSArray *texts) { (void)results; [answered addObject:texts]; };
    [batch start];
    // Tencent reports a rate limit or a signature error as Response.Error in an HTTP 200 body.
    batch.requests[0].reply([@"{\"Response\":{\"Error\":{\"Code\":\"RequestLimitExceeded\"}}}" dataUsingEncoding:NSUTF8StringEncoding]);
    batch.requests[1].reply(TencentResponse(@[@""]));
    assert(([answered isEqual:@[@[], @[@"你好"]]]));
    AssertReleased(batch);
}
static void TestTencentFailuresAndCancellation() {
    NSArray *items = @[TencentItem(@"Hello", @"hello", @"en", @"zh"), TencentItem(@"你好", @"你好", @"zh", @"en")];
    for (NSData *bad in @[TencentResponse(@[]), TencentResponse(@[@"one", @"two"]),
        [@"{\"Response\":{\"Error\":{}}}" dataUsingEncoding:NSUTF8StringEncoding],
        [@"malformed" dataUsingEncoding:NSUTF8StringEncoding]]) {
        __block NSUInteger calls = 0;
        SyntheticTranslationBatch *batch = TencentBatch(items, ^(NSArray *results) {
            assert(++calls == 1 && results.count == 1);
            assert([results[0][@"text"] isEqual:@"你好"]);
        });
        [batch start];
        batch.requests[0].reply(bad);
        batch.requests[1].reply(TencentResponse(@[@"hello"]));
        assert(calls == 1);
        AssertReleased(batch);
    }
    SyntheticTranslationBatch *cancelled = TencentBatch(items, ^(NSArray *results) { (void)results; assert(false); });
    [cancelled start]; [cancelled cancel];
    assert(cancelled.requests[0].cancelled);
    cancelled.requests[0].reply(TencentResponse(@[@"late"]));
    assert(cancelled.requests.count == 1);
    AssertReleased(cancelled);
    for (NSNumber *timerDriven in @[@NO, @YES]) {
        __block NSUInteger calls = 0;
        SyntheticTranslationBatch *batch = TencentBatch(items, ^(NSArray *results) { assert(++calls == 1 && results.count == 1); });
        NSMutableArray *answered = [NSMutableArray array];
        batch.onReply = ^(NSArray *results, NSArray *texts) { (void)results; [answered addObject:texts]; };
        [batch start];
        batch.requests[0].reply(TencentResponse(@[@"你好"]));
        batch.now = 6;
        if (timerDriven.boolValue) [[batch valueForKey:@"timer"] fire];
        else batch.requests[1].reply(TencentResponse(@[@"late"]));
        assert(calls == 1 && batch.requests[1].cancelled);
        // Answers are reported by original text, one language group at a time; the group the deadline cut off is not answered.
        if (timerDriven.boolValue) assert(([answered isEqual:@[@[@"Hello"]]]));
        else assert(([answered isEqual:@[@[@"Hello"], @[@"你好"]]]));
        AssertReleased(batch);
    }
    for (NSArray *invalid in @[@[@1], @[TencentItem(@"", @"hello", @"en", @"zh")],
        @[TencentItem(@"Hello", @"hello", @"invalid", @"zh")]]) {
        __block NSUInteger calls = 0;
        SyntheticTranslationBatch *batch = TencentBatch(invalid, ^(NSArray *results) { assert(++calls == 1 && results.count == 0); });
        [batch start];
        assert(calls == 1 && batch.requests.count == 0);
        AssertReleased(batch);
    }
    NSMutableArray *eleven = [NSMutableArray array];
    for (NSUInteger i = 0; i < 11; ++i) [eleven addObject:items[0]];
    __block NSUInteger calls = 0;
    SyntheticTranslationBatch *oversized = TencentBatch(eleven, ^(NSArray *results) { assert(++calls == 1 && results.count == 0); });
    [oversized start];
    assert(calls == 1 && oversized.requests.count == 0);
    AssertReleased(oversized);
    [eleven removeLastObject];
    __block BOOL tenDone = NO;
    SyntheticTranslationBatch *ten = TencentBatch(eleven, ^(NSArray *results) { assert(results.count == 10); tenDone = YES; });
    [ten start];
    assert(ten.requests.count == 1 && [ten.descriptors[0][@"expected_count"] isEqual:@10]);
    ten.requests[0].reply(TencentResponse(@[@"一", @"二", @"三", @"四", @"五", @"六", @"七", @"八", @"九", @"十"]));
    assert(tenDone);
    AssertReleased(ten);
    for (NSDictionary *config in @[@{}, @{@"enabled":@NO},
        @{@"enabled":@YES, @"secret_id":@"AKIDsynthetic", @"secret_key":@"synthetic", @"region":@"bad\nregion"}]) {
        __block BOOL rejected = NO;
        SyntheticTranslationBatch *invalidConfig = [[SyntheticTranslationBatch alloc] initWithTencentItems:items
            config:config configuration:NSURLSessionConfiguration.ephemeralSessionConfiguration
            completion:^(NSArray *results) { assert(results.count == 0); rejected = YES; }];
        [invalidConfig start];
        assert(rejected && invalidConfig.requests.count == 0);
        AssertReleased(invalidConfig);
    }
    __weak SyntheticTranslationBatch *weakBatch;
    SyntheticTranslationRequest *request;
    @autoreleasepool {
        SyntheticTranslationBatch *released = TencentBatch(items, ^(NSArray *results) { (void)results; assert(false); });
        weakBatch = released;
        [released start];
        request = released.requests[0];
    }
    assert(!weakBatch && request.cancelled);
    request.reply(TencentResponse(@[@"late"]));
}
static void TestDetachLetsInFlightLand() {
    SyntheticTranslationBatch *batch = Batch(@[Item(@"one"), Item(@"two")], ^(NSArray *results) { (void)results; assert(false); });
    NSMutableArray *replies = [NSMutableArray array];
    batch.onReply = ^(NSArray *results, NSArray *answered) { [replies addObject:@[results, answered]]; };
    [batch start];
    NSTimer *timer = [batch valueForKey:@"timer"];
    __block NSUInteger ended = 0;
    assert([batch detachWithCompletion:^{ ++ended; }]);
    // The request already sent is left running; nothing further is sent.
    assert(!batch.requests[0].cancelled && timer.valid && ended == 0);
    batch.requests[0].reply(Response(@"一"));
    assert(([replies isEqual:@[@[@[@{@"text":@"one", @"translation":@"一"}], @[@"one"]]]]));
    assert(batch.requests.count == 1 && ended == 1 && !timer.valid);
    batch.requests[0].reply(Response(@"again"));
    assert(replies.count == 1 && ended == 1);
    AssertReleased(batch);
    // The deadline still bounds a detached batch.
    SyntheticTranslationBatch *slow = Batch(@[Item(@"one")], ^(NSArray *results) { (void)results; assert(false); });
    __block NSUInteger slowReplies = 0;
    slow.onReply = ^(NSArray *results, NSArray *answered) { (void)results; (void)answered; ++slowReplies; };
    [slow start];
    __block BOOL slowEnded = NO;
    assert([slow detachWithCompletion:^{ slowEnded = YES; }]);
    [[slow valueForKey:@"timer"] fire];
    assert(slowEnded && slow.requests[0].cancelled);
    slow.requests[0].reply(Response(@"late"));
    assert(slowReplies == 0);
    AssertReleased(slow);
    // Nothing in flight, before start or after the last reply, cancels outright.
    SyntheticTranslationBatch *idle = Batch(@[Item(@"one")], ^(NSArray *results) { (void)results; assert(false); });
    assert(![idle detachWithCompletion:^{ assert(false); }]);
    [idle start];
    assert(idle.requests.count == 0);
    AssertReleased(idle);
    // A hard cancel of a detached batch drops its reply as well.
    SyntheticTranslationBatch *dropped = Batch(@[Item(@"one")], ^(NSArray *results) { (void)results; assert(false); });
    dropped.onReply = ^(NSArray *results, NSArray *answered) { (void)results; (void)answered; assert(false); };
    [dropped start];
    assert([dropped detachWithCompletion:^{ assert(false); }]);
    [dropped cancel];
    assert(dropped.requests[0].cancelled);
    dropped.requests[0].reply(Response(@"late"));
    AssertReleased(dropped);
}
static void TestAIItems() {
    NSArray *items = @[
        @{@"text":@"候选甲", @"candidate_limit":@1, @"request":@{@"url":@"https://ai.invalid/chat", @"method":@"POST", @"headers":@{@"Content-Type":@"application/json", @"Authorization":@"Bearer synthetic"}, @"body":@{@"model":@"synthetic"}, @"timeout_ms":@8000, @"connect_timeout_ms":@2500, @"max_response_bytes":@1048576}},
    ];
    __block BOOL done = NO;
    SyntheticTranslationBatch *batch = [[SyntheticTranslationBatch alloc] initWithAIItems:items configuration:NSURLSessionConfiguration.ephemeralSessionConfiguration completion:^(NSArray *results) {
        assert(([results isEqual:@[@{@"text":@"候选甲", @"translation":@"释义甲"}]])); done = YES;
    }];
    [batch start]; assert(batch.requests.count == 1 && batch.requests[0].started);
    NSData *body = [NSJSONSerialization dataWithJSONObject:@{@"choices":@[@{@"message":@{@"content":@"{\"candidates\":[{\"text\":\"释义甲\"},{\"text\":\"超出限额\"}]}"}}]} options:0 error:nil];
    batch.requests[0].reply(body); assert(done);
    AssertReleased(batch);
}
int main() {
    @autoreleasepool {
        TestSequentialResults();
        TestFailedRepliesAreNotAnswers();
        TestDeadline();
        TestCancellationAndLifetime();
        TestCopiedInput();
        TestBoundsAndEmptyResults();
        TestTencentGroups();
        TestTencentFailuresAndCancellation();
        TestTencentFailedRepliesAreNotAnswers();
        TestDetachLetsInFlightLand();
        TestAIItems();
    }
    return 0;
}
