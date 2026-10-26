#include "../../settings/CloudClipboardRoute.h"

#include <cassert>
#include <string>
#include <string_view>

namespace {
struct Response {
  bool ok;
  std::wstring value;
};
}

int main() {
  using namespace msime::settings;
  for (const auto source : {L"account", L"relay", L"unknown", L""}) {
    for (const bool source_ok : {false, true}) {
      for (const bool backend_ok : {false, true}) {
        int relay_calls = 0, account_calls = 0, invalid_calls = 0, decode_calls = 0;
        const auto response = cloud_clipboard_request(
            [=] { return Response{source_ok, source}; },
            [&](Response const &value) { ++decode_calls; return value.value; },
            [&] { ++relay_calls; return Response{backend_ok, L"relay_response"}; },
            [&] { ++account_calls; return Response{backend_ok, L"account_response"}; },
            [&] { ++invalid_calls; return Response{false, L"invalid_source"}; });
        if (!source_ok) {
          assert(!response.ok && response.value == source);
          assert(decode_calls == 0 && relay_calls == 0 && account_calls == 0 && invalid_calls == 0);
        } else if (std::wstring_view(source) == L"relay") {
          assert(response.ok == backend_ok && response.value == L"relay_response");
          assert(relay_calls == 1 && account_calls == 0 && invalid_calls == 0);
        } else if (std::wstring_view(source) == L"account") {
          assert(response.ok == backend_ok && response.value == L"account_response");
          assert(relay_calls == 0 && account_calls == 1 && invalid_calls == 0);
        } else {
          assert(!response.ok && response.value == L"invalid_source");
          assert(relay_calls == 0 && account_calls == 0 && invalid_calls == 1);
        }
      }
    }
  }
  assert(cloud_clipboard_failure(L"relay_unauthorized") == CloudClipboardState::relay_unauthorized);
  assert(cloud_clipboard_failure(L"unauthorized") == CloudClipboardState::signed_out);
  assert(cloud_clipboard_failure(L"account_unauthorized") == CloudClipboardState::signed_out);
  assert(cloud_clipboard_failure(L"clipboard_device_pending") == CloudClipboardState::pending);
  assert(cloud_clipboard_failure(L"clipboard_sync_disabled") == CloudClipboardState::disabled);
  assert(cloud_clipboard_failure(L"relay_unavailable") == CloudClipboardState::unavailable);
  // WebDAV 网盘的回退和限流不能当成未登录水杉账号，也不能混进笼统的「服务不可用」。
  assert(cloud_clipboard_failure(L"webdav_rollback") == CloudClipboardState::webdav_rollback);
  assert(cloud_clipboard_failure(L"webdav_rate_limited") == CloudClipboardState::webdav_rate_limited);
  assert(webdav_error_message(L"relay_not_configured") == L"地址、账号或应用密码不完整");
  assert(webdav_error_message(L"webdav_unsupported").find(L"独占创建、列目录") != std::wstring_view::npos);
  assert(webdav_error_message(L"relay_unauthorized") == L"网盘拒绝了这个账号或应用密码");
  assert(webdav_error_message(L"relay_unavailable").find(L"证书须由公网信任的机构签发") != std::wstring_view::npos);
  assert(webdav_error_message(L"webdav_rate_limited").find(L"每 30 分钟 600 次") != std::wstring_view::npos);
  assert(webdav_error_message(L"webdav_rollback").find(L"请重新测试连接并保存") != std::wstring_view::npos);
  // 不认识的码只给通用说明，不冒充成上面任何一种原因。
  for (const auto code : {L"invalid_cloud_clipboard", L"account_storage", L""}) {
    const auto message = webdav_error_message(code);
    assert(message == L"测试连接失败，请稍后重试");
    for (const auto known : {L"relay_not_configured", L"webdav_unsupported", L"relay_unauthorized",
                             L"relay_unavailable", L"webdav_rate_limited", L"webdav_rollback"})
      assert(message != webdav_error_message(known));
  }
  assert(webdav_explanation.find(L"网盘只保存密文") != std::wstring_view::npos);
  assert(webdav_url_placeholder == L"https://dav.jianguoyun.com/dav/msime");
}
