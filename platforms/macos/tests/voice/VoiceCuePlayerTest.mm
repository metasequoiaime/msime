#import "../../src/voice/VoiceCuePlayer.h"
#import <AVFoundation/AVFoundation.h>
#import <AppKit/AppKit.h>
#import <CoreAudio/CoreAudio.h>
#include <cassert>
#include <cstdio>

namespace {
// 默认输出设备同时带输入流时（Depot 的 macOS 虚拟机只有一个收放一体的 Apple Virtual Sound Device），coreaudiod 在开始播放前会按调用方的责任进程检查麦克风授权。授权还没决定过，它就弹授权框并同步等回应，NSSound play 跟着卡住；CI 上没人点，就一直卡到超时。这里只读 CoreAudio 属性、只做 TCC 预检，两者都不会弹框。
bool PlaybackWaitsOnMicrophonePrompt() {
    AudioObjectPropertyAddress output = {kAudioHardwarePropertyDefaultOutputDevice, kAudioObjectPropertyScopeGlobal, kAudioObjectPropertyElementMain};
    AudioObjectID device = kAudioObjectUnknown;
    UInt32 size = sizeof(device);
    if (AudioObjectGetPropertyData(kAudioObjectSystemObject, &output, 0, nullptr, &size, &device) != noErr || device == kAudioObjectUnknown) return false;
    AudioObjectPropertyAddress inputs = {kAudioDevicePropertyStreams, kAudioObjectPropertyScopeInput, kAudioObjectPropertyElementMain};
    size = 0;
    if (AudioObjectGetPropertyDataSize(device, &inputs, 0, nullptr, &size) != noErr || size == 0) return false;
    return [AVCaptureDevice authorizationStatusForMediaType:AVMediaTypeAudio] == AVAuthorizationStatusNotDetermined;
}

// A throwaway .bundle with the given cue files under Contents/Resources/audios, laid out as the input method bundle stages them.
NSBundle *MakeBundle(NSURL *root, NSString *name, NSArray<NSString *> *cues) {
    NSFileManager *files = NSFileManager.defaultManager;
    NSURL *bundle = [root URLByAppendingPathComponent:name isDirectory:YES];
    NSURL *contents = [bundle URLByAppendingPathComponent:@"Contents" isDirectory:YES];
    NSURL *audios = [contents URLByAppendingPathComponent:@"Resources/audios" isDirectory:YES];
    assert([files createDirectoryAtURL:audios withIntermediateDirectories:YES attributes:nil error:nil]);
    NSDictionary *info = @{@"CFBundleIdentifier": [@"app.msime.test." stringByAppendingString:name], @"CFBundlePackageType": @"BNDL"};
    assert([info writeToURL:[contents URLByAppendingPathComponent:@"Info.plist"] error:nil]);
    NSURL *source = [NSURL fileURLWithPath:@MSIME_VOICE_CUE_SOURCE_DIR isDirectory:YES];
    for (NSString *cue in cues) {
        assert([files copyItemAtURL:[source URLByAppendingPathComponent:cue] toURL:[audios URLByAppendingPathComponent:cue] error:nil]);
    }
    NSBundle *result = [NSBundle bundleWithURL:bundle];
    assert(result);
    return result;
}
}

int main() {
    @autoreleasepool {
        NSURL *root = [NSFileManager.defaultManager.temporaryDirectory URLByAppendingPathComponent:NSUUID.UUID.UUIDString isDirectory:YES];

        // Both product cues present: they are the ones loaded, and each is decodable audio rather than an empty placeholder.
        NSBundle *full = MakeBundle(root, @"Full.bundle", @[@"start.mp3", @"end.mp3"]);
        assert([MSIMEVoiceCueResourceURL(full, YES).lastPathComponent isEqual:@"start.mp3"]);
        assert([MSIMEVoiceCueResourceURL(full, NO).lastPathComponent isEqual:@"end.mp3"]);
        MSIMEVoiceCuePlayer *player = [[MSIMEVoiceCuePlayer alloc] initWithBundle:full];
        assert(player.startCueIsBundled && player.stopCueIsBundled);
        assert(player.startSound && player.stopSound && player.startSound != player.stopSound);
        assert(player.startSound.duration > 0 && player.stopSound.duration > 0);

        // A bundle without the cues must still give audible feedback: each side falls back to the system sound on its own.
        NSBundle *partial = MakeBundle(root, @"Partial.bundle", @[@"start.mp3"]);
        assert(!MSIMEVoiceCueResourceURL(partial, NO));
        MSIMEVoiceCuePlayer *fallback = [[MSIMEVoiceCuePlayer alloc] initWithBundle:partial];
        assert(fallback.startCueIsBundled && !fallback.stopCueIsBundled);
        assert(fallback.stopSound);

        // A test executable's main bundle carries no cues at all; the default initialiser must not fail.
        MSIMEVoiceCuePlayer *bare = [[MSIMEVoiceCuePlayer alloc] init];
        assert(bare && !bare.startCueIsBundled && !bare.stopCueIsBundled && bare.startSound && bare.stopSound);

        // 产品里 toggleVoiceInput: 先要求麦克风已授权才开始采集，提示音又只在采集开始之后播放，所以产品不会走到这个分支。
        if (PlaybackWaitsOnMicrophonePrompt()) {
            fprintf(stderr, "跳过播放断言：默认输出设备带输入流且麦克风授权未决定，播放会停在无人应答的授权框上\n");
            [NSFileManager.defaultManager removeItemAtURL:root error:nil];
            return 77;
        }

        // The start cue's completion (the deferred system-audio mute) runs once after the cue ends, never while it can still be heard. Restarting drops the earlier completion. Volume 0 keeps the run silent; a host without an output device fails play and runs the completion at once, which the same assertions accept.
        player.startSound.volume = 0;
        __block NSUInteger first = 0, second = 0;
        [player playStartCueThen:^{ ++first; }];
        const BOOL playing = player.startSound.isPlaying;
        assert(playing ? !first : first == 1);
        if (playing) {
            [player playStartCueThen:^{ ++second; }];
            NSDate *started = NSDate.date;
            while (!second && -started.timeIntervalSinceNow < player.startSound.duration + 3)
                [NSRunLoop.currentRunLoop runMode:NSDefaultRunLoopMode beforeDate:[NSDate dateWithTimeIntervalSinceNow:0.05]];
            assert(second == 1 && !first);
            assert(-started.timeIntervalSinceNow >= player.startSound.duration - 0.1);
            // The fallback timer of a finished cue must not run the completion a second time.
            [NSRunLoop.currentRunLoop runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.8]];
            assert(second == 1 && !first);
        }
        [player playStartCueThen:nil];
        [player.startSound stop];

        [NSFileManager.defaultManager removeItemAtURL:root error:nil];
    }
    return 0;
}
