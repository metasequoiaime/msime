import { expect, test } from "vitest";
import { communityPublishFields } from "@msime/ui";

test("community publish fields trim values and enforce publish limits", () => {
  expect(communityPublishFields("  我的皮肤  ", "  一段说明  ")).toEqual({
    normalizedName: "我的皮肤",
    normalizedDescription: "一段说明",
    nameValid: true,
    descriptionValid: true,
  });

  expect(communityPublishFields("", "x".repeat(281))).toMatchObject({
    nameValid: false,
    descriptionValid: false,
  });
});

test("community publish fields reject names that exceed grapheme limits", () => {
  expect(communityPublishFields("😀".repeat(33), "说明").nameValid).toBe(false);
});
