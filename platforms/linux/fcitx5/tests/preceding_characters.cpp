#include "../PrecedingCharacters.h"

#include <cstdio>
#include <cstddef>
#include <memory>
#include <stdexcept>
#include <string>
#include <vector>

template <typename T>
struct CountingAllocator {
  using value_type = T;
  inline static std::size_t allocations = 0;

  T *allocate(std::size_t count) {
    ++allocations;
    return std::allocator<T>{}.allocate(count);
  }
  void deallocate(T *pointer, std::size_t count) noexcept {
    std::allocator<T>{}.deallocate(pointer, count);
  }
  template <typename U>
  bool operator==(const CountingAllocator<U> &) const noexcept { return true; }
  template <typename U>
  bool operator!=(const CountingAllocator<U> &) const noexcept { return false; }
};

void require(bool ok, const char *message) {
  if (!ok) throw std::runtime_error(message);
}

int main() {
  try {
    const auto result = msime::fcitx_host::preceding_characters("甲乙丙丁", 4, 3);
    require(result && *result == std::vector<std::string>{"乙", "丙", "丁"},
            "preceding characters preserve order");

    using TrackedVector = std::vector<std::string, CountingAllocator<std::string>>;
    CountingAllocator<std::string>::allocations = 0;
    const auto tracked = msime::fcitx_host::preceding_characters_with_storage<TrackedVector>(
        "甲乙丙丁", 4, 3);
    require(tracked && tracked->size() == 3 && tracked->at(0) == "乙" &&
                tracked->at(1) == "丙" && tracked->at(2) == "丁",
            "tracked preceding characters preserve order");
    require(CountingAllocator<std::string>::allocations == 1,
            "preceding characters reserve before appending");

    const auto shorter = msime::fcitx_host::preceding_characters("甲乙", 2, 5);
    require(shorter && *shorter == std::vector<std::string>{"甲", "乙"},
            "preceding characters stop at the document start");

    const auto invalid = msime::fcitx_host::preceding_characters(std::string("\xff", 1), 1, 1);
    require(!invalid, "preceding characters reject invalid UTF-8");
    const auto beyond = msime::fcitx_host::preceding_characters("甲乙", 3, 1);
    require(!beyond, "preceding characters reject a caret beyond the document");
  } catch (const std::exception &error) {
    return (std::fprintf(stderr, "%s\n", error.what()), 1);
  }
  return 0;
}
