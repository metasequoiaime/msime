#include "LocalAsrCommandQueue.h"

#include <cassert>
#include <string>

namespace {

struct Item {
  std::string value;
};

} // namespace

int main() {
  msime::voice::BoundedCommandQueue<Item> queue(10);

  assert(queue.try_push(Item{"first"}, 6));
  assert(queue.bytes() == 6);
  assert(!queue.try_push(Item{"too large"}, 5));
  assert(queue.size() == 1);
  assert(queue.bytes() == 6);

  auto item = queue.pop();
  assert(item.has_value());
  assert(item->value == "first");
  assert(queue.empty());
  assert(queue.bytes() == 0);

  assert(!queue.try_push(Item{"oversized"}, 11));
  assert(queue.empty());
  return 0;
}
