import { expect, test } from "vitest";
import { localModeRowsFor } from "@msime/ui";

const base = { temporaryJapanese: true, triggers: false, mentions: false, ios: false };

test("常规快捷模式保持原有顺序和组合键文案", () => {
  const rows = localModeRowsFor(base);
  expect(rows.map(([key]) => key)).toEqual([
    "quick_phrase",
    "date_time",
    "date_time_inline",
    "date_time_menu",
    "unicode",
    "emoji",
    "kaomoji",
    "symbol",
    "super_jianpin",
    "temporary_english",
    "temporary_japanese",
    "chaizi",
  ]);
  expect(rows[0]).toEqual([
    "quick_phrase",
    "快捷短语(K 模式)",
    "中文模式下按 Shift+K，再输入编码即可调用快捷短语",
  ]);
});

test("宿主能力筛选日语、触发模式和从属地名并保持顺序", () => {
  const rows = localModeRowsFor({ ...base, temporaryJapanese: false, triggers: true });
  expect(rows.map(([key]) => key)).toEqual([
    "quick_phrase",
    "date_time",
    "date_time_inline",
    "date_time_menu",
    "unicode",
    "emoji",
    "kaomoji",
    "symbol",
    "super_jianpin",
    "temporary_english",
    "chaizi",
    "expression",
    "command",
  ]);
  expect(
    localModeRowsFor({ ...base, triggers: true, mentions: true })
      .slice(-4)
      .map(([key]) => key),
  ).toEqual(["expression", "command", "mention", "mention_places"]);
  expect(localModeRowsFor({ ...base, mentions: true })).toEqual(localModeRowsFor(base));
  expect(localModeRowsFor(base).map(([key]) => key)).toContain("temporary_japanese");
});

test("macOS 说明普通输入可直接调用快捷短语", () => {
  expect(
    localModeRowsFor({ ...base, quickPhraseCandidates: true }).find(
      ([key]) => key === "quick_phrase",
    )?.[2],
  ).toBe("中文模式下直接输入编码即可调用快捷短语，也可以按 Shift+K 后输入编码");
});

test("iOS 使用触屏入口文案，翻译提示只在明确未配置服务时追加", () => {
  const iosRows = localModeRowsFor({ ...base, ios: true, triggers: true });
  expect(iosRows.find(([key]) => key === "quick_phrase")?.[2]).toBe(
    "在候选栏点「水杉输入法」，或在「更多 → 本地输入」里选「快捷短语」，再输入编码即可调用快捷短语",
  );
  const command = (translationService?: boolean) =>
    localModeRowsFor({ ...base, ios: true, triggers: true, translationService }).find(
      ([key]) => key === "command",
    )?.[2];
  const original = command();
  expect(command(true)).toBe(original);
  expect(command(false)).toBe(
    `${original}。fy 翻译需要先在「标点与翻译 → 候选词翻译」选择翻译服务，目前未选择，fy 不会出结果。`,
  );
});
