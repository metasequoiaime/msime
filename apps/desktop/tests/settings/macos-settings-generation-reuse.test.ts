import { expect, test } from "vitest";

test("macOS settings reads reuse the shared async generation lifecycle", () => {
  const source = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/settings/use-macos-settings.ts", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];

  expect(source).toContain("useAsyncGeneration(client, macos)");
  expect(source).not.toContain("let active = true");
});
