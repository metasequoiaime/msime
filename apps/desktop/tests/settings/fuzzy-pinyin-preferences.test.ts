import { expect, test } from "vitest";
import {
  fuzzyPinyinEnabledChange,
  fuzzyPinyinRuleChange,
  resetFuzzyPinyin,
  type FuzzyPinyinPreferences,
} from "../../../../packages/ui/src/settings/fuzzy-pinyin-preferences";

function frozenPreferences(preferences: FuzzyPinyinPreferences): FuzzyPinyinPreferences {
  Object.freeze(preferences.rules);
  return Object.freeze(preferences);
}

test.each([undefined, false])(
  "首次启用在 seeded=%s 时替换旧规则并按目录顺序种下全部规则",
  (seeded) => {
    const previous = frozenPreferences({ enabled: false, rules: ["synthetic-rule"], seeded });
    const next = fuzzyPinyinEnabledChange(previous, true);
    expect(next).toEqual({
      enabled: true,
      rules: [
        "z-zh",
        "c-ch",
        "s-sh",
        "n-l",
        "f-h",
        "r-l",
        "an-ang",
        "en-eng",
        "in-ing",
        "ian-iang",
        "uan-uang",
      ],
      seeded: true,
    });
    expect(previous.rules).toEqual(["synthetic-rule"]);
  },
);

test.each([{ rules: [] }, { rules: ["synthetic-rule", "z-zh"] }])(
  "再次启用保留刻意选择的规则及引用：%j",
  ({ rules }) => {
    const previous = frozenPreferences({ enabled: false, rules, seeded: true });
    const next = fuzzyPinyinEnabledChange(previous, true);
    expect(next).toEqual({ ...previous, enabled: true });
    expect(next.rules).toBe(previous.rules);
    expect(next).not.toBe(previous);
    expect(previous.enabled).toBe(false);
  },
);

test("关闭保留规则引用，且不写入原本缺省的 seeded", () => {
  const previous = frozenPreferences({ enabled: true, rules: ["z-zh"] });
  const next = fuzzyPinyinEnabledChange(previous, false);
  expect(next).toEqual({ enabled: false, rules: ["z-zh"] });
  expect(next.rules).toBe(previous.rules);
  expect(next).not.toHaveProperty("seeded");
});

test("勾选去重并排序，同时保留未知规则及缺省 seeded", () => {
  const previous = frozenPreferences({ enabled: false, rules: ["z-zh", "synthetic-rule", "z-zh"] });
  const next = fuzzyPinyinRuleChange(previous, "an-ang", true);
  expect(next).toEqual({ enabled: false, rules: ["an-ang", "synthetic-rule", "z-zh"] });
  expect(previous.rules).toEqual(["z-zh", "synthetic-rule", "z-zh"]);
  expect(fuzzyPinyinRuleChange(next, "an-ang", true)).toEqual(next);
});

test("取消勾选移除所有同名规则并保留种子标记", () => {
  const previous = frozenPreferences({
    enabled: true,
    rules: ["z-zh", "n-l", "z-zh"],
    seeded: true,
  });
  expect(fuzzyPinyinRuleChange(previous, "z-zh", false)).toEqual({
    enabled: true,
    rules: ["n-l"],
    seeded: true,
  });
});

test.each([undefined, false, true])("重置在 seeded=%s 时清空规则并明确保留种子状态", (seeded) => {
  const previous = frozenPreferences({ enabled: true, rules: ["z-zh"], seeded });
  expect(resetFuzzyPinyin(previous)).toEqual({
    enabled: false,
    rules: [],
    seeded: seeded ?? false,
  });
  expect(previous.rules).toEqual(["z-zh"]);
});

test("重置返回固定字段，普通开关和勾选保留额外元数据", () => {
  const previous = frozenPreferences({
    enabled: true,
    rules: ["z-zh"],
    seeded: true,
    synthetic: 1,
  } as FuzzyPinyinPreferences);
  expect(fuzzyPinyinEnabledChange(previous, false)).toHaveProperty("synthetic", 1);
  expect(fuzzyPinyinRuleChange(previous, "z-zh", false)).toHaveProperty("synthetic", 1);
  expect(resetFuzzyPinyin(previous)).not.toHaveProperty("synthetic");
});
