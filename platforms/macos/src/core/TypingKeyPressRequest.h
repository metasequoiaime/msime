#pragma once

#import <Foundation/Foundation.h>
#include "TypingStatistics.h"

namespace msime::mac {
// 新客户端尚未接管时不用上一客户端的模式；英文关闭态优先于旧本地模式。
template <class Resolve>
TypingSource CaptureKeyPressSource(bool currentClient, bool englishMode, Resolve resolve) {
    if (!currentClient) return TypingSource::Unknown;
    if (englishMode) return TypingSource::English;
    return resolve();
}

// 只编码按下时的聚合值，统计线程不再读取会话或当前模式。
inline NSDictionary *KeyPressRequest(NSString *directory, const KeyPressFlush &batch) {
    NSMutableDictionary<NSString *, NSNumber *> *keys = [NSMutableDictionary dictionaryWithCapacity:batch.keys.size()];
    for (const auto &[key, count] : batch.keys)
        keys[[NSString stringWithUTF8String:key.c_str()]] = @(count);
    NSMutableDictionary<NSString *, NSDictionary *> *sources = [NSMutableDictionary dictionaryWithCapacity:batch.sources.size()];
    for (const auto &[source, counts] : batch.sources) {
        NSMutableDictionary<NSString *, NSNumber *> *values = [NSMutableDictionary dictionaryWithCapacity:counts.size()];
        for (const auto &[key, count] : counts)
            values[[NSString stringWithUTF8String:key.c_str()]] = @(count);
        sources[[NSString stringWithUTF8String:source.c_str()]] = values;
    }
    return @{ @"directory": directory, @"action": @{
        @"operation": @"record_keys", @"day": [NSString stringWithUTF8String:batch.day.c_str()],
        @"keys": keys, @"sources": sources } };
}
} // namespace msime::mac
