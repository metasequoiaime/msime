import { expect, test } from "vitest";

test("settings persistence subscriptions reuse the shared owner generation lifecycle", () => {
  const source = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/settings/use-settings-persistence.ts", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];
  const start = source.indexOf("if (!client.onPreferencesChanged) return;");
  const end = source.indexOf("  useEffect(() => {\n    let active = true;", start);
  const subscription = source.slice(start, end);

  expect(source).toContain("useAsyncGeneration(client)");
  expect(subscription).toContain("generation.current");
  expect(subscription).not.toContain("let active = true");
});
