import { expect, test } from "vitest";

test("emoji panel clipboard subscription reuses the shared effect generation lifecycle", () => {
  const source = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/keyboard/panels.tsx", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];
  const start = source.indexOf("if (!client.clipboard?.list)");
  const end = source.indexOf("  type DisplayGroup", start);
  const effect = source.slice(start, end);

  expect(source.slice(source.indexOf("export function EmojiPanel"), start)).toContain(
    "const clipboardLifecycle = useAsyncGeneration(client, page, clipboardRefresh)",
  );
  expect(effect).not.toContain("let active = true");
});
