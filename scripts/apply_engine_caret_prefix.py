#!/usr/bin/env python3
"""Port Engine bc46f27 caret-prefix candidate decoding.

The host already moves the composition caret through the shared Command API and receives the
caret in SessionSnapshot. This overlay makes that movement select candidates for the complete
pinyin units before the caret while preserving the pending suffix.
"""
from pathlib import Path


def replace_once(path: Path, before: str, after: str, marker: str) -> None:
    text = path.read_text(encoding="utf-8")
    if marker in text:
        return
    count = text.count(before)
    if count != 1:
        raise RuntimeError(f"Engine caret overlay expected one match in {path}, found {count}")
    path.write_text(text.replace(before, after, 1), encoding="utf-8")


def replace_every(path: Path, before: str, after: str) -> None:
    text = path.read_text(encoding="utf-8")
    if before not in text:
        if after in text:
            return
        raise RuntimeError(f"Engine caret overlay did not match {path}")
    path.write_text(text.replace(before, after), encoding="utf-8")


def apply(root: Path) -> None:
    public_header = root / "include/metasequoia/session.h"
    replace_once(public_header, "#include <memory>\n", "#include <memory>\n#include <optional>\n", "#include <optional>")
    replace_once(public_header,
        "    std::vector<std::size_t> segment_raw_boundaries() const;\n",
        "    std::vector<std::size_t> segment_raw_boundaries() const;\n"
        "    void set_caret(std::optional<std::size_t> caret);\n"
        "    std::size_t prefix_end() const;\n"
        "    std::string pending_suffix() const;\n",
        "void set_caret(std::optional<std::size_t> caret)")

    session_cpp = root / "core/session.cpp"
    replace_once(session_cpp,
        """std::vector<std::size_t> Session::segment_raw_boundaries() const
{
    if (impl_->nine_key.active())
        return {};
    return impl_->session.segment_raw_boundaries();
}
""",
        """std::vector<std::size_t> Session::segment_raw_boundaries() const
{
    if (impl_->nine_key.active())
        return {};
    return impl_->session.segment_raw_boundaries();
}
void Session::set_caret(std::optional<std::size_t> caret)
{
    if (impl_->nine_key.active())
        return;
    impl_->session.set_caret(caret);
    impl_->session.recompute_candidates();
}
std::size_t Session::prefix_end() const
{
    if (impl_->nine_key.active())
        return 0;
    return impl_->session.prefix_end();
}
std::string Session::pending_suffix() const
{
    if (impl_->nine_key.active())
        return {};
    return impl_->session.pending_suffix();
}
""",
        "Session::pending_suffix")

    interface = root / "schemes/input_scheme.h"
    replace_once(
        interface,
        "    virtual SchemeType type() const = 0;\n",
        "    virtual SchemeType type() const = 0;\n"
        "    virtual void set_raw_input(const std::string &raw_input, const std::string &raw_input_with_cases) = 0;\n",
        "virtual void set_raw_input",
    )
    for name in ["japanese_romaji_scheme.h", "quanpin_scheme.h", "shuangpin_scheme.h", "wubi_scheme.h"]:
        path = root / "schemes" / name
        replace_once(path, "void set_raw_input(const std::string &raw_input, const std::string &raw_input_with_cases);",
                     "void set_raw_input(const std::string &raw_input, const std::string &raw_input_with_cases) override;",
                     "set_raw_input(const std::string &raw_input, const std::string &raw_input_with_cases) override")

    ime_header = root / "core/ime_session.h"
    replace_once(
        ime_header,
        "    void replace_active_raw_input(const std::string &raw_input, const std::string &raw_input_with_cases);\n",
        "    void replace_active_raw_input(const std::string &raw_input, const std::string &raw_input_with_cases);\n"
        "    // Decode an independent raw spelling while preserving the live scheme and request state.\n"
        "    std::vector<WordItem> query_raw_candidates(const std::string &raw_input, const std::string &raw_input_with_cases);\n",
        "query_raw_candidates",
    )
    replace_once(
        ime_header,
        "  private:\n    void refresh_candidates();\n",
        "  private:\n    void apply_request_options(QueryRequest &request) const;\n    void refresh_candidates();\n",
        "apply_request_options",
    )
    ime = root / "core/ime_session.cpp"
    replace_once(
        ime,
        "int ImeSession::cache_dynamic_candidate_for_current_request(const std::string &word, CandidateSource source)\n",
        """std::vector<WordItem> ImeSession::query_raw_candidates(const std::string &raw_input,
                                                       const std::string &raw_input_with_cases)
{
    const std::unique_ptr<IInputScheme> query_scheme = create_scheme(scheme_->type());
    query_scheme->set_raw_input(raw_input, raw_input_with_cases);
    QueryRequest request = query_scheme->build_request();
    apply_request_options(request);
    ApplyShuangpinHelpcodeSegmentation(request, shuangpin_profile_);
    if (!request.valid)
        return {};
    std::vector<WordItem> candidates = provider_registry_.resolve(request.scheme).query(request);
    if (scheme_->type() != SchemeType::Wubi || !wubi_options_.mixed_pinyin)
        return candidates;
    QuanpinScheme pinyin;
    pinyin.set_raw_input(raw_input, raw_input_with_cases);
    QueryRequest pinyin_request = pinyin.build_request();
    apply_request_options(pinyin_request);
    pinyin_request.key_strokes = request.key_strokes;
    if (!pinyin_request.valid)
        return candidates;
    const auto pinyin_candidates = provider_registry_.resolve(SchemeType::Quanpin).query(pinyin_request);
    std::unordered_set<std::string> seen_words;
    seen_words.reserve(candidates.size() + pinyin_candidates.size());
    for (const auto &item : candidates)
        seen_words.insert(item.word);
    for (const auto &item : pinyin_candidates)
    {
        if (seen_words.insert(item.word).second)
            candidates.push_back(item);
    }
    return candidates;
}

int ImeSession::cache_dynamic_candidate_for_current_request(const std::string &word, CandidateSource source)
""",
        "query_scheme->set_raw_input",
    )
    old_options = """    state_.request.enable_shuangpin_helpcode = enable_shuangpin_helpcode_;
    state_.request.enable_quanpin_helpcode = enable_quanpin_helpcode_;
    state_.request.sentence_alternatives = sentence_alternatives_;
    state_.request.enable_quanpin_autocorrect_transposition =
        (quanpin_autocorrect_types_ & quanpin::kAutocorrectTransposition) != 0;
    state_.request.enable_quanpin_autocorrect_neighbor =
        (quanpin_autocorrect_types_ & quanpin::kAutocorrectNeighbor) != 0;
    state_.request.fuzzy_pinyin = fuzzy_pinyin_;
"""
    replace_once(ime, old_options, "    apply_request_options(state_.request);\n", "apply_request_options(state_.request)")
    mixed_options = """        fallback.enable_quanpin_helpcode = enable_quanpin_helpcode_;
        fallback.sentence_alternatives = sentence_alternatives_;
        fallback.enable_quanpin_autocorrect_transposition =
            (quanpin_autocorrect_types_ & quanpin::kAutocorrectTransposition) != 0;
        fallback.enable_quanpin_autocorrect_neighbor =
            (quanpin_autocorrect_types_ & quanpin::kAutocorrectNeighbor) != 0;
        fallback.fuzzy_pinyin = fuzzy_pinyin_;
"""
    text = ime.read_text(encoding="utf-8")
    if mixed_options in text:
        ime.write_text(text.replace(mixed_options, "        apply_request_options(fallback);\n", 1), encoding="utf-8")
    elif "        apply_request_options(fallback);\n" not in text and "        apply_request_options(mixed);\n" not in text:
        mixed_options = mixed_options.replace("fallback", "mixed")
        if mixed_options not in text:
            raise RuntimeError(f"Engine caret overlay did not match Wubi fallback options in {ime}")
        ime.write_text(text.replace(mixed_options, "        apply_request_options(mixed);\n", 1), encoding="utf-8")
    anchor = """std::unique_ptr<IInputScheme> ImeSession::create_scheme(SchemeType scheme_type) const
"""
    helper = """void ImeSession::apply_request_options(QueryRequest &request) const
{
    request.enable_shuangpin_helpcode = enable_shuangpin_helpcode_;
    request.enable_quanpin_helpcode = enable_quanpin_helpcode_;
    request.sentence_alternatives = sentence_alternatives_;
    request.enable_quanpin_autocorrect_transposition =
        (quanpin_autocorrect_types_ & quanpin::kAutocorrectTransposition) != 0;
    request.enable_quanpin_autocorrect_neighbor =
        (quanpin_autocorrect_types_ & quanpin::kAutocorrectNeighbor) != 0;
    request.fuzzy_pinyin = fuzzy_pinyin_;
}

"""
    replace_once(ime, anchor, helper + anchor, "void ImeSession::apply_request_options")

    session_header = root / "core/input_session.h"
    replace_once(session_header,
        "    void recompute_candidates();\n",
        "    void recompute_candidates();\n"
        "    void set_caret(std::optional<std::size_t> caret);\n"
        "    std::size_t prefix_end() const;\n"
        "    std::string pending_suffix() const;\n",
        "void set_caret(std::optional<std::size_t> caret)")
    replace_once(session_header,
        "    std::optional<std::size_t> caret_;\n",
        "    std::optional<std::size_t> caret_;\n"
        "    std::vector<WordItem> prefix_candidates_;\n"
        "    std::string prefix_query_input_;\n"
        "    bool prefix_candidates_active_ = false;\n",
        "prefix_candidates_active_ = false")
    replace_once(session_header,
        "    void update_mixed_candidates();\n",
        "    void update_mixed_candidates();\n"
        "    std::size_t quantized_prefix_end() const;\n"
        "    void refresh_prefix_candidates();\n",
        "void refresh_prefix_candidates")

    session = root / "core/input_session.cpp"
    replace_once(session,
        """    if (fixed_positions_enabled_ ||
        ((english_input_options_.mixed_candidates || mixed_expressive_options_.emoji_candidates ||
          mixed_expressive_options_.kaomoji_candidates) &&
         (scheme() == SchemeType::Quanpin || scheme() == SchemeType::Shuangpin)))
    {
        return mixed_candidates_;
    }
""",
        """    if (prefix_candidates_active_)
    {
        return prefix_candidates_;
    }
    if (fixed_positions_enabled_ ||
        ((english_input_options_.mixed_candidates || mixed_expressive_options_.emoji_candidates ||
          mixed_expressive_options_.kaomoji_candidates) &&
         (scheme() == SchemeType::Quanpin || scheme() == SchemeType::Shuangpin)))
    {
        return mixed_candidates_;
    }
""",
        "if (prefix_candidates_active_)")
    old_mixed = """void InputSession::update_mixed_candidates()
{
    mixed_candidates_ = candidate_queries_.mixed(engine_.get_candidates(), engine_.get_request().raw_input, scheme(),
                                                 english_input_options_, mixed_expressive_options_,
                                                 dedicated_english_mode_, local_input_mode_);
    apply_candidate_positions(mixed_candidates_);
}
"""
    new_mixed = """void InputSession::update_mixed_candidates()
{
    refresh_prefix_candidates();
    const auto &decoded = prefix_candidates_active_ ? prefix_candidates_ : engine_.get_candidates();
    const std::string association_input =
        prefix_candidates_active_ ? prefix_query_input_ : engine_.get_request().raw_input;
    mixed_candidates_ = candidate_queries_.mixed(decoded, association_input, scheme(), english_input_options_,
                                                 mixed_expressive_options_, dedicated_english_mode_, local_input_mode_);
    apply_candidate_positions(mixed_candidates_);
}

void InputSession::set_caret(std::optional<std::size_t> caret)
{
    if (caret.has_value())
        *caret = std::min(*caret, editing_text().size());
    caret_ = caret;
}

std::size_t InputSession::quantized_prefix_end() const
{
    const std::string &raw = get_pinyin_sequence_with_cases();
    if (!caret_.has_value())
        return raw.size();
    const auto boundaries = segment_raw_boundaries();
    if (boundaries.empty())
        return raw.size();
    return *std::prev(std::upper_bound(boundaries.begin(), boundaries.end(), caret_position()));
}

std::size_t InputSession::prefix_end() const
{
    return quantized_prefix_end();
}

std::string InputSession::pending_suffix() const
{
    const std::string &raw = get_pinyin_sequence_with_cases();
    const std::size_t end = quantized_prefix_end();
    return end < raw.size() ? raw.substr(end) : std::string{};
}

void InputSession::refresh_prefix_candidates()
{
    prefix_candidates_active_ = false;
    if (!caret_.has_value() || dedicated_english_mode_ || local_input_mode_ != LocalInputMode::None)
    {
        prefix_candidates_.clear();
        prefix_query_input_.clear();
        return;
    }
    const std::string &raw_with_cases = get_pinyin_sequence_with_cases();
    const std::size_t end = quantized_prefix_end();
    if (end >= raw_with_cases.size())
    {
        prefix_candidates_.clear();
        prefix_query_input_.clear();
        return;
    }
    std::string prefix = raw_with_cases.substr(0, end);
    std::transform(prefix.begin(), prefix.end(), prefix.begin(),
                   [](unsigned char character) { return static_cast<char>(std::tolower(character)); });
    if (prefix_query_input_ != prefix)
    {
        prefix_candidates_ = engine_.query_raw_candidates(prefix, raw_with_cases.substr(0, end));
        prefix_query_input_ = prefix;
    }
    prefix_candidates_active_ = true;
}
"""
    replace_once(session, old_mixed, new_mixed, "void InputSession::refresh_prefix_candidates")
    editing = root / "core/input_session_editing.cpp"
    replace_once(editing,
        """    caret_ = caret;
    return {true, std::nullopt, std::nullopt};
""",
        """    caret_ = caret;
    update_mixed_candidates();
    return {true, std::nullopt, std::nullopt};
""",
        "update_mixed_candidates();\n    return {true, std::nullopt, std::nullopt};")
    replace_once(editing,
        """    caret_ = caret;
    online_requests_.invalidate();
""",
        """    caret_ = caret;
    update_mixed_candidates();
    online_requests_.invalidate();
""",
        "update_mixed_candidates();\n    online_requests_.invalidate")
    replace_once(session,
        """void InputSession::reset_composition()
{
    caret_.reset();
""",
        """void InputSession::reset_composition()
{
    caret_.reset();
    prefix_candidates_.clear();
    prefix_query_input_.clear();
    prefix_candidates_active_ = false;
""",
        "prefix_candidates_active_ = false")
    composition = root / "core/input_session_composition.cpp"
    replace_once(composition,
        """    if (canonical_phrase_engine_)
        canonical_phrase_engine_->reset_cache();
""",
        """    if (canonical_phrase_engine_)
        canonical_phrase_engine_->reset_cache();
    prefix_query_input_.clear();
""",
        "prefix_query_input_.clear")


if __name__ == "__main__":
    import sys
    apply(Path(sys.argv[1]) if len(sys.argv) > 1 else Path(__file__).resolve().parents[1] / "vendor/MSIME-Engine")
