#pragma once
#include "msime_client.h"
#include <cstddef>
#include <cstdint>
#include <string>
#include <vector>
namespace msime::tsf {
struct EngineCandidate {
  std::string id;
  std::string text;
  bool highlighted = false;
  std::size_t index = 0;
};
struct EngineView {
  uint64_t session = 0;
  std::string preedit;
  std::string editing_text;
  // The kana a Japanese composition converts to; empty for every other scheme. The composition
  // shows this rather than the letters - see shared/input/CompositionDisplay.h.
  std::string reading;
  std::vector<EngineCandidate> candidates;
  uint64_t generation = 0;
  std::size_t caret = 0;
  // The Engine's SchemeType ordinal; the numbers are named in common/InputSchemeTraits.h.
  uint32_t scheme = 0;
  // The non-letter keys that spell the composition rather than end it (Zhuyin's digit and punctuation keys, VNI's tone digits); empty when every such key is punctuation.
  std::string spelling_symbols;
};
struct EngineResult {
  bool handled = false;
  bool has_commit = false;
  std::string commit;
  std::string diagnostic;
  EngineView view;
};
class EngineSessionAdapter final {
public:
  static bool parse_result(const std::string &, EngineResult *, std::string *);
  ~EngineSessionAdapter();
  bool create(const std::string &, std::string *);
  void destroy() noexcept;
  bool valid() const noexcept { return session_ != 0; }
  bool character(uint8_t, bool, std::string *, std::string *);
  bool command(uint32_t, std::string *, std::string *);
  bool select(uint64_t, std::size_t, std::string *, std::string *);
  bool select_edge(uint64_t, std::size_t, uint8_t, std::string *, std::string *);
  bool view(std::string *, std::string *) const;
  bool punctuation(uint8_t, std::string *, std::string *);
  bool focus(bool, std::string *, std::string *);
  bool chinese_punctuation(bool, std::string *, std::string *);
  bool character_width(bool, std::string *, std::string *);
  bool english_mode(bool, std::string *, std::string *);
  bool dedicated_english(bool, std::string *, std::string *);
  bool paired_punctuation(bool, std::string *, std::string *);
  bool punctuation_lock(uint8_t, std::string *, std::string *);
  bool update_preferences(const std::string &, std::string *, std::string *);
  // Loads the atomically published, validated snapshot without synthesizing
  // defaults. A busy store is reported as false and leaves the session intact.
  bool reload_preferences(const std::string &, std::string *, std::string *);
private:
  bool response(char *, std::string *, std::string *) const;
  uint64_t session_ = 0;
};
}
