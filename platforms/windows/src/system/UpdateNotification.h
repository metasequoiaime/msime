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
// 两次调用之间多久看一眼偏好里的通道。
inline constexpr std::chrono::minutes update_channel_poll{1};
// 通道变了以后至少等这么久才调用：换通道几乎总是在设置窗口的「关于」页上做的，用户多半当场点了「检查更新」、页上已经写着找到的构建，马上再弹一条系统通知说同一个版本是重复的。等满一个检查间隔还没装，再提醒才有用。到时共享层见到和记录不同的通道会立刻重查，不受一天一查的节流限制。
inline constexpr std::chrono::minutes update_channel_settle{60};
// 系统不接受通知（全屏游戏或演示、锁屏、新用户登录后的安静时间，见 SHQueryUserNotificationState）时，要提醒的构建先留在内存里，每隔这么久再看一次。
inline constexpr std::chrono::minutes update_notice_retry{1};
// 最多这样等多久，到时不管系统状态照样弹出：QUNS_BUSY 有误报（FullscreenForeground.h），一直等下去会永远不提醒；弹出后图标留着，系统压下的通知在操作中心里也还在。
inline constexpr std::chrono::minutes update_notice_max_defer{120};

// 要提醒的构建现在能不能弹出：系统接受通知，或者已经等满 `update_notice_max_defer`。
inline bool update_notice_may_show(bool accepts_notifications, std::chrono::minutes deferred) {
  return accepts_notifications || deferred >= update_notice_max_defer;
}

// 本程序编译成的处理器架构，写法同 Rust 的 `std::env::consts::ARCH`（与 settings/AboutPageModel.h 的 build_architecture 相同，那份是宽字符，Server 不引用设置窗口的头文件）。认不出时为空。
constexpr std::string_view update_build_architecture() {
#if defined(_M_ARM64) || defined(__aarch64__)
  return "aarch64";
#elif defined(_M_X64) || defined(__x86_64__)
  return "x86_64";
#elif defined(_M_IX86) || defined(__i386__)
  return "x86";
#else
  return {};
#endif
}

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

// `msime_client_update_background_check` 的请求：`directory` 是本用户的更新记录目录（UTF-8，common/UpdateRecordDirectory.h，与设置窗口读 `msime_client_update_pending` 用的是同一个目录）。普通构建照设置窗口不带架构（同一个安装程序同时支持 x64 和 Arm64），带上偏好里的通道。Windows 7 构建的平台是 `windows7`，只有正式发布，不带通道；它的安装包分 x64 和 x86 两个，共享层没有架构时找不到任何发布，所以照 Win7 专用设置带上本程序编译成的架构 `architecture`（`update_build_architecture()`，为空时不带）。
inline nlohmann::json update_background_request(const std::string &directory, bool windows7,
                                                const std::string &version,
                                                const std::string &edition,
                                                const std::string &channel,
                                                std::string_view architecture) {
  nlohmann::json request{{"directory", directory},
                         {"platform", windows7 ? "windows7" : "windows"},
                         {"current_version", version},
                         {"edition", edition}};
  if (!windows7)
    request["channel"] = std::string(update_channel::normalize(std::string_view(channel)));
  else if (!architecture.empty())
    request["arch"] = std::string(architecture);
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

// 托盘气泡的标题、正文和图标的提示。标题是「<产品名>有新版本 vX」，产品名按版本取（Server 传 MSIME_EDITION_DISPLAY_NAME，如「水杉输入法」「水杉五笔」），几个版本同时安装时分得清是哪个；正文写明是哪个通道的哪个构建，再说点这里打开设置的「关于」页。Windows 10/11 把它显示成通知，Windows 7 显示成气泡；标题最多 63 个字符、正文最多 255 个，版本号再长也装得下。气泡收起后图标还留着，提示在标题下面再说一句点它查看、右键移除。
struct UpdateNotificationText {
  std::wstring title;
  std::wstring body;
  std::wstring tip;
};

inline UpdateNotificationText update_notification_text(const UpdateNotice &notice,
                                                       std::wstring_view product_name) {
  UpdateNotificationText text;
  text.title = std::wstring(product_name) + L"有新版本 v" + notice.version;
  text.tip = text.title + L"\n点击查看，右键移除此提醒";
  text.body = update_channel::build_label(notice.channel, notice.version, notice.build_run);
  // 西文和汉字之间留一个空格，全角括号后面不留。
  if (text.body.back() != L'）')
    text.body.append(L" ");
  text.body.append(L"已发布。点这里打开设置的「关于」页查看并下载。");
  return text;
}

} // namespace msime::windows
