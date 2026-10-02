#include "../keyboard_composition_pipe.h"
#include <algorithm>
#include <cassert>
#include <cstring>

int main()
{
    using namespace FanyImeProtocol;
    using namespace FanyImeKeyboardCompositionPipe;
    static_assert(sizeof(FanyImeNamedpipeDataToTsfWorkerThread) == 404);
    static_assert(offsetof(FanyImeNamedpipeDataToTsfWorkerThread, data) == 4);
    static_assert(FanyImeWorkerReplyType::PunctuationLockChanged == 21);
    static_assert(FanyImeWorkerReplyType::CancelKeyboardComposition == 22);
    static_assert((Capabilities & KeyboardCompositionCancel) == 0);
    static_assert((RequiredCapabilities & KeyboardCompositionCancel) == 0);
    const auto supported = Capabilities | KeyboardCompositionCancel;
    const auto hello = Hello(7, 19, supported);
    const auto negotiated = Negotiate(hello, supported);
    assert(CanCancel(negotiated));
    assert(AcceptReply(Reply(hello, negotiated), 19));
    assert(!CanCancel(Negotiate(hello)));                   // new client / old server
    assert(!CanCancel(Negotiate(Hello(7, 19), supported))); // old client / new server
    FanyImeNamedpipeData unversioned{};
    unversioned.event_type = FanyImePipeEventType::ClientHello;
    unversioned.client_id = 7;
    assert(!Negotiate(unversioned, supported).accepted);
    assert(!CanCancel({false, supported}));
    assert(!EncodeCancel(19, Negotiate(hello)));
    assert(!EncodeCancel(0, negotiated));
    assert(!EncodeCancel(FANY_IME_NO_REQUEST_ID, negotiated));
    for (std::uint64_t token : {std::uint64_t{1}, std::uint64_t{77}, std::uint64_t{1} << 32, UINT64_MAX - 1})
    {
        const auto frame = EncodeCancel(token, negotiated);
        assert(frame && ParseCancel(*frame) == token);
        unsigned char bytes[404]{};
        std::memcpy(bytes, &*frame, sizeof(bytes));
        assert(bytes[0] == 22 && bytes[1] == 0 && bytes[2] == 0 && bytes[3] == 0);
        assert(bytes[5] == 0); // UTF-16 ASCII, not host-width wchar_t text.
    }
    for (const char *invalid :
         {"", "0", "01", "-1", "+1", "1 ", " 1", "1|2", "18446744073709551615", "18446744073709551616"})
    {
        auto frame = *EncodeCancel(77, negotiated);
        std::fill(std::begin(frame.data), std::end(frame.data), FanyImeWireChar{});
        for (std::size_t i = 0; invalid[i]; ++i)
            frame.data[i] = invalid[i];
        assert(!ParseCancel(frame));
    }
    auto frame = *EncodeCancel(77, negotiated);
    frame.data[3] = '1'; // Reject trailing data after the terminator.
    assert(!ParseCancel(frame));
    std::fill(std::begin(frame.data), std::end(frame.data), FanyImeWireChar{'1'});
    assert(!ParseCancel(frame));
    frame = *EncodeCancel(77, negotiated);
    frame.msg_type = FanyImeWorkerReplyType::CancelVoiceComposition;
    assert(!ParseCancel(frame));
    frame = *EncodeCancel(77, negotiated);
    frame.data[0] = static_cast<FanyImeWireChar>(0xFF11);
    assert(!ParseCancel(frame));
}
