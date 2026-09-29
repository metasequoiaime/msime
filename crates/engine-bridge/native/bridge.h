#pragma once
#include "rust/cxx.h"
#include <memory>
#include <cstdint>
#include <metasequoia/session.h>
#include "../../vendor/MSIME-Engine/common/helpcode_utils.h"

namespace msime {
rust::Vec<rust::String> handwriting_order_candidates(rust::Slice<const rust::String> candidates);
struct EngineOptions;
struct DictionaryRevision;
struct DictionaryRecordStream;
EngineOptions stage_dictionary_state(const EngineOptions& options, rust::Str generation,
    rust::Str content_id, std::size_t maximum_records, DictionaryRecordStream& stream);
void hash_dictionary_state(const EngineOptions& options, DictionaryRevision& sink);
struct EngineSnapshot;
struct EngineResult;
struct DictionaryReplaySummary;
struct OnlineQuerySnapshot;
struct EmojiCatalogItem;
struct EmojiCatalogSlice;
struct EmojiSymbolGroup;
struct CandidateGlossInput;
struct HandwritingPoint;
struct DictionaryEntry;
struct DictionaryPage;
struct DictionaryTablePage;
struct ShuangpinKeyHint;
class EngineSession {
public:
    explicit EngineSession(const EngineOptions& options);
    EngineSnapshot snapshot() const;
    OnlineQuerySnapshot online_query() const;
    void reset_cache();
    // u64::MAX clears the explicit caret and restores end-of-composition decoding.
    void set_caret(std::uint64_t caret);
    std::size_t prefix_end() const;
    rust::String pending_suffix() const;
    bool apply_online_candidate(const OnlineQuerySnapshot& query, rust::Str candidate,
                                std::uint8_t source);
    bool apply_online_candidates(const OnlineQuerySnapshot& query, rust::Slice<const rust::String> candidates,
                                std::uint8_t source);
    EngineResult character(std::uint8_t value, bool shift);
    bool expand_initial_candidates();
    void set_nine_key_enabled(bool enabled);
    EngineResult choose_nine_key_spelling(std::size_t index);
    EngineResult command(std::uint8_t value);
    EngineResult commit_raw_with_policy();
    EngineResult commit_raw_without_learning();
    EngineResult select(std::size_t index);
    EngineResult pin_candidate(std::size_t index);
    EngineResult remove_candidate(std::size_t index);
    EngineResult fix_candidate_position(std::size_t index, std::uint8_t position);
    EngineResult clear_candidate_position(std::size_t index);
    EngineResult select_edge(std::size_t index, std::uint8_t edge);
    EngineResult finish(std::size_t index);
    EngineResult punctuation(std::uint8_t value);
    void balance_paired_punctuation_after_auto_close(std::uint8_t opening);
    void set_chinese_punctuation_enabled(bool enabled);
    void set_punctuation_lock(std::uint8_t lock);
    void set_paired_punctuation_enabled(bool enabled);
    void set_dedicated_english(bool enabled);
private:
    metasequoia::Session session_;
    metasequoia::RuntimePaths paths_;
    bool nine_key_ = false;
    bool microsoft_shuangpin_;
    std::string shuangpin_profile_;
    HelpcodeUtils::SharedKeymap helpcode_keymap_;
    bool helpcode_enabled_ = false;
    bool show_helpcode_ = true;
};
std::unique_ptr<EngineSession> create_session(const EngineOptions& options);
rust::Vec<float> capture_audio(std::uint32_t milliseconds);
struct CaptureDevice;
rust::Vec<CaptureDevice> capture_devices();
EngineOptions prepare_options(rust::Str resources, rust::Str user_data, rust::Str cache, rust::Str content_id);
rust::String hanzi_to_pinyin(const EngineOptions& options, rust::Str text);
rust::String normalize_full_pinyin(rust::Str input, std::size_t expected_syllables);
rust::Vec<ShuangpinKeyHint> shuangpin_key_hints(rust::Str profile);
DictionaryPage dictionary_entries(const EngineOptions& options, std::size_t offset, std::size_t limit);
DictionaryPage dictionary_export_entries(const EngineOptions& options, std::size_t offset, std::size_t limit,
                                         bool include_learned_pinyin);
DictionaryTablePage dictionary_table_entries(const EngineOptions& options, std::uint8_t kind, rust::Str query,
                                             std::size_t offset, std::size_t limit);
void dictionary_edit_bundled(const EngineOptions& options, const DictionaryEntry& previous,
                             rust::Slice<const std::int64_t> weight, rust::Str request_id);
rust::Vec<rust::String> english_completions(rust::Str resources, rust::Str prefix, std::size_t limit);
DictionaryEntry dictionary_validate(const DictionaryEntry& entry);
void dictionary_edit(const EngineOptions& options, rust::Slice<const DictionaryEntry> previous,
                     rust::Slice<const DictionaryEntry> replacement, rust::Str request_id);
void reset_learned_data(const EngineOptions& options);
DictionaryReplaySummary replay_user_dictionary(rust::Str user_db_path, rust::Str main_db_path,
                                                rust::Str english_db_path);
rust::Vec<EmojiCatalogItem> emoji_catalog_filtered_page(rust::Str resources, rust::Str search,
    rust::Str category, rust::Str group, std::size_t offset, std::uint16_t limit, rust::Str parent);
EmojiCatalogSlice emoji_catalog_slice(rust::Str resources, rust::Str search,
    rust::Str category, rust::Str group, std::size_t offset, std::uint16_t limit, rust::Str parent);
rust::Vec<EmojiSymbolGroup> emoji_symbol_groups(rust::Str resources);
rust::Vec<rust::String> emoji_catalog_groups(rust::Str resources, rust::Str category);
rust::Vec<rust::String> candidate_glosses(
    rust::Str resources, rust::Slice<const CandidateGlossInput> candidates);
rust::Vec<rust::String> candidate_glosses_with_user(
    rust::Str resources, rust::Str user_data, rust::Slice<const CandidateGlossInput> candidates);
rust::Vec<rust::String> candidate_target_glosses(
    rust::Str database_path, rust::Str target_language, rust::Slice<const CandidateGlossInput> candidates);
bool save_candidate_gloss(rust::Str user_data, bool chinese_to_english, rust::Str key, rust::Str gloss);
#if !defined(__ANDROID__)
rust::Vec<rust::String> handwriting_recognize(rust::Str model_path,
                                               rust::Slice<const HandwritingPoint> points,
                                               float width, float height);
#endif
}
