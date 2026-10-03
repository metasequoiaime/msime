#import "VoiceAudioMuter.h"
#import "../settings/RuntimeOptions.h"
#include "../core/SystemPathAlias.h"
#include <fcntl.h>
#include <sys/file.h>
#include <sys/stat.h>
#include <unistd.h>
#include <filesystem>

static BOOL MSIMEVoiceSafeDirectoryPath(NSURL *url) {
    if (!url || !url.isFileURL) return NO;
    return msime::mac::StoragePathIsSafe(url.fileSystemRepresentation, true);
}
// The default output can move mid-recording while the device it left is already gone and cannot be handed back yet, so one journal may owe restores to several devices.
static const NSUInteger MSIMEVoiceOwnedDeviceLimit = 8;
static const AudioObjectPropertyAddress MSIMEVoiceDefaultOutputAddress = {kAudioHardwarePropertyDefaultOutputDevice,
    kAudioObjectPropertyScopeGlobal, kAudioObjectPropertyElementMain};
static BOOL MSIMEVoiceValidUID(id uid) {
    return [uid isKindOfClass:NSString.class] && [uid length] && [uid length] <= 4096;
}
@implementation MSIMEVoiceAudioMuter {
    MSIMEVoiceAudioAPI _api;
    // Devices this object muted and still owes a restore. Only unmuted devices are ever taken, so the original state of each is always unmuted.
    NSMutableArray<NSString *> *_ownedUIDs;
    BOOL _activeMute;
    // Bumped by every restore so a deferred mute created before it becomes inert.
    NSUInteger _epoch;
    AudioObjectPropertyListenerBlock _listener;
    NSURL *_recoveryDirectory;
    int _journalFD;
}
- (instancetype)init {
    _journalFD = -1;
    NSURL *directory = [MSIMEDefaultClientStateDirectory(NSFileManager.defaultManager)
        URLByAppendingPathComponent:@"voice-audio-recovery" isDirectory:YES];
    // A missing support directory must not silently disable crash protection.
    if (!directory) return nil;
    return [self initWithAudioAPI:{} recoveryDirectory:directory];
}
- (instancetype)initWithAudioAPI:(MSIMEVoiceAudioAPI)api {
    return [self initWithAudioAPI:api recoveryDirectory:nil];
}
- (instancetype)initWithAudioAPI:(MSIMEVoiceAudioAPI)api recoveryDirectory:(NSURL *)directory {
    self = [super init];
    if (self) { _api = api; _recoveryDirectory = [directory copy]; _journalFD = -1; _ownedUIDs = [NSMutableArray array]; }
    return self;
}
- (void)closeJournal {
    if (_journalFD >= 0) close(_journalFD);
    _journalFD = -1;
}
- (BOOL)loadJournal {
    if (!_recoveryDirectory || _journalFD >= 0) return YES;
    if (!MSIMEVoiceSafeDirectoryPath(_recoveryDirectory) ||
        ![NSFileManager.defaultManager createDirectoryAtURL:_recoveryDirectory
            withIntermediateDirectories:YES attributes:@{NSFilePosixPermissions:@0700} error:nil]) return NO;
    int directory = open(_recoveryDirectory.fileSystemRepresentation, O_RDONLY | O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC);
    if (directory < 0) return NO;
    struct stat info = {};
    if (fstat(directory, &info) || info.st_uid != geteuid() || (info.st_mode & 0777) != 0700) {
        close(directory); return NO;
    }
    int fd = openat(directory, "pending.json", O_RDWR | O_CREAT | O_NOFOLLOW | O_CLOEXEC | O_NONBLOCK, 0600);
    // Keep the directory entry durable before changing any audio state.
    BOOL synced = fsync(directory) == 0;
    close(directory);
    if (fd < 0) return NO;
    if (!synced || fstat(fd, &info) || !S_ISREG(info.st_mode) || info.st_uid != geteuid() ||
        info.st_nlink != 1 || (info.st_mode & 0777) != 0600 || flock(fd, LOCK_EX | LOCK_NB)) {
        close(fd); return NO;
    }
    // Re-read size after acquiring the lock; another instance may have just cleared it.
    if (fstat(fd, &info) || info.st_size < 0 || info.st_size > 32768) { close(fd); return NO; }
    if (info.st_size) {
        NSMutableData *data = [NSMutableData dataWithLength:(NSUInteger)info.st_size];
        if (pread(fd, data.mutableBytes, data.length, 0) != (ssize_t)data.length) { close(fd); return NO; }
        id record = [NSJSONSerialization JSONObjectWithData:data options:0 error:nil];
        NSArray *uids = nil;
        if (![record isKindOfClass:NSDictionary.class]) { close(fd); return NO; }
        if ([record[@"version"] isEqual:@2] && [record[@"uids"] isKindOfClass:NSArray.class] &&
            [record[@"uids"] count] && [record[@"uids"] count] <= MSIMEVoiceOwnedDeviceLimit) {
            uids = record[@"uids"];
            for (id uid in uids) if (!MSIMEVoiceValidUID(uid)) uids = nil;
        }
        if (!uids) { close(fd); return NO; }
        for (NSString *uid in uids) if (![_ownedUIDs containsObject:uid]) [_ownedUIDs addObject:[uid copy]];
    }
    _journalFD = fd;
    return YES;
}
- (BOOL)persistUIDs:(NSArray<NSString *> *)uids {
    if (!_recoveryDirectory) return YES;
    if (_journalFD < 0) return NO;
    // Never unlink the locked inode: other instances must lock this same file.
    if (!uids.count) return ftruncate(_journalFD, 0) == 0 && fsync(_journalFD) == 0;
    NSDictionary *record = @{@"version":@2, @"uids":uids};
    NSData *data = [NSJSONSerialization dataWithJSONObject:record options:0 error:nil];
    if (!data || data.length > 32768) return NO;
    // A device is only added to the snapshot before it is muted and only dropped after it has been restored.
    return ftruncate(_journalFD, 0) == 0 &&
        pwrite(_journalFD, data.bytes, data.length, 0) == (ssize_t)data.length && fsync(_journalFD) == 0;
}
- (NSString *)deviceUID:(AudioDeviceID)device {
    AudioObjectPropertyAddress address = {kAudioDevicePropertyDeviceUID,
        kAudioObjectPropertyScopeGlobal, kAudioObjectPropertyElementMain};
    CFStringRef value = nullptr;
    UInt32 bytes = sizeof(value);
    OSStatus status = _api.get(device, &address, 0, nullptr, &bytes, &value);
    id result = CFBridgingRelease(value);
    if (status != noErr || bytes != sizeof(value) || ![result isKindOfClass:NSString.class] ||
        ![result length] || [result length] > 4096) return nil;
    return [result copy];
}
- (AudioDeviceID)defaultOutputDevice {
    AudioDeviceID device = kAudioObjectUnknown;
    UInt32 bytes = sizeof(device);
    if (_api.get(kAudioObjectSystemObject, &MSIMEVoiceDefaultOutputAddress, 0, nullptr, &bytes, &device) != noErr ||
        bytes != sizeof(device)) return kAudioObjectUnknown;
    return device;
}
// Hands one owned device back to unmuted. NO keeps it owed for the next cleanup.
- (BOOL)restoreUID:(NSString *)owned {
    // Numeric IDs can be reused after disconnect. Resolve only the owned UID.
    AudioObjectPropertyAddress address = {kAudioHardwarePropertyTranslateUIDToDevice,
        kAudioObjectPropertyScopeGlobal, kAudioObjectPropertyElementMain};
    CFStringRef uid = (__bridge CFStringRef)owned;
    AudioDeviceID device = kAudioObjectUnknown;
    UInt32 bytes = sizeof(device);
    if (_api.get(kAudioObjectSystemObject, &address, sizeof(uid), &uid, &bytes, &device) != noErr ||
        bytes != sizeof(device) || device == kAudioObjectUnknown ||
        ![[self deviceUID:device] isEqual:owned]) return NO;
    address = {kAudioDevicePropertyMute,
        kAudioObjectPropertyScopeOutput, kAudioObjectPropertyElementMain};
    UInt32 current = 0, original = 0;
    bytes = sizeof(current);
    if (_api.get(device, &address, 0, nullptr, &bytes, &current) != noErr ||
        bytes != sizeof(current) || current > 1) return NO;
    // The user may already have unmuted it. Never resolve the new default here.
    return current == original || _api.set(device, &address, 0, nullptr, sizeof(original), &original) == noErr;
}
- (void)restoreOwnedExcept:(NSString *)kept {
    if (!_ownedUIDs.count || !_api.get || !_api.set) return;
    NSMutableArray<NSString *> *remaining = [NSMutableArray array];
    for (NSString *uid in _ownedUIDs)
        if ([uid isEqual:kept] || ![self restoreUID:uid]) [remaining addObject:uid];
    if (remaining.count == _ownedUIDs.count) return;
    // Failed reads/writes retain the original snapshot for the next cleanup.
    if ([self persistUIDs:remaining]) [_ownedUIDs setArray:remaining];
}
- (void)releaseIdleJournal {
    if (!_ownedUIDs.count) [self closeJournal];
}
// Takes the current default output device. YES also covers a device the user had already muted, which is left alone and never restored.
- (BOOL)muteDefaultOutputDevice {
    if (!_api.get || !_api.set || ![self loadJournal]) return NO;
    AudioDeviceID device = [self defaultOutputDevice];
    if (device == kAudioObjectUnknown) return NO;
    NSString *uid = [self deviceUID:device];
    // A device still owed a restore could not be resolved just now, so its current state says nothing about the user's.
    if (!uid || [_ownedUIDs containsObject:uid] || _ownedUIDs.count >= MSIMEVoiceOwnedDeviceLimit) return NO;
    AudioObjectPropertyAddress address = {kAudioDevicePropertyMute, kAudioDevicePropertyScopeOutput,
        kAudioObjectPropertyElementMain};
    UInt32 previous = 0;
    UInt32 bytes = sizeof(previous);
    if (_api.get(device, &address, 0, nullptr, &bytes, &previous) != noErr ||
        bytes != sizeof(previous) || previous > 1) return NO;
    // Do not take ownership of a mute that was already enabled by the user.
    if (previous) return YES;
    if (![self persistUIDs:[_ownedUIDs arrayByAddingObject:uid]]) return NO;
    [_ownedUIDs addObject:uid];
    UInt32 muted = 1;
    if (_api.set(device, &address, 0, nullptr, sizeof(muted), &muted) != noErr) {
        [self restoreOwnedExcept:nil]; return NO;
    }
    return YES;
}
- (void)defaultOutputDeviceDidChange {
    if (!_activeMute || ![self loadJournal]) return;
    AudioDeviceID device = [self defaultOutputDevice];
    NSString *current = device == kAudioObjectUnknown ? nil : [self deviceUID:device];
    // Hand every previous device back first, as stop would. A spurious notification for a device already held keeps it muted rather than flickering it.
    [self restoreOwnedExcept:current];
    if (!current || ![_ownedUIDs containsObject:current]) [self muteDefaultOutputDevice];
    [self releaseIdleJournal];
}
- (void)observeDefaultOutputDevice {
    if (_listener || !_api.addListener || !_api.removeListener) return;
    __weak MSIMEVoiceAudioMuter *weakSelf = self;
    AudioObjectPropertyListenerBlock listener = ^(UInt32 count, const AudioObjectPropertyAddress *addresses) {
        (void)count; (void)addresses;
        [weakSelf defaultOutputDeviceDidChange];
    };
    // The main queue owns every other call on this object.
    if (_api.addListener(kAudioObjectSystemObject, &MSIMEVoiceDefaultOutputAddress, dispatch_get_main_queue(), listener) == noErr)
        _listener = listener;
}
- (void)stopObservingDefaultOutputDevice {
    if (!_listener) return;
    _api.removeListener(kAudioObjectSystemObject, &MSIMEVoiceDefaultOutputAddress, dispatch_get_main_queue(), _listener);
    _listener = nil;
}
- (BOOL)mute:(NSError **)error {
    // Repeated starts must not replace the original device or mute snapshot.
    if (_activeMute) return YES;
    auto fail = [&] {
        if (error) *error = [NSError errorWithDomain:@"app.msime.client.voice" code:10
            userInfo:@{NSLocalizedDescriptionKey:@"无法静音系统音频"}];
        return NO;
    };
    if (![self loadJournal]) return fail();
    // Hand back what a crash, or a device that vanished while muted, left behind. One that is still missing stays journaled and does not block muting the current default.
    [self restoreOwnedExcept:nil];
    if (![self muteDefaultOutputDevice]) { [self releaseIdleJournal]; return fail(); }
    [self releaseIdleJournal];
    _activeMute = YES;
    [self observeDefaultOutputDevice];
    return YES;
}
- (void (^)(void))deferredMute {
    __weak MSIMEVoiceAudioMuter *weakSelf = self;
    const NSUInteger epoch = _epoch;
    return ^{
        MSIMEVoiceAudioMuter *muter = weakSelf;
        if (muter && muter->_epoch == epoch) [muter mute:nil];
    };
}
- (void)restore {
    _activeMute = NO;
    ++_epoch;
    [self stopObservingDefaultOutputDevice];
    if (![self loadJournal]) return;
    [self restoreOwnedExcept:nil];
    [self releaseIdleJournal];
}
- (void)dealloc { [self restore]; [self closeJournal]; }
@end
