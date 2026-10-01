import type { ReactNode } from "react";
import { type CredentialStatusMessageValue } from "./credential-status-message";
import { CredentialActions } from "./credential-actions";
import { SecretSettingField } from "./secret-setting-field";

export interface AiCredentialStored {
  endpoint: string;
  model: string;
}

export interface AiCredentialSectionProps {
  endpoint: string;
  model: string;
  origin: string | null;
  token: string;
  stored?: AiCredentialStored;
  invalid: boolean;
  busy: boolean;
  message?: CredentialStatusMessageValue;
  onTokenChange: (token: string) => void;
  onSave: () => void;
  onClear: () => void;
  children?: ReactNode;
}

/** Linux provider credentials for the shared AI assistant settings. */
export function AiCredentialSection({
  endpoint,
  model,
  origin,
  token,
  stored,
  invalid,
  busy,
  message,
  onTokenChange,
  onSave,
  onClear,
  children,
}: AiCredentialSectionProps) {
  const matchesStored = stored?.endpoint === endpoint && stored.model === model;
  return (
    <div className="section" role="group" aria-label="AI 凭据">
      <SecretSettingField
        label="API Token"
        inputLabel="AI API Token"
        value={token}
        description={
          invalid
            ? "现有 ai-provider.json 无效，provider 服务不会发出任何 AI 请求；请修复或删除该文件"
            : !stored
              ? "尚未保存；保存后只写入用户配置目录的 ai-provider.json，由 provider 服务读取"
              : matchesStored
                ? "已保存，留空则保留原凭据"
                : `已保存的凭据绑定 ${stored.endpoint}（${stored.model}），与上方设置不一致；保存后改为绑定当前接口和模型`
        }
        disabled={!origin}
        onChange={onTokenChange}
      />
      <CredentialActions
        saveDisabled={busy || !origin || !model.trim() || (!token.trim() && !stored)}
        clearDisabled={busy}
        hasStoredCredential={Boolean(stored)}
        message={message}
        onSave={onSave}
        onClear={onClear}
      />
      {children}
    </div>
  );
}
