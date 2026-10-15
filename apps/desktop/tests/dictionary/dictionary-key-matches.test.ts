import { expect, test } from "vitest";
import { dictionaryKeyMatches } from "../../../../packages/ui/src/dictionary/dictionary-errors";

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
