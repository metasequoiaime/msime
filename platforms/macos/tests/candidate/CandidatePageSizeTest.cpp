#include "../../src/candidate/CandidatePageSize.h"
#include <cassert>
int main()
{
    using namespace msime::mac;
    // Every size the shared preferences accept is honoured. The old rule kept 5, 7 and 9 and rewrote
    // everything else to 9, which turned the shared default of six into nine on this platform alone and
    // silently rewrote a document written anywhere else.
    for (size_t size = kMinimumCandidatePageSize; size <= kMaximumCandidatePageSize; ++size)
        assert(NormalizeCandidatePageSize(size) == size);
    assert(NormalizeCandidatePageSize(6) == 6);
    // Out of range is pulled to the nearest end rather than snapped to the top.
    assert(NormalizeCandidatePageSize(0) == kMinimumCandidatePageSize);
    assert(NormalizeCandidatePageSize(99) == kMaximumCandidatePageSize);
    // 窗口列出三到十（第十个候选用 0 键选，#6679）。
    assert(kMaximumCandidatePageSize == 10);
    assert(kOfferedCandidatePageSizes == 8);
    assert(CandidatePageSizeForOptionIndex(0) == 3);
    assert(CandidatePageSizeForOptionIndex(kOfferedCandidatePageSizes - 1) == 10);
    assert(CandidatePageSizeForOptionIndex(kOfferedCandidatePageSizes) == 10);
    assert(CandidatePageSizeOptionIndex(3) == 0);
    assert(CandidatePageSizeOptionIndex(6) == 3);
    assert(CandidatePageSizeOptionIndex(10) == kOfferedCandidatePageSizes - 1);
    // A document below the listed set still selects the first item rather than falling off the list.
    assert(CandidatePageSizeOptionIndex(1) == 0);
    return 0;
}
