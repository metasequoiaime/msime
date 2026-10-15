import { expect, test } from "vitest";
import { defaultFrequency as publicDefault, settingsInputPreferences } from "@msime/ui";
import { defaultFrequency as legacyDefault } from "../../../../packages/ui/src/settings/frequency-section";
import {
  defaultFrequency,
  frequencyValueOptions,
} from "../../../../packages/ui/src/settings/frequency-preferences";

test("纯偏好、旧组件导出和公共入口共用同一个默认对象", () => {
  expect(defaultFrequency).toEqual({ mode: "promote", trigger_count: 1, linear_step: 1 });
  expect(publicDefault).toBe(defaultFrequency);
  expect(legacyDefault).toBe(defaultFrequency);
  expect(settingsInputPreferences().frequency).toBe(defaultFrequency);
});

test.each([1, 4, 6])("常用范围内的当前值 %s 不重复追加", (current) => {
  expect(frequencyValueOptions(current)).toEqual([1, 2, 3, 4, 5, 6]);
});

test.each([7, 10, 100])("已保存的 %s 保持可选，不钳制到常用范围", (current) => {
  expect(frequencyValueOptions(current)).toEqual([1, 2, 3, 4, 5, 6, current]);
});

test("解析保留整份显式偏好和超出常用范围的值", () => {
  const frequency = { mode: "linear" as const, trigger_count: 10, linear_step: 7 };
  expect(settingsInputPreferences({ frequency }).frequency).toBe(frequency);
});
