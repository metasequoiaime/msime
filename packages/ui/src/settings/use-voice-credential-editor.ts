import {
  voiceCredentialEditorModel,
  voiceCredentialSavePayload,
  type VoiceCredentialEditorModel,
  type VoiceCredentialEditorModelOptions,
  type VoiceCredentialInput,
  type VoiceCredentialSaveInput,
} from "./voice-credential-model";
import { useCustomEndpointIntent } from "./use-custom-endpoint-intent";

export interface UseVoiceCredentialEditorOptions extends VoiceCredentialEditorModelOptions {
  onChange: (patch: Partial<VoiceCredentialInput>) => void;
  onSave: (credential: VoiceCredentialSaveInput) => void;
  onClear: () => void;
}

export interface VoiceCredentialEditorState extends VoiceCredentialEditorModel {
  setCustomEndpoint: (custom: boolean) => void;
  selectEndpoint: (endpoint: string) => void;
  editEndpoint: (endpoint: string) => void;
  editToken: (token: string) => void;
  editAppKey: (appKey: string) => void;
  save: () => void;
  clear: () => void;
}

/** 受控语音凭据编辑动作；自定义接口的手动展开意图只属于当前类别和服务。 */
export function useVoiceCredentialEditor(
  options: UseVoiceCredentialEditorOptions,
): VoiceCredentialEditorState {
  const { kind, provider, onChange, onSave, onClear } = options;
  const model = voiceCredentialEditorModel(options);
  const { custom: customEndpoint, setCustom: setCustomEndpoint } = useCustomEndpointIntent({
    baseCustom: model.customEndpoint,
    resetKey: `${kind}:${provider}`,
  });
  return {
    ...model,
    customEndpoint,
    setCustomEndpoint,
    selectEndpoint: (endpoint) => onChange({ endpoint }),
    editEndpoint: (endpoint) => {
      // 输入预设地址时也保持输入框，避免打到一半控件消失。
      if (model.doubao) setCustomEndpoint(true);
      onChange({ endpoint });
    },
    editToken: (token) => onChange({ token }),
    editAppKey: (appKey) => onChange({ appKey }),
    save: () => {
      if (!model.saveDisabled) onSave(voiceCredentialSavePayload(options));
    },
    clear: () => {
      if (!model.clearDisabled && model.stored) onClear();
    },
  };
}
