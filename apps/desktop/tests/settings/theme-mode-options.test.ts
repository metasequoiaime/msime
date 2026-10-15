import { expect, test } from "vitest";
import { themeModeOptions } from "@msime/ui";

test("theme mode options keep the shared order and labels", () => {
  expect(themeModeOptions).toEqual([
    { value: "system", label: "跟随系统" },
    { value: "light", label: "浅色" },
    { value: "dark", label: "深色" },
  ]);
});
