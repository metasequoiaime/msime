#pragma once

#include <cstddef>
#include <cstdint>
#include <string>

#include <nlohmann/json.hpp>

#include "msime_client.h"
#include "../../../common/HostApiString.h"
#include "VoiceProviderOptions.h"

namespace msime::linux_host {

// The user's own dictionary words as hotwords for on-device recognition, from msime_client_voice_hotwords. `host_options` is the HostOptions document the host opens its sessions with. The voice service cannot build that document (it knows neither the dictionary paths nor the preferences), so the host asks here and sends the words with the recording. It reads the dictionary store, so it runs on the voice worker thread. Recognition works without hotwords, so every failure, including a dictionary held by maintenance, gives an empty array.
inline nlohmann::json voice_hotwords(const nlohmann::json &host_options, std::size_t limit = 200) {
  const auto empty = nlohmann::json::array();
  if (!host_options.is_object())
    return empty;
  const auto request = nlohmann::json{{"options", host_options}, {"limit", limit}}.dump();
  if (request.size() > 65536)
    return empty;
  auto raw = msime::host_api::own_string(
      msime_client_voice_hotwords(reinterpret_cast<const uint8_t *>(request.data()), request.size()));
  if (!raw)
    return empty;
  const auto document = nlohmann::json::parse(raw.get(), nullptr, false);
  if (!document.is_object() || !document.value("ok", false))
    return empty;
  const auto value = document.find("value");
  if (value == document.end() || !value->is_object())
    return empty;
  const auto hotwords = value->find("hotwords");
  return hotwords != value->end() && hotwords->is_array() ? *hotwords : empty;
}

// The query a host sends for one recording: language, generation and provider options, plus the dictionary hotwords when the recording uses on-device recognition. `host_options` may be null for a host without a session document; the recording then goes without hotwords.
inline nlohmann::json voice_query(const std::string &language, std::uint64_t generation,
                                  const nlohmann::json &provider_options, const nlohmann::json &host_options) {
  auto query = nlohmann::json{{"language", language}, {"generation", generation}, {"options", provider_options}};
  if (voice_wants_hotwords(provider_options))
    add_voice_hotwords(query, voice_hotwords(host_options));
  return query;
}

}  // namespace msime::linux_host
