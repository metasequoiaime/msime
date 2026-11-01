#pragma once

#include <curl/curl.h>

#include <memory>
#include <string>
#include <string_view>

namespace msime::voice {

using CurlHeaderList = std::unique_ptr<curl_slist, decltype(&curl_slist_free_all)>;

// 向 CURLOPT_HTTPHEADER 使用的链表追加一项，并始终让容器持有真正的表头。
// 非空链表追加时，libcurl 返回的仍是原表头；不能把它再次交给 reset。
inline bool append_curl_header(CurlHeaderList &list, std::string_view value) {
  const std::string copy(value);
  auto *head = curl_slist_append(list.get(), copy.c_str());
  if (!head)
    return false;
  if (!list)
    list.reset(head);
  return true;
}

} // namespace msime::voice
