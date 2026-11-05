// @vitest-environment jsdom
import { expect, test } from "vitest";
import { dictionaryErrorMessage } from "@msime/ui";

const FALLBACK = "全拼保存失败，请稍后重试。";

test("the three actionable host reasons no longer read identically", () => {
  const busy = dictionaryErrorMessage({ code: "dictionary_busy" }, FALLBACK);
  const rejected = dictionaryErrorMessage({ code: "dictionary_import_rejected" }, FALLBACK);
  const unavailable = dictionaryErrorMessage({ code: "dictionary_unavailable" }, FALLBACK);
  for (const message of [busy, rejected, unavailable]) {
    expect(message).not.toBe(FALLBACK);
  }
  expect(new Set([busy, rejected, unavailable]).size).toBe(3);
});

test("a locked dictionary tells the user what to close, not to retry", () => {
  // This was the worst case: the IME being held by another process read as
  // "please retry later", which never becomes true on its own.
  const message = dictionaryErrorMessage({ code: "dictionary_busy" }, FALLBACK);
  expect(message).toContain("关闭");
  expect(message).not.toContain("稍后重试。");
});

test("the remaining host reasons are distinguished too", () => {
  expect(dictionaryErrorMessage({ code: "dictionary_read_rejected" }, FALLBACK)).toContain("读取");
  expect(dictionaryErrorMessage({ code: "dictionary_pinyin_unavailable" }, FALLBACK)).toContain(
    "拼音表",
  );
  expect(dictionaryErrorMessage({ code: "dictionary_bundled_readonly" }, FALLBACK)).toContain(
    "内置",
  );
});

test("an import too large for the host names both size limits", () => {
  const message = dictionaryErrorMessage({ code: "dictionary_too_large" }, FALLBACK);
  expect(message).not.toBe(FALLBACK);
  expect(message).toContain("32 MB");
  expect(message).toContain("单行");
  expect(message).toContain("拆分");
});

test("an unknown or absent code keeps the caller's sentence", () => {
  expect(dictionaryErrorMessage({ code: "storage" }, FALLBACK)).toBe(FALLBACK);
  expect(dictionaryErrorMessage({}, FALLBACK)).toBe(FALLBACK);
  expect(dictionaryErrorMessage(undefined, FALLBACK)).toBe(FALLBACK);
  expect(dictionaryErrorMessage(new Error("boom"), FALLBACK)).toBe(FALLBACK);
  expect(dictionaryErrorMessage("boom", FALLBACK)).toBe(FALLBACK);
});

test("a refused entry says what the code has to look like, per dictionary", () => {
  const refused = { code: "dictionary_invalid_entry" };
  const pinyin = dictionaryErrorMessage(refused, FALLBACK, "pinyin");
  expect(pinyin).toContain("完整音节");
  expect(pinyin).toContain("音节数需与汉字数一致");
  expect(pinyin).not.toContain("稍后重试");
  expect(dictionaryErrorMessage(refused, FALLBACK, "wubi")).toContain("1 到 4 个字母");
  expect(dictionaryErrorMessage(refused, FALLBACK, "wubixsj")).toContain("1 到 4 个字母");
  expect(dictionaryErrorMessage(refused, FALLBACK, "quick_phrase")).toContain("只能包含英文字母");
  expect(dictionaryErrorMessage(refused, FALLBACK, "english")).toContain("字母、连字符和撇号");
  // Without a kind (the import path) it still names the problem rather than asking for a retry.
  const generic = dictionaryErrorMessage(refused, FALLBACK);
  expect(generic).not.toBe(FALLBACK);
  expect(generic).not.toContain("稍后重试");
});

test("a refused word or weight does not blame a valid code", () => {
  const refused = { code: "dictionary_invalid_word" };
  for (const kind of ["pinyin", "wubi", "quick_phrase", "english"] as const) {
    const message = dictionaryErrorMessage(refused, FALLBACK, kind);
    expect(message).toContain("词条内容");
    expect(message).toContain("权重");
    expect(message).not.toContain("编码");
    expect(message).not.toContain("音节");
    expect(message).not.toContain("稍后重试");
  }
});
