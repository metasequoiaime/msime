#pragma once
#include "CandidateAction.h"
#include "NavigationPolicy.h"
#include "TypingEffectPolicy.h"
#include "WordCharacterPolicy.h"
#include "windows_ipc.h"
#include <nlohmann/json.hpp>
#include <string>
#include <thread>

namespace msime::windows {
struct KeyResult {
  uint64_t client_id;
  uint64_t activation_epoch;
  uint64_t request_id;
  bool reply_expected;
  nlohmann::json transition;
};
struct NavigationResult {
  KeyResult key;
  NavigationReply direction;
};
struct WordCharacterResult {
  KeyResult key;
  bool exact;
};

// Lives on the Server input queue, never inside the injected TSF DLL. The pipe
// router authenticates/negotiates a client before constructing its session, and
// assigns monotonically increasing focus epochs. This class is not a pipe
// server.
class ServerSession final {
public:
  ServerSession(uint64_t client_id, const std::string &prepared_options);
  ~ServerSession();
  ServerSession(const ServerSession &) = delete;
  ServerSession &operator=(const ServerSession &) = delete;
  nlohmann::json activate(uint64_t epoch);
  nlohmann::json deactivate(uint64_t epoch);
  void cancel_composition(uint64_t epoch);
  // MSIME_FINISH_COMPOSITION: the open composition becomes the commit. Korean uses it for the keys that end a syllable without a character of their own.
  nlohmann::json finish_composition(uint64_t epoch);
  // One MsimeCommand, for a key whose command does not follow from translate_key: the Korean Hanja list's keys (KoreanHanjaKey.h). Requires input enabled.
  nlohmann::json command(uint64_t epoch, uint32_t command);
  // Clear the Engine candidate-provider cache without requiring focus.
  void reset_cache();
  void set_input_enabled(uint64_t epoch, bool enabled);
  void set_chinese_punctuation(uint64_t epoch, bool enabled);
  // The TSF inserted the closing half of a pair whose opening the Engine resolved, so pay back the nesting the opening advanced. A no-op when the count is already zero.
  void balance_paired_punctuation(uint64_t epoch, uint8_t opening);
  // Toggle the host-side simplified/traditional output projection without
  // changing Engine composition. Persistence is owned by the caller.
  nlohmann::json toggle_traditional_output(uint64_t epoch);
  // Queue-owned runtime operation, never a write to default preferences.
  // Exit cancels composition without committing and returns the Engine view.
  nlohmann::json dedicated_english(uint64_t epoch, bool exit);
  // Ctrl+Shift+E: flip the dedicated English mode. Like the reference, the open composition is discarded rather than committed. Returns the new view.
  nlohmann::json toggle_dedicated_english(uint64_t epoch);
  bool input_enabled() const {
    check_thread();
    return input_enabled_;
  }
  KeyResult key(const FanyImeNamedpipeData &packet, uint64_t epoch);
  // Rebuild a deleted creating-word segment through Engine's ordinary
  // character path. This is the Windows Ctrl+Backspace restoration boundary;
  // the host composer owns the selected-prefix history.
  KeyResult restore_raw(uint64_t epoch, uint64_t request,
                        const std::string &raw);
  KeyResult punctuation(const FanyImeNamedpipeData &packet, uint64_t epoch);
  std::optional<WordCharacterResult>
  word_character(const FanyImeNamedpipeData &packet, uint64_t epoch,
                 WordCharacterBinding binding);
  std::optional<NavigationResult> navigate(const FanyImeNamedpipeData &packet,
                                           uint64_t epoch,
                                           const NavigationBindings &bindings);
  nlohmann::json select(uint64_t epoch, uint64_t generation, size_t index);
  nlohmann::json candidate_action(uint64_t epoch, uint64_t generation,
                                  size_t index, CandidateAction action,
                                  uint8_t position = 0);
  std::optional<std::string> online_query(uint64_t epoch);
  std::optional<std::string> ai_request(uint64_t epoch,
                                        const std::string &query);
  /// Re-rank with the settled model; the refreshed view, or nothing when the
  /// order did not move or no settled model is installed.
  std::optional<nlohmann::json> rerank_settled(uint64_t epoch);
  std::optional<nlohmann::json>
  apply_cloud_response(uint64_t epoch, const std::string &query,
                       const std::string &body);
  std::optional<nlohmann::json> apply_ai_candidates(uint64_t epoch,
                                                    const std::string &query,
                                                    const std::string &candidates);
  std::optional<std::string> translation_query(uint64_t epoch);
  std::optional<nlohmann::json>
  apply_translations(uint64_t epoch, uint64_t generation,
                     const std::string &translations);
  nlohmann::json update_preferences(uint64_t epoch,
                                    const std::string &snapshot);
  bool traditional_output() const {
    check_thread();
    return traditional_output_;
  }
  nlohmann::json page_candidate(uint64_t epoch, uint64_t session,
                                uint64_t generation, bool previous,
                                unsigned steps);
  nlohmann::json view() const;
  // Effect sounds, played by the shared library from this session's preferences. Each is a bounded queue post that never blocks and answers whether a sound was queued. Only the Server calls them: the TSF DLL links the same library into every process it is loaded into, and never starts its player.
  bool key_sound(uint32_t key_class);
  bool commit_sound();
  // The typing effect of one key or commit (msime_client_typing_effect): the packed combo count, tier-up bit and effect style the candidate window draws, 0 when effects and the combo counter are both off.
  uint32_t typing_effect(uint32_t event);
  // The resolved typing effect (msime_client_typing_effect_settings) as Windows draws it: read when the session gains the focus and after each preference update, never per key, so the packed word the key path publishes is ready.
  const TypingEffectSettings &typing_effect_settings() const {
    check_thread();
    return typing_effect_settings_;
  }
  // Whether background music may play: true while this client holds the focus. Remembered, so a preference update can repeat it and destroying the session stops music it started.
  void set_music_active(bool active);

private:
  void check_thread() const;
  void check_active(uint64_t epoch) const;
  const std::thread::id thread_ = std::this_thread::get_id();
  uint64_t client_;
  uint64_t session_ = 0;
  uint64_t epoch_ = 0;
  bool active_ = false;
  bool input_enabled_ = true;
  bool traditional_output_ = false;
  bool music_active_ = false;
  TypingEffectSettings typing_effect_settings_{};
  void refresh_typing_effect_settings();
};
} // namespace msime::windows
