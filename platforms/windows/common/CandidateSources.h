#pragma once

// 引擎 `CandidateSource` 枚举（`crates/engine/src/types.rs`）的取值。它是 Rust 的枚举，经共享 Host API 以数字到达这一侧，C++ 没法直接引用，所以这里按位置抄一份；`scripts/test-candidate-sources.py` 把下面每个名字钉在枚举里的位置上——在枚举中间插入一个来源会让它之后的全部位移，没有这道门禁就会悄悄把「删除」递给云候选。
//
// Server（候选窗、右键动作）和 TIP（TSF 里的选词）都要认这些值，所以放在 `common/` 而不是某一侧自己的目录里：两份各自定义的常量迟早会漂移，而漂移的后果是按错行。
namespace msime::windows {

inline constexpr unsigned candidate_source_database = 0;
inline constexpr unsigned candidate_source_user_database = 1;
inline constexpr unsigned candidate_source_cloud_suggestion = 2;
inline constexpr unsigned candidate_source_ai_suggestion = 3;
inline constexpr unsigned candidate_source_english_dictionary = 4;
// AI 占位行，不是候选，不给任何右键动作（#6888）。共享内核已经不再产出这种行（AI 回答到达时直接插在第一页最后一格），这里的处理留着，旧的库或数据里出现时仍按占位行对待。
inline constexpr unsigned candidate_source_ai_pending = 12;
// 云请求的占位行，文字是「☁」。引擎在等云端回答时先占住这个席位，回答到达而没有新词时还会留着它，免得候选位置来回跳（`crates/engine/src/ime/online_batch.rs`）。它不是候选：按它的数字键或点它，引擎「什么都不做但算处理过」（`crates/engine/src/session/commit.rs`，#6888），宿主也必须如此，不能把它当成一次失败的选择去丢掉组字。
inline constexpr unsigned candidate_source_cloud_pending = 15;

// 占位席位：占着一格和一个编号，但选不中。
inline constexpr bool candidate_source_is_pending_seat(unsigned source) {
  return source == candidate_source_ai_pending || source == candidate_source_cloud_pending;
}

} // namespace msime::windows
