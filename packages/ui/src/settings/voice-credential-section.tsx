import { CredentialActions } from "./credential-actions";
import { DoubaoStreamEndpointSelect } from "./doubao-stream-endpoint-section";
import { EndpointSettingField } from "./endpoint-setting-field";
import { PasswordSettingField } from "./password-setting-field";
import { SettingField } from "./setting-field";
import { SettingSectionTitle } from "./setting-section-title";
import { SettingsInputDescription } from "./settings-input-description";
import { SettingsManagerBlock } from "./settings-manager-block";

import type { VoiceCredentialMessage } from "./voice-credential-model";
import {
  useVoiceCredentialEditor,
  type UseVoiceCredentialEditorOptions,
} from "./use-voice-credential-editor";

export type {
  VoiceCredentialSectionKind,
  VoiceCredentialEntry,
  VoiceCredentialStatus,
  VoiceCredentialInput,
  VoiceCredentialSaveInput,
  VoiceCredentialMessage,
} from "./voice-credential-model";

export interface VoiceCredentialSectionProps extends UseVoiceCredentialEditorOptions {
  message?: VoiceCredentialMessage;
}

/** Linux 语音识别或润色服务的用户凭据编辑界面。 */
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
  const {
    doubao,
    legacy,
    stored,
    endpoint,
    streamEndpoint,
    customEndpoint,
    name,
    tokenLabel,
    status,
    description,
    saveDisabled,
    clearDisabled,
    setCustomEndpoint,
    selectEndpoint,
    editEndpoint,
    editToken,
    editAppKey,
    save,
    clear,
  } = useVoiceCredentialEditor({
    kind,
    provider,
    model,
    resourceId,
    authMode,
    credentials,
    input,
    busy,
    onChange,
    onSave,
    onClear,
  });

  return (
    <SettingsManagerBlock role="group" aria-label={`语音${name}凭据`}>
      <SettingSectionTitle as="div" title={`${name}凭据`} description={description} />
      <SettingsInputDescription>{status}</SettingsInputDescription>
      {doubao && (
        <SettingField
          label="流式接口"
          description="整句流式边录边传、说完返回整句，服务方称准确率更高并推荐用于输入法；双向流式返回增量结果，边说边显示刷新更频繁。保存凭据后生效；地址留空时语音服务使用双向流式。"
        >
          <DoubaoStreamEndpointSelect
            endpoint={streamEndpoint}
            custom={customEndpoint}
            onCustomChange={setCustomEndpoint}
            onChange={selectEndpoint}
          />
        </SettingField>
      )}
      {(!doubao || customEndpoint) && (
        <EndpointSettingField
          label={doubao ? "自定义接口地址" : "接口地址"}
          inputLabel={`${name}接口地址`}
          description={doubao ? "留空时语音服务使用双向流式" : "留空使用当前服务的默认地址"}
          value={endpoint}
          onChange={editEndpoint}
        />
      )}
      {legacy && (
        <PasswordSettingField
          label="Doubao App Key"
          inputLabel="Doubao App Key"
          description="旧版控制台鉴权使用"
          value={input.appKey}
          onChange={editAppKey}
        />
      )}
      <PasswordSettingField
        label={tokenLabel}
        inputLabel={tokenLabel}
        value={input.token}
        onChange={editToken}
      />
      <CredentialActions
        saveDisabled={saveDisabled}
        clearDisabled={clearDisabled}
        hasStoredCredential={Boolean(stored)}
        saveAriaLabel={`保存${name}凭据`}
        clearAriaLabel={`清除${name}凭据`}
        message={message}
        onSave={save}
        onClear={clear}
      />
    </SettingsManagerBlock>
  );
}
