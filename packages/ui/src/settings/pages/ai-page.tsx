import { AiSettingsContent } from "../ai-settings-content";
import { useSettingsForm } from "../settings-form-context";

/** The AI 辅助 page of the settings form. */
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
      grouped
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
      mcpConnect={null}
    />
  );
}
