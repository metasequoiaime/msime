#include "../../IPC/QuickPhraseCursorReply.h"
#include "../../../../../shared/contracts/ipc_negotiation.h"
#include <array>
#include <iostream>
#include <stdexcept>

namespace {
void require(bool value) { if (!value) throw std::runtime_error("quick phrase cursor reply assertion"); }
}
int main() {
    try {
        namespace Cursor = FanyImeQuickPhraseCursorPipe;
        constexpr auto candidate = FanyImeWorkerReplyType::CommitCurCandidate;
        constexpr auto continued = FanyImeWorkerReplyType::CommitCandidateAndContinue;
        const std::wstring unicode{L'前', 0xd83c, 0xdf32, L'e', 0x301, L'后'};
        for (auto type : {candidate, continued}) {
            for (const auto &text : {std::wstring{}, std::wstring{L"正文"}, unicode}) {
                for (std::size_t suffix : {std::size_t{0}, text.size()}) {
                    const auto wire = Cursor::Encode(type, suffix, true);
                    require(wire.has_value());
                    const auto payload = type == continued ? L"3\t" + text : text;
                    for (bool supported : {false, true}) {
                        const auto metadata = msime::tsf::DecodeWorkerCursorReply(*wire, payload.c_str(), payload.size() + 1, supported);
                        require(metadata.has_value() == supported);
                        if (metadata) require(metadata->messageType == type && metadata->suffixUnits == suffix);
                    }
                }
            }
        }
        const auto wire = *Cursor::Encode(continued, 2, true);
        for (const auto &payload : {L"2\tx", L"2\t", L"\txx", L"-1\txx", L"1048577\txx", L"xxx"})
            require(!msime::tsf::DecodeWorkerCursorReply(wire, payload, std::wstring(payload).size() + 1, true));
        const std::wstring invalid{L'2', L'\t', 0xd800};
        require(!msime::tsf::DecodeWorkerCursorReply(wire, invalid.c_str(), invalid.size() + 1, true));
        const auto split = *Cursor::Encode(candidate, 4, true);
        require(!msime::tsf::DecodeWorkerCursorReply(split, unicode.c_str(), unicode.size() + 1, true));
        const std::array<wchar_t, 2> unterminated{L'x', L'x'};
        require(!msime::tsf::DecodeWorkerCursorReply(wire, unterminated.data(), unterminated.size(), true));
        for (auto type : {wire | Cursor::Reserved, Cursor::Flag | FanyImeWorkerReplyType::InsertText})
            require(!msime::tsf::DecodeWorkerCursorReply(type, L"2\txx", 5, true));
        const auto legacy = msime::tsf::DecodeWorkerCursorReply(candidate, L"", 1, false);
        require(legacy && legacy->messageType == candidate && !legacy->suffixUnits);
        // 新旧版本只使用能力交集，重连到旧 Server 后不能沿用此前的位置能力。
        const auto hello = FanyImeProtocol::Hello(1, 2, FanyImeProtocol::Capabilities | FanyImeProtocol::QuickPhraseCursor);
        for (bool server : {true, false, true}) {
            const auto capabilities = FanyImeProtocol::Capabilities | (server ? FanyImeProtocol::QuickPhraseCursor : 0u);
            const auto negotiated = FanyImeProtocol::Negotiate(hello, capabilities);
            require(negotiated.accepted);
            const auto metadata = msime::tsf::DecodeWorkerCursorReply(wire, L"2\txx", 5,
                (negotiated.capabilities & FanyImeProtocol::QuickPhraseCursor) != 0);
            require(metadata.has_value() == server);
        }
        std::cout << "Quick phrase cursor reply: worker payload, empty commit, negotiation and invalid frames passed\n";
    } catch (const std::exception &error) {
        std::cerr << error.what() << '\n';
        return 1;
    }
}
