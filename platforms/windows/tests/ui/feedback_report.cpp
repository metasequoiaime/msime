#include "../../settings/FeedbackReport.h"
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
  check(feedback_kinds[0] == L"功能异常" && feedback_kinds[3] == L"其他", "kinds in the shared order");
  check(feedback_scheme_title(L"shuangpin") == L"双拼", "scheme title");
  check(feedback_scheme_title(L"stroke") == L"笔画", "stroke title");
  check(feedback_scheme_title(L"unknown") == L"unknown", "unknown scheme stays as is");

  check(feedback_diagnostics(L"1.2.3", L"10.0.26200", L"全拼") ==
            L"水杉 IME 1.2.3\nWindows 10.0.26200\n输入方案：全拼",
        "diagnostics with an OS version");
  check(feedback_diagnostics(L"1.2.3", L"", L"") == L"水杉 IME 1.2.3\n平台：windows",
        "diagnostics without an OS version or scheme");

  check(feedback_normalize_newlines(L"a\rb\r\nc\nd") == L"a\nb\nc\nd", "CR and CRLF become LF");

  const auto report = feedback_report(L"功能异常", L"a b*c~", L"水杉 IME 1.0");
  check(report == L"### 类型\n功能异常\n\n### 描述\na b*c~\n\n### 环境\n水杉 IME 1.0\n", "report text");

  // 期望值取自浏览器的 `new URLSearchParams({title, body}).toString()`。
  check(feedback_issue_url(feedback_issues_url, L"候选词不对", report) ==
            L"https://github.com/metasequoiaime/msime/issues/new?title=%E5%80%99%E9%80%89%E8%AF%8D%E4%B8%8D%E5%AF%B9"
            L"&body=%23%23%23+%E7%B1%BB%E5%9E%8B%0A%E5%8A%9F%E8%83%BD%E5%BC%82%E5%B8%B8%0A%0A%23%23%23+%E6%8F%8F%E8%BF%B0"
            L"%0Aa+b*c%7E%0A%0A%23%23%23+%E7%8E%AF%E5%A2%83%0A%E6%B0%B4%E6%9D%89+IME+1.0%0A",
        "issue url matches URLSearchParams");
  check(feedback_form_encode(L"\U0001F600x !'()~") == L"%F0%9F%98%80x+%21%27%28%29%7E",
        "astral characters and reserved characters");

  // 代理项只在 wchar_t 为 UTF-16 的 Windows 上出现。
  if constexpr (sizeof(wchar_t) == 2) {
    const std::wstring lone(1, static_cast<wchar_t>(0xD800));
    check(feedback_form_encode(lone + L"x") == L"%EF%BF%BDx", "a lone surrogate becomes U+FFFD");
    // 报告正文按 UTF-16 码元截到 4000；截在代理对中间时和 slice 一样留下落单的高代理项，编码成 U+FFFD。
    const std::wstring long_body = std::wstring(3999, L'a') + L"\U0001F600" + L"tail";
    check(feedback_issue_url(L"u", L"k", long_body) ==
              L"u/new?title=k&body=" + std::wstring(3999, L'a') + L"%EF%BF%BD",
          "body is cut at 4000 UTF-16 code units");
  }
  check(feedback_issue_url(L"u", L"k", std::wstring(4001, L'b')) ==
            L"u/new?title=k&body=" + std::wstring(4000, L'b'),
        "body is cut at 4000 characters");
  check(feedback_issue_url(L"u", L"k", L"short") == L"u/new?title=k&body=short", "short body is kept");

  if (failures == 0)
    std::puts("feedback_report: ok");
  return failures == 0 ? 0 : 1;
}
