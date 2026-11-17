#include "../../common/TypingKeyBatch.h"
#include "../../tsf/KeyTypingSourceResponse.h"
#include "TypingStatistics.h"
#include <cassert>
#include <iostream>

using namespace msime::windows;
using Json = nlohmann::json;

int main() {
  msime::tsf::KeyTypingSource current;
  const auto observe = [&](const Json &value) {
    msime::tsf::observe_key_typing_source(current, value);
  };
  observe(Json{{"scheme", 1},
               {"local_mode", "none"},
               {"shuangpin_profile", "microsoft"}});
  assert(current.source(1, false, true) == TypingSource::Microsoft);
  assert(current.source(1, false, false) == TypingSource::English);
  assert(current.source(1, true, true) == TypingSource::English);
  observe(Json{{"view", {{"scheme", 1}, {"local_mode", "temporary_japanese"}}},
               {"commit_context", {{"scheme", 1}, {"local_mode", "none"}}}});
  assert(current.source(1, false, true) == TypingSource::Japanese);
  assert(current.source(4, false, true) == TypingSource::Korean);
  observe(Json{{"query", "synthetic"}});
  assert(current.source(1, false, true) == TypingSource::Japanese);
  observe(Json{{"view", {{"scheme", 1}, {"local_mode", "none"}}},
               {"commit_context",
                {{"scheme", 1}, {"local_mode", "temporary_japanese"}}}});
  assert(current.source(1, false, true) == TypingSource::Shuangpin);
  observe(Json{{"view", {{"scheme", 1}, {"local_mode", "emoji"}}}});
  assert(current.source(1, false, true) == TypingSource::Local);
  current.reset();
  assert(current.source(3, false, true) == TypingSource::Unknown);

  TypingKeyBatch batch;
  batch.day = L"2026-11-01";
  batch.add(L"KeyA", TypingSource::Japanese);
  batch.add(L"KeyA", TypingSource::English);
  batch.add(L"ShiftLeft", TypingSource::English);
  const auto sealed = batch;
  batch.add(L"KeyA", TypingSource::Shuangpin);
  assert(sealed.counts.at(L"KeyA") == 2);
  assert(sealed.sources.at(TypingSource::Japanese).at(L"KeyA") == 1);
  assert(sealed.sources.at(TypingSource::English).at(L"KeyA") == 1);
  assert(sealed.sources.count(TypingSource::Shuangpin) == 0);
  for (const auto &[key, total] : batch.counts) {
    uint64_t sum = 0;
    for (const auto &[source, counts] : batch.sources) {
      (void)source;
      const auto found = counts.find(key);
      if (found != counts.end())
        sum += found->second;
    }
    assert(sum == total);
  }
  const std::map<std::string, uint64_t> keys{{"KeyA", 2}};
  const auto legacy = Json::parse(
      typing_statistics_record_keys_request("/synthetic", "2026-11-01", keys));
  assert(!legacy.at("action").contains("sources"));
  const auto sourced = Json::parse(typing_statistics_record_keys_request(
      "/synthetic", "2026-11-01", keys, "japanese"));
  assert(sourced.at("action").at("keys") ==
         sourced.at("action").at("sources").at("japanese"));
  std::cout << "Typing key sources passed\n";
}
