import type { Dispatch, SetStateAction } from "react";
import type { Preferences, SettingsClient, VoiceInputPreferences } from "../index";
import { LocalModelManager, localModelInUse } from "../voice/local-models";
import { VoiceModelMirrorSection } from "./voice-model-mirror-section";
import { VoiceModelPathDisclosure } from "./voice-model-path-disclosure";
import { defaultVoiceInput } from "./voice-input-defaults";

type LocalModelConfirm = Parameters<typeof LocalModelManager>[0]["confirm"];

export interface VoiceLocalModelSettingsSectionProps {
  client: SettingsClient;
  localVoice: boolean;
  mobile: boolean;
  voiceInput: VoiceInputPreferences;
  setDraft: Dispatch<SetStateAction<Preferences | undefined>>;
  confirm: LocalModelConfirm;
  openExternalUrl: (url: string) => Promise<void>;
  updateVoice: (patch: Partial<VoiceInputPreferences>) => void;
}

/** Shared local voice model download, mirror, and path settings. */
export function VoiceLocalModelSettingsSection({
  client,
  localVoice,
  mobile,
  voiceInput,
  setDraft,
  confirm,
  openExternalUrl,
  updateVoice,
}: VoiceLocalModelSettingsSectionProps) {
  if (!localVoice) return null;

  return (
    <>
      {client.localVoiceModels && (
        <LocalModelManager
          client={client.localVoiceModels}
          mobile={mobile}
          modelPath={voiceInput.asr_model_path ?? ""}
          onUse={(asr_model_path) => updateVoice({ asr_model_path })}
          onRemoved={(model) =>
            // Checked against the draft as it is once the removal lands.
            setDraft((current) =>
              current && localModelInUse(model, current.voice_input?.asr_model_path ?? "")
                ? {
                    ...current,
                    voice_input: {
                      ...defaultVoiceInput,
                      ...current.voice_input,
                      asr_model_path: "",
                    },
                  }
                : current,
            )
          }
          confirm={confirm}
          openExternalUrl={client.openExternalUrl ? openExternalUrl : undefined}
        />
      )}
      {client.localVoiceModels && (
        <VoiceModelMirrorSection
          value={voiceInput.asr_model_mirror ?? ""}
          onChange={(asr_model_mirror) => updateVoice({ asr_model_mirror })}
        />
      )}
      <VoiceModelPathDisclosure
        disclosure={Boolean(client.localVoiceModels)}
        path={voiceInput.asr_model_path ?? ""}
        pickPath={client.pickVoiceModelPath}
        onChange={(asr_model_path) => updateVoice({ asr_model_path })}
      />
    </>
  );
}
