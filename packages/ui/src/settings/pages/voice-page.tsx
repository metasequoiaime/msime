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
import {
  POLISH_PRESET_IDS,
  POLISH_PRESET_NAMES,
  isPolishCustomSlot,
  normalizePolishSlot,
  polishPromptFor,
  polishSlotField,
} from "../../voice/polish-presets";
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
import {
  asrProviderCredentialTestConfig,
  asrServiceCredentialTestConfig,
  asrServiceCredentialTestDisabled,
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
  const polishSlot = normalizePolishSlot(voiceInput.polish_prompt_id);
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
        {(windowsPlatform || macosPlatform || harmonyPlatform) &&
          isAsrServiceProvider(voiceInput.asr_provider ?? "") && (
            <GroupList title="检查识别配置">
              <p className={settings.groupNote}>
                测试会向当前服务发送一秒合成静音，不使用麦克风；服务可能计入 API 用量。
              </p>
              {client.testApiCredential && (
                <div className={settings.groupBlock}>
                  {credentialTestControl(
                    "voice.asr",
                    voiceInput.asr_provider === "doubao" ? "测试豆包识别配置" : "测试语音识别配置",
                    asrServiceCredentialTestConfig(voiceInput, doubaoAuthMode),
                    asrServiceCredentialTestDisabled(voiceInput, doubaoAuthMode),
                  )}
                </div>
              )}
            </GroupList>
          )}
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
          <GroupList title="文本润色 provider">
            <p className={settings.groupNote}>识别结果可交给用户管理的服务润色</p>
            <Row title="启用润色">
              <Switch
                aria-label="启用文本润色"
                checked={polishEnabled}
                onChange={(checked) =>
                  updateVoice({ polish_text: checked, polish_enabled: checked })
                }
              />
            </Row>
            <Row title="服务提供商">
              <Select
                aria-label="文本润色服务提供商"
                value={voiceInput.polish_provider ?? "siliconflow"}
                onChange={(event) =>
                  updateVoice(polishProviderUpdate(event.target.value, voiceInput))
                }
              >
                <option value="siliconflow">SiliconFlow</option>
                <option value="openai">OpenAI</option>
                <option value="deepseek">DeepSeek</option>
                <option value="groq">Groq</option>
              </Select>
            </Row>
            {providerPresetControls(
              "文本润色",
              POLISH_PROVIDER_DEFAULTS[voiceInput.polish_provider ?? "siliconflow"],
              voiceInput.polish_model ?? "",
              (polish_model) => updateVoice({ polish_model }),
              settings.managerBlock,
            )}
            <Row title="模型">
              <input
                aria-label="文本润色模型"
                value={voiceInput.polish_model ?? ""}
                onChange={(event) => updateVoice({ polish_model: event.target.value })}
              />
            </Row>
            {!linuxPlatform && (
              <PolishCredentialFieldsSection
                endpoint={voiceInput.polish_endpoint ?? ""}
                token={voiceInput.polish_token ?? ""}
                onEndpointChange={(polish_endpoint) => updateVoice({ polish_endpoint })}
                onTokenChange={(polish_token) => updateVoice({ polish_token })}
              />
            )}
            <Row title="润色方案">
              <Select
                aria-label="润色方案"
                value={polishSlot}
                onChange={(event) =>
                  updateVoice({
                    polish_prompt_id: event.target.value,
                    polish_prompt: polishPromptFor(event.target.value, voiceInput),
                  })
                }
              >
                {POLISH_PRESET_IDS.map((id) => (
                  <option key={id} value={id}>
                    {POLISH_PRESET_NAMES[id]}
                  </option>
                ))}
                <option value="custom_1">自定义一</option>
                <option value="custom_2">自定义二</option>
                <option value="custom_3">自定义三</option>
              </Select>
            </Row>
            <div className={settings.managerBlock}>
              <label className={settings.field}>
                <span>
                  <span data-row-title="">润色提示词</span>{" "}
                  {isPolishCustomSlot(polishSlot)
                    ? "这一段会保存到所选的自定义方案"
                    : "内置方案的完整提示词，可以就地修改"}
                </span>
                <textarea
                  aria-label="润色提示词"
                  className={settings.promptInput}
                  value={voiceInput.polish_prompt ?? ""}
                  onChange={(event) =>
                    updateVoice({
                      polish_prompt: event.target.value,
                      ...(polishSlotField(polishSlot)
                        ? {
                            [polishSlotField(polishSlot) as string]: event.target.value,
                          }
                        : {}),
                    })
                  }
                />
              </label>
              <div className={settings.managerActions}>
                <button
                  type="button"
                  className="secondary"
                  disabled={
                    (voiceInput.polish_prompt ?? "") === polishPromptFor(polishSlot, voiceInput)
                  }
                  onClick={() =>
                    updateVoice({
                      polish_prompt: polishPromptFor(polishSlot, voiceInput),
                    })
                  }
                >
                  恢复默认
                </button>
                {(windowsPlatform || macosPlatform || iosPlatform || harmonyPlatform) &&
                  credentialTestControl(
                    "voice.polish",
                    "测试语音润色配置",
                    polishServiceCredentialTestConfig(voiceInput),
                    polishServiceCredentialTestDisabled(voiceInput),
                  )}
              </div>
            </div>
          </GroupList>
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
          <GroupList title="语音快捷键">
            <p className={settings.groupNote}>
              {linuxPlatform
                ? "在当前输入上下文中生效。长按快捷键录音，松开结束；按住期间按空格锁定录音，Escape 取消。Ctrl+F9 按一次开始、再按一次结束，也能结束锁定的录音。没有 provider 时快捷键不会拦截编辑器输入"
                : macosPlatform
                  ? "输入法启用时按住修饰键快捷键录音，松开结束；组合键先按 Control。按住期间按空格锁定，Escape 取消。修饰键快捷键由输入法自身接收，不需要额外授权；Ctrl+F9 在输入法会话之外接收，需要在「系统设置 › 隐私与安全性 › 输入监控」中允许本输入法，否则按下没有任何反应。首次授权后请重新按键。"
                  : windowsPlatform
                    ? "输入法运行时全局生效。长按快捷键录音，松开结束；按住期间按空格锁定录音，锁定后再按一次快捷键或点 ✓ 结束，Escape 或 ✗ 取消。Ctrl+F9 按一次开始、再按一次结束。"
                    : "输入法运行时全局生效，用于开始和结束语音录音"}
            </p>
            {/* Both Linux hosts record while a modifier shortcut is held and lock on Space, as Windows does, so they share its labels; only Ctrl+F9 toggles. Only IBus requires the right Ctrl in the two-key chord: Fcitx5 starts on a Right Ctrl or Right Alt press while any Ctrl or Alt is down (so left Ctrl+Right Alt also records) and stops only when Right Alt or Right Ctrl is released. The label still holds because the right-Ctrl chord works on both hosts. */}
            {(
              [
                ["hotkey_ctrl_f9", "Ctrl+F9 切换语音"],
                [
                  "hotkey_ralt",
                  macosPlatform
                    ? "按住右 Option 录音"
                    : windowsPlatform || linuxPlatform
                      ? "长按右 Alt 录音"
                      : "右 Alt 切换语音",
                ],
                [
                  "hotkey_rctrl_ralt",
                  macosPlatform
                    ? "按住右 Control+右 Option 录音"
                    : windowsPlatform || linuxPlatform
                      ? "长按右 Ctrl+右 Alt 录音"
                      : "Ctrl+右 Alt 切换语音",
                ],
                [
                  "hotkey_ctrl_win",
                  macosPlatform
                    ? "按住 Control+Command 录音"
                    : windowsPlatform || linuxPlatform
                      ? "长按 Ctrl+Win 录音"
                      : "Ctrl+Win 切换语音",
                ],
                [
                  "hotkey_hold_space_lock",
                  windowsPlatform || linuxPlatform ? "长按录音时按空格锁定" : "空格锁定语音",
                ],
              ] as const
            ).map(([key, label]) => (
              <Row key={key} title={label}>
                <Switch
                  aria-label={label}
                  checked={draft.voice_input?.[key] !== false}
                  onChange={(checked) =>
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
              </Row>
            ))}
          </GroupList>
        )}
      </div>
    </fieldset>
  );
}
