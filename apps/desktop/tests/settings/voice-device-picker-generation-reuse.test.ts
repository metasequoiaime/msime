import { expect, test } from "vitest";

test("voice device refresh reuses the shared async action lifecycle", () => {
  const source = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/voice/voice-device-picker.tsx", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];

  expect(source).toContain("useAsyncActionRunner(");
  expect(source).not.toContain("const pending = useRef(false)");
});
