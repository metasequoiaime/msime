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
  } = useSettingsForm();

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
      trailing={
        <>
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
