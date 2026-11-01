#pragma once
#include <array>
#include <cstddef>
#include <cstdint>
#include <optional>
#include <string>
#include <string_view>
#include <vector>

// 「词库」页的规则和文案，照共享设置页（`dictionary-page.tsx`）移植。导入：`dictionary-format-options.tsx`（格式与名字）、`dictionary-file.ts`（文件上限、`decodeDictionaryBytes` 的解码顺序、细胞词库的文件头）、`dictionary-messages.ts`（`describeImportResult` 的提示）、`dictionary-errors.ts`（细胞词库的出错说明）和 `dictionary-page.tsx`（按扩展名换格式）。查找词库：`dictionary-kinds.ts`（词库种类和名字）、`resource-dictionary-lookup.tsx`（资源词库的提示和说明）和 `dictionary-file.ts` 的 `dictionaryPageStatus`。导出：`dictionary-export.ts`（文件名、按类型导出时拼音只留多字词、导出全部的行格式和 32 MiB 上限）。背单词导入词表：`vocabulary-review.tsx`（1 MiB 上限、按文件名起词表名）。导入导出本身经 `msime_client_settings_dictionary`，细胞词库先经 `msime_client_dictionary_scel_text` 转成标准文本，都在 main.cpp 的工作线程上。不依赖 Windows 和 WinRT 头文件，方便单独测试。
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
// 细胞词库的上限，与 client-services 的 `scel::MAX_SCEL_BYTES` 相同。
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

// ---- 词库种类 ----

// 一种本机词库：宿主认的名字、下拉框里的名字（`localDictionaryKinds`）、提示里说的名字（`dictionaryKindLabel`）和「导出当前类型」的文件名（`dictionaryExportName`）。
struct LocalKind {
  std::wstring_view id;
  std::wstring_view title;
  std::wstring_view label;
  std::wstring_view export_name;
};

// 粤语既是个人词库（自己加的粤语词，经 `list` 读）也是资源词库（发布的粤语词库，经 `browse` 读），两边的 id 都是 `cantonese`，下拉框和「是不是资源词库」的判断要按所选的那一项分开，不能只看 id。
inline constexpr std::array<LocalKind, 6> local_kinds{{
    {L"pinyin", L"中文 · 拼音", L"拼音", L"水杉IME-拼音用户词库.txt"},
    {L"wubi", L"中文 · 86 五笔", L"86 五笔", L"水杉IME-五笔用户词库.txt"},
    {L"wubi98", L"中文 · 98 五笔", L"98 五笔", L"水杉IME-98五笔用户词库.txt"},
    {L"english", L"英文", L"英文", L"水杉IME-英文用户词库.txt"},
    {L"quick_phrase", L"快捷短语", L"快捷短语", L"水杉IME-快捷短语用户词库.txt"},
    {L"cantonese", L"中文 · 粤语", L"粤语", L"水杉IME-粤语用户词库.txt"},
}};

// 一种只读的资源词库（`resourceDictionaryKinds`）：下拉框里的名字、搜索框的提示（`ResourceDictionaryLookup` 的 `placeholders`）和列表上面那句说明。日语和韩语的第一列是读音。
struct ResourceKind {
  std::wstring_view id;
  std::wstring_view title;
  std::wstring_view placeholder;
  std::wstring_view note;
  bool reading;
};

inline constexpr std::array<ResourceKind, 5> resource_kinds{{
    {L"japanese", L"日语", L"例如：日本、にほん 或 ニホン", L"基础词库只读，显示假名读音与词条。", true},
    {L"korean_hanja", L"韩语 · 汉字", L"输入韩文读音或汉字，例如：한 或 韓", L"基础词库只读，按内置汉字表顺序显示。", true},
    {L"cantonese", L"中文 · 粤语（基础词库）", L"输入词条或粤拼，例如：你好 或 nei", L"基础词库只读。", false},
    {L"zhuyin", L"中文 · 注音", L"输入词条或注音，例如：你好 或 ㄋㄧ", L"基础词库只读。", false},
    {L"stroke", L"中文 · 笔画", L"输入汉字或笔画编码，例如：一 或 h", L"基础词库只读。", false},
}};

// 「查找词库」下拉框里资源词库那几项的值：`resource:` 加 id，不和同名的个人词库（粤语）撞上。
inline constexpr std::wstring_view resource_option_prefix = L"resource:";

inline std::wstring resource_option(std::wstring_view id) {
  return std::wstring(resource_option_prefix) + std::wstring(id);
}

// 下拉框的值对应的资源词库 id；不是资源词库那几项时为空。
inline std::wstring_view resource_from_option(std::wstring_view value) {
  // 这个头文件也按 C++17 编译（Win7 设置窗口），没有 starts_with。
  return value.substr(0, resource_option_prefix.size()) == resource_option_prefix
             ? value.substr(resource_option_prefix.size())
             : std::wstring_view{};
}

// 个人词库搜索框的提示。
inline constexpr std::wstring_view personal_placeholder = L"例如：你好 或 nihao";

inline const LocalKind *find_local_kind(std::wstring_view id) {
  for (auto const &kind : local_kinds)
    if (kind.id == id)
      return &kind;
  return nullptr;
}

inline const ResourceKind *find_resource_kind(std::wstring_view id) {
  for (auto const &kind : resource_kinds)
    if (kind.id == id)
      return &kind;
  return nullptr;
}

// 提示里说的词库名字，不认得的种类是「词库」（`dictionaryKindLabel`）；资源词库用下拉框里的名字。
inline std::wstring_view kind_label(std::wstring_view id) {
  if (const auto *kind = find_local_kind(id))
    return kind->label;
  if (const auto *kind = find_resource_kind(id))
    return kind->title;
  return L"词库";
}

// 资源词库没有装时列表处的说法。共享页还让用户去「输入」设置里下载，Windows 没有这个下载入口，所以只说没有安装。用资源词库自己的名字：粤语的 id 也是个人词库的。
inline std::wstring resource_unavailable(std::wstring_view id) {
  const auto *kind = find_resource_kind(id);
  return std::wstring(kind ? kind->title : kind_label(id)) + L"词库尚未安装。";
}

// 一页查询结果的状态，与 `dictionaryPageStatus` 逐字相同。
inline std::wstring page_status(std::uint32_t offset, std::size_t count, bool has_more) {
  if (count == 0)
    return offset == 0 ? L"未找到匹配词条" : L"没有更多结果";
  return L"第 " + std::to_wstring(static_cast<std::uint64_t>(offset) + 1) + L"–" +
         std::to_wstring(static_cast<std::uint64_t>(offset) + count) + L" 条" +
         (has_more ? L"，后面还有结果" : L"");
}

// ---- 导出 ----

// 导出文件的上限，与 `MAX_DICTIONARY_EXPORT_BYTES` 相同；超过时提示分批导出（`dictionary_export_limit`）。
inline constexpr std::uint64_t max_export_bytes = 32ull * 1024 * 1024;

// 「导出全部」读的种类和顺序，以及每行开头的类别（`personalDictionaryExportKinds`），和「导出当前类型」的下拉框顺序不同。
inline constexpr std::array<std::wstring_view, 6> personal_export_kinds{
    {L"pinyin", L"wubi", L"wubi98", L"quick_phrase", L"english", L"cantonese"}};

inline constexpr std::wstring_view personal_export_name = L"水杉用户词库.txt";
inline constexpr std::wstring_view personal_export_header = L"# 类别\t编码\t词条\t权重\n";

// 「导出全部」的一行：类别、编码、词条、权重，用制表符分开（`personalDictionaryExportPayload`）。
inline std::wstring personal_export_row(std::wstring_view kind, std::wstring_view key,
                                        std::wstring_view value, std::int64_t weight) {
  return std::wstring(kind_label(kind)) + L"\t" + std::wstring(key) + L"\t" + std::wstring(value) +
         L"\t" + std::to_wstring(weight);
}

// 「导出当前类型」一页 `export` 回答里的行数，用来算下一页的起点：去掉末尾的空白后按换行数（`page.text.trimEnd().split("\n").length`），空文本是 0。
inline std::size_t export_page_rows(std::string_view text) {
  if (text.empty())
    return 0;
  auto end = text.size();
  while (end > 0 && (text[end - 1] == ' ' || text[end - 1] == '\t' || text[end - 1] == '\n' ||
                     text[end - 1] == '\r' || text[end - 1] == '\v' || text[end - 1] == '\f'))
    --end;
  std::size_t rows = 1;
  for (std::size_t i = 0; i < end; ++i)
    if (text[i] == '\n')
      ++rows;
  return rows;
}

inline bool is_space_byte(char c) {
  return c == ' ' || c == '\t' || c == '\r' || c == '\n' || c == '\v' || c == '\f';
}

inline std::string_view trim_bytes(std::string_view value) {
  while (!value.empty() && is_space_byte(value.front()))
    value.remove_prefix(1);
  while (!value.empty() && is_space_byte(value.back()))
    value.remove_suffix(1);
  return value;
}

// UTF-8 文本里的字数（码点数）。
inline std::size_t code_points(std::string_view value) {
  std::size_t count = 0;
  for (const char c : value)
    if ((static_cast<unsigned char>(c) & 0xC0) != 0x80)
      ++count;
  return count;
}

// 「导出当前类型」写进文件的内容：UTF-8 文本，`dictionaryExportPayload` 的规则。
struct ExportPayload {
  std::string body;
  std::int64_t rows = 0;
  // 超过 `max_export_bytes`。
  bool too_large = false;
};

// 宿主 `export` 读出的 UTF-8 文本整理成文件：去掉空行，拼音词库只留两个字以上的词（单字是拼音表里本来就有的读音，不是用户新增的词），开头加 UTF-8 BOM，末尾一个换行。`format` 是下拉框里选的格式，`windows` 时词在第二列。一行也没有时 `rows` 为 0、`body` 为空。
inline ExportPayload export_payload(std::wstring_view kind, std::wstring_view format,
                                    std::string_view text) {
  ExportPayload payload;
  const std::size_t word_column = format == L"windows" ? 1 : 0;
  std::string kept;
  std::size_t start = 0;
  while (start <= text.size()) {
    auto end = text.find('\n', start);
    if (end == std::string_view::npos)
      end = text.size();
    const auto line = text.substr(start, end - start);
    start = end + 1;
    if (trim_bytes(line).empty())
      continue;
    if (kind == L"pinyin") {
      std::size_t from = 0;
      for (std::size_t column = 0; column < word_column && from != std::string_view::npos; ++column) {
        from = line.find('\t', from);
        if (from != std::string_view::npos)
          ++from;
      }
      if (from == std::string_view::npos)
        continue;
      auto to = line.find('\t', from);
      if (to == std::string_view::npos)
        to = line.size();
      if (code_points(trim_bytes(line.substr(from, to - from))) <= 1)
        continue;
    }
    if (payload.rows > 0)
      kept += '\n';
    kept.append(line.data(), line.size());
    ++payload.rows;
  }
  if (payload.rows == 0)
    return payload;
  payload.body = "\xEF\xBB\xBF" + kept + "\n";
  payload.too_large = payload.body.size() > max_export_bytes;
  return payload;
}

// ---- 背单词导入词表 ----

// 词表文件的上限，与共享页的 `WORDBOOK_FILE_BYTES` 相同。
inline constexpr std::uint64_t max_wordbook_file_bytes = 1048576;

inline std::wstring wordbook_too_large() {
  return L"文件不能超过 " + size_label(max_wordbook_file_bytes) + L"，请拆分后分别导入。";
}

// 导入的词表叫什么：文件名去掉最后一个扩展名，最多 64 个 UTF-16 单元，剩下为空时是「导入的词表」（`file.name.replace(/\.[^.]+$/, "").slice(0, 64)`）。
inline std::wstring wordbook_name(std::wstring_view file_name) {
  std::wstring name(file_name);
  const auto dot = name.rfind(L'.');
  if (dot != std::wstring::npos && dot + 1 < name.size())
    name.erase(dot);
  if (name.size() > 64)
    name.resize(64);
  return name.empty() ? std::wstring(L"导入的词表") : name;
}

} // namespace msime::settings::lexicon_import
