#pragma once

// AI 辅助接口地址的传输策略：https 不限主机；http 只能指向本机或局域网，公网主机必须走 https，API Token 不能明文经过互联网。规则只在 Rust（crates/client-providers/src/ai/endpoint.rs），这里经 msime_client_ai_endpoint_check 询问，不再自己判断；用例在 shared/contracts/ai-endpoint/cases.json，windows-ai-endpoint-policy 经 C 接口逐条跑。
//
// 请求描述里的地址是用户填写的原文，curl 拿到的也是原文。Rust 对明文 http 只收主机按规范写法书写、不带反斜杠的地址，所以 curl 读出的主机与 Rust 判断的是同一个。

#include "msime_client.h"
#include "../../../common/HostApiString.h"

#include <nlohmann/json.hpp>

#include <algorithm>
#include <cctype>
#include <cstdint>
#include <string_view>

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
inline bool starts_with_ignoring_case(std::string_view value, std::string_view prefix)
{
    return value.size() >= prefix.size() &&
           std::equal(prefix.begin(), prefix.end(), value.begin(), [](char expected, char actual) {
               return expected == static_cast<char>(std::tolower(static_cast<unsigned char>(actual)));
           });
}
} // namespace ai_endpoint_detail

// C 接口拒绝请求、回复读不懂时一律按不合规处理，不发请求。
inline AiEndpointCheck check_ai_endpoint(std::string_view url)
{
    const auto response = host_api::own_string(
        msime_client_ai_endpoint_check(reinterpret_cast<const uint8_t *>(url.data()), url.size()));
    if (!response)
        return AiEndpointCheck::Invalid;
    const auto envelope = nlohmann::json::parse(response.get(), nullptr, false);
    if (!envelope.is_object())
        return AiEndpointCheck::Invalid;
    const auto ok = envelope.find("ok");
    const auto value = envelope.find("value");
    if (ok == envelope.end() || !ok->is_boolean() || !ok->get<bool>() || value == envelope.end() || !value->is_object())
        return AiEndpointCheck::Invalid;
    const auto problem = value->find("problem");
    if (problem == value->end())
        return AiEndpointCheck::Invalid;
    if (problem->is_null())
        return AiEndpointCheck::Allowed;
    return problem->is_string() && problem->get<std::string>() == "cleartext_public" ? AiEndpointCheck::CleartextPublic
                                                                                     : AiEndpointCheck::Invalid;
}

// 通过检查时返回 curl 只该用的协议：http 只给本机和局域网地址，其余只走 https；不通过时为空。Rust 会先去掉首尾空格再解析，curl 不会，所以原文不以 `http://` 或 `https://` 开头的也不发。
inline std::string_view ai_endpoint_protocol(std::string_view url)
{
    if (check_ai_endpoint(url) != AiEndpointCheck::Allowed)
        return {};
    if (ai_endpoint_detail::starts_with_ignoring_case(url, "https://"))
        return "https";
    if (ai_endpoint_detail::starts_with_ignoring_case(url, "http://"))
        return "http";
    return {};
}

// 通过检查的明文 http 地址只会是本机或局域网，这类请求直连、不走代理：经过代理就等于把 Token 明文交给代理。https 照旧按系统和环境变量的代理设置走。参数是 ai_endpoint_protocol 已经给出的协议而不是地址：调用方检查一次地址、拿同一个结果定协议和是否直连，不为第二个问题再经 C 接口检查一遍；空协议（不通过）不直连。
inline bool ai_endpoint_protocol_connects_directly(std::string_view protocol)
{
    return protocol == "http";
}
} // namespace msime::windows
