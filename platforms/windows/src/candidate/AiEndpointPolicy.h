#pragma once

// AI 辅助接口地址的传输策略，与 crates/client-core/src/ai/endpoint.rs 一致：https 不限主机；http 只能指向本机或局域网（回环、RFC 1918、100.64.0.0/10、169.254.0.0/16、fe80::/10、fc00::/7、`.local`），公网主机必须走 https，API Token 不能明文经过互联网。这里不做 DNS 解析，除 `localhost` 和 `.local` 以外的域名一律按公网处理。用例在 shared/contracts/ai-endpoint/cases.json，windows-ai-endpoint-policy 逐条跑。
//
// 请求描述里的地址是用户填写的原文，client-core 已经按 WHATWG 规则检查过；这里是发请求前的第二道关，只认规范写法。非常规写法（十六进制、省略段或带前导零的 IPv4）一律不当作局域网地址：它们在 curl 和系统解析器里可能被解释成另一个地址。

#include <algorithm>
#include <cctype>
#include <cstdint>
#include <string>
#include <string_view>

// winsock2.h 会带进 windows.h 的 min/max 宏，先关掉，免得包含本文件的源文件里 std::min/std::max 被宏替换。
#ifndef NOMINMAX
#define NOMINMAX
#endif
#include <winsock2.h>
#include <ws2tcpip.h>

namespace msime::windows
{
enum class AiEndpointCheck
{
    Allowed,
    CleartextPublic,
    Invalid,
};

namespace ai_endpoint_detail
{
inline std::string lower(std::string_view value)
{
    std::string result(value);
    std::transform(result.begin(), result.end(), result.begin(),
                   [](unsigned char ch) { return static_cast<char>(std::tolower(ch)); });
    return result;
}

inline bool starts_with_ignoring_case(std::string_view value, std::string_view prefix)
{
    return value.size() >= prefix.size() && lower(value.substr(0, prefix.size())) == prefix;
}

// 只认四段、每段 0 到 255、没有前导零的点分十进制。
inline bool parse_ipv4(std::string_view host, uint8_t (&octets)[4])
{
    size_t part = 0;
    size_t start = 0;
    while (part < 4) {
        const auto end = host.find('.', start);
        const auto text = host.substr(start, end == std::string_view::npos ? host.size() - start : end - start);
        if (text.empty() || text.size() > 3 || (text.size() > 1 && text.front() == '0') ||
            !std::all_of(text.begin(), text.end(), [](unsigned char ch) { return ch >= '0' && ch <= '9'; }))
            return false;
        const int value = std::stoi(std::string(text));
        if (value > 255)
            return false;
        octets[part++] = static_cast<uint8_t>(value);
        if (end == std::string_view::npos)
            return part == 4;
        start = end + 1;
    }
    return false;
}

inline bool local_ipv4(const uint8_t (&octets)[4])
{
    const auto a = octets[0];
    const auto b = octets[1];
    return a == 127 || a == 10 || (a == 172 && b >= 16 && b <= 31) || (a == 192 && b == 168) ||
           (a == 100 && b >= 64 && b <= 127) || (a == 169 && b == 254);
}

inline bool local_ipv6(const in6_addr &address)
{
    const auto *bytes = reinterpret_cast<const uint8_t *>(&address);
    const bool loopback = std::all_of(bytes, bytes + 15, [](uint8_t byte) { return byte == 0; }) && bytes[15] == 1;
    // fe80::/10 链路本地；fc00::/7 唯一本地地址。
    return loopback || (bytes[0] == 0xfe && (bytes[1] & 0xc0) == 0x80) || (bytes[0] & 0xfe) == 0xfc;
}
} // namespace ai_endpoint_detail

// `host` 是地址里的主机部分，IPv6 不带方括号。
inline bool ai_endpoint_host_is_local(std::string_view host, bool bracketed)
{
    using namespace ai_endpoint_detail;
    if (bracketed) {
        in6_addr address{};
        return inet_pton(AF_INET6, std::string(host).c_str(), &address) == 1 && local_ipv6(address);
    }
    uint8_t octets[4]{};
    if (parse_ipv4(host, octets))
        return local_ipv4(octets);
    const auto domain = lower(host);
    if (domain == "localhost")
        return true;
    constexpr std::string_view suffix = ".local";
    if (domain.size() <= suffix.size() || domain.compare(domain.size() - suffix.size(), suffix.size(), suffix) != 0)
        return false;
    return domain[domain.size() - suffix.size() - 1] != '.';
}

inline AiEndpointCheck check_ai_endpoint(std::string_view url)
{
    using namespace ai_endpoint_detail;
    // C1 控制字符在 UTF-8 里是 C2 80 到 C2 9F。
    bool control = false;
    for (size_t index = 0; index < url.size(); ++index) {
        const auto ch = static_cast<unsigned char>(url[index]);
        if (ch < 32 || ch == 127 ||
            (ch == 0xc2 && index + 1 < url.size() && static_cast<unsigned char>(url[index + 1]) >= 0x80 &&
             static_cast<unsigned char>(url[index + 1]) <= 0x9f))
            control = true;
    }
    if (url.empty() || url.size() > 2048 || control || url.find('#') != std::string_view::npos)
        return AiEndpointCheck::Invalid;
    const bool https = starts_with_ignoring_case(url, "https://");
    const bool http = starts_with_ignoring_case(url, "http://");
    if (!https && !http)
        return AiEndpointCheck::Invalid;
    const auto rest = url.substr(https ? 8 : 7);
    const auto authority_end = rest.find_first_of("/?");
    const auto authority = rest.substr(0, authority_end == std::string_view::npos ? rest.size() : authority_end);
    if (authority.empty() || authority.find('@') != std::string_view::npos ||
        authority.find('\\') != std::string_view::npos)
        return AiEndpointCheck::Invalid;
    std::string_view host;
    std::string_view port;
    bool bracketed = false;
    if (authority.front() == '[') {
        const auto close = authority.find(']');
        if (close == std::string_view::npos)
            return AiEndpointCheck::Invalid;
        host = authority.substr(1, close - 1);
        const auto after = authority.substr(close + 1);
        if (!after.empty() && after.front() != ':')
            return AiEndpointCheck::Invalid;
        port = after.empty() ? after : after.substr(1);
        bracketed = true;
        in6_addr address{};
        if (inet_pton(AF_INET6, std::string(host).c_str(), &address) != 1)
            return AiEndpointCheck::Invalid;
    } else {
        const auto colon = authority.find(':');
        host = authority.substr(0, colon);
        port = colon == std::string_view::npos ? std::string_view{} : authority.substr(colon + 1);
    }
    // 主机不收百分号编码（包括 IPv6 区域标识 `%25en0`），与 Rust 只认规范写法的主机一致；`evil.com%00.local` 原样看以 `.local` 结尾。
    if (host.empty() || host.find('%') != std::string_view::npos || port.size() > 5 ||
        !std::all_of(port.begin(), port.end(), [](unsigned char ch) { return ch >= '0' && ch <= '9'; }) ||
        (!port.empty() && std::stoi(std::string(port)) > 65535))
        return AiEndpointCheck::Invalid;
    if (https)
        return AiEndpointCheck::Allowed;
    return ai_endpoint_host_is_local(host, bracketed) ? AiEndpointCheck::Allowed : AiEndpointCheck::CleartextPublic;
}

// 通过检查时返回 curl 只该用的协议：http 只给本机和局域网地址，其余只走 https；不通过时为空。
inline std::string_view ai_endpoint_protocol(std::string_view url)
{
    if (check_ai_endpoint(url) != AiEndpointCheck::Allowed)
        return {};
    return ai_endpoint_detail::starts_with_ignoring_case(url, "https://") ? "https" : "http";
}

// 通过检查的明文 http 地址只会是本机或局域网，这类请求直连、不走代理：经过代理就等于把 Token 明文交给代理。https 照旧按系统和环境变量的代理设置走。
inline bool ai_endpoint_connects_directly(std::string_view url)
{
    return ai_endpoint_protocol(url) == "http";
}
} // namespace msime::windows
