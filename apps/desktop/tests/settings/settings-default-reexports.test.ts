import { expect, test } from "vitest";
import { defaultAiAssistant as dedicatedAiAssistant } from "../../../../packages/ui/src/settings/ai-assistant-defaults";
import { defaultVoiceInput as dedicatedVoiceInput } from "../../../../packages/ui/src/settings/voice-input-defaults";
import {
  defaultAiAssistant as optionsAiAssistant,
  defaultVoiceInput as optionsVoiceInput,
} from "../../../../packages/ui/src/settings/settings-options";

test("settings options re-export the dedicated AI and voice defaults", () => {
  expect(optionsAiAssistant).toBe(dedicatedAiAssistant);
  expect(optionsVoiceInput).toBe(dedicatedVoiceInput);
});
