import { expect, test } from "vitest";

test("voice panel reuses the shared client generation lifecycle", () => {
  const source = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/keyboard/panels.tsx", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];
  const start = source.indexOf("export function VoicePanel");
  const end = source.indexOf("export function CloudClipboardPanel", start);
  const voicePanel = source.slice(start, end);

  expect(voicePanel).toContain("const voiceGeneration = useAsyncGeneration(client)");
  expect(voicePanel).not.toContain("let active = true");
});
