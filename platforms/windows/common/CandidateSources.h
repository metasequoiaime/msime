#pragma once

#include <cstddef>
#include <optional>

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

// 占位席位：引擎列表里占一格，但不是候选，选不中。
inline constexpr bool candidate_source_is_pending_seat(unsigned source) {
  return source == candidate_source_ai_pending || source == candidate_source_cloud_pending;
}

// 这一行该不该出现在候选窗上。云的占位席位是引擎内部用来固定云候选位置的（`CandidateSource::CloudPending`，文字是「☁」），宿主不显示它，数字编号也不数它——否则用户会看到一个永远选不中的格子，它后面每一行的编号还都偏一位。移动三端早就这么做了（`CandidatePresentationPolicy.isVisible`），这里是桌面端的同一条规则。AI 占位行（12）照旧显示，只是画成次要色。
inline constexpr bool candidate_source_is_visible(unsigned source) {
  return source != candidate_source_cloud_pending;
}

// 当前页里第 slot 个看得见的候选（slot 从 0 数起），看得见的行不够这么多时是 nullptr。按位置选词的每个入口都要走它：数字键、二三候选键（`;` 和 `'`）。隐藏的占位席位在引擎的这一页里仍占一格，不跳过它，它后面每一行的位置都比用户看到的大一位——按第二个选到的是第三行。
// 两个入口原来各写一遍这个循环，于是漂移过一次：#9273 只改了数字键那一半，`;` 和 `'` 继续按原始下标选，隐藏席位在第二格时就选到了那个选不中的格子。所以这一份放在这里，两个入口共用。
// 按页的类型取模板，这个头就不必引入 nlohmann：pipe-only 配置（`MSIME_WINDOWS_PIPE_ONLY`）故意不带任何 vcpkg 依赖，而 `windows-candidate-menu` 这些目标包含本文件，引入 json 会让那套配置编不过。
template <typename Page>
auto visible_page_candidate(const Page &page, std::size_t slot) -> const typename Page::value_type * {
  std::size_t seen = 0;
  for (const auto &candidate : page) {
    if (!candidate_source_is_visible(candidate.value("source", 0u)))
      continue;
    if (seen == slot)
      return &candidate;
    ++seen;
  }
  return nullptr;
}


// 高亮所在的那一行：引擎下标（整个列表里的，不是页内的）和它在候选窗上看不看得见。
struct HighlightedSeat {
  std::size_t index = 0;
  bool visible = true;
};

// 方向键、横排的左右键和 End 在引擎里按原始下标移动高亮，隐藏的云占位席位也算一格（它在第二格，按一次下就落上去）。候选窗不画它，高亮停在那里时候选窗上一个高亮行也没有：Server 的 `candidate_presentation` 拒绝这种视图，而它在输入队列上抛出会让整个 Server 停下，表现是候选框消失、之后再也打不出中文（#10665）。所以高亮落到隐藏行上时，朝刚才移动的方向再走一步把它让过去；走到列表末尾走不动（席位是最后一行）时掉头走回来。`before` 是移动前高亮行的引擎下标，比它小说明是在往回走；`bound` 限定最多走几步，传这一页的行数即可。`highlighted()` 读会话眼下的高亮行，没有候选时为空；`step(forward)` 在会话里发一条 MSIME_NEXT_CANDIDATE（forward 为 true）或 MSIME_PREVIOUS_CANDIDATE，失败返回 false，这时本函数也返回 false。
// Server 和 TIP 的宿主会话各有一份引擎会话，两边都要按这同一条规则走，放在这里共用。模板化同样是为了不把 nlohmann 带进这个头（见下面 visible_page_candidate 的说明）。
template <typename Highlighted, typename Step>
bool leave_hidden_seat(std::optional<std::size_t> before, std::size_t bound, Highlighted highlighted, Step step) {
  std::optional<HighlightedSeat> seat = highlighted();
  if (!seat || seat->visible)
    return true;
  bool forward = !(before && seat->index < *before);
  bool turned = false;
  for (std::size_t attempts = bound + 1; attempts && seat && !seat->visible; --attempts) {
    const auto at = seat->index;
    if (!step(forward))
      return false;
    seat = highlighted();
    if (seat && !seat->visible && seat->index == at) {
      // 只有隐藏行可停（整页只剩占位）时两头都走不动，不再来回试。
      if (turned)
        break;
      turned = true;
      forward = !forward;
    }
  }
  return true;
}

} // namespace msime::windows
