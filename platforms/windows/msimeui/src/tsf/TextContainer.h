#pragma once

#include <Windows.h>

#include <cstddef>
#include <limits>

//----------------------------------------------------------------
//
//
//
//----------------------------------------------------------------

class CTextContainer
{
  public:
    // Keep the TSF text store bounded to the same UTF-16 size accepted by the
    // native text box clipboard contract.  This also keeps all ACP positions
    // representable as non-negative LONG values.
    static constexpr UINT kMaxTextUnits = 999'999;

    CTextContainer()
    {
        _psz = NULL;
        _nBufferSize = 0;
        _nTextSize = 0;
    }

    ~CTextContainer()
    {
        if (_psz)
            LocalFree(_psz);
    }

    BOOL InsertText(int nPos, const WCHAR *psz, UINT nCnt);
    BOOL RemoveText(int nPos, UINT nCnt);
    BOOL GetText(int nPos, WCHAR *psz, UINT nBuffSize);

    UINT GetTextLength()
    {
        return _nTextSize;
    }
    const WCHAR *GetTextBuffer()
    {
        return _psz;
    }
    const WCHAR *GetTextBuffer() const
    {
        return _psz;
    }

  protected:
    virtual void OnTextChanged()
    {
    }

  private:
    BOOL EnsureBuffer(UINT nNewTextSize);

    WCHAR *_psz;
    UINT _nBufferSize;
    UINT _nTextSize;
};
