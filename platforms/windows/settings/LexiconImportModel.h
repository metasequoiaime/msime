#pragma once
#include <array>
#include <cstddef>
#include <cstdint>
#include <optional>
#include <string>
#include <string_view>
#include <vector>

// 「词库」页「导入词库文件」的规则和文案，照共享设置页「导入与导出」的导入移植：`dictionary-format-options.tsx`（格式与名字）、`dictionary-file.ts`（文件上限、`decodeDictionaryBytes` 的解码顺序、细胞词库的文件头）、`dictionary-messages.ts`（`describeImportResult` 的提示）、`dictionary-errors.ts`（细胞词库的出错说明）和 `dictionary-page.tsx`（按扩展名换格式）。导入本身经 `msime_client_settings_dictionary`，细胞词库先经 `msime_client_dictionary_scel_text` 转成标准文本，都在 main.cpp 的工作线程上。不依赖 Windows 和 WinRT 头文件，方便单独测试。
namespace msime::settings::lexicon_import {

// 一种导入格式：宿主认的名字、下拉框里的字、文件对话框的过滤器，以及是否只有拼音词库能用。
struct Format {
  std::wstring_view id;
  std::wstring_view label;
  std::wstring_view filter_name;
  std::wstring_view filter;
  bool pinyin_only;
};

inline constexpr std::array<Format, 5> formats{{
    {L"standard", L"词在前（标准 TSV）", L"词库文本", L"*.txt;*.tsv", false},
    {L"windows", L"编码在前（Windows TSV）", L"词库文本", L"*.txt;*.tsv", false},
    {L"rime", L"Rime userdb / dict.yaml", L"Rime 词典", L"*.yaml;*.yml;*.txt", false},
    {L"hans", L"汉字自动注音（仅导入）", L"词库文本", L"*.txt;*.tsv", true},
    {L"scel", L"搜狗细胞词库 .scel（仅导入）", L"搜狗细胞词库", L"*.scel", true},
}};

// 文本文件的上限，与共享页的 `MAX_DICTIONARY_FILE_BYTES` 相同；`msime_client_settings_dictionary` 再把它分批送给 Server。
inline constexpr std::uint64_t max_text_file_bytes = 32ull * 1024 * 1024;
// 细胞词库的上限，与 client-core 的 `scel::MAX_SCEL_BYTES` 相同。
inline constexpr std::uint64_t max_scel_file_bytes = 8ull * 1024 * 1024;

// 这一类词库能选的格式，按下拉框里的顺序。
inline std::vector<Format> formats_for(std::wstring_view kind) {
  std::vector<Format> allowed;
  for (auto const &format : formats)
    if (!format.pinyin_only || kind == L"pinyin")
      allowed.push_back(format);
  return allowed;
}

inline const Format *find_format(std::wstring_view id) {
  for (auto const &format : formats)
    if (format.id == id)
      return &format;
  return nullptr;
}

// 换了词库类型后，原来选的格式还能不能用；不能用时退回标准格式。
inline std::wstring_view usable_format(std::wstring_view kind, std::wstring_view chosen) {
  const auto *format = find_format(chosen);
  return format && (!format->pinyin_only || kind == L"pinyin") ? format->id : formats[0].id;
}

inline wchar_t ascii_lower(wchar_t c) {
  return c >= L'A' && c <= L'Z' ? static_cast<wchar_t>(c - L'A' + L'a') : c;
}

inline bool ends_with_ignoring_case(std::wstring_view value, std::wstring_view suffix) {
  if (value.size() < suffix.size())
    return false;
  const auto tail = value.substr(value.size() - suffix.size());
  for (size_t i = 0; i < suffix.size(); ++i)
    if (ascii_lower(tail[i]) != suffix[i])
      return false;
  return true;
}

// 按扩展名换格式（`dictionary-page.tsx` 的 `fileFormat`）：`.yaml`、`.yml` 是 Rime，`.scel` 是细胞词库，其余用下拉框里选的。
inline std::wstring_view format_for_file(std::wstring_view file_name, std::wstring_view chosen) {
  if (ends_with_ignoring_case(file_name, L".yaml") || ends_with_ignoring_case(file_name, L".yml"))
    return L"rime";
  if (ends_with_ignoring_case(file_name, L".scel"))
    return L"scel";
  return chosen;
}

// 文件开头是不是细胞词库：只看前 4 字节，版本不对的也交给宿主去说「版本不支持」。文本不会以 NUL 开头，所以改了扩展名的细胞词库也认得出来。
inline bool is_scel_bytes(std::string_view bytes) {
  return bytes.size() >= 4 && bytes.substr(0, 4) == std::string_view("\x40\x15\x00\x00", 4);
}

// ---- 解码 ----

inline void append_utf8(std::string &out, char32_t c) {
  if (c < 0x80) {
    out += static_cast<char>(c);
  } else if (c < 0x800) {
    out += static_cast<char>(0xC0 | (c >> 6));
    out += static_cast<char>(0x80 | (c & 0x3F));
  } else if (c < 0x10000) {
    out += static_cast<char>(0xE0 | (c >> 12));
    out += static_cast<char>(0x80 | ((c >> 6) & 0x3F));
    out += static_cast<char>(0x80 | (c & 0x3F));
  } else {
    out += static_cast<char>(0xF0 | (c >> 18));
    out += static_cast<char>(0x80 | ((c >> 12) & 0x3F));
    out += static_cast<char>(0x80 | ((c >> 6) & 0x3F));
    out += static_cast<char>(0x80 | (c & 0x3F));
  }
}

// 严格的 UTF-8：拒绝过长编码、代理区码点、超出 U+10FFFF 和截断的序列，与 `TextDecoder("utf-8", {fatal: true})` 一致。
inline bool valid_utf8(std::string_view bytes) {
  size_t i = 0;
  while (i < bytes.size()) {
    const auto lead = static_cast<unsigned char>(bytes[i]);
    size_t length = 0;
    char32_t minimum = 0;
    char32_t c = 0;
    if (lead < 0x80) {
      ++i;
      continue;
    } else if (lead >= 0xC2 && lead <= 0xDF) {
      length = 2;
      minimum = 0x80;
      c = lead & 0x1F;
    } else if (lead >= 0xE0 && lead <= 0xEF) {
      length = 3;
      minimum = 0x800;
      c = lead & 0x0F;
    } else if (lead >= 0xF0 && lead <= 0xF4) {
      length = 4;
      minimum = 0x10000;
      c = lead & 0x07;
    } else {
      return false;
    }
    if (i + length > bytes.size())
      return false;
    for (size_t k = 1; k < length; ++k) {
      const auto next = static_cast<unsigned char>(bytes[i + k]);
      if ((next & 0xC0) != 0x80)
        return false;
      c = (c << 6) | (next & 0x3F);
    }
    if (c < minimum || c > 0x10FFFF || (c >= 0xD800 && c <= 0xDFFF))
      return false;
    i += length;
  }
  return true;
}

// UTF-16 转 UTF-8；孤立代理换成 U+FFFD，与 `TextDecoder("utf-16le")` 的非严格解码一致。
inline std::string utf16_to_utf8(std::string_view bytes, bool big_endian) {
  std::string out;
  out.reserve(bytes.size() * 3 / 2);
  auto unit = [&](size_t at) {
    const auto a = static_cast<unsigned char>(bytes[at]);
    const auto b = static_cast<unsigned char>(bytes[at + 1]);
    return static_cast<char32_t>(big_endian ? (a << 8) | b : (b << 8) | a);
  };
  size_t i = 0;
  while (i + 1 < bytes.size()) {
    const char32_t c = unit(i);
    i += 2;
    if (c >= 0xD800 && c <= 0xDBFF && i + 1 < bytes.size()) {
      const char32_t low = unit(i);
      if (low >= 0xDC00 && low <= 0xDFFF) {
        append_utf8(out, 0x10000 + ((c - 0xD800) << 10) + (low - 0xDC00));
        i += 2;
        continue;
      }
    }
    append_utf8(out, c >= 0xD800 && c <= 0xDFFF ? 0xFFFD : c);
  }
  // 奇数长度的最后一个字节凑不成一个单元。
  if (i < bytes.size())
    append_utf8(out, 0xFFFD);
  return out;
}

// 文件字节解成 UTF-8 文本，顺序与共享页的 `decodeDictionaryBytes` 相同：UTF-8 BOM（之后必须是严格 UTF-8）、UTF-16LE 和 UTF-16BE 的 BOM、严格 UTF-8，最后交给 `legacy`（Windows 上是 GB18030，代码页 54936）。解不出时为空。
template <typename Legacy>
std::optional<std::string> decode_text(std::string_view bytes, Legacy legacy) {
  if (bytes.size() >= 3 && bytes.substr(0, 3) == "\xEF\xBB\xBF") {
    const auto body = bytes.substr(3);
    if (!valid_utf8(body))
      return std::nullopt;
    return std::string(body);
  }
  if (bytes.size() >= 2 && bytes.substr(0, 2) == "\xFF\xFE")
    return utf16_to_utf8(bytes.substr(2), false);
  if (bytes.size() >= 2 && bytes.substr(0, 2) == "\xFE\xFF")
    return utf16_to_utf8(bytes.substr(2), true);
  if (valid_utf8(bytes))
    return std::string(bytes);
  return legacy(bytes);
}

// ---- 文案 ----

// 文件大小的说法，与共享页的 `dictionaryFileSizeLabel` 相同。
inline std::wstring size_label(std::uint64_t bytes) {
  return bytes >= 1048576 && bytes % 1048576 == 0 ? std::to_wstring(bytes / 1048576) + L" MB"
                                                  : std::to_wstring(bytes / 1024) + L" KB";
}

inline std::wstring text_too_large() {
  return L"文件不能超过 " + size_label(max_text_file_bytes) + L"，请拆分后分别导入。";
}

inline std::wstring scel_too_large() { return L"细胞词库不能超过 " + size_label(max_scel_file_bytes) + L"。"; }

inline constexpr std::wstring_view scel_needs_pinyin =
    L"搜狗细胞词库只能导入拼音词库，请先把「词库」切换到拼音。";

// 细胞词库的出错说明（`dictionary-errors.ts`）；不是细胞词库的码时为空。
inline std::wstring scel_error_text(std::wstring_view code) {
  if (code == L"dictionary_scel_unsupported")
    return L"这个文件不是能识别的搜狗细胞词库（.scel）。";
  if (code == L"dictionary_scel_malformed")
    return L"细胞词库文件不完整或已损坏，请重新下载后再导入。";
  if (code == L"dictionary_scel_empty")
    return L"细胞词库里没有词条。";
  if (code == L"dictionary_scel_invalid")
    return L"细胞词库读取失败，请重新选择文件。";
  return {};
}

// 细胞词库导入提示的来源句（`scelImportSource`）。
inline std::wstring scel_source(std::wstring_view name, std::int64_t entries) {
  return name.empty() ? L"已读取搜狗细胞词库的 " + std::to_wstring(entries) + L" 个词条。"
                      : L"已读取搜狗细胞词库「" + std::wstring(name) + L"」的 " +
                            std::to_wstring(entries) + L" 个词条。";
}

// 宿主的导入报告：`applied`、`failed`、`truncated`、`swapped` 和最早几处失败的行号与原因。
struct ImportReport {
  std::int64_t applied = 0;
  std::int64_t failed = 0;
  bool truncated = false;
  bool swapped = false;
  std::vector<std::pair<std::int64_t, std::wstring>> first_failures;
};

// 导入完成的提示，与共享页的 `describeImportResult` 逐字相同；`source` 是放在最前面的来源句。
inline std::wstring describe_import(std::wstring_view kind_label, ImportReport const &report,
                                    std::wstring_view source = {}) {
  std::wstring message = std::wstring(source) + std::wstring(kind_label) + L"导入完成，共 " +
                         std::to_wstring(report.applied) + L" 条。";
  if (report.failed > 0) {
    std::wstring lines;
    bool rejected = false;
    for (auto const &[line, issue] : report.first_failures) {
      if (!lines.empty())
        lines += L"、";
      lines += std::to_wstring(line);
      rejected = rejected || issue == L"rejected";
    }
    message += lines.empty() ? L"跳过 " + std::to_wstring(report.failed) + L" 行。"
                             : L"跳过 " + std::to_wstring(report.failed) + L" 行，首先出现在第 " +
                                   lines + L" 行。";
    if (rejected)
      message += L"其中部分行的编码与词不匹配，例如简拼、或音节数与汉字数不一致。";
  }
  if (report.truncated)
    message += L"文件过长，仅导入了前一部分。";
  if (report.swapped)
    message += L"该文件的两列与所选格式相反，已按文件本身的顺序读取。";
  return message;
}

} // namespace msime::settings::lexicon_import
