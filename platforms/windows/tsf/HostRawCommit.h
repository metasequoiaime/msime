#pragma once
#include "EngineSessionAdapter.h"

namespace msime::tsf {
enum class RawCommitStatus { Completed, Unhandled, Failed };

// The host owns input state; the editor write must precede UI teardown.
//
// Japanese asks first. Romaji is not what the user typed; かな is, and the Engine has a command
// for each: MSIME_COMMIT_READING gives the kana and MSIME_COMMIT_RAW gives the letters back. The
// reading command answers nothing at all unless the scheme is Japanese and a composition is open
// (`InputSession::CommitReading` returns an empty result otherwise), so asking for it first needs
// no knowledge of the scheme here - an unhandled answer leaves the composition untouched and the
// raw command runs exactly as before. Without this, Enter in Japanese put `nihon` in the document
// where the user meant にほん.
//
// Korean answers the same command with its open syllable and handled=false, because a Korean key that ends a syllable still does its own work afterwards. Only the commit matters here, so the handled flag is not required: the Engine has already let go of that syllable, and asking for the raw letters next would find nothing.
template<class Host, class Insert, class Cleanup>
RawCommitStatus CommitHostRaw(Host &host, Insert insert, Cleanup cleanup, std::string *error) {
    std::string raw;
    EngineResult result;
    std::string reading;
    EngineResult kana;
    if (host.command(MSIME_COMMIT_READING, &reading, nullptr) &&
        EngineSessionAdapter::parse_result(reading, &kana, nullptr) && kana.has_commit &&
        !kana.commit.empty()) {
        if (!insert(kana.commit)) return RawCommitStatus::Failed;
        cleanup();
        return RawCommitStatus::Completed;
    }
    if (!host.command(MSIME_COMMIT_RAW, &raw, error) ||
        !EngineSessionAdapter::parse_result(raw, &result, error)) return RawCommitStatus::Failed;
    if (!result.handled && !result.has_commit &&
        (!result.view.phrase_prefix.empty() || !result.view.editing_text.empty() || !result.view.preedit.empty())) return RawCommitStatus::Unhandled;
    if (result.has_commit && !result.commit.empty() && !insert(result.commit)) return RawCommitStatus::Failed;
    cleanup();
    return RawCommitStatus::Completed;
}
}
