import { GroupList } from "../../core/platform-controls";
import { SwitchRow } from "../switch-row";
import { createSettingsDraftActions } from "../settings-draft-actions";
import { aiTabs } from "../ai-tabs";
import { useSettingsTabs } from "../use-settings-tabs";
import { SettingsPageFieldset } from "../settings-page-fieldset";
import { McpSettingsContent } from "./mcp-page";
import { AiSettingsContent } from "../ai-settings-content";
import { useSettingsForm } from "../settings-form-context";
import { SubPageEntries } from "./sub-page-entries";
import { AiReplyTemplates } from "../ai-reply-templates";

/** AI 辅助内容保留原有表单与草稿，切换标签不卸载。 */
function AiAssistanceContent() {
  const {
    client,
    host,
    draft,
    setDraft,
    linuxPlatform,
    androidPlatform,
    iosPlatform,
    windowsPlatform,
    macosPlatform,
    showAiCandidateLimit,
    busy,
    page,
    aiModels,
    aiModelsStatus,
    aiModelsBusy,
    aiTestInput,
    setAiTestInput,
    aiTestOutput,
    aiTestStatus,
    aiTestBusy,
    providerCredentials,
    aiCredentialInput,
    setAiCredentialInput,
    providerCredentialBusy,
    ai,
    aiOrigin,
    aiToken,
    storedAiCredential,
    updateAi,
    updateAiToken,
    fetchAiModels,
    testAi,
    runProviderCredential,
    providerCredentialMessages,
    credentialTestControl,
    providerPresetControls,
    setError,
  } = useSettingsForm();

  const { onPreferencesChange } = createSettingsDraftActions({ setDraft });

  return (
    <AiSettingsContent
      disabled={busy}
      hidden={page !== "ai"}
      client={client}
      ai={ai}
      updateAi={updateAi}
      aiOrigin={aiOrigin}
      aiToken={aiToken}
      updateAiToken={updateAiToken}
      aiModels={aiModels}
      aiModelsStatus={aiModelsStatus}
      aiModelsBusy={aiModelsBusy}
      fetchAiModels={fetchAiModels}
      aiTestInput={aiTestInput}
      setAiTestInput={setAiTestInput}
      aiTestOutput={aiTestOutput}
      aiTestStatus={aiTestStatus}
      aiTestBusy={aiTestBusy}
      testAi={testAi}
      setError={setError}
      providerPresetControls={providerPresetControls}
      linuxPlatform={linuxPlatform}
      windowsPlatform={windowsPlatform}
      macosPlatform={macosPlatform}
      iosPlatform={iosPlatform}
      androidPlatform={androidPlatform}
      candidateLimitShown={showAiCandidateLimit}
      providerCredentials={providerCredentials}
      storedAiCredential={storedAiCredential}
      aiCredentialInput={aiCredentialInput}
      setAiCredentialInput={setAiCredentialInput}
      providerCredentialBusy={providerCredentialBusy}
      providerCredentialMessages={providerCredentialMessages}
      runProviderCredential={runProviderCredential}
      credentialTestControl={credentialTestControl}
      // 只有桌面宿主显示代理设置：设置页、账号与社区、更新检查、词库与模型下载都经 Rust 的 reqwest 发出，
      // iOS、Android 和鸿蒙的这几条请求走平台自己的网络栈，没有出口读这份偏好，所以整组不显示。
      showNetwork={macosPlatform || windowsPlatform || linuxPlatform}
      network={draft.network}
      onNetworkChange={(network) =>
        createSettingsDraftActions({ setDraft }).onPreferencesChange({ network })
      }
      trailing={
        <>
          {host?.english_online_candidates && (
            <GroupList title="英文补全与纠错">
              <SwitchRow
                title="英文模式也请求 AI / 云候选"
                description="开启后，英文模式和临时英文会在停顿时把当前字母串与前文发送给已启用的 AI 或自定义云候选服务。默认关闭；内置 Google 拼音服务不支持英文。"
                checked={draft.english_online_candidates ?? false}
                disabled={busy}
                onChange={(english_online_candidates) =>
                  onPreferencesChange({ english_online_candidates })
                }
              />
            </GroupList>
          )}
          <AiReplyTemplates />
          <SubPageEntries
            title="AI 对话"
            pages={[{ id: "chat", description: "与 AI 对话，结果可以直接用于输入" }]}
          />
        </>
      }
    />
  );
}

/** 桌面 AI 入口分开辅助配置与助手连接，触屏保留原有独立页面。 */
export function AiSettingsPage() {
  const { client, mobilePlatform, busy, page, aiTab, setAiTab } = useSettingsForm();
  const combined = !mobilePlatform && Boolean(client.mcpServerStatus);
  const { tabList, panel } = useSettingsTabs(aiTabs, "AI 设置分类", combined, aiTab, setAiTab);
  if (mobilePlatform) return <AiAssistanceContent />;
  return (
    <SettingsPageFieldset
      disabled={busy}
      hidden={page !== "ai"}
      ariaLabel="AI"
      className="[&_.ai-settings>div]:mt-0"
    >
      {tabList}
      {panel("assistance", <AiAssistanceContent />)}
      {combined && panel("connection", <McpSettingsContent />)}
    </SettingsPageFieldset>
  );
}
