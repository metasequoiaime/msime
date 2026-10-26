#pragma once

#include <string_view>

namespace msime::settings {

// 配置读取失败、来源未知和中继请求失败都直接返回，不能把内容改发给账号。WebDAV 网盘在来源里也报 relay，由中继入口按偏好自己分派，这里不用分辨。
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

// webdav_rollback 要用户重新测试连接才会恢复，webdav_rate_limited 稍后自己会好，两者都不能提示登录水杉账号。
enum class CloudClipboardState {
  checking, ready, signed_out, relay_unauthorized, pending, disabled, unavailable,
  webdav_rollback, webdav_rate_limited
};

inline CloudClipboardState cloud_clipboard_failure(std::wstring_view code) {
  if (code == L"relay_unauthorized")
    return CloudClipboardState::relay_unauthorized;
  if (code == L"webdav_rollback")
    return CloudClipboardState::webdav_rollback;
  if (code == L"webdav_rate_limited")
    return CloudClipboardState::webdav_rate_limited;
  const bool suffixed = code.size() >= 13 && code.substr(code.size() - 13) == L"_unauthorized";
  if (code == L"unauthorized" || suffixed)
    return CloudClipboardState::signed_out;
  if (code == L"clipboard_device_pending")
    return CloudClipboardState::pending;
  if (code == L"clipboard_sync_disabled")
    return CloudClipboardState::disabled;
  return CloudClipboardState::unavailable;
}

// WebDAV 设置的说明，WinUI 和 Windows 7 两个设置窗口共用同一段话。
inline constexpr std::wstring_view webdav_explanation =
    L"内容在本机加密，网盘只保存密文；打开面板或复制时同步，没有实时推送；坚果云免费账号每 30 分钟 600 次请求与其他应用共用；建议使用专用账号或专用应用密码，移除设备后更换应用密码；已删除的密文可能留在网盘回收站。";

// 文件夹地址输入框里的示例。
inline constexpr std::wstring_view webdav_url_placeholder = L"https://dav.jianguoyun.com/dav/msime";

// 「测试连接」通过时的说明。
inline constexpr std::wstring_view webdav_probe_passed = L"连接正常，可以保存";

// msime_client_cloud_clipboard_webdav_probe 和经网盘的列表、发送请求给出的错误码对应的说明；不认识的码给通用说明，不当成网盘不合格。
inline std::wstring_view webdav_error_message(std::wstring_view code) {
  if (code == L"relay_not_configured")
    return L"地址、账号或应用密码不完整";
  if (code == L"webdav_unsupported")
    return L"这个网盘不支持云剪贴板需要的文件操作（独占创建、列目录），请换一个 WebDAV 服务或文件夹";
  if (code == L"relay_unauthorized")
    return L"网盘拒绝了这个账号或应用密码";
  if (code == L"relay_unavailable")
    return L"连不上网盘，请检查地址和网络；证书须由公网信任的机构签发";
  if (code == L"webdav_rate_limited")
    return L"网盘暂时限制了请求次数（坚果云免费账号每 30 分钟 600 次，与其他应用共用），请稍后再试";
  if (code == L"webdav_rollback")
    return L"网盘上的云剪贴板记录比这台设备读到过的旧，或者被改动过，已停止同步。若是你自己清理过网盘文件夹，请重新测试连接并保存";
  return L"测试连接失败，请稍后重试";
}

} // namespace msime::settings
