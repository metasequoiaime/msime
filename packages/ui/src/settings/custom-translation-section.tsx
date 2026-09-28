import type { ReactNode } from "react";
import { SecretInput } from "../core/secret-input";
import * as settings from "./settings-style";

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
    <div className="section" role="group" aria-label="自定义翻译服务">
      <label className="section-header">
        <span className="section-title">
          自定义翻译服务
          <small>改用自建的兼容 DeepLX 的 HTTPS 服务；关闭后候选词翻译使用上面选择的在线服务</small>
        </span>
        <input
          aria-label="自定义翻译服务"
          className="toggle"
          type="checkbox"
          disabled={!available}
          checked={enabled}
          onChange={(event) => onToggle(event.target.checked)}
        />
      </label>
      <div className="input-option-divider" />
      <label className="section-header">
        <span className="section-title">翻译 Endpoint</span>
        <input
          aria-label="自定义翻译 Endpoint"
          type="url"
          value={endpoint}
          disabled={!available || !enabled}
          onChange={(event) => onEndpointChange(event.target.value)}
          placeholder="https://example.com/translate"
        />
      </label>
      {available && enabled && endpointIssue && (
        <p className={settings.settingsWarning} role="status">
          {endpointIssue}
        </p>
      )}
      <div className="input-option-divider" />
      <label className="section-header">
        <span className="section-title">API Key</span>
        <SecretInput
          label="自定义翻译 API Key"
          value={apiKey}
          disabled={!available || !enabled}
          onChange={onApiKeyChange}
        />
      </label>
      {enabled && children}
    </div>
  );
}
