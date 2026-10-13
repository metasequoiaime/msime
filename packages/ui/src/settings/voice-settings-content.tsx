import { useEffect, useState, type Dispatch, type SetStateAction } from "react";
import { fullScreenColumn, fullScreenSurface } from "../keyboard/touch-keyboard-skin-editor";
import { pushMobileSettingsState } from "./mobile-navigation";
import { useMobilePopState } from "./use-mobile-pop-state";
import * as settings from "./settings-style";
import { SettingsPageHeader } from "./settings-page-header";
import { SettingsGroupBlock } from "./settings-group-block";
import type {
  Preferences,
  ProviderCredentialStatus,
  SettingsClient,
  VoiceInputPreferences,
  VoiceCredentialKind,
} from "../index";
import { isVoicePolishEnabled } from "./voice-input-defaults";
import { asrProviderUpdate, isAsrServiceProvider } from "../voice/voice-providers";
import { VoiceInputBasicsSection } from "./voice-input-basics-section";
import { MobileVoiceLanguageRow, VoiceInputServiceRows } from "./voice-input-core-section";
import { harmonyPhoneVoiceFooter } from "./voice-input-intro-section";
import { SwitchRow } from "./switch-row";
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
  asrProviderCredentialTestDisabled,
  polishProviderCredentialTestConfig,
  polishServiceCredentialTestConfig,
  polishServiceCredentialTestDisabled,
} from "./voice-credential-test-config";
import type { ProviderPresetControlFactory } from "./provider-preset-control";
import type { useProviderCredentials, VoiceCredentialBindings } from "./use-provider-credentials";
import { GroupList, LinkRow, MoreOptions } from "../core/platform-controls";
import * as platform from "../core/platform-controls-style";
import { useSettingsTabs } from "./use-settings-tabs";
import { SettingsPageFieldset } from "./settings-page-fieldset";

/** HarmonyOS 手机上豆包「服务配置」行的状态，与键盘的 `DoubaoCredentialPolicy` 同一套判断：密钥取当前框，空时取 `asr_tokens.doubao`；一项都没填时键盘用系统识别，填了一部分时键盘报缺项。 */
export function doubaoServiceStatus(voiceInput: VoiceInputPreferences): string {
  const token = (voiceInput.asr_token ?? "").trim() || (voiceInput.asr_tokens?.doubao ?? "").trim();
  const appKey = (voiceInput.asr_app_key ?? "").trim();
  if (!token && !appKey) return "未填写，暂用系统识别";
  if (!token || (voiceInput.doubao_auth_mode === "legacy" && !appKey)) return "未填写完整";
  return "已填写";
}

export interface VoiceSettingsContentProps extends VoiceCredentialBindings {
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

/** 共用语音设置页，宿主状态与副作用由设置表单持有。 */
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
  const { tabList, panel } = useSettingsTabs(
    [
      { id: "service", title: "识别服务" },
      { id: "recording", title: "录音与结果" },
      ...(showVoiceProviderSettings ? [{ id: "polish", title: "润色" }] : []),
      ...(showVoiceHotkeys ? [{ id: "hotkeys", title: "快捷键" }] : []),
      ...(showVoiceCaptureDevices ? [{ id: "devices", title: "录音设备" }] : []),
    ],
    "语音输入设置分类",
    !mobilePlatform,
  );
  // HarmonyOS 手机上「服务配置」子视图开着没有。和「浏览更多」图库一样压一条历史记录，系统返回手势先关它；离开语音页时它随之关上。
  const [phoneServiceOpen, setPhoneServiceOpen] = useState(false);
  useMobilePopState(mobilePlatform, (event) => {
    const state = event.state as Record<string, unknown> | null;
    setPhoneServiceOpen(state?.voiceService === true);
  });
  useEffect(() => {
    if (hidden) setPhoneServiceOpen(false);
  }, [hidden]);
  const openPhoneService = () => {
    pushMobileSettingsState({ voiceService: true });
    setPhoneServiceOpen(true);
  };
  const closePhoneService = () => window.history.back();
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
          asrProviderCredentialTestDisabled(voiceInput),
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
      windows={windowsPlatform}
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
  const polish = showVoiceProviderSettings && (
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
  );
  const asrServiceTest = (
    <VoiceAsrServiceTestSection
      available={windowsPlatform || macosPlatform || harmonyPlatform}
      voiceInput={voiceInput}
      doubaoAuthMode={doubaoAuthMode}
      credentialTestControl={credentialTestControl}
    />
  );
  if (harmonyPlatform && mobilePlatform) {
    const punctuationSupported = voiceInput.asr_provider === "doubao";
    // 「服务配置」行右侧的状态：网络服务看有没有填密钥，本地识别看有没有选模型；系统识别不用配置，不画这一行。豆包是出厂选项，一项凭据都没填时键盘用的其实是系统识别，要照实说；填了一部分时键盘会报缺项，这里说没填完整。
    const serviceStatus = serviceVoice
      ? voiceInput.asr_provider === "doubao"
        ? doubaoServiceStatus(voiceInput)
        : (voiceInput.asr_token ?? "").trim()
          ? "已填写"
          : "未填写"
      : localVoice
        ? voiceInput.asr_model_path
          ? "已选模型"
          : "未选模型"
        : null;
    const soundEnabled = voiceInput.sound_enabled !== false;
    if (phoneServiceOpen)
      // 服务配置子视图：密钥、接口、模型、测试这些只有配置服务的人才看，连同润色服务、提示音细分和录音设备，都收在这里，不摊在语音页上。
      return (
        <SettingsPageFieldset disabled={disabled} hidden={hidden} ariaLabel="语音输入">
          <div className={fullScreenSurface} role="dialog" aria-modal="true" aria-label="服务配置">
            <section className={fullScreenColumn}>
              <SettingsPageHeader title="服务配置" onBack={closePhoneService} level={2} />
              <GroupList title="识别服务">
                {asrProviderSettings}
                {doubaoOptionsVisible && (
                  <MoreOptions>
                    <DoubaoOptionsRows {...doubaoOptionsProps} punctuation={false} />
                  </MoreOptions>
                )}
                {localModelSettings}
                {asrServiceTest}
              </GroupList>
              {polish}
              <GroupList title="提示音">
                <SwitchRow
                  title="开始录音提示音"
                  disabled={!soundEnabled}
                  checked={voiceInput.start_sound !== false}
                  onChange={(start_sound) => updateVoice({ start_sound })}
                />
                <SwitchRow
                  title="结束录音提示音"
                  disabled={!soundEnabled}
                  checked={voiceInput.end_sound !== false}
                  onChange={(end_sound) => updateVoice({ end_sound })}
                />
              </GroupList>
              {captureDevices}
            </section>
          </div>
        </SettingsPageFieldset>
      );
    // HarmonyOS 手机与 Android 的 `VoicePage` 一样遵循设计稿：「识别」组放语言、标点和边说边显示，「识别服务」组放总开关、服务和它的配置入口，「录音」组只留两个常用开关。密钥、接口、模型、润色服务、提示音细分和录音设备都在「服务配置」子视图里。设计稿中的「离线识别」「启动方式」「隐私」在 HarmonyOS 上没有对应功能（没有离线回退，没有上传管线，键盘工具栏也不再有可供 `touch_voice_shortcut` 显示的语音按钮），因此不绘制。不画「语音快捷键」：Ctrl+F9、长按右 Alt 这些要连实体键盘才用得上，2in1 照旧显示。
    return (
      <SettingsPageFieldset disabled={disabled} hidden={hidden} ariaLabel="语音输入">
        <GroupList title="识别">
          <MobileVoiceLanguageRow
            language={voiceInput.language}
            systemVoice={systemVoice}
            onLanguageChange={(language) => updateVoice({ language })}
          />
          <SwitchRow
            title="自动添加标点"
            description={
              punctuationSupported ? undefined : "当前识别服务不支持，仅豆包语音识别可以自动加标点"
            }
            disabled={!punctuationSupported}
            checked={voiceInput.doubao_enable_punc !== false}
            onChange={(doubao_enable_punc) => updateVoice({ doubao_enable_punc })}
          />
          {showVoiceStreamPreedit && (
            <SwitchRow
              title="边说边显示"
              checked={voiceInput.stream_inline_preedit === true}
              onChange={(stream_inline_preedit) => updateVoice({ stream_inline_preedit })}
            />
          )}
        </GroupList>
        <div className={platform.group}>
          <GroupList title="识别服务">
            <VoiceInputServiceRows
              enabled={voiceInput.enabled}
              provider={String(voiceInput.asr_provider)}
              showProviderSettings={showVoiceProviderSettings}
              macos={false}
              harmony
              android={false}
              localVoiceAvailable={localVoiceAvailable}
              nativeVoicePlatform={nativeVoicePlatform}
              harmonyUnsupportedAsr={harmonyUnsupportedAsr}
              onEnabledChange={(enabled) => updateVoice({ enabled })}
              onProviderChange={(provider) =>
                updateVoice({
                  ...asrProviderUpdate(provider, voiceInput),
                  // CoreSpeechKit 只识别普通话，且语言是作为它的 locale 传入的，所以选择它时固定为 `zh-CN`，而不是传入一个它会拒绝的语言。
                  ...(provider === "system" ? { language: "zh-CN" } : {}),
                })
              }
            />
            {serviceStatus && (
              <LinkRow title="服务配置" value={serviceStatus} onClick={openPhoneService} />
            )}
          </GroupList>
          <p className={platform.groupFooter}>
            {harmonyPhoneVoiceFooter({
              localVoice,
              localVoiceModelsAvailable: Boolean(client.localVoiceModels),
              systemVoice,
            })}
          </p>
        </div>
        <GroupList title="录音">
          <SwitchRow
            title="提示音"
            checked={soundEnabled}
            onChange={(sound_enabled) => updateVoice({ sound_enabled })}
          />
          <SwitchRow
            title="录音时静音其他声音"
            checked={voiceInput.mute_system_audio === true}
            onChange={(mute_system_audio) => updateVoice({ mute_system_audio })}
          />
        </GroupList>
      </SettingsPageFieldset>
    );
  }
  // 设置窗口的语音页：先选服务并把它配好（识别服务配置的末尾是检查按钮），再是快捷键、录音时的行为和识别结果怎么用，润色是可选的另一项服务，录音设备很少要改，放在最后。
  const content = (
    <>
      {tabList}
      {panel(
        "service",
        <>
          {basics}
          {providerConfigVisible && (
            <GroupList title="识别服务配置">
              {asrProviderSettings}
              {asrCredentialVisible && credentialControl("asr")}
              {doubaoOptionsVisible && <DoubaoOptionsRows {...doubaoOptionsProps} />}
              {localModelSettings}
              {asrCredentialTest && <SettingsGroupBlock>{asrCredentialTest}</SettingsGroupBlock>}
              {asrServiceTest}
            </GroupList>
          )}
        </>,
      )}
      {showVoiceHotkeys && panel("hotkeys", hotkeys)}
      {panel(
        "recording",
        <>
          {recordingBehavior}
          {recognitionResult}
        </>,
      )}
      {showVoiceProviderSettings && panel("polish", polish)}
      {showVoiceCaptureDevices && panel("devices", captureDevices)}
    </>
  );
  return (
    <SettingsPageFieldset disabled={disabled} hidden={hidden} ariaLabel="语音输入">
      {content}
    </SettingsPageFieldset>
  );
}
