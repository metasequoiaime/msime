#pragma once
#include <nlohmann/json.hpp>

#include <string>
#include <string_view>

namespace msime::windows {
// Provider text is shown in the overlay and may be committed to an editor. Keep
// control characters that are meaningful in prose, but reject NUL and the other
// ISO C0/DEL controls before they cross the host boundary.
inline bool valid_doubao_text(std::string_view text) {
  for (size_t index = 0; index < text.size(); ++index) {
    const auto byte = static_cast<unsigned char>(text[index]);
    if (byte == 0x7f || (byte < 0x20 && byte != '\t' && byte != '\n' && byte != '\r'))
      return false;
    // UTF-8 encodes the ISO C1 controls U+0080..U+009F as C2 80..9F.
    if (byte == 0xc2 && index + 1 < text.size() &&
        static_cast<unsigned char>(text[index + 1]) >= 0x80 &&
        static_cast<unsigned char>(text[index + 1]) <= 0x9f)
      return false;
  }
  return true;
}

// The transcript in one Doubao response body. bigmodel_async returns "result" as an object; bigmodel_nostream documents it as a list of segments, whose texts are joined in order. MSIME-Windows doubao_asr_client.cpp ExtractTranscript accepts either shape so one parser covers both endpoints.
inline std::string doubao_body_transcript(const nlohmann::json &body) {
  if (!body.is_object() || !body.contains("result"))
    return {};
  const auto &result = body["result"];
  if (result.is_object()) {
    const auto text = result.find("text");
    if (text == result.end() || !text->is_string())
      return {};
    auto value = text->get<std::string>();
    return valid_doubao_text(value) ? value : std::string();
  }
  if (!result.is_array())
    return {};
  std::string text;
  for (const auto &segment : result) {
    if (!segment.is_object())
      continue;
    const auto part = segment.find("text");
    if (part != segment.end() && part->is_string()) {
      auto value = part->get<std::string>();
      if (!valid_doubao_text(value))
        return {};
      text += value;
    }
  }
  return text;
}

// A decoded response message: the transcript at the top level, else inside the legacy "payload_msg" envelope.
inline std::string doubao_transcript(const nlohmann::json &message) {
  auto text = doubao_body_transcript(message);
  if (text.empty() && message.is_object() && message.contains("payload_msg"))
    text = doubao_body_transcript(message["payload_msg"]);
  return text;
}
} // namespace msime::windows
