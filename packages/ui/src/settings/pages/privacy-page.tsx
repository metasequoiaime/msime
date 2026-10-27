import { GroupList, LinkRow, Row } from "../../core/platform-controls";
import { createSettingsDraftActions } from "../settings-draft-actions";
import { privacyUrl } from "../app-resources";
import { SettingsPageFieldset } from "../settings-page-fieldset";
import { useSettingsForm } from "../settings-form-context";
import { TelemetryRow } from "../telemetry-section";
import { PrivacyConsentRow } from "../privacy-consent-row";

/**
 * 「我的 › 隐私」打开的页面：用几句话说清楚哪些数据留在手机上、什么时候才会发出去，下面是匿名使用统计的开关，最后才是去网页看完整隐私政策的链接。原来这一行直接打开隐私政策网页，点进去是一整篇法律文本，还离开了应用。
 *
 * 这几句与仓库根目录的 PRIVACY.md（逐项的网络请求说明）一致：云联想默认关闭；语音、AI 对话只在使用时发出那一次的内容；皮肤、词库、统计不会自动上传；匿名使用统计默认开启、不含输入内容。
 */
export function PrivacySettingsPage() {
  const { busy, page, draft, setDraft, selectPage, openExternalUrl, showAiGeneration, client } =
    useSettingsForm();
  const { onPreferencesChange } = createSettingsDraftActions({ setDraft });
  return (
    <SettingsPageFieldset disabled={busy} hidden={page !== "privacy"} ariaLabel="隐私">
      <GroupList title="你的数据">
        <Row
          title="打的字留在手机上"
          description="输入在本机处理。「云联想」默认关闭，打开后才会把正在打的拼音发给 Google 输入工具。"
        />
        {/* 不提供 AI 生成类功能的宿主（HarmonyOS）没有 AI 对话，这一句只说语音。 */}
        <Row
          title={showAiGeneration ? "语音和 AI 对话只在用的时候发出" : "语音只在用的时候发出"}
          description={
            showAiGeneration
              ? "只有你使用时，才把这一次的录音或文字发给你选的服务。"
              : "只有你使用时，才把这一次的录音发给你选的服务。"
          }
        />
        <Row
          title="皮肤、词库和统计不会自动上传"
          description="只有你点「上传本机设置」或主动发布作品时才会上传。"
        />
      </GroupList>
      <GroupList title="匿名使用统计">
        <TelemetryRow
          value={draft?.usage_reporting}
          onChange={(usage_reporting) => onPreferencesChange({ usage_reporting })}
          onOpenDetails={() => selectPage("usage-reporting")}
        />
      </GroupList>
      <GroupList>
        {/* HarmonyOS 首次启动时由原生弹窗征得授权，这里显示授权状态并可重新打开弹窗。 */}
        <PrivacyConsentRow consent={client.privacyConsent} />
        <LinkRow title="完整隐私政策" external onClick={() => void openExternalUrl(privacyUrl)} />
      </GroupList>
    </SettingsPageFieldset>
  );
}
