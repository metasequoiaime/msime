import { expect, test } from "vitest";
import { preeditControlState } from "@msime/ui";

test("missing preedit preferences show pinyin candidates and raw inline/shuangpin keys", () => {
  expect(preeditControlState({})).toEqual({
    candidateStyle: "pinyin",
    candidateVisible: true,
    shuangpinStyle: "raw",
    inlineStyle: "raw",
  });
});

test("stored hidden candidate and inline styles and expanded shuangpin keys stay effective", () => {
  const preferences = Object.freeze({
    candidate_preedit_style: "empty" as const,
    shuangpin_preedit_uses_raw: false,
    tsf_preedit_style: "empty" as const,
  });
  expect(preeditControlState(preferences)).toEqual({
    candidateStyle: "empty",
    candidateVisible: false,
    shuangpinStyle: "pinyin",
    inlineStyle: "empty",
  });
  expect(preferences).toEqual({
    candidate_preedit_style: "empty",
    shuangpin_preedit_uses_raw: false,
    tsf_preedit_style: "empty",
  });
});

test("visible candidates and pinyin inline keys preserve explicit styles", () => {
  expect(
    preeditControlState({
      candidate_preedit_style: "pinyin",
      shuangpin_preedit_uses_raw: true,
      tsf_preedit_style: "pinyin",
    }),
  ).toEqual({
    candidateStyle: "pinyin",
    candidateVisible: true,
    shuangpinStyle: "raw",
    inlineStyle: "pinyin",
  });
});
