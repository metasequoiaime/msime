#pragma once

#include <cstddef>
#include <cstdint>

// 引擎 `CandidateSource` 枚举（crates/engine/src/types.rs）里宿主需要单独对待的取值。它是 Rust 的枚举，经 Host API 以数字到达这一侧；Windows 的 `common/CandidateSources.h` 和 iOS、Android 的 `CandidatePresentationPolicy` 是同一组规则的另外几份，改这里时一起看。
//
// 云的占位席位（`CandidateSource::CloudPending`，15，文字是「☁」）是引擎给云候选留的固定位置：回答到达时在原位替换，本地候选不整体后移。它选不中，所以不能出现在候选窗上，也不能吃掉一个序号——否则它后面每一行的编号都比看到的大一位，按 3 上屏的是第 4 行。Windows（#9273）、macOS 和移动三端都跳过它；Linux 的 IBus 与 Fcitx5 到 0.17.0 都照原样画了出来，用户看到一个选不中的格子、按它的数字什么也不发生（#11850）。
namespace msime::linux_host {

inline constexpr std::uint64_t kCandidateSourceCloudPending = 15;

// 这一行该不该出现在候选窗上，以及要不要算进序号。目前只有云的占位席位不出现；旧的 AI 占位行（12）照旧画成次要色，共享内核已经不再产出它。
inline constexpr bool candidate_source_is_visible(std::uint64_t source) {
  return source != kCandidateSourceCloudPending;
}

// 一页里看得见的行：形状与 page 相同（候选窗、序号、数字键和点选都只认这一份）。非对象行照原样留着，让下游按原来的方式报错。
template <typename Page>
Page visible_page(const Page &page) {
  Page visible;
  for (const auto &candidate : page) {
    if (!candidate.is_object() ||
        candidate_source_is_visible(candidate.value("source", std::uint64_t{})))
      visible.push_back(candidate);
  }
  return visible;
}

// 这一页里第 slot 个看得见的行（slot 从 0 数起，就是数字键按的那一格），没有这么多可见行时是 nullptr。数字选词和点选走它：隐藏的席位在引擎这一页里仍占一格，按页内下标直接取会把它算进去。
template <typename Page>
auto visible_page_candidate(const Page &page, std::size_t slot) -> const typename Page::value_type * {
  std::size_t seen = 0;
  for (const auto &candidate : page) {
    if (!candidate.is_object() ||
        !candidate_source_is_visible(candidate.value("source", std::uint64_t{})))
      continue;
    if (seen == slot)
      return &candidate;
    ++seen;
  }
  return nullptr;
}

// 这一页看得见的行数：候选窗的行数、序号的终点和各处「有没有候选」的判断都用它。
template <typename Page>
std::size_t visible_page_size(const Page &page) {
  std::size_t seen = 0;
  for (const auto &candidate : page)
    if (!candidate.is_object() ||
        candidate_source_is_visible(candidate.value("source", std::uint64_t{})))
      ++seen;
  return seen;
}

} // namespace msime::linux_host
