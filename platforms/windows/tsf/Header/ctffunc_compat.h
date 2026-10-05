#pragma once

// MinGW's msctf.h does not ship the function interfaces that the Windows SDK
// exposes through ctffunc.h. Keep the declarations local to the TSF target so
// the Windows SDK remains the source of truth for MSVC builds.

#ifndef TF_CLIENTID_NULL
#define TF_CLIENTID_NULL ((TfClientId)0)
#endif
#ifndef TF_INVALID_EDIT_COOKIE
#define TF_INVALID_EDIT_COOKIE ((TfEditCookie)0)
#endif

#ifndef __ITfFunction_INTERFACE_DEFINED__
#define __ITfFunction_INTERFACE_DEFINED__
DEFINE_GUID(IID_ITfFunction, 0xdb593490, 0x098f, 0x11d3, 0x8d, 0xf0, 0x00, 0x10, 0x5a, 0x27, 0x99, 0xb5);
MIDL_INTERFACE("db593490-098f-11d3-8df0-00105a2799b5")
ITfFunction : public IUnknown
{
    virtual HRESULT STDMETHODCALLTYPE GetDisplayName(BSTR *pbstrName) = 0;
};
__CRT_UUID_DECL(ITfFunction, 0xdb593490, 0x098f, 0x11d3, 0x8d, 0xf0, 0x00, 0x10, 0x5a, 0x27, 0x99, 0xb5)
#endif

#ifndef __ITfTextInputProcessorEx_INTERFACE_DEFINED__
#define __ITfTextInputProcessorEx_INTERFACE_DEFINED__
DEFINE_GUID(IID_ITfTextInputProcessorEx, 0x6e4e2102, 0xf9cd, 0x433d, 0xb4, 0x96, 0x30, 0x3c, 0xe0, 0x3a, 0x65, 0x07);
MIDL_INTERFACE("6e4e2102-f9cd-433d-b496-303ce03a6507")
ITfTextInputProcessorEx : public ITfTextInputProcessor
{
    virtual HRESULT STDMETHODCALLTYPE ActivateEx(ITfThreadMgr *ptim, TfClientId tid, DWORD dwFlags) = 0;
};
__CRT_UUID_DECL(ITfTextInputProcessorEx, 0x6e4e2102, 0xf9cd, 0x433d, 0xb4, 0x96, 0x30, 0x3c, 0xe0, 0x3a, 0x65, 0x07)
#endif

#ifndef __ITfDisplayAttributeProvider_INTERFACE_DEFINED__
#define __ITfDisplayAttributeProvider_INTERFACE_DEFINED__
DEFINE_GUID(IID_ITfDisplayAttributeProvider, 0xfee47777, 0x163c, 0x4769, 0x99, 0x6a, 0x6e, 0x9c, 0x50, 0xad, 0x8f, 0x54);
MIDL_INTERFACE("fee47777-163c-4769-996a-6e9c50ad8f54")
ITfDisplayAttributeProvider : public IUnknown
{
    virtual HRESULT STDMETHODCALLTYPE EnumDisplayAttributeInfo(IEnumTfDisplayAttributeInfo **ppEnum) = 0;
    virtual HRESULT STDMETHODCALLTYPE GetDisplayAttributeInfo(REFGUID guid, ITfDisplayAttributeInfo **ppInfo) = 0;
};
__CRT_UUID_DECL(ITfDisplayAttributeProvider, 0xfee47777, 0x163c, 0x4769, 0x99, 0x6a, 0x6e, 0x9c, 0x50, 0xad, 0x8f, 0x54)
#endif

#ifndef __ITfTextLayoutSink_INTERFACE_DEFINED__
#define __ITfTextLayoutSink_INTERFACE_DEFINED__
DEFINE_GUID(IID_ITfTextLayoutSink, 0x2af2d06a, 0xdd5b, 0x4927, 0xa0, 0xb4, 0x54, 0xf1, 0x9c, 0x91, 0xfa, 0xde);
enum TfLayoutCode
{
    TF_LC_CREATE = 0,
    TF_LC_CHANGE = 1,
    TF_LC_DESTROY = 2
};
MIDL_INTERFACE("2af2d06a-dd5b-4927-a0b4-54f19c91fade")
ITfTextLayoutSink : public IUnknown
{
    virtual HRESULT STDMETHODCALLTYPE OnLayoutChange(ITfContext *pic, TfLayoutCode lcode,
                                                      ITfContextView *pView) = 0;
};
__CRT_UUID_DECL(ITfTextLayoutSink, 0x2af2d06a, 0xdd5b, 0x4927, 0xa0, 0xb4, 0x54, 0xf1, 0x9c, 0x91, 0xfa, 0xde)
#endif

#ifndef __ITfCandidateListUIElementBehavior_INTERFACE_DEFINED__
#ifndef __ITfCandidateListUIElement_INTERFACE_DEFINED__
#define __ITfCandidateListUIElement_INTERFACE_DEFINED__
DEFINE_GUID(IID_ITfCandidateListUIElement, 0xea1ea138, 0x19df, 0x11d7, 0xa6, 0xd2, 0x00, 0x06, 0x5b, 0x84, 0x43, 0x5c);
MIDL_INTERFACE("ea1ea138-19df-11d7-a6d2-00065b84435c")
ITfCandidateListUIElement : public ITfUIElement
{
    virtual HRESULT STDMETHODCALLTYPE GetUpdatedFlags(DWORD *pdwFlags) = 0;
    virtual HRESULT STDMETHODCALLTYPE GetDocumentMgr(ITfDocumentMgr **ppdim) = 0;
    virtual HRESULT STDMETHODCALLTYPE GetCount(UINT *puCount) = 0;
    virtual HRESULT STDMETHODCALLTYPE GetSelection(UINT *puIndex) = 0;
    virtual HRESULT STDMETHODCALLTYPE GetString(UINT uIndex, BSTR *pstr) = 0;
    virtual HRESULT STDMETHODCALLTYPE GetPageIndex(UINT *pIndex, UINT uSize, UINT *puPageCnt) = 0;
    virtual HRESULT STDMETHODCALLTYPE SetPageIndex(UINT *pIndex, UINT uPageCnt) = 0;
    virtual HRESULT STDMETHODCALLTYPE GetCurrentPage(UINT *puPage) = 0;
};
__CRT_UUID_DECL(ITfCandidateListUIElement, 0xea1ea138, 0x19df, 0x11d7, 0xa6, 0xd2, 0x00, 0x06, 0x5b, 0x84, 0x43, 0x5c)
#endif

#define __ITfCandidateListUIElementBehavior_INTERFACE_DEFINED__
DEFINE_GUID(IID_ITfCandidateListUIElementBehavior, 0x85fad185, 0x58ce, 0x497a, 0x94, 0x60, 0x35, 0x53, 0x66, 0xb6, 0x4b, 0x9a);
MIDL_INTERFACE("85fad185-58ce-497a-9460-355366b64b9a")
ITfCandidateListUIElementBehavior : public ITfCandidateListUIElement
{
    virtual HRESULT STDMETHODCALLTYPE SetSelection(UINT nIndex) = 0;
    virtual HRESULT STDMETHODCALLTYPE Finalize() = 0;
    virtual HRESULT STDMETHODCALLTYPE Abort() = 0;
};
__CRT_UUID_DECL(ITfCandidateListUIElementBehavior, 0x85fad185, 0x58ce, 0x497a, 0x94, 0x60, 0x35, 0x53, 0x66, 0xb6, 0x4b, 0x9a)
#endif

#ifndef __ITfIntegratableCandidateListUIElement_INTERFACE_DEFINED__
#define __ITfIntegratableCandidateListUIElement_INTERFACE_DEFINED__
enum TfIntegratableCandidateListSelectionStyle
{
    STYLE_ACTIVE_SELECTION = 0,
    STYLE_IMPLIED_SELECTION = 0x1
};
DEFINE_GUID(IID_ITfIntegratableCandidateListUIElement, 0xc7a6f54f, 0xb180, 0x416f, 0xb2, 0xbf, 0x7b, 0xf2, 0xe4, 0x68, 0x3d, 0x7b);
MIDL_INTERFACE("c7a6f54f-b180-416f-b2bf-7bf2e4683d7b")
ITfIntegratableCandidateListUIElement : public IUnknown
{
    virtual HRESULT STDMETHODCALLTYPE SetIntegrationStyle(GUID guidIntegrationStyle) = 0;
    virtual HRESULT STDMETHODCALLTYPE GetSelectionStyle(TfIntegratableCandidateListSelectionStyle *ptfSelectionStyle) = 0;
    virtual HRESULT STDMETHODCALLTYPE OnKeyDown(WPARAM wParam, LPARAM lParam, BOOL *pfEaten) = 0;
    virtual HRESULT STDMETHODCALLTYPE ShowCandidateNumbers(BOOL *pfShow) = 0;
    virtual HRESULT STDMETHODCALLTYPE FinalizeExactCompositionString() = 0;
};
__CRT_UUID_DECL(ITfIntegratableCandidateListUIElement, 0xc7a6f54f, 0xb180, 0x416f, 0xb2, 0xbf, 0x7b, 0xf2, 0xe4, 0x68, 0x3d, 0x7b)
#endif

#ifndef __ITfCandidateString_INTERFACE_DEFINED__
#define __ITfCandidateString_INTERFACE_DEFINED__
DEFINE_GUID(IID_ITfCandidateString, 0x581f317e, 0xfd9d, 0x443f, 0xb9, 0x72, 0xed, 0x00, 0x46, 0x7c, 0x5d, 0x40);
MIDL_INTERFACE("581f317e-fd9d-443f-b972-ed00467c5d40")
ITfCandidateString : public IUnknown
{
    virtual HRESULT STDMETHODCALLTYPE GetString(BSTR *pbstr) = 0;
    virtual HRESULT STDMETHODCALLTYPE GetIndex(ULONG *pnIndex) = 0;
};
__CRT_UUID_DECL(ITfCandidateString, 0x581f317e, 0xfd9d, 0x443f, 0xb9, 0x72, 0xed, 0x00, 0x46, 0x7c, 0x5d, 0x40)
#endif

#ifndef __IEnumTfCandidates_INTERFACE_DEFINED__
#define __IEnumTfCandidates_INTERFACE_DEFINED__
DEFINE_GUID(IID_IEnumTfCandidates, 0xdefb1926, 0x6c80, 0x4ce8, 0x87, 0xd4, 0xd6, 0xb7, 0x2b, 0x81, 0x2b, 0xde);
MIDL_INTERFACE("defb1926-6c80-4ce8-87d4-d6b72b812bde")
IEnumTfCandidates : public IUnknown
{
    virtual HRESULT STDMETHODCALLTYPE Clone(IEnumTfCandidates **ppEnum) = 0;
    virtual HRESULT STDMETHODCALLTYPE Next(ULONG ulCount, ITfCandidateString **ppCand, ULONG *pcFetched) = 0;
    virtual HRESULT STDMETHODCALLTYPE Reset() = 0;
    virtual HRESULT STDMETHODCALLTYPE Skip(ULONG ulCount) = 0;
};
__CRT_UUID_DECL(IEnumTfCandidates, 0xdefb1926, 0x6c80, 0x4ce8, 0x87, 0xd4, 0xd6, 0xb7, 0x2b, 0x81, 0x2b, 0xde)
#endif

#ifndef __ITfCandidateList_INTERFACE_DEFINED__
#define __ITfCandidateList_INTERFACE_DEFINED__
DEFINE_GUID(IID_ITfCandidateList, 0xa3ad50fb, 0x9bdb, 0x49e3, 0xa8, 0x43, 0x6c, 0x76, 0x52, 0x0f, 0xbf, 0x5d);
enum TfCandidateResult
{
    CAND_FINALIZED = 0,
    CAND_SELECTED = 0x1,
    CAND_CANCELED = 0x2
};
MIDL_INTERFACE("a3ad50fb-9bdb-49e3-a843-6c76520fbf5d")
ITfCandidateList : public IUnknown
{
    virtual HRESULT STDMETHODCALLTYPE EnumCandidates(IEnumTfCandidates **ppEnum) = 0;
    virtual HRESULT STDMETHODCALLTYPE GetCandidate(ULONG nIndex, ITfCandidateString **ppCand) = 0;
    virtual HRESULT STDMETHODCALLTYPE GetCandidateNum(ULONG *pnCnt) = 0;
    virtual HRESULT STDMETHODCALLTYPE SetResult(ULONG nIndex, TfCandidateResult imcr) = 0;
};
__CRT_UUID_DECL(ITfCandidateList, 0xa3ad50fb, 0x9bdb, 0x49e3, 0xa8, 0x43, 0x6c, 0x76, 0x52, 0x0f, 0xbf, 0x5d)
#endif

#ifndef __ITfFnSearchCandidateProvider_INTERFACE_DEFINED__
#define __ITfFnSearchCandidateProvider_INTERFACE_DEFINED__
DEFINE_GUID(IID_ITfFnSearchCandidateProvider, 0x87a2ad8f, 0xf27b, 0x4920, 0x85, 0x01, 0x67, 0x60, 0x22, 0x80, 0x17, 0x5d);
MIDL_INTERFACE("87a2ad8f-f27b-4920-8501-67602280175d")
ITfFnSearchCandidateProvider : public ITfFunction
{
    virtual HRESULT STDMETHODCALLTYPE GetSearchCandidates(BSTR bstrQuery, BSTR bstrApplicationId,
                                                           ITfCandidateList **pplist) = 0;
    virtual HRESULT STDMETHODCALLTYPE SetResult(BSTR bstrQuery, BSTR bstrApplicationID, BSTR bstrResult) = 0;
};
__CRT_UUID_DECL(ITfFnSearchCandidateProvider, 0x87a2ad8f, 0xf27b, 0x4920, 0x85, 0x01, 0x67, 0x60, 0x22, 0x80, 0x17, 0x5d)
#endif

#ifndef __ITfFnGetPreferredTouchKeyboardLayout_INTERFACE_DEFINED__
#define __ITfFnGetPreferredTouchKeyboardLayout_INTERFACE_DEFINED__
DEFINE_GUID(IID_ITfFnGetPreferredTouchKeyboardLayout, 0x5f309a41, 0x590a, 0x4acc, 0xa9, 0x7f, 0xd8, 0xef, 0xff, 0x13, 0xfd, 0xfc);
enum TKBLayoutType
{
    TKBLT_UNDEFINED = 0,
    TKBLT_CLASSIC = 1,
    TKBLT_OPTIMIZED = 2
};
MIDL_INTERFACE("5f309a41-590a-4acc-a97f-d8efff13fdfc")
ITfFnGetPreferredTouchKeyboardLayout : public ITfFunction
{
    virtual HRESULT STDMETHODCALLTYPE GetLayout(TKBLayoutType *pTKBLayoutType, WORD *pwPreferredLayoutId) = 0;
};
__CRT_UUID_DECL(ITfFnGetPreferredTouchKeyboardLayout, 0x5f309a41, 0x590a, 0x4acc, 0xa9, 0x7f, 0xd8, 0xef, 0xff, 0x13, 0xfd, 0xfc)
#endif

#ifndef TKBL_OPT_SIMPLIFIED_CHINESE_PINYIN
#define TKBL_OPT_SIMPLIFIED_CHINESE_PINYIN 0x0804
#endif
#ifndef TKBL_OPT_JAPANESE_ABC
#define TKBL_OPT_JAPANESE_ABC 0x0411
#endif
#ifndef TKBL_UNDEFINED
#define TKBL_UNDEFINED 0x0000
#endif

// ctfutb.h is also absent from MinGW. These are the language-bar contracts
// used by the TSF language-bar button implementation.
interface ITfMenu;
#ifndef TF_LBI_STATUS_HIDDEN
#define TF_LBI_STATUS_HIDDEN 0x00000001
#define TF_LBI_STATUS_DISABLED 0x00000002
#define TF_LBI_STATUS 0x00010000
#define TF_LBI_STATUS_BTN_TOGGLED 0x00010000
#define TF_LBI_STYLE_SHOWNINTRAY 0x00000002
#define TF_LBI_STYLE_BTN_BUTTON 0x00010000
#define TF_LBI_STYLE_BTN_MENU 0x00020000
#define TF_LBI_STYLE_BTN_TOGGLE 0x00040000
#define TF_LBI_ICON 0x00000001
#define TF_LBI_TEXT 0x00000002
#define TF_LBI_TOOLTIP 0x00000004
#define TF_CLUIE_DOCUMENTMGR 0x00000001
#define TF_CLUIE_COUNT 0x00000002
#define TF_CLUIE_SELECTION 0x00000004
#define TF_CLUIE_STRING 0x00000008
#define TF_CLUIE_PAGEINDEX 0x00000010
#define TF_CLUIE_CURRENTPAGE 0x00000020
#endif

#ifndef __ITfLangBarItemButton_INTERFACE_DEFINED__
#define __ITfLangBarItemButton_INTERFACE_DEFINED__
DEFINE_GUID(IID_ITfLangBarItemButton, 0x28c7f1d0, 0xde25, 0x11d2, 0xaf, 0xdd, 0x00, 0x10, 0x5a, 0x27, 0x99, 0xb5);
enum TfLBIClick
{
    TF_LBI_CLK_RIGHT = 1,
    TF_LBI_CLK_LEFT = 2
};
MIDL_INTERFACE("28c7f1d0-de25-11d2-afdd-00105a2799b5")
ITfLangBarItemButton : public ITfLangBarItem
{
    virtual HRESULT STDMETHODCALLTYPE OnClick(TfLBIClick click, POINT pt, const RECT *prcArea) = 0;
    virtual HRESULT STDMETHODCALLTYPE InitMenu(ITfMenu *pMenu) = 0;
    virtual HRESULT STDMETHODCALLTYPE OnMenuSelect(UINT wID) = 0;
    virtual HRESULT STDMETHODCALLTYPE GetIcon(HICON *phIcon) = 0;
    virtual HRESULT STDMETHODCALLTYPE GetText(BSTR *pbstrText) = 0;
};
__CRT_UUID_DECL(ITfLangBarItemButton, 0x28c7f1d0, 0xde25, 0x11d2, 0xaf, 0xdd, 0x00, 0x10, 0x5a, 0x27, 0x99, 0xb5)
#endif

#ifndef __ITfMenu_INTERFACE_DEFINED__
#define __ITfMenu_INTERFACE_DEFINED__
DEFINE_GUID(IID_ITfMenu, 0x6f8a98e4, 0xaaa0, 0x4f15, 0x8c, 0x5b, 0x07, 0xe0, 0xdf, 0x0a, 0x3d, 0xd8);
MIDL_INTERFACE("6f8a98e4-aaa0-4f15-8c5b-07e0df0a3dd8")
ITfMenu : public IUnknown
{
    virtual HRESULT STDMETHODCALLTYPE AddMenuItem(UINT uId, DWORD dwFlags, HBITMAP hbmp, HBITMAP hbmpMask,
                                                   const WCHAR *pch, ULONG cch, ITfMenu **ppMenu) = 0;
};
__CRT_UUID_DECL(ITfMenu, 0x6f8a98e4, 0xaaa0, 0x4f15, 0x8c, 0x5b, 0x07, 0xe0, 0xdf, 0x0a, 0x3d, 0xd8)
#endif

#ifndef GUID_LBI_INPUTMODE
DEFINE_GUID(GUID_LBI_INPUTMODE, 0x2c77a81e, 0x41cc, 0x4178, 0xa3, 0xa7, 0x5f, 0x8a, 0x98, 0x75, 0x68, 0xe6);
#endif
#ifndef GUID_COMPARTMENT_KEYBOARD_INPUTMODE_CONVERSION
DEFINE_GUID(GUID_COMPARTMENT_KEYBOARD_INPUTMODE_CONVERSION, 0xccf05dd8, 0x4a87, 0x11d7, 0xa6, 0xe2, 0x00, 0x06, 0x5b, 0x84, 0x43, 0x5c);
#define TF_CONVERSIONMODE_ALPHANUMERIC 0x0000
#define TF_CONVERSIONMODE_NATIVE 0x0001
#define TF_CONVERSIONMODE_FULLSHAPE 0x0008
#define TF_CONVERSIONMODE_SYMBOL 0x0400
#endif
#ifndef GUID_COMPARTMENT_KEYBOARD_INPUTMODE_SENTENCE
DEFINE_GUID(GUID_COMPARTMENT_KEYBOARD_INPUTMODE_SENTENCE, 0xccf05dd9, 0x4a87, 0x11d7, 0xa6, 0xe2, 0x00, 0x06, 0x5b, 0x84, 0x43, 0x5c);
#endif

#ifndef GUID_TFCAT_TIPCAP_SECUREMODE
DEFINE_GUID(GUID_TFCAT_TIPCAP_SECUREMODE, 0x49d2f9ce, 0x1f5e, 0x11d7, 0xa6, 0xd3, 0x00, 0x06, 0x5b, 0x84, 0x43, 0x5c);
DEFINE_GUID(GUID_TFCAT_TIPCAP_UIELEMENTENABLED, 0x49d2f9cf, 0x1f5e, 0x11d7, 0xa6, 0xd3, 0x00, 0x06, 0x5b, 0x84, 0x43, 0x5c);
DEFINE_GUID(GUID_TFCAT_TIPCAP_INPUTMODECOMPARTMENT, 0xccf05dd7, 0x4a87, 0x11d7, 0xa6, 0xe2, 0x00, 0x06, 0x5b, 0x84, 0x43, 0x5c);
DEFINE_GUID(GUID_TFCAT_TIPCAP_COMLESS, 0x364215d9, 0x75bc, 0x11d7, 0xa6, 0xef, 0x00, 0x06, 0x5b, 0x84, 0x43, 0x5c);
DEFINE_GUID(GUID_TFCAT_TIPCAP_IMMERSIVESUPPORT, 0x13a016df, 0x560b, 0x46cd, 0x94, 0x7a, 0x4c, 0x3a, 0xf1, 0xe0, 0xe3, 0x5d);
DEFINE_GUID(GUID_TFCAT_TIPCAP_SYSTRAYSUPPORT, 0x25504fb4, 0x7bab, 0x4bc1, 0x9c, 0x69, 0xcf, 0x81, 0x89, 0x0f, 0x0e, 0xf5);
#endif

#ifndef GUID_INTEGRATIONSTYLE_SEARCHBOX
DEFINE_GUID(GUID_INTEGRATIONSTYLE_SEARCHBOX, 0xe6d1bd11, 0x82f7, 0x4903, 0xae, 0x21, 0x1a, 0x63, 0x97, 0xcd, 0xe2, 0xeb);
#endif
