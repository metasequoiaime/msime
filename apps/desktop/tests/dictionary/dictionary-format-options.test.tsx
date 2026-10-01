// @vitest-environment jsdom
import { expect, test } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach } from "vitest";
import { DictionaryFormatOptions } from "@msime/ui";

afterEach(() => {
  cleanup();
});

test("offers standard and Windows formats for every dictionary", () => {
  render(
    <select aria-label="文件格式">
      <DictionaryFormatOptions pinyin={false} />
    </select>,
  );

  expect(Array.from(screen.getByRole("combobox").querySelectorAll("option"))).toHaveLength(2);
  expect(screen.getByRole("option", { name: "词在前（标准 TSV）" })).toBeTruthy();
  expect(screen.getByRole("option", { name: "编码在前（Windows TSV）" })).toBeTruthy();
  expect(screen.queryByRole("option", { name: "汉字自动注音（仅导入）" })).toBeNull();
});

test("offers Chinese auto-annotation for pinyin dictionaries", () => {
  render(
    <select aria-label="文件格式">
      <DictionaryFormatOptions pinyin />
    </select>,
  );

  expect(screen.getByRole("option", { name: "汉字自动注音（仅导入）" })).toBeTruthy();
});

test("can include the local Rime format before auto-annotation", () => {
  render(
    <select aria-label="文件格式">
      <DictionaryFormatOptions pinyin rime />
    </select>,
  );

  expect(
    Array.from(screen.getByRole("combobox").querySelectorAll("option"), (option) => option.value),
  ).toEqual(["standard", "windows", "rime", "hans"]);
});
