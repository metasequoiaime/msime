import {
  KeyPressBatch,
  KeyPressFlush,
  HeldKeys,
  KeyIdPolicy,
} from "../entry/src/main/ets/keyboard/KeyIdPolicy";
import { typingKeyPressRequest } from "../entry/src/main/ets/keyboard/TypingKeyPressRequest";
import { TypingStatisticsPolicy } from "../entry/src/main/ets/keyboard/TypingStatisticsPolicy";
import { KeyGrid } from "../entry/src/main/ets/keyboard/KeyboardScheme";

function check(value: boolean, message: string): void {
  if (!value) throw new Error(message);
}
function totalsMatch(batch: KeyPressFlush): void {
  const sum: Record<string, number> = {};
  Object.keys(batch.sources).forEach((source: string): void => {
    Object.keys(batch.sources[source]).forEach((id: string): void => {
      sum[id] = (sum[id] ?? 0) + batch.sources[source][id];
    });
  });
  check(
    JSON.stringify(Object.keys(sum).sort()) === JSON.stringify(Object.keys(batch.keys).sort()),
    "来源键集合与总计相同",
  );
  Object.keys(batch.keys).forEach((id: string): void =>
    check(sum[id] === batch.keys[id], "来源精确加总为总计"),
  );
}

const batch = new KeyPressBatch();
const day = "2026-11-17";
let english = false;
function press(id: string): void {
  const source = TypingStatisticsPolicy.source("quanpin", "xiaohe", english, KeyGrid.NONE, "none");
  check(batch.add(id, day, source).length === 0, "语言切换不强制封批");
}
press("KeyA");
press("Tab");
english = true;
press("KeyA");
press("Tab");
check(batch.add("KeyB", day, "japanese").length === 0, "日文继续同一批");
check(batch.add("KeyC", day).length === 0, "旧调用缺省未知");
check(
  batch.add("NotAKey", day, "english").length === 0 && batch.pending() === 6,
  "非法键不进入来源或总计",
);
const first = batch.drain();
check(first !== null, "存在完整批次");
if (first !== null) {
  totalsMatch(first);
  check(first.sources.quanpin.KeyA === 1 && first.sources.english.KeyA === 1, "按下时保留不同语言");
  check(first.sources.quanpin.Tab === 1 && first.sources.english.Tab === 1, "通用键由共享层分类");
  check(first.sources.japanese.KeyB === 1 && first.sources.unknown.KeyC === 1, "保留日文与未知");
  const request = typingKeyPressRequest("/synthetic", first);
  const wire = JSON.stringify(request);
  const parsed = JSON.parse(wire) as {
    directory: string;
    action: { operation: string; day: string; sources: Record<string, Record<string, number>> };
  };
  check(
    parsed.directory === "/synthetic" &&
      parsed.action.operation === "record_keys" &&
      parsed.action.day === day,
    "编码使用原有请求协议",
  );
  check(parsed.action.sources.english.KeyA === 1, "来源随同总计跨 JSON 边界");
  batch.add("KeyA", day, "korean");
  const midnight = batch.add("KeyB", "2026-11-18", "english");
  check(
    midnight.length === 1 && midnight[0].day === day && midnight[0].sources.korean.KeyA === 1,
    "跨日写出旧来源",
  );
  totalsMatch(midnight[0]);
  const next = batch.drain();
  check(next !== null && next.sources.english.KeyB === 1, "新日独立来源快照");
  check(JSON.stringify(request) === wire, "后续输入不修改已交出的嵌套快照");
}
for (let index = 0; index < KeyPressBatch.FLUSH_PRESSES; index++) {
  const due = batch.add("KeyA", day, index % 2 === 0 ? "quanpin" : "english");
  if (index + 1 < KeyPressBatch.FLUSH_PRESSES) check(due.length === 0, "未到阈值继续等待");
  else {
    check(due.length === 1 && batch.pending() === 0, "256 次沿用阈值封批");
    check(
      due[0].sources.quanpin.KeyA === 128 && due[0].sources.english.KeyA === 128,
      "阈值批次保留混合来源",
    );
    totalsMatch(due[0]);
  }
}
batch.add("KeyA", day, "japanese");
batch.clear();
check(batch.pending() === 0 && batch.drain() === null, "关闭统计清除待写批次");
batch.add("KeyB", day);
const cleared = batch.drain();
check(
  cleared !== null &&
    Object.keys(cleared.sources).length === 1 &&
    cleared.sources.unknown.KeyB === 1,
  "重新启用不残留旧来源",
);
const held = new HeldKeys();
const id = KeyIdPolicy.hardware(2017);
check(id === "KeyA" && held.press(2017), "实体键首次按下");
batch.add(id as string, day, "quanpin");
check(!held.press(2017), "持键重复不额外计数");
held.release(2017);
check(held.press(2017), "抬起后的新按下");
batch.add(id as string, day, "english");
const physical = batch.drain();
check(
  physical !== null &&
    physical.keys.KeyA === 2 &&
    physical.sources.quanpin.KeyA === 1 &&
    physical.sources.english.KeyA === 1,
  "实体键按次固定来源",
);
console.log("Harmony 按键来源：混合快照、JSON、跨日、阈值、关闭清空、旧调用和实体键重复规则通过");
