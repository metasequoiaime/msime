#import "../../src/voice/GlobalVoiceSession.h"
#import "../../src/voice/VoiceInputService.h"
#import "../../src/voice/VoiceWaveOverlay.h"
#import "../../src/voice/DoubaoVoiceRequest.h"
#import "../../src/voice/HTTPVoiceRequest.h"
#include <cassert>
#import "../../src/voice/VoiceProviderSocket.h"
#import "../settings/TestPreferenceSuite.h"

@interface MSIMEGlobalVoiceSession (Test)
- (void)verifyCapture:(id)token;
@end
@interface CaptureService : MSIMEVoiceInputService
@property(nonatomic) NSUInteger opens;
@property(nonatomic) AVAuthorizationStatus microphone;
@property(nonatomic) SFSpeechRecognizerAuthorizationStatus speech;
@property(nonatomic, copy) void (^permissionReply)(BOOL);
@property(nonatomic, copy) MSIMEVoicePCMChunk pcm;
@property(nonatomic, copy) MSIMEVoiceAudioBuffer raw;
@property(nonatomic, copy) void (^transcription)(NSString *, BOOL);
@property(nonatomic, copy) void (^onStart)(void);
@end
@implementation CaptureService
- (BOOL)startWithSession:(MSIMEClientSession *)session generation:(uint64_t *)generation error:(NSError **)error {
    BOOL started = [super startWithSession:session generation:generation error:error];
    if (_onStart) _onStart();
    return started;
}
- (AVAuthorizationStatus)microphoneAuthorizationStatus { return _microphone; }
- (SFSpeechRecognizerAuthorizationStatus)speechAuthorizationStatus { return _speech; }
- (void)requestMicrophonePermission:(void (^)(BOOL))reply { _permissionReply = reply; }
- (void)requestSpeechPermission:(void (^)(BOOL))reply { _permissionReply = reply; }
- (NSTimeInterval)recordedDuration { return 1; }
- (BOOL)startPCMStreaming:(MSIMEVoicePCMChunk)handler deviceUID:(NSString *)uid error:(NSError **)error {
    (void)uid; (void)error; ++_opens; _pcm = handler; return YES;
}
- (NSData *)finishPCMStreamingWithError:(NSError **)error { (void)error; return NSData.data; }
- (BOOL)startPCMRecording:(MSIMEVoiceAudioBuffer)handler deviceUID:(NSString *)uid failure:(void (^)(NSError *))failure error:(NSError **)error {
    (void)uid; (void)failure; (void)error; ++_opens; _raw = handler; return YES;
}
- (NSData *)finishPCMRecordingWithError:(NSError **)error { (void)error; return [NSMutableData dataWithLength:64]; }
- (BOOL)startTranscriptionWithLanguage:(NSString *)language textHandler:(void (^)(NSString *, BOOL))handler error:(NSError **)error {
    (void)language; (void)error; _transcription = handler; return YES;
}
- (BOOL)startMicrophoneCapture:(MSIMEVoiceAudioBuffer)handler deviceUID:(NSString *)uid error:(NSError **)error {
    (void)uid; (void)error; ++_opens; _raw = handler; return YES;
}
@end
@interface CaptureOverlay : NSObject
@property(nonatomic, copy) void (^actionHandler)(BOOL);
@property(nonatomic) BOOL listening;
@property(nonatomic) BOOL locked;
@property(nonatomic) NSUInteger failures;
@property(nonatomic) NSUInteger starts;
@property(nonatomic) BOOL failureVisible;
@end
@implementation CaptureOverlay
- (void)setListening:(BOOL)value { _failureVisible = NO; _listening = value; if (value) ++_starts; }
- (void)setRecordingLocked:(BOOL)value { _locked = value; }
- (void)setInputLevel:(float)value { (void)value; }
- (void)setProcessing:(BOOL)value { (void)value; }
- (void)setTranscript:(NSString *)value { (void)value; }
- (void)applyThemePreferences:(NSDictionary *)value { (void)value; }
- (void)showFailure:(MSIMEVoiceFailure)failure detail:(NSString *)detail { (void)failure; (void)detail; ++_failures; _failureVisible = YES; }
- (void)dismissProcessing {}
@end
@interface CaptureStream : NSObject <MSIMEStreamingVoiceRequest>
@property(nonatomic, copy) MSIMEDoubaoResult reply;
@property(nonatomic) NSUInteger finishes;
@property(nonatomic) NSUInteger cancels;
@end
@implementation CaptureStream
- (BOOL)startWithResult:(MSIMEDoubaoResult)reply error:(NSError **)error { (void)error; _reply = reply; return YES; }
- (BOOL)appendPCM:(NSData *)pcm error:(NSError **)error { (void)pcm; (void)error; return YES; }
- (BOOL)finishWithError:(NSError **)error { (void)error; ++_finishes; return YES; }
- (void)cancel { ++_cancels; }
@end
@interface CaptureHTTP : NSObject
@property(nonatomic, copy) void (^polishingHandler)(void);
@property(nonatomic, copy) void (^reply)(NSString *, NSError *);
@end
@implementation CaptureHTTP
- (NSUInteger)sampleLimit { return 1600000; }
- (BOOL)recognizePCM:(NSData *)pcm completion:(void (^)(NSString *, NSError *))reply error:(NSError **)error {
    (void)error; assert(pcm.length); _reply = reply; return YES;
}
- (BOOL)polishText:(NSString *)text completion:(void (^)(NSString *, NSError *))reply error:(NSError **)error {
    (void)error; assert(text.length); _reply = reply; return YES;
}
- (void)cancel {}
@end
@interface SocketSession : MSIMEClientSession
@property(atomic, copy) NSDictionary *lastQuery;
@property(atomic) uint64_t lastCancellation;
@end
@implementation SocketSession
- (BOOL)voiceProviderStream:(NSDictionary *)query socket:(NSString *)socket update:(MSIMEVoiceProviderUpdate)update
                     phase:(MSIMEVoiceProviderPhase)phase error:(NSError **)error {
    (void)socket; (void)error; self.lastQuery = query; phase(0); update(@"synthetic result", YES); return YES;
}
- (BOOL)voiceProviderCancelSocket:(NSString *)socket generation:(uint64_t)generation error:(NSError **)error {
    (void)socket; (void)error; self.lastCancellation = generation; return YES;
}
@end
@interface CaptureSession : MSIMEGlobalVoiceSession
@property(nonatomic) NSUserDefaults *defaults;
@property(nonatomic) NSDictionary *configuration;
@property(nonatomic) CaptureService *capture;
@property(nonatomic) CaptureOverlay *display;
@property(nonatomic) CaptureStream *request;
@property(nonatomic) CaptureHTTP *http;
@property(nonatomic) CaptureHTTP *polisher;
@property(nonatomic) NSString *socket;
@property(nonatomic) BOOL targetCurrent;
@property(nonatomic) NSMutableArray<NSDictionary *> *scheduled;
@end
@implementation CaptureSession
- (void)scheduleAfter:(NSTimeInterval)delay action:(void (^)(void))action {
    if (!_scheduled) _scheduled = [NSMutableArray new];
    [_scheduled addObject:@{@"delay":@(delay), @"action":[action copy]}];
}
- (NSUserDefaults *)voiceDefaults { return _defaults; }
- (NSDictionary *)runtimeOptions { return _configuration; }
- (NSString *)providerSocket { return _socket; }
- (MSIMEClientSession *)makeSession {
    return [[(_socket ? SocketSession.class : MSIMEClientSession.class) alloc] initWithOptions:_configuration error:nil];
}
- (MSIMEVoiceInputService *)makeService { return _capture; }
- (MSIMEVoiceWaveOverlay *)makeOverlay { return (id)_display; }
- (id)makeCue { return nil; }
- (id)makeMuter { return nil; }
- (id)makePolisher:(NSDictionary *)options { (void)options; return _polisher; }
- (id<MSIMEStreamingVoiceRequest>)makeStream:(NSDictionary *)options error:(NSError **)error {
    (void)options; (void)error; return _request;
}
- (MSIMEHTTPVoiceRequest *)makeHTTP:(NSDictionary *)options error:(NSError **)error {
    (void)options; (void)error; return (id)_http;
}
@end
static void Pump() { [NSRunLoop.currentRunLoop runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.02]]; }
static void Bind(CaptureSession *session) {
    MSIMEGlobalVoiceHost().owner = session;
    MSIMEGlobalVoiceHost().sessionToken = [NSObject new];
}
int main() {
    @autoreleasepool {
        NSString *root = [NSTemporaryDirectory() stringByAppendingPathComponent:NSUUID.UUID.UUIDString].stringByResolvingSymlinksInPath;
        NSMutableDictionary *options = [@{@"api_version":@1, @"preferences":@{@"scheme":@"quanpin", @"learning":@NO}} mutableCopy];
        for (NSString *key in @[@"resources", @"user_data", @"cache", @"dictionaries"]) {
            NSString *path = [root stringByAppendingPathComponent:key];
            assert([NSFileManager.defaultManager createDirectoryAtPath:path withIntermediateDirectories:YES attributes:nil error:nil]);
            options[key] = path;
        }
        NSString *model = [root stringByAppendingPathComponent:@"model"];
        assert([NSFileManager.defaultManager createDirectoryAtPath:model withIntermediateDirectories:YES attributes:nil error:nil]);
        assert([@"{}" writeToFile:[model stringByAppendingPathComponent:@"msime-model.json"] atomically:YES encoding:NSUTF8StringEncoding error:nil]);
        NSString *suite = [@"msime.synthetic.global-capture." stringByAppendingString:NSUUID.UUID.UUIDString];
        NSUserDefaults *defaults = [[NSUserDefaults alloc] initWithSuiteName:suite];
        [defaults setBool:NO forKey:@"MSIMEClientVoiceSoundEnabled"];
        [defaults setBool:NO forKey:@"MSIMEClientVoiceMuteSystemAudio"];
        [defaults setObject:model forKey:@"MSIMEClientVoiceASRModelPath"];
        [defaults setObject:@"synthetic-token" forKey:@"MSIMEClientVoiceASRToken"];
        auto Make = [&] {
            CaptureSession *s = [CaptureSession new]; s.defaults = defaults; s.configuration = options;
            s.targetCurrent = YES;
            s.capture = [CaptureService new]; s.capture.microphone = AVAuthorizationStatusAuthorized;
            s.capture.speech = SFSpeechRecognizerAuthorizationStatusAuthorized;
            s.display = [CaptureOverlay new]; s.request = [CaptureStream new]; s.http = [CaptureHTTP new];
            Bind(s); return s;
        };
        __block NSUInteger deliveries = 0;
        MSIMEGlobalVoiceHost().deliver = ^(NSString *text, NSString *mode, void (^done)(void)) {
            assert([text isEqual:@"synthetic result"] && [mode isEqual:@"tsf"]); ++deliveries; done();
        };
        // 三条进程内通道都无需 IMK 客户端；首帧才显示波形，最终结果经同一个交付边界。
        for (NSString *provider in @[@"local", @"doubao", @"openai", @"system"]) {
            [defaults setObject:provider forKey:@"MSIMEClientVoiceASRProvider"];
            CaptureSession *s = Make();
            assert([s startGlobalVoiceHolding:YES] == MSIMEGlobalVoiceStartAccepted);
            assert(s.capture.opens == 1 && !s.display.listening);
            AVAudioFormat *format = [[AVAudioFormat alloc] initStandardFormatWithSampleRate:16000 channels:1];
            AVAudioPCMBuffer *buffer = [[AVAudioPCMBuffer alloc] initWithPCMFormat:format frameCapacity:16]; buffer.frameLength = 16;
            if (s.capture.pcm) s.capture.pcm([NSMutableData dataWithLength:64], nil);
            else s.capture.raw(buffer);
            Pump(); assert(s.display.listening);
            [s lockGlobalVoice]; assert(s.display.locked);
            [s stopGlobalVoice]; assert([s globalVoiceProcessing]);
            if ([provider isEqual:@"openai"]) s.http.reply(@"synthetic result", nil);
            else if ([provider isEqual:@"system"]) s.capture.transcription(@"synthetic result", YES);
            else s.request.reply(@"synthetic result", YES, nil);
            assert(![s globalVoiceBusy] && !s.display.listening);
            [s endGlobalVoice];
        }
        assert(deliveries == 4);
        [defaults setObject:@"doubao" forKey:@"MSIMEClientVoiceASRProvider"];
        {
            CaptureSession *s = Make();
            __weak CaptureSession *weak = s;
            s.capture.onStart = ^{ weak.targetCurrent = NO; };
            assert([s startGlobalVoiceHolding:YES] == MSIMEGlobalVoiceStartAccepted);
            assert(s.capture.opens == 1 && [s globalVoiceBusy]);
            [s cancelGlobalVoice];
        }
        {
            CaptureSession *s = Make(); s.capture.microphone = AVAuthorizationStatusNotDetermined;
            assert([s startGlobalVoiceHolding:YES] == MSIMEGlobalVoiceStartAccepted && s.capture.opens == 0);
            void (^late)(BOOL) = s.capture.permissionReply;
            [s stopGlobalVoice]; s.capture.microphone = AVAuthorizationStatusAuthorized; late(YES); Pump();
            assert(s.capture.opens == 0 && ![s globalVoiceBusy]);
        }
        {
            CaptureSession *s = Make();
            assert([s startGlobalVoiceHolding:YES] == MSIMEGlobalVoiceStartAccepted);
            MSIMEDoubaoResult late = s.request.reply;
            uint64_t firstGeneration = [[s valueForKey:@"generation"] unsignedLongLongValue];
            [s cancelGlobalVoice]; Bind(s);
            assert([s startGlobalVoiceHolding:YES] == MSIMEGlobalVoiceStartAccepted);
            assert([[s valueForKey:@"generation"] unsignedLongLongValue] > firstGeneration);
            late(@"synthetic result", YES, nil);
            assert(deliveries == 4 && [s globalVoiceBusy]);
            [s cancelGlobalVoice];
        }
        {
            CaptureSession *s = Make(); s.socket = @"/synthetic/provider.sock";
            const uint64_t otherEntryLease = MSIMENextVoiceSocketLease();
            assert([s startGlobalVoiceHolding:YES] == MSIMEGlobalVoiceStartAccepted);
            const uint64_t lease = [[s valueForKey:@"socketLease"] unsignedLongLongValue];
            assert(lease > otherEntryLease);
            SocketSession *transport = (id)[s valueForKey:@"session"];
            for (NSUInteger attempt = 0; attempt < 50 && [s globalVoiceBusy]; ++attempt) Pump();
            assert(![s globalVoiceBusy] && s.capture.opens == 0 && s.display.starts == 1 && deliveries == 5);
            for (NSUInteger attempt = 0; attempt < 50 && !transport.lastCancellation; ++attempt) Pump();
            assert([transport.lastQuery[@"generation"] unsignedLongLongValue] == lease && transport.lastCancellation == lease);
        }
        {
            CaptureSession *s = Make(); s.polisher = [CaptureHTTP new];
            assert([s startGlobalVoiceHolding:YES] == MSIMEGlobalVoiceStartAccepted);
            s.request.reply(@"synthetic result", YES, nil);
            assert([s globalVoiceProcessing] && s.polisher.reply);
            void (^late)(NSString *, NSError *) = s.polisher.reply;
            [s cancelGlobalVoice]; late(@"synthetic result", nil);
            assert(deliveries == 5 && ![s globalVoiceBusy]);
        }
        {
            // 系统语音先授权 Speech、再授权麦克风：第一张弹窗的重复回调不得消费第二张。
            [defaults setObject:@"system" forKey:@"MSIMEClientVoiceASRProvider"];
            CaptureSession *s = Make(); s.capture.speech = SFSpeechRecognizerAuthorizationStatusNotDetermined;
            s.capture.microphone = AVAuthorizationStatusNotDetermined;
            assert([s startGlobalVoiceHolding:NO] == MSIMEGlobalVoiceStartAccepted);
            void (^first)(BOOL) = s.capture.permissionReply;
            s.capture.speech = SFSpeechRecognizerAuthorizationStatusAuthorized; first(YES); Pump();
            void (^second)(BOOL) = s.capture.permissionReply;
            first(YES); Pump(); assert(s.capture.opens == 0);
            s.capture.microphone = AVAuthorizationStatusAuthorized; second(YES); Pump();
            assert(s.capture.opens == 1 && [s globalVoiceBusy]);
            [s cancelGlobalVoice];
        }
        {
            [defaults setObject:@"doubao" forKey:@"MSIMEClientVoiceASRProvider"];
            CaptureSession *s = Make();
            assert([s startGlobalVoiceHolding:YES] == MSIMEGlobalVoiceStartAccepted);
            __block BOOL recovered = NO;
            MSIMEGlobalVoiceHost().recover = ^(NSString *text) { recovered = [text isEqual:@"synthetic result"]; };
            [s detachGlobalVoice]; s.request.reply(@"synthetic result", YES, nil);
            assert(recovered && deliveries == 5 && ![s globalVoiceBusy]);
            MSIMEGlobalVoiceHost().recover = nil;
        }
        {
            CaptureSession *s = Make();
            assert([s startGlobalVoiceHolding:YES] == MSIMEGlobalVoiceStartAccepted);
            id token = [s valueForKey:@"token"];
            [s verifyCapture:token]; assert(s.display.failures == 1 && ![s globalVoiceBusy]);
            [s endGlobalVoice]; [s endGlobalVoice];
            assert(s.display.failureVisible);
            Bind(s); assert([s startGlobalVoiceHolding:YES] == MSIMEGlobalVoiceStartAccepted);
            [s verifyCapture:token]; assert(s.display.failures == 1 && [s globalVoiceBusy]);
            [s cancelGlobalVoice];
        }
        {
            // 提供商仍可处理有效的长录音和慢响应，协调层不另设更短的整场期限。
            [defaults setObject:@"openai" forKey:@"MSIMEClientVoiceASRProvider"];
            CaptureSession *s = Make();
            assert([s startGlobalVoiceHolding:YES] == MSIMEGlobalVoiceStartAccepted);
            [s setValue:@YES forKey:@"ready"];
            NSArray *timers = [s.scheduled copy]; [s.scheduled removeAllObjects];
            for (NSDictionary *timer in timers) ((void (^)(void))timer[@"action"])();
            assert([s globalVoiceBusy]);
            [s stopGlobalVoice];
            timers = [s.scheduled copy]; [s.scheduled removeAllObjects];
            for (NSDictionary *timer in timers) ((void (^)(void))timer[@"action"])();
            assert([s globalVoiceBusy] && [s globalVoiceProcessing]);
            s.http.reply(@"synthetic result", nil);
            assert(![s globalVoiceBusy] && deliveries == 6);
        }
        MSIMEGlobalVoiceHost().owner = nil; MSIMEGlobalVoiceHost().sessionToken = nil;
        MSIMEGlobalVoiceHost().deliver = nil;
        MSIMERemoveTestPreferenceSuite(defaults, suite);
        assert([NSFileManager.defaultManager removeItemAtPath:root error:nil]);
    }
}
