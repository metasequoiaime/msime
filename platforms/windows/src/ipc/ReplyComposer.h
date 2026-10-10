#pragma once
#include "ReplyCodec.h"
#include "EditPolicy.h"
#include "JapaneseSpacePolicy.h"
#include "ServerSession.h"
#include <functional>
#include <optional>
#include <vector>

namespace msime::windows {
// Supplied by the TSF-compatible dispatch path, not inferred from a VK alone:
// e.g. a digit in Unicode mode is composition, not candidate selection.
enum class ReplyPath {
  Composition,
  AutoCommitAndContinue,
  // A Korean key that finished a syllable. The TIP inserts that syllable from its own host session, so the reply carries at most the composition that follows it.
  SyllableCommit,
  // 整句改字时的回车：TIP 从自己的宿主会话上屏改好的整句（MSIME_COMMIT_RAW），这边的会话做同样的事，不发回复帧。
  ConversionCommit,
  Selection,
  Punctuation,
  CandidatePunctuationFallback,
  LocalCommit,
  LocalCancel,
  NoReply,
  PreviousCandidate,
  NextCandidate,
  PreviousPage,
  NextPage,
  IgnoredNavigation
};
// How many letters the TIP drops from its own buffer when a Wubi commit takes AutoCommitAndContinue: the code the Engine held before the key, plus the key itself when the Engine holds nothing afterwards. The fourth letter of a unique code gives 3 + 1, a lowercase letter after a complete code (顶字) gives 4 and stays composing, and a capital the Engine does not take after a complete code goes out with the first candidate, giving 4 + 1. A letter typed ahead into the TIP buffer is never counted, so it survives every case.
inline std::size_t wubi_continue_consumed(std::size_t code_before, bool composing_after) {
  return code_before + (composing_after ? 0 : 1);
}
struct PendingReply {
  KeyResult source;
  std::optional<EncodedReply> encoded;
  std::string next_prefix;
  std::optional<UiSelectionFrames> ui_selection = std::nullopt;
  // Unsolicited worker delivery for a commit produced by a character key.
  std::optional<std::vector<uint8_t>> worker = std::nullopt;
  // A copied, bounded query for the optional asynchronous cloud provider.
  // It is submitted only after this reply has been delivered and confirmed.
  std::optional<std::string> online_query = std::nullopt;
  // A host-owned AI HTTP descriptor for the copied query. Credentials are
  // resolved inside ServerSession and consumed only by the native worker.
  std::optional<std::string> ai_request = std::nullopt;
  // A copied, bounded candidate-translation query, submitted after delivery.
  std::optional<std::string> translation_query = std::nullopt;
  bool traditional_output = false;
  // The exact text this reply puts into the document, set only on replies that
  // complete a commit. Partial selections leave it empty even though they
  // advance the prefix: that text is not in the document yet and is committed
  // in full by the later reply that clears the prefix, so counting both would
  // count it twice. Read by the owner after delivery is confirmed, which is
  // what makes "committed" mean "arrived" rather than "was composed".
  std::optional<std::string> committed_text = std::nullopt;
  struct SegmentRestore {
    std::string raw;
    std::string previous_prefix;
  };
  std::optional<SegmentRestore> segment_restore;
  bool restoring_segment = false;
};
// One instance per authenticated client activation, on the Server input queue.
// prefix is transport presentation state: text already selected by Engine but
// not yet committed by the legacy DLL. It never selects or edits Engine input.
class ReplyComposer final {
public:
  ReplyComposer(uint64_t client, uint64_t epoch);
  const PendingReply &
  stage(const KeyResult &result, ReplyPath path, bool uiless = false,
        std::optional<std::string> local_text = std::nullopt,
        std::size_t continue_consumed = 4);
  // Normal input entry: enforce the pending-reply gate BEFORE advancing Engine.
  // LocalCommit also requires an Enter/raw-commit key and caller-observed text
  // equal to selected_prefix + Engine editing_text BEFORE clearing composition.
  const PendingReply &
  dispatch(ServerSession &session, const FanyImeNamedpipeData &packet,
           uint64_t epoch, ReplyPath path, bool uiless = false,
           std::optional<std::string> local_text = std::nullopt);
  const PendingReply &pending() const;
  // Resolve special-profile and other native-only shortcuts first.
  std::optional<PendingReply> configured_key(
      ServerSession &session, const FanyImeNamedpipeData &packet,
      uint64_t epoch, TsfPreeditStyle style, const NavigationBindings &bindings,
      std::optional<std::string> local_text = std::nullopt,
      WordCharacterBinding word_binding = WordCharacterBinding::Disabled);
  // Ctrl+Shift+F changes only the host output projection. It still creates a
  // pending no-frame reply so SessionPump preserves the normal confirmation
  // and ordering gate.
  std::optional<PendingReply>
  toggle_character_set(ServerSession &session,
                       const FanyImeNamedpipeData &packet, uint64_t epoch,
                       bool enabled,
                       const std::function<bool(bool)> &persist);
  // Native configuration-specific priority routes must run first. Null leaves
  // Engine untouched and means this key needs another native route.
  std::optional<PendingReply> basic_key(ServerSession &session,
      const FanyImeNamedpipeData &packet, uint64_t epoch, TsfPreeditStyle style,
      std::optional<std::string> local_text = std::nullopt);
  // Ctrl+Enter commits the highlighted candidate's already-resolved
  // translation when one is present. The Engine selection still advances
  // normally; only the host projection uses the gloss text.
  std::optional<PendingReply> commit_candidate_translation(
      ServerSession &session, const FanyImeNamedpipeData &packet,
      uint64_t epoch);
  std::optional<PendingReply> translation_page_key(
      ServerSession &session, const FanyImeNamedpipeData &packet,
      uint64_t epoch);
  // 释义列快捷键（GlossColumnPolicy.h）：Alt/Ctrl+数字上屏对应候选的第 1/2 列释义，Tab/Shift+Tab 预选高亮候选的释义列，预选之后数字和空格上屏那一列。armed 是这次按键之前预选的列。不是这些键、或者那一列没有释义时返回空，按键照常处理。
  std::optional<PendingReply> gloss_column_key(
      ServerSession &session, const FanyImeNamedpipeData &packet,
      uint64_t epoch, int armed);
  // 眼下预选的释义列，0 表示没有预选。只活到下一个按键：Server 收到的任何按键都会先清掉它，只有预选列的 Tab 再设回去。
  int armed_gloss_column() const { return armed_gloss_column_; }
  PendingReply translation_page_reply(const FanyImeNamedpipeData &packet,
                                     uint64_t epoch,
                                     const nlohmann::json &view);
  std::optional<PendingReply> restore_segment(
      ServerSession &session, const FanyImeNamedpipeData &packet,
      uint64_t epoch);
  // The key that opens a scheme's candidate list while it composes (the Korean Hanja key, Zhuyin's Down), and the keys an open Hanja or Zhuyin list takes (KoreanHanjaKey.h), applied to the Server's session exactly as the TIP applies them to its host session. Nothing is sent back: the TIP writes what its own session chose, and the candidate window follows the delivered view. Null for every other key, scheme and state, which keep their routes.
  std::optional<PendingReply> korean_hanja(
      ServerSession &session, const FanyImeNamedpipeData &packet,
      uint64_t epoch);
  // 韩文、注音、越南文或藏文的光标或编辑键（回车、Tab、方向键、Home/End、Page Up/Down、Delete），因前面的键还在排队而被 TIP 吃掉，或是 TIP 据以上屏的键，会结束当前组字。其他按键和方案返回空。
  std::optional<PendingReply> korean_syllable_end(
      ServerSession &session, const FanyImeNamedpipeData &packet,
      uint64_t epoch);
  // 日语方案组字时的空格（japanese_space_applies 已经判过）：开始转换时回一个不上屏的导航回执，之后每一次在会话里移到下一个候选、过了末尾回到第一个，同样回导航回执，候选窗跟着送达的 view 走。状态机不接管时返回空，空格照常上屏高亮候选。
  std::optional<PendingReply> japanese_space(ServerSession &session,
                                             const FanyImeNamedpipeData &packet,
                                             uint64_t epoch,
                                             const nlohmann::json &view);
  // 组字时 ';' 或 '\'' 选当前页第二、第三个候选，走和数字键相同的选词回复。规则与 TIP 共用 SecondThirdCandidatePolicy.h；不归它管的键返回空，Engine 不动。
  std::optional<PendingReply> second_third_candidate(
      ServerSession &session, const FanyImeNamedpipeData &packet,
      uint64_t epoch);
  // Null: not an editing key; no Engine action. Non-null may have no frame
  // because TSF completed this edit locally; still confirm it through the pump.
  std::optional<PendingReply> edit(ServerSession &session,
      const FanyImeNamedpipeData &packet, uint64_t epoch, TsfPreeditStyle style);
  // Disabled navigation returns an ignored reply (UILess: unchanged page).
  // Null means this is not a navigation path; Engine is unchanged.
  // Caller may then run its ordinary TSF path. UiLess is read from the packet.
  std::optional<PendingReply> navigate(ServerSession &session,
                                       const FanyImeNamedpipeData &packet,
                                       uint64_t epoch,
                                       const NavigationBindings &bindings);
  bool has_pending() const { return pending_.has_value(); }
  // Stale/busy clicks are rejected without advancing Engine. UI receipts use
  // the resulting view generation, not the id-zero wire request identifier.
  std::optional<PendingReply> select_candidate(ServerSession &session,
      uint64_t expected_session, uint64_t generation, size_t index);
  void confirm_ui_delivery(uint64_t client, uint64_t epoch, uint64_t generation);
  // Call only after a complete frame write or successful local-only handling.
  // A failed/uncertain write leaves pending unchanged; never rerun Engine
  // input.
  void confirm_delivery(uint64_t client, uint64_t epoch, uint64_t request);
  // Explicit focus/transport cancellation; never an implicit error fallback.
  void cancel();
  const std::string &selected_prefix() const { return prefix_; }

private:
  uint64_t client_;
  uint64_t epoch_;
  uint64_t session_ = 0;
  std::string prefix_;
  std::optional<PendingReply> pending_;
  bool traditional_output_ = false;
  bool translation_page_active_ = false;
  int armed_gloss_column_ = 0;
  std::vector<std::string> translation_page_items_;
  nlohmann::json translation_page_view_;
  std::vector<PendingReply::SegmentRestore> segment_restore_history_;
  // 日语空格「変換」进行到哪一个候选、对着哪一段读音。组字结束（回复里 editing_text 为空）或取消时清掉，下一段组字从头开始。
  input::JapaneseConversion japanese_conversion_;
};
} // namespace msime::windows
