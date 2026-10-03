#import <Foundation/Foundation.h>

// 通知名由调用方按版本给出（`edition_notification_name`），与 platforms/macos/src/core/EditionIdentity.h 的 MSIMEEditionNotificationName 和 InputController.mm 收听的名字一致。设置应用和输入法是两个进程，分布式通知中心负责立刻叫醒 IMK；IMK 放手之前检查的是调用方已经写在词库锁旁边的 quiesce 租约，错过这条通知时它每秒一次的偏好定时器也会发现租约。同一条通知也发到本进程的通知中心，输入法自己的原生词库窗口靠它叫醒自己的控制器。
extern "C" void msime_macos_quiesce_input_sessions(const char *notification)
{
    NSString *name = notification ? [NSString stringWithUTF8String:notification] : nil;
    if (name.length == 0) return;
    [NSNotificationCenter.defaultCenter postNotificationName:name object:nil];
    [NSDistributedNotificationCenter.defaultCenter postNotificationName:name
                                                                   object:nil
                                                                 userInfo:nil
                                                        deliverImmediately:YES];
}

// 通知名同上，由调用方按版本给出。设置窗口和 IMK 输入法是两个进程，聚合统计开关的变化必须越过进程边界，热路径上的采集才能在第一条指令就停下（或恢复）。
extern "C" void msime_macos_notify_typing_statistics_enabled(const char *notification, bool enabled)
{
    NSString *name = notification ? [NSString stringWithUTF8String:notification] : nil;
    if (name.length == 0) return;
    [NSDistributedNotificationCenter.defaultCenter postNotificationName:name
                                                                   object:nil
                                                                 userInfo:@{ @"enabled": @(enabled) }
                                                        deliverImmediately:YES];
}
