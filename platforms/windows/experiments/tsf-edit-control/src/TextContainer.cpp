#include "TextContainer.h"

#include <algorithm>

//----------------------------------------------------------------
//
//
//
//----------------------------------------------------------------

BOOL CTextContainer::InsertText(int nPos, const WCHAR *psz, UINT nCnt)
{
    if (nPos < 0 || static_cast<UINT>(nPos) > _nTextSize)
        return FALSE;

    if (!nCnt)
        return TRUE;

    if (!psz || nCnt > kMaxTextUnits - _nTextSize)
        return FALSE;

    if (!EnsureBuffer(_nTextSize + nCnt))
    {
        return FALSE;
    }

    memmove(_psz + nPos + nCnt, _psz + nPos, (_nTextSize - nPos) * sizeof(WCHAR));
    memcpy(_psz + nPos, psz, nCnt * sizeof(WCHAR));
    _nTextSize += nCnt;
    return TRUE;
}

//----------------------------------------------------------------
//
//
//
//----------------------------------------------------------------

BOOL CTextContainer::RemoveText(int nPos, UINT nCnt)
{
    if (nPos < 0 || static_cast<UINT>(nPos) > _nTextSize)
        return FALSE;

    nCnt = std::min(nCnt, _nTextSize - static_cast<UINT>(nPos));
    if (!nCnt)
        return TRUE;

    memmove(_psz + nPos, _psz + nPos + nCnt,
            (_nTextSize - static_cast<UINT>(nPos) - nCnt) * sizeof(WCHAR));
    _nTextSize -= nCnt;
    return TRUE;
}

//----------------------------------------------------------------
//
//
//
//----------------------------------------------------------------

BOOL CTextContainer::GetText(int nPos, WCHAR *psz, UINT nCnt)
{
    if (!nCnt || !psz || nPos < 0 || static_cast<UINT>(nPos) > _nTextSize)
        return FALSE;

    nCnt = std::min(nCnt, _nTextSize - static_cast<UINT>(nPos));

    if (nCnt)
        memcpy(psz, _psz + nPos, nCnt * sizeof(WCHAR));

    return TRUE;
}

//----------------------------------------------------------------
//
//
//
//----------------------------------------------------------------

BOOL CTextContainer::EnsureBuffer(UINT nNewTextSize)
{
    if (!nNewTextSize)
    {
        if (_psz)
            LocalFree(_psz);
        _psz = NULL;
        _nBufferSize = 0;
        _nTextSize = 0;
        return FALSE;
    }

    if (nNewTextSize > kMaxTextUnits ||
        static_cast<size_t>(nNewTextSize) > std::numeric_limits<size_t>::max() / sizeof(WCHAR))
        return FALSE;

    if (nNewTextSize <= _nTextSize)
        goto Exit;

    const size_t bytes = static_cast<size_t>(nNewTextSize) * sizeof(WCHAR);

    if (_psz)
    {
        void *pvNew = LocalReAlloc(_psz, bytes, LMEM_MOVEABLE | LMEM_ZEROINIT);
        if (!pvNew)
            return FALSE;

        _psz = (WCHAR *)pvNew;
    }
    else
    {
        _psz = (WCHAR *)LocalAlloc(LPTR, bytes);
        if (!_psz)
            return FALSE;
    }
    _nBufferSize = nNewTextSize;
Exit:
    return TRUE;
}
