import type { Dispatch, SetStateAction } from "react";
import * as settings from "./settings-style";
import type {
  Preferences,
  ProviderCredentialClient,
  ProviderCredentialStatus,
  SettingsClient,
  VoiceInputPreferences,
  VoiceCredentialKind,
} from "../index";
import { isVoicePolishEnabled } from "./voice-input-defaults";
import { isAsrServiceProvider } from "../voice/voice-providers";
import { VoiceInputBasicsSection } from "./voice-input-basics-section";
import { VoiceLocalModelSettingsSection } from "./voice-local-model-settings-section";
import { VoiceAsrProviderSettingsSection } from "./voice-asr-provider-settings-section";
import { VoiceRecognitionResultSection } from "./voice-recognition-result-section";
import {
  VoiceCaptureDevicesSection,
  type VoiceCaptureBackendOption,
} from "./voice-capture-devices-section";
import { VoiceAsrServiceTestSection } from "./voice-asr-service-test-section";
import { VoiceHotkeysSection } from "./voice-hotkeys-section";
import { VoicePolishSettingsSection } from "./voice-polish-settings-section";
import { DoubaoOptionsRows } from "./doubao-options-section";
import { VoiceRecordingBehaviorSettingsSection } from "./voice-recording-behavior-settings-section";
import { VoiceCredentialControl } from "./voice-credential-control";
import {
  asrProviderCredentialTestConfig,
  polishProviderCredentialTestConfig,
  polishServiceCredentialTestConfig,
  polishServiceCredentialTestDisabled,
} from "./voice-credential-test-config";
import type { ProviderPresetControlFactory } from "./provider-preset-control";
import type { useProviderCredentials } from "./use-provider-credentials";
import { GroupList } from "../core/platform-controls";

export interface VoiceSettingsContentProps {
  disabled: boolean;
  hidden: boolean;
  client: SettingsClient;
  draft: Preferences;
  setDraft: Dispatch<SetStateAction<Preferences | undefined>>;
  confirm: (request: {
    message: string;
    title?: string;
    confirmLabel?: string;
    danger?: boolean;
  }) => Promise<boolean>;
  openExternalUrl: (url: string) => Promise<void>;
  openPanel: (action: (() => Promise<void>) | undefined) => Promise<void>;
  voiceInput: VoiceInputPreferences;
  systemVoice: boolean;
  systemVoiceHostName: string;
  localVoiceAvailable: boolean;
  localVoice: boolean;
  serviceVoice: boolean;
  harmonyUnsupportedAsr: boolean;
  doubaoAuthMode: "api_key" | "legacy";
  updateVoice: (patch: Partial<VoiceInputPreferences>) => void;
  providerCredentials: ProviderCredentialStatus | undefined;
  voiceCredentialInput: ReturnType<typeof useProviderCredentials>["voiceCredentialInput"];
  setVoiceCredentialInput: ReturnType<typeof useProviderCredentials>["setVoiceCredentialInput"];
  providerCredentialBusy: ReturnType<typeof useProviderCredentials>["providerCredentialBusy"];
  providerCredentialMessages: ReturnType<
    typeof useProviderCredentials
  >["providerCredentialMessages"];
  runVoiceCredential: (
    kind: VoiceCredentialKind,
    operation: (
      credentials: ProviderCredentialClient,
    ) => ReturnType<ProviderCredentialClient["saveVoice"]>,
    success: string,
  ) => Promise<void>;
  credentialTestControl: ReturnType<typeof useProviderCredentials>["credentialTestControl"];
  providerPresetControls: ProviderPresetControlFactory;
  androidPlatform: boolean;
  iosPlatform: boolean;
  macosPlatform: boolean;
  harmonyPlatform: boolean;
  linuxPlatform: boolean;
  windowsPlatform: boolean;
  mobilePlatform: boolean;
  nativeVoicePlatform: boolean;
  showVoiceHotkeys: boolean;
  showCredentialTests: boolean;
  showVoiceProviderSettings: boolean;
  showVoiceStreamPreedit: boolean;
  showVoiceCommitMode: boolean;
  showVoiceCaptureDevices: boolean;
  captureBackendOptions: readonly VoiceCaptureBackendOption[];
}

/** Complete shared voice settings page; host state and effects stay with SettingsPage. */
export function VoiceSettingsContent({
  disabled,
  hidden,
  client,
  draft,
  setDraft,
  confirm,
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
  androidPlatform,
  iosPlatform,
  macosPlatform,
  harmonyPlatform,
  linuxPlatform,
  windowsPlatform,
  mobilePlatform,
  nativeVoicePlatform,
  showVoiceHotkeys,
  showCredentialTests,
  showVoiceProviderSettings,
  showVoiceStreamPreedit,
  showVoiceCommitMode,
  showVoiceCaptureDevices,
  captureBackendOptions,
}: VoiceSettingsContentProps) {
  const polishEnabled = isVoicePolishEnabled(voiceInput);
  const asrProviderSettings = (
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
  );
  const asrCredentialTest =
    showCredentialTests && linuxPlatform
      ? credentialTestControl(
          "voice.asr",
          "测试语音识别配置",
          asrProviderCredentialTestConfig(voiceInput, doubaoAuthMode),
        )
      : null;
  const polishProviderCredentialTest =
    showCredentialTests && linuxPlatform
      ? credentialTestControl(
          "voice.polish",
          "测试语音润色配置",
          polishProviderCredentialTestConfig(voiceInput),
          !polishEnabled,
        )
      : null;
  const polishServiceCredentialTest =
    windowsPlatform || macosPlatform || iosPlatform || harmonyPlatform
      ? credentialTestControl(
          "voice.polish",
          "测试语音润色配置",
          polishServiceCredentialTestConfig(voiceInput),
          polishServiceCredentialTestDisabled(voiceInput),
        )
      : null;
  const basics = (
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
  );
  const localModelSettings = (
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
  );
  const credentialControl = (kind: VoiceCredentialKind) => (
    <VoiceCredentialControl
      available={Boolean(client.providerCredentials)}
      kind={kind}
      voiceInput={voiceInput}
      doubaoAuthMode={doubaoAuthMode}
      providerCredentials={providerCredentials}
      voiceCredentialInput={voiceCredentialInput}
      setVoiceCredentialInput={setVoiceCredentialInput}
      providerCredentialBusy={providerCredentialBusy}
      providerCredentialMessages={providerCredentialMessages}
      runVoiceCredential={runVoiceCredential}
    />
  );
  const asrCredentialVisible =
    linuxPlatform && isAsrServiceProvider(voiceInput.asr_provider ?? "doubao");
  const doubaoOptionsProps = {
    linux: linuxPlatform,
    enableItn: voiceInput.doubao_enable_itn !== false,
    enablePunc: voiceInput.doubao_enable_punc !== false,
    enableDdc: voiceInput.doubao_enable_ddc === true,
    boostingTableId: voiceInput.doubao_boosting_table_id ?? "",
    onEnableItnChange: (doubao_enable_itn: boolean) => updateVoice({ doubao_enable_itn }),
    onEnablePuncChange: (doubao_enable_punc: boolean) => updateVoice({ doubao_enable_punc }),
    onEnableDdcChange: (doubao_enable_ddc: boolean) => updateVoice({ doubao_enable_ddc }),
    onBoostingTableIdChange: (doubao_boosting_table_id: string) =>
      updateVoice({ doubao_boosting_table_id }),
  };
  const doubaoOptionsVisible = showVoiceProviderSettings && voiceInput.asr_provider === "doubao";
  const hotkeys = showVoiceHotkeys && (
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
      onChange={(key, enabled) => updateVoice({ [key]: enabled })}
    />
  );
  const recognitionResult = (
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
    />
  );
  const captureDevices = showVoiceCaptureDevices && (
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
  );
  const recordingBehavior = (
    <VoiceRecordingBehaviorSettingsSection
      android={androidPlatform}
      linux={linuxPlatform}
      voiceInput={voiceInput}
      updateVoice={updateVoice}
    />
  );
  const providerConfigVisible =
    (showVoiceProviderSettings && (serviceVoice || voiceInput.asr_provider === "doubao")) ||
    localVoice ||
    asrCredentialVisible ||
    Boolean(asrCredentialTest);
  // 设置窗口的语音页：先选服务并把它配好（识别服务配置的末尾是检查按钮），再是快捷键、录音时的行为和识别结果怎么用，润色是可选的另一项服务，录音设备很少要改，放在最后。
  const content = (
    <>
      {basics}
      {providerConfigVisible && (
        <GroupList title="识别服务配置">
          {asrProviderSettings}
          {asrCredentialVisible && credentialControl("asr")}
          {doubaoOptionsVisible && <DoubaoOptionsRows {...doubaoOptionsProps} />}
          {localModelSettings}
          {asrCredentialTest && <div className={settings.groupBlock}>{asrCredentialTest}</div>}
          <VoiceAsrServiceTestSection
            available={windowsPlatform || macosPlatform || harmonyPlatform}
            voiceInput={voiceInput}
            doubaoAuthMode={doubaoAuthMode}
            credentialTestControl={credentialTestControl}
          />
        </GroupList>
      )}
      {hotkeys}
      {recordingBehavior}
      {recognitionResult}
      {showVoiceProviderSettings && (
        <VoicePolishSettingsSection
          voiceInput={voiceInput}
          linux={linuxPlatform}
          providerPresetControls={providerPresetControls}
          updateVoice={updateVoice}
          linuxCredentials={credentialControl("polish")}
        >
          {polishProviderCredentialTest}
          {polishServiceCredentialTest}
        </VoicePolishSettingsSection>
      )}
      {captureDevices}
    </>
  );
  return (
    <fieldset disabled={disabled} hidden={hidden} aria-label="语音输入">
      <div className={settings.groups}>{content}</div>
    </fieldset>
  );
}
