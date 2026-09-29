"""Apply Engine #208: deduplicate online candidates before source quotas.

The locked Engine rejects an AI batch as soon as the provider sends more than ten rows, even when
some rows repeat. The reference counts the source quota after removing duplicates, while retaining
provider order and validating every original row. This overlay keeps the repository's Engine lock
reproducible without advancing the archive pin.
"""

from pathlib import Path


def replace_once(path: Path, before: str, after: str) -> None:
    text = path.read_text(encoding="utf-8")
    if after in text and before not in text:
        return
    count = text.count(before)
    if count != 1:
        raise RuntimeError(f"Engine overlay expected one match in {path}, found {count}")
    path.write_text(text.replace(before, after, 1), encoding="utf-8")


def apply(root: Path) -> None:
    path = root / "core/online_candidate_batch.h"
    replace_once(
        path,
        "#include <string>\n#include <vector>\n",
        "#include <string>\n#include <unordered_set>\n#include <vector>\n",
    )
    replace_once(
        path,
        """    const size_t limit = source == CandidateSource::AiSuggestion ? 10 : 1;
    if ((source != CandidateSource::AiSuggestion && source != CandidateSource::CloudSuggestion) || words.empty() ||
        words.size() > limit || std::any_of(words.begin(), words.end(), [](const std::string &word) {
            return word.empty() || word.size() > 4096 ||
                   std::any_of(word.begin(), word.end(), [](unsigned char c) { return c < 32 || c == 127; });
        }))
        return false;
""",
        """    const size_t limit = source == CandidateSource::AiSuggestion ? 10 : 1;
    if ((source != CandidateSource::AiSuggestion && source != CandidateSource::CloudSuggestion) || words.empty())
        return false;
    std::vector<std::string> unique_words;
    unique_words.reserve((std::min)(words.size(), limit));
    std::unordered_set<std::string> seen_words;
    seen_words.reserve((std::min)(words.size(), limit));
    for (const std::string &word : words)
    {
        if (word.empty() || word.size() > 4096 ||
            std::any_of(word.begin(), word.end(), [](unsigned char c) { return c < 32 || c == 127; }))
            return false;
        if (seen_words.insert(word).second)
            unique_words.push_back(word);
    }
    // Provider responses can repeat a candidate. Count the source quota after removing duplicates so a
    // repeated result cannot reject an otherwise valid batch or consume one of its available seats.
    if (unique_words.size() > limit)
        return false;
""",
    )
    replace_once(path, "    for (const auto &word : words)\n", "    for (const auto &word : unique_words)\n")


if __name__ == "__main__":
    apply(Path(__file__).resolve().parents[1] / "vendor/MSIME-Engine")
