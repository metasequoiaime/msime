import { expectTypeOf, test } from "vitest";
import type { CredentialStatusMessageValue, ProviderCredentialMessage } from "@msime/ui";
import type { CredentialStatusMessageValue as ModelCredentialStatusMessageValue } from "../../../../packages/ui/src/settings/credential-status-message-model";

test("凭据状态消息在 hook、模型和包入口保持同一契约", () => {
  expectTypeOf<CredentialStatusMessageValue>().toEqualTypeOf<ModelCredentialStatusMessageValue>();
  expectTypeOf<ProviderCredentialMessage>().toEqualTypeOf<CredentialStatusMessageValue>();
});
