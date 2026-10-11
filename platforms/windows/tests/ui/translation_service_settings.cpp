#include "../../settings/TranslationServiceSettings.h"
#include <cstdio>
#include <string>

using namespace msime::settings;
namespace {
int failures = 0;
void check(bool condition, const char *what) {
  if (!condition) {
    std::fprintf(stderr, "FAIL: %s\n", what);
    ++failures;
  }
}
TranslationServiceChoice choice(bool account, bool niutrans, bool custom, bool tencent,
                                std::wstring id = L"", std::wstring key = L"") {
  TranslationServiceChoice value;
  value.account = account;
  value.niutrans = niutrans;
  value.custom = custom;
  value.tencent = tencent;
  value.tencent_secret_id = std::move(id);
  value.tencent_secret_key = std::move(key);
  return value;
}
} // namespace

int main() {
  // 新装：腾讯云默认开着，没有凭据。
  check(translation_provider(choice(false, false, false, true)) == L"tencent",
        "the default is Tencent");
  check(translation_provider(choice(true, false, false, true)) == L"account",
        "Tencent without secrets does not hide the account");
  check(translation_provider(choice(true, false, false, true, L"AKID1", L"key")) == L"tencent",
        "Tencent with secrets wins over the account");
  check(translation_provider(choice(true, false, false, true, L"<id>", L"key")) == L"account",
        "a placeholder secret is not configured");
  check(translation_provider(choice(true, true, false, false)) == L"niutrans",
        "NiuTrans hides the account");
  check(translation_provider(choice(false, true, true, true)) == L"niutrans",
        "NiuTrans comes before custom and Tencent");
  check(translation_provider(choice(false, false, true, true)) == L"custom",
        "custom comes before Tencent");
  check(translation_provider(choice(false, false, false, false)) == L"none", "nothing is 关闭");

  check(!tencent_secret_configured(L"  　"), "whitespace is not configured");
  check(!tencent_secret_configured(L" FAKESECRET_x "), "FAKESECRET_ is not configured");
  check(tencent_secret_configured(L"<"), "a lone < is a value");

  check(tencent_credential_issue(L"", L"", L"ap-guangzhou").empty(), "defaults are fine");
  check(tencent_credential_issue(std::wstring(4097, L'a'), L"", L"") == L"凭据过长。",
        "too long");
  check(tencent_credential_issue(L"AK ID", L"", L"") ==
            L"SecretId 只能包含字母、数字、下划线和连字符。",
        "SecretId characters");
  check(tencent_credential_issue(L"AKID", L"a\tb", L"") == L"SecretKey 不能包含控制字符。",
        "SecretKey control characters");
  check(tencent_credential_issue(L"AKID", L"key", std::wstring(65, L'a')) == L"地域过长。",
        "region too long");
  check(tencent_credential_issue(L"AKID", L"key", L"ap_guangzhou") ==
            L"地域只能包含字母、数字和连字符。",
        "region characters");

  check(tencent_missing_credentials_warning(false, L"", L" "), "missing secrets warn");
  check(!tencent_missing_credentials_warning(true, L"", L""), "custom on does not warn");
  check(!tencent_missing_credentials_warning(false, L"AKID", L""), "one secret is enough");

  check(niutrans_credential_test_disabled(false, L"id", L"key"), "off disables the test");
  check(niutrans_credential_test_disabled(true, L" ", L"key"), "blank App ID disables it");
  check(!niutrans_credential_test_disabled(true, L"id", L"key"), "both filled enables it");
  check(issue_credential_test_disabled(true, L"接口地址过长。"), "an issue disables it");
  check(!issue_credential_test_disabled(true, L""), "no issue enables it");

  check(utf8_length(L"aé中\U0001F600") == 1 + 2 + 3 + 4, "UTF-8 length");
  check(tencent_secret_id_saveable(L"") && tencent_secret_id_saveable(L"AK_ID-1"),
        "SecretId the store accepts");
  check(!tencent_secret_id_saveable(L"AK.ID"), "SecretId the store refuses");
  check(!tencent_secret_key_saveable(L"key\u0085"), "the store refuses C1 controls too");
  check(tencent_region_saveable(L"") && !tencent_region_saveable(L"ap guangzhou"),
        "region the store accepts");
  check(niutrans_credential_saveable(L"") && niutrans_credential_saveable(L"app"),
        "NiuTrans values the store accepts");
  check(!niutrans_credential_saveable(L" ") && !niutrans_credential_saveable(L"<key>") &&
            !niutrans_credential_saveable(L"FAKESECRET_1"),
        "NiuTrans values the store refuses");
  check(custom_translation_api_key_saveable(L"") &&
            !custom_translation_api_key_saveable(std::wstring(4097, L'k')),
        "custom API Key bound");

  if (failures == 0)
    std::puts("translation service settings: ok");
  return failures == 0 ? 0 : 1;
}
