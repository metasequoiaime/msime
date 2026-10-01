import type { ReactNode } from "react";
import { Row, Switch } from "../core/platform-controls";
import * as settings from "./settings-style";
import { SecretSettingRow } from "./secret-setting-row";
import { EndpointInput } from "./endpoint-input";

export interface CustomTranslationSectionProps {
  enabled: boolean;
  available: boolean;
  endpoint: string;
  apiKey: string;
  endpointIssue?: string;
  onToggle: (enabled: boolean) => void;
  onEndpointChange: (value: string) => void;
  onApiKeyChange: (value: string) => void;
  children?: ReactNode;
}

/** Settings for a user-managed DeepLX-compatible translation endpoint. */
export function CustomTranslationSection({
  enabled,
  available,
  endpoint,
  apiKey,
  endpointIssue,
  onToggle,
  onEndpointChange,
  onApiKeyChange,
  children,
}: CustomTranslationSectionProps) {
  return (
    <div role="group" aria-label="自定义翻译服务" className={settings.rowStack}>
      <Row
        title="自定义翻译服务"
        description="改用自建的兼容 DeepLX 的 HTTPS 服务；关闭后候选词翻译使用上面选择的在线服务"
      >
        <Switch
          aria-label="自定义翻译服务"
          disabled={!available}
          checked={enabled}
          onChange={onToggle}
        />
      </Row>
      <Row title="翻译 Endpoint">
        <EndpointInput
          label="自定义翻译 Endpoint"
          value={endpoint}
          disabled={!available || !enabled}
          onChange={onEndpointChange}
          placeholder="https://example.com/translate"
        />
      </Row>
      {available && enabled && endpointIssue && (
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
        disabled={!available || !enabled}
        onChange={onApiKeyChange}
      />
      {enabled && children && <div className={settings.groupBlock}>{children}</div>}
    </div>
  );
}
