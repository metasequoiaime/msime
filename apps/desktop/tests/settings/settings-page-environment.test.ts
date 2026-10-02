import { testHost } from "../support/host";
import { expect, test } from "vitest";
import { settingsPageEnvironment } from "@msime/ui";

test("combines host context and capability projection", () => {
  const environment = settingsPageEnvironment({
    load: async () => ({}) as never,
    save: async () => ({}) as never,
    host: testHost({ platform: "macos" }),
  });

  expect(environment.macos).toBe(true);
  expect(environment.mobile).toBe(false);
  expect(environment.showFullwidthChord).toBe(true);
});
