import { expect, test } from "vitest";

test("settings persistence saves reuse the shared save generation", () => {
  const source = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/settings/use-settings-persistence.ts", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];

  expect(source).toContain("const saveTokenRef = useAsyncGeneration()");
  expect(source).not.toContain("const saveTokenRef = useRef(0)");
});
