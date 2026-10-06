#import "../../src/voice/VoiceAudioMuter.h"
#include <cassert>
#include <sys/stat.h>
#include <unistd.h>

namespace {
UInt32 muted = 0;
NSUInteger writes = 0;
BOOL connected = YES, failWrite = NO, crashBeforeWrite = NO;
NSURL *directory;
NSURL *Journal() { return [directory URLByAppendingPathComponent:@"pending.json"]; }
NSUInteger JournalSize() { return [NSData dataWithContentsOfURL:Journal()].length; }
OSStatus Get(AudioObjectID object, const AudioObjectPropertyAddress *address,
    UInt32 qualifierSize, const void *qualifier, UInt32 *size, void *data) {
    assert(address->mElement == kAudioObjectPropertyElementMain);
    switch (address->mSelector) {
    case kAudioHardwarePropertyDefaultOutputDevice:
        assert(object == kAudioObjectSystemObject && *size == sizeof(AudioDeviceID));
        *static_cast<AudioDeviceID *>(data) = 100; break;
    case kAudioHardwarePropertyTranslateUIDToDevice: {
        assert(object == kAudioObjectSystemObject && qualifierSize == sizeof(CFStringRef));
        NSString *uid = (__bridge NSString *)*static_cast<const CFStringRef *>(qualifier);
        assert([uid isEqual:@"synthetic-recovery-output"]);
        *static_cast<AudioDeviceID *>(data) = connected ? 100 : kAudioObjectUnknown; break;
    }
    case kAudioDevicePropertyDeviceUID:
        assert(object == 100 && *size == sizeof(CFStringRef));
        *static_cast<CFStringRef *>(data) = static_cast<CFStringRef>(CFBridgingRetain(@"synthetic-recovery-output")); break;
    case kAudioDevicePropertyMute:
        assert(object == 100 && *size == sizeof(UInt32));
        *static_cast<UInt32 *>(data) = muted; break;
    default: assert(false);
    }
    return noErr;
}
OSStatus Set(AudioObjectID object, const AudioObjectPropertyAddress *address,
    UInt32 qualifierSize, const void *qualifier, UInt32 size, const void *data) {
    assert(object == 100 && address->mSelector == kAudioDevicePropertyMute);
    assert(!qualifierSize && !qualifier && size == sizeof(UInt32));
    // Both mute and restore must retain a valid recovery record until completion.
    NSDictionary *record = [NSJSONSerialization JSONObjectWithData:[NSData dataWithContentsOfURL:Journal()]
        options:0 error:nil];
    assert([record[@"version"] isEqual:@2] && [record[@"uids"] isEqual:@[@"synthetic-recovery-output"]]);
    if (crashBeforeWrite) _exit(78);
    ++writes;
    if (failWrite) return kAudioHardwareUnspecifiedError;
    muted = *static_cast<const UInt32 *>(data);
    return noErr;
}
__attribute__((ns_returns_retained)) MSIMEVoiceAudioMuter *NewMuter() {
    return [[MSIMEVoiceAudioMuter alloc] initWithAudioAPI:{Get, Set, nullptr, nullptr} recoveryDirectory:directory];
}
void Crash(NSString *executable, NSString *mode, int expected) {
    NSTask *task = [[NSTask alloc] init];
    task.executableURL = [NSURL fileURLWithPath:executable];
    task.arguments = @[directory.path, mode];
    assert([task launchAndReturnError:nil]);
    [task waitUntilExit];
    assert(task.terminationReason == NSTaskTerminationReasonExit && task.terminationStatus == expected);
    assert(JournalSize() > 0);
}
}
int main(int argc, const char *argv[]) {
    @autoreleasepool {
        if (argc == 3) {
            directory = [NSURL fileURLWithPath:@(argv[1]) isDirectory:YES];
            if ([@(argv[2]) isEqual:@"contend"]) {
                MSIMEVoiceAudioMuter *competitor = NewMuter();
                assert(![competitor mute:nil]); [competitor restore]; assert(writes == 0);
                _exit(79);
            }
            crashBeforeWrite = [@(argv[2]) isEqual:@"before-write"];
            __attribute__((objc_precise_lifetime)) MSIMEVoiceAudioMuter *muter = NewMuter();
            assert([muter mute:nil] && muted == 1 && writes == 1);
            _exit(77); // Deliberately bypass ARC, dealloc and all graceful cleanup.
        }
        char temporary[] = "/tmp/msime-voice-recovery-test-XXXXXX";
        assert(mkdtemp(temporary));
        directory = [NSURL fileURLWithPath:@(temporary) isDirectory:YES];
        NSString *executable = @(argv[0]);
        Crash(executable, @"after-write", 77);
        struct stat info = {};
        assert(stat(Journal().fileSystemRepresentation, &info) == 0 && (info.st_mode & 0777) == 0600);
        muted = 1; writes = 0;
        MSIMEVoiceAudioMuter *muter = NewMuter();
        [muter restore]; assert(muted == 0 && writes == 1 && JournalSize() == 0);
        [muter restore]; assert(writes == 1);

        // A crash after journaling but before the hardware write requires no unmute.
        Crash(executable, @"before-write", 78);
        writes = 0; [muter restore]; assert(muted == 0 && writes == 0 && JournalSize() == 0);

        Crash(executable, @"after-write", 77);
        muted = 1; connected = NO;
        [muter restore]; assert(muted == 1 && JournalSize() > 0);
        assert(![muter mute:nil] && JournalSize() > 0);
        muter = nil; assert(JournalSize() > 0); // Failed cleanup must survive object destruction.
        connected = YES; failWrite = YES;
        muter = NewMuter(); [muter restore]; assert(muted == 1 && JournalSize() > 0);
        failWrite = NO; [muter restore]; assert(muted == 0 && JournalSize() == 0);

        // Another object/process cannot recover or overwrite a live owner's snapshot.
        assert([muter mute:nil] && muted == 1);
        Crash(executable, @"contend", 79);
        MSIMEVoiceAudioMuter *competitor = NewMuter();
        NSUInteger before = writes;
        [competitor restore]; assert(![competitor mute:nil] && writes == before && muted == 1);
        competitor = nil; assert(writes == before && JournalSize() > 0);
        [muter restore]; assert(muted == 0 && JournalSize() == 0);

        // Pre-existing user mute is never journaled or undone.
        muted = 1; before = writes;
        assert([muter mute:nil]); [muter restore]; assert(muted == 1 && writes == before && JournalSize() == 0);
        muted = 0;
        for (id invalid in @[@{}, @{@"version":@2, @"uid":@"synthetic-recovery-output", @"previous":@0},
                            @{@"version":@1, @"uid":@"synthetic-recovery-output", @"previous":@0},
                            @{@"version":@2, @"uids":@[]}, @{@"version":@2, @"uids":@[@""]},
                            @{@"version":@2, @"uids":@[@"synthetic-recovery-output", @3]},
                            @{@"version":@2, @"uids":@[@"a", @"b", @"c", @"d", @"e", @"f", @"g", @"h", @"i"]}]) {
            NSData *data = [NSJSONSerialization dataWithJSONObject:invalid options:0 error:nil];
            assert([data writeToURL:Journal() options:0 error:nil]);
            [muter restore]; assert(![muter mute:nil] && writes == before && JournalSize() == data.length);
        }
        // A valid record is recovered.
        NSData *multiple = [NSJSONSerialization dataWithJSONObject:@{@"version":@2, @"uids":@[@"synthetic-recovery-output"]}
            options:0 error:nil];
        assert([multiple writeToURL:Journal() options:0 error:nil]);
        muted = 1; [muter restore]; assert(muted == 0 && writes == before + 1 && JournalSize() == 0);
        before = writes;
        assert([[NSData data] writeToURL:Journal() options:0 error:nil]);
        assert([[NSMutableData dataWithLength:32769] writeToURL:Journal() options:0 error:nil]);
        assert(![muter mute:nil] && writes == before && JournalSize() == 32769);
        assert([[NSData data] writeToURL:Journal() options:0 error:nil]);
        assert(chmod(Journal().fileSystemRepresentation, 0644) == 0);
        assert(![muter mute:nil] && writes == before);
        assert(chmod(Journal().fileSystemRepresentation, 0600) == 0);
        assert([muter mute:nil]); [muter restore]; assert(muted == 0);
        muter = nil;
        // The journal must not follow a symlink to another file.
        NSURL *other = [directory URLByAppendingPathComponent:@"synthetic-other"];
        NSData *sentinel = [@"synthetic-untouched" dataUsingEncoding:NSUTF8StringEncoding];
        assert([sentinel writeToURL:other options:0 error:nil]);
        assert(unlink(Journal().fileSystemRepresentation) == 0);
        assert(symlink(other.fileSystemRepresentation, Journal().fileSystemRepresentation) == 0);
        muter = NewMuter(); before = writes;
        assert(![muter mute:nil]); [muter restore]; assert(writes == before);
        assert([[NSData dataWithContentsOfURL:other] isEqual:sentinel]);
        muter = nil;
        assert(unlink(Journal().fileSystemRepresentation) == 0);
        assert(link(other.fileSystemRepresentation, Journal().fileSystemRepresentation) == 0);
        assert(chmod(other.fileSystemRepresentation, 0600) == 0);
        muter = NewMuter();
        assert(![muter mute:nil]); [muter restore]; assert(writes == before);
        assert([[NSData dataWithContentsOfURL:other] isEqual:sentinel]);
        muter = nil;
        assert([NSFileManager.defaultManager removeItemAtURL:directory error:nil]);
    }
    return 0;
}
