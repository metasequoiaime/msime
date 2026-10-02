#import "../../src/voice/VoiceAudioMuter.h"
#include <cassert>
#include <map>
#include <unistd.h>

namespace {
AudioDeviceID selected;
std::map<AudioDeviceID, UInt32> muted;
NSMutableDictionary<NSNumber *, NSString *> *identities;
NSUInteger reads, writes;
BOOL failRead, failWrite, shortRead, missingUID, wrongTranslation;
OSStatus Get(AudioObjectID object, const AudioObjectPropertyAddress *address,
    UInt32 qualifierSize, const void *qualifier, UInt32 *size, void *data) {
    ++reads;
    if (failRead) return kAudioHardwareUnspecifiedError;
    assert(address->mElement == kAudioObjectPropertyElementMain);
    if (address->mSelector == kAudioHardwarePropertyTranslateUIDToDevice) {
        assert(object == kAudioObjectSystemObject && address->mScope == kAudioObjectPropertyScopeGlobal);
        assert(qualifierSize == sizeof(CFStringRef) && qualifier && *size == sizeof(AudioDeviceID));
        NSString *requested = (__bridge NSString *)*static_cast<const CFStringRef *>(qualifier);
        AudioDeviceID resolved = kAudioObjectUnknown;
        for (NSNumber *key in identities) {
            if ([identities[key] isEqual:requested]) resolved = key.unsignedIntValue;
        }
        *static_cast<AudioDeviceID *>(data) = wrongTranslation ? 200 : resolved;
        if (shortRead) *size = 1;
        return noErr;
    }
    assert(!qualifierSize && !qualifier);
    if (address->mSelector == kAudioDevicePropertyDeviceUID) {
        assert(address->mScope == kAudioObjectPropertyScopeGlobal && *size == sizeof(CFStringRef));
        *static_cast<CFStringRef *>(data) = missingUID ? nullptr :
            static_cast<CFStringRef>(CFBridgingRetain(identities[@(object)]));
        if (shortRead) *size = 1;
        return noErr;
    }
    assert(*size == sizeof(UInt32));
    if (address->mSelector == kAudioHardwarePropertyDefaultOutputDevice) {
        assert(object == kAudioObjectSystemObject && address->mScope == kAudioObjectPropertyScopeGlobal);
        *static_cast<UInt32 *>(data) = selected;
    } else {
        assert(address->mSelector == kAudioDevicePropertyMute && address->mScope == kAudioDevicePropertyScopeOutput);
        *static_cast<UInt32 *>(data) = muted.at(object);
    }
    assert(address->mElement == kAudioObjectPropertyElementMain);
    if (shortRead) *size = 1;
    return noErr;
}
OSStatus Set(AudioObjectID object, const AudioObjectPropertyAddress *address,
    UInt32 qualifierSize, const void *qualifier, UInt32 size, const void *data) {
    assert(!qualifierSize && !qualifier && size == sizeof(UInt32));
    assert(address->mSelector == kAudioDevicePropertyMute && address->mScope == kAudioDevicePropertyScopeOutput);
    assert(address->mElement == kAudioObjectPropertyElementMain);
    ++writes;
    if (failWrite) return kAudioHardwareUnspecifiedError;
    muted.at(object) = *static_cast<const UInt32 *>(data);
    return noErr;
}
AudioObjectPropertyListenerBlock listener;
NSUInteger listenerAdds, listenerRemoves;
OSStatus AddListener(AudioObjectID object, const AudioObjectPropertyAddress *address, dispatch_queue_t queue,
    AudioObjectPropertyListenerBlock block) {
    assert(object == kAudioObjectSystemObject && address->mSelector == kAudioHardwarePropertyDefaultOutputDevice);
    assert(address->mScope == kAudioObjectPropertyScopeGlobal && address->mElement == kAudioObjectPropertyElementMain);
    assert(queue == dispatch_get_main_queue() && block && !listener);
    listener = block; ++listenerAdds;
    return noErr;
}
OSStatus RemoveListener(AudioObjectID object, const AudioObjectPropertyAddress *address, dispatch_queue_t queue,
    AudioObjectPropertyListenerBlock block) {
    assert(object == kAudioObjectSystemObject && address->mSelector == kAudioHardwarePropertyDefaultOutputDevice);
    assert(queue == dispatch_get_main_queue() && block == listener);
    listener = nil; ++listenerRemoves;
    return noErr;
}
// Models CoreAudio announcing a new default output device.
const AudioObjectPropertyAddress changed = {kAudioHardwarePropertyDefaultOutputDevice,
    kAudioObjectPropertyScopeGlobal, kAudioObjectPropertyElementMain};
void MoveDefault(AudioDeviceID device) {
    selected = device;
    if (listener) listener(1, &changed);
}
void Reset() {
    selected = 100; muted = {{100, 0}, {200, 0}};
    identities = [@{@100:@"synthetic-output-a", @200:@"synthetic-output-b"} mutableCopy];
    reads = writes = 0; failRead = failWrite = shortRead = NO;
    missingUID = wrongTranslation = NO;
    listener = nil; listenerAdds = listenerRemoves = 0;
}
}
int main() {
    @autoreleasepool {
        Reset();
        MSIMEVoiceAudioMuter *muter = [[MSIMEVoiceAudioMuter alloc] initWithAudioAPI:{Get, Set, AddListener, RemoveListener}];
        [muter restore]; assert(!reads && !writes);
        assert([muter mute:nil] && muted[100] == 1 && writes == 1);
        selected = 200;
        const NSUInteger beforeRepeat = reads;
        assert([muter mute:nil] && reads == beforeRepeat && writes == 1);
        [muter restore]; assert(muted[100] == 0 && muted[200] == 0 && writes == 2);
        [muter restore]; assert(writes == 2);
        selected = 100; muted[100] = 1;
        assert([muter mute:nil] && writes == 2);
        [muter restore]; assert(muted[100] == 1 && writes == 2);
        muted[100] = 0;
        assert([muter mute:nil]);
        muted[100] = 0;
        NSUInteger beforeRestore = writes;
        [muter restore]; assert(writes == beforeRestore);
        assert([muter mute:nil]);
        failWrite = YES;
        [muter restore]; assert(muted[100] == 1);
        selected = 200;
        assert(![muter mute:nil] && muted[200] == 0);
        failWrite = NO;
        [muter restore]; assert(muted[100] == 0 && muted[200] == 0);
        selected = 100;
        assert([muter mute:nil]);
        failRead = YES; [muter restore]; assert(muted[100] == 1);
        failRead = NO; shortRead = YES; [muter restore]; assert(muted[100] == 1);
        shortRead = NO; [muter restore]; assert(muted[100] == 0);
        for (NSUInteger failure = 0; failure < 5; ++failure) {
            Reset();
            if (failure == 0) failRead = YES;
            if (failure == 1) selected = kAudioObjectUnknown;
            if (failure == 2) shortRead = YES;
            if (failure == 3) muted[100] = 2;
            if (failure == 4) failWrite = YES;
            NSError *error = nil;
            assert(![muter mute:&error] && error.code == 10);
            beforeRestore = writes;
            failRead = failWrite = shortRead = NO; selected = 100; muted[100] = 0;
            [muter restore]; assert(writes == beforeRestore);
            assert([muter mute:nil]); [muter restore]; assert(muted[100] == 0);
        }
        Reset();
        assert([muter mute:nil]);
        identities[@100] = @"synthetic-replacement";
        [muter restore]; assert(writes == 1 && muted[100] == 1);
        wrongTranslation = YES;
        [muter restore]; assert(writes == 1 && muted[200] == 0);
        wrongTranslation = NO;
        identities[@300] = @"synthetic-output-a"; muted[300] = 1;
        [muter restore]; assert(writes == 2 && muted[300] == 0 && muted[100] == 1);
        Reset(); missingUID = YES;
        NSError *error = nil;
        assert(![muter mute:&error] && error.code == 10 && writes == 0);
        missingUID = NO;
        assert([muter mute:nil]);
        missingUID = YES;
        [muter restore]; assert(writes == 1 && muted[100] == 1);
        missingUID = NO;
        muter = nil;
        assert(muted[100] == 0);

        // The mute follows the default output device while recording and hands each previous device back.
        Reset();
        muter = [[MSIMEVoiceAudioMuter alloc] initWithAudioAPI:{Get, Set, AddListener, RemoveListener}];
        assert([muter mute:nil] && muted[100] == 1 && listener && listenerAdds == 1);
        MoveDefault(200); assert(muted[100] == 0 && muted[200] == 1 && writes == 3);
        MoveDefault(200); assert(writes == 3); // A repeated notification for the held device must not flicker it.
        MoveDefault(100); assert(muted[100] == 1 && muted[200] == 0 && writes == 5);
        AudioObjectPropertyListenerBlock stale = listener;
        [muter restore]; assert(muted[100] == 0 && muted[200] == 0 && !listener && listenerRemoves == 1);
        selected = 200; stale(1, &changed); assert(muted[200] == 0 && writes == 6); // Queued after restore.
        // A new default the user had already muted is neither taken nor later undone.
        Reset();
        assert([muter mute:nil]);
        muted[200] = 1; MoveDefault(200); assert(muted[100] == 0 && muted[200] == 1);
        [muter restore]; assert(muted[200] == 1 && writes == 2);
        // A default that moves because the muted device vanished still mutes the new one; the vanished device stays owed.
        Reset();
        assert([muter mute:nil]);
        [identities removeObjectForKey:@100];
        MoveDefault(200); assert(muted[200] == 1 && muted[100] == 1);
        [muter restore]; assert(muted[200] == 0 && muted[100] == 1);
        // It does not block the next recording, and is handed back once it reappears.
        assert([muter mute:nil] && muted[200] == 1);
        [muter restore]; assert(muted[200] == 0 && muted[100] == 1);
        identities[@100] = @"synthetic-output-a";
        [muter restore]; assert(muted[100] == 0);
        // A device that cannot be read at the moment of the change is left alone.
        assert([muter mute:nil]);
        muted[300] = 0; MoveDefault(300); assert(muted[100] == 0 && muted[300] == 0);
        [muter restore]; assert(muted[100] == 0);
        // Without listener functions the mute still works for the current device.
        Reset();
        muter = [[MSIMEVoiceAudioMuter alloc] initWithAudioAPI:{Get, Set, nullptr, nullptr}];
        assert([muter mute:nil] && muted[100] == 1 && !listenerAdds);
        [muter restore]; assert(muted[100] == 0);

        // A deferred mute (run after the start cue) is cancelled by any restore that precedes it.
        Reset();
        muter = [[MSIMEVoiceAudioMuter alloc] initWithAudioAPI:{Get, Set, AddListener, RemoveListener}];
        void (^cancelled)(void) = [muter deferredMute];
        [muter restore]; cancelled(); assert(muted[100] == 0 && !writes && !listener);
        void (^pending)(void) = [muter deferredMute];
        pending(); assert(muted[100] == 1 && writes == 1 && listener);
        pending(); assert(writes == 1);
        [muter restore]; pending(); assert(muted[100] == 0 && writes == 2);
        muter = nil; cancelled = pending = nil;

        // Owing several devices is journaled as one record and trimmed as each is handed back.
        Reset();
        char temporary[] = "/tmp/msime-voice-muter-test-XXXXXX";
        assert(mkdtemp(temporary));
        NSURL *directory = [NSURL fileURLWithPath:@(temporary) isDirectory:YES];
        NSURL *journal = [directory URLByAppendingPathComponent:@"voice-audio-recovery/pending.json"];
        auto record = [&] {
            NSData *data = [NSData dataWithContentsOfURL:journal];
            return data.length ? [NSJSONSerialization JSONObjectWithData:data options:0 error:nil] : nil;
        };
        muter = [[MSIMEVoiceAudioMuter alloc] initWithAudioAPI:{Get, Set, AddListener, RemoveListener}
            recoveryDirectory:[directory URLByAppendingPathComponent:@"voice-audio-recovery" isDirectory:YES]];
        assert([muter mute:nil] && [record()[@"uids"] isEqual:@[@"synthetic-output-a"]] && [record()[@"version"] isEqual:@2]);
        [identities removeObjectForKey:@100];
        MoveDefault(200);
        assert([record()[@"version"] isEqual:@2] && [record()[@"uids"] isEqual:(@[@"synthetic-output-a", @"synthetic-output-b"])]);
        [muter restore];
        assert(muted[200] == 0 && [record()[@"uids"] isEqual:@[@"synthetic-output-a"]] && [record()[@"version"] isEqual:@2]);
        identities[@100] = @"synthetic-output-a";
        [muter restore]; assert(muted[100] == 0 && !record());
        muter = nil;
        assert([NSFileManager.defaultManager removeItemAtURL:directory error:nil]);
    }
    return 0;
}
