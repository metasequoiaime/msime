#import "../../src/voice/HTTPVoiceRequest.h"
#include <cassert>
// Long enough for the deliberately slow fixture responses; the loop leaves as soon as the work finishes,
// so raising the ceiling costs the passing cases nothing.
static void Wait(BOOL (^done)(void)) {
    NSDate *deadline = [NSDate dateWithTimeIntervalSinceNow:20];
    while (!done() && deadline.timeIntervalSinceNow > 0)
        [NSRunLoop.currentRunLoop runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.001]];
    assert(done());
}
int main(int argc, char **argv) {
    @autoreleasepool {
        assert(argc == 2);
        NSString *base = @(argv[1]);
        NSMutableString *model = [@"fixture-model" mutableCopy];
        NSMutableDictionary *options = [@{@"polish_enabled": @YES, @"polish_provider": @"openai",
            @"polish_endpoint": [base stringByAppendingString:@"/polish"], @"polish_model": model,
            @"polish_token": @"fixture-token", @"polish_prompt_id": @"custom_2", @"polish_prompt_custom_2": @"synthetic prompt"} mutableCopy];
        MSIMEHTTPVoiceRequest *request = [[MSIMEHTTPVoiceRequest alloc] initWithPolishOptions:options error:nil];
        assert(request);
        [model setString:@"mutated"];
        NSMutableString *input = [@"synthetic transcript" mutableCopy];
        __block BOOL done = NO;
        assert([request polishText:input completion:^(NSString *text, NSError *error) {
            assert(NSThread.isMainThread && !error && [text isEqual:@"synthetic polished"]); done = YES;
        } error:nil]);
        [input setString:@"mutated input"];
        Wait(^BOOL { return done; });
        assert(![request polishText:@"second" completion:^(NSString *, NSError *) {} error:nil]);
        options[@"polish_model"] = @"fixture-model";
        // A polish that fails hands back the transcript untouched. That is the whole reason the budget
        // matters: the failure is silent, so a budget the service cannot meet means the transcript went to
        // the provider and the answer was binned with nothing shown.
        for (NSString *path in @[@"/failure", @"/empty"]) {
            options[@"polish_endpoint"] = [base stringByAppendingString:path];
            request = [[MSIMEHTTPVoiceRequest alloc] initWithPolishOptions:options error:nil];
            done = NO;
            assert([request polishText:@"synthetic transcript" completion:^(NSString *text, NSError *error) {
                assert(!error && [text isEqual:@"synthetic transcript"]); done = YES;
            } error:nil]);
            Wait(^BOOL { return done; });
        }
        // Six seconds is slow for a chat completion and well inside the budget this host asks for, so the
        // answer is waited for and used. It used to be abandoned at three, which is under what cleaning a
        // minute of transcript takes - the request went out and its reply was thrown away every time.
        for (NSString *path in @[@"/stall-headers", @"/stall-body"]) {
            options[@"polish_endpoint"] = [base stringByAppendingString:path];
            request = [[MSIMEHTTPVoiceRequest alloc] initWithPolishOptions:options error:nil];
            done = NO;
            const NSTimeInterval started = NSProcessInfo.processInfo.systemUptime;
            assert([request polishText:@"synthetic transcript" completion:^(NSString *text, NSError *error) {
                assert(!error && [text isEqual:@"synthetic polished"]); done = YES;
            } error:nil]);
            Wait(^BOOL { return done; });
            const NSTimeInterval elapsed = NSProcessInfo.processInfo.systemUptime - started;
            assert(elapsed >= 5.5);
        }
        options[@"polish_enabled"] = @NO;
        request = [[MSIMEHTTPVoiceRequest alloc] initWithPolishOptions:options error:nil];
        assert([request polishText:@"cancelled" completion:^(NSString *, NSError *) { assert(false); } error:nil]);
        [request cancel];
        [NSRunLoop.currentRunLoop runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.1]];
        request = [[MSIMEHTTPVoiceRequest alloc] initWithPolishOptions:options error:nil];
        assert(![request polishText:@"" completion:^(NSString *, NSError *) {} error:nil]);
        assert(![request polishText:[@"x" stringByPaddingToLength:65537 withString:@"x" startingAtIndex:0] completion:^(NSString *, NSError *) {} error:nil]);
        unichar invalid = 0xd800;
        options[@"polish_prompt_custom_1"] = [NSString stringWithCharacters:&invalid length:1];
        assert(![[MSIMEHTTPVoiceRequest alloc] initWithPolishOptions:options error:nil]);
    }
}
