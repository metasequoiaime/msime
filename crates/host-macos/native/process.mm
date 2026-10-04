#import <AppKit/AppKit.h>

// 搬动 SQLite 数据库之前先停掉独立的 IMK 进程。`bundle_identifier` 是本版本输入法的 bundle id（同时安装的其他版本不受影响）；设置应用的 bundle id 与它不同，所以从不在这个集合里。
extern "C" bool msime_macos_stop_input_method(const char *bundle_identifier) {
    if (!NSThread.isMainThread || !bundle_identifier) return false;
    NSString *identifier = [NSString stringWithUTF8String:bundle_identifier];
    if (identifier.length == 0) return false;
    NSArray<NSRunningApplication *> *applications =
        [NSRunningApplication runningApplicationsWithBundleIdentifier:identifier];
    for (NSRunningApplication *application in applications) {
        if (!application.terminated && ![application terminate]) return false;
    }
    NSDate *deadline = [NSDate dateWithTimeIntervalSinceNow:3.0];
    while ([deadline timeIntervalSinceNow] > 0.0) {
        BOOL running = NO;
        for (NSRunningApplication *application in applications) {
            if (!application.terminated) {
                running = YES;
                break;
            }
        }
        if (!running) return true;
        [[NSRunLoop currentRunLoop] runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.02]];
    }
    return [applications indexOfObjectPassingTest:^BOOL(NSRunningApplication *application, NSUInteger, BOOL *) {
        return !application.terminated;
    }] == NSNotFound;
}
