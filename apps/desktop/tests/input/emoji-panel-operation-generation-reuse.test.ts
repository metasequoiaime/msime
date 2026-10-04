import { expect, test } from "vitest";

test("emoji panel operations reuse the shared action generation", () => {
  const source = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/keyboard/panels.tsx", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];
  const start = source.indexOf("export function EmojiPanel");
  const panel = source.slice(start);

  expect(panel).toContain("const operationRevision = useAsyncGeneration()");
  expect(panel).not.toContain("const operationRevision = useRef(0)");
});
