#pragma once
#include "Globals.h"
#include "Private.h"
#include "MetasequoiaIME.h"
#include "ClientKeyRouter.h"
#include <cstdint>
#include <string>

class CKeyStateCategory;

class CKeyStateCategoryFactory
{
  public:
    static CKeyStateCategoryFactory *Instance();
    CKeyStateCategory *MakeKeyStateCategory(KEYSTROKE_CATEGORY keyCategory, _In_ CMetasequoiaIME *pTextService);
    void Release();

  protected:
    CKeyStateCategoryFactory();

  private:
    static thread_local CKeyStateCategoryFactory *_instance;
};

typedef struct KeyHandlerEditSessionDTO
{
    KeyHandlerEditSessionDTO(TfEditCookie tFEC, _In_ ITfContext *pTfContext, UINT virualCode, WCHAR inputChar,
                             KEYSTROKE_FUNCTION arrowKeyFunction, uint64_t pipeRequestId,
                             const std::wstring &sessionPrefetchedText, UINT scanCode = 0)
    {
        ec = tFEC;
        pContext = pTfContext;
        code = virualCode;
        scan = scanCode;
        wch = inputChar;
        arrowKey = arrowKeyFunction;
        requestId = pipeRequestId;
        prefetchedText = sessionPrefetchedText;
    }

    TfEditCookie ec;
    ITfContext *pContext;
    UINT code;
    UINT scan;
    WCHAR wch;
    KEYSTROKE_FUNCTION arrowKey;
    uint64_t requestId;
    uint64_t deferredReplayToken = 0;
    bool groupedPanelKey = false;
    std::wstring prefetchedText;
} KeyHandlerEditSessionDTO;

inline ClientKeyEvent client_key_event(const KeyHandlerEditSessionDTO &dto,
                                       ClientFocusLease lease,
                                       std::uint32_t modifiers = 0,
                                       bool ui_less = false,
                                       std::uint32_t scan_code = 0) {
    // The TSF edit session does not always retain the original LPARAM. Keep
    // the boundary explicit: callers that observed a scan code can preserve
    // it, while legacy paths remain ABI-compatible with zero.
    return {lease, dto.code, scan_code != 0 ? scan_code : dto.scan, modifiers,
            static_cast<char16_t>(dto.wch), ui_less};
}

class CKeyStateCategory
{
  public:
    CKeyStateCategory(_In_ CMetasequoiaIME *pTextService);

  protected:
    ~CKeyStateCategory(void);

  public:
    HRESULT KeyStateHandler(KEYSTROKE_FUNCTION function, KeyHandlerEditSessionDTO dto);
    void Release(void);

  protected:
    // HandleKeyInput
    virtual HRESULT HandleKeyInput(KeyHandlerEditSessionDTO dto);

    // HandleKeyFinalizeTextStoreAndInput
    virtual HRESULT HandleKeyFinalizeTextStoreAndInput(KeyHandlerEditSessionDTO dto);

    // HandleKeyFinalizeTextStore
    virtual HRESULT HandleKeyFinalizeTextStore(KeyHandlerEditSessionDTO dto);

    // HandleKeyFinalizeCandidatelistAndInput
    virtual HRESULT HandleKeyFinalizeCandidatelistAndInput(KeyHandlerEditSessionDTO dto);

    // HandleKeyFinalizeCandidatelist
    virtual HRESULT HandleKeyFinalizeCandidatelist(KeyHandlerEditSessionDTO dto);

    // HandleKeyFinalizeCandidatelist
    virtual HRESULT HandleKeyFinalizeCandidatelistForVKReturn(KeyHandlerEditSessionDTO dto);

    // HandleKeyConvert
    virtual HRESULT HandleKeyConvert(KeyHandlerEditSessionDTO dto);

    // HandleKeyConvertWild
    virtual HRESULT HandleKeyConvertWildCard(KeyHandlerEditSessionDTO dto);

    // HandleKeyCancel
    virtual HRESULT HandleKeyCancel(KeyHandlerEditSessionDTO dto);

    // _HandleKeyToogleIMEMode
    virtual HRESULT HandleKeyToogleIMEMode(KeyHandlerEditSessionDTO dto);

    // HandleKeyBackspace
    virtual HRESULT HandleKeyBackspace(KeyHandlerEditSessionDTO dto);

    // HandleKeyDelete
    virtual HRESULT HandleKeyDelete(KeyHandlerEditSessionDTO dto);

    // HandleKeyArrow
    virtual HRESULT HandleKeyArrow(KeyHandlerEditSessionDTO dto);

    // HandleKeyDoubleSingleByte
    virtual HRESULT HandleKeyDoubleSingleByte(KeyHandlerEditSessionDTO dto);

    // HandleKeyPunctuation
    virtual HRESULT HandleKeyPunctuation(KeyHandlerEditSessionDTO dto);

    // HandleKeySelectByNumber
    virtual HRESULT HandleKeySelectByNumber(KeyHandlerEditSessionDTO dto);

  protected:
    CMetasequoiaIME *_pTextService;
};

class CKeyStateComposing : public CKeyStateCategory
{
  public:
    CKeyStateComposing(_In_ CMetasequoiaIME *pTextService);

  protected:
    // _HandleCompositionInput
    HRESULT HandleKeyInput(KeyHandlerEditSessionDTO dto);

    // HandleKeyCompositionFinalizeTextStoreAndInput
    HRESULT HandleKeyFinalizeTextStoreAndInput(KeyHandlerEditSessionDTO dto);

    // HandleKeyFinalizeTextStore
    HRESULT HandleKeyFinalizeTextStore(KeyHandlerEditSessionDTO dto);

    // HandleKeyCompositionFinalizeCandidatelistAndInput
    HRESULT HandleKeyFinalizeCandidatelistAndInput(KeyHandlerEditSessionDTO dto);

    // HandleKeyCompositionFinalizeCandidatelist
    HRESULT HandleKeyFinalizeCandidatelist(KeyHandlerEditSessionDTO dto);

    // HandleCompositionConvert
    HRESULT HandleKeyConvert(KeyHandlerEditSessionDTO dto);

    // HandleKeyCompositionConvertWildCard
    HRESULT HandleKeyConvertWildCard(KeyHandlerEditSessionDTO dto);

    // HandleCancel
    HRESULT HandleKeyCancel(KeyHandlerEditSessionDTO dto);

    // HandleKeyToogleIMEMode
    HRESULT HandleKeyToogleIMEMode(KeyHandlerEditSessionDTO dto);

    // HandleCompositionBackspace
    HRESULT HandleKeyBackspace(KeyHandlerEditSessionDTO dto);

    // HandleCompositionDelete
    HRESULT HandleKeyDelete(KeyHandlerEditSessionDTO dto);

    // HandleArrowKey
    HRESULT HandleKeyArrow(KeyHandlerEditSessionDTO dto);

    // HandleKeyDoubleSingleByte
    HRESULT HandleKeyDoubleSingleByte(KeyHandlerEditSessionDTO dto);

    // HandleKeyCompositionPunctuation
    HRESULT HandleKeyPunctuation(KeyHandlerEditSessionDTO dto);
};

class CKeyStateCandidate : public CKeyStateCategory
{
  public:
    CKeyStateCandidate(_In_ CMetasequoiaIME *pTextService);

  protected:
    // HandleKeyFinalizeCandidatelist
    HRESULT HandleKeyFinalizeCandidatelist(KeyHandlerEditSessionDTO dto);

    HRESULT HandleKeyFinalizeCandidatelistForVKReturn(KeyHandlerEditSessionDTO dto);

    // HandleKeyFinalizeCandidatelistAndInput
    HRESULT HandleKeyFinalizeCandidatelistAndInput(KeyHandlerEditSessionDTO dto);

    //_HandleCandidateConvert
    HRESULT HandleKeyConvert(KeyHandlerEditSessionDTO dto);

    //_HandleCancel
    HRESULT HandleKeyCancel(KeyHandlerEditSessionDTO dto);

    //_HandleCandidateArrowKey
    HRESULT HandleKeyArrow(KeyHandlerEditSessionDTO dto);

    //_HandleCandidateSelectByNumber
    HRESULT HandleKeySelectByNumber(KeyHandlerEditSessionDTO dto);
};

// degeneration class
class CKeyStateNull : public CKeyStateCategory
{
  public:
    CKeyStateNull(_In_ CMetasequoiaIME *pTextService) : CKeyStateCategory(pTextService) {};

  protected:
    // _HandleNullInput
    HRESULT HandleKeyInput(KeyHandlerEditSessionDTO dto)
    {
        return CKeyStateCategory::HandleKeyInput(dto);
    };

    // HandleKeyNullFinalizeTextStoreAndInput
    HRESULT HandleKeyFinalizeTextStoreAndInput(KeyHandlerEditSessionDTO dto)
    {
        return CKeyStateCategory::HandleKeyFinalizeTextStoreAndInput(dto);
    };

    // HandleKeyFinalizeTextStore
    HRESULT HandleKeyFinalizeTextStore(KeyHandlerEditSessionDTO dto)
    {
        return CKeyStateCategory::HandleKeyFinalizeTextStore(dto);
    };

    // HandleKeyNullFinalizeCandidatelistAndInput
    HRESULT HandleKeyFinalizeCandidatelistAndInput(KeyHandlerEditSessionDTO dto)
    {
        return CKeyStateCategory::HandleKeyFinalizeCandidatelistAndInput(dto);
    };

    // HandleKeyNullFinalizeCandidatelist
    HRESULT HandleKeyFinalizeCandidatelist(KeyHandlerEditSessionDTO dto)
    {
        return CKeyStateCategory::HandleKeyFinalizeCandidatelist(dto);
    };

    //_HandleNullConvert
    HRESULT HandleKeyConvert(KeyHandlerEditSessionDTO dto)
    {
        return CKeyStateCategory::HandleKeyConvert(dto);
    };

    //_HandleNullCancel
    HRESULT HandleKeyCancel(KeyHandlerEditSessionDTO dto)
    {
        return CKeyStateCategory::HandleKeyCancel(dto);
    };

    // HandleKeyNullConvertWild
    HRESULT HandleKeyConvertWildCard(KeyHandlerEditSessionDTO dto)
    {
        return CKeyStateCategory::HandleKeyConvertWildCard(dto);
    };

    //_HandleNullBackspace
    HRESULT HandleKeyBackspace(KeyHandlerEditSessionDTO dto)
    {
        return CKeyStateCategory::HandleKeyBackspace(dto);
    };

    //_HandleNullArrowKey
    HRESULT HandleKeyArrow(KeyHandlerEditSessionDTO dto)
    {
        return CKeyStateCategory::HandleKeyArrow(dto);
    };

    // HandleKeyDoubleSingleByte
    HRESULT HandleKeyDoubleSingleByte(KeyHandlerEditSessionDTO dto)
    {
        return CKeyStateCategory::HandleKeyDoubleSingleByte(dto);
    };

    // HandleKeyPunctuation
    HRESULT HandleKeyPunctuation(KeyHandlerEditSessionDTO dto)
    {
        return CKeyStateCategory::HandleKeyPunctuation(dto);
    };

    //_HandleNullCandidateSelectByNumber
    HRESULT HandleKeySelectByNumber(KeyHandlerEditSessionDTO dto)
    {
        return CKeyStateCategory::HandleKeySelectByNumber(dto);
    };
};
