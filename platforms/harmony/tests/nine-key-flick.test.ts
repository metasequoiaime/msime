/** 九键四向 flick 的触摸所有权、方向、取消、长按与共享阈值。 */
import {
  NineKeyFlick as F,
  NineKeyFlickGesture,
  NineKeyFlickPolicy as P,
} from "../entry/src/main/ets/keyboard/input/NineKeyFlickPolicy";

const assert = {
  equal(actual: unknown, expected: unknown): void {
    if (actual !== expected) throw new Error(`实际 ${String(actual)}，预期 ${String(expected)}`);
  },
};

for (const [x, y, expected] of [
  [100, 70, F.FIRST],
  [70, 100, F.SECOND],
  [130, 100, F.THIRD],
  [100, 130, F.NONE],
] as const) {
  const gesture = new NineKeyFlickGesture();
  gesture.down("2", 1, 100, 100, "ABC", 24);
  assert.equal(gesture.move("2", 1, x, y), expected);
  assert.equal(gesture.swallows("2"), true);
  assert.equal(gesture.hold("2"), false);
  gesture.move("2", 1, 100, 100);
  assert.equal(gesture.up("2", 1), expected);
}
for (const letters of ["", "A"]) {
  const gesture = new NineKeyFlickGesture();
  gesture.down("1", 1, 100, 100, letters, 24);
  assert.equal(gesture.move("1", 1, 130, 100), F.NONE);
  assert.equal(gesture.swallows("1"), true);
  assert.equal(gesture.move("1", 1, 100, 60), F.NONE);
  assert.equal(gesture.up("1", 1), F.NONE);
}
for (const threshold of [14, 24, 48]) {
  const gesture = new NineKeyFlickGesture();
  gesture.down("2", 1, 100, 100, "ABC", threshold);
  assert.equal(gesture.move("2", 1, 100, 100 - threshold), F.NONE);
  assert.equal(gesture.swallows("2"), false);
  assert.equal(gesture.move("2", 1, 100, 99 - threshold), F.FIRST);
  assert.equal(gesture.up("2", 1), F.FIRST);
}
{
  const gesture = new NineKeyFlickGesture();
  gesture.down("2", 1, 100, 100, "ABC", 24);
  gesture.move("2", 2, 100, 50);
  assert.equal(gesture.swallows("2"), false);
  assert.equal(gesture.up("2", 2), F.NONE);
  assert.equal(gesture.hold("2"), true);
  assert.equal(gesture.move("2", 1, 100, 50), F.NONE);
  assert.equal(gesture.up("2", 1), F.NONE);
  assert.equal(gesture.swallows("2"), false);
  gesture.down("2", 1, 100, 100, "ABC", 24);
  gesture.move("2", 1, 100, 50);
  gesture.cancel("2");
  assert.equal(gesture.up("2", 1), F.NONE);
  gesture.down("2", 1, 100, 100, "ABC", 24, false);
  assert.equal(gesture.move("2", 1, 100, 50), F.NONE);
  assert.equal(gesture.swallows("2"), false);
  gesture.clear();
  assert.equal(gesture.up("2", 1), F.NONE);
}
for (const value of [undefined, null, "24", 13, 25, 49, Number.NaN])
  assert.equal(P.threshold(value), 24);
for (const value of [14, 24, 48]) assert.equal(P.threshold(value), value);
assert.equal(P.character("ABC", F.SECOND), "b");
assert.equal(P.character("A", F.THIRD), "");
console.log("九键四向 flick 行为回归通过");
