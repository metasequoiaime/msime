#include "Private.h"
#include "Globals.h"
#include "MetasequoiaIME.h"
#include "CompositionProcessorEngine.h"
#include "Ipc.h"
#include "FanyUtils.h"
#include "TfTextLayoutSink.h"
#include "../HostComposition.h"
#include "../HostPrediction.h"
#include "../HostServerSelection.h"

bool CMetasequoiaIME::_IsSameComObject(_In_opt_ IUnknown *left, _In_opt_ IUnknown *right)
{
    if (left == nullptr || right == nullptr)
    {
        return left == right;
    }

    IUnknown *leftIdentity = nullptr;
    IUnknown *rightIdentity = nullptr;
    const HRESULT leftHr = left->QueryInterface(IID_IUnknown, reinterpret_cast<void **>(&leftIdentity));
    const HRESULT rightHr = right->QueryInterface(IID_IUnknown, reinterpret_cast<void **>(&rightIdentity));
    const bool matches = SUCCEEDED(leftHr) && SUCCEEDED(rightHr) && leftIdentity == rightIdentity;
    if (leftIdentity)
    {
        leftIdentity->Release();
    }
    if (rightIdentity)
    {
        rightIdentity->Release();
    }
    return matches;
}

void CMetasequoiaIME::_DebugCompositionRecovery(_In_z_ const WCHAR *reason, HRESULT hr) const
{
    if (!Global::TsfDiagnosticLogEnabled.load(std::memory_order_relaxed))
    {
        return;
    }

    WCHAR message[512] = {};
    const WCHAR *processName = Global::current_process_name.empty() ? L"unknown" : Global::current_process_name.c_str();
    StringCchPrintfW(message, ARRAYSIZE(message),
                     L"[msime][composition-recovery] reason=%s hr=0x%08lX epoch=%llu focus_token=%llu process=%s\n",
                     reason, static_cast<unsigned long>(hr),
                     static_cast<unsigned long long>(_CaptureCompositionEpoch()),
                     static_cast<unsigned long long>(_CaptureFocusSessionToken()), processName);
    QueueTsfDiagnosticLog(message);
}

namespace
{
bool HasActiveComposingProperty(_In_ ITfContext *context, TfEditCookie ecReadOnly, _In_ ITfComposition *composition,
                                _Out_ HRESULT *result)
{
    *result = E_FAIL;
    ITfRange *compositionRange = nullptr;
    HRESULT hr = composition->GetRange(&compositionRange);
    if (FAILED(hr) || compositionRange == nullptr)
    {
        *result = FAILED(hr) ? hr : E_FAIL;
        return false;
    }

    BOOL isEmpty = FALSE;
    hr = compositionRange->IsEmpty(ecReadOnly, &isEmpty);
    if (SUCCEEDED(hr) && isEmpty)
    {
        // StartComposition legitimately creates an empty range before the
        // first preedit update. There is no covered text on which GetValue
        // could return GUID_PROP_COMPOSING yet.
        compositionRange->Release();
        *result = S_OK;
        return true;
    }

    ITfProperty *composingProperty = nullptr;
    hr = context->GetProperty(GUID_PROP_COMPOSING, &composingProperty);
    if (FAILED(hr) || composingProperty == nullptr)
    {
        compositionRange->Release();
        *result = FAILED(hr) ? hr : E_FAIL;
        return false;
    }

    VARIANT value;
    VariantInit(&value);
    hr = composingProperty->GetValue(ecReadOnly, compositionRange, &value);
    const bool active = SUCCEEDED(hr) && value.vt == VT_I4 && value.lVal != 0;
    VariantClear(&value);
    composingProperty->Release();
    compositionRange->Release();
    *result = hr;
    return active;
}
} // namespace

//+---------------------------------------------------------------------------
//
// ITfTextEditSink::OnEndEdit
//
// Called by the system whenever anyone releases a write-access document lock.
//----------------------------------------------------------------------------

STDAPI CMetasequoiaIME::OnEndEdit(__RPC__in_opt ITfContext *pContext, TfEditCookie ecReadOnly,
                                  __RPC__in_opt ITfEditRecord *pEditRecord)
{
    if (!IsNamedpipeFocusStateOwner(this) || !Global::g_connected)
    {
        return S_OK;
    }
    BOOL isSelectionChanged;

    //
    // did the selection change?
    // The selection change includes the movement of caret as well.
    // The caret position is represent as the empty selection range when
    // there is no selection.
    //
    if (pEditRecord == nullptr)
    {
        return E_INVALIDARG;
    }
    if (_IsComposing() && !_localSessionResetPending.load(std::memory_order_acquire))
    {
        HRESULT compositionStateHr = S_OK;
        const bool ownerMatches = pContext && _pContext && _IsSameComObject(pContext, _pContext);
        const bool propertyActive =
            ownerMatches && HasActiveComposingProperty(pContext, ecReadOnly, _pComposition, &compositionStateHr);
        if (!ownerMatches || !propertyActive)
        {
            _DebugCompositionRecovery(ownerMatches ? L"composing-property-missing" : L"edit-context-mismatch",
                                      compositionStateHr);
            MarkNamedpipeSessionDirtyForOwner(this);
            return S_OK;
        }
    }
    // 上屏后联想没有组字，光标离开它显示时的位置就关掉它：点了别处、应用打进了交给它的键。我们自己上屏的那一次正好停在记下的位置，不算。
    if (!_IsComposing() && _IsPredictionVisible())
    {
        _CheckPredictionCaret(ecReadOnly, pContext);
    }
    if (SUCCEEDED(pEditRecord->GetSelectionStatus(&isSelectionChanged)) && isSelectionChanged)
    {
        // If the selection is moved to out side of the current composition,
        // we terminate the composition. This TextService supports only one
        // composition in one context object.
        if (_IsComposing())
        {
            TF_SELECTION tfSelection;
            ULONG fetched = 0;

            if (pContext == nullptr)
            {
                return E_INVALIDARG;
            }
            if (FAILED(pContext->GetSelection(ecReadOnly, TF_DEFAULT_SELECTION, 1, &tfSelection, &fetched)) ||
                fetched != 1)
            {
                return S_FALSE;
            }

            ITfRange *pRangeComposition = nullptr;
            if (SUCCEEDED(_pComposition->GetRange(&pRangeComposition)))
            {
                if (!_IsRangeCovered(ecReadOnly, tfSelection.range, pRangeComposition))
                {
                    const HRESULT endHr = _EndComposition(pContext);
                    DebugTsfIssue47(
                        L"selection-moved-outside-composition", FANY_IME_NO_REQUEST_ID, 0, L'\0', 0, 0, -1, TRUE,
                        _pCompositionProcessorEngine ? _pCompositionProcessorEngine->GetVirtualKeyLength() : 0, endHr,
                        _CaptureCompositionEpoch());
                    if (FAILED(endHr))
                    {
                        MarkNamedpipeSessionDirtyForOwner(this);
                    }
                }

                pRangeComposition->Release();
            }

            tfSelection.range->Release();
        }
    }

    return S_OK;
}

//+---------------------------------------------------------------------------
//
// _InitTextEditSink
//
// Init a text edit sink on the topmost context of the document.
// Always release any previous sink.
//----------------------------------------------------------------------------

BOOL CMetasequoiaIME::_InitTextEditSink(_In_opt_ ITfDocumentMgr *pDocMgr)
{
    ITfSource *pSource = nullptr;
    BOOL ret = TRUE;

    // clear out any previous sink first
    if (_textEditSinkCookie != TF_INVALID_COOKIE)
    {
        if (SUCCEEDED(_pTextEditSinkContext->QueryInterface(IID_ITfSource, (void **)&pSource)))
        {
            pSource->UnadviseSink(_textEditSinkCookie);
            pSource->Release();
        }

        _pTextEditSinkContext->Release();
        _pTextEditSinkContext = nullptr;
        _textEditSinkCookie = TF_INVALID_COOKIE;
    }

    if (pDocMgr == nullptr)
    {
        return TRUE; // caller just wanted to clear the previous sink
    }

    if (FAILED(pDocMgr->GetTop(&_pTextEditSinkContext)))
    {
        return FALSE;
    }

    if (_pTextEditSinkContext == nullptr)
    {
        return TRUE; // empty document, no sink possible
    }

    ret = FALSE;
    if (SUCCEEDED(_pTextEditSinkContext->QueryInterface(IID_ITfSource, (void **)&pSource)))
    {
        if (SUCCEEDED(pSource->AdviseSink(IID_ITfTextEditSink, (ITfTextEditSink *)this, &_textEditSinkCookie)))
        {
            ret = TRUE;
        }
        else
        {
            _textEditSinkCookie = TF_INVALID_COOKIE;
        }
        pSource->Release();
    }

    if (ret == FALSE)
    {
        _pTextEditSinkContext->Release();
        _pTextEditSinkContext = nullptr;
    }

    return ret;
}

//+---------------------------------------------------------------------------
//
// 上屏后联想
//
//----------------------------------------------------------------------------

namespace
{
// 光标处的屏幕坐标（物理像素，文字底边的左端）。空区间在有的应用里量不出高度，那就量光标前的一个字，取它的右端。
bool PredictionCaretPoint(TfEditCookie ec, _In_ ITfContext *context, _In_ ITfRange *caret, _Out_ POINT *point)
{
    ITfContextView *view = nullptr;
    if (FAILED(context->GetActiveView(&view)) || view == nullptr)
    {
        return false;
    }
    bool found = false;
    RECT rect = {};
    BOOL clipped = FALSE;
    if (SUCCEEDED(view->GetTextExt(ec, caret, &rect, &clipped)) && rect.bottom > rect.top)
    {
        *point = GetPhysicalTextAnchor(view, rect);
        found = true;
    }
    else
    {
        ITfRange *previous = nullptr;
        LONG shifted = 0;
        if (SUCCEEDED(caret->Clone(&previous)) && previous &&
            SUCCEEDED(previous->ShiftStart(ec, -1, &shifted, nullptr)) && shifted == -1 &&
            SUCCEEDED(view->GetTextExt(ec, previous, &rect, &clipped)) && rect.bottom > rect.top)
        {
            rect.left = rect.right;
            *point = GetPhysicalTextAnchor(view, rect);
            found = true;
        }
        if (previous)
        {
            previous->Release();
        }
    }
    view->Release();
    return found;
}
} // namespace

void CMetasequoiaIME::_ForgetPrediction()
{
    _predictionGeneration = 0;
    if (_pPredictionAnchor)
    {
        _pPredictionAnchor->Release();
        _pPredictionAnchor = nullptr;
    }
    if (_pPredictionContext)
    {
        _pPredictionContext->Release();
        _pPredictionContext = nullptr;
    }
}

void CMetasequoiaIME::_DismissPrediction()
{
    if (!_IsPredictionVisible())
    {
        return;
    }
    _ForgetPrediction();
    // Server 已经为这一代联想收到过 Show：这个隐藏让它清掉会话里的联想、收起候选窗（FocusedSession::hide_candidate）。
    if (Global::g_connected)
    {
        SendHideCandidateWndEventToUIProcess();
    }
    auto *host = _pCompositionProcessorEngine ? _pCompositionProcessorEngine->GetHostEngineAdapter() : nullptr;
    msime::tsf::EngineResult current;
    if (host && host->valid() && msime::tsf::HostView(*host, &current) &&
        msime::tsf::HostShowsPrediction(current.view))
    {
        (void)_CancelHostComposition();
    }
}

void CMetasequoiaIME::_RefreshPrediction(TfEditCookie ec, _In_ ITfContext *pContext)
{
    auto *host = _pCompositionProcessorEngine ? _pCompositionProcessorEngine->GetHostEngineAdapter() : nullptr;
    msime::tsf::EngineResult current;
    if (!_IsPredictionVisible() && (!host || !host->valid() || _IsComposing()))
    {
        return;
    }
    const bool read = host && host->valid() && msime::tsf::HostView(*host, &current);
    // UI-less 宿主自己画候选，Server 连不上时也没有候选窗可用，这两种情况下不在光标处显示联想。
    const bool showable = pContext != nullptr && Global::g_connected && !Global::IsUiLessMode() &&
                          !_IsServerUnavailableFallbackActive();
    switch (msime::tsf::DecidePredictionRefresh(_predictionGeneration, _IsComposing() != FALSE, showable && read,
                                                read ? current.view : msime::tsf::EngineView{}))
    {
    case msime::tsf::PredictionRefresh::None:
    case msime::tsf::PredictionRefresh::Keep:
        return;
    case msime::tsf::PredictionRefresh::Hide:
        // 组字已经开始时不再发隐藏：那会让 Server 取消新的组字。联想在开始组字的那个键按下时已经关掉了（KeyEventSink）。
        if (_IsComposing())
        {
            _ForgetPrediction();
            return;
        }
        _DismissPrediction();
        return;
    case msime::tsf::PredictionRefresh::Show:
        break;
    }

    TF_SELECTION selection = {};
    ULONG fetched = 0;
    ITfRange *anchor = nullptr;
    if (SUCCEEDED(pContext->GetSelection(ec, TF_DEFAULT_SELECTION, 1, &selection, &fetched)) && fetched == 1 &&
        selection.range)
    {
        if (FAILED(selection.range->Clone(&anchor)))
        {
            anchor = nullptr;
        }
        selection.range->Release();
    }
    POINT point = {};
    const bool placed = anchor && SUCCEEDED(anchor->Collapse(ec, TF_ANCHOR_END)) &&
                        SUCCEEDED(anchor->SetGravity(ec, TF_GRAVITY_BACKWARD, TF_GRAVITY_BACKWARD)) &&
                        PredictionCaretPoint(ec, pContext, anchor, &point);
    if (!placed)
    {
        // 量不出光标位置就不显示：候选窗落在别处比不出现更糟。宿主会话里的联想一并清掉，免得 Esc 被白白吃掉。
        if (anchor)
        {
            anchor->Release();
        }
        if (_IsPredictionVisible())
        {
            _DismissPrediction();
        }
        else if (msime::tsf::HostShowsPrediction(current.view))
        {
            (void)_CancelHostComposition();
        }
        return;
    }
    _ForgetPrediction();
    _pPredictionAnchor = anchor;
    _pPredictionContext = pContext;
    _pPredictionContext->AddRef();
    _predictionGeneration = current.view.generation != 0 ? current.view.generation : 1;
    Global::Point[0] = point.x;
    Global::Point[1] = point.y;
    WriteDataToNamedPipe(0, L'\0', 0, Global::Point, 0, L"", 0b001000);
    SendShowCandidateWndEventToUIProcess();
}

void CMetasequoiaIME::_CheckPredictionCaret(TfEditCookie ecReadOnly, _In_ ITfContext *pContext)
{
    if (!_IsPredictionVisible())
    {
        return;
    }
    bool stays = pContext != nullptr && _pPredictionAnchor != nullptr && _IsSameComObject(pContext, _pPredictionContext);
    if (stays)
    {
        TF_SELECTION selection = {};
        ULONG fetched = 0;
        BOOL startEqual = FALSE;
        BOOL endEqual = FALSE;
        stays = SUCCEEDED(pContext->GetSelection(ecReadOnly, TF_DEFAULT_SELECTION, 1, &selection, &fetched)) &&
                fetched == 1 && selection.range != nullptr;
        if (stays)
        {
            stays = SUCCEEDED(selection.range->IsEqualStart(ecReadOnly, _pPredictionAnchor, TF_ANCHOR_START,
                                                            &startEqual)) &&
                    SUCCEEDED(selection.range->IsEqualEnd(ecReadOnly, _pPredictionAnchor, TF_ANCHOR_END,
                                                          &endEqual)) &&
                    startEqual && endEqual;
        }
        if (selection.range)
        {
            selection.range->Release();
        }
    }
    if (!stays)
    {
        _DismissPrediction();
    }
}

bool CMetasequoiaIME::_FollowServerClickInHost(const std::wstring &text)
{
    auto *host = _pCompositionProcessorEngine ? _pCompositionProcessorEngine->GetHostEngineAdapter() : nullptr;
    if (!host || !host->valid() || text.empty())
    {
        return false;
    }
    msime::tsf::EngineResult current;
    if (!msime::tsf::HostView(*host, &current))
    {
        return false;
    }
    const auto committed = FanyUtils::wstring_to_string(text);
    // 点的是 Server 的候选窗，文字是 Server 那一次上屏的全部（组字时暂存的词段在前），宿主会话要选的只是候选自己那一段。
    const auto piece = msime::tsf::ServerSelectedPiece({FanyImeReplyType::Normal, committed}, current.view.phrase_prefix);
    std::string raw, error;
    if (msime::tsf::SelectHostCandidateByText(*host, current.view, piece, &raw, &error) !=
        msime::tsf::HostSelectionOutcome::Selected)
    {
        return false;
    }
    msime::tsf::EngineResult selected;
    // 两边上屏的必须是同一段文字，而且宿主会话里不能还剩组字；否则宿主会话已经和文档分开，交给调用方丢掉它。
    return msime::tsf::EngineSessionAdapter::parse_result(raw, &selected, &error) && selected.has_commit &&
           selected.commit == committed && !msime::tsf::HostViewComposes(selected.view);
}

HRESULT CMetasequoiaIME::_CommitPredictionCandidate(TfEditCookie ec, _In_ ITfContext *pContext,
                                                    const std::wstring &text)
{
    // 宿主会话按文字跟着选同一个联想，下一代联想才从它出来（_RefreshPrediction）；跟不上就清掉它的联想，编辑会话结束时随之关掉候选窗。
    if (!_FollowServerClickInHost(text))
    {
        auto *host = _pCompositionProcessorEngine ? _pCompositionProcessorEngine->GetHostEngineAdapter() : nullptr;
        msime::tsf::EngineResult current;
        if (host && host->valid() && msime::tsf::HostView(*host, &current) &&
            msime::tsf::HostShowsPrediction(current.view))
        {
            (void)_CancelHostComposition();
        }
    }
    const HRESULT hr = _HandleInsertText(ec, pContext, text);
    if (SUCCEEDED(hr) && !text.empty())
    {
        _smartPunctuationShadowChar = text.back();
        _smartPunctuationShadowValid = true;
    }
    return hr;
}
