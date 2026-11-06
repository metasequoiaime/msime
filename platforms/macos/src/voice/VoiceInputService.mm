#import "VoiceInputService.h"
#import "VoicePCMBuffer.h"
#import "VoiceCaptureDevice.h"
#import "../backend/voice/BackendSpeechAnalyzer.h"
#include "../../../../shared/voice/CaptureDuration.h"
#include "../../../../shared/voice/LocalAsr.h"
#import <AVFoundation/AVFoundation.h>
#import <CoreAudio/CoreAudio.h>
#include <memory>
#include <mutex>
#include <atomic>
#include <cmath>
#import <os/log.h>
static BOOL MSIMEVoiceStrictGeneration(id value, uint64_t *result) {
    if (![value isKindOfClass:NSNumber.class] ||
        CFGetTypeID((__bridge CFTypeRef)value) == CFBooleanGetTypeID() ||
        CFNumberIsFloatType((__bridge CFNumberRef)value)) return NO;
    NSNumber *number = (NSNumber *)value;
    if ([number compare:@0] == NSOrderedAscending) return NO;
    uint64_t generation = number.unsignedLongLongValue;
    if ([number compare:@(generation)] != NSOrderedSame) return NO;
    if (result) *result = generation;
    return YES;
}
namespace {
struct PCMStreamAdmission { std::mutex mutex; bool live = true; };
}
@implementation MSIMEVoiceInputService { __weak MSIMEClientSession *_session; BOOL _active; AVAudioEngine *_audioEngine; SFSpeechRecognizer *_recognizer; SFSpeechAudioBufferRecognitionRequest *_speechRequest; SFSpeechRecognitionTask *_speechTask; id<MSIMEBackendSpeechAnalyzerSession> _analyzer; uint64_t _transcriptionGeneration; MSIMEVoicePCMBuffer *_pcmRecording; std::shared_ptr<PCMStreamAdmission> _pcmStreamLive; std::shared_ptr<msime::voice::CaptureDuration> _captureDuration; NSTimeInterval _recordedDuration; id _captureObserver; uint64_t _captureGeneration; BOOL _captureTapInstalled; BOOL _captureRetried; }
- (NSTimeInterval)recordedDuration { return _recordedDuration + (_captureDuration ? _captureDuration->seconds() : 0); }
- (AVAuthorizationStatus)microphoneAuthorizationStatus { return [AVCaptureDevice authorizationStatusForMediaType:AVMediaTypeAudio]; }
- (void)requestMicrophonePermission:(void (^)(BOOL))completion { [AVCaptureDevice requestAccessForMediaType:AVMediaTypeAudio completionHandler:^(BOOL granted) { dispatch_async(dispatch_get_main_queue(), ^{ completion(granted); }); }]; }
- (SFSpeechRecognizerAuthorizationStatus)speechAuthorizationStatus { return [SFSpeechRecognizer authorizationStatus]; }
- (void)requestSpeechPermission:(void (^)(BOOL))completion { [SFSpeechRecognizer requestAuthorization:^(SFSpeechRecognizerAuthorizationStatus status) { dispatch_async(dispatch_get_main_queue(), ^{ completion(status == SFSpeechRecognizerAuthorizationStatusAuthorized); }); }]; }
- (BOOL)isActive { return _active; }
- (BOOL)startPCMRecording:(MSIMEVoiceAudioBuffer)handler deviceUID:(NSString *)deviceUID error:(NSError **)error {
    return [self startPCMRecording:handler deviceUID:deviceUID failure:nil error:error];
}
- (BOOL)startPCMRecording:(MSIMEVoiceAudioBuffer)handler deviceUID:(NSString *)deviceUID failure:(void (^)(NSError *))failure error:(NSError **)error {
    if (_pcmRecording || _audioEngine || _speechTask) {
        if (error) *error = [NSError errorWithDomain:@"app.msime.client.voice" code:4
            userInfo:@{NSLocalizedDescriptionKey: @"录音已在进行中"}];
        return NO;
    }
    MSIMEVoicePCMBuffer *recording = [MSIMEVoicePCMBuffer new];
    _pcmRecording = recording;
    __weak MSIMEVoiceInputService *weakSelf = self;
    auto failed = std::make_shared<std::atomic_bool>(false);
    // Capture this recording, not the mutable service slot: a late callback
    // after stop/cancel must never append to a successor recording.
    BOOL started = [self startMicrophoneCapture:^(AVAudioPCMBuffer *buffer) {
        NSError *conversionError = nil;
        if ([recording append:buffer error:&conversionError]) handler(buffer);
        else if (!failed->exchange(true)) {
            // Never stop AVAudioEngine from inside its capture callback.
            dispatch_async(dispatch_get_main_queue(), ^{
                MSIMEVoiceInputService *service = weakSelf;
                if (!service || service->_pcmRecording != recording) return;
                [service cancelWithError:nil];
                if (failure) failure(conversionError);
            });
        }
    } deviceUID:deviceUID error:error];
    if (!started) { [recording cancel]; _pcmRecording = nil; }
    return started;
}
- (NSData *)finishPCMRecordingWithError:(NSError **)error {
    MSIMEVoicePCMBuffer *recording = _pcmRecording;
    if (!recording) {
        if (error) *error = [NSError errorWithDomain:@"app.msime.client.voice" code:5
            userInfo:@{NSLocalizedDescriptionKey: @"没有可提交的录音"}];
        return nil;
    }
    [self stopPCMStreamDelivery];
    _pcmRecording = nil;
    [self stopMicrophoneCapture];
    return [recording finishWithError:error];
}
- (BOOL)startPCMStreaming:(MSIMEVoicePCMChunk)handler deviceUID:(NSString *)deviceUID error:(NSError **)error {
    if (!handler || _pcmRecording || _audioEngine || _speechTask) {
        if (error) *error = [NSError errorWithDomain:@"app.msime.client.voice" code:4
            userInfo:@{NSLocalizedDescriptionKey:@"无法开始流式录音"}];
        return NO;
    }
    // The stream is not capped, as the MSIME-Windows Doubao client is not: every chunk is drained as it is delivered, so the buffer only ever holds what has not been sent yet.
    MSIMEVoicePCMBuffer *recording = [[MSIMEVoicePCMBuffer alloc] initWithSampleLimit:NSUIntegerMax];
    auto live = std::make_shared<PCMStreamAdmission>();
    _pcmRecording = recording; _pcmStreamLive = live;
    // Both recording and admission token belong to this tap, never its successor.
    BOOL started = [self startMicrophoneCapture:^(AVAudioPCMBuffer *buffer) {
        std::lock_guard<std::mutex> lock(live->mutex);
        if (!live->live) return;
        NSError *failure = nil;
        NSData *pcm = [recording append:buffer error:&failure] ? [recording drainWithError:&failure] : nil;
        if (!pcm) {
            live->live = false; handler(nil, failure);
        } else if (pcm.length) handler(pcm, nil);
    } deviceUID:deviceUID error:error];
    if (!started) { [self stopPCMStreamDelivery]; [recording cancel]; _pcmRecording = nil; }
    return started;
}
- (void)stopPCMStreamDelivery {
    auto live = _pcmStreamLive;
    if (!live) return;
    {
        // Wait for an in-flight drain + delivery before finalizing the converter.
        // Release this lock before removing the tap or stopping AVAudioEngine.
        std::lock_guard<std::mutex> lock(live->mutex);
        live->live = false;
    }
    _pcmStreamLive.reset();
}
- (NSData *)finishPCMStreamingWithError:(NSError **)error {
    MSIMEVoicePCMBuffer *recording = _pcmRecording;
    if (![self finishPCMRecordingWithError:error]) return nil;
    return [recording drainWithError:error];
}
- (BOOL)startMicrophoneCapture:(MSIMEVoiceAudioBuffer)bufferHandler deviceUID:(NSString *)deviceUID error:(NSError **)error {
    if ([self microphoneAuthorizationStatus] != AVAuthorizationStatusAuthorized) { if (error) *error = [NSError errorWithDomain:@"app.msime.client.voice" code:1 userInfo:@{NSLocalizedDescriptionKey: @"麦克风权限未授权"}]; return NO; }
    if (_audioEngine) return YES;
    _recordedDuration = 0;
    _captureDuration.reset();
    _captureRetried = NO;
    _audioEngine = [self makeAudioEngine];
    const uint64_t generation = ++_captureGeneration;
    __weak MSIMEVoiceInputService *weakSelf = self;
    _captureObserver = [NSNotificationCenter.defaultCenter addObserverForName:AVAudioEngineConfigurationChangeNotification
        object:_audioEngine queue:nil usingBlock:^(NSNotification *notification) {
            AVAudioEngine *engine = notification.object;
            // 通知在 AVFoundation 内部队列上到达；退出通知栈后再操作引擎，避免同步拆卸死锁。
            dispatch_async(dispatch_get_main_queue(), ^{
                [weakSelf recoverStartupEngine:engine generation:generation handler:bufferHandler deviceUID:deviceUID];
            });
        }];
    if ([self installCaptureTapOnEngine:_audioEngine handler:bufferHandler deviceUID:deviceUID error:error]) return YES;
    [self clearMicrophoneEngine];
    return NO;
}
- (BOOL)installCaptureTapOnEngine:(AVAudioEngine *)engine handler:(MSIMEVoiceAudioBuffer)bufferHandler
                       deviceUID:(NSString *)deviceUID error:(NSError **)error {
    AVAudioInputNode *input = engine.inputNode;
    if (!MSIMEConfigureVoiceCaptureDevice(deviceUID, input.audioUnit, error)) return NO;
    AVAudioFormat *format = [input inputFormatForBus:0];
    if (!format || !std::isfinite(format.sampleRate) || format.sampleRate <= 0 || !format.channelCount) {
        if (error) *error = [NSError errorWithDomain:@"app.msime.client.voice" code:6
            userInfo:@{NSLocalizedDescriptionKey:@"麦克风音频格式尚未就绪"}];
        return NO;
    }
    SFSpeechAudioBufferRecognitionRequest *speechRequest = _speechRequest;
    id<MSIMEBackendSpeechAnalyzerSession> analyzer = _analyzer;
    auto duration = std::make_shared<msime::voice::CaptureDuration>(format.sampleRate);
    _captureDuration = duration;
    AVAudioNodeTapBlock capture = ^(AVAudioPCMBuffer *buffer, AVAudioTime *time) {
        (void)time;
        if (!duration->append(buffer.frameLength)) return;
        [speechRequest appendAudioPCMBuffer:buffer]; [analyzer appendBuffer:buffer]; bufferHandler(buffer);
    };
    // 当前最低 SDK 的输入节点 tap 没有 NSError 重载；只在这一处抑制旧接口警告。
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wdeprecated-declarations"
    [input installTapOnBus:0 bufferSize:1024 format:format block:capture];
#pragma clang diagnostic pop
    _captureTapInstalled = YES;
    if ([engine startAndReturnError:error]) return YES;
    _recordedDuration += duration->finish(); _captureDuration.reset();
    [input removeTapOnBus:0]; _captureTapInstalled = NO;
    return NO;
}
- (void)recoverStartupEngine:(AVAudioEngine *)engine generation:(uint64_t)generation
                    handler:(MSIMEVoiceAudioBuffer)handler deviceUID:(NSString *)deviceUID {
    if (_audioEngine != engine || _captureGeneration != generation || _captureRetried ||
        engine.running || self.recordedDuration > 0) return;
    // 仅恢复同一轮、首帧之前被系统停掉的引擎，最多一次；结束或新一轮的通知都不能补开麦克风。
    _captureRetried = YES;
    if (_captureDuration) { _recordedDuration += _captureDuration->finish(); _captureDuration.reset(); }
    if (_captureTapInstalled) { [engine.inputNode removeTapOnBus:0]; _captureTapInstalled = NO; }
    // 保留识别请求和 PCM 流，按硬件的新格式重装 tap；不能借 stopMicrophoneCapture 提前结束识别。
    const BOOL recovered = [self installCaptureTapOnEngine:engine handler:handler deviceUID:deviceUID error:nil];
    os_log(OS_LOG_DEFAULT, "MSIME voice capture: startup recovery succeeded=%{public}d", recovered);
    // 恢复失败保持无帧状态，由全局首帧超时显示错误；不循环重试或跳过用户的松手/取消。
}
- (AVAudioEngine *)makeAudioEngine { return [AVAudioEngine new]; }
- (void)clearMicrophoneEngine {
    ++_captureGeneration;
    if (_captureObserver) { [NSNotificationCenter.defaultCenter removeObserver:_captureObserver]; _captureObserver = nil; }
    if (_captureDuration) { _recordedDuration += _captureDuration->finish(); _captureDuration.reset(); }
    if (!_audioEngine) return;
    if (_captureTapInstalled) { [_audioEngine.inputNode removeTapOnBus:0]; _captureTapInstalled = NO; }
    [_audioEngine stop]; _audioEngine = nil;
}
- (void)stopMicrophoneCapture {
    const BOOL captured = _audioEngine != nil;
    [self clearMicrophoneEngine];
    if (captured) { [_speechRequest endAudio]; [_analyzer finishAudio]; }
}
- (BOOL)startTranscriptionWithLanguage:(NSString *)language textHandler:(void (^)(NSString *, BOOL))handler error:(NSError **)error {
    if ([SFSpeechRecognizer authorizationStatus] != SFSpeechRecognizerAuthorizationStatusAuthorized) { if (error) *error = [NSError errorWithDomain:@"app.msime.client.voice" code:2 userInfo:@{NSLocalizedDescriptionKey: @"语音识别权限未授权"}]; return NO; }
    [self stopTranscription];
    __weak MSIMEVoiceInputService *weakSelf = self;
    const uint64_t generation = _transcriptionGeneration;
    // macOS 26 and later: SpeechAnalyzer, fully on device, once the locale's model is installed. Its handler already runs on main. SpeechTranscriber can leave a space before Chinese punctuation ("调整 ，"), so its text goes through the same transcript rules as the local models'.
    _analyzer = MSIMEStartBackendSpeechAnalyzer(language ?: @"zh-CN", ^(NSString *text, BOOL final) {
        MSIMEVoiceInputService *service = weakSelf;
        if (!service || service->_transcriptionGeneration != generation) return;
        handler(@(msime::voice::tidy_local_transcript(text.UTF8String ?: "").c_str()), final);
        if (final && service->_transcriptionGeneration == generation) [service stopTranscription];
    });
    if (_analyzer) return YES;
    _recognizer = [[SFSpeechRecognizer alloc] initWithLocale:[[NSLocale alloc] initWithLocaleIdentifier:language ?: @"zh-CN"]];
    _speechRequest = [[SFSpeechAudioBufferRecognitionRequest alloc] init];
    // Keep the audio on this Mac whenever the locale has an on-device model; only a locale without one still goes to Apple's servers.
    if (_recognizer.supportsOnDeviceRecognition) _speechRequest.requiresOnDeviceRecognition = YES;
    _speechTask = [_recognizer recognitionTaskWithRequest:_speechRequest resultHandler:^(SFSpeechRecognitionResult *result, NSError *recognitionError) {
        // Speech can deliver a queued result after cancellation. Serialize with
        // host lifecycle operations and never let an old task affect its successor.
        dispatch_async(dispatch_get_main_queue(), ^{
            MSIMEVoiceInputService *service = weakSelf;
            if (!service || service->_transcriptionGeneration != generation) return;
            if (result) handler(result.bestTranscription.formattedString, result.isFinal);
            // Empty final is a terminal failure signal, never a partial commit.
            if (recognitionError && !result.isFinal && service->_transcriptionGeneration == generation)
                handler(@"", YES);
            // The handler may itself stop or replace the recognition task.
            if (service->_transcriptionGeneration == generation && (recognitionError || result.isFinal))
                [service stopTranscription];
        });
    }];
    return _speechTask != nil;
}
- (void)stopTranscription { ++_transcriptionGeneration; [_speechTask cancel]; _speechTask = nil; _speechRequest = nil; _recognizer = nil; [_analyzer cancel]; _analyzer = nil; }
- (BOOL)startWithSession:(MSIMEClientSession *)session generation:(uint64_t *)generation error:(NSError **)error { if (_active) return YES; NSDictionary *result = [session startVoiceWithError:error]; if (!result) return NO; uint64_t admittedGeneration = 0; if (!MSIMEVoiceStrictGeneration(result[@"generation"], &admittedGeneration)) return NO; _session = session; _active = YES; if (generation) *generation = admittedGeneration; return YES; }
- (BOOL)cancelWithError:(NSError **)error { [self stopPCMStreamDelivery]; [_pcmRecording cancel]; _pcmRecording = nil; if (!_active) { [self stopMicrophoneCapture]; [self stopTranscription]; return YES; } BOOL ok = [_session cancelVoiceWithError:error]; [self stopMicrophoneCapture]; [self stopTranscription]; _active = NO; _session = nil; return ok; }
- (void)applyText:(NSString *)text generation:(uint64_t)generation completion:(MSIMEVoiceInputResult)completion {
    MSIMEClientSession *session = _session;
    dispatch_async(dispatch_get_main_queue(), ^{
        NSError *error = nil;
        NSDictionary *result = [session applyVoiceText:text generation:generation error:&error];
        completion(result, error);
    });
}
- (void)dealloc { [self stopPCMStreamDelivery]; [_pcmRecording cancel]; [self stopMicrophoneCapture]; [self stopTranscription]; }
@end
