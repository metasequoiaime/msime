import type { ReactNode } from "react";
import { SecretInput } from "../core/secret-input";
import { SettingToggle } from "./setting-toggle";

export interface NiuTransSectionProps {
  enabled: boolean;
  available: boolean;
  appId: string;
  apiKey: string;
  onToggle: (enabled: boolean) => void;
  onAppIdChange: (value: string) => void;
  onApiKeyChange: (value: string) => void;
  children?: ReactNode;
}

/** NiuTrans candidate translation settings and credentials. */
export function NiuTransSection({
  enabled,
  available,
  appId,
  apiKey,
  onToggle,
  onAppIdChange,
  onApiKeyChange,
  children,
}: NiuTransSectionProps) {
  return (
    <div className="section" role="group" aria-label="小牛翻译（NiuTrans）">
      <SettingToggle
        label="小牛翻译（NiuTrans）"
        description="使用 App ID 和 API Key 为候选词提供逐条翻译"
        ariaLabel="小牛翻译（NiuTrans）"
        disabled={!available}
        checked={enabled}
        compact
        onChange={onToggle}
      />
      <div className="input-option-divider" />
      <label className="section-header">
        <span className="section-title">App ID</span>
        <input
          aria-label="NiuTrans App ID"
          value={appId}
          disabled={!available || !enabled}
          onChange={(event) => onAppIdChange(event.target.value)}
        />
      </label>
      <div className="input-option-divider" />
      <label className="section-header">
        <span className="section-title">API Key</span>
        <SecretInput
          label="NiuTrans API Key"
          value={apiKey}
          disabled={!available || !enabled}
          onChange={onApiKeyChange}
        />
      </label>
      {enabled && children}
    </div>
  );
}
