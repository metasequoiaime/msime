#include "../../src/core/SurroundingCharacters.h"

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
    const std::string text = "甲乙丙丁";
    const auto result = msime::linux_host::preceding_characters_from_byte_offset(
        text, text.size(), 3);
    require(result && *result == std::vector<std::string>{"乙", "丙", "丁"},
            "preceding characters preserve order");

    using TrackedVector = std::vector<std::string, CountingAllocator<std::string>>;
    CountingAllocator<std::string>::allocations = 0;
    const auto tracked =
        msime::linux_host::preceding_characters_from_byte_offset_with_storage<TrackedVector>(
            text, text.size(), 3);
    require(tracked && tracked->size() == 3 && tracked->at(0) == "乙" &&
                tracked->at(1) == "丙" && tracked->at(2) == "丁",
            "tracked preceding characters preserve order");
    require(CountingAllocator<std::string>::allocations == 1,
            "preceding characters reserve before appending");

    const auto at_start = msime::linux_host::preceding_characters_from_byte_offset(text, 0, 3);
    require(at_start && at_start->empty(), "preceding characters stop at the document start");
    const auto prefix = std::string("甲乙");
    const auto shorter = msime::linux_host::preceding_characters_from_byte_offset(
        text, prefix.size(), 5);
    require(shorter && *shorter == std::vector<std::string>{"甲", "乙"},
            "preceding characters truncate count at the document start");
    const auto beyond = msime::linux_host::preceding_characters_from_byte_offset(
        text, text.size() + 1, 1);
    require(!beyond, "preceding characters reject a byte offset beyond the document");
    const std::string invalid("\xff", 1);
    const auto invalid_result = msime::linux_host::preceding_characters_from_byte_offset(
        invalid, invalid.size(), 1);
    require(invalid_result && invalid_result->size() == 1 && invalid_result->front() == invalid,
            "preceding characters preserve the existing invalid-byte behavior");
  } catch (const std::exception &error) {
    return (std::fprintf(stderr, "%s\n", error.what()), 1);
  }
  return 0;
}
