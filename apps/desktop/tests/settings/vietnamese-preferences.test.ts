import { expect, test } from "vitest";
import { resolveVietnamesePreferences } from "../../../../packages/ui/src/settings/vietnamese-preferences";

test("越南语偏好缺省时使用 Telex 和新式声调", () => {
  expect(resolveVietnamesePreferences()).toEqual({
    input_method: "telex",
    tone_style: "modern",
  });
});

test("越南语偏好逐项覆盖缺省值", () => {
  expect(resolveVietnamesePreferences({ input_method: "vni" })).toEqual({
    input_method: "vni",
    tone_style: "modern",
  });
  expect(resolveVietnamesePreferences({ tone_style: "classic" })).toEqual({
    input_method: "telex",
    tone_style: "classic",
  });
  expect(resolveVietnamesePreferences({ input_method: "vni", tone_style: "classic" })).toEqual({
    input_method: "vni",
    tone_style: "classic",
  });
});
