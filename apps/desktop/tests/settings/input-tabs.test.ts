import { expect, test } from "vitest";
import { inputTabs } from "@msime/ui";

test("input tabs keep the shared order and labels", () => {
  expect(inputTabs).toEqual([
    { id: "scheme", title: "输入方案" },
    { id: "mode", title: "中英模式" },
    { id: "selection", title: "选词与翻页" },
    { id: "candidates", title: "候选与联想" },
    { id: "output", title: "输出设置" },
    { id: "advanced", title: "高级选项" },
  ]);
});
