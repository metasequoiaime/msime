export interface VoiceProviderOption {
  value: string;
  label: string;
}

/** Remote speech recognition providers shared by the settings selector and provider defaults. */
export const ASR_PROVIDER_OPTIONS: readonly VoiceProviderOption[] = [
  { value: "doubao", label: "豆包" },
  { value: "siliconflow", label: "SiliconFlow" },
  { value: "openai", label: "OpenAI" },
  { value: "groq", label: "Groq" },
  { value: "everyapi", label: "EveryAPI" },
  { value: "mistral", label: "Mistral · Voxtral" },
];

/** Text polishing providers shared by the settings selector and credential flow. */
export const POLISH_PROVIDER_OPTIONS: readonly VoiceProviderOption[] = [
  { value: "siliconflow", label: "SiliconFlow" },
  { value: "openai", label: "OpenAI" },
  { value: "deepseek", label: "DeepSeek" },
  { value: "groq", label: "Groq" },
];

/** Renders a provider option list while leaving host-specific options to the caller. */
export function VoiceProviderOptions({ options }: { options: readonly VoiceProviderOption[] }) {
  return options.map(({ value, label }) => (
    <option key={value} value={value}>
      {label}
    </option>
  ));
}
