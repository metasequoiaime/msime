import type { ReactNode } from "react";
import * as settings from "./settings-style";
import { SecretSettingRow } from "./secret-setting-row";
import { TextInputRow } from "./text-input-row";

export interface NiuTransSectionProps {
  available: boolean;
  appId: string;
  apiKey: string;
  onAppIdChange: (value: string) => void;
  onApiKeyChange: (value: string) => void;
  children?: ReactNode;
}

/** NiuTrans 凭据，在 NiuTrans 是所选翻译服务时显示；服务本身在「翻译服务」中选择。 */
export function NiuTransSection({
  available,
  appId,
  apiKey,
  onAppIdChange,
  onApiKeyChange,
  children,
}: NiuTransSectionProps) {
  return (
    <div role="group" aria-label="小牛翻译（NiuTrans）" className={settings.rowStack}>
      <p className={settings.groupNote}>使用 App ID 和 API Key 为候选词提供逐条翻译。</p>
      <TextInputRow
        title="App ID"
        label="NiuTrans App ID"
        value={appId}
        disabled={!available}
        onChange={onAppIdChange}
      />
      <SecretSettingRow
        title="API Key"
        label="NiuTrans API Key"
        value={apiKey}
        disabled={!available}
        onChange={onApiKeyChange}
      />
      {children && <div className={settings.groupBlock}>{children}</div>}
    </div>
  );
}
