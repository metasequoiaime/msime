#include "VoiceHotwordTexts.h"

#include <cassert>
#include <nlohmann/json.hpp>

int main() {
  const auto hotwords = nlohmann::json::array({
      {{"text", "水杉"}, {"pinyin", "shui shan"}},
      {{"text", "输入法"}, {"pinyin", "shu ru fa"}},
      {{"pinyin", "missing text"}},
  });

  const auto texts = msime::windows::hotword_texts(hotwords);
  assert((texts == std::vector<std::string>{"水杉", "输入法"}));
  assert(texts.capacity() >= hotwords.size());
}
