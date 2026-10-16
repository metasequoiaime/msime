import { expectTypeOf, test } from "vitest";
import type {
  ProviderCredentialClient as PublicClient,
  ProviderCredentialStatus as PublicStatus,
  VoiceCredentialKind as PublicKind,
  VoiceCredentialSaveResult as PublicSaveResult,
  VoiceProviderCredential as PublicVoiceCredential,
  VoiceCredentialSaveInput as PublicVoiceSaveInput,
} from "@msime/ui";
import type {
  ProviderCredentialClient,
  ProviderCredentialStatus,
  VoiceCredentialKind,
  VoiceCredentialSaveResult,
  VoiceProviderCredential,
} from "../../../../packages/ui/src/core/provider-credential-contracts";
import type {
  VoiceCredentialEntry,
  VoiceCredentialSaveInput,
  VoiceCredentialSectionKind,
  VoiceCredentialStatus,
} from "../../../../packages/ui/src/settings/voice-credential-section";

test("服务商凭据叶子契约与包入口保持一致", () => {
  expectTypeOf<ProviderCredentialClient>().toEqualTypeOf<PublicClient>();
  expectTypeOf<ProviderCredentialStatus>().toEqualTypeOf<PublicStatus>();
  expectTypeOf<VoiceCredentialKind>().toEqualTypeOf<PublicKind>();
  expectTypeOf<VoiceCredentialSaveResult>().toEqualTypeOf<PublicSaveResult>();
  expectTypeOf<VoiceProviderCredential>().toEqualTypeOf<PublicVoiceCredential>();
});

test("语音凭据表单复用服务商条目、种类和保存参数", () => {
  expectTypeOf<VoiceCredentialEntry>().toEqualTypeOf<VoiceProviderCredential>();
  expectTypeOf<VoiceCredentialSectionKind>().toEqualTypeOf<VoiceCredentialKind>();
  expectTypeOf<VoiceCredentialSaveInput>().toEqualTypeOf<
    Parameters<ProviderCredentialClient["saveVoice"]>[0]
  >();
  expectTypeOf<VoiceCredentialSaveInput>().toEqualTypeOf<PublicVoiceSaveInput>();
});

test("凭据快照可用于只读表单而不收窄宿主返回类型", () => {
  expectTypeOf<ProviderCredentialStatus>().toExtend<VoiceCredentialStatus>();
  expectTypeOf<VoiceCredentialStatus["voiceAsr"]>().toEqualTypeOf<
    readonly VoiceProviderCredential[]
  >();
  expectTypeOf<ProviderCredentialStatus["voiceAsr"]>().toEqualTypeOf<VoiceProviderCredential[]>();
});
