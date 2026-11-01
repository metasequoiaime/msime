#import <Foundation/Foundation.h>

#include <string>

// 与输入法及辅助进程使用同一系统目录解析，尊重系统用户目录及沙箱，不自行拼接 HOME。
extern "C" const char *msime_macos_telemetry_support_directory() {
    static const std::string directory = [] {
        @autoreleasepool {
            NSURL *support = [[NSFileManager.defaultManager URLsForDirectory:NSApplicationSupportDirectory
                                                                  inDomains:NSUserDomainMask] firstObject];
            const char *path = support.fileSystemRepresentation;
            return std::string(path ?: "");
        }
    }();
    // 字节由进程级静态字符串持有，调用方只读，不释放。
    return directory.empty() ? nullptr : directory.c_str();
}
