import { expect, test } from "vitest";

test("emoji panel clipboard requests reuse the shared request generation", () => {
  const source = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/keyboard/use-emoji-panel.ts", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];
  const start = source.indexOf("export function useEmojiPanel");
  const panel = source.slice(start);

  expect(panel).toContain("const clipboardGeneration = useAsyncGeneration()");
  expect(panel).not.toContain("const clipboardGeneration = useRef(0)");
});
