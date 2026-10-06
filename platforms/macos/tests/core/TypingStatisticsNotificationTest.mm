#import <Foundation/Foundation.h>

#include <cassert>

#import "../../src/core/EditionIdentity.h"

extern "C" void msime_macos_notify_typing_statistics_enabled(const char *notification, bool enabled);

// 设置应用发出的通知只到同一个版本的输入法：通知名随版本而变，full 不变。这里用一个只为测试存在的版本名收发，不会改动本机正在运行的输入法。
int main() {
    @autoreleasepool {
        NSString *const base = @"MetasequoiaTypingStatisticsEnabledChangedNotification";
        assert([MSIMEEditionNotificationNameIn(@{}, base) isEqualToString:base]);
        assert([MSIMEEditionNotificationNameIn(@{@"MSIMEEdition": @"full"}, base) isEqualToString:base]);
        NSString *const name = MSIMEEditionNotificationNameIn(@{@"MSIMEEdition": @"ctestedition"}, base);
        assert([name isEqualToString:[base stringByAppendingString:@".ctestedition"]]);

        NSDistributedNotificationCenter *center = NSDistributedNotificationCenter.defaultCenter;
        __block NSNumber *received = nil;
        __block BOOL fullReceived = NO;
        id observer = [center addObserverForName:name object:nil queue:NSOperationQueue.mainQueue
                                      usingBlock:^(NSNotification *notification) {
            id enabled = notification.userInfo[@"enabled"];
            if ([enabled isKindOfClass:NSNumber.class]) received = enabled;
        }];
        id fullObserver = [center addObserverForName:base object:nil queue:NSOperationQueue.mainQueue
                                          usingBlock:^(NSNotification *notification) {
            (void)notification;
            fullReceived = YES;
        }];

        msime_macos_notify_typing_statistics_enabled(name.UTF8String, true);
        NSDate *deadline = [NSDate dateWithTimeIntervalSinceNow:2.0];
        while (!received && deadline.timeIntervalSinceNow > 0) {
            [NSRunLoop.currentRunLoop runMode:NSDefaultRunLoopMode
                                  beforeDate:[NSDate dateWithTimeIntervalSinceNow:0.01]];
        }
        assert(received != nil);
        assert(received.boolValue);

        received = nil;
        msime_macos_notify_typing_statistics_enabled(name.UTF8String, false);
        deadline = [NSDate dateWithTimeIntervalSinceNow:2.0];
        while (!received && deadline.timeIntervalSinceNow > 0) {
            [NSRunLoop.currentRunLoop runMode:NSDefaultRunLoopMode
                                  beforeDate:[NSDate dateWithTimeIntervalSinceNow:0.01]];
        }
        assert(received != nil);
        assert(!received.boolValue);
        // 另一个版本的通知不会到 full 的输入法。
        assert(!fullReceived);
        [center removeObserver:observer];
        [center removeObserver:fullObserver];
    }
    return 0;
}
