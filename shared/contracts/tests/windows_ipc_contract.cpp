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

    // GameHostCandidate 是可选能力：不进默认宣告和必需集合，也不和已有的能力位重叠。
    static_assert((FanyImeProtocol::Capabilities & FanyImeProtocol::GameHostCandidate) == 0);
    static_assert((FanyImeProtocol::RequiredCapabilities & FanyImeProtocol::GameHostCandidate) == 0);
    static_assert((FanyImeProtocol::GameHostCandidate &
                   (FanyImeProtocol::RequestIds | FanyImeProtocol::FocusEpochs | FanyImeProtocol::FramedVoice |
                    FanyImeProtocol::CharacterSetShortcut | FanyImeProtocol::KeyboardCompositionCancel)) == 0);
    // 新 TSF 对旧 Server 协商不到这一位，所以不会发 GameHost；双方都宣告时才协商成功。
    const auto gameCapabilities = FanyImeProtocol::Capabilities | FanyImeProtocol::GameHostCandidate;
    const auto gameHello = FanyImeProtocol::Hello(7, 21, gameCapabilities);
    const auto gameOldServer = FanyImeProtocol::Negotiate(gameHello);
    CHECK(gameOldServer.accepted);
    CHECK((gameOldServer.capabilities & FanyImeProtocol::GameHostCandidate) == 0);
    const auto gameNewServer = FanyImeProtocol::Negotiate(gameHello, gameCapabilities);
    CHECK(gameNewServer.accepted);
    CHECK((gameNewServer.capabilities & FanyImeProtocol::GameHostCandidate) != 0);
    const auto gameReply = FanyImeProtocol::Reply(gameHello, gameNewServer);
    CHECK(FanyImeProtocol::AcceptReply(gameReply, 21));
    CHECK(FanyImeProtocol::ReplyCapabilities(gameReply) == gameCapabilities);
    // 旧 TSF 配新 Server：先确认仍能连上，否则被拒时 capabilities 本来就是 0，后一条测不出东西。
    const auto gameOldClient = FanyImeProtocol::Negotiate(hello, gameCapabilities);
    CHECK(gameOldClient.accepted);
    CHECK((gameOldClient.capabilities & FanyImeProtocol::GameHostCandidate) == 0);

    // FieldTraits 同样是可选能力，事件号 17 紧接着 PairedPunctuationAutoClosed，不和已有的 Main 事件撞号。
    static_assert((FanyImeProtocol::Capabilities & FanyImeProtocol::FieldTraits) == 0);
    static_assert((FanyImeProtocol::RequiredCapabilities & FanyImeProtocol::FieldTraits) == 0);
    static_assert((FanyImeProtocol::FieldTraits &
                   (FanyImeProtocol::RequestIds | FanyImeProtocol::FocusEpochs | FanyImeProtocol::FramedVoice |
                    FanyImeProtocol::CharacterSetShortcut | FanyImeProtocol::KeyboardCompositionCancel |
                    FanyImeProtocol::GameHostCandidate)) == 0);
    static_assert(FanyImePipeEventType::FieldTraits == 17);
    for (const auto taken :
         {FanyImePipeEventType::KeyEvent, FanyImePipeEventType::HideCandidateWnd,
          FanyImePipeEventType::ShowCandidateWnd, FanyImePipeEventType::MoveCandidateWnd,
          FanyImePipeEventType::LangbarRightClick, FanyImePipeEventType::IMESwitch, FanyImePipeEventType::PuncSwitch,
          FanyImePipeEventType::DoubleSingleByteSwitch, FanyImePipeEventType::ClientHello,
          FanyImePipeEventType::ClientActivated, FanyImePipeEventType::ClientDeactivated,
          FanyImePipeEventType::StatusSnapshot, FanyImePipeEventType::ClientSuspended,
          FanyImePipeEventType::FocusRestored, FanyImePipeEventType::PairedPunctuationAutoClosed})
        CHECK(taken != FanyImePipeEventType::FieldTraits);
    // 三种组合：新 TIP 配旧 Server、旧 TIP 配新 Server 都协商不到，TIP 不发，Server 的属性保持未知；只有双方都宣告时才发。
    const auto traitsCapabilities = FanyImeProtocol::Capabilities | FanyImeProtocol::FieldTraits;
    const auto traitsHello = FanyImeProtocol::Hello(7, 22, traitsCapabilities);
    const auto traitsOldServer = FanyImeProtocol::Negotiate(traitsHello);
    CHECK(traitsOldServer.accepted);
    CHECK((traitsOldServer.capabilities & FanyImeProtocol::FieldTraits) == 0);
    CHECK((FanyImeProtocol::ReplyCapabilities(FanyImeProtocol::Reply(traitsHello, traitsOldServer)) &
           FanyImeProtocol::FieldTraits) == 0);
    const auto traitsNewServer = FanyImeProtocol::Negotiate(traitsHello, traitsCapabilities);
    CHECK(traitsNewServer.accepted);
    const auto traitsReply = FanyImeProtocol::Reply(traitsHello, traitsNewServer);
    CHECK(FanyImeProtocol::AcceptReply(traitsReply, 22));
    CHECK((FanyImeProtocol::ReplyCapabilities(traitsReply) & FanyImeProtocol::FieldTraits) != 0);
    const auto traitsOldClient = FanyImeProtocol::Negotiate(hello, traitsCapabilities);
    CHECK(traitsOldClient.accepted);
    CHECK((traitsOldClient.capabilities & FanyImeProtocol::FieldTraits) == 0);
    // JapaneseToggleShortcut 同样是可选能力，不和已有的能力位重叠。
    static_assert((FanyImeProtocol::Capabilities & FanyImeProtocol::JapaneseToggleShortcut) == 0);
    static_assert((FanyImeProtocol::RequiredCapabilities & FanyImeProtocol::JapaneseToggleShortcut) == 0);
    static_assert((FanyImeProtocol::JapaneseToggleShortcut &
                   (FanyImeProtocol::RequestIds | FanyImeProtocol::FocusEpochs | FanyImeProtocol::FramedVoice |
                    FanyImeProtocol::CharacterSetShortcut | FanyImeProtocol::KeyboardCompositionCancel |
                    FanyImeProtocol::GameHostCandidate | FanyImeProtocol::FieldTraits)) == 0);
    // 新 TIP 配旧 Server 协商不到，TIP 把组合键交给应用；旧 TIP 配新 Server 也协商不到；双方都宣告时才协商成功。
    const auto japaneseCapabilities = FanyImeProtocol::Capabilities | FanyImeProtocol::JapaneseToggleShortcut;
    const auto japaneseHello = FanyImeProtocol::Hello(7, 23, japaneseCapabilities);
    const auto japaneseOldServer = FanyImeProtocol::Negotiate(japaneseHello);
    CHECK(japaneseOldServer.accepted);
    CHECK((japaneseOldServer.capabilities & FanyImeProtocol::JapaneseToggleShortcut) == 0);
    CHECK((FanyImeProtocol::ReplyCapabilities(FanyImeProtocol::Reply(japaneseHello, japaneseOldServer)) &
           FanyImeProtocol::JapaneseToggleShortcut) == 0);
    const auto japaneseNewServer = FanyImeProtocol::Negotiate(japaneseHello, japaneseCapabilities);
    CHECK(japaneseNewServer.accepted);
    const auto japaneseReply = FanyImeProtocol::Reply(japaneseHello, japaneseNewServer);
    CHECK(FanyImeProtocol::AcceptReply(japaneseReply, 23));
    CHECK((FanyImeProtocol::ReplyCapabilities(japaneseReply) & FanyImeProtocol::JapaneseToggleShortcut) != 0);
    const auto japaneseOldClient = FanyImeProtocol::Negotiate(hello, japaneseCapabilities);
    CHECK(japaneseOldClient.accepted);
    CHECK((japaneseOldClient.capabilities & FanyImeProtocol::JapaneseToggleShortcut) == 0);
    // 修饰键只认 Ctrl+Shift：单按、Ctrl+J、Shift+J、Ctrl+Alt+Shift+J 都不算，和简繁的 F 也不混。
    CHECK(FanyImeProtocol::IsJapaneseToggleShortcut('J', 3));
    CHECK(FanyImeProtocol::IsJapaneseToggleShortcut('J', 0x80000003u));
    for (unsigned modifiers = 0; modifiers < 8; ++modifiers)
        CHECK(FanyImeProtocol::IsJapaneseToggleShortcut('J', modifiers) == (modifiers == 3));
    CHECK(!FanyImeProtocol::IsJapaneseToggleShortcut('F', 3));
    CHECK(!FanyImeProtocol::IsCharacterSetShortcut('J', 3));
    // 载荷：停用的键盘、空上下文和安全桌面都是 SECURE，提权进程是 ELEVATED，别的位从不出现。
    static_assert(FanyImeFieldTraits::Reported == (FanyImeFieldTraits::Secure | FanyImeFieldTraits::Elevated));
    CHECK(FanyImeFieldTraits::Observe(false, false, false) == 0u);
    CHECK(FanyImeFieldTraits::Observe(true, false, false) == FanyImeFieldTraits::Secure);
    CHECK(FanyImeFieldTraits::Observe(false, true, false) == FanyImeFieldTraits::Secure);
    CHECK(FanyImeFieldTraits::Observe(false, false, true) == FanyImeFieldTraits::Elevated);
    CHECK(FanyImeFieldTraits::Observe(true, true, true) == FanyImeFieldTraits::Reported);
    for (unsigned flags = 0; flags < 8; ++flags)
        CHECK((FanyImeFieldTraits::Observe((flags & 1u) != 0, (flags & 2u) != 0, (flags & 4u) != 0) &
               ~FanyImeFieldTraits::Reported) == 0);
    // 查不出令牌时当作提权：提权窗口里不运行脚本靠的就是这一位，ELEVATED 又不让声音静音，多报没有代价。
    static_assert(FanyImeFieldTraits::ProcessElevated(true, true));
    static_assert(!FanyImeFieldTraits::ProcessElevated(true, false));
    static_assert(FanyImeFieldTraits::ProcessElevated(false, false));
    static_assert(FanyImeFieldTraits::ProcessElevated(false, true));

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

    static_assert(FanyImeWorkerReplyType::SwitchToEn == FanyImeWorkerReplyType::SwitchToEnglish);
    static_assert(FanyImeWorkerReplyType::CommitCandidate == FanyImeWorkerReplyType::CommitCurCandidate);
    // 新帧只能追加在最后：旧 TIP 丢掉大于它所知 MaxKnown 的类型，已有类型的编号不能动。
    static_assert(FanyImeWorkerReplyType::SecondThirdCandidateChanged == 30);
    static_assert(FanyImeWorkerReplyType::PunctuationMarksChanged == 31);
    static_assert(FanyImeWorkerReplyType::WubiCandidateSelectionChanged == 32);
    static_assert(FanyImeWorkerReplyType::CandidateContinuation == 33);
    static_assert(FanyImeWorkerReplyType::UiLessCandidatesChanged == 35);
    static_assert(FanyImeWorkerReplyType::HostOnlineCandidates == 36);
    static_assert(FanyImeWorkerReplyType::PreferenceTransition == 37);
    static_assert(FanyImeWorkerReplyType::MaxKnown == FanyImeWorkerReplyType::PreferenceTransition);
    static_assert(FanyImeVoiceControl::Start == 1);
    static_assert(FanyImeVoiceControl::Stop == 2);
    static_assert(FanyImeVoiceControl::Cancel == 3);
    static_assert(FanyImeVoiceControl::MaxMessageChars == 96);
    FanyImeVoiceControlHello control_hello{};
    CHECK(control_hello.magic == 0x56434C31);
    CHECK(control_hello.version == 1);
    static_assert(sizeof(control_hello) == 16);
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
