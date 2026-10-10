#include "Private.h"
#include "Globals.h"
#include "MetasequoiaIME.h"
#include "CompositionProcessorEngine.h"
#include <cwctype>
#include <debugapi.h>
#include <fmt/xchar.h>
#include <string>
#include "FanyDefines.h"
#include "Ipc.h"
#include "SmartPunctuationFingerprint.h"
#include "../Utils/PerfTimer.h"

namespace
{
// Keep SEH helpers free of C++ objects with destructors (C2712).
HRESULT SafeRangeSetText(_In_ ITfRange *range, TfEditCookie ec, DWORD flags, _In_reads_opt_(len) const WCHAR *text,
                         LONG len)
{
    if (range == nullptr)
    {
        return E_INVALIDARG;
    }

#ifdef __MINGW32__
    return range->SetText(ec, flags, text, len);
#else
    __try
    {
        return range->SetText(ec, flags, text, len);
    }
    __except (EXCEPTION_EXECUTE_HANDLER)
    {
        return E_FAIL;
    }
#endif
}

HRESULT SafeRangeGetText(_In_ ITfRange *range, TfEditCookie ec, DWORD flags, _Out_writes_(len) WCHAR *text, ULONG len,
                         _Out_ ULONG *fetched)
{
    if (range == nullptr || text == nullptr || fetched == nullptr || len == 0)
    {
        return E_INVALIDARG;
    }

#ifdef __MINGW32__
    return range->GetText(ec, flags, text, len, fetched);
#else
    __try
    {
        return range->GetText(ec, flags, text, len, fetched);
    }
    __except (EXCEPTION_EXECUTE_HANDLER)
    {
        *fetched = 0;
        return E_FAIL;
    }
#endif
}

HRESULT SafeRangeShiftStart(_In_ ITfRange *range, TfEditCookie ec, LONG count, _Out_ LONG *shifted)
{
    if (range == nullptr || shifted == nullptr)
    {
        return E_INVALIDARG;
    }

#ifdef __MINGW32__
    return range->ShiftStart(ec, count, shifted, nullptr);
#else
    __try
    {
        return range->ShiftStart(ec, count, shifted, nullptr);
    }
    __except (EXCEPTION_EXECUTE_HANDLER)
    {
        *shifted = 0;
        return E_FAIL;
    }
#endif
}

HRESULT SafeRangeShiftEnd(_In_ ITfRange *range, TfEditCookie ec, LONG count, _Out_ LONG *shifted)
{
    if (range == nullptr || shifted == nullptr)
    {
        return E_INVALIDARG;
    }

#ifdef __MINGW32__
    return range->ShiftEnd(ec, count, shifted, nullptr);
#else
    __try
    {
        return range->ShiftEnd(ec, count, shifted, nullptr);
    }
    __except (EXCEPTION_EXECUTE_HANDLER)
    {
        *shifted = 0;
        return E_FAIL;
    }
#endif
}

bool AreCaretModifiersPhysicallyDown()
{
    // VK_LWIN/VK_RWIN matter as much as Shift here: Win+Left is the window snap
    // shortcut, so an arrow released into a held Win chord rearranges the
    // desktop instead of moving the caret.
    static const int keys[] = {VK_SHIFT, VK_CONTROL, VK_MENU, VK_LWIN, VK_RWIN};
    for (int key : keys)
    {
        if ((GetAsyncKeyState(key) & 0x8000) != 0)
        {
            return true;
        }
    }
    return false;
}

WCHAR SmartPunctuationAsciiFor(WCHAR chinese)
{
    switch (chinese)
    {
    case L'。':
        return L'.';
    case L'，':
        return L',';
    case L'！':
        return L'!';
    case L'？':
        return L'?';
    case L'；':
        return L';';
    case L'：':
        return L':';
    case L'、':
        return L'/';
    case L'“':
    case L'”':
        return L'"';
    case L'‘':
    case L'’':
        return L'\'';
    case L'【':
        return L'[';
    case L'】':
        return L']';
    case L'《':
        return L'<';
    case L'》':
        return L'>';
    case L'（':
        return static_cast<WCHAR>(L'(');
    case L'）':
        return L')';
    default:
        return 0;
    }
}
} // namespace

int CMetasequoiaIME::_GetPrecedingDocumentChars(TfEditCookie ec, _In_ ITfContext *pContext,
                                                _Out_writes_(count) WCHAR *buffer, int count)
{
    if (pContext == nullptr || buffer == nullptr || count <= 0)
    {
        return 0;
    }

    ITfRange *pAnchor = nullptr;
    bool releaseAnchor = false;

    if (_IsComposing() && _pComposition != nullptr)
    {
        if (FAILED(_pComposition->GetRange(&pAnchor)) || pAnchor == nullptr)
        {
            return 0;
        }
        releaseAnchor = true;
    }
    else
    {
        TF_SELECTION tfSelection = {};
        ULONG fetched = 0;
        const HRESULT hr = pContext->GetSelection(ec, TF_DEFAULT_SELECTION, 1, &tfSelection, &fetched);
        if (FAILED(hr) || fetched != 1 || tfSelection.range == nullptr)
        {
            return 0;
        }
        pAnchor = tfSelection.range;
        releaseAnchor = true;
    }

    ITfRange *pClone = nullptr;
    int readCount = 0;
    HRESULT hr = pAnchor->Clone(&pClone);
    if (SUCCEEDED(hr) && pClone != nullptr)
    {
        hr = pClone->Collapse(ec, TF_ANCHOR_START);
        if (SUCCEEDED(hr))
        {
            LONG shifted = 0;
            // A shift shorter than requested is the document start, which is
            // still worth reading: the caller decides what a short read means.
            hr = SafeRangeShiftStart(pClone, ec, -count, &shifted);
            if (SUCCEEDED(hr) && shifted < 0)
            {
                // Terminals and other shallow text stores accept the shift but
                // expose no text, which reads back as 0 characters.
                ULONG fetched = 0;
                if (SUCCEEDED(SafeRangeGetText(pClone, ec, 0, buffer, static_cast<ULONG>(count), &fetched)))
                {
                    readCount = static_cast<int>(fetched);
                }
            }
        }
        pClone->Release();
    }

    if (releaseAnchor && pAnchor != nullptr)
    {
        pAnchor->Release();
    }
    return readCount;
}

WCHAR CMetasequoiaIME::_GetPrecedingDocumentChar(TfEditCookie ec, _In_ ITfContext *pContext)
{
    WCHAR buffer[1] = {};
    return _GetPrecedingDocumentChars(ec, pContext, buffer, 1) == 1 ? buffer[0] : 0;
}

bool CMetasequoiaIME::_SmartPunctuationFingerprintMatches(TfEditCookie ec, _In_ ITfContext *pContext, WCHAR beforeChar)
{
    if (beforeChar == 0)
    {
        return true;
    }
    // Two characters: the punctuation about to be rewritten, and before it the
    // one recorded when the rewrite armed.
    WCHAR buffer[2] = {};
    const int readCount = _GetPrecedingDocumentChars(ec, pContext, buffer, 2);
    return Global::SmartPunctuationFingerprintMatches(readCount, buffer[0], beforeChar);
}

WCHAR CMetasequoiaIME::_GetFollowingDocumentChar(TfEditCookie ec, _In_ ITfContext *pContext)
{
    if (pContext == nullptr)
    {
        return 0;
    }

    TF_SELECTION tfSelection = {};
    ULONG fetched = 0;
    if (FAILED(pContext->GetSelection(ec, TF_DEFAULT_SELECTION, 1, &tfSelection, &fetched)) || fetched != 1 ||
        tfSelection.range == nullptr)
    {
        return 0;
    }

    ITfRange *pClone = nullptr;
    WCHAR following = 0;
    HRESULT hr = tfSelection.range->Clone(&pClone);
    if (SUCCEEDED(hr) && pClone != nullptr)
    {
        hr = pClone->Collapse(ec, TF_ANCHOR_END);
        if (SUCCEEDED(hr))
        {
            LONG shifted = 0;
            hr = SafeRangeShiftEnd(pClone, ec, 1, &shifted);
            if (SUCCEEDED(hr) && shifted == 1)
            {
                // Terminals and other shallow text stores accept the shift but
                // expose no text, leaving following at 0.
                WCHAR buffer[2] = {};
                ULONG got = 0;
                hr = SafeRangeGetText(pClone, ec, 0, buffer, 1, &got);
                if (SUCCEEDED(hr) && got == 1)
                {
                    following = buffer[0];
                }
            }
        }
        pClone->Release();
    }

    tfSelection.range->Release();
    return following;
}

WCHAR CMetasequoiaIME::_GetPrecedingCharForSmartPunctuation(TfEditCookie ec, _In_ ITfContext *pContext)
{
    if (_smartPunctuationShadowValid)
    {
        return _smartPunctuationShadowChar;
    }
    return _GetPrecedingDocumentChar(ec, pContext);
}

bool CMetasequoiaIME::_CanConvertSmartPunctuationSpace() const
{
    return _smartPunctuationSpaceArmed &&
           Global::SmartPunctuationSpaceConvertEnabled.load(std::memory_order_relaxed) &&
           _IsFocusSessionCurrent(_smartPunctuationSpaceFocusToken) &&
           GetForegroundWindow() == _smartPunctuationSpaceForegroundWindow;
}

bool CMetasequoiaIME::_CanRevertSmartPunctuation(WCHAR wch) const
{
    const bool sameKey = wch == _smartPunctuationRevertAscii ||
                         (_smartPunctuationRevertAscii == L'/' && wch == L'\\');
    return _smartPunctuationRevertArmed && wch != 0 && sameKey &&
           Global::SmartPunctuationRepeatToChineseEnabled.load(std::memory_order_relaxed) &&
           GetTickCount64() <= _smartPunctuationRevertDeadline &&
           _IsFocusSessionCurrent(_smartPunctuationRevertFocusToken) &&
           GetForegroundWindow() == _smartPunctuationRevertForegroundWindow;
}

void CMetasequoiaIME::_ClearSmartPunctuationSpace()
{
    _smartPunctuationSpaceArmed = false;
    _smartPunctuationSpaceChinese = 0;
    _smartPunctuationSpaceBeforeChar = 0;
    _smartPunctuationSpaceFocusToken = 0;
    _smartPunctuationSpaceForegroundWindow = nullptr;
}

void CMetasequoiaIME::_ClearSmartPunctuationRevert()
{
    _smartPunctuationRevertArmed = false;
    _smartPunctuationRevertAscii = 0;
    _smartPunctuationRevertChinese = 0;
    _smartPunctuationRevertBeforeChar = 0;
    _smartPunctuationRevertFocusToken = 0;
    _smartPunctuationRevertForegroundWindow = nullptr;
    _smartPunctuationRevertDeadline = 0;
}

void CMetasequoiaIME::_ArmSmartPunctuationRevert(WCHAR ascii, WCHAR chinese, WCHAR beforeChar)
{
    _ClearSmartPunctuationRevert();
    if (ascii == 0 || chinese == 0 ||
        !Global::SmartPunctuationRepeatToChineseEnabled.load(std::memory_order_relaxed))
    {
        return;
    }
    _smartPunctuationRevertArmed = true;
    _smartPunctuationRevertAscii = ascii;
    _smartPunctuationRevertChinese = chinese;
    _smartPunctuationRevertBeforeChar = beforeChar;
    _smartPunctuationRevertFocusToken = _CaptureFocusSessionToken();
    _smartPunctuationRevertForegroundWindow = GetForegroundWindow();
    _smartPunctuationRevertDeadline = GetTickCount64() + SMART_PUNCTUATION_REPEAT_INTERVAL_MS;
}

void CMetasequoiaIME::_ArmSmartPunctuationSpace(WCHAR chinese, bool autoClosedPair, WCHAR beforeChar)
{
    _ClearSmartPunctuationSpace();
    _ClearSmartPunctuationRevert();
    if (autoClosedPair || chinese == 0 ||
        !Global::SmartPunctuationEnabled.load(std::memory_order_relaxed) ||
        !Global::SmartPunctuationSpaceConvertEnabled.load(std::memory_order_relaxed) ||
        SmartPunctuationAsciiFor(chinese) == 0)
    {
        return;
    }
    _smartPunctuationSpaceArmed = true;
    _smartPunctuationSpaceChinese = chinese;
    _smartPunctuationSpaceBeforeChar = beforeChar;
    _smartPunctuationSpaceFocusToken = _CaptureFocusSessionToken();
    _smartPunctuationSpaceForegroundWindow = GetForegroundWindow();
}

HRESULT CMetasequoiaIME::_HandleSmartPunctuationConvert(TfEditCookie ec, _In_ ITfContext *pContext)
{
    const WCHAR chinese = _smartPunctuationSpaceChinese;
    const WCHAR beforeChar = _smartPunctuationSpaceBeforeChar;
    const WCHAR ascii = SmartPunctuationAsciiFor(chinese);
    _ClearSmartPunctuationSpace();
    if (ascii == 0)
    {
        CStringRange space;
        space.Set(L" ", 1);
        return _AddCharAndFinalize(ec, pContext, &space);
    }

    const WCHAR preceding = _GetPrecedingDocumentChar(ec, pContext);
    // The character before the caret being the expected punctuation is not on
    // its own proof that it is the one that armed: the same punctuation is
    // usually somewhere else in the document too.
    if ((preceding != 0 && preceding != chinese) || !_SmartPunctuationFingerprintMatches(ec, pContext, beforeChar))
    {
        CStringRange space;
        space.Set(L" ", 1);
        return _AddCharAndFinalize(ec, pContext, &space);
    }
    // Reading nothing back is not "the document happens to be empty here": a
    // terminal's TSF context is a proxy that keeps no committed text, and it
    // accepts ShiftStart and SetText and reports success while changing
    // nothing on screen. Rewriting in place there silently does nothing. Go
    // through the input queue instead, which is what the host cannot fake.
    if (preceding == 0 && _QueueSmartPunctuationRewrite(ascii))
    {
        _smartPunctuationShadowChar = ascii;
        _smartPunctuationShadowValid = true;
        _ArmSmartPunctuationRevert(ascii, chinese, beforeChar);
        return S_OK;
    }

    TF_SELECTION selection = {};
    ULONG fetched = 0;
    HRESULT hr = pContext->GetSelection(ec, TF_DEFAULT_SELECTION, 1, &selection, &fetched);
    if (FAILED(hr) || fetched != 1 || selection.range == nullptr)
    {
        CStringRange space;
        space.Set(L" ", 1);
        return _AddCharAndFinalize(ec, pContext, &space);
    }

    LONG shifted = 0;
    hr = selection.range->Collapse(ec, TF_ANCHOR_START);
    if (SUCCEEDED(hr))
    {
        hr = SafeRangeShiftStart(selection.range, ec, -1, &shifted);
        if (SUCCEEDED(hr) && shifted != -1)
        {
            hr = E_FAIL;
        }
    }
    if (SUCCEEDED(hr))
    {
        const WCHAR replacement[] = {ascii};
        hr = SafeRangeSetText(selection.range, ec, 0, replacement, 1);
    }
    if (SUCCEEDED(hr))
    {
        selection.range->Collapse(ec, TF_ANCHOR_END);
        pContext->SetSelection(ec, 1, &selection);
        _smartPunctuationShadowChar = ascii;
        _smartPunctuationShadowValid = true;
        // The rewrite replaced the punctuation in place, so whatever sat before
        // it still does; the fingerprint carries over to the revert window.
        _ArmSmartPunctuationRevert(ascii, chinese, beforeChar);
    }
    selection.range->Release();
    if (FAILED(hr))
    {
        CStringRange space;
        space.Set(L" ", 1);
        return _AddCharAndFinalize(ec, pContext, &space);
    }
    return S_OK;
}

HRESULT CMetasequoiaIME::_HandleSmartPunctuationRevert(TfEditCookie ec, _In_ ITfContext *pContext, WCHAR wch)
{
    const WCHAR ascii = _smartPunctuationRevertAscii;
    const WCHAR chinese = _smartPunctuationRevertChinese;
    const WCHAR beforeChar = _smartPunctuationRevertBeforeChar;
    _ClearSmartPunctuationRevert();
    const bool sameKey = wch == ascii || (ascii == L'/' && wch == L'\\');
    if (ascii == 0 || chinese == 0 || !sameKey)
    {
        return E_INVALIDARG;
    }

    const WCHAR preceding = _GetPrecedingDocumentChar(ec, pContext);
    if ((preceding != 0 && preceding != ascii) || !_SmartPunctuationFingerprintMatches(ec, pContext, beforeChar))
    {
        CStringRange fallback;
        fallback.Set(&chinese, 1);
        return _AddCharAndFinalize(ec, pContext, &fallback);
    }
    // Same as the convert above: a store that gives nothing back cannot be
    // rewritten in place, and appending the Chinese form instead would leave
    // the ASCII one in front of it.
    if (preceding == 0 && _QueueSmartPunctuationRewrite(chinese))
    {
        _smartPunctuationShadowChar = chinese;
        _smartPunctuationShadowValid = true;
        return S_OK;
    }

    TF_SELECTION selection = {};
    ULONG fetched = 0;
    HRESULT hr = pContext->GetSelection(ec, TF_DEFAULT_SELECTION, 1, &selection, &fetched);
    if (FAILED(hr) || fetched != 1 || selection.range == nullptr)
    {
        CStringRange fallback;
        fallback.Set(&chinese, 1);
        return _AddCharAndFinalize(ec, pContext, &fallback);
    }

    LONG shifted = 0;
    hr = selection.range->Collapse(ec, TF_ANCHOR_START);
    if (SUCCEEDED(hr))
    {
        hr = SafeRangeShiftStart(selection.range, ec, -1, &shifted);
        if (SUCCEEDED(hr) && shifted != -1)
        {
            hr = E_FAIL;
        }
    }
    if (SUCCEEDED(hr))
    {
        hr = SafeRangeSetText(selection.range, ec, 0, &chinese, 1);
    }
    if (SUCCEEDED(hr))
    {
        selection.range->Collapse(ec, TF_ANCHOR_END);
        pContext->SetSelection(ec, 1, &selection);
        _smartPunctuationShadowChar = chinese;
        _smartPunctuationShadowValid = true;
    }
    selection.range->Release();
    if (FAILED(hr))
    {
        CStringRange fallback;
        fallback.Set(&chinese, 1);
        return _AddCharAndFinalize(ec, pContext, &fallback);
    }
    return S_OK;
}

WCHAR CMetasequoiaIME::_GetPairedPunctuationClosingFor(WCHAR opening)
{
    return Global::PairedPunctuationClosingFor(opening);
}

void CMetasequoiaIME::_PushPairedPunctuation(WCHAR opening, WCHAR closing)
{
    if (opening == 0 || closing == 0)
    {
        return;
    }

    if (_pairedPunctuationStack.size() >= PAIRED_PUNCTUATION_MAX_DEPTH)
    {
        _pairedPunctuationStack.erase(_pairedPunctuationStack.begin());
    }

    PairedPunctuationEntry entry;
    entry.opening = opening;
    entry.closing = closing;
    entry.focusToken = _CaptureFocusSessionToken();
    _pairedPunctuationStack.push_back(entry);
}

WCHAR CMetasequoiaIME::_CandidateCommitPairedClosing(const std::wstring &text) const
{
    if (!Global::PairedPunctuationEnabled.load(std::memory_order_relaxed) ||
        Global::IsPairedPunctuationExcludedProcess(Global::current_process_name))
    {
        return 0;
    }
    return Global::PairedPunctuationClosingForCandidate(text);
}

void CMetasequoiaIME::_OpenCandidateCommitPair(WCHAR opening, WCHAR closing)
{
    if (opening == 0 || closing == 0)
    {
        return;
    }
    _InvalidateSmartPunctuationShadow();
    _PushPairedPunctuation(opening, closing);
    _QueuePairedPunctuationCaretMove(-1);
}

void CMetasequoiaIME::_ClearPairedPunctuationStack()
{
    _pairedPunctuationStack.clear();
}

bool CMetasequoiaIME::_TryStepOverPairedPunctuation(TfEditCookie ec, _In_ ITfContext *pContext, WCHAR closing)
{
    if (closing == 0 || _pairedPunctuationStack.empty())
    {
        return false;
    }

    const PairedPunctuationEntry top = _pairedPunctuationStack.back();
    if (top.closing != closing || !_IsFocusSessionCurrent(top.focusToken, pContext))
    {
        _ClearPairedPunctuationStack();
        return false;
    }

    // Mouse clicks and host-side edits can move the caret without a key event,
    // so verify the closing half when the text store exposes it.
    const WCHAR following = _GetFollowingDocumentChar(ec, pContext);
    if (following != 0 && following != closing)
    {
        _ClearPairedPunctuationStack();
        return false;
    }

    _pairedPunctuationStack.pop_back();
    _ResetSmartPunctuationHistory();
    _InvalidateSmartPunctuationShadow();
    _QueuePairedPunctuationCaretMove(1);
    return true;
}

void CMetasequoiaIME::_NoteKeyForPairedPunctuation(UINT code)
{
    if (_pairedPunctuationStack.empty() && _pendingPairedCaretDelta == 0)
    {
        return;
    }

    switch (code)
    {
    case VK_SHIFT:
    case VK_LSHIFT:
    case VK_RSHIFT:
    case VK_CONTROL:
    case VK_LCONTROL:
    case VK_RCONTROL:
    case VK_MENU:
    case VK_LMENU:
    case VK_RMENU:
    case VK_CAPITAL:
        return;
    case VK_BACK:
    case VK_DELETE:
    case VK_INSERT:
    case VK_RETURN:
    case VK_TAB:
    case VK_ESCAPE:
    case VK_LEFT:
    case VK_RIGHT:
    case VK_UP:
    case VK_DOWN:
    case VK_HOME:
    case VK_END:
    case VK_PRIOR:
    case VK_NEXT:
        _ClearPairedPunctuationStack();
        _CancelPairedPunctuationCaretMove();
        return;
    default:
        break;
    }

    if (!_pairedPunctuationStack.empty() && !_IsFocusSessionCurrent(_pairedPunctuationStack.back().focusToken))
    {
        _ClearPairedPunctuationStack();
    }
}

void CMetasequoiaIME::_CancelPairedPunctuationCaretMove()
{
    if (_pairedCaretRetryTimerActive && _msgWndHandle != nullptr)
    {
        KillTimer(_msgWndHandle, TIMER_PAIRED_PUNCTUATION_CARET);
    }
    _pairedCaretRetryTimerActive = false;
    _pendingPairedCaretDelta = 0;
    _pendingPairedCaretFocusToken = 0;
    _pendingPairedCaretDeadline = 0;
}

void CMetasequoiaIME::_QueuePairedPunctuationCaretMove(int delta)
{
    if (delta == 0 || _msgWndHandle == nullptr)
    {
        return;
    }

    const uint64_t focusToken = _CaptureFocusSessionToken();
    if (focusToken == 0)
    {
        return;
    }

    if (_pendingPairedCaretDelta != 0 && _pendingPairedCaretFocusToken == focusToken)
    {
        delta += _pendingPairedCaretDelta;
    }

    delta = max(-PAIRED_PUNCTUATION_CARET_MAX_STEPS, min(PAIRED_PUNCTUATION_CARET_MAX_STEPS, delta));
    if (delta == 0)
    {
        _CancelPairedPunctuationCaretMove();
        return;
    }

    _pendingPairedCaretDelta = delta;
    _pendingPairedCaretFocusToken = focusToken;
    _pendingPairedCaretDeadline = GetTickCount64() + PAIRED_PUNCTUATION_CARET_TIMEOUT_MS;

    if (!PostMessage(_msgWndHandle, WM_PairedPunctuationCaretMove, static_cast<WPARAM>(focusToken & 0xFFFFFFFFULL),
                     static_cast<LPARAM>((focusToken >> 32) & 0xFFFFFFFFULL)))
    {
        _CancelPairedPunctuationCaretMove();
    }
}

void CMetasequoiaIME::_RunPairedPunctuationCaretMove()
{
    if (_pendingPairedCaretDelta == 0)
    {
        _CancelPairedPunctuationCaretMove();
        return;
    }

    if (!_IsFocusSessionCurrent(_pendingPairedCaretFocusToken) || GetTickCount64() > _pendingPairedCaretDeadline)
    {
        _ClearPairedPunctuationStack();
        _CancelPairedPunctuationCaretMove();
        return;
    }

    if (AreCaretModifiersPhysicallyDown())
    {
        if (!_pairedCaretRetryTimerActive && _msgWndHandle != nullptr)
        {
            _pairedCaretRetryTimerActive = SetTimer(_msgWndHandle, TIMER_PAIRED_PUNCTUATION_CARET,
                                                    PAIRED_PUNCTUATION_CARET_RETRY_MS, nullptr) != 0;
            if (!_pairedCaretRetryTimerActive)
            {
                _ClearPairedPunctuationStack();
                _CancelPairedPunctuationCaretMove();
            }
        }
        return;
    }

    const int delta = _pendingPairedCaretDelta;
    const WORD vk = delta < 0 ? VK_LEFT : VK_RIGHT;
    const int steps = delta < 0 ? -delta : delta;

    INPUT inputs[PAIRED_PUNCTUATION_CARET_MAX_STEPS * 2] = {};
    for (int i = 0; i < steps; ++i)
    {
        inputs[i * 2].type = INPUT_KEYBOARD;
        inputs[i * 2].ki.wVk = vk;
        inputs[i * 2].ki.dwExtraInfo = PAIRED_PUNCTUATION_SENDINPUT_EXTRA_INFO;
        inputs[i * 2 + 1] = inputs[i * 2];
        inputs[i * 2 + 1].ki.dwFlags = KEYEVENTF_KEYUP;
    }

    _CancelPairedPunctuationCaretMove();
    if (SendInput(static_cast<UINT>(steps * 2), inputs, sizeof(INPUT)) != static_cast<UINT>(steps * 2))
    {
        _ClearPairedPunctuationStack();
    }
    _InvalidateSmartPunctuationShadow();
}

void CMetasequoiaIME::_ResetSmartPunctuationHistory()
{
    _ClearSmartPunctuationSpace();
    _ClearSmartPunctuationRevert();
    _smartPunctuationKey = 0;
    _smartPunctuationPrecedingChar = 0;
    _smartPunctuationCommittedAscii = false;
    _smartPunctuationAsciiRejected = false;
    _smartPunctuationCommitTick = 0;
    _smartPunctuationFocusToken = 0;
    _smartPunctuationForegroundWindow = nullptr;
}

void CMetasequoiaIME::_ClearSmartPunctuationAction()
{
    _ResetSmartPunctuationHistory();
    _pendingSmartPunctuationReplacement = 0;
    _pendingSmartPunctuationFocusToken = 0;
    _pendingSmartPunctuationForegroundWindow = nullptr;
    _pendingSmartPunctuationDeadline = 0;
}

bool CMetasequoiaIME::_QueueSmartPunctuationRewrite(WCHAR replacement)
{
    if (replacement == 0 || _msgWndHandle == nullptr)
    {
        return false;
    }
    const uint64_t focusToken = _CaptureFocusSessionToken();
    if (focusToken == 0 || !_IsFocusSessionCurrent(focusToken))
    {
        return false;
    }

    _pendingSmartPunctuationReplacement = replacement;
    _pendingSmartPunctuationFocusToken = focusToken;
    _pendingSmartPunctuationForegroundWindow = GetForegroundWindow();
    _pendingSmartPunctuationDeadline = GetTickCount64() + SMART_PUNCTUATION_REWRITE_DEADLINE_MS;

    if (!PostMessage(_msgWndHandle, WM_ReplaceRepeatedSmartPunctuation, static_cast<WPARAM>(focusToken & 0xFFFFFFFFULL),
                     static_cast<LPARAM>((focusToken >> 32) & 0xFFFFFFFFULL)))
    {
        _pendingSmartPunctuationReplacement = 0;
        _pendingSmartPunctuationFocusToken = 0;
        _pendingSmartPunctuationForegroundWindow = nullptr;
        _pendingSmartPunctuationDeadline = 0;
        return false;
    }
    return true;
}

bool CMetasequoiaIME::_QueueRepeatedSmartPunctuationReplacement(WCHAR wch)
{
    // Backspace rejection means the ASCII form is already gone. Treating the
    // next press as "replace the still-visible ASCII punct" would SendInput a
    // Backspace into the preceding character instead.
    if (!_smartPunctuationCommittedAscii || _smartPunctuationAsciiRejected || _smartPunctuationKey != wch ||
        _smartPunctuationCommitTick == 0 || _msgWndHandle == nullptr || _pCompositionProcessorEngine == nullptr ||
        _IsComposing() || _candidateMode != CANDIDATE_NONE ||
        !Global::SmartPunctuationEnabled.load(std::memory_order_relaxed) ||
        !Global::SmartPunctuationRepeatToChineseEnabled.load(std::memory_order_relaxed))
    {
        return false;
    }

    const ULONGLONG now = GetTickCount64();
    if (now - _smartPunctuationCommitTick > SMART_PUNCTUATION_REPEAT_INTERVAL_MS ||
        !_IsFocusSessionCurrent(_smartPunctuationFocusToken) ||
        GetForegroundWindow() != _smartPunctuationForegroundWindow)
    {
        return false;
    }

    const WCHAR *chinese = _pCompositionProcessorEngine->GetPunctuation(wch);
    if (chinese == nullptr || chinese[0] == L'\0' || chinese[1] != L'\0')
    {
        return false;
    }

    _pendingSmartPunctuationReplacement = chinese[0];
    _pendingSmartPunctuationFocusToken = _smartPunctuationFocusToken;
    _pendingSmartPunctuationForegroundWindow = _smartPunctuationForegroundWindow;
    _pendingSmartPunctuationDeadline = _smartPunctuationCommitTick + SMART_PUNCTUATION_REPEAT_INTERVAL_MS;

    const uint64_t focusToken = _pendingSmartPunctuationFocusToken;
    if (!PostMessage(_msgWndHandle, WM_ReplaceRepeatedSmartPunctuation, static_cast<WPARAM>(focusToken & 0xFFFFFFFFULL),
                     static_cast<LPARAM>((focusToken >> 32) & 0xFFFFFFFFULL)))
    {
        _pendingSmartPunctuationReplacement = 0;
        _pendingSmartPunctuationFocusToken = 0;
        _pendingSmartPunctuationForegroundWindow = nullptr;
        _pendingSmartPunctuationDeadline = 0;
        return false;
    }

    _ResetSmartPunctuationHistory();
    return true;
}

void CMetasequoiaIME::_InvalidateSmartPunctuationShadow()
{
    _smartPunctuationShadowChar = 0;
    _smartPunctuationShadowValid = false;
}

void CMetasequoiaIME::_UpdateSmartPunctuationShadow(UINT code, WCHAR wch, bool isEaten)
{
    switch (code)
    {
    case VK_SHIFT:
    case VK_LSHIFT:
    case VK_RSHIFT:
    case VK_CONTROL:
    case VK_LCONTROL:
    case VK_RCONTROL:
    case VK_MENU:
    case VK_LMENU:
    case VK_RMENU:
    case VK_CAPITAL:
        // Modifier presses edit nothing, so the shadow still describes the caret.
        return;
    case VK_BACK:
    case VK_DELETE:
    case VK_INSERT:
    case VK_RETURN:
    case VK_TAB:
    case VK_ESCAPE:
    case VK_LEFT:
    case VK_RIGHT:
    case VK_UP:
    case VK_DOWN:
    case VK_HOME:
    case VK_END:
    case VK_PRIOR:
    case VK_NEXT:
        _InvalidateSmartPunctuationShadow();
        return;
    default:
        break;
    }

    if (CCompositionProcessorEngine::IsSmartAsciiPunctuationKey(wch))
    {
        // _ResolveSmartPunctuation needs the current shadow to decide the form,
        // and records whatever it commits once that decision is made.
        return;
    }

    if (isEaten || wch == 0 || std::iswprint(static_cast<wint_t>(wch)) == 0)
    {
        // Eaten keys feed the composition and reach the document as committed
        // text, which even a proxy store exposes, so let the document answer.
        _InvalidateSmartPunctuationShadow();
        return;
    }

    _smartPunctuationShadowChar = wch;
    _smartPunctuationShadowValid = true;
}

void CMetasequoiaIME::_NoteKeyForSmartPunctuation(UINT code, WCHAR wch, bool isEaten)
{
    // The replacement message normally runs before another input event. If it
    // does not, never let a later key leave the queued Backspace targeting an
    // unrelated character.
    if (_pendingSmartPunctuationReplacement != 0)
    {
        _pendingSmartPunctuationReplacement = 0;
        _pendingSmartPunctuationFocusToken = 0;
        _pendingSmartPunctuationForegroundWindow = nullptr;
        _pendingSmartPunctuationDeadline = 0;
    }

    if (_smartPunctuationSpaceArmed && !_CanConvertSmartPunctuationSpace())
    {
        _ClearSmartPunctuationSpace();
    }
    if (_smartPunctuationRevertArmed && !_CanRevertSmartPunctuation(wch))
    {
        _ClearSmartPunctuationRevert();
    }

    // Preserve the local conversion arm through OnTestKeyDown and OnKeyDown;
    // the edit session consumes it after both key-sink passes complete.
    if (code == VK_SPACE && _CanConvertSmartPunctuationSpace())
    {
        return;
    }
    if (_CanRevertSmartPunctuation(wch))
    {
        return;
    }

    _UpdateSmartPunctuationShadow(code, wch, isEaten);
    // Self-generated caret moves are filtered by the key sinks before they can
    // reach this bookkeeping path.
    _NoteKeyForPairedPunctuation(code);

    if (_smartPunctuationKey == 0)
    {
        return;
    }

    switch (code)
    {
    case VK_SHIFT:
    case VK_LSHIFT:
    case VK_RSHIFT:
    case VK_CONTROL:
    case VK_LCONTROL:
    case VK_RCONTROL:
    case VK_MENU:
    case VK_LMENU:
    case VK_RMENU:
    case VK_CAPITAL:
        // ':' needs Shift; modifier presses are not edits.
        return;
    case VK_BACK:
        // Only deleting the ASCII form says that form was unwanted. Deleting
        // the Chinese punctuation that replaced it must not undo the rejection.
        if (_smartPunctuationCommittedAscii)
        {
            _smartPunctuationAsciiRejected = true;
            // ASCII punct is gone; disarm repeat-to-Chinese replacement so a
            // quick retype takes the reject path instead of SendInput(VK_BACK).
            _smartPunctuationCommitTick = 0;
            _smartPunctuationFocusToken = 0;
            _smartPunctuationForegroundWindow = nullptr;
            // UpdateShadow already cleared the punctuation shadow. Restore the
            // preceding character recorded at commit so a retype can still match
            // the reject spot when the host text store cannot re-read it.
            if (_smartPunctuationPrecedingChar != 0)
            {
                _smartPunctuationShadowChar = _smartPunctuationPrecedingChar;
                _smartPunctuationShadowValid = true;
            }
        }
        return;
    case VK_DECIMAL:
        // Numpad '.' bypasses smart punctuation entirely.
        _ResetSmartPunctuationHistory();
        return;
    default:
        break;
    }

    if (wch != _smartPunctuationKey)
    {
        _ResetSmartPunctuationHistory();
    }
}

std::wstring CMetasequoiaIME::_ResolveSmartPunctuation(WCHAR wch, WCHAR precedingChar)
{
    if (_pCompositionProcessorEngine == nullptr)
    {
        return {};
    }

    const bool smartEnabled = Global::SmartPunctuationEnabled.load(std::memory_order_relaxed);
    std::wstring resolved = _pCompositionProcessorEngine->ResolvePunctuation(wch, precedingChar);
    if (!CCompositionProcessorEngine::IsSmartAsciiPunctuationKey(wch) || !smartEnabled)
    {
        _ResetSmartPunctuationHistory();
        return resolved;
    }

    bool committedAscii = resolved.size() == 1 && resolved[0] == wch;
    // The rejection is sticky for as long as this spot survives, so repeated
    // delete/retype cycles keep producing Chinese punctuation.
    const bool asciiRejected = _smartPunctuationAsciiRejected && _smartPunctuationKey == wch &&
                               _smartPunctuationPrecedingChar == precedingChar;
    if (asciiRejected && committedAscii)
    {
        const WCHAR *chinese = _pCompositionProcessorEngine->GetPunctuation(wch);
        if (chinese != nullptr && *chinese != L'\0')
        {
            resolved.assign(chinese);
            committedAscii = false;
        }
    }

    _smartPunctuationKey = wch;
    _smartPunctuationPrecedingChar = precedingChar;
    _smartPunctuationCommittedAscii = committedAscii;
    _smartPunctuationAsciiRejected = asciiRejected;
    if (committedAscii)
    {
        _smartPunctuationCommitTick = GetTickCount64();
        _smartPunctuationFocusToken = _CaptureFocusSessionToken();
        _smartPunctuationForegroundWindow = GetForegroundWindow();
    }
    else
    {
        _smartPunctuationCommitTick = 0;
        _smartPunctuationFocusToken = 0;
        _smartPunctuationForegroundWindow = nullptr;
    }
    if (!resolved.empty())
    {
        _smartPunctuationShadowChar = resolved.back();
        _smartPunctuationShadowValid = true;
    }
    return resolved;
}

//+---------------------------------------------------------------------------
//
// ITfCompositionSink::OnCompositionTerminated
//
// Callback for ITfCompositionSink.  The system calls this method whenever
// someone other than this service ends a composition.
//----------------------------------------------------------------------------

STDAPI CMetasequoiaIME::OnCompositionTerminated(TfEditCookie ecWrite, _In_ ITfComposition *pComposition)
{
    PerfTimer timer;
    if (pComposition == nullptr || !_IsCompositionCurrent(pComposition))
    {
        DebugTsfIssue47(L"host-terminated-stale-composition", FANY_IME_NO_REQUEST_ID, 0, L'\0', 0, 0, -1,
                        _IsComposing(),
                        _pCompositionProcessorEngine ? _pCompositionProcessorEngine->GetVirtualKeyLength() : 0, S_FALSE,
                        _CaptureCompositionEpoch());
        // A delayed termination callback for an older composition must never
        // delete the current candidate presenter or release newer ownership.
        return S_OK;
    }

    DebugTsfIssue47(L"host-terminated-current-composition", FANY_IME_NO_REQUEST_ID, 0, L'\0', 0, 0, -1, TRUE,
                    _pCompositionProcessorEngine ? _pCompositionProcessorEngine->GetVirtualKeyLength() : 0, S_OK,
                    _CaptureCompositionEpoch());

    // The callback already carries a write cookie and the host has already
    // ended this exact composition. Detach ownership before making COM calls,
    // so a re-entrant/stale callback cannot observe it as current or tear down
    // a composition created later.
    ITfComposition *terminatedComposition = pComposition;
    terminatedComposition->AddRef();
    _pComposition->Release();
    _pComposition = nullptr;
    _voiceCompositionActive = false;
    Global::ResetForcedOverlayAnchor();

    ITfContext *ownerContext = _pContext;
    if (ownerContext)
    {
        ownerContext->AddRef();
        _pContext->Release();
        _pContext = nullptr;
    }

    uint64_t nextEpoch = _compositionEpoch.fetch_add(1, std::memory_order_acq_rel) + 1;
    if (nextEpoch == 0)
    {
        _compositionEpoch.fetch_add(1, std::memory_order_acq_rel);
    }

    // 应用结束韩文、注音、越南文或藏文的组字时，文字留在文档里（scheme::AlwaysInlinePreedit 不论预编辑偏好如何都把它画在那里）。宿主会话也必须放开它，否则下一个键会在这段文字上继续组字，再上屏一次。TIP 自己结束的组字已经处理过宿主，那时宿主可能已经在组下一段。
    if (!_terminatingOwnComposition &&
        msime::windows::scheme::AlwaysInlinePreedit(Global::InputModeScheme.load(std::memory_order_relaxed)))
    {
        (void)_CancelHostComposition();
    }

    // Detach and end the old candidate/session before the COM cleanup calls
    // below can re-enter and create a presenter for a newer composition.
    _DeleteCandidateList(FALSE, ownerContext);
    if (Global::g_connected)
    {
        // EndCandidateUiSession is intentionally idempotent, but a presenter
        // can exist before its UI session becomes active. Always send one
        // exact routed clear so the Server cannot retain that composition.
        SendHideCandidateWndEventToUIProcess();
    }

    // Do NOT SetText(empty) here. Cancel paths already wipe via
    // _HandleCancel → _RemoveDummyCompositionForComposing. Wiping again after
    // a normal commit/EndComposition can delete the just-committed text and
    // destabilize fragile hosts (notably QQ).
    if (ownerContext)
    {
        _ClearCompositionDisplayAttributes(ecWrite, ownerContext, terminatedComposition);
    }
    terminatedComposition->Release();

    if (ownerContext)
    {
        ownerContext->Release();
    }

    return S_OK;
}

//+---------------------------------------------------------------------------
//
// _IsComposing
//
//----------------------------------------------------------------------------

BOOL CMetasequoiaIME::_IsComposing()
{
    return _pComposition != nullptr;
}

//+---------------------------------------------------------------------------
//
// _SetComposition
//
//----------------------------------------------------------------------------

void CMetasequoiaIME::_SetComposition(_In_ ITfComposition *pComposition)
{
    _pComposition = pComposition;
    uint64_t nextEpoch = _compositionEpoch.fetch_add(1, std::memory_order_acq_rel) + 1;
    if (nextEpoch == 0)
    {
        _compositionEpoch.fetch_add(1, std::memory_order_acq_rel);
    }
}

//+---------------------------------------------------------------------------
//
// _AddComposingAndChar
//
//----------------------------------------------------------------------------

HRESULT CMetasequoiaIME::_AddComposingAndChar(TfEditCookie ec, _In_ ITfContext *pContext,
                                              _In_ CStringRange *pstrAddString)
{
    HRESULT hr = S_OK;

    if (_pComposition != nullptr)
    {
        PerfTimer fastUpdateTimer;
        ITfRange *pRangeComposition = nullptr;
        hr = _pComposition->GetRange(&pRangeComposition);
        if (SUCCEEDED(hr) && pRangeComposition != nullptr)
        {
            PerfTimer setTextTimer;
            hr = SafeRangeSetText(pRangeComposition, ec, 0, pstrAddString->Get(), (LONG)pstrAddString->GetLength());
            double setTextElapsedMs = setTextTimer.ElapsedMs();
            if (SUCCEEDED(hr))
            {
                PerfTimer displayAttrTimer;
                _SetCompositionDisplayAttributesForRange(ec, pContext, pRangeComposition, _gaDisplayAttributeInput);
                double displayAttrElapsedMs = displayAttrTimer.ElapsedMs();

                PerfTimer selectionTimer;
                TF_SELECTION sel;
                pRangeComposition->Collapse(ec, TF_ANCHOR_END);
                sel.range = pRangeComposition;
                sel.style.ase = TF_AE_NONE;
                sel.style.fInterimChar = FALSE;
                pContext->SetSelection(ec, 1, &sel);
                double selectionElapsedMs = selectionTimer.ElapsedMs();

                pRangeComposition->Release();
                return hr;
            }
            pRangeComposition->Release();
        }
    }

    ULONG fetched = 0;
    TF_SELECTION tfSelection;

    if (pContext->GetSelection(ec, TF_DEFAULT_SELECTION, 1, &tfSelection, &fetched) != S_OK || fetched == 0)
        return S_FALSE;

    //
    // make range start to selection
    //
    ITfRange *pAheadSelection = nullptr;
    hr = pContext->GetStart(ec, &pAheadSelection);
    if (SUCCEEDED(hr))
    {
        hr = pAheadSelection->ShiftEndToRange(ec, tfSelection.range, TF_ANCHOR_START);
        if (SUCCEEDED(hr))
        {
            ITfRange *pRange = nullptr;
            BOOL exist_composing = _FindComposingRange(ec, pContext, pAheadSelection, &pRange);

            std::wstring strAddString(pstrAddString->Get(), pstrAddString->GetLength());

            _SetInputString(ec, pContext, pRange, pstrAddString, exist_composing);

            if (pRange)
            {
                pRange->Release();
            }
        }
    }

    tfSelection.range->Release();

    if (pAheadSelection)
    {
        pAheadSelection->Release();
    }

    return S_OK;
}

//+---------------------------------------------------------------------------
//
// _AddCharAndFinalize
//
//----------------------------------------------------------------------------

HRESULT CMetasequoiaIME::_AddCharAndFinalize(TfEditCookie ec, _In_ ITfContext *pContext,
                                             _In_ CStringRange *pstrAddString)
{
    HRESULT hr = E_FAIL;
    PerfTimer timer;

    if (_pComposition != nullptr)
    {
        PerfTimer directSetTimer;
        hr = _SetCompositionTextAndSelection(ec, pContext, pstrAddString);
        double directSetElapsedMs = directSetTimer.ElapsedMs();
        if (SUCCEEDED(hr))
        {
            return hr;
        }
    }

    ULONG fetched = 0;
    TF_SELECTION tfSelection;

    if ((hr = pContext->GetSelection(ec, TF_DEFAULT_SELECTION, 1, &tfSelection, &fetched)) != S_OK || fetched != 1)
        return hr;

    // We use SetText here instead of InsertTextAtSelection because we've already started a composition
    // We don't want to the app to adjust the insertion point inside our composition
    PerfTimer setTextTimer;
    hr = SafeRangeSetText(tfSelection.range, ec, 0, pstrAddString->Get(), (LONG)pstrAddString->GetLength());
    double setTextElapsedMs = setTextTimer.ElapsedMs();
    double setSelectionElapsedMs = 0;
    if (hr == S_OK)
    {
        // Update the selection, we'll make it an insertion point just past
        // the inserted text.
        tfSelection.range->Collapse(ec, TF_ANCHOR_END);
        PerfTimer setSelectionTimer;
        pContext->SetSelection(ec, 1, &tfSelection);
        setSelectionElapsedMs = setSelectionTimer.ElapsedMs();
    }

    tfSelection.range->Release();

    return hr;
}

HRESULT CMetasequoiaIME::_InsertTextToComposition(TfEditCookie ec, _In_ ITfContext *pContext,
                                                  _In_ CStringRange *pstrAddString)
{
    PerfTimer timer;
    if (_pComposition == nullptr)
    {
        return E_FAIL;
    }

    ITfRange *pRangeComposition = nullptr;
    PerfTimer getRangeTimer;
    HRESULT hr = _pComposition->GetRange(&pRangeComposition);
    double getRangeElapsedMs = getRangeTimer.ElapsedMs();
    if (FAILED(hr) || pRangeComposition == nullptr)
    {
        return FAILED(hr) ? hr : E_FAIL;
    }

    PerfTimer setTextTimer;
    hr = SafeRangeSetText(pRangeComposition, ec, 0, pstrAddString->Get(), (LONG)pstrAddString->GetLength());
    double setTextElapsedMs = setTextTimer.ElapsedMs();
    double setSelectionElapsedMs = 0;
    if (SUCCEEDED(hr))
    {
        TF_SELECTION tfSelection;
        pRangeComposition->Collapse(ec, TF_ANCHOR_END);
        tfSelection.range = pRangeComposition;
        tfSelection.style.ase = TF_AE_NONE;
        tfSelection.style.fInterimChar = FALSE;
        PerfTimer setSelectionTimer;
        pContext->SetSelection(ec, 1, &tfSelection);
        setSelectionElapsedMs = setSelectionTimer.ElapsedMs();
    }

    pRangeComposition->Release();
    return hr;
}

//+---------------------------------------------------------------------------
//
// _SetCompositionTextAndSelection
//
//----------------------------------------------------------------------------

HRESULT CMetasequoiaIME::_SetCompositionTextAndSelection(TfEditCookie ec, _In_ ITfContext *pContext,
                                                         _In_ CStringRange *pstrAddString)
{
    PerfTimer timer;
    if (_pComposition == nullptr)
    {
        return E_FAIL;
    }

    ITfRange *pRangeComposition = nullptr;
    PerfTimer getRangeTimer;
    HRESULT hr = _pComposition->GetRange(&pRangeComposition);
    double getRangeElapsedMs = getRangeTimer.ElapsedMs();
    if (FAILED(hr) || pRangeComposition == nullptr)
    {
        return FAILED(hr) ? hr : E_FAIL;
    }

    PerfTimer setTextTimer;
    hr = SafeRangeSetText(pRangeComposition, ec, 0, pstrAddString->Get(), (LONG)pstrAddString->GetLength());
    double setTextElapsedMs = setTextTimer.ElapsedMs();
    double setSelectionElapsedMs = 0;
    if (SUCCEEDED(hr))
    {
        TF_SELECTION tfSelection;
        pRangeComposition->Collapse(ec, TF_ANCHOR_END);
        tfSelection.range = pRangeComposition;
        tfSelection.style.ase = TF_AE_NONE;
        tfSelection.style.fInterimChar = FALSE;
        PerfTimer setSelectionTimer;
        pContext->SetSelection(ec, 1, &tfSelection);
        setSelectionElapsedMs = setSelectionTimer.ElapsedMs();
    }

    pRangeComposition->Release();
    return hr;
}

//+---------------------------------------------------------------------------
//
// _FindComposingRange
//
//----------------------------------------------------------------------------

BOOL CMetasequoiaIME::_FindComposingRange(TfEditCookie ec, _In_ ITfContext *pContext, _In_ ITfRange *pSelection,
                                          _Outptr_result_maybenull_ ITfRange **ppRange)
{
    if (ppRange == nullptr)
    {
        return FALSE;
    }

    *ppRange = nullptr;

    // find GUID_PROP_COMPOSING
    ITfProperty *pPropComp = nullptr;
    IEnumTfRanges *enumComp = nullptr;

    HRESULT hr = pContext->GetProperty(GUID_PROP_COMPOSING, &pPropComp);
    if (FAILED(hr) || pPropComp == nullptr)
    {
        return FALSE;
    }

    hr = pPropComp->EnumRanges(ec, &enumComp, pSelection);
    if (FAILED(hr) || enumComp == nullptr)
    {
        pPropComp->Release();
        return FALSE;
    }

    BOOL isCompExist = FALSE;
    VARIANT var;
    ULONG fetched = 0;

    while (enumComp->Next(1, ppRange, &fetched) == S_OK && fetched == 1)
    {
        hr = pPropComp->GetValue(ec, *ppRange, &var);
        if (hr == S_OK)
        {
            if (var.vt == VT_I4 && var.lVal != 0)
            {
                isCompExist = TRUE;
                break;
            }
        }
        (*ppRange)->Release();
        *ppRange = nullptr;
    }

    pPropComp->Release();
    enumComp->Release();

    return isCompExist;
}

//+---------------------------------------------------------------------------
//
// _SetInputString
//
//----------------------------------------------------------------------------

HRESULT CMetasequoiaIME::_SetInputString(TfEditCookie ec, _In_ ITfContext *pContext, _Out_opt_ ITfRange *pRange,
                                         _In_ CStringRange *pstrAddString, BOOL exist_composing)
{
    ITfRange *pRangeInsert = nullptr;
    if (!exist_composing)
    {
        _InsertAtSelection(ec, pContext, pstrAddString, &pRangeInsert);
        if (pRangeInsert == nullptr)
        {
            return S_OK;
        }
        else
        {
            // pRange = pRangeInsert;

            /* To make TsfPad work, we need to get range manually */
            _pComposition->GetRange(&pRange);
        }
    }
    if (pRange != nullptr)
    {
        SafeRangeSetText(pRange, ec, 0, pstrAddString->Get(), (LONG)pstrAddString->GetLength());
    }

    _SetCompositionLanguage(ec, pContext);

    _SetCompositionDisplayAttributes(ec, pContext, _gaDisplayAttributeInput);

    // update the selection, we'll make it an insertion point just past
    // the inserted text.
    ITfRange *pSelection = nullptr;
    TF_SELECTION sel;

    if ((pRange != nullptr) && (pRange->Clone(&pSelection) == S_OK))
    {
        pSelection->Collapse(ec, TF_ANCHOR_END);

        sel.range = pSelection;
        sel.style.ase = TF_AE_NONE;
        sel.style.fInterimChar = FALSE;
        pContext->SetSelection(ec, 1, &sel);
        pSelection->Release();
    }

    if (pRangeInsert)
    {
        pRangeInsert->Release();
    }

    return S_OK;
}

//+---------------------------------------------------------------------------
//
// _InsertAtSelection
//
//----------------------------------------------------------------------------

HRESULT CMetasequoiaIME::_InsertAtSelection(TfEditCookie ec, _In_ ITfContext *pContext,
                                            _In_ CStringRange *pstrAddString, _Outptr_ ITfRange **ppCompRange)
{
    ITfRange *rangeInsert = nullptr;
    ITfInsertAtSelection *pias = nullptr;
    HRESULT hr = S_OK;

    if (ppCompRange == nullptr)
    {
        hr = E_INVALIDARG;
        goto Exit;
    }

    *ppCompRange = nullptr;

    hr = pContext->QueryInterface(IID_ITfInsertAtSelection, (void **)&pias);
    if (FAILED(hr))
    {
        goto Exit;
    }

    hr = pias->InsertTextAtSelection(ec, TF_IAS_QUERYONLY, pstrAddString->Get(), (LONG)pstrAddString->GetLength(),
                                     &rangeInsert);

    if (FAILED(hr) || rangeInsert == nullptr)
    {
        rangeInsert = nullptr;
        pias->Release();
        goto Exit;
    }

    *ppCompRange = rangeInsert;
    pias->Release();
    hr = S_OK;

Exit:
    return hr;
}

//+---------------------------------------------------------------------------
//
// _RemoveDummyCompositionForComposing
//
//----------------------------------------------------------------------------

HRESULT CMetasequoiaIME::_RemoveDummyCompositionForComposing(TfEditCookie ec, _In_ ITfComposition *pComposition)
{
    HRESULT hr = S_OK;

    ITfRange *pRange = nullptr;

    if (pComposition)
    {
        hr = pComposition->GetRange(&pRange);
        if (SUCCEEDED(hr))
        {
            hr = SafeRangeSetText(pRange, ec, 0, nullptr, 0);
            pRange->Release();
        }
    }

    return hr;
}

//+---------------------------------------------------------------------------
//
// _SetCompositionLanguage
//
//----------------------------------------------------------------------------

BOOL CMetasequoiaIME::_SetCompositionLanguage(TfEditCookie ec, _In_ ITfContext *pContext)
{
    HRESULT hr = S_OK;
    BOOL ret = TRUE;

    CCompositionProcessorEngine *pCompositionProcessorEngine = nullptr;
    pCompositionProcessorEngine = _pCompositionProcessorEngine;

    LANGID langidProfile = 0;
    pCompositionProcessorEngine->GetLanguageProfile(&langidProfile);

    ITfRange *pRangeComposition = nullptr;
    ITfProperty *pLanguageProperty = nullptr;

    // we need a range and the context it lives in
    hr = _pComposition->GetRange(&pRangeComposition);
    if (FAILED(hr))
    {
        ret = FALSE;
        goto Exit;
    }

    // get our the language property
    hr = pContext->GetProperty(GUID_PROP_LANGID, &pLanguageProperty);
    if (FAILED(hr))
    {
        ret = FALSE;
        goto Exit;
    }

    VARIANT var;
    var.vt = VT_I4; // we're going to set DWORD
    var.lVal = langidProfile;

    hr = pLanguageProperty->SetValue(ec, pRangeComposition, &var);
    if (FAILED(hr))
    {
        ret = FALSE;
        goto Exit;
    }

    pLanguageProperty->Release();
    pRangeComposition->Release();

Exit:
    return ret;
}
