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
  /** 润色关闭时只留开关，服务、凭据、方案与提示词都收起；打开后原样展开，已填的值不受影响。语音页用它，旧面板不传，始终展开。 */
  collapsible?: boolean;
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
  collapsible = false,
  onEnabledChange,
  onProviderChange,
  onModelChange,
}: VoicePolishSectionProps) {
  const expanded = !collapsible || enabled;
  return (
    <GroupList title="文本润色">
      <p className={settings.groupNote}>识别结果可交给用户管理的服务润色</p>
      <SwitchRow
        title="启用润色"
        aria-label="启用文本润色"
        checked={enabled}
        onChange={onEnabledChange}
      />
      {expanded && (
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
