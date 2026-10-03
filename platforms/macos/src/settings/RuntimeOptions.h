#import <Foundation/Foundation.h>
#import "../core/EditionIdentity.h"

// 设置应用的 bundle identifier，也是设置应用和本输入法共用的、Application Support 下默认状态目录的名字。它随版本而变（full 是 app.msime.macos），见 EditionIdentity.h。
#define MSIMEClientApplicationIdentifier MSIMESettingsBundleIdentifier()
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
