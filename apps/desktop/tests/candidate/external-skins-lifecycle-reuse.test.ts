import { expect, test } from "vitest";

test("external skin catalog reuses the shared owner lifecycles", () => {
  const source = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/skin/external-skins.tsx", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];

  expect(source).toContain("useAsyncGeneration(scan)");
  expect(source).toContain("useAsyncGeneration(openDirectory)");
  expect(source).not.toContain("const generation = useRef(0)");
  expect(source).not.toContain("const openGeneration = useRef(0)");
});
