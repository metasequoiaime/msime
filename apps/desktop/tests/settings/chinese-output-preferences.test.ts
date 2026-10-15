import { expect, test } from "vitest";
import {
  chineseOutputForm,
  chineseOutputPatch,
  type TraditionalChineseVariant,
} from "../../../../packages/ui/src/settings/chinese-output-preferences";

const variants: (TraditionalChineseVariant | undefined)[] = [undefined, "s2t", "s2tw", "s2twp"];

test.each(variants)("缺省或关闭简繁输出时保留 %s 但显示简体", (variant) => {
  for (const supported of [undefined, false, true]) {
    expect(chineseOutputForm(undefined, variant, supported)).toBe("simplified");
    expect(chineseOutputForm(false, variant, supported)).toBe("simplified");
  }
});

test.each(variants)("只在宿主支持变体时显示保存的 %s", (variant) => {
  expect(chineseOutputForm(true, variant, true)).toBe(variant ?? "s2t");
  expect(chineseOutputForm(true, variant, false)).toBe("s2t");
  expect(chineseOutputForm(true, variant, undefined)).toBe("s2t");
});

test.each(variants)("选择简体不覆盖保存的 %s", (variant) => {
  for (const supported of [undefined, false, true]) {
    expect(chineseOutputPatch("simplified", variant, supported)).toEqual({
      traditional_chinese_output: false,
    });
  }
});

test.each(variants)("不支持变体的宿主切回繁体不覆盖 %s", (variant) => {
  expect(chineseOutputPatch("s2t", variant, false)).toEqual({
    traditional_chinese_output: true,
  });
  expect(chineseOutputPatch("s2t", variant, undefined)).toEqual({
    traditional_chinese_output: true,
  });
});

test.each([
  [undefined, "s2t", { traditional_chinese_output: true }],
  [undefined, "s2tw", { traditional_chinese_output: true, traditional_chinese_variant: "s2tw" }],
  ["s2tw", "s2tw", { traditional_chinese_output: true }],
  ["s2twp", "s2twp", { traditional_chinese_output: true }],
  ["s2twp", "s2t", { traditional_chinese_output: true, traditional_chinese_variant: "s2t" }],
  ["s2tw", "s2twp", { traditional_chinese_output: true, traditional_chinese_variant: "s2twp" }],
] as const)("支持变体时从 %s 选到 %s 只写需要的字段", (variant, next, expected) => {
  expect(chineseOutputPatch(next, variant, true)).toEqual(expected);
});

test("关闭并重新打开繁体时保留此前的台湾用词选择", () => {
  const initial = {
    traditional_chinese_output: true,
    traditional_chinese_variant: "s2twp" as const,
  };
  const disabled = {
    ...initial,
    ...chineseOutputPatch("simplified", initial.traditional_chinese_variant, true),
  };
  const enabled = {
    ...disabled,
    ...chineseOutputPatch("s2twp", disabled.traditional_chinese_variant, true),
  };
  expect(
    chineseOutputForm(
      disabled.traditional_chinese_output,
      disabled.traditional_chinese_variant,
      true,
    ),
  ).toBe("simplified");
  expect(enabled).toEqual(initial);
});
