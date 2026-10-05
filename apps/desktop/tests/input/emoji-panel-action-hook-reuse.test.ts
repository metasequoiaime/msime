import { expect, test } from "vitest";

test("emoji panel clipboard actions reuse the shared panel action lifecycle", () => {
  const source = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/keyboard/panels.tsx", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];
  const start = source.indexOf("export function EmojiPanel");
  const panel = source.slice(start);

  expect(panel).toContain("busy: clipboardBusy");
  expect(panel).toContain("run: runOperation");
  expect(panel).toContain("invalidate");
  expect(panel).not.toContain("async function runOperation");
});
