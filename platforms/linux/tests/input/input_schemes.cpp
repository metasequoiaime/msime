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
  assert(scheme_number("pinyin") == -1);
  assert(scheme_number("") == -1);

  // Vietnamese is a non-Chinese input language like Japanese and Korean; Cantonese and Zhuyin are Chinese schemes.
  for (int number : {0, 1, 2, 5, 6})
    assert(scheme::IsChinese(number));
  for (int number : {3, 4, 7, 8, -1})
    assert(!scheme::IsChinese(number));

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

  // Cantonese and Zhuyin are available only when their dictionary is a file in the language_dictionaries directory; every other known scheme always is.
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

  fs::remove_all(directory);
  return 0;
}
