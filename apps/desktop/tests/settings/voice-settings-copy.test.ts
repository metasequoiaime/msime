import { expect, test } from "vitest";

test("voice page and panel reuse shared voice settings content", () => {
  const page = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/settings/pages/voice-page.tsx", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];
  const panel = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/settings/voice-settings-panel.tsx", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];

  expect(page).toContain('import { VoiceSettingsContent } from "../voice-settings-content";');
  expect(panel).toContain("VoiceSettingsContent, type VoiceSettingsContentProps");
  expect(panel).toContain('from "./voice-settings-content";');
  expect(page).toContain("<VoiceSettingsContent");
  expect(panel).toContain("<VoiceSettingsContent");
  expect(page).not.toContain("<VoiceInputBasicsSection");
  expect(panel).not.toContain("<VoiceInputBasicsSection");
  expect(page).not.toContain("<VoicePolishSettingsSection");
  expect(panel).not.toContain("<VoicePolishSettingsSection");
});
