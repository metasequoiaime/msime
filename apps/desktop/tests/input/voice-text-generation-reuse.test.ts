import { expect, test } from "vitest";

test("语音文本修订复用公共请求代次", () => {
  const panel = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/keyboard/use-voice-panel.ts", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];

  expect(panel).toContain("const textRevision = useAsyncGeneration()");
  expect(panel).not.toContain("const textRevision = useRef(0)");
});
