#!/usr/bin/env python3
"""Enable simultaneous Wubi and quanpin candidates (MSIME-Engine b27031e).

The locked Engine predates b27031e. This overlay tags candidates with the scheme that produced
 them and appends distinct quanpin rows to Wubi rows while mixed-pinyin input is enabled. It is
applied after the existing compatibility overlays, so each replacement is strict and repeatable.
"""
from pathlib import Path


def replace_once(path: Path, old: str, new: str) -> None:
    text = path.read_text(encoding="utf-8")
    if new in text:
        return
    if old in text:
        path.write_text(text.replace(old, new, 1), encoding="utf-8", newline="\n")
    else:
        raise RuntimeError(f"Wubi mixed overlay did not match {path}")


def apply(root: Path) -> None:
    word_item = root / "core/word_item.h"
    replace_once(word_item, "#pragma once\n\n#include <string>", "#pragma once\n\n#include \"scheme_type.h\"\n#include <string>")
    replace_once(
        word_item,
        "    CandidateSource source = CandidateSource::Database;\n",
        "    CandidateSource source = CandidateSource::Database;\n    // Mixed Wubi input can contain candidates from both dictionaries; route operations by producer.\n    SchemeType scheme = SchemeType::Quanpin;\n",
    )

    pinyin = root / "providers/pinyin_candidate_provider.cpp"
    old = """    if (request.scheme == SchemeType::Shuangpin)\n    {\n        return shuangpin_engine_.query(request);\n    }\n\n    if (request.scheme == SchemeType::Quanpin)\n    {\n        return quanpin_engine_.query(request);\n    }\n\n    return {};\n"""
    new = """    std::vector<WordItem> candidates;\n    if (request.scheme == SchemeType::Shuangpin)\n        candidates = shuangpin_engine_.query(request);\n    else if (request.scheme == SchemeType::Quanpin)\n        candidates = quanpin_engine_.query(request);\n    else\n        return {};\n    for (WordItem &item : candidates)\n        item.scheme = request.scheme;\n    return candidates;\n"""
    replace_once(pinyin, old, new)

    wubi = root / "providers/wubi_candidate_provider.cpp"
    replace_once(
        wubi,
        "        candidates.emplace_back(key, value, sqlite3_column_int64(query_statement_, 2));\n",
        "        candidates.emplace_back(key, value, sqlite3_column_int64(query_statement_, 2));\n        candidates.back().scheme = SchemeType::Wubi;\n",
    )

    ime = root / "core/ime_session.cpp"
    replace_once(ime, "#include <stdexcept>\n", "#include <stdexcept>\n#include <algorithm>\n#include <unordered_set>\n")
    start = "    state_.candidates = provider_registry_.resolve(state_.request.scheme).query(state_.request);\n"
    end = "}\n\nstd::unique_ptr<IInputScheme> ImeSession::create_scheme"
    text = ime.read_text(encoding="utf-8")
    begin = text.find(start)
    finish = text.find(end, begin)
    if begin < 0 or finish < 0:
        raise RuntimeError(f"Wubi mixed refresh block did not match {ime}")
    block = """    state_.candidates = provider_registry_.resolve(state_.request.scheme).query(state_.request);\n\n    // Mixed Wubi asks quanpin for the same letters and appends words absent from the Wubi table.\n    // Keep the Wubi rows first so normal Wubi ranking and fixed positions retain precedence.\n    // Prefix rows do not answer the complete code, so they must not suppress the mixed query.\n    const bool wubi_table_answered =\n        std::any_of(state_.candidates.begin(), state_.candidates.end(), [this](const WordItem &item) {\n            return item.pinyin == state_.request.normalized_input;\n        });\n    if (wubi_scheme_ != nullptr)\n        wubi_scheme_->set_extended_length_allowed(wubi_options_.mixed_pinyin && !wubi_table_answered);\n    if (wubi_scheme_ != nullptr && wubi_options_.mixed_pinyin)\n    {\n        QuanpinScheme pinyin;\n        pinyin.set_raw_input(state_.request.raw_input, state_.request.raw_input_with_cases);\n        QueryRequest mixed = pinyin.build_request();\n        mixed.enable_quanpin_helpcode = enable_quanpin_helpcode_;\n        mixed.sentence_alternatives = sentence_alternatives_;\n        mixed.enable_quanpin_autocorrect_transposition =\n            (quanpin_autocorrect_types_ & quanpin::kAutocorrectTransposition) != 0;\n        mixed.enable_quanpin_autocorrect_neighbor =\n            (quanpin_autocorrect_types_ & quanpin::kAutocorrectNeighbor) != 0;\n        mixed.fuzzy_pinyin = fuzzy_pinyin_;\n        mixed.key_strokes = state_.request.key_strokes;\n        if (mixed.valid)\n        {\n            const auto pinyin_candidates = provider_registry_.resolve(mixed.scheme).query(mixed);\n            std::unordered_set<std::string> seen;\n            seen.reserve(state_.candidates.size() + pinyin_candidates.size());\n            for (const auto &item : state_.candidates)\n                seen.insert(item.word);\n            for (const auto &item : pinyin_candidates)\n                if (seen.insert(item.word).second)\n                    state_.candidates.push_back(item);\n            state_.answered_by_pinyin_fallback =\n                !state_.candidates.empty() && std::none_of(state_.candidates.begin(), state_.candidates.end(),\n                    [](const WordItem &item) { return item.scheme == SchemeType::Wubi; });\n        }\n    }\n"""
    ime.write_text(text[:begin] + block + text[finish:], encoding="utf-8", newline="\n")


if __name__ == "__main__":
    import sys
    apply(Path(sys.argv[1]))
