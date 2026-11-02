#include "../../src/core/JapaneseConversion.h"

#include <cassert>
#include <string>

using msime::linux_host::JapaneseConversion;
using Action = msime::linux_host::JapaneseConversion::Action;

int main()
{
    // Enter with nothing converted is the kana. This is the whole reason the class exists: both
    // front ends used to send the Engine's raw-input command here, which in Japanese is the romaji.
    {
        JapaneseConversion conversion;
        assert(conversion.enter("nihon") == Action::CommitReading);
    }

    // The first Space converts rather than committing, and leaves the first candidate named.
    {
        JapaneseConversion conversion;
        assert(conversion.space("nihon", 3, 0) == Action::Start);
        assert(conversion.index() == 0);
        // Enter now takes that candidate instead of the kana.
        assert(conversion.enter("nihon") == Action::CommitCandidate);
        assert(conversion.index() == 0);
    }

    // Further presses step, and running off the end comes back to the first.
    {
        JapaneseConversion conversion;
        assert(conversion.space("nihon", 3, 0) == Action::Start);
        assert(conversion.space("nihon", 3, 0) == Action::StepNext);
        assert(conversion.index() == 1);
        assert(conversion.space("nihon", 3, 0) == Action::StepNext);
        assert(conversion.index() == 2);
        assert(conversion.space("nihon", 3, 0) == Action::StepFirst);
        assert(conversion.index() == 0);
        assert(conversion.enter("nihon") == Action::CommitCandidate);
        assert(conversion.index() == 0);
    }

    // A single candidate has nowhere to step: the next press is the first one again rather than an
    // index the caller would read past the end of its own list.
    {
        JapaneseConversion conversion;
        assert(conversion.space("na", 1, 0) == Action::Start);
        assert(conversion.space("na", 1, 0) == Action::StepFirst);
        assert(conversion.index() == 0);
    }

    // Editing the reading abandons the conversion that was running on the old one.
    {
        JapaneseConversion conversion;
        assert(conversion.space("nihon", 3, 0) == Action::Start);
        assert(conversion.space("nihon", 3, 0) == Action::StepNext);
        assert(conversion.enter("nihongo") == Action::CommitReading);
        // And the next Space starts over rather than continuing from where it was.
        assert(conversion.space("nihongo", 3, 0) == Action::Start);
        assert(conversion.index() == 0);
    }

    // Nothing to convert: the caller keeps whatever Space meant before.
    {
        JapaneseConversion conversion;
        assert(conversion.space("nihon", 0, 0) == Action::None);
        // And that non-answer starts nothing, so Enter is still the kana.
        assert(conversion.enter("nihon") == Action::CommitReading);
    }

    // A lone Fallback row is the raw text the Engine commits on Space, so there is nothing to convert.
    {
        using msime::linux_host::kCandidateSourceFallback;
        JapaneseConversion conversion;
        assert(conversion.space("R", 1, kCandidateSourceFallback) == Action::None);
        assert(conversion.enter("R") == Action::CommitReading);
        // Only a lone Fallback row: a real candidate, or more than one row, still converts.
        assert(conversion.space("R", 1, 0) == Action::Start);
        JapaneseConversion several;
        assert(several.space("nihon", 2, kCandidateSourceFallback) == Action::Start);
    }

    // 开着云候选时第二格是云占位行「☁」。运行时的「下一个」越过它（#10665），所以空格数的、回车按 index() 选的都是去掉它之后的行：第二次空格停在「本」上，回车选的也是它。
    {
        using msime::linux_host::japanese_conversion_rows;
        using msime::linux_host::kCandidateSourceCloudPending;
        const nlohmann::json page = nlohmann::json::array({
            {{"text", "日"}, {"source", 0}},
            {{"text", "☁"}, {"source", kCandidateSourceCloudPending}},
            {{"text", "本"}, {"source", 0}},
            {{"text", "二本"}},
        });
        const auto rows = japanese_conversion_rows(page);
        assert(rows.size() == 3);
        assert(rows[0]["text"] == "日" && rows[1]["text"] == "本" && rows[2]["text"] == "二本");
        JapaneseConversion conversion;
        assert(conversion.space("nihon", rows.size(), 0) == Action::Start);
        assert(conversion.space("nihon", rows.size(), 0) == Action::StepNext);
        assert(conversion.enter("nihon") == Action::CommitCandidate);
        assert(rows[conversion.index()]["text"] == "本");
        // 只剩占位行时没有可转换的行，空格照旧。
        const auto pending = japanese_conversion_rows(nlohmann::json::array({{{"text", "☁"}, {"source", kCandidateSourceCloudPending}}}));
        assert(pending.empty());
        assert(japanese_conversion_rows(nlohmann::json::object()).empty());
    }

    // A reset puts it back to the state a fresh composition starts in.
    {
        JapaneseConversion conversion;
        assert(conversion.space("nihon", 3, 0) == Action::Start);
        conversion.reset();
        assert(conversion.enter("nihon") == Action::CommitReading);
    }

    return 0;
}
