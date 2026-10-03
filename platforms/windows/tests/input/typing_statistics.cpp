#include "TypingStatistics.h"
#include <chrono>
#include <condition_variable>
#include <iostream>
#include <memory>
#include <mutex>
#include <stdexcept>
#include <thread>

using namespace msime::windows;
using Json = nlohmann::json;
namespace {
void require(bool value) {
  if (!value)
    throw std::runtime_error("Windows typing statistics assertion failed");
}
Json transition(int scheme, const char *local_mode, bool dedicated_english,
                const char *shuangpin_profile, Json commit_context = nullptr) {
  return {{"commit", "你好"},
          {"commit_context", std::move(commit_context)},
          {"view",
           {{"scheme", scheme},
            {"nine_key", false},
            {"dedicated_english", dedicated_english},
            {"local_mode", local_mode},
            {"shuangpin_profile", shuangpin_profile}}}};
}
} // namespace
int main() {
  try {
    // Same mapping as the Linux and Apple hosts: they write the same document,
    // so a source identifier that differs here would split one user's history
    // across two buckets.
    require(resolve_typing_source(0, false, false, "none", "xiaohe") ==
            TypingSource::Quanpin);
    require(resolve_typing_source(0, true, false, "none", "xiaohe") ==
            TypingSource::NineKey);
    require(resolve_typing_source(1, false, false, "none", "xiaohe") ==
            TypingSource::Shuangpin);
    require(resolve_typing_source(1, false, false, "none", "ziranma") ==
            TypingSource::Ziranma);
    require(resolve_typing_source(1, false, false, "none", "microsoft") ==
            TypingSource::Microsoft);
    require(resolve_typing_source(1, false, false, "none", "shoudao") ==
            TypingSource::Shoudao);
    require(resolve_typing_source(2, false, false, "none", "xiaohe") ==
            TypingSource::Wubi);
    require(resolve_typing_source(3, false, false, "none", "xiaohe") ==
            TypingSource::Japanese);
    require(resolve_typing_source(4, false, false, "none", "xiaohe") ==
            TypingSource::Korean);
    require(resolve_typing_source(5, false, false, "none", "xiaohe") ==
            TypingSource::Cantonese);
    require(resolve_typing_source(6, false, false, "none", "xiaohe") ==
            TypingSource::Zhuyin);
    require(resolve_typing_source(7, false, false, "none", "xiaohe") ==
            TypingSource::Vietnamese);
    require(typing_source_id(TypingSource::Cantonese) == "cantonese" &&
            typing_source_id(TypingSource::Zhuyin) == "zhuyin" &&
            typing_source_id(TypingSource::Vietnamese) == "vietnamese");
    require(resolve_typing_source(8, false, false, "none", "xiaohe") ==
            TypingSource::Tibetan);
    require(typing_source_id(TypingSource::Tibetan) == "tibetan");
    require(resolve_typing_source(9, false, false, "none", "xiaohe") ==
            TypingSource::Unknown);
    require(resolve_typing_source(-1, false, false, "none", "xiaohe") ==
            TypingSource::Unknown);
    // Local modes outrank the keyboard scheme, and the temporary Japanese mode
    // is reported as Japanese rather than as a generic local mode.
    require(resolve_typing_source(0, false, false, "emoji", "xiaohe") ==
            TypingSource::Local);
    require(resolve_typing_source(0, false, false, "temporary_japanese",
                                  "xiaohe") == TypingSource::Japanese);
    require(resolve_typing_source(0, false, false, "", "xiaohe") ==
            TypingSource::Quanpin);
    // Dedicated English loses to a local mode and wins over the scheme.
    require(resolve_typing_source(0, false, true, "none", "xiaohe") ==
            TypingSource::English);
    require(resolve_typing_source(0, false, true, "emoji", "xiaohe") ==
            TypingSource::Local);
    require(typing_source_id(TypingSource::NineKey) == "nineKey");
    require(typing_source_id(TypingSource::Korean) == "korean");
    require(typing_source_id(TypingSource::Unknown) == "unknown");

    // Attribution follows the mode in force when the key was dispatched.
    // Committing an Emoji-mode candidate clears the local mode, so the
    // post-commit view alone would file it under quanpin.
    require(resolve_typing_source_from_transition(
                transition(0, "none", false, "xiaohe",
                           Json{{"scheme", 0}, {"local_mode", "emoji"}})) ==
            TypingSource::Local);
    // Without a commit context the view is all there is.
    require(resolve_typing_source_from_transition(
                transition(2, "none", false, "xiaohe")) == TypingSource::Wubi);
    // A Korean syllable finished by the next letter is attributed to Korean, the scheme recorded when the key was dispatched.
    require(resolve_typing_source_from_transition(
                transition(4, "none", false, "xiaohe",
                           Json{{"scheme", 4}, {"local_mode", "none"}})) ==
            TypingSource::Korean);
    require(resolve_typing_source_from_transition(
                transition(1, "none", false, "ziranma")) ==
            TypingSource::Ziranma);
    // A transition missing the fields entirely must not throw on the input
    // path; an unknown bucket is the honest answer.
    require(resolve_typing_source_from_transition(Json::object()) ==
            TypingSource::Unknown);

    // The text V, "/" and "@" generate is not typing; anything without the field still is.
    require(transition_counts_as_typing(Json::object()));
    require(transition_counts_as_typing(
        transition(0, "none", false, "xiaohe",
                   Json{{"scheme", 0}, {"local_mode", "none"}})));
    require(transition_counts_as_typing(
        transition(0, "none", false, "xiaohe",
                   Json{{"scheme", 0},
                        {"local_mode", "none"},
                        {"typing_statistics", true}})));
    require(!transition_counts_as_typing(
        transition(0, "none", false, "xiaohe",
                   Json{{"scheme", 0},
                        {"local_mode", "expression"},
                        {"typing_statistics", false}})));
    require(transition_counts_as_typing(transition(0, "none", false, "xiaohe")));

    const auto local = local_time_parts(0);
    require(local.has_value());
    require(local->day.size() == 10 && local->day[4] == '-' &&
            local->day[7] == '-');
    require(local->hour >= 0 && local->hour <= 23);

    const auto request =
        typing_statistics_record_request("C:\\Users\\ime\\state", "你好",
                                         TypingSource::Quanpin, "2026-09-21", 9);
    const auto parsed = Json::parse(request);
    require(parsed.at("directory") == "C:\\Users\\ime\\state");
    require(parsed.at("action").at("operation") == "record");
    require(parsed.at("action").at("text") == "你好");
    require(parsed.at("action").at("source") == "quanpin");
    require(parsed.at("action").at("day") == "2026-09-21");
    require(parsed.at("action").at("hour") == 9);
    // Only a record made behind a full-screen application carries the flag.
    require(!parsed.at("action").contains("quiet"));
    const auto quiet = Json::parse(typing_statistics_record_request(
        "C:\\state", "你好", TypingSource::Quanpin, "2026-09-21", 9, true));
    require(quiet.at("action").at("quiet") == true);
    // Nothing usable in, nothing out: a record with no text, no home, or no
    // resolvable day or hour would have to invent one of them.
    require(typing_statistics_record_request("C:\\state", "", TypingSource::Ai,
                                             "2026-09-21", 0)
                .empty());
    require(typing_statistics_record_request("", "你好", TypingSource::Ai,
                                             "2026-09-21", 0)
                .empty());
    require(typing_statistics_record_request("C:\\state", "你好",
                                             TypingSource::Ai, "", 0)
                .empty());
    // Both ends of the hour range, so an off-by-one on either bound shows up.
    require(!typing_statistics_record_request("C:\\state", "你好",
                                              TypingSource::Ai, "2026-09-21", 0)
                 .empty());
    require(!typing_statistics_record_request("C:\\state", "你好",
                                              TypingSource::Ai, "2026-09-21", 23)
                 .empty());
    require(typing_statistics_record_request("C:\\state", "你好",
                                             TypingSource::Ai, "2026-09-21", 24)
                .empty());
    require(typing_statistics_record_request("C:\\state", "你好",
                                             TypingSource::Ai, "2026-09-21", -1)
                .empty());
    // The shared entry point refuses buffers past 64 KiB, so an oversized
    // commit is dropped whole rather than counted as a shorter one.
    require(typing_statistics_record_request("C:\\state",
                                             std::string(70'000, 'a'),
                                             TypingSource::Reply, "2026-09-21", 9)
                .empty());
    // Key heatmap counts: the shared record_keys shape, filed under the day they were counted on, with no hour and no order.
    {
      const auto keys_request = typing_statistics_record_keys_request(
          "C:\\state", "2026-09-30", {{"KeyA", 3}, {"Space", 2}});
      const auto keys = Json::parse(keys_request);
      require(keys.at("directory") == "C:\\state");
      require(keys.at("action").at("operation") == "record_keys");
      require(keys.at("action").at("day") == "2026-09-30");
      require(keys.at("action").at("keys") == Json{{"KeyA", 3}, {"Space", 2}});
      require(!keys.at("action").contains("hour"));
      require(typing_statistics_record_keys_request("C:\\state", "2026-09-30", {})
                  .empty());
      require(typing_statistics_record_keys_request("", "2026-09-30", {{"KeyA", 1}})
                  .empty());
      require(typing_statistics_record_keys_request("C:\\state", "", {{"KeyA", 1}})
                  .empty());
    }
    // The Aux listener's view of the switch: answered from a cache, never by waiting on the store lock a detached write holds.
    {
      const TypingStatisticsSwitch fresh([] { return 1; }, std::chrono::hours(1));
      require(fresh.enabled());
      require(!TypingStatisticsSwitch([] { return -1; }, std::chrono::hours(1))
                   .enabled());

      // Owned by the reader so a refresh still running after this block cannot touch freed memory.
      struct Store {
        std::mutex mutex;
        std::condition_variable changed;
        bool locked = false;
        int value = 1;
        int reads = 0;
      };
      auto store = std::make_shared<Store>();
      const TypingStatisticsSwitch stale(
          [store] {
            std::unique_lock lock(store->mutex);
            ++store->reads;
            store->changed.notify_all();
            store->changed.wait(lock, [&] { return !store->locked; });
            return store->value;
          },
          std::chrono::steady_clock::duration::zero());
      {
        std::lock_guard lock(store->mutex);
        store->locked = true;
        store->value = 0;
      }
      // The store is held by a writer: the stale answer comes back at once, and only one refresh waits on it.
      require(stale.enabled());
      {
        std::unique_lock lock(store->mutex);
        require(store->changed.wait_for(lock, std::chrono::seconds(5),
                                        [&] { return store->reads == 2; }));
      }
      require(stale.enabled());
      {
        std::lock_guard lock(store->mutex);
        require(store->reads == 2);
        store->locked = false;
      }
      store->changed.notify_all();
      const auto deadline = std::chrono::steady_clock::now() + std::chrono::seconds(5);
      bool switched_off = false;
      while (!switched_off && std::chrono::steady_clock::now() < deadline) {
        switched_off = !stale.enabled();
        if (!switched_off)
          std::this_thread::sleep_for(std::chrono::milliseconds(5));
      }
      require(switched_off);
    }
    std::cout << "Windows typing statistics checks passed\n";
    return 0;
  } catch (const std::exception &error) {
    std::cerr << error.what() << '\n';
    return 1;
  }
}
