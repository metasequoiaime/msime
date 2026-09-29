#import <Foundation/Foundation.h>

// The settings app's bundle identifier, which is also the name of the default state directory under Application Support that the settings app and this input method share.
static NSString *const MSIMEClientApplicationIdentifier = @"app.msime.macos";
// The identifier and state directory before the settings app became app.msime.macos. The settings app copies it to the new directory on its first launch (macos_launch.rs); until then the input method keeps reading the prepared options it left behind, so installing only the input method never leaves it without a session.
static NSString *const MSIMELegacyClientApplicationIdentifier = @"app.msime.client";

static inline NSURL *MSIMEClientStateDirectory(NSFileManager *fileManager, NSString *identifier) {
    NSURL *support = [[fileManager URLsForDirectory:NSApplicationSupportDirectory inDomains:NSUserDomainMask] firstObject];
    return [support URLByAppendingPathComponent:identifier isDirectory:YES];
}

static inline NSString *MSIMELegacyRuntimeOptionsPath(NSFileManager *fileManager) {
    return [[MSIMEClientStateDirectory(fileManager, MSIMELegacyClientApplicationIdentifier) URLByAppendingPathComponent:@"runtime-options.json"] path];
}

static inline NSString *MSIMEDefaultRuntimeOptionsPath(NSFileManager *fileManager) {
    NSString *path = [[MSIMEClientStateDirectory(fileManager, MSIMEClientApplicationIdentifier) URLByAppendingPathComponent:@"runtime-options.json"] path];
    if (!path || [fileManager fileExistsAtPath:path]) return path;
    NSString *legacy = MSIMELegacyRuntimeOptionsPath(fileManager);
    return legacy && [fileManager fileExistsAtPath:legacy] ? legacy : path;
}

// Where default state lives right now: app.msime.client until the settings app has migrated it, app.msime.macos from then on. Nothing may be written to app.msime.macos before that, because the settings app migrates only into an empty directory.
static inline NSURL *MSIMEDefaultClientStateDirectory(NSFileManager *fileManager) {
    NSString *options = MSIMEDefaultRuntimeOptionsPath(fileManager);
    return options ? [NSURL fileURLWithPath:options.stringByDeletingLastPathComponent isDirectory:YES] : nil;
}

static inline NSString *MSIMERuntimeOptionsPath(void) {
    NSString *path = [NSBundle.mainBundle pathForResource:@"runtime-options" ofType:@"json"];
    return path ?: MSIMEDefaultRuntimeOptionsPath(NSFileManager.defaultManager);
}

static inline NSDictionary *MSIMELoadRuntimeOptions(void) {
    NSString *path = MSIMERuntimeOptionsPath();
    if (!path) return nil;
    NSData *data = [NSData dataWithContentsOfFile:path];
    id options = data ? [NSJSONSerialization JSONObjectWithData:data options:0 error:nil] : nil;
    return [options isKindOfClass:NSDictionary.class] ? options : nil;
}
