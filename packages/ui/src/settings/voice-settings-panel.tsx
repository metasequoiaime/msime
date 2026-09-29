import type { Dispatch, SetStateAction } from "react";
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
import { VoiceStreamPreeditSection } from "./voice-stream-preedit-section";
import { VoiceCommitModeSection } from "./voice-commit-mode-section";
import {
  VoiceCaptureDevicesSection,
  type VoiceCaptureBackendOption,
} from "./voice-capture-devices-section";
import { VoiceAsrServiceTestSection } from "./voice-asr-service-test-section";
import { VoiceHotkeysSection } from "./voice-hotkeys-section";
import { VoicePolishSettingsSection } from "./voice-polish-settings-section";
import { DoubaoOptionsSection } from "./doubao-options-section";
import { VoiceRecordingBehaviorSection } from "./voice-recording-behavior-section";
import { VoiceCredentialControl } from "./voice-credential-control";
import {
  asrProviderCredentialTestConfig,
  polishProviderCredentialTestConfig,
  polishServiceCredentialTestConfig,
  polishServiceCredentialTestDisabled,
} from "./voice-credential-test-config";
import type { ProviderPresetControlFactory } from "./provider-preset-control";
import type { useProviderCredentials } from "./use-provider-credentials";

export interface VoiceSettingsPanelProps {
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
  desktopPanels: boolean;
  showVoiceProviderSettings: boolean;
  showVoiceStreamPreedit: boolean;
  showVoiceCommitMode: boolean;
  showVoiceCaptureDevices: boolean;
  captureBackendOptions: readonly VoiceCaptureBackendOption[];
}

/** Complete shared voice settings page; host state and effects stay with SettingsPage. */
export function VoiceSettingsPanel({
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
  desktopPanels,
  showVoiceProviderSettings,
  showVoiceStreamPreedit,
  showVoiceCommitMode,
  showVoiceCaptureDevices,
  captureBackendOptions,
}: VoiceSettingsPanelProps) {
  return (
    <fieldset disabled={disabled} hidden={hidden} aria-label="语音输入">
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
      <VoiceAsrProviderSettingsSection
        voiceInput={voiceInput}
        showProviderSettings={showVoiceProviderSettings}
        serviceVoice={serviceVoice}
        linux={linuxPlatform}
        doubaoAuthMode={doubaoAuthMode}
        providerPresetControls={providerPresetControls}
        updateVoice={updateVoice}
      />
      {linuxPlatform &&
        isAsrServiceProvider(voiceInput.asr_provider ?? "doubao") &&
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
        />}
      {linuxPlatform &&
        credentialTestControl(
          "voice.asr",
          "测试语音识别配置",
          asrProviderCredentialTestConfig(voiceInput, doubaoAuthMode),
        )}
      {/*
       * Doubao belongs in this list, not in a HarmonyOS-only arm: the probe is the
       * shared one, and Windows and macOS have had it since it was added. Gating it
       * on HarmonyOS alone silently dropped the button on the two hosts whose tests
       * cover it.
       */}
      <VoiceAsrServiceTestSection
        available={windowsPlatform || macosPlatform || harmonyPlatform}
        voiceInput={voiceInput}
        doubaoAuthMode={doubaoAuthMode}
        credentialTestControl={credentialTestControl}
      />
      {showVoiceStreamPreedit && (
        <VoiceStreamPreeditSection
          enabled={voiceInput.stream_inline_preedit === true}
          onChange={(stream_inline_preedit) => updateVoice({ stream_inline_preedit })}
        />
      )}
      {showVoiceCommitMode && (
        <VoiceCommitModeSection
          macos={macosPlatform}
          value={voiceInput.commit_mode ?? "tsf"}
          onChange={(commit_mode) => updateVoice({ commit_mode })}
        />
      )}
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
      {/* Not provider configuration: these four are the host's own recording
          behaviour, and the Android host plays no prompt tones and does not mute
          system audio while it records. Four switches with nothing behind them is
          what this page keeps being audited for. */}
      {!androidPlatform && (
        <VoiceRecordingBehaviorSection
          linux={linuxPlatform}
          soundEnabled={voiceInput.sound_enabled !== false}
          startSound={voiceInput.start_sound !== false}
          endSound={voiceInput.end_sound !== false}
          muteSystemAudio={voiceInput.mute_system_audio === true}
          onSoundEnabledChange={(sound_enabled) => updateVoice({ sound_enabled })}
          onStartSoundChange={(start_sound) => updateVoice({ start_sound })}
          onEndSoundChange={(end_sound) => updateVoice({ end_sound })}
          onMuteSystemAudioChange={(mute_system_audio) => updateVoice({ mute_system_audio })}
        />
      )}
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
          {linuxPlatform &&
            credentialTestControl(
              "voice.polish",
              "测试语音润色配置",
              polishProviderCredentialTestConfig(voiceInput),
              !isVoicePolishEnabled(voiceInput),
            )}
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
      {desktopPanels && (
        <VoiceHotkeysSection
          platform={
            macosPlatform
              ? "macos"
              : windowsPlatform
                ? "windows"
                : linuxPlatform
                  ? "linux"
                  : "other"
          }
          values={draft.voice_input ?? {}}
          onChange={(key, enabled) =>
            setDraft({
              ...draft,
              voice_input: {
                ...draft.voice_input,
                enabled: draft.voice_input?.enabled ?? true,
                language: draft.voice_input?.language ?? "zh-CN",
                [key]: enabled,
              },
            })
          }
        />
      )}
    </fieldset>
  );
}
