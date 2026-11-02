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
#include "PassthroughKeySoundQueue.h"
#include "../../common/KeySoundClass.h"
#include "FanyDefines.h"
#include "AltGrKeyPolicy.h"
#include "FullwidthChordPolicy.h"
#include "CandidateActiveKeyPolicy.h"
#include "FanyUtils.h"
#include "FanyLog.h"
#include "../Utils/PerfTimer.h"
#include "../HostKoreanKey.h"
#include "../HostPrediction.h"
#include "../../common/PipeMetadata.h"
#include "../../common/SecondThirdCandidatePolicy.h"
#include "../../common/WubiCandidateSelectionPolicy.h"
#include <atomic>
#include <chrono>
#include "../../../../shared/contracts/ipc_negotiation.h"

bool CMetasequoiaIME::_HasGroupedPanelComposition() const
{
    auto *host = _pCompositionProcessorEngine ? _pCompositionProcessorEngine->GetHostEngineAdapter() : nullptr;
    std::string raw, error;
    msime::tsf::EngineResult current;
    return host && host->valid() && host->view(&raw, &error) &&
           msime::tsf::EngineSessionAdapter::parse_result(raw, &current, &error) &&
           current.view.key_grid == "eighteen_key" &&
           (!current.view.editing_text.empty() || !current.view.phrase_prefix.empty());
}

bool CMetasequoiaIME::_CaptureGroupedPanelOwner(msime::panel_key::Owner &owner, ITfContext **context)
{
    *context = nullptr;
    if (!_pThreadMgr || !_pCompositionProcessorEngine || _serverUnavailableFallbackActive ||
        !SupportsGroupedPanelKeys() || _IsKeyboardDisabled() ||
        _localSessionResetPending.load(std::memory_order_acquire) ||
        !_pCompositionProcessorEngine->GetIMEMode(_pThreadMgr, _tfClientId) ||
        Global::InputModeScheme.load(std::memory_order_relaxed) != msime::windows::scheme::Quanpin)
        return false;
    const auto generation = _deferredKeyFocusGeneration;
    const auto workerToken = _CaptureFocusSessionToken();
    const HWND foreground = GetForegroundWindow();
    GUITHREADINFO info = {};
    info.cbSize = sizeof(info);
    if (!GetGUIThreadInfo(GetCurrentThreadId(), &info) || !info.hwndFocus) return false;
    const HWND focus = info.hwndFocus;
    ITfDocumentMgr *document = nullptr;
    if (FAILED(_pThreadMgr->GetFocus(&document)) || !document) return false;
    const HRESULT topResult = document->GetTop(context);
    document->Release();
    if (FAILED(topResult) || !*context) return false;
    const bool matches = _IsDeferredKeyOwnerCurrent(*context, generation, foreground) &&
                         _IsFocusSessionCurrent(workerToken, *context);
    info = {};
    info.cbSize = sizeof(info);
    if (!matches || generation != _deferredKeyFocusGeneration || workerToken != _CaptureFocusSessionToken() ||
        foreground != GetForegroundWindow() || !GetGUIThreadInfo(GetCurrentThreadId(), &info) ||
        info.hwndFocus != focus)
    {
        (*context)->Release();
        *context = nullptr;
        return false;
    }
    owner = {generation, workerToken, reinterpret_cast<std::uintptr_t>(*context),
             reinterpret_cast<std::uintptr_t>(foreground), reinterpret_cast<std::uintptr_t>(focus)};
    return owner.valid();
}

bool CMetasequoiaIME::_IsGroupedPanelReplayCurrent(uint64_t token)
{
    if (!_hasDeferredKeyInFlight || token == 0 || _deferredKeyReplayToken != token ||
        !_deferredKeyInFlight.keyState.groupedPanelKey) return false;
    const auto expected = _deferredKeyInFlight.groupedOwner;
    msime::panel_key::Owner current;
    ITfContext *context = nullptr;
    if (!_CaptureGroupedPanelOwner(current, &context)) return false;
    context->Release();
    return _hasDeferredKeyInFlight && token == _deferredKeyReplayToken && current == expected &&
           expected.generation == _deferredKeyFocusGeneration && expected.worker_token == _CaptureFocusSessionToken();
}

bool CMetasequoiaIME::_HandleGroupedPanelKey(const msime::panel_key::Request &request)
{
    if (!msime::panel_key::valid(request)) return false;
    msime::panel_key::Owner owner;
    ITfContext *context = nullptr;
    if (!_CaptureGroupedPanelOwner(owner, &context)) return false;
    struct ReleaseContext {
        ITfContext *value;
        ~ReleaseContext() { value->Release(); }
    } release{context};
    auto *host = _pCompositionProcessorEngine->GetHostEngineAdapter();
    std::string raw, error;
    msime::tsf::EngineResult current;
    if (!host || !host->valid() || !host->view(&raw, &error) ||
        !msime::tsf::EngineSessionAdapter::parse_result(raw, &current, &error) ||
        current.view.scheme != msime::windows::scheme::Quanpin || current.view.dedicated_english ||
        current.view.local_mode != "none" ||
        ((!current.view.editing_text.empty() || !current.view.phrase_prefix.empty()) &&
         current.view.key_grid != "eighteen_key")) return false;
    // 不能只检查当前空组合：尚未执行的硬件字母也会开始一段确定全拼。
    if ((_hasDeferredKeyInFlight && !_deferredKeyInFlight.keyState.groupedPanelKey) ||
        _deferredKeyClassificationPending || _HasPendingApplicationKeyReplay()) return false;
    for (const auto &key : _deferredKeyDowns)
        if (!key.keyState.groupedPanelKey) return false;
    const auto now = GetTickCount64();
    if (request.operation == msime::panel_key::Operation::Query)
        return _groupedPanelLeases.query(request.nonce, owner, now);
    return _groupedPanelLeases.press(request.nonce, owner, now, [&] {
        if (!_DeferredKeyQueueHasCapacity() ||
            !_IsDeferredKeyOwnerCurrent(context, owner.generation, reinterpret_cast<HWND>(owner.foreground)))
            return false;
        _KEYSTROKE_STATE state = {};
        state.Category = CATEGORY_COMPOSING;
        state.Function = FUNCTION_INPUT;
        state.groupedPanelKey = true;
        return _QueueDeferredKeyDown(context, request.letter - 'a' + 'A', 0,
                                     static_cast<WCHAR>(request.letter), 0, state, &owner);
    });
}

// 0xF003, 0xF004 are the keys that the touch keyboard sends for next/previous
#define THIRDPARTY_NEXTPAGE static_cast<WORD>(0xF003)
#define THIRDPARTY_PREVPAGE static_cast<WORD>(0xF004)

namespace
{
constexpr UINT kMaxDeferredKeyReplayAttempts = 8;

// 诊断日志里拼「1」或「0」时，`L"x=" + (cond ? L"1" : L"0")` 两边都是指针，会变成指针加。
// 统一走这个函数，返回 std::wstring，和左右两侧的 wstring 串接。
inline std::wstring BoolFlag(bool value)
{
    return value ? L"1" : L"0";
}

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
    bool phraseActive = false;
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
        shadow.phraseActive = false;
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
        // 有暂存词段时删空可能恢复读音或上屏，等 Runtime 返回后再分类后续键。
        if (shadow.phraseActive) { shadow.projectionValid = false; break; }
        shadow.caret = min(shadow.caret, shadow.rawInput.size());
        if (shadow.caret > 0)
        {
            const WCHAR removed = shadow.rawInput[shadow.caret - 1];
            shadow.rawInput.erase(shadow.caret - 1, 1);
            --shadow.caret;
            shadow.urlMode = Global::UrlModeAfterDeletion(shadow.urlMode, shadow.rawInput.c_str(),
                                                          shadow.rawInput.size(), removed,
                                                          shadow.caret == shadow.rawInput.size());
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
        // 日语空格通常保留组字。选择结果未定时后续普通键只捕获原文，执行时读取真实状态；唯一 Fallback 上屏后的空格因此交回应用。
        if (code == VK_SPACE &&
            Global::InputModeScheme.load(std::memory_order_relaxed) == msime::windows::scheme::Japanese)
        {
            break;
        }
        // 普通空格走候选完成；补全或部分选词是否继续组字，要到执行后才能确定。
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
            shadow.urlMode = Global::UrlModeAfterDeletion(shadow.urlMode, shadow.rawInput.c_str(),
                                                          shadow.rawInput.size(), removed,
                                                          shadow.caret == shadow.rawInput.size());
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

// 释义列快捷键（Server 的 GlossColumnPolicy.h）：只按 Alt 的数字上屏那个候选的第 1 列释义，只按 Ctrl 的数字上屏第 2 列，和 macOS 的 Option/Control+数字一样。只认主键盘的 1-9：Alt+小键盘数字是 Windows 的 Alt 码输入。
bool IsGlossColumnShortcut(UINT code, UINT modifiers)
{
    const UINT chord = modifiers & 0b00000111u;
    return code >= '1' && code <= '9' && (chord == 0b00000010u || chord == 0b00000100u) &&
           (GetAsyncKeyState(VK_LWIN) & 0x8000) == 0 && (GetAsyncKeyState(VK_RWIN) & 0x8000) == 0;
}

// 分段编辑键，和 Server 的 is_segment_backspace_key / is_segment_caret_key（src/input/InputKeyPolicy.h）同一条规则：只按 Ctrl 的退格、左右键，以及只按 Shift 的左右键，都不带 Alt 和 Win。Shift+左右在所有方案里和 Ctrl+左右一样按音节移动拼音光标（全拼、双拼也是，不再换成逐个字母）；韩文、注音、越南文和藏文的光标锁在组字末尾（scheme::LocksCaret），Shift+左右在那里照旧走宿主组字的按键规则（host_composed_key_action）。Shift+退格不是分段键。调用方只在组字时接这些键，没有组字时它们属于应用（选择文字、按词跳）。
KEYSTROKE_FUNCTION SegmentEditFunction(UINT code, UINT modifiers, int scheme)
{
    const UINT chord = modifiers & 0b00000111u;
    const bool shiftCaret = chord == 0b00000001u && !msime::windows::scheme::LocksCaret(scheme);
    if ((chord != 0b00000010u && !shiftCaret) || (GetAsyncKeyState(VK_LWIN) & 0x8000) != 0 ||
        (GetAsyncKeyState(VK_RWIN) & 0x8000) != 0)
    {
        return FUNCTION_NONE;
    }
    switch (code)
    {
    case VK_BACK: return shiftCaret ? FUNCTION_NONE : FUNCTION_BACKSPACE_SEGMENT;
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

// Ctrl+Shift+J 在中文方案和日语之间切换：开关打开、Server 协商到这一位、本版本同时提供两头时才吃。Engine 自己的英文模式（Ctrl+Shift+E）里 TIP 仍报告中文，这时组合键交给应用，和中文输入法关着时一样。
bool IsJapaneseInputModeToggle(UINT code, UINT modifiers)
{
    return FanyImeProtocol::IsJapaneseToggleShortcut(code, modifiers) && (GetAsyncKeyState(VK_LWIN) & 0x8000) == 0 &&
           (GetAsyncKeyState(VK_RWIN) & 0x8000) == 0 && msime::windows::scheme::offers_japanese_toggle() &&
           SupportsJapaneseToggleShortcut() && !Global::DedicatedEnglish.active(GetTickCount64()) &&
           FanyUtils::ReadConfiguredSwitchLanguageHotkeys().japanese_toggle_ctrl_shift_j;
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

// 除 Caps Lock 自己以外还有键按着：Caps Lock 是组合键的一部分，不算轻点。
bool IsKeyOtherThanCapsLockDown()
{
    for (UINT code = VK_BACK; code <= 0xfe; ++code)
    {
        if (code != VK_CAPITAL && (GetAsyncKeyState(code) & 0x8000) != 0)
        {
            return true;
        }
    }
    return false;
}

// 「要关闭 Caps Lock：按 SHIFT 键」（KLF_SHIFTLOCK）打开时，再按一次 Caps Lock 关不掉大写锁定，注入的那一下也一样，SendInput 却照样报告成功。这时撤不回，所以不布防，轻点照常是大写锁定。这个设置存在 HKCU\Keyboard Layout 的 Attributes 里，只在开关打开时、每次按下 Caps Lock 读一次。
bool CapsLockOffNeedsShift()
{
    DWORD attributes = 0;
    DWORD size = sizeof(attributes);
    return RegGetValueW(HKEY_CURRENT_USER, L"Keyboard Layout", L"Attributes", RRF_RT_REG_DWORD, nullptr, &attributes,
                        &size) == ERROR_SUCCESS &&
           (attributes & KLF_SHIFTLOCK) != 0;
}

// 撤回轻点打开的大写锁定：再按一次 Caps Lock，带上自己的标记，不会被当成又一次轻点。
bool ReleaseCapsLockAfterTap()
{
    INPUT inputs[2] = {};
    inputs[0].type = INPUT_KEYBOARD;
    inputs[0].ki.wVk = VK_CAPITAL;
    inputs[0].ki.dwExtraInfo = CAPS_LOCK_SENDINPUT_EXTRA_INFO;
    inputs[1] = inputs[0];
    inputs[1].ki.dwFlags = KEYEVENTF_KEYUP;
    return SendInput(ARRAYSIZE(inputs), inputs, sizeof(INPUT)) == ARRAYSIZE(inputs);
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
            // 普通按下即使没有交给 TSF，也必须同时取消两路轻点判定。
            owner->_shiftHotkeyArmed = false;
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
    // 按着 Caps Lock 时按了别的键，是在用大写锁定打字，不再算轻点。Caps Lock 自己的自动重复不算。
    if (code != VK_CAPITAL)
    {
        _capsLockTap.interrupt();
    }
    const bool isShift = IsShiftVk(code);
    const bool isCtrl = IsControlVk(code);
    const bool isOtherModifier = IsAltVk(code) || IsWinVk(code);

    // TSF 直接处理的键可能不到线程钩子，两条路径必须共同取消单按判定。
    if (!isShift)
    {
        _bareShiftArmed = false;
    }

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
    // 钩子已经取消的这一轮不能被重复通知重新布防；先按着别的普通键也属于组合键。
    if (isShift && IsOnlyModifierPhysicallyDown(VK_SHIFT) && !IsOtherKeyboardKeyDown() &&
        (_bareShiftHook == nullptr || _bareShiftDownMask == 0 || _bareShiftArmed))
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
    // Alt+Shift+H 也切换全半角（共享偏好可关），只在中文模式下认；先比按键再读开关和中英文状态。
    if (code == 'H' && shift && alt && !ctrl)
    {
        BOOL isOpen = FALSE;
        CCompartment CompartmentKeyboardOpen(_pThreadMgr, _tfClientId, GUID_COMPARTMENT_KEYBOARD_OPENCLOSE);
        CompartmentKeyboardOpen._GetCompartmentBOOL(isOpen);
        const bool win = (GetKeyState(VK_LWIN) & 0x8000) != 0 || (GetKeyState(VK_RWIN) & 0x8000) != 0;
        if (Global::IsFullwidthAltShiftH(code, shift, ctrl, alt, win, isOpen != FALSE,
                                         FanyUtils::ReadConfiguredSwitchLanguageHotkeys().fullwidth_alt_shift_h))
        {
            *hotkeyGuid = Global::MetasequoiaIMEGuidDoubleSingleBytePreserveKey;
            return true;
        }
        return false;
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

    // 实体仍按住时不能靠宿主的松开回调切换，保留布防等待真实松开。
    if (IsShiftVk(code) && (GetAsyncKeyState(VK_SHIFT) & 0x8000) != 0)
    {
        return false;
    }

    // 布防记录证明这次单按过；不依赖宿主可能陈旧的 GetKeyState。
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

void CMetasequoiaIME::_ArmCapsLockTap(bool lockWasOn)
{
    const bool enabled =
        FanyUtils::ReadConfiguredSwitchLanguageHotkeys().caps_lock && !_IsKeyboardDisabled() && !CapsLockOffNeedsShift();
    _capsLockTap.press(static_cast<std::int64_t>(GetTickCount64()), enabled, lockWasOn, IsKeyOtherThanCapsLockDown());
}

bool CMetasequoiaIME::_QueueCapsLockTap(_In_ ITfContext *pContext, WPARAM wParam)
{
    if (pContext == nullptr || LOWORD(wParam) != VK_CAPITAL ||
        !_capsLockTap.release(static_cast<std::int64_t>(GetTickCount64())))
    {
        return false;
    }
    // 撤不回大写锁定就不切换：切到英文却留着大写锁定，比什么都不做更糟。
    if (!ReleaseCapsLockAfterTap())
    {
        return false;
    }
    // 撤回的那一下带自注入标记，OnTestKeyDown 直接放行、不会再记大写锁定状态，所以在这里记成关。Server 的低级钩子也计入那一下，随后推来的状态同样是关。
    Global::CapsLockEnabled.store(false, std::memory_order_relaxed);
    _RequestLanguageBarCapsIconRefresh();
    // 和单按 Shift 走同一个切换：组字中先上屏原文再切换。
    BOOL queued = FALSE;
    (void)_QueueInputHotkey(pContext, Global::MetasequoiaIMEGuidImeModePreserveKey, &queued);
    return true;
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
        // 方案由 Server 写进偏好，host-api 等这次组字结束、运行时空闲后才换方案，所以组字中按下不动组字，和托盘换方案一样。
        if (!_serverUnavailableFallbackActive && IsJapaneseInputModeToggle(*pCodeOut, shortcutModifiers))
        {
            if (pKeyState)
            {
                pKeyState->Category = CATEGORY_COMPOSING;
                pKeyState->Function = FUNCTION_TOGGLE_JAPANESE;
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
        // Alt/Ctrl+数字取释义列只在候选列表打开时接（和 Ctrl+Enter 一样看 _candidateMode != CANDIDATE_NONE：普通组字的候选列表是 CANDIDATE_INCREMENTAL，只认 CANDIDATE_ORIGINAL 会让这些组合键在普通组字里永远交给应用），其余时候这些组合键照旧属于应用；发出时带 CandidateActive，见 Global::CandidateKeyReportsActiveList。和 Ctrl+Enter 一样不接 TIP 自己宿主会话组字的方案；宿主自己画候选的 UILess 场合（游戏、全屏）Server 不上屏释义，Server 连不上时也没有释义可取，这两种情况下组合键都留给应用。
        if (!hostComposed && !freshCompositionState && _candidateMode != CANDIDATE_NONE &&
            !_serverUnavailableFallbackActive && !Global::IsUiLessMode() &&
            IsGlossColumnShortcut(*pCodeOut, shortcutModifiers))
        {
            if (pKeyState)
            {
                pKeyState->Category = CATEGORY_CANDIDATE;
                pKeyState->Function = FUNCTION_SERVER_CANDIDATE_KEY;
            }
            return TRUE;
        }

        // 组字时的分段编辑键（Ctrl+退格、Ctrl+左右、Shift+左右）排在其他组合键前面，见 SegmentEditFunction。
        const KEYSTROKE_FUNCTION segmentEdit = SegmentEditFunction(*pCodeOut, shortcutModifiers, scheme);
        if (segmentEdit != FUNCTION_NONE && _IsComposing())
        {
            if (pKeyState)
            {
                pKeyState->Category = CATEGORY_COMPOSING;
                pKeyState->Function = segmentEdit;
            }
            return TRUE;
        }
        // Other Ctrl/Alt/Windows combinations belong to the application.
        // IME-owned shortcuts are handled before this normal key classifier.
        if ((shortcutModifiers & 0b00000110u) != 0 || (GetAsyncKeyState(VK_LWIN) & 0x8000) != 0 ||
            (GetAsyncKeyState(VK_RWIN) & 0x8000) != 0)
        {
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
        _deferredKeyProjectionValid || !_deferredKeyDowns.empty() || _hasDeferredKeyInFlight ||
        _HasPendingApplicationKeyReplay() || _wubiCommitOwner.active() || _preferenceBarrier.active() ||
        _preferenceNotificationPending.load(std::memory_order_acquire))
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
    // 首键从当前组字快照开始投影；宿主点选补全可整段替换预编辑，不能从旧键击缓冲重建。
    const auto composition = _pCompositionProcessorEngine ? _pCompositionProcessorEngine->GetKeyCompositionState()
                                                         : msime::tsf::KeyCompositionState{};
    _deferredProjectedRawInput = composition.rawInput;
    _deferredProjectedInputLength = min(static_cast<size_t>(MAX_PINYIN_LENGTH), composition.rawInput.size());
    _deferredProjectedCaret = composition.caret;
    // 增量候选继续扩展原文；只有显式候选列表才让下一个输入先结束旧组字。
    _deferredProjectedCandidateActive = _candidateMode == CANDIDATE_ORIGINAL;
    _deferredProjectedUnicodeMode = composition.unicodeMode;
    _deferredProjectedUrlMode = composition.urlMode;
    _deferredProjectedPhraseActive = composition.phraseActive;
    _deferredProjectedKoreanHanjaListOpen =
        msime::windows::scheme::OpensCandidateList(Global::InputModeScheme.load(std::memory_order_relaxed)) &&
        _IsKoreanHanjaListOpen();
}

void CMetasequoiaIME::_ApplyDeferredKeyProjection(const _KEYSTROKE_STATE &keyState, WCHAR wch, UINT code)
{
    // 选择可能只补全网址或钉住一段；结果出来前不能固定后续键的归属。
    switch (keyState.Function)
    {
    case FUNCTION_SELECT_BY_NUMBER:
    case FUNCTION_CONVERT:
    case FUNCTION_FINALIZE_CANDIDATELIST:
    case FUNCTION_FINALIZE_CANDIDATELISTForVKReturn:
    case FUNCTION_KOREAN_HANJA_KEY:
        _deferredKeyClassificationPending = true;
        break;
    default:
        break;
    }
    _EnsureDeferredKeyProjection();
    if (keyState.Function == FUNCTION_BACKSPACE_SEGMENT ||
        (keyState.Function == FUNCTION_BACKSPACE && _deferredProjectedPhraseActive))
        _deferredKeyClassificationPending = true;
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
    shadow.phraseActive = _deferredProjectedPhraseActive;
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
        _deferredProjectedPhraseActive = false;
        _deferredProjectedKoreanHanjaListOpen = false;
        return;
    }
    _deferredProjectedInputLength = shadow.inputLength;
    _deferredProjectedRawInput = std::move(shadow.rawInput);
    _deferredProjectedCaret = shadow.caret;
    _deferredProjectedCandidateActive = shadow.candidateActive;
    _deferredProjectedUnicodeMode = shadow.unicodeMode;
    _deferredProjectedUrlMode = shadow.urlMode;
    _deferredProjectedPhraseActive = shadow.phraseActive;
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
        _deferredProjectedPhraseActive = false;
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

    // 多义来源不能从 rawInput 还原，断线取消它，不把组码当确定字母恢复。
    if (_HasGroupedPanelComposition()) return true;

    // 与 _RequestLocalSessionReset 一致：这些方案的现有组字会在 reset 时上屏，不能再入队重放。
    if (msime::windows::scheme::CommitsOnBlur(Global::InputModeScheme.load(std::memory_order_relaxed)))
    {
        return true;
    }

    // 宿主拥有组字时旧键击缓冲可能为空；恢复检查点与按键分类读取同一份当前快照。
    const auto composition = _pCompositionProcessorEngine->GetKeyCompositionState();
    const auto &raw = composition.rawInput;
    const size_t rawLength = raw.size();
    const size_t caret = min(rawLength, composition.caret);
    const size_t moveLeftCount = rawLength - caret;
    // 这里把当前整串拼音摊回按键，重连之后它们会连同失败的那个键一起重放：用户在屏幕上看到的
    // 就是「整段输入被重来一次」(#9518)。只记长度与光标，不记内容。
    if (Global::TsfDiagnosticLogEnabled.load(std::memory_order_relaxed))
    {
        QueueTsfDiagnosticLog(L"[msime][deferred-recovery] stage=prefix raw_len=" + std::to_wstring(rawLength) +
                              L" caret=" + std::to_wstring(caret) + L" move_left=" +
                              std::to_wstring(moveLeftCount));
    }
    if (rawLength + moveLeftCount > MAX_DEFERRED_KEY_DOWN_COUNT)
    {
        return false;
    }

    for (size_t index = 0; index < rawLength; ++index)
    {
        const WCHAR wch = raw[index];
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

    if (_HasGroupedPanelComposition())
    {
        _ClearDeferredKeyDowns();
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
                                               _Out_ _KEYSTROKE_STATE *keyState, _In_opt_ const DeferredKeyDown *replayedKey)
{
    if (pContext == nullptr || classifiedWch == nullptr || classifiedCode == nullptr || keyState == nullptr ||
        _pCompositionProcessorEngine == nullptr || _pThreadMgr == nullptr)
    {
        return false;
    }

    const bool useProjection = replayedKey == nullptr && _deferredKeyProjectionValid;
    const bool capsLockOn = replayedKey ? replayedKey->capsLockOn : (GetKeyState(VK_CAPITAL) & 1) != 0;
    const bool windowsKeyDown = replayedKey ? replayedKey->windowsKeyDown :
        (GetAsyncKeyState(VK_LWIN) & 0x8000) != 0 || (GetAsyncKeyState(VK_RWIN) & 0x8000) != 0;
    *classifiedWch = translatedWch ? *translatedWch : ConvertVKey(static_cast<UINT>(wParam));
    *classifiedCode = VKeyFromVKPacketAndWchar(static_cast<UINT>(wParam), *classifiedWch);
    keyState->Category = CATEGORY_NONE;
    keyState->Function = FUNCTION_NONE;

    // AltGr 的 Ctrl+Alt 组合在产生字符时按输入处理（例如 AZERTY 的 AltGr+0 产生 @）。
    const UINT capturedModifiers =
        Global::CharacterModifiers(modifiersDown ? *modifiersDown : CaptureIpcModifiers(), *classifiedWch,
                                   *classifiedCode);
    if (!replayedKey && (_deferredKeyClassificationPending || _HasPendingApplicationKeyReplay() || _preferenceBarrier.active() ||
         _preferenceNotificationPending.load(std::memory_order_acquire)) && (capturedModifiers & 0b110u) == 0 &&
        !windowsKeyDown && !IsBareModifierKey(*classifiedCode) && !_IsKeyboardDisabled())
    {
        const bool printable = *classifiedWch != L'\0' && std::iswprint(static_cast<wint_t>(*classifiedWch)) != 0;
        const bool plainEditingKey = (capturedModifiers & 1u) == 0 &&
            (msime::tsf::is_korean_caret_or_edit_key(*classifiedCode) ||
             *classifiedCode == VK_BACK || *classifiedCode == VK_ESCAPE);
        if (printable || plainEditingKey) return true;
    }
    const bool projectedImeOpen = useProjection
                                      ? _deferredProjectedImeOpen
                                      : _pCompositionProcessorEngine->GetIMEMode(_pThreadMgr, _tfClientId) != FALSE;
    if (projectedImeOpen && !_serverUnavailableFallbackActive && !_IsKeyboardDisabled() &&
        IsCharacterSetInputModeToggle(*classifiedCode, capturedModifiers))
    {
        keyState->Category = CATEGORY_COMPOSING;
        keyState->Function = FUNCTION_TOGGLE_CHARACTER_SET;
        return true;
    }
    if (projectedImeOpen && !_serverUnavailableFallbackActive && !_IsKeyboardDisabled() &&
        IsJapaneseInputModeToggle(*classifiedCode, capturedModifiers))
    {
        keyState->Category = CATEGORY_COMPOSING;
        keyState->Function = FUNCTION_TOGGLE_JAPANESE;
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

    // 候选列表开着：和 _IsKeyEaten 一样不只看 CANDIDATE_ORIGINAL，普通组字的增量候选也算。排队按键看投影里还有没有组字，首个投影从当前拥有组字的会话读取。
    const bool projectedCandidateListOpen =
        useProjection ? (_deferredProjectedInputLength > 0 || _deferredProjectedCandidateActive)
                                    : (_candidateMode != CANDIDATE_NONE);
    // 释义列快捷键，条件和 _IsKeyEaten 里的相同。
    if (projectedImeOpen && !_IsKeyboardDisabled() && projectedCandidateListOpen &&
        !_serverUnavailableFallbackActive && !Global::IsUiLessMode() &&
        !msime::windows::scheme::AlwaysInlinePreedit(scheme) &&
        IsGlossColumnShortcut(*classifiedCode, capturedModifiers))
    {
        keyState->Category = CATEGORY_CANDIDATE;
        keyState->Function = FUNCTION_SERVER_CANDIDATE_KEY;
        return true;
    }
    if (projectedImeOpen && !_IsKeyboardDisabled() && projectedCandidateListOpen &&
        IsTranslationCommitShortcut(*classifiedCode, capturedModifiers))
    {
        keyState->Category = CATEGORY_CANDIDATE;
        keyState->Function = FUNCTION_SERVER_CANDIDATE_KEY;
        return true;
    }

    // 只有段落编辑快捷键依赖组字状态，其余修饰键组合交给应用。
    const bool projectedCompositionActive = useProjection
                                                ? (_deferredProjectedInputLength > 0 ||
                                                   _deferredProjectedCandidateActive || _deferredProjectedPhraseActive)
                                                : (_IsComposing() || !_pCompositionProcessorEngine->GetKeyCompositionState().rawInput.empty() ||
                                                   _candidateMode != CANDIDATE_NONE);
    const KEYSTROKE_FUNCTION segmentEdit = SegmentEditFunction(*classifiedCode, capturedModifiers, scheme);
    if (segmentEdit != FUNCTION_NONE && !_IsKeyboardDisabled() && projectedCompositionActive)
    {
        keyState->Category = CATEGORY_COMPOSING;
        keyState->Function = segmentEdit;
        return true;
    }
    if ((capturedModifiers & 0b00000110) != 0 || windowsKeyDown || IsBareModifierKey(*classifiedCode) || _IsKeyboardDisabled())
    {
        return false;
    }

    DeferredShadowState shadow;
    if (useProjection)
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
        shadow.phraseActive = _deferredProjectedPhraseActive;
        shadow.koreanHanjaListOpen = _deferredProjectedKoreanHanjaListOpen;
    }
    else
    {
        shadow.imeOpen = _pCompositionProcessorEngine->GetIMEMode(_pThreadMgr, _tfClientId) != FALSE;
        shadow.punctuationOpen = _pCompositionProcessorEngine->GetPunctuationMode(_pThreadMgr, _tfClientId) != FALSE;
        shadow.doubleSingleByteOpen =
            _pCompositionProcessorEngine->GetDoubleSingleByteMode(_pThreadMgr, _tfClientId) != FALSE;
        const auto composition = _pCompositionProcessorEngine->GetKeyCompositionState();
        shadow.rawInput = composition.rawInput;
        shadow.inputLength = min(static_cast<size_t>(MAX_PINYIN_LENGTH), composition.rawInput.size());
        shadow.caret = composition.caret;
        shadow.candidateActive = _candidateMode == CANDIDATE_ORIGINAL;
        shadow.unicodeMode = composition.unicodeMode;
        shadow.urlMode = composition.urlMode;
        shadow.phraseActive = composition.phraseActive;
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

    if (shadow.imeOpen && msime::windows::wubi_candidate_character(
            Global::WubiCandidateKeys.load(std::memory_order_relaxed),
            static_cast<wchar_t>(Global::InputModeIndicator.load(std::memory_order_relaxed)),
            Global::DedicatedEnglish.active(GetTickCount64()), *classifiedWch))
    {
        return setKeyState(CATEGORY_COMPOSING, FUNCTION_INPUT);
    }

    // 与 _IsKeyEaten（CCompositionProcessorEngine::IsSecondThirdCandidateKey）相同：打开「二三候选」后，组字中的 ';' 和 '\'' 是数字选词，排在音节分隔符和标点前面；网址模式和 V 模式拼写的符号、微软双拼的韵母 ing 仍是输入。Ctrl 和 Alt 组合键在上面已经交给应用。组字和光标前的字母来自队列投影，选择结果未定后的重分类则读取宿主当前快照，与下面的数字键相同。
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
        if (shadow.inputLength == 0 && capsLockOn && *classifiedWch >= L'A' &&
            *classifiedWch <= L'Z' && *classifiedCode >= L'A' && *classifiedCode <= L'Z')
        {
            // 与普通首键路径一致：Caps Lock 打出的开头大写字母交给应用。
            isInputKey = false;
        }
        // Match the normal path: "/" and "@" open their modes on an empty composition.
        if (!isInputKey && shadow.inputLength == 0 && !shadow.candidateActive && !shadow.phraseActive &&
            Global::OpensLocalMode(*classifiedWch, false,
                                   shadow.punctuationOpen && Global::PunctuationLockMode.load(std::memory_order_relaxed) !=
                                                                 Global::PunctuationLock::AlwaysEnglish,
                                   Global::CommandModeEnabled.load(std::memory_order_relaxed),
                                   Global::MentionModeEnabled.load(std::memory_order_relaxed)))
        {
            isInputKey = true;
        }
    }

    // 仅剩已选词段时，斜杠结束它并原样上屏，不能在词段后重新打开命令模式。
    if (shadow.imeOpen && shadow.phraseActive && *classifiedWch == L'/')
        return setKeyState(CATEGORY_COMPOSING, FUNCTION_PUNCTUATION);

    if (shadow.candidateActive && isInputKey)
    {
        return setKeyState(CATEGORY_CANDIDATE, FUNCTION_FINALIZE_CANDIDATELIST_AND_INPUT);
    }
    if (!shadow.candidateActive && isInputKey)
    {
        return setKeyState(CATEGORY_COMPOSING, FUNCTION_INPUT);
    }

    if (shadow.imeOpen && (shadow.inputLength > 0 || shadow.phraseActive))
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
    // 设置应用面板注入的文字由它自己按来源计数（PassthroughStatistics.h）。
    if (IsPanelTextSendInput(static_cast<std::uintptr_t>(GetMessageExtraInfo())))
    {
        return;
    }
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
// _NotePassthroughKeySound
//
// 交给应用的一次按下也要出按键音、计入打字特效的连击，和 macOS 每个按键都出声一样：没有组字时的空格、回车、退格、数字和方向键都不经过 Server 的按键路径，只能在这里报。只是观察：按键吃不吃、延迟队列和编辑路径都不受影响，入队之后的管道读写在线程池上，不占按键路径的时间。哪些键出声由 passthrough_key_sound_class 决定。
//----------------------------------------------------------------------------

void CMetasequoiaIME::_NotePassthroughKeySound(WPARAM wParam, LPARAM lParam, WCHAR wch)
{
    // 按键音和打字特效都关着时 Server 不回 "OK"，队列停发；这时连扫描码都不必记。
    if (PassthroughKeySoundSuppressed())
    {
        return;
    }
    // 同一次按下的 Test 探测可能来不止一次，扫描码和消息时间都相同；真正的第二次按下中间隔着一次抬起，消息时间不同。
    const UINT physicalKey = KeyPressPhysicalKey(static_cast<std::uintptr_t>(lParam));
    const LONG messageTime = GetMessageTime();
    if (physicalKey == _passthroughSoundKey && messageTime == _passthroughSoundMessageTime)
    {
        return;
    }
    _passthroughSoundKey = physicalKey;
    _passthroughSoundMessageTime = messageTime;
    const uint64_t focusToken = GetNamedpipeFocusToken();
    if (focusToken == 0)
    {
        return;
    }
    msime::windows::PassthroughKeyState key;
    key.virtual_key = static_cast<uint32_t>(wParam);
    key.auto_repeat = IsAutoRepeat(lParam);
    const UINT modifiers = CaptureIpcModifiers();
    // AltGr 读作 Ctrl+Alt：它按出字符时是打字，照样出声、计入连击，和 macOS 的 Option 一样。这个键已经交给应用，不会选候选，所以不用 Global::CharacterModifiers 给数字键留的例外。
    const bool altGrCharacter = Global::IsAltGrCharacter(modifiers, wch);
    key.ctrl = !altGrCharacter && (modifiers & 0b00000010u) != 0;
    key.alt = !altGrCharacter && (modifiers & 0b00000100u) != 0;
    key.win = (GetAsyncKeyState(VK_LWIN) & 0x8000) != 0 || (GetAsyncKeyState(VK_RWIN) & 0x8000) != 0;
    key.injected = IsPanelTextSendInput(static_cast<std::uintptr_t>(GetMessageExtraInfo()));
    // 先用不需要查询 TSF 的部分判断，修饰键、快捷键和自动重复不必再读隔间。
    key.keyboard_open = true;
    if (!msime::windows::passthrough_key_sound_class(key))
    {
        return;
    }
    BOOL isOpen = FALSE;
    CCompartment CompartmentKeyboardOpen(_pThreadMgr, _tfClientId, GUID_COMPARTMENT_KEYBOARD_OPENCLOSE);
    CompartmentKeyboardOpen._GetCompartmentBOOL(isOpen);
    key.keyboard_open = isOpen != FALSE;
    key.keyboard_disabled = _IsKeyboardDisabled() != FALSE;
    key.secure = _IsSecureMode() != FALSE;
    if (const auto keyClass = msime::windows::passthrough_key_sound_class(key))
    {
        QueuePassthroughKeySound(GetNamedpipeClientId(), focusToken, *keyClass);
    }
}

namespace
{
// 客户端进程是否以管理员身份运行。进程的令牌在它的一生里不会变，查一次就够；查不到时当作提权（FanyImeFieldTraits::ProcessElevated）。这个 DLL 用 /Zc:threadSafeInit- 编译，函数内的静态变量没有线程安全的初始化，所以缓存放在常量初始化的原子量里（0 没查过，1 没有提权，2 提权）：几个线程同时第一次查，各自算出同一个值，不会有数据竞争。
bool CurrentProcessIsElevated()
{
    static std::atomic<int> cached{0};
    int value = cached.load(std::memory_order_relaxed);
    if (value == 0)
    {
        bool queried = false;
        bool tokenElevated = false;
        HANDLE token = nullptr;
        if (OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &token))
        {
            TOKEN_ELEVATION elevation = {};
            DWORD size = 0;
            queried = GetTokenInformation(token, TokenElevation, &elevation, sizeof(elevation), &size) != FALSE;
            tokenElevated = queried && elevation.TokenIsElevated != 0;
            CloseHandle(token);
        }
        value = FanyImeFieldTraits::ProcessElevated(queried, tokenElevated) ? 2 : 1;
        cached.store(value, std::memory_order_relaxed);
    }
    return value == 2;
}
} // namespace

//+---------------------------------------------------------------------------
//
// _NoteFieldTraitsForServer
//
// 密码框以「键盘停用或空上下文」的形式到达 TIP，和按键统计、交给应用的按键音用的是同一个判据；安全桌面（登录、UAC）同样按密码框算。Server 据此让生命周期核心在这里不出声、不画、停掉背景音乐。
//----------------------------------------------------------------------------

void CMetasequoiaIME::_NoteFieldTraitsForServer()
{
    NoteFieldTraitsForServer(FanyImeFieldTraits::Observe(_IsKeyboardDisabled() != FALSE, _IsSecureMode() != FALSE,
                                                         CurrentProcessIsElevated()));
}

void CMetasequoiaIME::_ApplyCapsLockEdge(bool enabled)
{
    Global::CapsLockEnabled.store(enabled, std::memory_order_relaxed);
    _RequestLanguageBarCapsIconRefresh();
    _ArmCapsLockTap(!enabled);
}

// IMM 宿主可能只回调 TestKeyDown，而且此时切换已经生效；等按键消息处理完再读真实值。
void CMetasequoiaIME::_QueueCapsLockEdge()
{
    if (_msgWndHandle && PostMessage(_msgWndHandle, WM_ApplyCapsLockEdge, 0, 0))
        return;
    _ApplyCapsLockEdge((GetKeyState(VK_CAPITAL) & 0x0001) == 0);
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
    const ULONG_PTR replayMarker = static_cast<ULONG_PTR>(GetMessageExtraInfo());
    if (IsApplicationKeyReplayExtraInfo(replayMarker))
    {
        *pIsEaten = _ObserveApplicationKeyReplay(pContext, static_cast<UINT>(wParam), replayMarker) ? FALSE : TRUE;
        return S_OK;
    }
    if (IsSelfGeneratedSendInputExtraInfo(static_cast<ULONG_PTR>(GetMessageExtraInfo())))
    {
        *pIsEaten = FALSE;
        return S_OK;
    }
    // Ahead of the backspace-hold, Shift and hotkey returns below, each of which would otherwise hide a real press.
    _NoteKeyPressStatistics(wParam, lParam);
    // 只有第一次按下切换状态，长按产生的重复消息不重新武装轻点。
    if (wParam == VK_CAPITAL && !IsAutoRepeat(lParam))
    {
        _QueueCapsLockEdge();
    }
    PerfTimer onTestKeyDownTimer;
    Global::UpdateModifiers(wParam, lParam);
    _TrackModifierHotkeyArming(wParam, lParam, false);
    // 上屏后联想显示着：Esc 只关掉它（真正关在 OnKeyDown），别的键先关掉它再照常处理。交给应用的键不一定还会到 OnKeyDown，所以在这里关。
    switch (msime::tsf::PredictionKey(_IsPredictionVisible(), LOWORD(wParam), CaptureIpcModifiers()))
    {
    case msime::tsf::PredictionKeyAction::Close:
        *pIsEaten = TRUE;
        return S_OK;
    case msime::tsf::PredictionKeyAction::Dismiss:
        _DismissPrediction();
        break;
    case msime::tsf::PredictionKeyAction::None:
        break;
    }
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
            _NotePassthroughKeySound(wParam, lParam, deferredWch);
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
            _NotePassthroughKeySound(wParam, lParam, deferredWch);
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
        _NotePassthroughKeySound(wParam, lParam, wch);
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
                                            WCHAR translatedWch, UINT modifiersDown, const _KEYSTROKE_STATE &keyState,
                                            const msime::panel_key::Owner *groupedOwner)
{
    // Repeats are owned but do not enqueue another global configuration toggle.
    if ((keyState.Function == FUNCTION_TOGGLE_CHARACTER_SET || keyState.Function == FUNCTION_TOGGLE_JAPANESE) &&
        (lParam & 0x40000000) != 0)
        return true;
    if (pContext == nullptr || !_DeferredKeyQueueHasCapacity() ||
        (keyState.groupedPanelKey && (!groupedOwner || !groupedOwner->valid())))
    {
        return false;
    }

    pContext->AddRef();
    DeferredKeyDown key;
    if (groupedOwner) key.groupedOwner = *groupedOwner;
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
    key.needsClassification = (_deferredKeyClassificationPending || _HasPendingApplicationKeyReplay() || _preferenceBarrier.active() ||
         _preferenceNotificationPending.load(std::memory_order_acquire)) &&
        keyState.Category == CATEGORY_NONE && keyState.Function == FUNCTION_NONE;
    key.capsLockOn = (GetKeyState(VK_CAPITAL) & 1) != 0;
    key.windowsKeyDown = (GetAsyncKeyState(VK_LWIN) & 0x8000) != 0 || (GetAsyncKeyState(VK_RWIN) & 0x8000) != 0;
    key.foregroundWindow = GetForegroundWindow();
    if (key.needsClassification) _deferredKeyClassificationPending = true;
    _deferredKeyDowns.push_back(key);
    if (key.kind == DeferredKeyDown::Kind::KeyDown && !key.needsClassification)
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
    _preferenceBarrier.reset();
    _preferenceTransitionStarted = 0;
    if (_msgWndHandle) KillTimer(_msgWndHandle, TIMER_PREFERENCE_TRANSITION);
    {
        std::lock_guard<std::mutex> lock(_pendingCommitCandidateMutex);
        _preferenceTransitions.clear();
        _preferenceNotificationPending.store(false, std::memory_order_release);
    }
    _groupedPanelLeases.clear();
    _wubiCommitOwner.clear();
    _ClearApplicationKeyReplay();
    _deferredKeyClassificationPending = false;
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
        // 整队作废的时候把当时的传输状态一起记下来：重置是否还挂着、前缀有多长、代次走到哪。
        if (Global::TsfDiagnosticLogEnabled.load(std::memory_order_relaxed))
        {
            QueueTsfDiagnosticLog(L"[msime][deferred-clear] stage=queue-cleared queued=" +
                                  std::to_wstring(queuedCount) + L" in_flight=" + BoolFlag(hadInFlight) +
                                  L" applied_prefix=" + std::to_wstring(_deferredAppliedPrefix.size()) +
                                  L" reset_pending=" +
                                  BoolFlag(_localSessionResetPending.load(std::memory_order_acquire)) +
                                  L" composing=" + BoolFlag(_IsComposing()) + L" generation=" +
                                  std::to_wstring(_deferredKeyFocusGeneration) + L" token=" +
                                  std::to_wstring(inFlightToken));
        }
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
    _deferredProjectedPhraseActive = false;
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

    if (_wubiCommitOwner.current(replayToken)) return;

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
        _deferredKeyClassificationPending = false;
        _deferredProjectedInputLength = 0;
        _deferredProjectedRawInput.clear();
        _deferredProjectedCaret = 0;
        _deferredProjectedCandidateActive = false;
        _deferredProjectedUnicodeMode = false;
        _deferredProjectedUrlMode = false;
        _deferredProjectedPhraseActive = false;
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
            // 选择可能补全网址或只钉住一段，仍有组字时不能丢掉恢复前缀；完整上屏后快照为空，会自然清掉旧检查点。
            (void)_RefreshDeferredRecoveryPrefix(context);
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
    if (_wubiCommitOwner.current(replayToken))
    {
        if (_wubiCommitOwner.applied())
        {
            _FinishWubiWorkerEdit(replayToken, false);
            return;
        }
        _wubiCommitOwner.clear();
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

    if (_deferredKeyInFlight.keyState.groupedPanelKey || _HasGroupedPanelComposition())
    {
        // 已发送的组码可能已到 Server，清空本批并用新焦点代次取消双方，不能重放首字母。
        _ClearDeferredKeyDowns();
        MarkNamedpipeSessionDirtyForOwner(this);
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

void CMetasequoiaIME::_ScheduleDeferredKeyDownDrain(bool allowSyncFallback)
{
    if (!_deferredKeyDowns.empty() && !_hasDeferredKeyInFlight && !_wubiCommitOwner.active() && !_HasPendingApplicationKeyReplay() &&
        !_deferredKeyDrainPosted && _msgWndHandle &&
        IsWindow(_msgWndHandle))
    {
        _deferredKeyDrainPosted = true;
        if (!PostMessage(_msgWndHandle, WM_DrainDeferredKeyDown, 0, 0))
        {
            // key sink 确认回放时，必须先返回应用处理当前键；投递失败由已有 UI 定时器重试。
            _deferredKeyDrainPosted = false;
            if (allowSyncFallback) SendMessage(_msgWndHandle, WM_DrainDeferredKeyDown, 0, 0);
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

bool CMetasequoiaIME::_ResolveDeferredKeyClassification(uint64_t replayToken)
{
    if (!_hasDeferredKeyInFlight || _deferredKeyReplayToken != replayToken) return false;
    if (!_deferredKeyInFlight.needsClassification) return true;
    // 焦点查询与 compartment 读取会进入 COM；保留本地副本，重入清队列后不再写回旧槽。
    const DeferredKeyDown captured = _deferredKeyInFlight;
    ITfContext *context = captured.context;
    context->AddRef();
    const auto current = [this, &captured, context, replayToken]() {
        return _IsDeferredKeyReplayCurrent(replayToken, captured.focusGeneration, context);
    };
    const bool initiallyEnabled = _pCompositionProcessorEngine && _pThreadMgr && !_IsKeyboardDisabled();
    bool ownerCurrent = _IsDeferredKeyOwnerCurrent(context, captured.focusGeneration, captured.foregroundWindow, true);
    if (!current()) { context->Release(); return false; }
    if (!ownerCurrent || !initiallyEnabled)
    {
        context->Release();
        if (current()) _ClearDeferredKeyDowns();
        return false;
    }
    WCHAR wch = captured.translatedWch;
    UINT code = 0;
    _KEYSTROKE_STATE state = {};
    (void)_ClassifyDeferredKeyDown(context, captured.wParam, captured.lParam, &captured.translatedWch,
                                  &captured.modifiersDown, &wch, &code, &state, &captured);
    const bool enabled = _pCompositionProcessorEngine && _pThreadMgr && !_IsKeyboardDisabled();
    const uint64_t focusToken = _CaptureFocusSessionToken();
    const bool ready = _serverUnavailableFallbackActive || _IsFocusSessionCurrent(focusToken, context);
    ownerCurrent = _IsDeferredKeyOwnerCurrent(context, captured.focusGeneration, captured.foregroundWindow, true);
    context->Release();
    if (!current()) return false;
    if (!ownerCurrent || !enabled || GetForegroundWindow() != captured.foregroundWindow)
    {
        // 真正失去焦点或禁用键盘时才作废；同焦点传输重置由下方保留尚未发送的键。
        if (Global::TsfDiagnosticLogEnabled.load(std::memory_order_relaxed))
        {
            QueueTsfDiagnosticLog(L"[msime][deferred-clear] stage=classification-aborted owner_current=" +
                                  BoolFlag(ownerCurrent) + L" enabled=" + BoolFlag(enabled) +
                                  L" reset_pending=" +
                                  BoolFlag(_localSessionResetPending.load(std::memory_order_acquire)) +
                                  L" foreground_changed=" +
                                  BoolFlag(GetForegroundWindow() != captured.foregroundWindow) +
                                  L" composing=" + BoolFlag(_IsComposing()) + L" queued=" +
                                  std::to_wstring(_deferredKeyDowns.size()));
        }
        _ClearDeferredKeyDowns();
        return false;
    }
    if (_localSessionResetPending.load(std::memory_order_acquire))
    {
        // COM 分类期间可重入本地重置；此键尚未发送，不消耗发送重试次数，也不再标脏传输。
        // 先交回槽的引用所有权，再让现有恢复路径将检查点放到尾键之前；重置完成负责唤醒。
        --_deferredKeyInFlight.replayAttempts;
        _deferredKeyDowns.push_front(_deferredKeyInFlight);
        _deferredKeyInFlight = {};
        _hasDeferredKeyInFlight = false;
        _deferredKeyReplayToken = 0;
        _ArmDeferredRecoveryForTransport(context);
        return false;
    }
    if (!ready || (!_serverUnavailableFallbackActive && !_IsFocusSessionCurrent(focusToken)))
    {
        _RetryDeferredKeyReplay(replayToken);
        return false;
    }
    DeferredKeyDown::Kind kind = DeferredKeyDown::Kind::KeyDown;
    if (state.Category == CATEGORY_NONE && state.Function == FUNCTION_NONE)
    {
        // 编辑键保留系统语义；可打印字符沿用原有 TSF 直接写字路径。
        if (msime::tsf::is_korean_caret_or_edit_key(code) || code == VK_BACK || code == VK_ESCAPE)
            kind = DeferredKeyDown::Kind::ApplicationKey;
        else if (wch != L'\0' && std::iswprint(static_cast<wint_t>(wch)) != 0)
            kind = DeferredKeyDown::Kind::ApplicationText;
        else
        {
            _ClearDeferredKeyDowns();
            return false;
        }
    }
    _deferredKeyInFlight.kind = kind;
    _deferredKeyInFlight.translatedWch = wch;
    _deferredKeyInFlight.keyState = state;
    // 重试复用本次决定；恢复检查点的强制 INPUT/MOVE 从不经过此分类。
    _deferredKeyInFlight.needsClassification = false;
    return true;
}

void CMetasequoiaIME::_DrainOneDeferredKeyDown()
{
    _deferredKeyDrainPosted = false;
    if (_preferenceBarrier.active() || _preferenceNotificationPending.load(std::memory_order_acquire) ||
        _hasDeferredKeyInFlight || _wubiCommitOwner.active() || _HasPendingApplicationKeyReplay() || _deferredKeyDowns.empty() ||
        !Global::g_connected)
    {
        return;
    }
    if (_localSessionResetPending.load(std::memory_order_acquire))
    {
        PostOwnerMessageWithSyncFallback(_msgWndHandle, WM_IpcReconnect);
        return;
    }
    // 补发的键之前可能要先发 ClientActivated：先记下字段属性，让它排在激活前面。
    _NoteFieldTraitsForServer();
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

    if (key.keyState.groupedPanelKey && !_IsGroupedPanelReplayCurrent(replayToken))
    {
        _ClearDeferredKeyDowns();
        MarkNamedpipeSessionDirtyForOwner(this);
        return;
    }
    if (!_ResolveDeferredKeyClassification(replayToken)) return;
    BOOL eaten = FALSE;
    if (key.kind == DeferredKeyDown::Kind::ApplicationKey)
    {
        if (_QueueKoreanSyllableKeyReplay(VKeyFromVKPacketAndWchar(static_cast<UINT>(key.wParam), key.translatedWch),
                                         key.context, replayToken))
            _CompleteDeferredKeyReplay(replayToken);
        else
            _RetryDeferredKeyReplay(replayToken);
        return;
    }
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

    if ((key.keyState.groupedPanelKey && !SupportsGroupedPanelKeys()) ||
        (_serverUnavailableFallbackActive && (key.keyState.groupedPanelKey || _HasGroupedPanelComposition())))
    {
        _ClearDeferredKeyDowns();
        MarkNamedpipeSessionDirtyForOwner(this);
        return;
    }
    if (_serverUnavailableFallbackActive)
    {
        _KEYSTROKE_STATE offlineState = key.keyState;
        if (offlineState.Function == FUNCTION_TOGGLE_CHARACTER_SET || offlineState.Function == FUNCTION_TOGGLE_JAPANESE)
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
    const ULONG_PTR replayMarker = static_cast<ULONG_PTR>(GetMessageExtraInfo());
    if (IsApplicationKeyReplayExtraInfo(replayMarker))
    {
        *pIsEaten = _ObserveApplicationKeyReplay(pContext, static_cast<UINT>(wParam), replayMarker) ? FALSE : TRUE;
        return S_OK;
    }
    if (IsSelfGeneratedSendInputExtraInfo(static_cast<ULONG_PTR>(GetMessageExtraInfo())))
    {
        *pIsEaten = FALSE;
        return S_OK;
    }
    // Usually a repeat of the OnTestKeyDown probe and dropped as one; it counts only for a host that calls OnKeyDown without testing first.
    _NoteKeyPressStatistics(wParam, lParam);
    PerfTimer onKeyDownTimer;
    // 和 OnTestKeyDown 同一条规则；有的宿主不先测试就直接交来按键。
    switch (msime::tsf::PredictionKey(_IsPredictionVisible(), LOWORD(wParam), CaptureIpcModifiers()))
    {
    case msime::tsf::PredictionKeyAction::Close:
        _DismissPrediction();
        *pIsEaten = TRUE;
        return S_OK;
    case msime::tsf::PredictionKeyAction::Dismiss:
        _DismissPrediction();
        break;
    case msime::tsf::PredictionKeyAction::None:
        break;
    }
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

    // 空闲退格通常交还应用，不会进入组字处理器；仍通知 Server 撤销刚刚删除的交替引号状态。保持未处理，实际文本删除仍由应用完成。
    if (code == VK_BACK && !*pIsEaten && !_IsComposing() && _pCompositionProcessorEngine != nullptr)
    {
        if (auto *host = _pCompositionProcessorEngine->GetHostEngineAdapter(); host && host->valid())
        {
            std::string raw, error;
            (void)host->command(MSIME_BACKSPACE, &raw, &error);
        }
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
        // 同上：吃下这个键之后换上的旧 Server 不认识它，不能当普通按键送进组字。
        if (KeystrokeState.Function == FUNCTION_TOGGLE_JAPANESE && !SupportsJapaneseToggleShortcut())
        {
            return KeyDownDispatchResult::Complete;
        }

        // 日语转换开始之后的回车上屏 Server 高亮的候选（Global/JapaneseConversionPolicy.h）：按候选键处理并带上 CandidateActive，Server 走选中高亮候选的回车路径，TIP 读它的回复上屏，而不是自己宿主会话里的假名。
        const bool japaneseCandidateEnter =
            code == VK_RETURN && KeystrokeState.Category != CATEGORY_NONE && _IsComposing() &&
            _JapaneseEnterCommitsCandidate(capturedModifiers);
        const bool groupedCandidateEnter = code == VK_RETURN && capturedModifiers == 0 &&
                                           _HasGroupedPanelComposition();
        if (groupedCandidateEnter)
        {
            std::string raw, error;
            msime::tsf::EngineResult current;
            auto *host = _pCompositionProcessorEngine->GetHostEngineAdapter();
            if (!host || !host->view(&raw, &error) ||
                !msime::tsf::EngineSessionAdapter::parse_result(raw, &current, &error))
                return KeyDownDispatchResult::Retry;
            // 空候选的回车不产生主管道事件，也不把组码当原文结束组字。
            if (current.view.candidates.empty()) return KeyDownDispatchResult::Complete;
        }
        if (japaneseCandidateEnter || groupedCandidateEnter)
        {
            KeystrokeState.Category = CATEGORY_CANDIDATE;
            KeystrokeState.Function = FUNCTION_FINALIZE_CANDIDATELIST;
        }
        Global::Keycode = code;
        Global::wch = wch;
        // The modifiers the key was classified with: an AltGr character goes without Ctrl+Alt, or the Server would cancel it as a shortcut.
        Global::ModifiersDown = Global::CharacterModifiers(capturedModifiers, wch, code);

        PerfTimer writeShmTimer;
        // 回车由 TIP 本地提交，沿用拼音载荷向 Server 交付原文观测，让它核对并清理同一段组字。
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
            localCommitObservation = msime::tsf::ReadLocalCommitObservation(
                _pCompositionProcessorEngine->GetHostEngineAdapter(), std::move(localCommitObservation),
                FanyUtils::string_to_wstring);
            if (localCommitObservation.size() >= 128)
            {
                // 不截断成另一段文字；超界时保留 Server 既有的拒绝路径。
                localCommitObservation.clear();
            }
        }
        // CandidateActive：通配转换的候选列表（CANDIDATE_ORIGINAL）里每个键都带；普通组字的增量候选只给释义列、Ctrl+Enter 和释义页认的键带上，其他键靠没有这一位走别的路（裸回车上屏原文、以词定字），见 Global::CandidateKeyReportsActiveList。
        const bool candidateActive =
            _candidateMode == CANDIDATE_ORIGINAL || japaneseCandidateEnter || groupedCandidateEnter ||
            (_candidateMode != CANDIDATE_NONE && _pCandidateListUIPresenter != nullptr &&
             Global::CandidateKeyReportsActiveList(code, Global::ModifiersDown));
        const UINT ipcModifiers =
            Global::ModifiersDown | (candidateActive ? msime::windows::PipeMetadata::CandidateActive : 0u) |
            (IsAutoRepeat(lParam) ? msime::windows::PipeMetadata::AutoRepeat : 0u) |
            (KeystrokeState.groupedPanelKey ? msime::windows::PipeMetadata::GroupedPanelKey : 0u);
        WriteDataToNamedPipe(Global::Keycode, wch, ipcModifiers, nullptr, 0,
                             localCommitObservation,
                             hasLocalCommitObservation && !localCommitObservation.empty()
                                 ? 0b110111
                                 : 0b000111);

        // 每个键之前比较一次字段属性，变了就排在这个键前面报给 Server。
        _NoteFieldTraitsForServer();
        PerfTimer sendKeyEventTimer;
        // 必须在 IPC 和可能重入的 COM 之前注册；worker 可能比本地编辑会话先到。
        if (deferredReplayToken != 0 && KeystrokeState.Function == FUNCTION_INPUT &&
            Global::InputModeScheme.load(std::memory_order_relaxed) == msime::windows::scheme::Wubi &&
            _pCompositionProcessorEngine->GetHostEngineAdapter() &&
            _pCompositionProcessorEngine->GetHostEngineAdapter()->valid())
            _wubiCommitOwner.begin(deferredReplayToken, GetTickCount64());
        const bool orderedToggle = KeystrokeState.Function == FUNCTION_TOGGLE_JAPANESE && SupportsOrderedPreferences();
        if (orderedToggle) {
            _preferenceBarrier.begin(_expectedWorkerFocusToken.load(std::memory_order_acquire));
            _preferenceTransitionStarted = GetTickCount64();
            SetTimer(_msgWndHandle, TIMER_PREFERENCE_TRANSITION, 10, nullptr);
        }
        const KeyEventSendResult sendResult = SendKeyEventToUIProcess(&requestId);
        if (orderedToggle) {
            if (sendResult == KeyEventSendResult::Sent) _preferenceBarrier.sent(requestId);
            else MarkNamedpipeSessionDirtyForOwner(this);
        }
        if (KeystrokeState.Function == FUNCTION_TOGGLE_CHARACTER_SET || KeystrokeState.Function == FUNCTION_TOGGLE_JAPANESE)
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

    // 和韩文汉字键一样排在屏障检查之前：切换排在还没处理完的键后面，顺序不乱。松开本身照常交给应用。
    if (_QueueCapsLockTap(pContext, wParam))
    {
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

    // 给不调 OnTestKeyUp 的宿主；OnTestKeyUp 已经认定过的话，判定已经清掉，这里不会再切换一次。
    if (_QueueCapsLockTap(pContext, wParam))
    {
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
