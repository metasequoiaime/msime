import { expect, test } from "vitest";

test("cloud candidates input reuses the shared latest ref", () => {
  const source = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/keyboard/use-cloud-candidates-panel.ts", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];
  expect(source).toContain("const textRef = useLatestRef(text)");
  expect(source).not.toContain('const textRef = useRef("")');
});
