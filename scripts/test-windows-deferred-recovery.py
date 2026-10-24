#!/usr/bin/env python3
"""编译执行实际的恢复检查点方法；只替换 Win32/COM 环境，不复制生产逻辑。"""

from __future__ import annotations

import pathlib
import shutil
import subprocess
import tempfile

ROOT = pathlib.Path(__file__).resolve().parent.parent
SOURCE = ROOT / "platforms/windows/tsf/Key/KeyEventSink.cpp"

HARNESS = r'''
#include <algorithm>
#include <atomic>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <deque>
#include <utility>
#include <string>
#include "InputSchemeTraits.h"
namespace Global {
std::atomic<int> InputModeScheme{msime::windows::scheme::Quanpin};
std::atomic_bool TsfDiagnosticLogEnabled{false};
}
// 生产代码里的诊断日志在测试里关闭，记录进空实现即可。
void QueueTsfDiagnosticLog(const std::wstring &) {}
inline std::wstring BoolFlag(bool value) { return value ? L"1" : L"0"; }
using std::min;
using WCHAR = wchar_t;
using SHORT = short;
using WPARAM = std::uintptr_t;
#define _In_
#define _In_opt_
constexpr WPARAM VK_PACKET = 0xe7, VK_LEFT = 0x25;
using UINT = unsigned;
// 生产按键类型
constexpr int FANY_IME_NO_REQUEST_ID = 0, S_OK = 0, S_FALSE = 1, E_FAIL = -1;
constexpr unsigned kMaxDeferredKeyReplayAttempts = 8, WM_IpcReconnect = 1;
void *testWindow = nullptr;
bool IsWindow(void *) { return false; }
void PostOwnerMessageWithSyncFallback(void *, unsigned) {}
constexpr std::size_t MAX_DEFERRED_KEY_DOWN_COUNT = 10;
unsigned LOBYTE(SHORT value) { return static_cast<unsigned short>(value) & 0xff; }
unsigned HIBYTE(SHORT value) { return static_cast<unsigned short>(value) >> 8; }
SHORT VkKeyScanW(WCHAR value) {
    if (value >= L'a' && value <= L'z') return static_cast<SHORT>(value - L'a' + 'A');
    if (value >= L'A' && value <= L'Z') return static_cast<SHORT>(0x100 | value);
    return -1;
}
struct ITfContext {
    int refs = 1;
    void AddRef() { ++refs; }
    void Release() { --refs; }
};
struct CStringRange {
    std::wstring text;
    std::size_t GetLength() const { return text.size(); }
    const WCHAR *Get() const { return text.data(); }
};
struct CompositionState { std::wstring rawInput; std::size_t caret = 0; };
struct Engine {
    CStringRange legacy;
    std::size_t legacyCaret = 0;
    CompositionState current;
    CStringRange &GetKeystrokeBuffer() { return legacy; }
    std::size_t GetCaretPosition() const { return legacyCaret; }
    CompositionState GetKeyCompositionState() const { return current; }
    std::size_t GetVirtualKeyLength() const { return legacy.text.size(); }
    enum class PreservedKeyAction { None, ToggleImeMode };
    PreservedKeyAction GetPreservedKeyAction(int) const { return PreservedKeyAction::None; }
};
using KeyState = _KEYSTROKE_STATE;
struct DeferredKeyDown {
    enum class Kind { KeyDown, PreservedKey, ApplicationText };
    Kind kind{};
    ITfContext *context = nullptr;
    WPARAM wParam = 0;
    WCHAR translatedWch = 0;
    unsigned modifiersDown = 0;
    KeyState keyState{};
    std::uint64_t focusGeneration = 0;
    int preservedKey = 0;
    unsigned replayAttempts = 0;
};
using CCompositionProcessorEngine = Engine;
template<typename... Args> void DebugTsfIssue47(Args...) {}
// 生产前缀规则
struct CMetasequoiaIME {
    Engine *_pCompositionProcessorEngine = nullptr;
    std::deque<DeferredKeyDown> _deferredAppliedPrefix;
    std::uint64_t _deferredKeyFocusGeneration = 7;
    bool _RefreshDeferredRecoveryPrefix(ITfContext *);
    std::deque<DeferredKeyDown> _deferredKeyDowns;
    DeferredKeyDown _deferredKeyInFlight;
    bool _hasDeferredKeyInFlight = false;
    std::uint64_t _deferredKeyReplayToken = 0;
    bool _deferredKeyClassificationPending = false;
    bool _deferredKeyProjectionValid = false;
    std::size_t _deferredProjectedInputLength = 0, _deferredProjectedCaret = 0;
    std::wstring _deferredProjectedRawInput;
    bool _deferredProjectedCandidateActive = false, _deferredProjectedUnicodeMode = false,
         _deferredProjectedPhraseActive = false, _deferredProjectedUrlMode = false, _deferredProjectedKoreanHanjaListOpen = false;
    int drainSchedules = 0;
    bool _IsComposing() const { return ! _pCompositionProcessorEngine->current.rawInput.empty(); }
    void _TryLeaveServerUnavailableFallback() {}
    void _ScheduleDeferredKeyDownDrain() { ++drainSchedules; }
    void _CompleteDeferredKeyReplay(std::uint64_t);
    ITfContext *_pContext = nullptr;
    void _EnsureDeferredKeyProjection() { _deferredKeyProjectionValid = true; }
    void _ArmDeferredRecoveryForTransport(ITfContext *);
    void _RetryDeferredKeyReplay(std::uint64_t);
    void *_msgWndHandle = testWindow;
    void _ClearDeferredKeyDowns() { std::abort(); }
};
void MarkNamedpipeSessionDirtyForOwner(CMetasequoiaIME *) {}
'''

TESTS = r'''
int main() {
    int failures = 0;
    const auto check = [&](bool value, const char *message) {
        if (!value) { std::fprintf(stderr, "FAIL: %s\n", message); ++failures; }
    };
    Engine engine;
    ITfContext context;
    CMetasequoiaIME ime;
    ime._pCompositionProcessorEngine = &engine;
    engine.current = {L"nihao", 2};
    check(ime._RefreshDeferredRecoveryPrefix(&context), "checkpoint created");
    const auto &keys = ime._deferredAppliedPrefix;
    std::wstring text;
    std::size_t left = 0;
    for (const auto &key : keys) {
        if (key.keyState.Function == FUNCTION_INPUT) text += key.translatedWch;
        if (key.keyState.Function == FUNCTION_MOVE_LEFT) ++left;
        check(key.context == &context && key.focusGeneration == 7 &&
              key.keyState.Category == CATEGORY_COMPOSING, "keys retain context and focus ownership");
    }
    check(text == L"nihao", "host raw survives an empty legacy buffer");
    check(left == 3, "host caret survives an empty legacy buffer");
    check(context.refs == 9, "each queued key retains the context");

    engine.legacy.text = L"obsolete";
    engine.legacyCaret = 8;
    engine.current = {L"new", 1};
    check(ime._RefreshDeferredRecoveryPrefix(&context), "replacement checkpoint created");
    check(keys.size() == 5 && keys.front().translatedWch == L'n' &&
          keys.back().keyState.Function == FUNCTION_MOVE_LEFT, "current host replaces stale checkpoint and legacy text");
    check(context.refs == 6, "replacement releases previous key references");

    engine.current = {};
    check(ime._RefreshDeferredRecoveryPrefix(&context) && keys.empty(), "empty host cannot resurrect legacy text");
    check(context.refs == 1, "empty checkpoint releases all references");

    engine.current = {L"abcdef", 0};
    check(!ime._RefreshDeferredRecoveryPrefix(&context) && keys.empty() && context.refs == 1,
          "over-budget host checkpoint is rejected atomically");
    engine.current = {L"abcde", 0};
    check(ime._RefreshDeferredRecoveryPrefix(&context) && keys.size() == 10 && context.refs == 11,
          "exact-budget host checkpoint retains text and caret");
    engine.current = {L"A", 99};
    check(ime._RefreshDeferredRecoveryPrefix(&context) && keys.size() == 1 &&
          keys.front().wParam == L'A' && keys.front().modifiersDown == 1,
          "caret is clamped and uppercase replay retains Shift");
    engine.current = {L"?", 1};
    check(ime._RefreshDeferredRecoveryPrefix(&context) && keys.size() == 1 &&
          keys.front().wParam == VK_PACKET && keys.front().translatedWch == L'?',
          "unmapped characters retain the packet fallback");
    // 本地 reset 会将这些方案的现有组字上屏，检查点不能再次重放它。
    for (const auto scheme : {msime::windows::scheme::Korean, msime::windows::scheme::Zhuyin,
                              msime::windows::scheme::Vietnamese, msime::windows::scheme::Tibetan}) {
        Global::InputModeScheme.store(scheme);
        engine.current = {L"su3", 3};
        check(ime._RefreshDeferredRecoveryPrefix(&context) && keys.empty() && context.refs == 1,
              "reset-committed composition is not replayed twice");
    }
    check(!ime._RefreshDeferredRecoveryPrefix(nullptr) && keys.empty() && context.refs == 1,
          "missing context clears old references");
    ime._pCompositionProcessorEngine = nullptr;
    check(!ime._RefreshDeferredRecoveryPrefix(&context) && keys.empty(), "missing engine cannot generate replay");
    ime._pCompositionProcessorEngine = &engine;
    Global::InputModeScheme.store(msime::windows::scheme::Quanpin);
    const auto releaseQueue = [](auto &queue) {
        for (auto &key : queue) if (key.context) key.context->Release();
        queue.clear();
    };
    // 选择完成时后续字母仍在队列里，检查点必须表示选择后的原文。
    for (const auto function : {FUNCTION_SELECT_BY_NUMBER, FUNCTION_CONVERT}) {
        for (const auto &result : {CompositionState{L"https://", 8}, CompositionState{L"hao", 1},
                                    CompositionState{L"", 0}}) {
          for (const bool inFlightFailure : {false, true}) {
            engine.current = {L"old", 3};
            check(ime._RefreshDeferredRecoveryPrefix(&context), "old checkpoint created");
            DeferredKeyDown next;
            next.context = &context;
            next.translatedWch = L'x';
            next.focusGeneration = 7;
            next.replayAttempts = 1;
            next.keyState = {CATEGORY_COMPOSING, FUNCTION_INPUT};
            context.AddRef();
            ime._deferredKeyDowns.push_back(next);
            ime._deferredKeyInFlight = next;
            ime._deferredKeyInFlight.keyState.Function = function;
            context.AddRef();
            ime._hasDeferredKeyInFlight = true;
            ime._deferredKeyReplayToken = 42;
            engine.current = result;
            ime._CompleteDeferredKeyReplay(41);
            check(ime._hasDeferredKeyInFlight && keys.size() == 3,
                  "stale completion cannot alter checkpoint or ownership");
            ime._CompleteDeferredKeyReplay(42);
            std::wstring recovered;
            std::size_t moves = 0;
            for (const auto &key : keys) {
                if (key.keyState.Function == FUNCTION_INPUT) recovered += key.translatedWch;
                if (key.keyState.Function == FUNCTION_MOVE_LEFT) ++moves;
                check(key.context == &context && key.focusGeneration == 7,
                      "completed selection checkpoint retains owner");
            }
            check(recovered == result.rawInput && moves == result.rawInput.size() - result.caret,
                  "selection checkpoint uses post-selection raw text and caret with a pending tail");
            check(!ime._hasDeferredKeyInFlight && ime._deferredKeyReplayToken == 0 &&
                  ime._deferredKeyDowns.size() == 1 && ime._deferredKeyDowns.front().translatedWch == L'x',
                  "completion retires only its own slot and preserves the pending tail");
            check(context.refs == static_cast<int>(2 + keys.size()),
                  "completion releases old prefix and in-flight references exactly once");
            ime._deferredKeyProjectionValid = true;
            if (inFlightFailure) {
                ime._deferredKeyInFlight = ime._deferredKeyDowns.front();
                ime._deferredKeyDowns.pop_front();
                ime._hasDeferredKeyInFlight = true;
                ime._deferredKeyReplayToken = 43;
                ime._RetryDeferredKeyReplay(43);
                check(!ime._hasDeferredKeyInFlight && ime._deferredKeyReplayToken == 0,
                      "failed next key returns ownership to the reconnect FIFO");
            } else {
                ime._ArmDeferredRecoveryForTransport(&context);
            }
            check(keys.empty() && ime._deferredKeyDowns.size() ==
                  1 + result.rawInput.size() + result.rawInput.size() - result.caret,
                  "transport reset materializes the checkpoint before the pending tail");
            recovered.clear();
            moves = 0;
            for (const auto &key : ime._deferredKeyDowns) {
                if (key.keyState.Function == FUNCTION_INPUT) recovered += key.translatedWch;
                if (key.keyState.Function == FUNCTION_MOVE_LEFT) ++moves;
            }
            check(recovered == result.rawInput + L"x" && moves == result.rawInput.size() - result.caret,
                  "reconnect FIFO retains post-selection raw text, caret and following key");
            const auto count = ime._deferredKeyDowns.size();
            ime._ArmDeferredRecoveryForTransport(&context);
            check(ime._deferredKeyDowns.size() == count,
                  "repeated transport notification cannot materialize the checkpoint twice");
            releaseQueue(ime._deferredKeyDowns);
            releaseQueue(ime._deferredAppliedPrefix);
            check(context.refs == 1, "batch releases every context reference");
          }
        }
    }
    std::printf("deferred recovery checkpoint: %d failures\n", failures);
    return failures ? 1 : 0;
}
'''


def main() -> int:
    compiler = shutil.which("clang++") or shutil.which("g++") or shutil.which("c++")
    if compiler is None:
        print("skipped: C++ compiler unavailable for Windows recovery checkpoint regression")
        return 0
    source = SOURCE.read_text(encoding="utf-8")
    start = source.index("bool CMetasequoiaIME::_RefreshDeferredRecoveryPrefix(")
    end = source.index("\nvoid CMetasequoiaIME::_ArmDeferredRecoveryForTransport(", start)
    types = (ROOT / "platforms/windows/tsf/IME/MetasequoiaIMEBaseStructure.h").read_text(encoding="utf-8")
    types = types[types.index("enum KEYSTROKE_CATEGORY"):types.index("struct _PUNCTUATION")]
    policy_start = source.index("bool IsRecoverableDeferredPrefix(")
    policy_end = source.index("\nUINT CaptureIpcModifiers(", policy_start)
    harness = HARNESS.replace("// 生产按键类型", types).replace(
        "// 生产前缀规则", source[policy_start:policy_end])
    arm_start = source.index("void CMetasequoiaIME::_ArmDeferredRecoveryForTransport(")
    arm_end = source.index("\nbool CMetasequoiaIME::_ClassifyDeferredKeyDown(", arm_start)
    complete_start = source.index("void CMetasequoiaIME::_CompleteDeferredKeyReplay(")
    complete_end = source.index("\nbool CMetasequoiaIME::_IsDeferredKeyReplayCurrent(", complete_start)
    retry_start = source.index("void CMetasequoiaIME::_RetryDeferredKeyReplay(")
    retry_end = source.index("\nvoid CMetasequoiaIME::_ScheduleDeferredKeyDownDrain(", retry_start)
    with tempfile.TemporaryDirectory(prefix="msime-deferred-recovery-") as directory:
        root = pathlib.Path(directory)
        unit, binary = root / "recovery.cpp", root / "recovery"
        unit.write_text(
            harness + source[start:end] + source[complete_start:complete_end] + source[arm_start:arm_end] +
            source[retry_start:retry_end] + TESTS,
            encoding="utf-8",
        )
        subprocess.run([compiler, "-std=c++17", "-Wall", "-Wextra", "-Werror", "-DMSIME_EDITION_FULL",
                        "-I" + str(ROOT / "platforms/windows/common"), str(unit), "-o", str(binary)], check=True)
        return subprocess.run([str(binary)], check=False).returncode


if __name__ == "__main__":
    raise SystemExit(main())
