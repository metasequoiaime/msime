import { expect, test } from "vitest";

test("云剪贴板草稿复用公共代次", () => {
  const panel = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/keyboard/use-cloud-clipboard-panel.ts", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];

  expect(panel).toContain("const draftRevision = useAsyncGeneration()");
  expect(panel).not.toContain("const draftRevision = useRef(0)");
});
