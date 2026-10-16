import { expect, test } from "vitest";
import {
  dictionaryEntryMatches,
  dictionaryKeyMatches,
} from "../../../../packages/ui/src/dictionary/dictionary-errors";

test("word queries match literal values and preserve ASCII code lookup", () => {
  const entry = { kind: "pinyin" as const, key: "he'cheng", value: "合成%_词条", weight: 10 };
  expect(dictionaryEntryMatches(entry, "合成%_")).toBe(true);
  expect(dictionaryEntryMatches(entry, "成%_词")).toBe(true);
  expect(dictionaryEntryMatches(entry, "合成_词")).toBe(false);
  expect(dictionaryEntryMatches(entry, "其他")).toBe(false);
  expect(dictionaryEntryMatches(entry, "HeCh")).toBe(true);
  expect(dictionaryEntryMatches(entry, "cheng")).toBe(false);
});

test.each([
  ["english", "  SYNTHETIC-Entry  ", "synthetic", true],
  ["english", "  SYNTHETIC-Entry  ", " synthetic", false],
  ["english", "ÄBC", "Äb", true],
  ["english", "ÄBC", "äb", false],
  ["english", "İBC", "ib", false],
  ["pinyin", "NI' HAO", "ni h", true],
  ["pinyin", "NI' HAO", "ni'hao", true],
  ["english", "NI' HAO", "nihao", false],
  ["wubi", "  ABCD  ", "ab", true],
  ["wubi", "ABCD", "bc", false],
  ["english", "任意合成编码", "", true],
])("dictionary prefix matching keeps %j %j %j", (kind, key, query, expected) => {
  expect(dictionaryKeyMatches(kind, key, query)).toBe(expected);
});
