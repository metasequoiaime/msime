import { expect, test } from "vitest";

test("cloud clipboard availability reuses the shared generation lifecycle", () => {
  const source = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/settings/clipboard-history-section.tsx", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];

  expect(source).toContain("const cloudRevision = useAsyncGeneration(cloudRequest, historyShown)");
  expect(source).not.toContain("const cloudRevision = useRef(0)");
});
