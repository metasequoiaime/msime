import { expect, test } from "vitest";

test("AI 抽卡组件委托公共状态，状态复用异步 runner 与进度代次", () => {
  const component = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/keyboard/ai-skin-generation.tsx", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];
  const state = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/keyboard/use-ai-skin-generation.ts", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];
  expect(component).toContain("useAiSkinGeneration(");
  expect(component).not.toContain("useState(");
  expect(state).toContain("useAsyncActionRunner(");
  expect(state).toContain("useAsyncGeneration(client, library, communitySkins, requestId)");
  expect(state).not.toContain("let active = true");
  expect(state).not.toContain("const mounted = useRef(true)");
});
