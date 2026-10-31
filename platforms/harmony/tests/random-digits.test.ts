/**
 * 随机数字（#10254）：键序策略与 Android `RandomDigitOrderPolicySmoke` 逐条相同（排列完整、每输入一个数字换序、离开数字页不取随机数、返回沿用、关闭恢复、恒等随机排列仍换序），以及按键序画网格的 `NineKeyLayout.digitsInOrder`。
 */
import { RandomDigitOrderPolicy } from "../entry/src/main/ets/keyboard/input/RandomDigitOrderPolicy";
import { NineKeyLayout } from "../entry/src/main/ets/keyboard/input/NineKeyLayout";

let failures = 0;

function group(name: string, body: () => void): void {
  try {
    body();
    console.log(`  ok  ${name}`);
  } catch (error) {
    failures++;
    console.log(`FAIL  ${name}`);
    console.log(`      ${error instanceof Error ? error.message : String(error)}`);
  }
}

function check(condition: boolean, message: string): void {
  if (!condition) {
    throw new Error(message);
  }
}

function same(left: readonly number[], right: readonly number[]): boolean {
  return (
    left.length === right.length &&
    left.every((value: number, index: number): boolean => value === right[index])
  );
}

/** 固定种子的线性同余随机源，并数它被调用了几次：离开数字页不能取随机数。 */
class CountedRandom {
  calls = 0;
  private state = 10254;

  readonly nextInt = (bound: number): number => {
    this.calls++;
    this.state = (this.state * 48271) % 2147483647;
    return this.state % bound;
  };
}

const STANDARD: number[] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 0];

group("键序与 Android 的策略逐条相同", () => {
  const random = new CountedRandom();
  const policy = new RandomDigitOrderPolicy(random.nextInt);
  check(!policy.enabled() && same(policy.faces(true), STANDARD), "默认不重排");
  policy.setEnabled(true);
  check(random.calls === 0, "字母页启用时不计算键序");
  check(same(policy.faces(false), STANDARD), "英文和字母页保留标准排列");
  policy.afterDigit(false);
  check(random.calls === 0, "页面外数字输入不取随机数");
  let previous: number[] = policy.faces(true);
  for (let round = 0; round < 1000; round++) {
    check(same(previous.slice().sort(), [0, 1, 2, 3, 4, 5, 6, 7, 8, 9]), "十个数字完整且无重复");
    const calls: number = random.calls;
    check(same(previous, policy.faces(true)) && calls === random.calls, "普通重画不换序");
    policy.faces(false);
    policy.afterDigit(false);
    check(random.calls === calls && policy.enabled(), "离开数字页不计算也不关闭");
    check(same(previous, policy.faces(true)), "返回数字页沿用上次键序");
    policy.afterDigit(true);
    const next: number[] = policy.faces(true);
    check(!same(previous, next), "每次数字输入后换序");
    previous = next;
  }
  const calls: number = random.calls;
  policy.setEnabled(false);
  check(same(policy.faces(true), STANDARD) && random.calls === calls, "关闭立即恢复且不计算");
  policy.afterDigit(true);
  check(random.calls === calls, "关闭后输入不计算");
});

group("随机源恰好给出恒等排列时也换序", () => {
  const unchanged = new RandomDigitOrderPolicy((bound: number): number => bound - 1);
  unchanged.setEnabled(true);
  const first: number[] = unchanged.faces(true);
  check(!same(first, STANDARD), "首次仍换序");
  unchanged.afterDigit(true);
  check(!same(first, unchanged.faces(true)), "恒等排列不能使输入后的换序失效");
});

group("键位越界直接报错", () => {
  const policy = new RandomDigitOrderPolicy();
  for (const slot of [-1, 10, 1.5]) {
    let threw = false;
    try {
      policy.digit(slot, true);
    } catch {
      threw = true;
    }
    check(threw, `键位 ${slot} 报错`);
  }
});

group("网格按键序画前九个键位，标准键序就是原来的数字层", () => {
  check(NineKeyLayout.digitsInOrder(STANDARD) === NineKeyLayout.digits(), "标准键序沿用同一份网格");
  const order: number[] = [7, 0, 3, 9, 1, 5, 2, 8, 4, 6];
  const rows = NineKeyLayout.digitsInOrder(order);
  const faces: string[] = rows.flat().map((key) => key.label);
  check(faces.join("") === "703915284", "前九个键位依次填进 3×3 网格，第十个是底行的 0 那一格");
  check(
    rows.flat().every((key) => key.input === key.label && key.description === key.label),
    "上屏和读屏都按键面上的数字",
  );
  let threw = false;
  try {
    NineKeyLayout.digitsInOrder([1, 2, 3]);
  } catch {
    threw = true;
  }
  check(threw, "不是十个键位时报错");
});

if (failures > 0) {
  console.log(`${failures} random digits group(s) failed`);
  throw new Error("random digits checks failed");
}
