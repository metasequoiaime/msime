// @vitest-environment jsdom
import { expect, test, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { SettingsVoiceAiPages } from "@msime/ui";

vi.mock("../../../../packages/ui/src/settings/voice-settings-panel", () => ({
  VoiceSettingsPanel: () => <section aria-label="语音测试面板" />,
}));
vi.mock("../../../../packages/ui/src/settings/ai-settings-panel", () => ({
  AiSettingsPanel: () => <section aria-label="AI 测试面板" />,
}));

test("composes voice and AI settings panels", () => {
  render(<SettingsVoiceAiPages voice={{} as never} ai={{} as never} />);

  expect(screen.getByRole("region", { name: "语音测试面板" })).toBeTruthy();
  expect(screen.getByRole("region", { name: "AI 测试面板" })).toBeTruthy();
});
