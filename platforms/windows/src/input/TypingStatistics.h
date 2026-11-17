#pragma once
#include "../../common/TypingSource.h"
#include <atomic>
#include <chrono>
#include <cstdint>
#include <ctime>
#include <functional>
#include <map>
#include <memory>
#include <nlohmann/json.hpp>
#include <optional>
#include <string>
#include <string_view>
#include <system_error>
#include <thread>
#include <utility>

namespace msime::windows {
// Attribute a commit to the mode that produced it, not to the mode left behind.
// Committing can clear a local mode, so `commit_context` - the Engine's own
// record of the scheme and local mode in force when the key was dispatched - is
// authoritative for those two fields; an Emoji-mode commit read off the
// post-commit view would otherwise be counted as quanpin. The remaining fields
// have no pre-dispatch counterpart and come from the view.
inline TypingSource
resolve_typing_source_from_transition(const nlohmann::json &transition) {
  static const nlohmann::json empty = nlohmann::json::object();
  // By reference throughout: this runs on the input queue, and value() would
  // deep-copy the candidate list hanging off the view on every commit.
  const auto view_field = transition.find("view");
  const nlohmann::json &view =
      view_field != transition.end() && view_field->is_object() ? *view_field
                                                                : empty;
  const auto context_field = transition.find("commit_context");
  const nlohmann::json &context =
      context_field != transition.end() && context_field->is_object()
          ? *context_field
          : view;
  return resolve_typing_source(
      context.value("scheme", -1), view.value("nine_key", false),
      view.value("dedicated_english", false),
      context.value("local_mode", std::string("none")),
      view.value("shuangpin_profile", std::string("xiaohe")));
}

// Whether a commit counts as typing. The Engine marks the text its V, "/" and "@" modes generate (a result, an expanded command, a name from the list) as not typed, so it reaches neither the statistics nor their milestones. Without the field every commit counts, as every commit did before it existed.
inline bool transition_counts_as_typing(const nlohmann::json &transition) {
  const auto context = transition.find("commit_context");
  if (context == transition.end() || !context->is_object())
    return true;
  const auto flag = context->find("typing_statistics");
  return flag == context->end() || !flag->is_boolean() || flag->get<bool>();
}

// Resolved local calendar fields, or nothing when the conversion failed. The
// day and hour axes both have to be the user's, and only this process knows
// which timezone that is; a failure means the caller skips the record rather
// than attributing it to a guessed day.
struct LocalTimeParts {
  std::string day; // YYYY-MM-DD
  int hour = 0;    // 0-23
};

inline std::optional<LocalTimeParts> local_time_parts(std::time_t instant) {
  std::tm local{};
#ifdef _WIN32
  if (localtime_s(&local, &instant) != 0)
    return std::nullopt;
#else
  if (localtime_r(&instant, &local) == nullptr)
    return std::nullopt;
#endif
  char day[11]{};
  if (std::strftime(day, sizeof(day), "%Y-%m-%d", &local) == 0)
    return std::nullopt;
  return LocalTimeParts{std::string(day), local.tm_hour};
}

// Build the shared host request for one commit. Empty text, an empty directory,
// or a day or hour that did not resolve all yield an empty string: statistics
// are best effort and must never manufacture a record they cannot place.
// Whether the directory is absolute is not re-decided here - the shared host
// owns that rule and rejects the request - because a second copy would drift.
// `quiet` keeps the achievement jingle a milestone would play silent, as every other effect sound is while a full-screen application is in front; the commit is still counted.
inline std::string typing_statistics_record_request(std::string_view directory,
                                                    const std::string &text,
                                                    TypingSource source,
                                                    const std::string &day,
                                                    int hour,
                                                    bool quiet = false) {
  if (text.empty() || directory.empty() || day.size() != 10 || hour < 0 ||
      hour > 23)
    return {};
  auto action = nlohmann::json{{"operation", "record"},
                               {"text", text},
                               {"source", std::string(typing_source_id(source))},
                               {"day", day},
                               {"hour", hour}};
  if (quiet)
    action["quiet"] = true;
  const auto request =
      nlohmann::json{{"directory", std::string(directory)},
                     {"action", std::move(action)}}
          .dump();
  // The shared entry point rejects buffers past 64 KiB. A single commit never
  // approaches that; a pathological paste is dropped instead of truncated,
  // because a truncated commit would be counted as a shorter one.
  if (request.size() > 65'536)
    return {};
  return request;
}

// Build the shared host request for one day's per-key press counts. The day is the one the presses were counted on, never the flush time. Nothing to record, an empty directory or a malformed day yields an empty string; which ids are canonical is the shared store's rule (KEY_IDS), and it rejects the whole batch for one it does not know.
inline std::string
typing_statistics_record_keys_request(std::string_view directory,
                                      const std::string &day,
                                      const std::map<std::string, uint64_t> &keys,
                                      std::string_view source = {}) {
  if (keys.empty() || directory.empty() || day.size() != 10)
    return {};
  nlohmann::json action{{"operation", "record_keys"}, {"day", day}, {"keys", keys}};
  if (!source.empty()) action["sources"] = nlohmann::json{{std::string(source), keys}};
  return nlohmann::json{{"directory", std::string(directory)}, {"action", std::move(action)}}.dump();
}

// The statistics switch as the Aux listener sees it. The listener must answer every statistics message within the DLL's 150 ms window, but the store keeps its enabled flag behind the same exclusive file lock a detached record holds for a whole read, validate, serialize and fsync, so asking the store from the listener thread makes the second message of a multi-part batch wait on the first one's write. The listener answers from this cached copy instead; a copy older than `lifetime` is refreshed on a detached thread while the stale answer is returned. A stale "on" records nothing, because the store checks the switch itself on every write; a stale "off" only delays counting until the DLL's next probe.
class TypingStatisticsSwitch {
public:
  // `read` follows msime_client_typing_statistics_enabled: 1 is on, 0 is off and anything else is an unreadable store, which counts as off. It is called once here, on the constructing thread, so the first answer is a real one.
  TypingStatisticsSwitch(std::function<int()> read,
                         std::chrono::steady_clock::duration lifetime)
      : state_(std::make_shared<State>(std::move(read), lifetime)) {
    state_->refresh();
  }

  // Never touches the store on the calling thread.
  bool enabled() const {
    const auto now = std::chrono::steady_clock::now().time_since_epoch();
    const auto refreshed = std::chrono::steady_clock::duration(
        state_->refreshed_at.load(std::memory_order_acquire));
    if (now - refreshed >= state_->lifetime &&
        !state_->refreshing.exchange(true, std::memory_order_acq_rel)) {
      try {
        // The thread owns a share of the state, so a refresh still running when the switch is destroyed writes into memory that is still alive.
        std::thread([state = state_] {
          state->refresh();
          state->refreshing.store(false, std::memory_order_release);
        }).detach();
      } catch (const std::system_error &) {
        // Thread exhaustion keeps the cached answer; the next call tries again.
        state_->refreshing.store(false, std::memory_order_release);
      }
    }
    return state_->enabled.load(std::memory_order_acquire);
  }

private:
  struct State {
    State(std::function<int()> reader,
          std::chrono::steady_clock::duration age)
        : read(std::move(reader)), lifetime(age) {}
    void refresh() {
      enabled.store(read() == 1, std::memory_order_release);
      refreshed_at.store(
          std::chrono::steady_clock::now().time_since_epoch().count(),
          std::memory_order_release);
    }
    const std::function<int()> read;
    const std::chrono::steady_clock::duration lifetime;
    std::atomic<bool> enabled{false};
    std::atomic<bool> refreshing{false};
    std::atomic<std::chrono::steady_clock::rep> refreshed_at{0};
  };
  std::shared_ptr<State> state_;
};
} // namespace msime::windows
