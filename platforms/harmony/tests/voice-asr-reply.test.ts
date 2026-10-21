import { VoiceResponsePolicy } from "../entry/src/main/ets/keyboard/input/VoiceResponsePolicy";

declare const process: { exitCode?: number };
const transcript = "合成转写";
let failures = 0;
let checked = 0;
function check(name: string, body: Object, shouldFail: boolean): void {
  const result = VoiceResponsePolicy.batchResult(200, JSON.stringify(body));
  checked++;
  if (
    result.text !== (shouldFail ? "" : transcript) ||
    (shouldFail ? result.failure !== "语音服务未返回有效识别结果。" : result.failure !== "") ||
    !result.last
  ) {
    failures++;
    console.error(`FAIL ASR reply ${name}`);
  }
}
for (const shape of ["text", "chat"]) {
  function reply(fields: Record<string, Object | null> = {}): Object {
    return {
      ...(shape === "text" ? { text: transcript } : {}),
      choices: [{ message: { content: transcript }, ...fields }],
    };
  }
  check(`${shape}/legacy`, reply(), false);
  check(`${shape}/stop`, reply({ finish_reason: "stop" }), false);
  check(`${shape}/null-finish`, reply({ finish_reason: null }), false);
  check(`${shape}/unknown-finish`, reply({ finish_reason: "provider_extension" }), false);
  check(`${shape}/null-error`, { ...reply(), error: null }, false);
  for (const reason of ["length", "content_filter"]) {
    check(`${shape}/${reason}`, reply({ finish_reason: reason }), true);
  }
  for (const [index, error] of [
    {},
    { message: "synthetic failure" },
    false,
    0,
    "",
    "synthetic failure",
  ].entries()) {
    check(`${shape}/error-${index}`, { ...reply(), error }, true);
  }
}
console.log(`ASR replies: ${checked} cases, ${failures} failures`);
if (failures > 0) process.exitCode = 1;
