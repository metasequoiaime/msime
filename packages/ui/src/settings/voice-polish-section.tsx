import type { ReactNode } from "react";
import { GroupList, Row, Switch } from "../core/platform-controls";
import { POLISH_PROVIDER_OPTIONS } from "../voice/voice-provider-options";
import * as settings from "./settings-style";
import { VoiceProviderSelect } from "./voice-provider-select";

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
    <GroupList title="文本润色 provider">
      <p className={settings.groupNote}>识别结果可交给用户管理的服务润色</p>
      <Row title="启用润色">
        <Switch aria-label="启用文本润色" checked={enabled} onChange={onEnabledChange} />
      </Row>
      <Row title="服务提供商">
        <VoiceProviderSelect
          options={POLISH_PROVIDER_OPTIONS}
          ariaLabel="文本润色服务提供商"
          value={provider}
          onChange={onProviderChange}
        />
      </Row>
      {providerPreset}
      <Row title="模型">
        <input
          aria-label="文本润色模型"
          value={model}
          onChange={(event) => onModelChange(event.target.value)}
        />
      </Row>
      {children}
    </GroupList>
  );
}
