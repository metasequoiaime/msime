#pragma once
#import <Foundation/Foundation.h>

// 输入法进程内成功保存后即时唤醒范围监听；跨进程修改仍由低频兜底发现。
inline NSNotificationName MSIMEGlobalVoicePreferencesDidChangeNotification() {
    return @"MSIMEGlobalVoicePreferencesDidChangeNotification";
}
inline void MSIMENotifyGlobalVoicePreferencesChanged() {
    [NSNotificationCenter.defaultCenter postNotificationName:MSIMEGlobalVoicePreferencesDidChangeNotification() object:nil];
}
