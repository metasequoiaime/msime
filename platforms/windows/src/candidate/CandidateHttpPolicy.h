#pragma once

#include <algorithm>
#include <string_view>

namespace msime::windows
{
inline bool valid_candidate_url(std::string_view url, bool allow_http = false)
{
    const bool https = url.rfind("https://", 0) == 0;
    const bool http = allow_http && url.rfind("http://", 0) == 0;
    return url.size() <= 2048 && (https || http) &&
           std::none_of(url.begin(), url.end(), [](unsigned char ch) {
               return ch < 32 || ch == 127;
           });
}
} // namespace msime::windows
