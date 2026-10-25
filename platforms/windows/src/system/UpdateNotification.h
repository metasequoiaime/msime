#pragma once
#include "../../common/UpdateChannelText.h"
#include <chrono>
#include <cstdint>
#include <nlohmann/json.hpp>
#include <optional>
#include <string>
#include <string_view>

// Server 后台更新检查里不碰 Windows API 的部分：从偏好读通道、拼 `msime_client_update_background_check` 的请求、读它的回答、写通知的文字。线程和托盘气泡在 UpdateNotifier.cpp；这里方便在主机上单独测试（tests/system/update_notification.cpp）。到没到时间、只提醒一次、换通道立刻重查都在共享层（crates/client-services/src/update_check/background.rs），这里不重复。
namespace msime::windows {

// 启动后等多久才第一次调用：登录时一起启动的程序很多，晚几分钟不挡开机，也不在用户刚开始打字时联网。
inline constexpr std::chrono::minutes update_first_check_delay{5};
// 之后多久调用一次。共享层自己按一天一查、失败六小时后再试节流，没到时间时只读一个小文件，每小时问一次不联网。
inline constexpr std::chrono::minutes update_check_interval{60};
// 两次调用之间多久看一眼偏好里的通道。通道变了就不等到点，马上调用一次：共享层见到和记录不同的通道会立刻重查。
inline constexpr std::chrono::minutes update_channel_poll{1};

// 偏好里的 `update_channel`，按 UpdateChannelText.h 的规则归一成 stable、beta、nightly。
inline std::string update_channel_preference(const nlohmann::json &preferences) {
  std::string_view stored;
  if (preferences.is_object()) {
    const auto value = preferences.find("update_channel");
    if (value != preferences.end() && value->is_string())
      stored = value->get_ref<const std::string &>();
  }
  return std::string(update_channel::normalize(stored));
}

// `msime_client_update_background_check` 的请求：`directory` 是 Server 的状态根（UTF-8，与设置窗口读 `msime_client_update_pending` 用的是同一个目录），版本 id 照设置窗口不带架构（同一个安装程序同时支持 x64 和 Arm64）。Windows 7 构建的平台是 `windows7`，只有正式发布，不带通道；其他构建带上偏好里的通道。
inline nlohmann::json update_background_request(const std::string &directory, bool windows7,
                                                const std::string &version,
                                                const std::string &edition,
                                                const std::string &channel) {
  nlohmann::json request{{"directory", directory},
                         {"platform", windows7 ? "windows7" : "windows"},
                         {"current_version", version},
                         {"edition", edition}};
  if (!windows7)
    request["channel"] = std::string(update_channel::normalize(std::string_view(channel)));
  return request;
}

// 一个要提醒的构建，取自回答里的 `notify`。
struct UpdateNotice {
  // 版本号，如 1.4.0。
  std::wstring version;
  // 这个构建所属的通道：stable、beta 或 nightly（UpdateChannelText.h 的常量）。
  std::wstring channel;
  // 构建它的发布 workflow 运行 ID，每日构建靠它区分同一版本号的不同构建。
  std::optional<std::uint64_t> build_run;
  // 发布页，打不开设置窗口时用浏览器打开它；不在本仓库的 tag 页下时为空。
  std::wstring release_url;
};

enum class UpdateCheckStatus {
  // 开发构建或设置了 MSIME_DISABLE_UPDATE_CHECK：本进程里以后也不会变，后台线程可以结束。
  disabled,
  not_due,
  unavailable,
  checked,
  // 回答读不出来、请求被拒或者记录写不进去：下一轮照常再问。
  failed,
};

struct UpdateBackgroundResult {
  UpdateCheckStatus status = UpdateCheckStatus::failed;
  std::optional<UpdateNotice> notice;
};

namespace detail {
// 版本号和发布页地址都只是 ASCII：认不出的字符不往通知里放，免得把回答里的任意文本显示给用户。
inline bool ascii_in(std::string_view value, std::string_view extra) {
  for (const char c : value) {
    const bool alnum = (c >= '0' && c <= '9') || (c >= 'A' && c <= 'Z') || (c >= 'a' && c <= 'z');
    if (!alnum && extra.find(c) == std::string_view::npos)
      return false;
  }
  return !value.empty();
}
inline std::wstring widen_ascii(std::string_view value) { return std::wstring(value.begin(), value.end()); }
} // namespace detail

// 读 `msime_client_update_background_check` 的回答（标准的 {ok, value|error}）。`notify` 是共享层这次让提醒、而且已经记成提醒过的构建；版本号不合形状（只允许字母、数字和 `.+-`）或通道认不出时不提醒，免得弹一条说不清是什么的通知。
inline UpdateBackgroundResult parse_update_background_response(std::string_view response) {
  UpdateBackgroundResult result;
  const auto document = nlohmann::json::parse(response, nullptr, false);
  if (document.is_discarded() || !document.is_object())
    return result;
  const auto ok = document.find("ok");
  const auto value = document.find("value");
  if (ok == document.end() || !ok->is_boolean() || !ok->get<bool>() || value == document.end() ||
      !value->is_object())
    return result;
  const auto status = value->find("status");
  if (status == value->end() || !status->is_string())
    return result;
  const auto &name = status->get_ref<const std::string &>();
  if (name == "disabled")
    result.status = UpdateCheckStatus::disabled;
  else if (name == "not_due")
    result.status = UpdateCheckStatus::not_due;
  else if (name == "unavailable")
    result.status = UpdateCheckStatus::unavailable;
  else if (name == "checked")
    result.status = UpdateCheckStatus::checked;
  else
    return result;
  const auto notify = value->find("notify");
  if (notify == value->end() || !notify->is_object())
    return result;
  const auto version = notify->find("version");
  if (version == notify->end() || !version->is_object())
    return result;
  const auto display = version->find("display");
  if (display == version->end() || !display->is_string() ||
      !detail::ascii_in(display->get_ref<const std::string &>(), ".+-"))
    return result;
  UpdateNotice notice;
  notice.version = detail::widen_ascii(display->get_ref<const std::string &>());
  // 以前的共享层不报通道，那时只有正式发布。
  std::string channel = "stable";
  if (const auto field = notify->find("channel"); field != notify->end()) {
    if (!field->is_string())
      return result;
    channel = field->get<std::string>();
  }
  if (channel != "stable" && channel != "beta" && channel != "nightly")
    return result;
  notice.channel = detail::widen_ascii(channel);
  if (const auto run = notify->find("build_run"); run != notify->end() && run->is_number_unsigned())
    notice.build_run = run->get<std::uint64_t>();
  constexpr std::string_view releases = "https://github.com/metasequoiaime/msime/releases/tag/";
  if (const auto url = notify->find("release_url"); url != notify->end() && url->is_string()) {
    const auto &text = url->get_ref<const std::string &>();
    if (text.size() > releases.size() && text.compare(0, releases.size(), releases) == 0 &&
        detail::ascii_in(std::string_view(text).substr(releases.size()), "._+-"))
      notice.release_url = detail::widen_ascii(text);
  }
  result.notice = std::move(notice);
  return result;
}

// 托盘气泡的标题和正文。标题是「水杉输入法有新版本 vX」，正文写明是哪个通道的哪个构建，再说点这里打开设置的「关于」页。Windows 10/11 把它显示成通知，Windows 7 显示成气泡；标题最多 63 个字符、正文最多 255 个，版本号再长也装得下。
struct UpdateNotificationText {
  std::wstring title;
  std::wstring body;
};

inline UpdateNotificationText update_notification_text(const UpdateNotice &notice) {
  UpdateNotificationText text;
  text.title = L"水杉输入法有新版本 v" + notice.version;
  text.body = update_channel::build_label(notice.channel, notice.version, notice.build_run);
  // 西文和汉字之间留一个空格，全角括号后面不留。
  if (text.body.back() != L'）')
    text.body.append(L" ");
  text.body.append(L"已发布。点这里打开设置的「关于」页查看并下载。");
  return text;
}

} // namespace msime::windows
