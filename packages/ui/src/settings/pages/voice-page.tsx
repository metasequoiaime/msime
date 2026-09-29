import * as settings from "../settings-style";
import {
  asrProviderUpdate,
  ASR_PROVIDER_DEFAULTS,
  isAsrServiceProvider,
  polishProviderUpdate,
  POLISH_PROVIDER_DEFAULTS,
} from "../../voice/voice-providers";
import { LocalModelManager, localModelInUse } from "../../voice/local-models";
import {
  defaultVoiceInput,
  isVoicePolishEnabled,
  voiceAsrTokenLabel,
} from "../voice-input-defaults";
import { useSettingsForm } from "../settings-form-context";
import { GroupList, Row, Select, Switch } from "../../core/platform-controls";
import { VoiceInputIntroSection } from "../voice-input-intro-section";
import { VoiceInputCoreSection } from "../voice-input-core-section";
import { VoiceModelMirrorSection } from "../voice-model-mirror-section";
import { VoiceModelPathDisclosure } from "../voice-model-path-disclosure";
import { VoiceModelSection } from "../voice-model-section";
import { DoubaoAuthModeSection } from "../doubao-auth-mode-section";
import { DoubaoStreamEndpointSection } from "../doubao-stream-endpoint-section";
import { VoiceEndpointSection } from "../voice-endpoint-section";
import { VoiceCredentialFieldsSection } from "../voice-credential-fields-section";
import { PolishCredentialFieldsSection } from "../polish-credential-fields-section";
import { DoubaoResourceIdSection } from "../doubao-resource-id-section";
import { VoiceStreamPreeditSection } from "../voice-stream-preedit-section";
import { VoiceCommitModeSection } from "../voice-commit-mode-section";
import { VoiceCaptureDevicesSection } from "../voice-capture-devices-section";
import { VoiceRecordingBehaviorSection } from "../voice-recording-behavior-section";
import { DoubaoOptionsSection } from "../doubao-options-section";
import { VoiceCredentialControl } from "../voice-credential-control";
import { VoiceHotkeysSection } from "../voice-hotkeys-section";
import { VoicePolishSection } from "../voice-polish-section";
import { PolishPromptSection } from "../polish-prompt-section";
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
    desktopPanels,
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
  const asrTokenLabel = voiceAsrTokenLabel(voiceInput.asr_provider, doubaoAuthMode);
  const polishEnabled = isVoicePolishEnabled(voiceInput);
  return (
    <fieldset disabled={busy} hidden={page !== "voice"} aria-label="语音输入">
      <div className={settings.groups}>
        <VoiceInputIntroSection
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
          onOpenVoice={client.openVoice ? () => void openPanel(client.openVoice) : undefined}
        />
        <VoiceInputCoreSection
          enabled={voiceInput.enabled}
          provider={String(voiceInput.asr_provider)}
          language={voiceInput.language}
          showProviderSettings={showVoiceProviderSettings}
          systemVoice={systemVoice}
          macos={macosPlatform}
          harmony={harmonyPlatform}
          android={androidPlatform}
          localVoiceAvailable={localVoiceAvailable}
          nativeVoicePlatform={nativeVoicePlatform}
          harmonyUnsupportedAsr={harmonyUnsupportedAsr}
          onEnabledChange={(enabled) => updateVoice({ enabled })}
          onProviderChange={(provider) =>
            updateVoice({
              ...asrProviderUpdate(provider, voiceInput),
              ...(provider === "system" && voiceInput.language === "auto"
                ? { language: "zh-CN" }
                : {}),
              ...(linuxPlatform ? { asr_resource_id: "", doubao_boosting_table_id: "" } : {}),
            })
          }
          onLanguageChange={(language) => updateVoice({ language })}
        />
        {localVoice && client.localVoiceModels && (
          <LocalModelManager
            client={client.localVoiceModels}
            mobile={mobilePlatform}
            modelPath={voiceInput.asr_model_path ?? ""}
            onUse={(asr_model_path) => updateVoice({ asr_model_path })}
            onRemoved={(model) =>
              // Checked against the draft as it is once the removal lands.
              setDraft((current) =>
                current && localModelInUse(model, current.voice_input?.asr_model_path ?? "")
                  ? {
                      ...current,
                      voice_input: {
                        ...defaultVoiceInput,
                        ...current.voice_input,
                        asr_model_path: "",
                      },
                    }
                  : current,
              )
            }
            confirm={confirm}
            openExternalUrl={client.openExternalUrl ? openExternalUrl : undefined}
          />
        )}
        {localVoice && client.localVoiceModels && (
          <VoiceModelMirrorSection
            value={voiceInput.asr_model_mirror ?? ""}
            onChange={(asr_model_mirror) => updateVoice({ asr_model_mirror })}
          />
        )}
        {localVoice && (
          <VoiceModelPathDisclosure
            disclosure={Boolean(client.localVoiceModels)}
            path={voiceInput.asr_model_path ?? ""}
            pickPath={client.pickVoiceModelPath}
            onChange={(asr_model_path) => updateVoice({ asr_model_path })}
          />
        )}
        {showVoiceProviderSettings && (serviceVoice || voiceInput.asr_provider === "doubao") && (
          <GroupList title="识别服务配置">
            {serviceVoice &&
              providerPresetControls(
                "识别服务",
                ASR_PROVIDER_DEFAULTS[String(voiceInput.asr_provider)],
                voiceInput.asr_model ?? "",
                (asr_model) => updateVoice({ asr_model }),
                settings.managerBlock,
              )}
            {serviceVoice && (
              <VoiceModelSection
                value={voiceInput.asr_model ?? ""}
                onChange={(asr_model) => updateVoice({ asr_model })}
              />
            )}
            {voiceInput.asr_provider === "doubao" && (
              <DoubaoAuthModeSection
                value={doubaoAuthMode}
                linux={linuxPlatform}
                onChange={(doubao_auth_mode) => updateVoice({ doubao_auth_mode })}
              />
            )}
            {!linuxPlatform && serviceVoice && (
              <>
                {voiceInput.asr_provider === "doubao" && (
                  <DoubaoStreamEndpointSection
                    endpoint={voiceInput.asr_endpoint ?? ""}
                    onChange={(asr_endpoint) => updateVoice({ asr_endpoint })}
                  />
                )}
                <VoiceEndpointSection
                  value={voiceInput.asr_endpoint ?? ""}
                  onChange={(asr_endpoint) => updateVoice({ asr_endpoint })}
                />
                <VoiceCredentialFieldsSection
                  showAppKey={voiceInput.asr_provider === "doubao" && doubaoAuthMode === "legacy"}
                  appKey={voiceInput.asr_app_key ?? ""}
                  tokenLabel={asrTokenLabel}
                  token={voiceInput.asr_token ?? ""}
                  onAppKeyChange={(asr_app_key) => updateVoice({ asr_app_key })}
                  onTokenChange={(asr_token) => updateVoice({ asr_token })}
                />
              </>
            )}
            {serviceVoice && (
              <DoubaoResourceIdSection
                value={voiceInput.asr_resource_id ?? ""}
                onChange={(asr_resource_id) => updateVoice({ asr_resource_id })}
              />
            )}
          </GroupList>
        )}
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
        {(showVoiceStreamPreedit || showVoiceCommitMode) && (
          <GroupList title="识别结果">
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
          </GroupList>
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
        {/* Not provider configuration: these four are the host's own recording behaviour, and the Android host plays no prompt tones and does not mute system audio while it records. Four switches with nothing behind them is what this page keeps being audited for. */}
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
          <VoicePolishSection
            enabled={polishEnabled}
            provider={voiceInput.polish_provider ?? "siliconflow"}
            model={voiceInput.polish_model ?? ""}
            providerPreset={providerPresetControls(
              "文本润色",
              POLISH_PROVIDER_DEFAULTS[voiceInput.polish_provider ?? "siliconflow"],
              voiceInput.polish_model ?? "",
              (polish_model) => updateVoice({ polish_model }),
              "provider-preset-section",
            )}
            onEnabledChange={(enabled) =>
              updateVoice({ polish_text: enabled, polish_enabled: enabled })
            }
            onProviderChange={(provider) => updateVoice(polishProviderUpdate(provider, voiceInput))}
            onModelChange={(polish_model) => updateVoice({ polish_model })}
          >
            {!linuxPlatform && (
              <PolishCredentialFieldsSection
                endpoint={voiceInput.polish_endpoint ?? ""}
                token={voiceInput.polish_token ?? ""}
                onEndpointChange={(polish_endpoint) => updateVoice({ polish_endpoint })}
                onTokenChange={(polish_token) => updateVoice({ polish_token })}
              />
            )}
            <PolishPromptSection
              promptId={voiceInput.polish_prompt_id}
              prompt={voiceInput.polish_prompt ?? ""}
              customPrompts={{
                custom_1: voiceInput.polish_prompt_custom_1,
                custom_2: voiceInput.polish_prompt_custom_2,
                custom_3: voiceInput.polish_prompt_custom_3,
              }}
              onSelectPrompt={(polish_prompt_id, polish_prompt) =>
                updateVoice({ polish_prompt_id, polish_prompt })
              }
              onPromptChange={(polish_prompt, customSlot) =>
                updateVoice({
                  polish_prompt,
                  ...(customSlot ? { [`polish_prompt_${customSlot}`]: polish_prompt } : {}),
                })
              }
              onRestore={(polish_prompt) => updateVoice({ polish_prompt })}
            />
            {(windowsPlatform || macosPlatform || iosPlatform || harmonyPlatform) &&
              credentialTestControl(
                "voice.polish",
                "测试语音润色配置",
                polishServiceCredentialTestConfig(voiceInput),
                polishServiceCredentialTestDisabled(voiceInput),
              )}
          </VoicePolishSection>
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
            onChange={(key, checked) =>
              setDraft({
                ...draft,
                voice_input: {
                  ...draft.voice_input,
                  enabled: draft.voice_input?.enabled ?? true,
                  language: draft.voice_input?.language ?? "zh-CN",
                  [key]: checked,
                },
              })
            }
          />
        )}
      </div>
    </fieldset>
  );
}
