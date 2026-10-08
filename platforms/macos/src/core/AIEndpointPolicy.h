#pragma once

// AI 辅助接口地址的传输策略，与 crates/client-core/src/ai/endpoint.rs 逐条一致：https 不限主机；http 只能指向本机或局域网（回环、RFC 1918、100.64.0.0/10、链路本地、IPv6 唯一本地地址、`.local`），因为 API Token 不能明文经过互联网。检查时不做 DNS 解析：IP 只认严格的字面量，除 `localhost` 和 `.local` 以外的域名一律按公网处理。两边共用 shared/contracts/ai-endpoint/cases.json 里的用例（tests/settings/AIEndpointPolicyTest.mm）。
//
// 只用于 AI 辅助这条路径；云候选和翻译描述符仍按各自的规则检查。

#import <Foundation/Foundation.h>
#include <arpa/inet.h>

typedef NS_ENUM(NSInteger, MSIMEAIEndpointProblem) {
    MSIMEAIEndpointProblemNone,
    /// 不是带主机的完整 `http(s)://` 地址，或者带了用户名、密码、`#` 片段、控制字符。
    MSIMEAIEndpointProblemInvalid,
    /// `http://` 地址指向的不是本机或局域网主机。
    MSIMEAIEndpointProblemCleartextPublicHost,
};

/// 公网 http 被拒绝时给用户看的说明。
static NSString *const MSIMEAIEndpointCleartextMessage = @"http 只能用于本机或局域网地址（如 192.168.x.x、localhost、*.local），公网服务请用 https，以免 API Token 明文经过互联网。";

/// 严格的点分十进制：四段、每段 0～255、不带前导零。`010.0.0.1` 在系统解析里按八进制处理，会连到另一个地址，所以不当作 IP，按域名归入公网。
static inline BOOL MSIMEAIParseIPv4(NSString *text, int octets[4]) {
    NSArray<NSString *> *parts = [text componentsSeparatedByString:@"."];
    if (parts.count != 4) return NO;
    for (NSUInteger i = 0; i < 4; ++i) {
        NSString *part = parts[i];
        if (part.length < 1 || part.length > 3 || (part.length > 1 && [part characterAtIndex:0] == '0')) return NO;
        int value = 0;
        for (NSUInteger j = 0; j < part.length; ++j) {
            unichar c = [part characterAtIndex:j];
            if (c < '0' || c > '9') return NO;
            value = value * 10 + (c - '0');
        }
        if (value > 255) return NO;
        octets[i] = value;
    }
    return YES;
}

/// `host` 取自 `NSURLComponents.host`，IPv6 带方括号。
static inline BOOL MSIMEAIIsLocalNetworkHost(NSString *host) {
    if ([host hasPrefix:@"["] && [host hasSuffix:@"]"] && host.length > 2) {
        struct in6_addr address;
        // inet_pton 只解析文本，不做任何查询。
        if (inet_pton(AF_INET6, [host substringWithRange:NSMakeRange(1, host.length - 2)].UTF8String, &address) != 1) return NO;
        const uint8_t *b = address.s6_addr;
        BOOL loopback = b[15] == 1;
        for (int i = 0; i < 15 && loopback; ++i) loopback = b[i] == 0;
        // fe80::/10 链路本地；fc00::/7 唯一本地地址。
        return loopback || (b[0] == 0xfe && (b[1] & 0xc0) == 0x80) || (b[0] & 0xfe) == 0xfc;
    }
    int o[4];
    if (MSIMEAIParseIPv4(host, o)) {
        return o[0] == 127 || o[0] == 10 || (o[0] == 172 && o[1] >= 16 && o[1] <= 31) || (o[0] == 192 && o[1] == 168)
            // 100.64.0.0/10：运营商级 NAT 与 Tailscale。
            || (o[0] == 100 && o[1] >= 64 && o[1] <= 127) || (o[0] == 169 && o[1] == 254);
    }
    NSString *domain = host.lowercaseString;
    if ([domain isEqualToString:@"localhost"]) return YES;
    if (![domain hasSuffix:@".local"]) return NO;
    NSString *label = [domain substringToIndex:domain.length - @".local".length];
    return label.length > 0 && ![label hasSuffix:@"."];
}

/// 检查地址；通过时 `components` 带回解析结果。调用方负责去掉首尾空白。
static inline MSIMEAIEndpointProblem MSIMEAIEndpointCheck(NSString *endpoint, NSURLComponents **components) {
    if (![endpoint isKindOfClass:NSString.class] || !endpoint.length ||
        [endpoint lengthOfBytesUsingEncoding:NSUTF8StringEncoding] > 2048 ||
        [endpoint rangeOfCharacterFromSet:NSCharacterSet.controlCharacterSet].location != NSNotFound) return MSIMEAIEndpointProblemInvalid;
    // `https:///v1` 会被解析成没有主机的地址；要求 `://` 后面紧跟主机。
    NSRange separator = [endpoint rangeOfString:@"://"];
    if (separator.location == NSNotFound || NSMaxRange(separator) >= endpoint.length ||
        [endpoint characterAtIndex:NSMaxRange(separator)] == '/') return MSIMEAIEndpointProblemInvalid;
    NSURLComponents *url = [NSURLComponents componentsWithString:endpoint];
    NSString *scheme = url.scheme.lowercaseString;
    // `#` 后面哪怕是空的也算片段（`fragment` 为空字符串而不是 nil）。
    if (!url || !([scheme isEqualToString:@"https"] || [scheme isEqualToString:@"http"]) || !url.host.length ||
        url.user != nil || url.password != nil || url.fragment != nil ||
        (url.port && (url.port.integerValue < 0 || url.port.integerValue > 65535)) || ![NSURL URLWithString:endpoint]) return MSIMEAIEndpointProblemInvalid;
    if ([scheme isEqualToString:@"http"] && !MSIMEAIIsLocalNetworkHost(url.host)) return MSIMEAIEndpointProblemCleartextPublicHost;
    if (components) *components = url;
    return MSIMEAIEndpointProblemNone;
}

/// 保存 Token 用的来源键 `scheme://host:port`：主机小写，端口总是写出来（https 默认 443，http 默认 80），IPv6 保留方括号。地址不合规时为 nil。
static inline NSString *MSIMEAIEndpointOrigin(NSString *endpoint) {
    NSURLComponents *url = nil;
    if (MSIMEAIEndpointCheck(endpoint, &url) != MSIMEAIEndpointProblemNone) return nil;
    NSString *scheme = url.scheme.lowercaseString;
    NSInteger port = url.port ? url.port.integerValue : ([scheme isEqualToString:@"https"] ? 443 : 80);
    return [NSString stringWithFormat:@"%@://%@:%ld", scheme, url.host.lowercaseString, (long)port];
}
