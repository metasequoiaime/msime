import type {
  ProviderCredentialStatus,
  VoiceCredentialKind,
} from "../core/provider-credential-contracts";
import type { VoiceInputPreferences } from "../index";
import { VoiceCredentialSection } from "./voice-credential-section";
import type { VoiceCredentialSaveInput } from "./voice-credential-model";
import type { VoiceCredentialBindings } from "./use-provider-credentials";

export interface VoiceCredentialControlProps extends VoiceCredentialBindings {
  available: boolean;
  kind: VoiceCredentialKind;
  voiceInput: VoiceInputPreferences;
  doubaoAuthMode: "api_key" | "legacy";
  providerCredentials?: ProviderCredentialStatus;
}

/** Connects the Linux voice credential state to the shared credential editor. */
export function VoiceCredentialControl({
  available,
  kind,
  voiceInput,
  doubaoAuthMode,
  providerCredentials,
  voiceCredentialInput,
  setVoiceCredentialInput,
  providerCredentialBusy,
  providerCredentialMessages,
  runVoiceCredential,
}: VoiceCredentialControlProps) {
  if (!available) return null;
  const provider =
    kind === "asr"
      ? (voiceInput.asr_provider ?? "doubao")
      : (voiceInput.polish_provider ?? "siliconflow");
  return (
    <VoiceCredentialSection
      kind={kind}
      provider={provider}
      model={(kind === "asr" ? voiceInput.asr_model : voiceInput.polish_model) ?? ""}
      resourceId={kind === "asr" ? voiceInput.asr_resource_id : undefined}
      authMode={kind === "asr" ? doubaoAuthMode : undefined}
      credentials={providerCredentials}
      input={voiceCredentialInput[kind]}
      busy={providerCredentialBusy === kind}
      message={providerCredentialMessages[kind]}
      onChange={(patch) =>
        setVoiceCredentialInput((current) => ({
          ...current,
          [kind]: { ...current[kind], ...patch },
        }))
      }
      onSave={(credential: VoiceCredentialSaveInput) =>
        void runVoiceCredential(
          kind,
          (credentials) => credentials.saveVoice(credential),
          "凭据已保存，语音服务下次请求时生效。",
        )
      }
      onClear={() =>
        void runVoiceCredential(
          kind,
          (credentials) => credentials.clearVoice(kind, provider),
          "凭据已清除。",
        )
      }
    />
  );
}
