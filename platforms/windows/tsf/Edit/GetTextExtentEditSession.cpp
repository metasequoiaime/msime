#include "Private.h"
#include "EditSession.h"
#include "GetTextExtentEditSession.h"
#include "TfTextLayoutSink.h"
#include "Ipc.h"
#include "FanyDefines.h"
#include <debugapi.h>
#include <fmt/xchar.h>
#include "../Utils/PerfTimer.h"

//+---------------------------------------------------------------------------
//
// ctor
//
//----------------------------------------------------------------------------

CGetTextExtentEditSession::CGetTextExtentEditSession(_In_ CMetasequoiaIME *pTextService, _In_ ITfContext *pContext,
                                                     _In_ ITfContextView *pContextView,
                                                     _In_ ITfRange *pRangeComposition,
                                                     _In_ CTfTextLayoutSink *pTfTextLayoutSink)
    : CEditSessionBase(pTextService, pContext)
{
    _pContextView = pContextView;
    _pRangeComposition = pRangeComposition;
    _pTfTextLayoutSink = pTfTextLayoutSink;
}

//+---------------------------------------------------------------------------
//
// ITfEditSession::DoEditSession
//
//----------------------------------------------------------------------------

STDAPI CGetTextExtentEditSession::DoEditSession(TfEditCookie ec)
{
    PerfTimer timer;
    RECT rc = {0, 0, 0, 0};
    BOOL isClipped = TRUE;
    HRESULT hr = S_OK;
    double getTextExtElapsedMs = 0;
    double layoutChangeElapsedMs = 0;

    if (_pTfTextLayoutSink->_IsDetached())
    {
        return S_OK;
    }

    PerfTimer getTextExtTimer;
    hr = _pContextView->GetTextExt(ec, _pRangeComposition, &rc, &isClipped);
    getTextExtElapsedMs = getTextExtTimer.ElapsedMs();

    if (SUCCEEDED(hr))
    {
        // Office may set isClipped with a usable rect; ignore only empty extents.
        if (isClipped && (rc.right <= rc.left || rc.bottom <= rc.top))
        {
            if (_pTfTextLayoutSink->_HasValidAnchor())
            {
                // 同一次组字里已经量到过锚点，这次量到空 extent（Firefox 在 textarea 内部滚动、
                // 换行时就这样答）只是这一段还没排好版。保留当前锚点，不清成哨兵值：清掉之后
                // 候选要等这个会话里再有一次成功测量才回来，持续输入时可能整段都不显示。
                if (Global::TsfDiagnosticLogEnabled.load(std::memory_order_relaxed))
                {
                    QueueTsfDiagnosticLog(fmt::format(
                        L"[candidate-layout] per-key empty clipped extent, kept anchor y={} rect={},{},{},{}",
                        Global::Point[1], rc.left, rc.top, rc.right, rc.bottom));
                }
                return S_OK;
            }
            Global::Point[0] = 0;
            Global::Point[1] = Global::INVALID_Y;
            if (Global::TsfDiagnosticLogEnabled.load(std::memory_order_relaxed))
            {
                QueueTsfDiagnosticLog(fmt::format(
                    L"[candidate-layout] per-key empty clipped extent, no usable anchor yet, hid rect={},{},{},{}",
                    rc.left, rc.top, rc.right, rc.bottom));
            }
            return S_OK;
        }

        const POINT anchor = GetPhysicalTextAnchor(_pContextView, rc);
        Global::Point[0] = anchor.x;
        Global::Point[1] = anchor.y;
        // This is the per-keystroke caret source. Tell the sink the session now
        // has a usable anchor, so a later transient TS_E_NOLAYOUT can be answered
        // by keeping this position instead of parking the window off-screen.
        _pTfTextLayoutSink->_MarkAnchorValid();
        if (Global::current_process_name == Global::ZEN_BROWSER)
        {
            Global::firefox_like_cnt++;
            if (Global::firefox_like_cnt == 3)
            {
                PerfTimer layoutChangeTimer;
                _pTfTextLayoutSink->_LayoutChangeNotification(&rc);
                layoutChangeElapsedMs = layoutChangeTimer.ElapsedMs();
            }
        }
        else
        {
            PerfTimer layoutChangeTimer;
            _pTfTextLayoutSink->_LayoutChangeNotification(&rc);
            layoutChangeElapsedMs = layoutChangeTimer.ElapsedMs();
        }
    }
    else if (Global::TsfDiagnosticLogEnabled.load(std::memory_order_relaxed))
    {
        // 失败时不写锚点，沿用上一次量到的位置；这一行只为区分「测量失败」和「量到空 extent」。
        QueueTsfDiagnosticLog(
            fmt::format(L"[candidate-layout] per-key extent query failed hr=0x{:08X} clipped={} rect={},{},{},{}",
                        static_cast<unsigned>(hr), isClipped ? 1 : 0, rc.left, rc.top, rc.right, rc.bottom));
    }

    return S_OK;
}
