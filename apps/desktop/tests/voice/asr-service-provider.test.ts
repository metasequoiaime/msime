import { expect, test } from "vitest";
import { isAsrServiceProvider } from "@msime/ui";

test("recognizes credential-backed ASR service providers", () => {
  expect(isAsrServiceProvider("openai")).toBe(true);
  expect(isAsrServiceProvider("doubao")).toBe(true);
  expect(isAsrServiceProvider("local")).toBe(false);
  expect(isAsrServiceProvider("unknown-provider")).toBe(false);
});
