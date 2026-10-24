import { expect, test } from "vitest";
import {
  candidateSkinSelection,
  type CandidateSkinSelectionOptions,
  type CandidateSkinSelectionPackage,
} from "@msime/ui";
import { applyCandidateSkin } from "../../../../packages/ui/src/theme/global-theme";

const packages: readonly CandidateSkinSelectionPackage[] = Object.freeze([
  Object.freeze({ id: "light-example", name: "合成浅色", base: "paper" as const }),
  Object.freeze({ id: "dark-example", name: "合成深色", base: "night" as const }),
  Object.freeze({ id: "system-example", name: "合成系统", base: "system" as const }),
]);

function selection(patch: Partial<CandidateSkinSelectionOptions> = {}) {
  return candidateSkinSelection({
    globalTheme: "custom",
    customTheme: { candidate_skin: "light-example", candidate_skin_dark: "dark-example" },
    packages,
    previewTheme: "light",
    ...patch,
  });
}

test("未启用自定义主题时，保留的槽位不算选中", () => {
  const model = selection({ globalTheme: "night" });
  expect(model.skinsInUse).toEqual([]);
  expect(model.packageInUse).toBe(-1);
  expect(model.slotSummary).toBeNull();
  expect(model.slotOf("light-example")).toBe("light");
});

test.each([undefined, {}, { candidate_skin: "", candidate_skin_dark: null }])(
  "空槽位不产生选中卡片或说明：%j",
  (customTheme) => {
    const model = selection({ customTheme });
    expect(model.skinsInUse).toEqual([]);
    expect(model.packageInUse).toBe(-1);
    expect(model.slotSummary).toBeNull();
  },
);

test.each([
  ["light", 0],
  ["dark", 1],
] as const)("轮播优先定位当前 %s 预览的皮肤", (previewTheme, expected) => {
  const model = selection({ previewTheme });
  expect(model.skinsInUse).toEqual(["light-example", "dark-example"]);
  expect(model.packageInUse).toBe(expected);
  expect(model.slotSummary).toBe("浅色模式用「合成浅色」，深色模式用「合成深色」。");
});

test.each([
  ["light", "missing-light", "dark-example", 1],
  ["dark", "light-example", "missing-dark", 0],
] as const)("%s 预览的包缺失时定位另一个已安装槽位", (previewTheme, light, dark, expected) => {
  const model = selection({
    previewTheme,
    customTheme: { candidate_skin: light, candidate_skin_dark: dark },
  });
  expect(model.packageInUse).toBe(expected);
});

test("目录缺失时保留槽位 id 说明，卡片没有对应选中项", () => {
  const model = selection({ packages: [] });
  expect(model.skinsInUse).toEqual(["light-example", "dark-example"]);
  expect(model.packageInUse).toBe(-1);
  expect(model.slotSummary).toBe("浅色模式用「light-example」，深色模式用「dark-example」。");
  expect(model.slotOf("light-example")).toBeUndefined();
});

test.each([
  [{ candidate_skin: "dark-example" }, "浅色模式不用皮肤，深色模式用「合成深色」。", 1],
  [{ candidate_skin: "light-example" }, "浅色模式用「合成浅色」，深色模式不用皮肤。", 0],
  [{ candidate_skin_dark: "dark-example" }, "浅色模式不用皮肤，深色模式用「合成深色」。", 1],
])("说明按槽位回退和包的固定明暗绘制：%j", (customTheme, summary, index) => {
  const model = selection({ customTheme });
  expect(model.packageInUse).toBe(index);
  expect(model.slotSummary).toBe(summary);
});

test("系统底的同一皮肤占两个槽位时，两个模式都显示其名称", () => {
  const model = selection({
    customTheme: { candidate_skin: "system-example", candidate_skin_dark: "system-example" },
  });
  expect(model.skinsInUse).toEqual(["system-example", "system-example"]);
  expect(model.packageInUse).toBe(2);
  expect(model.slotSummary).toBe("浅色模式用「合成系统」，深色模式用「合成系统」。");
});

test("目录槽位解析保留浅色、深色、系统和未知的区别", () => {
  const model = selection();
  expect(model.slotOf("light-example")).toBe("light");
  expect(model.slotOf("dark-example")).toBe("dark");
  expect(model.slotOf("system-example")).toBeNull();
  expect(model.slotOf("missing-example")).toBeUndefined();
});

test("公共槽位解析与已有应用算法配合，保留旧文档的深色皮肤", () => {
  const customTheme = Object.freeze({ base: "night" as const, candidate_skin: "dark-example" });
  const model = selection({ customTheme });
  expect(applyCandidateSkin(customTheme, "light-example", "paper", model.slotOf)).toEqual({
    base: "paper",
    candidate_skin: "light-example",
    candidate_skin_dark: "dark-example",
  });
  expect(customTheme).toEqual({ base: "night", candidate_skin: "dark-example" });
  expect(packages.map((item) => item.id)).toEqual([
    "light-example",
    "dark-example",
    "system-example",
  ]);
});
