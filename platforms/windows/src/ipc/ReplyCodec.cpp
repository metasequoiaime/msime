#include "ReplyCodec.h"
#include <algorithm>
#include <charconv>

namespace msime::windows {
namespace {
EncodedReply failed(ReplyError error) { return {error, {}}; }
EncodedReply text_reply(uint64_t request, uint32_t type,
                        std::string_view text) {
  if (!request || request == FANY_IME_NO_REQUEST_ID)
    return failed(ReplyError::InvalidRequest);
  EncodedReply result;
  result.packet.request_id = request;
  result.packet.msg_type = type;
  size_t output = 0;
  for (size_t input = 0; input < text.size();) {
    const auto first = static_cast<uint8_t>(text[input++]);
    uint32_t scalar;
    uint32_t minimum;
    unsigned count;
    if (first < 0x80) {
      scalar = first;
      minimum = 0;
      count = 0;
    } else if (first >= 0xC2 && first <= 0xDF) {
      scalar = first & 0x1F;
      minimum = 0x80;
      count = 1;
    } else if (first >= 0xE0 && first <= 0xEF) {
      scalar = first & 0x0F;
      minimum = 0x800;
      count = 2;
    } else if (first >= 0xF0 && first <= 0xF4) {
      scalar = first & 0x07;
      minimum = 0x10000;
      count = 3;
    } else
      return failed(ReplyError::InvalidUtf8);
    if (text.size() - input < count)
      return failed(ReplyError::InvalidUtf8);
    for (unsigned index = 0; index < count; ++index) {
      auto next = static_cast<uint8_t>(text[input++]);
      if ((next & 0xC0) != 0x80)
        return failed(ReplyError::InvalidUtf8);
      scalar = (scalar << 6) | (next & 0x3F);
    }
    if (scalar < minimum || scalar > 0x10FFFF ||
        (scalar >= 0xD800 && scalar <= 0xDFFF))
      return failed(ReplyError::InvalidUtf8);
    if (scalar == 0)
      return failed(ReplyError::EmbeddedNul);
    const size_t units = scalar > 0xFFFF ? 2 : 1;
    if (output + units > FanyImePipeLimits::CandidateTextMaxLength)
      return failed(ReplyError::TooLong);
    if (units == 2) {
      scalar -= 0x10000;
      result.packet.candidate_string[output++] =
          static_cast<FanyImeWireChar>(0xD800 + (scalar >> 10));
      result.packet.candidate_string[output++] =
          static_cast<FanyImeWireChar>(0xDC00 + (scalar & 0x3FF));
    } else
      result.packet.candidate_string[output++] =
          static_cast<FanyImeWireChar>(scalar);
  }
  // Whole packet was zero-initialized: both terminator and unused tail stay
  // zero.
  return result;
}
bool contains_delimiter(std::string_view text) {
  return text.find('\t') != std::string_view::npos;
}
} // namespace
EncodedReply candidate_commit(uint64_t request, std::string_view text) {
  return text_reply(request, FanyImeReplyType::Normal, text);
}
EncodedReply exact_commit(uint64_t request, std::string_view text) {
  return text_reply(request, FanyImeReplyType::CommitExactText, text);
}
EncodedReply preedit_reply(uint64_t request, std::string_view text) {
  return text_reply(request, FanyImeReplyType::Preedit, text);
}
EncodedReply ignored_reply(uint64_t request) {
  return text_reply(request, FanyImeReplyType::NavigationIgnored, {});
}
EncodedReply navigation_reply(uint64_t request, NavigationReply navigation) {
  uint32_t type;
  switch (navigation) {
  case NavigationReply::Ignored:
    type = FanyImeReplyType::NavigationIgnored;
    break;
  case NavigationReply::PreviousCandidate:
    type = FanyImeReplyType::MoveSelectionPrevious;
    break;
  case NavigationReply::NextCandidate:
    type = FanyImeReplyType::MoveSelectionNext;
    break;
  case NavigationReply::PreviousPage:
    type = FanyImeReplyType::MovePagePrevious;
    break;
  case NavigationReply::NextPage:
    type = FanyImeReplyType::MovePageNext;
    break;
  default:
    return failed(ReplyError::InvalidFields);
  }
  return text_reply(request, type, {});
}
namespace {
ReplyBytes packet_bytes(const FanyImeNamedpipeDataToTsf &packet) {
  ReplyBytes bytes{};
  const auto put = [&](size_t offset, uint64_t value, size_t width) {
    for (size_t index = 0; index < width; ++index)
      bytes[offset + index] = static_cast<uint8_t>(value >> (8 * index));
  };
  put(offsetof(FanyImeNamedpipeDataToTsf, msg_type), packet.msg_type, 4);
  put(offsetof(FanyImeNamedpipeDataToTsf, request_id), packet.request_id, 8);
  for (size_t index = 0; index < FanyImePipeLimits::CandidateTextCapacity;
       ++index)
    put(offsetof(FanyImeNamedpipeDataToTsf, candidate_string) + 2 * index,
        static_cast<uint16_t>(packet.candidate_string[index]), 2);
  return bytes;
}
} // namespace
std::optional<ReplyBytes> wire_bytes(const EncodedReply &reply) {
  if (!reply || !reply.packet.request_id ||
      reply.packet.request_id == FANY_IME_NO_REQUEST_ID)
    return std::nullopt;
  return packet_bytes(reply.packet);
}
std::optional<ReplyBytes>
protocol_reply_bytes(const FanyImeNamedpipeDataToTsf &packet) {
  if ((packet.msg_type != FanyImeReplyType::ProtocolReady &&
       packet.msg_type != FanyImeReplyType::ProtocolMismatch) ||
      !packet.request_id || packet.request_id == FANY_IME_NO_REQUEST_ID)
    return std::nullopt;
  return packet_bytes(packet);
}
std::optional<std::vector<uint8_t>> pipe_ready_bytes(uint32_t role) {
  if (role == FanyImePipeRole::ToTsf) {
    FanyImeNamedpipeDataToTsf packet{};
    packet.msg_type = FanyImeReplyType::PipeReady;
    const auto bytes = packet_bytes(packet);
    return std::vector<uint8_t>(bytes.begin(), bytes.end());
  }
  if (role != FanyImePipeRole::ToTsfWorkerThread)
    return std::nullopt;
  std::vector<uint8_t> bytes(sizeof(FanyImeNamedpipeDataToTsfWorkerThread), 0);
  for (size_t i = 0; i < sizeof(uint32_t); ++i)
    bytes[i] =
        static_cast<uint8_t>(FanyImeWorkerReplyType::PipeReady >> (8 * i));
  return bytes;
}
std::optional<std::vector<uint8_t>> focus_ready_bytes(uint64_t client_id,
                                                       uint64_t activation_epoch,
                                                       uint64_t token) {
  if (!client_id || !activation_epoch || !token)
    return std::nullopt;
  std::array<char, 20> decimal{};
  const auto converted =
      std::to_chars(decimal.data(), decimal.data() + decimal.size(), token);
  if (converted.ec != std::errc{})
    return std::nullopt;
  std::vector<uint8_t> bytes(sizeof(FanyImeNamedpipeDataToTsfWorkerThread), 0);
  for (size_t i = 0; i < sizeof(uint32_t); ++i)
    bytes[i] = static_cast<uint8_t>(FanyImeWorkerReplyType::FocusSessionReady >>
                                    (8 * i));
  const auto count = static_cast<size_t>(converted.ptr - decimal.data());
  for (size_t i = 0; i < count; ++i)
    bytes[offsetof(FanyImeNamedpipeDataToTsfWorkerThread, data) + 2 * i] =
        static_cast<uint8_t>(decimal[i]);
  return bytes;
}
std::optional<std::vector<uint8_t>> worker_mode_bytes(WorkerMode mode) {
  uint32_t type;
  switch (mode) {
  case WorkerMode::English:
    type = FanyImeWorkerReplyType::SwitchToEnglish;
    break;
  case WorkerMode::Chinese:
    type = FanyImeWorkerReplyType::SwitchToChinese;
    break;
  case WorkerMode::AsciiPunctuation:
    type = FanyImeWorkerReplyType::SwitchToPuncEn;
    break;
  case WorkerMode::ChinesePunctuation:
    type = FanyImeWorkerReplyType::SwitchToPuncCn;
    break;
  case WorkerMode::Fullwidth:
    type = FanyImeWorkerReplyType::SwitchToFullwidth;
    break;
  case WorkerMode::Halfwidth:
    type = FanyImeWorkerReplyType::SwitchToHalfwidth;
    break;
  default:
    return std::nullopt;
  }
  std::vector<uint8_t> bytes(sizeof(FanyImeNamedpipeDataToTsfWorkerThread), 0);
  for (size_t i = 0; i < sizeof(type); ++i)
    bytes[i] = static_cast<uint8_t>(type >> (8 * i));
  return bytes;
}
namespace {
// A worker frame whose payload is a short wide string, NUL terminated inside
// the fixed-size data array.
std::vector<uint8_t> worker_text_frame(uint32_t type, std::wstring_view text) {
  std::vector<uint8_t> bytes(sizeof(FanyImeNamedpipeDataToTsfWorkerThread), 0);
  for (size_t i = 0; i < sizeof(type); ++i)
    bytes[i] = static_cast<uint8_t>(type >> (8 * i));
  const size_t offset = offsetof(FanyImeNamedpipeDataToTsfWorkerThread, data);
  const size_t capacity = FanyImePipeLimits::CandidateTextCapacity;
  // Leave room for the terminator; the TIP reads the payload as a C string.
  const size_t count = (std::min)(text.size(), capacity - 1);
  for (size_t i = 0; i < count; ++i) {
    const auto unit = static_cast<uint16_t>(text[i]);
    bytes[offset + i * 2] = static_cast<uint8_t>(unit & 0xFF);
    bytes[offset + i * 2 + 1] = static_cast<uint8_t>(unit >> 8);
  }
  return bytes;
}
std::vector<uint8_t> worker_flag_frame(uint32_t type, bool value) {
  return worker_text_frame(type, value ? L"1" : L"0");
}
} // namespace

std::vector<uint8_t> caps_lock_frame(bool enabled) {
  return worker_flag_frame(FanyImeWorkerReplyType::CapsLockChanged, enabled);
}

std::vector<std::vector<uint8_t>> tsf_config_frames(const TsfLocalConfig &config) {
  std::vector<std::vector<uint8_t>> frames;
  frames.reserve(9);
  // The paging frame carries the preedit style after a '|', which is how the
  // TIP receives it - there is no separate message type for it.
  std::wstring paging = config.paging_comma_period ? L"1" : L"0";
  const auto &style = config.preedit_style;
  if (style == "pinyin" || style == "empty" || style == "raw") {
    paging.push_back(L'|');
    for (char ch : style)
      paging.push_back(static_cast<wchar_t>(ch));
  }
  frames.push_back(worker_text_frame(
      FanyImeWorkerReplyType::PagingCommaPeriodChanged, paging));
  frames.push_back(worker_flag_frame(
      FanyImeWorkerReplyType::SmartPunctuationChanged, config.smart_punctuation));
  frames.push_back(worker_flag_frame(
      FanyImeWorkerReplyType::SmartPunctuationRepeatToChineseChanged,
      config.smart_punctuation_repeat_to_chinese));
  frames.push_back(worker_flag_frame(
      FanyImeWorkerReplyType::PairedPunctuationChanged, config.paired_punctuation));
  frames.push_back(worker_flag_frame(
      FanyImeWorkerReplyType::MicrosoftShuangpinChanged, config.microsoft_shuangpin));
  // "0" 全拼、双拼或五笔，"1" 日文，"2" 韩文，"3" 粤拼，"4" 注音，"5" 越南文，"6" 藏文。
  frames.push_back(worker_text_frame(
      FanyImeWorkerReplyType::InputModeChanged,
      std::wstring(1, scheme::input_mode_code(config.input_mode))));
  frames.push_back(worker_flag_frame(
      FanyImeWorkerReplyType::TsfDiagnosticLogChanged, config.tsf_diagnostic_log));
  // "<lock>|s<0|1>d<0|1>l<0|1>": the lock code followed by the space-convert, direct-digit and direct-letter policy.
  std::wstring lock = {static_cast<wchar_t>(L'0' + (config.punctuation_lock % 3)),
                       L'|', L's',
                       static_cast<wchar_t>(config.smart_punctuation_space_convert ? L'1' : L'0'),
                       L'd',
                       static_cast<wchar_t>(config.smart_punctuation_direct_digit ? L'1' : L'0'),
                       L'l',
                       static_cast<wchar_t>(config.smart_punctuation_direct_letter ? L'1' : L'0')};
  frames.push_back(
      worker_text_frame(FanyImeWorkerReplyType::PunctuationLockChanged, lock));
  // Last, so the frames before it keep their positions. The Engine's own English mode opens none of the three (its spelling_symbols is empty), so all of them go off while it holds: a "/" the TIP composed there would never reach the Engine.
  const bool pinyin_modes = !config.dedicated_english;
  const std::wstring triggers = {config.expression_mode && pinyin_modes ? L'1' : L'0',
                                 config.command_mode && pinyin_modes ? L'1' : L'0',
                                 config.mention_mode && pinyin_modes ? L'1' : L'0'};
  frames.push_back(worker_text_frame(
      FanyImeWorkerReplyType::LocalModeTriggersChanged, triggers));
  return frames;
}

std::optional<std::vector<std::vector<uint8_t>>>
voice_composition_bytes(uint32_t message, std::wstring_view text,
                         wchar_t generation) {
  if (message != FanyImeWorkerReplyType::UpdateVoiceComposition &&
      message != FanyImeWorkerReplyType::CommitVoiceComposition &&
      message != FanyImeWorkerReplyType::CancelVoiceComposition)
    return std::nullopt;
  if (!generation || text.find(L'\0') != std::wstring_view::npos)
    return std::nullopt;
  const auto frames = FanyImeVoiceCompositionPipe::EncodeSnapshot(
      std::wstring(text), generation);
  std::vector<std::vector<uint8_t>> encoded;
  encoded.reserve(frames.size());
  for (const auto &frame : frames) {
    if (frame.size() >= FanyImeVoiceCompositionPipe::kPacketChars)
      return std::nullopt;
    std::vector<uint8_t> bytes(
        sizeof(FanyImeNamedpipeDataToTsfWorkerThread), 0);
    for (size_t i = 0; i < sizeof(uint32_t); ++i)
      bytes[i] = static_cast<uint8_t>(message >> (8 * i));
    for (size_t i = 0; i < frame.size(); ++i) {
      const auto unit = static_cast<uint16_t>(frame[i]);
      bytes[offsetof(FanyImeNamedpipeDataToTsfWorkerThread, data) + 2 * i] =
          static_cast<uint8_t>(unit);
      bytes[offsetof(FanyImeNamedpipeDataToTsfWorkerThread, data) + 2 * i + 1] =
          static_cast<uint8_t>(unit >> 8);
    }
    encoded.push_back(std::move(bytes));
  }
  return encoded;
}
EncodedReply partial_selection(uint64_t request, std::string_view raw,
                               std::string_view prefix,
                               std::string_view display) {
  if (raw.empty() || prefix.empty() || contains_delimiter(raw) ||
      contains_delimiter(prefix) || contains_delimiter(display) ||
      std::any_of(raw.begin(), raw.end(),
                  [](unsigned char c) { return c < 0x20 || c > 0x7E; }))
    return failed(ReplyError::InvalidFields);
  // Bound each field before allocating a composite string; a representable
  // UTF-8 scalar needs at most three bytes per UTF-16 code unit.
  constexpr size_t max_bytes = FanyImePipeLimits::CandidateTextMaxLength * 3;
  if (raw.size() > max_bytes || prefix.size() > max_bytes ||
      display.size() > max_bytes)
    return failed(ReplyError::TooLong);
  return text_reply(request, FanyImeReplyType::NeedToCreateWord,
                    std::string(raw) + '\t' + std::string(prefix) + '\t' +
                        std::string(display));
}
namespace {
std::vector<uint8_t> worker_from_text(uint32_t type, const EncodedReply &text) {
  std::vector<uint8_t> bytes(sizeof(FanyImeNamedpipeDataToTsfWorkerThread), 0);
  for (size_t i = 0; i < 4; ++i)
    bytes[i] = static_cast<uint8_t>(type >> (8 * i));
  for (size_t i = 0; i < FanyImePipeLimits::CandidateTextCapacity; ++i) {
    const auto unit = static_cast<uint16_t>(text.packet.candidate_string[i]);
    bytes[4 + 2 * i] = static_cast<uint8_t>(unit);
    bytes[5 + 2 * i] = static_cast<uint8_t>(unit >> 8);
  }
  return bytes;
}
std::vector<uint8_t> candidate_worker(const EncodedReply &text) {
  return worker_from_text(FanyImeWorkerReplyType::CommitCurCandidate, text);
}
UiSelectionFrames triggered_selection(EncodedReply reply) {
  reply.packet.request_id = 0; // Dedicated UI encoding, never wire_bytes().
  return {packet_bytes(reply.packet),
          candidate_worker(candidate_commit(1, {}))};
}
} // namespace
std::optional<std::vector<uint8_t>>
commit_candidate_and_continue_bytes(size_t consumed, std::string_view text) {
  constexpr size_t maximum_consumed = 1u << 20;
  if (consumed > maximum_consumed)
    return std::nullopt;
  std::array<char, 20> decimal{};
  const auto converted =
      std::to_chars(decimal.data(), decimal.data() + decimal.size(), consumed);
  if (converted.ec != std::errc{})
    return std::nullopt;
  std::string payload(decimal.data(), converted.ptr);
  payload.push_back('\t');
  payload.append(text);
  const auto encoded = candidate_commit(1, payload);
  if (!encoded)
    return std::nullopt;
  return worker_from_text(FanyImeWorkerReplyType::CommitCandidateAndContinue,
                          encoded);
}
std::optional<UiSelectionFrames> ui_complete_selection(std::string_view text) {
  // Empty worker text is a trigger to consume id 0, not an empty commit.
  if (text.empty())
    return std::nullopt;
  const auto encoded = candidate_commit(1, text);
  if (!encoded)
    return std::nullopt;
  return UiSelectionFrames{std::nullopt, candidate_worker(encoded)};
}
std::optional<UiSelectionFrames>
ui_partial_selection(std::string_view raw, std::string_view prefix,
                     std::string_view display) {
  const auto encoded = partial_selection(1, raw, prefix, display);
  if (!encoded)
    return std::nullopt;
  return triggered_selection(encoded);
}
UiSelectionFrames ui_rejected_selection() {
  auto reply = candidate_commit(1, {});
  reply.packet.msg_type = FanyImeReplyType::OutofRange;
  return triggered_selection(reply);
}
EncodedReply uiless_reply(uint64_t request, std::string_view display,
                          const std::vector<std::string> &page,
                          size_t highlighted) {
  if (contains_delimiter(display) || page.size() > 9 ||
      (page.empty() ? highlighted != 0 : highlighted >= page.size()))
    return failed(ReplyError::InvalidFields);
  constexpr size_t max_bytes = FanyImePipeLimits::CandidateTextMaxLength * 3;
  if (display.size() > max_bytes)
    return failed(ReplyError::TooLong);
  std::string payload(display);
  payload += '\t';
  for (size_t index = 0; index < page.size(); ++index) {
    const auto &candidate = page[index];
    // The existing delimiter protocol has no escaping. Refuse ambiguous
    // candidates rather than silently split, omit or change their text.
    if (candidate.empty() || contains_delimiter(candidate) ||
        candidate.find(',') != std::string::npos)
      return failed(ReplyError::InvalidFields);
    if (candidate.size() > max_bytes ||
        payload.size() + candidate.size() + 1 > max_bytes)
      return failed(ReplyError::TooLong);
    if (index)
      payload += ',';
    payload += candidate;
  }
  payload += '\t';
  payload += std::to_string(highlighted);
  return text_reply(request, FanyImeReplyType::UiLessComposition, payload);
}
} // namespace msime::windows
