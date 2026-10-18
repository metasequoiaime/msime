import { expect, test } from "vitest";

test("云剪贴板搜索复用公共最新值引用", () => {
  const panel = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/keyboard/use-cloud-clipboard-panel.ts", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];

  expect(panel).toContain("const searchRef = useLatestRef(search)");
  expect(panel).not.toContain('const searchRef = useRef("")');
});
