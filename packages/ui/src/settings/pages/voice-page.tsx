import { VoiceSettingsContent } from "../voice-settings-content";
import { useSettingsForm } from "../settings-form-context";

/** The 语音输入 page of the settings form. */
export function VoiceSettingsPage() {
  const {
    client,
    confirm,
    linuxPlatform,
    androidPlatform,
    iosPlatform,
    harmonyPlatform,
    mobilePlatform,
    windowsPlatform,
    macosPlatform,
    nativeVoicePlatform,
    showVoiceCommitMode,
    showVoiceProviderSettings,
    showVoiceStreamPreedit,
    showVoiceCaptureDevices,
    captureBackendOptions,
    showVoiceHotkeys,
    draft,
    setDraft,
    busy,
    page,
    openExternalUrl,
    openPanel,
    voiceInput,
    systemVoice,
    systemVoiceHostName,
    localVoiceAvailable,
    localVoice,
    serviceVoice,
    harmonyUnsupportedAsr,
    doubaoAuthMode,
    updateVoice,
    providerCredentials,
    voiceCredentialInput,
    setVoiceCredentialInput,
    providerCredentialBusy,
    providerCredentialMessages,
    runVoiceCredential,
    credentialTestControl,
    providerPresetControls,
  } = useSettingsForm();

  return (
    <VoiceSettingsContent
      disabled={busy}
      hidden={page !== "voice"}
      client={client}
      draft={draft}
      setDraft={setDraft}
      confirm={confirm}
      openExternalUrl={openExternalUrl}
      openPanel={openPanel}
      voiceInput={voiceInput}
      systemVoice={systemVoice}
      systemVoiceHostName={systemVoiceHostName}
      localVoiceAvailable={localVoiceAvailable}
      localVoice={localVoice}
      serviceVoice={serviceVoice}
      harmonyUnsupportedAsr={harmonyUnsupportedAsr}
      doubaoAuthMode={doubaoAuthMode}
      updateVoice={updateVoice}
      providerCredentials={providerCredentials}
      voiceCredentialInput={voiceCredentialInput}
      setVoiceCredentialInput={setVoiceCredentialInput}
      providerCredentialBusy={providerCredentialBusy}
      providerCredentialMessages={providerCredentialMessages}
      runVoiceCredential={runVoiceCredential}
      credentialTestControl={credentialTestControl}
      providerPresetControls={providerPresetControls}
      androidPlatform={androidPlatform}
      iosPlatform={iosPlatform}
      macosPlatform={macosPlatform}
      harmonyPlatform={harmonyPlatform}
      linuxPlatform={linuxPlatform}
      windowsPlatform={windowsPlatform}
      mobilePlatform={mobilePlatform}
      nativeVoicePlatform={nativeVoicePlatform}
      showVoiceHotkeys={showVoiceHotkeys}
      showCredentialTests={Boolean(client.testApiCredential)}
      showVoiceProviderSettings={showVoiceProviderSettings}
      showVoiceStreamPreedit={showVoiceStreamPreedit}
      showVoiceCommitMode={showVoiceCommitMode}
      showVoiceCaptureDevices={showVoiceCaptureDevices}
      captureBackendOptions={captureBackendOptions}
    />
  );
}
