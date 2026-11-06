#pragma once
// 语音标点（voice_input.punctuation_mode）：最终识别结果在润色之后、交给会话上屏之前按设置处理一次，规则只在共享层 msime_client_voice_punctuation。中间结果不经过这里。

#import <Foundation/Foundation.h>
#include "msime_client.h"
#include "../../../common/HostApiString.h"
#include <cstring>

// 共享文档 voice_input 里的语音标点方式。出厂默认 auto 不写进文档（client-core 的 skip_serializing_if），所以缺键就是 auto，不能像其他语音偏好那样「缺键保留本机旧值」：那样从别的方式改回 auto 后这里永远收不到。不认识的取值原样交给共享层，由它按 auto 处理。
static inline NSString *MSIMEVoicePunctuationMode(id voice) {
    id mode = [voice isKindOfClass:NSDictionary.class] ? voice[@"punctuation_mode"] : nil;
    return [mode isKindOfClass:NSString.class] && [mode length] ? [mode copy] : @"auto";
}

// 按方式处理最终文本。auto 和空文本不调用共享层；调用失败或响应格式不对时保留原文，设置故障不能丢掉已经识别出的文字。整段只有标点时共享层返回空串，这里原样返回，调用方按没有识别到文字提示。
static inline NSString *MSIMEVoicePunctuated(NSString *text, NSString *mode) {
    if (!text.length || !mode.length || [mode isEqualToString:@"auto"]) return text;
    NSData *body = [NSJSONSerialization dataWithJSONObject:@{@"text" : text, @"mode" : mode} options:0 error:nil];
    if (!body) return text;
    auto raw = msime::host_api::own_string(
        msime_client_voice_punctuation(static_cast<const uint8_t *>(body.bytes), body.length));
    if (!raw) return text;
    id response = [NSJSONSerialization JSONObjectWithData:[NSData dataWithBytesNoCopy:raw.get() length:std::strlen(raw.get()) freeWhenDone:NO]
                                                  options:0 error:nil];
    if (![response isKindOfClass:NSDictionary.class] || ![response[@"ok"] isEqual:@YES]) return text;
    id value = response[@"value"];
    id processed = [value isKindOfClass:NSDictionary.class] ? value[@"text"] : nil;
    return [processed isKindOfClass:NSString.class] ? processed : text;
}
