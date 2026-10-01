import type { ReactNode } from "react";
import { SettingField } from "./setting-field";
import { SettingToggle } from "./setting-toggle";

export type AiProviderOption = { id: string; title: string };

export function AiBasicSettingsSection({
  enabled,
  enabledDescription,
  provider,
  providerOptions,
  model,
  endpoint,
  providerPreset,
  onEnabledChange,
  onProviderChange,
  onModelChange,
  onEndpointChange,
}: {
  enabled: boolean;
  enabledDescription: string;
  provider: string;
  providerOptions: readonly AiProviderOption[];
  model: string;
  endpoint: string;
  providerPreset?: ReactNode;
  onEnabledChange: (enabled: boolean) => void;
  onProviderChange: (provider: string) => void;
  onModelChange: (model: string) => void;
  onEndpointChange: (endpoint: string) => void;
}) {
  return (
    <>
      <SettingToggle
        label="启用 AI 辅助"
        description={enabledDescription}
        ariaLabel="启用 AI 辅助"
        checked={enabled}
        onChange={onEnabledChange}
      />
      <div className="section">
        <SettingField label="服务提供商">
          <select
            aria-label="AI 服务提供商"
            value={provider}
            onChange={(event) => onProviderChange(event.target.value)}
          >
            {providerOptions.map((option) => (
              <option key={option.id} value={option.id}>
                {option.title}
              </option>
            ))}
          </select>
        </SettingField>
      </div>
      {providerPreset}
      <div className="section">
        <SettingField label="模型">
          <input
            aria-label="AI 模型"
            value={model}
            onChange={(event) => onModelChange(event.target.value)}
          />
        </SettingField>
      </div>
      <div className="section">
        <SettingField label="接口地址">
          <input
            aria-label="AI 接口地址"
            type="url"
            value={endpoint}
            onChange={(event) => onEndpointChange(event.target.value)}
          />
        </SettingField>
      </div>
    </>
  );
}
