#include "GroupedPanelKeyLease.h"
#include <iostream>
#include <stdexcept>

namespace {
void require(bool value) {
  if (!value) throw std::runtime_error("grouped panel key lease check failed");
}
void check() {
  using namespace msime::panel_key;
  Request request;
  require(!valid(request));
  request.nonce = 42;
  require(valid(request));
  request.operation = Operation::Press;
  require(!valid(request));
  request.letter = 'n';
  require(valid(request));
  request.reserved = 1;
  require(!valid(request));
  request.reserved = 0;
  request.version = 2;
  require(!valid(request));
  request.version = Version;
  request.letter = 0x100 + 'n';
  require(!valid(request));

  Leases leases;
  // 高位不同也必须拒绝，覆盖 x86 返回值截断无法表达的焦点变化。
  Owner owner{0x100000001ull, 0x200000001ull, 3, 4, 5};
  int calls = 0;
  const auto enqueue = [&] { ++calls; return true; };
  require(!leases.press(42, owner, 10, enqueue));
  require(leases.query(42, owner, 10));
  require(!leases.query(42, owner, 20));
  for (int field = 0; field < 5; ++field) {
    auto stale = owner;
    if (field == 0) stale.generation += 0x100000000ull;
    if (field == 1) stale.worker_token += 0x100000000ull;
    if (field == 2) ++stale.context;
    if (field == 3) ++stale.foreground;
    if (field == 4) ++stale.focus;
    require(!leases.press(42, stale, 20, enqueue));
  }
  require(calls == 0);
  require(leases.press(42, owner, 20, [&] {
    require(!leases.press(42, owner, 21, enqueue));
    return enqueue();
  }));
  require(leases.press(42, owner, 22, enqueue));
  require(calls == 1);
  require(!leases.press(42, owner, 510, enqueue));
  require(leases.query(43, owner, 600));
  require(!leases.press(43, owner, 599, enqueue));
  leases.clear();
  for (std::uint64_t nonce = 1; nonce <= 8; ++nonce)
    require(leases.query(nonce, owner, 1000));
  require(!leases.query(9, owner, 1000));
  require(leases.query(9, owner, 1500));
  require(!leases.press(9, owner, 1501, [] { return false; }));
  require(!leases.press(9, owner, 1502, enqueue));
  leases.clear();
  require(!leases.press(9, owner, 1503, enqueue));
  require(calls == 1);
}
}
int main() {
  try { check(); std::cout << "Grouped panel key lease checks passed\n"; return 0; }
  catch (const std::exception &error) { std::cerr << error.what() << '\n'; return 1; }
}
