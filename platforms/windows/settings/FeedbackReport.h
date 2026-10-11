#pragma once
#include <array>
#include <cstddef>
#include <cstdint>
#include <string>
#include <string_view>

// 「帮助与反馈」页「提交可复现的问题」一组的报告：反馈类型、附上的诊断信息、报告正文和预填的 GitHub 新 issue 地址。逐条照搬共享设置的 `feedback-kind-options.tsx`、`support-diagnostics.ts`、`use-feedback-report.ts` 和 `label-helpers.ts` 的 `schemeTitle`；不依赖 Windows 和 WinRT 头文件，方便单独测试。
namespace msime::settings {

// 反馈类型的选项，顺序与共享页相同；第一项是默认值。选项的值就是显示的文字，报告和 issue 标题都用它。
inline constexpr std::array<std::wstring_view, 4> feedback_kinds{
    {L"功能异常", L"候选词不对", L"功能建议", L"其他"}};

// 描述框的字数上限（共享页 textarea 的 `maxLength`），也是 GitHub 地址里报告正文的截断长度。两者都按 UTF-16 码元计，与浏览器的 `maxLength` 和 `String.prototype.slice` 一致。
inline constexpr std::size_t feedback_text_limit = 4000;

// 共享页的 issue 地址（`platform-resource-urls.ts` 的 `issuesUrl`，各平台都是 `linuxIssuesUrl`）、QQ 群号和 Telegram 群组。
inline constexpr std::wstring_view feedback_issues_url = L"https://github.com/metasequoiaime/msime/issues";
inline constexpr std::wstring_view feedback_qq_group = L"829919142";
inline constexpr std::wstring_view feedback_telegram_url = L"https://t.me/msimegroup";

// 输入方案 id 的中文名（`schemeTitle`）。未知的 id 原样返回。
inline std::wstring_view feedback_scheme_title(std::wstring_view scheme) {
  if (scheme == L"quanpin")
    return L"全拼";
  if (scheme == L"shuangpin")
    return L"双拼";
  if (scheme == L"wubi")
    return L"五笔";
  if (scheme == L"japanese")
    return L"日语";
  if (scheme == L"korean")
    return L"韩语";
  if (scheme == L"cantonese")
    return L"粤拼";
  if (scheme == L"zhuyin")
    return L"注音";
  if (scheme == L"vietnamese")
    return L"越南语";
  if (scheme == L"tibetan")
    return L"藏文";
  if (scheme == L"stroke")
    return L"笔画";
  return scheme;
}

// 附在报告里的诊断信息（`supportDiagnostics`），宿主为 windows：「水杉 IME <版本>」；知道系统版本时是「Windows <版本>」，否则「平台：windows」；有输入方案时「输入方案：<名称>」。本窗口没有浏览器的 User-Agent，那一行不出现。空行去掉，以换行连接。
inline std::wstring feedback_diagnostics(std::wstring_view version, std::wstring_view os_version,
                                         std::wstring_view scheme_title) {
  std::wstring text = L"水杉 IME ";
  text.append(version);
  text.append(L"\n");
  if (!os_version.empty()) {
    text.append(L"Windows ");
    text.append(os_version);
  } else {
    text.append(L"平台：windows");
  }
  if (!scheme_title.empty()) {
    text.append(L"\n输入方案：");
    text.append(scheme_title);
  }
  return text;
}

// TextBox 的换行是单个 CR（粘贴进来的还可能是 CRLF）；浏览器 textarea 的值只用 LF，报告也照此写。
inline std::wstring feedback_normalize_newlines(std::wstring_view value) {
  std::wstring result;
  result.reserve(value.size());
  for (std::size_t i = 0; i < value.size(); ++i) {
    if (value[i] == L'\r') {
      result.push_back(L'\n');
      if (i + 1 < value.size() && value[i + 1] == L'\n')
        ++i;
    } else {
      result.push_back(value[i]);
    }
  }
  return result;
}

// 报告正文，格式与 `useFeedbackReport` 的 `report` 逐字相同。
inline std::wstring feedback_report(std::wstring_view kind, std::wstring_view detail,
                                    std::wstring_view diagnostics) {
  std::wstring report = L"### 类型\n";
  report.append(kind);
  report.append(L"\n\n### 描述\n");
  report.append(detail);
  report.append(L"\n\n### 环境\n");
  report.append(diagnostics);
  report.append(L"\n");
  return report;
}

// `URLSearchParams.toString()` 对一个值的编码（application/x-www-form-urlencoded）：先转 UTF-8，落单的代理项换成 U+FFFD；字母、数字和 `*-._` 不变，空格写成 `+`，其余字节写成大写的 %XX。
inline std::wstring feedback_form_encode(std::wstring_view value) {
  constexpr wchar_t hex[] = L"0123456789ABCDEF";
  std::wstring out;
  auto put_byte = [&](std::uint32_t byte) {
    const bool letter = (byte >= 'a' && byte <= 'z') || (byte >= 'A' && byte <= 'Z');
    const bool digit = byte >= '0' && byte <= '9';
    if (letter || digit || byte == '*' || byte == '-' || byte == '.' || byte == '_') {
      out.push_back(static_cast<wchar_t>(byte));
    } else if (byte == ' ') {
      out.push_back(L'+');
    } else {
      out.push_back(L'%');
      out.push_back(hex[(byte >> 4) & 0xF]);
      out.push_back(hex[byte & 0xF]);
    }
  };
  // Windows 的 wchar_t 是 UTF-16 码元；在 wchar_t 为 32 位的编译器上（主机单元测试可能在那里编译）一个字符就是一个码点，同样处理。
  for (std::size_t i = 0; i < value.size(); ++i) {
    std::uint32_t code = static_cast<std::uint32_t>(value[i]);
    const std::uint32_t next = i + 1 < value.size() ? static_cast<std::uint32_t>(value[i + 1]) : 0;
    if (code >= 0xD800 && code <= 0xDBFF && next >= 0xDC00 && next <= 0xDFFF) {
      code = 0x10000 + ((code - 0xD800) << 10) + (next - 0xDC00);
      ++i;
    } else if ((code >= 0xD800 && code <= 0xDFFF) || code > 0x10FFFF) {
      code = 0xFFFD;
    }
    if (code < 0x80) {
      put_byte(code);
    } else if (code < 0x800) {
      put_byte(0xC0 | (code >> 6));
      put_byte(0x80 | (code & 0x3F));
    } else if (code < 0x10000) {
      put_byte(0xE0 | (code >> 12));
      put_byte(0x80 | ((code >> 6) & 0x3F));
      put_byte(0x80 | (code & 0x3F));
    } else {
      put_byte(0xF0 | (code >> 18));
      put_byte(0x80 | ((code >> 12) & 0x3F));
      put_byte(0x80 | ((code >> 6) & 0x3F));
      put_byte(0x80 | (code & 0x3F));
    }
  }
  return out;
}

// 「在 GitHub 提交」打开的地址：`<issuesUrl>/new?title=<类型>&body=<报告>`，报告只取前 4000 个 UTF-16 码元，与 `useFeedbackReport` 的 `submit` 相同。
inline std::wstring feedback_issue_url(std::wstring_view issues_url, std::wstring_view kind,
                                       std::wstring_view report) {
  std::wstring url(issues_url);
  url.append(L"/new?title=");
  url.append(feedback_form_encode(kind));
  url.append(L"&body=");
  url.append(feedback_form_encode(report.substr(0, feedback_text_limit)));
  return url;
}

} // namespace msime::settings
