import { expect, test } from "vitest";

test("voice page reuses shared voice settings content", () => {
  const page = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/settings/pages/voice-page.tsx", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];
  expect(page).toContain('import { VoiceSettingsContent } from "../voice-settings-content";');
  expect(page).toContain("<VoiceSettingsContent");
  expect(page).not.toContain("<VoiceInputBasicsSection");
  expect(page).not.toContain("<VoicePolishSettingsSection");
});
