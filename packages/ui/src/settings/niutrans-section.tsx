import type { ReactNode } from "react";
import { Row, Switch } from "../core/platform-controls";
import * as settings from "./settings-style";
import { SecretSettingRow } from "./secret-setting-row";
import { TextInputRow } from "./text-input-row";

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
    <div role="group" aria-label="小牛翻译（NiuTrans）" className={settings.rowStack}>
      <Row title="小牛翻译（NiuTrans）" description="使用 App ID 和 API Key 为候选词提供逐条翻译">
        <Switch
          aria-label="小牛翻译（NiuTrans）"
          disabled={!available}
          checked={enabled}
          onChange={onToggle}
        />
      </Row>
      <TextInputRow
        title="App ID"
        label="NiuTrans App ID"
        value={appId}
        disabled={!available || !enabled}
        onChange={onAppIdChange}
      />
      <SecretSettingRow
        title="API Key"
        label="NiuTrans API Key"
        value={apiKey}
        disabled={!available || !enabled}
        onChange={onApiKeyChange}
      />
      {enabled && children && <div className={settings.groupBlock}>{children}</div>}
    </div>
  );
}
