import { expect, test } from "vitest";
import { candidateTabs } from "@msime/ui";

test("candidate tabs keep the shared order and labels", () => {
  expect(candidateTabs).toEqual([
    { id: "layout", title: "布局" },
    { id: "font", title: "字体与大小" },
    { id: "style", title: "窗口样式" },
    { id: "preedit", title: "预编辑" },
  ]);
});
