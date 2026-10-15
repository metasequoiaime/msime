import { expect, test } from "vitest";
import { expressionTabs } from "@msime/ui";

test("expression tabs keep the shared order and labels", () => {
  expect(expressionTabs).toEqual([
    { id: "punctuation", title: "标点" },
    { id: "language", title: "候选与释义" },
    { id: "translation", title: "候选翻译" },
    { id: "providers", title: "翻译服务" },
  ]);
});
