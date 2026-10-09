#include <fcntl.h>
#include <unistd.h>

#include <cassert>
#include <cerrno>
#include <cstdio>
#include <filesystem>
#include <fstream>
#include <iterator>
#include <string>
#include <string_view>
#include <system_error>
#include <vector>

#include "../../src/core/SafePath.h"

static int write_calls = 0;

ssize_t write_interrupted_once(int descriptor, const void *bytes, size_t size) {
  ++write_calls;
  if (write_calls == 1) {
    errno = EINTR;
    return -1;
  }
  return ::write(descriptor, bytes, size);
}

#define write write_interrupted_once
#include "../../src/candidates/AtomicWrite.h"
#undef write

int main() {
  const auto root = std::filesystem::temp_directory_path() /
                    ("msime-candidate-write-eintr-" + std::to_string(::getpid()));
  std::filesystem::remove_all(root);
  std::filesystem::create_directories(root);
  const auto file = root / "candidate-panel.json";
  assert(msime::linux_host::write_candidate_file_atomically(file, "synthetic status"));
  assert(write_calls == 2);
  std::ifstream input(file, std::ios::binary);
  assert(std::string(std::istreambuf_iterator<char>(input), {}) == "synthetic status");
  std::filesystem::remove_all(root);
}
