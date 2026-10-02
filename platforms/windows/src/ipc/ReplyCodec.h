#pragma once
#include "windows_ipc.h"
#include "InputSchemeTraits.h"
#include "../../../shared/contracts/voice_composition_pipe.h"
#include <array>
#include <optional>
#include <string>
#include <string_view>
#include <vector>

namespace msime::windows {
enum class ReplyError {
  None,
  InvalidRequest,
  InvalidUtf8,
  EmbeddedNul,
  TooLong,
  InvalidFields
};
struct EncodedReply {
  ReplyError error = ReplyError::None;
  FanyImeNamedpipeDataToTsf packet{};
  explicit operator bool() const { return error == ReplyError::None; }
};
// These are wire encoders, not decisions about which reply a TSF key path
// reads. The sender must revalidate client/activation ownership before writing
// a frame.
EncodedReply candidate_commit(uint64_t request, std::string_view complete_text);
EncodedReply exact_commit(uint64_t request, std::string_view complete_text);
EncodedReply preedit_reply(uint64_t request, std::string_view display_text);
EncodedReply ignored_reply(uint64_t request);
enum class NavigationReply {
  Ignored,
  PreviousCandidate,
  NextCandidate,
  PreviousPage,
  NextPage
};
// Navigation replies carry intent, even when already at a page/list boundary.
// They are never candidate text and do not commit composition.
EncodedReply navigation_reply(uint64_t request, NavigationReply navigation);
// Serialize members explicitly as little-endian bytes; never send struct
// padding.
using ReplyBytes = std::array<uint8_t, sizeof(FanyImeNamedpipeDataToTsf)>;
std::optional<ReplyBytes> wire_bytes(const EncodedReply &reply);
// Registration-only frames. These must never enter the candidate reply queue.
std::optional<std::vector<uint8_t>> pipe_ready_bytes(uint32_t role);
// Worker focus fence echoes the TSF activation request token, NOT the Server
// epoch. Caller must check current client/activation/transport ownership and
// order this before subsequent worker output. Encoding is not authorization.
std::optional<std::vector<uint8_t>> focus_ready_bytes(uint64_t client_id,
                                                      uint64_t activation_epoch,
                                                      uint64_t focus_token);
enum class WorkerMode {
  English,
  Chinese,
  AsciiPunctuation,
  ChinesePunctuation,
  Fullwidth,
  Halfwidth
};
// Only six mode commands, never arbitrary opcodes or unsolicited text.
std::optional<std::vector<uint8_t>> worker_mode_bytes(WorkerMode mode);
// Commit text through the worker endpoint and trim only the named count from
// the TSF-local key buffer, preserving any keys typed ahead of delivery.
std::optional<std::vector<uint8_t>>
commit_candidate_and_continue_bytes(size_t consumed, std::string_view text);

// The TSF-local settings the TIP keeps in its own globals.
//
// The TIP consumes all of these, but the Server never encoded them, so they sat
// at their compiled defaults forever: turning smart or paired punctuation off
// did nothing, the Microsoft shuangpin ';' key was never enabled, and the
// inline preedit style stayed "raw" whatever the user chose.
struct TsfLocalConfig {
  bool paging_comma_period = false;
  // "raw" | "pinyin" | "empty" - rides along with the paging frame.
  std::string preedit_style = "raw";
  bool smart_punctuation = false;
  bool smart_punctuation_repeat_to_chinese = false;
  bool smart_punctuation_space_convert = false;
  // Fine-grained direct ASCII punctuation policy, carried by the punctuation-lock frame.
  bool smart_punctuation_direct_digit = false;
  bool smart_punctuation_direct_letter = false;
  bool paired_punctuation = true;
  bool microsoft_shuangpin = false;
  // The configured scheme's family, sent as the InputModeChanged code (common/InputSchemeTraits.h).
  scheme::InputMode input_mode = scheme::InputMode::Chinese;
  bool tsf_diagnostic_log = false;
  // 0 follow, 1 always Chinese, 2 always English.
  uint8_t punctuation_lock = 0;
  // The local modes whose keys the TIP must route as composition input: Shift+V's digits and operators, and the "/" or "@" that opens a mode on an empty composition. Each is on only in a pinyin scheme, the only schemes the Engine opens them in.
  bool expression_mode = false;
  bool command_mode = false;
  bool mention_mode = false;
  // The focused client's Engine is in its own English mode, where a capital is composed English and V is a letter rather than the expression mode. Not a preference: the Server follows the focused session and sends the V, "/" and "@" flags off while it holds, so the TIP keeps digits selecting and '-'/'.' paging in an English word that starts with V.
  bool dedicated_english = false;
};
// One frame per setting, in the order the reference pushes them.
std::vector<std::vector<uint8_t>> tsf_config_frames(const TsfLocalConfig &config);
// Caps Lock changes on its own cadence, so it gets its own frame rather than
// resending the whole configuration on every press.
std::vector<uint8_t> caps_lock_frame(bool enabled);
// Encode a voice composition snapshot for the TSF worker pipe. The shared
// contract clips it to its snapshot cap before splitting it into ordered,
// fixed-size worker packets. Generation is a voice session generation and
// must be non-zero.
std::optional<std::vector<std::vector<uint8_t>>>
voice_composition_bytes(uint32_t message, std::wstring_view text,
                         wchar_t generation);
// UI selection is not a key reply: complete text travels only on the worker
// endpoint. Partial/out-of-range replies use id 0 BEFORE an empty worker
// trigger. Caller must own a pending selection and validate focus for the
// entire send.
struct UiSelectionFrames {
  std::optional<ReplyBytes> before_trigger;
  std::vector<uint8_t> worker;
};
std::optional<UiSelectionFrames> ui_complete_selection(std::string_view text);
std::optional<UiSelectionFrames>
ui_partial_selection(std::string_view raw, std::string_view selected_prefix,
                     std::string_view display);
UiSelectionFrames ui_rejected_selection();
std::optional<ReplyBytes>
protocol_reply_bytes(const FanyImeNamedpipeDataToTsf &packet);
// Existing DLL expects remaining raw input, the ENTIRE selected prefix and the
// display preedit. Do not pass only the latest incremental Engine commit here.
EncodedReply partial_selection(uint64_t request, std::string_view remaining_raw,
                               std::string_view selected_prefix,
                               std::string_view display_preedit);
EncodedReply uiless_reply(uint64_t request, std::string_view display_preedit,
                          const std::vector<std::string> &page,
                          size_t highlighted);
} // namespace msime::windows
