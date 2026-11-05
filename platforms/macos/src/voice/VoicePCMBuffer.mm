#import "VoicePCMBuffer.h"
#include "../../../../shared/voice/VoiceProviders.h"
#include <cmath>
#include <algorithm>

@implementation MSIMEVoicePCMBuffer {
    AVAudioConverter *_converter;
    NSMutableData *_pcm;
    BOOL _finished;
    BOOL _failed;
    uint64_t _inputFrames;
    double _inputRate;
    // Output samples already drained and released; _pcm holds the samples after them.
    NSUInteger _drainedFrames;
    NSUInteger _sampleLimit;
    uint64_t _inputLimit;
}
- (instancetype)init { return [self initWithSampleLimit:msime::voice::batch_capture_sample_limit]; }
- (instancetype)initWithSampleLimit:(NSUInteger)sampleLimit {
    self = [super init];
    if (self) _sampleLimit = sampleLimit;
    return self;
}
- (NSUInteger)convertedFrames { return _drainedFrames + _pcm.length / sizeof(float); }
- (BOOL)fail:(NSError **)error {
    _failed = YES;
    _pcm = nil;
    _converter = nil;
    if (error) *error = [NSError errorWithDomain:@"app.msime.client.voice" code:3
        userInfo:@{NSLocalizedDescriptionKey: @"录音格式无效"}];
    return NO;
}
- (BOOL)convert:(AVAudioPCMBuffer *)input final:(BOOL)final error:(NSError **)error {
    __block BOOL supplied = NO;
    while (true) {
        AVAudioPCMBuffer *output = [[AVAudioPCMBuffer alloc] initWithPCMFormat:_converter.outputFormat frameCapacity:4096];
        NSError *conversionError = nil;
        AVAudioConverterOutputStatus status = [_converter convertToBuffer:output error:&conversionError
            withInputFromBlock:^AVAudioBuffer *(AVAudioPacketCount count, AVAudioConverterInputStatus *inputStatus) {
                (void)count;
                if (input && !supplied) {
                    supplied = YES;
                    *inputStatus = AVAudioConverterInputStatus_HaveData;
                    return input;
                }
                *inputStatus = final ? AVAudioConverterInputStatus_EndOfStream : AVAudioConverterInputStatus_NoDataNow;
                return nil;
            }];
        if (conversionError || status == AVAudioConverterOutputStatus_Error) return [self fail:error];
        if (output.frameLength) {
            float *samples = output.floatChannelData[0];
            for (AVAudioFrameCount index = 0; index < output.frameLength; ++index) {
                if (!std::isfinite(samples[index])) return [self fail:error];
                // 有界输入也会在重采样滤波时出现短暂越界；限制输出满幅，不能因此取消录音。
                samples[index] = std::clamp(samples[index], -1.0f, 1.0f);
            }
            // The converter can append filter padding at end-of-stream, and the last admitted input buffer can run past the sample limit. Neither is kept.
            NSUInteger converted = [self convertedFrames];
            NSUInteger remaining = converted < _sampleLimit ? _sampleLimit - converted : 0;
            [_pcm appendBytes:samples length:MIN(remaining, output.frameLength) * sizeof(float)];
        }
        if (status == AVAudioConverterOutputStatus_EndOfStream || status == AVAudioConverterOutputStatus_InputRanDry)
            return YES;
        if (!output.frameLength) return [self fail:error];
    }
}
- (BOOL)append:(AVAudioPCMBuffer *)buffer error:(NSError **)error {
    @synchronized(self) {
        if (_finished || _failed) return [self fail:error];
        AVAudioFormat *format = buffer.format;
        if (!buffer || !std::isfinite(format.sampleRate) || format.sampleRate < 8000 ||
            format.sampleRate > 192000 || format.channelCount < 1 || format.channelCount > 8)
            return [self fail:error];
        if (!_converter) {
            AVAudioFormat *target = [[AVAudioFormat alloc] initStandardFormatWithSampleRate:16000 channels:1];
            _converter = [[AVAudioConverter alloc] initFromFormat:format toFormat:target];
            if (!_converter) return [self fail:error];
            _converter.primeMethod = AVAudioConverterPrimeMethod_None;
            _pcm = [NSMutableData data];
            _inputRate = format.sampleRate;
            _inputLimit = _sampleLimit == NSUIntegerMax ? UINT64_MAX
                : (uint64_t)std::ceil(_sampleLimit * _inputRate / 16000.0);
        } else if (![_converter.inputFormat isEqual:format]) {
            return [self fail:error];
        }
        if (!buffer.frameLength) return YES;
        // Past the limit this recording stops taking audio, which bounds memory. The host ends a batch recording as soon as it reaches the provider's limit (InputController startHTTPVoiceInputWithOptions:), so this only drops the tail of the buffer that arrives before that stop lands.
        if (_inputFrames >= _inputLimit) return YES;
        _inputFrames += buffer.frameLength;
        return [self convert:buffer final:NO error:error];
    }
}
- (NSData *)finishWithError:(NSError **)error {
    @synchronized(self) {
        if (_failed) { [self fail:error]; return nil; }
        if (!_finished && _converter && ![self convert:nil final:YES error:error]) return nil;
        if (_inputRate > 0) {
            NSUInteger frames = MIN((NSUInteger)std::llround(_inputFrames * 16000.0 / _inputRate), _sampleLimit);
            if ([self convertedFrames] > frames)
                _pcm.length = frames > _drainedFrames ? (frames - _drainedFrames) * sizeof(float) : 0;
        }
        _finished = YES;
        _converter = nil;
        return [_pcm copy] ?: NSData.data;
    }
}
- (void)cancel {
    @synchronized(self) {
        _failed = YES;
        _pcm = nil;
        _converter = nil;
    }
}
- (NSData *)drainWithError:(NSError **)error {
    @synchronized(self) {
        if (_failed) { [self fail:error]; return nil; }
        NSUInteger available = [self convertedFrames];
        if (!_finished && _inputRate > 0) {
            NSUInteger captured = (NSUInteger)std::floor(_inputFrames * 16000.0 / _inputRate);
            available = MIN(available, captured);
        }
        if (available <= _drainedFrames) return NSData.data;
        const NSRange range = NSMakeRange(0, (available - _drainedFrames) * sizeof(float));
        NSData *result = [_pcm subdataWithRange:range];
        [_pcm replaceBytesInRange:range withBytes:NULL length:0];
        _drainedFrames = available;
        return result;
    }
}
@end
