import type { ReactNode } from "react";

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
      <div className="section">
        <label className="section-header">
          <span className="section-title">
            启用 AI 辅助
            <small>{enabledDescription}</small>
          </span>
          <input
            aria-label="启用 AI 辅助"
            className="toggle"
            type="checkbox"
            checked={enabled}
            onChange={(event) => onEnabledChange(event.target.checked)}
          />
        </label>
      </div>
      <div className="section">
        <label className="section-header">
          <span className="section-title">服务提供商</span>
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
        </label>
      </div>
      {providerPreset}
      <div className="section">
        <label className="section-header">
          <span className="section-title">模型</span>
          <input
            aria-label="AI 模型"
            value={model}
            onChange={(event) => onModelChange(event.target.value)}
          />
        </label>
      </div>
      <div className="section">
        <label className="section-header">
          <span className="section-title">接口地址</span>
          <input
            aria-label="AI 接口地址"
            type="url"
            value={endpoint}
            onChange={(event) => onEndpointChange(event.target.value)}
          />
        </label>
      </div>
    </>
  );
}
