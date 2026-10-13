import { ModelPickerRows } from "./model-picker-rows";

export interface VoiceModelSectionProps {
  /** 识别服务预置的模型。 */
  models: readonly string[];
  value: string;
  onChange: (value: string) => void;
}

/** Provider-selected speech recognition model, a row of the 识别服务配置 group: the provider's known models in one list, with a field for anything else. */
export function VoiceModelSection({ models, value, onChange }: VoiceModelSectionProps) {
  return (
    <ModelPickerRows
      title="识别模型"
      description="由语音服务选择对应模型"
      label="识别模型"
      models={models}
      model={value}
      onChange={onChange}
    />
  );
}
