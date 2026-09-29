#if __has_include(<metasequoia/handwriting_candidates.h>)
#include <metasequoia/handwriting_candidates.h>
#define MSIME_HAS_HANDWRITING_CANDIDATES 1
#else
#define MSIME_HAS_HANDWRITING_CANDIDATES 0
#endif
#include "bridge.h"
#ifndef MSIME_ENGINE_BRIDGE_AUDIO_CAPTURE
#define MSIME_ENGINE_BRIDGE_AUDIO_CAPTURE 1
#endif
#if MSIME_ENGINE_BRIDGE_AUDIO_CAPTURE
#include <msime/voice/audio_capture.h>
#include "miniaudio.h"
#endif
#include "msime-engine-bridge/src/lib.rs.h"
#include <metasequoia/personal_dictionary.h>
#include <user_dictionary/user_dictionary_journal.h>
#if !defined(__ANDROID__) && MSIME_HAS_HANDWRITING_CANDIDATES
#include <metasequoia/handwriting.h>
#endif
#include <algorithm>
#include <chrono>
#include <cctype>
#include <cstdint>
#include <metasequoia/dictionary_state.h>
#include <stdexcept>
#include <type_traits>
#include <limits>
#include <map>
#include <mutex>
#include <condition_variable>
#include <filesystem>
#include "../../vendor/MSIME-Engine/contracts/assets/assets.h"
#include "../../vendor/MSIME-Engine/english/english_dictionary.h"
#include "../../vendor/MSIME-Engine/quanpin/quanpin_query.h"
#include "../../vendor/MSIME-Engine/quanpin/quanpin_utils.h"
#include <sqlite3.h>
#include <unordered_map>
#include <unordered_set>
#include <memory>
#include <string_view>
#include <vector>

namespace msime {

rust::Vec<float> capture_audio(std::uint32_t milliseconds) {
    rust::Vec<float> samples;
#if !MSIME_ENGINE_BRIDGE_AUDIO_CAPTURE
    (void)milliseconds;
    return samples;
#else
    if (milliseconds == 0 || milliseconds > 60000) return samples;
    metasequoia::voice::AudioCapture capture;
    std::mutex mutex;
    std::condition_variable done;
    bool failed = false;
    std::size_t maximum = 16000u * milliseconds / 1000u;
    samples.reserve(maximum);
    auto callback = [&](const float *input, std::size_t frames) {
        std::lock_guard lock(mutex);
        if (samples.size() + frames > maximum) frames = maximum - samples.size();
        for (std::size_t index = 0; index < frames; ++index) {
            samples.push_back(input[index]);
        }
        if (samples.size() >= maximum) done.notify_one();
    };
    if (!capture.start(callback)) return samples;
    std::unique_lock lock(mutex);
    done.wait_for(lock, std::chrono::milliseconds(milliseconds), [&] {
        return samples.size() >= maximum;
    });
    // AudioCapture::stop waits for the callback thread. Do not hold the
    // sample mutex while stopping: the callback may be waiting on this mutex
    // to finish its final delivery.
    lock.unlock();
    capture.stop();
    failed = capture.callback_failed();
    if (failed) samples.clear();
    return samples;
#endif
}

rust::Vec<CaptureDevice> capture_devices() {
    rust::Vec<CaptureDevice> devices;
#if MSIME_ENGINE_BRIDGE_AUDIO_CAPTURE
    const auto available = metasequoia::voice::AudioCapture::devices();
    devices.reserve(available.size());
    for (const auto &device : available) {
        CaptureDevice value;
        value.id = device.id;
        value.label = device.label;
        devices.push_back(std::move(value));
    }
#endif
    return devices;
}
rust::Vec<rust::String> handwriting_order_candidates(rust::Slice<const rust::String> candidates) {
    std::vector<std::string> input;
    input.reserve(candidates.size());
    for (const auto &candidate : candidates) input.emplace_back(std::string(candidate));
    rust::Vec<rust::String> output;
    output.reserve(candidates.size());
#if MSIME_HAS_HANDWRITING_CANDIDATES
    for (const auto &candidate : metasequoia::handwriting::order_candidates(input))
        output.push_back(rust::String(candidate));
#else
    for (const auto &candidate : input) output.push_back(rust::String(candidate));
#endif
    return output;
}

namespace {
metasequoia::RuntimePaths paths_for(const EngineOptions& value) {
    return {std::filesystem::u8path(std::string(value.resources)),
            std::filesystem::u8path(std::string(value.user_data)),
            std::filesystem::u8path(std::string(value.cache)),
            std::filesystem::u8path(std::string(value.dictionaries))};
}
bool next_utf8(const std::string& text, std::size_t& offset, std::string& character,
               std::uint32_t& codepoint) {
    if (offset >= text.size()) return false;
    const auto first = static_cast<unsigned char>(text[offset]);
    std::size_t width = 0;
    if (first <= 0x7f) width = 1;
    else if (first >= 0xc2 && first <= 0xdf) width = 2;
    else if (first >= 0xe0 && first <= 0xef) width = 3;
    else if (first >= 0xf0 && first <= 0xf4) width = 4;
    else return false;
    if (offset + width > text.size()) return false;
    codepoint = first & (width == 1 ? 0x7f : width == 2 ? 0x1f : width == 3 ? 0x0f : 0x07);
    for (std::size_t index = 1; index < width; ++index) {
        const auto byte = static_cast<unsigned char>(text[offset + index]);
        if ((byte & 0xc0) != 0x80) return false;
        codepoint = (codepoint << 6) | (byte & 0x3f);
    }
    if ((width == 3 && codepoint < 0x800) || (width == 4 && codepoint < 0x10000) ||
        (codepoint >= 0xd800 && codepoint <= 0xdfff) || codepoint > 0x10ffff)
        return false;
    character.assign(text, offset, width);
    offset += width;
    return true;
}
bool is_han(std::uint32_t codepoint) {
    return (codepoint >= 0x3400 && codepoint <= 0x4dbf) ||
           (codepoint >= 0x4e00 && codepoint <= 0x9fff) ||
           (codepoint >= 0xf900 && codepoint <= 0xfaff) ||
           (codepoint >= 0x20000 && codepoint <= 0x2fa1f);
}
bool candidate_gloss_key(const CandidateGlossInput& candidate, std::string& key,
                         bool& chinese_to_english) {
    if (candidate.source == static_cast<std::uint8_t>(CandidateSource::Emoji) ||
        candidate.source == static_cast<std::uint8_t>(CandidateSource::Kaomoji))
        return false;
    const std::string text(candidate.text);
    bool has_ascii_letter = false;
    key.clear();
    key.reserve(text.size());
    for (const unsigned char ch : text) {
        if (ch >= 'A' && ch <= 'Z') {
            key.push_back(static_cast<char>(ch + ('a' - 'A')));
            has_ascii_letter = true;
        } else if (ch >= 'a' && ch <= 'z') {
            key.push_back(static_cast<char>(ch));
            has_ascii_letter = true;
        } else if (ch == ' ' || ch == '-' || ch == '\'') {
            key.push_back(static_cast<char>(ch));
        } else {
            has_ascii_letter = false;
            break;
        }
    }
    if (has_ascii_letter) {
        chinese_to_english = false;
        return true;
    }
    std::size_t offset = 0;
    while (offset < text.size()) {
        std::string character;
        std::uint32_t codepoint = 0;
        if (!next_utf8(text, offset, character, codepoint)) return false;
        if (is_han(codepoint)) {
            chinese_to_english = true;
            key = text;
            return true;
        }
    }
    return false;
}
std::string collapse_ascii_whitespace(std::string_view text) {
    std::string output;
    output.reserve(text.size());
    bool pending_space = false;
    for (const unsigned char ch : text) {
        if (ch == ' ' || ch == '\t' || ch == '\r' || ch == '\n') {
            pending_space = !output.empty();
            continue;
        }
        if (pending_space) {
            output.push_back(' ');
            pending_space = false;
        }
        output.push_back(static_cast<char>(ch));
    }
    return output;
}
std::string candidate_gloss_display(const std::string& text) {
    std::string output;
    std::size_t begin = 0;
    std::size_t count = 0;
    constexpr std::string_view fullwidth_delimiter = "；";
    while (begin <= text.size() && count < 2) {
        const auto ascii = text.find(';', begin);
        const auto fullwidth = text.find(fullwidth_delimiter, begin);
        const bool use_fullwidth =
            fullwidth != std::string::npos && (ascii == std::string::npos || fullwidth < ascii);
        const auto end = use_fullwidth ? fullwidth : ascii;
        const auto stop = end == std::string::npos ? text.size() : end;
        auto sense = collapse_ascii_whitespace(
            std::string_view(text).substr(begin, stop - begin));
        if (!sense.empty()) {
            if (!output.empty()) output += "; ";
            output += sense;
            ++count;
        }
        if (end == std::string::npos) break;
        begin = end + (use_fullwidth ? fullwidth_delimiter.size() : 1);
    }
    std::size_t offset = 0;
    while (offset < output.size()) {
        std::string character;
        std::uint32_t codepoint = 0;
        if (!next_utf8(output, offset, character, codepoint) || codepoint < 0x20 ||
            (codepoint >= 0x7f && codepoint <= 0x9f))
            return {};
    }
    return output;
}
using HanziReadings = std::unordered_map<std::string, std::string>;
HanziReadings single_hanzi_map(sqlite3* database) {
    std::unordered_map<std::string, std::string> result;
    for (char initial = 'a'; initial <= 'z'; ++initial) {
        const auto table = std::string("tbl_1_") + initial;
        const auto sql = "SELECT \"key\", \"value\" FROM \"" + table +
                         "\" ORDER BY \"weight\" DESC, \"key\" ASC";
        sqlite3_stmt* statement = nullptr;
        if (sqlite3_prepare_v2(database, sql.c_str(), -1, &statement, nullptr) != SQLITE_OK)
            continue;
        while (sqlite3_step(statement) == SQLITE_ROW) {
            const auto* key = reinterpret_cast<const char*>(sqlite3_column_text(statement, 0));
            const auto* value = reinterpret_cast<const char*>(sqlite3_column_text(statement, 1));
            if (key && value && result.find(value) == result.end()) result.emplace(value, key);
        }
        sqlite3_finalize(statement);
    }
    return result;
}
/// `single_hanzi_map` for a dictionary file, built once instead of once per call.
///
/// The scan behind it walks all 26 single-character tables — 19,637 rows on the shipped dictionary
/// — sorts each by weight with no index to sort by, and builds a map of every reading, which the
/// caller then copies. Paying that per call is why `hanzi_to_pinyin` costs milliseconds, and
/// `msime_client_dictionary_validate` calls it in a loop over as many as a thousand words.
///
/// Caching is safe because the file cannot change under a running process: the dictionary is
/// read-only, its bytes are pinned by sha256 in `desktop-dictionary.lock.json`, and an update is
/// installed by quiescing the Server first. Keyed by path, because two sessions may legitimately
/// be pointed at different dictionaries and one cache serving the other's readings would be
/// silently wrong rather than slow.
///
/// Built outside the lock: two threads arriving together may both build it, and the first to
/// finish wins, which costs one redundant scan at worst and never blocks a keystroke behind one.
std::shared_ptr<const HanziReadings> cached_single_hanzi_map(sqlite3* database,
                                                             const std::string& path) {
    static std::mutex guard;
    static std::unordered_map<std::string, std::shared_ptr<const HanziReadings>> cache;
    {
        const std::lock_guard<std::mutex> lock(guard);
        const auto found = cache.find(path);
        if (found != cache.end()) return found->second;
    }
    auto built = std::make_shared<const HanziReadings>(single_hanzi_map(database));
    const std::lock_guard<std::mutex> lock(guard);
    return cache.emplace(path, std::move(built)).first->second;
}
std::string exact_hanzi_pinyin(sqlite3* database, const std::string& word, std::size_t length) {
    for (char initial = 'a'; initial <= 'z'; ++initial) {
        const auto table = "tbl_" + std::to_string(length) + "_" + initial;
        const auto sql = "SELECT \"key\" FROM \"" + table +
                         "\" WHERE \"value\"=?1 ORDER BY \"weight\" DESC, \"key\" ASC LIMIT 1";
        sqlite3_stmt* statement = nullptr;
        if (sqlite3_prepare_v2(database, sql.c_str(), -1, &statement, nullptr) != SQLITE_OK)
            continue;
        sqlite3_bind_text(statement, 1, word.c_str(), static_cast<int>(word.size()), SQLITE_TRANSIENT);
        if (sqlite3_step(statement) == SQLITE_ROW) {
            const auto* key = reinterpret_cast<const char*>(sqlite3_column_text(statement, 0));
            if (key) {
                const std::string result(key);
                sqlite3_finalize(statement);
                return result;
            }
        }
        sqlite3_finalize(statement);
    }
    return {};
}
void prepare_translation_sidecar(const EngineOptions& value) {
    const auto paths = paths_for(value);
    const auto name = std::filesystem::path("custom_translations.txt");
    const auto target = paths.dictionary(name);
    auto source = paths.user(name);
    std::error_code error;
    if (!std::filesystem::is_regular_file(source, error)) {
        error.clear();
        source = paths.resource(name);
    }
    if (std::filesystem::is_regular_file(source, error)) {
        error.clear();
        std::filesystem::create_directories(target.parent_path(), error);
        if (!error)
            std::filesystem::copy_file(source, target,
                                       std::filesystem::copy_options::overwrite_existing,
                                       error);
        if (error)
            throw std::runtime_error("Unable to prepare custom translation sidecar");
    } else {
        error.clear();
        std::filesystem::remove(target, error);
    }
}
metasequoia::PersonalDictionaryEntry entry_for(const DictionaryEntry& value) {
    using Kind = metasequoia::PersonalDictionaryKind;
    Kind kind;
    switch (value.kind) {
        case DictionaryKind::Pinyin: kind = Kind::Pinyin; break;
        case DictionaryKind::Wubi: kind = Kind::Wubi; break;
        case DictionaryKind::QuickPhrase: kind = Kind::QuickPhrase; break;
        case DictionaryKind::English: kind = Kind::English; break;
        default: throw std::invalid_argument("Unsupported dictionary kind");
    }
    return {kind, std::string(value.key), std::string(value.value), value.weight};
}
DictionaryEntry entry_for(const metasequoia::PersonalDictionaryEntry& value) {
    using Kind = metasequoia::PersonalDictionaryKind;
    DictionaryKind kind;
    switch (value.kind) {
        case Kind::Pinyin: kind = DictionaryKind::Pinyin; break;
        case Kind::Wubi: kind = DictionaryKind::Wubi; break;
        case Kind::QuickPhrase: kind = DictionaryKind::QuickPhrase; break;
        case Kind::English: kind = DictionaryKind::English; break;
        default: throw std::invalid_argument("Unsupported dictionary kind");
    }
    return {kind, value.key, value.value, value.weight};
}
metasequoia::SessionOptions options_for(const EngineOptions& value) {
    prepare_translation_sidecar(value);
    metasequoia::SessionOptions options;
    options.paths = paths_for(value);
    switch (value.scheme) {
        case 0: options.scheme = SchemeType::Quanpin; break;
        case 1: options.scheme = SchemeType::Shuangpin; break;
        case 2: options.scheme = SchemeType::Wubi; break;
        case 3: options.scheme = SchemeType::JapaneseRomaji; break;
        default: throw std::invalid_argument("Unsupported input scheme");
    }
    switch (value.shuangpin_profile) {
        case 0: options.shuangpin_profile = GetXiaoheShuangpinProfile(); break;
        case 1: options.shuangpin_profile = GetZiranmaShuangpinProfile(); break;
        case 2: options.shuangpin_profile = GetShoudaoShuangpinProfile(); break;
        case 3: options.shuangpin_profile = GetMicrosoftShuangpinProfile(); break;
        default: throw std::invalid_argument("Unsupported shuangpin profile");
    }
    options.shuangpin_preedit_uses_raw = value.shuangpin_preedit_uses_raw;
    options.learning = value.learning;
    options.autocorrect_types =
        (value.autocorrect_transposition ? quanpin::kAutocorrectTransposition : 0u) |
        (value.autocorrect_neighbor ? quanpin::kAutocorrectNeighbor : 0u);
    options.fuzzy_pinyin.rules = value.fuzzy_pinyin_rules & 0x7ffu;
    options.wubi.mixed_pinyin = value.wubi_mixed_pinyin;
    options.chinese_punctuation = value.chinese_punctuation;
    options.paired_punctuation = value.paired_punctuation;
    options.punctuation_lock = value.punctuation_lock;
    options.helpcode = value.helpcode;
    options.helpcode_schema = std::string(value.helpcode_schema);
    const std::string frequency(value.frequency_mode);
    using metasequoia::FrequencyAdjustmentMode;
    if (frequency == "disabled") options.frequency.mode = FrequencyAdjustmentMode::Disabled;
    else if (frequency == "pin") options.frequency.mode = FrequencyAdjustmentMode::Pin;
    else if (frequency == "halve") options.frequency.mode = FrequencyAdjustmentMode::Halve;
    else if (frequency == "linear") options.frequency.mode = FrequencyAdjustmentMode::Linear;
    else if (frequency == "promote") options.frequency.mode = FrequencyAdjustmentMode::Promote;
    else throw std::invalid_argument("Unsupported frequency mode");
    options.frequency.trigger_count = value.frequency_trigger_count;
    options.frequency.linear_step = value.frequency_linear_step;
    options.english = {value.mixed_english, value.english_minimum_prefix};
    options.expressive = {value.mixed_emoji, value.mixed_kaomoji};
    options.local_modes = {value.local_unicode, value.local_date_time, value.local_quick_phrase, value.local_emoji,
                           value.local_kaomoji, value.local_super_jianpin, value.local_temporary_english,
                           value.local_temporary_japanese};
    options.sentence_alternatives = value.sentence_alternatives;
    options.sentence_association = {value.sentence_association.word_lattice,
                                    value.sentence_association.google,
                                    value.sentence_association.neural_desktop,
                                    value.sentence_association.neural_keyboard,
                                    value.sentence_association.show_next_on_duplicate};
    options.rescoring_context = std::string(value.rescoring_context);
    return options;
}
EngineResult result_for(const metasequoia::KeyResult& value) {
    return {value.handled, value.commit.has_value(), value.commit.value_or(""), value.diagnostic.value_or("")};
}
const char* local_mode_name(metasequoia::LocalInputMode mode) {
    using metasequoia::LocalInputMode;
    switch (mode) {
        case LocalInputMode::None: return "none";
        case LocalInputMode::Unicode: return "unicode";
        case LocalInputMode::DateTime: return "date_time";
        case LocalInputMode::QuickPhrase: return "quick_phrase";
        case LocalInputMode::Emoji: return "emoji";
        case LocalInputMode::Kaomoji: return "kaomoji";
        case LocalInputMode::SuperJianpin: return "super_jianpin";
        case LocalInputMode::TemporaryEnglish: return "temporary_english";
        case LocalInputMode::TemporaryJapanese: return "temporary_japanese";
    }
    throw std::logic_error("Unknown Engine local mode");
}
}
EngineSession::EngineSession(const EngineOptions& options) : session_(options_for(options)),
    paths_(paths_for(options)),
    microsoft_shuangpin_(options.scheme == 1 && options.shuangpin_profile == 3),
    shuangpin_profile_(options_for(options).shuangpin_profile.name),
    helpcode_keymap_(options.helpcode
                         ? HelpcodeUtils::load_helpcode_keymap(
                               std::filesystem::u8path(std::string(options.resources)),
                               std::string(options.helpcode_schema))
                         : nullptr),
    helpcode_enabled_(options.helpcode), show_helpcode_(options.show_helpcode) {}
std::unique_ptr<EngineSession> create_session(const EngineOptions& options) {
    return std::make_unique<EngineSession>(options);
}
// Framing matches Apple DictionaryStateRevision at 2b0250f4dd7012520392b310dfcc0288c3208a75.
void hash_dictionary_state(const EngineOptions& options, DictionaryRevision& sink) {
    metasequoia::stream_dictionary_state(paths_for(options), [&](const metasequoia::DictionaryStateRecord& record) {
        std::visit([&](const auto& value) {
            using T = std::decay_t<decltype(value)>;
            if constexpr (std::is_same_v<T, metasequoia::DictionaryStateEntry>) {
                sink.text("entry");
                switch (value.kind) {
                case metasequoia::PersonalDictionaryKind::Pinyin: sink.text("pinyin"); break;
                case metasequoia::PersonalDictionaryKind::Wubi: sink.text("wubi"); break;
                case metasequoia::PersonalDictionaryKind::QuickPhrase: sink.text("quick"); break;
                case metasequoia::PersonalDictionaryKind::English: sink.text("english"); break;
                }
                sink.text(value.key); sink.text(value.value);
                sink.integer(static_cast<std::uint64_t>(value.weight));
                sink.text(value.display); sink.integer(value.deleted); sink.integer(value.user_inserted);
            } else {
                if constexpr (std::is_same_v<T, metasequoia::DictionaryStatePosition>) sink.text("position");
                else sink.text("selection");
                sink.text(value.context); sink.text(value.key); sink.text(value.value);
                if constexpr (std::is_same_v<T, metasequoia::DictionaryStatePosition>) sink.integer(value.position);
                else sink.integer(value.count);
            }
        }, record);
        return true;
    });
}
EngineOptions stage_dictionary_state(const EngineOptions& options, rust::Str generation,
    rust::Str content_id, std::size_t maximum_records, DictionaryRecordStream& stream) {
    if (maximum_records == 0) throw std::invalid_argument("Invalid snapshot record limit");
    const auto paths = metasequoia::stage_dictionary_state(
        std::filesystem::u8path(std::string(options.resources)),
        std::filesystem::u8path(std::string(generation)), std::string(content_id),
        [&](metasequoia::DictionaryStateRecord& output) {
            const auto value = stream.next(); // Transport failure throws; only verified EOF returns false.
            if (value.record_type == 0) return false;
            if (value.record_type == 1) {
                using Kind = metasequoia::PersonalDictionaryKind;
                Kind kind;
                switch (value.kind) {
                case DictionaryKind::Pinyin: kind = Kind::Pinyin; break;
                case DictionaryKind::Wubi: kind = Kind::Wubi; break;
                case DictionaryKind::QuickPhrase: kind = Kind::QuickPhrase; break;
                case DictionaryKind::English: kind = Kind::English; break;
                default: throw std::invalid_argument("Invalid snapshot dictionary kind");
                }
                output = metasequoia::DictionaryStateEntry{kind, std::string(value.key),
                    std::string(value.value), value.number, std::string(value.display),
                    value.deleted, value.user_inserted};
            } else if (value.record_type == 2) {
                if (value.number < std::numeric_limits<int>::min() || value.number > std::numeric_limits<int>::max())
                    throw std::invalid_argument("Invalid snapshot position");
                output = metasequoia::DictionaryStatePosition{std::string(value.context),
                    std::string(value.key), std::string(value.value), static_cast<int>(value.number)};
            } else if (value.record_type == 3) {
                if (value.number < std::numeric_limits<int>::min() || value.number > std::numeric_limits<int>::max())
                    throw std::invalid_argument("Invalid snapshot selection count");
                output = metasequoia::DictionaryStateSelection{std::string(value.context),
                    std::string(value.key), std::string(value.value), static_cast<int>(value.number)};
            } else throw std::invalid_argument("Invalid snapshot record type");
            return true;
        }, maximum_records);
    auto result = options;
    result.resources = paths.resources.u8string();
    result.user_data = paths.user_data.u8string();
    result.cache = paths.cache.u8string();
    result.dictionaries = paths.dictionaries.u8string();
    return result;
}
DictionaryPage dictionary_entries(const EngineOptions& options, std::size_t offset, std::size_t limit) {
    auto page = metasequoia::personal_dictionary_entries(paths_for(options), offset, limit);
    if (!page.error.empty()) throw std::runtime_error(page.error);
    DictionaryPage result;
    result.has_more = page.has_more;
    for (const auto& entry : page.entries) result.entries.push_back(entry_for(entry));
    return result;
}
DictionaryPage dictionary_export_entries(const EngineOptions& options, std::size_t offset, std::size_t limit,
                                         bool include_learned_pinyin) {
    auto page = metasequoia::personal_dictionary_entries(paths_for(options), offset, limit, include_learned_pinyin);
    if (!page.error.empty()) throw std::runtime_error(page.error);
    DictionaryPage result;
    result.has_more = page.has_more;
    for (const auto& entry : page.entries) result.entries.push_back(entry_for(entry));
    return result;
}
DictionaryTablePage dictionary_table_entries(const EngineOptions& options, std::uint8_t kind, rust::Str query,
                                             std::size_t offset, std::size_t limit) {
    using Kind = metasequoia::PersonalDictionaryKind;
    Kind engine_kind;
    switch (kind) {
        case 0: engine_kind = Kind::Pinyin; break;
        case 1: engine_kind = Kind::Wubi; break;
        case 2: engine_kind = Kind::QuickPhrase; break;
        case 3: engine_kind = Kind::English; break;
        default: throw std::invalid_argument("Unsupported dictionary kind");
    }
    auto page = metasequoia::dictionary_table_entries(paths_for(options), engine_kind, std::string(query), offset,
                                                      limit);
    if (!page.error.empty()) throw std::runtime_error(page.error);
    DictionaryTablePage result;
    result.has_more = page.has_more;
    for (const auto& row : page.entries) result.entries.push_back({entry_for(row.entry), row.user_inserted});
    return result;
}
void dictionary_edit_bundled(const EngineOptions& options, const DictionaryEntry& previous,
                             rust::Slice<const std::int64_t> weight, rust::Str request_id) {
    if (weight.size() > 1) throw std::invalid_argument("Expected at most one weight");
    std::optional<std::int64_t> target;
    if (!weight.empty()) target = weight[0];
    auto result = metasequoia::edit_bundled_dictionary_entry(paths_for(options), entry_for(previous), target,
                                                             std::string(request_id));
    if (!result.success) throw std::runtime_error(result.error);
}
rust::Vec<rust::String> english_completions(rust::Str resources, rust::Str prefix, std::size_t limit) {
    if (limit == 0 || limit > 32) throw std::invalid_argument("Invalid English completion limit");
    std::string lowered(prefix);
    for (char &character : lowered) {
        const auto value = static_cast<unsigned char>(character);
        if (value >= 'A' && value <= 'Z') character = static_cast<char>(value + ('a' - 'A'));
        else if (value < 'a' || value > 'z') throw std::invalid_argument("Invalid English completion prefix");
    }
    if (lowered.empty()) return {};
    const auto path = std::filesystem::u8path(std::string(resources)) /
                      metasequoia::assets::english_dictionary;
    EnglishDictionary dictionary(path.u8string(), false);
    if (!dictionary.ready()) throw std::runtime_error("English dictionary unavailable");
    rust::Vec<rust::String> result;
    result.reserve(limit);
    for (const auto &item : dictionary.query_prefix(lowered, limit))
        result.push_back(rust::String(item.word));
    return result;
}
DictionaryEntry dictionary_validate(const DictionaryEntry& entry) {
    const auto validation = metasequoia::validate_personal_dictionary_entry(entry_for(entry));
    if (!validation.entry) throw std::invalid_argument(validation.error);
    return entry_for(*validation.entry);
}
void dictionary_edit(const EngineOptions& options, rust::Slice<const DictionaryEntry> previous,
                     rust::Slice<const DictionaryEntry> replacement, rust::Str request_id) {
    if (previous.size() > 1 || replacement.size() > 1)
        throw std::invalid_argument("Expected at most one dictionary entry");
    std::optional<metasequoia::PersonalDictionaryEntry> before, after;
    if (!previous.empty()) before = entry_for(previous[0]);
    if (!replacement.empty()) after = entry_for(replacement[0]);
    auto result = metasequoia::edit_personal_dictionary(paths_for(options), before, after, std::string(request_id));
    if (!result.success) throw std::runtime_error(result.error);
}
void reset_learned_data(const EngineOptions& options) {
    const auto paths = paths_for(options);
    paths.validate();
    const auto resources = std::filesystem::weakly_canonical(paths.resources);
    const auto dictionaries = std::filesystem::weakly_canonical(paths.dictionaries);
    if (resources == dictionaries)
        throw std::invalid_argument("Cannot reset packaged dictionaries in place");
    const auto main_source = paths.resources / metasequoia::assets::main_dictionary;
    const auto english_source = paths.resources / metasequoia::assets::english_dictionary;
    const auto main_target = paths.dictionaries / metasequoia::assets::main_dictionary;
    const auto english_target = paths.dictionaries / metasequoia::assets::english_dictionary;
    for (const auto &source : {main_source, english_source})
        if (!std::filesystem::is_regular_file(source))
            throw std::runtime_error("Packaged dictionary is unavailable");
    std::filesystem::create_directories(paths.user_data);
    std::filesystem::create_directories(paths.dictionaries);
    user_dictionary::close_default_user_database();

    const auto stamp = std::to_string(std::chrono::steady_clock::now().time_since_epoch().count());
    // Derive the sibling names by concatenating onto the path's own native
    // string. path::string() converts through the system narrow encoding, which
    // on Windows is the ANSI code page: under a profile such as
    // C:\Users\陆傲天 it either produces different bytes or throws outright, and
    // throwing here would abort a reset that has already published files.
    // Everything added below is ASCII, the one thing every code page agrees on.
    const auto affixed = [&](const std::filesystem::path &target, const char *infix) {
        std::filesystem::path name(".");
        name += target.filename();
        name += infix;
        name += stamp;
        return target.parent_path() / name;
    };
    const auto temporary = [&](const std::filesystem::path &target) { return affixed(target, ".reset."); };
    const auto backup = [&](const std::filesystem::path &target) { return affixed(target, ".backup."); };
    const auto journal = paths.user_data / metasequoia::assets::user_journal;
    const auto journal_temporary = temporary(journal);
    const auto journal_backup = backup(journal);
    if (!user_dictionary::ensure_user_database(journal_temporary.u8string()))
        throw std::runtime_error("Cannot prepare empty learning journal");

    struct Replacement {
        std::filesystem::path target;
        std::filesystem::path temporary;
        std::filesystem::path backup;
        bool had_original = false;
        bool published = false;
    };
    std::vector<Replacement> replacements;
    replacements.reserve(3);
    replacements.push_back({main_target, temporary(main_target), backup(main_target)});
    replacements.push_back({english_target, temporary(english_target), backup(english_target)});
    replacements.push_back({journal, journal_temporary, journal_backup});
    const auto fail_cleanup = [&] {
        for (auto it = replacements.rbegin(); it != replacements.rend(); ++it) {
            std::error_code ignored;
            if (it->published) std::filesystem::remove(it->target, ignored);
            if (it->had_original && std::filesystem::exists(it->backup))
                std::filesystem::rename(it->backup, it->target, ignored);
            std::filesystem::remove(it->temporary, ignored);
        }
    };
    try {
        if (!std::filesystem::copy_file(main_source, replacements[0].temporary,
                                        std::filesystem::copy_options::none) ||
            !std::filesystem::copy_file(english_source, replacements[1].temporary,
                                        std::filesystem::copy_options::none))
            throw std::runtime_error("Cannot stage fresh dictionaries");
        for (auto &replacement : replacements) {
            std::error_code error;
            replacement.had_original = std::filesystem::exists(replacement.target);
            if (replacement.had_original && std::filesystem::exists(replacement.backup))
                std::filesystem::remove_all(replacement.backup, error);
            if (replacement.had_original) {
                std::filesystem::rename(replacement.target, replacement.backup, error);
                if (error)
                    throw std::runtime_error("Cannot stage learned-data reset");
                // The original is safely held aside until every replacement is published.
            }
            error.clear();
            std::filesystem::rename(replacement.temporary, replacement.target, error);
            if (!error) {
                replacement.published = true;
            } else {
                throw std::runtime_error("Cannot publish learned-data reset");
            }
        }
        for (const auto &suffix : {"-wal", "-shm", "-journal"}) {
            // Same reason as `affixed` above: the journal path carries the user
            // profile, so it is the one most likely to hold non-ASCII. Leaving
            // a -wal behind would let the learned data the user just erased
            // come back on the next open.
            std::filesystem::path sidecar = journal;
            sidecar += suffix;
            std::filesystem::remove(sidecar);
        }
        for (const auto &replacement : replacements) {
            std::error_code ignored;
            std::filesystem::remove_all(replacement.backup, ignored);
        }
    } catch (...) {
        fail_cleanup();
        throw;
    }
}
DictionaryReplaySummary replay_user_dictionary(rust::Str user_db_path, rust::Str main_db_path,
                                                rust::Str english_db_path) {
    const auto result = user_dictionary::replay(std::string(user_db_path), std::string(main_db_path),
                                                std::string(english_db_path));
    return {result.applied, result.skipped, result.failed, rust::String(result.error)};
}
EngineOptions prepare_options(rust::Str resources, rust::Str user_data, rust::Str cache, rust::Str content_id) {
    auto paths = metasequoia::prepare_runtime_paths(std::filesystem::u8path(std::string(resources)),
        std::filesystem::u8path(std::string(user_data)), std::filesystem::u8path(std::string(cache)), std::string(content_id));
    EngineOptions result;
    result.resources = paths.resources.u8string();
    result.user_data = paths.user_data.u8string();
    result.cache = paths.cache.u8string();
    result.dictionaries = paths.dictionaries.u8string();
    result.scheme = 0;
    result.shuangpin_profile = 0;
    result.learning = false;
    result.autocorrect_transposition = false;
    result.autocorrect_neighbor = false;
    result.fuzzy_pinyin_rules = 0;
    result.helpcode = true;
    result.show_helpcode = true;
    result.helpcode_schema = "ziranma";
    result.chinese_punctuation = true;
    result.paired_punctuation = true;
    result.punctuation_lock = 0;
    result.frequency_mode = "promote";
    result.frequency_trigger_count = 1;
    result.frequency_linear_step = 1;
    result.mixed_english = true;
    result.english_minimum_prefix = 5;
    result.mixed_emoji = false;
    result.mixed_kaomoji = false;
    result.local_unicode = true;
    result.local_date_time = true;
    result.local_quick_phrase = true;
    result.local_emoji = true;
    result.local_kaomoji = true;
    result.local_super_jianpin = true;
    result.local_temporary_english = true;
    result.local_temporary_japanese = true;
    return result;
}
rust::String hanzi_to_pinyin(const EngineOptions& options, rust::Str text) {
    const std::string word(text);
    if (word.empty()) return {};
    std::size_t offset = 0;
    std::size_t length = 0;
    while (offset < word.size()) {
        std::string character;
        std::uint32_t codepoint = 0;
        if (!next_utf8(word, offset, character, codepoint) || !is_han(codepoint)) return {};
        ++length;
    }
    if (length == 0 || length > 128) return {};
    const auto database_path = paths_for(options).dictionary(metasequoia::assets::main_dictionary);
    sqlite3* database = nullptr;
    if (sqlite3_open_v2(database_path.u8string().c_str(), &database,
                        SQLITE_OPEN_READONLY | SQLITE_OPEN_FULLMUTEX, nullptr) != SQLITE_OK) {
        if (database) sqlite3_close(database);
        return {};
    }
    std::string result = exact_hanzi_pinyin(database, word, length);
    if (result.empty()) {
        const auto singles = cached_single_hanzi_map(database, database_path.u8string());
        offset = 0;
        while (offset < word.size()) {
            std::string character;
            std::uint32_t codepoint = 0;
            if (!next_utf8(word, offset, character, codepoint)) {
                result.clear();
                break;
            }
            const auto found = singles->find(character);
            if (found == singles->end()) {
                result.clear();
                break;
            }
            if (!result.empty()) result.push_back('\'');
            result += found->second;
        }
    }
    sqlite3_close(database);
    return result;
}
rust::String normalize_full_pinyin(rust::Str input, std::size_t expected_syllables) {
    std::string source(input);
    source.erase(std::remove_if(source.begin(), source.end(), [](unsigned char ch) {
        return std::isspace(ch) != 0;
    }), source.end());
    std::transform(source.begin(), source.end(), source.begin(), [](unsigned char ch) {
        return static_cast<char>(std::tolower(ch));
    });
    if (source.empty() || source.front() == '\'' || source.back() == '\'' || source.find("''") != std::string::npos)
        return {};

    quanpin::Segments segments;
    if (source.find('\'') != std::string::npos) {
        segments = quanpin::split_segments(source);
    } else {
        const auto cuts = quanpin::cut_pinyin_by_mode(source, "correction");
        if (cuts.empty()) return {};
        segments = cuts.front();
        if (expected_syllables != 0 && segments.size() != expected_syllables) {
            const auto alternatives = quanpin::enumerate_complete_segmentations(
                quanpin::build_syllable_graph(source));
            const auto match = std::find_if(alternatives.begin(), alternatives.end(),
                [expected_syllables](const quanpin::Segments& cut) {
                    return cut.size() == expected_syllables;
                });
            if (match != alternatives.end()) segments = *match;
        }
    }

    if (expected_syllables != 0 && segments.size() != expected_syllables) return {};

    const auto& valid = quanpin::intact_pinyin_set();
    if (segments.empty() || !std::all_of(segments.begin(), segments.end(), [&valid](const std::string& segment) {
        return !segment.empty() && valid.find(segment) != valid.end();
    })) return {};

    const std::string normalized = quanpin::join_segments(segments);
    std::string without_delimiters = normalized;
    without_delimiters.erase(std::remove(without_delimiters.begin(), without_delimiters.end(), '\''),
                             without_delimiters.end());
    std::string source_without_delimiters = source;
    source_without_delimiters.erase(std::remove(source_without_delimiters.begin(), source_without_delimiters.end(), '\''),
                                    source_without_delimiters.end());
    return without_delimiters == source_without_delimiters ? rust::String(normalized) : rust::String();
}
namespace {
// The Engine spells the ü finals with a leading v because that is what the key sequence uses. A
// person reads the hint, so show the vowel.
std::string shuangpin_display_unit(const std::string& unit) {
    return !unit.empty() && unit.front() == 'v' ? "ü" + unit.substr(1) : unit;
}
void collect_shuangpin_units(const std::unordered_map<std::string, std::string>& mapping,
                             std::map<std::string, std::vector<std::string>>& units_by_key) {
    for (const auto& [unit, key] : mapping) {
        std::string upper = key;
        std::transform(upper.begin(), upper.end(), upper.begin(),
                       [](unsigned char character) { return static_cast<char>(std::toupper(character)); });
        units_by_key[upper].push_back(shuangpin_display_unit(unit));
    }
}
std::string join_shuangpin_units(std::vector<std::string> units) {
    std::sort(units.begin(), units.end());
    std::string joined;
    for (const auto& unit : units) {
        if (!joined.empty()) joined += " ";
        joined += unit;
    }
    return joined;
}
} // namespace
// Per-key double-pinyin hint text read out of the profile the session itself runs, so a keyboard
// face never carries a second copy of the keymap that can drift from it. An unknown profile name
// yields no hints rather than the default profile's: labelling the keys with a scheme the session
// is not running is worse than labelling nothing.
rust::Vec<ShuangpinKeyHint> shuangpin_key_hints(rust::Str profile) {
    rust::Vec<ShuangpinKeyHint> hints;
    hints.reserve(29);
    const std::string name(profile);
    if (name != "xiaohe" && name != "ziranma" && name != "shoudao" && name != "microsoft") return hints;

    const ShuangpinProfile& source = GetShuangpinProfile(name);
    std::map<std::string, std::vector<std::string>> initials_by_key;
    std::map<std::string, std::vector<std::string>> finals_by_key;
    collect_shuangpin_units(source.initials, initials_by_key);
    collect_shuangpin_units(source.finals, finals_by_key);

    for (const auto* key : {"Q", "W", "E", "R", "T", "Y", "U", "I", "O", "P", "A", "S", "D", "F",
                            "G", "H", "J", "K", "L", "Z", "X", "C", "V", "B", "N", "M", ";"}) {
        const std::string initials = join_shuangpin_units(initials_by_key[key]);
        const std::string finals = join_shuangpin_units(finals_by_key[key]);
        if (initials.empty() && finals.empty()) continue;
        ShuangpinKeyHint hint;
        hint.key = rust::String(key);
        // "initials / finals" when the key carries both, otherwise whichever side it carries.
        hint.hint = rust::String(initials.empty()   ? finals
                                 : finals.empty()   ? initials
                                                    : initials + " / " + finals);
        hints.push_back(std::move(hint));
    }
    return hints;
}
EngineSnapshot EngineSession::snapshot() const {
    auto value = session_.snapshot();
    EngineSnapshot output;
    output.local_mode = local_mode_name(value.local_mode);
    output.dedicated_english = value.dedicated_english;
    output.nine_key = nine_key_;
    output.nine_key_spellings.reserve(value.nine_key_spellings.size());
    for (const auto& spelling : value.nine_key_spellings)
        output.nine_key_spellings.push_back(rust::String(spelling));
    output.microsoft_shuangpin = microsoft_shuangpin_;
    output.scheme = static_cast<std::uint8_t>(value.scheme);
    output.shuangpin_profile = rust::String(shuangpin_profile_);
    output.answered_by_pinyin_fallback = value.answered_by_pinyin_fallback;
    output.wubi_unique_four_code = value.wubi_unique_four_code;
    output.preedit = value.preedit;
    output.reading = value.scheme == SchemeType::JapaneseRomaji
                         ? value.normalized_segmentation
                         : std::string{};
    output.editing_text = value.editing_text;
    output.caret_position = value.caret_position;
    const auto boundaries = session_.segment_raw_boundaries();
    output.segment_raw_boundaries.reserve(boundaries.size());
    for (const auto boundary : boundaries)
        output.segment_raw_boundaries.push_back(static_cast<std::uint64_t>(boundary));
    const auto candidate_count = value.candidates.size();
    output.candidates.reserve(candidate_count);
    output.candidate_codes.reserve(candidate_count);
    output.candidate_annotations.reserve(candidate_count);
    output.candidate_sources.reserve(candidate_count);
    output.candidate_positions.reserve(candidate_count);
    output.candidate_corrected.reserve(candidate_count);
    output.candidate_answers_key.reserve(candidate_count);
    for (std::size_t index = 0; index < candidate_count; ++index) {
        const auto &candidate = value.candidates[index];
        output.candidates.push_back(rust::String(candidate.word));
        output.candidate_codes.push_back(rust::String(candidate.pinyin));
        auto annotation = index < value.candidate_annotations.size()
                              ? value.candidate_annotations[index]
                              : candidate.corrected_from;
        if (!show_helpcode_ && helpcode_enabled_ && helpcode_keymap_ &&
            (value.scheme == SchemeType::Quanpin || value.scheme == SchemeType::Shuangpin)) {
            const auto helpcode = HelpcodeUtils::compute_helpcodes(
                candidate.word, value.scheme == SchemeType::Quanpin, helpcode_keymap_.get());
            if (!helpcode.empty() && annotation == helpcode) annotation = candidate.corrected_from;
        }
        if (annotation.empty() && show_helpcode_ && helpcode_enabled_ && helpcode_keymap_ &&
            candidate.source == CandidateSource::Generated &&
            (value.scheme == SchemeType::Quanpin || value.scheme == SchemeType::Shuangpin)) {
            annotation = HelpcodeUtils::compute_helpcodes(
                candidate.word, value.scheme == SchemeType::Quanpin, helpcode_keymap_.get());
        }
        output.candidate_annotations.push_back(rust::String(annotation));
        output.candidate_sources.push_back(static_cast<std::uint8_t>(candidate.source));
        output.candidate_positions.push_back(static_cast<std::uint8_t>(candidate.fixed_position));
        output.candidate_corrected.push_back(!candidate.corrected_from.empty());
        // A short vector would be an engine bug. False is the safe reading of one: a consumer that
        // sees nothing answering the key declines to reorder, rather than comparing candidates that
        // answered different keys.
        output.candidate_answers_key.push_back(index < value.candidate_answers_key.size() &&
                                               value.candidate_answers_key[index]);
    }
    return output;
}
void EngineSession::reset_cache() {
    session_.reset_cache();
}
void EngineSession::set_caret(std::uint64_t caret) {
    if (caret == std::numeric_limits<std::uint64_t>::max())
        session_.set_caret(std::nullopt);
    else
        session_.set_caret(static_cast<std::size_t>(caret));
}
std::size_t EngineSession::prefix_end() const {
    return session_.prefix_end();
}
rust::String EngineSession::pending_suffix() const {
    return rust::String(session_.pending_suffix());
}
void EngineSession::reset_context() {
    session_.reset_context();
}
OnlineQuerySnapshot EngineSession::online_query() const {
    OnlineQuerySnapshot output;
    const auto query = session_.online_query();
    if (!query.has_value()) return output;
    output.available = true;
    output.scheme = static_cast<std::uint8_t>(query->scheme);
    output.generation = query->generation;
    output.identity = query->identity;
    output.query_text = query->query_text;
    output.cache_key = query->cache_key;
    output.pinyin_segments.reserve(query->pinyin_segments.size());
    for (const auto& segment : query->pinyin_segments)
        output.pinyin_segments.push_back(rust::String(segment));
    output.cloud_eligible = query->cloud_eligible;
    output.ai_eligible = query->ai_eligible;
    output.session_id = query->session_id;
    return output;
}
bool EngineSession::apply_online_candidate(const OnlineQuerySnapshot& query,
                                           rust::Str candidate, std::uint8_t source) {
    if (!query.available || (source != 0 && source != 1)) return false;
    metasequoia::OnlineQuery request;
    request.scheme = static_cast<SchemeType>(query.scheme);
    request.generation = query.generation;
    request.identity = std::string(query.identity);
    request.query_text = std::string(query.query_text);
    request.cache_key = std::string(query.cache_key);
    request.pinyin_segments.reserve(query.pinyin_segments.size());
    for (const auto& segment : query.pinyin_segments)
        request.pinyin_segments.emplace_back(std::string(segment));
    request.cloud_eligible = query.cloud_eligible;
    request.ai_eligible = query.ai_eligible;
    request.session_id = query.session_id;
    const auto kind = source == 0 ? CandidateSource::CloudSuggestion
                                  : CandidateSource::AiSuggestion;
    return session_.apply_online_candidate(request, std::string(candidate), kind);
}
bool EngineSession::apply_online_candidates(const OnlineQuerySnapshot& query,
                                           rust::Slice<const rust::String> candidates, std::uint8_t source) {
    if (!query.available || (source != 0 && source != 1)) return false;
    metasequoia::OnlineQuery request;
    request.scheme = static_cast<SchemeType>(query.scheme);
    request.generation = query.generation;
    request.identity = std::string(query.identity);
    request.query_text = std::string(query.query_text);
    request.cache_key = std::string(query.cache_key);
    request.pinyin_segments.reserve(query.pinyin_segments.size());
    for (const auto& segment : query.pinyin_segments)
        request.pinyin_segments.emplace_back(std::string(segment));
    request.cloud_eligible = query.cloud_eligible;
    request.ai_eligible = query.ai_eligible;
    request.session_id = query.session_id;
    const auto kind = source == 0 ? CandidateSource::CloudSuggestion
                                  : CandidateSource::AiSuggestion;
    std::vector<std::string> words;
    words.reserve(candidates.size());
    for (const auto& candidate : candidates) words.emplace_back(std::string(candidate));
    return session_.apply_online_candidates(request, words, kind);
}
static EmojiCatalogSlice read_emoji_catalog_slice(rust::Str resources, rust::Str search,
    rust::Str category, rust::Str group, std::size_t offset, std::uint16_t limit, rust::Str parent,
    bool deduplicate) {
    EmojiCatalogSlice result;
    result.next_offset = offset;
    if (limit == 0 || limit > 4096 || offset > static_cast<std::size_t>(std::numeric_limits<sqlite3_int64>::max()))
        throw std::invalid_argument("Invalid emoji catalog page");
    result.items.reserve(limit);
    const auto path = std::filesystem::u8path(std::string(resources)) / "others.db";
    sqlite3 *database = nullptr;
    const int opened = sqlite3_open_v2(path.u8string().c_str(), &database,
                                     SQLITE_OPEN_READONLY | SQLITE_OPEN_FULLMUTEX, nullptr);
    const std::unique_ptr<sqlite3, decltype(&sqlite3_close)> database_guard(database, sqlite3_close);
    if (opened != SQLITE_OK)
        throw std::runtime_error("Emoji catalog unavailable");
    const std::string category_text(category);
    const std::string group_text(group);
    const std::string parent_text(parent);
    const bool kaomoji = category_text == "kaomoji";
    const bool symbols = category_text == "symbols";
    const char *sql = nullptr;
    if (kaomoji) {
        sql = "SELECT kaomoji,'All',keywords FROM kaomoji_catalog "
              "WHERE (?1 = '' OR kaomoji LIKE ?2 OR keywords LIKE ?2) "
              "AND (?5 = '' OR ?5 = 'All') "
              "ORDER BY sort_order LIMIT ?3 OFFSET ?4";
    } else if (symbols) {
        sql = "SELECT symbol,category,keywords FROM symbol_catalog "
              "WHERE (?1 = '' OR symbol LIKE ?2 OR category LIKE ?2 OR "
              "parent_category LIKE ?2 OR keywords LIKE ?2) "
              "AND (?5 = '' OR category = ?5) "
              "AND (?6 = '' OR COALESCE(NULLIF(parent_category,''),category) = ?6) "
              "ORDER BY sort_order LIMIT ?3 OFFSET ?4";
    } else {
        sql = "SELECT emoji,category,keywords FROM emoji "
              "WHERE (?1 = '' OR category = ?1) "
              "AND (?2 = '' OR pinyin LIKE ?3 OR keywords LIKE ?3 OR emoji LIKE ?3) "
              "ORDER BY sort_order LIMIT ?4 OFFSET ?5";
    }
    sqlite3_stmt *statement = nullptr;
    const int prepared = sqlite3_prepare_v2(database, sql, -1, &statement, nullptr);
    const std::unique_ptr<sqlite3_stmt, decltype(&sqlite3_finalize)> statement_guard(statement, sqlite3_finalize);
    if (prepared != SQLITE_OK)
        throw std::runtime_error("Emoji catalog query unavailable");
    // Do not include SQLite diagnostics: they can expose resource paths or data.
    const auto check_bind = [](int status) {
        if (status != SQLITE_OK)
            throw std::runtime_error("Emoji catalog query rejected");
    };
    const std::string search_text(search);
    const std::string pattern = "%" + search_text + "%";
    if (kaomoji || symbols) {
        check_bind(sqlite3_bind_text(statement, 1, search_text.c_str(), -1, SQLITE_TRANSIENT));
        check_bind(sqlite3_bind_text(statement, 2, pattern.c_str(), -1, SQLITE_TRANSIENT));
        check_bind(sqlite3_bind_int(statement, 3, limit));
        check_bind(sqlite3_bind_int64(statement, 4, static_cast<sqlite3_int64>(offset)));
        check_bind(sqlite3_bind_text(statement, 5, group_text.c_str(), -1, SQLITE_TRANSIENT));
        if (symbols) check_bind(sqlite3_bind_text(statement, 6, parent_text.c_str(), -1, SQLITE_TRANSIENT));
    } else {
        const auto &selected_group = group_text.empty() ? category_text : group_text;
        check_bind(sqlite3_bind_text(statement, 1, selected_group.c_str(), -1, SQLITE_TRANSIENT));
        check_bind(sqlite3_bind_text(statement, 2, search_text.c_str(), -1, SQLITE_TRANSIENT));
        check_bind(sqlite3_bind_text(statement, 3, pattern.c_str(), -1, SQLITE_TRANSIENT));
        check_bind(sqlite3_bind_int(statement, 4, limit));
        check_bind(sqlite3_bind_int64(statement, 5, static_cast<sqlite3_int64>(offset)));
    }
    std::unordered_set<std::string> seen;
    int status = SQLITE_OK;
    while ((status = sqlite3_step(statement)) == SQLITE_ROW) {
        if (result.next_offset == static_cast<std::size_t>(std::numeric_limits<sqlite3_int64>::max()))
            throw std::invalid_argument("Invalid emoji catalog cursor");
        ++result.next_offset;
        const auto *text = reinterpret_cast<const char *>(sqlite3_column_text(statement, 0));
        const auto *group = reinterpret_cast<const char *>(sqlite3_column_text(statement, 1));
        const auto *annotation = reinterpret_cast<const char *>(sqlite3_column_text(statement, 2));
        if (!deduplicate && (!text || !text[0] || (!kaomoji && (!group || !group[0]))))
            continue;
        if (text && (!deduplicate || seen.insert(text).second))
            result.items.push_back({rust::String(text), rust::String(annotation ? annotation : ""),
                              rust::String(group ? group : "")});
    }
    if (status != SQLITE_DONE)
        throw std::runtime_error("Emoji catalog read failed");
    result.complete = result.next_offset - offset < limit;
    return result;
}
rust::Vec<EmojiCatalogItem> emoji_catalog_filtered_page(rust::Str resources, rust::Str search,
    rust::Str category, rust::Str group, std::size_t offset, std::uint16_t limit, rust::Str parent) {
    return read_emoji_catalog_slice(resources, search, category, group, offset, limit, parent, true).items;
}
EmojiCatalogSlice emoji_catalog_slice(rust::Str resources, rust::Str search,
    rust::Str category, rust::Str group, std::size_t offset, std::uint16_t limit, rust::Str parent) {
    return read_emoji_catalog_slice(resources, search, category, group, offset, limit, parent, false);
}
rust::Vec<rust::String> emoji_catalog_groups(rust::Str resources, rust::Str category) {
    const auto path = std::filesystem::u8path(std::string(resources)) / "others.db";
    sqlite3 *database = nullptr;
    const int opened = sqlite3_open_v2(path.u8string().c_str(), &database,
                                     SQLITE_OPEN_READONLY | SQLITE_OPEN_FULLMUTEX, nullptr);
    const std::unique_ptr<sqlite3, decltype(&sqlite3_close)> database_guard(database, sqlite3_close);
    if (opened != SQLITE_OK) throw std::runtime_error("Emoji catalog unavailable");
    const std::string kind(category);
    const char *sql = kind == "kaomoji"
        ? "SELECT 'All' FROM kaomoji_catalog LIMIT 1"
        : kind == "symbols"
            ? "SELECT category FROM symbol_catalog WHERE category IS NOT NULL AND category != '' GROUP BY category ORDER BY MIN(sort_order), category"
            : "SELECT category FROM emoji WHERE category IS NOT NULL AND category != '' GROUP BY category ORDER BY MIN(sort_order), category";
    sqlite3_stmt *statement = nullptr;
    const int prepared = sqlite3_prepare_v2(database, sql, -1, &statement, nullptr);
    const std::unique_ptr<sqlite3_stmt, decltype(&sqlite3_finalize)> statement_guard(statement, sqlite3_finalize);
    if (prepared != SQLITE_OK) throw std::runtime_error("Emoji catalog query unavailable");
    rust::Vec<rust::String> groups;
    int status = SQLITE_OK;
    while ((status = sqlite3_step(statement)) == SQLITE_ROW) {
        const auto *value = reinterpret_cast<const char *>(sqlite3_column_text(statement, 0));
        if (value) groups.push_back(rust::String(value));
    }
    if (status != SQLITE_DONE) throw std::runtime_error("Emoji catalog read failed");
    return groups;
}
rust::Vec<rust::String> candidate_glosses(
    rust::Str resources, rust::Slice<const CandidateGlossInput> candidates) {
    return candidate_glosses_with_user(resources, rust::Str(), candidates);
}
bool save_candidate_gloss(rust::Str user_data, bool chinese_to_english, rust::Str key, rust::Str gloss) {
    const auto path = std::filesystem::u8path(std::string(user_data)) / "translation-glosses.db";
    return EnglishDictionary::upsert_gloss(path.u8string(), chinese_to_english,
                                            std::string(key), std::string(gloss));
}
rust::Vec<rust::String> candidate_glosses_with_user(
    rust::Str resources, rust::Str user_data, rust::Slice<const CandidateGlossInput> candidates) {
    const auto database_path = std::filesystem::u8path(std::string(resources)) /
                               metasequoia::assets::english_dictionary;
    const auto open_dictionary = [](const std::filesystem::path& path) {
        std::unique_ptr<EnglishDictionary> dictionary;
        std::error_code error;
        if (std::filesystem::is_regular_file(path, error)) {
            dictionary = std::make_unique<EnglishDictionary>(path.u8string(), false);
            if (!dictionary->ready()) dictionary.reset();
        }
        return dictionary;
    };
    // An empty resource path requests only the user overlay, never cwd/english.db.
    auto dictionary = resources.empty() ? std::unique_ptr<EnglishDictionary>() : open_dictionary(database_path);
    std::unique_ptr<EnglishDictionary> learned;
    if (!user_data.empty()) {
        const auto path = std::filesystem::u8path(std::string(user_data)) / "translation-glosses.db";
        learned = open_dictionary(path);
    }
    if (!dictionary && !learned)
        throw std::runtime_error("Candidate gloss dictionary unavailable");
    rust::Vec<rust::String> output;
    output.reserve(candidates.size());
    for (const auto& candidate : candidates) {
        std::string key;
        bool chinese_to_english = false;
        if (!candidate_gloss_key(candidate, key, chinese_to_english)) {
            output.push_back(rust::String());
            continue;
        }
        auto gloss = learned ? candidate_gloss_display(
                                   chinese_to_english ? learned->query_english_gloss(key)
                                                      : learned->query_chinese_gloss(key))
                             : std::string{};
        if (gloss.empty() && dictionary)
            gloss = candidate_gloss_display(
                chinese_to_english ? dictionary->query_english_gloss(key)
                                   : dictionary->query_chinese_gloss(key));
        output.push_back(rust::String(gloss));
    }
    return output;
}
// Glosses Chinese candidates into one non-English language from an offline-glosses/zh-<lang>.db built by scripts/build_offline_glosses.py. The file states its schema version and language; one that disagrees with the request is refused rather than shown, so a renamed file cannot put French under Japanese. Latin candidates get nothing: the file only maps Chinese words.
rust::Vec<rust::String> candidate_target_glosses(
    rust::Str database_path, rust::Str target_language, rust::Slice<const CandidateGlossInput> candidates) {
    sqlite3 *database = nullptr;
    const int opened = sqlite3_open_v2(std::string(database_path).c_str(), &database,
                                     SQLITE_OPEN_READONLY | SQLITE_OPEN_FULLMUTEX, nullptr);
    const std::unique_ptr<sqlite3, decltype(&sqlite3_close)> database_guard(database, sqlite3_close);
    if (opened != SQLITE_OK) throw std::runtime_error("Offline gloss dictionary unavailable");
    const auto prepare = [&](const char *sql) {
        sqlite3_stmt *statement = nullptr;
        if (sqlite3_prepare_v2(database, sql, -1, &statement, nullptr) != SQLITE_OK) {
            sqlite3_finalize(statement);
            throw std::runtime_error("Offline gloss dictionary unreadable");
        }
        return std::unique_ptr<sqlite3_stmt, decltype(&sqlite3_finalize)>(statement, sqlite3_finalize);
    };
    const auto version = prepare("PRAGMA user_version");
    if (sqlite3_step(version.get()) != SQLITE_ROW || sqlite3_column_int(version.get(), 0) != 1)
        throw std::runtime_error("Offline gloss dictionary version unsupported");
    const auto language = prepare("SELECT value FROM meta WHERE key = 'target_language'");
    const auto *stored = sqlite3_step(language.get()) == SQLITE_ROW
                             ? reinterpret_cast<const char *>(sqlite3_column_text(language.get(), 0))
                             : nullptr;
    if (!stored || std::string(stored) != std::string(target_language))
        throw std::runtime_error("Offline gloss dictionary language mismatch");
    const auto lookup = prepare("SELECT gloss FROM zh_glosses WHERE chinese = ?1");
    rust::Vec<rust::String> output;
    output.reserve(candidates.size());
    for (const auto& candidate : candidates) {
        std::string key;
        bool chinese_to_english = false;
        std::string gloss;
        if (candidate_gloss_key(candidate, key, chinese_to_english) && chinese_to_english) {
            sqlite3_reset(lookup.get());
            sqlite3_bind_text(lookup.get(), 1, key.data(), static_cast<int>(key.size()), SQLITE_TRANSIENT);
            const int status = sqlite3_step(lookup.get());
            if (status == SQLITE_ROW) {
                const auto *text = reinterpret_cast<const char *>(sqlite3_column_text(lookup.get(), 0));
                if (text) gloss = candidate_gloss_display(text);
            } else if (status != SQLITE_DONE) {
                throw std::runtime_error("Offline gloss dictionary read failed");
            }
        }
        output.push_back(rust::String(gloss));
    }
    return output;
}
rust::Vec<EmojiSymbolGroup> emoji_symbol_groups(rust::Str resources) {
    const auto path = std::filesystem::u8path(std::string(resources)) / "others.db";
    sqlite3 *database = nullptr;
    const int opened = sqlite3_open_v2(path.u8string().c_str(), &database,
                                     SQLITE_OPEN_READONLY | SQLITE_OPEN_FULLMUTEX, nullptr);
    const std::unique_ptr<sqlite3, decltype(&sqlite3_close)> database_guard(database, sqlite3_close);
    if (opened != SQLITE_OK) throw std::runtime_error("Emoji catalog unavailable");
    const char *sql = "SELECT COALESCE(NULLIF(parent_category,''),category) AS parent, category "
        "FROM symbol_catalog WHERE category IS NOT NULL AND category != '' "
        "GROUP BY parent, category ORDER BY MIN(sort_order), parent, category";
    sqlite3_stmt *statement = nullptr;
    const int prepared = sqlite3_prepare_v2(database, sql, -1, &statement, nullptr);
    const std::unique_ptr<sqlite3_stmt, decltype(&sqlite3_finalize)> statement_guard(statement, sqlite3_finalize);
    if (prepared != SQLITE_OK) throw std::runtime_error("Emoji catalog query unavailable");
    rust::Vec<EmojiSymbolGroup> groups;
    int status = SQLITE_OK;
    while ((status = sqlite3_step(statement)) == SQLITE_ROW) {
        const auto *parent = reinterpret_cast<const char *>(sqlite3_column_text(statement, 0));
        const auto *title = reinterpret_cast<const char *>(sqlite3_column_text(statement, 1));
        if (parent && title) groups.push_back({rust::String(parent), rust::String(title)});
    }
    if (status != SQLITE_DONE) throw std::runtime_error("Emoji catalog read failed");
    return groups;
}
#if !defined(__ANDROID__)
rust::Vec<rust::String> handwriting_recognize(rust::Str model_path,
                                               rust::Slice<const HandwritingPoint> points,
                                               float width, float height) {
    if (model_path.empty() || points.empty())
        return {};
#if !MSIME_HAS_HANDWRITING_CANDIDATES
    (void)model_path; (void)points; (void)width; (void)height;
    return {};
#else
    std::vector<metasequoia::handwriting::Stroke> strokes;
    std::uint32_t stroke_count = 0;
    for (const auto &point : points)
        stroke_count = std::max(stroke_count, point.stroke + 1);
    strokes.resize(stroke_count);
    for (const auto &point : points)
        strokes[point.stroke].push_back({point.x, point.y});
    metasequoia::handwriting::Recognizer recognizer{std::string(model_path)};
    const auto candidates = recognizer.recognize(strokes, width, height);
    rust::Vec<rust::String> result;
    result.reserve(candidates.size());
    for (const auto &candidate : candidates)
        result.push_back(rust::String(candidate));
    return result;
#endif
}
#endif
EngineResult EngineSession::character(std::uint8_t value, bool shift) {
    if (value > 127) throw std::invalid_argument("Engine character must be ASCII");
    return result_for(session_.character(static_cast<char>(value), shift));
}
bool EngineSession::expand_initial_candidates() {
    return session_.expand_initial_candidates();
}
void EngineSession::set_nine_key_enabled(bool enabled) {
    session_.set_nine_key_enabled(enabled);
    nine_key_ = enabled;
}
EngineResult EngineSession::choose_nine_key_spelling(std::size_t index) {
    return result_for(session_.choose_nine_key_spelling(index));
}
EngineResult EngineSession::command(std::uint8_t value) {
    using metasequoia::Command;
    switch (value) {
        case 0: return result_for(session_.command(Command::Backspace));
        case 1: return result_for(session_.command(Command::CommitCandidate));
        case 2: return result_for(session_.command(Command::CommitRaw));
        case 3: return result_for(session_.command(Command::Cancel));
        case 4: return result_for(session_.command(Command::MoveLeft));
        case 5: return result_for(session_.command(Command::MoveRight));
        case 6: return result_for(session_.command(Command::MoveHome));
        case 7: return result_for(session_.command(Command::MoveEnd));
        case 8: return result_for(session_.command(Command::DeleteForward));
        case 9: return result_for(session_.command(Command::CycleKanaVariant));
        case 10: return result_for(session_.command(Command::CommitReading));
        case 11: return commit_raw_without_learning();
        default: throw std::invalid_argument("Unsupported input command");
    }
}
// The letters as typed, learned as nothing. Code 2 reaches commit_raw_with_policy instead, and the Engine's own raw commit learns the word itself in dedicated English, so that mode takes the preedit and cancels, stripping the trigger letter of a temporary mode the way `InputSession` does.
EngineResult EngineSession::commit_raw_without_learning() {
    const auto before = session_.snapshot();
    if (!before.dedicated_english) return result_for(session_.command(metasequoia::Command::CommitRaw));
    std::string raw = before.preedit;
    if ((before.local_mode == metasequoia::LocalInputMode::TemporaryEnglish ||
         before.local_mode == metasequoia::LocalInputMode::TemporaryJapanese) &&
        !raw.empty())
        raw.erase(raw.begin());
    session_.command(metasequoia::Command::Cancel);
    return result_for(metasequoia::KeyResult{true, std::move(raw), std::nullopt});
}
EngineResult EngineSession::commit_raw_with_policy() {
    const auto before = session_.snapshot();
    const bool chinese_scheme = before.scheme == SchemeType::Quanpin ||
                                before.scheme == SchemeType::Shuangpin;
    const bool local_special_mode = before.local_mode != metasequoia::LocalInputMode::None;
    bool complete_pure_pinyin = false;
    if (chinese_scheme) {
        const auto &segmentation = before.normalized_segmentation.empty()
                                       ? before.raw_segmentation
                                       : before.normalized_segmentation;
        complete_pure_pinyin = !segmentation.empty() &&
                               quanpin::is_complete_pinyin_input(segmentation);
    }
    const bool should_learn = before.dedicated_english || local_special_mode ||
                              (chinese_scheme && !complete_pure_pinyin);
    auto result = session_.command(metasequoia::Command::CommitRaw);
    if (should_learn && result.commit && !result.commit->empty()) {
        std::string word = *result.commit;
        if (before.local_mode == metasequoia::LocalInputMode::TemporaryJapanese)
            word.insert(0, "R");
        if (!user_dictionary::learn_entered_english_word(
                paths_.dictionary(metasequoia::assets::english_dictionary).u8string(),
                paths_.user(metasequoia::assets::user_journal).u8string(), word)) {
            result.diagnostic = "English word could not be learned.";
        }
    }
    return result_for(result);
}
EngineResult EngineSession::select(std::size_t index) { return result_for(session_.select(index)); }
EngineResult EngineSession::pin_candidate(std::size_t index) { return result_for(session_.pin(index)); }
EngineResult EngineSession::remove_candidate(std::size_t index) { return result_for(session_.remove(index)); }
EngineResult EngineSession::fix_candidate_position(std::size_t index, std::uint8_t position) {
    if (position < 1 || position > 5)
        throw std::invalid_argument("Invalid candidate position");
    return result_for(session_.fix_position(index, position));
}
EngineResult EngineSession::clear_candidate_position(std::size_t index) {
    return result_for(session_.clear_position(index));
}
EngineResult EngineSession::select_edge(std::size_t index, std::uint8_t edge) {
    if (edge > 1) throw std::invalid_argument("Invalid candidate edge");
    return result_for(session_.select_edge(index, edge == 0 ? metasequoia::CandidateEdge::FirstHan
                                                          : metasequoia::CandidateEdge::LastHan));
}
EngineResult EngineSession::finish(std::size_t index) { return result_for(session_.finish(index)); }
EngineResult EngineSession::punctuation(std::uint8_t value) {
    if (value > 127) throw std::invalid_argument("Engine punctuation must be ASCII");
    return result_for(session_.punctuation(static_cast<char>(value)));
}
void EngineSession::balance_paired_punctuation_after_auto_close(std::uint8_t opening) {
    if (opening > 127) throw std::invalid_argument("Paired punctuation opening must be ASCII");
    session_.balance_paired_punctuation_after_auto_close(static_cast<char>(opening));
}
void EngineSession::set_chinese_punctuation_enabled(bool enabled) {
    session_.set_chinese_punctuation_enabled(enabled);
}

void EngineSession::set_punctuation_lock(std::uint8_t lock) {
    session_.set_punctuation_lock(lock);
}

void EngineSession::set_paired_punctuation_enabled(bool enabled) { session_.set_paired_punctuation_enabled(enabled); }

void EngineSession::set_dedicated_english(bool enabled) {
    session_.set_dedicated_english(enabled);
}
}
