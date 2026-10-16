#include "../src/candidates/CandidateScriptAnnotation.h"

#include <cassert>
#include <string>

int main() {
  using msime::linux_host::candidate_script_annotation;
  using msime::linux_host::candidate_script_annotation_suffix;
  using msime::linux_host::script_annotation_text_allowed;
  using Json = nlohmann::json;

  // 插件注释只读 `script_annotation`，引擎的 `annotation` 不会被当成插件注释，反过来也一样。
  assert(candidate_script_annotation(Json{{"text", "你"}, {"annotation", "ab"}}).empty());
  assert(candidate_script_annotation(
             Json{{"text", "你"}, {"annotation", "ab"}, {"script_annotation", "常用"}}) == "常用");
  assert(candidate_script_annotation(Json{{"text", "你"}, {"script_annotation", ""}}).empty());
  assert(candidate_script_annotation(Json{{"text", "你"}, {"script_annotation", 7}}).empty());
  assert(candidate_script_annotation(Json{{"text", "你"}, {"script_annotation", nullptr}}).empty());
  assert(candidate_script_annotation(Json::array()).empty());
  // AI 占位行（来源 12，共享内核已经不再产出）不画；云和 AI 的真实行照画。
  assert(candidate_script_annotation(Json{{"text", "…"}, {"source", 12}, {"script_annotation", "注"}}).empty());
  assert(candidate_script_annotation(Json{{"text", "云"}, {"source", 2}, {"script_annotation", "注"}}) == "注");

  // 长度按 UTF-16 码元算，上限 32：32 个 ASCII 或 16 个辅助平面字符可以，再多一个码元就整条不画。
  assert(script_annotation_text_allowed(std::string(32, 'a')));
  assert(!script_annotation_text_allowed(std::string(33, 'a')));
  std::string astral;
  for (int index = 0; index < 16; ++index) astral += "\xF0\x9F\x98\x80";
  assert(script_annotation_text_allowed(astral));
  assert(!script_annotation_text_allowed(astral + "a"));
  std::string han;
  for (int index = 0; index < 32; ++index) han += "注";
  assert(script_annotation_text_allowed(han));
  assert(!script_annotation_text_allowed(han + "注"));

  // 折行、改方向和零宽的字符一律不画；普通的中文、拉丁、阿拉伯字母照画。
  for (const char *broken : {"a\nb", "a\rb", "a\tb", "\x7f", "\xC2\x85",
                             "\xE2\x80\xA8", "\xE2\x80\xA9", "\xE2\x80\xAE", "\xE2\x80\x8B",
                             "\xE2\x81\xA6", "\xE2\x81\xA9", "\xD8\x9C", "\xEF\xBB\xBF"})
    assert(!script_annotation_text_allowed(broken));
  assert(script_annotation_text_allowed("拼音 pinyin"));
  assert(script_annotation_text_allowed("\xD8\xB9\xD8\xB1\xD8\xA8\xD9\x8A"));

  // 坏的 UTF-8：截断、过长编码、代理码位、超出 U+10FFFF。
  for (const char *malformed : {"\xE6\xB3", "\xC0\xAF", "\xED\xA0\x80", "\xF4\x90\x80\x80", "\xFF"})
    assert(!script_annotation_text_allowed(malformed));
  assert(candidate_script_annotation(Json{{"text", "你"}, {"script_annotation", std::string(40, 'x')}}).empty());

  // 行尾那一段：两个空格加六角括号，没有注释时什么都不加。
  assert(candidate_script_annotation_suffix("常用") == "  〔常用〕");
  assert(candidate_script_annotation_suffix("").empty());
}
