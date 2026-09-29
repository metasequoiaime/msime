#include "ClipboardHistory.h"
#include <cassert>
#include <filesystem>
#include <fstream>
#include <stdexcept>
#include <string>
namespace {
void require(bool value, int line) {
  if (!value)
    throw std::runtime_error("clipboard history contract failed at line " + std::to_string(line));
}
#define REQUIRE(value) require((value), __LINE__)
}
int main() {
  const auto directory = std::filesystem::temp_directory_path() / "msime-clipboard-history-test";
  const auto path = directory / "history.json";
  std::error_code error; std::filesystem::remove_all(directory, error);
  std::filesystem::create_directory(directory);
  msime::windows::ClipboardHistory history(path);

  // Newlines and whitespace are user content and survive the round trip
  // exactly: `normalize_clipboard_text` removes only what CF_UNICODETEXT adds.
  // Collapsing CRLF or trimming the ends would hand back something the user
  // did not copy.
  const std::string copied = " first\r\nsecond \t\n";
  REQUIRE(history.add(copied));
  REQUIRE(history.load().front() == copied);

  // Deduplication is on the normalised form, so a capture that differs only in
  // what normalisation removes is the same record.
  REQUIRE(!history.add(copied + "\r"));
  REQUIRE(history.load().size() == 1);
  // Differing anywhere normalisation preserves makes it a different record.
  REQUIRE(history.add(" first\nsecond"));
  REQUIRE(history.load().size() == 2);
  REQUIRE(history.remove(" first\nsecond"));

  REQUIRE(history.add("second"));
  REQUIRE(history.remove("second"));
  REQUIRE(history.remove(copied + "\r\r"));
  REQUIRE(history.load().empty());
  // Nothing is left of a capture that is only a terminator, so there is
  // nothing to add or remove.
  REQUIRE(!history.add("\r"));
  REQUIRE(!history.remove("\r"));
  REQUIRE(history.clear());
  REQUIRE(history.load().empty());

  // CF_UNICODETEXT is NUL-terminated and the source builds a wide string from
  // that pointer, so an embedded NUL ends the captured value rather than being
  // dropped out of the middle of it.
  std::string terminated(5000, 'x');
  terminated.insert(17, 1, '\0');
  REQUIRE(history.add(terminated));
  const auto truncated = history.load().front();
  REQUIRE(truncated == std::string(17, 'x'));
  REQUIRE(history.clear());

  // The cap is in UTF-16 units, which is what the editors receiving this text
  // count in.
  REQUIRE(history.add(std::string(5000, 'x')));
  const auto capped = history.load().front();
  REQUIRE(capped.size() == msime::windows::ClipboardHistory::max_chars);
  REQUIRE(capped.find('\0') == std::string::npos && capped.back() == 'x');
  history.clear();

  for (size_t index = 0; index < msime::windows::ClipboardHistory::max_items + 10; ++index)
    REQUIRE(history.add("item-" + std::to_string(index)));
  REQUIRE(history.load().size() == msime::windows::ClipboardHistory::max_items);

  // Records that normalise to nothing must not consume capacity or hide the
  // entries after them. A lone carriage return is the whole of such a record.
  {
    std::ofstream output(path, std::ios::trunc);
    output << "[";
    for (size_t index = 0; index < msime::windows::ClipboardHistory::max_items; ++index)
      output << "\"\\r\",";
    for (size_t index = 0; index < msime::windows::ClipboardHistory::max_items + 1; ++index) {
      if (index) output << ",";
      output << "\"synthetic-" << index << "\"";
    }
    output << "]";
  }
  const auto filtered = history.load();
  REQUIRE(filtered.size() == msime::windows::ClipboardHistory::max_items);
  REQUIRE(filtered.front() == "synthetic-0");
  REQUIRE(filtered.back() == "synthetic-" + std::to_string(msime::windows::ClipboardHistory::max_items - 1));

  { std::ofstream output(path, std::ios::trunc); output << "not-json"; }
  REQUIRE(history.load().empty());

  // A damaged or attacker-controlled store must not make the input method
  // read and allocate an unbounded JSON document before it can reject it.
  {
    std::ofstream output(path, std::ios::trunc);
    output << "[\"" << std::string(1024 * 1024, 'x') << "\"]";
  }
  REQUIRE(history.load().empty());

  std::filesystem::remove_all(directory, error);
}
