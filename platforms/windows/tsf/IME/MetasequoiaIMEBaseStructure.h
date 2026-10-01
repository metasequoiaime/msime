#pragma once

#include "stdafx.h"
#include "../Candidate/CandidateListItem.h"
#include <vector>
#include "assert.h"
#include <iostream>

using std::endl;

//---------------------------------------------------------------------
// defined keyword
//---------------------------------------------------------------------
template <class VALUE> struct _DEFINED_KEYWORD
{
    LPCWSTR _pwszKeyword;
    VALUE _value;
};

//---------------------------------------------------------------------
// enum
//---------------------------------------------------------------------
enum KEYSTROKE_CATEGORY
{
    CATEGORY_NONE = 0,
    CATEGORY_COMPOSING,
    CATEGORY_CANDIDATE,
    CATEGORY_INVOKE_COMPOSITION_EDIT_SESSION
};

enum KEYSTROKE_FUNCTION
{
    FUNCTION_NONE = 0,
    FUNCTION_INPUT,

    FUNCTION_CANCEL,
    FUNCTION_TOGGLE_IME_MODE, // Toggle IME mode
    FUNCTION_FINALIZE_TEXTSTORE,
    FUNCTION_FINALIZE_TEXTSTORE_AND_INPUT,
    FUNCTION_FINALIZE_CANDIDATELIST,
    FUNCTION_FINALIZE_CANDIDATELISTForVKReturn,
    FUNCTION_FINALIZE_CANDIDATELIST_AND_INPUT,
    FUNCTION_CONVERT,
    FUNCTION_CONVERT_WILDCARD,
    FUNCTION_SELECT_BY_NUMBER,
    FUNCTION_BACKSPACE,
    FUNCTION_DELETE,
    FUNCTION_MOVE_LEFT,
    FUNCTION_MOVE_RIGHT,
    FUNCTION_MOVE_UP,
    FUNCTION_MOVE_DOWN,
    FUNCTION_MOVE_PAGE_UP,
    FUNCTION_MOVE_PAGE_DOWN,
    FUNCTION_MOVE_PAGE_TOP,
    FUNCTION_MOVE_PAGE_BOTTOM,

    // Function Double/Single byte
    FUNCTION_DOUBLE_SINGLE_BYTE,

    // Function Punctuation
    FUNCTION_PUNCTUATION,

    // Raw candidate navigation key; Server decides its configured behavior.
    FUNCTION_SERVER_CANDIDATE_KEY,

    // Unsolicited insert (voice ASR via worker pipe). Append-only so existing
    // ordinals stay stable across builds.
    FUNCTION_INSERT_TEXT,
    FUNCTION_UPDATE_VOICE_COMPOSITION,
    FUNCTION_COMMIT_VOICE_COMPOSITION,
    FUNCTION_CANCEL_VOICE_COMPOSITION,
    FUNCTION_TOGGLE_CHARACTER_SET,
    // Local edit: convert the immediately preceding Chinese punctuation when
    // the following space is claimed by smart punctuation.
    FUNCTION_SMART_PUNCTUATION_CONVERT,
    FUNCTION_SMART_PUNCTUATION_REVERT,

    // Append new functions here; preserve every pre-existing ordinal.
    FUNCTION_BACKSPACE_SEGMENT,
    FUNCTION_MOVE_LEFT_SEGMENT,
    FUNCTION_MOVE_RIGHT_SEGMENT,
    // Korean: commit the open syllable, then insert the key's own ASCII character when it has one (space, digit, punctuation). Caret and editing keys commit and then reach the application.
    FUNCTION_COMMIT_SYLLABLE,
    // Korean, behind the deferred-key barrier: a caret or editing key was eaten to keep its place in the queue, so after the syllable is committed the key is replayed to the application.
    FUNCTION_COMMIT_SYLLABLE_AND_REPLAY,
    // Korean: the Hanja key, or a key the open Hanja list takes (KoreanHanjaKey.h). Decided against the host session when the key runs, so a list key queued behind the key that opened or closed the list still does the right thing.
    FUNCTION_KOREAN_HANJA_KEY
};

static_assert(FUNCTION_MOVE_UP == 16 && FUNCTION_MOVE_PAGE_BOTTOM == 21,
              "Segment editing must not renumber existing navigation functions");
static_assert(FUNCTION_INSERT_TEXT == 25 && FUNCTION_TOGGLE_CHARACTER_SET == 29,
              "Preserve unsolicited input and character-set function ordinals");
static_assert(FUNCTION_SMART_PUNCTUATION_REVERT == 31 && FUNCTION_BACKSPACE_SEGMENT == 32 &&
                  FUNCTION_MOVE_LEFT_SEGMENT == 33 && FUNCTION_MOVE_RIGHT_SEGMENT == 34 &&
                  FUNCTION_COMMIT_SYLLABLE == 35 && FUNCTION_COMMIT_SYLLABLE_AND_REPLAY == 36 &&
                  FUNCTION_KOREAN_HANJA_KEY == 37,
              "New composition functions must be append-only");

//---------------------------------------------------------------------
// candidate list
//---------------------------------------------------------------------
enum CANDIDATE_MODE
{
    CANDIDATE_NONE = 0,
    CANDIDATE_ORIGINAL,
    CANDIDATE_INCREMENTAL
};

//---------------------------------------------------------------------
// structure
//---------------------------------------------------------------------
struct _KEYSTROKE_STATE
{
    KEYSTROKE_CATEGORY Category;
    KEYSTROKE_FUNCTION Function;
};

struct _PUNCTUATION
{
    WCHAR _Code;
    WCHAR _Punctuation[3];
};

BOOL CLSIDToString(REFGUID refGUID, _Out_writes_(39) WCHAR *pCLSIDString);

HRESULT SkipWhiteSpace(LCID locale, _In_ LPCWSTR pwszBuffer, DWORD_PTR dwBufLen, _Out_ DWORD_PTR *pdwIndex);
HRESULT FindChar(WCHAR wch, _In_ LPCWSTR pwszBuffer, DWORD_PTR dwBufLen, _Out_ DWORD_PTR *pdwIndex);

BOOL IsSpace(LCID locale, WCHAR wch);

template <class T> class CMetasequoiaImeArray
{
    typedef typename std::vector<T> CMetasequoiaImeInnerArray;
    typedef typename std::vector<T>::iterator CMetasequoiaImeInnerIter;

  public:
    CMetasequoiaImeArray() : _innerVect()
    {
    }

    explicit CMetasequoiaImeArray(size_t count) : _innerVect(count)
    {
    }

    virtual ~CMetasequoiaImeArray()
    {
    }

    inline T *GetAt(size_t index)
    {
        assert(index >= 0);
        assert(index < _innerVect.size());

        T &curT = _innerVect.at(index);

        return &(curT);
    }

    inline const T *GetAt(size_t index) const
    {
        assert(index >= 0);
        assert(index < _innerVect.size());

        const T &curT = _innerVect.at(index);

        return &(curT);
    }

    void RemoveAt(size_t index)
    {
        assert(index >= 0);
        assert(index < _innerVect.size());

        CMetasequoiaImeInnerIter iter = _innerVect.begin();
        _innerVect.erase(iter + index);
    }

    UINT Count() const
    {
        return static_cast<UINT>(_innerVect.size());
    }

    T *Append()
    {
        T newT;
        _innerVect.push_back(newT);
        T &backT = _innerVect.back();

        return &(backT);
    }

    void reserve(size_t Count)
    {
        _innerVect.reserve(Count);
    }

    void Clear()
    {
        _innerVect.clear();
    }

  private:
    CMetasequoiaImeInnerArray _innerVect;
};

class CCandidateRange
{
  public:
    CCandidateRange(void);
    ~CCandidateRange(void);

    BOOL IsRange(UINT vKey);
    int GetIndex(UINT vKey);

    inline int Count() const
    {
        return _CandidateListIndexRange.Count();
    }
    inline DWORD *GetAt(int index)
    {
        return _CandidateListIndexRange.GetAt(index);
    }
    inline DWORD *Append()
    {
        return _CandidateListIndexRange.Append();
    }

  private:
    CMetasequoiaImeArray<DWORD> _CandidateListIndexRange;
};

class CStringRange
{
  public:
    CStringRange();
    CStringRange(const CStringRange &sr);
    ~CStringRange();

    const WCHAR *Get() const;
    const DWORD_PTR GetLength() const;
    void Clear();
    void Set(const WCHAR *pwch, DWORD_PTR dwLength);
    void Set(CStringRange &sr);
    CStringRange &operator=(const CStringRange &sr);
    void CharNext(_Inout_ CStringRange *pCharNext);
    static int Compare(LCID locale, _In_ CStringRange *pString1, _In_ CStringRange *pString2);
    static BOOL WildcardCompare(LCID locale, _In_ CStringRange *stringWithWildcard, _In_ CStringRange *targetString);
    std::wstring ToWString() const
    {
        if (!_pStringBuf || _stringBufLen == 0)
            return std::wstring();
        return std::wstring(_pStringBuf, _pStringBuf + _stringBufLen);
    }

  protected:
    DWORD_PTR _stringBufLen;  // Length is in character count.
    const WCHAR *_pStringBuf; // Buffer which is not add zero terminate.
};

class CPunctuationPair
{
  public:
    CPunctuationPair();
    CPunctuationPair(WCHAR code, const WCHAR *punctuation, const WCHAR *pair);

    struct _PUNCTUATION _punctuation;
    WCHAR _pairPunctuation[3];
    BOOL _isPairToggle;
};

class CPunctuationNestPair
{
  public:
    CPunctuationNestPair();
    CPunctuationNestPair(              //
        WCHAR codeBegin,               //
        const WCHAR *punctuationBegin, //
        const WCHAR *pairBegin,        //
        const WCHAR codeEnd,           //
        const WCHAR *punctuationEnd,   //
        const WCHAR *pairEnd           //
    );

    struct _PUNCTUATION _punctuation_begin;
    WCHAR _pairPunctuation_begin[3];

    struct _PUNCTUATION _punctuation_end;
    WCHAR _pairPunctuation_end[3];

    int _nestCount;
};
