#!/usr/bin/env python3
"""编译实际队列分类方法，验证选择结果未定时保留尾键。"""
from __future__ import annotations

import pathlib
import shutil
import subprocess
import tempfile

ROOT = pathlib.Path(__file__).resolve().parent.parent
HARNESS = r'''
#include <algorithm>
#include <atomic>
#include <cstdint>
#include <cwctype>
#include <cstdio>
#include <deque>
#include <functional>
#include <set>
#include <string>
#include <string_view>
#include <vector>
#include "InputSchemeTraits.h"
#include "SecondThirdCandidatePolicy.h"
#include "WubiCandidateSelectionPolicy.h"
#include "LocalModeKeyPolicy.h"
#include "AltGrKeyPolicy.h"
#include "HostKoreanKey.h"
using std::min;
using UINT = unsigned;
using WCHAR = wchar_t;
using BOOL = int;
using WPARAM = std::uintptr_t;
using LPARAM = std::intptr_t;
using DWORD = unsigned long;
using ULONGLONG = std::uint64_t;
using HWND = void *;
using GUID = int;
#define _In_
#define _In_opt_
#define _Out_
[[maybe_unused]] constexpr BOOL TRUE = 1, FALSE = 0;
constexpr int S_OK = 0;
constexpr int FANY_IME_NO_REQUEST_ID = 0;
[[maybe_unused]] constexpr UINT WM_IpcReconnect = 1;
void PostOwnerMessageWithSyncFallback(void *, UINT) {}
bool EnsureNamedpipeFocusSessionActivated() { return true; }
template<typename... Args> void DebugTsfKeyLatency(Args...) {}
constexpr UINT MAX_PINYIN_LENGTH = 256;
// 生产按键类型与虚拟键码
UINT LOWORD(WPARAM value) { return static_cast<UINT>(value) & 0xffffu; }
bool capsOn = false, winDown = false;
short GetKeyState(UINT vk) { return vk == VK_CAPITAL && capsOn ? 1 : 0; }
short GetAsyncKeyState(UINT vk) { return winDown && (vk == VK_LWIN || vk == VK_RWIN) ? short(0x8000) : 0; }
ULONGLONG GetTickCount64() { return 1; }
HWND GetForegroundWindow() { return reinterpret_cast<HWND>(1); }
bool IsAutoRepeat(LPARAM value) { return (value & 0x40000000) != 0; }
UINT CaptureIpcModifiers() { return 0; }
UINT VKeyFromVKPacketAndWchar(UINT vk, WCHAR) { return vk; }
bool IsCharacterSetInputModeToggle(UINT, UINT) { return false; }
bool IsJapaneseInputModeToggle(UINT, UINT) { return false; }
bool IsEnglishInputModeToggle(UINT, UINT) { return false; }
bool IsGlossColumnShortcut(UINT, UINT) { return false; }
bool IsTranslationCommitShortcut(UINT, UINT) { return false; }
// 生产分段编辑规则
namespace Global {
bool g_connected = true;
std::atomic<int> InputModeScheme{msime::windows::scheme::Quanpin}, InputModeIndicator{'0'};
std::atomic<bool> SecondThirdCandidateEnabled{false}, MicrosoftShuangpinEnabled{false},
    ExpressionModeEnabled{false}, CommandModeEnabled{false}, MentionModeEnabled{false};
std::atomic<unsigned> WubiCandidateKeys{0};
enum class PunctuationLock { AlwaysEnglish, Normal };
std::atomic<PunctuationLock> PunctuationLockMode{PunctuationLock::Normal};
struct { bool active(ULONGLONG) const { return false; } } DedicatedEnglish;
std::set<WCHAR> CommitWithHighlightedCandPunc;
bool IsUiLessMode() { return false; }
std::atomic_bool TsfDiagnosticLogEnabled{false};
}
// 生产代码里的诊断日志在测试里关闭，记录进空实现即可。
void QueueTsfDiagnosticLog(const std::wstring &) {}
inline std::wstring BoolFlag(bool value) { return value ? L"1" : L"0"; }
struct ITfContext { int refs = 1; void AddRef() { ++refs; } void Release() { --refs; } };
namespace msime::tsf {
struct KeyCompositionState { std::wstring rawInput; std::size_t caret = 0; bool unicodeMode = false, urlMode = false, phraseActive = false; };
}
struct Engine {
    msime::tsf::KeyCompositionState current;
    bool imeOpen = true, punctuationOpen = false, fullwidth = false;
    std::function<void()> onRead;
    BOOL GetIMEMode(void *, int) const { if (onRead) onRead(); return imeOpen; }
    BOOL GetPunctuationMode(void *, int) const { return punctuationOpen; }
    BOOL GetDoubleSingleByteMode(void *, int) const { return fullwidth; }
    auto GetKeyCompositionState() const { return current; }
    std::size_t GetVirtualKeyLength() const { return 0; }
    enum class PreservedKeyAction { None, ToggleImeMode };
    PreservedKeyAction GetPreservedKeyAction(GUID) { return PreservedKeyAction::None; }
    bool IsPunctuation(WCHAR wch) const { return wch == L'.' || wch == L';'; }
    bool IsDoubleSingleByte(WCHAR) const { return false; }
    bool IsWildcard() const { return false; }
    bool IsWildcardChar(WCHAR) const { return false; }
    BOOL IsVirtualKeyNeedForFreshComposition(UINT vk, WCHAR *, _KEYSTROKE_STATE *) {
        return vk >= 'A' && vk <= 'Z';
    }
};
using CCompositionProcessorEngine = Engine;
// 生产投影规则
struct CMetasequoiaIME {
    Engine *_pCompositionProcessorEngine = nullptr;
    void *_pThreadMgr = reinterpret_cast<void *>(1);
    int _tfClientId = 1, _candidateMode = CANDIDATE_NONE;
    bool _serverUnavailableFallbackActive = false, _backspaceHoldArmed = false;
    bool _deferredKeyProjectionValid = false;
    bool _deferredKeyClassificationPending = false;
    bool _deferredProjectedImeOpen = false, _deferredProjectedPunctuationOpen = false,
        _deferredProjectedDoubleSingleByteOpen = false;
    std::wstring _deferredProjectedRawInput;
    std::size_t _deferredProjectedInputLength = 0, _deferredProjectedCaret = 0;
    bool _deferredProjectedCandidateActive = false, _deferredProjectedUnicodeMode = false,
         _deferredProjectedPhraseActive = false, _deferredProjectedUrlMode = false, _deferredProjectedKoreanHanjaListOpen = false;
    std::uint64_t _deferredKeyFocusGeneration = 7;
    // 生产队列类型
    std::deque<DeferredKeyDown> _deferredKeyDowns;
    bool _IsComposing() const { return !_pCompositionProcessorEngine->current.rawInput.empty() || _pCompositionProcessorEngine->current.phraseActive; }
    bool _IsKoreanHanjaListOpen() const { return false; }
    bool disabled = false, ownerCurrent = true;
    int ownerQueries = 0;
    std::function<void()> onOwnerQuery;
    BOOL _IsKeyboardDisabled() const { return disabled; }
    WCHAR ConvertVKey(UINT vk) const { return vk >= 'A' && vk <= 'Z' ? WCHAR(vk + 32) : WCHAR(vk); }
    bool _DeferredKeyQueueHasCapacity() const { return _deferredKeyDowns.size() < 64; }
    void _ScheduleDeferredKeyDownDrain() {}
    void _EnsureDeferredKeyProjection();
    void _ApplyDeferredKeyProjection(const _KEYSTROKE_STATE &, WCHAR, UINT);
    bool _QueueDeferredKeyDown(ITfContext *, WPARAM, LPARAM, WCHAR, UINT, const _KEYSTROKE_STATE &);
    DeferredKeyDown _deferredKeyInFlight;
    bool _hasDeferredKeyInFlight = false, _deferredKeyDrainPosted = false;
    std::uint64_t _deferredKeyReplayToken = 0, _nextDeferredKeyReplayToken = 0;
    std::atomic<bool> _localSessionResetPending{false};
    void *_msgWndHandle = nullptr;
    std::vector<KEYSTROKE_FUNCTION> dispatched;
    std::wstring applicationText;
    UINT applicationKey = 0;
    int completions = 0, retries = 0;
    bool dispatchRetry = false;
    UINT observedModifiers = 0;
    WCHAR observedCharacter = 0;
    // 生产派发类型
    bool _ResolveDeferredKeyClassification(uint64_t);
    void _DrainOneDeferredKeyDown();
    bool _IsDeferredKeyOwnerCurrent(ITfContext *, uint64_t, HWND) {
        ++ownerQueries; if (onOwnerQuery) onOwnerQuery(); return ownerCurrent;
    }
    bool _IsDeferredKeyReplayCurrent(uint64_t token, uint64_t generation, ITfContext *context) const {
        return _hasDeferredKeyInFlight && token == _deferredKeyReplayToken && generation == _deferredKeyFocusGeneration && context == _deferredKeyInFlight.context;
    }
    bool _QueueKoreanSyllableKeyReplay(UINT code, ITfContext *, uint64_t) { applicationKey = code; return true; }
    bool pendingApplicationKey = false;
    bool _HasPendingApplicationKeyReplay() const { return pendingApplicationKey; }
    bool _IsServerUnavailableFallbackActive() const { return _serverUnavailableFallbackActive; }
    void _NoteFieldTraitsForServer() {}
    void _NoteKeyEventIpcFailure() {}
    uint64_t _CaptureFocusSessionToken() const { return 11; }
    bool _IsFocusSessionCurrent(uint64_t, ITfContext * = nullptr) const { return true; }
    void _ClearDeferredKeyDowns() {
        ++_deferredKeyFocusGeneration;
        for (auto &key : _deferredKeyDowns) key.context->Release();
        _deferredKeyDowns.clear();
        if (_hasDeferredKeyInFlight) _deferredKeyInFlight.context->Release();
        _hasDeferredKeyInFlight = false; _deferredKeyInFlight = {}; _deferredKeyReplayToken = 0;
    }
    void _RetryDeferredKeyReplay(uint64_t) { ++retries; }
    void _CompleteDeferredKeyReplay(uint64_t token) {
        if (!_hasDeferredKeyInFlight || token != _deferredKeyReplayToken) return;
        _deferredKeyInFlight.context->Release();
        _deferredKeyInFlight = {}; _hasDeferredKeyInFlight = false;
        _deferredKeyReplayToken = 0; ++completions;
    }
    void _RequestDeferredApplicationTextEditSession(ITfContext *, WCHAR wch, uint64_t, uint64_t, uint64_t token) {
        applicationText += wch; _CompleteDeferredKeyReplay(token);
    }
    void _InvokeKeyHandler(ITfContext *, WPARAM, WCHAR, DWORD, const _KEYSTROKE_STATE &state,
                          int, const std::wstring &, int, int, int, uint64_t token) {
        dispatched.push_back(state.Function); _CompleteDeferredKeyReplay(token);
    }
    KeyDownDispatchResult _DispatchKeyDown(ITfContext *, WPARAM, LPARAM, BOOL *, const WCHAR *wch, const UINT *modifiers,
                                          const _KEYSTROKE_STATE *state, bool, uint64_t, uint64_t) {
        dispatched.push_back(state->Function); observedModifiers = *modifiers; observedCharacter = *wch;
        return dispatchRetry ? KeyDownDispatchResult::Retry : KeyDownDispatchResult::Complete;
    }
    void _DispatchPreservedKey(ITfContext *, GUID, BOOL *, uint64_t, bool, uint64_t) {}
    // 生产分类声明
};
'''
TESTS = r'''
int failures = 0, checks = 0;
void check(bool condition, const char *message) {
    ++checks; if (!condition) { ++failures; std::fprintf(stderr, "FAIL: %s\n", message); }
}
struct Fixture {
    Engine engine;
    ITfContext context;
    CMetasequoiaIME ime;
    Fixture(KEYSTROKE_FUNCTION operation = FUNCTION_SELECT_BY_NUMBER) {
        capsOn = winDown = false;
        Global::InputModeScheme = msime::windows::scheme::Quanpin;
        engine.current = {L"ht", 2, false, false};
        ime._pCompositionProcessorEngine = &engine;
        _KEYSTROKE_STATE selection{};
        selection.Category = CATEGORY_CANDIDATE; selection.Function = operation;
        ime._QueueDeferredKeyDown(&context, '1', 0, L'1', 0, selection);
    }
    bool capture(UINT code, WCHAR original, UINT modifiers = 0) {
        WCHAR wch = 0; UINT classified = 0; _KEYSTROKE_STATE state{};
        const bool captured = ime._ClassifyDeferredKeyDown(&context, code, 0, &original, &modifiers,
                                                         &wch, &classified, &state);
        if (captured) ime._QueueDeferredKeyDown(&context, code, 0, wch, modifiers, state);
        return captured;
    }
    void finishSelection(std::wstring raw = L"https://", bool urlMode = true) {
        ime._deferredKeyDowns.front().context->Release(); ime._deferredKeyDowns.pop_front();
        engine.current = {raw, raw.size(), false, urlMode};
    }
    ~Fixture() {
        ime._ClearDeferredKeyDowns();
        check(context.refs == 1, "all queued and in-flight context references released");
    }
};
int main() {
    {
        Fixture f;
        f.finishSelection(L"", false);
        f.engine.current.phraseActive = true;
        f.ime._deferredKeyClassificationPending = false;
        f.ime._deferredKeyProjectionValid = false;
        WCHAR wch = 0; UINT code = 0, modifiers = 2; WCHAR original = 0;
        _KEYSTROKE_STATE state{};
        check(f.ime._ClassifyDeferredKeyDown(&f.context, VK_BACK, 0, &original, &modifiers, &wch, &code, &state) &&
              state.Function == FUNCTION_BACKSPACE_SEGMENT, "prefix-only Ctrl+Backspace remains an IME edit");
        f.ime._EnsureDeferredKeyProjection();
        modifiers = 0;
        check(f.ime._ClassifyDeferredKeyDown(&f.context, VK_BACK, 0, &original, &modifiers, &wch, &code, &state) &&
              state.Function == FUNCTION_BACKSPACE, "prefix-only ordinary Backspace remains an IME edit");
        WCHAR mark = L'/';
        _KEYSTROKE_STATE markState{};
        check(f.ime._ClassifyDeferredKeyDown(&f.context, 0xBF, 0, &mark, &modifiers, &wch, &code, &markState) &&
              markState.Function == FUNCTION_PUNCTUATION, "slash ends held-only phrase instead of opening a local mode");
        f.ime._ApplyDeferredKeyProjection(state, 0, VK_BACK);
        check(f.ime._deferredKeyClassificationPending && !f.ime._deferredKeyProjectionValid,
              "phrase Backspace waits for Runtime before classifying queued keys");
    }
    // 组字时只按 Shift 或只按 Ctrl 的左右键是分段光标键，全拼、五笔都一样；锁光标的方案（韩文、注音、越南文、藏文）里 Shift+左右不是分段键，Ctrl+左右照旧是；Shift+退格、带 Win 的 Shift+左右、没有组字时的 Shift+左右都不是。
    {
        const auto classify = [](Fixture &f, UINT code, UINT modifiers) {
            WCHAR wch = 0, original = 0; UINT classified = 0;
            _KEYSTROKE_STATE state{};
            f.ime._ClassifyDeferredKeyDown(&f.context, code, 0, &original, &modifiers, &wch, &classified, &state);
            return state.Function;
        };
        const auto segment = [](KEYSTROKE_FUNCTION function) {
            return function == FUNCTION_MOVE_LEFT_SEGMENT || function == FUNCTION_MOVE_RIGHT_SEGMENT ||
                   function == FUNCTION_BACKSPACE_SEGMENT;
        };
        for (int scheme : {msime::windows::scheme::Quanpin, msime::windows::scheme::Shuangpin,
                           msime::windows::scheme::Wubi}) {
            Fixture f; f.finishSelection(L"nihao", false);
            f.ime._deferredKeyClassificationPending = false; f.ime._deferredKeyProjectionValid = false;
            Global::InputModeScheme = scheme;
            for (UINT modifiers : {1u, 2u}) {
                check(classify(f, VK_LEFT, modifiers) == FUNCTION_MOVE_LEFT_SEGMENT, "Shift/Ctrl+Left composing is a segment caret key");
                check(classify(f, VK_RIGHT, modifiers) == FUNCTION_MOVE_RIGHT_SEGMENT, "Shift/Ctrl+Right composing is a segment caret key");
            }
            check(classify(f, VK_BACK, 2) == FUNCTION_BACKSPACE_SEGMENT, "Ctrl+Backspace composing stays a segment erase");
            check(!segment(classify(f, VK_BACK, 1)), "Shift+Backspace is not a segment key");
            check(!segment(classify(f, VK_LEFT, 3)) && !segment(classify(f, VK_LEFT, 5)),
                  "Shift with Ctrl or Alt is not a segment caret key");
            winDown = true;
            check(!segment(classify(f, VK_LEFT, 1)), "Win+Shift+Left is not a segment caret key");
            winDown = false;
        }
        for (int scheme : {msime::windows::scheme::Korean, msime::windows::scheme::Zhuyin,
                           msime::windows::scheme::Vietnamese, msime::windows::scheme::Tibetan}) {
            Fixture f; f.finishSelection(L"gks", false);
            f.ime._deferredKeyClassificationPending = false; f.ime._deferredKeyProjectionValid = false;
            Global::InputModeScheme = scheme;
            check(!segment(classify(f, VK_LEFT, 1)) && !segment(classify(f, VK_RIGHT, 1)),
                  "Shift+Left/Right is not a segment key where the caret is locked");
            check(classify(f, VK_LEFT, 2) == FUNCTION_MOVE_LEFT_SEGMENT, "Ctrl+Left keeps its segment classification where the caret is locked");
        }
        {
            Fixture f; f.finishSelection(L"", false);
            f.ime._deferredKeyClassificationPending = false; f.ime._deferredKeyProjectionValid = false;
            check(!segment(classify(f, VK_LEFT, 1)) && !segment(classify(f, VK_RIGHT, 1)) &&
                  !segment(classify(f, VK_LEFT, 2)), "caret chords belong to the application with nothing composing");
        }
    }

    struct Case { UINT code; WCHAR wch; KEYSTROKE_FUNCTION continuing; bool applicationKey; };
    const Case cases[] = {
        {'0', L'0', FUNCTION_INPUT, false}, {'A', L'a', FUNCTION_INPUT, false},
        {VK_OEM_PERIOD, L'.', FUNCTION_INPUT, false}, {VK_SPACE, L' ', FUNCTION_CONVERT, false},
        {VK_RETURN, L'\r', FUNCTION_FINALIZE_CANDIDATELISTForVKReturn, true},
        {VK_BACK, L'\b', FUNCTION_BACKSPACE, true}, {VK_ESCAPE, L'\0', FUNCTION_CANCEL, true},
        {VK_TAB, L'\t', FUNCTION_SERVER_CANDIDATE_KEY, true},
        {VK_LEFT, L'\0', FUNCTION_MOVE_LEFT, true}, {VK_DELETE, L'\0', FUNCTION_DELETE, true}
    };
    for (const auto &test : cases) for (bool continues : {true, false}) {
        Fixture f;
        check(f.capture(test.code, test.wch), "tail remains ordered behind unresolved selection");
        check(f.ime._deferredKeyDowns.back().needsClassification, "tail classification deferred");
        f.finishSelection(continues ? L"https://" : L"", continues);
        f.ime._DrainOneDeferredKeyDown();
        if (continues || test.code == 'A') {
            check(f.ime.dispatched.size() == 1 && f.ime.dispatched[0] == test.continuing,
                  "tail uses actual continuing or fresh composition");
            check(f.ime.applicationText.empty() && !f.ime.applicationKey, "IME tail never sent prematurely to application");
        } else if (test.applicationKey) {
            check(f.ime.applicationKey == test.code && f.ime.applicationText.empty(), "idle editing key uses application barrier");
            check(f.ime.dispatched.empty(), "idle editing key does not reach IME");
        } else {
            check(f.ime.applicationText == std::wstring(1, test.wch), "idle printable tail keeps original text");
            check(f.ime.dispatched.empty(), "idle printable tail does not reach IME");
        }
        check(f.ime.completions == 1, "resolved tail completes its own slot");
    }
    for (auto operation : {FUNCTION_SELECT_BY_NUMBER, FUNCTION_CONVERT, FUNCTION_FINALIZE_CANDIDATELIST,
                           FUNCTION_FINALIZE_CANDIDATELISTForVKReturn, FUNCTION_KOREAN_HANJA_KEY}) {
        Fixture f(operation);
        check(f.capture(VK_BACK, L'\b'), "every uncertain candidate operation captures editing tail");
        f.finishSelection(L"remaining", false);
        f.ime._DrainOneDeferredKeyDown();
        check(f.ime.dispatched.size() == 1 && f.ime.dispatched[0] == FUNCTION_BACKSPACE,
              "partial choice edits remaining composition");
    }
    for (bool continues : {true, false}) {
        Fixture f(FUNCTION_CONVERT);
        Global::InputModeScheme = msime::windows::scheme::Japanese;
        check(f.capture(VK_SPACE, L' '), "Japanese repeated space captured while conversion unresolved");
        f.finishSelection(continues ? L"kana" : L"", false);
        f.ime._DrainOneDeferredKeyDown();
        check(continues ? f.ime.dispatched.size() == 1 && f.ime.dispatched[0] == FUNCTION_CONVERT
                        : f.ime.applicationText == L" ", "Japanese fallback commit preserves following application space");
    }
    for (bool capturedCaps : {true, false}) {
        Fixture f; capsOn = capturedCaps;
        check(f.capture('A', L'A', 1), "uppercase text captured with original shift");
        capsOn = !capturedCaps; winDown = true;
        f.finishSelection(L"", false); f.ime._DrainOneDeferredKeyDown();
        check(capturedCaps ? f.ime.applicationText == L"A"
                           : f.ime.dispatched.size() == 1 && f.ime.dispatched[0] == FUNCTION_INPUT,
              "classification uses captured Caps and Win state");
        if (!capturedCaps) check(f.ime.observedModifiers == 1 && f.ime.observedCharacter == L'A', "original shifted character and modifiers survive");
    }
    {
        Fixture f; check(f.capture('0', L'@', 6), "AltGr printable character is captured after normalization");
        f.finishSelection(); f.ime._DrainOneDeferredKeyDown();
        check(f.ime.dispatched.size() == 1 && f.ime.dispatched[0] == FUNCTION_INPUT &&
              f.ime.observedCharacter == L'@' && f.ime.observedModifiers == 6, "AltGr retains captured wire modifiers and spelling");
    }
    {
        Fixture f;
        check(!f.capture('A', L'a', 2) && !f.capture('A', L'a', 4), "Ctrl and Alt shortcuts remain outside raw capture");
        winDown = true; check(!f.capture('A', L'a'), "Win shortcuts remain outside raw capture");
        winDown = false; check(!f.capture(VK_LEFT, L'\0', 1), "Shift navigation remains outside raw capture");
    }
    {
        Fixture f; f.capture(VK_RETURN, L'\r'); f.finishSelection();
        f.ime._deferredProjectedImeOpen = false;
        f.ime.dispatchRetry = true; f.ime._DrainOneDeferredKeyDown();
        check(f.ime.retries == 1 && !f.ime._deferredKeyInFlight.needsClassification, "classification fixed before transport retry");
        f.engine.current = {};
        check(f.ime._ResolveDeferredKeyClassification(f.ime._deferredKeyReplayToken) &&
              f.ime._deferredKeyInFlight.keyState.Function == FUNCTION_FINALIZE_CANDIDATELISTForVKReturn,
              "retry never reclassifies against changed state or future projection");
    }
    {
        Fixture f; f.finishSelection(L"", false);
        f.ime._deferredKeyClassificationPending = false; f.ime.pendingApplicationKey = true;
        check(f.capture(VK_RETURN, L'\r') && f.ime._deferredKeyDowns.back().needsClassification,
              "pending application replay captures tail after FIFO emptied");
        f.ime.pendingApplicationKey = false;
        check(f.capture(VK_LEFT, L'\0') && f.ime._deferredKeyDowns.back().needsClassification,
              "captured tail keeps classification sticky after application acknowledgement");
    }
    for (bool disabled : {true, false}) {
        Fixture f; f.capture(VK_RETURN, L'\r'); f.finishSelection();
        f.ime.disabled = disabled; f.ime.ownerCurrent = !disabled ? false : true;
        f.ime._DrainOneDeferredKeyDown();
        check(f.ime.applicationText.empty() && f.ime.dispatched.empty() && !f.ime.applicationKey,
              "disabled keyboard or lost owner never converts classifier failure to input");
    }
    for (bool duringClassifier : {true, false}) {
        Fixture f; f.capture(VK_RETURN, L'\r'); f.finishSelection();
        if (duringClassifier) f.engine.onRead = [&] { f.ime._ClearDeferredKeyDowns(); };
        else f.ime.onOwnerQuery = [&] { f.ime._ClearDeferredKeyDowns(); };
        f.ime._DrainOneDeferredKeyDown();
        check(f.ime.applicationText.empty() && f.ime.dispatched.empty() && !f.ime._hasDeferredKeyInFlight,
              "COM reentry cannot resurrect cleared replay slot");
    }
    {
        Fixture f; f.capture(VK_RETURN, L'\r'); f.finishSelection(L"", false);
        f.ime._serverUnavailableFallbackActive = true; f.ime._DrainOneDeferredKeyDown();
        check(f.ime.applicationKey == VK_RETURN, "offline editing tail keeps application semantics");
    }
    std::printf("deferred selection classification: %d checks, %d failures\n", checks, failures);
    return failures ? 1 : 0;
}
'''



def between(source: str, start: str, end: str) -> str:
    begin = source.index(start)
    return source[begin:source.index(end, begin)]


def main() -> int:
    compiler = shutil.which("clang++") or shutil.which("g++") or shutil.which("c++")
    if compiler is None:
        print("skipped: C++ compiler unavailable for Windows deferred classification regression")
        return 0
    sink = (ROOT / "platforms/windows/tsf/Key/KeyEventSink.cpp").read_text()
    header = (ROOT / "platforms/windows/tsf/IME/MetasequoiaIME.h").read_text()
    types = (ROOT / "platforms/windows/tsf/IME/MetasequoiaIMEBaseStructure.h").read_text()
    types = between(types, "enum KEYSTROKE_CATEGORY", "struct _PUNCTUATION")
    virtual_keys = {
        "BACK": 8, "TAB": 9, "RETURN": 13, "SHIFT": 16, "CONTROL": 17, "MENU": 18,
        "CAPITAL": 20, "ESCAPE": 27, "SPACE": 32, "PRIOR": 33, "NEXT": 34,
        "END": 35, "HOME": 36, "LEFT": 37, "UP": 38, "RIGHT": 39, "DOWN": 40,
        "INSERT": 45, "DELETE": 46, "LWIN": 91, "RWIN": 92, "LSHIFT": 160,
        "RSHIFT": 161, "LCONTROL": 162, "RCONTROL": 163, "LMENU": 164, "RMENU": 165,
        "OEM_1": 186, "OEM_PLUS": 187, "OEM_COMMA": 188, "OEM_MINUS": 189,
        "OEM_PERIOD": 190, "OEM_4": 219, "OEM_6": 221, "PACKET": 231,
    }
    vk = "\n".join(f"[[maybe_unused]] constexpr UINT VK_{name} = {value};" for name, value in virtual_keys.items())
    declaration = between(header, "    bool _ClassifyDeferredKeyDown(", "    bool _QueueDeferredKeyDown(")
    queue_type = between(header, "    struct DeferredKeyDown", "    enum class KeyDownDispatchResult")
    shadow = between(sink, "struct DeferredShadowState", "bool IsRecoverableDeferredPrefix(")
    dispatch_type = between(header, "    enum class KeyDownDispatchResult", "    bool _HasDeferredKeyBarrier")
    segment = between(sink, "KEYSTROKE_FUNCTION SegmentEditFunction(", "\n}\n") + "\n}\n"
    harness = HARNESS.replace("// 生产分段编辑规则", segment).replace("    // 生产派发类型", dispatch_type).replace("// 生产按键类型与虚拟键码", types + vk).replace(
        "// 生产投影规则", shadow).replace("    // 生产队列类型", queue_type).replace(
        "    // 生产分类声明", declaration)
    body = between(sink, "void CMetasequoiaIME::_EnsureDeferredKeyProjection()", "void CMetasequoiaIME::_ApplyDeferredPreservedKeyProjection(")
    body += between(sink, "bool CMetasequoiaIME::_ClassifyDeferredKeyDown(", "//+---------------------------------------------------------------------------")
    body += between(sink, "bool CMetasequoiaIME::_QueueDeferredKeyDown(", "bool CMetasequoiaIME::_QueueDeferredPreservedKey(")
    body += between(sink, "bool CMetasequoiaIME::_ResolveDeferredKeyClassification(", "//+---------------------------------------------------------------------------")
    with tempfile.TemporaryDirectory(prefix="msime-deferred-classification-") as directory:
        unit = pathlib.Path(directory) / "classification.cpp"
        binary = pathlib.Path(directory) / "classification"
        unit.write_text(harness + body + TESTS)
        subprocess.run([compiler, "-std=c++17", "-Wall", "-Wextra", "-Werror", "-DMSIME_EDITION_FULL",
                        "-I" + str(ROOT / "platforms/windows/common"),
                        "-I" + str(ROOT / "crates/host-api/include"),
                        "-I" + str(ROOT / "platforms/windows/tsf/Global"),
                        "-I" + str(ROOT / "platforms/windows/tsf"), str(unit), "-o", str(binary)], check=True)
        return subprocess.run([str(binary)], check=False).returncode


if __name__ == "__main__":
    raise SystemExit(main())
