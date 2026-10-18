import { expect, test } from "vitest";

test("cloud dictionary search reuses the shared latest ref", () => {
  const source = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/keyboard/use-cloud-dictionary-panel.ts", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];
  const panel = source;

  expect(panel).toContain("const searchRef = useLatestRef(search)");
  expect(panel).not.toContain('const searchRef = useRef("")');
});
