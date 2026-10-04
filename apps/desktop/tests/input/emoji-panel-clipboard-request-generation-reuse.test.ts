import { expect, test } from "vitest";

test("emoji panel clipboard requests reuse the shared request generation", () => {
  const source = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/keyboard/panels.tsx", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];
  const start = source.indexOf("export function EmojiPanel");
  const panel = source.slice(start);

  expect(panel).toContain("const clipboardGeneration = useAsyncGeneration()");
  expect(panel).not.toContain("const clipboardGeneration = useRef(0)");
});
