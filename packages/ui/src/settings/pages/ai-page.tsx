import { AiSettingsContent } from "../ai-settings-content";
import { useSettingsForm } from "../settings-form-context";
import { SubPageEntries } from "./sub-page-entries";
import { AiReplyTemplates } from "../ai-reply-templates";

/** AI 辅助页提供 AI 对话二级入口，并列出社区回复模板。 */
export function AiSettingsPage() {
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
