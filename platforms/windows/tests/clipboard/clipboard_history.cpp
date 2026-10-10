#include "ClipboardHistory.h"
#include "../core/MaterializedSymlink.h"
#include <cassert>
#include <chrono>
#include <filesystem>
#include <fstream>
#include <iterator>
#include <stdexcept>
#include <string>
#include <thread>
#ifdef _WIN32
#include <windows.h>
#endif
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

  // Native callers can hand us a byte string that is not valid UTF-8. It must
  // be rejected before nlohmann::json serialisation, which otherwise throws
  // an uncaught exception and takes the Server down.
  const std::string invalid_utf8("valid\xFF", 6);
  REQUIRE(msime::windows::normalize_clipboard_text(invalid_utf8).empty());
  REQUIRE(!history.add(invalid_utf8));
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

  {
    std::ofstream output(path, std::ios::trunc);
    output << "[\"synthetic-capacity\"]";
  }
  const auto reserved = history.load();
  REQUIRE(reserved.size() == 1);
  REQUIRE(reserved.capacity() >= msime::windows::ClipboardHistory::max_items);

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

#ifdef _WIN32
  // A replaced state subdirectory must not redirect the lock, temporary
  // archive, or final store outside the state root.
  const auto outside_directory =
      directory.parent_path() /
      ("msime-clipboard-history-outside-" +
       std::to_string(GetCurrentProcessId()) + "-" +
       std::to_string(GetTickCount64()));
  REQUIRE(std::filesystem::create_directory(outside_directory));
  const auto linked_directory = directory / "linked";
  if (msime::windows::tests::create_materialized_symlink(
          linked_directory, outside_directory, SYMBOLIC_LINK_FLAG_DIRECTORY)) {
    msime::windows::ClipboardHistory linked_history(
        linked_directory / "history.json");
    REQUIRE(!linked_history.add("synthetic-reparse"));
    REQUIRE(!std::filesystem::exists(outside_directory / "history.json"));
    std::filesystem::remove(linked_directory, error);
  }

  // The final store file is untrusted too. A leaf reparse point must not make
  // history load or mutate a file outside the state directory.
  const auto outside_store = outside_directory / "outside-history.json";
  const auto linked_store = directory / "linked-history.json";
  {
    std::ofstream output(outside_store, std::ios::trunc);
    output << "[\"synthetic-outside\"]";
  }
  if (msime::windows::tests::create_materialized_symlink(
          linked_store, outside_store, 0)) {
    msime::windows::ClipboardHistory linked_history(linked_store);
    REQUIRE(linked_history.load().empty());
    REQUIRE(!linked_history.add("synthetic-reparse"));
    REQUIRE(!linked_history.remove("synthetic-outside"));
    REQUIRE(linked_history.clear());
    std::ifstream input(outside_store);
    const std::string outside_payload((std::istreambuf_iterator<char>(input)),
                                      std::istreambuf_iterator<char>());
    REQUIRE(outside_payload == "[\"synthetic-outside\"]");
  }
  std::filesystem::remove(outside_store, error);
  std::filesystem::remove_all(outside_directory, error);

  // A held history lock must not be deleted and recreated around another
  // writer. FILE_SHARE_DELETE would let a second process unlink this file
  // while the first process still owns the byte-range lock.
  {
    auto lock_path = path;
    lock_path += ".lock";
    HANDLE blocker = CreateFileW(
        lock_path.c_str(), GENERIC_READ | GENERIC_WRITE,
        FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE, nullptr,
        OPEN_ALWAYS, FILE_ATTRIBUTE_NORMAL | FILE_FLAG_OPEN_REPARSE_POINT,
        nullptr);
    REQUIRE(blocker != INVALID_HANDLE_VALUE);
    OVERLAPPED offset{};
    REQUIRE(LockFileEx(blocker, LOCKFILE_EXCLUSIVE_LOCK, 0, MAXDWORD, MAXDWORD,
                       &offset));

    std::thread writer([&] {
      (void)history.add("synthetic-lock-delete");
    });
    bool writer_handle_opened = false;
    for (size_t attempt = 0; attempt < 5000; ++attempt) {
      HANDLE probe = CreateFileW(
          lock_path.c_str(), DELETE, FILE_SHARE_READ | FILE_SHARE_WRITE,
          nullptr, OPEN_EXISTING,
          FILE_ATTRIBUTE_NORMAL | FILE_FLAG_OPEN_REPARSE_POINT, nullptr);
      if (probe == INVALID_HANDLE_VALUE) {
        if (GetLastError() == ERROR_SHARING_VIOLATION) {
          writer_handle_opened = true;
          break;
        }
      } else {
        CloseHandle(probe);
      }
      std::this_thread::sleep_for(std::chrono::milliseconds(1));
    }
    const bool delete_blocked =
        writer_handle_opened && !DeleteFileW(lock_path.c_str());

    UnlockFileEx(blocker, 0, MAXDWORD, MAXDWORD, &offset);
    CloseHandle(blocker);
    writer.join();
    REQUIRE(delete_blocked);
    std::filesystem::remove(path, error);
    std::filesystem::remove(lock_path, error);
  }
#endif

  std::filesystem::remove_all(directory, error);
}
