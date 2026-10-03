#include "DedicatedEnglishMirror.h"
#include <cassert>

using msime::windows::DedicatedEnglishMirror;

int main() {
  constexpr uint64_t confirm = DedicatedEnglishMirror::kConfirmMs;

  // 推送之前是关的，推送什么就是什么。
  {
    DedicatedEnglishMirror mirror;
    assert(!mirror.active(0));
    mirror.server(true);
    assert(mirror.active(5));
    mirror.server(false);
    assert(!mirror.active(10));
  }

  // 吞下切换键后立刻是新值，不等推送；推送确认后一直保持。
  {
    DedicatedEnglishMirror mirror;
    mirror.toggled(100);
    assert(mirror.active(100));
    assert(mirror.active(100 + 250));
    mirror.server(true);
    assert(mirror.active(100 + confirm + 5000));
  }

  // 没有推送（Server 没切换）：时限一到退回推送过的值。
  {
    DedicatedEnglishMirror mirror;
    mirror.server(true);
    mirror.toggled(1000);
    assert(!mirror.active(1000));
    assert(!mirror.active(1000 + confirm - 1));
    assert(mirror.active(1000 + confirm));
  }

  // 推送与临时值不一致时以推送为准。
  {
    DedicatedEnglishMirror mirror;
    mirror.toggled(0);
    assert(mirror.active(1));
    mirror.server(false);
    assert(!mirror.active(2));
  }

  // 连按两次切换回到原值；时限内和时限后都一样。
  {
    DedicatedEnglishMirror mirror;
    mirror.toggled(0);
    mirror.toggled(10);
    assert(!mirror.active(20));
    assert(!mirror.active(10 + confirm));
  }
  return 0;
}
