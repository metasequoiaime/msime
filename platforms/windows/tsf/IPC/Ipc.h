#pragma once

#include "KeyEventSendResult.h"
#include "VoiceCompositionPipe.h"
#include <Windows.h>
#include <atomic>
#include <cstddef>
#include <cstdint>
#include <string>
#include <vector>

#include "../../../../shared/contracts/windows_ipc.h"
#include "../Global/CandidateOverlayHostPolicy.h"
#include "../../common/DedicatedEnglishMirror.h"
#include "../../common/InputSchemeTraits.h"

int InitNamedpipe();
int ConnectToAllNamedpipe();
int ConnectToTsfNamedpipe();
int CloseIpc();
int CloseNamedpipe();
void ResetNamedpipeReplyState();
HANDLE GetToTsfWorkerThreadNamedpipe();
void BindNamedpipeFocusState(
    _In_ const void *owner, _In_opt_ bool *focusResetPending, _In_opt_ bool *activationRequired,
    _In_opt_ std::atomic<uint64_t> *expectedWorkerFocusToken, _In_opt_ std::atomic<bool> *localSessionResetPending,
    _In_opt_ std::atomic<UINT> *localSessionResetToken, _In_opt_ std::atomic<bool> *workerCommitReady,
    _In_opt_ std::atomic<uint64_t> *acknowledgedWorkerFocusToken, _In_opt_ std::atomic<HANDLE> *workerPipeHandle,
    _In_opt_ std::atomic<UINT> *workerPipeGeneration);
void UnbindNamedpipeFocusState(_In_ const void *owner);
bool IsNamedpipeFocusStateOwner(_In_ const void *owner);
UINT BeginNamedpipeLocalSessionReset();
void InvalidateNamedpipeWorkerGeneration();
void MarkNamedpipeFocusLost();
void RequireNamedpipeFocusActivation();
void MarkNamedpipeSessionDirty();
bool MarkNamedpipeSessionDirtyForOwner(_In_ const void *owner);
bool EnsureNamedpipeFocusSessionActivated();
bool SupportsCharacterSetShortcut();
bool SupportsGameHostCandidate();
bool SupportsKeyboardCompositionCancel(_In_ const void *owner);
bool FlushNamedpipeFocusSessionReset();
bool FlushNamedpipeImeDeactivation(uint64_t focusToken = 0);
// 本线程当前的焦点令牌，也就是 ClientActivated 交给 Server 的那个；线程没有焦点时为 0。Aux 管道上的 KeySound 带着它，Server 只让正持有这个焦点的会话出声。
uint64_t GetNamedpipeFocusToken();
// 本线程的管道客户端号 (pid << 32) | tid，和 Hello 里报给 Server 的相同。
uint64_t GetNamedpipeClientId();

KeyEventSendResult SendKeyEventToUIProcess(_Out_opt_ uint64_t *requestId = nullptr);
void DebugTsfKeyLatency(_In_z_ const wchar_t *stage, uint64_t requestId, double elapsedMs, HRESULT result);
void DebugTsfIssue47(_In_z_ const wchar_t *stage, uint64_t requestId, UINT code, WCHAR wch, UINT category,
                     UINT function, int eaten, BOOL composing, size_t virtualKeyLength, HRESULT result,
                     uint64_t correlationToken = 0);
void QueueTsfDiagnosticLog(const std::wstring &line);
int SendHideCandidateWndEventToUIProcess();
int SendShowCandidateWndEventToUIProcess();
int SendMoveCandidateWndEventToUIProcess();
int SendLangbarRightClickEventToUIProcess(const RECT *prcArea);
int SendIMEActivationEventToUIProcessViaNamedPipe();
int SendIMEDeactivationEventToUIProcessViaNamedPipe();
int SendClientActivatedEventToServerViaNamedPipe(uint64_t focusToken);
int SendClientDeactivatedEventToServerViaNamedPipe(uint64_t focusToken = 0);
int SendClientSuspendedEventToServerViaNamedPipe();
int SendIMEStatusSnapshotToUIProcessViaNamedPipe(bool kbdIsOpen, bool fullwidthIsOpen, bool puncIsOpen,
                                                 bool assertsFocusOwnership = false);
int SendIMEStatusEventToUIProcessViaNamedPipe(bool kbdIsOpen, bool fullwidthIsOpen, bool puncIsOpen);
int SendIMESwitchEventToUIProcessViaNamedPipe(UINT uImeStatus);
int SendPuncSwitchEventToUIProcessViaNamedPipe(BOOL isPunc);
int SendPairedPunctuationAutoClosedToServerViaNamedPipe(WCHAR opening);
int SendDoubleSingleByteSwitchEventToUIProcessViaNamedPipe(BOOL isDoubleSingleByte);

bool SendToAuxNamedpipe(const std::wstring &pipeData, bool waitForAcknowledgement = false);

//
// For named pipe
//
int WriteDataToNamedPipe(              //
    UINT keycode,                      //
    WCHAR wch,                         //
    UINT modifiers_down,               //
    const int point[2],                //
    int pinyin_length,                 //
    const std::wstring &pinyin_string, //
    UINT write_flag                    //
);
KeyEventSendResult SendKeyEventToUIProcessViaNamedPipe(_Out_opt_ uint64_t *requestId = nullptr);
int SendHideCandidateWndEventToUIProcessViaNamedPipe();
int SendShowCandidateWndEventToUIProcessViaNamedPipe();
int SendMoveCandidateWndEventToUIProcessViaNamedPipe();
int SendLangbarRightClickEventToUIProcessViaNamedPipe(const RECT *prcArea);
void ClearNamedpipeDataIfExists();
struct FanyImeNamedpipeDataToTsf *TryReadDataFromServerPipeWithTimeout(uint64_t expectedRequestId);
// When abortTransportOnTimeout is false, a missed reply leaves the pipe up and
// returns a non-TransportUnavailable empty frame for the caller to fall back.
struct FanyImeNamedpipeDataToTsf *TryReadDataFromServerPipeWithTimeout(uint64_t expectedRequestId,
                                                                       bool abortTransportOnTimeout);
struct FanyImeNamedpipeDataToTsf *ReadDataFromServerViaNamedPipe(uint64_t expectedRequestId);

//
// Modifiers:
//     0b00000001: Shift
//     0b00000010: Control
//     0b00000100: Alt
// TODO: Make it able to denote explicit modifiers, e.g. LShift, RShift, we could use left keys
//
namespace Global
{
inline thread_local UINT Keycode = 0;
inline thread_local WCHAR wch = L'\0';
inline thread_local UINT ModifiersDown = 0;
inline thread_local int Point[2] = {100, 100};
inline thread_local int PinyinLength = 0;
extern thread_local std::wstring PinyinString;

// TF_TMF_UIELEMENTENABLEDONLY at ActivateEx, and/or BeginUIElement pbShow=FALSE.
inline thread_local bool HostUiLessMode = false;
inline thread_local bool CandidateUiLessMode = false;
inline bool IsUiLessMode()
{
    return HostUiLessMode || CandidateUiLessMode;
}
// 本次激活命中了游戏候选窗策略（CandidateOverlayHostPolicy.h）：不向 Server 报 UILess，也不调 BeginUIElement，候选由水杉的候选窗画。
inline thread_local bool ForceOverlayCandidate = false;
// 强制叠加时组字结束就把 Global::Point 复位成 {0, INVALID_Y}：下一次组字的第一个按键先于 _StartCandidateList 发出，不复位就会带着上一个输入框的坐标。只在组字真正结束时调，候选列表关闭而组字继续（韩文 Hanja、注音）时不调。
void ResetForcedOverlayAnchor();
extern thread_local CandidateOverlayDecision GameOverlayDecision;

inline thread_local int firefox_like_cnt = 0; // Apps like firefox, e.g. firefox, zen...
extern thread_local std::wstring current_process_name;

inline thread_local wchar_t app_name[512] = {0};

namespace DataFromServerMsgType = FanyImeReplyType;

namespace DataToTsfWorkerThreadMsgType = FanyImeWorkerReplyType;

namespace PunctuationLock
{
constexpr int Follow = 0;
constexpr int AlwaysChinese = 1;
constexpr int AlwaysEnglish = 2;
} // namespace PunctuationLock

inline std::atomic<int> PunctuationLockMode{PunctuationLock::Follow};

inline BOOL ResolvePunctuationOpen(BOOL followImeOpen)
{
    switch (PunctuationLockMode.load(std::memory_order_relaxed))
    {
    case PunctuationLock::AlwaysChinese:
        return TRUE;
    case PunctuationLock::AlwaysEnglish:
        return FALSE;
    default:
        return followImeOpen;
    }
}

inline bool IsPunctuationLocked()
{
    return PunctuationLockMode.load(std::memory_order_relaxed) != PunctuationLock::Follow;
}

inline std::atomic_bool PagingCommaPeriodEnabled{false};
// Opt-in: keep the safe disabled state until the Server sends persisted
// settings, including during a transient worker reconnect.
inline std::atomic_bool SmartPunctuationEnabled{false};
inline std::atomic_bool SmartPunctuationRepeatToChineseEnabled{false};
inline std::atomic_bool SmartPunctuationSpaceConvertEnabled{false};
inline std::atomic_bool SmartPunctuationDirectDigitEnabled{false};
inline std::atomic_bool SmartPunctuationDirectLetterEnabled{false};
// Default on until the Server sends the persisted setting.
inline std::atomic_bool PairedPunctuationEnabled{true};
inline std::atomic_bool MicrosoftShuangpinEnabled{false};
// 组字时 ';' 和 '\'' 选第二、第三个候选。Server 用 SecondThirdCandidateChanged 推送之前保持关闭，两个键照常是标点。
inline std::atomic_bool SecondThirdCandidateEnabled{false};
// The scheme the TIP keys before its host session answers a key: scheme::mode_scheme of the mode the Server last announced in InputModeChanged, or of the scheme the preferences run before it has (common/InputSchemeTraits.h). The pinyin and shape schemes all read as quanpin, which they key alike.
inline std::atomic_int InputModeScheme{0};
// 任务栏模式图标显示的模式：Server 上次在 InputModeChanged 里给的码（scheme::input_mode_code），双拼和五笔也各有自己的码；Server 还没给时是偏好里正在运行的方案。只有语言栏读它，键入看上面的 InputModeScheme。
inline std::atomic_int InputModeIndicator{L'0'};
// The V, "/" and "@" local modes, off until the Server sends LocalModeTriggersChanged: while off their keys route exactly as before the modes existed.
inline std::atomic_bool ExpressionModeEnabled{false};
inline std::atomic_bool CommandModeEnabled{false};
inline std::atomic_bool MentionModeEnabled{false};
// 焦点会话的 Engine 处于它自己的英文模式，TIP 从 compartment 看不到（那里仍是中文）：Server 用 DedicatedEnglishChanged 推送权威值，TIP 吞下 Ctrl+Shift+E 时先行翻转，见 DedicatedEnglishMirror。打开时，笔画方案空闲时的字母交给 Engine 而不是应用（scheme::LetterPassesWhileIdle）。
inline msime::windows::DedicatedEnglishMirror DedicatedEnglish;
inline std::atomic_bool CapsLockEnabled{false};
inline std::atomic_bool TsfDiagnosticLogEnabled{false};
inline thread_local bool g_connected = false;

} // namespace Global
