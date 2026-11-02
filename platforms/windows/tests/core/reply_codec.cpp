#include "ReplyCodec.h"
#include "quick_phrase_cursor_pipe.h"
#include "ipc_negotiation.h"
#include <array>
#include <iostream>
#include <stdexcept>

using namespace msime::windows;
namespace {
void require_at(bool condition, int line) {
  if (!condition)
    throw std::runtime_error("Windows reply codec assertion failed at line " + std::to_string(line));
}
#define require(condition) require_at((condition), __LINE__)
std::u16string payload(const EncodedReply &reply) {
  require(static_cast<bool>(reply));
  std::u16string result;
  for (const auto c : reply.packet.candidate_string) {
    if (!c)
      break;
    result += static_cast<char16_t>(c);
  }
  return result;
}
void error(const EncodedReply &reply, ReplyError expected) {
  require(!reply && reply.error == expected && reply.packet.request_id == 0 &&
          reply.packet.msg_type == 0);
  for (const auto c : reply.packet.candidate_string)
    require(c == 0);
}
FanyImeVoiceCompositionPipe::Frame
voice_frame(const std::vector<uint8_t> &bytes, uint32_t message) {
  require(bytes.size() == sizeof(FanyImeNamedpipeDataToTsfWorkerThread));
  uint32_t encoded_message = 0;
  for (size_t i = 0; i < sizeof(encoded_message); ++i)
    encoded_message |= static_cast<uint32_t>(bytes[i]) << (8 * i);
  require(encoded_message == message);
  std::array<wchar_t, FanyImeVoiceCompositionPipe::kPacketChars> payload{};
  for (size_t i = 0; i < payload.size(); ++i) {
    const auto offset =
        offsetof(FanyImeNamedpipeDataToTsfWorkerThread, data) + 2 * i;
    payload[i] =
        static_cast<wchar_t>(static_cast<uint16_t>(bytes[offset]) |
                             (static_cast<uint16_t>(bytes[offset + 1]) << 8));
  }
  return FanyImeVoiceCompositionPipe::ParseFrame(payload.data());
}
std::wstring
assembled_voice_text(const std::vector<std::vector<uint8_t>> &frames,
                     uint32_t message, wchar_t generation) {
  std::wstring text;
  for (size_t i = 0; i < frames.size(); ++i) {
    const auto frame = voice_frame(frames[i], message);
    require(frame.valid && frame.generation == generation);
    require(frame.first == (i == 0));
    require(frame.last == (i + 1 == frames.size()));
    text += frame.chunk;
  }
  return text;
}
void cursor_contract() {
  namespace cursor = FanyImeQuickPhraseCursorPipe;
  static_assert(sizeof(FanyImeNamedpipeDataToTsf) == 416);
  static_assert(sizeof(FanyImeNamedpipeDataToTsfWorkerThread) == 404);
  for (const auto &suffix : std::vector<std::string>{"", "后", "😀后", "e\xcc\x81", "👩‍💻", "\r\n后", std::string(199, 'a')}) {
    const std::string text = suffix.size() == 199 ? suffix : "前" + suffix;
    const auto plain = candidate_commit(81, text);
    const auto encoded = candidate_commit(81, text, suffix);
    const auto units = cursor::SuffixUnits(encoded.packet.msg_type, false);
    require(encoded && units && *units == payload(candidate_commit(1, suffix)).size());
    require(payload(encoded) == payload(plain));
    require(cursor::MessageType(encoded.packet.msg_type, false) == FanyImeReplyType::Normal);
    require(cursor::ValidText(encoded.packet.candidate_string, 200, *units));
    require(!cursor::Decode(encoded.packet.msg_type, encoded.packet.candidate_string, 200, false, false));
    const auto decoded = cursor::Decode(encoded.packet.msg_type, encoded.packet.candidate_string, 200, false, true);
    require(decoded && decoded->messageType == FanyImeReplyType::Normal && decoded->suffixUnits == units);
    const auto plain_bytes = *wire_bytes(plain), encoded_bytes = *wire_bytes(encoded);
    require(std::equal(plain_bytes.begin() + 4, plain_bytes.end(), encoded_bytes.begin() + 4));
    const auto exact = exact_commit(81, text, suffix);
    require(cursor::MessageType(exact.packet.msg_type, false) == FanyImeReplyType::CommitExactText);
    const auto ui = ui_complete_selection(text, suffix);
    require(ui && !ui->before_trigger && ui->worker.size() == 404);
    uint32_t type = 0;
    for (size_t n = 0; n < 4; ++n) type |= static_cast<uint32_t>(ui->worker[n]) << (8 * n);
    require(cursor::SuffixUnits(type, true) == units);
    require(cursor::MessageType(type, true) == FanyImeWorkerReplyType::CommitCurCandidate);
    for (size_t n = 0; n < 400; ++n) require(ui->worker[4 + n] == encoded_bytes[16 + n]);
  }
  const auto continued = *commit_candidate_and_continue_bytes(4, "前😀后", "😀后");
  uint32_t type = 0;
  for (size_t n = 0; n < 4; ++n) type |= static_cast<uint32_t>(continued[n]) << (8 * n);
  require(cursor::SuffixUnits(type, true) == 3);
  require(cursor::MessageType(type, true) == FanyImeWorkerReplyType::CommitCandidateAndContinue);
  require(continued[4] == '4' && continued[6] == '\t');
  const auto invalid_continued = *commit_candidate_and_continue_bytes(4, "", "4\t");
  require(invalid_continued[3] == 0);
  for (const auto &suffix : std::vector<std::string>{"失配", std::string("\0", 1), "\xff"})
    require(candidate_commit(81, "前后", suffix).packet.msg_type == FanyImeReplyType::Normal);
  const auto valid = *cursor::Encode(FanyImeReplyType::Normal, 1, false);
  for (auto bad : {valid | cursor::Reserved, cursor::Flag | (200u << 8),
                   cursor::Flag | (1u << 8) | FanyImeReplyType::Preedit})
    require(!cursor::MessageType(bad, false));
  require(!cursor::Encode(FanyImeReplyType::NavigationIgnored, 1, false));
  require(!cursor::Encode(FanyImeWorkerReplyType::InsertText, 1, true));
  require(cursor::Encode(FanyImeReplyType::Normal, 0, false) == cursor::Flag);
  require(!cursor::ValidText(u"😀后", 4, 2));
  require(cursor::ValidText(u"😀后", 4, 3));
  const char16_t unterminated[] = {u'a', u'b'}, high[] = {0xd800, 0}, low[] = {0xdc00, 0};
  require(!cursor::ValidText(unterminated, 2, 1));
  require(!cursor::ValidText(high, 2, 1) && !cursor::ValidText(low, 2, 1));
  require(!cursor::ValidText(u"a", 2, 2) && !cursor::ValidText(u"", 1, 1));
  require(!cursor::Decode(valid | cursor::Reserved, u"后", 2, false, true));
  require(!cursor::Decode(*cursor::Encode(FanyImeReplyType::Normal, 2, false), u"😀后", 4, false, true));
  const auto empty = ui_complete_selection("", "");
  require(empty && empty->worker[3] == 0x80);
  for (const auto &suffix : std::vector<std::string>{"失配", std::string("\0", 1), "\xff"})
    require(!ui_complete_selection("", suffix));
  const auto plain = cursor::Decode(FanyImeReplyType::Normal, u"正文", 3, false, false);
  require(plain && !plain->suffixUnits);
  const auto oldCaps = FanyImeProtocol::Capabilities;
  const auto newCaps = oldCaps | FanyImeProtocol::QuickPhraseCursor;
  for (auto server : {oldCaps, newCaps}) for (auto tip : {oldCaps, newCaps}) {
    const auto agreed = FanyImeProtocol::Negotiate(FanyImeProtocol::Hello(1, 2, tip), server);
    require(agreed.accepted);
    require(bool(agreed.capabilities & FanyImeProtocol::QuickPhraseCursor) == (server == newCaps && tip == newCaps));
  }
}
} // namespace
int main() {
  try {
    cursor_contract();
    {
      const auto continued = commit_candidate_and_continue_bytes(4, "合成候选");
      require(continued && continued->size() == 404);
      require(continued->at(0) ==
              FanyImeWorkerReplyType::CommitCandidateAndContinue);
      require(continued->at(4) == '4' && continued->at(5) == 0 &&
              continued->at(6) == '\t' && continued->at(7) == 0);
      require(!commit_candidate_and_continue_bytes((1u << 20) + 1,
                                                   "合成候选"));
      require(!commit_candidate_and_continue_bytes(
          4, std::string(1, static_cast<char>(0xff))));
      require(!commit_candidate_and_continue_bytes(4, std::string(200, 'a')));

      require(!ui_complete_selection(""));
      require(!ui_complete_selection(std::string(200, 'a')));
      require(!ui_complete_selection(std::string("a\0b", 3)));
      require(!ui_complete_selection(std::string(1, static_cast<char>(0xff))));
      const auto complete = ui_complete_selection("中😀");
      require(complete && !complete->before_trigger &&
              complete->worker.size() == 404);
      require(complete->worker[0] ==
              FanyImeWorkerReplyType::CommitCurCandidate);
      require(complete->worker[4] == 0x2d && complete->worker[5] == 0x4e);
      require(complete->worker[6] == 0x3d && complete->worker[7] == 0xd8);
      require(complete->worker[8] == 0 && complete->worker[9] == 0xde);
      for (size_t i = 10; i < complete->worker.size(); ++i)
        require(complete->worker[i] == 0);
      const auto partial = ui_partial_selection("hao", "你", "你好");
      require(partial && partial->before_trigger);
      auto expected_partial =
          *wire_bytes(partial_selection(1, "hao", "你", "你好"));
      expected_partial[8] = 0;
      require(*partial->before_trigger == expected_partial);
      require(partial->worker[0] == FanyImeWorkerReplyType::CommitCurCandidate);
      for (size_t i = 1; i < partial->worker.size(); ++i)
        require(partial->worker[i] == 0);
      require(!ui_partial_selection("", "你", "你好"));
      require(!ui_partial_selection("hao", "你\t", "你好"));
      const auto rejected = ui_rejected_selection();
      require(rejected.before_trigger &&
              rejected.before_trigger->at(0) == FanyImeReplyType::OutofRange);
      for (size_t i = 1; i < rejected.before_trigger->size(); ++i)
        require(rejected.before_trigger->at(i) == 0);
    }
    for (auto [navigation, type] :
         std::vector<std::pair<NavigationReply, uint32_t>>{
             {NavigationReply::Ignored, FanyImeReplyType::NavigationIgnored},
             {NavigationReply::PreviousCandidate,
              FanyImeReplyType::MoveSelectionPrevious},
             {NavigationReply::NextCandidate,
              FanyImeReplyType::MoveSelectionNext},
             {NavigationReply::PreviousPage,
              FanyImeReplyType::MovePagePrevious},
             {NavigationReply::NextPage, FanyImeReplyType::MovePageNext}}) {
      const auto reply = navigation_reply(77, navigation);
      require(reply && reply.packet.msg_type == type && payload(reply).empty());
      const auto bytes = *wire_bytes(reply);
      require(bytes[0] == type && bytes[8] == 77);
      for (size_t i = 16; i < bytes.size(); ++i)
        require(bytes[i] == 0);
      error(navigation_reply(0, navigation), ReplyError::InvalidRequest);
      error(navigation_reply(FANY_IME_NO_REQUEST_ID, navigation),
            ReplyError::InvalidRequest);
    }
    error(navigation_reply(77, static_cast<NavigationReply>(99)),
          ReplyError::InvalidFields);
      require(!focus_ready_bytes(7, 8, 0));
      require(!focus_ready_bytes(0, 8, 9));
      require(!focus_ready_bytes(7, 0, 9));
    for (const auto &[mode, opcode] :
         std::vector<std::pair<WorkerMode, uint32_t>>{
             {WorkerMode::English, FanyImeWorkerReplyType::SwitchToEnglish},
             {WorkerMode::Chinese, FanyImeWorkerReplyType::SwitchToChinese},
             {WorkerMode::AsciiPunctuation,
              FanyImeWorkerReplyType::SwitchToPuncEn},
             {WorkerMode::ChinesePunctuation,
              FanyImeWorkerReplyType::SwitchToPuncCn},
             {WorkerMode::Fullwidth, FanyImeWorkerReplyType::SwitchToFullwidth},
             {WorkerMode::Halfwidth,
              FanyImeWorkerReplyType::SwitchToHalfwidth}}) {
      const auto bytes = worker_mode_bytes(mode);
      require(bytes && bytes->size() == 404 && bytes->at(0) == opcode);
      for (size_t i = 1; i < bytes->size(); ++i)
        require(bytes->at(i) == 0);
    }
    require(!worker_mode_bytes(static_cast<WorkerMode>(99)));
    {
      constexpr auto update = FanyImeWorkerReplyType::UpdateVoiceComposition;
      // 线格式是 UTF-16 单元，显式写出代理项，让 32 位 wchar_t 的宿主也验证同一份字节。
      const std::wstring utf16_text{0x4f60, 0x597d, 0xd83d, 0xde00};
      const auto frames = voice_composition_bytes(update, utf16_text, 7);
      require(frames && frames->size() == 1 && frames->front().size() == 404);
      const auto parsed = voice_frame(frames->front(), update);
      require(parsed.valid && parsed.first && parsed.last &&
              parsed.generation == 7 && parsed.chunk == utf16_text);

      const std::wstring long_text(400, L'测');
      const auto split = voice_composition_bytes(update, long_text, 9);
      require(split && split->size() >= 2 &&
              assembled_voice_text(*split, update, 9) == long_text);

      const std::wstring exact(FanyImeVoiceCompositionPipe::kMaxChunkChars,
                               L'字');
      const auto exact_frames = voice_composition_bytes(update, exact, 4);
      require(exact_frames && exact_frames->size() == 1 &&
              assembled_voice_text(*exact_frames, update, 4) == exact);

      const std::wstring oversized(
          FanyImeVoiceCompositionPipe::kMaxSnapshotChars + 80, L'啊');
      const auto clipped = voice_composition_bytes(update, oversized, 2);
      require(clipped &&
              assembled_voice_text(*clipped, update, 2) ==
                  oversized.substr(
                      0, FanyImeVoiceCompositionPipe::kMaxSnapshotChars));

      require(!voice_composition_bytes(update, L"", 0));
      require(!voice_composition_bytes(update, std::wstring(L"a\0b", 3), 7));
      require(
          !voice_composition_bytes(FanyImeWorkerReplyType::PipeReady, L"x", 7));
    }
    for (const auto &example : std::vector<std::pair<uint64_t, std::string>>{
             {1, "1"},
             {77, "77"},
             {4294967296ULL, "4294967296"},
             {UINT64_MAX, "18446744073709551615"}}) {
      const auto fence = focus_ready_bytes(7, 8, example.first);
      require(fence &&
              fence->size() == sizeof(FanyImeNamedpipeDataToTsfWorkerThread));
      require(fence->at(0) == FanyImeWorkerReplyType::FocusSessionReady);
      for (size_t i = 1; i < 4; ++i)
        require(fence->at(i) == 0);
      for (size_t i = 0; i < example.second.size(); ++i) {
        require(fence->at(4 + 2 * i) ==
                static_cast<uint8_t>(example.second[i]));
        require(fence->at(5 + 2 * i) == 0);
      }
      for (size_t i = 4 + example.second.size() * 2; i < fence->size(); ++i)
        require(fence->at(i) == 0);
    }
    for (auto role :
         {FanyImePipeRole::ToTsf, FanyImePipeRole::ToTsfWorkerThread}) {
      auto ready = pipe_ready_bytes(role);
      require(ready &&
              ready->size() ==
                  (role == FanyImePipeRole::ToTsf
                       ? sizeof(FanyImeNamedpipeDataToTsf)
                       : sizeof(FanyImeNamedpipeDataToTsfWorkerThread)));
      require(ready->at(0) == 9);
      for (size_t i = 1; i < ready->size(); ++i)
        require(ready->at(i) == 0);
    }
    require(!pipe_ready_bytes(FanyImePipeRole::Main));
    require(!pipe_ready_bytes(99));
    auto hello = FanyImeProtocol::Hello(9, 0x0102030405060708ULL);
    auto protocol = FanyImeProtocol::Negotiate(
        hello, FanyImeProtocol::RequiredCapabilities);
    auto packet = FanyImeProtocol::Reply(hello, protocol);
    auto ack = protocol_reply_bytes(packet);
    require(ack && ack->at(0) == FanyImeReplyType::ProtocolReady &&
            ack->at(8) == 8 && ack->at(15) == 1 && ack->at(20) == 3);
    require(ack->at(24) == 0x50 && ack->at(25) == 0x49 && ack->at(26) == 0x53 &&
            ack->at(27) == 0x4d);
    for (size_t i = 4; i < 8; ++i)
      require(ack->at(i) == 0);
    for (size_t i = 28; i < ack->size(); ++i)
      require(ack->at(i) == 0);
    hello.point[1] |= FanyImeProtocol::FramedVoice;
    protocol = FanyImeProtocol::Negotiate(
        hello, FanyImeProtocol::RequiredCapabilities);
    require(!protocol.accepted);
    ack = protocol_reply_bytes(FanyImeProtocol::Reply(hello, protocol));
    require(ack && ack->at(0) == FanyImeReplyType::ProtocolMismatch);
    packet.request_id = 0;
    require(!protocol_reply_bytes(packet));
    packet.request_id = FANY_IME_NO_REQUEST_ID;
    require(!protocol_reply_bytes(packet));
    packet.request_id = 1;
    packet.msg_type = FanyImeReplyType::Normal;
    require(!protocol_reply_bytes(packet));
    auto full = candidate_commit(23, "你好😀");
    require(full.packet.msg_type == FanyImeReplyType::Normal &&
            full.packet.request_id == 23);
    require(payload(full) == u"你好😀");
    auto bytes = wire_bytes(exact_commit(0x0102030405060708ULL, "中😀"));
    require(bytes.has_value() &&
            bytes->at(0) == FanyImeReplyType::CommitExactText);
    require(bytes->at(4) == 0 && bytes->at(5) == 0 && bytes->at(6) == 0 &&
            bytes->at(7) == 0);
    require(bytes->at(8) == 8 && bytes->at(15) == 1);
    require(bytes->at(16) == 0x2D && bytes->at(17) == 0x4E &&
            bytes->at(18) == 0x3D && bytes->at(19) == 0xD8 &&
            bytes->at(20) == 0 && bytes->at(21) == 0xDE);
    for (size_t i = 22; i < bytes->size(); ++i)
      require(bytes->at(i) == 0);
    require(!wire_bytes(exact_commit(0, "中")).has_value());
    auto exact = exact_commit(24, "你好，");
    require(exact.packet.msg_type == FanyImeReplyType::CommitExactText &&
            payload(exact) == u"你好，");
    auto preedit = preedit_reply(25, "ni hao");
    require(preedit.packet.msg_type == FanyImeReplyType::Preedit &&
            payload(preedit) == u"ni hao");
    auto partial = partial_selection(26, "hao", "你", "你好");
    require(partial.packet.msg_type == FanyImeReplyType::NeedToCreateWord &&
            payload(partial) == u"hao\t你\t你好");
    auto page = uiless_reply(27, "ni hao", {"你好", "拟好"}, 1);
    require(page.packet.msg_type == FanyImeReplyType::UiLessComposition &&
            payload(page) == u"ni hao\t你好,拟好\t1");
    require(payload(uiless_reply(28, "", {}, 0)) == u"\t\t0");
    require(ignored_reply(29).packet.msg_type ==
            FanyImeReplyType::NavigationIgnored);
    require(payload(exact_commit(30, std::string(199, 'a'))).size() == 199);
    error(exact_commit(30, std::string(200, 'a')), ReplyError::TooLong);
    require(payload(exact_commit(31, std::string(197, 'a') + "😀")).size() ==
            199);
    error(exact_commit(31, std::string(198, 'a') + "😀"), ReplyError::TooLong);
    std::string chinese;
    for (int i = 0; i < 199; ++i)
      chinese += "中";
    require(payload(candidate_commit(32, chinese)).size() == 199);
    error(candidate_commit(32, chinese + "中"), ReplyError::TooLong);
    for (const auto &invalid :
         {std::string("\x80"), std::string("\xc0\xaf"),
          std::string("\xe0\x80\xaf"), std::string("\xed\xa0\x80"),
          std::string("\xf4\x90\x80\x80"), std::string("\xf0\x9f"),
          std::string("\xe4x")})
      error(exact_commit(33, invalid), ReplyError::InvalidUtf8);
    error(exact_commit(34, std::string("a\0b", 3)), ReplyError::EmbeddedNul);
    error(exact_commit(0, "中"), ReplyError::InvalidRequest);
    error(exact_commit(FANY_IME_NO_REQUEST_ID, "中"),
          ReplyError::InvalidRequest);
    error(partial_selection(35, "", "你", "你"), ReplyError::InvalidFields);
    error(partial_selection(35, "hao", "你\t", "你"),
          ReplyError::InvalidFields);
    error(uiless_reply(36, "a\tb", {"中"}, 0), ReplyError::InvalidFields);
    error(uiless_reply(36, "a", {"a,b"}, 0), ReplyError::InvalidFields);
    error(uiless_reply(36, "a", {"中"}, 1), ReplyError::InvalidFields);
    error(uiless_reply(36, "a", std::vector<std::string>(10, "中"), 0),
          ReplyError::InvalidFields);
    error(uiless_reply(36, std::string(199, 'a'), {"中"}, 0),
          ReplyError::TooLong);
    std::cout
        << "Windows reply codec: legacy layouts, Unicode and bounds passed\n";
  } catch (const std::exception &exception) {
    std::cerr << exception.what() << '\n';
    return 1;
  }
}
