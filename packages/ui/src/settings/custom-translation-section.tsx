import type { ReactNode } from "react";
import { Row } from "../core/platform-controls";
import * as settings from "./settings-style";
import { SecretSettingRow } from "./secret-setting-row";
import { EndpointSettingRow } from "./endpoint-setting-row";

export interface CustomTranslationSectionProps {
  available: boolean;
  endpoint: string;
  apiKey: string;
  endpointIssue?: string;
  onEndpointChange: (value: string) => void;
  onApiKeyChange: (value: string) => void;
  children?: ReactNode;
}

/** 用户自行管理的 DeepLX 兼容翻译接口的设置，在它是所选翻译服务时显示；服务本身在「翻译服务」中选择。 */
export function CustomTranslationSection({
  available,
  endpoint,
  apiKey,
  endpointIssue,
  onEndpointChange,
  onApiKeyChange,
  children,
}: CustomTranslationSectionProps) {
  return (
    <div role="group" aria-label="自定义翻译服务" className={settings.rowStack}>
      <p className={settings.groupNote}>使用自建的兼容 DeepLX 的 HTTPS 服务。</p>
      <EndpointSettingRow
        title="翻译 Endpoint"
        inputLabel="自定义翻译 Endpoint"
        value={endpoint}
        disabled={!available}
        onChange={onEndpointChange}
        placeholder="https://example.com/translate"
      />
      {available && endpointIssue && (
        <div className={settings.groupBlock}>
          <p className={settings.settingsWarning} role="status">
            {endpointIssue}
          </p>
        </div>
      )}
      <SecretSettingRow
        title="API Key"
        label="自定义翻译 API Key"
        value={apiKey}
        disabled={!available}
        onChange={onApiKeyChange}
      />
      {children && <div className={settings.groupBlock}>{children}</div>}
    </div>
  );
}
