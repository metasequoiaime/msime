import { expect, test } from "vitest";

test("emoji panel catalog loading reuses the shared client generation lifecycle", () => {
  const source = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/keyboard/panels.tsx", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];
  const start = source.indexOf("export function EmojiPanel");
  const catalogStart = source.indexOf("if (!client.loadCatalog)", start);
  const catalogEnd = source.indexOf("  useEffect(() => {", catalogStart + 1);
  const catalogEffect = source.slice(catalogStart, catalogEnd);

  expect(source.slice(start, catalogStart)).toContain(
    "const catalogGeneration = useAsyncGeneration(client, catalogRetry)",
  );
  expect(catalogEffect).not.toContain("let active = true");
});
