#!/usr/bin/env python3
"""在 posted 先于 input 的消息模型中编译执行实际应用键回放方法。"""
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
#include <cstdio>
#include <cstdlib>
#include <deque>
#include <functional>
#include <utility>
#include <vector>
#include <string>
#include "InputSchemeTraits.h"
#include "WubiCommitOwner.h"
namespace Global {
std::atomic<int> InputModeScheme{msime::windows::scheme::Quanpin};
bool g_connected = true;
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
constexpr int FANY_IME_NO_REQUEST_ID = 0, S_OK = 0, E_INVALIDARG = -1;

using WORD = unsigned short;
using DWORD = unsigned long;
using ULONG_PTR = std::uintptr_t;
using LPARAM = std::intptr_t;
using BOOL = int;
using HRESULT = int;
using ULONG = unsigned long;
using TfEditCookie = int;
#define STDMETHODIMP int
constexpr int S_FALSE = 1, E_FAIL = -2, TF_DEFAULT_SELECTION = 0, TF_ANCHOR_END = 1;
using HWND = void *;
using GUID = int;
bool SUCCEEDED(int value) { return value >= 0; }
constexpr BOOL TRUE = 1, FALSE = 0;
ULONG_PTR messageExtraInfo = 0;
ULONG_PTR GetMessageExtraInfo() { return messageExtraInfo; }
using ULONGLONG = std::uint64_t;
constexpr UINT VK_TAB = 9, VK_RETURN = 13, VK_BACK = 8, VK_ESCAPE = 27, INPUT_KEYBOARD = 1;
constexpr DWORD KEYEVENTF_EXTENDEDKEY = 1, KEYEVENTF_KEYUP = 2;
// 生产标记规则
constexpr UINT WM_ReplayKoreanSyllableKey = 30, WM_DrainDeferredKeyDown = 18;
struct KEYBDINPUT { WORD wVk = 0; DWORD dwFlags = 0; ULONG_PTR dwExtraInfo = 0; };
struct INPUT { DWORD type = 0; KEYBDINPUT ki; };
#define ARRAYSIZE(x) (sizeof(x) / sizeof((x)[0]))
struct Message { UINT message; WPARAM arg; };
std::deque<Message> posted;
std::deque<INPUT> input;
void *testWindow = reinterpret_cast<void *>(1);
HWND foregroundWindow = testWindow;
HWND GetForegroundWindow() { return foregroundWindow; }
bool postFails = false;
int syncCalls = 0;
ULONGLONG now = 1;
ULONGLONG GetTickCount64() { return now; }
BOOL PostMessage(void *, UINT message, WPARAM arg, LPARAM) {
    if (postFails) return 0;
    posted.push_back({message,arg}); return 1;
}
void SendMessage(void *, UINT, WPARAM, LPARAM) { ++syncCalls; }
std::deque<UINT> sendResults;
int sentDowns = 0, sentUps = 0;
UINT SendInput(UINT count, INPUT *events, int) {
    const UINT sent = sendResults.empty() ? count : std::min(count, sendResults.front());
    if (!sendResults.empty()) sendResults.pop_front();
    for (UINT i=0;i<sent;++i) {
        input.push_back(events[i]);
        if (events[i].ki.dwFlags & KEYEVENTF_KEYUP) ++sentUps; else ++sentDowns;
    }
    return sent;
}

bool IsWindow(void *window) { return window != nullptr; }
void PostOwnerMessageWithSyncFallback(void *, unsigned) {}
constexpr std::size_t MAX_DEFERRED_KEY_DOWN_COUNT = 10;
unsigned LOBYTE(SHORT value) { return static_cast<unsigned short>(value) & 0xff; }
unsigned HIBYTE(SHORT value) { return static_cast<unsigned short>(value) >> 8; }
SHORT VkKeyScanW(WCHAR value) {
    if (value >= L'a' && value <= L'z') return static_cast<SHORT>(value - L'a' + 'A');
    if (value >= L'A' && value <= L'Z') return static_cast<SHORT>(0x100 | value);
    return -1;
}
std::wstring appliedText;
struct TestRange {
    int refs = 1;
    void AddRef() { ++refs; }
    void Release() { --refs; }
    int SetText(int, int, const WCHAR *text, int length) { appliedText.append(text, length); return S_OK; }
    void Collapse(int, int) {}
};
struct TF_SELECTION { TestRange *range = nullptr; };
struct ITfContext {
    int refs = 1, setSelectionResult = 0;
    TestRange range;
    std::function<void()> onGetSelection;
    void AddRef() { ++refs; }
    void Release() { --refs; }
    int GetSelection(int, int, int, TF_SELECTION *selection, ULONG *fetched) {
        if (onGetSelection) onGetSelection();
        selection->range = &range; range.AddRef(); *fetched = 1; return S_OK;
    }
    int SetSelection(int, int, TF_SELECTION *) { return setSelectionResult; }
};
struct ITfDocumentMgr {
    ITfContext *top = nullptr;
    int refs = 1, result = 0;
    std::function<void()> onGetTop;
    void AddRef() { ++refs; }
    void Release() { --refs; }
    int GetTop(ITfContext **value) {
        if (onGetTop) onGetTop();
        *value = result >= 0 ? top : nullptr;
        if (*value) (*value)->AddRef();
        return result;
    }
};
struct ITfThreadMgr {
    ITfDocumentMgr *document = nullptr;
    bool focused = true;
    int refs = 1, focusResult = 0, threadResult = 0;
    std::function<void()> onGetFocus;
    void AddRef() { ++refs; }
    void Release() { --refs; }
    int IsThreadFocus(BOOL *value) { *value = focused; return threadResult; }
    int GetFocus(ITfDocumentMgr **value) {
        if (onGetFocus) onGetFocus();
        *value = focusResult >= 0 ? document : nullptr;
        if (*value) (*value)->AddRef();
        return focusResult;
    }
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
// 生产队列类型
namespace msime::tsf { bool is_korean_caret_or_edit_key(UINT code) { return code == VK_LEFT; } }
using CCompositionProcessorEngine = Engine;
template<typename... Args> void DebugTsfIssue47(Args...) {}
// 生产前缀规则
struct CMetasequoiaIME {
    msime::tsf::WubiCommitOwner _wubiCommitOwner;
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
    void _ScheduleDeferredKeyDownDrain(bool allowSyncFallback = true);
    void _CompleteDeferredKeyReplay(std::uint64_t);
    ITfContext *_pContext = nullptr;
    void _EnsureDeferredKeyProjection() { _deferredKeyProjectionValid = true; }
    void _ArmDeferredRecoveryForTransport(ITfContext *);
    int retries = 0;
    void _RetryDeferredKeyReplay(std::uint64_t) { ++retries; }
    void *_msgWndHandle = testWindow;
    void _ClearDeferredKeyDowns() { std::abort(); }
    bool _deferredKeyDrainPosted = false;
    // 生产回放状态
    bool _HasPendingApplicationKeyReplay() const;
    bool _ObserveApplicationKeyReplay(ITfContext *, UINT, ULONG_PTR);
    void _ClearApplicationKeyReplay();
    void _PollApplicationKeyReplay();
    bool _QueueKoreanSyllableKeyReplay(UINT, ITfContext *, uint64_t);
    void _RunKoreanSyllableKeyReplay(UINT);
    bool _IsDeferredKeyReplayCurrent(uint64_t, uint64_t, ITfContext * = nullptr) const;
    bool _IsDeferredApplicationTextCurrent(uint64_t, uint64_t, ITfContext *, HWND, uint64_t);
    bool _IsDeferredKeyOwnerCurrent(ITfContext *, uint64_t, HWND) const;
    ITfThreadMgr *_pThreadMgr = nullptr;
    std::atomic<bool> _localSessionResetPending{false};
    bool _serverUnavailableFallbackActive = false, disabled = false;
    bool _IsServerUnavailableFallbackActive() const { return _serverUnavailableFallbackActive; }
    bool _IsKeyboardDisabled() const { return disabled; }
    uint64_t focusToken = 11;
    bool focusCurrent = true, clearDuringFocusQuery = false;
    uint64_t _CaptureFocusSessionToken() const { return focusToken; }
    bool _IsFocusSessionCurrent(uint64_t token, ITfContext * = nullptr) {
        if (clearDuringFocusQuery) { clearDuringFocusQuery = false; _ClearApplicationKeyReplay(); }
        return focusCurrent && token == 11;
    }
    int ProbeTest(ITfContext *, WPARAM, LPARAM, BOOL *);
    int ProbeDown(ITfContext *, WPARAM, LPARAM, BOOL *);
    void _InvalidateSmartPunctuationShadow() {}
};
struct CEditSessionBase {
    CMetasequoiaIME *_pTextService;
    ITfContext *_pContext;
    CEditSessionBase(CMetasequoiaIME *service, ITfContext *context) : _pTextService(service), _pContext(context) { context->AddRef(); }
    virtual ~CEditSessionBase() { _pContext->Release(); }
    virtual int DoEditSession(int) = 0;
};
// 生产应用插字会话
void MarkNamedpipeSessionDirtyForOwner(CMetasequoiaIME *) {}
'''

TESTS = r'''
int failures = 0, checks = 0;
void check(bool condition, const char *message) {
    ++checks;
    if (!condition) { ++failures; std::fprintf(stderr, "FAIL: %s\n", message); }
}
void reset() {
    posted.clear(); input.clear(); sendResults.clear(); appliedText.clear(); postFails = false;
    sentDowns = sentUps = syncCalls = 0; now = 1; foregroundWindow = testWindow;
}
struct Fixture {
    Engine engine;
    ITfContext context;
    ITfDocumentMgr document;
    ITfThreadMgr threadMgr;
    CMetasequoiaIME ime;
    Fixture() {
        reset(); ime._pCompositionProcessorEngine = &engine;
        document.top = &context; threadMgr.document = &document; ime._pThreadMgr = &threadMgr;
        DeferredKeyDown tail;
        tail.foregroundWindow = foregroundWindow;
        tail.context = &context; tail.translatedWch = L'x';
        tail.focusGeneration = ime._deferredKeyFocusGeneration;
        context.AddRef(); ime._deferredKeyDowns.push_back(tail);
        ime._deferredKeyInFlight = tail;
        ime._deferredKeyInFlight.keyState.Function = FUNCTION_COMMIT_SYLLABLE_AND_REPLAY;
        context.AddRef(); ime._hasDeferredKeyInFlight = true; ime._deferredKeyReplayToken = 42;
    }
    UINT queue() {
        ime._QueueKoreanSyllableKeyReplay(VK_LEFT, &context, 42);
        return ime._applicationKeyReplay.marker;
    }
    void complete() { ime._CompleteDeferredKeyReplay(42); }
    void run(UINT marker) { ime._RunKoreanSyllableKeyReplay(marker); }
    bool probe(int mode, UINT marker) {
        messageExtraInfo = marker;
        BOOL eaten = TRUE;
        if (mode == 0) ime.ProbeTest(&context, VK_LEFT, 0, &eaten);
        else ime.ProbeDown(&context, VK_LEFT, 0, &eaten);
        return eaten == FALSE;
    }
    ~Fixture() {
        ime._ClearApplicationKeyReplay();
        if (ime._hasDeferredKeyInFlight) ime._deferredKeyInFlight.context->Release();
        for (auto &key : ime._deferredKeyDowns) key.context->Release();
        for (auto &key : ime._deferredAppliedPrefix) key.context->Release();
        check(context.refs == 1, "all replay and queue context references released");
        check(document.refs == 1 && threadMgr.refs == 1 && context.range.refs == 1, "all COM owner and range references released");
    }
};
int main() {
    {
        Fixture f;
        f.ime._wubiCommitOwner.begin(42, 1);
        const auto refs = f.context.refs;
        f.complete();
        check(f.ime._hasDeferredKeyInFlight && f.ime._deferredKeyReplayToken == 42 &&
              f.context.refs == refs && f.ime._deferredAppliedPrefix.empty() && posted.empty(),
              "current Wubi owner retains the in-flight slot until worker completion");
        f.ime._wubiCommitOwner.clear();
        f.complete();
        check(!f.ime._hasDeferredKeyInFlight && posted.size() == 1,
              "clearing the Wubi owner releases completion and schedules the next key once");
    }
    {
        Fixture f;
        f.ime._wubiCommitOwner.begin(41, 1);
        f.complete();
        check(!f.ime._hasDeferredKeyInFlight && f.ime._wubiCommitOwner.current(41) && posted.empty(),
              "a different Wubi token cannot block completion but still blocks the next drain");
        f.ime._wubiCommitOwner.clear();
        f.ime._ScheduleDeferredKeyDownDrain();
        f.ime._ScheduleDeferredKeyDownDrain();
        check(posted.size() == 1, "clearing an active Wubi owner schedules only one pending drain");
    }
    for (int mode = 0; mode != 3; ++mode) {
        Fixture f; f.queue(); f.complete();
        check(!f.ime._hasDeferredKeyInFlight, "committed syllable retires before application ack");
        check(f.ime._deferredAppliedPrefix.empty(), "committed syllable is not retained for transport retry");
        std::wstring trace;
        while (!posted.empty() || !input.empty()) {
            if (!posted.empty()) {
                const auto event = posted.front(); posted.pop_front();
                if (event.message == WM_ReplayKoreanSyllableKey) f.run(static_cast<UINT>(event.arg));
                if (event.message == WM_DrainDeferredKeyDown) {
                    f.ime._deferredKeyDrainPosted = false;
                    if (!f.ime._HasPendingApplicationKeyReplay()) trace += L'X';
                }
            } else {
                const auto event = input.front(); input.pop_front();
                if (!(event.ki.dwFlags & KEYEVENTF_KEYUP)) {
                    const UINT marker = static_cast<UINT>(event.ki.dwExtraInfo);
                    check(f.probe(mode == 1 ? 1 : 0, marker), "valid replay passes sink");
                    check(f.probe(mode == 1 ? 1 : 0, marker), "duplicate query passes sink");
                    if (mode == 2) check(f.probe(1, marker), "KeyDown after Test passes sink");
                    trace += L'L';
                }
            }
        }
        check(trace == L"LX", "application Left precedes posted tail in Test/Down/both hosts");
        check(sentDowns == 1, "one application keydown");
    }
    {
        Fixture f; postFails = true; const UINT marker = f.queue(); f.complete();
        check(f.ime._HasPendingApplicationKeyReplay(), "post failure keeps application barrier");
        postFails = false; f.ime._PollApplicationKeyReplay();
        check(sentDowns == 1, "timer sends after replay post failure");
        postFails = true; check(f.probe(0, marker), "ack passes when drain post fails");
        check(syncCalls == 0 && !f.ime._deferredKeyDrainPosted, "ack never synchronously drains and clears failed latch");
        postFails = false; f.ime._PollApplicationKeyReplay();
        check(f.ime._deferredKeyDrainPosted, "timer retries drain after ack post failure");
    }
    for (UINT sent = 0; sent <= 2; ++sent) {
        Fixture f; const UINT marker = f.queue(); f.complete();
        sendResults.push_back(sent); f.run(marker);
        check(f.ime._HasPendingApplicationKeyReplay(), "SendInput alone does not ack application");
        f.ime._PollApplicationKeyReplay();
        check(sentDowns == 1 && sentUps == 1, "zero retry or partial keyup recovery never repeats keydown");
        f.ime._PollApplicationKeyReplay();
        check(sentDowns == 1 && sentUps == 1, "no ack never causes duplicate injection");
        check(f.probe(1, marker), "keydown-only acknowledgement needs no keyup callback");
        check(!f.ime._HasPendingApplicationKeyReplay(), "ack releases barrier");
    }
    {
        Fixture f; const UINT marker = f.queue(); f.complete();
        sendResults.push_back(1); f.run(marker);
        check(f.probe(0, marker), "partial injection can be observed");
        check(f.ime._HasPendingApplicationKeyReplay(), "keyup insertion remains gated after early ack");
        f.ime._PollApplicationKeyReplay();
        check(!f.ime._HasPendingApplicationKeyReplay() && sentDowns == 1 && sentUps == 1, "partial send releases only after keyup inserted");
    }
    for (int cancel = 0; cancel != 4; ++cancel) {
        Fixture f; const UINT marker = f.queue(); f.complete();
        sendResults.push_back(1); f.run(marker);
        if (cancel == 0) f.ime.focusCurrent = false;
        if (cancel == 1) ++f.ime._deferredKeyFocusGeneration;
        if (cancel == 2) now += 2000;
        if (cancel == 3) f.ime._ClearApplicationKeyReplay();
        f.ime._PollApplicationKeyReplay();
        check(!f.ime._HasPendingApplicationKeyReplay(), "focus/generation/timeout/clear cancels barrier");
        check(!f.probe(0, marker), "cancelled keydown is eaten at sink");
        check(sentDowns == 1 && sentUps == 1, "cancel releases partial keydown without another press");
    }
    {
        Fixture f; const UINT marker = f.queue(); f.complete(); f.run(marker);
        ITfContext other;
        check(!f.ime._ObserveApplicationKeyReplay(&other, VK_LEFT, marker), "foreign context rejected");
        check(!f.ime._ObserveApplicationKeyReplay(&f.context, VK_RETURN, marker), "wrong virtual key rejected");
        check(!f.probe(0, marker + 1), "unknown nonce rejected");
        now += 2000; f.ime._PollApplicationKeyReplay();
        check(!f.probe(1, marker) && sentDowns == 1, "no-ack timeout cancels instead of resending");
    }
    {
        Fixture f; const UINT first = f.queue(); f.ime._ClearApplicationKeyReplay();
        const UINT second = f.queue(); check(second != first, "same-key replacement never reuses marker");
        f.run(first); check(sentDowns == 0, "stale posted replay cannot send replacement key");
        f.run(second); check(!f.probe(0, first) && f.probe(0, second), "only replacement identity acknowledged");
        CMetasequoiaIME other;
        check(!other._ObserveApplicationKeyReplay(&f.context, VK_LEFT, second), "another TIP cannot ack marker");
        check(NextApplicationKeyReplayMarker() != second, "nonce allocator is shared across instances");
    }
    {
        Fixture f; const UINT marker = f.queue(); f.complete();
        f.ime.clearDuringFocusQuery = true; f.run(marker);
        check(sentDowns == 0 && !f.ime._HasPendingApplicationKeyReplay(), "COM focus reentry cannot send cancelled key");
    }
    {
        Fixture f; const UINT marker = f.queue(); f.complete(); f.run(marker);
        f.ime.clearDuringFocusQuery = true;
        check(!f.probe(0, marker), "COM focus reentry cannot ack cancelled key");
    }
    {
        Fixture f;
        f.ime._QueueKoreanSyllableKeyReplay(VK_LEFT, &f.context, 41);
        check(!f.ime._HasPendingApplicationKeyReplay(), "wrong original edit slot cannot queue application key");
    }
    {
        Fixture f; const UINT marker = f.queue(); f.complete(); f.run(marker);
        now += 2000;
        check(!f.probe(0, marker) && !f.ime._HasPendingApplicationKeyReplay(), "late first probe expires without timer");
    }

    for (UINT key : {VK_BACK, VK_ESCAPE}) {
        Fixture f;
        check(f.ime._QueueKoreanSyllableKeyReplay(key, &f.context, 42), "Backspace and Escape use application replay");
        const UINT marker = f.ime._applicationKeyReplay.marker;
        f.complete(); f.run(marker);
        check(input.size() == 2 && !(input.front().ki.dwFlags & KEYEVENTF_EXTENDEDKEY), "Backspace and Escape are not extended keys");
        check(f.ime._ObserveApplicationKeyReplay(&f.context, key, marker), "editing replay acknowledged by exact virtual key");
    }
    {
        Fixture f; f.ime._serverUnavailableFallbackActive = true; f.ime.focusCurrent = false; f.ime.focusToken = 0;
        const UINT marker = f.queue(); f.complete(); f.run(marker);
        check(marker && sentDowns == 1 && f.probe(0, marker), "offline replay uses real local owner without worker Ready");
    }
    for (int invalid = 0; invalid != 10; ++invalid) {
        Fixture f; f.ime._serverUnavailableFallbackActive = true;
        const UINT marker = f.queue(); f.complete();
        ITfContext other;
        if (invalid == 0) f.threadMgr.focused = false;
        if (invalid == 1) f.threadMgr.document = nullptr;
        if (invalid == 2) f.document.top = &other;
        if (invalid == 3) foregroundWindow = reinterpret_cast<void *>(2);
        if (invalid == 4) f.ime._localSessionResetPending = true;
        if (invalid == 5) f.threadMgr.threadResult = -1;
        if (invalid == 6) f.threadMgr.focusResult = -1;
        if (invalid == 7) f.document.result = -1;
        if (invalid == 8) f.document.onGetTop = [&] { ++f.ime._deferredKeyFocusGeneration; };
        if (invalid == 9) f.document.onGetTop = [&] { foregroundWindow = reinterpret_cast<void *>(2); };
        f.run(marker);
        check(sentDowns == 0 && !f.ime._HasPendingApplicationKeyReplay(), "offline never bypasses thread/context/window/reset owner checks");
    }
    {
        Fixture f; const UINT marker = f.queue(); f.complete();
        f.threadMgr.onGetFocus = [&] { f.threadMgr.focused = false; f.threadMgr.onGetFocus = nullptr; };
        f.run(marker);
        check(sentDowns == 0, "COM top-context query cannot invalidate earlier thread focus check");
    }
    {
        Fixture f; const UINT marker = f.queue(); f.complete(); f.run(marker);
        f.threadMgr.focused = false;
        check(!f.probe(0, marker), "ack rejects lost real thread focus even if worker token remains Ready");
    }
    for (int invalid = 0; invalid != 8; ++invalid) {
        Fixture f; f.ime._serverUnavailableFallbackActive = true; f.ime.focusCurrent = false;
        CDeferredApplicationTextEditSession edit(&f.ime, &f.context, L'.', 0, 7, 42, testWindow);
        if (invalid == 1) f.ime.disabled = true;
        if (invalid == 2) f.threadMgr.focused = false;
        if (invalid == 3) foregroundWindow = reinterpret_cast<void *>(2);
        if (invalid == 4) f.context.onGetSelection = [&] { ++f.ime._deferredKeyFocusGeneration; };
        if (invalid == 5) f.context.onGetSelection = [&] { f.threadMgr.focused = false; };
        if (invalid == 6) f.context.onGetSelection = [&] { ++f.ime._deferredKeyReplayToken; };
        if (invalid == 7) f.context.setSelectionResult = E_FAIL;
        edit.DoEditSession(1);
        if (invalid == 0 || invalid == 7) {
            check(appliedText == L"." && f.ime.retries == 0 && !f.ime._hasDeferredKeyInFlight,
                  "offline application text completes once even when caret placement fails");
            edit.DoEditSession(1);
            check(appliedText == L".", "late duplicate edit session cannot insert twice");
        } else check(appliedText.empty() && f.ime.retries == 1, "async application text rejects disabled or stale owner before SetText");
    }
    UINT lastMarker = NextApplicationKeyReplayMarker();
    bool increasing = true;
    for (UINT marker = NextApplicationKeyReplayMarker(); marker; marker = NextApplicationKeyReplayMarker()) {
        increasing = increasing && marker > lastMarker;
        lastMarker = marker;
    }
    check(increasing && (lastMarker & APPLICATION_KEY_REPLAY_NONCE_MASK) == APPLICATION_KEY_REPLAY_NONCE_MASK,
          "nonce remains monotonic through exhaustion");
    check(NextApplicationKeyReplayMarker() == 0, "exhausted nonce never wraps or reuses identity");
    check(!IsApplicationKeyReplayExtraInfo(APPLICATION_KEY_REPLAY_PREFIX), "zero nonce rejected");
    check(IsApplicationKeyReplayExtraInfo(APPLICATION_KEY_REPLAY_PREFIX | 1u), "32-bit marker accepted");
    check(!IsApplicationKeyReplayExtraInfo((ULONG_PTR{1} << 32) | APPLICATION_KEY_REPLAY_PREFIX | 1u), "64-bit high bits rejected");
    std::printf("application key replay: %d checks, %d failures\n", checks, failures);
    return failures ? 1 : 0;
}
'''
def method(source: str, start: str, end: str) -> str:
    begin = source.index(start)
    return source[begin:source.index(end, begin)]

def main() -> int:
    compiler = shutil.which("clang++") or shutil.which("g++") or shutil.which("c++")
    if compiler is None:
        print("skipped: C++ compiler unavailable for Windows application key replay regression")
        return 0
    sink = (ROOT / "platforms/windows/tsf/Key/KeyEventSink.cpp").read_text()
    handler = (ROOT / "platforms/windows/tsf/Key/KeyHandler.cpp").read_text()
    types = (ROOT / "platforms/windows/tsf/IME/MetasequoiaIMEBaseStructure.h").read_text()
    types = types[types.index("enum KEYSTROKE_CATEGORY"):types.index("struct _PUNCTUATION")]
    header = (ROOT / "platforms/windows/tsf/IME/MetasequoiaIME.h").read_text()
    marker_rules = method(header, "constexpr UINT APPLICATION_KEY_REPLAY_PREFIX", "// 单按 Caps Lock")
    replay_state = method(header, "    struct ApplicationKeyReplay", "    int _pendingPairedCaretDelta")
    ime_source = (ROOT / "platforms/windows/tsf/IME/MetasequoiaIME.cpp").read_text()
    application_text_session = method(ime_source, "class CDeferredApplicationTextEditSession", "\n} // namespace")
    harness = HARNESS.replace("// 生产应用插字会话", application_text_session).replace("// 生产队列类型", method(header, "    struct DeferredKeyDown", "    enum class KeyDownDispatchResult")).replace("// 生产标记规则", marker_rules).replace("    // 生产回放状态", replay_state).replace("// 生产按键类型", types).replace("// 生产前缀规则", method(
        sink, "bool IsRecoverableDeferredPrefix(", "\nUINT CaptureIpcModifiers("))
    body = method(sink, "bool CMetasequoiaIME::_RefreshDeferredRecoveryPrefix(",
                  "\nvoid CMetasequoiaIME::_ArmDeferredRecoveryForTransport(")
    body += method(sink, "void CMetasequoiaIME::_CompleteDeferredKeyReplay(",
                   "\nbool CMetasequoiaIME::_IsDeferredKeyReplayCurrent(")
    body += method(sink, "void CMetasequoiaIME::_ScheduleDeferredKeyDownDrain(",
                   "\nbool CMetasequoiaIME::_IsServerUnavailableFallbackActive(")
    body += method(sink, "bool CMetasequoiaIME::_IsDeferredKeyReplayCurrent(",
                   "\nvoid CMetasequoiaIME::_RetryDeferredKeyReplay(")
    body += method(handler, "namespace\n{\nUINT NextApplicationKeyReplayMarker()",
                   "\n//+---------------------------------------------------------------------------")
    for original, probe in (("OnTestKeyDown", "ProbeTest"), ("OnKeyDown", "ProbeDown")):
        entry = method(sink, "STDAPI CMetasequoiaIME::" + original + "(",
                       "    if (IsSelfGeneratedSendInputExtraInfo")
        body += entry.replace("STDAPI", "int").replace(original, probe).replace("LPARAM lParam", "LPARAM")
        body += "    return E_INVALIDARG;\n}\n"
    # 接线检查保证测试中的真实方法同时用于 timer、清理和每个 drain 入口。
    assert "_HasPendingApplicationKeyReplay()" in method(sink, "bool CMetasequoiaIME::_HasDeferredKeyBarrier()", "\nbool CMetasequoiaIME::_Classify")
    drain = method(sink, "void CMetasequoiaIME::_DrainOneDeferredKeyDown()", "    //")
    assert "_HasPendingApplicationKeyReplay()" in drain
    clear = method(sink, "void CMetasequoiaIME::_ClearDeferredKeyDowns()", "\nvoid CMetasequoiaIME::_CompleteDeferredKeyReplay(")
    assert "_ClearApplicationKeyReplay();" in clear
    ime_source = (ROOT / "platforms/windows/tsf/IME/MetasequoiaIME.cpp").read_text()
    assert "_PollApplicationKeyReplay();" in ime_source
    assert "_ClearApplicationKeyReplay();" in method(ime_source, "void CMetasequoiaIME::_RequestLocalSessionReset(", "\nvoid CMetasequoiaIME::_ScheduleCandidatePresenterCleanup(")
    edit_session = (ROOT / "platforms/windows/tsf/Key/KeyHandlerEditSession.cpp").read_text()
    category = (ROOT / "platforms/windows/tsf/Key/KeyStateCategory.cpp").read_text()
    assert "keyHandlerEditSessioDTO.deferredReplayToken = _deferredReplayToken;" in edit_session
    assert "dto.deferredReplayToken" in method(category, "    case FUNCTION_COMMIT_SYLLABLE_AND_REPLAY:", "    case FUNCTION_KOREAN_HANJA_KEY:")
    assert "dto.deferredReplayToken" in method(category, "    case FUNCTION_KOREAN_HANJA_KEY:", "    case FUNCTION_CANDIDATE_CONTINUATION:")
    assert "_HandleSyllableCommit(ec, pContext, code, wch, replayToken)" in method(handler, "        case msime::tsf::KoreanKeyAction::CommitAndPass:", "        default:")
    assert "_QueueKoreanSyllableKeyReplay(code, pContext, replayToken)" in method(handler, "HRESULT CMetasequoiaIME::_HandleSyllableCommit(", "bool KoreanHanjaListOpen(")
    with tempfile.TemporaryDirectory(prefix="msime-application-key-order-") as directory:
        unit, binary = pathlib.Path(directory) / "order.cpp", pathlib.Path(directory) / "order"
        unit.write_text(harness + body + TESTS)
        subprocess.run([compiler, "-std=c++17", "-Wall", "-Wextra", "-Werror", "-DMSIME_EDITION_FULL",
                        "-I" + str(ROOT / "platforms/windows/common"),
                        "-I" + str(ROOT / "platforms/windows/tsf"),
                        "-I" + str(ROOT / "crates/host-api/include"), str(unit), "-o", str(binary)], check=True)
        return subprocess.run([str(binary)], check=False).returncode
if __name__ == "__main__":
    raise SystemExit(main())
