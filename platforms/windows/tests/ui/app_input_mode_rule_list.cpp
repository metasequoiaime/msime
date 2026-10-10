#include "../../settings/AppInputModeRuleList.h"
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
} // namespace

int main() {
  check(normalize_app_input_mode_rule(L"  Code.EXE ") == L"code.exe", "trims and lower-cases ASCII");
  check(normalize_app_input_mode_rule(L"C:\\Program Files\\Microsoft VS Code\\Code.exe") == L"code.exe",
        "a pasted path keeps only the file name");
  check(normalize_app_input_mode_rule(L"\u3000微信.exe\u00A0") == L"微信.exe", "full-width and no-break spaces are trimmed");
  check(normalize_app_input_mode_rule(L"Ä.EXE") == L"Ä.exe", "non-ASCII letters keep their case");
  // 偏好库用 Rust 的 str::trim 判首尾空白，这里去掉的必须是同一个 White_Space 集合，否则名字过了本地校验仍会在保存时被拒。
  check(normalize_app_input_mode_rule(L"\u2002code.exe\u2009") == L"code.exe", "en space and thin space are trimmed");
  check(normalize_app_input_mode_rule(L"\u202Fcode.exe\u205F\u1680") == L"code.exe",
        "narrow no-break, math and ogham spaces are trimmed");
  check(normalize_app_input_mode_rule(L"\u2028code.exe\u2029\x0085") == L"code.exe", "line and paragraph separators are trimmed");
  check(normalize_app_input_mode_rule(L"C:\\Tools\\\u2003Code.exe") == L"code.exe", "the file name after a path is trimmed too");
  // 资源管理器「复制文件地址」给出的路径带一对双引号。
  check(normalize_app_input_mode_rule(L" \"C:\\Program Files\\Microsoft VS Code\\Code.exe\" ") == L"code.exe",
        "quotes from Copy as path are removed");
  check(normalize_app_input_mode_rule(L"\"") == L"\"", "a lone quote is kept and later rejected");

  const std::vector<std::wstring> none;
  check(validate_app_input_mode_rule(L"", none) == AppInputModeRuleError::Empty, "empty");
  check(validate_app_input_mode_rule(L"code", none) == AppInputModeRuleError::NotExe, "needs .exe");
  check(validate_app_input_mode_rule(L"bad\u0007.exe", none) == AppInputModeRuleError::InvalidCharacter,
        "control characters");
  check(!validate_app_input_mode_rule(std::wstring(60, L'a') + L".exe", none), "64 bytes is allowed");
  check(validate_app_input_mode_rule(std::wstring(61, L'a') + L".exe", none) == AppInputModeRuleError::TooLong,
        "65 bytes is too long");
  // 汉字在 UTF-8 里占 3 个字节：20 个加 .exe 是 64 字节，21 个是 67 字节。
  check(!validate_app_input_mode_rule(std::wstring(20, L'微') + L".exe", none), "bytes, not characters");
  check(validate_app_input_mode_rule(std::wstring(21, L'微') + L".exe", none) == AppInputModeRuleError::TooLong,
        "multi-byte characters count by their UTF-8 size");
  check(validate_app_input_mode_rule(L"code.exe", {L"Code.EXE"}) == AppInputModeRuleError::Duplicate,
        "duplicates ignore ASCII case");
  // macOS 的 bundle id 和这里的程序名在同一张表里，不会相撞。
  check(!validate_app_input_mode_rule(L"code.exe", {L"com.microsoft.VSCode"}), "macOS identifiers do not collide");
  std::vector<std::wstring> full;
  for (int i = 0; i < 32; ++i)
    full.push_back(L"app" + std::to_wstring(i) + L".exe");
  check(validate_app_input_mode_rule(L"another.exe", full) == AppInputModeRuleError::TooMany, "at most 32 rules");
  if (failures == 0)
    std::puts("App input mode rule list: names are checked before they are written");
  return failures == 0 ? 0 : 1;
}
