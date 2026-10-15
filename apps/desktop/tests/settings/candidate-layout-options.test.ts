import { expect, test } from "vitest";
import { candidateLayoutOptions } from "@msime/ui";

test("candidate layout options keep the shared order and labels", () => {
  expect(candidateLayoutOptions).toEqual([
    { value: "horizontal", label: "横向" },
    { value: "vertical", label: "纵向" },
  ]);
});
