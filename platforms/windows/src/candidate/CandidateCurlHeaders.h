#pragma once

#include "CandidateHttpPolicy.h"

#include <curl/curl.h>
#include <nlohmann/json.hpp>

#include <memory>
#include <optional>
#include <string>

namespace msime::windows
{
using CurlHeaderList = std::unique_ptr<curl_slist, decltype(&curl_slist_free_all)>;

// 把请求描述里的 headers 对象转成交给 CURLOPT_HTTPHEADER 的链表。名字或值不合法、或者追加失败时返回 nullopt；空对象得到空链表。
inline std::optional<CurlHeaderList> candidate_curl_headers(const nlohmann::json &headers)
{
    CurlHeaderList list(nullptr, curl_slist_free_all);
    if (!headers.is_object())
        return std::nullopt;
    for (auto it = headers.begin(); it != headers.end(); ++it) {
        if (!valid_candidate_header_name(it.key()) || !it.value().is_string() || it.key().size() > 128)
            return std::nullopt;
        const auto &value = it.value().get_ref<const std::string &>();
        if (value.size() > 8192 || !valid_candidate_header_value(value))
            return std::nullopt;
        const auto line = it.key() + ": " + value;
        auto *head = curl_slist_append(list.get(), line.c_str());
        if (!head)
            return std::nullopt;
        // curl_slist_append 返回的是表头：只有第一次追加得到新表，之后返回的就是已经持有的指针。unique_ptr::reset 收到同一个指针也会释放旧值，整条表随之被释放，curl_easy_perform 再读这些头部就是读已释放的内存（Windows 0.3.0 在 libcurl 里的 EXCEPTION_ACCESS_VIOLATION），所以只在表还空时接管。
        if (!list)
            list.reset(head);
    }
    return list;
}
} // namespace msime::windows
