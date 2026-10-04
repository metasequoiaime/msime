import { expect, test } from "vitest";

test("voice device refresh reuses the shared owner generation lifecycle", () => {
  const source = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/voice/voice-device-picker.tsx", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];

  expect(source).toContain("useAsyncGeneration(read)");
  expect(source).not.toContain("const revision = useRef(0)");
});
