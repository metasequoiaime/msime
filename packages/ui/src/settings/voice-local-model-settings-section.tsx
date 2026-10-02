import type { Dispatch, SetStateAction } from "react";
import type { Preferences, SettingsClient, VoiceInputPreferences } from "../index";
import { LocalModelManager, localModelInUse } from "../voice/local-models";
import { MoreOptions } from "../core/platform-controls";
import { VoiceModelMirrorRow } from "./voice-model-mirror-section";
import { VoiceModelPathSection } from "./voice-model-path-section";
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

/** Shared local voice model download, mirror, and path settings, inside the 识别服务配置 group: the model list is a block in the group, and the download mirror and manual directory sit under 更多选项. A host without a model store has only the manual directory, so it is the one entry and shows as a plain row. */
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

  const manager = client.localVoiceModels && (
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
  );
  const pathRow = (
    <VoiceModelPathSection
      path={voiceInput.asr_model_path ?? ""}
      pickPath={client.pickVoiceModelPath}
      onChange={(asr_model_path) => updateVoice({ asr_model_path })}
    />
  );
  return client.localVoiceModels ? (
    <>
      {manager}
      <MoreOptions>
        <VoiceModelMirrorRow
          value={voiceInput.asr_model_mirror ?? ""}
          onChange={(asr_model_mirror) => updateVoice({ asr_model_mirror })}
        />
        {pathRow}
      </MoreOptions>
    </>
  ) : (
    pathRow
  );
}
