import { expect, test } from "vitest";

test("macOS input mode refreshes reuse the shared request generation", () => {
  const source = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/settings/use-macos-input-modes.ts", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];

  expect(source).toContain("const request = useAsyncGeneration(client)");
  expect(source).not.toContain("const request = useRef(0)");
});
