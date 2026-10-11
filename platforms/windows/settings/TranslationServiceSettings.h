#pragma once
#include <cstddef>
#include <cwchar>
#include <string>
#include <string_view>

// 「标点与翻译」页翻译服务的规则：选的是哪一种服务（共享设置 `use-translation-settings.ts` 的 translationProvider，平台固定为 Windows），腾讯云凭据的提示（`translation-validation.ts` 的 tencentCredentialIssue）、「未填写腾讯云凭据」的警告（`translation-provider-settings.ts`）和三个「测试配置」按钮何时可用（`translation-credential-test-config.ts`）。另有偏好库会不会接受一个凭据字段，与 crates/client-core/src/preferences.rs 的 validate 相同，界面据此只把能保存的值写进设置。自定义接口地址要按 WHATWG 网址解析，那部分在 Rust（msime_client_translation_endpoint_issue）。不依赖 Windows 和 WinRT 头文件，方便单独测试。
namespace msime::settings {

// JavaScript 的 String.prototype.trim 去掉的字符：空白和行终止符。
inline bool js_whitespace(wchar_t c) {
  return c == L'\t' || c == L'\n' || c == L'\v' || c == L'\f' || c == L'\r' || c == L' ' ||
         c == 0x00A0 || c == 0x1680 || (c >= 0x2000 && c <= 0x200A) || c == 0x2028 ||
         c == 0x2029 || c == 0x202F || c == 0x205F || c == 0x3000 || c == 0xFEFF;
}

inline std::wstring_view js_trim(std::wstring_view value) {
  std::size_t first = 0;
  std::size_t last = value.size();
  while (first < last && js_whitespace(value[first]))
    ++first;
  while (last > first && js_whitespace(value[last - 1]))
    --last;
  return value.substr(first, last - first);
}

// `credential-utils.ts` 的 tencentSecretConfigured：去掉首尾空白后非空，也不是 <…> 或 FAKESECRET_ 开头的示例值。
inline bool tencent_secret_configured(std::wstring_view value) {
  const auto trimmed = js_trim(value);
  if (trimmed.empty())
    return false;
  if (trimmed.front() == L'<' && trimmed.back() == L'>')
    return false;
  return trimmed.substr(0, 11) != L"FAKESECRET_";
}

// 偏好里与选择服务有关的几项。
struct TranslationServiceChoice {
  bool account = false;
  bool niutrans = false;
  bool custom = false;
  bool tencent = false;
  std::wstring tencent_secret_id;
  std::wstring tencent_secret_key;
};

// 选中的服务：account、niutrans、custom、tencent 或 none。与共享页一样，腾讯云默认开着、却没有可用凭据时不算用户的选择，不会盖过水杉账号；Windows 的 Server 认得水杉账号，所以这一支在 Windows 上生效。
inline std::wstring_view translation_provider(TranslationServiceChoice const &choice) {
  const bool tencent_in_use = choice.tencent &&
                              tencent_secret_configured(choice.tencent_secret_id) &&
                              tencent_secret_configured(choice.tencent_secret_key);
  if (choice.account && !choice.niutrans && !choice.custom && !tencent_in_use)
    return L"account";
  if (choice.niutrans)
    return L"niutrans";
  if (choice.custom)
    return L"custom";
  if (choice.tencent)
    return L"tencent";
  return L"none";
}

inline bool ascii_alphanumeric(wchar_t c) {
  return (c >= L'0' && c <= L'9') || (c >= L'A' && c <= L'Z') || (c >= L'a' && c <= L'z');
}

// tencentCredentialIssue：长度按 UTF-16 码元计，与 JavaScript 的 length 相同；没有问题时为空。
inline std::wstring tencent_credential_issue(std::wstring_view secret_id,
                                             std::wstring_view secret_key,
                                             std::wstring_view region) {
  if (secret_id.size() > 4096 || secret_key.size() > 4096)
    return L"凭据过长。";
  for (const wchar_t c : secret_id)
    if (!ascii_alphanumeric(c) && c != L'_' && c != L'-')
      return L"SecretId 只能包含字母、数字、下划线和连字符。";
  for (const wchar_t c : secret_key)
    if (c <= 0x1F || c == 0x7F)
      return L"SecretKey 不能包含控制字符。";
  if (region.size() > 64)
    return L"地域过长。";
  for (const wchar_t c : region)
    if (!ascii_alphanumeric(c) && c != L'-')
      return L"地域只能包含字母、数字和连字符。";
  return {};
}

// 腾讯云一组里「未填写腾讯云凭据」的警告：没选自定义服务，SecretId 和 SecretKey 都没有可用的值。
inline bool tencent_missing_credentials_warning(bool custom_enabled, std::wstring_view secret_id,
                                                std::wstring_view secret_key) {
  return !custom_enabled && !tencent_secret_configured(secret_id) &&
         !tencent_secret_configured(secret_key);
}

// 三个「测试配置」按钮不可用的条件。available 是「候选词翻译」开关，issue 是腾讯云凭据或接口地址的提示。
inline bool niutrans_credential_test_disabled(bool available, std::wstring_view app_id,
                                              std::wstring_view apikey) {
  return !available || js_trim(app_id).empty() || js_trim(apikey).empty();
}

inline bool issue_credential_test_disabled(bool available, std::wstring_view issue) {
  return !available || !issue.empty();
}

// ---- 偏好库接受什么（preferences.rs 的 validate） ----

// 字符串转成 UTF-8 后的字节数；落单的代理按替换字符的 3 字节算。wchar_t 是 32 位的平台（Windows 规则在 macOS、Linux 上跑测试时）一个 wchar_t 就是一个码点，基本平面以外的直接算 4 字节；用预处理判断，16 位 wchar_t 上 `> 0xFFFF` 恒假会让 MinGW 的 -Werror=type-limits 报错。
inline std::size_t utf8_length(std::wstring_view value) {
  std::size_t bytes = 0;
  for (std::size_t i = 0; i < value.size(); ++i) {
    const wchar_t c = value[i];
    if (c < 0x80)
      bytes += 1;
    else if (c < 0x800)
      bytes += 2;
#if WCHAR_MAX > 0xFFFF
    else if (static_cast<unsigned long>(c) > 0xFFFF)
      bytes += 4;
#endif
    else if (c >= 0xD800 && c <= 0xDBFF && i + 1 < value.size() && value[i + 1] >= 0xDC00 &&
             value[i + 1] <= 0xDFFF) {
      bytes += 4;
      ++i;
    } else
      bytes += 3;
  }
  return bytes;
}

// Rust 的 char::is_control（Cc 类：U+0000–U+001F、U+007F–U+009F）。
inline bool rust_control(wchar_t c) { return c <= 0x1F || (c >= 0x7F && c <= 0x9F); }

// text::is_bounded_text：UTF-8 不超过 maximum 字节，没有控制字符。
inline bool bounded_text(std::wstring_view value, std::size_t maximum) {
  if (utf8_length(value) > maximum)
    return false;
  for (const wchar_t c : value)
    if (rust_control(c))
      return false;
  return true;
}

inline bool tencent_secret_id_saveable(std::wstring_view value) {
  if (utf8_length(value) > 4096)
    return false;
  for (const wchar_t c : value)
    if (!ascii_alphanumeric(c) && c != L'_' && c != L'-')
      return false;
  return true;
}

inline bool tencent_secret_key_saveable(std::wstring_view value) { return bounded_text(value, 4096); }

inline bool tencent_region_saveable(std::wstring_view value) {
  if (value.size() > 64)
    return false;
  for (const wchar_t c : value)
    if (!ascii_alphanumeric(c) && c != L'-')
      return false;
  return true;
}

// NiuTrans 的 App ID 和 API Key：可以为空；不为空时要是 translation::usable_credential 认的值（去掉首尾空格、制表和换行后非空，不是 <…>，不以 FAKESECRET_ 开头）。
inline bool niutrans_credential_saveable(std::wstring_view value) {
  if (!bounded_text(value, 4096))
    return false;
  if (value.empty())
    return true;
  std::size_t first = 0;
  std::size_t last = value.size();
  auto blank = [](wchar_t c) { return c == L' ' || c == L'\t' || c == L'\r' || c == L'\n'; };
  while (first < last && blank(value[first]))
    ++first;
  while (last > first && blank(value[last - 1]))
    --last;
  const auto trimmed = value.substr(first, last - first);
  if (trimmed.empty())
    return false;
  if (trimmed.front() == L'<' && trimmed.back() == L'>')
    return false;
  return trimmed.substr(0, 11) != L"FAKESECRET_";
}

inline bool custom_translation_api_key_saveable(std::wstring_view value) {
  return bounded_text(value, 4096);
}

} // namespace msime::settings
