import { SettingsGroupNote } from "./settings-group-note";
import { SettingsWarning } from "./settings-warning";
import type { ReactNode } from "react";
import { SecretSettingRow } from "./secret-setting-row";
import { TextInputRow } from "./text-input-row";
import { SettingsGroupBlock } from "./settings-group-block";
import { SettingsRowStack } from "./settings-row-stack";
import { MoreOptions } from "../core/platform-controls";
import { tencentCredentialIssue } from "./translation-validation";
import type { TranslationOfflineFallback } from "./translation-service-selector-section";

/** 缺凭据时的警告，按 `TranslationOfflineFallback` 照实说：macOS 仍有离线翻译，Windows 和 HarmonyOS 只是不请求在线翻译（离线词库照样有释义），其余宿主沿用原来的说法。 */
const missingCredentialsWarning: Record<TranslationOfflineFallback | "none", string> = {
  system:
    "未填写腾讯云凭据，填好之前候选仍用离线翻译。请填入 SecretId 与 SecretKey，或在上面的翻译服务中改选其他服务。",
  dictionary:
    "未填写腾讯云凭据，填好之前不会请求在线翻译。请填入 SecretId 与 SecretKey，或在上面的翻译服务中改选其他服务。",
  none: "未填写腾讯云凭据，候选词翻译不会有任何结果。请填入 SecretId 与 SecretKey，或在上面的翻译服务中改选其他服务。",
};

export interface TencentTranslationSectionProps {
  available: boolean;
  secretId: string;
  secretKey: string;
  region: string;
  credentialIssue?: string;
  showMissingCredentialsWarning?: boolean;
  /** 见 `TranslationServiceSelectorSectionProps.offlineFallback`：缺凭据时 host-api 不把腾讯云放进查询，宿主照样出离线释义，警告照实说，而不是说没有结果。 */
  offlineFallback?: TranslationOfflineFallback;
  /** 把「地域」收进「更多选项」；外层已经整体折叠（HarmonyOS 手机的「翻译服务」）时传 false，免得两层「更多选项」套在一起。 */
  foldRegion?: boolean;
  onSecretIdChange: (value: string) => void;
  onSecretKeyChange: (value: string) => void;
  onRegionChange: (value: string) => void;
  children?: ReactNode;
}

/** 腾讯云凭据，在腾讯翻译是所选翻译服务时显示；服务本身在「翻译服务」中选择。 */
export function TencentTranslationSection({
  available,
  secretId,
  secretKey,
  region,
  credentialIssue,
  showMissingCredentialsWarning = false,
  offlineFallback,
  foldRegion = true,
  onSecretIdChange,
  onSecretKeyChange,
  onRegionChange,
  children,
}: TencentTranslationSectionProps) {
  const regionRow = (
    <TextInputRow
      title="地域"
      label="腾讯云地域"
      type="text"
      autoComplete="off"
      spellCheck={false}
      value={region}
      disabled={!available}
      onChange={onRegionChange}
      placeholder="ap-guangzhou"
    />
  );
  return (
    <SettingsRowStack role="group" aria-label="腾讯云机器翻译">
      <SettingsGroupNote>需要填入你自己的腾讯云 API 凭据。</SettingsGroupNote>
      <TextInputRow
        title="SecretId"
        label="腾讯云 SecretId"
        type="text"
        autoComplete="off"
        spellCheck={false}
        value={secretId}
        disabled={!available}
        onChange={onSecretIdChange}
        placeholder="AKIDxxxxxxxxxxxxxxxx"
      />
      <SecretSettingRow
        title="SecretKey"
        label="腾讯云 SecretKey"
        value={secretKey}
        disabled={!available}
        onChange={onSecretKeyChange}
      />
      {/* 请求一律发往就近接入的 tmt.tencentcloudapi.com，地域只进请求头，留空也按广州处理，所以平时用不着改，收进「更多选项」；需要境外数据驻留的人仍能填。值本身不合规时展开，让错误旁边就是要改的那一格。 */}
      {foldRegion ? (
        <MoreOptions defaultOpen={tencentCredentialIssue("", "", region) !== ""}>
          {regionRow}
        </MoreOptions>
      ) : (
        regionRow
      )}
      {children && <SettingsGroupBlock>{children}</SettingsGroupBlock>}
      {available && credentialIssue && (
        <SettingsGroupBlock>
          <SettingsWarning>{credentialIssue}</SettingsWarning>
        </SettingsGroupBlock>
      )}
      {available && !credentialIssue && showMissingCredentialsWarning && (
        <SettingsGroupBlock>
          <SettingsWarning>{missingCredentialsWarning[offlineFallback ?? "none"]}</SettingsWarning>
        </SettingsGroupBlock>
      )}
    </SettingsRowStack>
  );
}
