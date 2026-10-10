#pragma once
#include <cstddef>
namespace msime::mac {
// How many candidates a page may hold, and what to do with a number outside that.
//
// This used to accept 5, 7 and 9 and rewrite everything else to 9 - the set the Apple reference's own
// window offered. Two things followed from it that no one asked for: the shared default of six became
// nine on this platform and nowhere else, and a preferences document carrying any other size - written
// by another host, or by hand - was silently rewritten rather than honoured. The reference this client
// replicates offers three through nine, and the shared preferences accept one through nine, so the
// range is theirs and out-of-range values are pulled to the nearest end instead of snapped to the top.
// 共享偏好的上限后来放宽到 10（#6679），第十个候选用 0 键选，这里跟着放宽。
constexpr size_t kMinimumCandidatePageSize = 1;
constexpr size_t kMaximumCandidatePageSize = 10;
// What the settings windows list, which is the reference's set rather than the whole accepted range:
// one or two candidates a page is a document this host honours, not a choice it suggests.
constexpr size_t kFirstOfferedCandidatePageSize = 3;
// What an unset preference means. The shared preferences default to six and so does the reference's
// window; reading an absent value as anything else would make this platform disagree with both before
// the user has touched the setting.
constexpr size_t kDefaultCandidatePageSize = 6;
constexpr size_t kOfferedCandidatePageSizes =
    kMaximumCandidatePageSize - kFirstOfferedCandidatePageSize + 1;
constexpr size_t NormalizeCandidatePageSize(size_t value)
{
    return value < kMinimumCandidatePageSize  ? kMinimumCandidatePageSize
           : value > kMaximumCandidatePageSize ? kMaximumCandidatePageSize
                                               : value;
}
constexpr size_t CandidatePageSizeForOptionIndex(size_t index)
{
    return index >= kOfferedCandidatePageSizes ? kMaximumCandidatePageSize
                                               : kFirstOfferedCandidatePageSize + index;
}
constexpr size_t CandidatePageSizeOptionIndex(size_t value)
{
    value = NormalizeCandidatePageSize(value);
    return value <= kFirstOfferedCandidatePageSize ? 0 : value - kFirstOfferedCandidatePageSize;
}
}
