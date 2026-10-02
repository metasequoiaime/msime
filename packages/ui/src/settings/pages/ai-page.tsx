import { AiSettingsContent } from "../ai-settings-content";
import { useSettingsForm } from "../settings-form-context";
import { SubPageEntries } from "./sub-page-entries";

/** The AI 辅助 page of the settings form, a page of the 工具 group; AI 对话 opens from its last group. */
export function AiSettingsPage() {
  const {
    client,
    linuxPlatform,
    androidPlatform,
    iosPlatform,
    windowsPlatform,
    macosPlatform,
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
      providerCredentials={providerCredentials}
      storedAiCredential={storedAiCredential}
      aiCredentialInput={aiCredentialInput}
      setAiCredentialInput={setAiCredentialInput}
      providerCredentialBusy={providerCredentialBusy}
      providerCredentialMessages={providerCredentialMessages}
      runProviderCredential={runProviderCredential}
      credentialTestControl={credentialTestControl}
      trailing={
        <SubPageEntries
          title="AI 对话"
          pages={[{ id: "chat", description: "与 AI 对话，结果可以直接用于输入" }]}
        />
      }
    />
  );
}
