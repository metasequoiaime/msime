import { expect, test } from "vitest";

test("语音提交复用公共请求代次", () => {
  const panel = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/keyboard/use-voice-panel.ts", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];

  expect(panel).toContain("const submissionRevision = useAsyncGeneration()");
  expect(panel).not.toContain("const submissionRevision = useRef(0)");
});
