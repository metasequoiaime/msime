#include "EngineSessionAdapter.h"
#include <nlohmann/json.hpp>
namespace msime::tsf {
using json = nlohmann::json;
bool EngineSessionAdapter::parse_result(const std::string &text,
                                        EngineResult *out,
                                        std::string *error) {
  try {
    const auto root = json::parse(text);
    if (!root.value("ok", false)) {
      if (error) *error = "Engine request failed";
      return false;
    }
    const auto &value = root.at("value");
    EngineResult parsed;
    parsed.handled = value.value("handled", false);
    parsed.has_commit = value.value("has_commit", value.contains("commit") && !value.at("commit").is_null());
    if (value.contains("commit") && !value.at("commit").is_null())
      parsed.commit = value.at("commit").get<std::string>();
    if (value.contains("diagnostic") && !value.at("diagnostic").is_null())
      parsed.diagnostic = value.at("diagnostic").get<std::string>();
    if (value.contains("view") || value.contains("preedit")) {
      const auto &view = value.contains("view") ? value.at("view") : value;
      parsed.view.session = view.value("session", uint64_t{0});
      parsed.view.preedit = view.value("preedit", "");
      parsed.view.editing_text = view.value("editing_text", "");
      parsed.view.reading = view.value("reading", "");
      parsed.view.generation = view.value("generation", uint64_t{0});
      parsed.view.caret = view.value("caret_position", std::size_t{0});
      parsed.view.scheme = view.value("scheme", uint32_t{0});
      parsed.view.spelling_symbols = view.value("spelling_symbols", "");
      for (const auto &candidate : view.value("candidates", json::array())) {
        std::string id;
        std::size_t index = candidate.value("index", std::size_t{0});
        if (candidate.contains("id")) {
          const auto &raw_id = candidate.at("id");
          if (raw_id.is_object()) index = raw_id.value("index", index);
          id = raw_id.is_string() ? raw_id.get<std::string>() : raw_id.dump();
          if (parsed.view.generation == 0 && raw_id.is_object())
            parsed.view.generation = raw_id.value("generation", uint64_t{0});
        }
        parsed.view.candidates.push_back({std::move(id), candidate.value("text", ""),
                                          candidate.value("highlighted", false), index});
      }
    }
    if (out) *out = std::move(parsed);
    return true;
  } catch (...) {
    if (error) *error = "Invalid Engine response";
    return false;
  }
}
} // namespace msime::tsf
