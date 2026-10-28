import { TelemetryPolicy, CrashReport } from "../entry/src/main/ets/telemetry/TelemetryPolicy";

let assertions = 0;
function check(condition: boolean, message: string): void {
  assertions++;
  if (!condition) throw new Error(message);
}
const epoch = "12345678-1234-4234-8234-123456789abc";
const capture = "abcdabcd-abcd-4bcd-abcd-abcdabcdabcd";
const consent = { epoch, since: 1000 };
const context = {
  platform: "harmony",
  version: "0.1.0",
  revision: "abcdef12",
  run: "7",
  attempt: "1",
  arch: "aarch64",
  component: "input",
  consent_epoch: epoch,
};
const header = `MSIME-CRASH/1 ${JSON.stringify(context)}\n`;
const encoded = TelemetryPolicy.systemCapture(capture, header, 42, 2000, consent);
check(encoded !== null && encoded.length <= 1024, "原身份适合系统单参数限制");
const report: CrashReport = {
  pid: 42,
  time: 3000,
  version: "0.1.0",
  message: "SIGABRT",
  stack: "",
  capture: encoded ?? "",
};
const original = TelemetryPolicy.originalCapture(report, consent, 4000);
check(
  original?.capture === capture && original.header === header,
  "升级补报仍保留原 UUID 和精确原构建头",
);
check(original?.pid === 42 && original.since === 2000, "归属含原进程及准备时刻");
for (const invalid of [
  { ...report, capture: "" },
  { ...report, capture: "broken" },
  { ...report, capture: "[]" },
  { ...report, capture: "x".repeat(1025) },
  { ...report, pid: 43 },
  { ...report, version: "0.2.0" },
  { ...report, time: 1500 },
  { ...report, time: 5000 },
])
  check(
    TelemetryPolicy.originalCapture(invalid, consent, 4000) === null,
    "缺失、损坏、跨进程、版本或时间不匹配均拒绝",
  );
check(
  TelemetryPolicy.originalCapture(report, { ...consent, epoch: capture }, 4000) === null,
  "同意关闭重开后不能认领旧系统参数",
);
check(
  TelemetryPolicy.originalCapture(report, { ...consent, since: 2500 }, 4000) === null,
  "捕获准备必须在本轮同意之后",
);
for (const patch of [
  { capture: "not-uuid" },
  { pid: 0 },
  { since: 0 },
  { since: "2000" },
  { format: 2 },
  { header: "SIGABRT\n" },
  { header: `MSIME-CRASH/1 ${JSON.stringify({ ...context, consent_epoch: capture })}\n` },
  { header: `MSIME-CRASH/1 ${JSON.stringify({ ...context, platform: "android" })}\n` },
  { header: `MSIME-CRASH/1 ${JSON.stringify({ ...context, component: "synthetic" })}\n` },
  { header: `MSIME-CRASH/1 ${JSON.stringify({ ...context, revision: "/private/synthetic" })}\n` },
  { header: `MSIME-CRASH/1 ${JSON.stringify({ ...context, arch: "" })}\n` },
]) {
  const invalid = JSON.stringify({ ...original, ...patch });
  check(
    TelemetryPolicy.originalCapture({ ...report, capture: invalid }, consent, 4000) === null,
    "原身份的字段必须完整有效，不能补默认值",
  );
}
check(
  TelemetryPolicy.systemCapture(capture, header, 42, 999, consent) === null,
  "未获授权时不能生成系统参数",
);
check(
  TelemetryPolicy.systemCapture("not-uuid", header, 42, 2000, consent) === null,
  "无效 UUID 不能交给系统",
);
const decoded = TelemetryPolicy.crashReport(
  JSON.stringify({
    time: 3000,
    pid: 42,
    bundle_version: "0.1.0",
    msime_capture: encoded,
    crash_type: "NativeCrash",
    exception: {
      signal: { signo: 11 },
      frames: [
        {
          file: "/private/synthetic/libfixture.so",
          pc: "000042",
          buildId: "ABCDef1234",
          symbol: "synthetic",
        },
        { file: "libfixture.so", pc: "private arbitrary text", buildId: "private arbitrary text" },
      ],
    },
  }),
);
check(decoded?.capture === encoded, "系统原参数完整传给认领规则");
check(
  decoded?.stack === "libfixture.so+0x42 [id 0xabcdef1234] synthetic\nlibfixture.so",
  "帧保留可验证 ELF 身份，拒绝无效偏移和 build-id",
);
const freeze = TelemetryPolicy.freezeReport(
  JSON.stringify({
    time: 3000,
    pid: 42,
    bundle_version: "0.1.0",
    msime_capture: encoded,
    exception: { name: "THREAD_BLOCK_6S" },
  }),
);
check(
  freeze?.capture === encoded && freeze.outsideSession === true,
  "卡死保留原身份且独立于会话崩溃",
);
console.log(`harmony system capture identity: ${assertions} assertions passed`);
