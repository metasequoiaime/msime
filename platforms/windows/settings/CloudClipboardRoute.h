#pragma once

#include <string_view>

namespace msime::settings {

// 配置读取失败、来源未知和中继请求失败都直接返回，不能把内容改发给账号。
template <typename Source, typename Decode, typename Relay, typename Account, typename Invalid>
auto cloud_clipboard_request(Source source, Decode decode, Relay relay, Account account,
                             Invalid invalid) {
  auto response = source();
  if (!response.ok)
    return response;
  const auto selected = decode(response);
  if (selected == L"relay")
    return relay();
  if (selected == L"account")
    return account();
  return invalid();
}

enum class CloudClipboardState {
  checking, ready, signed_out, relay_unauthorized, pending, disabled, unavailable
};

inline CloudClipboardState cloud_clipboard_failure(std::wstring_view code) {
  if (code == L"relay_unauthorized")
    return CloudClipboardState::relay_unauthorized;
  const bool suffixed = code.size() >= 13 && code.substr(code.size() - 13) == L"_unauthorized";
  if (code == L"unauthorized" || suffixed)
    return CloudClipboardState::signed_out;
  if (code == L"clipboard_device_pending")
    return CloudClipboardState::pending;
  if (code == L"clipboard_sync_disabled")
    return CloudClipboardState::disabled;
  return CloudClipboardState::unavailable;
}

} // namespace msime::settings
