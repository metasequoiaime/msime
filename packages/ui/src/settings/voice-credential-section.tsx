import { findDoubaoStreamEndpoint } from "../voice/voice-providers";
import { CredentialActions } from "./credential-actions";
import { type CredentialStatusMessageValue } from "./credential-status-message";
import { DoubaoStreamEndpointSelect } from "./doubao-stream-endpoint-section";
import { EndpointSettingField } from "./endpoint-setting-field";
import { PasswordSettingField } from "./password-setting-field";
import { SettingField } from "./setting-field";
import { SettingSectionTitle } from "./setting-section-title";
import { SettingsManagerBlock } from "./settings-manager-block";

export type VoiceCredentialSectionKind = "asr" | "polish";

export interface VoiceCredentialEntry {
  provider: string;
  model: string;
  endpoint: string;
  resourceId: string | null;
  authMode: string | null;
}

export interface VoiceCredentialStatus {
  voiceAsr: readonly VoiceCredentialEntry[];
  voicePolish: readonly VoiceCredentialEntry[];
  voiceInvalid: boolean;
}

export interface VoiceCredentialInput {
  token: string;
  appKey: string;
  endpoint?: string;
}

export interface VoiceCredentialSaveInput {
  kind: VoiceCredentialSectionKind;
  provider: string;
  endpoint: string;
  model: string;
  token?: string;
  appKey?: string;
  resourceId: string;
  authMode: string;
}

export type VoiceCredentialMessage = CredentialStatusMessageValue;

export interface VoiceCredentialSectionProps {
  kind: VoiceCredentialSectionKind;
  provider: string;
  model: string;
  resourceId?: string;
  authMode?: string;
  credentials?: VoiceCredentialStatus;
  input: VoiceCredentialInput;
  busy: boolean;
  message?: VoiceCredentialMessage;
  onChange: (patch: Partial<VoiceCredentialInput>) => void;
  onSave: (credential: VoiceCredentialSaveInput) => void;
  onClear: () => void;
}

/** Owner-only credentials for the Linux voice provider's recognition or polishing service. */
export function VoiceCredentialSection({
  kind,
  provider,
  model,
  resourceId,
  authMode,
  credentials,
  input,
  busy,
  message,
  onChange,
  onSave,
  onClear,
}: VoiceCredentialSectionProps) {
  const doubao = kind === "asr" && provider === "doubao";
  const legacy = doubao && authMode === "legacy";
  const stored = (kind === "asr" ? credentials?.voiceAsr : credentials?.voicePolish)?.find(
    (entry) => entry.provider === provider,
  );
  const endpoint = input.endpoint ?? stored?.endpoint ?? "";
  const name = kind === "asr" ? "识别" : "润色";
  const tokenLabel = doubao && !legacy ? "Doubao API Key" : `${name} API Token`;
  const mismatch =
    stored &&
    ((model.trim() && stored.model !== model.trim()) ||
      (doubao &&
        ((resourceId?.trim() && stored.resourceId !== resourceId.trim()) ||
          stored.authMode !== authMode)));
  const streamEndpoint = endpoint.trim() || findDoubaoStreamEndpoint("async")?.endpoint || "";

  return (
    <SettingsManagerBlock role="group" aria-label={`语音${name}凭据`}>
      <SettingSectionTitle
        as="div"
        title={`${name}凭据`}
        description={
          credentials?.voiceInvalid
            ? "现有 voice-provider.json 无效，语音服务不会启动；请修复或删除该文件"
            : !stored
              ? "尚未保存；保存后只写入用户配置目录的 voice-provider.json，由语音服务读取"
              : mismatch
                ? "已保存的凭据与上方模型或豆包设置不一致；保存后改为绑定当前设置"
                : "已保存，留空则保留原凭据"
        }
      />
      {doubao && (
        <SettingField
          label="流式接口"
          description="整句流式边录边传、说完返回整句，服务方称准确率更高并推荐用于输入法；双向流式返回增量结果，流式预编辑刷新更频繁。选择后写入下方接口地址，保存凭据后生效；地址留空时语音服务使用双向流式。"
        >
          <DoubaoStreamEndpointSelect
            endpoint={streamEndpoint}
            onChange={(value) => onChange({ endpoint: value })}
          />
        </SettingField>
      )}
      <EndpointSettingField
        label="接口地址"
        inputLabel={`${name}接口地址`}
        description="留空使用当前服务的默认地址"
        value={endpoint}
        onChange={(endpoint) => onChange({ endpoint })}
      />
      {legacy && (
        <PasswordSettingField
          label="Doubao App Key"
          inputLabel="Doubao App Key"
          description="旧版控制台鉴权使用"
          value={input.appKey}
          onChange={(appKey) => onChange({ appKey })}
        />
      )}
      <PasswordSettingField
        label={tokenLabel}
        inputLabel={tokenLabel}
        value={input.token}
        onChange={(token) => onChange({ token })}
      />
      <CredentialActions
        saveDisabled={
          busy ||
          (!input.token.trim() && !stored) ||
          (legacy && !input.appKey.trim() && stored?.authMode !== "legacy")
        }
        clearDisabled={busy}
        hasStoredCredential={Boolean(stored)}
        saveAriaLabel={`保存${name}凭据`}
        clearAriaLabel={`清除${name}凭据`}
        message={message}
        onSave={() =>
          onSave({
            kind,
            provider,
            endpoint,
            model,
            ...(input.token.trim() ? { token: input.token } : {}),
            ...(legacy && input.appKey.trim() ? { appKey: input.appKey } : {}),
            resourceId: doubao ? (resourceId ?? "") : "",
            authMode: doubao ? (authMode ?? "") : "",
          })
        }
        onClear={onClear}
      />
    </SettingsManagerBlock>
  );
}
