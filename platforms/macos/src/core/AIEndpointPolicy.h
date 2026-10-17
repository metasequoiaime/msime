#pragma once

// AI 辅助接口地址的传输策略：https 不限主机；http 只能指向本机或局域网，因为 API Token 不能明文经过互联网。规则只在 Rust（crates/client-providers/src/ai/endpoint.rs），这里经 msime_client_ai_endpoint_check 询问，不再自己判断；tests/settings/AIEndpointPolicyTest.mm 经 C 接口逐条跑 shared/contracts/ai-endpoint/cases.json。
//
// 只用于 AI 辅助这条路径；云候选和翻译描述符仍按各自的规则检查。

#import <Foundation/Foundation.h>
#include <string.h>
#include "msime_client.h"

typedef NS_ENUM(NSInteger, MSIMEAIEndpointProblem) {
    MSIMEAIEndpointProblemNone,
    /// 不是带主机的完整 `http(s)://` 地址，或者带了用户名、密码、`#` 片段、控制字符、反斜杠，或者 `http://` 的主机不是规范写法。
    MSIMEAIEndpointProblemInvalid,
    /// `http://` 地址指向的不是本机或局域网主机。
    MSIMEAIEndpointProblemCleartextPublicHost,
};

/// 公网 http 被拒绝时给用户看的说明。
static NSString *const MSIMEAIEndpointCleartextMessage = @"http 只能用于本机或局域网地址（如 192.168.x.x、localhost、*.local），公网服务请用 https，以免 API Token 明文经过互联网。";

/// Rust 的回答 `{origin, problem, hint}`；C 接口拒绝请求或回复读不懂时为 nil。不做 I/O。
static inline NSDictionary *MSIMEAIEndpointAnswer(NSString *endpoint) {
    if (![endpoint isKindOfClass:NSString.class]) return nil;
    NSData *bytes = [endpoint dataUsingEncoding:NSUTF8StringEncoding];
    char *raw = msime_client_ai_endpoint_check((const uint8_t *)bytes.bytes, bytes.length);
    if (!raw) return nil;
    NSData *reply = [NSData dataWithBytes:raw length:strlen(raw)];
    msime_client_string_free(raw);
    NSDictionary *envelope = [NSJSONSerialization JSONObjectWithData:reply options:0 error:nil];
    if (![envelope isKindOfClass:NSDictionary.class] || ![envelope[@"ok"] isEqual:@YES]) return nil;
    NSDictionary *value = envelope[@"value"];
    return [value isKindOfClass:NSDictionary.class] ? value : nil;
}

/// 检查地址。调用方负责去掉首尾空白；Rust 回答不了时按不合规处理。
static inline MSIMEAIEndpointProblem MSIMEAIEndpointCheck(NSString *endpoint) {
    id problem = MSIMEAIEndpointAnswer(endpoint)[@"problem"];
    if (problem == NSNull.null) return MSIMEAIEndpointProblemNone;
    return [problem isEqual:@"cleartext_public"] ? MSIMEAIEndpointProblemCleartextPublicHost : MSIMEAIEndpointProblemInvalid;
}

/// 保存 Token 用的来源键 `scheme://host:port`，与设置页和 Rust 算出的相同；地址不合规时为 nil。
static inline NSString *MSIMEAIEndpointOrigin(NSString *endpoint) {
    id origin = MSIMEAIEndpointAnswer(endpoint)[@"origin"];
    return [origin isKindOfClass:NSString.class] ? origin : nil;
}
