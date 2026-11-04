#import "GlobalVoiceSession.h"
#import "VoiceInputService.h"
#import "VoiceProviderOptions.h"
#import "VoiceProviderSocket.h"
#import "LocalVoiceRequest.h"
#import "HTTPVoiceRequest.h"
#import "VoiceWaveOverlay.h"
#import "VoiceCuePlayer.h"
#import "VoiceAudioMuter.h"
#import "VoiceFailureMessages.h"
#import "VoicePCMBuffer.h"
#import "../core/SharedVoicePreferences.h"
#import "../settings/RuntimeOptions.h"
#include "../../../../shared/voice/CaptureDuration.h"
#import "VoiceInputLevel.h"
#import <os/log.h>

@implementation MSIMEGlobalVoiceSession {
    MSIMEClientSession *_session;
    MSIMEVoiceInputService *_service;
    MSIMEVoiceWaveOverlay *_overlay;
    MSIMEVoiceCuePlayer *_cue;
    MSIMEVoiceAudioMuter *_muter;
    MSIMEHTTPVoiceRequest *_http;
    MSIMEHTTPVoiceRequest *_polisher;
    id<MSIMEStreamingVoiceRequest> _stream;
    NSString *_socket;
    NSDictionary *_options;
    id _token;
    id _hostToken;
    uint64_t _generation;
    BOOL _permission;
    BOOL _processing;
    BOOL _final;
    BOOL _ready;
    BOOL _detached;
    BOOL _locked;
}
// 这些边界只在测试中替换；替身不读取安装配置、不打开设备、不发送真实请求。
- (NSUserDefaults *)voiceDefaults { return NSUserDefaults.standardUserDefaults; }
- (NSDictionary *)runtimeOptions { return MSIMELoadRuntimeOptions(); }
- (NSString *)providerSocket { return MSIMEVoiceProviderSocket(); }
- (MSIMEVoiceInputService *)makeService { return [MSIMEVoiceInputService new]; }
- (MSIMEVoiceWaveOverlay *)makeOverlay { return [MSIMEVoiceWaveOverlay new]; }
- (MSIMEVoiceCuePlayer *)makeCue { return [MSIMEVoiceCuePlayer new]; }
- (MSIMEVoiceAudioMuter *)makeMuter { return [MSIMEVoiceAudioMuter new]; }
- (MSIMEClientSession *)makeSession {
    return [[MSIMEClientSession alloc] initWithOptions:[self runtimeOptions] error:nil];
}
- (id<MSIMEStreamingVoiceRequest>)makeStream:(NSDictionary *)options error:(NSError **)error {
    return [options[@"asr_provider"] isEqual:@"local"]
        ? [[MSIMELocalVoiceRequest alloc] initWithOptions:options hostOptions:_session.hostOptions error:error]
        : [[MSIMEDoubaoVoiceRequest alloc] initWithOptions:options error:error];
}
- (MSIMEHTTPVoiceRequest *)makeHTTP:(NSDictionary *)options error:(NSError **)error {
    return [[MSIMEHTTPVoiceRequest alloc] initWithOptions:options error:error];
}
- (MSIMEHTTPVoiceRequest *)makePolisher:(NSDictionary *)options {
    if (!([options[@"polish_enabled"] boolValue] || [options[@"polish_text"] boolValue]) || ![options[@"polish_token"] length]) return nil;
    return [[MSIMEHTTPVoiceRequest alloc] initWithPolishOptions:options error:nil];
}
- (pid_t)globalVoiceTargetPID {
    id focus = MSIMEGlobalVoiceFocusIdentity();
    if (!focus) return 0;
    CFTypeRef role = nullptr;
    AXUIElementCopyAttributeValue((__bridge AXUIElementRef)focus, kAXRoleAttribute, &role);
    BOOL editable = role && (CFEqual(role, kAXTextAreaRole) || CFEqual(role, kAXTextFieldRole) || CFEqual(role, kAXComboBoxRole));
    if (role) CFRelease(role);
    return editable ? NSWorkspace.sharedWorkspace.frontmostApplication.processIdentifier : 0;
}
- (BOOL)globalVoiceBusy { return _token != nil; }
- (BOOL)globalVoiceProcessing { return _processing; }
- (BOOL)captureTargetCurrent {
    id original = MSIMEGlobalVoiceHost().focusIdentity;
    id focus = MSIMEGlobalVoiceFocusIdentity();
    return original && focus && CFEqual((__bridge CFTypeRef)original, (__bridge CFTypeRef)focus);
}
- (BOOL)owns:(id)token {
    return token && _hostToken && _token == token && MSIMEGlobalVoiceHost().owner == self && MSIMEGlobalVoiceHost().sessionToken == _hostToken;
}
- (void)fail:(MSIMEVoiceFailure)failure error:(NSError *)error {
    os_log(OS_LOG_DEFAULT, "MSIME global voice: standalone failure category=%{public}ld", (long)failure);
    [self cancelGlobalVoice];
    [_overlay showFailure:failure detail:MSIMEVoiceFailureDetail(error)];
}
- (void)captureEnded {
    [_muter restore];
    if (_ready && MSIMEVoiceCuesEnabled([self voiceDefaults])) [_cue playStopCue];
    _ready = NO;
}
- (void)receivedLevel:(float)level token:(id)token {
    if (![self owns:token] || _processing) return;
    if (!_ready) {
        _ready = YES;
        [_overlay setListening:YES];
        [_overlay setRecordingLocked:_locked];
        os_log(OS_LOG_DEFAULT, "MSIME global voice: standalone capture ready");
        void (^mute)(void) = nil;
        if (MSIMEVoiceMuteSystemAudioEnabled([self voiceDefaults])) mute = [_muter deferredMute];
        if (MSIMEVoiceCuesEnabled([self voiceDefaults])) [_cue playStartCueThen:mute];
        else if (mute) mute();
    }
    [_overlay setInputLevel:level];
}
- (void)verifyCapture:(id)token {
    if ([self owns:token] && !_processing && !_ready)
        [self fail:MSIMEVoiceFailureCapture error:nil];
}
- (void)scheduleAfter:(NSTimeInterval)delay action:(void (^)(void))action {
    dispatch_after(dispatch_time(DISPATCH_TIME_NOW, (int64_t)(delay * NSEC_PER_SEC)), dispatch_get_main_queue(), action);
}
- (MSIMEGlobalVoiceStartResult)startGlobalVoiceHolding:(BOOL)holding {
    if (_token) return MSIMEGlobalVoiceStartRetry;
    if (!MSIMEVoiceInputEnabled([self voiceDefaults])) return MSIMEGlobalVoiceStartFailed;
    _hostToken = MSIMEGlobalVoiceHost().sessionToken;
    _token = [NSObject new];
    if (![self owns:_token]) { _token = nil; return MSIMEGlobalVoiceStartRetry; }
    _detached = NO; _processing = NO; _final = NO; _ready = NO; _locked = NO;
    _service = [self makeService]; _overlay = [self makeOverlay]; _cue = [self makeCue]; _muter = [self makeMuter];
    _options = MSIMEVoiceQuery([self voiceDefaults], 0, NO);
    _socket = [self providerSocket];
    if (MSIMEVoiceASRTokenMissing(_options[@"asr_provider"], _options[@"asr_token"], _socket.length != 0)) {
        [self fail:MSIMEVoiceFailureMissingToken error:nil]; return MSIMEGlobalVoiceStartFailed;
    }
    if (MSIMEVoiceLocalModelMissing(_options[@"asr_provider"], _socket.length != 0,
        MSIMELocalVoiceModelDirectory(_options[@"asr_model_path"]))) {
        [self fail:MSIMEVoiceFailureMissingLocalModel error:nil]; return MSIMEGlobalVoiceStartFailed;
    }
    // 核心会话跨录音保留，socket 协议的代次随真实 voice start 单调递增。
    if (!_session) _session = [self makeSession];
    if (!_session) { [self fail:MSIMEVoiceFailureSession error:nil]; return MSIMEGlobalVoiceStartFailed; }
    NSString *directory = [self runtimeOptions][@"preferences_directory"];
    NSDictionary *preferences = [directory isKindOfClass:NSString.class] && directory.isAbsolutePath
        ? [MSIMEClientSession loadPreferencesInDirectory:directory error:nil][@"preferences"] : @{};
    [_overlay applyThemePreferences:preferences ?: @{}];
    if (![self captureTargetCurrent] || ![self owns:_token]) { [self cancelGlobalVoice]; return MSIMEGlobalVoiceStartFailed; }
    id token = _token;
    __weak MSIMEGlobalVoiceSession *weakSelf = self;
    _overlay.actionHandler = ^(BOOL cancel) {
        MSIMEGlobalVoiceSession *self = weakSelf;
        if (![self owns:token]) return;
        if (cancel) [self cancelGlobalVoice];
        else if (self->_processing) [self->_overlay dismissProcessing];
        else [self stopGlobalVoice];
    };
    const BOOL native = MSIMEVoiceUsesNativeHTTPProvider(_options[@"asr_provider"], _socket.length != 0) ||
        [_options[@"asr_provider"] isEqual:@"local"] || [_options[@"asr_provider"] isEqual:@"doubao"];
    const BOOL speech = !native && !_socket.length;
    // 权限弹窗可比手势更长；按住说话授权后要求新的手势，不在松手后补开录音。
    if ((speech && _service.speechAuthorizationStatus != SFSpeechRecognizerAuthorizationStatusAuthorized) ||
        _service.microphoneAuthorizationStatus != AVAuthorizationStatusAuthorized) {
        _permission = YES;
        const BOOL requestSpeech = speech && _service.speechAuthorizationStatus != SFSpeechRecognizerAuthorizationStatusAuthorized;
        void (^completion)(BOOL) = ^(BOOL granted) {
            dispatch_async(dispatch_get_main_queue(), ^{
                MSIMEGlobalVoiceSession *self = weakSelf;
                if (![self owns:token] || !self->_permission) return;
                self->_permission = NO;
                if (!granted) [self fail:requestSpeech ? MSIMEVoiceFailureSpeechPermission : MSIMEVoiceFailureMicrophonePermission error:nil];
                else if (holding) [self cancelGlobalVoice];
                else { [self cancelGlobalVoice]; [self startGlobalVoiceHolding:NO]; }
            });
        };
        if (requestSpeech) [_service requestSpeechPermission:completion];
        else [_service requestMicrophonePermission:completion];
        return MSIMEGlobalVoiceStartAccepted;
    }
    if (![_service startWithSession:_session generation:&_generation error:nil]) {
        [self fail:MSIMEVoiceFailureSession error:nil]; return MSIMEGlobalVoiceStartFailed;
    }
    if (![self owns:token] || ![self captureTargetCurrent] || ![self owns:token]) {
        [self cancelGlobalVoice]; return MSIMEGlobalVoiceStartFailed;
    }
    _options = MSIMEVoiceQuery([self voiceDefaults], _generation, NO);
    [self startProvider:token];
    [self scheduleAfter:2 action:^{ [weakSelf verifyCapture:token]; }];
    return _token ? MSIMEGlobalVoiceStartAccepted : MSIMEGlobalVoiceStartFailed;
}
- (void)startProvider:(id)token {
    __weak MSIMEGlobalVoiceSession *weakSelf = self;
    NSString *device = MSIMEVoiceCaptureDeviceUID([self voiceDefaults]);
    NSError *error = nil;
    if (_socket.length) {
        MSIMEClientSession *session = _session; NSDictionary *options = _options; NSString *socket = _socket;
        dispatch_async(dispatch_get_global_queue(QOS_CLASS_USER_INITIATED, 0), ^{
            [session voiceProviderStream:options socket:socket update:^(NSString *text, BOOL final) {
                dispatch_async(dispatch_get_main_queue(), ^{ [weakSelf result:text final:final error:nil token:token]; });
            } phase:^(NSUInteger phase) {
                dispatch_async(dispatch_get_main_queue(), ^{
                    MSIMEGlobalVoiceSession *self = weakSelf;
                    if (![self owns:token] || self->_final) return;
                    if (phase == 0) [self receivedLevel:0 token:token];
                    else { self->_processing = YES; [self captureEnded]; [self->_overlay setProcessing:phase == 2]; }
                });
            } error:nil];
            dispatch_async(dispatch_get_main_queue(), ^{
                MSIMEGlobalVoiceSession *self = weakSelf;
                if ([self owns:token] && !self->_final) [self fail:MSIMEVoiceFailureProvider error:nil];
            });
        });
        return;
    }
    if (MSIMEVoiceUsesNativeHTTPProvider(_options[@"asr_provider"], NO)) {
        _http = [self makeHTTP:_options error:&error];
        if (!_http) { [self fail:MSIMEVoiceFailureProvider error:error]; return; }
        _http.polishingHandler = ^{
            MSIMEGlobalVoiceSession *self = weakSelf;
            if ([self owns:token]) [self->_overlay setProcessing:YES];
        };
        if (![_service startPCMRecording:^(AVAudioPCMBuffer *buffer) {
            const float level = MSIMEVoiceInputLevel(buffer);
            dispatch_async(dispatch_get_main_queue(), ^{
                MSIMEGlobalVoiceSession *self = weakSelf;
                if (![self owns:token] || self->_processing) return;
                [self receivedLevel:level token:token];
                if (self->_service.recordedDuration * 16000 >= self->_http.sampleLimit) [self stopGlobalVoice];
            });
        } deviceUID:device failure:^(NSError *failure) {
            MSIMEGlobalVoiceSession *self = weakSelf;
            if ([self owns:token]) [self fail:MSIMEVoiceFailureCapture error:failure];
        } error:&error]) [self fail:MSIMEVoiceFailureCapture error:error];
        return;
    }
    _polisher = [self makePolisher:_options];
    if ([_options[@"asr_provider"] isEqual:@"local"] || [_options[@"asr_provider"] isEqual:@"doubao"]) {
        _stream = [self makeStream:_options error:&error];
        if (!_stream || ![_stream startWithResult:^(NSString *text, BOOL final, NSError *failure) {
            [weakSelf result:text final:final error:failure token:token];
        } error:&error]) { [self fail:MSIMEVoiceFailureProvider error:error]; return; }
        id<MSIMEStreamingVoiceRequest> stream = _stream;
        if (![self owns:token] || _final) return;
        if (![_service startPCMStreaming:^(NSData *pcm, NSError *failure) {
            NSError *sendError = failure;
            BOOL sent = !failure && pcm && [stream appendPCM:pcm error:&sendError];
            const float level = sent ? msime::voice::input_level(static_cast<const float *>(pcm.bytes), pcm.length / sizeof(float)) : 0;
            dispatch_async(dispatch_get_main_queue(), ^{
                MSIMEGlobalVoiceSession *self = weakSelf;
                if (![self owns:token] || self->_processing) return;
                if (sent) [self receivedLevel:level token:token];
                else [self fail:MSIMEVoiceFailureCapture error:sendError];
            });
        } deviceUID:device error:&error]) [self fail:MSIMEVoiceFailureCapture error:error];
        return;
    }
    if (![_service startTranscriptionWithLanguage:_options[@"language"] textHandler:^(NSString *text, BOOL final) {
        [weakSelf result:text final:final error:nil token:token];
    } error:&error]) { [self fail:MSIMEVoiceFailureProvider error:error]; return; }
    if (![_service startMicrophoneCapture:^(AVAudioPCMBuffer *buffer) {
        const float level = MSIMEVoiceInputLevel(buffer);
        dispatch_async(dispatch_get_main_queue(), ^{ [weakSelf receivedLevel:level token:token]; });
    } deviceUID:device error:&error]) [self fail:MSIMEVoiceFailureCapture error:error];
}
- (void)result:(NSString *)text final:(BOOL)final error:(NSError *)error token:(id)token {
    if (![self owns:token] || _final) return;
    if (error) { [self fail:MSIMEVoiceFailureProvider error:error]; return; }
    if (!final) { if (text.length <= 65536) [_overlay setTranscript:text ?: @""]; return; }
    _final = YES;
    if (!_processing) {
        _processing = YES;
        if (_stream) [_service finishPCMStreamingWithError:nil];
        else [_service stopMicrophoneCapture];
        [self captureEnded];
    }
    if (!_socket.length && msime::voice::short_capture(_service.recordedDuration)) { [self cancelGlobalVoice]; return; }
    if (!text.length || text.length > 65536) { [self fail:MSIMEVoiceFailureNoSpeech error:nil]; return; }
    [_overlay setProcessing:_polisher != nil];
    __weak MSIMEGlobalVoiceSession *weakSelf = self;
    if (_polisher && [_polisher polishText:text completion:^(NSString *polished, NSError *failure) {
        [weakSelf deliver:!failure && polished.length ? polished : text token:token];
    } error:nil]) return;
    [self deliver:text token:token];
}
- (void)deliver:(NSString *)text token:(id)token {
    if (![self owns:token]) return;
    NSDictionary *result = [_session applyVoiceText:text generation:_generation error:nil];
    NSString *commit = result[@"commit"];
    if (!commit.length) { [self fail:MSIMEVoiceFailureSession error:nil]; return; }
    if (_detached || !MSIMEGlobalVoiceHost().deliver) {
        if (MSIMEGlobalVoiceHost().recover) MSIMEGlobalVoiceHost().recover(commit);
        [self cancelGlobalVoice]; return;
    }
    __weak MSIMEGlobalVoiceSession *weakSelf = self;
    MSIMEGlobalVoiceHost().deliver(commit, _options[@"commit_mode"], ^{
        MSIMEGlobalVoiceSession *self = weakSelf;
        if ([self owns:token]) [self cancelGlobalVoice];
    });
}
- (void)stopGlobalVoice {
    if (!_token || _processing) return;
    if (_permission) { [self cancelGlobalVoice]; return; }
    _processing = YES;
    id token = _token;
    NSError *error = nil;
    NSData *pcm = nil;
    if (_http) pcm = [_service finishPCMRecordingWithError:&error];
    else if (_stream) pcm = [_service finishPCMStreamingWithError:&error];
    else [_service stopMicrophoneCapture];
    [self captureEnded]; [_overlay setProcessing:NO];
    if (!_socket.length && msime::voice::short_capture(_service.recordedDuration)) { [self cancelGlobalVoice]; return; }
    __weak MSIMEGlobalVoiceSession *weakSelf = self;
    if (_http) {
        if (!pcm.length || error || ![_http recognizePCM:pcm completion:^(NSString *text, NSError *failure) {
            [weakSelf result:text final:YES error:failure token:token];
        } error:&error]) [self fail:MSIMEVoiceFailureProvider error:error];
    } else if (_stream) {
        if (!pcm || error || (pcm.length && ![_stream appendPCM:pcm error:&error]) || ![_stream finishWithError:&error])
            [self fail:MSIMEVoiceFailureProvider error:error];
    } else if (_socket.length) {
        MSIMEClientSession *session = _session; NSString *socket = _socket; uint64_t generation = _generation;
        dispatch_async(dispatch_get_global_queue(QOS_CLASS_USER_INITIATED, 0), ^{ [session voiceProviderStopSocket:socket generation:generation error:nil]; });
    }
}
- (void)cancelGlobalVoice {
    // fail 已完成回收后，协调器的第二次结束不能顺带隐藏仍需阅读的错误。
    if (!_token) return;
    // 先撤销令牌，停止设备与请求引发的迟到回调无法作用于本轮或下一轮。
    _token = nil; _hostToken = nil; _permission = NO;
    [_http cancel]; [_stream cancel]; [_polisher cancel];
    _http = nil; _stream = nil; _polisher = nil;
    if (_socket.length && _generation) {
        MSIMEClientSession *session = _session; NSString *socket = _socket; uint64_t generation = _generation;
        dispatch_async(dispatch_get_global_queue(QOS_CLASS_USER_INITIATED, 0), ^{ [session voiceProviderCancelSocket:socket generation:generation error:nil]; });
    }
    [_service cancelWithError:nil]; [self captureEnded];
    [_overlay setListening:NO]; _overlay.actionHandler = nil;

    _socket = nil; _options = nil; _processing = NO; _generation = 0;
}
- (void)endGlobalVoice { [self cancelGlobalVoice]; }
- (void)dealloc { [self cancelGlobalVoice]; [_session closeWithError:nil]; }
- (void)lockGlobalVoice { _locked = YES; [_overlay setRecordingLocked:YES]; }
- (void)detachGlobalVoice { _detached = YES; [self stopGlobalVoice]; }
@end
