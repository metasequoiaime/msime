#include "../src/core/InputSchemes.h"

#include <cassert>
#include <cstdlib>
#include <filesystem>
#include <fstream>
#include <string>
#include <unistd.h>

int main() {
  using namespace msime::linux_host;
  using Json = nlohmann::json;
  namespace fs = std::filesystem;

  // The ids keep the Engine's `SchemeType` order, so an id and a view's `scheme` number name the same scheme.
  assert(scheme_number("quanpin") == scheme::Quanpin);
  assert(scheme_number("korean") == scheme::Korean);
  assert(scheme_number("cantonese") == scheme::Cantonese);
  assert(scheme_number("zhuyin") == scheme::Zhuyin);
  assert(scheme_number("vietnamese") == scheme::Vietnamese);
  assert(scheme_number("tibetan") == scheme::Tibetan);
  assert(scheme::Tibetan == 8);
  assert(scheme_number("stroke") == scheme::Stroke);
  assert(scheme::Stroke == 9);
  assert(scheme_number("pinyin") == -1);
  assert(scheme_number("") == -1);

  // 越南文和藏文与日文、韩文一样是非中文输入语言；粤拼、注音和笔画是中文方案。
  for (int number : {0, 1, 2, 5, 6, 9})
    assert(scheme::IsChinese(number));
  for (int number : {3, 4, 7, 8, 10, -1})
    assert(!scheme::IsChinese(number));
  // K、"/"、"@" 只查自己的表，五笔也打开（`opens_table_modes`）；其余 Shift+字母模式只在全拼和双拼里（`opens_local_modes`）。
  for (int number : {0, 1, 2})
    assert(scheme::OpensTableModes(number));
  for (int number : {3, 4, 5, 6, 7, 8, 9, -1})
    assert(!scheme::OpensTableModes(number));
  assert(!scheme::OpensLocalModes(scheme::Wubi));
  // 藏文的宿主特性和越南文相同：字母直接组成文字，CapsLock 的大写字母照样组字，离开时上屏，光标锁在末尾，不转繁体，不用中文标点，不变全角，没有宿主的智能标点，也没有要打开的候选列表。
  static_assert(scheme::LetterComposition(scheme::Tibetan) && scheme::CapsLockBypassExempt(scheme::Tibetan) &&
                scheme::AlwaysInlinePreedit(scheme::Tibetan) && scheme::CommitsOnBlur(scheme::Tibetan) &&
                scheme::LocksCaret(scheme::Tibetan));
  static_assert(!scheme::FoldsLetterCase(scheme::Tibetan) && !scheme::OpensCandidateList(scheme::Tibetan) &&
                !scheme::ScriptConversionApplies(scheme::Tibetan) && !scheme::LearnsIntoMainDictionary(scheme::Tibetan) &&
                !scheme::OpensLocalModes(scheme::Tibetan) && !scheme::UsesChinesePunctuation(scheme::Tibetan) &&
                !scheme::HostSmartPunctuation(scheme::Tibetan) && !scheme::WidensFullWidth(scheme::Tibetan));
  // 笔画照抄粤拼的特性：候选按 msime-stroke.db 里存的原样取用，不学习，组字离开时不上屏，也不因预编辑样式而强制内嵌显示。
  assert(!scheme::ScriptConversionApplies(scheme::Stroke));
  assert(!scheme::LearnsIntoMainDictionary(scheme::Stroke));
  assert(!scheme::OpensLocalModes(scheme::Stroke));
  assert(!scheme::CommitsOnBlur(scheme::Stroke));
  assert(!scheme::LocksCaret(scheme::Stroke));
  assert(scheme::UsesChinesePunctuation(scheme::Stroke));
  assert(scheme::HostSmartPunctuation(scheme::Stroke));
  assert(scheme::WidensFullWidth(scheme::Stroke));
  assert(!scheme::OpensCandidateList(scheme::Stroke));
  assert(!scheme::AlwaysInlinePreedit(scheme::Stroke));
  // 笔画组字（editing_text 里是 ASCII 字母，reading 里是笔画字形）不是列表组字。
  const Json stroke = {{"scheme", 9}, {"editing_text", "hs"}, {"reading", "一丨"}, {"candidates", Json::array({Json{{"text", "十"}}})}};
  assert(!candidate_list_composition(stroke));
  assert(!opened_candidate_list(stroke));
  assert(!zhuyin_list_down_key(stroke));
  assert(scheme_rules(stroke) == scheme::Stroke);

  // Only Korean and Zhuyin keep their candidates in a list the user opens.
  const Json zhuyin = {{"scheme", 6}, {"editing_text", "ㄋㄧˇ"}, {"candidates", Json::array()}};
  assert(candidate_list_composition(zhuyin));
  assert(!opened_candidate_list(zhuyin));
  assert(zhuyin_list_down_key(zhuyin));
  Json zhuyin_open = zhuyin;
  zhuyin_open["candidates"] = Json::array({Json{{"text", "你"}}});
  assert(opened_candidate_list(zhuyin_open));
  assert(!zhuyin_list_down_key(zhuyin_open));
  const Json zhuyin_idle = {{"scheme", 6}, {"editing_text", ""}};
  assert(!candidate_list_composition(zhuyin_idle));
  assert(!zhuyin_list_down_key(zhuyin_idle));
  const Json korean = {{"scheme", 4}, {"editing_text", "한"}, {"candidates", Json::array()}};
  assert(candidate_list_composition(korean));
  assert(!zhuyin_list_down_key(korean));
  const Json quanpin = {{"scheme", 0}, {"editing_text", "ni"}, {"candidates", Json::array({Json{{"text", "你"}}})}};
  assert(!candidate_list_composition(quanpin));
  assert(!opened_candidate_list(quanpin));
  const Json vietnamese = {{"scheme", 7}, {"editing_text", "viet"}};
  assert(!candidate_list_composition(vietnamese));
  const Json tibetan = {{"scheme", 8}, {"editing_text", "བཀྲ"}};
  assert(!candidate_list_composition(tibetan));
  // A local mode keeps its own rules in every scheme.
  Json zhuyin_mode = zhuyin;
  zhuyin_mode["local_mode"] = "expression";
  assert(scheme_rules(zhuyin_mode) == -1);
  assert(!candidate_list_composition(zhuyin_mode));
  assert(!candidate_list_composition(nullptr));

  // 整句改字：只有全拼和双拼的普通组字把左右键交给改字命令，本地模式、专用英文和其他方案照旧移字母光标或分段。
  assert(scheme::EditsSentence(scheme::Quanpin) && scheme::EditsSentence(scheme::Shuangpin));
  for (int number : {2, 3, 4, 5, 6, 7, 8, 9, 10, -1})
    assert(!scheme::EditsSentence(number));
  assert(edits_sentence(quanpin));
  assert(edits_sentence(Json{{"scheme", 1}, {"local_mode", "none"}}));
  assert(!edits_sentence(stroke));
  assert(!edits_sentence(Json{{"scheme", 0}, {"local_mode", "expression"}}));
  assert(!edits_sentence(Json{{"scheme", 0}, {"dedicated_english", true}}));
  assert(!edits_sentence(Json::object()));

  // 改字时的行内文字：已选的词加整句，光标在焦点字前；位置按标量给出，换成字节和标量两种单位（𠮷 是四个字节的一个标量）。
  assert(!conversion_preedit(quanpin).active);
  assert(!conversion_preedit(Json{{"conversion", ""}}).active);
  const auto converted = conversion_preedit(Json{{"phrase_prefix", "𠮷"}, {"conversion", "我去背景"},
                                                 {"conversion_focus_start", 2}, {"conversion_focus_end", 4}});
  assert(converted.active && converted.text == "𠮷我去背景");
  assert(converted.caret_bytes == 4 + 6 && converted.caret_scalars == 3);
  assert(converted.focus_end_bytes == 4 + 12 && converted.focus_end_scalars == 5);
  // 光标在句末：焦点段为空；越界或缺少的位置取末尾，结尾不早于光标。
  const auto at_end = conversion_preedit(Json{{"conversion", "我去北京"}, {"conversion_focus_start", 4}, {"conversion_focus_end", 4}});
  assert(at_end.caret_bytes == 12 && at_end.focus_end_bytes == 12 && at_end.caret_scalars == 4);
  const auto clamped = conversion_preedit(Json{{"conversion", "我去"}, {"conversion_focus_start", 9}, {"conversion_focus_end", 1}});
  assert(clamped.caret_bytes == 6 && clamped.focus_end_bytes == 6 && clamped.caret_scalars == 2);

  // Cantonese, Zhuyin and Stroke are available only when their dictionary is a file in the language_dictionaries directory; every other known scheme always is.
  char pattern[] = "/tmp/msime-input-schemes-XXXXXX";
  assert(mkdtemp(pattern) != nullptr);
  const fs::path directory = pattern;
  const Json options = {{"language_dictionaries", directory.string()}};
  const auto empty = language_dictionary_availability(options);
  const auto no_directory = language_dictionary_availability(Json::object());
  for (const char *id : {"quanpin", "shuangpin", "wubi", "japanese", "korean", "vietnamese", "tibetan"}) {
    assert(input_scheme_available(id, empty));
    assert(input_scheme_available(id, no_directory));
  }
  assert(!input_scheme_available("pinyin", empty));
  assert(!input_scheme_available("cantonese", empty));
  assert(!input_scheme_available("zhuyin", empty));
  assert(!input_scheme_available("stroke", empty));
  assert(!input_scheme_available("stroke", no_directory));
  assert(!input_scheme_available("cantonese", no_directory));
  assert(!language_dictionary_availability(Json{{"language_dictionaries", ""}}).zhuyin);
  assert(!language_dictionary_availability(nullptr).cantonese);

  // Without the data the scheme falls back to the last Chinese scheme, or to quanpin when that one cannot run or is not Chinese.
  assert(effective_input_scheme("cantonese", "wubi", empty) == "wubi");
  assert(effective_input_scheme("zhuyin", "cantonese", empty) == "quanpin");
  assert(effective_input_scheme("zhuyin", "vietnamese", empty) == "quanpin");
  assert(effective_input_scheme("unknown", "shuangpin", empty) == "shuangpin");
  assert(effective_input_scheme("vietnamese", "wubi", empty) == "vietnamese");
  assert(effective_input_scheme("tibetan", "wubi", empty) == "tibetan");
  assert(effective_input_scheme("zhuyin", "tibetan", empty) == "quanpin");
  assert(effective_input_scheme("korean", "wubi", no_directory) == "korean");
  assert(effective_input_scheme("stroke", "wubi", empty) == "wubi");
  assert(effective_input_scheme("stroke", "stroke", empty) == "quanpin");

  // A directory with the dictionary name is not a dictionary.
  fs::create_directory(directory / "msime-zhuyin.db");
  std::ofstream(directory / "msime-cantonese.db") << "db";
  // The availability is a snapshot of the disk when the options were read: it does not change until they are read again.
  assert(!input_scheme_available("cantonese", empty));
  const auto installed = language_dictionary_availability(options);
  assert(!input_scheme_available("zhuyin", installed));
  assert(input_scheme_available("cantonese", installed));
  assert(effective_input_scheme("cantonese", "wubi", installed) == "cantonese");
  assert(effective_input_scheme("zhuyin", "cantonese", installed) == "cantonese");
  assert(!installed.stroke);

  // msime-stroke.db alone makes Stroke available, and a Stroke preference whose dictionary is missing falls back to it as the last Chinese scheme.
  fs::create_directory(directory / "stroke-only");
  std::ofstream(directory / "stroke-only" / "msime-stroke.db") << "db";
  const auto stroke_only = language_dictionary_availability(Json{{"language_dictionaries", (directory / "stroke-only").string()}});
  assert(stroke_only.stroke && !stroke_only.cantonese && !stroke_only.zhuyin);
  assert(input_scheme_available("stroke", stroke_only));
  assert(effective_input_scheme("stroke", "wubi", stroke_only) == "stroke");
  assert(effective_input_scheme("cantonese", "stroke", stroke_only) == "stroke");
  // A directory named msime-stroke.db is not a dictionary.
  fs::create_directory(directory / "msime-stroke.db");
  assert(!language_dictionary_availability(options).stroke);

  fs::remove_all(directory);
  return 0;
}
