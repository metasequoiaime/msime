#!/usr/bin/env python3
"""Let a pinned or promoted English word take the first seat of the mixed candidate list.

MSIME-Windows ranks an English word against the whole candidate window it shows, Chinese candidates included: `event_listener.cpp` hands `Global::candidate_ui.items` to `adjust_english_candidate_ranking`, so a pin (置顶) or learning that reaches the top writes the maximum weight of the mixed list plus 1000. Its `NormalizeMixedCandidateOrder` (`server/src/ipc/candidate_selection_policy.h`, added in 2af10f56) then moves an EnglishDictionary candidate whose weight is the unique maximum of the list, and which has no fixed position, to index zero. `PromotedEnglishCandidateCanBecomeTheFirstMixedCandidate` pins that order.

The locked Engine does neither. `adjust_candidate_frequency` ranks English among the English words alone, so the new weight need not beat a Chinese one, and `CandidateQueries::mixed` seats the first English word after the leading Chinese candidate with no weight check. A pinned English word therefore never came first and Space committed Chinese.

This overlay ranks a mixed-input English word against the full mixed list and adds the reference's promotion rule at the end of `CandidateQueries::mixed`. Dedicated and temporary English keep ranking among their English words: the temporary list leads with the raw text as a Generated candidate, and counting it would change every learning step there. A word with a stored position is left to `apply_candidate_positions`, which reseats it afterwards, the same outcome as the reference's `fixed_position == 0` guard followed by its fixed-English reseat. The Engine test fixtures whose English weights sat above their Chinese ones are lifted so they keep describing the default order, and new cases cover the pin and the fixed-position exception.

Only a weight the user gave the word may win that comparison. The shipped `english.db` (dict-v2.0.1) stores Google unigram counts as weights, `dancing` at 14310606 against 225415 for 单词, and 18726 of its 19991 words outweigh the heaviest pinyin row of `tbl_2_d`, so a raw maximum put an English completion first for almost every prefix it matched. The promotion therefore also requires the journal to hold an English upsert carrying the candidate's current weight, which pinning, learning and the dictionary manager all write, and `user_dictionary::has_user_english_weight` is added for that lookup. The pin case now gives the unpinned word a weight above the Chinese ones, so it also covers a corpus weight that must stay in its default seat.
"""

from pathlib import Path

APPLIED = "promoted English word takes the first seat"


def replace_once(path: Path, before: str, after: str) -> None:
    text = path.read_text(encoding="utf-8")
    count = text.count(before)
    if count != 1:
        raise RuntimeError(f"Engine overlay expected one match in {path}, found {count}")
    path.write_text(text.replace(before, after, 1), encoding="utf-8")


RANKING_BEFORE = """        std::vector<WordItem> ranked_candidates;
        std::copy_if(candidates().begin(), candidates().end(), std::back_inserter(ranked_candidates),
                     [](const WordItem &candidate) { return candidate.source == CandidateSource::EnglishDictionary; });
"""

RANKING_AFTER = """        // In mixed input the English word is ranked against the whole list the user sees, Chinese candidates included, as the reference does with its full candidate window. A pin or learning that reaches the top then writes a weight above every Chinese candidate, so the promoted English word takes the first seat in CandidateQueries::mixed. Dedicated and temporary English rank among their English words only; the temporary list leads with the raw text, which is not a ranking neighbour.
        const bool mixed_list = !dedicated_english_mode_ && local_input_mode_ == LocalInputMode::None;
        std::vector<WordItem> ranked_candidates;
        std::copy_if(candidates().begin(), candidates().end(), std::back_inserter(ranked_candidates),
                     [mixed_list](const WordItem &candidate) {
                         return mixed_list || candidate.source == CandidateSource::EnglishDictionary;
                     });
"""

MIXED_BEFORE = """    for (auto *source : {&english_candidates, &emoji_candidates, &kaomoji_candidates})
    {
        candidates.insert(candidates.end(), std::make_move_iterator(source->begin()),
                          std::make_move_iterator(source->end()));
    }
    return candidates;
}
"""

MIXED_AFTER = """    for (auto *source : {&english_candidates, &emoji_candidates, &kaomoji_candidates})
    {
        candidates.insert(candidates.end(), std::make_move_iterator(source->begin()),
                          std::make_move_iterator(source->end()));
    }

    // A promoted English word takes the first seat: an English candidate whose weight is the unique maximum of the mixed list moves to index zero, as the reference's NormalizeMixedCandidateOrder does. Pinning it, or learning that reaches the top, writes the list's maximum weight plus 1000, so this is how Space comes to commit it. A tie keeps the default order, and a word with a stored position is reseated by apply_candidate_positions afterwards. The weight must be one the user gave the word: shipped English weights are corpus counts, far above pinyin weights, and would otherwise seat an unlearned completion ahead of every Chinese candidate.
    const auto promoted =
        std::max_element(candidates.begin(), candidates.end(),
                         [](const WordItem &left, const WordItem &right) { return left.weight < right.weight; });
    if (promoted != candidates.end() && promoted->source == CandidateSource::EnglishDictionary &&
        promoted->fixed_position == 0 && std::count_if(candidates.begin(), candidates.end(), [&](const WordItem &item) {
                                             return item.weight == promoted->weight;
                                         }) == 1 &&
        user_dictionary::has_user_english_weight(path_to_utf8(paths_.user(assets::user_journal)), promoted->pinyin,
                                                 promoted->word, promoted->weight))
    {
        std::rotate(candidates.begin(), promoted, std::next(promoted));
    }
    return candidates;
}
"""

INCLUDE_BEFORE = """#include "../local_modes/unicode_query.h"
"""

INCLUDE_AFTER = """#include "../local_modes/unicode_query.h"
#include "../user_dictionary/user_dictionary_journal.h"
"""

JOURNAL_HEADER_BEFORE = """bool is_fixed(const std::string &user_db_path, const std::string &context_key, const std::string &entry_key,
              const std::string &value);
"""

JOURNAL_HEADER_AFTER = """bool is_fixed(const std::string &user_db_path, const std::string &context_key, const std::string &entry_key,
              const std::string &value);
// True when the journal holds an English upsert for the word carrying this weight, i.e. the weight was written by pinning, learning or the dictionary manager rather than shipped with english.db.
bool has_user_english_weight(const std::string &user_db_path, const std::string &entry_key, const std::string &value,
                             std::int64_t weight);
"""

JOURNAL_SOURCE_BEFORE = """           bind_text(stmt.get(), 3, value) && sqlite3_step(stmt.get()) == SQLITE_ROW;
}
"""

JOURNAL_SOURCE_AFTER = """           bind_text(stmt.get(), 3, value) && sqlite3_step(stmt.get()) == SQLITE_ROW;
}

bool has_user_english_weight(const std::string &user_db_path, const std::string &entry_key, const std::string &value,
                             std::int64_t weight)
{
    UserDatabase db(user_db_path);
    if (!db)
        return false;
    auto stmt = prepare(db.get(), "SELECT 1 FROM user_dictionary_operations WHERE dictionary='english' AND key=?1 AND"
                                  " value=?2 AND operation='upsert' AND weight=?3");
    return stmt && bind_text(stmt.get(), 1, entry_key) && bind_text(stmt.get(), 2, value) &&
           sqlite3_bind_int64(stmt.get(), 3, weight) == SQLITE_OK && sqlite3_step(stmt.get()) == SQLITE_ROW;
}
"""

FIXTURE_BEFORE = """    database.execute("INSERT INTO tbl_1_n VALUES('ni','n','你',200)");
    if (two_candidates)
    {
        database.execute("INSERT INTO tbl_1_n VALUES('ni','n','倪',100)");
    }
"""

# The mixed-order assertions describe the default seating, so their Chinese weights have to stay above the English ones the way the shipped dictionaries' do. Before this overlay the weights never met.
FIXTURE_AFTER = """    database.execute("INSERT INTO tbl_1_n VALUES('ni','n','你',2000)");
    if (two_candidates)
    {
        database.execute("INSERT INTO tbl_1_n VALUES('ni','n','倪',1000)");
    }
"""

EXPRESSIVE_FIXTURE_BEFORE = """                     "INSERT INTO tbl_1_n VALUES('ni','n','你',200);"
                     "INSERT INTO tbl_1_n VALUES('ni','n','倪',100);"
"""

EXPRESSIVE_FIXTURE_AFTER = """                     "INSERT INTO tbl_1_n VALUES('ni','n','你',2000);"
                     "INSERT INTO tbl_1_n VALUES('ni','n','倪',1000);"
"""

TEST_BEFORE = """    metasequoia::InputSession threshold(SchemeType::Quanpin);
    mixed_options.minimum_prefix = 3;
"""

TEST_AFTER = """    {
        // A promoted English word takes the first seat, so Space commits it: pinning ranks it against the whole mixed list, Chinese included, and the list then seats the unique maximum first. A shipped weight above every Chinese one, as the corpus counts in english.db are, is not a promotion. A word with a stored position keeps that position instead.
        const std::filesystem::path pinned_directory = root / "english-pinned";
        prepare_main_database(pinned_directory, true);
        {
            Database database(pinned_directory / "english.db");
            database.execute("CREATE TABLE english_words(word TEXT COLLATE BINARY NOT NULL,display TEXT NOT NULL,"
                             "weight INTEGER NOT NULL DEFAULT 0,PRIMARY KEY(word,display)) WITHOUT ROWID");
            database.execute("CREATE TABLE en_zh_glosses(english TEXT PRIMARY KEY,chinese_gloss TEXT NOT NULL)");
            database.execute("CREATE TABLE zh_en_glosses(chinese TEXT PRIMARY KEY,english_gloss TEXT NOT NULL)");
            database.execute("INSERT INTO english_words VALUES('nimbus','Nimbus',50000)");
            database.execute("INSERT INTO english_words VALUES('ninja','Ninja',10)");
        }
        set_data_directory(pinned_directory);
        metasequoia::InputSession pinned(SchemeType::Quanpin);
        require(pinned.set_english_input_options(mixed_options), "Valid mixed-English options were rejected.");
        type(pinned, "ni");
        require(pinned.candidates().size() == 4 && pinned.candidates()[0].word == "你" &&
                    pinned.candidates()[1].word == "Nimbus" && pinned.candidates()[3].word == "Ninja",
                "An unpinned English word whose shipped weight beats the Chinese ones left its default mixed seat.");
        require(pinned.pin_candidate(3).handled, "Pinning a mixed English candidate was not handled.");
        require(pinned.candidates().size() == 4 && pinned.candidates()[0].word == "Ninja" &&
                    pinned.candidates()[0].source == CandidateSource::EnglishDictionary &&
                    pinned.candidates()[1].word == "你",
                "A pinned English word did not take the first mixed seat.");
        {
            Database database(pinned_directory / "english.db");
            require(database.query_integer("SELECT weight FROM english_words WHERE word='ninja' AND "
                                           "display='Ninja'") == 51000,
                    "Pinning a mixed English word did not rank it above the Chinese candidates.");
        }
        const auto committed = pinned.select_candidate(0);
        require(committed.handled && committed.commit == "Ninja",
                "The first seat did not commit the pinned English word.");

        metasequoia::InputSession positioned(SchemeType::Quanpin);
        positioned.enable_fixed_positions();
        require(positioned.set_english_input_options(mixed_options), "Valid mixed-English options were rejected.");
        type(positioned, "ni");
        require(positioned.candidates()[0].word == "Ninja", "The pinned English word lost the first seat.");
        require(positioned.set_candidate_position(0, 3).handled, "Fixing a mixed English position was not handled.");
        require(positioned.candidates().size() == 4 && positioned.candidates()[0].word == "你" &&
                    positioned.candidates()[2].word == "Ninja",
                "An English word with a stored position took the first seat instead of its position.");
        set_data_directory(english_mode_directory);
    }

    metasequoia::InputSession threshold(SchemeType::Quanpin);
    mixed_options.minimum_prefix = 3;
"""


def apply(root: Path) -> None:
    queries = root / "core/candidate_queries.cpp"
    if APPLIED in queries.read_text(encoding="utf-8"):
        return
    replace_once(root / "core/input_session.cpp", RANKING_BEFORE, RANKING_AFTER)
    replace_once(queries, MIXED_BEFORE, MIXED_AFTER)
    replace_once(queries, INCLUDE_BEFORE, INCLUDE_AFTER)
    replace_once(root / "user_dictionary/user_dictionary_journal.h", JOURNAL_HEADER_BEFORE, JOURNAL_HEADER_AFTER)
    replace_once(root / "user_dictionary/user_dictionary_journal.cpp", JOURNAL_SOURCE_BEFORE, JOURNAL_SOURCE_AFTER)
    tests = root / "tests/src/test_english_input_session.cpp"
    replace_once(tests, FIXTURE_BEFORE, FIXTURE_AFTER)
    replace_once(tests, TEST_BEFORE, TEST_AFTER)
    replace_once(root / "tests/src/test_mixed_expressive_input_session.cpp", EXPRESSIVE_FIXTURE_BEFORE, EXPRESSIVE_FIXTURE_AFTER)


if __name__ == "__main__":
    import sys

    apply(Path(sys.argv[1]))
