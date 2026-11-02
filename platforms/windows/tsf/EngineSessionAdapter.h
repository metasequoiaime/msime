#pragma once
#include "msime_client.h"
#include <memory>
#include <optional>
namespace msime::windows { class HostOnlineCandidateSync; }
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
  // 引擎的 `CandidateSource`（`common/CandidateSources.h`）。占位席位要靠它认出来：它们占着一格和一个编号却选不中。
  unsigned source = 0;
};
struct EngineView {
  uint64_t session = 0;
  std::string preedit;
  std::string editing_text;
  // Runtime 暂存的已选词段，显示在剩余拼写前面，不是一次提交。
  std::string phrase_prefix;
  // The kana a Japanese composition converts to; empty for every other scheme. The composition
  // shows this rather than the letters - see shared/input/CompositionDisplay.h.
  std::string reading;
  std::vector<EngineCandidate> candidates;
  uint64_t generation = 0;
  std::size_t caret = 0;
  // The Engine's SchemeType ordinal; the numbers are named in common/InputSchemeTraits.h.
  uint32_t scheme = 0;
  std::string key_grid = "none";
  // The non-letter keys that spell the composition rather than end it (Zhuyin's digit and punctuation keys, VNI's tone digits); empty when every such key is punctuation.
  std::string spelling_symbols;
  // 本地模式名（"none" 是普通组字）和引擎自己的英文模式，决定整句改字的方向键是否适用（HostEditsSentence）。
  std::string local_mode = "none";
  bool dedicated_english = false;
  // 整句改字时改好的整句（UTF-8），否则为空；光标在第 conversion_focus_start 个字之前，conversion_focus_start..conversion_focus_end 是候选所替换的那一段，都按 Unicode 标量计（msime_client.h）。
  std::string conversion;
  std::size_t conversion_focus_start = 0;
  std::size_t conversion_focus_end = 0;
  // 上屏后联想（msime_client.h 的 View.prediction）：没有组字，候选是建议的后续文字，点选、数字键或空格才上屏。普通状态的 view 不带这个字段，按 false 读。
  bool prediction = false;
};
struct EngineResult {
  bool handled = false;
  bool has_commit = false;
  std::string commit;
  // 只属于本次提交；空字符串表示显式位于正文末尾。
  std::optional<std::string> commit_cursor_suffix;
  std::string diagnostic;
  EngineView view;
};
class EngineSessionAdapter final {
public:
  EngineSessionAdapter();
  static bool parse_result(const std::string &, EngineResult *, std::string *);
  ~EngineSessionAdapter();
  bool create(const std::string &, std::string *);
  void destroy() noexcept;
  bool valid() const noexcept { return session_ != 0; }
  bool character(uint8_t, bool, std::string *, std::string *);
  bool panel_character(uint8_t, bool, bool, std::string *, std::string *);
  bool command(uint32_t, std::string *, std::string *);
  bool select(uint64_t, std::size_t, std::string *, std::string *);
  bool select_edge(uint64_t, std::size_t, uint8_t, std::string *, std::string *);
  // msime_client_all_candidates 与 msime_client_select_any_candidate：续接 Server 卷轴展开网格里的候选时，这个候选不在本会话的当前页上，要到这一代的完整列表里找、按全局下标选（HostCandidateContinuation.h）。
  bool all_candidates(std::string *, std::string *);
  bool select_any(uint64_t, std::size_t, std::string *, std::string *);
  bool view(std::string *, std::string *) const;
  bool online_query(std::string *, std::string *) const;
  bool apply_cloud(const std::string &, const std::string &, std::string *, std::string *);
  bool apply_ai(const std::string &, const std::string &, std::string *, std::string *);
  bool resolve_ai(const std::string &, std::string *, std::string *);
  bool rerank_settled(std::string *, std::string *);
  void capture_online_request(uint64_t request);
  bool apply_online_update(const std::string &payload);
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
  std::unique_ptr<msime::windows::HostOnlineCandidateSync> online_;
};
}
