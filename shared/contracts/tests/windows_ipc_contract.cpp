#include "../ipc_negotiation.h"
#include "../voice_composition_pipe.h"

#include <algorithm>
#include <array>
#include <cstring>
#include <iostream>

#define CHECK(expression)                                                                                              \
    do                                                                                                                 \
    {                                                                                                                  \
        if (!(expression))                                                                                             \
        {                                                                                                              \
            std::cerr << #expression << " at " << __LINE__ << '\n';                                                    \
            return 1;                                                                                                  \
        }                                                                                                              \
    } while (false)

int main()
{
    // Wire bytes, not just matching C++ declarations: the x86 and x64 DLLs both write these offsets to the same Server, so changes must not move UTF-16 data.
    std::array<unsigned char, 304> bytes{};
    bytes[0] = 10; // ClientHello
    bytes[8] = 7;  // client id
    FanyImeNamedpipeData unversioned{};
    std::memcpy(&unversioned, bytes.data(), bytes.size());
    CHECK(unversioned.client_id == 7);
    // A hello without the magic, version and request id is rejected.
    CHECK(!FanyImeProtocol::Negotiate(unversioned).accepted);

    auto hello = FanyImeProtocol::Hello(7, 19);
    auto result = FanyImeProtocol::Negotiate(hello);
    CHECK(result.accepted);
    auto reply = FanyImeProtocol::Reply(hello, result);
    CHECK(FanyImeProtocol::AcceptReply(reply, 19));
    CHECK(!FanyImeProtocol::AcceptReply(reply, 18)); // stale reconnect ACK
    CHECK(!FanyImeProtocol::AcceptReply(reply, 0));

    const auto shortcutCapabilities = FanyImeProtocol::Capabilities | FanyImeProtocol::CharacterSetShortcut;
    const auto shortcutHello = FanyImeProtocol::Hello(7, 20, shortcutCapabilities);
    const auto oldServer = FanyImeProtocol::Negotiate(shortcutHello);
    CHECK(oldServer.accepted);
    CHECK((oldServer.capabilities & FanyImeProtocol::CharacterSetShortcut) == 0);
    const auto newServer = FanyImeProtocol::Negotiate(shortcutHello, shortcutCapabilities);
    CHECK(newServer.accepted);
    CHECK((newServer.capabilities & FanyImeProtocol::CharacterSetShortcut) != 0);
    const auto shortcutReply = FanyImeProtocol::Reply(shortcutHello, newServer);
    CHECK(FanyImeProtocol::AcceptReply(shortcutReply, 20));
    CHECK(FanyImeProtocol::ReplyCapabilities(shortcutReply) == shortcutCapabilities);
    CHECK((FanyImeProtocol::Negotiate(hello, shortcutCapabilities).capabilities &
           FanyImeProtocol::CharacterSetShortcut) == 0); // old client/new server
    CHECK(FanyImeProtocol::IsCharacterSetShortcut('F', 3));
    CHECK(FanyImeProtocol::IsCharacterSetShortcut('F', 0x80000003u));
    for (unsigned modifiers = 0; modifiers < 8; ++modifiers)
        CHECK(FanyImeProtocol::IsCharacterSetShortcut('F', modifiers) == (modifiers == 3));
    CHECK(!FanyImeProtocol::IsCharacterSetShortcut('E', 3));

    hello.wch += 1;
    result = FanyImeProtocol::Negotiate(hello);
    CHECK(!result.accepted);
    CHECK(!FanyImeProtocol::AcceptReply(FanyImeProtocol::Reply(hello, result), 19));
    hello = FanyImeProtocol::Hello(7, 19);
    hello.point[1] |= 1u << 20; // client requires an unknown capability
    CHECK(!FanyImeProtocol::Negotiate(hello).accepted);
    hello = FanyImeProtocol::Hello(7, 19);
    hello.modifiers_down |= 1u << 20; // unknown optional capabilities are ignored
    CHECK(FanyImeProtocol::Negotiate(hello).accepted);
    hello.modifiers_down &= ~FanyImeProtocol::FocusEpochs;
    CHECK(!FanyImeProtocol::Negotiate(hello).accepted);
    hello = FanyImeProtocol::Hello(7, 19);
    hello.keycode ^= 1;
    CHECK(!FanyImeProtocol::Negotiate(hello).accepted);
    hello = FanyImeProtocol::Hello(7, 0);
    CHECK(!FanyImeProtocol::Negotiate(hello).accepted);

    FanyImeNamedpipeDataToTsf old_server{};
    old_server.msg_type = FanyImeReplyType::PipeReady;
    old_server.request_id = 19;
    CHECK(!FanyImeProtocol::AcceptReply(old_server, 19));
    // Corrupt/missing negotiated capabilities cannot authorize the new DLL.
    reply.candidate_string[2] = 0;
    CHECK(!FanyImeProtocol::AcceptReply(reply, 19));

    CHECK(FanyImeWorkerReplyType::SwitchToEn == FanyImeWorkerReplyType::SwitchToEnglish);
    CHECK(FanyImeWorkerReplyType::CommitCandidate == FanyImeWorkerReplyType::CommitCurCandidate);
    CHECK(FanyImeVoiceControl::Start == 1);
    CHECK(FanyImeVoiceControl::Stop == 2);
    CHECK(FanyImeVoiceControl::Cancel == 3);
    CHECK(FanyImeVoiceControl::MaxMessageChars == 96);
    FanyImeVoiceControlHello control_hello{};
    CHECK(control_hello.magic == 0x56434C31);
    CHECK(control_hello.version == 1);
    CHECK(sizeof(control_hello) == 16);
    const std::wstring voice(1000, L'x');
    const auto frames = FanyImeVoiceCompositionPipe::EncodeSnapshot(voice, 7);
    CHECK(FanyImeVoiceCompositionPipe::AssembleFrames(frames) == voice);
    auto incomplete = frames;
    incomplete.erase(incomplete.begin());
    CHECK(FanyImeVoiceCompositionPipe::AssembleFrames(incomplete).empty());
    // A middle frame legally carries flags 0, so an unterminated packet must be rejected on the chunk, not accepted
    // because data[0] happens to be a NUL.
    std::array<wchar_t, FanyImeVoiceCompositionPipe::kPacketChars> packet{};
    packet.fill(L'x');
    packet[0] = 0; // middle frame
    packet[1] = 7; // generation
    CHECK(!FanyImeVoiceCompositionPipe::ParseFrame(packet.data()).valid);
    packet[FanyImeVoiceCompositionPipe::kPacketChars - 1] = 0;
    const auto middle = FanyImeVoiceCompositionPipe::ParseFrame(packet.data());
    CHECK(middle.valid && !middle.first && !middle.last && middle.generation == 7);
    CHECK(middle.chunk == std::wstring(FanyImeVoiceCompositionPipe::kMaxChunkChars, L'x'));
    const auto blank = FanyImeVoiceCompositionPipe::EncodeSnapshot(std::wstring(), 7);
    CHECK(blank.size() == 1);
    std::array<wchar_t, FanyImeVoiceCompositionPipe::kPacketChars> header{};
    std::copy(blank[0].begin(), blank[0].end(), header.begin());
    const auto empty = FanyImeVoiceCompositionPipe::ParseFrame(header.data());
    CHECK(empty.valid && empty.first && empty.last && empty.chunk.empty());
    std::cout << "Windows IPC layout, negotiation, upgrade and voice contracts passed\n";
}
