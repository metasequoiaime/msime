#import "../../src/voice/DoubaoVoiceRequest.h"
#include <cassert>
#include <limits>
#include <vector>

static void Until(BOOL (^done)(void), NSTimeInterval timeout = 5) {
    NSDate *deadline = [NSDate dateWithTimeIntervalSinceNow:timeout];
    while (!done() && deadline.timeIntervalSinceNow > 0)
        [NSRunLoop.currentRunLoop runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.01]];
    assert(done());
}
static NSData *PCM(NSUInteger count) {
    std::vector<float> samples(count, 0.25f);
    return [NSData dataWithBytes:samples.data() length:samples.size() * sizeof(float)];
}
int main(int argc, const char *argv[]) {
    @autoreleasepool {
        assert(argc == 2);
        NSString *base = @(argv[1]);
        const unichar surrogate = 0xd800;
        NSString *invalidUTF8 = [NSString stringWithCharacters:&surrogate length:1];
        NSDictionary *(^options)(NSString *) = ^(NSString *path) {
            return @{@"asr_endpoint":[base stringByAppendingString:path], @"asr_token":@"fixture-token",
                @"doubao_auth_mode":@"api_key", @"asr_app_key":@"stale-fixture-app",
                @"doubao_enable_itn":@NO, @"doubao_enable_punc":@NO, @"doubao_enable_ddc":@YES,
                @"doubao_boosting_table_id":@"fixture-table"};
        };
        for (NSDictionary *invalid in @[
            @{@"asr_endpoint":@"ws://example.invalid/asr", @"asr_token":@"fixture-token"},
            @{@"asr_endpoint":@"https://example.invalid/asr", @"asr_token":@"fixture-token"},
            @{@"asr_endpoint":@"wss://user@example.invalid/asr", @"asr_token":@"fixture-token"},
            @{@"asr_endpoint":@"wss://example.invalid/asr#fragment", @"asr_token":@"fixture-token"},
            @{@"asr_token":@"fixture\r\nheader"}, @{@"asr_token":@""}, @{@"asr_token":@42},
            @{@"asr_token":@"fixture-token", @"doubao_auth_mode":@"other"},
            @{@"asr_token":@"fixture-token", @"doubao_auth_mode":@"legacy"},
            @{@"asr_token":@"<stored>"}, @{@"asr_token":@"***"}, @{@"asr_token":@"   "},
            @{@"asr_token":@"fixture-token", @"asr_resource_id":@"<stored>"},
            @{@"asr_token":@"fixture-token", @"asr_resource_id":@"***"},
            @{@"asr_token":@"fixture-token", @"doubao_auth_mode":@"legacy", @"asr_app_key":@"<stored>"},
            @{@"asr_token":@"fixture-token", @"doubao_auth_mode":@"legacy", @"asr_app_key":@"***"},
            @{@"asr_token":@"fixture-token", @"doubao_boosting_table_id":invalidUTF8},
            @{@"asr_token":@"fixture-token", @"doubao_enable_itn":@1}
        ]) {
            NSError *error = nil;
            assert(![[MSIMEDoubaoVoiceRequest alloc] initWithOptions:invalid error:&error] && error);
        }
        // An unset mode is the API key route even with an App ID stored.
        for (NSString *path in @[@"/api", @"/legacy", @"/unset", @"/masked-app", @"/trimmed", @"/trimmed-legacy"]) {
            NSMutableDictionary *snapshot = [options(path) mutableCopy];
            if ([path isEqual:@"/legacy"]) snapshot[@"doubao_auth_mode"] = @"legacy";
            if ([path isEqual:@"/unset"]) [snapshot removeObjectForKey:@"doubao_auth_mode"];
            if ([path isEqual:@"/masked-app"]) snapshot[@"asr_app_key"] = @"<stored>";
            if ([path hasPrefix:@"/trimmed"]) {
                snapshot[@"doubao_auth_mode"] = [path isEqual:@"/trimmed-legacy"] ? @" legacy " : @" api_key ";
                snapshot[@"asr_token"] = @" fixture-token ";
                snapshot[@"asr_app_key"] = @" stale-fixture-app ";
                snapshot[@"asr_resource_id"] = @" volc.bigasr.sauc.duration ";
            }
            MSIMEDoubaoVoiceRequest *request = [[MSIMEDoubaoVoiceRequest alloc] initWithOptions:snapshot error:nil];
            assert(request);
            snapshot[@"asr_token"] = @"changed-after-snapshot";
            __block BOOL partial = NO, final = NO;
            __block NSUInteger calls = 0;
            assert([request startWithResult:^(NSString *text, BOOL last, NSError *error) {
                assert(NSThread.isMainThread && !error);
                ++calls;
                assert([text isEqual:last ? @"synthetic final" : @"synthetic partial"]);
                if (last) final = YES; else partial = YES;
            } error:nil]);
            assert(![request startWithResult:^(NSString *, BOOL, NSError *) {} error:nil]);
            assert([request appendPCM:PCM(3219) error:nil]);
            Until(^BOOL { return partial; });
            assert(!final && calls == 1); // A real response arrives before recording finishes.
            assert([request finishWithError:nil]);
            assert(![request finishWithError:nil] && ![request appendPCM:PCM(1) error:nil]);
            Until(^BOOL { return final; });
            assert(calls == 2);
        }
        // The failures MSIME-Windows names carry its sentence, with the credential hint for the console in use; the rest keep only the generic description. None carries the token.
        NSMutableDictionary *refusedLegacy = [options(@"/refused") mutableCopy];
        refusedLegacy[@"asr_endpoint"] = @"ws://127.0.0.1:1/refused";
        refusedLegacy[@"doubao_auth_mode"] = @"legacy";
        NSArray *failures = @[
            @[options(@"/malformed"), NSNull.null], @[options(@"/oversized"), NSNull.null],
            @[options(@"/server-error"), @"豆包语音识别失败（code 45000001）。请检查 Access Token。"],
            @[options(@"/redirect"), @"无法连接豆包语音识别。请检查 API Key 和接口地址。"],
            @[refusedLegacy, @"无法连接豆包语音识别。请检查 App ID、Access Token 和接口地址。"]];
        for (NSArray *failure in failures) {
            MSIMEDoubaoVoiceRequest *request = [[MSIMEDoubaoVoiceRequest alloc] initWithOptions:failure[0] error:nil];
            id expected = failure[1];
            __block BOOL failed = NO;
            assert([request startWithResult:^(NSString *text, BOOL final, NSError *error) {
                assert(NSThread.isMainThread && final && error && !text);
                assert([error.domain isEqual:@"app.msime.client.voice.doubao"]);
                if (expected == NSNull.null) assert(error.userInfo.count == 1);
                else assert(error.userInfo.count == 2 && [error.userInfo[NSLocalizedFailureReasonErrorKey] isEqual:expected]);
                assert(![error.userInfo.description containsString:@"fixture-token"]);
                failed = YES;
            } error:nil]);
            Until(^BOOL { return failed; });
        }
        // Streaming has no length cap: 61 s of audio, one second past the old limit, is all sent and recognised.
        MSIMEDoubaoVoiceRequest *longStream = [[MSIMEDoubaoVoiceRequest alloc] initWithOptions:options(@"/long") error:nil];
        __block NSString *longFinal = nil;
        assert([longStream startWithResult:^(NSString *text, BOOL final, NSError *error) {
            assert(!error); if (final) longFinal = text;
        } error:nil]);
        for (NSUInteger chunk = 0; chunk < 61 * 5; ++chunk) assert([longStream appendPCM:PCM(3200) error:nil]);
        assert([longStream finishWithError:nil]);
        Until(^BOOL { return longFinal != nil; }, 10);
        assert([longFinal isEqual:@"synthetic long 976000"]);
        MSIMEDoubaoVoiceRequest *cancelled = [[MSIMEDoubaoVoiceRequest alloc] initWithOptions:options(@"/cancel") error:nil];
        __block BOOL heard = NO;
        __block NSUInteger calls = 0;
        assert([cancelled startWithResult:^(NSString *, BOOL final, NSError *error) {
            assert(!final && !error); heard = YES; ++calls;
        } error:nil]);
        assert([cancelled appendPCM:PCM(3200) error:nil]);
        Until(^BOOL { return heard; });
        [cancelled cancel]; [cancelled cancel];
        assert(![cancelled finishWithError:nil] && ![cancelled appendPCM:PCM(1) error:nil]);
        [NSRunLoop.currentRunLoop runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.2]];
        assert(calls == 1);
        __weak MSIMEDoubaoVoiceRequest *released;
        __block BOOL dropPartial = NO;
        @autoreleasepool {
            MSIMEDoubaoVoiceRequest *drop = [[MSIMEDoubaoVoiceRequest alloc] initWithOptions:options(@"/drop") error:nil];
            released = drop;
            assert([drop startWithResult:^(NSString *, BOOL final, NSError *error) {
                assert(!final && !error); dropPartial = YES;
            } error:nil]);
            assert([drop appendPCM:PCM(3200) error:nil]);
            Until(^BOOL { return dropPartial; });
            drop = nil;
        }
        Until(^BOOL { return released == nil; });
        MSIMEDoubaoVoiceRequest *invalidPCM = [[MSIMEDoubaoVoiceRequest alloc] initWithOptions:options(@"/invalid-pcm") error:nil];
        __block BOOL rejected = NO;
        assert([invalidPCM startWithResult:^(NSString *text, BOOL final, NSError *error) {
            assert(!text && final && error); rejected = YES;
        } error:nil]);
        float nan = std::numeric_limits<float>::quiet_NaN();
        assert(![invalidPCM appendPCM:[NSData dataWithBytes:&nan length:sizeof(nan)] error:nil]);
        Until(^BOOL { return rejected; });
        MSIMEDoubaoVoiceRequest *silent = [[MSIMEDoubaoVoiceRequest alloc] initWithOptions:options(@"/silent") error:nil];
        __block BOOL timedOut = NO;
        assert([silent startWithResult:^(NSString *text, BOOL final, NSError *error) {
            assert(!text && final && error); timedOut = YES;
        } error:nil]);
        assert([silent finishWithError:nil]);
        Until(^BOOL { return timedOut; }, 35);
    }
}
