#include "../../settings/AccountPageModel.h"
#include <cstdio>
#include <nlohmann/json.hpp>
#include <string>

// CMake 把 shared/contracts/account-errors/cases.json 编进 account_errors_cases.inc（逐字节的数组），Wine 里跑测试时不用再找仓库路径。
static const unsigned char kAccountErrorCases[] = {
#include "account_errors_cases.inc"
};

using namespace msime::settings::account;
namespace {
int failures = 0;
void check(bool condition, const char *what) {
  if (!condition) {
    std::fprintf(stderr, "FAIL: %s\n", what);
    ++failures;
  }
}

// 用例是 UTF-8；宽字符串在 Windows 上是 UTF-16，在其他平台上是 UTF-32。用例里没有不合法的 UTF-8。
std::wstring widen(const std::string &text) {
  std::wstring result;
  for (size_t index = 0; index < text.size();) {
    const auto lead = static_cast<unsigned char>(text[index]);
    const size_t length = lead < 0x80 ? 1 : lead < 0xE0 ? 2 : lead < 0xF0 ? 3 : 4;
    char32_t point = length == 1 ? lead : lead & (0x3F >> (length - 1));
    for (size_t next = 1; next < length; ++next)
      point = (point << 6) | (static_cast<unsigned char>(text[index + next]) & 0x3F);
    index += length;
    if constexpr (sizeof(wchar_t) == 2) {
      if (point > 0xFFFF) {
        point -= 0x10000;
        result += static_cast<wchar_t>(0xD800 + (point >> 10));
        result += static_cast<wchar_t>(0xDC00 + (point & 0x3FF));
        continue;
      }
    }
    result += static_cast<wchar_t>(point);
  }
  return result;
}
} // namespace

int main() {
  // 登录目标沿用现有的去空白方式；昵称规则的测试随实现迁到共享 Rust。
  check(trim_login_target(L"  demo@example.test\t") == L"demo@example.test", "ascii trim");
  check(trim_login_target(L"\u3000\u00a0demo@example.test\ufeff\u2028") == L"demo@example.test",
        "unicode trim");
  check(trim_login_target(L" \t ").empty(), "blank trims to empty");

  // 账号 ID 的短写。
  check(id_label(L"0a1b2c3d-x") == L"#0A1B2C", "id label");
  check(id_label(L"ab") == L"#AB", "short id");
  check(initial(L"水杉") == L"水" && initial(L"\U0001F98C鹿") == L"\U0001F98C" && initial(L"").empty(),
        "avatar initial");

  // 宿主读取共享名称，不重建已知登录方式的映射。
  const ProviderNames provider_names{{L"email", L"合成邮件方式"}, {L"sms", L"合成短信方式"},
                                    {L"synthetic-provider", L"合成新方式"}};
  check(provider_name(L"email", provider_names) == L"合成邮件方式" &&
            provider_name(L"sms", provider_names) == L"合成短信方式" &&
            provider_name(L"synthetic-provider", provider_names) == L"合成新方式" &&
            provider_name(L"wechat", provider_names) == L"wechat", "shared provider names");

  // accountMessage：逐条跑各平台共用的 shared/contracts/account-errors/cases.json，每个码说成 `shared` 那一句（与共享账号页相同，不读 Retry-After）。
  const auto contract =
      nlohmann::json::parse(std::string(reinterpret_cast<const char *>(kAccountErrorCases), sizeof(kAccountErrorCases)));
  size_t error_cases = 0;
  for (const auto &item : contract.at("cases")) {
    const auto code = item.at("code").get<std::string>();
    const bool same = error_message(widen(code)) == widen(item.at("shared").get<std::string>());
    if (!same)
      std::fprintf(stderr, "FAIL: account error %s\n", code.c_str());
    check(same, "account error case");
    ++error_cases;
  }
  check(error_cases > 10, "account error cases were read");
  // 用例之外：不是账号码的写法和空串也是服务暂不可用。
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
  // 有绑定时标题是「QQ 群助手」、说明换成已绑定那句，表单要点「绑定其他 QQ」才出现；绑定列表还没读到时不画表单。
  check(qq_link_title(true) == L"QQ 群助手" && qq_link_title(false) == L"绑定 QQ 群助手", "bind card title");
  check(qq_link_bound.substr(0, 4) == L"已绑定。" && qq_link_bound.find(L"上传到云端") != std::wstring_view::npos &&
            qq_link_succeeded == L"绑定成功。",
        "bind card copy");
  check(!qq_link_form_shown(false, false, false) && qq_link_form_shown(true, false, false) &&
            !qq_link_form_shown(true, true, false) && qq_link_form_shown(true, true, true),
        "bind form visibility");

  // 未登录的登录卡片：标题、说明和没有登录方式时的三种说法与共享页桌面那一支相同。
  check(sign_in_welcome == L"欢迎来到水杉" &&
            sign_in_description == L"登录后在设备之间同步设置和词库，还可以发布你的候选窗口皮肤" &&
            sign_in_code_description == L"我们会发送一个 6 位验证码完成登录",
        "sign-in copy");
  check(providers_unavailable(true, true) == L"当前没有可用的验证码登录方式，请稍后重试。" &&
            providers_unavailable(false, true) == L"正在加载登录方式…" &&
            providers_unavailable(false, false) == L"登录方式加载失败，请重试。",
        "providers unavailable copy");

  if (failures == 0)
    std::puts("account_page_model: ok");
  return failures == 0 ? 0 : 1;
}
