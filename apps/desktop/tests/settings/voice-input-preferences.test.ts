import { expect, expectTypeOf, test } from "vitest";
import type { Preferences, VoiceInputPreferences } from "@msime/ui";
import { defaultVoiceInput as sharedDefault } from "../../../../packages/ui/src/settings/voice-input-preferences";
import { defaultVoiceInput as legacyDefault } from "../../../../packages/ui/src/settings/voice-input-defaults";
import type { VoiceInputPreferences as SharedVoiceInputPreferences } from "../../../../packages/ui/src/settings/voice-input-preferences";

test("语音输入偏好公共模型与总偏好保持一致", () => {
  expectTypeOf<SharedVoiceInputPreferences>().toEqualTypeOf<VoiceInputPreferences>();
  expectTypeOf<SharedVoiceInputPreferences>().toEqualTypeOf<
    NonNullable<Preferences["voice_input"]>
  >();
  expect(sharedDefault).toBe(legacyDefault);
  expect(sharedDefault).toEqual({
    enabled: true,
    language: "zh-CN",
    asr_provider: "doubao",
    doubao_auth_mode: "api_key",
    asr_resource_id: "volc.seedasr.sauc.duration",
  });
});
