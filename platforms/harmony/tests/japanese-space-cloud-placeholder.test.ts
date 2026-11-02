import { JapaneseSpacePolicy } from "../entry/src/main/ets/keyboard/input/JapaneseSpacePolicy";

declare const process: { exitCode?: number };

// 开着云候选时第二格是云占位行「☁」（来源 15）。候选条不画它，运行时的「下一个」也越过它（#10665），所以日文空格只数高亮能停的行：[行, ☁, 行, 行] 是三行，第四次空格才回到首条。
let failures = 0;
function check(condition: boolean, message: string): void {
  if (!condition) {
    console.error(`FAIL ${message}`);
    failures++;
  }
}

check(
  JapaneseSpacePolicy.countedSources([0, 15, 0, 0]).join(",") === "0,0,0",
  "Space counts the rows the highlight can stop on, without the cloud placeholder",
);
// 第一行是占位行时，firstSource 取第一条看得见的行：唯一看得见的是 Fallback，空格照常上屏。
const behindPlaceholder: number[] = JapaneseSpacePolicy.countedSources([
  15,
  JapaneseSpacePolicy.CANDIDATE_SOURCE_FALLBACK,
]);
check(
  behindPlaceholder.length === 1 &&
    !JapaneseSpacePolicy.converts(behindPlaceholder.length, behindPlaceholder[0]),
  "a lone Fallback row behind the cloud placeholder still commits on Space",
);
check(
  JapaneseSpacePolicy.countedSources([15]).length === 0,
  "a page holding only the cloud placeholder has nothing to convert",
);
// 没写来源的行（记作 -1）照常数进去；已退役的 AI 占位行（12）照旧画出来，也照常数。
check(
  JapaneseSpacePolicy.countedSources([-1, 12]).length === 2,
  "only the cloud placeholder is left out",
);

if (failures > 0) process.exitCode = 1;
else console.log("  ok  日文空格越过云占位行计数");
