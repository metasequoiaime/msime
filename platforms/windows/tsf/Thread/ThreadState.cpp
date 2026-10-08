#include "FanyDefines.h"
#include "Ipc.h"

// One definition for dynamically initialized TLS. MinGW emits duplicate TLS
// initialization symbols when these strings are inline in shared headers.
// Storage remains per-thread; no composition state is shared across threads.
namespace Global {
thread_local std::wstring PinyinString;
thread_local std::wstring current_process_name;
thread_local CandidateOverlayDecision GameOverlayDecision;
} // namespace Global

namespace GlobalIme {
thread_local std::wstring word_for_creating_word;
thread_local std::wstring pending_create_word_preedit;
} // namespace GlobalIme
