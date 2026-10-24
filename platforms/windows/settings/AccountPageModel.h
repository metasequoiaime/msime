#pragma once
#include <cstdint>
#include <map>
#include <optional>
#include <string>
#include <string_view>
#include "SettingsText.h"

// 「账号与同步」页的展示规则：错误文案、确认文字、验证码输入和倒计时、账号 ID 短写、头像首字、加入日期，以及设置同步卡片的说明。昵称规范化、校验和默认显示名经 `msime_client_account_name` 调共享 Rust；账号操作经 `msime_client_account`。不依赖 Windows 和 WinRT 头文件，方便单独测试。
namespace msime::settings::account {

// 登录目标的首尾去掉空白；昵称规范化由共享 Rust 处理。
inline std::wstring trim_login_target(std::wstring_view value) {
  return text::trim(value);
}

// 账号 ID 的前六个字符转成大写，ID 是 ASCII 的 UUID。
inline std::wstring short_id(std::wstring_view id) {
  std::wstring head(id.substr(0, 6));
  for (auto &c : head)
    if (c >= L'a' && c <= L'z')
      c = static_cast<wchar_t>(c - L'a' + L'A');
  return head;
}

// 编辑资料里「账号 ID」按钮的文字：「#」加 ID 的前六位。
inline std::wstring id_label(std::wstring_view id) { return L"#" + short_id(id); }

// 没有头像或头像还没取到时，头像里显示的那一个字：名字的第一个字符（一个代理对整个取，免得画出半个字）。
inline std::wstring initial(std::wstring_view name) {
  if (name.empty())
    return {};
  const size_t length = name.size() > 1 && name[0] >= 0xD800 && name[0] <= 0xDBFF &&
                                name[1] >= 0xDC00 && name[1] <= 0xDFFF
                            ? 2
                            : 1;
  return std::wstring(name.substr(0, length));
}

using ProviderNames = std::map<std::wstring, std::wstring, std::less<>>;

// 登录方式名称由共享资料响应提供，宿主只查表。
inline std::wstring provider_name(std::wstring_view provider, const ProviderNames &names) {
  const auto found = names.find(provider);
  return found == names.end() ? std::wstring(provider) : found->second;
}

// isAccountCancellation：取消的操作不报错。
inline bool cancelled(std::wstring_view code) { return code == L"account_cancelled"; }

// accountMessage：审核类的三种先按 communityModerationMessage 说，其余按账号错误码；不认识的码（含设置写回失败的 conflict、storage）都说服务暂不可用。
inline std::wstring error_message(std::wstring_view code) {
  if (code == L"community_blocked_content" || code == L"account_blocked_content")
    return L"内容包含不允许发布的词语，请修改后再提交";
  if (code == L"community_screening_unavailable" || code == L"account_screening_unavailable")
    return L"审核服务暂时不可用，请稍后重试";
  if (code == L"community_account_banned" || code == L"account_banned")
    return L"该账号已被封禁，暂时无法使用账号相关功能";
  if (code == L"account_invalid")
    return L"填写的内容无效，请检查后重试。";
  if (code == L"account_avatar_invalid")
    return L"请选择 1 MiB 以内的 PNG 或 JPEG 图片。";
  if (code == L"account_unauthorized")
    return L"登录已失效，请重新登录。";
  if (code == L"account_conflict")
    return L"云端设置已被其他设备更新，请刷新后重新确认。";
  if (code == L"account_rate_limited")
    return L"操作过于频繁，请稍后再试。";
  if (code == L"account_storage")
    return L"无法安全读取登录状态，请检查设备安全设置。";
  if (code == L"account_cancelled")
    return L"操作已取消，请重试。";
  if (code == L"account_network")
    return L"网络连接失败，请检查网络后重试。";
  if (code == L"account_recent_login_required")
    return L"需要重新登录后再注销：请先退出登录，重新登录后再试。";
  return L"账号服务暂不可用，请稍后再试。";
}

// AccountConfirmation 的两种（桌面账号页只确认这两种；「退出登录」不确认）。
enum class Confirmation { logout_all, remove };

struct ConfirmationText {
  std::wstring_view label;
  std::wstring_view message;
};

constexpr ConfirmationText confirmation_text(Confirmation action) {
  return action == Confirmation::logout_all
             ? ConfirmationText{L"确认退出所有设备", L"退出所有设备后，所有设备都需要重新登录。"}
             : ConfirmationText{L"确认注销账号",
                                L"注销账号将删除已发布皮肤、评分及其他云端账号数据，无法撤销。"};
}

// 验证码输入框的 onChange：只留数字，最多 6 位。
inline std::wstring code_digits(std::wstring_view value) {
  std::wstring digits;
  for (const wchar_t c : value) {
    if (c >= L'0' && c <= L'9')
      digits.push_back(c);
    if (digits.size() == 6)
      break;
  }
  return digits;
}

// /^\d{6}$/
inline bool six_digits(std::wstring_view code) {
  if (code.size() != 6)
    return false;
  for (const wchar_t c : code)
    if (c < L'0' || c > L'9')
      return false;
  return true;
}

// 发出验证码后过多久才能重发。
constexpr int64_t resend_interval_ms = 60'000;

// `Math.max(0, Math.ceil((resendAt - now) / 1000))`。
constexpr int64_t resend_seconds(int64_t remaining_ms) {
  return remaining_ms <= 0 ? 0 : (remaining_ms + 999) / 1000;
}

inline std::wstring resend_label(int64_t seconds) {
  return seconds > 0 ? std::to_wstring(seconds) + L" 秒后可重新发送" : L"获取验证码";
}

inline std::wstring login_label(bool expired, bool busy) {
  return expired ? L"验证码已过期，请重新获取" : busy ? L"正在登录…" : L"登录";
}

// ISO 8601 的时间（账号的 createdAt，如 2026-10-11T05:40:05.123Z 或带 +08:00；只有日期时按 UTC 零点，与 JavaScript 的 Date 相同）换成 Unix 毫秒。读不懂时为空，页面就不写日期（formatZhDate 对无效日期给空串）。
inline std::optional<int64_t> parse_iso_time(std::wstring_view value) {
  size_t at = 0;
  auto number = [&](size_t digits) -> std::optional<int> {
    if (at + digits > value.size())
      return std::nullopt;
    int result = 0;
    for (size_t i = 0; i < digits; ++i) {
      const wchar_t c = value[at + i];
      if (c < L'0' || c > L'9')
        return std::nullopt;
      result = result * 10 + (c - L'0');
    }
    at += digits;
    return result;
  };
  auto expect = [&](wchar_t c) {
    if (at < value.size() && value[at] == c) {
      ++at;
      return true;
    }
    return false;
  };
  const auto year = number(4);
  if (!year || !expect(L'-'))
    return std::nullopt;
  const auto month = number(2);
  if (!month || !expect(L'-'))
    return std::nullopt;
  const auto day = number(2);
  if (!day)
    return std::nullopt;
  int hour = 0, minute = 0, second = 0, millisecond = 0;
  int64_t offset_minutes = 0;
  if (at < value.size()) {
    if (!expect(L'T') && !expect(L't') && !expect(L' '))
      return std::nullopt;
    const auto h = number(2);
    if (!h || !expect(L':'))
      return std::nullopt;
    const auto m = number(2);
    if (!m)
      return std::nullopt;
    hour = *h;
    minute = *m;
    if (expect(L':')) {
      const auto s = number(2);
      if (!s)
        return std::nullopt;
      second = *s;
      if (expect(L'.') || expect(L',')) {
        int scale = 100;
        size_t digits = 0;
        while (at < value.size() && value[at] >= L'0' && value[at] <= L'9') {
          millisecond += (value[at] - L'0') * scale;
          scale /= 10;
          ++at;
          ++digits;
        }
        if (digits == 0)
          return std::nullopt;
      }
    }
    if (expect(L'Z') || expect(L'z')) {
    } else if (at < value.size() && (value[at] == L'+' || value[at] == L'-')) {
      const bool negative = value[at] == L'-';
      ++at;
      const auto oh = number(2);
      if (!oh)
        return std::nullopt;
      expect(L':');
      const auto om = number(2);
      if (!om || *oh > 23 || *om > 59)
        return std::nullopt;
      offset_minutes = (negative ? -1 : 1) * (static_cast<int64_t>(*oh) * 60 + *om);
    } else {
      // 没有时区的日期时间 JavaScript 按本地时间读；账号服务总带时区，读不懂就不写。
      return std::nullopt;
    }
    if (at != value.size())
      return std::nullopt;
  }
  if (*month < 1 || *month > 12 || hour > 23 || minute > 59 || second > 59)
    return std::nullopt;
  constexpr int month_days[] = {31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31};
  if (*day < 1 || *day > month_days[*month - 1])
    return std::nullopt;
  const bool leap = (*year % 4 == 0 && *year % 100 != 0) || *year % 400 == 0;
  if (*month == 2 && *day == 29 && !leap)
    return std::nullopt;
  // Howard Hinnant 的 days_from_civil。
  const int y = *year - (*month <= 2 ? 1 : 0);
  const int era = (y >= 0 ? y : y - 399) / 400;
  const int yoe = y - era * 400;
  const int mp = (*month + 9) % 12;
  const int doy = (153 * mp + 2) / 5 + *day - 1;
  const int doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
  const int64_t days = static_cast<int64_t>(era) * 146097 + doe - 719468;
  return ((days * 24 + hour) * 60 + minute - offset_minutes) * 60'000 + second * 1000LL +
         millisecond;
}

// toLocaleDateString("zh-CN")：年/月/日，月和日不补零。
inline std::wstring zh_date(int year, int month, int day) {
  return std::to_wstring(year) + L"/" + std::to_wstring(month) + L"/" + std::to_wstring(day);
}

// 未登录时那张登录卡片的标题和说明（`account-page.tsx` 桌面那一支，宿主有 `settingsSync`）：没选验证码方式时是欢迎，选了以后标题换成「邮箱登录」或「手机号登录」，说明换成验证码那句。
constexpr std::wstring_view sign_in_welcome = L"欢迎来到水杉";
constexpr std::wstring_view sign_in_description =
    L"登录后在设备之间同步设置和词库，还可以发布你的候选窗口皮肤";
constexpr std::wstring_view sign_in_code_description = L"我们会发送一个 6 位验证码完成登录";

// 一种登录方式都没有时登录按钮列里的那句：登录方式读到了但一个都没开、正在重读、读失败。
constexpr std::wstring_view providers_unavailable(bool loaded, bool busy) {
  return loaded ? L"当前没有可用的验证码登录方式，请稍后重试。"
         : busy ? L"正在加载登录方式…"
                : L"登录方式加载失败，请重试。";
}

// Windows 的设置同步卡片（createDesktopSettingsSync）：同步的范围和应用后的提示；两种确认的文字与按钮（SettingsSyncCard 的桌面卡片）。
constexpr std::wstring_view sync_description =
    L"同步输入方案、繁体输出、双拼和五笔版本、词库学习、调频和标点。凭据、联网授权及输入内容不会随设置上传。";
constexpr std::wstring_view sync_applied = L"已应用云端设置。";
constexpr std::wstring_view sync_uploaded = L"本机设置已上传。";
constexpr std::wstring_view sync_upload_confirmation =
    L"将更新云端对应设置，并保留其他平台专属设置。版本冲突时不会自动覆盖。";
constexpr std::wstring_view sync_apply_confirmation =
    L"将替换本机对应设置，不会下载词库或开启数据上传。";

// 「绑定 QQ 群助手」卡片（`qq-link-card.tsx`）。绑定码输入框的值去掉空白和连字符（`code.replace(/[\s-]/g, "")`），正好 8 位数字时才能点「绑定」。
inline std::wstring qq_link_digits(std::wstring_view value) {
  std::wstring digits;
  for (const wchar_t c : value)
    if (!text::js_space(c) && c != L'-')
      digits.push_back(c);
  return digits;
}

inline bool eight_digits(std::wstring_view code) {
  if (code.size() != 8)
    return false;
  for (const wchar_t c : code)
    if (c < L'0' || c > L'9')
      return false;
  return true;
}

// qqLinkMessage：绑定失败的几种各有说法，其余按账号错误码。
inline std::wstring qq_link_message(std::wstring_view code) {
  if (code == L"account_invalid")
    return L"绑定码是 8 位数字，请检查后重试。";
  if (code == L"account_qq_link_code_not_found")
    return L"绑定码不对或已过期（10 分钟内有效，只能用一次）。请在 QQ 群里重新发 /bind 拿一个新的。";
  if (code == L"account_qq_link_limit")
    return L"这个账号已经绑定了 10 个 QQ 群友，先解绑一个再试。";
  if (code == L"account_rate_limited")
    return L"尝试次数太多，请一小时后再试。";
  return error_message(code);
}

constexpr std::wstring_view qq_link_note =
    L"在水杉输入法的 QQ 群里 @机器人 发 /bind，把收到的 8 位绑定码填在这里。绑定后发 /stats "
    L"能看到你的打字数据（先在「打字统计」页打开「上传到云端」）；机器人在群里的回复所有人都看得到。";
// 已有绑定时卡片标题下的说明（换掉 `qq_link_note`），和绑定成功后的那一句。
constexpr std::wstring_view qq_link_bound =
    L"已绑定。在 QQ 群里 @机器人 发 /stats "
    L"就能看到你的打字数据（先在「打字统计」页打开「上传到云端」）；机器人在群里的回复所有人都看得到。";
constexpr std::wstring_view qq_link_succeeded = L"绑定成功。";
constexpr std::wstring_view qq_link_removed = L"已解绑。";

// 卡片标题：有绑定时是「QQ 群助手」，没有时是「绑定 QQ 群助手」。
constexpr std::wstring_view qq_link_title(bool has_links) {
  return has_links ? L"QQ 群助手" : L"绑定 QQ 群助手";
}

// 共享卡片的 `showForm`：绑定列表读到之后，没有绑定、或点了「绑定其他 QQ」（`adding`）时才出现绑定码表单。
constexpr bool qq_link_form_shown(bool loaded, bool has_links, bool adding) {
  return loaded && (!has_links || adding);
}

} // namespace msime::settings::account
