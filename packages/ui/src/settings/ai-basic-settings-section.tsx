import type { ReactNode } from "react";
import { EndpointSettingField } from "./endpoint-setting-field";
import { SettingToggle } from "./setting-toggle";
import { SelectSettingField } from "./select-setting-field";
import { TextSettingField } from "./text-setting-field";

export type AiProviderOption = { id: string; title: string };

/** Shared provider option markup for the legacy and grouped AI selects. */
export function AiProviderOptions({ options }: { options: readonly AiProviderOption[] }) {
  return (
    <>
      {options.map((option) => (
        <option key={option.id} value={option.id}>
          {option.title}
        </option>
      ))}
    </>
  );
}

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
        <SelectSettingField
          label="服务提供商"
          inputLabel="AI 服务提供商"
          value={provider}
          onChange={onProviderChange}
        >
          <AiProviderOptions options={providerOptions} />
        </SelectSettingField>
      </div>
      {providerPreset}
      <div className="section">
        <TextSettingField
          label="模型"
          inputLabel="AI 模型"
          value={model}
          onChange={onModelChange}
        />
      </div>
      <div className="section">
        <EndpointSettingField
          label="接口地址"
          inputLabel="AI 接口地址"
          value={endpoint}
          onChange={onEndpointChange}
        />
      </div>
    </>
  );
}
