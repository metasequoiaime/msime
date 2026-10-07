#import <Foundation/Foundation.h>
#import "../core/EditionIdentity.h"
#include <cerrno>
#include <cstdint>
#include <fcntl.h>
#include <sys/stat.h>
#include <unistd.h>

// macOS 宿主与其它平台一样只读取有限大小的运行时配置，避免环境变量指向异常文件时无界分配。
static constexpr NSUInteger MSIMERuntimeOptionsReadLimit = 2 * 1024 * 1024;

static inline NSData *MSIMEReadRuntimeOptionsData(NSString *path) {
    if (!path.length) return nil;
    const int descriptor = open(path.fileSystemRepresentation, O_RDONLY | O_NOFOLLOW | O_NONBLOCK);
    if (descriptor < 0) return nil;
    struct stat fileStat = {};
    if (fstat(descriptor, &fileStat) != 0 || !S_ISREG(fileStat.st_mode) ||
        fileStat.st_size < 0 ||
        static_cast<uint64_t>(fileStat.st_size) > MSIMERuntimeOptionsReadLimit) {
        close(descriptor);
        return nil;
    }
    NSMutableData *data = [NSMutableData dataWithCapacity:static_cast<NSUInteger>(fileStat.st_size)];
    uint8_t buffer[8192];
    for (;;) {
        const ssize_t count = read(descriptor, buffer, sizeof(buffer));
        if (count == 0) break;
        if (count < 0) {
            if (errno == EINTR) continue;
            close(descriptor);
            return nil;
        }
        if (data.length > MSIMERuntimeOptionsReadLimit - static_cast<NSUInteger>(count)) {
            close(descriptor);
            return nil;
        }
        [data appendBytes:buffer length:static_cast<NSUInteger>(count)];
    }
    close(descriptor);
    return data;
}

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
    NSData *data = MSIMEReadRuntimeOptionsData(path);
    id options = data ? [NSJSONSerialization JSONObjectWithData:data options:0 error:nil] : nil;
    return [options isKindOfClass:NSDictionary.class] ? options : nil;
}
