import { expect, test } from "vitest";

test("macOS input source status refreshes reuse a dedicated shared generation", () => {
  const source = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/settings/use-macos-settings.ts", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];

  expect(source).toContain("const inputSourceRequest = useAsyncGeneration(client, macos)");
  expect(source).not.toContain("const inputSourceRequest = useRef(0)");
});
