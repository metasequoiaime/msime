import { SettingsGroupNote } from "./settings-group-note";
import type { ReactNode } from "react";
import { GroupList } from "../core/platform-controls";
import { POLISH_PROVIDER_OPTIONS } from "../voice/voice-provider-options";
import { VoiceProviderRow } from "./voice-provider-row";
import { TextInputRow } from "./text-input-row";
import { SwitchRow } from "./switch-row";

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

/** Shared voice text-polish provider controls and extension slot for credentials and prompts. While polishing is off only the switch shows; turning it on expands the service, credentials, presets and prompts with their values intact. */
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
    <GroupList title="文本润色">
      <SettingsGroupNote>识别结果可交给用户管理的服务润色</SettingsGroupNote>
      <SwitchRow
        title="启用润色"
        aria-label="启用文本润色"
        checked={enabled}
        onChange={onEnabledChange}
      />
      {enabled && (
        <>
          <VoiceProviderRow
            title="服务提供商"
            options={POLISH_PROVIDER_OPTIONS}
            ariaLabel="文本润色服务提供商"
            value={provider}
            onChange={onProviderChange}
          />
          {providerPreset}
          <TextInputRow title="模型" label="文本润色模型" value={model} onChange={onModelChange} />
          {children}
        </>
      )}
    </GroupList>
  );
}
