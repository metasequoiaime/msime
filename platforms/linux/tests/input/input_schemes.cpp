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
  assert(scheme_number("stroke") == scheme::Stroke);
  assert(scheme::Stroke == 8);
  assert(scheme_number("pinyin") == -1);
  assert(scheme_number("") == -1);

  // Vietnamese is a non-Chinese input language like Japanese and Korean; Cantonese, Zhuyin and Stroke are Chinese schemes.
  for (int number : {0, 1, 2, 5, 6, 8})
    assert(scheme::IsChinese(number));
  for (int number : {3, 4, 7, 9, -1})
    assert(!scheme::IsChinese(number));
  // Stroke copies Cantonese's traits: its candidates come from stroke.db as stored, nothing is learned, and its composition is neither kept on blur nor drawn inline whatever the preedit style.
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
  // A Stroke composition (ASCII letters in editing_text, the stroke glyphs in reading) is not a list composition.
  const Json stroke = {{"scheme", 8}, {"editing_text", "hs"}, {"reading", "一丨"}, {"candidates", Json::array({Json{{"text", "十"}}})}};
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
  // A local mode keeps its own rules in every scheme.
  Json zhuyin_mode = zhuyin;
  zhuyin_mode["local_mode"] = "expression";
  assert(scheme_rules(zhuyin_mode) == -1);
  assert(!candidate_list_composition(zhuyin_mode));
  assert(!candidate_list_composition(nullptr));

  // Cantonese, Zhuyin and Stroke are available only when their dictionary is a file in the language_dictionaries directory; every other known scheme always is.
  char pattern[] = "/tmp/msime-input-schemes-XXXXXX";
  assert(mkdtemp(pattern) != nullptr);
  const fs::path directory = pattern;
  const Json options = {{"language_dictionaries", directory.string()}};
  const auto empty = language_dictionary_availability(options);
  const auto no_directory = language_dictionary_availability(Json::object());
  for (const char *id : {"quanpin", "shuangpin", "wubi", "japanese", "korean", "vietnamese"}) {
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
  assert(effective_input_scheme("korean", "wubi", no_directory) == "korean");
  assert(effective_input_scheme("stroke", "wubi", empty) == "wubi");
  assert(effective_input_scheme("stroke", "stroke", empty) == "quanpin");

  // A directory with the dictionary name is not a dictionary.
  fs::create_directory(directory / "zhuyin.db");
  std::ofstream(directory / "cantonese.db") << "db";
  // The availability is a snapshot of the disk when the options were read: it does not change until they are read again.
  assert(!input_scheme_available("cantonese", empty));
  const auto installed = language_dictionary_availability(options);
  assert(!input_scheme_available("zhuyin", installed));
  assert(input_scheme_available("cantonese", installed));
  assert(effective_input_scheme("cantonese", "wubi", installed) == "cantonese");
  assert(effective_input_scheme("zhuyin", "cantonese", installed) == "cantonese");
  assert(!installed.stroke);

  // stroke.db alone makes Stroke available, and a Stroke preference whose dictionary is missing falls back to it as the last Chinese scheme.
  fs::create_directory(directory / "stroke-only");
  std::ofstream(directory / "stroke-only" / "stroke.db") << "db";
  const auto stroke_only = language_dictionary_availability(Json{{"language_dictionaries", (directory / "stroke-only").string()}});
  assert(stroke_only.stroke && !stroke_only.cantonese && !stroke_only.zhuyin);
  assert(input_scheme_available("stroke", stroke_only));
  assert(effective_input_scheme("stroke", "wubi", stroke_only) == "stroke");
  assert(effective_input_scheme("cantonese", "stroke", stroke_only) == "stroke");
  // A directory named stroke.db is not a dictionary.
  fs::create_directory(directory / "stroke.db");
  assert(!language_dictionary_availability(options).stroke);

  fs::remove_all(directory);
  return 0;
}
