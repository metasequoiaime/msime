#pragma once
#include "ipc_negotiation.h"
#include <array>
#include <charconv>
#include <limits>
#include <optional>

namespace FanyImeKeyboardCompositionPipe
{
inline bool CanCancel(const FanyImeProtocol::Negotiation &negotiated)
{
    return negotiated.accepted &&
           (negotiated.capabilities & FanyImeProtocol::KeyboardCompositionCancel) != 0;
}

inline std::optional<FanyImeNamedpipeDataToTsfWorkerThread> EncodeCancel(std::uint64_t focusToken,
                                                                         const FanyImeProtocol::Negotiation &negotiated)
{
    if (!CanCancel(negotiated) || focusToken == 0 || focusToken == FANY_IME_NO_REQUEST_ID)
        return std::nullopt;
    std::array<char, 20> digits{};
    const auto converted = std::to_chars(digits.data(), digits.data() + digits.size(), focusToken);
    if (converted.ec != std::errc{})
        return std::nullopt;
    FanyImeNamedpipeDataToTsfWorkerThread frame{};
    frame.msg_type = FanyImeWorkerReplyType::CancelKeyboardComposition;
    for (std::size_t i = 0; i < static_cast<std::size_t>(converted.ptr - digits.data()); ++i)
        frame.data[i] = static_cast<FanyImeWireChar>(digits[i]);
    return frame;
}

inline std::optional<std::uint64_t> ParseCancel(const FanyImeNamedpipeDataToTsfWorkerThread &frame)
{
    if (frame.msg_type != FanyImeWorkerReplyType::CancelKeyboardComposition || frame.data[0] < '1' ||
        frame.data[0] > '9')
        return std::nullopt;
    std::uint64_t token = 0;
    bool terminated = false;
    for (const auto unit : frame.data)
    {
        if (unit == 0)
        {
            terminated = true;
            continue;
        }
        if (terminated || unit < '0' || unit > '9')
            return std::nullopt;
        const auto digit = static_cast<std::uint64_t>(unit - '0');
        if (token > (std::numeric_limits<std::uint64_t>::max() - digit) / 10)
            return std::nullopt;
        token = token * 10 + digit;
    }
    if (!terminated || token == FANY_IME_NO_REQUEST_ID)
        return std::nullopt;
    return token;
}
} // namespace FanyImeKeyboardCompositionPipe
