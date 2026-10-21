import { voiceProviderCatalog } from "./voice-provider-catalog";

export interface VoiceProviderOption {
  value: string;
  label: string;
}

/** 识别选项保持共享目录的顺序，平台特有选项由调用方追加。 */
export const ASR_PROVIDER_OPTIONS: readonly VoiceProviderOption[] = voiceProviderCatalog.asr.map(
  ({ id, title }) => ({ value: id, label: title }),
);

/** 润色服务的标题与默认值读取同一目录。 */
export const POLISH_PROVIDER_OPTIONS: readonly VoiceProviderOption[] =
  voiceProviderCatalog.polish.map(({ id, title }) => ({ value: id, label: title }));

/** Renders a provider option list while leaving host-specific options to the caller. */
export function VoiceProviderOptions({ options }: { options: readonly VoiceProviderOption[] }) {
  return options.map(({ value, label }) => (
    <option key={value} value={value}>
      {label}
    </option>
  ));
}
