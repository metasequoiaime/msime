import { useEffect, useRef, type Dispatch, type SetStateAction } from "react";
import type { VoiceInputPreferences } from "./voice-input-preferences";
import type { SettingsClient } from "../core/settings-client";
import type { Preferences } from "../core/preferences-model";
import { LocalModelManager, localModelInUse } from "../voice/local-models";
import { MoreOptions } from "../core/platform-controls";
import { VoiceModelMirrorRow } from "./voice-model-mirror-section";
import { VoiceModelPathSection } from "./voice-model-path-section";
import { defaultVoiceInput } from "./voice-input-defaults";
import { ResourcePackRow, useResourcePacks } from "./resource-packs";

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

/** Shared local voice model download, mirror, and path settings, inside the 识别服务配置 group: the model list is a block in the group, and the download mirror and manual directory sit under 更多选项. A host without a model store has only the manual directory, so it is the one entry and shows as a plain row.
 *
 * 输入法不带本地语音运行库时（只有省略了它的 macOS 发布包，宿主的资源包列表里才有 `voice-runtime-macos`），选用本地识别就开始下载运行库，模型列表上方显示它的下载行；宿主不列出这个资源包时什么也不显示。 */
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
  const runtimePacks = useResourcePacks(client.resourcePacks, {
    value: voiceInput.asr_model_mirror ?? "",
    onChange: (asr_model_mirror) => updateVoice({ asr_model_mirror }),
  });
  // 只在用户刚选用本地识别时下载；打开页面时不下载，已保存的选择由宿主启动时补齐。下载失败后不自动重试，等用户点下载行的「重试」。
  const wasLocal = useRef(localVoice);
  useEffect(() => {
    if (localVoice && !wasLocal.current) runtimePacks.ensure("voice-runtime-macos");
    wasLocal.current = localVoice;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [localVoice]);
  if (!localVoice) return null;

  const runtimeRow = (
    <ResourcePackRow packs={runtimePacks} id="voice-runtime-macos" note="本地识别需要它" />
  );

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
      {runtimeRow}
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
    <>
      {runtimeRow}
      {pathRow}
    </>
  );
}
