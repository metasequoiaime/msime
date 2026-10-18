import { expect, test } from "vitest";

test("语音客户端代次复用公共异步生命周期", () => {
  const voicePanel = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/keyboard/use-voice-panel.ts", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];

  expect(voicePanel).toContain("const voiceGeneration = useAsyncGeneration(client)");
  expect(voicePanel).not.toContain("let active = true");
});
