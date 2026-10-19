import { VoiceResponsePolicy } from "../entry/src/main/ets/keyboard/input/VoiceResponsePolicy";

declare const process: { exitCode?: number };
const polished = "合成润色结果";
function reply(fields: Record<string, Object | null> = {}): Object {
  return { choices: [{ message: { content: polished }, ...fields }] };
}

const cases: { name: string; body: Object; expected: string | null }[] = [
  { name: "length", body: reply({ finish_reason: "length" }), expected: null },
  { name: "filtered", body: reply({ finish_reason: "content_filter" }), expected: null },
  ...[{}, { message: "synthetic failure" }, false, 0, "", "synthetic failure"].map(
    (error, index) => ({
      name: `error-${index}`,
      body: { ...reply(), error },
      expected: null,
    }),
  ),
  { name: "empty-choices", body: { choices: [] }, expected: null },
  { name: "legacy", body: reply(), expected: polished },
  { name: "stop", body: reply({ finish_reason: "stop" }), expected: polished },
  { name: "null-finish", body: reply({ finish_reason: null }), expected: polished },
  { name: "null-error", body: { ...reply(), error: null }, expected: polished },
];

let failures = 0;
function check(name: string, status: number, body: string | null, expected: string | null): void {
  const actual = VoiceResponsePolicy.polishResult(status, body);
  if (actual !== expected) {
    failures++;
    console.error(`FAIL 润色回复 ${name}`);
  } else {
    console.log(`  ok  润色回复 ${name}`);
  }
}
for (const test of cases) check(test.name, 200, JSON.stringify(test.body), test.expected);
check("HTTP 失败", 500, JSON.stringify(reply()), null);
check("无正文", 200, null, null);
check("无效 JSON", 200, "not json", null);
check("非对象", 200, "null", null);
check("非文字", 200, JSON.stringify({ choices: [{ message: { content: 42 } }] }), null);
check(
  "控制字符",
  200,
  JSON.stringify({ choices: [{ message: { content: "合成\u0001文本" } }] }),
  null,
);
check(
  "文字字节上限",
  200,
  JSON.stringify({ choices: [{ message: { content: "中".repeat(6_000) } }] }),
  null,
);
check("正文大小上限", 200, " ".repeat(1024 * 1024) + JSON.stringify(reply()), null);
check(
  "规范化",
  200,
  JSON.stringify({ choices: [{ message: { content: "  合成润色结果  " } }] }),
  polished,
);
if (failures > 0) process.exitCode = 1;
