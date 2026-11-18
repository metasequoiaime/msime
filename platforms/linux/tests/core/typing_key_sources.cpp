#include "system/TypingKeyPressRequest.h"
#include "msime_client.h"

#include <unistd.h>
#include <cassert>
#include <filesystem>

using msime::linux_host::KeyPressCounter;
using msime::linux_host::TypingSource;
using msime::linux_host::typing_key_press_request;
using Json = nlohmann::json;

namespace {
void source_capture_waits_for_a_view_and_prioritizes_closed_mode() {
  int reads = 0;
  auto source = [&] {
    ++reads;
    return TypingSource::Japanese;
  };
  using msime::linux_host::typing_key_press_source;
  assert(typing_key_press_source(false, Json(), source) == TypingSource::English);
  assert(typing_key_press_source(true, Json(), source) == TypingSource::Unknown);
  assert(typing_key_press_source(true, Json::object(), source) == TypingSource::Unknown);
  assert(reads == 0);
  const Json local_mode{{"scheme", 0}, {"local_mode", "temporary_japanese"}};
  assert(typing_key_press_source(false, local_mode, source) == TypingSource::English);
  assert(reads == 0);
  assert(typing_key_press_source(true, local_mode, source) == TypingSource::Japanese);
  assert(reads == 1);
}

void mixed_sources_and_snapshots() {
  KeyPressCounter counter;
  assert(!counter.add("KeyA", "/synthetic", "2026-11-17", 0, TypingSource::Quanpin));
  assert(!counter.add("KeyA", "/synthetic", "2026-11-17", 1, TypingSource::English));
  assert(!counter.add("KeyA", "/synthetic", "2026-11-17", 2, TypingSource::English));
  assert(!counter.add("KeyB", "/synthetic", "2026-11-17", 3, TypingSource::Japanese));
  assert(!counter.add("Tab", "/synthetic", "2026-11-17", 4, TypingSource::English));
  assert(!counter.add("KeyC", "/synthetic", "2026-11-17", 5));
  assert(counter.pending() == 6);
  auto batch = counter.take();
  assert(batch && counter.pending() == 0);
  assert(batch->keys.at("KeyA") == 3);
  assert(batch->sources.at("quanpin").at("KeyA") == 1);
  assert(batch->sources.at("english").at("KeyA") == 2);
  assert(batch->sources.at("japanese").at("KeyB") == 1);
  assert(batch->sources.at("unknown").at("KeyC") == 1);
  const auto snapshot = typing_key_press_request(*batch);
  assert(snapshot.at("action").size() == 4);
  assert(snapshot.at("action").at("keys") == Json(batch->keys));
  assert(snapshot.at("action").at("sources") == Json(batch->sources));
  assert(!counter.add("KeyA", "/synthetic", "2026-11-17", 6, TypingSource::Korean));
  auto next = counter.take();
  assert(next && next->sources.size() == 1);
  assert(next->sources.at("korean").at("KeyA") == 1);
  assert(typing_key_press_request(*batch) == snapshot);
}

void flush_and_repeat_rules() {
  KeyPressCounter counter;
  assert(!counter.add("KeyA", "/first", "2026-11-17", 0, TypingSource::Quanpin));
  auto yesterday = counter.add("KeyA", "/first", "2026-11-18", 10, TypingSource::English);
  assert(yesterday && yesterday->day == "2026-11-17");
  assert(yesterday->sources.at("quanpin").at("KeyA") == 1);
  assert(!counter.take_due(KeyPressCounter::kFlushAgeMicroseconds));
  auto old_directory = counter.add("KeyB", "/second", "2026-11-18", 20, TypingSource::Japanese);
  assert(old_directory && old_directory->directory == "/first");
  assert(old_directory->sources.at("english").at("KeyA") == 1);
  auto due = counter.take_due(KeyPressCounter::kFlushAgeMicroseconds + 20);
  assert(due && due->sources.at("japanese").at("KeyB") == 1);
  for (uint64_t count = 1; count < KeyPressCounter::kFlushPresses; ++count)
    assert(!counter.add("KeyA", "/first", "2026-11-18", 0,
                        count % 2 ? TypingSource::Quanpin : TypingSource::English));
  auto full = counter.add("KeyA", "/first", "2026-11-18", 1, TypingSource::English);
  assert(full && full->keys.at("KeyA") == KeyPressCounter::kFlushPresses);
  assert(full->sources.at("quanpin").at("KeyA") == 128);
  assert(full->sources.at("english").at("KeyA") == 128);

  constexpr auto gap = KeyPressCounter::kEventRepeatGapMicroseconds;
  auto id = counter.down(30, 100, gap);
  assert(id == "KeyA");
  assert(!counter.add(id, "/first", "2026-11-18", 100, TypingSource::Quanpin));
  assert(counter.down(30, 200, gap).empty());
  auto held = counter.take();
  assert(held && held->sources.at("quanpin").at("KeyA") == 1);
  assert(counter.down(30, 300, gap).empty());
  counter.up(30, 400);
  assert(counter.down(30, 400, gap).empty());
  counter.forget_held();
  id = counter.down(30, 10'000, gap);
  assert(id == "KeyA");
  assert(!counter.add(id, "/first", "2026-11-18", 10'000, TypingSource::English));
  assert(counter.take()->sources.at("english").at("KeyA") == 1);
}

Json call(const Json &request) {
  const auto encoded = request.dump();
  auto *raw = msime_client_typing_statistics(
      reinterpret_cast<const uint8_t *>(encoded.data()), encoded.size());
  assert(raw);
  const auto result = Json::parse(raw);
  msime_client_string_free(raw);
  return result;
}

void shared_store_accepts_source_batches() {
  const auto root = std::filesystem::temp_directory_path() /
      ("msime-linux-key-sources-" + std::to_string(::getpid()));
  std::filesystem::remove_all(root);
  std::filesystem::create_directories(root);
  const auto directory = root.string();
  const auto day = msime::linux_host::local_day(std::time(nullptr));
  auto action = [&](const Json &value) {
    return call({{"directory", directory}, {"action", value}});
  };
  KeyPressCounter counter;
  assert(!counter.add("KeyA", directory, day, 0, TypingSource::Quanpin));
  assert(!counter.add("KeyA", directory, day, 1, TypingSource::English));
  assert(!counter.add("KeyB", directory, day, 2, TypingSource::Japanese));
  assert(!counter.add("Tab", directory, day, 3, TypingSource::Quanpin));
  assert(!counter.add("Tab", directory, day, 4, TypingSource::English));
  assert(!counter.add("KeyC", directory, day, 5));
  auto batch = counter.take();
  assert(batch);
  const auto request = typing_key_press_request(*batch);
  // 默认关闭不因携带来源而改变。
  assert(call(request).at("value").at("recorded") == 0);
  assert(action({{"operation", "set_enabled"}, {"enabled", true}}).at("ok") == true);
  assert(call(request).at("value").at("recorded") == 6);
  auto loaded = action({{"operation", "load"}});
  const auto &value = loaded.at("value");
  assert(value.at("dailyKeys").at(day) == Json(batch->keys));
  const auto &groups = value.at("dailyKeyLanguages").at(day);
  assert(groups.at("chinese").at("KeyA") == 1);
  assert(groups.at("english").at("KeyA") == 1);
  assert(groups.at("japanese").at("KeyB") == 1);
  assert(groups.at("unknown").at("KeyC") == 1);
  assert(groups.at("common").at("Tab") == 2);
  // 旧宿主缺省来源时仍只补未知，不按当前模式回填。
  assert(action({{"operation", "record_keys"}, {"day", day},
                 {"keys", {{"KeyA", 1}}}}).at("ok") == true);
  loaded = action({{"operation", "load"}});
  assert(loaded.at("value").at("dailyKeyLanguages").at(day).at("unknown").at("KeyA") == 1);
  auto malformed = request;
  malformed["action"]["sources"]["english"]["KeyA"] = 2;
  assert(call(malformed).at("ok") == false);
  assert(action({{"operation", "load"}}) == loaded);
  // 全部来源经真实共享入口校验，C++ 不复制语言分类规则。
  for (int raw = 0; raw <= static_cast<int>(TypingSource::SmartAbc); ++raw)
    assert(!counter.add("KeyZ", directory, day, raw, static_cast<TypingSource>(raw)));
  auto all = counter.take();
  assert(all);
  assert(call(typing_key_press_request(*all)).at("value").at("recorded") ==
         static_cast<int>(TypingSource::SmartAbc) + 1);
  std::filesystem::remove_all(root);
}
} // namespace

int main() {
  source_capture_waits_for_a_view_and_prioritizes_closed_mode();
  mixed_sources_and_snapshots();
  flush_and_repeat_rules();
  shared_store_accepts_source_batches();
}
