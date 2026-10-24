#include "Private.h"
#include "Globals.h"
#include "EditSession.h"
#include "MetasequoiaIME.h"
#include "CandidateListUIPresenter.h"
#include "CompositionProcessorEngine.h"
#include "../Composition/PreeditCaret.h"
#include "MetasequoiaIMEBaseStructure.h"
#include <debugapi.h>
#include <minwindef.h>
#include <string>
#include <fmt/xchar.h>
#include "FanyUtils.h"
#include "Ipc.h"
#include "CommitCandidateAndContinuePayload.h"
#include "FanyDefines.h"
#include "../Utils/PerfTimer.h"
#include "../HostRawCommit.h"
#pragma push_macro("max")
#undef max
#pragma push_macro("min")
#undef min
#include "../HostCandidateContinuation.h"
#pragma pop_macro("min")
#pragma pop_macro("max")
#include "../HostCharacterResult.h"
#include "../HostComposition.h"
#include "../HostKoreanKey.h"
#include "../KeyboardCancellation.h"
#include "../../../../shared/input/CompositionDisplay.h"
#include <limits>
#include <algorithm>
#include <atomic>

namespace
{
thread_local std::wstring g_toggleImeFallbackBuffer;

// 范围定位完整成功后才调用 SetSelection；定位未完成时不发布临时范围，引用仍由调用方释放。
HRESULT SetCompositionCaret(ITfRange *range, ITfContext *context, TfEditCookie ec, std::size_t position)
{
    if (range == nullptr || context == nullptr || position > static_cast<std::size_t>((std::numeric_limits<LONG>::max)()))
        return E_INVALIDARG;
    HRESULT hr = range->Collapse(ec, TF_ANCHOR_START);
    if (FAILED(hr)) return hr;
    LONG shifted = 0;
    const LONG requested = static_cast<LONG>(position);
    hr = range->ShiftEnd(ec, requested, &shifted, nullptr);
    if (FAILED(hr)) return hr;
    if (shifted != requested) return E_FAIL;
    hr = range->Collapse(ec, TF_ANCHOR_END);
    if (FAILED(hr)) return hr;
    TF_SELECTION selection = {};
    selection.range = range;
    selection.style.ase = TF_AE_NONE;
    selection.style.fInterimChar = FALSE;
    return context->SetSelection(ec, 1, &selection);
}

HRESULT ClearKeyboardRange(ITfRange *range, TfEditCookie ec)
{
#ifdef __MINGW32__
    return range->SetText(ec, 0, nullptr, 0);
#else
    __try { return range->SetText(ec, 0, nullptr, 0); }
    __except (EXCEPTION_EXECUTE_HANDLER) { return E_FAIL; }
#endif
}

HRESULT EndKeyboardComposition(ITfComposition *composition, TfEditCookie ec)
{
#ifdef __MINGW32__
    return composition->EndComposition(ec);
#else
    __try { return composition->EndComposition(ec); }
    __except (EXCEPTION_EXECUTE_HANDLER) { return E_FAIL; }
#endif
}

class CKeyboardCancellationEditSession final : public CEditSessionBase
{
  public:
    CKeyboardCancellationEditSession(CMetasequoiaIME *service, ITfContext *context,
                                     ITfComposition *composition, uint64_t focus, uint64_t epoch)
        : CEditSessionBase(service, context), composition_(composition), focus_(focus), epoch_(epoch)
    {
        composition_->AddRef();
    }
    ~CKeyboardCancellationEditSession() override { composition_->Release(); }
    STDMETHODIMP DoEditSession(TfEditCookie ec) override
    {
        return _pTextService->_ApplyKeyboardCancellation(ec, _pContext, composition_, focus_, epoch_);
    }
  private:
    ITfComposition *composition_;
    uint64_t focus_;
    uint64_t epoch_;
};

WCHAR GetPairedPunctuationClosing(const std::wstring &text)
{
    if (text.empty())
    {
        return 0;
    }
    // 成对表在 Global/PairedPunctuationHostPolicy.h，候选上屏的补全用同一张表。
    return Global::PairedPunctuationClosingFor(text.back());
}

} // namespace

//////////////////////////////////////////////////////////////////////
//
// CMetasequoiaIME class
//
//////////////////////////////////////////////////////////////////////

//+---------------------------------------------------------------------------
//
// _IsRangeCovered
//
// Returns TRUE if pRangeTest is entirely contained within pRangeCover.
//
//----------------------------------------------------------------------------

BOOL CMetasequoiaIME::_IsRangeCovered(TfEditCookie ec, _In_ ITfRange *pRangeTest, _In_ ITfRange *pRangeCover)
{
    LONG lResult = 0;
    ;

    if (FAILED(pRangeCover->CompareStart(ec, pRangeTest, TF_ANCHOR_START, &lResult)) || (lResult > 0))
    {
        return FALSE;
    }

    if (FAILED(pRangeCover->CompareEnd(ec, pRangeTest, TF_ANCHOR_END, &lResult)) || (lResult < 0))
    {
        return FALSE;
    }

    return TRUE;
}

//+---------------------------------------------------------------------------
//
// _DeleteCandidateList
//
//----------------------------------------------------------------------------

VOID CMetasequoiaIME::_DeleteCandidateList(BOOL isForce, _In_opt_ ITfContext *pContext)
{
    PerfTimer timer;
    pContext;

    CCompositionProcessorEngine *pCompositionProcessorEngine = nullptr;
    pCompositionProcessorEngine = _pCompositionProcessorEngine;
    if (pCompositionProcessorEngine)
    {
        PerfTimer purgeTimer;
        pCompositionProcessorEngine->PurgeVirtualKey();
        double purgeElapsedMs = purgeTimer.ElapsedMs();
    }

    double endCandidateElapsedMs = 0;
    if (_pCandidateListUIPresenter)
    {
        PerfTimer endCandidateTimer;
        CCandidateListUIPresenter *pPresenter = _pCandidateListUIPresenter;
        _pCandidateListUIPresenter = nullptr;
        // In Korean and Zhuyin the only list is the one the user opens, and it can close while the composition keeps going; the Server follows that from the keys themselves.
        if (msime::windows::scheme::OpensCandidateList(Global::InputModeScheme.load(std::memory_order_relaxed)))
        {
            pPresenter->_ForgetCandidateUiSession();
        }
        if (isForce || _msgWndHandle == nullptr)
        {
            delete pPresenter; // destructor calls _EndCandidateList() once
        }
        else
        {
            _ScheduleCandidatePresenterCleanup(pPresenter);
        }
        endCandidateElapsedMs = endCandidateTimer.ElapsedMs();

        _candidateMode = CANDIDATE_NONE;
        _isCandidateWithWildcard = FALSE;
    }
}

//+---------------------------------------------------------------------------
//
// _HandleComplete
//
//----------------------------------------------------------------------------

HRESULT CMetasequoiaIME::_HandleComplete(TfEditCookie ec, _In_ ITfContext *pContext)
{
    PerfTimer timer;
    g_toggleImeFallbackBuffer.clear();
    _creatingWordRestoreHistory.clear();
    PerfTimer deleteTimer;
    _DeleteCandidateList(FALSE, pContext);
    double deleteElapsedMs = deleteTimer.ElapsedMs();

    // just terminate the composition
    PerfTimer terminateTimer;
    _TerminateComposition(ec, pContext);
    double terminateElapsedMs = terminateTimer.ElapsedMs();

    return S_OK;
}

HRESULT CMetasequoiaIME::_HandleCompleteCommitFirst(TfEditCookie ec, _In_ ITfContext *pContext)
{
    PerfTimer timer;
    g_toggleImeFallbackBuffer.clear();
    _creatingWordRestoreHistory.clear();

    PerfTimer deleteTimer;
    _DeleteCandidateList(FALSE, pContext);
    double deleteElapsedMs = deleteTimer.ElapsedMs();

    PerfTimer terminateTimer;
    _TerminateComposition(ec, pContext);
    double terminateElapsedMs = terminateTimer.ElapsedMs();

    return S_OK;
}

HRESULT CMetasequoiaIME::_HandleHostRawCommit(TfEditCookie ec, _In_ ITfContext *pContext)
{
    auto *host = _pCompositionProcessorEngine->GetHostEngineAdapter();
    if (!host || !host->valid()) return S_FALSE;
    HRESULT writeResult = E_FAIL;
    std::string error;
    const auto status = msime::tsf::CommitHostRaw(*host, [&](const std::string &text) {
        if (text.size() > static_cast<size_t>((std::numeric_limits<int>::max)())) return false;
        const int length = MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, text.data(),
                                               static_cast<int>(text.size()), nullptr, 0);
        if (length <= 0) return false;
        std::wstring commit(static_cast<size_t>(length), L'\0');
        if (MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, text.data(),
                               static_cast<int>(text.size()), commit.data(), length) != length) return false;
        CStringRange range;
        range.Set(commit.c_str(), commit.size());
        writeResult = _AddCharAndFinalize(ec, pContext, &range);
        if (FAILED(writeResult)) return false;
        _smartPunctuationShadowChar = commit.back();
        _smartPunctuationShadowValid = true;
        return true;
    }, [&] {
        GlobalIme::word_for_creating_word.clear();
        GlobalIme::pending_create_word_preedit.clear();
        _HandleCompleteCommitFirst(ec, pContext);
    }, &error);
    if (status == msime::tsf::RawCommitStatus::Completed) return S_OK;
    if (status == msime::tsf::RawCommitStatus::Unhandled) return S_FALSE;
    return FAILED(writeResult) ? writeResult : E_FAIL;
}

namespace
{
// 宿主会话当前的 editing_text（日语是罗马字原文）。宿主会话不可用或读不出 view 时为空，日语转换的记号随之不成立，回车照旧上屏假名。
std::string HostEditingText(CCompositionProcessorEngine *engine)
{
    auto *host = engine ? engine->GetHostEngineAdapter() : nullptr;
    if (!host || !host->valid())
        return {};
    std::string raw, error;
    msime::tsf::EngineResult result;
    if (!host->view(&raw, &error) || !msime::tsf::EngineSessionAdapter::parse_result(raw, &result, &error))
        return {};
    return result.view.editing_text;
}
} // namespace

void CMetasequoiaIME::_NoteJapaneseConversionStarted()
{
    _japaneseConversion.compositionEpoch = _CaptureCompositionEpoch();
    _japaneseConversion.reading = HostEditingText(_pCompositionProcessorEngine);
}

bool CMetasequoiaIME::_JapaneseEnterCommitsCandidate(UINT modifiers)
{
    const bool japanese =
        Global::InputModeScheme.load(std::memory_order_relaxed) == msime::windows::scheme::Japanese;
    if (!japanese || _japaneseConversion.compositionEpoch == 0)
        return false;
    // 线上的修饰键位 Shift=1、Control=2、Alt=4；带修饰键的回车保留原来的意思。
    return Global::JapaneseEnterCommitsCandidate(_japaneseConversion, _CaptureCompositionEpoch(),
                                                 HostEditingText(_pCompositionProcessorEngine), japanese,
                                                 (modifiers & 0b111u) == 0);
}

HRESULT CMetasequoiaIME::_HandleConversionKey(TfEditCookie ec, _In_ ITfContext *pContext, bool enter,
                                              uint64_t requestId, bool *handled)
{
    *handled = false;
    auto *host = _pCompositionProcessorEngine ? _pCompositionProcessorEngine->GetHostEngineAdapter() : nullptr;
    if (!host || !host->valid() || !_IsComposing() || !msime::tsf::HostConversionActive(*host))
        return S_OK;
    *handled = true;
    // 回车在 Server 那边走 ConversionCommit，不发回复帧。
    if (enter)
        return _HandleHostRawCommit(ec, pContext);
    // 空格在 Server 那边是一次选择，有回复（选中或上屏），内容由 TIP 自己的宿主会话给出，这里只把它读掉，免得留在待取的回复里。
    if (requestId != FANY_IME_NO_REQUEST_ID)
        (void)TryReadDataFromServerPipeWithTimeout(requestId, /*abortTransportOnTimeout=*/false);
    std::string raw, error;
    msime::tsf::EngineResult result;
    if (!host->command(MSIME_COMMIT_CANDIDATE, &raw, &error) ||
        !msime::tsf::EngineSessionAdapter::parse_result(raw, &result, &error))
        return E_FAIL;
    if (result.has_commit && !result.commit.empty())
    {
        if (result.commit.size() > static_cast<size_t>((std::numeric_limits<int>::max)())) return E_FAIL;
        const int length = MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, result.commit.data(),
                                               static_cast<int>(result.commit.size()), nullptr, 0);
        if (length <= 0) return E_FAIL;
        std::wstring commit(static_cast<size_t>(length), L'\0');
        if (MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, result.commit.data(),
                                static_cast<int>(result.commit.size()), commit.data(), length) != length)
            return E_FAIL;
        GlobalIme::word_for_creating_word.clear();
        GlobalIme::pending_create_word_preedit.clear();
        CStringRange range;
        range.Set(commit.c_str(), commit.size());
        const HRESULT hr = _AddCharAndFinalize(ec, pContext, &range);
        _DeleteCandidateList(FALSE, pContext);
        return hr;
    }
    // 选中的段钉住后组字还在，按引擎视图重画整句和光标处的候选。
    return _HandleCompositionInputWorker(_pCompositionProcessorEngine, ec, pContext, FANY_IME_NO_REQUEST_ID);
}

HRESULT CMetasequoiaIME::_HandleSyllableCommit(TfEditCookie ec, _In_ ITfContext *pContext, UINT code, WCHAR wch,
                                               uint64_t replayToken)
{
    std::wstring text;
    bool keyText = msime::tsf::is_host_text_key(wch);
    // Set when the host had already let go of the composition the document still shows.
    bool hostLetGo = true;
    auto *host = _pCompositionProcessorEngine ? _pCompositionProcessorEngine->GetHostEngineAdapter() : nullptr;
    if (host && host->valid())
    {
        std::string error;
        auto ended = msime::tsf::EndHostComposition(*host, Global::InputModeScheme.load(std::memory_order_relaxed),
                                                    wch, &error);
        hostLetGo = ended.hostLetGo;
        keyText = ended.keyFollows;
        if (!ended.commit.empty() && ended.commit.size() <= static_cast<size_t>((std::numeric_limits<int>::max)()))
        {
            const int length = MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, ended.commit.data(),
                                                   static_cast<int>(ended.commit.size()), nullptr, 0);
            if (length > 0)
            {
                text.assign(static_cast<size_t>(length), L'\0');
                if (MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, ended.commit.data(),
                                        static_cast<int>(ended.commit.size()), text.data(), length) != length)
                    text.clear();
            }
        }
    }
    GlobalIme::word_for_creating_word.clear();
    GlobalIme::pending_create_word_preedit.clear();
    // No commit from the host means focus or a scheme change already made it let go of the syllable. The composition still shows that syllable, so it is ended first, which leaves that text in the document as the commit, and the key's character goes in after it.
    if (hostLetGo) _HandleCompleteCommitFirst(ec, pContext);
    if (keyText) text.push_back(wch);
    if (!text.empty())
    {
        CStringRange range;
        range.Set(text.c_str(), text.size());
        const HRESULT hr = _AddCharAndFinalize(ec, pContext, &range);
        if (FAILED(hr)) return hr;
        _smartPunctuationShadowChar = text.back();
        _smartPunctuationShadowValid = true;
    }
    _HandleCompleteCommitFirst(ec, pContext);
    // 方向或编辑键由应用处理，Server 通过组字终止时的 clear 同步；带文本的键会自行结束 Server 中的音节。
    if (code != 0 && !msime::tsf::is_host_text_key(wch) && Global::g_connected) SendHideCandidateWndEventToUIProcess();
    if (replayToken != 0 && !msime::tsf::is_host_text_key(wch))
        _QueueKoreanSyllableKeyReplay(code, pContext, replayToken);
    return S_OK;
}

namespace
{
// Under the Korean and Zhuyin rules the Engine lists candidates only after MSIME_OPEN_CANDIDATE_LIST (or Zhuyin's Space), so a composing view with candidates is the open list (msime_client.h). The TIP never enters a local or dedicated English mode in either scheme, the two states that keep their own rules there.
bool KoreanHanjaListOpen(const msime::tsf::EngineView &view)
{
    return msime::windows::scheme::OpensCandidateList(static_cast<int>(view.scheme)) && !view.editing_text.empty() &&
           !view.candidates.empty();
}
} // namespace

CMetasequoiaIME::HostComposedView CMetasequoiaIME::_ReadHostComposedView() const
{
    HostComposedView result;
    auto *host = _pCompositionProcessorEngine ? _pCompositionProcessorEngine->GetHostEngineAdapter() : nullptr;
    if (!host || !host->valid()) return result;
    std::string raw, error;
    msime::tsf::EngineResult current;
    if (!host->view(&raw, &error) || !msime::tsf::EngineSessionAdapter::parse_result(raw, &current, &error))
        return result;
    result.listOpen = KoreanHanjaListOpen(current.view);
    result.composing = !current.view.editing_text.empty();
    result.spellingSymbols = std::move(current.view.spelling_symbols);
    return result;
}

bool CMetasequoiaIME::_IsKoreanHanjaListOpen() const
{
    auto *host = _pCompositionProcessorEngine ? _pCompositionProcessorEngine->GetHostEngineAdapter() : nullptr;
    if (!host || !host->valid()) return false;
    std::string raw, error;
    msime::tsf::EngineResult current;
    return host->view(&raw, &error) && msime::tsf::EngineSessionAdapter::parse_result(raw, &current, &error) &&
           KoreanHanjaListOpen(current.view);
}

HRESULT CMetasequoiaIME::_HandleKoreanHanjaKey(TfEditCookie ec, _In_ ITfContext *pContext, UINT code, WCHAR wch,
                                               uint64_t requestId, uint64_t replayToken)
{
    auto *host = _pCompositionProcessorEngine ? _pCompositionProcessorEngine->GetHostEngineAdapter() : nullptr;
    if (!host || !host->valid()) return S_OK;
    std::string raw, error;
    msime::tsf::EngineResult current;
    if (!host->view(&raw, &error) || !msime::tsf::EngineSessionAdapter::parse_result(raw, &current, &error))
        return E_FAIL;
    const auto key = msime::windows::korean_hanja_key(code, static_cast<uint32_t>(wch));
    const int scheme = static_cast<int>(current.view.scheme);
    const bool listOpen = KoreanHanjaListOpen(current.view);
    const bool trigger = msime::windows::opens_candidate_list(scheme, code, listOpen);
    if (!trigger && !listOpen)
    {
        // 列表在排队期间关闭后按无列表规则处理；已经吞下的方向或编辑键携带原槽令牌，上屏后再交给应用。
        switch (msime::tsf::host_composed_key_action(scheme, code, wch, true, false, current.view.spelling_symbols))
        {
        case msime::tsf::KoreanKeyAction::CommitWithText:
            return _HandleSyllableCommit(ec, pContext, code, wch);
        case msime::tsf::KoreanKeyAction::CommitAndPass:
            return _HandleSyllableCommit(ec, pContext, code, wch, replayToken);
        default:
            break;
        }
        if (code == VK_BACK) return _HandleCompositionBackspace(ec, pContext, requestId);
        if (code == VK_ESCAPE) return _HandleCancel(ec, pContext);
        return S_OK;
    }
    // The Hanja key with nothing composing is the application's and never eaten; one eaten while the composition ended on the way is spent.
    if (current.view.editing_text.empty()) return S_OK;

    raw.clear();
    bool applied = false;
    if (trigger)
    {
        applied = host->command(MSIME_OPEN_CANDIDATE_LIST, &raw, &error);
    }
    else if (key.kind == msime::windows::KoreanHanjaKeyKind::Select)
    {
        // A digit past the visible page chooses nothing and is swallowed, as with any candidate list.
        if (key.value >= current.view.candidates.size()) return S_OK;
        applied = host->select(current.view.generation, current.view.candidates[key.value].index, &raw, &error);
    }
    else
    {
        applied = host->command(key.value, &raw, &error);
    }
    msime::tsf::EngineResult result;
    if (!applied || !msime::tsf::EngineSessionAdapter::parse_result(raw, &result, &error)) return E_FAIL;

    if (result.has_commit && !result.commit.empty() &&
        result.commit.size() <= static_cast<size_t>((std::numeric_limits<int>::max)()))
    {
        // A Hanja was chosen and the composition is over.
        const int length = MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, result.commit.data(),
                                               static_cast<int>(result.commit.size()), nullptr, 0);
        if (length <= 0) return E_FAIL;
        std::wstring text(static_cast<size_t>(length), L'\0');
        if (MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, result.commit.data(),
                                static_cast<int>(result.commit.size()), text.data(), length) != length)
            return E_FAIL;
        CStringRange range;
        range.Set(text.c_str(), text.size());
        const HRESULT hr = _AddCharAndFinalize(ec, pContext, &range);
        if (FAILED(hr)) return hr;
        _smartPunctuationShadowChar = text.back();
        _smartPunctuationShadowValid = true;
        return _HandleCompleteCommitFirst(ec, pContext);
    }
    // The list opened, moved or closed and the composition is still going: a Zhuyin choice fixes that reading and keeps composing. A closed list takes the presenter with it (quietly, see _DeleteCandidateList), so a later commit does not hide a composition the Server is still holding.
    if (result.view.candidates.empty()) _DeleteCandidateList(FALSE, pContext);
    if (result.view.editing_text.empty()) return _HandleCompleteCommitFirst(ec, pContext);
    return _HandleCompositionInputWorker(_pCompositionProcessorEngine, ec, pContext, FANY_IME_NO_REQUEST_ID);
}

namespace
{
UINT NextApplicationKeyReplayMarker()
{
    // DLL 禁用了函数内静态量的动态初始化保护；原子量采用常量初始化，所有 TIP 实例共用且永不回绕。
    static std::atomic<UINT> nonce{0};
    UINT current = nonce.load(std::memory_order_relaxed);
    while (current < APPLICATION_KEY_REPLAY_NONCE_MASK)
    {
        if (nonce.compare_exchange_weak(current, current + 1, std::memory_order_relaxed))
            return APPLICATION_KEY_REPLAY_PREFIX | (current + 1);
    }
    return 0;
}
constexpr ULONGLONG kApplicationKeyReplayWaitMs = 2000;
} // namespace

bool CMetasequoiaIME::_IsDeferredKeyOwnerCurrent(ITfContext *context, uint64_t focusGeneration,
                                                   HWND foregroundWindow, bool allowPendingReset) const
{
    const auto localCurrent = [this, context, focusGeneration, foregroundWindow, allowPendingReset]() {
        return context && _pThreadMgr && focusGeneration == _deferredKeyFocusGeneration &&
               (allowPendingReset || !_localSessionResetPending.load(std::memory_order_acquire)) && foregroundWindow &&
               GetForegroundWindow() == foregroundWindow;
    };
    if (!localCurrent()) return false;
    // 离线没有 worker Ready 令牌，仍须核验真实线程焦点与顶层 context；document focus 本身不代表前台。
    ITfThreadMgr *threadMgr = _pThreadMgr;
    threadMgr->AddRef();
    BOOL threadFocused = FALSE;
    ITfDocumentMgr *documentMgr = nullptr;
    ITfContext *topContext = nullptr;
    bool matches = false;
    if (SUCCEEDED(threadMgr->IsThreadFocus(&threadFocused)) && threadFocused &&
        SUCCEEDED(threadMgr->GetFocus(&documentMgr)) && documentMgr &&
        SUCCEEDED(documentMgr->GetTop(&topContext)) && topContext)
        matches = topContext == context;
    matches = matches && SUCCEEDED(threadMgr->IsThreadFocus(&threadFocused)) && threadFocused;
    if (topContext) topContext->Release();
    if (documentMgr) documentMgr->Release();
    // 释放引用也可能重入；最后再次核验本地身份。
    threadMgr->Release();
    return matches && _pThreadMgr == threadMgr && localCurrent();
}

bool CMetasequoiaIME::_IsDeferredApplicationTextCurrent(uint64_t replayToken, uint64_t focusGeneration,
                                                           ITfContext *context, HWND foregroundWindow,
                                                           uint64_t focusToken)
{
    if (!_IsDeferredKeyReplayCurrent(replayToken, focusGeneration, context)) return false;
    const bool enabled = !_IsKeyboardDisabled();
    const bool ready = _serverUnavailableFallbackActive || _IsFocusSessionCurrent(focusToken, context);
    const bool ownerCurrent = _IsDeferredKeyOwnerCurrent(context, focusGeneration, foregroundWindow);
    // COM 查询可能清掉或替换槽，最后只读本地身份与在线 Ready 令牌。
    return enabled && ready && ownerCurrent && !_localSessionResetPending.load(std::memory_order_acquire) &&
        GetForegroundWindow() == foregroundWindow &&
        _IsDeferredKeyReplayCurrent(replayToken, focusGeneration, context) &&
        (_serverUnavailableFallbackActive || _IsFocusSessionCurrent(focusToken));
}

bool CMetasequoiaIME::_HasPendingApplicationKeyReplay() const
{
    return _applicationKeyReplay.marker != 0 &&
           !(_applicationKeyReplay.downObserved && _applicationKeyReplay.upSent);
}

void CMetasequoiaIME::_ClearApplicationKeyReplay()
{
    const auto cancelled = _applicationKeyReplay;
    _applicationKeyReplay = {};
    if (cancelled.context) cancelled.context->Release();
    if (cancelled.downSent && !cancelled.upSent)
    {
        // 取消后只补抬起，不能把按下发送到新的焦点。系统拒绝注入时只能尽力释放。
        INPUT release = {};
        release.type = INPUT_KEYBOARD;
        release.ki.wVk = static_cast<WORD>(cancelled.virtualKey);
        release.ki.dwFlags = KEYEVENTF_KEYUP;
        if (cancelled.virtualKey != VK_TAB && cancelled.virtualKey != VK_RETURN &&
            cancelled.virtualKey != VK_BACK && cancelled.virtualKey != VK_ESCAPE)
            release.ki.dwFlags |= KEYEVENTF_EXTENDEDKEY;
        release.ki.dwExtraInfo = cancelled.marker;
        (void)SendInput(1, &release, sizeof(INPUT));
    }
}

bool CMetasequoiaIME::_QueueKoreanSyllableKeyReplay(UINT virtualKey, ITfContext *context, uint64_t replayToken)
{
    if (!_msgWndHandle || !(msime::tsf::is_korean_caret_or_edit_key(virtualKey) || virtualKey == VK_BACK || virtualKey == VK_ESCAPE) ||
        !_IsDeferredKeyReplayCurrent(replayToken, _deferredKeyFocusGeneration, context) ||
        _HasPendingApplicationKeyReplay())
        return false;
    const uint64_t focusToken = _CaptureFocusSessionToken();
    if (!focusToken && !_serverUnavailableFallbackActive) return false;
    const UINT marker = NextApplicationKeyReplayMarker();
    if (!marker) return false;
    _ClearApplicationKeyReplay();
    context->AddRef();
    _applicationKeyReplay.context = context;
    _applicationKeyReplay.marker = marker;
    _applicationKeyReplay.virtualKey = virtualKey;
    _applicationKeyReplay.focusToken = focusToken;
    _applicationKeyReplay.focusGeneration = _deferredKeyFocusGeneration;
    _applicationKeyReplay.foregroundWindow = _deferredKeyInFlight.foregroundWindow
        ? _deferredKeyInFlight.foregroundWindow : GetForegroundWindow();
    _applicationKeyReplay.deadline = GetTickCount64() + kApplicationKeyReplayWaitMs;
    // 投递失败仍保留屏障，由已有 UI 定时器尝试发送；这里不能同步进入应用按键处理。
    (void)PostMessage(_msgWndHandle, WM_ReplayKoreanSyllableKey, static_cast<WPARAM>(marker), 0);
    return true;
}

void CMetasequoiaIME::_RunKoreanSyllableKeyReplay(UINT marker)
{
    if (!marker || marker != _applicationKeyReplay.marker || !_HasPendingApplicationKeyReplay()) return;
    ITfContext *context = _applicationKeyReplay.context;
    context->AddRef();
    const auto captured = _applicationKeyReplay;
    const bool ready = _serverUnavailableFallbackActive || _IsFocusSessionCurrent(captured.focusToken, context);
    const bool ownerCurrent = _IsDeferredKeyOwnerCurrent(context, captured.focusGeneration, captured.foregroundWindow);
    context->Release();
    // 查询焦点可能重入 COM，再次检查槽身份。
    if (marker != _applicationKeyReplay.marker) return;
    if (!ready || !ownerCurrent || _localSessionResetPending.load(std::memory_order_acquire) ||
        GetForegroundWindow() != captured.foregroundWindow ||
        (!_serverUnavailableFallbackActive && !_IsFocusSessionCurrent(captured.focusToken)) ||
        _applicationKeyReplay.focusGeneration != _deferredKeyFocusGeneration ||
        GetTickCount64() >= _applicationKeyReplay.deadline)
    {
        _ClearApplicationKeyReplay();
        _ScheduleDeferredKeyDownDrain(false);
        return;
    }
    if (_applicationKeyReplay.downSent && _applicationKeyReplay.upSent) return;
    INPUT inputs[2] = {};
    inputs[0].type = INPUT_KEYBOARD;
    inputs[0].ki.wVk = static_cast<WORD>(_applicationKeyReplay.virtualKey);
    if (_applicationKeyReplay.virtualKey != VK_TAB && _applicationKeyReplay.virtualKey != VK_RETURN &&
        _applicationKeyReplay.virtualKey != VK_BACK && _applicationKeyReplay.virtualKey != VK_ESCAPE)
        inputs[0].ki.dwFlags = KEYEVENTF_EXTENDEDKEY;
    inputs[0].ki.dwExtraInfo = marker;
    inputs[1] = inputs[0];
    inputs[1].ki.dwFlags |= KEYEVENTF_KEYUP;
    // 部分发送后只能补抬起，不能让已经进入输入流的方向键再移动一次。
    const bool releaseOnly = _applicationKeyReplay.downSent;
    const UINT sent = SendInput(releaseOnly ? 1u : 2u, releaseOnly ? &inputs[1] : inputs, sizeof(INPUT));
    if (marker != _applicationKeyReplay.marker)
    {
        if (!releaseOnly && sent == 1) (void)SendInput(1, &inputs[1], sizeof(INPUT));
        return;
    }
    if (releaseOnly)
        _applicationKeyReplay.upSent = sent == 1;
    else
    {
        _applicationKeyReplay.downSent = sent > 0;
        _applicationKeyReplay.upSent = sent == 2;
    }
    if (sent) _InvalidateSmartPunctuationShadow();
    if (!_HasPendingApplicationKeyReplay()) _ScheduleDeferredKeyDownDrain(false);
}

bool CMetasequoiaIME::_ObserveApplicationKeyReplay(ITfContext *context, UINT virtualKey, ULONG_PTR marker)
{
    if (marker != _applicationKeyReplay.marker || virtualKey != _applicationKeyReplay.virtualKey ||
        context != _applicationKeyReplay.context || !_applicationKeyReplay.downSent ||
        _applicationKeyReplay.focusGeneration != _deferredKeyFocusGeneration)
        return false;
    if (!_applicationKeyReplay.downObserved && GetTickCount64() >= _applicationKeyReplay.deadline)
    {
        _ClearApplicationKeyReplay();
        _ScheduleDeferredKeyDownDrain(false);
        return false;
    }
    context->AddRef();
    const auto captured = _applicationKeyReplay;
    const bool ready = _serverUnavailableFallbackActive || _IsFocusSessionCurrent(captured.focusToken, context);
    const bool ownerCurrent = _IsDeferredKeyOwnerCurrent(context, captured.focusGeneration, captured.foregroundWindow);
    context->Release();
    if (!ready || !ownerCurrent || _localSessionResetPending.load(std::memory_order_acquire) ||
        GetForegroundWindow() != captured.foregroundWindow ||
        (!_serverUnavailableFallbackActive && !_IsFocusSessionCurrent(captured.focusToken)) ||
        marker != _applicationKeyReplay.marker ||
        _applicationKeyReplay.focusGeneration != _deferredKeyFocusGeneration) return false;
    // Test 是查询而非应用处理完成；只能投递后续 drain，让当前按下先返回给应用。已确认身份保留到下次回放或 clear，兼容重复 probe。
    if (!_applicationKeyReplay.downObserved)
    {
        _applicationKeyReplay.downObserved = true;
        if (!_HasPendingApplicationKeyReplay()) _ScheduleDeferredKeyDownDrain(false);
    }
    return true;
}

void CMetasequoiaIME::_PollApplicationKeyReplay()
{
    if (_HasPendingApplicationKeyReplay())
        _RunKoreanSyllableKeyReplay(_applicationKeyReplay.marker);
    if (!_HasPendingApplicationKeyReplay()) _ScheduleDeferredKeyDownDrain(false);
}

//+---------------------------------------------------------------------------
//
// _HandleCancel
//
//----------------------------------------------------------------------------

bool CMetasequoiaIME::_IsKeyboardCancellationCurrent(ITfContext *context, ITfComposition *composition,
                                                    uint64_t focusToken, uint64_t compositionEpoch) const
{
    if (!context || !composition || !SupportsKeyboardCompositionCancel(this) ||
        !_IsFocusSessionCurrent(focusToken, context))
        return false;
    // GetFocus/GetTop cross COM; recheck local identities after they return.
    return _pContext == context && _IsCompositionCurrent(composition) &&
           _IsFocusSessionCurrent(focusToken) &&
           msime::tsf::keyboard_cancellation_matches(
               {focusToken, compositionEpoch}, {_CaptureFocusSessionToken(), _CaptureCompositionEpoch()},
               SupportsKeyboardCompositionCancel(this), !_voiceCompositionActive);
}

HRESULT CMetasequoiaIME::_RequestKeyboardCancellation(uint64_t focusToken, uint64_t compositionEpoch)
{
    if (!_pComposition || !_pContext || !SupportsKeyboardCompositionCancel(this)) return S_FALSE;
    // Keep exact references across both focus validation and RequestEditSession.
    ITfContext *context = _pContext;
    ITfComposition *composition = _pComposition;
    context->AddRef();
    composition->AddRef();
    HRESULT result = S_FALSE;
    if (_IsKeyboardCancellationCurrent(context, composition, focusToken, compositionEpoch))
    {
        auto *session = new (std::nothrow) CKeyboardCancellationEditSession(
            this, context, composition, focusToken, compositionEpoch);
        if (!session) result = E_OUTOFMEMORY;
        else
        {
            result = E_FAIL;
            const HRESULT requested = context->RequestEditSession(
                _tfClientId, session, TF_ES_ASYNCDONTCARE | TF_ES_READWRITE, &result);
            session->Release();
            if (FAILED(requested)) result = requested;
        }
    }
    composition->Release();
    context->Release();
    return result;
}

HRESULT CMetasequoiaIME::_ApplyKeyboardCancellation(TfEditCookie ec, ITfContext *context,
                                                   ITfComposition *composition, uint64_t focusToken,
                                                   uint64_t compositionEpoch)
{
    struct RangeReference
    {
        ITfRange *value = nullptr;
        ~RangeReference() { if (value) value->Release(); }
    } range;
    const auto current = [&] {
        return _IsKeyboardCancellationCurrent(context, composition, focusToken, compositionEpoch);
    };
    return msime::tsf::cancel_keyboard_composition(
        S_OK, S_FALSE, current,
        [&]() -> HRESULT {
            const HRESULT result = composition->GetRange(&range.value);
            return result == S_OK && !range.value ? E_FAIL : result;
        },
        [&]() -> HRESULT {
            const HRESULT result = ClearKeyboardRange(range.value, ec);
            if (result != S_OK) return result;
            if (!current()) return S_FALSE;
            if (!_CancelHostComposition()) return E_FAIL;
            g_toggleImeFallbackBuffer.clear();
            GlobalIme::word_for_creating_word.clear();
            GlobalIme::pending_create_word_preedit.clear();
            return S_OK;
        },
        [&] { return EndKeyboardComposition(composition, ec); },
        [&]() -> HRESULT {
            // The existing termination callback detaches this exact object
            // before cleaning its presenter. Never clean a re-entrant new one.
            return _IsCompositionCurrent(composition) ? OnCompositionTerminated(ec, composition) : S_OK;
        });
}

HRESULT CMetasequoiaIME::_HandleHostCandidateNavigation(TfEditCookie ec, ITfContext *context, uint32_t command)
{
    auto *host = _pCompositionProcessorEngine ? _pCompositionProcessorEngine->GetHostEngineAdapter() : nullptr;
    if (!Global::IsUiLessMode() || !host || !host->valid() || !_IsComposing()) return S_FALSE;
    std::string raw, error;
    if (command && !host->command(command, &raw, &error)) return E_FAIL;
    return _HandleCompositionInputWorker(_pCompositionProcessorEngine, ec, context, FANY_IME_NO_REQUEST_ID);
}

HRESULT CMetasequoiaIME::_HandleHostOnlineCandidates(TfEditCookie ec, ITfContext *context,
                                                    const std::wstring &payload)
{
    auto *host = _pCompositionProcessorEngine ? _pCompositionProcessorEngine->GetHostEngineAdapter() : nullptr;
    if (!Global::IsUiLessMode() || !host || !host->valid() || !_IsComposing()) return S_OK;
    const std::string encoded(payload.begin(), payload.end());
    if (!host->apply_online_update(encoded)) return S_OK;
    return _HandleCompositionInputWorker(_pCompositionProcessorEngine, ec, context, FANY_IME_NO_REQUEST_ID);
}

bool CMetasequoiaIME::_CancelHostComposition()
{
    auto *host = _pCompositionProcessorEngine ? _pCompositionProcessorEngine->GetHostEngineAdapter() : nullptr;
    if (!host || !host->valid()) return true;
    std::string raw, error;
    if (!host->command(MSIME_CANCEL, &raw, &error)) return false;
    // 韩文汉字列表或注音列表打开时，MSIME_CANCEL 只关闭列表、组字保留（msime_client.h）；越南文词和藏文音节串上的第一次 MSIME_CANCEL 只把原文重新显示出来；全拼、双拼整句改字时的第一次只退出改字回到拼音。组字还在就再发一次来丢弃它，与 Server 的 cancel_again 相同。
    msime::tsf::EngineResult result;
    if (msime::tsf::EngineSessionAdapter::parse_result(raw, &result, &error) &&
        !result.has_commit && !result.view.editing_text.empty())
        return host->command(MSIME_CANCEL, &raw, &error);
    return true;
}

HRESULT CMetasequoiaIME::_HandleEscape(TfEditCookie ec, _In_ ITfContext *pContext)
{
    // 越南文词和藏文音节串在第一次 Esc 时重新显示原文并继续组字；下一次 Esc 像其他组字一样丢弃它。
    auto *host = _pCompositionProcessorEngine ? _pCompositionProcessorEngine->GetHostEngineAdapter() : nullptr;
    std::string error;
    if (host && host->valid() && _IsComposing() && msime::tsf::RestoreHostRawOnEscape(*host, &error))
        return _HandleCompositionInputWorker(_pCompositionProcessorEngine, ec, pContext, FANY_IME_NO_REQUEST_ID);
    return _HandleCancel(ec, pContext);
}

HRESULT CMetasequoiaIME::_HandleCancel(TfEditCookie ec, _In_ ITfContext *pContext)
{
    PerfTimer timer;
    (void)_CancelHostComposition();
    g_toggleImeFallbackBuffer.clear();
    _creatingWordRestoreHistory.clear();
    GlobalIme::word_for_creating_word = L"";
    GlobalIme::pending_create_word_preedit.clear();
    PerfTimer removeDummyTimer;
    _RemoveDummyCompositionForComposing(ec, _pComposition);
    double removeDummyElapsedMs = removeDummyTimer.ElapsedMs();

    PerfTimer deleteTimer;
    _DeleteCandidateList(FALSE, pContext);
    double deleteElapsedMs = deleteTimer.ElapsedMs();

    PerfTimer terminateTimer;
    _TerminateComposition(ec, pContext);
    double terminateElapsedMs = terminateTimer.ElapsedMs();

    return S_OK;
}

HRESULT CMetasequoiaIME::_HandleToogleIMEMode(TfEditCookie ec, _In_ ITfContext *pContext)
{
    if (auto *host = _pCompositionProcessorEngine->GetHostEngineAdapter(); host && host->valid())
    {
        const HRESULT hr = _HandleHostRawCommit(ec, pContext);
        if (hr == S_OK)
            _pCompositionProcessorEngine->ApplyPendingImeModeAfterCompositionCommit(_GetThreadMgr(), _GetClientId());
        return hr;
    }
    CStringRange keyStrokebuffer = _pCompositionProcessorEngine->GetKeystrokeBuffer();
    std::wstring commitString;

    if (keyStrokebuffer.GetLength())
    {
        commitString.assign(keyStrokebuffer.Get(), keyStrokebuffer.GetLength());
    }
    else if (!g_toggleImeFallbackBuffer.empty())
    {
        commitString = g_toggleImeFallbackBuffer;
    }

    // Claim the reading string before ending composition so a second toggle
    // edit session cannot commit the same text again.
    _pCompositionProcessorEngine->PurgeVirtualKey();
    g_toggleImeFallbackBuffer.clear();

    if (!commitString.empty())
    {
        // Keyboard OPENCLOSE is still open here when Shift deferred the close.
        // Finalize in place; closing first makes CUAS/Win32 EDIT double-insert.
        CStringRange commitStringRange;
        commitStringRange.Set(commitString.c_str(), commitString.length());
        const HRESULT hr = _AddCharAndFinalize(ec, pContext, &commitStringRange);
        if (SUCCEEDED(hr))
        {
            _HandleComplete(ec, pContext);
        }
        else
        {
            _HandleCancel(ec, pContext);
            FanyUtils::SendKeys(commitString);
        }
    }
    else
    {
        _HandleComplete(ec, pContext);
    }

    _pCompositionProcessorEngine->ApplyPendingImeModeAfterCompositionCommit(_GetThreadMgr(), _GetClientId());
    return S_OK;
}

HRESULT CMetasequoiaIME::_HandleInsertText(TfEditCookie ec, _In_ ITfContext *pContext, const std::wstring &text)
{
    if (text.empty())
    {
        return S_OK;
    }

    CStringRange insertString;
    insertString.Set(text.c_str(), text.length());
    HRESULT hr = _AddCharAndFinalize(ec, pContext, &insertString);
    if (FAILED(hr))
    {
        return hr;
    }
    return _HandleCompleteCommitFirst(ec, pContext);
}

HRESULT CMetasequoiaIME::_HandleCommitCandidateAndContinue(TfEditCookie ec, _In_ ITfContext *pContext,
                                                           const std::wstring &payload)
{
    std::size_t consumed = 0;
    std::wstring commitText;
    if (!ParseCommitCandidateAndContinuePayload(payload, consumed, commitText))
    {
        return E_INVALIDARG;
    }

    CCompositionProcessorEngine *engine = _pCompositionProcessorEngine;
    if (auto *host = engine ? engine->GetHostEngineAdapter() : nullptr; host && host->valid())
    {
        std::string raw, error;
        msime::tsf::EngineResult live;
        if (!host->view(&raw, &error) || !msime::tsf::EngineSessionAdapter::parse_result(raw, &live, &error))
            return E_FAIL;
        return _ApplyHostCompositionResult(ec, pContext,
            msime::tsf::ServerWubiCommit(std::move(live), FanyUtils::wstring_to_string(commitText)), true);
    }
    const std::wstring buffer = engine ? engine->GetKeystrokeBuffer().ToWString() : std::wstring{};
    if (_pComposition == nullptr)
    {
        if (commitText.empty())
        {
            return S_OK;
        }
        CStringRange commitRange;
        commitRange.Set(commitText.c_str(), commitText.length());
        return _AddCharAndFinalize(ec, pContext, &commitRange);
    }

    const std::size_t consume = (std::min)(consumed, buffer.size());
    const std::wstring remainder = buffer.substr(consume);
    if (!commitText.empty())
    {
        CStringRange commitRange;
        commitRange.Set(commitText.c_str(), commitText.length());
        HRESULT hr = _InsertTextToComposition(ec, pContext, &commitRange);
        if (FAILED(hr))
        {
            hr = _AddComposingAndChar(ec, pContext, &commitRange);
        }
        if (FAILED(hr))
        {
            return hr;
        }
    }

    _HandleCompleteCommitFirst(ec, pContext);
    if (remainder.empty() || engine == nullptr)
    {
        return S_OK;
    }
    _StartComposition(pContext);
    if (_pComposition == nullptr)
    {
        CStringRange remainderRange;
        remainderRange.Set(remainder.c_str(), remainder.length());
        return _AddCharAndFinalize(ec, pContext, &remainderRange);
    }
    engine->PurgeVirtualKey();
    for (const wchar_t value : remainder)
    {
        engine->AddVirtualKey(value);
    }
    return _HandleCompositionInputWorker(engine, ec, pContext, FANY_IME_NO_REQUEST_ID);
}

HRESULT CMetasequoiaIME::_HandleCandidateContinuation(TfEditCookie ec, _In_ ITfContext *pContext,
                                                        const std::wstring &text)
{
    std::string payload;
    for (const auto ch : text)
    {
        if (ch <= 0 || ch > 0x7f)
        {
            return S_FALSE;
        }
        payload.push_back(static_cast<char>(ch));
    }
    auto *host = _pCompositionProcessorEngine ? _pCompositionProcessorEngine->GetHostEngineAdapter() : nullptr;
    if (!host || !msime::tsf::ContinueHostCandidate(*host, payload))
    {
        return S_FALSE;
    }
    return _HandleCompositionInputWorker(_pCompositionProcessorEngine, ec, pContext, FANY_IME_NO_REQUEST_ID);
}

HRESULT CMetasequoiaIME::_HandleUpdateVoiceComposition(TfEditCookie ec, _In_ ITfContext *pContext,
                                                       const std::wstring &text)
{
    if (text.empty())
    {
        return S_OK;
    }

    if (!_voiceCompositionActive)
    {
        if (_pCompositionProcessorEngine)
        {
            _pCompositionProcessorEngine->PurgeVirtualKey();
        }
        GlobalIme::word_for_creating_word.clear();
        GlobalIme::pending_create_word_preedit.clear();
        _DeleteCandidateList(FALSE, pContext);
        _voiceCompositionActive = true;
    }

    if (_pComposition == nullptr)
    {
        _StartComposition(pContext);
    }

    CStringRange voiceString;
    voiceString.Set(text.c_str(), text.length());
    return _AddComposingAndChar(ec, pContext, &voiceString);
}

HRESULT CMetasequoiaIME::_HandleCommitVoiceComposition(TfEditCookie ec, _In_ ITfContext *pContext,
                                                       const std::wstring &text)
{
    _voiceCompositionActive = false;
    if (_pCompositionProcessorEngine)
    {
        _pCompositionProcessorEngine->PurgeVirtualKey();
    }
    GlobalIme::word_for_creating_word.clear();
    GlobalIme::pending_create_word_preedit.clear();

    if (text.empty())
    {
        if (_pComposition == nullptr)
        {
            return S_OK;
        }
        return _HandleCompleteCommitFirst(ec, pContext);
    }

    CStringRange commitString;
    commitString.Set(text.c_str(), text.length());
    HRESULT hr = _AddCharAndFinalize(ec, pContext, &commitString);
    if (FAILED(hr))
    {
        return hr;
    }
    return _HandleCompleteCommitFirst(ec, pContext);
}

HRESULT CMetasequoiaIME::_HandleCancelVoiceComposition(TfEditCookie ec, _In_ ITfContext *pContext)
{
    if (!_voiceCompositionActive && _pComposition == nullptr)
    {
        return S_OK;
    }
    _voiceCompositionActive = false;
    if (_pCompositionProcessorEngine)
    {
        _pCompositionProcessorEngine->PurgeVirtualKey();
    }
    return _HandleCancel(ec, pContext);
}

//+---------------------------------------------------------------------------
//
// _HandleCompositionInput
//
// If the keystroke happens within a composition, eat the key and return S_OK.
//
//----------------------------------------------------------------------------

HRESULT CMetasequoiaIME::_HandleCompositionInput(TfEditCookie ec, _In_ ITfContext *pContext, WCHAR wch,
                                                 uint64_t requestId, bool groupedPanelKey)
{
    HRESULT workerResult = S_OK;
    ITfRange *pRangeComposition = nullptr;
    TF_SELECTION tfSelection;
    ULONG fetched = 0;
    BOOL isCovered = TRUE;
    DWORD_PTR previousLength = 0;

    CCompositionProcessorEngine *pCompositionProcessorEngine = nullptr;
    pCompositionProcessorEngine = _pCompositionProcessorEngine;

    // 在延迟按键屏障后面被分类为输入的注音、越南文或藏文按键，是按静态拼写规则判断的（VNI 的数字、列表投影为关闭时的大千键、藏文的威利符号）。由实时视图决定：它不拼写的键结束组字并跟在后面，与 Server 会话处理同一个键的方式一致。藏文组字时的空格也算组字接收的键（host_composition_takes_key）。
    if (const int scheme = Global::InputModeScheme.load(std::memory_order_relaxed);
        scheme != msime::windows::scheme::Korean && msime::windows::scheme::AlwaysInlinePreedit(scheme) &&
        msime::tsf::is_korean_text_key(wch) && pCompositionProcessorEngine->GetHostEngineAdapter() &&
        pCompositionProcessorEngine->GetHostEngineAdapter()->valid())
    {
        const auto hostView = _ReadHostComposedView();
        if (!msime::tsf::host_composition_takes_key(scheme, hostView.spellingSymbols, wch, hostView.composing))
            return _HandleSyllableCommit(ec, pContext, static_cast<UINT>(wch), wch);
    }

    if ((_pCandidateListUIPresenter != nullptr) && (_candidateMode != CANDIDATE_INCREMENTAL))
    {
        _HandleCompositionFinalize(ec, pContext, FALSE);
    }

    if (groupedPanelKey && (!SupportsGroupedPanelKeys() || _serverUnavailableFallbackActive ||
        !pCompositionProcessorEngine->GetHostEngineAdapter() ||
        !pCompositionProcessorEngine->GetHostEngineAdapter()->valid())) return E_FAIL;

    // A composition this key did not start may be showing a syllable the host has since let go of (see the Korean check below).
    const bool composingBeforeKey = _IsComposing() != FALSE;

    // Start the new (std::nothrow) compositon if there is no composition.
    if (!_IsComposing())
    {
        _StartComposition(pContext);
        if (!_IsComposing())
        {
            DebugTsfIssue47(L"composition-start-missing", requestId, 0, wch, CATEGORY_COMPOSING, FUNCTION_INPUT, 1,
                            FALSE, pCompositionProcessorEngine ? pCompositionProcessorEngine->GetVirtualKeyLength() : 0,
                            E_FAIL);
            return E_FAIL;
        }
    }

    // first, test where a keystroke would go in the document if we did an insert
    if (pContext->GetSelection(ec, TF_DEFAULT_SELECTION, 1, &tfSelection, &fetched) != S_OK || fetched != 1)
    {
        DebugTsfIssue47(L"composition-selection-failed", requestId, 0, wch, CATEGORY_COMPOSING, FUNCTION_INPUT, 1,
                        _IsComposing(),
                        pCompositionProcessorEngine ? pCompositionProcessorEngine->GetVirtualKeyLength() : 0, E_FAIL);
        return S_FALSE;
    }

    // is the insertion point covered by a composition?
    if (SUCCEEDED(_pComposition->GetRange(&pRangeComposition)))
    {
        isCovered = _IsRangeCovered(ec, tfSelection.range, pRangeComposition);

        pRangeComposition->Release();

        if (!isCovered)
        {
            DebugTsfIssue47(L"composition-selection-outside", requestId, 0, wch, CATEGORY_COMPOSING, FUNCTION_INPUT, 1,
                            _IsComposing(), pCompositionProcessorEngine->GetVirtualKeyLength(), S_FALSE);
            // 字符尚未交给本地会话，回放槽必须走失败恢复。
            workerResult = S_FALSE;
            goto Exit;
        }
    }

    // An initialized host owns this key, including failures. Only profiles
    // without a host session may use the legacy processor below.
    if (auto *host = pCompositionProcessorEngine->GetHostEngineAdapter(); host && host->valid())
    {
        std::string raw, error;
        msime::tsf::EngineResult result;
        const int scheme = Global::InputModeScheme.load(std::memory_order_relaxed);
        if (composingBeforeKey && msime::windows::scheme::AlwaysInlinePreedit(scheme) && _IsComposing())
        {
            // The composition still shows a syllable the host has already let go of: focus moved, or an edit session that would have ended the composition was refused. That syllable is text now, so end the composition around it and let this letter start the next one after it.
            std::string viewRaw, viewError;
            msime::tsf::EngineResult current;
            if (host->view(&viewRaw, &viewError) &&
                msime::tsf::EngineSessionAdapter::parse_result(viewRaw, &current, &viewError) &&
                current.view.editing_text.empty())
                _HandleCompleteCommitFirst(ec, pContext);
        }
        const bool shift = !groupedPanelKey && (GetKeyState(VK_SHIFT) & 0x8000) != 0;
        if (wch > 0x7f)
            workerResult = S_FALSE;
        else if (!host->panel_character(static_cast<uint8_t>(wch), shift, groupedPanelKey, &raw, &error) ||
                 !msime::tsf::EngineSessionAdapter::parse_result(raw, &result, &error))
            workerResult = E_FAIL;
        else
        {
            // Esc 锁定原文后，藏文组字时的空格只上屏原文并把空格交回宿主（未处理）。这个键已经被 TIP 吃掉，不会再到达应用，所以由 TIP 把空格写在原文后面。
            if (scheme == msime::windows::scheme::Tibetan && result.has_commit && !result.handled &&
                msime::tsf::is_host_text_key(wch))
            {
                result.commit.push_back(static_cast<char>(wch));
                result.handled = true;
            }
            workerResult = _ApplyHostCompositionResult(ec, pContext, result);
        }
        tfSelection.range->Release();
        return workerResult;
    }

    // Add virtual key to composition processor engine
    previousLength = pCompositionProcessorEngine->GetVirtualKeyLength();
    if (pCompositionProcessorEngine->AddVirtualKey(wch) &&
        pCompositionProcessorEngine->GetVirtualKeyLength() > previousLength)
    {
        g_toggleImeFallbackBuffer.push_back(wch);
    }

    workerResult = _HandleCompositionInputWorker(pCompositionProcessorEngine, ec, pContext, requestId);

    DebugTsfIssue47(L"composition-input-complete", requestId, 0, wch, CATEGORY_COMPOSING, FUNCTION_INPUT, 1,
                    _IsComposing(), pCompositionProcessorEngine->GetVirtualKeyLength(), workerResult,
                    static_cast<uint64_t>(previousLength));

Exit:
    tfSelection.range->Release();
    return workerResult;
}

HRESULT CMetasequoiaIME::_ApplyHostCompositionResult(TfEditCookie ec, ITfContext *pContext,
                                                       const msime::tsf::EngineResult &result, bool serverCommit)
{
    // 在线五笔先推进会话，文档等待同一回放 token 的 Server 最终文本。
    if (!serverCommit && _wubiCommitOwner.observe(result)) return S_OK;
    HRESULT outcome = S_OK;
    const auto status = msime::tsf::ApplyHostCharacterResult(result, [&](const std::string &text) {
        if (text.size() > static_cast<size_t>((std::numeric_limits<int>::max)())) return false;
        const int length = MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, text.data(),
                                               static_cast<int>(text.size()), nullptr, 0);
        if (length <= 0) return false;
        std::wstring commit(static_cast<size_t>(length), L'\0');
        if (MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, text.data(),
                               static_cast<int>(text.size()), commit.data(), length) != length) return false;
        CStringRange range;
        range.Set(commit.c_str(), commit.size());
        outcome = _AddCharAndFinalize(ec, pContext, &range);
        return outcome == S_OK;
    }, [&] {
        GlobalIme::word_for_creating_word.clear();
        GlobalIme::pending_create_word_preedit.clear();
        // 无提交的删空必须移除旧组字，直接结束会把最后一个拼写或词段留在文档里。
        outcome = result.has_commit ? _HandleCompleteCommitFirst(ec, pContext)
                                    : _HandleCancel(ec, pContext);
        return outcome == S_OK;
    }, [&] {
        if (!_IsComposing()) _StartComposition(pContext);
        if (!_IsComposing()) { outcome = E_FAIL; return false; }
        outcome = _HandleCompositionInputWorker(_pCompositionProcessorEngine, ec, pContext,
                                                FANY_IME_NO_REQUEST_ID);
        return outcome == S_OK;
    });
    if (status == msime::tsf::CharacterResultStatus::Unhandled) outcome = S_FALSE;
    else if (status == msime::tsf::CharacterResultStatus::Failed && SUCCEEDED(outcome))
        outcome = E_FAIL;
    return outcome;
}

//+---------------------------------------------------------------------------
//
// _HandleCompositionInputWorker
//
// If the keystroke happens within a composition, eat the key and return S_OK.
//
//----------------------------------------------------------------------------

HRESULT CMetasequoiaIME::_HandleCompositionInputWorker(_In_ CCompositionProcessorEngine *pCompositionProcessorEngine,
                                                       TfEditCookie ec, _In_ ITfContext *pContext, uint64_t requestId)
{
    HRESULT hr = S_OK;
    PerfTimer timer;
    CMetasequoiaImeArray<CStringRange> readingStrings;
    // CStringRange borrows its buffer; retain the host text through rendering.
    std::wstring hostPreedit;
    // 整句改字时改好的整句和光标（UTF-16 下标），不改字时为空。
    std::wstring hostConversion;
    size_t hostConversionCaret = 0;
    msime::tsf::KeyCompositionState composition;
    BOOL isWildcardIncluded = FALSE;

    //
    // Get reading string from composition processor engine
    //
    PerfTimer readingTimer;
    pCompositionProcessorEngine->GetReadingStrings(&readingStrings, &isWildcardIncluded);
    if (auto *host = pCompositionProcessorEngine->GetHostEngineAdapter(); host && host->valid())
    {
        std::string raw, error;
        msime::tsf::EngineResult result;
        if (host->view(&raw, &error) && msime::tsf::EngineSessionAdapter::parse_result(raw, &result, &error))
        {
            // 文字与光标来自同一次读取；宿主按键不会推进旧键击缓冲。
            composition = msime::tsf::KeyCompositionStateFromView(result.view, FanyUtils::string_to_wstring);
            // A Japanese composition is かな, not the letters that produced it: it is what the user
            // means, what the candidates are for, and what Enter commits. The one case that keeps
            // the letters is a caret the user moved into them, because the Engine's offset is an
            // offset into the romaji - see shared/input/CompositionDisplay.h.
            const auto &value = msime::input::composition_shows_reading(
                                    result.view.reading, result.view.caret,
                                    result.view.editing_text.size())
                                    ? result.view.reading
                                    : result.view.preedit;
            const int n = value.empty() ? 0 : MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS,
                                                                  value.data(), static_cast<int>(value.size()), nullptr, 0);
            hostPreedit.assign(static_cast<size_t>(n > 0 ? n : 0), L'\0');
            if (n > 0) MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, value.data(),
                                           static_cast<int>(value.size()), hostPreedit.data(), n);
            readingStrings.Clear();
            if (!hostPreedit.empty() || !result.view.phrase_prefix.empty())
            {
                auto *reading = readingStrings.Append();
                if (reading) reading->Set(hostPreedit.c_str(), hostPreedit.size());
            }
            // 旧全局只作为绘制镜像；选词、撤回与提交的持有者是当前 Runtime。
            const auto &prefix = result.view.phrase_prefix;
            const int prefixLength = prefix.empty() ? 0 : MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS,
                prefix.data(), static_cast<int>(prefix.size()), nullptr, 0);
            GlobalIme::word_for_creating_word.assign(static_cast<size_t>(prefixLength > 0 ? prefixLength : 0), L'\0');
            if (prefixLength > 0) MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, prefix.data(),
                static_cast<int>(prefix.size()), GlobalIme::word_for_creating_word.data(), prefixLength);
            const std::string &conversion = result.view.conversion;
            const auto toWide = [](const char *data, size_t size) {
                const int length = size == 0 ? 0 : MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, data,
                                                                        static_cast<int>(size), nullptr, 0);
                std::wstring wide(static_cast<size_t>(length > 0 ? length : 0), L'\0');
                if (length > 0) MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, data, static_cast<int>(size),
                                                    wide.data(), length);
                return wide;
            };
            if (!conversion.empty() && conversion.size() <= static_cast<size_t>((std::numeric_limits<int>::max)()))
            {
                hostConversion = toWide(conversion.data(), conversion.size());
                const size_t focusBytes =
                    msime::tsf::Utf8ScalarOffset(conversion, result.view.conversion_focus_start);
                hostConversionCaret = (std::min)(toWide(conversion.data(), focusBytes).size(), hostConversion.size());
            }
        }
        else
        {
            return E_FAIL;
        }
    }
    else
    {
        composition.rawInput = pCompositionProcessorEngine->GetKeystrokeBuffer().ToWString();
        composition.caret = pCompositionProcessorEngine->GetCaretPosition();
    }
    double readingElapsedMs = readingTimer.ElapsedMs();

    if (readingStrings.Count())
    {
    }

    std::wstring uiLessPreedit;
    FanyImeUiLessPage::Payload uiLessPage;
    bool gotUiLessComposition = false;

    /* 一般来说，readingStrings 数组中只有一个元素，这个元素就是当前输入的拼音 */
    double preeditPipeElapsedMs = 0;
    double addComposingElapsedMs = 0;

    // UILess hosts need a synchronous candidate page before UpdateUIElement.
    if (Global::IsUiLessMode() && requestId != FANY_IME_NO_REQUEST_ID)
    {
        PerfTimer preeditPipeTimer;
        struct FanyImeNamedpipeDataToTsf *receivedData =
            TryReadDataFromServerPipeWithTimeout(requestId, /*abortTransportOnTimeout=*/false);
        preeditPipeElapsedMs += preeditPipeTimer.ElapsedMs();
        if (receivedData->msg_type == Global::DataFromServerMsgType::TransportUnavailable)
        {
            return HRESULT_FROM_WIN32(ERROR_BROKEN_PIPE);
        }
        if (receivedData->msg_type == Global::DataFromServerMsgType::UiLessComposition)
        {
            uiLessPage = FanyImeUiLessPage::Parse(receivedData->candidate_string);
            uiLessPreedit = uiLessPage.preedit;
            gotUiLessComposition = true;
        }
        else if (receivedData->msg_type == Global::DataFromServerMsgType::Preedit)
        {
            uiLessPreedit.assign(receivedData->candidate_string, wcslen(receivedData->candidate_string));
            gotUiLessComposition = true;
        }
    }

    for (UINT index = 0; index < readingStrings.Count(); index++)
    {
        CStringRange curReadingStr;
        std::wstring readingStr = readingStrings.GetAt(0)->ToWString();
        // Korean, Zhuyin and Vietnamese always mark their composition inline: it is the text the user is writing, not a reading, and until a list is opened there is no candidate window to show it in (scheme::AlwaysInlinePreedit).
        const std::string_view preeditStyle =
            msime::windows::scheme::AlwaysInlinePreedit(Global::InputModeScheme.load(std::memory_order_relaxed)) &&
                    GlobalSettings::getTsfPreeditStyle() == GlobalSettings::TsfPreeditStyle::Empty
                ? GlobalSettings::TsfPreeditStyle::Raw
                : std::string_view(GlobalSettings::getTsfPreeditStyle());

        if (preeditStyle == GlobalSettings::TsfPreeditStyle::Empty)
        {
            // Inline preedit hidden; composition/candidates still run as usual.
            GlobalIme::pending_create_word_preedit.clear();
            readingStr.clear();
            curReadingStr.Set(readingStr.c_str(), readingStr.length());
        }
        else if (preeditStyle == GlobalSettings::TsfPreeditStyle::Pinyin)
        {
            bool gotServerPreedit = false;
            if (!GlobalIme::pending_create_word_preedit.empty())
            {
                readingStr = std::move(GlobalIme::pending_create_word_preedit);
                GlobalIme::pending_create_word_preedit.clear();
                gotServerPreedit = true;
            }
            else if (gotUiLessComposition)
            {
                readingStr = uiLessPreedit;
                gotServerPreedit = true;
            }
            else if (requestId != FANY_IME_NO_REQUEST_ID)
            {
                PerfTimer preeditPipeTimer;
                struct FanyImeNamedpipeDataToTsf *receivedData =
                    TryReadDataFromServerPipeWithTimeout(requestId, /*abortTransportOnTimeout=*/false);
                preeditPipeElapsedMs += preeditPipeTimer.ElapsedMs();
                if (receivedData->msg_type == Global::DataFromServerMsgType::TransportUnavailable)
                {
                    return HRESULT_FROM_WIN32(ERROR_BROKEN_PIPE);
                }
                if (receivedData->msg_type == Global::DataFromServerMsgType::Preedit)
                {
                    readingStr.assign(receivedData->candidate_string, wcslen(receivedData->candidate_string));
                    gotServerPreedit = true;
                }
            }

            if (!gotServerPreedit && !GlobalIme::word_for_creating_word.empty())
            {
                // Fallback when Preedit is missing: keep 汉字 + remaining raw,
                // matching raw create-word structure until the next Preedit.
                readingStr = GlobalIme::word_for_creating_word + readingStr;
            }
            curReadingStr.Set(readingStr.c_str(), readingStr.length());
        }
        else
        {
            // raw (default)
            GlobalIme::pending_create_word_preedit.clear();
            if (!GlobalIme::word_for_creating_word.empty())
            { /* 造词过程中 */
                readingStr = GlobalIme::word_for_creating_word + readingStr;
            }
            curReadingStr.Set(readingStr.c_str(), readingStr.length());
        }

        // 整句改字：行内画已选前缀加改好的整句，不管预编辑样式选的是什么，光标在焦点字前。
        if (!hostConversion.empty())
        {
            readingStr = GlobalIme::word_for_creating_word + hostConversion;
            curReadingStr.Set(readingStr.c_str(), readingStr.length());
        }
        const size_t preeditPrefixLength =
            preeditStyle == GlobalSettings::TsfPreeditStyle::Empty && hostConversion.empty()
                ? 0
                : GlobalIme::word_for_creating_word.size();
        // A Korean syllable, a Zhuyin conversion and a Vietnamese word have no caret inside them: the Engine ignores caret moves there, so the caret always follows the last key (scheme::LocksCaret).
        const DWORD_PTR displayCaret =
            !hostConversion.empty() ? preeditPrefixLength + hostConversionCaret
            : msime::windows::scheme::LocksCaret(Global::InputModeScheme.load(std::memory_order_relaxed))
                ? curReadingStr.GetLength()
                : msime::tsf::MapPreeditCaret(composition.rawInput, composition.caret,
                                             curReadingStr.ToWString(), preeditPrefixLength);
        pCompositionProcessorEngine->SetRenderedPreedit(curReadingStr.ToWString(), preeditPrefixLength);

        PerfTimer addComposingTimer;
        hr = _AddComposingAndChar(ec, pContext, &curReadingStr);
        addComposingElapsedMs += addComposingTimer.ElapsedMs();

        if (FAILED(hr))
        {
            return hr;
        }

        if (_pComposition)
        {
            ITfRange *caretRange = nullptr;
            if (SUCCEEDED(_pComposition->GetRange(&caretRange)) && caretRange)
            {
                // 文字已经写入；光标定位失败不把本次输入改为可重放的失败。
                (void)SetCompositionCaret(caretRange, pContext, ec, displayCaret);
                caretRange->Release();
            }
        }
    }

    //
    // Get candidate string from composition processor engine
    //
    CMetasequoiaImeArray<CCandidateListItem> candidateList;

    //
    // Important: Generate candidate list here
    //
    // There is no need to use neither IncrementalWordSearch nor WildcardSearch, so we set them both FALSE
    PerfTimer candidateListTimer;
    pCompositionProcessorEngine->GetCandidateList(&candidateList, FALSE, FALSE);
    double candidateListElapsedMs = candidateListTimer.ElapsedMs();

    double createCandidateElapsedMs = 0;
    double clearListElapsedMs = 0;
    double setTextElapsedMs = 0;
    if ((candidateList.Count()))
    {
        PerfTimer createCandidateTimer;
        hr = _CreateAndStartCandidate(pCompositionProcessorEngine, ec, pContext);
        createCandidateElapsedMs = createCandidateTimer.ElapsedMs();
        if (SUCCEEDED(hr))
        {
            if (gotUiLessComposition && _pCandidateListUIPresenter)
            {
                _pCandidateListUIPresenter->_ApplyUiLessPayload(uiLessPage);
            }
            PerfTimer clearListTimer;
            if (!gotUiLessComposition || !uiLessPage.hasPage)
            {
                _pCandidateListUIPresenter->_ClearList();
            }
            clearListElapsedMs = clearListTimer.ElapsedMs();
            PerfTimer setTextTimer;
            _pCandidateListUIPresenter->_SetText(&candidateList, TRUE);
            for (UINT i = 0; !Global::IsUiLessMode() && i < candidateList.Count(); ++i)
            {
                if (const auto *item = candidateList.GetAt(i); item && item->_EngineHighlighted)
                {
                    _pCandidateListUIPresenter->_SetSelection(static_cast<int>(i));
                    break;
                }
            }
            setTextElapsedMs = setTextTimer.ElapsedMs();
        }
    }
    else if (_pCandidateListUIPresenter &&
             msime::windows::scheme::OpensCandidateList(Global::InputModeScheme.load(std::memory_order_relaxed)))
    {
        // A letter typed into an open Hanja or Zhuyin list closed it and keeps composing: the presenter goes with the list, quietly (see _DeleteCandidateList).
        _DeleteCandidateList(FALSE, pContext);
    }
    else if (_pCandidateListUIPresenter)
    {
        if (gotUiLessComposition)
        {
            if (!uiLessPage.hasPage)
                _pCandidateListUIPresenter->_ClearList();
            _pCandidateListUIPresenter->_ApplyUiLessPayload(uiLessPage);
            _pCandidateListUIPresenter->_NotifyUiLessHost();
        }
        else
        {
            PerfTimer clearListTimer;
            _pCandidateListUIPresenter->_ClearList();
            clearListElapsedMs = clearListTimer.ElapsedMs();
        }
    }
    else if (readingStrings.Count() && isWildcardIncluded)
    {
        PerfTimer createCandidateTimer;
        hr = _CreateAndStartCandidate(pCompositionProcessorEngine, ec, pContext);
        createCandidateElapsedMs = createCandidateTimer.ElapsedMs();
        if (SUCCEEDED(hr))
        {
            PerfTimer clearListTimer;
            _pCandidateListUIPresenter->_ClearList();
            clearListElapsedMs = clearListTimer.ElapsedMs();
        }
    }
    return hr;
}
//+---------------------------------------------------------------------------
//
// _CreateAndStartCandidate
//
//----------------------------------------------------------------------------

HRESULT CMetasequoiaIME::_CreateAndStartCandidate(_In_ CCompositionProcessorEngine *pCompositionProcessorEngine,
                                                  TfEditCookie ec, _In_ ITfContext *pContext)
{
    HRESULT hr = S_OK;
    PerfTimer timer;
    double recreateElapsedMs = 0;

    if ((_candidateMode == CANDIDATE_NONE) && (_pCandidateListUIPresenter))
    {
        // Recreate candidate list — dtor handles _EndCandidateList()
        PerfTimer recreateTimer;
        delete _pCandidateListUIPresenter;
        _pCandidateListUIPresenter = nullptr;

        _candidateMode = CANDIDATE_NONE;
        _isCandidateWithWildcard = FALSE;
        recreateElapsedMs = recreateTimer.ElapsedMs();
    }

    double allocElapsedMs = 0;
    double getDocMgrElapsedMs = 0;
    double getRangeElapsedMs = 0;
    double startCandidateListElapsedMs = 0;
    if (_pCandidateListUIPresenter == nullptr)
    {
        PerfTimer allocTimer;
        _pCandidateListUIPresenter = new (std::nothrow) CCandidateListUIPresenter(
            this, CATEGORY_CANDIDATE, pCompositionProcessorEngine->GetCandidateListIndexRange(), FALSE);
        allocElapsedMs = allocTimer.ElapsedMs();
        if (!_pCandidateListUIPresenter)
        {
            return E_OUTOFMEMORY;
        }

        _candidateMode = CANDIDATE_INCREMENTAL;
        _isCandidateWithWildcard = FALSE;

        // we don't cache the document manager object. So get it from pContext.
        ITfDocumentMgr *pDocumentMgr = nullptr;
        PerfTimer getDocMgrTimer;
        if (SUCCEEDED(pContext->GetDocumentMgr(&pDocumentMgr)))
        {
            getDocMgrElapsedMs = getDocMgrTimer.ElapsedMs();
            // get the composition range.
            ITfRange *pRange = nullptr;
            PerfTimer getRangeTimer;
            if (SUCCEEDED(_pComposition->GetRange(&pRange)))
            {
                getRangeElapsedMs = getRangeTimer.ElapsedMs();
                PerfTimer startCandidateListTimer;
                hr = _pCandidateListUIPresenter->_StartCandidateList(
                    _tfClientId, pDocumentMgr, pContext, ec, pRange,
                    pCompositionProcessorEngine->GetCandidateWindowWidth());
                startCandidateListElapsedMs = startCandidateListTimer.ElapsedMs();
                pRange->Release();
            }
            pDocumentMgr->Release();
        }
    }

    return hr;
}

//+---------------------------------------------------------------------------
//
// _HandleCompositionFinalize
//
//----------------------------------------------------------------------------

HRESULT CMetasequoiaIME::_HandleCompositionFinalize(TfEditCookie ec, _In_ ITfContext *pContext, BOOL isCandidateList)
{
    HRESULT hr = S_OK;
    PerfTimer timer;
    double finalizeCandidateElapsedMs = 0;

    if (isCandidateList && _pCandidateListUIPresenter)
    {
        // Finalize selected candidate string from CCandidateListUIPresenter
        DWORD_PTR candidateLen = 0;
        const WCHAR *pCandidateString = nullptr;

        candidateLen = _pCandidateListUIPresenter->_GetSelectedCandidateString(&pCandidateString);

        CStringRange candidateString;
        candidateString.Set(pCandidateString, candidateLen);

        if (candidateLen)
        {
            // Finalize character
            PerfTimer finalizeCandidateTimer;
            hr = _AddCharAndFinalize(ec, pContext, &candidateString);
            finalizeCandidateElapsedMs = finalizeCandidateTimer.ElapsedMs();
            if (FAILED(hr))
            {
                return hr;
            }
        }
    }
    // For the non-candidate path, the current composition text is already in
    // the text store. _HandleCancel below owns the exact write cookie and
    // terminates it synchronously; requesting a nested edit session here can
    // legitimately fail with TF_E_SYNCHRONOUS and is redundant.

    PerfTimer cancelTimer;
    _HandleCancel(ec, pContext);
    double cancelElapsedMs = cancelTimer.ElapsedMs();

    return S_OK;
}

//+---------------------------------------------------------------------------
//
// _HandleCompositionConvert
//
//----------------------------------------------------------------------------

HRESULT CMetasequoiaIME::_HandleCompositionConvert(TfEditCookie ec, _In_ ITfContext *pContext, BOOL isWildcardSearch)
{
    HRESULT hr = S_OK;
    PerfTimer timer;

    CMetasequoiaImeArray<CCandidateListItem> candidateList;

    //
    // Get candidate string from composition processor engine
    //
    CCompositionProcessorEngine *pCompositionProcessorEngine = nullptr;
    pCompositionProcessorEngine = _pCompositionProcessorEngine;
    PerfTimer getCandidateListTimer;
    pCompositionProcessorEngine->GetCandidateList(&candidateList, FALSE, isWildcardSearch);
    double getCandidateListElapsedMs = getCandidateListTimer.ElapsedMs();

    // If there is no candlidate listin the current reading string, we don't do anything. Just wait for
    // next char to be ready for the conversion with it.
    int nCount = candidateList.Count();
    double rebuildPresenterElapsedMs = 0;
    double allocPresenterElapsedMs = 0;
    double startCandidateListElapsedMs = 0;
    double setTextElapsedMs = 0;
    if (nCount)
    {
        if (_pCandidateListUIPresenter)
        {
            PerfTimer rebuildPresenterTimer;
            delete _pCandidateListUIPresenter; // dtor handles _EndCandidateList()
            _pCandidateListUIPresenter = nullptr;

            _candidateMode = CANDIDATE_NONE;
            _isCandidateWithWildcard = FALSE;
            rebuildPresenterElapsedMs = rebuildPresenterTimer.ElapsedMs();
        }

        //
        // create an instance of the candidate list class.
        //
        if (_pCandidateListUIPresenter == nullptr)
        {
            PerfTimer allocPresenterTimer;
            _pCandidateListUIPresenter = new (std::nothrow) CCandidateListUIPresenter(
                this, CATEGORY_CANDIDATE, pCompositionProcessorEngine->GetCandidateListIndexRange(), FALSE);
            allocPresenterElapsedMs = allocPresenterTimer.ElapsedMs();
            if (!_pCandidateListUIPresenter)
            {
                return E_OUTOFMEMORY;
            }

            _candidateMode = CANDIDATE_ORIGINAL;
        }

        _isCandidateWithWildcard = isWildcardSearch;

        // we don't cache the document manager object. So get it from pContext.
        ITfDocumentMgr *pDocumentMgr = nullptr;
        if (SUCCEEDED(pContext->GetDocumentMgr(&pDocumentMgr)))
        {
            // get the composition range.
            ITfRange *pRange = nullptr;
            if (SUCCEEDED(_pComposition->GetRange(&pRange)))
            {
                PerfTimer startCandidateListTimer;
                hr = _pCandidateListUIPresenter->_StartCandidateList(
                    _tfClientId, pDocumentMgr, pContext, ec, pRange,
                    pCompositionProcessorEngine->GetCandidateWindowWidth());
                startCandidateListElapsedMs = startCandidateListTimer.ElapsedMs();
                pRange->Release();
            }
            pDocumentMgr->Release();
        }
        if (SUCCEEDED(hr))
        {
            PerfTimer setTextTimer;
            _pCandidateListUIPresenter->_SetText(&candidateList, FALSE);
            setTextElapsedMs = setTextTimer.ElapsedMs();
        }
    }

    return hr;
}

//+---------------------------------------------------------------------------
//
// _HandleCompositionBackspace
//
//----------------------------------------------------------------------------

HRESULT CMetasequoiaIME::_HandleCompositionBackspace(TfEditCookie ec, _In_ ITfContext *pContext, uint64_t requestId)
{
    HRESULT workerResult = S_OK;
    ITfRange *pRangeComposition = nullptr;
    TF_SELECTION tfSelection;
    ULONG fetched = 0;
    BOOL isCovered = TRUE;
    CCompositionProcessorEngine *pCompositionProcessorEngine = nullptr;
    DWORD_PTR vKeyLen = 0;

    // Start the new (std::nothrow) compositon if there is no composition.
    if (!_IsComposing())
    {
        return S_OK;
    }

    // first, test where a keystroke would go in the document if we did an insert
    if (FAILED(pContext->GetSelection(ec, TF_DEFAULT_SELECTION, 1, &tfSelection, &fetched)) || fetched != 1)
    {
        return S_FALSE;
    }

    // is the insertion point covered by a composition?
    if (SUCCEEDED(_pComposition->GetRange(&pRangeComposition)))
    {
        isCovered = _IsRangeCovered(ec, tfSelection.range, pRangeComposition);

        pRangeComposition->Release();

        if (!isCovered)
        {
            goto Exit;
        }
    }

    //
    // Add virtual key to composition processor engine
    //
    pCompositionProcessorEngine = _pCompositionProcessorEngine;

    if (auto *host = pCompositionProcessorEngine->GetHostEngineAdapter(); host && host->valid())
    {
        std::string raw, error;
        if (host->command(MSIME_BACKSPACE, &raw, &error))
        {
            msime::tsf::EngineResult result;
            if (msime::tsf::EngineSessionAdapter::parse_result(raw, &result, &error) && result.handled)
            {
                // 删空拼写可能提交已选词：先替换组字再结束，不能把上一帧最后一个字母留下。
                if (result.view.editing_text.empty() && result.view.phrase_prefix.empty())
                    _backspaceHoldArmed = true;
                tfSelection.range->Release();
                return _ApplyHostCompositionResult(ec, pContext, result);
            }
        }
    }

    vKeyLen = pCompositionProcessorEngine->GetVirtualKeyLength();

    if (!g_toggleImeFallbackBuffer.empty())
    {
        g_toggleImeFallbackBuffer.pop_back();
    }

    if (vKeyLen)
    {
        pCompositionProcessorEngine->RemoveVirtualKeyBeforeCaret();

        if (pCompositionProcessorEngine->GetVirtualKeyLength())
        {
            workerResult = _HandleCompositionInputWorker(pCompositionProcessorEngine, ec, pContext, requestId);
        }
        else
        {
            _backspaceHoldArmed = true;
            _HandleCancel(ec, pContext);
        }
    }

Exit:
    tfSelection.range->Release();
    return workerResult;
}

HRESULT CMetasequoiaIME::_HandleCompositionSegmentEdit(TfEditCookie ec, _In_ ITfContext *pContext,
                                                       KEYSTROKE_FUNCTION keyFunction, uint64_t requestId)
{
    const bool isBackspace = keyFunction == FUNCTION_BACKSPACE_SEGMENT;
    const uint32_t command = isBackspace
                                 ? MSIME_BACKSPACE_SEGMENT
                                 : (keyFunction == FUNCTION_MOVE_LEFT_SEGMENT ? MSIME_MOVE_LEFT_SEGMENT
                                                                               : MSIME_MOVE_RIGHT_SEGMENT);
    // 宿主会话没接分段命令时退回逐个字母移光标（`letterCaret`），不进整句改字：Shift/Ctrl+左右从不变成改字命令或移动候选。
    auto fallback = [&]() -> HRESULT {
        if (isBackspace)
            return _HandleCompositionBackspace(ec, pContext, requestId);
        return _HandleCompositionArrowKey(ec, pContext,
                                          keyFunction == FUNCTION_MOVE_LEFT_SEGMENT ? FUNCTION_MOVE_LEFT
                                                                                     : FUNCTION_MOVE_RIGHT,
                                          requestId, true);
    };
    if (!_IsComposing())
        return S_OK;
    auto *host = _pCompositionProcessorEngine->GetHostEngineAdapter();
    if (!host || !host->valid())
        return fallback();
    // Shift/Ctrl+左右在所有方案里都发分段命令，全拼、双拼也是（Server 同样原样发出，见 sentence_edit_command）：偏好 `navigation.left_right` 打开时共享 runtime 只把不带修饰键的左右键换成移动候选，分段命令从不被换，所以高亮在哪个候选上它们都按音节编辑拼音。
    msime::tsf::EngineResult before;
    const bool viewed = !isBackspace && msime::tsf::HostView(*host, &before);

    std::string raw, error;
    msime::tsf::EngineResult result;
    if (!host->command(command, &raw, &error) || !msime::tsf::EngineSessionAdapter::parse_result(raw, &result, &error) ||
        !result.handled)
        return fallback();

    if (isBackspace)
    {
        // 包括无剩余拼写、只有暂存词段的状态，历史由 Runtime 回退，不在 TIP 重放。
        return _ApplyHostCompositionResult(ec, pContext, result);
    }

    // 整句改字时分段命令先退出改字再移光标，预编辑从改好的句子换回拼音、候选也换了，要按引擎视图整个重画；高亮换了候选时同样重画（与左右键的 _HandleCompositionArrowKey 同一条件）。否则只有拼音光标动了，只移预编辑里的光标。
    if ((viewed && msime::tsf::HostCompositionNeedsRedraw(before.view, result.view)) || !result.view.conversion.empty())
        return _HandleCompositionInputWorker(_pCompositionProcessorEngine, ec, pContext, FANY_IME_NO_REQUEST_ID);
    // 光标对不上已画的预编辑时按引擎视图整个重画，不能再走 fallback：分段命令已经在宿主会话里移过光标，fallback 会再发一次字母光标命令，光标多移一格，高亮不在首选时还会被共享 runtime 换成移动候选。
    const HRESULT moved = _MoveCompositionCaretToHost(ec, pContext, result);
    return moved == S_FALSE
               ? _HandleCompositionInputWorker(_pCompositionProcessorEngine, ec, pContext, FANY_IME_NO_REQUEST_ID)
               : moved;
}

HRESULT CMetasequoiaIME::_MoveCompositionCaretToHost(TfEditCookie ec, _In_ ITfContext *pContext,
                                                     const msime::tsf::EngineResult &result)
{
    // The runtime caret is a byte offset in ASCII editing_text, not a preedit prefix length. Map it against the text already rendered.
    for (unsigned char byte : result.view.editing_text)
        if (byte > 0x7f)
            return S_FALSE;
    const std::wstring editing(result.view.editing_text.begin(), result.view.editing_text.end());
    const DWORD_PTR displayCaret = _pCompositionProcessorEngine->GetRenderedCaretPosition(editing, result.view.caret);
    if (_pComposition == nullptr)
        return S_OK;
    ITfRange *caretRange = nullptr;
    if (FAILED(_pComposition->GetRange(&caretRange)) || caretRange == nullptr)
        return S_OK;
    const HRESULT hr = SetCompositionCaret(caretRange, pContext, ec, displayCaret);
    caretRange->Release();
    return SUCCEEDED(hr) ? S_OK : hr;
}

bool CMetasequoiaIME::_HandleHostHomeKey(TfEditCookie ec, _In_ ITfContext *pContext, uint64_t requestId,
                                         _Out_ HRESULT *outcome)
{
    *outcome = S_OK;
    auto *host = _pCompositionProcessorEngine ? _pCompositionProcessorEngine->GetHostEngineAdapter() : nullptr;
    if (!host || !host->valid() || !_IsComposing())
        return false;
    msime::tsf::EngineResult before;
    if (!msime::tsf::HostView(*host, &before))
        return false;
    std::string raw, error;
    msime::tsf::EngineResult result;
    if (!host->command(msime::tsf::HostHomeCommand(before.view), &raw, &error) ||
        !msime::tsf::EngineSessionAdapter::parse_result(raw, &result, &error) || !result.handled)
        return false;
    // 第一下高亮回到首选，候选窗要按引擎视图重画；已经在首选时第二下只把光标移到输入串开头，高亮和候选都不变，只移预编辑里的光标，和左右键一样。
    if (!msime::tsf::HostCompositionNeedsRedraw(before.view, result.view) && before.view.caret != result.view.caret)
    {
        *outcome = _MoveCompositionCaretToHost(ec, pContext, result);
        if (*outcome != S_FALSE)
            return true;
    }
    *outcome = _HandleCompositionInputWorker(_pCompositionProcessorEngine, ec, pContext, requestId);
    return true;
}

//+---------------------------------------------------------------------------
//
// _HandleCompositionDelete
//
//----------------------------------------------------------------------------

HRESULT CMetasequoiaIME::_HandleCompositionDelete(TfEditCookie ec, _In_ ITfContext *pContext, uint64_t requestId)
{
    HRESULT workerResult = S_OK;
    ITfRange *pRangeComposition = nullptr;
    TF_SELECTION tfSelection = {};
    ULONG fetched = 0;

    if (!_IsComposing())
    {
        return S_OK;
    }

    if (FAILED(pContext->GetSelection(ec, TF_DEFAULT_SELECTION, 1, &tfSelection, &fetched)) || fetched != 1)
    {
        return S_FALSE;
    }

    BOOL isCovered = TRUE;
    if (SUCCEEDED(_pComposition->GetRange(&pRangeComposition)))
    {
        isCovered = _IsRangeCovered(ec, tfSelection.range, pRangeComposition);
        pRangeComposition->Release();
    }

    if (isCovered)
    {
        CCompositionProcessorEngine *pCompositionProcessorEngine = _pCompositionProcessorEngine;
        if (auto *host = pCompositionProcessorEngine->GetHostEngineAdapter(); host && host->valid())
        {
            std::string raw, error;
            if (host->command(MSIME_DELETE_FORWARD, &raw, &error))
            {
                msime::tsf::EngineResult result;
                if (msime::tsf::EngineSessionAdapter::parse_result(raw, &result, &error) && result.handled)
                {
                    const auto &value = result.view.preedit;
                    const int n = value.empty() ? 0 : MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS,
                                                                          value.data(), static_cast<int>(value.size()), nullptr, 0);
                    std::wstring preedit(static_cast<size_t>(n > 0 ? n : 0), L'\0');
                    if (n > 0) MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, value.data(),
                                                   static_cast<int>(value.size()), preedit.data(), n);
                    tfSelection.range->Release();
                    if (preedit.empty()) return _HandleCancel(ec, pContext);
                    CStringRange rendered;
                    rendered.Set(preedit.c_str(), preedit.size());
                    return _AddComposingAndChar(ec, pContext, &rendered);
                }
            }
        }
        const DWORD_PTR caret = pCompositionProcessorEngine->GetCaretPosition();
        const BOOL removed = pCompositionProcessorEngine->RemoveVirtualKeyAtCaret();

        if (removed && caret < g_toggleImeFallbackBuffer.size())
        {
            g_toggleImeFallbackBuffer.erase(static_cast<size_t>(caret), 1);
        }

        if (removed)
        {
            if (pCompositionProcessorEngine->GetVirtualKeyLength())
            {
                workerResult = _HandleCompositionInputWorker(pCompositionProcessorEngine, ec, pContext, requestId);
            }
            else
            {
                _HandleCancel(ec, pContext);
            }
        }
    }

    tfSelection.range->Release();
    return workerResult;
}

//+---------------------------------------------------------------------------
//
// _HandleCompositionArrowKey
//
// Update the selection within a composition.
//
//----------------------------------------------------------------------------

HRESULT CMetasequoiaIME::_HandleCompositionArrowKey(TfEditCookie ec, _In_ ITfContext *pContext,
                                                    KEYSTROKE_FUNCTION keyFunction, uint64_t requestId,
                                                    bool letterCaret)
{
    if (keyFunction == FUNCTION_MOVE_LEFT || keyFunction == FUNCTION_MOVE_RIGHT)
    {
        DWORD_PTR displayCaret = 0;
        bool usedHost = false;
        if (auto *host = _pCompositionProcessorEngine->GetHostEngineAdapter(); host && host->valid())
        {
            std::string raw, error;
            // 全拼和双拼的左右键交给整句改字（引擎进不了改字时按字母移光标）。改字前后预编辑在拼音和汉字之间换、候选也换成光标处那一段的，所以只要改字在进行或刚结束，就按引擎视图整个重画，而不是只移选区。
            msime::tsf::EngineResult before;
            const bool viewed = msime::tsf::HostView(*host, &before);
            const bool sentence = !letterCaret && viewed && msime::tsf::HostEditsSentence(before.view);
            const uint32_t command = keyFunction == FUNCTION_MOVE_LEFT
                                         ? (sentence ? MSIME_CONVERSION_LEFT : MSIME_MOVE_LEFT)
                                         : (sentence ? MSIME_CONVERSION_RIGHT : MSIME_MOVE_RIGHT);
            msime::tsf::EngineResult result;
            if (!host->command(command, &raw, &error) ||
                !msime::tsf::EngineSessionAdapter::parse_result(raw, &result, &error)) return E_FAIL;
            if (!result.handled) return S_OK;
            // 横排候选窗里，高亮不在首选、或者首选上 → 已经到了输入串末尾时，共享 runtime 会把这次左右键换成移动候选高亮（偏好 `navigation.left_right`，竖排时不换）。那种情况下预编辑一个字都没变，只有高亮动了，所以这里也要重画候选窗；下面的 SetSelection 只移动预编辑里的光标。
            if ((viewed && msime::tsf::HostCompositionNeedsRedraw(before.view, result.view)) ||
                !result.view.conversion.empty())
                return _HandleCompositionInputWorker(_pCompositionProcessorEngine, ec, pContext,
                                                     FANY_IME_NO_REQUEST_ID);
            // The runtime caret is a byte offset in ASCII editing_text, not a
            // preedit prefix length. Map it against the text already rendered.
            for (unsigned char byte : result.view.editing_text)
                if (byte > 0x7f) return E_FAIL;
            const std::wstring editing(result.view.editing_text.begin(), result.view.editing_text.end());
            displayCaret = _pCompositionProcessorEngine->GetRenderedCaretPosition(editing, result.view.caret);
            usedHost = true;
        }
        else
        {
            _pCompositionProcessorEngine->MoveCaret(keyFunction == FUNCTION_MOVE_LEFT ? -1 : 1);
            displayCaret = _pCompositionProcessorEngine->GetRenderedCaretPosition();
        }
        if (_pComposition == nullptr)
        {
            return S_OK;
        }

        ITfRange *caretRange = nullptr;
        if (FAILED(_pComposition->GetRange(&caretRange)) || caretRange == nullptr)
        {
            return S_OK;
        }
        (void)SetCompositionCaret(caretRange, pContext, ec, displayCaret);
        caretRange->Release();
        if (!usedHost && Global::IsUiLessMode() && _pCandidateListUIPresenter)
        {
            _pCandidateListUIPresenter->_ConsumeUiLessCompositionReply(requestId);
        }
        return S_OK;
    }

    if (Global::IsUiLessMode())
    {
        auto *host = _pCompositionProcessorEngine ? _pCompositionProcessorEngine->GetHostEngineAdapter() : nullptr;
        if (host && host->valid())
        {
            uint32_t command = 0;
            switch (keyFunction) {
            case FUNCTION_MOVE_PAGE_UP: command = MSIME_PREVIOUS_PAGE; break;
            case FUNCTION_MOVE_PAGE_DOWN: command = MSIME_NEXT_PAGE; break;
            case FUNCTION_MOVE_PAGE_TOP: command = MSIME_FIRST_CANDIDATE_OR_HOME; break;
            case FUNCTION_MOVE_PAGE_BOTTOM: command = MSIME_LAST_CANDIDATE; break;
            }
            if (command)
            {
                if (_pCandidateListUIPresenter)
                    _pCandidateListUIPresenter->_ConsumeUiLessCompositionReply(requestId);
                // Home 键分两段，命令和重画由 _HandleHostHomeKey 决定。
                if (command == MSIME_FIRST_CANDIDATE_OR_HOME)
                {
                    HRESULT home = S_OK;
                    return _HandleHostHomeKey(ec, pContext, FANY_IME_NO_REQUEST_ID, &home) ? home : S_FALSE;
                }
                return _HandleHostCandidateNavigation(ec, pContext, command);
            }
        }
    }

    if ((keyFunction == FUNCTION_MOVE_PAGE_UP) || (keyFunction == FUNCTION_MOVE_PAGE_DOWN) ||
        (keyFunction == FUNCTION_MOVE_PAGE_TOP) || (keyFunction == FUNCTION_MOVE_PAGE_BOTTOM))
    {
        if ((_pCandidateListUIPresenter == nullptr) || (_pCandidateListUIPresenter->_GetCount() <= 1))
        {
            if (Global::IsUiLessMode() && _pCandidateListUIPresenter)
            {
                _pCandidateListUIPresenter->_ConsumeUiLessCompositionReply(requestId);
            }
            return S_OK;
        }
    }

    ITfRange *pRangeComposition = nullptr;
    TF_SELECTION tfSelection;
    ULONG fetched = 0;

    // get the selection
    if (FAILED(pContext->GetSelection(ec, TF_DEFAULT_SELECTION, 1, &tfSelection, &fetched)) || fetched != 1)
    {
        // no selection, eat the keystroke
        return S_OK;
    }

    // get the composition range
    if ((_pComposition == nullptr) || FAILED(_pComposition->GetRange(&pRangeComposition)))
    {
        goto Exit;
    }

    // For incremental candidate list
    if (_pCandidateListUIPresenter)
    {
        if (Global::IsUiLessMode())
        {
            _pCandidateListUIPresenter->_ConsumeUiLessCompositionReply(requestId);
        }
        else
        {
            _pCandidateListUIPresenter->AdviseUIChangedByArrowKey(keyFunction);
        }
    }

    pContext->SetSelection(ec, 1, &tfSelection);

    pRangeComposition->Release();

Exit:
    tfSelection.range->Release();
    return S_OK;
}

//+---------------------------------------------------------------------------
//
// _HandleCompositionPunctuation
// 处理标点的上屏：
//   1. 没有候选词的情况下，纯标点的上屏
//   2. 有候选词的情况下，候选词和标点的一并上屏
//
// 标点这里不会触发造词行为。
//
//----------------------------------------------------------------------------

HRESULT CMetasequoiaIME::_HandleCompositionPunctuation(TfEditCookie ec, _In_ ITfContext *pContext, UINT code, WCHAR wch,
                                                       uint64_t requestId, const std::wstring &prefetchedText)
{
    HRESULT hr = S_OK;
    PerfTimer timer;

    if (_QueueRepeatedSmartPunctuationReplacement(wch))
    {
        return S_OK;
    }

    //
    // Get punctuation char from composition processor engine
    //
    CCompositionProcessorEngine *pCompositionProcessorEngine = nullptr;
    pCompositionProcessorEngine = _pCompositionProcessorEngine;

    std::wstring pendingPunctuationCommitText = prefetchedText;
    const bool hasPendingPunctuationCommitText = !pendingPunctuationCommitText.empty();
    std::wstring punctuationStr;
    if (!hasPendingPunctuationCommitText)
    {
        if (auto *host = pCompositionProcessorEngine->GetHostEngineAdapter(); host && host->valid())
        {
            std::string raw, error;
            msime::tsf::EngineResult result;
            if (host->punctuation(static_cast<uint8_t>(wch & 0xff), &raw, &error) &&
                msime::tsf::EngineSessionAdapter::parse_result(raw, &result, &error) && result.handled &&
                result.has_commit && !result.commit.empty())
            {
                const int n = MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, result.commit.data(),
                                                  static_cast<int>(result.commit.size()), nullptr, 0);
                std::wstring commit(static_cast<size_t>(n > 0 ? n : 0), L'\0');
                if (n > 0) MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, result.commit.data(),
                                               static_cast<int>(result.commit.size()), commit.data(), n);
                CStringRange text;
                text.Set(commit.c_str(), commit.size());
                return _AddCharAndFinalize(ec, pContext, &text);
            }
        }
    }
    if (hasPendingPunctuationCommitText)
    {
        // Prefetch already contains the fully resolved commit text (candidate
        // plus punctuation, or a pure punctuation string resolved upstream).
        punctuationStr = std::move(pendingPunctuationCommitText);
    }
    else if (code == VK_DECIMAL)
    {
        // Numpad decimal should always commit ASCII '.' even in Chinese
        // punctuation mode (main-keyboard '.' still maps to '。').
        punctuationStr = L".";
    }

    double pipeReadElapsedMs = 0;
    if (!hasPendingPunctuationCommitText && _candidateMode != CANDIDATE_NONE && _pCandidateListUIPresenter)
    {
        //
        // 请求当前高亮候选词；服务端也可能返回以词定字的精确提交文本。
        //
        if (Global::CommitWithHighlightedCandPunc.count(wch) > 0)
        {
            PerfTimer pipeReadTimer;
            struct FanyImeNamedpipeDataToTsf *receivedData = TryReadDataFromServerPipeWithTimeout(requestId);
            pipeReadElapsedMs = pipeReadTimer.ElapsedMs();

            if (receivedData->msg_type == Global::DataFromServerMsgType::TransportUnavailable)
            {
                return HRESULT_FROM_WIN32(ERROR_BROKEN_PIPE);
            }

            // The Server is authoritative for configurable candidate-navigation
            // keys. The local paging snapshot can briefly lag behind while a TSF
            // client connects or a setting changes, so a comma/period may have
            // entered this punctuation path while the Server already treated it
            // as navigation. Never turn such a response into committed text.
            if (receivedData->msg_type == Global::DataFromServerMsgType::CommitExactText)
            {
                punctuationStr.assign(receivedData->candidate_string);
            }
            else if (receivedData->msg_type != Global::DataFromServerMsgType::Normal)
            {
                // The Server already updated the authoritative candidate state.
                // Consuming the response is sufficient; advancing the TSF-side
                // presenter here would apply the same navigation a second time.
                return S_OK;
            }
            else
            {
                const std::wstring candidate(receivedData->candidate_string);
                if (const WCHAR literal = Global::LiteralCandidatePunctuation(code, wch))
                {
                    punctuationStr = candidate + literal;
                }
                else
                {
                    const WCHAR preceding =
                        candidate.empty() ? _GetPrecedingCharForSmartPunctuation(ec, pContext) : candidate.back();
                    punctuationStr = candidate + _ResolveSmartPunctuation(wch, preceding);
                }
            }
        }
    }

    // Kept for the space-conversion fingerprint: after the commit this
    // character sits directly before the punctuation, and reading it back then
    // is what tells the armed punctuation apart from an identical one the
    // caret may have been moved to since.
    WCHAR smartPunctuationBeforeChar = 0;
    if (!hasPendingPunctuationCommitText && punctuationStr.empty() && code != VK_DECIMAL)
    {
        // Pure punctuation (no candidate prefix): choose ASCII vs Chinese from
        // the character immediately before the caret / composition.
        const WCHAR preceding = _GetPrecedingCharForSmartPunctuation(ec, pContext);
        smartPunctuationBeforeChar = preceding;
        punctuationStr = _ResolveSmartPunctuation(wch, preceding);
    }

    const bool pairedPunctuationEnabled = Global::PairedPunctuationEnabled.load(std::memory_order_relaxed) &&
                                          !Global::IsPairedPunctuationExcludedProcess(Global::current_process_name);
    // 全角模式下 `{` 补出 ｛｝、`}` 跨过 ｝，与 macOS 一致；只在成对补全打开时才需要读这个状态。
    const bool fullWidthPair = pairedPunctuationEnabled && (wch == L'{' || wch == L'}') &&
                               pCompositionProcessorEngine->GetDoubleSingleByteMode(_pThreadMgr, _tfClientId) != FALSE;
    if (pairedPunctuationEnabled && !_IsComposing() && _candidateMode == CANDIDATE_NONE)
    {
        // A pair whose closing half is still waiting on the right of the caret is closed by stepping over it. Without this the closing key inserts a second one （内容）） and, because of the pinning below, the right quote could never be typed at all.
        const WCHAR stepOver = Global::PairedPunctuationStepOverCandidate(wch, punctuationStr, fullWidthPair);
        if (_TryStepOverPairedPunctuation(ec, pContext, stepOver))
        {
            return S_OK;
        }
    }

    if (pairedPunctuationEnabled && !punctuationStr.empty())
    {
        // Quotes share one physical key for both sides. In paired mode every
        // press starts a fresh pair instead of following the legacy left/right
        // toggle maintained by GetPunctuation().
        // 直角引号样式下同理：』改回『，」改回「。
        punctuationStr.back() = Global::ReopenedPairedQuote(wch, punctuationStr.back());
        punctuationStr.back() = Global::PairedPunctuationKeyOpening(wch, punctuationStr.back(), fullWidthPair);
    }

    const WCHAR pairedOpening = punctuationStr.empty() ? 0 : punctuationStr.back();
    const WCHAR pairedClosing = pairedPunctuationEnabled ? GetPairedPunctuationClosing(punctuationStr) : 0;
    if (pairedClosing != 0)
    {
        punctuationStr.push_back(pairedClosing);
    }

    CStringRange punctuationString;
    punctuationString.Set(punctuationStr.c_str(), punctuationStr.length());

    const bool hasActiveComposition = _IsComposing() ? true : false;
    if (hasActiveComposition)
    {
        double insertElapsedMs = 0;
        PerfTimer insertTextTimer;
        hr = _InsertTextToComposition(ec, pContext, &punctuationString);
        insertElapsedMs = insertTextTimer.ElapsedMs();
        if (FAILED(hr))
        {
            PerfTimer fallbackTimer;
            hr = _AddComposingAndChar(ec, pContext, &punctuationString);
            insertElapsedMs += fallbackTimer.ElapsedMs();
        }
        if (FAILED(hr))
        {
            return hr;
        }
    }
    else
    {
        PerfTimer addCharTimer;
        hr = _AddCharAndFinalize(ec, pContext, &punctuationString);
        double addCharElapsedMs = addCharTimer.ElapsedMs();
        if (FAILED(hr))
        {
            return hr;
        }
    }

    PerfTimer completeTimer;
    if (hasActiveComposition)
    {
        _HandleCompleteCommitFirst(ec, pContext);
    }
    else
    {
        _HandleComplete(ec, pContext);
    }
    double completeElapsedMs = completeTimer.ElapsedMs();
    // Arm the local space conversion only for a standalone one-character
    // Chinese punctuation commit. Auto-completed pairs and candidate-prefixed
    // commits are deliberately left untouched so a following space cannot
    // rewrite half of a pair or historical candidate text.
    _ArmSmartPunctuationSpace(punctuationStr.size() == 1 ? punctuationStr.back() : 0,
                              pairedClosing != 0 || punctuationStr.size() != 1, smartPunctuationBeforeChar);
    if (pairedClosing != 0)
    {
        // The closing half was emitted here, not by a closing keystroke, so the nest-pair depth that resolving the opening advanced would never be paid back (the '>' is consumed by step-over). Balance it now, or the next 《》 degrades into 〈〉.
        pCompositionProcessorEngine->BalanceNestPairAfterAutoClose(wch);
        if (wch == L'<')
        {
            // With candidates open the Server's Engine resolved the opening and advanced its own nesting count, which this TSF cannot reach, so the Server pays it back too. Both counts stop at zero, so telling the Server when the TSF resolved the opening itself is harmless.
            SendPairedPunctuationAutoClosedToServerViaNamedPipe(wch);
        }
        _InvalidateSmartPunctuationShadow();
        // Track the pair so its closing key steps over the auto-inserted half, and move the caret between the halves through the queued move: WM_PairedPunctuationCaretMove only runs a move whose focus token _QueuePairedPunctuationCaretMove recorded.
        _PushPairedPunctuation(pairedOpening, pairedClosing);
        _QueuePairedPunctuationCaretMove(-1);
    }

    return S_OK;
}

//+---------------------------------------------------------------------------
//
// _HandleCompositionDoubleSingleByte
//
//----------------------------------------------------------------------------

HRESULT CMetasequoiaIME::_HandleCompositionDoubleSingleByte(TfEditCookie ec, _In_ ITfContext *pContext, WCHAR wch)
{
    HRESULT hr = S_OK;

    WCHAR fullWidth = Global::FullWidthCharTable[wch - 0x20];

    CStringRange fullWidthString;
    fullWidthString.Set(&fullWidth, 1);

    // Finalize character
    hr = _AddCharAndFinalize(ec, pContext, &fullWidthString);
    if (FAILED(hr))
    {
        return hr;
    }

    _HandleCancel(ec, pContext);

    return S_OK;
}

//+---------------------------------------------------------------------------
//
// _InvokeKeyHandler
//
// This text service is interested in handling keystrokes to demonstrate the
// use the compositions. Some apps will cancel compositions if they receive
// keystrokes while a compositions is ongoing.
//
// param
//    [in] uCode - virtual key code of WM_KEYDOWN wParam
//    [in] dwFlags - WM_KEYDOWN lParam
//    [in] dwKeyFunction - Function regarding virtual key
//----------------------------------------------------------------------------

HRESULT CMetasequoiaIME::_InvokeKeyHandler(_In_ ITfContext *pContext, UINT code, WCHAR wch, DWORD flags,
                                           _KEYSTROKE_STATE keyState, uint64_t requestId, std::wstring prefetchedText,
                                           UINT localResetToken, uint64_t expectedCompositionEpoch,
                                           uint64_t expectedFocusToken, uint64_t deferredReplayToken)
{
    flags;

    CKeyHandlerEditSession *pEditSession = nullptr;
    HRESULT hr = E_FAIL;
    HRESULT editSessionHr = E_FAIL;
    HRESULT requestHr = E_FAIL;

    // we'll insert a char ourselves in place of this keystroke
    LARGE_INTEGER requestStartQpc;
    QueryPerformanceCounter(&requestStartQpc);
    pEditSession = new (std::nothrow) CKeyHandlerEditSession(
        this, pContext, code, wch, keyState, requestId, requestStartQpc, std::move(prefetchedText),
        localResetToken == 0 ? (expectedFocusToken != 0 ? expectedFocusToken : _CaptureFocusSessionToken()) : 0,
        localResetToken, expectedCompositionEpoch, deferredReplayToken);
    if (pEditSession == nullptr)
    {
        if (deferredReplayToken != 0)
        {
            _RetryDeferredKeyReplay(deferredReplayToken);
        }
        goto Exit;
    }

    //
    // Call CKeyHandlerEditSession::DoEditSession().
    //
    // Do not specify TF_ES_SYNC so edit session is not invoked on WinWord
    //
    requestHr =
        pContext->RequestEditSession(_tfClientId, pEditSession, TF_ES_ASYNCDONTCARE | TF_ES_READWRITE, &editSessionHr);
    hr = FAILED(requestHr) ? requestHr : editSessionHr;
    DebugTsfIssue47(L"edit-session-request", requestId, code, wch, keyState.Category, keyState.Function, 1,
                    _IsComposing(),
                    _pCompositionProcessorEngine ? _pCompositionProcessorEngine->GetVirtualKeyLength() : 0, hr,
                    deferredReplayToken);
    if ((FAILED(requestHr) || FAILED(editSessionHr)) && deferredReplayToken != 0)
    {
        _RetryDeferredKeyReplay(deferredReplayToken);
    }

    pEditSession->Release();

Exit:
    return hr;
}
