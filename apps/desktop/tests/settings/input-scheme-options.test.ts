import { expect, test } from "vitest";
import {
  chineseInputSchemeOptions,
  japaneseInputSchemeOptions,
} from "../../../../packages/ui/src/settings/input-scheme-options";

test("shares the input scheme labels used by settings controls", () => {
  expect(chineseInputSchemeOptions).toEqual([
    { value: "quanpin", label: "全拼" },
    { value: "shuangpin", label: "双拼" },
    { value: "wubi", label: "五笔" },
  ]);
  expect(japaneseInputSchemeOptions).toEqual([{ value: "romaji", label: "罗马音" }]);
});
