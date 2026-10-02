#import <Foundation/Foundation.h>

// The settings app's bundle identifier, which is also the name of the default state directory under Application Support that the settings app and this input method share.
static NSString *const MSIMEClientApplicationIdentifier = @"app.msime.macos";
static inline NSURL *MSIMEClientStateDirectory(NSFileManager *fileManager, NSString *identifier) {
    NSURL *support = [[fileManager URLsForDirectory:NSApplicationSupportDirectory inDomains:NSUserDomainMask] firstObject];
    return [support URLByAppendingPathComponent:identifier isDirectory:YES];
}

static inline NSString *MSIMEDefaultRuntimeOptionsPath(NSFileManager *fileManager) {
    return [[MSIMEClientStateDirectory(fileManager, MSIMEClientApplicationIdentifier) URLByAppendingPathComponent:@"runtime-options.json"] path];
}

// Where default state lives: the settings app's state directory under Application Support.
static inline NSURL *MSIMEDefaultClientStateDirectory(NSFileManager *fileManager) {
    return MSIMEClientStateDirectory(fileManager, MSIMEClientApplicationIdentifier);
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
