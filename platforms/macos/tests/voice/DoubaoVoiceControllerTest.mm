#import "../../src/input/InputController.mm"
#import "VoiceCueFixture.h"
#import "VoiceClientFixture.h"
#import "VoiceMeterFixture.h"
#include <cassert>

@interface DoubaoRequestFixture : NSObject
@property(copy) MSIMEDoubaoResult result;
@property NSMutableData *audio;
@property NSUInteger finishes;
@property NSUInteger cancellations;
@property BOOL failStart;
@property BOOL failAppend;
- (BOOL)startWithResult:(MSIMEDoubaoResult)result error:(NSError **)error;
- (BOOL)appendPCM:(NSData *)pcm error:(NSError **)error;
- (BOOL)finishWithError:(NSError **)error;
- (void)cancel;
@end
@implementation DoubaoRequestFixture
- (BOOL)startWithResult:(MSIMEDoubaoResult)result error:(NSError **)error {
    (void)error; self.result = result; self.audio = [NSMutableData data]; return !self.failStart;
}
- (BOOL)appendPCM:(NSData *)pcm error:(NSError **)error {
    (void)error; if (self.failAppend) return NO; [self.audio appendData:pcm]; return YES;
}
- (BOOL)finishWithError:(NSError **)error { (void)error; ++self.finishes; return YES; }
- (void)cancel { ++self.cancellations; }
@end

@interface DoubaoCaptureFixture : MSIMEVoiceInputService
@property NSTimeInterval capturedSeconds;
@property(copy) MSIMEVoicePCMChunk chunk;
@property BOOL failStart;
@property NSUInteger finishes;
@end
@implementation DoubaoCaptureFixture
- (NSTimeInterval)recordedDuration { return self.capturedSeconds; }
- (AVAuthorizationStatus)microphoneAuthorizationStatus { return AVAuthorizationStatusAuthorized; }
- (SFSpeechRecognizerAuthorizationStatus)speechAuthorizationStatus { assert(false && "Doubao must not request Speech permission"); return SFSpeechRecognizerAuthorizationStatusDenied; }
- (BOOL)startPCMStreaming:(MSIMEVoicePCMChunk)handler deviceUID:(NSString *)device error:(NSError **)error {
    (void)device; (void)error; self.chunk = handler; return !self.failStart;
}
- (NSData *)finishPCMStreamingWithError:(NSError **)error {
    (void)error; ++self.finishes; return [NSMutableData dataWithLength:16];
}
@end

@interface DoubaoPresentationFixture : NSObject
@property(copy) NSString *detail;
// The controller picks the overlay screen from the caret on every failure; model the real overlay property so the assignment lands somewhere.
@property(nonatomic, weak) NSScreen *preferredScreen;
@property float lastLevel;
@property NSUInteger levelUpdates;
@property(copy) void (^actionHandler)(BOOL);
@property BOOL dismissed;
@property(copy) NSString *preview;
@property NSUInteger phase;
@property NSUInteger failure;
@property NSUInteger failures;
- (void)setListening:(BOOL)listening;
- (void)setProcessing:(BOOL)polishing;
- (void)setInputLevel:(float)level;
- (void)restore;
- (void (^)(void))deferredMute;
- (void)playStartCue;
@end
@implementation DoubaoPresentationFixture
- (void)applyThemePreferences:(NSDictionary *)preferences { (void)preferences; }
- (void)dismissProcessing { self.dismissed = YES; }
- (void)setListening:(BOOL)listening { self.phase = listening ? 1 : 0; self.failure = 0; self.preview = @""; }
- (void)setTranscript:(NSString *)text { self.preview = text; }
- (void)showFailure:(MSIMEVoiceFailure)failure detail:(NSString *)detail { self.failure = failure; self.detail = detail; self.phase = 4; ++self.failures; self.preview = @""; }
- (void)dismissFailure { if (self.failure) [self setListening:NO]; }
- (void)setProcessing:(BOOL)polishing { self.phase = polishing ? 3 : 2; }
- (void)setInputLevel:(float)level { self.lastLevel = level; ++self.levelUpdates; }
- (void)restore {}
- (void (^)(void))deferredMute { return ^{}; }
- (void)playStartCue {}
@end

@interface DoubaoTextFixture : NSObject <MSIMETextClient>
@property(copy) NSString *marked;
@property NSMutableArray *commits;
@end
@implementation DoubaoTextFixture
// The controller asks the client for the caret whenever it repositions the overlay; an empty rectangle reads as "no usable position".
- (NSDictionary *)attributesForCharacterIndex:(NSUInteger)index lineHeightRectangle:(NSRect *)rectangle {
    (void)index; if (rectangle) *rectangle = NSZeroRect; return @{};
}
- (void)insertText:(id)text replacementRange:(NSRange)range {
    (void)range; if (!self.commits) self.commits = [NSMutableArray array]; [self.commits addObject:text]; self.marked = @"";
}
- (void)setMarkedText:(id)text selectionRange:(NSRange)selection replacementRange:(NSRange)replacement {
    (void)selection; (void)replacement; self.marked = text;
}
@end

@interface DoubaoPolishFixture : NSObject
@property(copy) void (^completion)(NSString *, NSError *);
@property NSUInteger submissions;
@property NSUInteger cancellations;
- (BOOL)polishText:(NSString *)text completion:(void (^)(NSString *, NSError *))completion error:(NSError **)error;
- (void)cancel;
@end
@implementation DoubaoPolishFixture
- (BOOL)polishText:(NSString *)text completion:(void (^)(NSString *, NSError *))completion error:(NSError **)error {
    (void)text; (void)error; ++self.submissions; self.completion = completion; return YES;
}
- (void)cancel { ++self.cancellations; }
@end

@interface DoubaoControllerFixture : MSIMEInputController
@property DoubaoRequestFixture *fixture;
@property BOOL failRequest;
@property BOOL usePolishFixture;
@property DoubaoPolishFixture *polishFixture;
@property NSUInteger externalCommits;
@end
@implementation DoubaoControllerFixture
- (MSIMEVoiceCommitOutcome)postVoiceText:(NSString *)text route:(const MSIMEVoiceCommitRoute &)route {
    assert([text isEqual:@"synthetic routed"] && ![route.mode isEqual:@"tsf"]);
    ++self.externalCommits; return MSIMEVoiceCommitOutcome::posted;
}
- (MSIMEHTTPVoiceRequest *)makeDoubaoPolishRequest:(NSDictionary *)options {
    if (!self.usePolishFixture) return [super makeDoubaoPolishRequest:options];
    self.polishFixture = [DoubaoPolishFixture new]; return (id)self.polishFixture;
}
- (MSIMEDoubaoVoiceRequest *)makeDoubaoVoiceRequest:(NSDictionary *)options error:(NSError **)error {
    (void)options; (void)error; self.fixture = [DoubaoRequestFixture new]; self.fixture.failStart = self.failRequest; return (id)self.fixture;
}
- (void)apply:(NSDictionary *)transition {
    MSIMEApplyTransition(transition, [self valueForKey:@"activeClient"]);
}
@end

static BOOL Start(DoubaoControllerFixture *controller, DoubaoCaptureFixture *capture, MSIMEClientSession *session, BOOL stream) {
    uint64_t generation = 0;
    assert([capture startWithSession:session generation:&generation error:nil] && generation);
    [controller setValue:@(generation) forKey:@"voiceGeneration"];
    return [controller startDoubaoVoiceInputWithOptions:@{@"stream": @(stream)}];
}
static void Pump(void) {
    [NSRunLoop.currentRunLoop runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.03]];
}

static void ApplyPunctuationPreferences(MSIMEInputController *controller, NSString *mode) {
    [NSApplication sharedApplication];
    NSUserDefaults *defaults = NSUserDefaults.standardUserDefaults;
    NSDictionary *old = [defaults volatileDomainForName:NSArgumentDomain];
    NSMutableDictionary *arguments = [old mutableCopy] ?: [NSMutableDictionary dictionary];
    arguments[@"MSIMEClientVoiceEnabled"] = @YES;
    [defaults setVolatileDomain:arguments forName:NSArgumentDomain];
    NSDictionary *voice = mode ? @{ @"punctuation_mode": mode } : @{};
    [controller applySharedToolbarPreferences:@{@"voice_input": voice}];
    [defaults setVolatileDomain:old forName:NSArgumentDomain];
}

int main() {
    @autoreleasepool {
        NSString *root = [NSTemporaryDirectory() stringByAppendingPathComponent:NSUUID.UUID.UUIDString];
        // This fixture exercises finishing a Chinese composition before voice;
        // do not depend on the product's default (English) input mode.
        NSMutableDictionary *options = [@{@"api_version": @1, @"preferences": @{@"scheme": @"quanpin", @"default_ime_mode": @"chinese", @"learning": @NO, @"candidate_page_size": @5, @"chinese_punctuation": @YES}} mutableCopy];
        for (NSString *name in @[@"resources", @"user_data", @"cache", @"dictionaries"]) {
            NSString *path = [root stringByAppendingPathComponent:name];
            assert([NSFileManager.defaultManager createDirectoryAtPath:path withIntermediateDirectories:YES attributes:nil error:nil]);
            options[name] = path;
        }
        MSIMEClientSession *session = [[MSIMEClientSession alloc] initWithOptions:options error:nil];
        assert(session);
        DoubaoControllerFixture *controller = [DoubaoControllerFixture alloc];
        DoubaoPresentationFixture *presentation = [DoubaoPresentationFixture new];
        [controller setValue:presentation forKey:@"voiceOverlay"];
        [controller setValue:presentation forKey:@"voiceAudioMuter"];
        MSIMEVoiceCueFixture *cues = [MSIMEVoiceCueFixture new];
        [controller setValue:cues forKey:@"voiceCuePlayer"];
        NSUserDefaults *cueDefaults = NSUserDefaults.standardUserDefaults;
        NSDictionary *oldCueArguments = [cueDefaults volatileDomainForName:NSArgumentDomain];
        [cueDefaults setVolatileDomain:@{@"MSIMEClientVoiceSoundEnabled": @YES, @"MSIMEClientVoiceStartSound": @YES, @"MSIMEClientVoiceEndSound": @YES} forName:NSArgumentDomain];
        DoubaoCaptureFixture *capture = [DoubaoCaptureFixture new];
        capture.capturedSeconds = 0.25;
        DoubaoTextFixture *client = [DoubaoTextFixture new];
        [controller setValue:session forKey:@"session"];
        [controller setValue:capture forKey:@"voiceService"];
        [controller setValue:client forKey:@"activeClient"];
        assert(Start(controller, capture, session, YES));
        assert(cues.starts == 1 && cues.stops == 0);
        DoubaoRequestFixture *request = controller.fixture;
        void (^oldAction)(BOOL) = presentation.actionHandler;
        request.result(@"synthetic partial", NO, nil);
        assert(!presentation.preview.length);
        assert([client.marked isEqual:@"synthetic partial"] && client.commits.count == 0);
        capture.chunk(MSIMEVoiceMeterFixturePCM(), nil); Pump();
        assert(presentation.lastLevel > 0.5f && presentation.lastLevel < 0.7f && presentation.levelUpdates == 1);
        assert([request.audio isEqual:MSIMEVoiceMeterFixturePCM()]); // Metering never changes recognition audio.
        MSIMEVoicePCMChunk oldMeter = capture.chunk;
        presentation.actionHandler(NO);
        presentation.actionHandler(NO);
        assert(presentation.dismissed && capture.active);
        assert(request.audio.length == 48 && request.finishes == 1 && capture.finishes == 1);
        assert(!request.cancellations);
        request.result(@"synthetic final", YES, nil);
        assert(client.commits.count == 1 && [client.commits[0] isEqual:@"synthetic final"] && !capture.active);
        assert(cues.starts == 1 && cues.stops == 1);
        request.result(@"duplicate", YES, nil);
        assert(client.commits.count == 1);
        assert(Start(controller, capture, session, NO));
        oldMeter(MSIMEVoiceMeterFixturePCM(), nil); Pump();
        assert(presentation.levelUpdates == 1); // Old request cannot meter a successor.
        oldAction(YES); oldAction(NO);
        assert(capture.active && !controller.fixture.finishes && !controller.fixture.cancellations);
        controller.fixture.result(@"hidden partial", NO, nil);
        assert(!client.marked.length);
        assert([presentation.preview isEqual:@"hidden partial"]);
        [controller cancelDoubaoVoiceInput];
        assert(!presentation.preview.length);
        assert(Start(controller, capture, session, YES));
        request = controller.fixture;
        request.result(@"cancelled partial", NO, nil);
        presentation.actionHandler(YES);
        assert(!client.marked.length && request.cancellations == 1);
        assert(Start(controller, capture, session, YES));
        request.result(@"late old final", YES, nil);
        assert(capture.active && client.commits.count == 1);
        [controller finishDoubaoVoiceInput];
        [controller finishDoubaoVoiceInput];
        assert(!capture.active && controller.fixture.finishes == 1);
        for (NSString *field in @[@"activeClient", @"session", @"voiceGeneration"]) {
            assert(Start(controller, capture, session, YES));
            id original = [controller valueForKey:field];
            [controller setValue:[field isEqual:@"voiceGeneration"] ? @99999 : [MSIMEVoiceClientFixture new] forKey:field];
            controller.fixture.result(@"stale focus", YES, nil);
            [controller setValue:original forKey:field];
            assert(client.commits.count == 1);
        }
        capture.failStart = YES;
        const auto beforeFailure = cues.starts;
        assert(!Start(controller, capture, session, YES) && !capture.active);
        assert(presentation.failure == MSIMEVoiceFailureCapture);
        assert(cues.starts == beforeFailure && cues.stops == beforeFailure);
        capture.failStart = NO;
        controller.failRequest = YES;
        assert(!Start(controller, capture, session, YES) && !capture.active);
        assert(presentation.failure == MSIMEVoiceFailureProvider);
        controller.failRequest = NO;
        assert(Start(controller, capture, session, YES));
        controller.fixture.failAppend = YES;
        capture.chunk([NSMutableData dataWithLength:32], nil);
        Pump();
        assert(!capture.active);
        // Exercise the actual toggle route without touching persistent defaults.
        NSUserDefaults *defaults = NSUserDefaults.standardUserDefaults;
        NSDictionary *oldArguments = [defaults volatileDomainForName:NSArgumentDomain];
        NSDictionary *toggleArguments = @{@"MSIMEClientVoiceEnabled": @YES, @"MSIMEClientVoiceASRProvider": @"doubao", @"MSIMEClientVoiceMuteSystemAudio": @NO, @"MSIMEClientVoiceSoundEnabled": @YES, @"MSIMEClientVoiceStartSound": @YES, @"MSIMEClientVoiceEndSound": @YES};
        [defaults setVolatileDomain:toggleArguments forName:NSArgumentDomain];
        assert([controller usesNativeDoubaoVoice] && ![controller usesNativeHTTPVoice]);
        // No token: Windows asks for one before recording rather than failing after the user has spoken.
        [controller toggleVoiceInput:nil];
        assert(!capture.active && presentation.failure == MSIMEVoiceFailureMissingToken);
        // `local` holding a Whisper model file, not an installed model directory, refuses before recording too, rather than starting the system recognizer.
        NSMutableDictionary *whisperFile = [toggleArguments mutableCopy];
        whisperFile[@"MSIMEClientVoiceASRProvider"] = @"local";
        whisperFile[@"MSIMEClientVoiceASRModelPath"] = @"/synthetic/ggml-large-v3-turbo.bin";
        [defaults setVolatileDomain:whisperFile forName:NSArgumentDomain];
        assert(![controller usesNativeDoubaoVoice] && ![controller usesNativeHTTPVoice]);
        [controller toggleVoiceInput:nil];
        assert(!capture.active && presentation.failure == MSIMEVoiceFailureMissingLocalModel);
        NSMutableDictionary *withToken = [toggleArguments mutableCopy];
        withToken[@"MSIMEClientVoiceASRToken"] = @"synthetic-token";
        [defaults setVolatileDomain:withToken forName:NSArgumentDomain];
        assert([session setFocused:YES error:nil]);
        assert([session typeASCII:'U' shift:YES error:nil]);
        for (const char *key = "4e2d"; *key; ++key) assert([session typeASCII:*key shift:NO error:nil]);
        [controller toggleVoiceInput:nil];
        assert(client.commits.count == 2 && [client.commits[1] isEqual:@"中"]);
        assert(!client.marked.length); // Engine composition finishes before voice presentation.
        assert(capture.active && [controller valueForKey:@"doubaoVoiceRequest"] == controller.fixture);
        [controller toggleVoiceInput:nil];
        assert(controller.fixture.finishes == 1);
        [controller voiceProviderSettingsChanged:nil];
        assert(!capture.active);
        [defaults setVolatileDomain:oldArguments forName:NSArgumentDomain];
        controller.usePolishFixture = YES;
        assert(Start(controller, capture, session, NO));
        const auto beforePolishCaptureStops = capture.finishes;
        controller.fixture.failAppend = YES;
        capture.chunk([NSMutableData dataWithLength:32], nil); // A queued send failure after the server final is obsolete.
        controller.fixture.result(@"unpolished", YES, nil);
        controller.fixture.result(@"duplicate final", YES, nil);
        controller.fixture.result(nil, YES, [NSError errorWithDomain:@"synthetic" code:1 userInfo:nil]);
        Pump();
        assert(capture.finishes == beforePolishCaptureStops + 1 && controller.fixture.finishes == 0);
        assert(client.commits.count == 2 && controller.polishFixture.submissions == 1);
        assert([[controller valueForKey:@"voiceOverlay"] phase] == 3);
        assert([presentation.preview isEqual:@"unpolished"] && !client.marked.length);
        assert(presentation.failure == 0);
        controller.polishFixture.completion(@"synthetic polished", nil);
        assert(client.commits.count == 3 && [client.commits.lastObject isEqual:@"synthetic polished"]);
        assert([[controller valueForKey:@"voiceOverlay"] phase] == 0);
        assert(!presentation.preview.length);
        assert(Start(controller, capture, session, YES));
        controller.fixture.result(@"cancelled polish", YES, nil);
        DoubaoPolishFixture *oldPolish = controller.polishFixture;
        [controller cancelDoubaoVoiceInput];
        assert(oldPolish.cancellations == 1);
        assert(Start(controller, capture, session, YES));
        oldPolish.completion(@"late polish", nil);
        assert(capture.active && client.commits.count == 3);
        controller.fixture.result(@"focus moved", YES, nil);
        [controller setValue:[DoubaoTextFixture new] forKey:@"activeClient"];
        controller.polishFixture.completion(@"wrong focus", nil);
        assert(client.commits.count == 3);
        [controller setValue:client forKey:@"activeClient"];
        const NSUInteger beforeShort = client.commits.count, beforeShortFailures = presentation.failures;
        for (NSNumber *duration in @[@0, @0.2499375]) {
            capture.capturedSeconds = duration.doubleValue;
            assert(Start(controller, capture, session, YES));
            DoubaoRequestFixture *shortRequest = controller.fixture;
            shortRequest.result(@"synthetic short partial", NO, nil);
            [controller finishDoubaoVoiceInput];
            shortRequest.result(@"synthetic late final", YES, nil);
            assert(!capture.active && !shortRequest.finishes && shortRequest.cancellations == 1);
            assert(!client.marked.length && client.commits.count == beforeShort && presentation.failures == beforeShortFailures);
            assert(Start(controller, capture, session, NO));
            controller.fixture.result(@"synthetic early final", YES, nil);
            assert(!capture.active && !controller.polishFixture.submissions && client.commits.count == beforeShort);
        }
        capture.capturedSeconds = 0.25;
        controller.usePolishFixture = NO;
        // 语音标点（#10597）：本地模型和豆包共用这条流式路线。省略所有标点时，行内预编辑里带标点的中间结果照原样显示，最终结果去掉标点后替换它上屏。
        ApplyPunctuationPreferences(controller, @"omit_all");
        const NSUInteger beforePunctuated = client.commits.count, beforePunctuatedFailures = presentation.failures;
        assert(Start(controller, capture, session, YES));
        controller.fixture.result(@"天啊，他是谁？", NO, nil);
        assert([client.marked isEqual:@"天啊，他是谁？"]);
        controller.fixture.result(@"天啊，他是谁？", YES, nil);
        assert(client.commits.count == beforePunctuated + 1 && [client.commits.lastObject isEqual:@"天啊他是谁"]);
        assert(!client.marked.length && !capture.active && presentation.failures == beforePunctuatedFailures);
        assert(![controller valueForKey:@"doubaoVoiceRequest"]);
        // 整段只有标点：和空结果一样提示没听到，清掉行内预编辑、停掉录音、放开请求；下一次录音照常开始并上屏，不会卡在识别中。
        assert(Start(controller, capture, session, YES));
        DoubaoRequestFixture *punctuationOnly = controller.fixture;
        punctuationOnly.result(@"。", NO, nil);
        punctuationOnly.result(@"。", YES, nil);
        assert(presentation.failure == MSIMEVoiceFailureNoSpeech && presentation.failures == beforePunctuatedFailures + 1);
        assert(client.commits.count == beforePunctuated + 1 && !client.marked.length && !capture.active);
        assert(![controller valueForKey:@"doubaoVoiceRequest"] && punctuationOnly.cancellations == 1);
        assert(Start(controller, capture, session, YES));
        controller.fixture.result(@"明天见，好吗？", YES, nil);
        assert(client.commits.count == beforePunctuated + 2 && [client.commits.lastObject isEqual:@"明天见好吗"]);
        assert(!capture.active && ![controller valueForKey:@"doubaoVoiceRequest"]);
        ApplyPunctuationPreferences(controller, nil);
        // 录音中和停止采集后重载偏好都只改变下一次识别；当前结果继续使用开始时的方式。
        for (NSNumber *processing in @[@NO, @YES]) {
            ApplyPunctuationPreferences(controller, @"omit_all");
            assert(Start(controller, capture, session, YES));
            if (processing.boolValue) [controller finishDoubaoVoiceInput];
            ApplyPunctuationPreferences(controller, nil);
            controller.fixture.result(@"synthetic。", YES, nil);
            assert([client.commits.lastObject isEqual:@"synthetic"] && !capture.active);
            assert(Start(controller, capture, session, YES));
            controller.fixture.result(@"synthetic。", YES, nil);
            assert([client.commits.lastObject isEqual:@"synthetic。"] && !capture.active);
        }
        controller.usePolishFixture = YES;
        ApplyPunctuationPreferences(controller, @"omit_all");
        assert(Start(controller, capture, session, YES));
        controller.fixture.result(@"synthetic。", YES, nil);
        ApplyPunctuationPreferences(controller, nil);
        controller.polishFixture.completion(@"synthetic。", nil);
        assert([client.commits.lastObject isEqual:@"synthetic"] && !capture.active);
        DoubaoPolishFixture *obsoletePolish = controller.polishFixture;
        const NSUInteger beforeObsolete = client.commits.count;
        assert(Start(controller, capture, session, YES));
        obsoletePolish.completion(@"obsolete。", nil);
        assert(client.commits.count == beforeObsolete && capture.active);
        controller.fixture.result(@"synthetic。", YES, nil);
        controller.polishFixture.completion(@"synthetic。", nil);
        assert(client.commits.count == beforeObsolete + 1 && [client.commits.lastObject isEqual:@"synthetic。"]);
        controller.usePolishFixture = NO;
        for (NSString *mode in @[@"sendinput", @"ctrl_v"]) {
            uint64_t generation = 0;
            assert([capture startWithSession:session generation:&generation error:nil]);
            [controller setValue:@(generation) forKey:@"voiceGeneration"];
            const NSUInteger imk = client.commits.count, external = controller.externalCommits;
            assert(([controller startDoubaoVoiceInputWithOptions:@{@"stream": @YES, @"commit_mode": mode}]));
            assert(![[controller valueForKey:@"doubaoVoiceInline"] boolValue]);
            controller.fixture.result(@"synthetic routed", YES, nil);
            controller.fixture.result(@"synthetic routed", YES, nil);
            assert(client.commits.count == imk && controller.externalCommits == external + 1);
        }
        assert([session closeWithError:nil]);
        assert([NSFileManager.defaultManager removeItemAtPath:root error:nil]);
        assert(cues.starts == cues.stops);
        [cueDefaults setVolatileDomain:oldCueArguments forName:NSArgumentDomain];
    }
}
