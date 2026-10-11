#include "../../settings/CommunityPageModel.h"
#include <cstdio>
#include <string>
#include <string_view>
#include <vector>

using namespace msime::settings::community;
namespace {
int failures = 0;
void check(bool condition, const char *what) {
  if (!condition) {
    std::fprintf(stderr, "FAIL: %s\n", what);
    ++failures;
  }
}
// The Windows targets build as C++17, which has no std::wstring::starts_with or ends_with.
bool starts_with(std::wstring_view value, std::wstring_view prefix) {
  return value.substr(0, prefix.size()) == prefix;
}
bool ends_with(std::wstring_view value, std::wstring_view suffix) {
  return value.size() >= suffix.size() && value.substr(value.size() - suffix.size()) == suffix;
}
} // namespace

int main() {
  // boundedGraphemes：按字截断，组合符号、表情序列和旗帜不被拆开。
  check(bounded_graphemes(L"水杉输入法", 3) == L"水杉输", "cjk truncation");
  check(bounded_graphemes(L"abc", 5) == L"abc", "shorter text is kept");
  check(bounded_graphemes(L"éx", 1) == L"é", "combining mark stays with its base");
  check(bounded_graphemes(L"\U0001F468‍\U0001F469‍\U0001F467!", 1) ==
            L"\U0001F468‍\U0001F469‍\U0001F467",
        "zwj sequence is one grapheme");
  check(bounded_graphemes(L"\U0001F44D\U0001F3FD\U0001F44D", 1) == L"\U0001F44D\U0001F3FD",
        "skin tone modifier");
  check(bounded_graphemes(L"\U0001F1E8\U0001F1F3\U0001F1EF\U0001F1F5", 1) ==
            L"\U0001F1E8\U0001F1F3",
        "regional indicator pair is one flag");
  check(bounded_graphemes(L"\U0001F1E8\U0001F1F3\U0001F1EF\U0001F1F5!", 2) == L"\U0001F1E8\U0001F1F3\U0001F1EF\U0001F1F5",
        "two flags");
  check(bounded_graphemes(L"a\r\nb", 2) == L"a\r\n", "crlf is one grapheme");
  check(bounded_graphemes(L"❤️❤", 1) == L"❤️", "variation selector");

  // communityPublishFields：名称去掉首尾空白后 1–32 个字，说明最多 280 个字。
  {
    const auto fields = publish_fields(L"  我的皮肤 ", L" 说明\n");
    check(fields.name == L"我的皮肤" && fields.description == L"说明", "fields are trimmed");
    check(fields.name_valid && fields.description_valid, "short fields are valid");
  }
  check(!publish_fields(L"   ", L"").name_valid, "blank name");
  check(publish_fields(std::wstring(32, L'名'), L"").name_valid, "32 characters");
  check(!publish_fields(std::wstring(33, L'名'), L"").name_valid, "33 characters");
  check(publish_fields(L"x", std::wstring(280, L'字')).description_valid, "280 characters");
  check(!publish_fields(L"x", std::wstring(281, L'字')).description_valid, "281 characters");
  check(publish_fields(L"x", L"").description_valid, "empty description is fine");

  // 「其他」素材授权：非空，UTF-8 不超过 120 字节（40 个汉字）。
  check(asset_license_valid(L" 仅限个人使用 "), "custom license");
  check(!asset_license_valid(L"  "), "blank license");
  check(asset_license_valid(std::wstring(40, L'授')), "40 chinese characters");
  check(!asset_license_valid(std::wstring(41, L'授')), "41 chinese characters");
  check(utf8_length(L"aé水\U0001F98C") == 1 + 2 + 3 + 4, "utf8 length");

  // 数字、评分和大小。
  check(zh_number(0) == L"0" && zh_number(999) == L"999" && zh_number(1234) == L"1,234" &&
            zh_number(1234567) == L"1,234,567",
        "zh-CN grouping");
  check(rating(0, 0) == L"暂无评分" && rating(3, 4.25) == L"4.3 分" && rating(2, 4.75) == L"4.8 分" &&
            rating(1, 4.04) == L"4.0 分" && rating(1, 5) == L"5.0 分",
        "rating");
  check(rating_metrics(1200, L"下载", 3, 4.5) == L"1,200 人下载 · 4.5 分 · 3 人评分",
        "rating metrics");
  check(megabytes(1024 * 1024 * 1.5) == L"1.5 MB" && megabytes(0) == L"0.0 MB", "megabytes");

  // 错误码的句子：审核类的三种各宿主相同；发布时 community_invalid 另有一句。
  check(candidate_skin_message(L"community_blocked_content") ==
            L"内容包含不允许发布的词语，请修改后再提交",
        "moderation sentence");
  check(candidate_skin_message(L"community_invalid") == L"内容无效，请修改后重试。" &&
            starts_with(candidate_skin_message(L"community_invalid", true), L"服务器未接受这款皮肤"),
        "publish invalid");
  check(candidate_skin_message(L"candidate_skin_exists") == L"已存在同名皮肤，安装会整体替换它。",
        "exists");
  check(candidate_skin_message(L"whatever") == L"社区暂时不可用，请稍后重试。", "skin fallback");
  check(plugin_message(L"community_conflict", L"", true) ==
            L"发布信息已变更，或已达到发布上限（最多 20 个、合计 32 MB）。" &&
            plugin_message(L"community_conflict", L"") == L"作品状态已变化，请刷新后重试。",
        "plugin conflict");
  check(plugin_message(L"plugin_invalid", L"缺少 license") == L"插件不符合要求（缺少 license）。",
        "plugin pack failure reads as on the 插件 page");
  check(plugin_message(L"whatever", L"") == L"社区暂时不可用，请稍后重试。", "plugin fallback");
  check(needs_sign_in(L"community_unauthorized") && !needs_sign_in(L"community_forbidden"),
        "sign in");

  // 授权行、作者行、分类和本机选项。
  check(license_line(L" CC-BY-4.0 ", L"", L"https://x") ==
            L"素材授权 CC-BY-4.0 / 来源 https://x",
        "license line");
  check(license_line(L"", L"", L"").empty(), "empty license line");
  check(card_author(L"自然", L"小鹿", false, false, false) == L"自然 · 小鹿", "author");
  check(card_author(L"", L"小鹿", true, true, true) == L"我的作品 · 私有 · 已下架", "own author");
  check(card_author(L"", L"小鹿", false, false, true) == L"小鹿", "removed only for own items");
  check(category_label(L"tech") == L"科技夜色" && category_label(L"").empty() &&
            category_label(L"unknown").empty(),
        "category label");
  check(local_option_label(L"a", L"a") == L"a" && local_option_label(L"名字", L"id") == L"名字（id）",
        "local option label");
  // 多行输入框的 CR 和 CR LF 都按 textarea 写成 LF。
  check(line_feeds(L"a\rb\r\nc\n") == L"a\nb\nc\n", "line feeds");
  check(line_feeds(L"\r\r\n") == L"\n\n", "consecutive breaks");
  check(join_dot({L"自然", L"", L"小鹿", L"v1"}) == L"自然 · 小鹿 · v1", "join");

  // 社区插件空列表的句子。
  check(ends_with(plugin_empty_notice(true, false, false, true), L"查看更早发布的插件。"),
        "skipped pages");
  check(plugin_empty_notice(false, false, true, true) == L"你还没有发布过插件。", "mine empty");
  check(plugin_empty_notice(false, true, false, true) == L"没有匹配的插件。", "filtered empty");
  check(starts_with(plugin_empty_notice(false, false, false, true), L"社区里还没有插件，"),
        "publishable empty");

  // 社区词库与回复模板（community-resources.tsx）。
  check(resource_message(L"community_forbidden") == L"没有权限执行此操作。", "resource forbidden");
  check(resource_message(L"community_storage") == L"无法安全保存本地模板，请稍后重试。",
        "resource storage");
  check(resource_message(L"community_blocked_content") == L"内容包含不允许发布的词语，请修改后再提交",
        "resource moderation");
  check(resource_message(L"dictionary_busy") == L"社区暂时不可用，请稍后重试。", "resource fallback");
  check(resource_heading(L"dictionary", L"") == L"好词，随手可得", "dictionary heading");
  check(resource_heading(L"reply", L"mine") == L"我的回复模板作品", "mine heading");
  check(resource_heading(L"dictionary", L"saved") == L"收藏的词库", "saved heading");
  check(resource_card_description(L"reply", L"") == L"回复模板", "reply fallback");
  check(resource_card_metrics(1234, 0, 0) == L"☆ 暂无评分 · 1,234 人收藏", "card metrics");
  check(resource_destinations[1].kind == L"reply" && resource_destinations[1].scope == L"mine",
        "published replies destination");

  // Number(weight) 与 Number.isSafeInteger。
  check(js_safe_integer(L"100000") == 100000, "plain weight");
  check(js_safe_integer(L"  42 ") == 42, "trimmed weight");
  check(js_safe_integer(L"") == 0, "empty is zero");
  check(js_safe_integer(L"1e3") == 1000, "exponent");
  check(js_safe_integer(L"2.0") == 2, "integral decimal");
  check(!js_safe_integer(L"2.5"), "fraction");
  check(js_safe_integer(L"0x10") == 16, "hex");
  check(!js_safe_integer(L"12abc"), "garbage");
  check(!js_safe_integer(L"9007199254740992"), "unsafe integer");
  check(!js_safe_integer(L"Infinity"), "infinity");
  check(js_safe_integer(L"-3") == -3, "negative parses");

  // addEntry 的规则。
  {
    std::vector<SharedWord> entries;
    auto first = shared_word(entries, L"pinyin", L" shui'shan ", L"水杉", L"100000");
    check(first && first->code == L"shui'shan" && first->weight == 100000, "entry accepted");
    entries.push_back(*first);
    check(!shared_word(entries, L"pinyin", L"shui'shan", L"水杉", L"1"), "duplicate entry");
    check(shared_word(entries, L"wubi", L"shui'shan", L"水杉", L"1").has_value(),
          "same word of another kind");
    check(!shared_word(entries, L"pinyin", L"  ", L"水杉", L"1"), "blank code");
    check(!shared_word(entries, L"pinyin", L"a", L"", L"1"), "empty word");
    check(!shared_word(entries, L"pinyin", L"a", L"啊", L"-1"), "negative weight");
    std::vector<SharedWord> full(shared_word_limit, SharedWord{L"pinyin", L"a", L"啊", 1});
    check(!shared_word(full, L"pinyin", L"b", L"吧", L"1"), "at most 128 entries");
  }

  // submit 的规则。
  check(resource_publish_error(L"reply", L"模板", L"", L"  ", 0, false, true) ==
            L"请填写有效内容并确认拥有公开发布所需的权利。",
        "blank prompt");
  check(resource_publish_error(L"reply", L"模板", L"", L"写得体贴些", 0, false, true).empty(),
        "reply publishable");
  check(!resource_publish_error(L"dictionary", L"词库", L"", L"", 1, false, false).empty(),
        "first publish needs agreement");
  check(resource_publish_error(L"dictionary", L"词库", L"", L"", 1, true, false).empty(),
        "new version needs no agreement");
  check(resource_publish_error(L"dictionary", L"词库", L"", L"", 0, true, false) ==
            L"请填写有效的作品信息。",
        "new version without entries");
  check(!resource_publish_error(L"reply", L"模板", L"", std::wstring(2001, L'a'), 0, true, true)
             .empty(),
        "prompt too long");

  // importCommunityDictionary 的分组。
  {
    const auto groups = local_import_groups({{L"quick", L"dz", L"地址", 1},
                                             {L"pinyin", L"shui'shan", L"水杉", 100},
                                             {L"quick", L"yx", L"邮箱", 2}});
    check(groups.size() == 2 && groups[0].kind == L"quick_phrase" && groups[0].source == L"quick" &&
              groups[1].kind == L"pinyin",
          "groups in first-seen order");
    check(groups[0].text == L"地址\tdz\t1\n邮箱\tyx\t2", "standard lines");
  }

  if (failures)
    return 1;
  std::puts("community page model: ok");
  return 0;
}
