import type { ProviderCredentialStatus } from "../core/provider-credential-contracts";
import { AiApiTokenSection } from "./ai-api-token-section";
import type { AiCredentialPreferences } from "./ai-credential-preferences";
import type { AiCredentialStored } from "./ai-credential-model";
import { aiCredentialSaveConfig } from "./ai-credential-save-config";
import { AiCredentialSection } from "./ai-credential-section";
import { AiLinuxProviderSection } from "./ai-linux-provider-section";
import { aiProviderUsesToken } from "./ai-provider-options";
import type { useProviderCredentials } from "./use-provider-credentials";

export interface AiCredentialControlProps extends Pick<
  ReturnType<typeof useProviderCredentials>,
  | "aiCredentialInput"
  | "providerCredentialBusy"
  | "providerCredentialMessages"
  | "runProviderCredential"
> {
  ai: AiCredentialPreferences;
  aiOrigin: string | null;
  aiToken: string;
  updateAiToken: (value: string) => void;
  setAiCredentialInput: (value: string) => void;
  linuxPlatform: boolean;
  providerCredentialAvailable: boolean;
  providerCredentials?: ProviderCredentialStatus;
  storedAiCredential?: AiCredentialStored;
}

/** 选择宿主的 AI 凭据界面，并将受控输入和保存、清除接到公共凭据状态。 */
export function AiCredentialControl({
  ai,
  aiOrigin,
  aiToken,
  updateAiToken,
  linuxPlatform,
  providerCredentialAvailable,
  providerCredentials,
  storedAiCredential,
  aiCredentialInput,
  setAiCredentialInput,
  providerCredentialBusy,
  providerCredentialMessages,
  runProviderCredential,
}: AiCredentialControlProps) {
  // 不鉴权的服务（Ollama）没有 Token 可填，整块不显示。
  if (!aiProviderUsesToken(ai.provider)) return null;
  if (linuxPlatform && providerCredentialAvailable) {
    return (
      <AiCredentialSection
        endpoint={ai.endpoint}
        model={ai.model}
        origin={aiOrigin}
        token={aiCredentialInput}
        stored={storedAiCredential}
        invalid={providerCredentials?.aiInvalid === true}
        busy={providerCredentialBusy === "ai"}
        message={providerCredentialMessages.ai}
        onTokenChange={setAiCredentialInput}
        onSave={() =>
          void runProviderCredential(
            "ai",
            (credentials) => credentials.saveAi(aiCredentialSaveConfig(ai, aiCredentialInput)),
            "凭据已保存，provider 服务下次请求时生效。",
          )
        }
        onClear={() =>
          void runProviderCredential(
            "ai",
            (credentials) => credentials.clearAi(ai.provider),
            "凭据已清除。",
          )
        }
      />
    );
  }
  return linuxPlatform ? (
    <AiLinuxProviderSection />
  ) : (
    <AiApiTokenSection
      origin={aiOrigin}
      endpoint={ai.endpoint}
      token={aiToken}
      onTokenChange={updateAiToken}
    />
  );
}
