import { expect, test } from "vitest";
import { expressionTabs } from "@msime/ui";

test("expression tabs keep the shared order and labels", () => {
  expect(expressionTabs).toEqual([
    { id: "punctuation", title: "标点" },
    { id: "gloss", title: "候选释义" },
    { id: "providers", title: "翻译服务" },
  ]);
});
