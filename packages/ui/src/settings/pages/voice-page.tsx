import * as settings from "../settings-style";
import { isAsrServiceProvider } from "../../voice/voice-providers";
import { isVoicePolishEnabled } from "../voice-input-defaults";
import { useSettingsForm } from "../settings-form-context";
import { GroupList, Row, Select, Switch } from "../../core/platform-controls";
import { VoiceInputBasicsSection } from "../voice-input-basics-section";
import { VoiceLocalModelSettingsSection } from "../voice-local-model-settings-section";
import { VoiceAsrProviderSettingsSection } from "../voice-asr-provider-settings-section";
import { VoiceRecognitionResultSection } from "../voice-recognition-result-section";
import { VoiceCaptureDevicesSection } from "../voice-capture-devices-section";
import { VoiceRecordingBehaviorSettingsSection } from "../voice-recording-behavior-settings-section";
import { DoubaoOptionsSection } from "../doubao-options-section";
import { VoiceCredentialControl } from "../voice-credential-control";
import { VoiceHotkeysSection } from "../voice-hotkeys-section";
import { VoicePolishSettingsSection } from "../voice-polish-settings-section";
import { VoiceAsrServiceTestSection } from "../voice-asr-service-test-section";
import {
  asrProviderCredentialTestConfig,
  polishProviderCredentialTestConfig,
  polishServiceCredentialTestConfig,
  polishServiceCredentialTestDisabled,
} from "../voice-credential-test-config";

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
  const polishEnabled = isVoicePolishEnabled(voiceInput);
  return (
    <fieldset disabled={busy} hidden={page !== "voice"} aria-label="语音输入">
      <div className={settings.groups}>
        <VoiceInputBasicsSection
          localVoice={localVoice}
          localVoiceModelsAvailable={Boolean(client.localVoiceModels)}
          systemVoice={systemVoice}
          systemVoiceHostName={systemVoiceHostName}
          android={androidPlatform}
          ios={iosPlatform}
          macos={macosPlatform}
          harmony={harmonyPlatform}
          linux={linuxPlatform}
          showVoiceProviderSettings={showVoiceProviderSettings}
          localVoiceAvailable={localVoiceAvailable}
          nativeVoicePlatform={nativeVoicePlatform}
          harmonyUnsupportedAsr={harmonyUnsupportedAsr}
          voiceInput={voiceInput}
          updateVoice={updateVoice}
          onOpenVoice={client.openVoice ? () => void openPanel(client.openVoice) : undefined}
        />
        <VoiceLocalModelSettingsSection
          client={client}
          localVoice={localVoice}
          mobile={mobilePlatform}
          voiceInput={voiceInput}
          setDraft={setDraft}
          confirm={confirm}
          openExternalUrl={openExternalUrl}
          updateVoice={updateVoice}
        />
        {showVoiceProviderSettings && (serviceVoice || voiceInput.asr_provider === "doubao") && (
          <GroupList title="识别服务配置">
            <VoiceAsrProviderSettingsSection
              voiceInput={voiceInput}
              showProviderSettings={showVoiceProviderSettings}
              serviceVoice={serviceVoice}
              linux={linuxPlatform}
              doubaoAuthMode={doubaoAuthMode}
              providerPresetControls={providerPresetControls}
              providerPresetClassName={settings.managerBlock}
              updateVoice={updateVoice}
            />
          </GroupList>
        )}
        {linuxPlatform && isAsrServiceProvider(voiceInput.asr_provider ?? "doubao") && (
          <VoiceCredentialControl
            available={Boolean(client.providerCredentials)}
            kind="asr"
            voiceInput={voiceInput}
            doubaoAuthMode={doubaoAuthMode}
            providerCredentials={providerCredentials}
            voiceCredentialInput={voiceCredentialInput}
            setVoiceCredentialInput={setVoiceCredentialInput}
            providerCredentialBusy={providerCredentialBusy}
            providerCredentialMessages={providerCredentialMessages}
            runVoiceCredential={runVoiceCredential}
          />
        )}
        {linuxPlatform && client.testApiCredential && (
          <GroupList title="检查识别配置">
            <div className={settings.groupBlock}>
              {credentialTestControl(
                "voice.asr",
                "测试语音识别配置",
                asrProviderCredentialTestConfig(voiceInput, doubaoAuthMode),
              )}
            </div>
          </GroupList>
        )}
        {/* Doubao belongs in this list, not in a HarmonyOS-only arm: the probe is the shared one, and Windows and macOS have had it since it was added. Gating it on HarmonyOS alone silently dropped the button on the two hosts whose tests cover it. */}
        <VoiceAsrServiceTestSection
          available={windowsPlatform || macosPlatform || harmonyPlatform}
          voiceInput={voiceInput}
          doubaoAuthMode={doubaoAuthMode}
          credentialTestControl={credentialTestControl}
        />
        <VoiceRecognitionResultSection
          showStreamPreedit={showVoiceStreamPreedit}
          showCommitMode={showVoiceCommitMode}
          macos={macosPlatform}
          streamInlinePreedit={voiceInput.stream_inline_preedit === true}
          commitMode={voiceInput.commit_mode ?? "tsf"}
          onStreamInlinePreeditChange={(stream_inline_preedit) =>
            updateVoice({ stream_inline_preedit })
          }
          onCommitModeChange={(commit_mode) => updateVoice({ commit_mode })}
          grouped
        />
        {showVoiceCaptureDevices && (
          <VoiceCaptureDevicesSection
            windows={windowsPlatform}
            harmony={harmonyPlatform}
            backend={voiceInput.capture_backend ?? ""}
            device={voiceInput.capture_device ?? ""}
            backendOptions={captureBackendOptions}
            readDevices={client.listVoiceCaptureDevices!}
            onBackendChange={(capture_backend, capture_device) =>
              updateVoice({ capture_backend, capture_device })
            }
            onDeviceChange={(capture_device) => updateVoice({ capture_device })}
          />
        )}
        <VoiceRecordingBehaviorSettingsSection
          android={androidPlatform}
          linux={linuxPlatform}
          voiceInput={voiceInput}
          updateVoice={updateVoice}
        />
        {showVoiceProviderSettings && voiceInput.asr_provider === "doubao" && (
          <DoubaoOptionsSection
            linux={linuxPlatform}
            enableItn={voiceInput.doubao_enable_itn !== false}
            enablePunc={voiceInput.doubao_enable_punc !== false}
            enableDdc={voiceInput.doubao_enable_ddc === true}
            boostingTableId={voiceInput.doubao_boosting_table_id ?? ""}
            onEnableItnChange={(doubao_enable_itn) => updateVoice({ doubao_enable_itn })}
            onEnablePuncChange={(doubao_enable_punc) => updateVoice({ doubao_enable_punc })}
            onEnableDdcChange={(doubao_enable_ddc) => updateVoice({ doubao_enable_ddc })}
            onBoostingTableIdChange={(doubao_boosting_table_id) =>
              updateVoice({ doubao_boosting_table_id })
            }
          />
        )}
        {showVoiceProviderSettings && (
          <VoicePolishSettingsSection
            voiceInput={voiceInput}
            linux={linuxPlatform}
            providerPresetControls={providerPresetControls}
            updateVoice={updateVoice}
          >
            {(windowsPlatform || macosPlatform || iosPlatform || harmonyPlatform) &&
              credentialTestControl(
                "voice.polish",
                "测试语音润色配置",
                polishServiceCredentialTestConfig(voiceInput),
                polishServiceCredentialTestDisabled(voiceInput),
              )}
          </VoicePolishSettingsSection>
        )}
        {showVoiceProviderSettings && linuxPlatform && (
          <VoiceCredentialControl
            available={Boolean(client.providerCredentials)}
            kind="polish"
            voiceInput={voiceInput}
            doubaoAuthMode={doubaoAuthMode}
            providerCredentials={providerCredentials}
            voiceCredentialInput={voiceCredentialInput}
            setVoiceCredentialInput={setVoiceCredentialInput}
            providerCredentialBusy={providerCredentialBusy}
            providerCredentialMessages={providerCredentialMessages}
            runVoiceCredential={runVoiceCredential}
          />
        )}
        {showVoiceProviderSettings && linuxPlatform && client.testApiCredential && (
          <GroupList title="检查润色配置">
            <div className={settings.groupBlock}>
              {credentialTestControl(
                "voice.polish",
                "测试语音润色配置",
                polishProviderCredentialTestConfig(voiceInput),
                !polishEnabled,
              )}
            </div>
          </GroupList>
        )}
        {showVoiceHotkeys && (
          <VoiceHotkeysSection
            platform={
              macosPlatform
                ? "macos"
                : windowsPlatform
                  ? "windows"
                  : linuxPlatform
                    ? "linux"
                    : harmonyPlatform
                      ? "harmony"
                      : "other"
            }
            values={draft.voice_input ?? {}}
            onChange={(key, checked) => updateVoice({ [key]: checked })}
          />
        )}
      </div>
    </fieldset>
  );
}
