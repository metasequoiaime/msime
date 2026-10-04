// Golden-fixture recorder for the C++ reference engine (MSIME-Engine a9b9f092 + 23 msime overlays).
//
// Records what the reference engine does so the Rust engine (crates/engine) can be tested against committed JSON after the C++ is deleted. It drives only the public metasequoia::Session API (include/metasequoia/session.h) plus the personal dictionary functions (include/metasequoia/personal_dictionary.h); two internal helpers are used for fixture preparation and determinism: EnglishDictionary::ensure_schema (a fixture without msime-english.db still needs one, runtime_paths.cpp copies both) and PersonalNgramStore::flush_all (personal context rows are written asynchronously after a 2 s delay).
//
// Modes:
//   recorder scenario <scenario.json> <work-root> <out.json>
//   recorder real <resources-dir> <tsv> <out.jsonl> <work-root> [--subset N] [--top N]
//
// Run one scenario per process: PersonalNgramStore is a process-lifetime singleton keyed by journal path.

#include <metasequoia/personal_dictionary.h>
#include <metasequoia/session.h>

#include "contracts/assets/assets.h"
#include "english/english_dictionary.h"
#include "user_dictionary/personal_ngram_store.h"

#include <nlohmann/json.hpp>
#include <sqlite3.h>

#include <algorithm>
#include <cstdlib>
#include <filesystem>
#include <fstream>
#include <iostream>
#include <memory>
#include <optional>
#include <sstream>
#include <stdexcept>
#include <string>
#include <vector>

using nlohmann::ordered_json;
using namespace metasequoia;
namespace fs = std::filesystem;

namespace
{
std::vector<std::string> g_root_texts;

// Diagnostics can carry absolute scratch paths; replace them so outputs do not depend on where the recorder ran.
std::string scrub(std::string text)
{
    for (const auto &root : g_root_texts)
        for (std::size_t at = text.find(root); at != std::string::npos; at = text.find(root, at))
        {
            text.replace(at, root.size(), "$ROOT");
            at += 5;
        }
    return text;
}

[[noreturn]] void fail(const std::string &message)
{
    throw std::runtime_error(message);
}

void exec_sql(const fs::path &path, const std::string &sql)
{
    fs::create_directories(path.parent_path());
    sqlite3 *db = nullptr;
    if (sqlite3_open(path.string().c_str(), &db) != SQLITE_OK)
        fail("cannot open " + path.string());
    char *error = nullptr;
    const int rc = sqlite3_exec(db, sql.c_str(), nullptr, nullptr, &error);
    std::string message = error ? error : "";
    sqlite3_free(error);
    sqlite3_close(db);
    if (rc != SQLITE_OK)
        fail("fixture SQL failed on " + path.filename().string() + ": " + message);
}

ordered_json column_value(sqlite3_stmt *statement, int column)
{
    switch (sqlite3_column_type(statement, column))
    {
    case SQLITE_INTEGER:
        return sqlite3_column_int64(statement, column);
    case SQLITE_FLOAT:
        return sqlite3_column_double(statement, column);
    case SQLITE_NULL:
        return nullptr;
    default: {
        const auto *text = reinterpret_cast<const char *>(sqlite3_column_text(statement, column));
        return std::string(text ? text : "");
    }
    }
}

// Runs a read-only query and returns rows as arrays. Missing file or error -> {"error": ...}.
ordered_json query_rows(const fs::path &path, const std::string &sql)
{
    if (!fs::exists(path))
        return ordered_json{{"missing", true}};
    sqlite3 *db = nullptr;
    if (sqlite3_open_v2(path.string().c_str(), &db, SQLITE_OPEN_READONLY, nullptr) != SQLITE_OK)
    {
        sqlite3_close(db);
        return ordered_json{{"error", "open failed"}};
    }
    sqlite3_stmt *statement = nullptr;
    ordered_json rows = ordered_json::array();
    if (sqlite3_prepare_v2(db, sql.c_str(), -1, &statement, nullptr) != SQLITE_OK)
    {
        ordered_json result{{"error", std::string(sqlite3_errmsg(db))}};
        sqlite3_close(db);
        return result;
    }
    while (sqlite3_step(statement) == SQLITE_ROW)
    {
        ordered_json row = ordered_json::array();
        for (int i = 0; i < sqlite3_column_count(statement); ++i)
            row.push_back(column_value(statement, i));
        rows.push_back(std::move(row));
    }
    sqlite3_finalize(statement);
    sqlite3_close(db);
    return rows;
}

std::vector<std::string> string_column(const fs::path &path, const std::string &sql)
{
    std::vector<std::string> values;
    for (const auto &row : query_rows(path, sql))
        if (row.is_array() && !row.empty() && row[0].is_string())
            values.push_back(row[0].get<std::string>());
    return values;
}

// Every journal table, every column except wall-clock timestamps, ordered by all remaining columns.
ordered_json dump_journal(const fs::path &journal)
{
    user_dictionary::PersonalNgramStore::flush_all();
    ordered_json result = ordered_json::object();
    if (!fs::exists(journal))
        return result;
    for (const auto &table : string_column(journal, "SELECT name FROM sqlite_master WHERE type='table' AND name NOT "
                                                    "LIKE 'sqlite_%' ORDER BY name"))
    {
        std::vector<std::string> columns;
        for (const auto &row : query_rows(journal, "PRAGMA table_info(\"" + table + "\")"))
        {
            const auto name = row[1].get<std::string>();
            if (name != "updated_at" && name != "created_at")
                columns.push_back(name);
        }
        if (columns.empty())
            continue;
        std::string select;
        std::string order;
        for (std::size_t i = 0; i < columns.size(); ++i)
        {
            select += (i ? "," : "") + ("\"" + columns[i] + "\"");
            order += (i ? "," : "") + std::to_string(i + 1);
        }
        const auto rows = query_rows(journal, "SELECT " + select + " FROM \"" + table + "\" ORDER BY " + order);
        if (rows.is_array() && rows.empty())
            continue;
        result[table] = ordered_json{{"columns", columns}, {"rows", rows}};
    }
    return result;
}

SchemeType scheme_from(const std::string &name)
{
    if (name == "quanpin")
        return SchemeType::Quanpin;
    if (name == "shuangpin")
        return SchemeType::Shuangpin;
    if (name == "wubi")
        return SchemeType::Wubi;
    if (name == "japanese")
        return SchemeType::JapaneseRomaji;
    fail("unknown scheme " + name);
}

const char *scheme_name(SchemeType scheme)
{
    switch (scheme)
    {
    case SchemeType::Quanpin:
        return "quanpin";
    case SchemeType::Shuangpin:
        return "shuangpin";
    case SchemeType::Wubi:
        return "wubi";
    case SchemeType::JapaneseRomaji:
        return "japanese";
    }
    return "?";
}

const char *local_mode_name(LocalInputMode mode)
{
    switch (mode)
    {
    case LocalInputMode::None:
        return "none";
    case LocalInputMode::Unicode:
        return "unicode";
    case LocalInputMode::DateTime:
        return "date_time";
    case LocalInputMode::QuickPhrase:
        return "quick_phrase";
    case LocalInputMode::Emoji:
        return "emoji";
    case LocalInputMode::Kaomoji:
        return "kaomoji";
    case LocalInputMode::SuperJianpin:
        return "super_jianpin";
    case LocalInputMode::TemporaryEnglish:
        return "temporary_english";
    case LocalInputMode::TemporaryJapanese:
        return "temporary_japanese";
    }
    return "?";
}

FrequencyAdjustmentMode frequency_from(const std::string &name)
{
    if (name == "disabled")
        return FrequencyAdjustmentMode::Disabled;
    if (name == "pin")
        return FrequencyAdjustmentMode::Pin;
    if (name == "halve")
        return FrequencyAdjustmentMode::Halve;
    if (name == "linear")
        return FrequencyAdjustmentMode::Linear;
    if (name == "promote")
        return FrequencyAdjustmentMode::Promote;
    fail("unknown frequency mode " + name);
}

Command command_from(const std::string &name)
{
    static const std::pair<const char *, Command> table[] = {
        {"Backspace", Command::Backspace},         {"CommitCandidate", Command::CommitCandidate},
        {"CommitRaw", Command::CommitRaw},         {"Cancel", Command::Cancel},
        {"MoveLeft", Command::MoveLeft},           {"MoveRight", Command::MoveRight},
        {"MoveHome", Command::MoveHome},           {"MoveEnd", Command::MoveEnd},
        {"DeleteForward", Command::DeleteForward}, {"CycleKanaVariant", Command::CycleKanaVariant},
        {"CommitReading", Command::CommitReading},
    };
    for (const auto &[text, value] : table)
        if (name == text)
            return value;
    fail("unknown command " + name);
}

CandidateSource source_from(const ordered_json &value)
{
    if (value.is_number_integer())
        return static_cast<CandidateSource>(value.get<int>());
    const auto name = value.get<std::string>();
    if (name == "cloud")
        return CandidateSource::CloudSuggestion;
    if (name == "ai")
        return CandidateSource::AiSuggestion;
    fail("unknown online source " + name);
}

PersonalDictionaryKind kind_from(const std::string &name)
{
    if (name == "pinyin")
        return PersonalDictionaryKind::Pinyin;
    if (name == "wubi")
        return PersonalDictionaryKind::Wubi;
    if (name == "quick_phrase")
        return PersonalDictionaryKind::QuickPhrase;
    if (name == "english")
        return PersonalDictionaryKind::English;
    fail("unknown dictionary kind " + name);
}

const char *kind_name(PersonalDictionaryKind kind)
{
    switch (kind)
    {
    case PersonalDictionaryKind::Pinyin:
        return "pinyin";
    case PersonalDictionaryKind::Wubi:
        return "wubi";
    case PersonalDictionaryKind::QuickPhrase:
        return "quick_phrase";
    case PersonalDictionaryKind::English:
        return "english";
    }
    return "?";
}

std::optional<PersonalDictionaryEntry> entry_from(const ordered_json &value)
{
    if (value.is_null())
        return std::nullopt;
    PersonalDictionaryEntry entry;
    entry.kind = kind_from(value.at("kind").get<std::string>());
    entry.key = value.at("key").get<std::string>();
    entry.value = value.at("value").get<std::string>();
    if (value.contains("weight"))
        entry.weight = value.at("weight").get<std::int64_t>();
    return entry;
}

ordered_json entry_json(const PersonalDictionaryEntry &entry)
{
    return ordered_json{{"kind", kind_name(entry.kind)}, {"key", entry.key}, {"value", entry.value},
                        {"weight", entry.weight}};
}

const ShuangpinProfile &profile_from(const std::string &name)
{
    if (name == "xiaohe")
        return GetXiaoheShuangpinProfile();
    if (name == "ziranma")
        return GetZiranmaShuangpinProfile();
    if (name == "shoudao")
        return GetShoudaoShuangpinProfile();
    if (name == "microsoft")
        return GetMicrosoftShuangpinProfile();
    fail("unknown shuangpin profile " + name);
}

// Only fields present in this reference's SessionOptions (session.h:12-54). learning_undo is a dropped feature and is
// rejected rather than silently recorded.
void apply_options(SessionOptions &options, const ordered_json &value)
{
    for (const auto &[key, v] : value.items())
    {
        if (key == "scheme")
            options.scheme = scheme_from(v.get<std::string>());
        else if (key == "shuangpin_profile")
            options.shuangpin_profile = profile_from(v.get<std::string>());
        else if (key == "shuangpin_preedit_uses_raw")
            options.shuangpin_preedit_uses_raw = v.get<bool>();
        else if (key == "helpcode_schema")
            options.helpcode_schema = v.get<std::string>();
        else if (key == "autocorrect_types")
            options.autocorrect_types = v.get<unsigned>();
        else if (key == "helpcode")
            options.helpcode = v.get<bool>();
        else if (key == "chinese_punctuation")
            options.chinese_punctuation = v.get<bool>();
        else if (key == "paired_punctuation")
            options.paired_punctuation = v.get<bool>();
        else if (key == "punctuation_lock")
            options.punctuation_lock = v.get<int>();
        else if (key == "learning")
            options.learning = v.get<bool>();
        else if (key == "sentence_alternatives")
            options.sentence_alternatives = v.get<bool>();
        else if (key == "fuzzy_pinyin_rules")
            options.fuzzy_pinyin.rules = v.get<std::uint32_t>();
        else if (key == "frequency")
        {
            if (v.contains("mode"))
                options.frequency.mode = frequency_from(v.at("mode").get<std::string>());
            if (v.contains("trigger_count"))
                options.frequency.trigger_count = v.at("trigger_count").get<int>();
            if (v.contains("linear_step"))
                options.frequency.linear_step = v.at("linear_step").get<int>();
        }
        else if (key == "local_modes")
        {
            auto &m = options.local_modes;
            for (const auto &[name, flag] : v.items())
            {
                const bool on = flag.get<bool>();
                if (name == "unicode")
                    m.unicode = on;
                else if (name == "date_time")
                    m.date_time = on;
                else if (name == "quick_phrase")
                    m.quick_phrase = on;
                else if (name == "emoji")
                    m.emoji = on;
                else if (name == "kaomoji")
                    m.kaomoji = on;
                else if (name == "super_jianpin")
                    m.super_jianpin = on;
                else if (name == "temporary_english")
                    m.temporary_english = on;
                else if (name == "temporary_japanese")
                    m.temporary_japanese = on;
                else
                    fail("unknown local mode " + name);
            }
        }
        else if (key == "english")
        {
            if (v.contains("mixed_candidates"))
                options.english.mixed_candidates = v.at("mixed_candidates").get<bool>();
            if (v.contains("minimum_prefix"))
                options.english.minimum_prefix = v.at("minimum_prefix").get<std::size_t>();
        }
        else if (key == "expressive")
        {
            if (v.contains("emoji_candidates"))
                options.expressive.emoji_candidates = v.at("emoji_candidates").get<bool>();
            if (v.contains("kaomoji_candidates"))
                options.expressive.kaomoji_candidates = v.at("kaomoji_candidates").get<bool>();
        }
        else if (key == "wubi_mixed_pinyin")
            options.wubi.mixed_pinyin = v.get<bool>();
        else if (key == "personal_context")
            options.personal_context = v.get<bool>();
        else if (key == "page_size" || key == "content_id")
            continue; // recorder-side
        else
            fail("unknown or unsupported option " + key);
    }
}

// Product options for the real-dictionary goldens: the bridge's prepare_options defaults (crates/engine-bridge/native/bridge.cpp:698-733) mapped through options_for (:354-406), host-api's unconditional sentence_alternatives=true (crates/host-api/src/lib.rs:365), and learning off so every row is a cold start (as convert_eval and eval_sentences do).
void apply_product_options(SessionOptions &options)
{
    options.scheme = SchemeType::Quanpin;
    options.shuangpin_profile = GetXiaoheShuangpinProfile();
    options.learning = false;
    options.autocorrect_types = 0;
    options.fuzzy_pinyin.rules = 0;
    options.helpcode = true;
    options.helpcode_schema = "ziranma";
    options.chinese_punctuation = true;
    options.paired_punctuation = true;
    options.punctuation_lock = 0;
    options.frequency = {FrequencyAdjustmentMode::Promote, 1, 1};
    options.english = {true, 5};
    options.expressive = {false, false};
    options.local_modes = {true, true, true, true, true, true, true, true};
    options.wubi.mixed_pinyin = false;
    options.sentence_alternatives = true;
}

ordered_json result_json(const KeyResult &result)
{
    ordered_json out{{"handled", result.handled}};
    if (result.commit)
        out["commit"] = *result.commit;
    if (result.diagnostic)
        out["diagnostic"] = scrub(*result.diagnostic);
    return out;
}

ordered_json candidate_json(const WordItem &item, const SessionSnapshot &view, std::size_t index)
{
    ordered_json out{{"word", item.word},
                     {"pinyin", item.pinyin},
                     {"canonical_pinyin", item.canonical_pinyin},
                     {"source", static_cast<int>(item.source)},
                     {"scheme", scheme_name(item.scheme)},
                     {"weight", item.weight}};
    if (item.fixed_position)
        out["fixed_position"] = item.fixed_position;
    if (item.fuzzy)
        out["fuzzy"] = true;
    if (!item.corrected_from.empty())
        out["corrected_from"] = item.corrected_from;
    if (item.sentence_association)
        out["sentence_association"] = true;
    if (!item.sentence_words.empty())
        out["sentence_words"] = item.sentence_words;
    if (index < view.candidate_annotations.size() && !view.candidate_annotations[index].empty())
        out["annotation"] = view.candidate_annotations[index];
    if (index < view.candidate_answers_key.size())
        out["answers_key"] = static_cast<bool>(view.candidate_answers_key[index]);
    return out;
}

struct Scenario
{
    fs::path root;
    fs::path resources;
    SessionOptions options;
    std::string content_id = "v1";
    std::size_t page_size = 0;
    std::size_t page = 0;
    std::unique_ptr<Session> session;
    RuntimePaths paths;

    void open()
    {
        paths = prepare_runtime_paths(resources, root / "user", root / "cache", content_id);
        auto options_copy = options;
        options_copy.paths = paths;
        session = std::make_unique<Session>(options_copy);
        page = 0;
    }

    void close()
    {
        session.reset();
        user_dictionary::PersonalNgramStore::flush_all();
    }

    ordered_json snapshot() const
    {
        const auto view = session->snapshot();
        ordered_json out{{"scheme", scheme_name(view.scheme)},
                         {"local_mode", local_mode_name(view.local_mode)},
                         {"preedit", view.preedit},
                         {"raw_segmentation", view.raw_segmentation},
                         {"normalized_segmentation", view.normalized_segmentation},
                         {"editing_text", view.editing_text},
                         {"caret", view.caret_position},
                         {"dedicated_english", view.dedicated_english},
                         {"shuangpin_profile", view.shuangpin_profile}};
        if (view.answered_by_pinyin_fallback)
            out["answered_by_pinyin_fallback"] = true;
        if (view.wubi_unique_four_code)
            out["wubi_unique_four_code"] = true;
        if (!view.nine_key_spellings.empty())
            out["nine_key_spellings"] = view.nine_key_spellings;
        const auto boundaries = session->segment_raw_boundaries();
        if (!boundaries.empty())
            out["segment_boundaries"] = boundaries;
        out["candidate_count"] = view.candidates.size();
        ordered_json candidates = ordered_json::array();
        for (std::size_t i = 0; i < view.candidates.size(); ++i)
            candidates.push_back(candidate_json(view.candidates[i], view, i));
        out["candidates"] = std::move(candidates);
        if (page_size)
            out["page"] = ordered_json{{"index", page}, {"size", page_size}};
        if (const auto query = session->online_query())
        {
            // generation / identity / session_id are per-process counters, not behaviour.
            out["online_query"] = ordered_json{{"scheme", scheme_name(query->scheme)},
                                               {"query_text", query->query_text},
                                               {"cache_key", query->cache_key},
                                               {"pinyin_segments", query->pinyin_segments},
                                               {"cloud_eligible", query->cloud_eligible},
                                               {"ai_eligible", query->ai_eligible}};
        }
        return out;
    }
};

void prepare_fixture(const ordered_json &fixture, const fs::path &resources)
{
    fs::create_directories(resources);
    // Reference tests that call EnglishDictionary::ensure_schema before inserting their English rows set this.
    if (fixture.value("english_schema", false) &&
        !EnglishDictionary::ensure_schema((resources / assets::english_dictionary).string()))
        fail("english schema failed");
    if (fixture.contains("databases"))
        for (const auto &[name, sql] : fixture.at("databases").items())
            exec_sql(resources / name, sql.get<std::string>());
    if (fixture.contains("files"))
        for (const auto &[name, text] : fixture.at("files").items())
        {
            const auto path = resources / name;
            fs::create_directories(path.parent_path());
            std::ofstream stream(path, std::ios::binary);
            stream << text.get<std::string>();
            if (!stream)
                fail("cannot write fixture file " + name);
        }
    for (const auto *name : {assets::main_dictionary, assets::english_dictionary})
        if (!fs::exists(resources / name))
        {
            if (std::string(name) == assets::english_dictionary)
            {
                if (!EnglishDictionary::ensure_schema((resources / name).string()))
                    fail("english schema failed");
            }
            else
                exec_sql(resources / name, "CREATE TABLE golden_empty(x); DROP TABLE golden_empty;");
        }
}

std::size_t index_arg(const ordered_json &step, const Scenario &scenario)
{
    const auto &arg = step.at("arg");
    if (arg.is_number_integer())
        return arg.get<std::size_t>();
    // {"word": "..."} selects by word, like the reference tests' candidate_index helpers.
    const auto word = arg.at("word").get<std::string>();
    const auto view = scenario.session->snapshot();
    for (std::size_t i = 0; i < view.candidates.size(); ++i)
        if (view.candidates[i].word == word)
            return i;
    fail("step selects a word that is not a candidate: " + word);
}

ordered_json run_step(Scenario &scenario, const ordered_json &step)
{
    const auto op = step.at("op").get<std::string>();
    const auto before = scenario.session->snapshot();
    const auto before_text = before.editing_text;
    std::vector<std::string> before_words;
    for (const auto &item : before.candidates)
        before_words.push_back(item.word);
    auto &session = *scenario.session;
    const auto arg = step.contains("arg") ? step.at("arg") : ordered_json();
    ordered_json out{{"op", op}};
    if (!arg.is_null())
        out["arg"] = arg;
    ordered_json result;
    bool snapshot = true;

    if (op == "type")
    {
        // Each character is one Session::character call; the step reports the combined outcome.
        const auto text = arg.get<std::string>();
        ordered_json unhandled = ordered_json::array();
        std::string commit;
        bool committed = false;
        ordered_json diagnostics = ordered_json::array();
        for (std::size_t i = 0; i < text.size(); ++i)
        {
            const auto r = session.character(text[i]);
            if (!r.handled)
                unhandled.push_back(i);
            if (r.commit)
            {
                committed = true;
                commit += *r.commit;
            }
            if (r.diagnostic)
                diagnostics.push_back(scrub(*r.diagnostic));
        }
        result = ordered_json{{"handled", unhandled.empty()}};
        if (!unhandled.empty())
            result["unhandled_at"] = unhandled;
        if (committed)
            result["commit"] = commit;
        if (!diagnostics.empty())
            result["diagnostics"] = diagnostics;
    }
    else if (op == "char")
    {
        const auto text = arg.get<std::string>();
        if (text.size() != 1)
            fail("char takes one byte");
        result = result_json(session.character(text[0], step.value("shift", false)));
    }
    else if (op == "command")
        result = result_json(session.command(command_from(arg.get<std::string>())));
    else if (op == "candidate_key")
        result = result_json(session.candidate_key(arg.get<std::string>().at(0)));
    else if (op == "punctuation")
        result = result_json(session.punctuation(arg.get<std::string>().at(0)));
    else if (op == "select")
        result = result_json(session.select(index_arg(step, scenario)));
    else if (op == "select_on_page")
    {
        if (!scenario.page_size)
            fail("select_on_page needs options.page_size");
        result = result_json(session.select(scenario.page * scenario.page_size + arg.get<std::size_t>()));
    }
    else if (op == "select_edge")
        result = result_json(session.select_edge(index_arg(step, scenario), step.at("edge") == "first"
                                                                               ? CandidateEdge::FirstHan
                                                                               : CandidateEdge::LastHan));
    else if (op == "pin")
        result = result_json(session.pin(index_arg(step, scenario)));
    else if (op == "remove")
        result = result_json(session.remove(index_arg(step, scenario)));
    else if (op == "fix_position")
        result = result_json(session.fix_position(index_arg(step, scenario), step.at("position").get<int>()));
    else if (op == "clear_position")
        result = result_json(session.clear_position(index_arg(step, scenario)));
    else if (op == "finish")
        result = result_json(arg.is_null() ? session.finish() : session.finish(arg.get<std::size_t>()));
    else if (op == "page")
    {
        // Paging is host behaviour (input-runtime); the engine hands back the whole list. This only moves the recorded
        // view so page-relative selections can be scripted.
        if (!scenario.page_size)
            fail("page needs options.page_size");
        if (arg == "next")
            ++scenario.page;
        else if (arg == "prev")
            scenario.page = scenario.page ? scenario.page - 1 : 0;
        else
            scenario.page = arg.get<std::size_t>();
    }
    else if (op == "switch_scheme")
        session.switch_scheme(scheme_from(arg.get<std::string>()));
    else if (op == "set_helpcode_schema")
        result = ordered_json{{"accepted", session.set_helpcode_schema(arg.get<std::string>())}};
    else if (op == "set_helpcode_enabled")
        session.set_helpcode_enabled(arg.get<bool>());
    else if (op == "set_dedicated_english")
        session.set_dedicated_english(arg.get<bool>());
    else if (op == "set_wubi_mixed_pinyin")
        session.set_wubi_mixed_pinyin(arg.get<bool>());
    else if (op == "set_chinese_punctuation_enabled")
        session.set_chinese_punctuation_enabled(arg.get<bool>());
    else if (op == "set_punctuation_lock")
        session.set_punctuation_lock(arg.get<int>());
    else if (op == "set_paired_punctuation_enabled")
        session.set_paired_punctuation_enabled(arg.get<bool>());
    else if (op == "balance_paired_punctuation_after_auto_close")
        session.balance_paired_punctuation_after_auto_close(arg.get<std::string>().at(0));
    else if (op == "set_nine_key_enabled")
        session.set_nine_key_enabled(arg.get<bool>());
    else if (op == "choose_nine_key_spelling")
    {
        // {"spelling": "ni"} picks by text, like the reference test's std::find over nine_key_spellings.
        std::size_t index = 0;
        if (arg.is_number_integer())
            index = arg.get<std::size_t>();
        else
        {
            const auto spellings = session.snapshot().nine_key_spellings;
            const auto found = std::find(spellings.begin(), spellings.end(), arg.at("spelling").get<std::string>());
            if (found == spellings.end())
                fail("spelling not offered");
            index = static_cast<std::size_t>(found - spellings.begin());
        }
        result = result_json(session.choose_nine_key_spelling(index));
    }
    else if (op == "set_personal_context_enabled")
        session.set_personal_context_enabled(arg.get<bool>());
    else if (op == "reset_cache")
        session.reset_cache();
    else if (op == "reset_context")
        session.reset_context();
    else if (op == "expand_initial_candidates")
        result = ordered_json{{"grew", session.expand_initial_candidates()}};
    else if (op == "apply_online_candidates")
    {
        // Uses the live query, as a host does when its request returns before the composition changes.
        const auto query = session.online_query();
        if (!query)
            result = ordered_json{{"applied", false}, {"no_query", true}};
        else
        {
            const auto words = arg.get<std::vector<std::string>>();
            result = ordered_json{{"applied", session.apply_online_candidates(*query, words, source_from(step.at("source")))}};
        }
    }
    else if (op == "reopen")
    {
        // A new Session on the same user data: same generation is re-prepared, which replays the journal.
        scenario.close();
        scenario.open();
    }
    else if (op == "new_generation")
    {
        scenario.close();
        scenario.content_id = arg.get<std::string>();
        scenario.open();
    }
    else if (op == "dump_journal")
    {
        snapshot = false;
        out["journal"] = dump_journal(scenario.paths.user(assets::user_journal));
    }
    else if (op == "query")
    {
        // Read-only SQL against the live generation copy of a dictionary (msime-pinyin.db / msime-english.db) or the journal.
        snapshot = false;
        user_dictionary::PersonalNgramStore::flush_all();
        const auto db = step.at("db").get<std::string>();
        const auto path = db == assets::user_journal ? scenario.paths.user(db) : scenario.paths.dictionary(db);
        out["rows"] = query_rows(path, arg.get<std::string>());
    }
    else if (op == "validate_entry")
    {
        snapshot = false;
        const auto validation = validate_personal_dictionary_entry(*entry_from(arg));
        result = validation.entry ? ordered_json{{"entry", entry_json(*validation.entry)}}
                                  : ordered_json{{"error", validation.error}};
    }
    else if (op == "dict_edit")
    {
        const auto previous = entry_from(arg.value("previous", ordered_json()));
        const auto replacement = entry_from(arg.value("replacement", ordered_json()));
        const auto edit =
            edit_personal_dictionary(scenario.paths, previous, replacement, arg.value("request_id", std::string()));
        result = ordered_json{{"success", edit.success}};
        if (!edit.error.empty())
            result["error"] = scrub(edit.error);
    }
    else if (op == "dict_list")
    {
        snapshot = false;
        const auto page = personal_dictionary_entries(scenario.paths, arg.value("offset", std::size_t{0}),
                                                      arg.value("limit", std::size_t{100}),
                                                      arg.value("include_learned", false));
        ordered_json entries = ordered_json::array();
        for (const auto &entry : page.entries)
            entries.push_back(entry_json(entry));
        result = ordered_json{{"entries", entries}, {"has_more", page.has_more}};
        if (!page.error.empty())
            result["error"] = scrub(page.error);
    }
    else
        fail("unknown op " + op);

    // A host starts again from the first page whenever the composition or its candidates change.
    if (op != "page" && op != "dump_journal" && op != "query" && op != "validate_entry" && op != "dict_list")
    {
        const auto after = scenario.session->snapshot();
        std::vector<std::string> words;
        for (const auto &item : after.candidates)
            words.push_back(item.word);
        if (after.editing_text != before_text || words != before_words)
            scenario.page = 0;
    }
    if (!result.is_null())
        out["result"] = result;
    if (snapshot)
        out["snapshot"] = scenario.snapshot();
    return out;
}

int record_scenario(const fs::path &scenario_path, const fs::path &work, const fs::path &out_path)
{
    std::ifstream in(scenario_path);
    const auto spec = ordered_json::parse(in);
    const auto name = spec.at("name").get<std::string>();
    Scenario scenario;
    scenario.root = fs::absolute(work) / name;
    fs::remove_all(scenario.root);
    fs::create_directories(scenario.root);
    // Longest first, so the canonical /private/tmp spelling is replaced before its /tmp alias.
    g_root_texts = {fs::weakly_canonical(scenario.root).string(), scenario.root.string()};
    const auto &fixture = spec.at("fixture");
    if (fixture.contains("resources"))
        fail("scripted scenarios use inline fixtures; real resources go through the `real` mode");
    scenario.resources = scenario.root / "resources";
    prepare_fixture(fixture, scenario.resources);
    const auto options = spec.value("options", ordered_json::object());
    apply_options(scenario.options, options);
    scenario.page_size = options.value("page_size", std::size_t{0});
    scenario.content_id = options.value("content_id", std::string("v1"));

    ordered_json steps = ordered_json::array();
    scenario.open();
    steps.push_back(ordered_json{{"op", "open"}, {"snapshot", scenario.snapshot()}});
    for (const auto &step : spec.at("steps"))
        steps.push_back(run_step(scenario, step));
    scenario.close();
    ordered_json out{{"name", name}, {"steps", steps}};
    if (spec.value("dump_journal_at_end", false))
        out["final_journal"] = dump_journal(scenario.paths.user(assets::user_journal));
    std::ofstream file(out_path, std::ios::binary);
    file << out.dump(1, ' ', false) << '\n';
    fs::remove_all(scenario.root);
    return file ? 0 : 1;
}

struct EvalRow
{
    std::string id;
    std::string input;
    std::string gold;
};

// TSV columns: id, input, gold, ...; '#' lines and the `id` header row are skipped.
std::vector<EvalRow> load_eval(const fs::path &path)
{
    std::vector<EvalRow> rows;
    std::ifstream file(path);
    std::string line;
    while (std::getline(file, line))
    {
        if (!line.empty() && line.back() == '\r')
            line.pop_back();
        if (line.empty() || line[0] == '#')
            continue;
        std::vector<std::string> columns;
        std::stringstream stream(line);
        std::string column;
        while (std::getline(stream, column, '\t'))
            columns.push_back(column);
        if (columns.size() < 3 || columns[0] == "id")
            continue;
        rows.push_back({columns[0], columns[1], columns[2]});
    }
    return rows;
}

int record_real(const fs::path &resources, const fs::path &tsv, const fs::path &out_path, const fs::path &work,
                std::size_t subset, std::size_t top)
{
    auto rows = load_eval(tsv);
    if (rows.empty())
        fail("no rows in " + tsv.string());
    if (subset && subset < rows.size())
    {
        // Equidistant: row floor(i * N / subset) for i in [0, subset).
        std::vector<EvalRow> picked;
        for (std::size_t i = 0; i < subset; ++i)
            picked.push_back(rows[i * rows.size() / subset]);
        rows = std::move(picked);
    }
    const auto root = fs::absolute(work) / ("real-" + tsv.stem().string());
    fs::remove_all(root);
    fs::create_directories(root);
    SessionOptions options;
    apply_product_options(options);
    options.paths = prepare_runtime_paths(fs::absolute(resources), root / "user", root / "cache", "real");
    Session session(options);
    std::ofstream out(out_path, std::ios::binary);
    for (const auto &row : rows)
    {
        bool handled = true;
        for (const char letter : row.input)
            handled = session.character(letter).handled && handled;
        const auto view = session.snapshot();
        ordered_json candidates = ordered_json::array();
        for (std::size_t i = 0; i < view.candidates.size() && i < top; ++i)
        {
            const auto &item = view.candidates[i];
            ordered_json entry = ordered_json::array({item.word, static_cast<int>(item.source)});
            if (!item.corrected_from.empty())
                entry.push_back(item.corrected_from);
            candidates.push_back(std::move(entry));
        }
        ordered_json line{{"id", row.id}, {"input", row.input}, {"gold", row.gold}};
        if (!handled)
            line["unhandled"] = true;
        line["n"] = view.candidates.size();
        line["c"] = std::move(candidates);
        out << line.dump(-1, ' ', false) << '\n';
        session.command(Command::Cancel);
        session.reset_context();
    }
    fs::remove_all(root);
    return out ? 0 : 1;
}
} // namespace

int main(int argc, char **argv)
{
    try
    {
        const std::string mode = argc > 1 ? argv[1] : "";
        if (mode == "scenario" && argc == 5)
            return record_scenario(argv[2], argv[3], argv[4]);
        if (mode == "real" && argc >= 6)
        {
            std::size_t subset = 0;
            std::size_t top = 9;
            for (int i = 6; i + 1 < argc; i += 2)
            {
                const std::string flag = argv[i];
                if (flag == "--subset")
                    subset = std::stoul(argv[i + 1]);
                else if (flag == "--top")
                    top = std::stoul(argv[i + 1]);
                else
                    fail("unknown flag " + flag);
            }
            return record_real(argv[2], argv[3], argv[4], argv[5], subset, top);
        }
        std::cerr << "usage: recorder scenario <scenario.json> <work-root> <out.json>\n"
                     "       recorder real <resources-dir> <tsv> <out.jsonl> <work-root> [--subset N] [--top N]\n";
        return 2;
    }
    catch (const std::exception &error)
    {
        std::cerr << "recorder: " << scrub(error.what()) << '\n';
        return 1;
    }
}
