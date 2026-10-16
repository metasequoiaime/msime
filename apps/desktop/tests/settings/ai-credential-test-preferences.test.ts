import { expectTypeOf, test } from "vitest";
import type { AiAssistantPreferences } from "@msime/ui";
import type { AiCredentialTestPreferences } from "../../../../packages/ui/src/settings/ai-credential-preferences";

test("AI 凭据测试条件使用最小的公共偏好投影", () => {
  expectTypeOf<AiCredentialTestPreferences>().toEqualTypeOf<
    Pick<AiAssistantPreferences, "enabled" | "model">
  >();
});
