#include "../src/system/TypingStatistics.h"
#include "msime_client.h"

#include <sys/stat.h>
#include <unistd.h>

#include <cassert>
#include <chrono>
#include <cstdint>
#include <filesystem>
#include <map>
#include <set>
#include <string>
#include <string_view>
#include <thread>

using msime::linux_host::evdev_key_id;
using msime::linux_host::KeyPressCounter;
using msime::linux_host::local_day;
using msime::linux_host::PassthroughModifiers;
using msime::linux_host::resolve_typing_source;
using msime::linux_host::should_count_passthrough_character;
using msime::linux_host::typing_source_id;
using msime::linux_host::TypingSource;
using msime::linux_host::TypingStatisticsSwitch;

namespace {

int queries = 0;
int32_t counted_query(const uint8_t *directory, std::size_t length) {
  ++queries;
  return msime_client_typing_statistics_enabled(directory, length);
}
int failed_queries = 0;
int32_t failing_query(const uint8_t *, std::size_t) {
  ++failed_queries;
  return -1;
}

std::string call(const std::string &directory, const std::string &action) {
  const auto request = "{\"directory\":\"" + directory + "\",\"action\":" + action + "}";
  char *raw = msime_client_typing_statistics(reinterpret_cast<const uint8_t *>(request.data()), request.size());
  assert(raw);
  std::string result(raw);
  msime_client_string_free(raw);
  return result;
}

// What the settings page's switch does to the shared store.
void set_enabled(const std::string &directory, bool enabled) {
  const auto result = call(directory, std::string("{\"operation\":\"set_enabled\",\"enabled\":") +
                                          (enabled ? "true" : "false") + "}");
  assert(result.find(enabled ? "\"enabled\":true" : "\"enabled\":false") != std::string::npos);
}

struct FileIdentity {
  ino_t inode = 0;
  timespec modified{};
  bool operator==(const FileIdentity &other) const {
    return inode == other.inode && modified.tv_sec == other.modified.tv_sec &&
           modified.tv_nsec == other.modified.tv_nsec;
  }
};
FileIdentity identity(const std::string &path) {
  struct stat info {};
  const int status = ::stat(path.c_str(), &info);
  assert(status == 0);
  (void)status;
  return {info.st_ino, info.st_mtim};
}

void switch_follows_the_store() {
  const auto root = std::filesystem::temp_directory_path() /
                    ("msime-linux-typing-statistics-" + std::to_string(::getpid()));
  std::filesystem::remove_all(root);
  std::filesystem::create_directories(root);
  const auto directory = root.string();
  const auto document = (root / "typing-statistics.json").string();

  TypingStatisticsSwitch statistics{counted_query};
  // Off until the store has been read: nothing is recorded on a guess.
  assert(!statistics.enabled());
  // A host without an absolute preferences directory has no store to ask.
  statistics.refresh("");
  statistics.refresh("relative/directory");
  assert(!statistics.enabled());
  assert(queries == 0);

  // A fresh profile: no document yet, and the store ships statistics off.
  statistics.refresh(directory);
  assert(queries == 1);
  assert(!statistics.enabled());
  statistics.refresh(directory);
  assert(queries == 1);
  assert(!std::filesystem::exists(document));

  // Statistics turned off explicitly. The host's capture gate stays shut, and preference ticks while it is off only stat the document: it is neither read again nor written.
  set_enabled(directory, false);
  const auto off = identity(document);
  statistics.refresh(directory);
  assert(queries == 2);
  assert(!statistics.enabled());
  for (int tick = 0; tick < 5; ++tick)
    statistics.refresh(directory);
  assert(queries == 2);
  assert(!statistics.enabled());
  assert(identity(document) == off);

  // Turned on in the settings: the next preference tick sees the rewritten document and opens the gate. That the hosts then record commits, and record none while it is shut, is checked end to end in fcitx5/tests/native.cpp; this test only covers the switch.
  set_enabled(directory, true);
  assert(!statistics.enabled());
  statistics.refresh(directory);
  assert(queries == 3);
  assert(statistics.enabled());
  // Any store write replaces the document, a recorded commit included; this one goes straight to the store only to change the document's identity, and the tick after it reads it again and stays on.
  const auto recorded = call(directory, "{\"operation\":\"record\",\"text\":\"输入法\",\"source\":\"quanpin\",\"day\":\"2026-09-23\",\"hour\":10}");
  assert(recorded.find("\"recorded\":3") != std::string::npos);
  statistics.refresh(directory);
  assert(queries == 4);
  assert(statistics.enabled());
  statistics.refresh(directory);
  assert(queries == 4);

  // Turned off again: the next tick shuts the gate.
  set_enabled(directory, false);
  statistics.refresh(directory);
  assert(queries == 5);
  assert(!statistics.enabled());

  // Moving to another store always asks it, even when both documents happen to look alike.
  const auto other = (root / "other").string();
  statistics.refresh(other);
  assert(queries == 6);
  assert(!statistics.enabled());

  // A store that cannot be read keeps the gate shut and is asked again on the next tick instead of being trusted as unchanged.
  TypingStatisticsSwitch unreadable{failing_query};
  unreadable.refresh(directory);
  unreadable.refresh(directory);
  assert(failed_queries == 2);
  assert(!unreadable.enabled());

  std::filesystem::remove_all(root);
}

// Every evdev code the hosts can see names at most one key, and every name is one the store accepts: one id outside its whitelist would make it reject the whole batch the key landed in.
void key_ids_match_the_store() {
  assert(evdev_key_id(0).empty());
  assert(evdev_key_id(30) == "KeyA");
  assert(evdev_key_id(44) == "KeyZ");
  assert(evdev_key_id(2) == "Digit1");
  assert(evdev_key_id(11) == "Digit0");
  assert(evdev_key_id(57) == "Space");
  assert(evdev_key_id(14) == "Backspace");
  assert(evdev_key_id(28) == "Enter");
  assert(evdev_key_id(42) == "ShiftLeft");
  assert(evdev_key_id(54) == "ShiftRight");
  assert(evdev_key_id(41) == "Backquote");
  assert(evdev_key_id(40) == "Quote");
  assert(evdev_key_id(86) == "IntlBackslash");
  assert(evdev_key_id(89) == "IntlRo");
  assert(evdev_key_id(124) == "IntlYen");
  assert(evdev_key_id(96) == "NumpadEnter");
  assert(evdev_key_id(125) == "MetaLeft");
  assert(evdev_key_id(464) == "Fn");
  // PrintScreen, ScrollLock, Pause and F13 have no id in the store's list and are not counted.
  assert(evdev_key_id(99).empty());
  assert(evdev_key_id(70).empty());
  assert(evdev_key_id(119).empty());
  assert(evdev_key_id(183).empty());

  std::map<std::string, uint64_t> all;
  std::set<std::string_view> seen;
  for (uint32_t code = 0; code < 1024; ++code) {
    const auto id = evdev_key_id(code);
    if (id.empty())
      continue;
    assert(seen.insert(id).second);
    all[std::string(id)] = 1;
  }
  assert(all.size() == 110);

  const auto root = std::filesystem::temp_directory_path() /
                    ("msime-linux-key-ids-" + std::to_string(::getpid()));
  std::filesystem::remove_all(root);
  std::filesystem::create_directories(root);
  const auto directory = root.string();
  set_enabled(directory, true);
  std::string keys;
  for (const auto &[id, count] : all)
    keys += (keys.empty() ? "\"" : ",\"") + id + "\":" + std::to_string(count);
  const auto recorded =
      call(directory, "{\"operation\":\"record_keys\",\"day\":\"2026-09-30\",\"keys\":{" + keys + "}}");
  assert(recorded.find("\"ok\":true") != std::string::npos);
  assert(recorded.find("\"recorded\":" + std::to_string(all.size())) != std::string::npos);
  std::filesystem::remove_all(root);
}

void counter_batches_presses() {
  KeyPressCounter counter;
  constexpr auto gap = KeyPressCounter::kEventRepeatGapMicroseconds;
  // Auto-repeat sends more downs for a held key; only the first counts, and the release lets the next press count again.
  assert(counter.down(30, 0, gap) == "KeyA");
  assert(counter.add("KeyA", "/store", "2026-09-30", 0) == std::nullopt);
  assert(counter.down(30, 30'000, gap).empty());
  assert(counter.down(30, 60'000, gap).empty());
  counter.up(30, 100'000);
  assert(counter.down(30, 200'000, gap) == "KeyA");
  assert(!counter.add("KeyA", "/store", "2026-09-30", 1000));
  // A key without an id is not remembered or counted.
  assert(counter.down(99, 300'000, gap).empty());
  counter.up(99, 300'000);
  counter.up(4096, 300'000);
  assert(counter.pending() == 2);

  // Not due before 30 s from the batch's first press, due after.
  assert(!counter.take_due(KeyPressCounter::kFlushAgeMicroseconds - 1));
  auto due = counter.take_due(KeyPressCounter::kFlushAgeMicroseconds);
  assert(due && due->directory == "/store" && due->day == "2026-09-30");
  assert(due->keys.size() == 1 && due->keys.at("KeyA") == 2);
  assert(counter.pending() == 0);
  assert(!counter.take());
  assert(!counter.take_due(KeyPressCounter::kFlushAgeMicroseconds * 10));

  // A flush does not release a key that is still down; focus loss forgets it.
  assert(counter.down(30, 400'000, gap).empty());
  counter.forget_held();
  assert(counter.down(30, 500'000, gap) == "KeyA");
  counter.up(30, 600'000);

  // Presses before midnight stay on their day: the first press of the new day hands back the old day's batch and starts a new one.
  assert(counter.down(57, 700'000, gap) == "Space");
  assert(!counter.add("Space", "/store", "2026-09-30", 0));
  counter.up(57, 800'000);
  assert(counter.down(57, 900'000, gap) == "Space");
  auto yesterday = counter.add("Space", "/store", "2026-10-01", 10);
  counter.up(57, 1'000'000);
  assert(yesterday && yesterday->day == "2026-09-30" && yesterday->keys.at("Space") == 1);
  assert(counter.pending() == 1);
  // The age restarts with the new batch.
  assert(!counter.take_due(KeyPressCounter::kFlushAgeMicroseconds));
  // Another store gets its own batch too.
  auto previous_store = counter.add("Space", "/other", "2026-10-01", 20);
  assert(previous_store && previous_store->directory == "/store" && previous_store->day == "2026-10-01");
  auto other = counter.take();
  assert(other && other->directory == "/other" && other->keys.at("Space") == 1);

  // The batch is handed back as soon as it holds 256 presses.
  for (uint64_t press = 1; press < KeyPressCounter::kFlushPresses; ++press)
    assert(!counter.add(press % 2 ? "KeyJ" : "KeyK", "/store", "2026-10-01", 0));
  auto full = counter.add("KeyK", "/store", "2026-10-01", 0);
  assert(full && full->keys.at("KeyJ") + full->keys.at("KeyK") == KeyPressCounter::kFlushPresses);
  assert(counter.pending() == 0);

  // The day is the store's YYYY-MM-DD form.
  const auto today = local_day(std::time(nullptr));
  assert(today.size() == 10 && today[4] == '-' && today[7] == '-');
}

void counter_ignores_synthetic_repeat_pairs() {
  constexpr auto event_gap = KeyPressCounter::kEventRepeatGapMicroseconds;
  constexpr auto arrival_gap = KeyPressCounter::kArrivalRepeatGapMicroseconds;
  KeyPressCounter counter;
  // X11 without detectable auto-repeat: each repeat is a release and a press stamped with the same time, 33 ms apart at 30 Hz.
  assert(counter.down(30, 1'000'000, event_gap) == "KeyA");
  for (int64_t at = 1'500'000; at < 2'000'000; at += 33'000) {
    counter.up(30, at);
    assert(counter.down(30, at, event_gap).empty());
  }
  // The stamps have millisecond resolution, so a pair one stamp apart is still a repeat.
  counter.up(30, 2'100'000);
  assert(counter.down(30, 2'101'000, event_gap).empty());
  // The real release ends the hold; a deliberate retap tens of milliseconds later counts.
  counter.up(30, 2'200'000);
  assert(counter.down(30, 2'230'000, event_gap) == "KeyA");
  counter.up(30, 2'300'000);
  // Only the key just released can be repeating: another key pressed at the same moment counts.
  assert(counter.down(31, 2'300'000, event_gap) == "KeyS");
  counter.up(31, 2'400'000);
  // A key released and then pressed again past the gap counts.
  assert(counter.down(31, 2'400'000 + event_gap + 1, event_gap) == "KeyS");
  counter.up(31, 2'500'000);
  // A press stamped before the last release (another clock, or a wrapped one) is not taken for a repeat.
  assert(counter.down(31, 2'499'000, event_gap) == "KeyS");
  counter.up(31, 2'600'000);
  // Focus loss forgets the last release too.
  counter.forget_held();
  assert(counter.down(31, 2'600'000, event_gap) == "KeyS");
  counter.up(31, 2'700'000);

  // A host without event times (IBus) measures arrival: the press reaches it a few milliseconds after the release was answered.
  KeyPressCounter arrival;
  assert(arrival.down(57, 0, arrival_gap) == "Space");
  arrival.up(57, 500'000);
  assert(arrival.down(57, 503'000, arrival_gap).empty());
  arrival.up(57, 536'000);
  assert(arrival.down(57, 536'000 + arrival_gap, arrival_gap).empty());
  arrival.up(57, 700'000);
  assert(arrival.down(57, 760'000, arrival_gap) == "Space");
}

void pending_writes_wait_for_running_writes() {
  msime::linux_host::PendingWrites writes;
  assert(writes.wait_idle(std::chrono::milliseconds(0)));
  writes.begin();
  writes.begin();
  assert(!writes.wait_idle(std::chrono::milliseconds(1)));
  std::thread worker([&writes] {
    std::this_thread::sleep_for(std::chrono::milliseconds(20));
    writes.end();
    writes.end();
  });
  assert(writes.wait_idle(std::chrono::seconds(5)));
  worker.join();
}

} // namespace

int main() {
  assert(resolve_typing_source(0, false, false, "none", "xiaohe") ==
         TypingSource::Quanpin);
  assert(resolve_typing_source(0, true, false, "none", "xiaohe") ==
         TypingSource::NineKey);
  assert(resolve_typing_source(1, false, false, "none", "xiaohe") ==
         TypingSource::Shuangpin);
  assert(resolve_typing_source(1, false, false, "none", "ziranma") ==
         TypingSource::Ziranma);
  assert(resolve_typing_source(1, false, false, "none", "microsoft") ==
         TypingSource::Microsoft);
  assert(resolve_typing_source(1, false, false, "none", "shoudao") ==
         TypingSource::Shoudao);
  assert(resolve_typing_source(2, false, false, "none", "xiaohe") ==
         TypingSource::Wubi);
  assert(resolve_typing_source(3, false, false, "none", "xiaohe") ==
         TypingSource::Japanese);
  assert(resolve_typing_source(4, false, false, "none", "xiaohe") ==
         TypingSource::Korean);
  assert(typing_source_id(TypingSource::Korean) ==
         std::string_view("korean"));
  assert(resolve_typing_source(5, false, false, "none", "xiaohe") ==
         TypingSource::Cantonese);
  assert(resolve_typing_source(6, false, false, "none", "xiaohe") ==
         TypingSource::Zhuyin);
  assert(resolve_typing_source(7, false, false, "none", "xiaohe") ==
         TypingSource::Vietnamese);
  assert(typing_source_id(TypingSource::Cantonese) ==
         std::string_view("cantonese"));
  assert(typing_source_id(TypingSource::Zhuyin) ==
         std::string_view("zhuyin"));
  assert(typing_source_id(TypingSource::Vietnamese) ==
         std::string_view("vietnamese"));
  assert(resolve_typing_source(8, false, false, "none", "xiaohe") ==
         TypingSource::Tibetan);
  assert(typing_source_id(TypingSource::Tibetan) ==
         std::string_view("tibetan"));
  assert(resolve_typing_source(9, false, false, "none", "xiaohe") ==
         TypingSource::Unknown);
  assert(resolve_typing_source(0, false, true, "none", "xiaohe") ==
         TypingSource::English);
  assert(resolve_typing_source(0, false, false, "temporary_japanese",
                               "xiaohe") == TypingSource::Japanese);
  assert(resolve_typing_source(0, false, false, "emoji", "xiaohe") ==
         TypingSource::Local);
  assert(resolve_typing_source(99, false, false, "none", "xiaohe") ==
         TypingSource::Unknown);
  assert(typing_source_id(TypingSource::NineKey) ==
         std::string_view("nineKey"));
  assert(typing_source_id(TypingSource::Voice) == std::string_view("voice"));

  // Passthrough keys count as typed text only when they are printable and no shortcut modifier is held; Shift picks a character and does not make a shortcut.
  assert(should_count_passthrough_character(U'a', {}));
  assert(should_count_passthrough_character(U' ', {}));
  assert(should_count_passthrough_character(U'~', {}));
  assert(should_count_passthrough_character(U'\u00e9', {}));
  assert(should_count_passthrough_character(U'\U0001F600', {}));
  assert(!should_count_passthrough_character(U'\t', {}));
  assert(!should_count_passthrough_character(U'\r', {}));
  assert(!should_count_passthrough_character(0x1b, {}));
  assert(!should_count_passthrough_character(0x7f, {}));
  assert(!should_count_passthrough_character(0, {}));
  assert(!should_count_passthrough_character(0xd800, {}));
  assert(!should_count_passthrough_character(0x110000, {}));
  PassthroughModifiers control;
  control.control = true;
  assert(!should_count_passthrough_character(U'c', control));
  PassthroughModifiers alt;
  alt.alt = true;
  assert(!should_count_passthrough_character(U'f', alt));
  PassthroughModifiers super;
  super.super = true;
  assert(!should_count_passthrough_character(U'l', super));
  PassthroughModifiers hyper;
  hyper.hyper = true;
  assert(!should_count_passthrough_character(U'h', hyper));
  PassthroughModifiers meta;
  meta.meta = true;
  assert(!should_count_passthrough_character(U'm', meta));

  switch_follows_the_store();
  key_ids_match_the_store();
  counter_batches_presses();
  counter_ignores_synthetic_repeat_pairs();
  pending_writes_wait_for_running_writes();
  return 0;
}
