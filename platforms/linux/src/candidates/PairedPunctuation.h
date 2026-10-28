#pragma once

#include <cstddef>
#include <cstdint>
#include <cctype>
#include <optional>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

namespace msime::linux_host {

// Shift is intentionally absent from the rejected mask: on common layouts it
// is part of the key stroke for ), }, >, and quotes. The other modifiers must
// not turn a caret-over-existing-pair shortcut into an IME action.
enum class PairedPunctuationModifier : std::uint32_t {
  Control = 1u << 0,
  Alt = 1u << 1,
  Super = 1u << 2,
  Meta = 1u << 3,
  Hyper = 1u << 4,
  Mod5 = 1u << 5,
  Shift = 1u << 6,
};

constexpr bool paired_closing_modifiers_allowed(std::uint32_t modifiers) {
  constexpr auto disallowed =
      static_cast<std::uint32_t>(PairedPunctuationModifier::Control) |
      static_cast<std::uint32_t>(PairedPunctuationModifier::Alt) |
      static_cast<std::uint32_t>(PairedPunctuationModifier::Super) |
      static_cast<std::uint32_t>(PairedPunctuationModifier::Meta) |
      static_cast<std::uint32_t>(PairedPunctuationModifier::Hyper) |
      static_cast<std::uint32_t>(PairedPunctuationModifier::Mod5);
  return (modifiers & disallowed) == 0;
}

// Keep the same document invariant as the native Windows host: a closing mark
// may be skipped only while it is immediately to the right of the caret. The
// bound prevents swallowed keys from growing this state without limit.
class PairedPunctuationTracker {
 public:
  static constexpr std::size_t kMaxDepth = 16;

  void clear() { closings_.clear(); }
  bool empty() const { return closings_.empty(); }
  std::size_t size() const { return closings_.size(); }

  void push(std::string closing) {
    if (closing.empty()) return;
    if (closings_.size() >= kMaxDepth) closings_.erase(closings_.begin());
    closings_.push_back(std::move(closing));
  }

  bool matches(std::string_view closing) const {
    return !closings_.empty() && closings_.back() == closing;
  }

  // A failed document check invalidates the complete stack because the caret
  // has moved outside the sequence that was being tracked.
  bool consume(std::string_view closing, std::string_view following,
               bool following_known, bool modifiers_allowed = true) {
    if (!modifiers_allowed) return false;
    if (!matches(closing)) {
      if (!closings_.empty()) clear();
      return false;
    }
    if (!following_known || following != closing) {
      clear();
      return false;
    }
    closings_.pop_back();
    return true;
  }

 private:
  std::vector<std::string> closings_;
};

// 这个键能跨过的后半个。引号和方头括号有两组字（quote_style、bracket_style），样式可能在一对还开着时换掉，所以按 tracker 栈顶挑：栈顶是其中之一就给它，否则给缺省那个，consume 会照旧清空栈。
inline std::optional<std::string> paired_closing_for_key(
    char key, bool fullwidth, const PairedPunctuationTracker &tracker) {
  const auto either = [&tracker](const char *first,
                                 const char *second) -> std::string {
    return tracker.matches(second) ? second : first;
  };
  switch (key) {
    case '"': return either("”", "』");
    case '\'': return either("’", "」");
    case ')': return std::string("）");
    case ']': return either("】", "〗");
    // 中文标点下 `{` 出「、补」（契约第 5 版），`}` 跨过」；引擎写 ASCII 标点时（专用英文）`{` 仍补 `{}`/`｛｝`，那时跨过的是花括号。
    case '}':
      if (tracker.matches("」")) return std::string("」");
      return fullwidth ? std::string("｝") : std::string("}");
    case '>': return std::string("〉");
    default: return std::nullopt;
  }
}

inline const char *paired_punctuation_closing(std::string_view text) {
  for (const auto &[opening, closing] : {
           std::pair<std::string_view, const char *> {"（", "）"},
           {"【", "】"},
           {"〖", "〗"},
           {"《", "》"},
           {"〈", "〉"}}) {
    if (text.size() >= opening.size() &&
        text.compare(text.size() - opening.size(), opening.size(), opening) == 0)
      return closing;
  }
  return nullptr;
}
// 宿主怎样补全引擎为成对键写出的前半个：方括号、圆括号和书名号补后半个；`{` 在中文标点下出「（契约第 5 版）、补」，引擎写的是 ASCII 花括号时（专用英文）补 `}`；引号键不论引擎出了哪一半，都补成完整的一对。两个 Linux 宿主都用它，再把光标退回两半之间。
enum class PunctuationPairMode {
  Unpaired,
  Bracket,
  Brace,
  DoubleQuote,
  SingleQuote
};
inline bool normalize_punctuation_pair(std::string &text, PunctuationPairMode mode) {
  if (mode == PunctuationPairMode::Unpaired)
    return false;
  if (mode == PunctuationPairMode::Brace) {
    if (text.size() >= 3 && text.compare(text.size() - 3, 3, "「") == 0) {
      text += "」";
      return true;
    }
    if (!text.empty() && text.back() == '{') {
      text.push_back('}');
      return true;
    }
    // The Fcitx5 host receives the mark after Engine applied full width.
    if (text.size() >= 3 && text.compare(text.size() - 3, 3, "｛") == 0) {
      text += "｝";
      return true;
    }
    return false;
  }
  if (mode == PunctuationPairMode::Bracket) {
    if (const auto *closing = paired_punctuation_closing(text)) {
      text += closing;
      return true;
    }
    return false;
  }
  // 引擎按样式写出弯引号或直角引号的某一半；文本末尾是哪一组就补哪一组。
  const bool double_quote = mode == PunctuationPairMode::DoubleQuote;
  const std::pair<std::string_view, std::string_view> groups[] = {
      double_quote ? std::pair<std::string_view, std::string_view>{"“", "”"}
                   : std::pair<std::string_view, std::string_view>{"‘", "’"},
      double_quote ? std::pair<std::string_view, std::string_view>{"『", "』"}
                   : std::pair<std::string_view, std::string_view>{"「", "」"}};
  for (const auto &[opening, closing] : groups) {
    for (const auto suffix : {opening, closing}) {
      if (text.size() < suffix.size() ||
          text.compare(text.size() - suffix.size(), suffix.size(), suffix) != 0)
        continue;
      text.erase(text.size() - suffix.size());
      text += opening;
      text += closing;
      return true;
    }
  }
  return false;
}
inline std::optional<std::string> paired_closing_from_text(std::string_view text) {
  for (const auto closing : {std::string_view("）"), std::string_view("】"),
                             std::string_view("》"), std::string_view("〉"),
                             std::string_view("｝"), std::string_view("}"),
                             std::string_view("”"), std::string_view("’"),
                             std::string_view("〗"), std::string_view("』"),
                             std::string_view("」")}) {
    if (text.size() >= closing.size() &&
        text.compare(text.size() - closing.size(), closing.size(), closing) ==
            0)
      return std::string(closing);
  }
  return std::nullopt;
}
// Spreadsheet cells on Linux cannot reliably preserve the caret move used by
// paired punctuation.  IBus supplies the focused client name, so keep the
// exclusion narrow to applications whose executable identifies as a
// spreadsheet.  LibreOffice's shared soffice.bin name is intentionally not
// included because it also hosts Writer and other editors.
inline bool paired_punctuation_excluded_client(std::string_view client) {
  const auto slash = client.find_last_of("/\\");
  client = client.substr(slash == std::string_view::npos ? 0 : slash + 1);
  std::string normalized;
  normalized.reserve(client.size());
  for (const unsigned char value : client)
    normalized.push_back(static_cast<char>(std::tolower(value)));
  return normalized == "scalc" || normalized == "scalc.bin" ||
         normalized == "libreoffice-calc" || normalized == "gnumeric" ||
         normalized == "org.gnome.gnumeric" ||
         normalized == "calligrasheets" ||
         normalized == "org.kde.calligrasheets";
}

}  // namespace msime::linux_host
