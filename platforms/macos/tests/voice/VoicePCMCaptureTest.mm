#import "../../src/voice/VoiceInputService.h"
#include "../../../../shared/voice/CaptureDuration.h"
#include <cassert>
#include <limits>
#include <memory>

static void Drain() {
    __block BOOL done = NO;
    dispatch_async(dispatch_get_main_queue(), ^{ done = YES; });
    while (!done) [NSRunLoop.currentRunLoop runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.001]];
}

@interface SyntheticCapture : MSIMEVoiceInputService
@property(copy) MSIMEVoiceAudioBuffer capture;
@property(copy) NSString *device;
@property BOOL failStart;
@property NSUInteger stops;
@end
@interface DurationInput : NSObject
@property(copy) AVAudioNodeTapBlock tap;
@property AVAudioFormat *format;
@end
@implementation DurationInput
- (AudioUnit)audioUnit { return nullptr; }
- (AVAudioFormat *)inputFormatForBus:(AVAudioNodeBus)bus {
    (void)bus; return self.format ?: [[AVAudioFormat alloc] initStandardFormatWithSampleRate:48000 channels:1];
}
- (void)installTapOnBus:(AVAudioNodeBus)bus bufferSize:(AVAudioFrameCount)size format:(AVAudioFormat *)format block:(AVAudioNodeTapBlock)tap {
    (void)bus; (void)size; (void)format; self.tap = tap;
}
- (void)installTapOnBus:(AVAudioNodeBus)bus bufferSize:(AVAudioFrameCount)size format:(AVAudioFormat *)format error:(NSError **)error block:(AVAudioNodeTapBlock)tap {
    (void)error; [self installTapOnBus:bus bufferSize:size format:format block:tap];
}
- (void)removeTapOnBus:(AVAudioNodeBus)bus { (void)bus; }
@end
@interface DurationEngine : NSObject
@property DurationInput *inputNode;
@property BOOL failStart;
@property(getter=isRunning) BOOL running;
@property NSUInteger starts;
@end
@implementation DurationEngine
- (BOOL)startAndReturnError:(NSError **)error { (void)error; ++self.starts; self.running = !self.failStart; return self.running; }
- (void)stop { self.running = NO; }
@end
@interface DurationCapture : MSIMEVoiceInputService
@property DurationEngine *engine;
@property BOOL failStart;
@end
@implementation DurationCapture
- (AVAuthorizationStatus)microphoneAuthorizationStatus { return AVAuthorizationStatusAuthorized; }
- (AVAudioEngine *)makeAudioEngine {
    self.engine = [DurationEngine new]; self.engine.inputNode = [DurationInput new]; self.engine.failStart = self.failStart; return (id)self.engine;
}
@end
@implementation SyntheticCapture
- (BOOL)startMicrophoneCapture:(MSIMEVoiceAudioBuffer)handler deviceUID:(NSString *)device error:(NSError **)error {
    (void)error;
    self.capture = handler; self.device = device;
    return !self.failStart;
}
- (void)stopMicrophoneCapture { ++self.stops; self.capture = nil; }
@end
int main() {
    @autoreleasepool {
        for (double rate : {16000.0, 44100.0, 48000.0, 96000.0}) {
            auto previous = std::make_shared<msime::voice::CaptureDuration>(rate);
            assert(msime::voice::short_capture(previous->seconds()));
            assert(previous->append(static_cast<uint64_t>(rate / 4) - 1));
            assert(msime::voice::short_capture(previous->seconds()));
            assert(previous->append(1));
            assert(previous->finish() == 0.25 && !msime::voice::short_capture(previous->seconds()));
            msime::voice::CaptureDuration next(rate);
            assert(!previous->append(1000) && next.seconds() == 0 && previous->seconds() == 0.25);
        }
        assert(msime::voice::short_capture(std::numeric_limits<double>::quiet_NaN()));
        AVAudioFormat *format = [[AVAudioFormat alloc] initStandardFormatWithSampleRate:48000 channels:1];
        AVAudioPCMBuffer *buffer = [[AVAudioPCMBuffer alloc] initWithPCMFormat:format frameCapacity:4800];
        buffer.frameLength = 4800;
        for (NSUInteger i = 0; i < 4800; ++i) buffer.floatChannelData[0][i] = 0.125f;
        SyntheticCapture *service = [SyntheticCapture new];
        __block NSUInteger callbacks = 0;
        MSIMEVoiceAudioBuffer observe = ^(AVAudioPCMBuffer *value) { assert(value == buffer); ++callbacks; };
        assert([service startPCMRecording:observe deviceUID:@"synthetic-device" error:nil]);
        assert([service.device isEqual:@"synthetic-device"]);
        assert(![service startPCMRecording:observe deviceUID:nil error:nil]);
        MSIMEVoiceAudioBuffer old = service.capture;
        old(buffer);
        NSData *pcm = [service finishPCMRecordingWithError:nil];
        assert(pcm.length == 1600 * sizeof(float) && callbacks == 1 && service.stops == 1);
        assert(![service finishPCMRecordingWithError:nil]);
        assert([service startPCMRecording:observe deviceUID:nil error:nil]);
        old(buffer); // Late audio must neither alter the successor nor notify its UI.
        assert(callbacks == 1);
        service.capture(buffer);
        assert([[service finishPCMRecordingWithError:nil] isEqual:pcm]);
        assert([service startPCMRecording:observe deviceUID:nil error:nil]);
        old = service.capture;
        old(buffer);
        assert([service cancelWithError:nil]);
        old(buffer);
        assert(callbacks == 3 && ![service finishPCMRecordingWithError:nil]);
        service.failStart = YES;
        assert(![service startPCMRecording:observe deviceUID:nil error:nil]);
        assert(![service finishPCMRecordingWithError:nil]);
        service.failStart = NO;
        assert([service startPCMRecording:observe deviceUID:nil error:nil]);
        assert([service finishPCMRecordingWithError:nil].length == 0);
        __block NSUInteger failures = 0;
        void (^failure)(NSError *) = ^(NSError *error) { assert(NSThread.isMainThread && error); ++failures; };
        assert([service startPCMRecording:observe deviceUID:nil failure:failure error:nil]);
        buffer.floatChannelData[0][0] = std::numeric_limits<float>::quiet_NaN();
        old = service.capture;
        NSUInteger stopped = service.stops;
        old(buffer); old(buffer);
        assert(failures == 0 && service.stops == stopped);
        Drain();
        assert(failures == 1 && service.stops == stopped + 1 && ![service finishPCMRecordingWithError:nil]);
        assert([service startPCMRecording:observe deviceUID:nil failure:failure error:nil]);
        service.capture(buffer); // Queue an error, then replace the recording.
        assert([service cancelWithError:nil]);
        assert([service startPCMRecording:observe deviceUID:nil failure:failure error:nil]);
        stopped = service.stops;
        Drain();
        assert(failures == 1 && service.stops == stopped);
        buffer.floatChannelData[0][0] = 0.125f;
        // Past the old 60 s cut the recording neither fails nor stops: all of it is submitted.
        for (NSUInteger i = 0; i < 601; ++i) service.capture(buffer);
        Drain();
        assert(failures == 1 && service.stops == stopped);
        assert([service finishPCMRecordingWithError:nil].length == 601 * 1600 * sizeof(float));
        assert(service.stops == stopped + 1);
        {
            DurationCapture *startup = [DurationCapture new];
            __block NSUInteger firstFrames = 0;
            assert([startup startMicrophoneCapture:^(AVAudioPCMBuffer *) { ++firstFrames; } deviceUID:nil error:nil]);
            DurationEngine *engine = startup.engine;
            AVAudioNodeTapBlock initialTap = engine.inputNode.tap;
            [NSNotificationCenter.defaultCenter postNotificationName:AVAudioEngineConfigurationChangeNotification object:engine];
            Drain();
            assert(engine.starts == 1); // 仍在运行的配置通知不能打断正常采集。
            engine.running = NO;
            engine.inputNode.format = [[AVAudioFormat alloc] initStandardFormatWithSampleRate:44100 channels:1];
            [NSNotificationCenter.defaultCenter postNotificationName:AVAudioEngineConfigurationChangeNotification object:engine];
            [NSNotificationCenter.defaultCenter postNotificationName:AVAudioEngineConfigurationChangeNotification object:engine];
            Drain();
            assert(engine.starts == 2 && engine.running); // 同一批通知只恢复一次。
            initialTap(buffer, [AVAudioTime timeWithSampleTime:0 atRate:48000]);
            assert(firstFrames == 0 && startup.recordedDuration == 0);
            AVAudioPCMBuffer *changed = [[AVAudioPCMBuffer alloc] initWithPCMFormat:engine.inputNode.format frameCapacity:4410];
            changed.frameLength = 4410;
            engine.inputNode.tap(changed, [AVAudioTime timeWithSampleTime:0 atRate:44100]);
            assert(firstFrames == 1 && startup.recordedDuration == 0.1); // 新硬件格式重新计时。
            [startup stopMicrophoneCapture];
        }
        {
            DurationCapture *startup = [DurationCapture new];
            assert([startup startMicrophoneCapture:^(AVAudioPCMBuffer *) {} deviceUID:nil error:nil]);
            DurationEngine *oldEngine = startup.engine;
            oldEngine.running = NO;
            [NSNotificationCenter.defaultCenter postNotificationName:AVAudioEngineConfigurationChangeNotification object:oldEngine];
            [startup cancelWithError:nil];
            assert([startup startMicrophoneCapture:^(AVAudioPCMBuffer *) {} deviceUID:nil error:nil]);
            DurationEngine *next = startup.engine;
            Drain();
            assert(oldEngine.starts == 1 && !oldEngine.running && next.starts == 1 && next.running);
            [startup stopMicrophoneCapture];
            [NSNotificationCenter.defaultCenter postNotificationName:AVAudioEngineConfigurationChangeNotification object:next];
            Drain();
            assert(next.starts == 1 && !next.running); // 松手、取消和旧一轮通知都不能补开麦克风。
        }
        {
            DurationCapture *startup = [DurationCapture new];
            assert([startup startMicrophoneCapture:^(AVAudioPCMBuffer *) {} deviceUID:nil error:nil]);
            DurationEngine *engine = startup.engine;
            engine.running = NO; engine.failStart = YES;
            [NSNotificationCenter.defaultCenter postNotificationName:AVAudioEngineConfigurationChangeNotification object:engine];
            Drain();
            assert(engine.starts == 2 && !engine.running);
            [NSNotificationCenter.defaultCenter postNotificationName:AVAudioEngineConfigurationChangeNotification object:engine];
            Drain();
            assert(engine.starts == 2 && startup.recordedDuration == 0); // 恢复失败不能无限循环。
            [startup stopMicrophoneCapture];
        }
        {
            DurationCapture *startup = [DurationCapture new];
            assert([startup startMicrophoneCapture:^(AVAudioPCMBuffer *) {} deviceUID:nil error:nil]);
            DurationEngine *engine = startup.engine;
            engine.inputNode.tap(buffer, [AVAudioTime timeWithSampleTime:0 atRate:48000]);
            engine.running = NO;
            [NSNotificationCenter.defaultCenter postNotificationName:AVAudioEngineConfigurationChangeNotification object:engine];
            Drain();
            assert(engine.starts == 1 && startup.recordedDuration == 0.1); // 已有录音不作为新的启动重试。
            [startup stopMicrophoneCapture];
        }
        // Exercise the production tap wiring without opening an audio device.
        DurationCapture *durationService = [DurationCapture new];
        __block NSUInteger delivered = 0;
        MSIMEVoiceAudioBuffer durationObserve = ^(AVAudioPCMBuffer *) { ++delivered; };
        assert([durationService startMicrophoneCapture:durationObserve deviceUID:nil error:nil]);
        AVAudioNodeTapBlock previousTap = durationService.engine.inputNode.tap;
        AVAudioTime *timestamp = [AVAudioTime timeWithSampleTime:0 atRate:48000];
        previousTap(buffer, timestamp);
        assert(durationService.recordedDuration == 0.1);
        [durationService stopMicrophoneCapture];
        previousTap(buffer, timestamp);
        assert(durationService.recordedDuration == 0.1 && delivered == 1);
        assert([durationService startMicrophoneCapture:durationObserve deviceUID:nil error:nil]);
        assert(durationService.recordedDuration == 0);
        previousTap(buffer, timestamp);
        assert(durationService.recordedDuration == 0 && delivered == 1);
        durationService.engine.inputNode.tap(buffer, timestamp);
        durationService.engine.inputNode.tap(buffer, timestamp);
        assert(durationService.recordedDuration == 0.2 && delivered == 3);
        [durationService stopMicrophoneCapture];
        durationService.failStart = YES;
        assert(![durationService startMicrophoneCapture:durationObserve deviceUID:nil error:nil]);
        durationService.engine.inputNode.tap(buffer, timestamp);
        assert(durationService.recordedDuration == 0 && delivered == 3);
    }
}
