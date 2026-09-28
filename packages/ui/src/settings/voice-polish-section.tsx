import type { ReactNode } from "react";
import { SettingToggle } from "./setting-toggle";

export interface VoicePolishSectionProps {
  enabled: boolean;
  provider: string;
  model: string;
  providerPreset?: ReactNode;
  children?: ReactNode;
  onEnabledChange: (enabled: boolean) => void;
  onProviderChange: (provider: string) => void;
  onModelChange: (model: string) => void;
}

const providers = [
  ["siliconflow", "SiliconFlow"],
  ["openai", "OpenAI"],
  ["deepseek", "DeepSeek"],
  ["groq", "Groq"],
] as const;

/** Shared voice text-polish provider controls and extension slot for credentials and prompts. */
export function VoicePolishSection({
  enabled,
  provider,
  model,
  providerPreset,
  children,
  onEnabledChange,
  onProviderChange,
  onModelChange,
}: VoicePolishSectionProps) {
  return (
    <div className="section">
      <div className="section-title">
        文本润色 provider<small>识别结果可交给用户管理的服务润色</small>
      </div>
      <SettingToggle
        label="启用润色"
        ariaLabel="启用文本润色"
        checked={enabled}
        compact
        onChange={onEnabledChange}
      />
      <label className="section-header">
        <span className="section-title">服务提供商</span>
        <select
          aria-label="文本润色服务提供商"
          value={provider}
          onChange={(event) => onProviderChange(event.target.value)}
        >
          {providers.map(([value, label]) => (
            <option key={value} value={value}>
              {label}
            </option>
          ))}
        </select>
      </label>
      {providerPreset}
      <label className="section-header">
        <span className="section-title">模型</span>
        <input
          aria-label="文本润色模型"
          value={model}
          onChange={(event) => onModelChange(event.target.value)}
        />
      </label>
      {children}
    </div>
  );
}
