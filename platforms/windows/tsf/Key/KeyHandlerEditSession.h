#pragma once

#include "EditSession.h"
#include "Globals.h"
#include <cstdint>
#include <string>
#include <utility>
#include <optional>

class CKeyHandlerEditSession : public CEditSessionBase
{
  public:
    CKeyHandlerEditSession(CMetasequoiaIME *pTextService, ITfContext *pContext, UINT uCode, WCHAR wch,
                           _KEYSTROKE_STATE keyState, uint64_t requestId, LARGE_INTEGER requestStartQpc = {},
                           std::wstring prefetchedText = {}, uint64_t focusToken = 0, UINT localResetToken = 0,
                           uint64_t expectedCompositionEpoch = 0, uint64_t deferredReplayToken = 0,
                           std::optional<std::size_t> cursorSuffix = std::nullopt)
        : CEditSessionBase(pTextService, pContext)
    {
        _uCode = uCode;
        _wch = wch;
        _KeyState = keyState;
        _requestId = requestId;
        _focusToken = focusToken;
        _localResetToken = localResetToken;
        _expectedCompositionEpoch = expectedCompositionEpoch;
        _deferredReplayToken = deferredReplayToken;
        _prefetchedText = std::move(prefetchedText);
        _cursorSuffix = cursorSuffix;
        _requestStartQpc = requestStartQpc;
        if (_requestStartQpc.QuadPart == 0)
        {
            QueryPerformanceCounter(&_requestStartQpc);
        }
    }

    // ITfEditSession
    STDMETHODIMP DoEditSession(TfEditCookie ec);

  private:
    UINT _uCode;                // virtual key code
    WCHAR _wch;                 // character code
    _KEYSTROKE_STATE _KeyState; // key function regarding virtual key
    uint64_t _requestId;
    uint64_t _focusToken;
    UINT _localResetToken;
    uint64_t _expectedCompositionEpoch;
    uint64_t _deferredReplayToken;
    std::wstring _prefetchedText;
    std::optional<std::size_t> _cursorSuffix;
    LARGE_INTEGER _requestStartQpc;
};
