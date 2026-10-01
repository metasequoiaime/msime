import type { ReactNode } from "react";
import { GroupList, Row, Select, Switch } from "../core/platform-controls";
import { POLISH_PROVIDER_OPTIONS, VoiceProviderOptions } from "../voice/voice-provider-options";
import * as settings from "./settings-style";

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
        <Select
          aria-label="文本润色服务提供商"
          value={provider}
          onChange={(event) => onProviderChange(event.target.value)}
        >
          <VoiceProviderOptions options={POLISH_PROVIDER_OPTIONS} />
        </Select>
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
