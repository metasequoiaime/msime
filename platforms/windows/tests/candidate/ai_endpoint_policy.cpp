#include "AiEndpointPolicy.h"

#include <nlohmann/json.hpp>

#include <iostream>
#include <string>
#include <string_view>

// CMake 把 shared/contracts/ai-endpoint/cases.json 编进 ai_endpoint_cases.inc（逐字节的数组），Wine 里跑测试时不用再找仓库路径。
static const unsigned char kCases[] = {
#include "ai_endpoint_cases.inc"
};

using namespace msime::windows;

int main()
{
    const auto contract = nlohmann::json::parse(std::string(reinterpret_cast<const char *>(kCases), sizeof(kCases)));
    int failures = 0;
    size_t count = 0;
    for (const auto &item : contract.at("cases")) {
        const auto endpoint = item.at("endpoint").get<std::string>();
        const auto expected = item.at("result").get<std::string>();
        const auto check = check_ai_endpoint(endpoint);
        const std::string actual = check == AiEndpointCheck::Allowed           ? "allowed"
                                   : check == AiEndpointCheck::CleartextPublic ? "cleartext_public"
                                                                               : "invalid";
        ++count;
        if (actual != expected) {
            std::cerr << "ai endpoint policy: " << endpoint << " is " << actual << ", expected " << expected << '\n';
            ++failures;
        }
        // 只有通过检查的地址才交给 curl，协议跟着地址的 scheme 走。
        const auto protocol = ai_endpoint_protocol(endpoint);
        if ((expected == "allowed") != !protocol.empty()) {
            std::cerr << "ai endpoint protocol: " << endpoint << '\n';
            ++failures;
        }
    }
    // 非常规写法的 IPv4 不当作局域网地址，它们在 curl 和系统解析器里可能是另一个地址。
    // 百分号编码的主机同样不收：`evil.com%00.local` 原样看以 `.local` 结尾。
    for (const char *endpoint : {"http://010.0.0.1/v1", "http://0x7f000001/v1", "http://10.1/v1", "http://evil.com%00.local/v1",
                                 "http://[fe80::1%25en0]/v1"}) {
        if (check_ai_endpoint(endpoint) == AiEndpointCheck::Allowed) {
            std::cerr << "ai endpoint policy: non-canonical " << endpoint << " allowed\n";
            ++failures;
        }
    }
    // 规则在 Rust：原来这里的 C++ 副本只在主机部分查反斜杠，查询串里的反斜杠照样放行；Rust 整个地址都不收。
    if (check_ai_endpoint("https://api.example.com/v1?q=a\\b") != AiEndpointCheck::Invalid) {
        std::cerr << "ai endpoint policy: a backslash anywhere must be refused, as Rust does\n";
        ++failures;
    }
    // Rust 解析前去掉首尾空格，curl 不会；原文不以协议开头的地址不交给 curl。
    if (check_ai_endpoint(" https://api.example.com/v1") != AiEndpointCheck::Allowed ||
        !ai_endpoint_protocol(" https://api.example.com/v1").empty()) {
        std::cerr << "ai endpoint protocol: a leading space must not reach curl\n";
        ++failures;
    }
    if (ai_endpoint_protocol("http://192.168.1.20:1234/v1") != "http" ||
        ai_endpoint_protocol("HTTPS://api.example.com/v1") != "https") {
        std::cerr << "ai endpoint protocol mismatch\n";
        ++failures;
    }
    // 局域网的 http 请求直连，https 照旧可以走代理；拒绝的地址根本不发。是否直连由已经算出的协议决定，与 AiCandidateWorker、TranslationWorker 里一样：地址只检查一次，同一个结果定协议和是否直连。
    const auto connects_directly = [](std::string_view endpoint) {
        return ai_endpoint_protocol_connects_directly(ai_endpoint_protocol(endpoint));
    };
    if (!connects_directly("http://192.168.1.20:1234/v1") || !connects_directly("HTTP://LOCALHOST:1234/v1") ||
        connects_directly("https://api.example.com/v1") || connects_directly("http://8.8.8.8/v1") ||
        connects_directly(" http://127.0.0.1/v1")) {
        std::cerr << "ai endpoint direct connection mismatch\n";
        ++failures;
    }
    // 不通过检查时协议为空，空协议不直连；只有 http 直连，https 不直连。
    if (ai_endpoint_protocol_connects_directly({}) || ai_endpoint_protocol_connects_directly("https") ||
        !ai_endpoint_protocol_connects_directly("http")) {
        std::cerr << "ai endpoint direct connection by protocol mismatch\n";
        ++failures;
    }
    if (count < 40) {
        std::cerr << "ai endpoint policy: only " << count << " cases\n";
        ++failures;
    }
    return failures == 0 ? 0 : 1;
}
