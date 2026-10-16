import { expectTypeOf, test } from "vitest";
import type { VoiceCredentialInput } from "@msime/ui";
import type { VoiceCredentialInput as SectionVoiceCredentialInput } from "../../../../packages/ui/src/settings/voice-credential-section";
import type { ProviderCredentialInput } from "../../../../packages/ui/src/settings/provider-credential-input";

test("语音凭据输入在包入口、组件和模型路径保持同一契约", () => {
  expectTypeOf<VoiceCredentialInput>().toEqualTypeOf<ProviderCredentialInput>();
  expectTypeOf<VoiceCredentialInput>().toEqualTypeOf<SectionVoiceCredentialInput>();
});
