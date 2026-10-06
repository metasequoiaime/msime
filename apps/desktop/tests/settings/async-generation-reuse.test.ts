import { expect, test } from "vitest";

test("settings async hooks reuse the shared generation lifecycle", () => {
  const sources = [
    Object.values(
      import.meta.glob<string>("../../../../packages/ui/src/settings/use-app-version.ts", {
        eager: true,
        query: "?raw",
        import: "default",
      }),
    )[0],
    Object.values(
      import.meta.glob<string>("../../../../packages/ui/src/settings/use-window-state.ts", {
        eager: true,
        query: "?raw",
        import: "default",
      }),
    )[0],
  ];

  for (const source of sources) {
    expect(source).toContain("useAsyncGeneration(");
    expect(source).not.toContain("const generation = useRef(0)");
  }
});
