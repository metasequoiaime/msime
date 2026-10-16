import { expectTypeOf, test } from "vitest";
import type { AiAssistantPreferences } from "@msime/ui";
import type { AiCredentialPreferences } from "../../../../packages/ui/src/settings/ai-credential-preferences";

test("AI 凭据公共偏好投影与设置模型保持一致", () => {
  expectTypeOf<AiCredentialPreferences>().toEqualTypeOf<
    Pick<AiAssistantPreferences, "provider" | "endpoint" | "model">
  >();
});
