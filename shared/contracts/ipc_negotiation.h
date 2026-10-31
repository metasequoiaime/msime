#pragma once

#include "windows_ipc.h"

// Main ClientHello carries the protocol fields in otherwise unused FanyImeNamedpipeData members, so the packet size, opcodes and reverse-pipe handshake layout stay those of every other frame.
namespace FanyImeProtocol
{
constexpr std::uint32_t Magic = 0x4D534950; // MSIP
constexpr std::uint16_t Major = 1;
constexpr std::uint16_t Minor = 0;
constexpr std::uint32_t RequestIds = 1u << 0;
constexpr std::uint32_t FocusEpochs = 1u << 1;
constexpr std::uint32_t FramedVoice = 1u << 2;
// Optional: Ctrl+Shift+F KeyEvent toggles the configured character set without
// changing composition. A consumer must opt in only after implementing it.
constexpr std::uint32_t CharacterSetShortcut = 1u << 3;
// Optional: focus-bound keyboard preedit cancellation without a text commit.
// Advertise only after implementing the UI-thread/edit-session cancellation.
constexpr std::uint32_t KeyboardCompositionCancel = 1u << 4;
// 可选：Server 认得 PipeMetadata::GameHost（强制叠加候选窗的游戏宿主）。TSF 只在协商到这一位之后才会在包上设 GameHost；旧 Server 不认这一位时，TSF 照常发包、不带 GameHost。和 KeyboardCompositionCancel 一样不放进 Capabilities 和 RequiredCapabilities，由双方各自在实现后再宣告。
constexpr std::uint32_t GameHostCandidate = 1u << 5;
// 可选：Server 认得 Main 事件 FieldTraits（windows_ipc.h）。TIP 只在协商到这一位之后才发它；协商不到时（新 TIP 配旧 Server、旧 TIP 配新 Server）不发，Server 那边的字段属性保持未知。和 GameHostCandidate 一样不放进 Capabilities 和 RequiredCapabilities。
constexpr std::uint32_t FieldTraits = 1u << 6;
// 可选：Ctrl+Shift+J 的 KeyEvent 在中文方案和日语方案之间切换，不改组字，Server 不回复。TIP 只在协商到这一位之后才吃这个组合键并发给 Server；协商不到时（新 TIP 配旧 Server）把它交给应用，免得旧 Server 把它当普通按键送进组字。和 FieldTraits 一样不放进 Capabilities 和 RequiredCapabilities。
constexpr std::uint32_t JapaneseToggleShortcut = 1u << 7;
// 可选：TIP 能在自己的宿主会话里执行外部候选窗发来的继续组字选择。
constexpr std::uint32_t CandidateContinuation = 1u << 8;
// 可选：TIP 以 ServerUiLessCandidates 明确请求旧管道路径的后台候选页。
constexpr std::uint32_t UiLessCandidates = 1u << 9;
constexpr std::uint32_t HostOnlineCandidates = 1u << 10;
// 可选：明确区分十八键面板组码与实体键盘的确定字母。
constexpr std::uint32_t GroupedPanelKeys = 1u << 11;
// 可选：方案通知携带存储修订，日语切换在双方应用后才放行后续键。
constexpr std::uint32_t OrderedPreferences = 1u << 12;
// 可选：提交帧携带模板尾段长度，固定包长与正文容量保持，见 quick_phrase_cursor_pipe.h。
constexpr std::uint32_t QuickPhraseCursor = 1u << 13;
constexpr std::uint32_t Capabilities = RequestIds | FocusEpochs | FramedVoice;
constexpr std::uint32_t RequiredCapabilities = RequestIds | FocusEpochs;

struct Negotiation
{
    bool accepted = false;
    std::uint32_t capabilities = 0;
};

inline FanyImeNamedpipeData Hello(std::uint64_t client, std::uint64_t request,
                                  std::uint32_t capabilities = Capabilities)
{
    FanyImeNamedpipeData hello{};
    hello.event_type = FanyImePipeEventType::ClientHello;
    hello.client_id = client;
    hello.request_id = request;
    hello.keycode = Magic;
    hello.wch = Major;
    hello.point[0] = Minor;
    hello.point[1] = static_cast<int>(RequiredCapabilities);
    hello.modifiers_down = capabilities;
    return hello;
}

inline Negotiation Negotiate(const FanyImeNamedpipeData &hello, std::uint32_t capabilities = Capabilities)
{
    if (hello.event_type != FanyImePipeEventType::ClientHello || hello.client_id == 0)
        return {};
    if (hello.keycode != Magic || hello.wch != Major || hello.request_id == 0 ||
        hello.request_id == FANY_IME_NO_REQUEST_ID || hello.point[0] < 0 || hello.point[1] < 0)
        return {};
    const auto required = static_cast<std::uint32_t>(hello.point[1]);
    const auto common = hello.modifiers_down & capabilities;
    if ((required & common) != required || (common & RequiredCapabilities) != RequiredCapabilities)
        return {};
    return {true, common};
}

inline FanyImeNamedpipeDataToTsf Reply(const FanyImeNamedpipeData &hello, Negotiation result)
{
    FanyImeNamedpipeDataToTsf reply{};
    reply.msg_type = result.accepted ? FanyImeReplyType::ProtocolReady : FanyImeReplyType::ProtocolMismatch;
    reply.request_id = hello.request_id;
    reply.candidate_string[0] = Major;
    reply.candidate_string[1] = Minor;
    reply.candidate_string[2] = static_cast<FanyImeWireChar>(result.capabilities & 0xFFFFu);
    reply.candidate_string[3] = static_cast<FanyImeWireChar>(result.capabilities >> 16);
    reply.candidate_string[4] = static_cast<FanyImeWireChar>(Magic & 0xFFFFu);
    reply.candidate_string[5] = static_cast<FanyImeWireChar>(Magic >> 16);
    return reply;
}

inline bool IsNegotiationReply(std::uint32_t type)
{
    return type == FanyImeReplyType::ProtocolReady || type == FanyImeReplyType::ProtocolMismatch;
}

inline std::uint32_t ReplyCapabilities(const FanyImeNamedpipeDataToTsf &reply)
{
    return static_cast<std::uint32_t>(reply.candidate_string[2]) |
           (static_cast<std::uint32_t>(reply.candidate_string[3]) << 16);
}

constexpr bool IsCharacterSetShortcut(std::uint32_t keycode, std::uint32_t modifiers)
{
    return keycode == 'F' && (modifiers & 7u) == 3u;
}

// 和 IsCharacterSetShortcut 一样只认 Ctrl+Shift，带 Alt 的组合不算。
constexpr bool IsJapaneseToggleShortcut(std::uint32_t keycode, std::uint32_t modifiers)
{
    return keycode == 'J' && (modifiers & 7u) == 3u;
}

inline bool AcceptReply(const FanyImeNamedpipeDataToTsf &reply, std::uint64_t expected_request)
{
    const auto capabilities = ReplyCapabilities(reply);
    const auto magic = static_cast<std::uint32_t>(reply.candidate_string[4]) |
                       (static_cast<std::uint32_t>(reply.candidate_string[5]) << 16);
    return expected_request != 0 && reply.request_id == expected_request &&
           reply.msg_type == FanyImeReplyType::ProtocolReady && magic == Magic && reply.candidate_string[0] == Major &&
           (capabilities & RequiredCapabilities) == RequiredCapabilities;
}
} // namespace FanyImeProtocol
