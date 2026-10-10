#include "Private.h"
#include "Globals.h"
#include "MetasequoiaIME.h"
#include "CandidateListUIPresenter.h"
#include "CompositionProcessorEngine.h"
#include "KeyHandlerEditSession.h"
#include "KeyRepeatGuard.h"
#include "Compartment.h"
#include "MetasequoiaIMEBaseStructure.h"
#include <debugapi.h>
#include <cwctype>
#include <string>
#include "Ipc.h"
#include "PassthroughStatistics.h"
#include "PassthroughStatisticsQueue.h"
#include "KeyPressStatistics.h"
#include "KeyPressStatisticsQueue.h"
#include "FanyDefines.h"
#include "AltGrKeyPolicy.h"
#include "FanyUtils.h"
#include "FanyLog.h"
#include "../Utils/PerfTimer.h"
#include "../HostKoreanKey.h"
#include "../../common/PipeMetadata.h"
#include "../../common/SecondThirdCandidatePolicy.h"
#include <chrono>
#include "../../../../shared/contracts/ipc_negotiation.h"

// 0xF003, 0xF004 are the keys that the touch keyboard sends for next/previous
#define THIRDPARTY_NEXTPAGE static_cast<WORD>(0xF003)
#define THIRDPARTY_PREVPAGE static_cast<WORD>(0xF004)

namespace
{
constexpr UINT kMaxDeferredKeyReplayAttempts = 8;

// A recovery checkpoint may need one INPUT for every raw pinyin character and
// one MOVE_LEFT for every character to the right of the caret.  Keep enough
// additional room for a short burst that arrives while the replacement IPC
// epoch is becoming ready.
constexpr size_t MAX_DEFERRED_KEY_DOWN_COUNT = static_cast<size_t>(MAX_PINYIN_LENGTH) * 2 + 32;

struct DeferredShadowState
{
    bool imeOpen = false;
    bool punctuationOpen = false;
    bool doubleSingleByteOpen = false;
    size_t inputLength = 0;
    std::wstring rawInput;
    size_t caret = 0;
    bool candidateActive = false;
    bool unicodeMode = false;
    // 网址模式由触发键进入后一直保持，不从缓冲前缀推断，规则与 CCompositionProcessorEngine::IsUrlModeComposition 相同。
    bool urlMode = false;
    // Korean and Zhuyin: whether the composition's list is open once the keys ahead have run (see project_korean_hanja_key).
    bool koreanHanjaListOpen = false;
    bool projectionValid = true;
};

bool IsBareModifierKey(UINT code)
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
    case VK_LWIN:
    case VK_RWIN:
        return true;
    default:
        return false;
    }
}

void ApplyDeferredKeyState(DeferredShadowState &shadow, const _KEYSTROKE_STATE &keyState, WCHAR wch = 0,
                           UINT code = 0)
{
    const auto clearComposition = [&shadow]() {
        shadow.inputLength = 0;
        shadow.rawInput.clear();
        shadow.caret = 0;
        shadow.candidateActive = false;
        shadow.unicodeMode = false;
        shadow.urlMode = false;
        shadow.koreanHanjaListOpen = false;
    };

    switch (keyState.Function)
    {
    case FUNCTION_INPUT:
        // 藏文的空格和 `/` 不进入原文：组字时引擎把整串音节连同音节点或垂符上屏，空闲的 `/` 直接输出垂符，之后都没有组字。
        if (Global::InputModeScheme.load(std::memory_order_relaxed) == msime::windows::scheme::Tibetan &&
            (wch == L' ' || wch == L'/'))
        {
            clearComposition();
            break;
        }
        // A Korean letter, or a key Zhuyin spells with, closes the open list and keeps composing.
        shadow.koreanHanjaListOpen = false;
        if (shadow.inputLength == 0)
        {
            shadow.unicodeMode = (wch == L'U');
        }
        shadow.caret = min(shadow.caret, shadow.rawInput.size());
        if (shadow.rawInput.size() < MAX_PINYIN_LENGTH && wch != L'\0')
        {
            // 与 AddVirtualKey 一致：网址模式里的 `'` 是网址字符，连续的也不合并。
            const bool duplicateSeparator =
                wch == L'\'' && !shadow.urlMode &&
                ((shadow.caret > 0 && shadow.rawInput[shadow.caret - 1] == L'\'') ||
                 (shadow.caret < shadow.rawInput.size() && shadow.rawInput[shadow.caret] == L'\''));
            if (!duplicateSeparator)
            {
                shadow.urlMode =
                    shadow.urlMode ||
                    Global::OpensUrlMode(
                        shadow.rawInput.c_str(), shadow.rawInput.size(), shadow.caret, wch,
                        msime::windows::scheme::DetectsUrls(Global::InputModeScheme.load(std::memory_order_relaxed)),
                        Global::DedicatedEnglish.active(GetTickCount64()));
                shadow.rawInput.insert(shadow.caret, 1, wch);
                ++shadow.caret;
            }
        }
        shadow.inputLength = shadow.rawInput.size();
        shadow.candidateActive = false;
        break;
    case FUNCTION_FINALIZE_TEXTSTORE_AND_INPUT:
    case FUNCTION_FINALIZE_CANDIDATELIST_AND_INPUT:
        shadow.rawInput.assign(wch == L'\0' ? 0 : 1, wch);
        shadow.caret = shadow.rawInput.size();
        shadow.inputLength = shadow.rawInput.size();
        shadow.candidateActive = false;
        shadow.unicodeMode = (wch == L'U');
        shadow.urlMode = false;
        shadow.koreanHanjaListOpen = false;
        break;
    case FUNCTION_BACKSPACE:
        shadow.caret = min(shadow.caret, shadow.rawInput.size());
        if (shadow.caret > 0)
        {
            const WCHAR removed = shadow.rawInput[shadow.caret - 1];
            shadow.rawInput.erase(shadow.caret - 1, 1);
            --shadow.caret;
            shadow.urlMode =
                Global::UrlModeAfterDeletion(shadow.urlMode, shadow.rawInput.c_str(), shadow.rawInput.size(), removed);
        }
        shadow.inputLength = shadow.rawInput.size();
        if (shadow.inputLength == 0)
        {
            shadow.candidateActive = false;
            shadow.unicodeMode = false;
        }
        break;
    case FUNCTION_BACKSPACE_SEGMENT:
    case FUNCTION_MOVE_LEFT_SEGMENT:
    case FUNCTION_MOVE_RIGHT_SEGMENT:
        shadow.projectionValid = false;
        break;
    case FUNCTION_CONVERT_WILDCARD:
        shadow.candidateActive = shadow.inputLength > 0;
        break;
    case FUNCTION_CONVERT:
        // This TIP routes Space+Convert to WM_AsyncFinalizeCandidate, which
        // commits and ends the composition rather than merely opening a list.
        clearComposition();
        break;
    case FUNCTION_CANCEL:
        clearComposition();
        break;
    case FUNCTION_MOVE_LEFT:
        // 全拼和双拼的左右键可能进入整句改字，之后的空格和数字是钉住一段而不是上屏，这里推算不出来，交回真实状态。
        if (msime::windows::scheme::EditsSentence(Global::InputModeScheme.load(std::memory_order_relaxed)))
        {
            shadow.projectionValid = false;
            break;
        }
        if (shadow.caret > 0)
        {
            --shadow.caret;
        }
        break;
    case FUNCTION_DELETE:
        shadow.caret = min(shadow.caret, shadow.rawInput.size());
        if (shadow.caret < shadow.rawInput.size())
        {
            const WCHAR removed = shadow.rawInput[shadow.caret];
            shadow.rawInput.erase(shadow.caret, 1);
            shadow.urlMode =
                Global::UrlModeAfterDeletion(shadow.urlMode, shadow.rawInput.c_str(), shadow.rawInput.size(), removed);
        }
        shadow.inputLength = shadow.rawInput.size();
        if (shadow.inputLength == 0)
        {
            shadow.candidateActive = false;
            shadow.unicodeMode = false;
        }
        break;
    case FUNCTION_MOVE_RIGHT:
        if (msime::windows::scheme::EditsSentence(Global::InputModeScheme.load(std::memory_order_relaxed)))
        {
            shadow.projectionValid = false;
            break;
        }
        if (shadow.caret < shadow.rawInput.size())
        {
            ++shadow.caret;
        }
        break;
    case FUNCTION_KOREAN_HANJA_KEY: {
        // Only the key that opens the list, and a list key while the projection has the list open, are queued as this function.
        const auto projected = msime::tsf::project_korean_hanja_key(Global::InputModeScheme.load(std::memory_order_relaxed),
                                                                    code, wch, shadow.koreanHanjaListOpen);
        if (projected.syllableEnds)
        {
            clearComposition();
        }
        shadow.koreanHanjaListOpen = projected.listOpen;
        break;
    }
    case FUNCTION_FINALIZE_TEXTSTORE:
    case FUNCTION_COMMIT_SYLLABLE:
    case FUNCTION_COMMIT_SYLLABLE_AND_REPLAY:
    case FUNCTION_FINALIZE_CANDIDATELIST:
    case FUNCTION_FINALIZE_CANDIDATELISTForVKReturn:
    case FUNCTION_SELECT_BY_NUMBER:
    case FUNCTION_TOGGLE_IME_MODE:
    case FUNCTION_PUNCTUATION:
    case FUNCTION_DOUBLE_SINGLE_BYTE:
        clearComposition();
        break;
    default:
        break;
    }
}

bool IsRecoverableDeferredPrefix(const _KEYSTROKE_STATE &keyState)
{
    switch (keyState.Function)
    {
    case FUNCTION_INPUT:
    case FUNCTION_BACKSPACE:
    case FUNCTION_BACKSPACE_SEGMENT:
    case FUNCTION_MOVE_LEFT_SEGMENT:
    case FUNCTION_MOVE_RIGHT_SEGMENT:
    case FUNCTION_DELETE:
    case FUNCTION_MOVE_LEFT:
    case FUNCTION_MOVE_RIGHT:
    case FUNCTION_MOVE_UP:
    case FUNCTION_MOVE_DOWN:
    case FUNCTION_MOVE_PAGE_UP:
    case FUNCTION_MOVE_PAGE_DOWN:
    case FUNCTION_MOVE_PAGE_TOP:
    case FUNCTION_MOVE_PAGE_BOTTOM:
    case FUNCTION_CONVERT_WILDCARD:
    case FUNCTION_SERVER_CANDIDATE_KEY:
        return true;
    default:
        return false;
    }
}

bool StartsNewDeferredPrefix(const _KEYSTROKE_STATE &keyState)
{
    return keyState.Function == FUNCTION_FINALIZE_TEXTSTORE_AND_INPUT ||
           keyState.Function == FUNCTION_FINALIZE_CANDIDATELIST_AND_INPUT;
}

UINT CaptureIpcModifiers()
{
    UINT modifiers = 0;
    if ((GetAsyncKeyState(VK_SHIFT) & 0x8000) != 0)
        modifiers |= 0b00000001;
    if ((GetAsyncKeyState(VK_CONTROL) & 0x8000) != 0)
        modifiers |= 0b00000010;
    if ((GetAsyncKeyState(VK_MENU) & 0x8000) != 0)
        modifiers |= 0b00000100;
    return modifiers;
}

bool IsEnglishInputModeToggle(UINT code, UINT modifiers)
{
    // Ctrl+Shift+E, without Alt.
    return code == 'E' && (modifiers & 0b00000111u) == 0b00000011u;
}

bool IsTranslationCommitShortcut(UINT code, UINT modifiers)
{
    return code == VK_RETURN && (modifiers & 0b00000111u) == 0b00000010u &&
           (GetAsyncKeyState(VK_LWIN) & 0x8000) == 0 && (GetAsyncKeyState(VK_RWIN) & 0x8000) == 0;
}

KEYSTROKE_FUNCTION SegmentEditFunction(UINT code, UINT modifiers)
{
    if ((modifiers & 0b00000111u) != 0b00000010u ||
        (GetAsyncKeyState(VK_LWIN) & 0x8000) != 0 || (GetAsyncKeyState(VK_RWIN) & 0x8000) != 0)
    {
        return FUNCTION_NONE;
    }
    switch (code)
    {
    case VK_BACK: return FUNCTION_BACKSPACE_SEGMENT;
    case VK_LEFT: return FUNCTION_MOVE_LEFT_SEGMENT;
    case VK_RIGHT: return FUNCTION_MOVE_RIGHT_SEGMENT;
    default: return FUNCTION_NONE;
    }
}

bool IsCharacterSetInputModeToggle(UINT code, UINT modifiers)
{
    return FanyImeProtocol::IsCharacterSetShortcut(code, modifiers) && (GetAsyncKeyState(VK_LWIN) & 0x8000) == 0 &&
           (GetAsyncKeyState(VK_RWIN) & 0x8000) == 0 && SupportsCharacterSetShortcut() &&
           FanyUtils::ReadConfiguredSwitchLanguageHotkeys().character_set_ctrl_shift_f;
}

void PostOwnerMessageWithSyncFallback(HWND window, UINT message, WPARAM wParam = 0, LPARAM lParam = 0)
{
    if (!window || !IsWindow(window))
    {
        return;
    }
    if (!PostMessage(window, message, wParam, lParam) &&
        GetWindowThreadProcessId(window, nullptr) == GetCurrentThreadId())
    {
        SendMessage(window, message, wParam, lParam);
    }
}

constexpr auto kModifierHotkeyToggleLimit = std::chrono::milliseconds(500);

bool IsShiftVk(UINT code)
{
    return code == VK_SHIFT || code == VK_LSHIFT || code == VK_RSHIFT;
}

bool IsControlVk(UINT code)
{
    return code == VK_CONTROL || code == VK_LCONTROL || code == VK_RCONTROL;
}

// The right Ctrl key: its own code from a host that reports sides, or VK_CONTROL with the extended-key bit.
bool IsRightControlKey(WPARAM wParam, LPARAM lParam)
{
    const UINT code = LOWORD(wParam);
    return code == VK_RCONTROL || (code == VK_CONTROL && (lParam & 0x01000000) != 0);
}

bool IsAltVk(UINT code)
{
    return code == VK_MENU || code == VK_LMENU || code == VK_RMENU;
}

bool IsWinVk(UINT code)
{
    return code == VK_LWIN || code == VK_RWIN;
}

bool IsOtherKeyboardKeyDown()
{
    // Mouse buttons occupy the first virtual-key values.  Start at Backspace
    // so holding a mouse button does not turn a bare Shift into a chord.
    for (UINT code = VK_BACK; code <= 0xfe; ++code)
    {
        if (!IsShiftVk(code) && (GetAsyncKeyState(code) & 0x8000) != 0)
        {
            return true;
        }
    }
    return false;
}

// GetKeyState() can lag behind the key message when a host calls TSF outside
// its normal message dispatch. Read the physical modifier state while arming a
// bare-key toggle so a stale thread snapshot cannot prevent the latch.
bool IsOnlyModifierPhysicallyDown(UINT keptDownVk)
{
    static const UINT modifiers[] = {VK_CONTROL, VK_MENU, VK_SHIFT, VK_LWIN, VK_RWIN};
    for (const UINT modifier : modifiers)
    {
        if (modifier == keptDownVk)
        {
            continue;
        }
        if ((GetAsyncKeyState(modifier) & 0x8000) != 0)
        {
            return false;
        }
    }
    return (GetAsyncKeyState(keptDownVk) & 0x8000) != 0;
}

void ClearReleasedShiftModifierState()
{
    if ((GetAsyncKeyState(VK_SHIFT) & 0x8000) != 0)
    {
        return;
    }

    Global::IsShiftKeyDownOnly = FALSE;
    Global::PureShiftKeyDown = FALSE;
    Global::PureShiftKeyUp = FALSE;
    Global::ModifiersValue &= ~(TF_MOD_SHIFT | TF_MOD_LSHIFT | TF_MOD_RSHIFT);
}
} // namespace

bool CMetasequoiaIME::_IsCompositionActiveForKeyGuard()
{
    if (_IsComposing() != FALSE) return true;
    if (_pCompositionProcessorEngine != nullptr && _pCompositionProcessorEngine->GetVirtualKeyLength() > 0)
        return true;
    return !GlobalIme::word_for_creating_word.empty();
}

bool CMetasequoiaIME::_ApplyBackspaceHoldGuard(WPARAM wParam, LPARAM lParam)
{
    if (static_cast<UINT>(wParam) != VK_BACK) return false;
    if (!IsAutoRepeat(lParam)) {
        _backspaceHoldArmed = _IsCompositionActiveForKeyGuard();
        return false;
    }
    return ShouldSuppressBackspaceRepeat(_backspaceHoldArmed, _IsCompositionActiveForKeyGuard(), true);
}

void CMetasequoiaIME::_InitBareShiftKeyboardHook()
{
    if (_bareShiftHook != nullptr || _bareShiftHookOwner != nullptr)
    {
        return;
    }

    _bareShiftHookOwner = this;
    _bareShiftHook = SetWindowsHookExW(WH_KEYBOARD, _BareShiftKeyboardHookProc, nullptr, GetCurrentThreadId());
    if (_bareShiftHook == nullptr)
    {
        _bareShiftHookOwner = nullptr;
    }
}

void CMetasequoiaIME::_UninitBareShiftKeyboardHook()
{
    HHOOK hook = _bareShiftHook;
    _bareShiftHook = nullptr;
    if (_bareShiftHookOwner == this)
    {
        _bareShiftHookOwner = nullptr;
    }
    if (hook != nullptr)
    {
        UnhookWindowsHookEx(hook);
    }

    _bareShiftDownMask = 0;
    _bareShiftArmed = false;
    _bareShiftFocusGeneration = 0;
    _bareShiftExpireTick = 0;
}

LRESULT CALLBACK CMetasequoiaIME::_BareShiftKeyboardHookProc(int code, WPARAM wParam, LPARAM lParam)
{
    CMetasequoiaIME *owner = _bareShiftHookOwner;
    if (code == HC_ACTION && owner != nullptr &&
        !IsSelfGeneratedSendInputExtraInfo(static_cast<ULONG_PTR>(GetMessageExtraInfo())))
    {
        const UINT virtualKey = static_cast<UINT>(wParam);
        const bool isShift = IsShiftVk(virtualKey);
        const bool isKeyUp = (static_cast<ULONG_PTR>(lParam) & 0x80000000u) != 0;
        const bool wasDown = (static_cast<ULONG_PTR>(lParam) & 0x40000000u) != 0;

        if (isShift)
        {
            const UINT scanCode = (static_cast<UINT>(lParam) >> 16) & 0x00ffu;
            const BYTE shiftBit = scanCode == 0x36 ? 0x02 : 0x01;
            if (!isKeyUp)
            {
                if (!wasDown)
                {
                    if (owner->_bareShiftDownMask == 0)
                    {
                        owner->_bareShiftArmed = !IsOtherKeyboardKeyDown();
                        ++owner->_bareShiftSequence;
                        if (owner->_bareShiftSequence == 0)
                        {
                            ++owner->_bareShiftSequence;
                        }
                        owner->_bareShiftFocusGeneration = owner->_deferredKeyFocusGeneration;
                        owner->_bareShiftExpireTick =
                            GetTickCount64() + static_cast<ULONGLONG>(kModifierHotkeyToggleLimit.count());
                    }
                    owner->_bareShiftDownMask |= shiftBit;
                }
            }
            else
            {
                owner->_bareShiftDownMask &= static_cast<BYTE>(~shiftBit);
                if (owner->_bareShiftDownMask == 0)
                {
                    const bool shouldPost =
                        owner->_bareShiftArmed && GetTickCount64() < owner->_bareShiftExpireTick;
                    owner->_bareShiftArmed = false;
                    if (shouldPost && owner->_msgWndHandle != nullptr)
                    {
                        PostMessage(owner->_msgWndHandle, WM_BareShiftRelease, owner->_bareShiftSequence, 0);
                    }
                }
            }
        }
        else if (!isKeyUp)
        {
            owner->_bareShiftArmed = false;
        }
    }

    return CallNextHookEx(owner ? owner->_bareShiftHook : nullptr, code, wParam, lParam);
}

void CMetasequoiaIME::_MarkBareShiftHandled()
{
    if (_bareShiftHook != nullptr)
    {
        _bareShiftHandledSequence = _bareShiftSequence;
    }
}

void CMetasequoiaIME::_HandleHookedBareShiftRelease(UINT sequence)
{
    if (_bareShiftHook == nullptr || sequence == 0 || sequence != _bareShiftSequence ||
        sequence == _bareShiftHandledSequence || _bareShiftFocusGeneration != _deferredKeyFocusGeneration ||
        !Global::g_connected || _pThreadMgr == nullptr || _pCompositionProcessorEngine == nullptr ||
        (GetAsyncKeyState(VK_SHIFT) & 0x8000) != 0 || !FanyUtils::ReadConfiguredSwitchLanguageHotkeys().shift)
    {
        return;
    }

    BOOL hasThreadFocus = FALSE;
    if (FAILED(_pThreadMgr->IsThreadFocus(&hasThreadFocus)) || !hasThreadFocus)
    {
        return;
    }

    ITfDocumentMgr *documentMgr = nullptr;
    ITfContext *context = nullptr;
    if (FAILED(_pThreadMgr->GetFocus(&documentMgr)) || documentMgr == nullptr)
    {
        return;
    }
    const HRESULT getTopResult = documentMgr->GetTop(&context);
    documentMgr->Release();
    if (FAILED(getTopResult) || context == nullptr)
    {
        return;
    }

    BOOL eaten = FALSE;
    const bool queued = _QueueInputHotkey(context, Global::MetasequoiaIMEGuidImeModePreserveKey, &eaten);
    DebugTsfIssue47(L"bare-shift-hook-release", FANY_IME_NO_REQUEST_ID, VK_SHIFT, L'\0', 0, 0, queued ? 1 : 0,
                    _IsComposing(),
                    _pCompositionProcessorEngine ? _pCompositionProcessorEngine->GetVirtualKeyLength() : 0, S_OK);
    if (queued)
    {
        _bareShiftHandledSequence = sequence;
        _shiftHotkeyArmed = false;
        _ctrlHotkeyArmed = false;
        Global::IsShiftKeyDownOnly = FALSE;
        Global::PureShiftKeyDown = FALSE;
        Global::PureShiftKeyUp = FALSE;
        Global::ModifiersValue &= ~(TF_MOD_SHIFT | TF_MOD_LSHIFT | TF_MOD_RSHIFT);
    }
    context->Release();
}

void CMetasequoiaIME::_TrackModifierHotkeyArming(WPARAM wParam, LPARAM lParam, bool isKeyUp)
{
    if (isKeyUp)
    {
        return;
    }

    const UINT code = LOWORD(wParam);
    const bool isShift = IsShiftVk(code);
    const bool isCtrl = IsControlVk(code);
    const bool isOtherModifier = IsAltVk(code) || IsWinVk(code);

    if (!isShift && !isCtrl && !isOtherModifier)
    {
        _shiftHotkeyArmed = false;
        _ctrlHotkeyArmed = false;
        return;
    }

    // Ignore auto-repeat; only the first down can arm a bare-modifier toggle.
    if ((lParam & 0x40000000) != 0)
    {
        return;
    }

    const auto now = std::chrono::steady_clock::now();
    if (isShift && IsOnlyModifierPhysicallyDown(VK_SHIFT))
    {
        _shiftHotkeyArmed = true;
        _ctrlHotkeyArmed = false;
        _modifierHotkeyExpire = now + kModifierHotkeyToggleLimit;
        return;
    }
    if (isCtrl && IsOnlyModifierPhysicallyDown(VK_CONTROL))
    {
        _ctrlHotkeyArmed = true;
        _shiftHotkeyArmed = false;
        _modifierHotkeyExpire = now + kModifierHotkeyToggleLimit;
        return;
    }

    _shiftHotkeyArmed = false;
    _ctrlHotkeyArmed = false;
}

bool CMetasequoiaIME::_MatchChordInputHotkey(WPARAM wParam, _Out_ GUID *hotkeyGuid) const
{
    if (hotkeyGuid == nullptr)
    {
        return false;
    }

    const UINT code = LOWORD(wParam);
    const bool shift = (GetKeyState(VK_SHIFT) & 0x8000) != 0;
    const bool ctrl = (GetKeyState(VK_CONTROL) & 0x8000) != 0;
    const bool alt = (GetKeyState(VK_MENU) & 0x8000) != 0;

    // Runs on every key-down: read the preferences only once the chord matches.
    if (code == VK_SPACE && ctrl && alt && !shift)
    {
        if (!FanyUtils::ReadConfiguredSwitchLanguageHotkeys().ctrl_alt_space)
        {
            return false;
        }
        *hotkeyGuid = Global::MetasequoiaIMEGuidImeModePreserveKey02;
        return true;
    }
    if (code == VK_SPACE && ctrl && shift && !alt)
    {
        *hotkeyGuid = Global::MetasequoiaIMEGuidDoubleSingleBytePreserveKey;
        return true;
    }
    if (code == VK_OEM_PERIOD && ctrl && !shift && !alt)
    {
        *hotkeyGuid = Global::MetasequoiaIMEGuidPunctuationPreserveKey;
        return true;
    }
    // The Korean keyboard's 한/영 key switches between Korean and English as Shift does, committing the open syllable first. VK_HANGUL shares its code with VK_KANA, so it only means this while the Korean scheme is active.
    if (code == msime::tsf::kVirtualKeyHangul && !ctrl && !alt &&
        Global::InputModeScheme.load(std::memory_order_relaxed) == msime::windows::scheme::Korean)
    {
        *hotkeyGuid = Global::MetasequoiaIMEGuidImeModePreserveKey;
        return true;
    }
    return false;
}

bool CMetasequoiaIME::_MatchModifierReleaseHotkey(WPARAM wParam, _Out_ GUID *hotkeyGuid)
{
    if (hotkeyGuid == nullptr)
    {
        return false;
    }

    const UINT code = LOWORD(wParam);
    const auto now = std::chrono::steady_clock::now();

    // The arming latch already proves that this modifier was pressed alone;
    // unlike PureShiftKeyUp, it does not depend on a host's stale GetKeyState.
    if (IsShiftVk(code) && _shiftHotkeyArmed)
    {
        const bool fire = now < _modifierHotkeyExpire && FanyUtils::ReadConfiguredSwitchLanguageHotkeys().shift;
        _shiftHotkeyArmed = false;
        _ctrlHotkeyArmed = false;
        if (fire)
        {
            *hotkeyGuid = Global::MetasequoiaIMEGuidImeModePreserveKey;
            return true;
        }
        return false;
    }

    if (IsControlVk(code) && _ctrlHotkeyArmed)
    {
        const bool fire = now < _modifierHotkeyExpire && FanyUtils::ReadConfiguredSwitchLanguageHotkeys().ctrl;
        _shiftHotkeyArmed = false;
        _ctrlHotkeyArmed = false;
        if (fire)
        {
            *hotkeyGuid = Global::MetasequoiaIMEGuidImeModePreserveKey03;
            return true;
        }
        return false;
    }

    return false;
}

bool CMetasequoiaIME::_QueueKoreanHanjaTap(_In_ ITfContext *pContext, WPARAM wParam, LPARAM lParam)
{
    // The arming latch proves the Ctrl was pressed alone, with no other key between press and release.
    if (pContext == nullptr || !_ctrlHotkeyArmed || !IsRightControlKey(wParam, lParam) ||
        std::chrono::steady_clock::now() >= _modifierHotkeyExpire ||
        Global::InputModeScheme.load(std::memory_order_relaxed) != msime::windows::scheme::Korean ||
        _pCompositionProcessorEngine == nullptr ||
        _IsKeyboardDisabled())
    {
        return false;
    }
    // Keys still queued ahead may be the ones composing the syllable this tap converts.
    const bool imeOpen = _deferredKeyProjectionValid
                             ? _deferredProjectedImeOpen
                             : _pCompositionProcessorEngine->GetIMEMode(_pThreadMgr, _tfClientId) != FALSE;
    const bool composing =
        _IsComposing() != FALSE || (_deferredKeyProjectionValid && _deferredProjectedInputLength > 0);
    if (!imeOpen || !composing || !_DeferredKeyQueueHasCapacity())
    {
        return false;
    }
    _shiftHotkeyArmed = false;
    _ctrlHotkeyArmed = false;
    // Queued as the Hanja key it stands for, so the Server receives that key and both sessions convert the same syllable.
    _KEYSTROKE_STATE hanjaState = {};
    hanjaState.Category = CATEGORY_COMPOSING;
    hanjaState.Function = FUNCTION_KOREAN_HANJA_KEY;
    if (!_QueueDeferredKeyDown(pContext, msime::tsf::kVirtualKeyHanja, 0, L'\0', 0, hanjaState))
    {
        return false;
    }
    if (_localSessionResetPending.load(std::memory_order_acquire))
    {
        const UINT resetToken = _localSessionResetToken.load(std::memory_order_acquire);
        _RequestLocalSessionReset(pContext, resetToken);
    }
    return true;
}

bool CMetasequoiaIME::_QueueInputHotkey(_In_ ITfContext *pContext, REFGUID hotkeyGuid, _Out_ BOOL *pIsEaten)
{
    if (pIsEaten == nullptr)
    {
        return false;
    }
    *pIsEaten = FALSE;
    if (pContext == nullptr || !_DeferredKeyQueueHasCapacity())
    {
        return false;
    }

    *pIsEaten = _QueueDeferredPreservedKey(pContext, hotkeyGuid) ? TRUE : FALSE;
    if (*pIsEaten && _localSessionResetPending.load(std::memory_order_acquire))
    {
        const UINT resetToken = _localSessionResetToken.load(std::memory_order_acquire);
        _RequestLocalSessionReset(pContext, resetToken);
    }
    return *pIsEaten != FALSE;
}

// Because the code mostly works with VKeys, here map a WCHAR back to a VKKey for certain
// vkeys that the IME handles specially
__inline UINT VKeyFromVKPacketAndWchar(UINT vk, WCHAR wch)
{
    UINT vkRet = vk;
    if (LOWORD(vk) == VK_PACKET)
    {
        if (wch == L' ')
        {
            vkRet = VK_SPACE;
        }
        else if ((wch >= L'0') && (wch <= L'9'))
        {
            vkRet = static_cast<UINT>(wch);
        }
        else if ((wch >= L'a') && (wch <= L'z'))
        {
            // Same VK as the uppercase letter (SampleIME mirrored the alphabet here).
            vkRet = static_cast<UINT>(L'A') + (static_cast<UINT>(wch) - static_cast<UINT>(L'a'));
        }
        else if ((wch >= L'A') && (wch <= L'Z'))
        {
            vkRet = static_cast<UINT>(wch);
        }
        else if (wch == THIRDPARTY_NEXTPAGE)
        {
            vkRet = VK_NEXT;
        }
        else if (wch == THIRDPARTY_PREVPAGE)
        {
            vkRet = VK_PRIOR;
        }
    }
    return vkRet;
}

//+---------------------------------------------------------------------------
//
// _IsKeyEaten
//
//----------------------------------------------------------------------------

BOOL CMetasequoiaIME::_IsKeyEaten(         //
    _In_ ITfContext *pContext,             //
    UINT codeIn,                           //
    _Out_ UINT *pCodeOut,                  //
    _Out_writes_(1) WCHAR *pwch,           //
    _Out_opt_ _KEYSTROKE_STATE *pKeyState, //
    _In_opt_ const WCHAR *translatedWch,   //
    bool freshCompositionState             //
)
{
    pContext;

    *pCodeOut = codeIn;

    BOOL isOpen = FALSE;
    CCompartment CompartmentKeyboardOpen(_pThreadMgr, _tfClientId, GUID_COMPARTMENT_KEYBOARD_OPENCLOSE);
    CompartmentKeyboardOpen._GetCompartmentBOOL(isOpen);

    BOOL isDoubleSingleByte = FALSE;
    CCompartment CompartmentDoubleSingleByte(_pThreadMgr, _tfClientId,
                                             Global::MetasequoiaIMEGuidCompartmentDoubleSingleByte);
    CompartmentDoubleSingleByte._GetCompartmentBOOL(isDoubleSingleByte);

    BOOL isPunctuation = FALSE;
    CCompartment CompartmentPunctuation(_pThreadMgr, _tfClientId, Global::MetasequoiaIMEGuidCompartmentPunctuation);
    CompartmentPunctuation._GetCompartmentBOOL(isPunctuation);

    if (pKeyState)
    {
        pKeyState->Category = CATEGORY_NONE;
        pKeyState->Function = FUNCTION_NONE;
    }
    if (pwch)
    {
        *pwch = L'\0';
    }

    // If the keyboard is disabled(e.g. no focused edit control), we don't eat keys.
    if (_IsKeyboardDisabled())
    {
        return FALSE;
    }

    //
    // Map virtual key to character code
    //
    BOOL isTouchKeyboardSpecialKeys = FALSE;
    WCHAR wch = translatedWch ? *translatedWch : ConvertVKey(codeIn);
    *pCodeOut = VKeyFromVKPacketAndWchar(codeIn, wch);
    if ((wch == THIRDPARTY_NEXTPAGE) || (wch == THIRDPARTY_PREVPAGE))
    {
        // We always eat the above softkeyboard special keys
        isTouchKeyboardSpecialKeys = TRUE;
        if (pwch)
        {
            *pwch = wch;
        }
    }

    // if the keyboard is closed, we don't eat keys, with the exception of the touch keyboard specials keys
    if (!isOpen && !isDoubleSingleByte && !isPunctuation)
    {
        return isTouchKeyboardSpecialKeys;
    }
    // 韩文、越南文和藏文在两种模式下都写半角 ASCII，所以键盘关闭时标点和全角开关没有东西可转换。
    const int scheme = Global::InputModeScheme.load(std::memory_order_relaxed);
    // 韩文、注音、越南文和藏文在 TIP 自己的宿主会话里组字（scheme::AlwaysInlinePreedit）。
    const bool hostComposed = msime::windows::scheme::AlwaysInlinePreedit(scheme);
    if (!isOpen && !msime::windows::scheme::UsesChinesePunctuation(scheme))
    {
        return isTouchKeyboardSpecialKeys;
    }

    if (pwch)
    {
        *pwch = wch;
    }

    //
    // Get composition engine
    //
    CCompositionProcessorEngine *pCompositionProcessorEngine;
    pCompositionProcessorEngine = _pCompositionProcessorEngine;

    if (isOpen) // Chinese mode
    {
        // AltGr reads as Ctrl+Alt; a character it types is input, not a shortcut (AltGr+0 is '@' on AZERTY).
        const UINT shortcutModifiers = Global::CharacterModifiers(CaptureIpcModifiers(), wch, *pCodeOut);
        if (!_serverUnavailableFallbackActive && IsCharacterSetInputModeToggle(*pCodeOut, shortcutModifiers))
        {
            if (pKeyState)
            {
                pKeyState->Category = CATEGORY_COMPOSING;
                pKeyState->Function = FUNCTION_TOGGLE_CHARACTER_SET;
            }
            return TRUE;
        }
        // Keep the Chinese compartment open: this only toggles the Server-owned English candidate sub-mode. Korean, Zhuyin and Vietnamese have no such mode here: the TIP composes them in its own host session, which would not follow a mode only the Server session entered, so the chord belongs to the application like any other Ctrl chord.
        if (!hostComposed && IsEnglishInputModeToggle(*pCodeOut, shortcutModifiers))
        {
            if (pKeyState)
            {
                pKeyState->Category = CATEGORY_COMPOSING;
                pKeyState->Function = FUNCTION_CANCEL;
            }
            return TRUE;
        }

        // Korean's only list is the Hanja list. Its rows may show a translation under their 훈음, but both are display only: the TIP composes Hangul in its own host session, which a translation committed by the Server would leave behind, and the Server refuses the key for Korean too. Zhuyin and Vietnamese compose there as well. It stays the application's like any other Ctrl chord.
        if (!hostComposed && !freshCompositionState && _candidateMode != CANDIDATE_NONE &&
            IsTranslationCommitShortcut(*pCodeOut, shortcutModifiers))
        {
            if (pKeyState)
            {
                pKeyState->Category = CATEGORY_CANDIDATE;
                pKeyState->Function = FUNCTION_SERVER_CANDIDATE_KEY;
            }
            return TRUE;
        }

        // Other Ctrl/Alt/Windows combinations belong to the application.
        // IME-owned shortcuts are handled before this normal key classifier.
        if ((shortcutModifiers & 0b00000110u) != 0 || (GetAsyncKeyState(VK_LWIN) & 0x8000) != 0 ||
            (GetAsyncKeyState(VK_RWIN) & 0x8000) != 0)
        {
            const KEYSTROKE_FUNCTION segmentEdit = SegmentEditFunction(*pCodeOut, shortcutModifiers);
            if (segmentEdit != FUNCTION_NONE && _IsComposing())
            {
                if (pKeyState)
                {
                    pKeyState->Category = CATEGORY_COMPOSING;
                    pKeyState->Function = segmentEdit;
                }
                return TRUE;
            }
            return isTouchKeyboardSpecialKeys;
        }

        // Korean has no Chinese candidates, no Chinese punctuation and no smart punctuation to arm, and Zhuyin and Vietnamese compose in the host session the same way, so their keys are settled here before any of those paths can claim them (see host_composed_key_action in HostKoreanKey.h). Korean reads its Hanja list from the host session only for the keys that list takes; Zhuyin and Vietnamese read the view for the keys it spells with.
        if (hostComposed)
        {
            const bool hostComposing = !freshCompositionState && _IsComposing() != FALSE;
            std::string spellingSymbols;
            bool listOpen = false;
            if (scheme == msime::windows::scheme::Korean)
            {
                listOpen = hostComposing && msime::tsf::is_korean_hanja_list_key(*pCodeOut, wch) &&
                           _IsKoreanHanjaListOpen();
            }
            else
            {
                const auto hostView = _ReadHostComposedView();
                listOpen = hostComposing && hostView.listOpen;
                spellingSymbols = hostView.spellingSymbols;
            }
            switch (msime::tsf::host_composed_key_action(scheme, *pCodeOut, wch, hostComposing, listOpen,
                                                         spellingSymbols))
            {
            case msime::tsf::KoreanKeyAction::ConvertHanja:
            case msime::tsf::KoreanKeyAction::HanjaList:
                if (pKeyState)
                {
                    pKeyState->Category = CATEGORY_COMPOSING;
                    pKeyState->Function = FUNCTION_KOREAN_HANJA_KEY;
                }
                return TRUE;
            case msime::tsf::KoreanKeyAction::Compose:
                if (pwch && msime::windows::scheme::FoldsLetterCase(scheme))
                {
                    // A VK_PACKET letter (touch keyboard, injected text) carries its case itself; a physical key takes it from Shift, never from Caps Lock.
                    const bool upper = LOWORD(codeIn) == VK_PACKET ? (wch >= L'A' && wch <= L'Z')
                                                                   : (shortcutModifiers & 0b00000001u) != 0;
                    *pwch = msime::tsf::korean_letter(wch, upper);
                }
                if (pKeyState)
                {
                    pKeyState->Category = CATEGORY_COMPOSING;
                    pKeyState->Function = FUNCTION_INPUT;
                }
                return TRUE;
            case msime::tsf::KoreanKeyAction::CommitWithText:
                if (pKeyState)
                {
                    pKeyState->Category = CATEGORY_COMPOSING;
                    pKeyState->Function = FUNCTION_COMMIT_SYLLABLE;
                }
                return TRUE;
            case msime::tsf::KoreanKeyAction::CommitAndPass:
                // Not eaten: the syllable is committed in an edit session and the key then does its own work in the application.
                if (pKeyState)
                {
                    pKeyState->Category = CATEGORY_INVOKE_COMPOSITION_EDIT_SESSION;
                    pKeyState->Function = FUNCTION_COMMIT_SYLLABLE;
                }
                return FALSE;
            case msime::tsf::KoreanKeyAction::Pass:
                return isTouchKeyboardSpecialKeys;
            case msime::tsf::KoreanKeyAction::Default:
                break;
            }
        }

        const bool isComposing = freshCompositionState ? false : _IsComposing() != FALSE;
        const CANDIDATE_MODE candidateMode = freshCompositionState ? CANDIDATE_NONE : _candidateMode;
        const bool isCapsLockOn = (GetKeyState(VK_CAPITAL) & 0x0001) != 0;
        const bool isUppercaseAlphabet = (wch >= L'A' && wch <= L'Z') && (*pCodeOut >= L'A' && *pCodeOut <= L'Z');
        const bool isInputInProgress =
            !freshCompositionState &&
            (isComposing || (candidateMode != CANDIDATE_NONE) ||
             (pCompositionProcessorEngine && pCompositionProcessorEngine->GetVirtualKeyLength() > 0));

        // A following space is a local document rewrite only when the last
        // commit armed the smart-punctuation action. Candidate conversion and
        // composition spaces retain their normal Server-owned semantics.
        if (codeIn == VK_SPACE && !isInputInProgress && candidateMode == CANDIDATE_NONE &&
            _CanConvertSmartPunctuationSpace())
        {
            if (pKeyState)
            {
                pKeyState->Category = CATEGORY_COMPOSING;
                pKeyState->Function = FUNCTION_SMART_PUNCTUATION_CONVERT;
            }
            return TRUE;
        }
        if (!isInputInProgress && candidateMode == CANDIDATE_NONE && _CanRevertSmartPunctuation(wch))
        {
            if (pKeyState)
            {
                pKeyState->Category = CATEGORY_COMPOSING;
                pKeyState->Function = FUNCTION_SMART_PUNCTUATION_REVERT;
            }
            return TRUE;
        }

        // CapsLock ON + uppercase(没有按 Shift) alphabet:
        // - start of input: don't eat
        // - middle of input: eat
        if (isCapsLockOn && isUppercaseAlphabet && !isInputInProgress)
        {
            return isTouchKeyboardSpecialKeys;
        }

        // With nothing composing, a Stroke letter other than the five strokes is the application's: the Engine answers handled=false for it, so the TIP must not eat it (scheme::LetterPassesWhileIdle). Not in the Engine's own English mode, which composes every letter.
        if (!isInputInProgress &&
            msime::windows::scheme::LetterPassesWhileIdle(
                scheme, Global::DedicatedEnglish.active(GetTickCount64()), wch))
        {
            return isTouchKeyboardSpecialKeys;
        }

        // "/" and "@" open their modes on an empty composition instead of typing a mark: they start the composition, and the Server hands them to the Engine as its first character.
        if (!isInputInProgress && candidateMode == CANDIDATE_NONE &&
            Global::OpensLocalMode(wch, false,
                                   isPunctuation != FALSE && Global::PunctuationLockMode.load(std::memory_order_relaxed) !=
                                                                 Global::PunctuationLock::AlwaysEnglish,
                                   Global::CommandModeEnabled.load(std::memory_order_relaxed),
                                   Global::MentionModeEnabled.load(std::memory_order_relaxed)))
        {
            if (pKeyState)
            {
                pKeyState->Category = CATEGORY_COMPOSING;
                pKeyState->Function = FUNCTION_INPUT;
            }
            return TRUE;
        }

        //
        // The candidate or phrase list handles the keys through ITfKeyEventSink.
        //
        // eat only keys that CKeyHandlerEditSession can handles.
        //
        const BOOL ret =
            freshCompositionState
                ? pCompositionProcessorEngine->IsVirtualKeyNeedForFreshComposition(*pCodeOut, pwch, pKeyState)
                : pCompositionProcessorEngine->IsVirtualKeyNeed(*pCodeOut, pwch, isComposing, candidateMode,
                                                                _isCandidateWithWildcard, pKeyState);
        if (ret)
        {
            return TRUE;
        }
    }

    //
    // Punctuation
    //
    if (pCompositionProcessorEngine->IsPunctuation(wch))
    {
        const CANDIDATE_MODE candidateMode = freshCompositionState ? CANDIDATE_NONE : _candidateMode;
        if ((candidateMode == CANDIDATE_NONE || candidateMode == CANDIDATE_INCREMENTAL) && isPunctuation)
        {
            if (pKeyState)
            {
                pKeyState->Category = CATEGORY_COMPOSING;
                pKeyState->Function = FUNCTION_PUNCTUATION;
            }
            return TRUE;
        }
    }

    //
    // Double/Single byte
    //
    if (isDoubleSingleByte && pCompositionProcessorEngine->IsDoubleSingleByte(wch))
    {
        if ((freshCompositionState ? CANDIDATE_NONE : _candidateMode) == CANDIDATE_NONE)
        {
            if (pKeyState)
            {
                pKeyState->Category = CATEGORY_COMPOSING;
                pKeyState->Function = FUNCTION_DOUBLE_SINGLE_BYTE;
            }
            return TRUE;
        }
    }

    return isTouchKeyboardSpecialKeys;
}

//+---------------------------------------------------------------------------
//
// ConvertVKey
//
//----------------------------------------------------------------------------

WCHAR CMetasequoiaIME::ConvertVKey(UINT code)
{
    //
    // Map virtual key to scan code
    //
    UINT scanCode = 0;
    scanCode = MapVirtualKey(code, 0);

    //
    // Keyboard state
    //
    BYTE abKbdState[256] = {'\0'};
    if (!GetKeyboardState(abKbdState))
    {
        return 0;
    }

    //
    // Map virtual key to character code
    //
    WCHAR wch = '\0';
    if (ToUnicode(code, scanCode, abKbdState, &wch, 1, 0) == 1)
    {
        return wch;
    }

    return 0;
}

//+---------------------------------------------------------------------------
//
// _IsKeyboardDisabled
//
//----------------------------------------------------------------------------

BOOL CMetasequoiaIME::_IsKeyboardDisabled()
{
    /* Steal from weasel: https://github.com/rime/weasel */
    ITfCompartmentMgr *pCompMgr = NULL;
    ITfDocumentMgr *pDocMgrFocus = NULL;
    ITfContext *pContext = NULL;
    BOOL fDisabled = FALSE;

    if ((_pThreadMgr->GetFocus(&pDocMgrFocus) != S_OK) || (pDocMgrFocus == NULL))
    {
        fDisabled = TRUE;
        goto Exit;
    }

    if ((pDocMgrFocus->GetTop(&pContext) != S_OK) || (pContext == NULL))
    {
        fDisabled = TRUE;
        goto Exit;
    }

    if (pContext->QueryInterface(IID_ITfCompartmentMgr, (void **)&pCompMgr) == S_OK)
    {
        ITfCompartment *pCompartmentDisabled;
        ITfCompartment *pCompartmentEmptyContext;

        /* Check GUID_COMPARTMENT_KEYBOARD_DISABLED */
        if (pCompMgr->GetCompartment(GUID_COMPARTMENT_KEYBOARD_DISABLED, &pCompartmentDisabled) == S_OK)
        {
            VARIANT var;
            if (pCompartmentDisabled->GetValue(&var) == S_OK)
            {
                // Even VT_EMPTY, GetValue() can succeed. Either compartment
                // disables input; one must not clear the other.
                if (var.vt == VT_I4 && var.lVal != 0)
                    fDisabled = TRUE;
            }
            pCompartmentDisabled->Release();
        }

        /* Check GUID_COMPARTMENT_EMPTYCONTEXT */
        if (pCompMgr->GetCompartment(GUID_COMPARTMENT_EMPTYCONTEXT, &pCompartmentEmptyContext) == S_OK)
        {
            VARIANT var;
            if (pCompartmentEmptyContext->GetValue(&var) == S_OK)
            {
                if (var.vt == VT_I4 && var.lVal != 0) // Even VT_EMPTY, GetValue() can succeed
                    fDisabled = TRUE;
            }
            pCompartmentEmptyContext->Release();
        }
        pCompMgr->Release();
    }

Exit:
    if (pContext)
        pContext->Release();
    if (pDocMgrFocus)
        pDocMgrFocus->Release();
    return fDisabled;
}

//+---------------------------------------------------------------------------
//
// ITfKeyEventSink::OnSetFocus
//
// Called by the system whenever this service gets the keystroke device focus.
//----------------------------------------------------------------------------

STDAPI CMetasequoiaIME::OnSetFocus(BOOL fForeground)
{
    fForeground;

    _backspaceHoldArmed = false;

    return S_OK;
}

bool CMetasequoiaIME::_HasDeferredKeyBarrier() const
{
    if (_localSessionResetPending.load(std::memory_order_acquire) || _focusResetPending || _activationRequired ||
        _deferredKeyProjectionValid || !_deferredKeyDowns.empty() || _hasDeferredKeyInFlight)
    {
        return true;
    }
    if (Global::g_connected)
    {
        const uint64_t expectedToken = _expectedWorkerFocusToken.load(std::memory_order_acquire);
        return expectedToken == 0 || _acknowledgedWorkerFocusToken.load(std::memory_order_acquire) != expectedToken ||
               !_workerCommitReady.load(std::memory_order_acquire);
    }
    return false;
}

bool CMetasequoiaIME::_DeferredKeyQueueHasCapacity() const
{
    size_t deferredCount = _deferredKeyDowns.size() + _deferredAppliedPrefix.size();
    if (_hasDeferredKeyInFlight)
    {
        ++deferredCount;
    }
    return deferredCount < MAX_DEFERRED_KEY_DOWN_COUNT;
}

void CMetasequoiaIME::_EnsureDeferredKeyProjection()
{
    if (_deferredKeyProjectionValid)
    {
        return;
    }
    _deferredKeyProjectionValid = true;
    _deferredProjectedImeOpen = _pCompositionProcessorEngine && _pThreadMgr &&
                                _pCompositionProcessorEngine->GetIMEMode(_pThreadMgr, _tfClientId) != FALSE;
    _deferredProjectedPunctuationOpen =
        _pCompositionProcessorEngine && _pThreadMgr &&
        _pCompositionProcessorEngine->GetPunctuationMode(_pThreadMgr, _tfClientId) != FALSE;
    _deferredProjectedDoubleSingleByteOpen =
        _pCompositionProcessorEngine && _pThreadMgr &&
        _pCompositionProcessorEngine->GetDoubleSingleByteMode(_pThreadMgr, _tfClientId) != FALSE;
    // In the healthy path every IME-owned key enters the FIFO as well, so its
    // first projection starts from the composition that is already visible.
    // A transport-recovery checkpoint arms this projection before the local
    // reset cancels that composition.
    _deferredProjectedInputLength = _pCompositionProcessorEngine
                                        ? min(static_cast<size_t>(MAX_PINYIN_LENGTH),
                                              static_cast<size_t>(_pCompositionProcessorEngine->GetVirtualKeyLength()))
                                        : 0;
    _deferredProjectedRawInput.clear();
    _deferredProjectedCaret = 0;
    if (_pCompositionProcessorEngine)
    {
        const CStringRange &buffer = _pCompositionProcessorEngine->GetKeystrokeBuffer();
        if (buffer.Get() && buffer.GetLength() > 0)
        {
            _deferredProjectedRawInput.assign(buffer.Get(), buffer.GetLength());
        }
        _deferredProjectedCaret = min(static_cast<size_t>(_pCompositionProcessorEngine->GetCaretPosition()),
                                      _deferredProjectedRawInput.size());
    }
    // Incremental candidates are still the ordinary composing path: another
    // letter extends the same raw input.  Only an explicit/original candidate
    // list makes the next input a finalize-and-start-new boundary.
    _deferredProjectedCandidateActive = _candidateMode == CANDIDATE_ORIGINAL;
    _deferredProjectedUnicodeMode =
        _pCompositionProcessorEngine && _pCompositionProcessorEngine->IsUnicodeModeComposition() != FALSE;
    _deferredProjectedUrlMode =
        _pCompositionProcessorEngine && _pCompositionProcessorEngine->IsUrlModeComposition() != FALSE;
    _deferredProjectedKoreanHanjaListOpen =
        msime::windows::scheme::OpensCandidateList(Global::InputModeScheme.load(std::memory_order_relaxed)) &&
        _IsKoreanHanjaListOpen();
}

void CMetasequoiaIME::_ApplyDeferredKeyProjection(const _KEYSTROKE_STATE &keyState, WCHAR wch, UINT code)
{
    _EnsureDeferredKeyProjection();
    DeferredShadowState shadow;
    shadow.imeOpen = _deferredProjectedImeOpen;
    shadow.punctuationOpen = _deferredProjectedPunctuationOpen;
    shadow.doubleSingleByteOpen = _deferredProjectedDoubleSingleByteOpen;
    shadow.inputLength = _deferredProjectedInputLength;
    shadow.rawInput = _deferredProjectedRawInput;
    shadow.caret = _deferredProjectedCaret;
    shadow.candidateActive = _deferredProjectedCandidateActive;
    shadow.unicodeMode = _deferredProjectedUnicodeMode;
    shadow.urlMode = _deferredProjectedUrlMode;
    shadow.koreanHanjaListOpen = _deferredProjectedKoreanHanjaListOpen;
    ApplyDeferredKeyState(shadow, keyState, wch, code);
    if (!shadow.projectionValid)
    {
        _deferredKeyProjectionValid = false;
        _deferredProjectedInputLength = 0;
        _deferredProjectedRawInput.clear();
        _deferredProjectedCaret = 0;
        _deferredProjectedCandidateActive = false;
        _deferredProjectedUnicodeMode = false;
        _deferredProjectedUrlMode = false;
        _deferredProjectedKoreanHanjaListOpen = false;
        return;
    }
    _deferredProjectedInputLength = shadow.inputLength;
    _deferredProjectedRawInput = std::move(shadow.rawInput);
    _deferredProjectedCaret = shadow.caret;
    _deferredProjectedCandidateActive = shadow.candidateActive;
    _deferredProjectedUnicodeMode = shadow.unicodeMode;
    _deferredProjectedUrlMode = shadow.urlMode;
    _deferredProjectedKoreanHanjaListOpen = shadow.koreanHanjaListOpen;
    if (keyState.Function == FUNCTION_BACKSPACE && shadow.inputLength == 0)
        _backspaceHoldArmed = true;
}

void CMetasequoiaIME::_ApplyDeferredPreservedKeyProjection(REFGUID preservedKey)
{
    _EnsureDeferredKeyProjection();
    if (_pCompositionProcessorEngine == nullptr)
    {
        return;
    }
    switch (_pCompositionProcessorEngine->GetPreservedKeyAction(preservedKey))
    {
    case CCompositionProcessorEngine::PreservedKeyAction::ToggleImeMode:
        FanyUtils::RefreshPunctuationLockFromConfig();
        _deferredProjectedImeOpen = !_deferredProjectedImeOpen;
        _deferredProjectedPunctuationOpen = Global::ResolvePunctuationOpen(_deferredProjectedImeOpen) != FALSE;
        _deferredProjectedInputLength = 0;
        _deferredProjectedRawInput.clear();
        _deferredProjectedCaret = 0;
        _deferredProjectedCandidateActive = false;
        _deferredProjectedUnicodeMode = false;
        _deferredProjectedUrlMode = false;
        _deferredProjectedKoreanHanjaListOpen = false;
        break;
    case CCompositionProcessorEngine::PreservedKeyAction::ToggleDoubleSingleByteMode:
        _deferredProjectedDoubleSingleByteOpen = !_deferredProjectedDoubleSingleByteOpen;
        break;
    case CCompositionProcessorEngine::PreservedKeyAction::TogglePunctuationMode:
        FanyUtils::RefreshPunctuationLockFromConfig();
        _deferredProjectedPunctuationOpen = Global::ResolvePunctuationOpen(!_deferredProjectedPunctuationOpen) != FALSE;
        break;
    default:
        break;
    }
}

bool CMetasequoiaIME::_RefreshDeferredRecoveryPrefix(_In_ ITfContext *pContext)
{
    while (!_deferredAppliedPrefix.empty())
    {
        ITfContext *prefixContext = _deferredAppliedPrefix.front().context;
        _deferredAppliedPrefix.pop_front();
        if (prefixContext)
        {
            prefixContext->Release();
        }
    }

    if (pContext == nullptr || _pCompositionProcessorEngine == nullptr)
    {
        return false;
    }

    CStringRange &raw = _pCompositionProcessorEngine->GetKeystrokeBuffer();
    const size_t rawLength = static_cast<size_t>(raw.GetLength());
    const size_t caret = min(rawLength, static_cast<size_t>(_pCompositionProcessorEngine->GetCaretPosition()));
    const size_t moveLeftCount = rawLength - caret;
    if (rawLength + moveLeftCount > MAX_DEFERRED_KEY_DOWN_COUNT)
    {
        return false;
    }

    for (size_t index = 0; index < rawLength; ++index)
    {
        const WCHAR wch = raw.Get()[index];
        const SHORT virtualKey = VkKeyScanW(wch);
        DeferredKeyDown recoveryKey;
        recoveryKey.kind = DeferredKeyDown::Kind::KeyDown;
        recoveryKey.context = pContext;
        recoveryKey.wParam =
            virtualKey == -1 ? static_cast<WPARAM>(VK_PACKET) : static_cast<WPARAM>(LOBYTE(virtualKey));
        recoveryKey.translatedWch = wch;
        recoveryKey.modifiersDown = virtualKey != -1 && (HIBYTE(virtualKey) & 1) != 0 ? 1u : 0u;
        recoveryKey.keyState.Category = CATEGORY_COMPOSING;
        recoveryKey.keyState.Function = FUNCTION_INPUT;
        recoveryKey.focusGeneration = _deferredKeyFocusGeneration;
        pContext->AddRef();
        _deferredAppliedPrefix.push_back(recoveryKey);
    }
    for (size_t index = 0; index < moveLeftCount; ++index)
    {
        DeferredKeyDown recoveryKey;
        recoveryKey.kind = DeferredKeyDown::Kind::KeyDown;
        recoveryKey.context = pContext;
        recoveryKey.wParam = VK_LEFT;
        recoveryKey.keyState.Category = CATEGORY_COMPOSING;
        recoveryKey.keyState.Function = FUNCTION_MOVE_LEFT;
        recoveryKey.focusGeneration = _deferredKeyFocusGeneration;
        pContext->AddRef();
        _deferredAppliedPrefix.push_back(recoveryKey);
    }
    return true;
}

void CMetasequoiaIME::_ArmDeferredRecoveryForTransport(_In_opt_ ITfContext *pContext)
{
    // An in-flight key owns its exact retry token.  Its failure path moves the
    // checkpoint in front of that key; moving it here as well would duplicate
    // the prefix.
    if (_hasDeferredKeyInFlight)
    {
        return;
    }

    ITfContext *recoveryContext = pContext ? pContext : _pContext;
    const bool activeDeferredState = !_deferredKeyDowns.empty() || _deferredKeyProjectionValid;
    if (!activeDeferredState)
    {
        // The dormant checkpoint may have been superseded by a non-eaten
        // application key that finalized/cancelled the composition.  Refresh
        // from the engine at the transport boundary instead of trusting stale
        // history.
        if (recoveryContext)
        {
            (void)_RefreshDeferredRecoveryPrefix(recoveryContext);
        }
    }
    else if (_deferredAppliedPrefix.empty())
    {
        // A retry has already materialized the checkpoint in the FIFO.  The
        // real engine can still contain the same raw text until the queued
        // reset edit session runs, so snapshotting it again would duplicate
        // the whole prefix.
        return;
    }
    if (_deferredAppliedPrefix.empty())
    {
        return;
    }

    // Capture the current real state before the local reset cancels it.  The
    // queued checkpoint then represents that same future state while reset is
    // pending, so later key classification remains ordered.
    _EnsureDeferredKeyProjection();
    while (!_deferredAppliedPrefix.empty())
    {
        _deferredKeyDowns.push_front(_deferredAppliedPrefix.back());
        _deferredAppliedPrefix.pop_back();
    }
    _ScheduleDeferredKeyDownDrain();
}

bool CMetasequoiaIME::_ClassifyDeferredKeyDown(_In_ ITfContext *pContext, WPARAM wParam, LPARAM lParam,
                                               _In_opt_ const WCHAR *translatedWch, _In_opt_ const UINT *modifiersDown,
                                               _Out_ WCHAR *classifiedWch, _Out_ UINT *classifiedCode,
                                               _Out_ _KEYSTROKE_STATE *keyState)
{
    if (pContext == nullptr || classifiedWch == nullptr || classifiedCode == nullptr || keyState == nullptr ||
        _pCompositionProcessorEngine == nullptr || _pThreadMgr == nullptr)
    {
        return false;
    }

    *classifiedWch = translatedWch ? *translatedWch : ConvertVKey(static_cast<UINT>(wParam));
    *classifiedCode = VKeyFromVKPacketAndWchar(static_cast<UINT>(wParam), *classifiedWch);
    keyState->Category = CATEGORY_NONE;
    keyState->Function = FUNCTION_NONE;

    // AltGr reads as Ctrl+Alt; a character it types is input, not a shortcut (AltGr+0 is '@' on AZERTY).
    const UINT capturedModifiers =
        Global::CharacterModifiers(modifiersDown ? *modifiersDown : CaptureIpcModifiers(), *classifiedWch,
                                   *classifiedCode);
    const bool projectedImeOpen = _deferredKeyProjectionValid
                                      ? _deferredProjectedImeOpen
                                      : _pCompositionProcessorEngine->GetIMEMode(_pThreadMgr, _tfClientId) != FALSE;
    if (projectedImeOpen && !_serverUnavailableFallbackActive && !_IsKeyboardDisabled() &&
        IsCharacterSetInputModeToggle(*classifiedCode, capturedModifiers))
    {
        keyState->Category = CATEGORY_COMPOSING;
        keyState->Function = FUNCTION_TOGGLE_CHARACTER_SET;
        return true;
    }
    // Korean, Zhuyin and Vietnamese have no Server-owned English candidate mode, as in _IsKeyEaten.
    const int scheme = Global::InputModeScheme.load(std::memory_order_relaxed);
    if (projectedImeOpen && !msime::windows::scheme::AlwaysInlinePreedit(scheme) &&
        IsEnglishInputModeToggle(*classifiedCode, capturedModifiers))
    {
        keyState->Category = CATEGORY_COMPOSING;
        keyState->Function = FUNCTION_CANCEL;
        return true;
    }

    const bool projectedCandidateActive =
        _deferredKeyProjectionValid ? _deferredProjectedCandidateActive : (_candidateMode == CANDIDATE_ORIGINAL);
    if (projectedImeOpen && !_IsKeyboardDisabled() && projectedCandidateActive &&
        IsTranslationCommitShortcut(*classifiedCode, capturedModifiers))
    {
        keyState->Category = CATEGORY_CANDIDATE;
        keyState->Function = FUNCTION_SERVER_CANDIDATE_KEY;
        return true;
    }

    // Segment edits are the only Ctrl chords claimed while a projected
    // composition is live; all other modifier combinations belong to the host.
    const bool projectedCompositionActive = _deferredKeyProjectionValid
                                                ? (_deferredProjectedInputLength > 0 ||
                                                   _deferredProjectedCandidateActive)
                                                : (_pCompositionProcessorEngine->GetVirtualKeyLength() > 0 ||
                                                   _candidateMode != CANDIDATE_NONE);
    const KEYSTROKE_FUNCTION segmentEdit = SegmentEditFunction(*classifiedCode, capturedModifiers);
    if (segmentEdit != FUNCTION_NONE && !_IsKeyboardDisabled() && projectedCompositionActive)
    {
        keyState->Category = CATEGORY_COMPOSING;
        keyState->Function = segmentEdit;
        return true;
    }
    if ((capturedModifiers & 0b00000110) != 0 || (GetAsyncKeyState(VK_LWIN) & 0x8000) != 0 ||
        (GetAsyncKeyState(VK_RWIN) & 0x8000) != 0 || IsBareModifierKey(*classifiedCode) || _IsKeyboardDisabled())
    {
        return false;
    }

    DeferredShadowState shadow;
    if (_deferredKeyProjectionValid)
    {
        shadow.imeOpen = _deferredProjectedImeOpen;
        shadow.punctuationOpen = _deferredProjectedPunctuationOpen;
        shadow.doubleSingleByteOpen = _deferredProjectedDoubleSingleByteOpen;
        shadow.inputLength = _deferredProjectedInputLength;
        shadow.rawInput = _deferredProjectedRawInput;
        shadow.caret = _deferredProjectedCaret;
        shadow.candidateActive = _deferredProjectedCandidateActive;
        shadow.unicodeMode = _deferredProjectedUnicodeMode;
        shadow.urlMode = _deferredProjectedUrlMode;
        shadow.koreanHanjaListOpen = _deferredProjectedKoreanHanjaListOpen;
    }
    else
    {
        shadow.imeOpen = _pCompositionProcessorEngine->GetIMEMode(_pThreadMgr, _tfClientId) != FALSE;
        shadow.punctuationOpen = _pCompositionProcessorEngine->GetPunctuationMode(_pThreadMgr, _tfClientId) != FALSE;
        shadow.doubleSingleByteOpen =
            _pCompositionProcessorEngine->GetDoubleSingleByteMode(_pThreadMgr, _tfClientId) != FALSE;
        shadow.inputLength = min(static_cast<size_t>(MAX_PINYIN_LENGTH),
                                 static_cast<size_t>(_pCompositionProcessorEngine->GetVirtualKeyLength()));
        const CStringRange &buffer = _pCompositionProcessorEngine->GetKeystrokeBuffer();
        if (buffer.Get() && buffer.GetLength() > 0)
        {
            shadow.rawInput.assign(buffer.Get(), buffer.GetLength());
        }
        shadow.caret =
            min(static_cast<size_t>(_pCompositionProcessorEngine->GetCaretPosition()), shadow.rawInput.size());
        shadow.candidateActive = _candidateMode == CANDIDATE_ORIGINAL;
        shadow.unicodeMode = _pCompositionProcessorEngine->IsUnicodeModeComposition() != FALSE;
        shadow.urlMode = _pCompositionProcessorEngine->IsUrlModeComposition() != FALSE;
        shadow.koreanHanjaListOpen =
            msime::windows::scheme::OpensCandidateList(Global::InputModeScheme.load(std::memory_order_relaxed)) &&
            _IsKoreanHanjaListOpen();
    }

    if (static_cast<UINT>(wParam) == VK_BACK && _backspaceHoldArmed && IsAutoRepeat(lParam) &&
        shadow.inputLength == 0 && !shadow.candidateActive)
    {
        keyState->Category = CATEGORY_COMPOSING;
        keyState->Function = FUNCTION_BACKSPACE;
        return true;
    }

    const auto setKeyState = [keyState](KEYSTROKE_CATEGORY category, KEYSTROKE_FUNCTION function) {
        keyState->Category = category;
        keyState->Function = function;
        return true;
    };

    if (!shadow.imeOpen && !msime::windows::scheme::UsesChinesePunctuation(scheme))
    {
        // Korean and Vietnamese write half-width ASCII with the keyboard closed too; queue printable keys as application text so they keep their place behind earlier keys.
        return *classifiedWch != L'\0' && std::iswprint(static_cast<wint_t>(*classifiedWch)) != 0;
    }
    if (shadow.imeOpen && msime::windows::scheme::AlwaysInlinePreedit(scheme))
    {
        // Keys queued ahead of this one may still be composing, so a composition counts as open when the projection or the document has one.
        const bool composing = shadow.inputLength > 0 || _IsComposing() != FALSE;
        const bool listOpen = composing && shadow.koreanHanjaListOpen;
        // A queued key cannot see the view, so the keys a composition spells with come from the scheme's static rules; the live view decides again when the key runs (_HandleCompositionInput).
        std::string_view spellingSymbols;
        if (scheme == msime::windows::scheme::Zhuyin)
            spellingSymbols = !composing  ? msime::tsf::kZhuyinIdleSymbols
                              : listOpen ? msime::tsf::kZhuyinListOpenSymbols
                                         : msime::tsf::kZhuyinComposingSymbols;
        else if (scheme == msime::windows::scheme::Vietnamese && composing)
            spellingSymbols = msime::tsf::kVietnameseVniDigits;
        else if (scheme == msime::windows::scheme::Tibetan)
            spellingSymbols = composing ? msime::tsf::kTibetanComposingSymbols : msime::tsf::kTibetanIdleSymbols;
        // Keys queued ahead may also open or close the list, so the list is read from the projection, which carries it forward from the host session's. With the list projected closed every key keeps the action it has without one, which is what commits a composition ended by an arrow so a Backspace queued after it still reaches the application; with it projected open the list's keys become FUNCTION_KOREAN_HANJA_KEY, which decides against the host session when it runs, as the Server does against its own.
        switch (msime::tsf::host_composed_key_action(scheme, *classifiedCode, *classifiedWch, composing, listOpen,
                                                     spellingSymbols))
        {
        case msime::tsf::KoreanKeyAction::ConvertHanja:
        case msime::tsf::KoreanKeyAction::HanjaList:
            return setKeyState(CATEGORY_COMPOSING, FUNCTION_KOREAN_HANJA_KEY);
        case msime::tsf::KoreanKeyAction::Compose: {
            if (msime::windows::scheme::FoldsLetterCase(scheme))
            {
                const bool upper = LOWORD(wParam) == VK_PACKET ? (*classifiedWch >= L'A' && *classifiedWch <= L'Z')
                                                               : (capturedModifiers & 0b00000001u) != 0;
                *classifiedWch = msime::tsf::korean_letter(*classifiedWch, upper);
            }
            return setKeyState(CATEGORY_COMPOSING, FUNCTION_INPUT);
        }
        case msime::tsf::KoreanKeyAction::CommitWithText:
            return setKeyState(CATEGORY_COMPOSING, FUNCTION_COMMIT_SYLLABLE);
        case msime::tsf::KoreanKeyAction::CommitAndPass:
            // Behind a barrier the key cannot reach the application in order, so it is eaten and queued; once the syllable is committed the key is replayed to the application.
            return setKeyState(CATEGORY_COMPOSING, FUNCTION_COMMIT_SYLLABLE_AND_REPLAY);
        case msime::tsf::KoreanKeyAction::Pass:
            // Printable keys become queued application text, the same as the closed-keyboard case above; the Hanja key with nothing composing carries no text and goes straight to the application.
            return *classifiedWch != L'\0' && std::iswprint(static_cast<wint_t>(*classifiedWch)) != 0;
        case msime::tsf::KoreanKeyAction::Default:
            break;
        }
    }

    // As in _IsKeyEaten: with nothing projected composing, a Stroke letter other than the five strokes is queued as application text.
    if (shadow.imeOpen && shadow.inputLength == 0 && !shadow.candidateActive &&
        msime::windows::scheme::LetterPassesWhileIdle(
            scheme, Global::DedicatedEnglish.active(GetTickCount64()), *classifiedWch))
    {
        return true;
    }

    // 与 _IsKeyEaten（CCompositionProcessorEngine::IsSecondThirdCandidateKey）相同：打开「二三候选」后，组字中的 ';' 和 '\'' 是数字选词，排在音节分隔符和标点前面；网址模式和 V 模式拼写的符号、微软双拼的韵母 ing 仍是输入。Ctrl 和 Alt 组合键在上面已经交给应用。是否在组字和光标前的字母取自排队按键的投影，与下面的数字键相同；宿主会话拥有组字时投影从空开始，这是数字键同样有的限制（.agents/notes/implemented/bug-fix/2026-10-10-second-third-candidate-tip-slot.md）。
    if (shadow.imeOpen && shadow.inputLength > 0 && Global::SecondThirdCandidateEnabled.load(std::memory_order_relaxed) &&
        msime::windows::second_third_candidate_slot(*classifiedCode, *classifiedWch))
    {
        const bool spelled =
            (shadow.urlMode && Global::ClassifyModeKey(Global::UrlSpellingSymbols, *classifiedCode, *classifiedWch) ==
                                   Global::ExpressionKey::Input) ||
            (Global::IsExpressionModeComposition(shadow.rawInput.c_str(), shadow.rawInput.size(),
                                                 Global::ExpressionModeEnabled.load(std::memory_order_relaxed)) &&
             Global::ClassifyModeKey(Global::ExpressionSpellingSymbols, *classifiedCode, *classifiedWch) ==
                 Global::ExpressionKey::Input);
        const bool microsoftFinal = *classifiedWch == L';' &&
                                    Global::MicrosoftShuangpinEnabled.load(std::memory_order_relaxed) &&
                                    msime::windows::microsoft_shuangpin_final_position(shadow.rawInput, shadow.caret);
        if (msime::windows::second_third_candidate_selection(true, *classifiedCode, *classifiedWch, 0, true, scheme,
                                                             Global::DedicatedEnglish.active(GetTickCount64()),
                                                             spelled || microsoftFinal))
        {
            return setKeyState(CATEGORY_CANDIDATE, FUNCTION_SELECT_BY_NUMBER);
        }
    }

    _KEYSTROKE_STATE inputState = {};
    WCHAR inputWch = *classifiedWch;
    bool isInputKey = false;
    if (shadow.imeOpen)
    {
        isInputKey = _pCompositionProcessorEngine->IsVirtualKeyNeedForFreshComposition(*classifiedCode, &inputWch,
                                                                                       &inputState) != FALSE;
        if (!isInputKey && shadow.inputLength > 0 &&
            ((*classifiedWch == L'\'' && !msime::windows::scheme::ApostropheIsPunctuationWhileComposing(scheme)) ||
             (_pCompositionProcessorEngine->IsWildcard() &&
                                           _pCompositionProcessorEngine->IsWildcardChar(*classifiedWch))))
        {
            isInputKey = true;
        }
        if (!isInputKey && Global::MicrosoftShuangpinEnabled.load(std::memory_order_relaxed) &&
            *classifiedCode == VK_OEM_1 && *classifiedWch == L';' && !shadow.rawInput.empty())
        {
            const size_t caret = min(shadow.caret, shadow.rawInput.size());
            const size_t separator = caret == 0 ? std::wstring::npos : shadow.rawInput.rfind(L'\'', caret - 1);
            const size_t chunkStart = separator == std::wstring::npos ? 0 : separator + 1;
            isInputKey = (caret - chunkStart) % 2 == 1;
        }
        if (shadow.inputLength == 0 && (GetKeyState(VK_CAPITAL) & 0x0001) != 0 && *classifiedWch >= L'A' &&
            *classifiedWch <= L'Z' && *classifiedCode >= L'A' && *classifiedCode <= L'Z')
        {
            // Match the normal fresh-composition path: CapsLock uppercase at
            // the beginning belongs to the application.
            isInputKey = false;
        }
        // Match the normal path: "/" and "@" open their modes on an empty composition.
        if (!isInputKey && shadow.inputLength == 0 && !shadow.candidateActive &&
            Global::OpensLocalMode(*classifiedWch, false,
                                   shadow.punctuationOpen && Global::PunctuationLockMode.load(std::memory_order_relaxed) !=
                                                                 Global::PunctuationLock::AlwaysEnglish,
                                   Global::CommandModeEnabled.load(std::memory_order_relaxed),
                                   Global::MentionModeEnabled.load(std::memory_order_relaxed)))
        {
            isInputKey = true;
        }
    }

    if (shadow.candidateActive && isInputKey)
    {
        return setKeyState(CATEGORY_CANDIDATE, FUNCTION_FINALIZE_CANDIDATELIST_AND_INPUT);
    }
    if (!shadow.candidateActive && isInputKey)
    {
        return setKeyState(CATEGORY_COMPOSING, FUNCTION_INPUT);
    }

    if (shadow.imeOpen && shadow.inputLength > 0)
    {
        // Match the normal path: V's digits and operators compose ahead of their paging and punctuation meanings, and a digit key printing anything else selects.
        if (Global::IsExpressionModeComposition(shadow.rawInput.c_str(), shadow.rawInput.size(),
                                                Global::ExpressionModeEnabled.load(std::memory_order_relaxed)))
        {
            switch (Global::ClassifyModeKey(Global::ExpressionSpellingSymbols, *classifiedCode, *classifiedWch))
            {
            case Global::ExpressionKey::Input:
                return setKeyState(CATEGORY_COMPOSING, FUNCTION_INPUT);
            case Global::ExpressionKey::SelectByNumber:
                return setKeyState(CATEGORY_CANDIDATE, FUNCTION_SELECT_BY_NUMBER);
            case Global::ExpressionKey::Unclaimed:
                break;
            }
        }
        // 与普通路径一致：触发词后的触发键打开网址模式，之后网址的数字和符号排在翻页、标点和数字选词之前作为输入。
        const bool detectsUrls = msime::windows::scheme::DetectsUrls(scheme);
        if (shadow.urlMode)
        {
            switch (Global::ClassifyModeKey(Global::UrlSpellingSymbols, *classifiedCode, *classifiedWch))
            {
            case Global::ExpressionKey::Input:
                return setKeyState(CATEGORY_COMPOSING, FUNCTION_INPUT);
            case Global::ExpressionKey::SelectByNumber:
                return setKeyState(CATEGORY_CANDIDATE, FUNCTION_SELECT_BY_NUMBER);
            case Global::ExpressionKey::Unclaimed:
                break;
            }
        }
        else if (Global::OpensUrlMode(shadow.rawInput.c_str(), shadow.rawInput.size(), shadow.caret, *classifiedWch,
                                      detectsUrls, Global::DedicatedEnglish.active(GetTickCount64())))
        {
            return setKeyState(CATEGORY_COMPOSING, FUNCTION_INPUT);
        }
        const bool candidateKey = shadow.candidateActive;
        switch (*classifiedCode)
        {
        case VK_BACK:
            return candidateKey ? setKeyState(CATEGORY_CANDIDATE, FUNCTION_CANCEL)
                                : setKeyState(CATEGORY_COMPOSING, FUNCTION_BACKSPACE);
        case VK_DELETE:
            return candidateKey ? setKeyState(CATEGORY_CANDIDATE, FUNCTION_CANCEL)
                                : setKeyState(CATEGORY_COMPOSING, FUNCTION_DELETE);
        case VK_SPACE:
            return setKeyState(CATEGORY_CANDIDATE, FUNCTION_CONVERT);
        case VK_RETURN:
            return candidateKey ? setKeyState(CATEGORY_CANDIDATE, FUNCTION_FINALIZE_CANDIDATELIST)
                                : setKeyState(CATEGORY_CANDIDATE, FUNCTION_FINALIZE_CANDIDATELISTForVKReturn);
        case VK_ESCAPE:
            return setKeyState(CATEGORY_CANDIDATE, FUNCTION_CANCEL);
        case VK_LEFT:
            return setKeyState(CATEGORY_COMPOSING, FUNCTION_MOVE_LEFT);
        case VK_RIGHT:
            return setKeyState(CATEGORY_COMPOSING, FUNCTION_MOVE_RIGHT);
        case VK_HOME:
            return setKeyState(CATEGORY_CANDIDATE, FUNCTION_MOVE_PAGE_TOP);
        case VK_END:
            return setKeyState(CATEGORY_CANDIDATE, FUNCTION_MOVE_PAGE_BOTTOM);
        case VK_OEM_MINUS:
        case VK_OEM_PLUS:
            if (*classifiedCode == VK_OEM_PLUS && shadow.unicodeMode && shadow.inputLength == 1 &&
                *classifiedWch == L'+')
            {
                return setKeyState(CATEGORY_COMPOSING, FUNCTION_INPUT);
            }
            if (Global::InputModeScheme.load(std::memory_order_relaxed) == msime::windows::scheme::Japanese)
            {
                return *classifiedCode == VK_OEM_MINUS && *classifiedWch == L'-'
                           ? setKeyState(CATEGORY_COMPOSING, FUNCTION_INPUT)
                           : setKeyState(CATEGORY_COMPOSING, FUNCTION_PUNCTUATION);
            }
            return setKeyState(CATEGORY_CANDIDATE, FUNCTION_SERVER_CANDIDATE_KEY);
        case VK_OEM_COMMA:
        case VK_OEM_PERIOD:
        case VK_OEM_4:
        case VK_OEM_6:
        case VK_TAB:
        case VK_PRIOR:
        case VK_NEXT:
        case VK_UP:
        case VK_DOWN:
            return setKeyState(CATEGORY_CANDIDATE, FUNCTION_SERVER_CANDIDATE_KEY);
        default:
            break;
        }

        if (*classifiedCode >= L'1' && *classifiedCode <= L'9')
        {
            // U-mode: bare digits compose hex; Shift+1..9 selects candidates.
            if (shadow.unicodeMode)
            {
                const bool shift_only = (capturedModifiers & 0b00000111u) == 0b00000001u;
                if (shift_only)
                {
                    return setKeyState(CATEGORY_CANDIDATE, FUNCTION_SELECT_BY_NUMBER);
                }
                return setKeyState(CATEGORY_COMPOSING, FUNCTION_INPUT);
            }
            return setKeyState(CATEGORY_CANDIDATE, FUNCTION_SELECT_BY_NUMBER);
        }
        if (*classifiedCode == L'0' && shadow.unicodeMode)
        {
            return setKeyState(CATEGORY_COMPOSING, FUNCTION_INPUT);
        }
        if (Global::CommitWithHighlightedCandPunc.count(*classifiedWch) > 0 ||
            (shadow.punctuationOpen && _pCompositionProcessorEngine->IsPunctuation(*classifiedWch)))
        {
            return setKeyState(CATEGORY_COMPOSING, FUNCTION_PUNCTUATION);
        }
        return false;
    }

    if (shadow.punctuationOpen && _pCompositionProcessorEngine->IsPunctuation(*classifiedWch))
    {
        return setKeyState(CATEGORY_COMPOSING, FUNCTION_PUNCTUATION);
    }
    if (shadow.doubleSingleByteOpen && _pCompositionProcessorEngine->IsDoubleSingleByte(*classifiedWch))
    {
        return setKeyState(CATEGORY_COMPOSING, FUNCTION_DOUBLE_SINGLE_BYTE);
    }
    if (!shadow.imeOpen && *classifiedWch != L'\0' && std::iswprint(static_cast<wint_t>(*classifiedWch)) != 0)
    {
        // A queued Shift may make this future key English. It still belongs
        // behind the FIFO prefix; CATEGORY_NONE/FUNCTION_NONE marks a direct
        // application-text replay instead of misclassifying it as Chinese.
        return true;
    }
    return false;
}

//+---------------------------------------------------------------------------
//
// _NotePassthroughStatistics
//
// Counts one printable character that this tip hands back to the application. The commit paths never see these keys - the host inserts them - so this is the only capture point for half-width digits, the symbols outside the punctuation table and English-mode letters. Observation only: the eaten result, the deferred queue and the edit path stay untouched, and every failure mode is a dropped count.
//----------------------------------------------------------------------------

void CMetasequoiaIME::_NotePassthroughStatistics(UINT virtualKey, WCHAR wch, bool keyboardKnownEnabled)
{
    const LONG messageTime = GetMessageTime();
    if (virtualKey != 0 && virtualKey == _passthroughStatsVirtualKey && messageTime == _passthroughStatsMessageTime)
    {
        // The system can query the same key event more than once; only the first pass counts. The marker is consumed here: the probes for one event arrive back to back, so anything later is a genuine second press that GetMessageTime cannot separate inside the same tick.
        _passthroughStatsVirtualKey = 0;
        return;
    }

    if (wch == L'\0')
    {
        // The keyboard-closed early return in _IsKeyEaten leaves its out-char blank even though the key reaches the application; widen it from the layout here. Keys that genuinely produce no character keep the zero and are dropped by the filter.
        wch = ConvertVKey(virtualKey);
    }

    // The same physical-state read _IsKeyEaten uses for application-owned combinations.
    const UINT modifiers = CaptureIpcModifiers();
    const bool ctrlDown = (modifiers & 0b00000010u) != 0;
    const bool altDown = (modifiers & 0b00000100u) != 0;
    const bool winDown = (GetAsyncKeyState(VK_LWIN) & 0x8000) != 0 || (GetAsyncKeyState(VK_RWIN) & 0x8000) != 0;
    // A non-zero out-char from _IsKeyEaten passed that function's own keyboard-disabled check, so the compartment query is only needed when the caller could not prove the keyboard was live.
    const bool keyboardDisabled = !keyboardKnownEnabled && _IsKeyboardDisabled() != FALSE;
    if (!ShouldCountPassthroughChar(wch, keyboardDisabled, ctrlDown, altDown, winDown))
    {
        return;
    }

    BOOL isOpen = FALSE;
    CCompartment CompartmentKeyboardOpen(_pThreadMgr, _tfClientId, GUID_COMPARTMENT_KEYBOARD_OPENCLOSE);
    CompartmentKeyboardOpen._GetCompartmentBOOL(isOpen);

    _passthroughStatsVirtualKey = virtualKey;
    _passthroughStatsMessageTime = messageTime;
    QueuePassthroughStatistics(wch, isOpen == FALSE);
}

//+---------------------------------------------------------------------------
//
// _NoteKeyPressStatistics
//
// Counts one physical key press for the key heatmap: every key while this tip is active, eaten or passed through, Shift and hotkeys included. Only the key's id goes into the count. It runs ahead of every early return in OnTestKeyDown and again in OnKeyDown, for hosts that skip the test probe, so each press is seen at least once and the de-duplication keeps it to once. Observation only: nothing here changes how the key is handled.
//----------------------------------------------------------------------------

void CMetasequoiaIME::_NoteKeyPressStatistics(WPARAM wParam, LPARAM lParam)
{
    const wchar_t *keyId = KeyPressIdFromKeyDown(static_cast<std::uintptr_t>(wParam), static_cast<std::uintptr_t>(lParam));
    if (keyId == nullptr)
    {
        return;
    }
    // The Test and Key probes of one press share its scan code and message time. Unlike the passthrough marker this one is not consumed, because a press can be probed more than twice; a genuine second press of the same key needs a key-up in between and so a later message.
    const UINT physicalKey = KeyPressPhysicalKey(static_cast<std::uintptr_t>(lParam));
    const LONG messageTime = GetMessageTime();
    if (physicalKey == _keyPressStatsKey && messageTime == _keyPressStatsMessageTime)
    {
        return;
    }
    _keyPressStatsKey = physicalKey;
    _keyPressStatsMessageTime = messageTime;
    if (!ShouldCountKeyPress(keyId, _IsKeyboardDisabled() != FALSE, _IsSecureMode() != FALSE))
    {
        return;
    }
    QueueKeyPressStatistics(keyId);
}

//+---------------------------------------------------------------------------
//
// ITfKeyEventSink::OnTestKeyDown
//
// Called by the system to query this service wants a potential keystroke.
//----------------------------------------------------------------------------

STDAPI CMetasequoiaIME::OnTestKeyDown(ITfContext *pContext, WPARAM wParam, LPARAM lParam, BOOL *pIsEaten)
{
    if (pContext == nullptr || pIsEaten == nullptr)
    {
        return E_INVALIDARG;
    }
    if (IsSelfGeneratedSendInputExtraInfo(static_cast<ULONG_PTR>(GetMessageExtraInfo())))
    {
        *pIsEaten = FALSE;
        return S_OK;
    }
    // Ahead of the backspace-hold, Shift and hotkey returns below, each of which would otherwise hide a real press.
    _NoteKeyPressStatistics(wParam, lParam);
    // Only the first press toggles; auto-repeat would flip the prediction back.
    if (wParam == VK_CAPITAL && !IsAutoRepeat(lParam))
    {
        Global::CapsLockEnabled.store((GetKeyState(VK_CAPITAL) & 0x0001) == 0, std::memory_order_relaxed);
        _RequestLanguageBarCapsIconRefresh();
    }
    PerfTimer onTestKeyDownTimer;
    Global::UpdateModifiers(wParam, lParam);
    _TrackModifierHotkeyArming(wParam, lParam, false);
    if (_ApplyBackspaceHoldGuard(wParam, lParam))
    {
        *pIsEaten = TRUE;
        _NoteKeyForSmartPunctuation(VK_BACK, ConvertVKey(VK_BACK), true);
        return S_OK;
    }
    if (IsShiftVk(LOWORD(wParam)))
    {
        *pIsEaten = FALSE;
        return S_OK;
    }

    if (_HasDeferredKeyBarrier())
    {
        _KEYSTROKE_STATE deferredState = {};
        WCHAR deferredWch = L'\0';
        UINT deferredCode = 0;
        if (!_DeferredKeyQueueHasCapacity())
        {
            // Still observe Backspace for smart-punctuation rejection. Uneaten
            // keys often never reach OnKeyDown, and this is the only sink that
            // always sees them.
            deferredWch = ConvertVKey(static_cast<UINT>(wParam));
            deferredCode = VKeyFromVKPacketAndWchar(static_cast<UINT>(wParam), deferredWch);
            _NoteKeyForSmartPunctuation(deferredCode, deferredWch, false);
            // _ClassifyDeferredKeyDown is not reached on this exit and ConvertVKey fills the char without checking the keyboard state.
            _NotePassthroughStatistics(static_cast<UINT>(wParam), deferredWch, false);
            *pIsEaten = FALSE;
            return S_OK;
        }
        *pIsEaten =
            _ClassifyDeferredKeyDown(pContext, wParam, lParam, nullptr, nullptr, &deferredWch, &deferredCode, &deferredState)
                ? TRUE
                : FALSE;
        // Classify always fills code/wch before failing. Track rejection even
        // when the key is handed back to the app (typical for VK_BACK).
        _NoteKeyForSmartPunctuation(deferredCode, deferredWch, *pIsEaten ? true : false);
        if (!*pIsEaten)
        {
            // The deferred classifier fills its out-char before its own keyboard-disabled check, and not every exit runs that check, so the char proves nothing about the keyboard state.
            _NotePassthroughStatistics(static_cast<UINT>(wParam), deferredWch, false);
        }
        return S_OK;
    }

    GUID hotkeyGuid = {};
    if (_MatchChordInputHotkey(wParam, &hotkeyGuid))
    {
        *pIsEaten = TRUE;
        return S_OK;
    }

    _KEYSTROKE_STATE KeystrokeState;
    WCHAR wch = '\0';
    UINT code = 0;
    *pIsEaten = _IsKeyEaten(pContext, (UINT)wParam, &code, &wch, &KeystrokeState);

    // Every keydown reaches this sink, including the ones handed back to the
    // application (backspace with no composition), so the smart-punctuation
    // rejection state is tracked here rather than in the eaten-key path.
    _NoteKeyForSmartPunctuation(code, wch, *pIsEaten ? true : false);

    if (!*pIsEaten)
    {
        // A half-width digit, a symbol outside the tables or an English-mode letter lands here: the tip let it through, so the host inserts it outside every commit path.
        _NotePassthroughStatistics(static_cast<UINT>(wParam), wch, wch != L'\0');
    }

    DebugTsfIssue47(L"test-keydown-classified", FANY_IME_NO_REQUEST_ID, code, wch, KeystrokeState.Category,
                    KeystrokeState.Function, *pIsEaten ? 1 : 0, _IsComposing(),
                    _pCompositionProcessorEngine ? _pCompositionProcessorEngine->GetVirtualKeyLength() : 0, S_OK);

    if (KeystrokeState.Category == CATEGORY_INVOKE_COMPOSITION_EDIT_SESSION)
    {
        //
        // Invoke key handler edit session
        //
        KeystrokeState.Category = CATEGORY_COMPOSING;

        _InvokeKeyHandler(pContext, code, wch, (DWORD)lParam, KeystrokeState, FANY_IME_NO_REQUEST_ID);
    }

    return S_OK;
}

bool CMetasequoiaIME::_QueueDeferredKeyDown(_In_ ITfContext *pContext, WPARAM wParam, LPARAM lParam,
                                            WCHAR translatedWch, UINT modifiersDown, const _KEYSTROKE_STATE &keyState)
{
    // Repeats are owned but do not enqueue another global configuration toggle.
    if (keyState.Function == FUNCTION_TOGGLE_CHARACTER_SET && (lParam & 0x40000000) != 0)
        return true;
    if (pContext == nullptr || !_DeferredKeyQueueHasCapacity())
    {
        return false;
    }

    pContext->AddRef();
    DeferredKeyDown key;
    key.kind = keyState.Category == CATEGORY_NONE && keyState.Function == FUNCTION_NONE
                   ? DeferredKeyDown::Kind::ApplicationText
                   : DeferredKeyDown::Kind::KeyDown;
    key.context = pContext;
    key.wParam = wParam;
    key.lParam = lParam;
    key.translatedWch = translatedWch;
    key.modifiersDown = modifiersDown;
    key.keyState = keyState;
    key.focusGeneration = _deferredKeyFocusGeneration;
    key.queuedAtMs = GetTickCount64();
    _deferredKeyDowns.push_back(key);
    if (key.kind == DeferredKeyDown::Kind::KeyDown)
    {
        _ApplyDeferredKeyProjection(keyState, translatedWch,
                                    VKeyFromVKPacketAndWchar(static_cast<UINT>(wParam), translatedWch));
    }
    else
    {
        _EnsureDeferredKeyProjection();
    }
    _ScheduleDeferredKeyDownDrain();
    return true;
}

bool CMetasequoiaIME::_QueueDeferredPreservedKey(_In_ ITfContext *pContext, REFGUID preservedKey)
{
    if (pContext == nullptr || !_DeferredKeyQueueHasCapacity())
    {
        return false;
    }

    pContext->AddRef();
    DeferredKeyDown key;
    key.kind = DeferredKeyDown::Kind::PreservedKey;
    key.context = pContext;
    key.preservedKey = preservedKey;
    key.focusGeneration = _deferredKeyFocusGeneration;
    key.queuedAtMs = GetTickCount64();
    _deferredKeyDowns.push_back(key);
    _ApplyDeferredPreservedKeyProjection(preservedKey);
    _ScheduleDeferredKeyDownDrain();
    return true;
}

void CMetasequoiaIME::_ClearDeferredKeyDowns()
{
    _backspaceHoldArmed = false;
    const size_t queuedCount = _deferredKeyDowns.size();
    const bool hadInFlight = _hasDeferredKeyInFlight;
    const uint64_t inFlightToken = _deferredKeyReplayToken;
    if (queuedCount != 0 || hadInFlight)
    {
        DebugTsfIssue47(L"deferred-queue-cleared", FANY_IME_NO_REQUEST_ID,
                        hadInFlight ? static_cast<UINT>(_deferredKeyInFlight.wParam) : 0,
                        hadInFlight ? _deferredKeyInFlight.translatedWch : L'\0',
                        hadInFlight ? _deferredKeyInFlight.keyState.Category : 0,
                        hadInFlight ? _deferredKeyInFlight.keyState.Function : 0, -1, _IsComposing(),
                        _pCompositionProcessorEngine ? _pCompositionProcessorEngine->GetVirtualKeyLength() : 0, S_FALSE,
                        inFlightToken);
    }
    // A posted drain belongs to the current message window/generation.  It
    // may never run if Deactivate destroys that window, so never carry this
    // latch into a later Activate on the same TIP instance.
    _deferredKeyDrainPosted = false;
    if (++_deferredKeyFocusGeneration == 0)
    {
        ++_deferredKeyFocusGeneration;
    }
    while (!_deferredKeyDowns.empty())
    {
        ITfContext *context = _deferredKeyDowns.front().context;
        _deferredKeyDowns.pop_front();
        if (context)
        {
            context->Release();
        }
    }
    while (!_deferredAppliedPrefix.empty())
    {
        ITfContext *context = _deferredAppliedPrefix.front().context;
        _deferredAppliedPrefix.pop_front();
        if (context)
        {
            context->Release();
        }
    }
    if (_hasDeferredKeyInFlight)
    {
        ITfContext *context = _deferredKeyInFlight.context;
        _hasDeferredKeyInFlight = false;
        _deferredKeyReplayToken = 0;
        _deferredKeyInFlight = {};
        if (context)
        {
            context->Release();
        }
    }
    _deferredKeyProjectionValid = false;
    _deferredProjectedImeOpen = false;
    _deferredProjectedPunctuationOpen = false;
    _deferredProjectedDoubleSingleByteOpen = false;
    _deferredProjectedInputLength = 0;
    _deferredProjectedRawInput.clear();
    _deferredProjectedCaret = 0;
    _deferredProjectedCandidateActive = false;
    _deferredProjectedUnicodeMode = false;
    _deferredProjectedUrlMode = false;
    _deferredProjectedKoreanHanjaListOpen = false;
    _shiftHotkeyArmed = false;
    _ctrlHotkeyArmed = false;
}

void CMetasequoiaIME::_CompleteDeferredKeyReplay(uint64_t replayToken)
{
    if (replayToken == 0 || !_hasDeferredKeyInFlight || _deferredKeyReplayToken != replayToken)
    {
        return;
    }

    DebugTsfIssue47(L"deferred-replay-complete", FANY_IME_NO_REQUEST_ID, static_cast<UINT>(_deferredKeyInFlight.wParam),
                    _deferredKeyInFlight.translatedWch, _deferredKeyInFlight.keyState.Category,
                    _deferredKeyInFlight.keyState.Function, 1, _IsComposing(),
                    _pCompositionProcessorEngine ? _pCompositionProcessorEngine->GetVirtualKeyLength() : 0, S_OK,
                    replayToken);

    ITfContext *context = _deferredKeyInFlight.context;
    if (_deferredKeyDowns.empty())
    {
        // The exact final edit session has completed, so the real composition
        // has caught up with the future projection.  Compact the applied
        // event history into a bounded raw-text/caret checkpoint.  This
        // checkpoint is dormant (not a barrier) until a transport reset needs
        // to rebuild the composition.
        _deferredKeyProjectionValid = false;
        _deferredProjectedInputLength = 0;
        _deferredProjectedRawInput.clear();
        _deferredProjectedCaret = 0;
        _deferredProjectedCandidateActive = false;
        _deferredProjectedUnicodeMode = false;
        _deferredProjectedUrlMode = false;
        _deferredProjectedKoreanHanjaListOpen = false;
        (void)_RefreshDeferredRecoveryPrefix(context);
        _deferredKeyInFlight = {};
        _hasDeferredKeyInFlight = false;
        _deferredKeyReplayToken = 0;
        if (context)
        {
            context->Release();
        }
        _TryLeaveServerUnavailableFallback();
        _ScheduleDeferredKeyDownDrain();
        return;
    }

    bool contextTransferredToPrefix = false;
    const auto clearAppliedPrefix = [this]() {
        while (!_deferredAppliedPrefix.empty())
        {
            ITfContext *prefixContext = _deferredAppliedPrefix.front().context;
            _deferredAppliedPrefix.pop_front();
            if (prefixContext)
            {
                prefixContext->Release();
            }
        }
    };
    if (_deferredKeyInFlight.kind == DeferredKeyDown::Kind::KeyDown)
    {
        if (StartsNewDeferredPrefix(_deferredKeyInFlight.keyState))
        {
            clearAppliedPrefix();
            // The old text/candidate side of this combined operation has
            // already committed successfully.  A replacement Server epoch
            // must rebuild only the new raw character; replaying the original
            // finalize-and-input function would try to finalize state that no
            // longer exists.
            _deferredKeyInFlight.keyState.Category = CATEGORY_COMPOSING;
            _deferredKeyInFlight.keyState.Function = FUNCTION_INPUT;
            _deferredAppliedPrefix.push_back(_deferredKeyInFlight);
            contextTransferredToPrefix = true;
        }
        else if (IsRecoverableDeferredPrefix(_deferredKeyInFlight.keyState))
        {
            _deferredAppliedPrefix.push_back(_deferredKeyInFlight);
            contextTransferredToPrefix = true;
        }
        else
        {
            clearAppliedPrefix();
        }
    }
    else if (_deferredKeyInFlight.kind == DeferredKeyDown::Kind::PreservedKey && _pCompositionProcessorEngine &&
             _pCompositionProcessorEngine->GetPreservedKeyAction(_deferredKeyInFlight.preservedKey) ==
                 CCompositionProcessorEngine::PreservedKeyAction::ToggleImeMode)
    {
        clearAppliedPrefix();
    }

    _deferredKeyInFlight = {};
    _hasDeferredKeyInFlight = false;
    _deferredKeyReplayToken = 0;
    if (context && !contextTransferredToPrefix)
    {
        context->Release();
    }
    _TryLeaveServerUnavailableFallback();
    _ScheduleDeferredKeyDownDrain();
}

bool CMetasequoiaIME::_IsDeferredKeyReplayCurrent(uint64_t replayToken, uint64_t focusGeneration,
                                                  _In_opt_ ITfContext *expectedContext) const
{
    return replayToken != 0 && _hasDeferredKeyInFlight && _deferredKeyReplayToken == replayToken &&
           focusGeneration == _deferredKeyFocusGeneration && _deferredKeyInFlight.focusGeneration == focusGeneration &&
           (expectedContext == nullptr || _deferredKeyInFlight.context == expectedContext);
}

void CMetasequoiaIME::_RetryDeferredKeyReplay(uint64_t replayToken)
{
    if (replayToken == 0 || !_hasDeferredKeyInFlight || _deferredKeyReplayToken != replayToken)
    {
        return;
    }
    if (_deferredKeyInFlight.focusGeneration != _deferredKeyFocusGeneration)
    {
        DebugTsfIssue47(L"deferred-replay-focus-changed", FANY_IME_NO_REQUEST_ID,
                        static_cast<UINT>(_deferredKeyInFlight.wParam), _deferredKeyInFlight.translatedWch,
                        _deferredKeyInFlight.keyState.Category, _deferredKeyInFlight.keyState.Function, 1,
                        _IsComposing(),
                        _pCompositionProcessorEngine ? _pCompositionProcessorEngine->GetVirtualKeyLength() : 0, S_FALSE,
                        replayToken);
        _CompleteDeferredKeyReplay(replayToken);
        return;
    }

    if (_deferredKeyInFlight.replayAttempts >= kMaxDeferredKeyReplayAttempts)
    {
        DebugTsfIssue47(L"deferred-replay-abandoned", FANY_IME_NO_REQUEST_ID,
                        static_cast<UINT>(_deferredKeyInFlight.wParam), _deferredKeyInFlight.translatedWch,
                        _deferredKeyInFlight.keyState.Category, _deferredKeyInFlight.keyState.Function, 1,
                        _IsComposing(),
                        _pCompositionProcessorEngine ? _pCompositionProcessorEngine->GetVirtualKeyLength() : 0, E_FAIL,
                        replayToken);
        // A permanently unanswerable request must not monopolize the ordered
        // replay queue forever. The real composition is still intact because
        // the failing edit session did not commit; discard this poisoned batch
        // and reconnect for subsequent physical input.
        MarkNamedpipeSessionDirtyForOwner(this);
        _ClearDeferredKeyDowns();
        return;
    }

    const bool needsBackoff = _deferredKeyInFlight.replayAttempts >= 2;
    DebugTsfIssue47(needsBackoff ? L"deferred-replay-retry-backoff" : L"deferred-replay-retry", FANY_IME_NO_REQUEST_ID,
                    static_cast<UINT>(_deferredKeyInFlight.wParam), _deferredKeyInFlight.translatedWch,
                    _deferredKeyInFlight.keyState.Category, _deferredKeyInFlight.keyState.Function, 1, _IsComposing(),
                    _pCompositionProcessorEngine ? _pCompositionProcessorEngine->GetVirtualKeyLength() : 0, S_FALSE,
                    replayToken);
    _deferredKeyDowns.push_front(_deferredKeyInFlight);
    while (!_deferredAppliedPrefix.empty())
    {
        _deferredKeyDowns.push_front(_deferredAppliedPrefix.back());
        _deferredAppliedPrefix.pop_back();
    }
    _deferredKeyInFlight = {};
    _hasDeferredKeyInFlight = false;
    _deferredKeyReplayToken = 0;
    // Finish the ownership transfer before a dirty notification can fall
    // back to synchronous window dispatch and re-enter reset bookkeeping.
    MarkNamedpipeSessionDirtyForOwner(this);
    if (!needsBackoff)
    {
        _ScheduleDeferredKeyDownDrain();
    }
    else if (_msgWndHandle && IsWindow(_msgWndHandle))
    {
        PostOwnerMessageWithSyncFallback(_msgWndHandle, WM_IpcReconnect);
    }
}

void CMetasequoiaIME::_ScheduleDeferredKeyDownDrain()
{
    if (!_deferredKeyDowns.empty() && !_hasDeferredKeyInFlight && !_deferredKeyDrainPosted && _msgWndHandle &&
        IsWindow(_msgWndHandle))
    {
        _deferredKeyDrainPosted = true;
        if (!PostMessage(_msgWndHandle, WM_DrainDeferredKeyDown, 0, 0))
        {
            // The owner window is thread-affine. A synchronous fallback keeps
            // a transient queue-post failure from stranding an eaten key.
            SendMessage(_msgWndHandle, WM_DrainDeferredKeyDown, 0, 0);
        }
    }
}

bool CMetasequoiaIME::_IsServerUnavailableFallbackActive() const
{
    return _serverUnavailableFallbackActive;
}

void CMetasequoiaIME::_TryLeaveServerUnavailableFallback()
{
    const uint64_t expectedToken = _expectedWorkerFocusToken.load(std::memory_order_acquire);
    if (_serverUnavailableFallbackActive && !_IsComposing() && expectedToken != 0 &&
        _workerCommitReady.load(std::memory_order_acquire) &&
        _acknowledgedWorkerFocusToken.load(std::memory_order_acquire) == expectedToken)
    {
        _serverUnavailableFallbackActive = false;
    }
}

void CMetasequoiaIME::_DrainOneDeferredKeyDown()
{
    _deferredKeyDrainPosted = false;
    if (_hasDeferredKeyInFlight || _deferredKeyDowns.empty() || !Global::g_connected)
    {
        return;
    }
    if (_localSessionResetPending.load(std::memory_order_acquire))
    {
        PostOwnerMessageWithSyncFallback(_msgWndHandle, WM_IpcReconnect);
        return;
    }
    if (_serverUnavailableFallbackActive)
    {
        // Every key typed into the offline lane is one more request for the
        // Server. Re-probing the transport here is not allowed while the local
        // composition owns the keystroke stream, so count the key itself.
        _NoteKeyEventIpcFailure();
    }
    else if (!EnsureNamedpipeFocusSessionActivated())
    {
        // Do not strand an eaten key when the Server cannot establish a
        // focus session. The queued FIFO becomes an isolated local/raw-input
        // lane until its composition has been finalized.
        _serverUnavailableFallbackActive = true;
        _NoteKeyEventIpcFailure();
    }

    _deferredKeyInFlight = _deferredKeyDowns.front();
    _deferredKeyDowns.pop_front();
    _hasDeferredKeyInFlight = true;
    do
    {
        _deferredKeyReplayToken = ++_nextDeferredKeyReplayToken;
    } while (_deferredKeyReplayToken == 0);

    DeferredKeyDown &key = _deferredKeyInFlight;
    if (key.queuedAtMs != 0)
    {
        DebugTsfKeyLatency(L"deferred-key-queue", 0, static_cast<double>(GetTickCount64() - key.queuedAtMs), S_OK);
    }
    ++key.replayAttempts;
    const uint64_t replayToken = _deferredKeyReplayToken;
    const uint64_t focusToken = _CaptureFocusSessionToken();
    if (key.focusGeneration != _deferredKeyFocusGeneration)
    {
        // All queued keys belong to the focused top context that captured
        // them. Never replay into another editor after another focus change.
        _ClearDeferredKeyDowns();
        return;
    }
    if (!_serverUnavailableFallbackActive && !_IsFocusSessionCurrent(focusToken, key.context))
    {
        // A token/Ready fence can close without a document focus change.
        // Preserve the exact item for the replacement Server epoch; an actual
        // focus-generation change is handled by _ClearDeferredKeyDowns above.
        _RetryDeferredKeyReplay(replayToken);
        return;
    }

    BOOL eaten = FALSE;
    if (key.kind == DeferredKeyDown::Kind::ApplicationText)
    {
        (void)_RequestDeferredApplicationTextEditSession(key.context, key.translatedWch, focusToken,
                                                         key.focusGeneration, replayToken);
        return;
    }
    if (key.kind == DeferredKeyDown::Kind::PreservedKey)
    {
        const auto preservedAction = _pCompositionProcessorEngine
                                         ? _pCompositionProcessorEngine->GetPreservedKeyAction(key.preservedKey)
                                         : CCompositionProcessorEngine::PreservedKeyAction::None;
        const bool awaitsEditSession =
            preservedAction == CCompositionProcessorEngine::PreservedKeyAction::ToggleImeMode;
        if (!key.preservedApplied)
        {
            // OnPreservedKey changes compartments synchronously. Mark that
            // phase before entering COM so a failed async commit retries only
            // the commit phase and never toggles twice.
            key.preservedApplied = true;
            _DispatchPreservedKey(key.context, key.preservedKey, &eaten, key.focusGeneration, true, replayToken);
        }
        else if (awaitsEditSession)
        {
            _KEYSTROKE_STATE toggleState = {};
            toggleState.Category = CATEGORY_COMPOSING;
            toggleState.Function = FUNCTION_TOGGLE_IME_MODE;
            _InvokeKeyHandler(key.context, 0, L'\0', 0, toggleState, FANY_IME_NO_REQUEST_ID, {}, 0, 0, 0, replayToken);
        }
        if (!awaitsEditSession && _deferredKeyReplayToken == replayToken)
        {
            _CompleteDeferredKeyReplay(replayToken);
        }
        return;
    }

    if (_serverUnavailableFallbackActive)
    {
        _KEYSTROKE_STATE offlineState = key.keyState;
        if (offlineState.Function == FUNCTION_TOGGLE_CHARACTER_SET)
        {
            _CompleteDeferredKeyReplay(replayToken);
            return;
        }
        offlineState.Category = CATEGORY_COMPOSING;
        switch (offlineState.Function)
        {
        case FUNCTION_CONVERT:
        case FUNCTION_FINALIZE_CANDIDATELIST:
        case FUNCTION_FINALIZE_CANDIDATELISTForVKReturn:
        case FUNCTION_SELECT_BY_NUMBER:
        case FUNCTION_SERVER_CANDIDATE_KEY:
            // There is no candidate authority offline. Commit exactly the
            // raw text already held by the TSF composition.
            offlineState.Function = FUNCTION_FINALIZE_TEXTSTORE;
            break;
        case FUNCTION_MOVE_PAGE_UP:
        case FUNCTION_MOVE_PAGE_DOWN:
        case FUNCTION_MOVE_PAGE_TOP:
        case FUNCTION_MOVE_PAGE_BOTTOM:
            _CompleteDeferredKeyReplay(replayToken);
            return;
        default:
            break;
        }
        _InvokeKeyHandler(key.context, key.wParam, key.translatedWch, static_cast<DWORD>(key.lParam), offlineState,
                          FANY_IME_NO_REQUEST_ID, {}, 0, 0, 0, replayToken);
        return;
    }

    const KeyDownDispatchResult result =
        _DispatchKeyDown(key.context, key.wParam, key.lParam, &eaten, &key.translatedWch, &key.modifiersDown,
                         &key.keyState, false, key.focusGeneration, replayToken);
    if (result == KeyDownDispatchResult::Retry)
    {
        _RetryDeferredKeyReplay(replayToken);
        return;
    }

    if (result == KeyDownDispatchResult::Complete && _deferredKeyReplayToken == replayToken)
    {
        _CompleteDeferredKeyReplay(replayToken);
    }
}

//+---------------------------------------------------------------------------
//
// ITfKeyEventSink::OnKeyDown
//
// Called by the system to offer this service a keystroke.
// on exit, the application will not handle the keystroke.
//----------------------------------------------------------------------------

STDAPI CMetasequoiaIME::OnKeyDown(ITfContext *pContext, WPARAM wParam, LPARAM lParam, BOOL *pIsEaten)
{
    if (pContext == nullptr || pIsEaten == nullptr)
    {
        return E_INVALIDARG;
    }
    if (IsSelfGeneratedSendInputExtraInfo(static_cast<ULONG_PTR>(GetMessageExtraInfo())))
    {
        *pIsEaten = FALSE;
        return S_OK;
    }
    // Usually a repeat of the OnTestKeyDown probe and dropped as one; it counts only for a host that calls OnKeyDown without testing first.
    _NoteKeyPressStatistics(wParam, lParam);
    PerfTimer onKeyDownTimer;
    const uint64_t focusGeneration = _deferredKeyFocusGeneration;
    (void)_DispatchKeyDown(pContext, wParam, lParam, pIsEaten, nullptr, nullptr, nullptr, true, focusGeneration);
    // TIP 只把 Ctrl+Shift+E 当作 Engine 英文模式的切换键吞下（_IsKeyEaten 与 _ClassifyDeferredKeyDown 同一条规则），之后不读 Server 的回复。在这里先翻转镜像：后面的字母无论立即分类还是入队时分类，都在本线程上排在它之后。每次按下 TSF 只调用一次 OnKeyDown，排队回放不经过这里，所以只翻一次。
    if (*pIsEaten && IsEnglishInputModeToggle(static_cast<UINT>(LOWORD(wParam)), CaptureIpcModifiers()))
    {
        Global::DedicatedEnglish.toggled(GetTickCount64());
    }
    DebugTsfKeyLatency(L"on-key-down", 0, onKeyDownTimer.ElapsedMs(), S_OK);
    return S_OK;
}

CMetasequoiaIME::KeyDownDispatchResult CMetasequoiaIME::_DispatchKeyDown(
    _In_ ITfContext *pContext, WPARAM wParam, LPARAM lParam, _Out_ BOOL *pIsEaten, _In_opt_ const WCHAR *translatedWch,
    _In_opt_ const UINT *modifiersDown, _In_opt_ const _KEYSTROKE_STATE *prevalidatedKeyState, bool canDefer,
    uint64_t expectedFocusGeneration, uint64_t deferredReplayToken)
{
    if (pContext == nullptr || pIsEaten == nullptr)
    {
        return KeyDownDispatchResult::Complete;
    }

    if (_ApplyBackspaceHoldGuard(wParam, lParam))
    {
        *pIsEaten = TRUE;
        _NoteKeyForSmartPunctuation(VK_BACK, ConvertVKey(VK_BACK), true);
        if (deferredReplayToken != 0) _CompleteDeferredKeyReplay(deferredReplayToken);
        return KeyDownDispatchResult::Complete;
    }

    if (translatedWch == nullptr)
    {
        Global::UpdateModifiers(wParam, lParam);
        _TrackModifierHotkeyArming(wParam, lParam, false);
        if (IsShiftVk(LOWORD(wParam)))
        {
            *pIsEaten = FALSE;
            return KeyDownDispatchResult::Complete;
        }
    }

    _KEYSTROKE_STATE KeystrokeState = {};
    WCHAR wch = '\0';
    UINT code = 0;
    uint64_t requestId = FANY_IME_NO_REQUEST_ID;
    const UINT capturedModifiers = modifiersDown ? *modifiersDown : CaptureIpcModifiers();

    if (canDefer && translatedWch == nullptr && prevalidatedKeyState == nullptr && !_HasDeferredKeyBarrier())
    {
        GUID hotkeyGuid = {};
        if (_MatchChordInputHotkey(wParam, &hotkeyGuid))
        {
            _shiftHotkeyArmed = false;
            _ctrlHotkeyArmed = false;
            if (expectedFocusGeneration == 0 || expectedFocusGeneration != _deferredKeyFocusGeneration)
            {
                *pIsEaten = TRUE;
                return KeyDownDispatchResult::Complete;
            }
            _QueueInputHotkey(pContext, hotkeyGuid, pIsEaten);
            return KeyDownDispatchResult::Complete;
        }
    }

    if (canDefer && _HasDeferredKeyBarrier())
    {
        if (!_DeferredKeyQueueHasCapacity() ||
            !_ClassifyDeferredKeyDown(pContext, wParam, lParam, translatedWch, &capturedModifiers, &wch, &code,
                                      &KeystrokeState))
        {
            // Mirror OnTestKeyDown: uneaten keys (esp. Backspace) must still
            // update smart-punctuation rejection state.
            if (code == 0 && wch == L'\0')
            {
                wch = translatedWch ? *translatedWch : ConvertVKey(static_cast<UINT>(wParam));
                code = VKeyFromVKPacketAndWchar(static_cast<UINT>(wParam), wch);
            }
            _NoteKeyForSmartPunctuation(code, wch, false);
            *pIsEaten = FALSE;
            DebugTsfIssue47(L"keydown-deferred-rejected", FANY_IME_NO_REQUEST_ID, code, wch, KeystrokeState.Category,
                            KeystrokeState.Function, 0, _IsComposing(),
                            _pCompositionProcessorEngine ? _pCompositionProcessorEngine->GetVirtualKeyLength() : 0,
                            HRESULT_FROM_WIN32(ERROR_INSUFFICIENT_BUFFER));
            return KeyDownDispatchResult::Complete;
        }
        if (expectedFocusGeneration == 0 || expectedFocusGeneration != _deferredKeyFocusGeneration)
        {
            *pIsEaten = TRUE;
            return KeyDownDispatchResult::Complete;
        }
        // Queued keys note on replay; note now too so a Backspace that is
        // somehow classified+queued still records rejection before drain.
        _NoteKeyForSmartPunctuation(code, wch, true);
        *pIsEaten =
            _QueueDeferredKeyDown(pContext, wParam, lParam, wch, capturedModifiers, KeystrokeState) ? TRUE : FALSE;
        DebugTsfIssue47(*pIsEaten ? L"keydown-deferred-queued" : L"keydown-deferred-queue-failed",
                        FANY_IME_NO_REQUEST_ID, code, wch, KeystrokeState.Category, KeystrokeState.Function,
                        *pIsEaten ? 1 : 0, _IsComposing(),
                        _pCompositionProcessorEngine ? _pCompositionProcessorEngine->GetVirtualKeyLength() : 0,
                        *pIsEaten ? S_OK : E_FAIL);
        if (*pIsEaten && _localSessionResetPending.load(std::memory_order_acquire))
        {
            const UINT resetToken = _localSessionResetToken.load(std::memory_order_acquire);
            _RequestLocalSessionReset(pContext, resetToken);
        }
        return KeyDownDispatchResult::Complete;
    }

    if (prevalidatedKeyState != nullptr)
    {
        KeystrokeState = *prevalidatedKeyState;
        wch = translatedWch ? *translatedWch : ConvertVKey(static_cast<UINT>(wParam));
        code = VKeyFromVKPacketAndWchar(static_cast<UINT>(wParam), wch);
        *pIsEaten = TRUE;
    }
    else
    {
        PerfTimer isKeyEatenTimer;
        *pIsEaten = _IsKeyEaten( //
            pContext,            //
            (UINT)wParam,        //
            &code,               //
            &wch,                //
            &KeystrokeState,     //
            translatedWch        //
        );
    }
    // Idempotent with the OnTestKeyDown call; replayed keys only pass here.
    _NoteKeyForSmartPunctuation(code, wch, *pIsEaten ? true : false);

    DebugTsfIssue47(L"keydown-classified", FANY_IME_NO_REQUEST_ID, code, wch, KeystrokeState.Category,
                    KeystrokeState.Function, *pIsEaten ? 1 : 0, _IsComposing(),
                    _pCompositionProcessorEngine ? _pCompositionProcessorEngine->GetVirtualKeyLength() : 0, S_OK,
                    deferredReplayToken);

    if (expectedFocusGeneration == 0 || expectedFocusGeneration != _deferredKeyFocusGeneration)
    {
        // A COM callback inside key classification changed the focused
        // topology. The old key must not enter the replacement Server epoch.
        *pIsEaten = TRUE;
        DebugTsfIssue47(L"keydown-focus-generation-changed", FANY_IME_NO_REQUEST_ID, code, wch, KeystrokeState.Category,
                        KeystrokeState.Function, 1, _IsComposing(),
                        _pCompositionProcessorEngine ? _pCompositionProcessorEngine->GetVirtualKeyLength() : 0, S_FALSE,
                        deferredReplayToken);
        return KeyDownDispatchResult::Complete;
    }

    const bool resetPending = _localSessionResetPending.load(std::memory_order_acquire);
    if (resetPending)
    {
        if (!canDefer)
        {
            DebugTsfIssue47(L"keydown-reset-retry", FANY_IME_NO_REQUEST_ID, code, wch, KeystrokeState.Category,
                            KeystrokeState.Function, 1, _IsComposing(),
                            _pCompositionProcessorEngine ? _pCompositionProcessorEngine->GetVirtualKeyLength() : 0,
                            S_FALSE, deferredReplayToken);
            return KeyDownDispatchResult::Retry;
        }

        // The reset gate may have closed concurrently with normal
        // classification. Reclassify against the FIFO's future state before
        // retaining the key.
        if (!_DeferredKeyQueueHasCapacity() ||
            !_ClassifyDeferredKeyDown(pContext, wParam, lParam, translatedWch, &capturedModifiers, &wch, &code,
                                      &KeystrokeState) ||
            !_QueueDeferredKeyDown(pContext, wParam, lParam, wch, capturedModifiers, KeystrokeState))
        {
            *pIsEaten = FALSE;
        }
        const UINT resetToken = _localSessionResetToken.load(std::memory_order_acquire);
        DebugTsfIssue47(*pIsEaten ? L"keydown-reset-queued" : L"keydown-reset-queue-failed", FANY_IME_NO_REQUEST_ID,
                        code, wch, KeystrokeState.Category, KeystrokeState.Function, *pIsEaten ? 1 : 0, _IsComposing(),
                        _pCompositionProcessorEngine ? _pCompositionProcessorEngine->GetVirtualKeyLength() : 0,
                        *pIsEaten ? S_OK : E_FAIL, resetToken);
        _RequestLocalSessionReset(pContext, resetToken);
        return KeyDownDispatchResult::Complete;
    }

    if (canDefer && *pIsEaten && (KeystrokeState.Category != CATEGORY_NONE || KeystrokeState.Function != FUNCTION_NONE))
    {
        // Give every IME-owned key a member-owned replay token before any IPC
        // write or asynchronous TSF edit session is started.  Consequently a
        // write success followed by a reply/edit failure follows the same
        // exact retry path as a key that arrived behind a reconnect barrier.
        const bool healthyImmediateDispatch = _deferredKeyDowns.empty() && !_hasDeferredKeyInFlight;
        const bool queued =
            _QueueDeferredKeyDown(pContext, wParam, lParam, wch, capturedModifiers, KeystrokeState) != FALSE;
        *pIsEaten = queued ? TRUE : FALSE;
        DebugTsfIssue47(queued ? L"keydown-owned-queued" : L"keydown-owned-queue-failed", FANY_IME_NO_REQUEST_ID, code,
                        wch, KeystrokeState.Category, KeystrokeState.Function, queued ? 1 : 0, _IsComposing(),
                        _pCompositionProcessorEngine ? _pCompositionProcessorEngine->GetVirtualKeyLength() : 0,
                        queued ? S_OK : E_FAIL, deferredReplayToken);
        if (queued && healthyImmediateDispatch)
        {
            // Healthy fast path: drain the FIFO synchronously inside
            // OnKeyDown instead of waiting for the posted
            // WM_DrainDeferredKeyDown.  In Excel, the first keydown is what
            // puts the selected cell into edit mode; the app pushes a new TSF
            // context during that same message, and OnPushContext clears the
            // deferred queue before the posted drain could run, swallowing
            // the first key.  Dispatching immediately restores the
            // reference-sample timing while retaining the replay-token
            // machinery for IPC/edit-session failures.  If the transport or
            // focus session is not ready, _DrainOneDeferredKeyDown leaves the
            // key queued for the ordinary asynchronous retry path.
            _DrainOneDeferredKeyDown();
        }
        return KeyDownDispatchResult::Complete;
    }

    const bool isPunctuationKey = _pCompositionProcessorEngine && _pCompositionProcessorEngine->IsPunctuation(wch);
    const bool isNoOpForwardDelete =
        KeystrokeState.Function == FUNCTION_DELETE && _pCompositionProcessorEngine &&
        _pCompositionProcessorEngine->GetCaretPosition() >= _pCompositionProcessorEngine->GetVirtualKeyLength();

    Global::firefox_like_cnt = 0;

    /* Send key event to server process */
    if (*pIsEaten && !isNoOpForwardDelete)
    {
        // 检查是否应该跳过发送此键到服务器，由于长度限制。
        // 当达到限制时，我们只阻止字符输入键（FUNCTION_INPUT）。
        // 这允许功能键如 Backspace、Space、Enter 仍然工作。
        if (KeystrokeState.Function == FUNCTION_INPUT &&
            _pCompositionProcessorEngine->GetVirtualKeyLength() >= MAX_PINYIN_LENGTH)
        {
            // 这个键仍然被吃掉（以防止它到达应用程序），
            // 但我们不把它发送到 server 端，也不进一步处理它。
            DebugTsfIssue47(L"keydown-input-length-limit", FANY_IME_NO_REQUEST_ID, code, wch, KeystrokeState.Category,
                            KeystrokeState.Function, 1, _IsComposing(),
                            _pCompositionProcessorEngine->GetVirtualKeyLength(),
                            HRESULT_FROM_WIN32(ERROR_BUFFER_OVERFLOW), deferredReplayToken);
            return KeyDownDispatchResult::Complete;
        }

        if (expectedFocusGeneration != _deferredKeyFocusGeneration)
        {
            return KeyDownDispatchResult::Complete;
        }

        if (KeystrokeState.Function == FUNCTION_TOGGLE_CHARACTER_SET && !SupportsCharacterSetShortcut())
        {
            // The transport may have been replaced by an older Server after
            // TestKeyDown owned this key. Do not send it as ordinary input.
            return KeyDownDispatchResult::Complete;
        }

        Global::Keycode = code;
        Global::wch = wch;
        // The modifiers the key was classified with: an AltGr character goes without Ctrl+Alt, or the Server would cancel it as a shortcut.
        Global::ModifiersDown = Global::CharacterModifiers(capturedModifiers, wch, code);

        PerfTimer writeShmTimer;
        // Enter is finalized by the in-process TSF path. Reuse the legacy
        // pinyin payload to carry the exact bounded text that path is about
        // to commit, allowing the Server session to validate and clear its
        // matching raw composition instead of rejecting an unobserved commit.
        std::wstring localCommitObservation;
        const bool hasLocalCommitObservation =
            code == VK_RETURN && _IsComposing() && _pCompositionProcessorEngine;
        if (hasLocalCommitObservation)
        {
            localCommitObservation = GlobalIme::word_for_creating_word;
            const CStringRange &raw = _pCompositionProcessorEngine->GetKeystrokeBuffer();
            if (raw.Get() != nullptr)
            {
                localCommitObservation.append(raw.Get(), raw.GetLength());
            }
            if (localCommitObservation.size() >= 128)
            {
                // A truncated observation is worse than an explicit fail
                // closed response: the Server must not acknowledge text that
                // differs from what TSF committed.
                localCommitObservation.clear();
            }
        }
        const UINT ipcModifiers =
            Global::ModifiersDown |
            (_candidateMode == CANDIDATE_ORIGINAL
                 ? msime::windows::PipeMetadata::CandidateActive
                 : 0u) |
            (IsAutoRepeat(lParam) ? msime::windows::PipeMetadata::AutoRepeat : 0u);
        WriteDataToNamedPipe(Global::Keycode, wch, ipcModifiers, nullptr, 0,
                             localCommitObservation,
                             hasLocalCommitObservation && !localCommitObservation.empty()
                                 ? 0b110111
                                 : 0b000111);

        PerfTimer sendKeyEventTimer;
        const KeyEventSendResult sendResult = SendKeyEventToUIProcess(&requestId);
        if (KeystrokeState.Function == FUNCTION_TOGGLE_CHARACTER_SET)
        {
            // No document edit or response is needed. In particular, an ambiguous
            // delivery must never replay a toggle after reconnecting.
            return KeyDownDispatchResult::Complete;
        }
        DebugTsfKeyLatency(L"main-pipe-send", requestId, sendKeyEventTimer.ElapsedMs(),
                           sendResult == KeyEventSendResult::Sent ? S_OK : E_FAIL);
        DebugTsfIssue47(sendResult == KeyEventSendResult::Sent ? L"keydown-sent" : L"keydown-send-failed", requestId,
                        code, wch, KeystrokeState.Category, KeystrokeState.Function, *pIsEaten ? 1 : 0, _IsComposing(),
                        _pCompositionProcessorEngine->GetVirtualKeyLength(),
                        sendResult == KeyEventSendResult::Sent ? S_OK : E_FAIL, deferredReplayToken);
        if (sendResult != KeyEventSendResult::Sent)
        {
            if (!canDefer)
            {
                return KeyDownDispatchResult::Retry;
            }
            const bool queued = _QueueDeferredKeyDown(pContext, wParam, lParam, wch, capturedModifiers, KeystrokeState);
            if (!queued)
            {
                // This path is defensive now that every normal eaten key is
                // tokenized before dispatch.  If it is ever reached, handing
                // the key back is preferable to silently dropping an
                // ambiguous delivery.
                *pIsEaten = FALSE;
            }
            // A failed write dirties the session and forces a new activation
            // token. If the old Server epoch did receive the ambiguous frame,
            // that epoch is rejected/cleared before this queued key is replayed.
            // Thus retrying after the exact FocusSessionReady fence cannot
            // apply the same key twice to one Server composition.
            return KeyDownDispatchResult::Complete;
        }

        if (KeystrokeState.Function == FUNCTION_SERVER_CANDIDATE_KEY && _msgWndHandle)
        {
            _PostAsyncKeyRequest(WM_AsyncServerCandidateKey, code, wch, requestId, {}, 0, 0, deferredReplayToken);
            return deferredReplayToken != 0 ? KeyDownDispatchResult::AwaitingCompletion
                                            : KeyDownDispatchResult::Complete;
        }

        if (code == VK_SPACE && KeystrokeState.Function == FUNCTION_CONVERT)
        {
            if (_msgWndHandle)
            {
                _PostAsyncKeyRequest(WM_AsyncFinalizeCandidate, code, wch, requestId, {}, 0, 0, deferredReplayToken);
                return deferredReplayToken != 0 ? KeyDownDispatchResult::AwaitingCompletion
                                                : KeyDownDispatchResult::Complete;
            }
        }

        if (KeystrokeState.Function == FUNCTION_PUNCTUATION && _msgWndHandle)
        {
            PerfTimer asyncPuncTimer;
            std::wstring punctuationCommitText;
            const bool shouldFinalizeHighlightedCandidateWithPunctuation =
                _candidateMode != CANDIDATE_NONE && _pCandidateListUIPresenter &&
                Global::CommitWithHighlightedCandPunc.count(wch) > 0;
            if (shouldFinalizeHighlightedCandidateWithPunctuation)
            {
                // Empty means the edit session must consume this request's
                // candidate reply and append the punctuation derived from wch
                // (including smart-punctuation against the candidate text).
                punctuationCommitText.clear();
            }
            else if (code == VK_DECIMAL)
            {
                // Numpad '.' keeps ASCII '.' in Chinese punctuation mode. It is checked after the highlighted-candidate commit, so with candidates open it ends the composition with the candidate followed by '.' instead of discarding the candidate.
                punctuationCommitText = L".";
            }
            else if (CCompositionProcessorEngine::IsSmartAsciiPunctuationKey(wch) &&
                     Global::SmartPunctuationEnabled.load(std::memory_order_relaxed))
            {
                // Defer mapping until the edit session can inspect the
                // preceding document character (letters/digits → ASCII).
                punctuationCommitText.clear();
            }
            else
            {
                const WCHAR *punctuation = _pCompositionProcessorEngine->GetPunctuation(wch);
                punctuationCommitText = punctuation ? punctuation : L"";
            }
            _PostAsyncKeyRequest(WM_AsyncPunctuationCommit, code, wch, requestId, std::move(punctuationCommitText), 0,
                                 0, deferredReplayToken);
            return deferredReplayToken != 0 ? KeyDownDispatchResult::AwaitingCompletion
                                            : KeyDownDispatchResult::Complete;
        }

        if (KeystrokeState.Function == FUNCTION_SELECT_BY_NUMBER && _msgWndHandle)
        {
            _PostAsyncKeyRequest(WM_AsyncNumberCandidateCommit, code, wch, requestId, {}, 0, 0, deferredReplayToken);
            return deferredReplayToken != 0 ? KeyDownDispatchResult::AwaitingCompletion
                                            : KeyDownDispatchResult::Complete;
        }
    }

    if (*pIsEaten)
    {
        bool needInvokeKeyHandler = true;
        /* Invoke key handler edit session */
        if (code == VK_ESCAPE)
        {
            KeystrokeState.Category = CATEGORY_COMPOSING;
        }

        /* Always eat THIRDPARTY_NEXTPAGE and THIRDPARTY_PREVPAGE
        keys, but don't always process them. */
        if ((wch == THIRDPARTY_NEXTPAGE) || (wch == THIRDPARTY_PREVPAGE))
        {
            needInvokeKeyHandler = !((KeystrokeState.Category == CATEGORY_NONE) && //
                                     (KeystrokeState.Function == FUNCTION_NONE));
        }
        if (needInvokeKeyHandler)
        {
            PerfTimer invokeTimer;
            _InvokeKeyHandler(pContext, code, wch, (DWORD)lParam, KeystrokeState, requestId, {}, 0, 0, 0,
                              deferredReplayToken);
            if (deferredReplayToken != 0)
            {
                return KeyDownDispatchResult::AwaitingCompletion;
            }
        }
    }
    else if (KeystrokeState.Category == CATEGORY_INVOKE_COMPOSITION_EDIT_SESSION)
    {
        // Invoke key handler edit session
        KeystrokeState.Category = CATEGORY_COMPOSING;
        PerfTimer invokeTimer;
        _InvokeKeyHandler(pContext, code, wch, (DWORD)lParam, KeystrokeState, FANY_IME_NO_REQUEST_ID);
    }

    if (isPunctuationKey)
    {
    }
    return KeyDownDispatchResult::Complete;
}

//+---------------------------------------------------------------------------
//
// ITfKeyEventSink::OnTestKeyUp
//
// Called by the system to query this service wants a potential keystroke.
//----------------------------------------------------------------------------

STDAPI CMetasequoiaIME::OnTestKeyUp(ITfContext *pContext, WPARAM wParam, LPARAM lParam, BOOL *pIsEaten)
{
    if (pContext == nullptr || pIsEaten == nullptr)
    {
        return E_INVALIDARG;
    }
    if (IsSelfGeneratedSendInputExtraInfo(static_cast<ULONG_PTR>(GetMessageExtraInfo())))
    {
        *pIsEaten = FALSE;
        return S_OK;
    }

    Global::UpdateModifiers(wParam, lParam);

    if (IsShiftVk(LOWORD(wParam)))
    {
        // TSF does not call OnKeyUp after a FALSE test result. Claim and
        // queue the bare-Shift toggle here while leaving its release visible.
        // Hosts that never route the release here (Word, mintty) fall back to
        // the bare-Shift keyboard hook, which _MarkBareShiftHandled() disarms.
        GUID hotkeyGuid = {};
        BOOL queued = FALSE;
        const bool toggled =
            _MatchModifierReleaseHotkey(wParam, &hotkeyGuid) && _QueueInputHotkey(pContext, hotkeyGuid, &queued);
        if (toggled)
        {
            _MarkBareShiftHandled();
        }
        DebugTsfIssue47(L"bare-shift-testkeyup", FANY_IME_NO_REQUEST_ID, LOWORD(wParam), L'\0', 0, 0, toggled ? 1 : 0,
                        _IsComposing(),
                        _pCompositionProcessorEngine ? _pCompositionProcessorEngine->GetVirtualKeyLength() : 0, S_OK);
        ClearReleasedShiftModifierState();
        *pIsEaten = FALSE;
        return S_OK;
    }

    // Ahead of the barrier check: the tap is queued behind any keys still waiting, which keeps it in order.
    if (_QueueKoreanHanjaTap(pContext, wParam, lParam))
    {
        // As with the language toggle the release itself still reaches the application, and OnKeyUp will not be called for it.
        *pIsEaten = FALSE;
        return S_OK;
    }

    if (_HasDeferredKeyBarrier())
    {
        // A matching deferred key-down may or may not have fit in the bounded
        // queue. Letting all key-ups through is harmless and guarantees the
        // application never observes a down without its release.
        *pIsEaten = FALSE;
        return S_OK;
    }

    GUID hotkeyGuid = {};
    if (_MatchModifierReleaseHotkey(wParam, &hotkeyGuid))
    {
        // A bare Ctrl release that toggles the input mode; Shift returned
        // above. Same rule as the Shift branch: the toggle does not need to
        // own the keystroke, and a host that keys off the release loses its
        // own shortcut when it is eaten (double-Ctrl is Run Anything in
        // JetBrains IDEs). Report the key as not eaten -- which also means
        // OnKeyUp will not be called, so queue the toggle here instead of
        // peeking.
        BOOL hotkeyQueued = FALSE;
        (void)_QueueInputHotkey(pContext, hotkeyGuid, &hotkeyQueued);
        *pIsEaten = FALSE;
        return S_OK;
    }

    _KEYSTROKE_STATE KeystrokeState = {};
    WCHAR wch = '\0';
    UINT code = 0;

    *pIsEaten = _IsKeyEaten(pContext, (UINT)wParam, &code, &wch, &KeystrokeState);

    return S_OK;
}

//+---------------------------------------------------------------------------
//
// ITfKeyEventSink::OnKeyUp
//
// Called by the system to offer this service a keystroke.  If *pIsEaten == TRUE
// on exit, the application will not handle the keystroke.
//----------------------------------------------------------------------------

STDAPI CMetasequoiaIME::OnKeyUp(ITfContext *pContext, WPARAM wParam, LPARAM lParam, BOOL *pIsEaten)
{
    if (pContext == nullptr || pIsEaten == nullptr)
    {
        return E_INVALIDARG;
    }
    if (IsSelfGeneratedSendInputExtraInfo(static_cast<ULONG_PTR>(GetMessageExtraInfo())))
    {
        *pIsEaten = FALSE;
        return S_OK;
    }
    Global::UpdateModifiers(wParam, lParam);

    if (IsShiftVk(LOWORD(wParam)))
    {
        // Defend against hosts that offer KeyUp without a preceding test.
        GUID hotkeyGuid = {};
        BOOL queued = FALSE;
        const bool toggled =
            _MatchModifierReleaseHotkey(wParam, &hotkeyGuid) && _QueueInputHotkey(pContext, hotkeyGuid, &queued);
        if (toggled)
        {
            _MarkBareShiftHandled();
        }
        DebugTsfIssue47(L"bare-shift-keyup", FANY_IME_NO_REQUEST_ID, LOWORD(wParam), L'\0', 0, 0, toggled ? 1 : 0,
                        _IsComposing(),
                        _pCompositionProcessorEngine ? _pCompositionProcessorEngine->GetVirtualKeyLength() : 0, S_OK);
        ClearReleasedShiftModifierState();
        *pIsEaten = FALSE;
        return S_OK;
    }

    // For hosts that call OnKeyUp without OnTestKeyUp, as the language toggle below.
    if (_QueueKoreanHanjaTap(pContext, wParam, lParam))
    {
        *pIsEaten = FALSE;
        return S_OK;
    }

    if (_HasDeferredKeyBarrier())
    {
        *pIsEaten = FALSE;
        return S_OK;
    }

    GUID hotkeyGuid = {};
    if (_MatchModifierReleaseHotkey(wParam, &hotkeyGuid))
    {
        // Ctrl again; OnTestKeyUp normally ran the toggle and disarmed
        // already, so this only fires for hosts that call KeyUp without
        // TestKeyUp. Same rule as there: run the toggle, hand the release
        // back to the host.
        BOOL hotkeyQueued = FALSE;
        (void)_QueueInputHotkey(pContext, hotkeyGuid, &hotkeyQueued);
        *pIsEaten = FALSE;
        return S_OK;
    }

    _KEYSTROKE_STATE KeystrokeState = {};
    WCHAR wch = '\0';
    UINT code = 0;
    *pIsEaten = _IsKeyEaten(pContext, (UINT)wParam, &code, &wch, &KeystrokeState);

    return S_OK;
}

//+---------------------------------------------------------------------------
//
// ITfKeyEventSink::OnPreservedKey
//
// Called when a hotkey (registered by us, or by the system) is typed.
//----------------------------------------------------------------------------

STDAPI CMetasequoiaIME::OnPreservedKey(ITfContext *pContext, REFGUID rguid, BOOL *pIsEaten)
{
    if (pContext == nullptr || pIsEaten == nullptr)
    {
        return E_INVALIDARG;
    }
    // No TSF PreserveKey registrations remain for input-mode shortcuts.
    // Shift/Ctrl/Ctrl+Alt+Space/Ctrl+Shift+Space/Ctrl+./Ctrl+Shift+E are all
    // handled from ITfKeyEventSink.
    UNREFERENCED_PARAMETER(rguid);
    *pIsEaten = FALSE;
    return S_OK;
}

void CMetasequoiaIME::_DispatchPreservedKey(_In_ ITfContext *pContext, REFGUID preservedKey, _Out_ BOOL *pIsEaten,
                                            uint64_t expectedFocusGeneration, bool isPrevalidated,
                                            uint64_t deferredReplayToken)
{
    *pIsEaten = FALSE;
    if (pContext == nullptr || _pCompositionProcessorEngine == nullptr || expectedFocusGeneration == 0 ||
        expectedFocusGeneration != _deferredKeyFocusGeneration)
    {
        return;
    }

    BOOL pNeedToggleIMEMode = FALSE;

    _pCompositionProcessorEngine->OnPreservedKey(       //
        pContext,                                       //
        preservedKey,                                   //
        pIsEaten,                                       //
        _GetThreadMgr(),                                //
        _GetClientId(),                                 //
        &pNeedToggleIMEMode,                            //
        isPrevalidated ? TRUE : FALSE,                  //
        _serverUnavailableFallbackActive ? FALSE : TRUE //
    );

    if (pNeedToggleIMEMode && expectedFocusGeneration == _deferredKeyFocusGeneration)
    {
        // The preserved-key implementation also sends the Shift event to the
        // Server.  A failed/ambiguous send marks the local session dirty.  Do
        // not let the subsequent local edit falsely Complete this token; the
        // replacement epoch receives the authoritative status snapshot and
        // then retries only the already-applied local toggle phase.
        if (deferredReplayToken != 0 && _localSessionResetPending.load(std::memory_order_acquire))
        {
            _RetryDeferredKeyReplay(deferredReplayToken);
            return;
        }
        _KEYSTROKE_STATE KeystrokeState = {};
        WCHAR wch = '\0';
        UINT code = 0;
        KeystrokeState.Category = CATEGORY_COMPOSING;
        KeystrokeState.Function = FUNCTION_TOGGLE_IME_MODE;
        _InvokeKeyHandler(pContext, code, wch, (DWORD)0, KeystrokeState, FANY_IME_NO_REQUEST_ID, {}, 0, 0, 0,
                          deferredReplayToken);
    }
}

//+---------------------------------------------------------------------------
//
// _InitKeyEventSink
//
// Advise a keystroke sink.
//----------------------------------------------------------------------------

BOOL CMetasequoiaIME::_InitKeyEventSink()
{
    ITfKeystrokeMgr *pKeystrokeMgr = nullptr;
    HRESULT hr = S_OK;

    if (FAILED(_pThreadMgr->QueryInterface(IID_ITfKeystrokeMgr, (void **)&pKeystrokeMgr)))
    {
        return FALSE;
    }

    hr = pKeystrokeMgr->AdviseKeyEventSink(_tfClientId, (ITfKeyEventSink *)this, TRUE);

    pKeystrokeMgr->Release();

    return (hr == S_OK);
}

//+---------------------------------------------------------------------------
//
// _UninitKeyEventSink
//
// Unadvise a keystroke sink.  Assumes we have advised one already.
//----------------------------------------------------------------------------

void CMetasequoiaIME::_UninitKeyEventSink()
{
    ITfKeystrokeMgr *pKeystrokeMgr = nullptr;

    if (FAILED(_pThreadMgr->QueryInterface(IID_ITfKeystrokeMgr, (void **)&pKeystrokeMgr)))
    {
        return;
    }

    pKeystrokeMgr->UnadviseKeyEventSink(_tfClientId);

    pKeystrokeMgr->Release();
}
