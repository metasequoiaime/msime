#pragma once

#include <cstddef>
#include <optional>
#include <string>
#include <string_view>

namespace msime::input {

// The Engine's `CandidateSource::Fallback`, as it appears in a view candidate's `source`.
inline constexpr int kCandidateSourceFallback = 9;

// Space converts and Enter takes what is on screen - the way every Japanese input method works.
//
// Romaji is not what the user typed; かな is. The Engine keeps both (`editing_text` is the romaji,
// `reading` the kana it converts to) and has one command for each ending: `MSIME_COMMIT_RAW` gives
// the romaji back and `MSIME_COMMIT_READING` gives the kana. Both front ends here sent COMMIT_RAW
// on Enter for every scheme, so Japanese input committed `nihon` where the user meant にほん, and
// Space committed the first conversion outright, so the second one could not be reached.
//
// The rule this encodes is the one the touch hosts already follow: the first Space means "convert"
// and leaves the first candidate highlighted, later presses step through them, and Enter commits
// the one the user stopped on - or the kana, if they never pressed Space. Editing the reading
// abandons the conversion that was running on the old one, which is why the reading is kept here
// rather than only the index.
//
// The two front ends disagree about almost everything else - IBus routes keysyms and draws its own
// lookup table, fcitx5 hands its panel a candidate list - so what they share is this decision and
// nothing more: the caller performs whatever the answer names.
//
// 这个决定也是 Windows Server 的：它拥有候选，空格的开始、步进和回到首条都在 Server 的会话里执行（`ReplyComposer::japanese_space`），TIP 只从回复里得知转换已经开始，回车时改为上屏高亮候选。Linux 的两个前端经 platforms/linux/src/core/JapaneseConversion.h 用同一份。
class JapaneseConversion
{
  public:
    enum class Action
    {
        /// Not claimed. The key means what it always meant.
        None,
        /// The conversion starts. The first candidate is already highlighted, so nothing moves.
        Start,
        /// Step to the next candidate.
        StepNext,
        /// Step back to the first candidate, having run off the end.
        StepFirst,
        /// Commit the candidate at `index()`.
        CommitCandidate,
        /// Commit the reading, which is the kana the user typed.
        CommitReading,
    };

    /// `first_source` is the `source` of the first candidate, or -1 when there is none.
    Action space(std::string_view reading, std::size_t candidates, int first_source)
    {
        forget_if_reading_changed(reading);
        if (candidates == 0)
            return Action::None;
        // A lone Fallback row is the raw composition the Engine shows when there is nothing to convert (a bare Shift+R prefix, or romaji it cannot read). Windows commits it on the first Space, so Space keeps its commit meaning here.
        if (candidates == 1 && first_source == kCandidateSourceFallback)
            return Action::None;
        if (!index_)
        {
            index_ = 0;
            reading_ = std::string(reading);
            return Action::Start;
        }
        const std::size_t next = *index_ + 1;
        if (next >= candidates)
        {
            index_ = 0;
            return Action::StepFirst;
        }
        index_ = next;
        return Action::StepNext;
    }

    Action enter(std::string_view reading)
    {
        forget_if_reading_changed(reading);
        if (!index_)
            return Action::CommitReading;
        return Action::CommitCandidate;
    }

    /// The candidate a `CommitCandidate` answer names. Meaningless without one.
    std::size_t index() const
    {
        return index_.value_or(0);
    }

    /// Whatever was in progress is over: a commit happened, the composition ended, or focus left.
    void reset()
    {
        index_.reset();
        reading_.clear();
    }

  private:
    void forget_if_reading_changed(std::string_view reading)
    {
        if (index_ && reading != reading_)
            reset();
    }

    std::optional<std::size_t> index_;
    std::string reading_;
};

} // namespace msime::input
