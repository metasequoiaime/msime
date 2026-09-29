#!/usr/bin/env python3
"""Route operations for mixed Wubi/quanpin candidates by each row's producer scheme."""
from pathlib import Path


def replace_once(path: Path, old: str, new: str) -> None:
    text = path.read_text(encoding="utf-8")
    if new in text:
        return
    if old not in text:
        raise RuntimeError(f"Wubi routing overlay did not match {path}")
    path.write_text(text.replace(old, new, 1), encoding="utf-8", newline="\n")


def apply(root: Path) -> None:
    ime_h = root / "core/ime_session.h"
    replace_once(ime_h, "int update_weight_by_pinyin_and_word(std::string pinyin, std::string word);", "int update_weight_by_pinyin_and_word(SchemeType scheme, std::string pinyin, std::string word);")
    replace_once(ime_h, "int delete_by_pinyin_and_word(std::string pinyin, std::string word);", "int delete_by_pinyin_and_word(SchemeType scheme, std::string pinyin, std::string word);")
    replace_once(ime_h, "std::optional<WordItem> find_candidate(const std::string &key, const std::string &value);", "std::optional<WordItem> find_candidate(SchemeType scheme, const std::string &key, const std::string &value);")
    ime_cpp = root / "core/ime_session.cpp"
    replace_once(ime_cpp, "int ImeSession::update_weight_by_pinyin_and_word(std::string pinyin, std::string word)\n{\n    return provider_registry_.update_weight_by_pinyin_and_word(candidate_scheme(), std::move(pinyin), std::move(word));\n}", "int ImeSession::update_weight_by_pinyin_and_word(SchemeType scheme, std::string pinyin, std::string word)\n{\n    return provider_registry_.update_weight_by_pinyin_and_word(scheme, std::move(pinyin), std::move(word));\n}")
    replace_once(ime_cpp, "int ImeSession::delete_by_pinyin_and_word(std::string pinyin, std::string word)\n{\n    return provider_registry_.delete_by_pinyin_and_word(current_scheme_type(), std::move(pinyin), std::move(word));\n}", "int ImeSession::delete_by_pinyin_and_word(SchemeType scheme, std::string pinyin, std::string word)\n{\n    return provider_registry_.delete_by_pinyin_and_word(scheme, std::move(pinyin), std::move(word));\n}")
    replace_once(ime_cpp, "std::optional<WordItem> ImeSession::find_candidate(const std::string &key, const std::string &value)\n{\n    return provider_registry_.find_candidate(candidate_scheme(), key, value);\n}", "std::optional<WordItem> ImeSession::find_candidate(SchemeType scheme, const std::string &key, const std::string &value)\n{\n    return provider_registry_.find_candidate(scheme, key, value);\n}")

    h = root / "core/input_session.h"
    replace_once(h, "        std::string selected_canonical_pinyin;\n", "        std::string selected_canonical_pinyin;\n        bool wubi_native = false;\n")
    replace_once(h, "std::optional<WordItem> find_candidate(const std::string &key, const std::string &value);", "std::optional<WordItem> find_candidate(const std::string &key, const std::string &value, SchemeType scheme = SchemeType::Quanpin);")
    replace_once(h, "const std::string &selected_canonical_pinyin);", "const std::string &selected_canonical_pinyin, SchemeType selected_scheme = SchemeType::Quanpin);")
    replace_once(h, "bool selection_completes_composition(const std::string &selected_pinyin, const std::string &selected_word) const;", "bool selection_completes_composition(const std::string &selected_pinyin, const std::string &selected_word, SchemeType selected_scheme = SchemeType::Quanpin) const;")
    replace_once(h, "    bool wubi_candidates_are_native() const;", "    bool wubi_candidates_are_native() const;\n    static bool is_wubi_native_candidate(const WordItem &item);\n    std::size_t wubi_native_candidate_count() const;")

    c = root / "core/input_session_composition.cpp"
    replace_once(c, "std::optional<WordItem> InputSession::find_candidate(const std::string &key, const std::string &value)\n{\n    return engine_.find_candidate(key, value);\n}", "std::optional<WordItem> InputSession::find_candidate(const std::string &key, const std::string &value, SchemeType scheme)\n{\n    return engine_.find_candidate(scheme, key, value);\n}")
    replace_once(c, "return engine_.update_weight_by_pinyin_and_word(std::move(pinyin), std::move(word));", "return engine_.update_weight_by_pinyin_and_word(scheme(), std::move(pinyin), std::move(word));")
    replace_once(c, "return engine_.delete_by_pinyin_and_word(std::move(pinyin), std::move(word));", "return engine_.delete_by_pinyin_and_word(scheme(), std::move(pinyin), std::move(word));")
    candidates = root / "core/input_session_candidates.cpp"
    replace_once(candidates,
                 "[this](const std::string &key, const std::string &word) { return engine_.find_candidate(key, word); },",
                 "[this](const std::string &key, const std::string &word) { return engine_.find_candidate(key, word, scheme()); },")
    replace_once(c, "bool InputSession::selection_completes_composition(const std::string &selected_pinyin,\n                                                   const std::string &selected_word) const", "bool InputSession::selection_completes_composition(const std::string &selected_pinyin, const std::string &selected_word,\n                                                   SchemeType selected_scheme) const")
    replace_once(c, "if (is_japanese() || wubi_candidates_are_native())", "if (is_japanese() || selected_scheme == SchemeType::Wubi)")
    replace_once(c, "const std::string &selected_pinyin, const std::string &selected_word, const std::string &selected_canonical_pinyin)\n{\n    SelectionTransition transition;\n    transition.selected_canonical_pinyin = selected_canonical_pinyin;", "const std::string &selected_pinyin, const std::string &selected_word, const std::string &selected_canonical_pinyin,\n    SchemeType selected_scheme)\n{\n    SelectionTransition transition;\n    transition.selected_canonical_pinyin = selected_canonical_pinyin;\n    transition.wubi_native = selected_scheme == SchemeType::Wubi;")
    replace_once(c, "if (wubi_candidates_are_native())\n    {\n        transition.full_pure_pinyin", "if (transition.wubi_native)\n    {\n        transition.full_pure_pinyin")
    text = c.read_text(encoding="utf-8")
    text = text.replace("selection_completes_composition(selected_pinyin, selected_word)",
                        "selection_completes_composition(selected_pinyin, selected_word, selected_scheme)")
    c.write_text(text, encoding="utf-8", newline="\n")
    replace_once(c, "    if (wubi_candidates_are_native())\n    {\n        progress.pinyin", "    if (selection_transition.wubi_native)\n    {\n        progress.pinyin")
    old = """bool InputSession::answered_by_pinyin_fallback() const
{
    return engine_.answered_by_pinyin_fallback();
}

bool InputSession::wubi_candidates_are_native() const
{
    return is_wubi() && !engine_.answered_by_pinyin_fallback();
}

bool InputSession::candidates_follow_pinyin() const
{
    return current_scheme_type() == SchemeType::Quanpin || current_scheme_type() == SchemeType::Shuangpin ||
           engine_.answered_by_pinyin_fallback();
}
"""
    new = """bool InputSession::answered_by_pinyin_fallback() const
{
    return is_wubi() && !candidates().empty() &&
           std::all_of(candidates().begin(), candidates().end(), [](const WordItem &item) {
               return item.scheme != SchemeType::Wubi;
           });
}

bool InputSession::wubi_candidates_are_native() const
{
    return is_wubi() && std::any_of(candidates().begin(), candidates().end(), is_wubi_native_candidate);
}

bool InputSession::is_wubi_native_candidate(const WordItem &item)
{
    return item.scheme == SchemeType::Wubi;
}

std::size_t InputSession::wubi_native_candidate_count() const
{
    return static_cast<std::size_t>(std::count_if(candidates().begin(), candidates().end(), is_wubi_native_candidate));
}

bool InputSession::candidates_follow_pinyin() const
{
    return current_scheme_type() == SchemeType::Quanpin || current_scheme_type() == SchemeType::Shuangpin ||
           !wubi_candidates_are_native();
}
"""
    replace_once(c, old, new)
    replace_once(c, """if (!wubi_candidates_are_native() || !engine_.wubi_code_is_complete())
        return false;
    return candidates().size() == 1;""", """if (!wubi_candidates_are_native() || !engine_.wubi_code_is_complete())
        return false;
    return wubi_native_candidate_count() == 1;""")

    cpp = root / "core/input_session.cpp"
    replace_once(cpp, "advance_composition_after_selection(selected->pinyin, selected->word, selected->canonical_pinyin)", "advance_composition_after_selection(selected->pinyin, selected->word, selected->canonical_pinyin, selected->scheme)")
    replace_once(cpp, "engine_.update_weight_by_pinyin_and_word(pinyin, selected.word)", "engine_.update_weight_by_pinyin_and_word(selected.scheme, pinyin, selected.word)")
    replace_once(cpp, "    const bool wubi = wubi_candidates_are_native();", "    const bool wubi = selected.scheme == SchemeType::Wubi;")

    candidates = root / "core/input_session_candidates.cpp"
    replace_once(candidates, "std::string InputSession::position_context(bool english) const", "std::string InputSession::position_context(bool english, bool wubi) const")
    replace_once(candidates, """    if (wubi_candidates_are_native())
        return engine_.get_request().raw_input;""", """    if (wubi)
        return engine_.get_request().raw_input;""")
    old_positions = """    if (local_input_mode_ == LocalInputMode::None && !dedicated_english_mode_ && scheme() != SchemeType::JapaneseRomaji)
        user_dictionary::apply_fixed_positions(
            journal, position_context(false), items, engine_.get_request().raw_input.size() == 1,
            [this](const std::string &key, const std::string &word) { return engine_.find_candidate(key, word, scheme()); },
            has_active_helpcode());
    else if (local_input_mode_ == LocalInputMode::SuperJianpin)
"""
    new_positions = """    const bool regular =
        local_input_mode_ == LocalInputMode::None && !dedicated_english_mode_ && scheme() != SchemeType::JapaneseRomaji;
    if (regular && is_wubi())
    {
        std::vector<WordItem> wubi_items;
        std::vector<WordItem> pinyin_items;
        for (auto &item : items)
            (is_wubi_native_candidate(item) ? wubi_items : pinyin_items).push_back(std::move(item));
        const bool include_missing = engine_.get_request().raw_input.size() == 1;
        if (!wubi_items.empty())
            user_dictionary::apply_fixed_positions(
                journal, position_context(false, true), wubi_items, include_missing,
                [this](const std::string &key, const std::string &word) {
                    return engine_.find_candidate(key, word, SchemeType::Wubi);
                },
                has_active_helpcode());
        if (!pinyin_items.empty())
            user_dictionary::apply_fixed_positions(
                journal, position_context(false, false), pinyin_items, include_missing,
                [this](const std::string &key, const std::string &word) {
                    return engine_.find_candidate(key, word, SchemeType::Quanpin);
                },
                has_active_helpcode());
        items.clear();
        items.insert(items.end(), std::make_move_iterator(wubi_items.begin()),
                     std::make_move_iterator(wubi_items.end()));
        items.insert(items.end(), std::make_move_iterator(pinyin_items.begin()),
                     std::make_move_iterator(pinyin_items.end()));
    }
    else if (regular)
        user_dictionary::apply_fixed_positions(
            journal, position_context(false, false), items, engine_.get_request().raw_input.size() == 1,
            [this](const std::string &key, const std::string &word) { return engine_.find_candidate(key, word, scheme()); },
            has_active_helpcode());
    else if (local_input_mode_ == LocalInputMode::SuperJianpin)
"""
    replace_once(candidates, old_positions, new_positions)
    replace_once(candidates, """const auto context = position_context(english);
    const bool wubi = wubi_candidates_are_native() && local_input_mode_ != LocalInputMode::SuperJianpin;""", """const bool wubi = selected.scheme == SchemeType::Wubi && local_input_mode_ != LocalInputMode::SuperJianpin;
    const auto context = position_context(english, wubi);""")
    replace_once(candidates, """const bool wubi = wubi_candidates_are_native() && local_input_mode_ != LocalInputMode::SuperJianpin;
    const auto kind =""", """const bool wubi = selected.scheme == SchemeType::Wubi && local_input_mode_ != LocalInputMode::SuperJianpin;
    const auto kind =""")


    session = root / "core/session.cpp"
    replace_once(session, "session.selection_completes_composition(candidate.pinyin, candidate.word)", "session.selection_completes_composition(candidate.pinyin, candidate.word, candidate.scheme)")


def main() -> None:
    import sys
    apply(Path(sys.argv[1]))


if __name__ == "__main__":
    main()
