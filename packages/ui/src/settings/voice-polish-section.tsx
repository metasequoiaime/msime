import type { ReactNode } from "react";
import { GroupList, Row } from "../core/platform-controls";
import { POLISH_PROVIDER_OPTIONS } from "../voice/voice-provider-options";
import * as settings from "./settings-style";
import { VoiceProviderSelect } from "./voice-provider-select";
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
      <p className={settings.groupNote}>识别结果可交给用户管理的服务润色</p>
      <SwitchRow
        title="启用润色"
        aria-label="启用文本润色"
        checked={enabled}
        onChange={onEnabledChange}
      />
      {enabled && (
        <>
          <Row title="服务提供商">
            <VoiceProviderSelect
              options={POLISH_PROVIDER_OPTIONS}
              ariaLabel="文本润色服务提供商"
              value={provider}
              onChange={onProviderChange}
            />
          </Row>
          {providerPreset}
          <TextInputRow title="模型" label="文本润色模型" value={model} onChange={onModelChange} />
          {children}
        </>
      )}
    </GroupList>
  );
}
