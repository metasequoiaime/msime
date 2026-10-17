#include "../../settings/AccountPageModel.h"
#include <cstdio>
#include <string>

using namespace msime::settings::account;
namespace {
int failures = 0;
void check(bool condition, const char *what) {
  if (!condition) {
    std::fprintf(stderr, "FAIL: %s\n", what);
    ++failures;
  }
}
} // namespace

int main() {
  // normalizeAccountName 与 JavaScript 的 trim 一样去掉全角空格、不换行空格和 BOM，中间的空白留着。
  check(normalize_name(L"  水杉 小鹿\t") == L"水杉 小鹿", "ascii trim");
  check(normalize_name(L"　 名字﻿ ") == L"名字", "unicode trim");
  check(normalize_name(L" \t ").empty(), "blank trims to empty");

  // isValidAccountName：非空、最多 64 个码点、不含控制字符。
  check(!valid_name(L""), "empty name is invalid");
  check(!valid_name(L"   "), "blank name is invalid");
  check(valid_name(L" 小鹿 "), "trimmed name is valid");
  check(valid_name(std::wstring(64, L'名')), "64 characters");
  check(!valid_name(std::wstring(65, L'名')), "65 characters");
  std::wstring emoji;
  for (int i = 0; i < 64; ++i)
    emoji += L"\U0001F98C";
  check(code_points(emoji) == 64 && valid_name(emoji), "a surrogate pair counts once");
  check(!valid_name(emoji + L"\U0001F98C"), "65 surrogate pairs");
  check(!valid_name(L"a\nb") && !valid_name(L"a\x7F") && !valid_name(std::wstring(L"a\0b", 3)),
        "control characters");

  // preferredAccountName 和账号 ID 的短写。
  check(preferred_name(L"  小鹿 ", L"abcdef12-3456") == L"小鹿", "own name");
  check(preferred_name(L" ", L"abcdef12-3456") == L"水杉小鹿·ABCDEF", "generated name");
  check(id_label(L"0a1b2c3d-x") == L"#0A1B2C", "id label");
  check(id_label(L"ab") == L"#AB", "short id");
  check(initial(L"水杉") == L"水" && initial(L"\U0001F98C鹿") == L"\U0001F98C" && initial(L"").empty(),
        "avatar initial");

  // accountProviderName。
  check(provider_name(L"sms") == L"手机号" && provider_name(L"phone") == L"手机号" &&
            provider_name(L"email") == L"邮箱" && provider_name(L"google") == L"Google" &&
            provider_name(L"apple") == L"Apple" && provider_name(L"wechat") == L"wechat",
        "provider names");

  // accountMessage：审核的三种、账号错误码，其余都是服务暂不可用。
  check(error_message(L"account_blocked_content") == L"内容包含不允许发布的词语，请修改后再提交",
        "blocked content");
  check(error_message(L"account_banned") == L"该账号已被封禁，暂时无法使用账号相关功能", "banned");
  check(error_message(L"account_invalid") == L"填写的内容无效，请检查后重试。", "invalid");
  check(error_message(L"account_avatar_invalid") == L"请选择 1 MiB 以内的 PNG 或 JPEG 图片。",
        "avatar invalid");
  check(error_message(L"account_unauthorized") == L"登录已失效，请重新登录。", "unauthorized");
  check(error_message(L"account_conflict") == L"云端设置已被其他设备更新，请刷新后重新确认。",
        "conflict");
  check(error_message(L"conflict") == L"账号服务暂不可用，请稍后再试。" &&
            error_message(L"") == L"账号服务暂不可用，请稍后再试。",
        "unknown codes");
  check(cancelled(L"account_cancelled") && !cancelled(L"account_invalid"), "cancellation");

  check(confirmation_text(Confirmation::logout_all).label == L"确认退出所有设备" &&
            confirmation_text(Confirmation::remove).message ==
                L"注销账号将删除已发布皮肤、评分及其他云端账号数据，无法撤销。",
        "confirmation copy");

  // 验证码：只留数字、最多 6 位；登录要正好 6 位数字。
  check(code_digits(L"12a3-45 678") == L"123456", "code digits");
  check(code_digits(L"１２") == L"", "full-width digits are not \\d");
  check(six_digits(L"012345") && !six_digits(L"12345") && !six_digits(L"12345a"), "six digits");

  // 重发倒计时按秒向上取整。
  check(resend_seconds(60'000) == 60 && resend_seconds(59'001) == 60 && resend_seconds(1) == 1 &&
            resend_seconds(0) == 0 && resend_seconds(-5) == 0,
        "resend seconds");
  check(resend_label(0) == L"获取验证码" && resend_label(42) == L"42 秒后可重新发送", "resend label");
  check(login_label(true, true) == L"验证码已过期，请重新获取" &&
            login_label(false, true) == L"正在登录…" && login_label(false, false) == L"登录",
        "login label");

  // createdAt 换成 Unix 毫秒，与 JavaScript 的 Date 相同。
  check(parse_iso_time(L"1970-01-01T00:00:00Z") == 0, "epoch");
  check(parse_iso_time(L"2026-10-11T05:40:05.123Z") == 1791697205123LL, "milliseconds");
  check(parse_iso_time(L"2026-10-11T13:40:05+08:00") == 1791697205000LL, "offset");
  check(parse_iso_time(L"2026-10-11T05:40:05.123456789Z") == 1791697205123LL, "nanoseconds");
  check(parse_iso_time(L"2026-10-11") == 1791676800000LL, "date only is UTC midnight");
  check(parse_iso_time(L"2024-02-29T00:00:00Z").has_value() &&
            !parse_iso_time(L"2025-02-29T00:00:00Z").has_value(),
        "leap day");
  check(!parse_iso_time(L"").has_value() && !parse_iso_time(L"yesterday").has_value() &&
            !parse_iso_time(L"2026-13-01T00:00:00Z").has_value() &&
            !parse_iso_time(L"2026-10-11T05:40:05").has_value() &&
            !parse_iso_time(L"2026-10-11T05:40:05Zjunk").has_value(),
        "unreadable times");
  check(zh_date(2026, 1, 5) == L"2026/1/5", "zh-CN date");

  check(sync_description.find(L"双拼和五笔版本") != std::wstring_view::npos &&
            sync_applied == L"已应用云端设置。",
        "settings sync copy");

  // 「绑定 QQ 群助手」：去掉空白和连字符后正好 8 位数字才能绑定；失败的说法与 qqLinkMessage 相同。
  check(qq_link_digits(L" 1234-5678　") == L"12345678" && eight_digits(L"12345678"), "bind code digits");
  check(!eight_digits(L"1234567") && !eight_digits(L"123456789") && !eight_digits(L"1234567a"),
        "bad bind codes");
  check(qq_link_message(L"account_qq_link_code_not_found").find(L"/bind") != std::wstring::npos &&
            qq_link_message(L"account_qq_link_limit").find(L"10 个") != std::wstring::npos &&
            qq_link_message(L"account_unauthorized") == error_message(L"account_unauthorized"),
        "bind messages");

  if (failures == 0)
    std::puts("account_page_model: ok");
  return failures == 0 ? 0 : 1;
}
