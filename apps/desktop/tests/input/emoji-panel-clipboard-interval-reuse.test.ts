import { expect, test } from "vitest";

test("emoji panel clipboard polling reuses the shared interval subscription", () => {
  const source = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/keyboard/use-emoji-panel.ts", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];
  const start = source.indexOf("if (!client.clipboard?.list)");
  const end = source.indexOf("  type DisplayGroup", start);
  const effect = source.slice(start, end);

  expect(effect).toContain("subscribeToInterval");
  expect(effect).not.toContain("setInterval(pollVisible");
  expect(effect).not.toContain("clearInterval(poll)");
});
