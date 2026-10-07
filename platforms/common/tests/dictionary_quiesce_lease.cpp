#include "../DictionaryQuiesceLease.h"

#include <cassert>
#include <cstdlib>
#include <filesystem>
#include <fstream>
#include <iterator>
#include <sys/stat.h>
#include <unistd.h>

// The lease every desktop input host reads: built by both the Linux and the macOS test suites.
int main() {
  using msime::dictionary_lease::dictionary_quiesce_lease_live;
  using msime::dictionary_lease::dictionary_quiesced;
  using msime::dictionary_lease::raise_dictionary_quiesce_lease;

  assert(dictionary_quiesce_lease_live("1010000", 1000000));
  assert(dictionary_quiesce_lease_live("1010000\n", 1000000));
  assert(dictionary_quiesce_lease_live("1030000", 1000000));
  // Expired, or further out than any real lease (a clock jump or a stray file), is ignored.
  assert(!dictionary_quiesce_lease_live("1000000", 1000000));
  assert(!dictionary_quiesce_lease_live("999999", 1000000));
  assert(!dictionary_quiesce_lease_live("1030001", 1000000));
  assert(!dictionary_quiesce_lease_live("", 1000000));
  assert(!dictionary_quiesce_lease_live("10x0000", 1000000));
  assert(!dictionary_quiesce_lease_live("-1010000", 1000000));
  assert(!dictionary_quiesce_lease_live("99999999999999999999999", 1000000));

  char pattern[] = "/tmp/msime-quiesce-XXXXXX";
  const std::filesystem::path root = mkdtemp(pattern);
  assert(!dictionary_quiesced(root.string(), 1000000));
  assert(!dictionary_quiesced("", 1000000));
  assert(!dictionary_quiesced("relative", 1000000));

  // 临时文件名可预测时，不能跟随预先放置的符号链接写到外部文件。
  char staged_outside_pattern[] = "/tmp/msime-quiesce-staged-outside-XXXXXX";
  const std::filesystem::path staged_outside = mkdtemp(staged_outside_pattern);
  const auto staged_target = staged_outside / "target";
  std::ofstream(staged_target) << "keep";
  const auto staged = root / (".msime-dictionary-quiesce." + std::to_string(getpid()) + "-0");
  std::filesystem::create_symlink(staged_target, staged);
  std::string staged_written;
  assert(!raise_dictionary_quiesce_lease(root.string(), staged_written, 1000000));
  {
    std::ifstream preserved(staged_target);
    assert(std::string(std::istreambuf_iterator<char>(preserved), std::istreambuf_iterator<char>()) == "keep");
  }
  std::filesystem::remove(staged);
  std::filesystem::remove_all(staged_outside);

  {
    std::ofstream(root / ".msime-dictionary-quiesce") << "1005000";
  }
  assert(dictionary_quiesced(root.string(), 1000000));
  assert(dictionary_quiesced(root.string() + "/", 1000000));
  assert(!dictionary_quiesced(root.string(), 1006000));
  // A lease still being staged is not the lease; the rename is what raises it.
  std::filesystem::remove(root / ".msime-dictionary-quiesce");
  {
    std::ofstream(root / ".msime-dictionary-quiesce.4242") << "1030000\n";
  }
  assert(!dictionary_quiesced(root.string(), 1000000));
  std::filesystem::remove(root / ".msime-dictionary-quiesce.4242");
  // A planted FIFO must be rejected without blocking the input host's timer.
  assert(::mkfifo((root / ".msime-dictionary-quiesce").c_str(), 0600) == 0);
  assert(!dictionary_quiesced(root.string(), 1000000));
  std::filesystem::remove(root / ".msime-dictionary-quiesce");
  // A host raising the lease itself leaves exactly the lease behind, live for the bound, with its owner line after the expiry, and lowering it clears it.
  using msime::dictionary_lease::lower_dictionary_quiesce_lease;
  using msime::dictionary_lease::raise_dictionary_quiesce_lease;
  std::string written;
  assert(raise_dictionary_quiesce_lease(root.string(), written, 1000000));
  assert(written.rfind("1030000\n", 0) == 0 && written.size() > 8 && written.back() == '\n');
  {
    std::ifstream lease(root / ".msime-dictionary-quiesce");
    assert(std::string(std::istreambuf_iterator<char>(lease), std::istreambuf_iterator<char>()) == written);
  }
  assert(dictionary_quiesced(root.string(), 1000000));
  assert(dictionary_quiesced(root.string(), 1029999));
  assert(!dictionary_quiesced(root.string(), 1030000));
  assert(std::distance(std::filesystem::directory_iterator(root), std::filesystem::directory_iterator()) == 2);
  lower_dictionary_quiesce_lease(root.string(), written);
  assert(!dictionary_quiesced(root.string(), 1000000));
  assert(std::filesystem::exists(root / ".msime-dictionary-quiesce.lock"));
  assert(std::distance(std::filesystem::directory_iterator(root), std::filesystem::directory_iterator()) == 1);
  // Two raises in one process are told apart.
  std::string second;
  assert(raise_dictionary_quiesce_lease(root.string(), written, 1000000));
  assert(raise_dictionary_quiesce_lease(root.string(), second, 1000000));
  assert(second != written);
  // A lease another writer has put up since is theirs: lowering ours leaves it in place.
  lower_dictionary_quiesce_lease(root.string(), written);
  assert(dictionary_quiesced(root.string(), 1000000));
  {
    std::ofstream(root / ".msime-dictionary-quiesce", std::ios::trunc) << "1030000\n999 0\n";
  }
  lower_dictionary_quiesce_lease(root.string(), second);
  assert(dictionary_quiesced(root.string(), 1000000));
  {
    std::ofstream oversized(root / ".msime-dictionary-quiesce", std::ios::trunc);
    oversized << std::string(msime::dictionary_lease::kDictionaryQuiesceLeaseMaxBytes + 1, 'x');
  }
  lower_dictionary_quiesce_lease(root.string(), second);
  assert(std::filesystem::exists(root / ".msime-dictionary-quiesce"));
  std::filesystem::remove(root / ".msime-dictionary-quiesce");
  assert(!raise_dictionary_quiesce_lease("relative", written, 1000000));
  assert(!raise_dictionary_quiesce_lease((root / "missing").string(), written, 1000000));

  char outside_pattern[] = "/tmp/msime-quiesce-outside-XXXXXX";
  const std::filesystem::path outside = mkdtemp(outside_pattern);
  char link_pattern[] = "/tmp/msime-quiesce-link-XXXXXX";
  const std::filesystem::path link_parent = mkdtemp(link_pattern);
  const auto linked = link_parent / "linked-user-data";
  std::filesystem::create_directory_symlink(outside, linked);
  assert(!raise_dictionary_quiesce_lease(linked.string(), written, 1000000));
  assert(!dictionary_quiesced(linked.string(), 1000000));
  lower_dictionary_quiesce_lease(linked.string(), written);
  assert(std::filesystem::is_empty(outside));
  std::filesystem::remove(linked);
  std::filesystem::remove_all(link_parent);
  std::filesystem::remove_all(outside);

  char existing_outside_pattern[] = "/tmp/msime-quiesce-existing-XXXXXX";
  const std::filesystem::path existing_outside = mkdtemp(existing_outside_pattern);
  const auto existing_data = existing_outside / "data";
  std::filesystem::create_directory(existing_data);
  std::ofstream(existing_data / ".msime-dictionary-quiesce") << "1005000\n";
  char existing_link_pattern[] = "/tmp/msime-quiesce-existing-link-XXXXXX";
  const std::filesystem::path existing_link_parent = mkdtemp(existing_link_pattern);
  const auto existing_link = existing_link_parent / "linked";
  std::filesystem::create_directory_symlink(existing_outside, existing_link);
  const auto existing_user_data = existing_link / "data";
  assert(!raise_dictionary_quiesce_lease(existing_user_data.string(), written, 1000000));
  assert(!dictionary_quiesced(existing_user_data.string(), 1000000));
  std::filesystem::remove_all(existing_link_parent);
  std::filesystem::remove_all(existing_outside);
  std::filesystem::remove_all(root);
  return 0;
}
