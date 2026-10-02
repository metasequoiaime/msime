import type { Dispatch, SetStateAction } from "react";
import type { Preferences, SettingsClient, VoiceInputPreferences } from "../index";
import { LocalModelManager, localModelInUse } from "../voice/local-models";
import { MoreOptions } from "../core/platform-controls";
import { VoiceModelMirrorRow, VoiceModelMirrorSection } from "./voice-model-mirror-section";
import { VoiceModelPathDisclosure } from "./voice-model-path-disclosure";
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
  /** 语音页把本地模型放进「识别服务配置」组：模型列表画成组内的一块，下载镜像和手动目录收进「更多选项」。没有模型商店的宿主只有手动目录，它就是唯一的入口，直接显示成一行。 */
  grouped?: boolean;
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
  grouped = false,
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
      grouped={grouped}
    />
  );
  if (grouped) {
    const pathRow = (
      <VoiceModelPathSection
        grouped
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

  return (
    <>
      {manager}
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
