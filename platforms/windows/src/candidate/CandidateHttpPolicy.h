#pragma once

#include <algorithm>
#include <string_view>

namespace msime::windows
{
inline bool valid_candidate_url(std::string_view url, bool allow_http = false)
{
    if (url.size() > 2048 || url.find('#') != std::string_view::npos ||
        std::any_of(url.begin(), url.end(), [](unsigned char ch) {
               return ch < 32 || ch == 127;
           }))
        return false;
    const bool https = url.rfind("https://", 0) == 0;
    const bool http = url.rfind("http://", 0) == 0;
    if (!https && !(allow_http && http))
        return false;
    const auto scheme_length = https ? std::string_view("https://").size()
                                     : std::string_view("http://").size();
    const auto authority_end = url.find_first_of("/?#", scheme_length);
    const auto authority = url.substr(scheme_length,
                                      authority_end == std::string_view::npos
                                          ? url.size() - scheme_length
                                          : authority_end - scheme_length);
    if (authority.empty() || authority.find('@') != std::string_view::npos)
        return false;
    if (https)
        return true;
    if (authority.front() == '[') {
        const auto close = authority.find(']');
        return close != std::string_view::npos &&
               authority.substr(1, close - 1) == "::1" &&
               (close == authority.size() - 1 ||
                authority[close + 1] == ':');
    }
    const auto colon = authority.find(':');
    const auto host = authority.substr(0, colon);
    return host == "localhost" || host == "127.0.0.1";
}

inline bool valid_candidate_header_name(std::string_view name)
{
    if (name.empty())
        return false;
    return std::all_of(name.begin(), name.end(), [](unsigned char ch) {
        return (ch >= 'a' && ch <= 'z') || (ch >= 'A' && ch <= 'Z') ||
               (ch >= '0' && ch <= '9') ||
               std::string_view("!#$%&'*+-.^_`|~").find(static_cast<char>(ch)) !=
                   std::string_view::npos;
    });
}

inline bool valid_candidate_header_value(std::string_view value)
{
    return std::all_of(value.begin(), value.end(), [](unsigned char ch) {
        return ch == '\t' || (ch >= 0x20 && ch != 0x7f);
    });
}
} // namespace msime::windows
