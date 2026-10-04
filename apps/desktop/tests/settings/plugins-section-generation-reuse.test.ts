import { expect, test } from "vitest";

test("plugins section reuses the shared owner generation lifecycle", () => {
  const source = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/settings/plugins-section.tsx", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];

  expect(source).toContain("useAsyncGeneration(active, client, mentionsEditable)");
  expect(source).not.toContain("const clientGeneration = useRef(0)");
  expect(source).not.toContain("const generation = ++clientGeneration.current");
});
