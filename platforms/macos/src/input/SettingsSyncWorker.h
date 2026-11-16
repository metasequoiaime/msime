#pragma once

#import <Foundation/Foundation.h>
#include "msime_client.h"

// 调用处只提供与设置应用相同的偏好目录，文件和网络都由 Rust 线程处理。
static inline void MSIMEStartSettingsSync(NSString *directory) {
    if (!directory.isAbsolutePath) return;
    NSData *body = [NSJSONSerialization dataWithJSONObject:@{@"preferences_directory": directory}
                                                 options:0 error:nil];
    if (!body) return;
    char *response = msime_client_settings_sync_start(static_cast<const uint8_t *>(body.bytes), body.length);
    if (response) msime_client_string_free(response);
}
