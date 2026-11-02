#include "../../settings/LexiconImportModel.h"
#include <cstdio>
#include <optional>
#include <string>
#include <string_view>

using namespace msime::settings::lexicon_import;
namespace {
int failures = 0;
void check(bool condition, const char *what) {
  if (!condition) {
    std::fprintf(stderr, "FAIL: %s\n", what);
    ++failures;
  }
}

std::optional<std::string> no_legacy(std::string_view) { return std::nullopt; }
} // namespace

int main() {
  // 格式：细胞词库和汉字自动注音只给拼音词库，顺序与共享页的下拉框相同。
  const auto pinyin = formats_for(L"pinyin");
  check(pinyin.size() == 5 && pinyin[0].id == L"standard" && pinyin[3].id == L"hans" &&
            pinyin[4].id == L"scel",
        "pinyin offers every format, the cell dictionary last");
  check(pinyin[4].label == L"搜狗细胞词库 .scel（仅导入）", "cell dictionary label");
  check(pinyin[4].filter == L"*.scel", "cell dictionary filter");
  const auto wubi = formats_for(L"wubi");
  check(wubi.size() == 3 && wubi[2].id == L"rime", "wubi offers the text formats only");
  check(usable_format(L"wubi", L"scel") == L"standard", "a pinyin-only format falls back");
  check(usable_format(L"pinyin", L"scel") == L"scel", "the cell dictionary stays for pinyin");
  check(usable_format(L"english", L"rime") == L"rime", "rime stays for every kind");

  // 按扩展名换格式。
  check(format_for_file(L"化学词汇.SCEL", L"standard") == L"scel", "scel extension");
  check(format_for_file(L"luna.dict.yaml", L"standard") == L"rime", "yaml extension");
  check(format_for_file(L"a.yml", L"windows") == L"rime", "yml extension");
  check(format_for_file(L"a.txt", L"hans") == L"hans", "other files keep the chosen format");

  // 细胞词库的文件头。
  check(is_scel_bytes(std::string_view("\x40\x15\x00\x00\x44\x43\x53\x01", 8)), "scel header");
  check(!is_scel_bytes(std::string_view("\x40\x15\x00", 3)), "too short");
  check(!is_scel_bytes("你好\tni'hao"), "text");

  // 解码顺序与共享页的 decodeDictionaryBytes 相同。
  check(decode_text("\xEF\xBB\xBF你好\tni'hao", no_legacy) == std::optional<std::string>("你好\tni'hao"),
        "utf-8 bom is stripped");
  check(!decode_text("\xEF\xBB\xBF\xFF", no_legacy), "a bom before invalid utf-8 is refused");
  check(decode_text(std::string_view("\xFF\xFE\x60\x4F\x7D\x59", 6), no_legacy) ==
            std::optional<std::string>("你好"),
        "utf-16le");
  check(decode_text(std::string_view("\xFE\xFF\x4F\x60\x59\x7D", 6), no_legacy) ==
            std::optional<std::string>("你好"),
        "utf-16be");
  check(decode_text(std::string_view("\xFF\xFE\x3D\xD8\x00\xDE", 6), no_legacy) ==
            std::optional<std::string>("\xF0\x9F\x98\x80"),
        "utf-16 surrogate pair");
  check(decode_text(std::string_view("\xFF\xFE\x00\xD8\x41\x00", 6), no_legacy) ==
            std::optional<std::string>("\xEF\xBF\xBD" "A"),
        "a lone surrogate becomes U+FFFD");
  check(decode_text("plain", no_legacy) == std::optional<std::string>("plain"), "plain utf-8");
  bool legacy_called = false;
  const auto legacy = decode_text("\xC4\xE3\xBA\xC3", [&](std::string_view bytes) {
    legacy_called = bytes.size() == 4;
    return std::optional<std::string>("你好");
  });
  check(legacy_called && legacy == std::optional<std::string>("你好"),
        "invalid utf-8 goes to the legacy code page");
  check(!valid_utf8("\xC0\xAF"), "overlong encoding");
  check(!valid_utf8("\xED\xA0\x80"), "encoded surrogate");
  check(!valid_utf8("\xF4\x90\x80\x80"), "beyond U+10FFFF");
  check(!valid_utf8("\xE4\xBD"), "truncated sequence");
  check(valid_utf8("\xF0\x9F\x98\x80"), "four-byte sequence");

  // 文案。
  check(text_too_large() == L"文件不能超过 32 MB，请拆分后分别导入。", "text file bound");
  check(scel_too_large() == L"细胞词库不能超过 8 MB。", "cell dictionary bound");
  check(scel_error_text(L"dictionary_scel_malformed") ==
            L"细胞词库文件不完整或已损坏，请重新下载后再导入。",
        "malformed");
  check(scel_error_text(L"dictionary_busy").empty(), "other codes are not the cell dictionary's");
  check(scel_source(L"化学", 3) == L"已读取搜狗细胞词库「化学」的 3 个词条。", "source sentence");
  check(scel_source(L"", 3) == L"已读取搜狗细胞词库的 3 个词条。", "source sentence without a name");

  ImportReport clean;
  clean.applied = 12;
  check(describe_import(L"快捷短语", clean) == L"快捷短语导入完成，共 12 条。", "clean import");
  ImportReport skipped;
  skipped.applied = 2;
  skipped.failed = 3;
  skipped.first_failures = {{2, L"column_count"}, {9, L"rejected"}};
  check(describe_import(L"全拼", skipped, scel_source(L"化学", 5)) ==
            L"已读取搜狗细胞词库「化学」的 5 个词条。全拼导入完成，共 2 条。跳过 3 行，首先出现在第 2、9 行。"
            L"其中部分行的编码与词不匹配，例如简拼、或音节数与汉字数不一致。",
        "skipped rows with an engine rejection");
  ImportReport partial;
  partial.applied = 4;
  partial.failed = 1;
  partial.truncated = true;
  partial.swapped = true;
  check(describe_import(L"五笔", partial) ==
            L"五笔导入完成，共 4 条。跳过 1 行。文件过长，仅导入了前一部分。"
            L"该文件的两列与所选格式相反，已按文件本身的顺序读取。",
        "truncated and swapped");

  // 词库种类：本机词库的下拉名字照 macOS 改成「中文 · 拼音」，提示里仍说「拼音」；资源词库只读，日语和韩语的第一列是读音。
  check(local_kinds.size() == 6 && local_kinds[0].title == L"中文 · 拼音" &&
            local_kinds[1].title == L"中文 · 86 五笔" && local_kinds[2].title == L"中文 · 98 五笔" &&
            local_kinds[3].id == L"english" && local_kinds[4].id == L"quick_phrase" &&
            local_kinds[5].id == L"cantonese" && local_kinds[5].title == L"中文 · 粤语",
        "local kinds in the shared order and words");
  check(kind_label(L"pinyin") == L"拼音" && kind_label(L"wubi98") == L"98 五笔", "message labels");
  check(kind_label(L"zhuyin") == L"中文 · 注音", "resource kinds are named by their title");
  // 粤语两边都有：提示里说个人词库的「粤语」，资源词库没装时说它自己的名字；下拉框里资源那一项带前缀。
  check(kind_label(L"cantonese") == L"粤语", "the personal Cantonese dictionary");
  check(resource_unavailable(L"cantonese") == L"中文 · 粤语（基础词库）词库尚未安装。", "the Cantonese resource");
  check(resource_option(L"cantonese") == L"resource:cantonese" &&
            resource_from_option(L"resource:cantonese") == L"cantonese" && resource_from_option(L"cantonese").empty(),
        "resource options do not collide with personal kinds");
  check(kind_label(L"unknown") == L"词库", "an unknown kind");
  check(resource_kinds.size() == 5 && resource_kinds[0].id == L"japanese" &&
            resource_kinds[1].id == L"korean_hanja" && resource_kinds[4].id == L"stroke",
        "resource kinds in the shared order");
  check(find_resource_kind(L"japanese")->reading && find_resource_kind(L"korean_hanja")->reading &&
            !find_resource_kind(L"zhuyin")->reading,
        "reading column");
  check(!find_resource_kind(L"pinyin") && !find_local_kind(L"stroke"), "only Cantonese is in both sets");
  check(find_resource_kind(L"japanese")->note == L"基础词库只读，显示假名读音与词条。", "japanese note");
  check(find_resource_kind(L"stroke")->placeholder == L"输入汉字或笔画编码，例如：一 或 h", "stroke placeholder");
  check(resource_unavailable(L"japanese") == L"日语词库尚未安装。", "resource not installed");

  // 一页查询结果的状态。
  check(page_status(0, 0, false) == L"未找到匹配词条", "nothing found");
  check(page_status(100, 0, false) == L"没有更多结果", "past the last page");
  check(page_status(0, 100, true) == L"第 1–100 条，后面还有结果", "a full first page");
  check(page_status(100, 3, false) == L"第 101–103 条", "the last page");

  // 导出当前类型：拼音只留两个字以上的词，空行去掉，开头 BOM、末尾换行。
  const auto pinyin_export = export_payload(L"pinyin", L"standard", "你好\tni'hao\t5\n好\thao\t1\n\n水杉\tshui'shan\t3\n");
  check(pinyin_export.rows == 2 &&
            pinyin_export.body == "\xEF\xBB\xBF你好\tni'hao\t5\n水杉\tshui'shan\t3\n",
        "pinyin keeps words of two characters or more");
  const auto windows_export = export_payload(L"pinyin", L"windows", "ni'hao\t你好\t5\nhao\t好\t1\n");
  check(windows_export.rows == 1 && windows_export.body == "\xEF\xBB\xBFni'hao\t你好\t5\n",
        "windows format reads the word from the second column");
  const auto phrase_export = export_payload(L"quick_phrase", L"rime", "地址\tdz\t1\n");
  check(phrase_export.rows == 1 && phrase_export.body == "\xEF\xBB\xBF地址\tdz\t1\n",
        "other kinds keep single rows");
  const auto single = export_payload(L"pinyin", L"standard", "好\thao\t1\n");
  check(single.rows == 0 && single.body.empty(), "nothing left to export");
  check(export_page_rows("") == 0 && export_page_rows("a\t1\n") == 1 && export_page_rows("a\nb\n\n") == 2,
        "rows of an export page");
  check(personal_export_kinds[3] == L"quick_phrase" && personal_export_kinds[4] == L"english",
        "export all reads quick phrases before english");
  check(personal_export_row(L"wubi98", L"aaaa", L"工", 7) == L"98 五笔\taaaa\t工\t7", "export all row");
  check(personal_export_header == L"# 类别\t编码\t词条\t权重\n", "export all header");
  check(local_kinds[1].export_name == L"水杉IME-五笔用户词库.txt", "export file name");

  // 背单词导入词表。
  check(wordbook_too_large() == L"文件不能超过 1 MB，请拆分后分别导入。", "word list bound");
  check(wordbook_name(L"CET4.csv") == L"CET4", "the extension is dropped");
  check(wordbook_name(L"a.b.txt") == L"a.b", "only the last extension");
  check(wordbook_name(L".csv") == L"导入的词表", "an empty name");
  check(wordbook_name(L"notes.") == L"notes.", "a trailing dot is not an extension");
  check(wordbook_name(std::wstring(70, L'x') + L".txt") == std::wstring(64, L'x'), "64 characters at most");

  if (failures == 0)
    std::printf("lexicon import model passed\n");
  return failures == 0 ? 0 : 1;
}
