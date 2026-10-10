#import "../../src/input/InputController.mm"
#import "VoiceCueFixture.h"
#import "VoiceClientFixture.h"
#import "VoiceMeterFixture.h"
#include <cassert>

@interface HTTPRequestFixture : NSObject
@property(copy) void (^polishingHandler)(void);
@property(copy) void (^completion)(NSString *, NSError *);
@property NSUInteger cancellations;
@property BOOL submitted;
@property NSUInteger sampleLimit;
- (BOOL)recognizePCM:(NSData *)pcm completion:(void (^)(NSString *, NSError *))completion error:(NSError **)error;
- (void)cancel;
@end
@implementation HTTPRequestFixture
- (instancetype)init { self = [super init]; if (self) _sampleLimit = NSUIntegerMax; return self; }
- (BOOL)recognizePCM:(NSData *)pcm completion:(void (^)(NSString *, NSError *))completion error:(NSError **)error {
    (void)error; assert(pcm.length == 640); self.submitted = YES; self.completion = completion; return YES;
}
- (void)cancel { ++self.cancellations; }
@end
@interface HTTPCaptureFixture : NSObject
@property(copy) MSIMEVoiceAudioBuffer bufferHandler;
@property NSTimeInterval capturedSeconds;
@property(getter=isActive) BOOL active;
@property BOOL failStart;
@property NSUInteger cancellations;
@property(copy) void (^failure)(NSError *);
- (BOOL)startPCMRecording:(MSIMEVoiceAudioBuffer)handler deviceUID:(NSString *)device failure:(void (^)(NSError *))failure error:(NSError **)error;
- (NSData *)finishPCMRecordingWithError:(NSError **)error;
- (BOOL)cancelWithError:(NSError **)error;
@end
@implementation HTTPCaptureFixture
- (NSTimeInterval)recordedDuration { return self.capturedSeconds; }
- (BOOL)startPCMRecording:(MSIMEVoiceAudioBuffer)handler deviceUID:(NSString *)device failure:(void (^)(NSError *))failure error:(NSError **)error {
    (void)device; (void)error; self.bufferHandler = handler; self.failure = failure; self.active = !self.failStart; return self.active;
}
- (NSData *)finishPCMRecordingWithError:(NSError **)error { (void)error; return [NSMutableData dataWithLength:640]; }
- (BOOL)cancelWithError:(NSError **)error { (void)error; self.active = NO; ++self.cancellations; return YES; }
@end
@interface HTTPHostFixture : NSObject
@property NSUInteger submissions;
- (NSDictionary *)applyVoiceText:(NSString *)text generation:(uint64_t)generation error:(NSError **)error;
@end
@implementation HTTPHostFixture
- (NSDictionary *)setFocused:(BOOL)focused error:(NSError **)error { (void)focused; (void)error; return @{}; }
// A client switch hands the new app's punctuation and width to the session; this fixture has no Engine view to change.
- (NSDictionary *)setChinesePunctuationEnabled:(BOOL)enabled error:(NSError **)error { (void)enabled; (void)error; return nil; }
- (NSDictionary *)setPairedPunctuationEnabled:(BOOL)enabled error:(NSError **)error { (void)enabled; (void)error; return nil; }
- (NSDictionary *)setPunctuationLock:(NSString *)lock error:(NSError **)error { (void)lock; (void)error; return nil; }
- (NSDictionary *)setCharacterWidthFull:(BOOL)full error:(NSError **)error { (void)full; (void)error; return nil; }
- (BOOL)setCapsLockEnabled:(BOOL)enabled error:(NSError **)error { (void)enabled; (void)error; return YES; }
// 换客户端获得焦点后控制器会读一次打字特效设置（429ce0d01）；这个夹具没有特效可给。
- (NSDictionary *)typingEffectSettingsWithError:(NSError **)error { (void)error; return nil; }
- (NSDictionary *)applyVoiceText:(NSString *)text generation:(uint64_t)generation error:(NSError **)error {
    (void)error; assert(NSThread.isMainThread && generation == 42 && [text isEqual:@"synthetic"]);
    ++self.submissions; return @{@"commit": text};
}
@end
@interface HTTPControllerFixture : MSIMEInputController
@property HTTPRequestFixture *requestFixture;
@property NSUInteger applies;
@property NSUInteger imkCommits;
@property NSUInteger externalCommits;
@property MSIMEVoiceCommitOutcome commitOutcome;
@property(copy) NSString *commitMode;
@property NSUInteger requestSampleLimit;
@end
@implementation HTTPControllerFixture
- (void)ensureAppearance {}
- (MSIMEHTTPVoiceRequest *)makeHTTPVoiceRequest:(NSDictionary *)options error:(NSError **)error {
    (void)options; (void)error; self.requestFixture = [HTTPRequestFixture new];
    if (self.requestSampleLimit) self.requestFixture.sampleLimit = self.requestSampleLimit;
    return (id)self.requestFixture;
}
- (void)apply:(NSDictionary *)result { if (result[@"commit"]) ++self.imkCommits; ++self.applies; }
- (MSIMEVoiceCommitOutcome)postVoiceText:(NSString *)text route:(const MSIMEVoiceCommitRoute &)route {
    assert([text isEqual:@"synthetic"]); self.commitMode = route.mode;
    ++self.externalCommits; return self.commitOutcome;
}
@end

@interface HTTPOverlayFixture : NSObject
@property(copy) NSString *detail;
// The controller picks the overlay screen from the caret on every failure; model the real overlay property so the assignment lands somewhere.
@property(nonatomic, weak) NSScreen *preferredScreen;
@property float lastLevel;
@property NSUInteger levelUpdates;
@property(copy) void (^actionHandler)(BOOL);
@property BOOL dismissed;
@property NSUInteger phase;
@property NSUInteger failure;
@property NSUInteger failures;
@end
@implementation HTTPOverlayFixture
- (void)setInputLevel:(float)level { self.lastLevel = level; ++self.levelUpdates; }
- (void)dismissProcessing { self.dismissed = YES; }
- (void)setListening:(BOOL)listening { self.phase = listening ? 1 : 0; self.failure = 0; }
- (void)showFailure:(MSIMEVoiceFailure)failure detail:(NSString *)detail { self.failure = failure; self.detail = detail; self.phase = 4; ++self.failures; }
- (void)dismissFailure { if (self.failure) [self setListening:NO]; }
- (void)setProcessing:(BOOL)polishing { self.phase = polishing ? 3 : 2; }
@end

// Records the audible order of cues and system-audio mute changes on a synthetic output device.
namespace {
NSMutableArray<NSString *> *audioEvents;
UInt32 outputMuted;
OSStatus OrderGet(AudioObjectID object, const AudioObjectPropertyAddress *address,
    UInt32 qualifierSize, const void *qualifier, UInt32 *size, void *data) {
    (void)object; (void)qualifierSize; (void)qualifier; (void)size;
    if (address->mSelector == kAudioHardwarePropertyDefaultOutputDevice ||
        address->mSelector == kAudioHardwarePropertyTranslateUIDToDevice) *static_cast<AudioDeviceID *>(data) = 7;
    else if (address->mSelector == kAudioDevicePropertyDeviceUID)
        *static_cast<CFStringRef *>(data) = static_cast<CFStringRef>(CFBridgingRetain(@"synthetic-http-output"));
    else *static_cast<UInt32 *>(data) = outputMuted;
    return noErr;
}
OSStatus OrderSet(AudioObjectID object, const AudioObjectPropertyAddress *address,
    UInt32 qualifierSize, const void *qualifier, UInt32 size, const void *data) {
    (void)object; (void)address; (void)qualifierSize; (void)qualifier; (void)size;
    outputMuted = *static_cast<const UInt32 *>(data);
    [audioEvents addObject:outputMuted ? @"mute" : @"unmute"];
    return noErr;
}
}
@interface HTTPOrderedCueFixture : MSIMEVoiceCueFixture
@end
@implementation HTTPOrderedCueFixture
- (void)playStartCueThen:(void (^)(void))completion { [audioEvents addObject:@"start"]; [super playStartCueThen:completion]; }
- (void)playStopCue { [audioEvents addObject:@"stop"]; [super playStopCue]; }
@end

int main() {
    @autoreleasepool {
        HTTPControllerFixture *controller = [HTTPControllerFixture alloc];
        HTTPOverlayFixture *overlay = [HTTPOverlayFixture new];
        [controller setValue:overlay forKey:@"voiceOverlay"];
        MSIMEVoiceCueFixture *cues = [MSIMEVoiceCueFixture new];
        [controller setValue:cues forKey:@"voiceCuePlayer"];
        NSUserDefaults *defaults = NSUserDefaults.standardUserDefaults;
        NSDictionary *oldArguments = [defaults volatileDomainForName:NSArgumentDomain];
        [defaults setVolatileDomain:@{@"MSIMEClientVoiceSoundEnabled": @YES, @"MSIMEClientVoiceStartSound": @YES, @"MSIMEClientVoiceEndSound": @YES} forName:NSArgumentDomain];
        HTTPCaptureFixture *capture = [HTTPCaptureFixture new];
        capture.capturedSeconds = 0.25;
        HTTPHostFixture *session = [HTTPHostFixture new];
        NSObject *client = [MSIMEVoiceClientFixture new];
        [controller setValue:capture forKey:@"voiceService"];
        [controller setValue:session forKey:@"session"];
        [controller setValue:client forKey:@"activeClient"];
        [controller setValue:@42 forKey:@"voiceGeneration"];
        assert([controller startHTTPVoiceInputWithOptions:@{}]);
        assert(cues.starts == 1 && cues.stops == 0);
        MSIMEVoiceAudioBuffer oldMeter = capture.bufferHandler;
        oldMeter(MSIMEVoiceMeterFixtureBuffer()); MSIMEVoiceMeterFixturePump();
        assert(overlay.lastLevel > 0.5f && overlay.lastLevel < 0.7f && overlay.levelUpdates == 1);
        for (NSString *field in @[@"activeClient", @"session", @"voiceGeneration"]) {
            id original = [controller valueForKey:field];
            [controller setValue:[field isEqual:@"voiceGeneration"] ? @43 : [MSIMEVoiceClientFixture new] forKey:field];
            oldMeter(MSIMEVoiceMeterFixtureBuffer()); MSIMEVoiceMeterFixturePump();
            assert(overlay.levelUpdates == 1);
            [controller setValue:original forKey:field];
        }
        [controller finishVoiceInputForDisable];
        oldMeter(MSIMEVoiceMeterFixtureBuffer()); MSIMEVoiceMeterFixturePump();
        assert(overlay.levelUpdates == 1); // A queued meter cannot change processing presentation.
        [controller finishVoiceInputForDisable];
        assert(controller.requestFixture.submitted && !controller.requestFixture.cancellations);
        assert(overlay.phase == 2);
        controller.requestFixture.polishingHandler();
        assert(overlay.phase == 3);
        controller.requestFixture.completion(@"synthetic", nil);
        controller.requestFixture.completion = nil;
        assert(session.submissions == 1 && controller.applies == 1 && !capture.active);
        controller.requestFixture.polishingHandler();
        assert(overlay.phase == 0);
        assert(cues.starts == 1 && cues.stops == 1);
        // Exercise all controller identity checks with deliberately late results.
        for (NSString *field in @[@"activeClient", @"session", @"voiceGeneration"]) {
            assert([controller startHTTPVoiceInputWithOptions:@{}]);
            [controller finishHTTPVoiceInput];
            id original = [controller valueForKey:field];
            [controller setValue:[field isEqual:@"voiceGeneration"] ? @43 : [MSIMEVoiceClientFixture new] forKey:field];
            capture.bufferHandler(MSIMEVoiceMeterFixtureBuffer()); MSIMEVoiceMeterFixturePump();
            assert(overlay.levelUpdates == 1);
            controller.requestFixture.polishingHandler();
            assert(overlay.phase == 2);
            controller.requestFixture.completion(@"synthetic", nil);
            controller.requestFixture.completion = nil;
            [controller setValue:original forKey:field];
            assert(session.submissions == 1 && controller.applies == 1);
        }
        assert([controller startHTTPVoiceInputWithOptions:@{}]);
        [controller finishHTTPVoiceInput];
        HTTPRequestFixture *old = controller.requestFixture;
        [controller finishHTTPVoiceInput]; // A second stop while processing cancels.
        assert(old.cancellations == 1);
        assert([controller startHTTPVoiceInputWithOptions:@{}]);
        old.completion(@"synthetic", nil); old.completion = nil;
        old.polishingHandler();
        assert(overlay.phase == 0);
        assert(capture.active && controller.requestFixture.cancellations == 0);
        [controller cancelHTTPVoiceInput];
        capture.failStart = YES;
        const auto beforeFailure = cues.starts;
        assert(![controller startHTTPVoiceInputWithOptions:@{}]);
        assert(overlay.failure == MSIMEVoiceFailureCapture);
        assert(cues.starts == beforeFailure && cues.stops == beforeFailure);
        assert(controller.requestFixture.cancellations == 1);
        capture.failStart = NO;
        assert([controller startHTTPVoiceInputWithOptions:@{}]);
        void (^oldFailure)(NSError *) = capture.failure;
        oldFailure([NSError errorWithDomain:@"synthetic" code:1 userInfo:nil]);
        assert(!capture.active && controller.requestFixture.cancellations == 1 && !controller.requestFixture.submitted);
        assert(overlay.failure == MSIMEVoiceFailureCapture);
        assert([controller startHTTPVoiceInputWithOptions:@{}]);
        oldFailure([NSError errorWithDomain:@"synthetic" code:1 userInfo:nil]);
        assert(capture.active && controller.requestFixture.cancellations == 0);
        [controller cancelHTTPVoiceInput];
        assert(cues.starts == cues.stops);
        const NSUInteger beforeErrors = overlay.failures;
        assert([controller startHTTPVoiceInputWithOptions:@{}]);
        [controller finishHTTPVoiceInput];
        controller.requestFixture.completion(nil, [NSError errorWithDomain:@"synthetic" code:1
            userInfo:@{NSLocalizedDescriptionKey:@"synthetic detail must not be presented"}]);
        assert(overlay.failure == MSIMEVoiceFailureProvider && overlay.failures == beforeErrors + 1 && !capture.active);
        assert(!overlay.detail); // Only the voice requests' own detail reaches the overlay.
        controller.requestFixture.completion(nil, nil);
        assert(overlay.failures == beforeErrors + 1);
        assert([controller startHTTPVoiceInputWithOptions:@{}]);
        [controller finishHTTPVoiceInput];
        controller.requestFixture.completion(nil, [NSError errorWithDomain:@"app.msime.client.voice" code:6
            userInfo:@{NSLocalizedFailureReasonErrorKey:@"语音识别失败：synthetic provider message"}]);
        assert(overlay.failure == MSIMEVoiceFailureProvider && [overlay.detail isEqual:@"语音识别失败：synthetic provider message"]);
        assert([controller startHTTPVoiceInputWithOptions:@{}]);
        [controller finishHTTPVoiceInput];
        controller.requestFixture.completion(@"", nil);
        assert(overlay.failure == MSIMEVoiceFailureNoSpeech && !capture.active);
        assert([controller startHTTPVoiceInputWithOptions:@{}]);
        [controller finishHTTPVoiceInput];
        [controller setValue:[MSIMEVoiceClientFixture new] forKey:@"activeClient"];
        const NSUInteger beforeStale = overlay.failures;
        controller.requestFixture.completion(nil, [NSError errorWithDomain:@"synthetic" code:1 userInfo:nil]);
        assert(overlay.failures == beforeStale && !capture.active);
        [controller setValue:client forKey:@"activeClient"];
        // Overlay confirmation stops capture once, then only dismisses processing.
        assert([controller startHTTPVoiceInputWithOptions:@{}]);
        void (^oldAction)(BOOL) = overlay.actionHandler;
        overlay.actionHandler(NO);
        assert(controller.requestFixture.submitted && !controller.requestFixture.cancellations);
        overlay.actionHandler(NO);
        assert(overlay.dismissed && capture.active && !controller.requestFixture.cancellations);
        NSUInteger beforeConfirm = session.submissions;
        controller.requestFixture.completion(@"synthetic", nil);
        assert(session.submissions == beforeConfirm + 1 && !capture.active);
        assert([controller startHTTPVoiceInputWithOptions:@{}]);
        oldAction(YES); oldAction(NO);
        assert(capture.active && !controller.requestFixture.submitted && !controller.requestFixture.cancellations);
        // Focus, session and generation mismatches make both buttons inert.
        for (NSString *field in @[@"activeClient", @"session", @"voiceGeneration"]) {
            id original = [controller valueForKey:field];
            [controller setValue:[field isEqual:@"voiceGeneration"] ? @43 : [MSIMEVoiceClientFixture new] forKey:field];
            overlay.actionHandler(YES); overlay.actionHandler(NO);
            assert(capture.active && !controller.requestFixture.submitted);
            [controller setValue:original forKey:field];
        }
        overlay.actionHandler(NO);
        HTTPRequestFixture *cancelled = controller.requestFixture;
        overlay.actionHandler(YES);
        cancelled.completion(@"synthetic", nil); cancelled.completion = nil;
        assert(!capture.active && session.submissions == beforeConfirm + 1);
        const NSUInteger beforeShort = session.submissions, beforeShortFailures = overlay.failures;
        for (NSNumber *duration in @[@0, @0.2499375]) {
            capture.capturedSeconds = duration.doubleValue;
            assert([controller startHTTPVoiceInputWithOptions:@{}]);
            overlay.actionHandler(NO);
            assert(!capture.active && !controller.requestFixture.submitted && controller.requestFixture.cancellations == 1);
            assert(session.submissions == beforeShort && overlay.failures == beforeShortFailures);
        }
        capture.capturedSeconds = 0.25;
        assert([controller startHTTPVoiceInputWithOptions:@{}]);
        [controller finishHTTPVoiceInput]; assert(controller.requestFixture.submitted);
        controller.requestFixture.completion(@"synthetic", nil);
        assert(session.submissions == beforeShort + 1 && cues.starts == cues.stops);
        for (NSString *mode in @[@"sendinput", @"ctrl_v"]) {
            for (auto outcome : {MSIMEVoiceCommitOutcome::posted, MSIMEVoiceCommitOutcome::unavailable, MSIMEVoiceCommitOutcome::stale}) {
                controller.commitOutcome = outcome;
                const NSUInteger external = controller.externalCommits, imk = controller.imkCommits;
                NSMutableDictionary *options = [@{@"commit_mode": mode} mutableCopy];
                assert([controller startHTTPVoiceInputWithOptions:options]);
                options[@"commit_mode"] = @"tsf";
                [controller finishHTTPVoiceInput];
                controller.requestFixture.completion(@"synthetic", nil);
                controller.requestFixture.completion(@"synthetic", nil);
                assert(controller.externalCommits == external + 1 && [controller.commitMode isEqual:mode]);
                assert(controller.imkCommits == imk + (outcome == MSIMEVoiceCommitOutcome::unavailable ? 1 : 0));
            }
        }
        // Capturing as much as the provider takes ends the recording the way a release does: 识别中 shows, the end cue plays, and what was kept is submitted once.
        controller.requestSampleLimit = 16000 * 60;
        capture.capturedSeconds = 59.9;
        assert([controller startHTTPVoiceInputWithOptions:@{}]);
        const NSUInteger stopsBeforeLimit = cues.stops, submissionsBeforeLimit = session.submissions;
        capture.bufferHandler(MSIMEVoiceMeterFixtureBuffer()); MSIMEVoiceMeterFixturePump();
        assert(!controller.requestFixture.submitted && capture.active && cues.stops == stopsBeforeLimit);
        capture.capturedSeconds = 60;
        capture.bufferHandler(MSIMEVoiceMeterFixtureBuffer()); MSIMEVoiceMeterFixturePump();
        assert(controller.requestFixture.submitted && overlay.phase == 2 && cues.stops == stopsBeforeLimit + 1);
        capture.bufferHandler(MSIMEVoiceMeterFixtureBuffer()); MSIMEVoiceMeterFixturePump();
        assert(!controller.requestFixture.cancellations); // A meter queued behind the stop cannot stop it again, which would cancel.
        controller.requestFixture.completion(@"synthetic", nil);
        assert(session.submissions == submissionsBeforeLimit + 1 && !capture.active);
        controller.requestSampleLimit = 0;
        capture.capturedSeconds = 0.25;
        // IMK may deliver a new client's event before deactivateServer for the old one.
        // Revoke both active capture and pending recognition immediately.
        for (NSNumber *processing in @[@NO, @YES]) {
            [controller setValue:client forKey:@"activeClient"];
            assert([controller startHTTPVoiceInputWithOptions:@{}]);
            HTTPRequestFixture *departing = controller.requestFixture;
            MSIMEVoiceAudioBuffer oldBuffer = capture.bufferHandler;
            if (processing.boolValue) [controller finishHTTPVoiceInput];
            NSObject *successor = [MSIMEVoiceClientFixture new];
            NSEvent *event = [NSEvent keyEventWithType:NSEventTypeFlagsChanged location:NSZeroPoint modifierFlags:0 timestamp:1 windowNumber:0 context:nil characters:@"" charactersIgnoringModifiers:@"" isARepeat:NO keyCode:56];
            const NSUInteger submissions = session.submissions;
            assert(![controller handleEvent:event client:successor]);
            assert(!capture.active && departing.cancellations == 1);
            assert([controller valueForKey:@"activeClient"] == successor);
            assert([controller startHTTPVoiceInputWithOptions:@{}]);
            [controller deactivateServer:client]; // Delayed old-client deactivation must not cancel the successor.
            assert(![controller handleEvent:event client:successor]);
            const NSUInteger levels = overlay.levelUpdates;
            oldBuffer(MSIMEVoiceMeterFixtureBuffer()); MSIMEVoiceMeterFixturePump();
            departing.polishingHandler();
            if (departing.completion) departing.completion(@"synthetic", nil);
            assert(capture.active && !controller.requestFixture.cancellations);
            assert(session.submissions == submissions && overlay.levelUpdates == levels && overlay.phase != 3);
            [controller cancelHTTPVoiceInput];
        }
        // Windows plays the start cue before muting other audio and unmutes before the end cue. The macOS mute covers the whole output device, so it must wait for the start cue to finish.
        [controller setValue:client forKey:@"activeClient"];
        audioEvents = [NSMutableArray array];
        HTTPOrderedCueFixture *ordered = [HTTPOrderedCueFixture new];
        [controller setValue:ordered forKey:@"voiceCuePlayer"];
        [controller setValue:[[MSIMEVoiceAudioMuter alloc] initWithAudioAPI:{OrderGet, OrderSet, nullptr, nullptr}] forKey:@"voiceAudioMuter"];
        [defaults setVolatileDomain:@{@"MSIMEClientVoiceSoundEnabled": @YES, @"MSIMEClientVoiceStartSound": @YES, @"MSIMEClientVoiceEndSound": @YES, @"MSIMEClientVoiceMuteSystemAudio": @YES} forName:NSArgumentDomain];
        assert([controller startHTTPVoiceInputWithOptions:@{}]);
        assert([audioEvents isEqual:@[@"start"]] && !outputMuted && ordered.startCompletion);
        ordered.startCompletion();
        assert([audioEvents isEqual:(@[@"start", @"mute"])] && outputMuted);
        [controller finishHTTPVoiceInput];
        assert([audioEvents isEqual:(@[@"start", @"mute", @"unmute", @"stop"])] && !outputMuted);
        controller.requestFixture.completion(@"synthetic", nil);
        // Stopping before the start cue ends leaves other audio untouched; its late completion cannot mute.
        [audioEvents removeAllObjects];
        assert([controller startHTTPVoiceInputWithOptions:@{}]);
        void (^late)(void) = ordered.startCompletion;
        [controller cancelHTTPVoiceInput];
        late();
        assert([audioEvents isEqual:(@[@"start", @"stop"])] && !outputMuted);
        // A failed start never mutes either.
        [audioEvents removeAllObjects];
        capture.failStart = YES;
        assert(![controller startHTTPVoiceInputWithOptions:@{}] && !audioEvents.count && !outputMuted);
        capture.failStart = NO;
        // Without a start cue the mute follows the start of capture directly.
        [defaults setVolatileDomain:@{@"MSIMEClientVoiceSoundEnabled": @YES, @"MSIMEClientVoiceStartSound": @NO, @"MSIMEClientVoiceEndSound": @YES, @"MSIMEClientVoiceMuteSystemAudio": @YES} forName:NSArgumentDomain];
        assert([controller startHTTPVoiceInputWithOptions:@{}]);
        assert([audioEvents isEqual:@[@"mute"]] && outputMuted);
        [controller cancelHTTPVoiceInput];
        assert([audioEvents isEqual:(@[@"mute", @"unmute", @"stop"])] && !outputMuted);
        [defaults setVolatileDomain:oldArguments forName:NSArgumentDomain];
    }
}
