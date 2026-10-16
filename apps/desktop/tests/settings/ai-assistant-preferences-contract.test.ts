import { expectTypeOf, test } from "vitest";
import type { AiAssistantPreferences } from "@msime/ui";
import type { AiAssistantPreferences as AiAssistantPreferencesModel } from "../../../../packages/ui/src/settings/ai-assistant-preferences";

test("AI 助手偏好公共模型与总偏好保持一致", () => {
  expectTypeOf<AiAssistantPreferencesModel>().toEqualTypeOf<AiAssistantPreferences>();
});
