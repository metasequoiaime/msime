#import "../../src/voice/LocalVoiceRequest.h"
#include <cassert>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <vector>

// Drives MSIMELocalVoiceRequest against the msime-voice-local protocol. By default the helper is tests/voice/local_voice_helper_fixture.py (MSIME_VOICE_LOCAL_HELPER, set by CMake); `--model <dir> --wav <file>` instead streams a real recording through whichever helper that variable names, for the opt-in end-to-end check against an installed model.

namespace {
BOOL Spin(BOOL (^done)(void), NSTimeInterval seconds) {
    NSDate *deadline = [NSDate dateWithTimeIntervalSinceNow:seconds];
    while (!done() && deadline.timeIntervalSinceNow > 0)
        [NSRunLoop.currentRunLoop runMode:NSDefaultRunLoopMode beforeDate:[NSDate dateWithTimeIntervalSinceNow:0.01]];
    return done();
}

NSString *ModelDirectory(NSString *hotwords) {
    NSString *directory = [NSTemporaryDirectory() stringByAppendingPathComponent:NSUUID.UUID.UUIDString];
    assert([NSFileManager.defaultManager createDirectoryAtPath:directory withIntermediateDirectories:YES attributes:nil error:nil]);
    NSData *manifest = [NSJSONSerialization dataWithJSONObject:@{@"id" : @"fixture", @"hotwords" : hotwords} options:0 error:nil];
    assert([manifest writeToFile:[directory stringByAppendingPathComponent:@"msime-model.json"] atomically:YES]);
    return directory;
}

NSData *Samples(NSUInteger count, float value) {
    std::vector<float> samples(count, value);
    return [NSData dataWithBytes:samples.data() length:samples.size() * sizeof(float)];
}

// Spin's blocks copy a captured C++ object, so they read an Outcome through a pointer to see what the callbacks wrote.
struct Outcome {
    NSMutableArray<NSString *> *partials = [NSMutableArray array];
    NSString *final = nil;
    NSError *error = nil;
    NSUInteger calls = 0;
};

MSIMEDoubaoResult Recorder(Outcome *outcome) {
    return ^(NSString *text, BOOL final, NSError *error) {
        ++outcome->calls;
        assert([NSThread isMainThread]);
        if (error) { assert(!outcome->error && !outcome->final); outcome->error = error; return; }
        if (final) { assert(!outcome->final); outcome->final = text; return; }
        [outcome->partials addObject:text];
    };
}

NSArray<NSDictionary *> *LoggedRequests(NSString *log) {
    NSString *contents = [NSString stringWithContentsOfFile:log encoding:NSUTF8StringEncoding error:nil] ?: @"";
    NSMutableArray *requests = [NSMutableArray array];
    for (NSString *line in [contents componentsSeparatedByString:@"\n"])
        if (line.length) [requests addObject:[NSJSONSerialization JSONObjectWithData:[line dataUsingEncoding:NSUTF8StringEncoding] options:0 error:nil]];
    return requests;
}

int RunModel(NSString *model, NSString *wav) {
    NSData *file = [NSData dataWithContentsOfFile:wav];
    // The canonical 44-byte header of a 16 kHz mono 16-bit recording, which is what the model catalogs ship and scripts produce.
    assert(file.length > 44 && std::memcmp(file.bytes, "RIFF", 4) == 0);
    const int16_t *pcm = reinterpret_cast<const int16_t *>(static_cast<const uint8_t *>(file.bytes) + 44);
    const NSUInteger count = (file.length - 44) / 2;
    MSIMELocalVoiceRequest *request = [[MSIMELocalVoiceRequest alloc] initWithOptions:@{@"asr_model_path" : model, @"language" : @"zh-cn"} hostOptions:nil error:nil];
    assert(request);
    Outcome outcome;
    Outcome *outcomeState = &outcome;
    assert([request startWithResult:Recorder(&outcome) error:nil]);
    // Capture delivers about 100 ms at a time; send the recording the same way.
    for (NSUInteger offset = 0; offset < count; offset += 1600) {
        const NSUInteger length = MIN((NSUInteger)1600, count - offset);
        std::vector<float> samples(length);
        for (NSUInteger index = 0; index < length; ++index) samples[index] = pcm[offset + index] / 32768.0f;
        assert([request appendPCM:[NSData dataWithBytes:samples.data() length:length * sizeof(float)] error:nil]);
    }
    assert([request finishWithError:nil]);
    assert(Spin(^{ return (BOOL)(outcomeState->final || outcomeState->error); }, 120));
    assert(!outcome.error);
    std::printf("final after %lu partials: %s\n", static_cast<unsigned long>(outcome.partials.count), outcome.final.UTF8String);
    assert(outcome.final.length);
    return 0;
}
} // namespace

int main(int argc, char **argv) {
    @autoreleasepool {
        if (argc == 5 && std::strcmp(argv[1], "--model") == 0 && std::strcmp(argv[3], "--wav") == 0)
            return RunModel(@(argv[2]), @(argv[4]));
        NSString *log = NSProcessInfo.processInfo.environment[@"MSIME_LOCAL_VOICE_FIXTURE_LOG"];
        assert(log.length && MSIMELocalVoiceHelperPath());
        [NSFileManager.defaultManager removeItemAtPath:log error:nil];

        // Only an installed model directory is accepted: not a file, not a directory the installer never finished, not a relative path.
        NSString *native = ModelDirectory(@"native");
        NSString *external = ModelDirectory(@"native");
        NSString *linked = [NSTemporaryDirectory() stringByAppendingPathComponent:NSUUID.UUID.UUIDString];
        assert([NSFileManager.defaultManager createSymbolicLinkAtPath:linked withDestinationPath:external error:nil]);
        assert(!MSIMELocalVoiceModelDirectory(linked));
        NSError *linkedError = nil;
        assert(![[MSIMELocalVoiceRequest alloc] initWithOptions:@{@"asr_model_path" : linked} hostOptions:nil error:&linkedError] && linkedError);
        NSString *nested = [external stringByAppendingPathComponent:@"nested"];
        assert([NSFileManager.defaultManager createDirectoryAtPath:nested withIntermediateDirectories:NO attributes:nil error:nil]);
        assert([@"{}" writeToFile:[nested stringByAppendingPathComponent:@"msime-model.json"] atomically:YES encoding:NSUTF8StringEncoding error:nil]);
        NSString *linkedAncestor = [NSTemporaryDirectory() stringByAppendingPathComponent:NSUUID.UUID.UUIDString];
        assert([NSFileManager.defaultManager createSymbolicLinkAtPath:linkedAncestor withDestinationPath:external error:nil]);
        NSString *viaLinkedAncestor = [linkedAncestor stringByAppendingPathComponent:@"nested"];
        assert(!MSIMELocalVoiceModelDirectory(viaLinkedAncestor));
        NSError *ancestorError = nil;
        assert(![[MSIMELocalVoiceRequest alloc] initWithOptions:@{@"asr_model_path" : viaLinkedAncestor} hostOptions:nil error:&ancestorError] && ancestorError);
        NSString *linkedManifestModel = ModelDirectory(@"native");
        NSString *linkedManifestPath = [linkedManifestModel stringByAppendingPathComponent:@"msime-model.json"];
        assert([NSFileManager.defaultManager removeItemAtPath:linkedManifestPath error:nil]);
        assert([NSFileManager.defaultManager createSymbolicLinkAtPath:linkedManifestPath withDestinationPath:[external stringByAppendingPathComponent:@"msime-model.json"] error:nil]);
        assert(!MSIMELocalVoiceModelDirectory(linkedManifestModel));
        NSString *unfinished = [NSTemporaryDirectory() stringByAppendingPathComponent:NSUUID.UUID.UUIDString];
        assert([NSFileManager.defaultManager createDirectoryAtPath:unfinished withIntermediateDirectories:YES attributes:nil error:nil]);
        NSString *file = [native stringByAppendingPathComponent:@"msime-model.json"];
        for (id rejected in @[@"", @"relative", unfinished, file, @42]) {
            NSError *error = nil;
            assert(![[MSIMELocalVoiceRequest alloc] initWithOptions:@{@"asr_model_path" : rejected} hostOptions:nil error:&error] && error);
            assert(!MSIMELocalVoiceModelDirectory([rejected isKindOfClass:NSString.class] ? rejected : nil));
        }
        assert(MSIMELocalVoiceModelDirectory(native));

        NSString *oversized = [NSTemporaryDirectory() stringByAppendingPathComponent:NSUUID.UUID.UUIDString];
        assert([NSFileManager.defaultManager createDirectoryAtPath:oversized withIntermediateDirectories:YES attributes:nil error:nil]);
        NSDictionary *oversizedManifest = @{
            @"id" : @"fixture",
            @"hotwords" : @"pinyin",
            @"padding" : [@"x" stringByPaddingToLength:64 * 1024 withString:@"x" startingAtIndex:0]
        };
        NSData *oversizedData = [NSJSONSerialization dataWithJSONObject:oversizedManifest options:0 error:nil];
        assert(oversizedData.length > 64 * 1024);
        assert([oversizedData writeToFile:[oversized stringByAppendingPathComponent:@"msime-model.json"] atomically:YES]);
        assert(!MSIMELocalVoiceModelDirectory(oversized));

        // Streaming: partial text as audio arrives, then one final; the start carries the model and language and, with no dictionary to read, no hotwords.
        {
            MSIMELocalVoiceRequest *request = [[MSIMELocalVoiceRequest alloc] initWithOptions:@{@"asr_model_path" : native, @"language" : @"zh-cn"} hostOptions:nil error:nil];
            assert(request);
            Outcome outcome;
            Outcome *outcomeState = &outcome;
            assert(![request appendPCM:Samples(160, 0.5f) error:nil]);
            assert([request startWithResult:Recorder(&outcome) error:nil]);
            assert(![request startWithResult:Recorder(&outcome) error:nil]);
            assert(![request appendPCM:[NSData dataWithBytes:"abc" length:3] error:nil]);
            assert([request appendPCM:Samples(160, 0.5f) error:nil]);
            assert([request appendPCM:Samples(320, -2.0f) error:nil]);
            assert(Spin(^{ return (BOOL)(outcomeState->partials.count == 2); }, 10));
            assert([outcome.partials isEqual:(@[@"partial 160", @"partial 480"])]);
            assert([request finishWithError:nil]);
            assert(![request finishWithError:nil] && ![request appendPCM:Samples(160, 0) error:nil]);
            assert(Spin(^{ return (BOOL)(outcomeState->final != nil); }, 10));
            assert([outcome.final isEqual:@"final 480"] && !outcome.error);
            NSDictionary *start = LoggedRequests(log).firstObject;
            assert([start[@"op"] isEqual:@"start"] && [start[@"model"] isEqual:native] && [start[@"language"] isEqual:@"zh-cn"]);
            assert([start[@"hotwords"] isEqual:@[]]);
        }

        // Cancelling mid-session tells the helper and delivers nothing more; the next request reuses the same helper. The first partial is what says the session reached the helper: a cancel that beats the queued start leaves nothing to cancel.
        {
            MSIMELocalVoiceRequest *request = [[MSIMELocalVoiceRequest alloc] initWithOptions:@{@"asr_model_path" : native, @"language" : @"zh-cn"} hostOptions:nil error:nil];
            Outcome outcome;
            Outcome *outcomeState = &outcome;
            assert([request startWithResult:Recorder(&outcome) error:nil]);
            assert([request appendPCM:Samples(160, 0.1f) error:nil]);
            assert(Spin(^{ return (BOOL)(outcomeState->calls == 1); }, 10));
            [request cancel];
            assert(![request appendPCM:Samples(160, 0.1f) error:nil] && ![request finishWithError:nil]);
            assert(Spin(^{ return (BOOL)[[LoggedRequests(log).lastObject objectForKey:@"op"] isEqual:@"cancel"]; }, 10));
            Spin(^{ return NO; }, 0.3);
            assert(outcome.calls == 1 && !outcome.final && !outcome.error);
        }

        // A model the helper cannot load fails the request with the helper's own message as the detail.
        {
            MSIMELocalVoiceRequest *request = [[MSIMELocalVoiceRequest alloc] initWithOptions:@{@"asr_model_path" : native, @"language" : @"error"} hostOptions:nil error:nil];
            Outcome outcome;
            Outcome *outcomeState = &outcome;
            assert([request startWithResult:Recorder(&outcome) error:nil]);
            assert(Spin(^{ return (BOOL)(outcomeState->error != nil); }, 10));
            assert([outcome.error.userInfo[NSLocalizedFailureReasonErrorKey] isEqual:@"synthetic model failure"]);
        }

        // A helper that dies mid-session fails that session once, and the next session starts a fresh helper.
        {
            MSIMELocalVoiceRequest *request = [[MSIMELocalVoiceRequest alloc] initWithOptions:@{@"asr_model_path" : native, @"language" : @"exit"} hostOptions:nil error:nil];
            Outcome outcome;
            Outcome *outcomeState = &outcome;
            assert([request startWithResult:Recorder(&outcome) error:nil]);
            assert([request appendPCM:Samples(160, 0.1f) error:nil]);
            assert(Spin(^{ return (BOOL)(outcomeState->error != nil); }, 10));
            Spin(^{ return NO; }, 0.3);
            assert(outcome.calls == 1 && !outcome.final);

            MSIMELocalVoiceRequest *next = [[MSIMELocalVoiceRequest alloc] initWithOptions:@{@"asr_model_path" : native, @"language" : @"zh-cn"} hostOptions:nil error:nil];
            Outcome recovered;
            Outcome *recoveredState = &recovered;
            assert([next startWithResult:Recorder(&recovered) error:nil]);
            assert([next appendPCM:Samples(80, 0.1f) error:nil]);
            assert([next finishWithError:nil]);
            assert(Spin(^{ return (BOOL)(recoveredState->final != nil); }, 10));
            assert([recovered.final isEqual:@"final 80"] && !recovered.error);
        }

        // A request released without cancel still ends its helper session.
        {
            __block NSUInteger partials = 0;
            @autoreleasepool {
                MSIMELocalVoiceRequest *dropped = [[MSIMELocalVoiceRequest alloc] initWithOptions:@{@"asr_model_path" : native, @"language" : @"zh-cn"} hostOptions:nil error:nil];
                assert([dropped startWithResult:^(NSString *, BOOL final, NSError *error) {
                    assert(!final && !error);
                    ++partials;
                } error:nil]);
                assert([dropped appendPCM:Samples(160, 0.1f) error:nil]);
                assert(Spin(^{ return (BOOL)(partials == 1); }, 10));
            }
            assert(Spin(^{ return (BOOL)[[LoggedRequests(log).lastObject objectForKey:@"op"] isEqual:@"cancel"]; }, 10));
            Spin(^{ return NO; }, 0.3);
        }

        [NSFileManager.defaultManager removeItemAtPath:native error:nil];
        [NSFileManager.defaultManager removeItemAtPath:linked error:nil];
        [NSFileManager.defaultManager removeItemAtPath:linkedAncestor error:nil];
        [NSFileManager.defaultManager removeItemAtPath:external error:nil];
        [NSFileManager.defaultManager removeItemAtPath:linkedManifestModel error:nil];
        [NSFileManager.defaultManager removeItemAtPath:unfinished error:nil];
        [NSFileManager.defaultManager removeItemAtPath:log error:nil];
    }
    return 0;
}
