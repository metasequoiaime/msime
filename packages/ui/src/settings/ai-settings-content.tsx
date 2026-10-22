import type { ProviderCredentialStatus } from "../core/provider-credential-contracts";
import type { ReactNode } from "react";
import type { AiAssistantPreferences } from "./ai-assistant-preferences";
import type { SettingsClient } from "../core/settings-client";
import type { useProviderCredentials } from "./use-provider-credentials";
import { aiProviderOption, aiProviderOptionsFor, aiProviderUsesToken } from "./ai-provider-options";
import { aiProviderUpdate, aiEndpointUpdate, aiModelUpdate } from "./ai-provider-update";
import {
  aiCredentialTestDisabled,
  aiProviderCredentialTestConfig,
  aiServiceCredentialTestConfig,
  aiServiceCredentialTestDisabled,
} from "./ai-credential-test-config";
import { aiCredentialSaveConfig } from "./ai-credential-save-config";
import { AiSettingsPageSection } from "./ai-settings-page-section";
import { AiCredentialSection } from "./ai-credential-section";
import { AiLinuxProviderSection } from "./ai-linux-provider-section";
import { AiApiTokenSection } from "./ai-api-token-section";
import { aiEndpointHint } from "./ai-endpoint-policy";
import { AiTestToolsSection } from "./ai-test-tools-section";
import { NetworkProxySection } from "./network-proxy-section";
import type { NetworkProxyPreferences } from "./network-proxy-preferences";
import type { ProviderPresetControlFactory } from "./provider-preset-control";
import { useMountedRef } from "./use-mounted-ref";
import * as settings from "./settings-style";

export interface AiSettingsContentProps {
  disabled: boolean;
  hidden: boolean;
  client: SettingsClient;
  ai: AiAssistantPreferences;
  updateAi: (patch: Partial<AiAssistantPreferences>) => void;
  aiOrigin: string | null;
  aiToken: string;
  updateAiToken: (value: string) => void;
  aiModels: string[] | null;
  aiModelsStatus: string;
  aiModelsBusy: boolean;
  fetchAiModels: () => Promise<void>;
  aiTestInput: string;
  setAiTestInput: (value: string) => void;
  aiTestOutput: string;
  aiTestStatus: string;
  aiTestBusy: boolean;
  testAi: () => Promise<void>;
  setError?: (message: string) => void;
  providerPresetControls: ProviderPresetControlFactory;
  linuxPlatform: boolean;
  windowsPlatform: boolean;
  macosPlatform: boolean;
  iosPlatform: boolean;
  androidPlatform: boolean;
  /** 宿主让用户选 AI 候选数量（`settingsCapabilities` 的 `showAiCandidateLimit`）。 */
  candidateLimitShown: boolean;
  providerCredentials: ProviderCredentialStatus | undefined;
  storedAiCredential: ProviderCredentialStatus["ai"][number] | undefined;
  aiCredentialInput: string;
  setAiCredentialInput: (value: string) => void;
  providerCredentialBusy: ReturnType<typeof useProviderCredentials>["providerCredentialBusy"];
  providerCredentialMessages: ReturnType<
    typeof useProviderCredentials
  >["providerCredentialMessages"];
  runProviderCredential: ReturnType<typeof useProviderCredentials>["runProviderCredential"];
  credentialTestControl: ReturnType<typeof useProviderCredentials>["credentialTestControl"];
  /** 云候选、翻译、AI、语音、账号、更新检查和下载共用的代理设置；旧宿主不给时整组不显示。 */
  network?: NetworkProxyPreferences;
  onNetworkChange?: (value: NetworkProxyPreferences) => void;
  showNetwork?: boolean;
  /** 最后一组之后的内容：设置页在这里放进入「AI 对话」的入口。 */
  trailing: ReactNode;
}

/** Complete AI settings composition; host state and provider operations stay with SettingsPage. */
export function AiSettingsContent({
  disabled,
  hidden,
  client,
  ai,
  updateAi,
  aiOrigin,
  aiToken,
  updateAiToken,
  aiModels,
  aiModelsStatus,
  aiModelsBusy,
  fetchAiModels,
  aiTestInput,
  setAiTestInput,
  aiTestOutput,
  aiTestStatus,
  aiTestBusy,
  testAi,
  setError,
  providerPresetControls,
  linuxPlatform,
  windowsPlatform,
  macosPlatform,
  iosPlatform,
  androidPlatform,
  candidateLimitShown,
  providerCredentials,
  storedAiCredential,
  aiCredentialInput,
  setAiCredentialInput,
  providerCredentialBusy,
  providerCredentialMessages,
  runProviderCredential,
  credentialTestControl,
  network,
  onNetworkChange,
  showNetwork = false,
  trailing,
}: AiSettingsContentProps) {
  const mounted = useMountedRef();
  const copyAiTestOutput = client.copyText
    ? async () => {
        if (!mounted.current) return;
        setError?.("");
        try {
          await client.copyText!(aiTestOutput);
        } catch {
          if (mounted.current) setError?.("复制 AI 测试结果失败，请重试。");
        }
      }
    : undefined;
  const linuxCredentialTest = credentialTestControl(
    "ai.assistant",
    "测试 AI 辅助配置",
    aiProviderCredentialTestConfig(ai),
    aiCredentialTestDisabled(ai, aiOrigin),
  );
  const serviceCredentialTest =
    windowsPlatform || macosPlatform || iosPlatform
      ? credentialTestControl(
          "ai.assistant",
          "测试 AI 辅助配置",
          aiServiceCredentialTestConfig(ai, aiToken),
          aiServiceCredentialTestDisabled(ai, aiOrigin, aiToken),
        )
      : null;
  return (
    <AiSettingsPageSection
      endpointValid={aiOrigin !== null}
      endpointHint={aiEndpointHint(ai.endpoint)}
      disabled={disabled}
      hidden={hidden}
      enabled={ai.enabled}
      enabledDescription={
        iosPlatform
          ? "为键盘 AI 联想、回复与润色提供共享配置"
          : androidPlatform
            ? "为拼音联想和 Android 选中文字润色提供共享配置"
            : "为拼音联想提供共享配置"
      }
      provider={ai.provider}
      providerOptions={aiProviderOptionsFor(
        Boolean(windowsPlatform || macosPlatform || linuxPlatform),
        ai.provider,
      )}
      model={ai.model}
      presetModels={aiProviderOption(ai.provider)?.models ?? []}
      endpoint={ai.endpoint}
      providerPreset={providerPresetControls(
        "AI ",
        aiProviderOption(ai.provider),
        settings.managerBlock,
      )}
      onEnabledChange={(enabled) => updateAi({ enabled })}
      onProviderChange={(provider) => updateAi(aiProviderUpdate(provider, ai))}
      onModelChange={(model) => updateAi(aiModelUpdate(model, ai))}
      onEndpointChange={(endpoint) => updateAi(aiEndpointUpdate(endpoint, ai))}
      thinking={ai.thinking}
      onThinkingChange={(thinking) => updateAi({ thinking })}
      // 不鉴权的服务（Ollama）没有 Token 可填，整块不显示；Linux 的 provider 直接用页面上的地址和模型。
      credentialSection={
        !aiProviderUsesToken(ai.provider) ? null : linuxPlatform && client.providerCredentials ? (
          <AiCredentialSection
            endpoint={ai.endpoint}
            model={ai.model}
            origin={aiOrigin}
            token={aiCredentialInput}
            stored={storedAiCredential}
            invalid={providerCredentials?.aiInvalid === true}
            busy={providerCredentialBusy === "ai"}
            message={providerCredentialMessages.ai}
            onTokenChange={setAiCredentialInput}
            onSave={() =>
              void runProviderCredential(
                "ai",
                (credentials) => credentials.saveAi(aiCredentialSaveConfig(ai, aiCredentialInput)),
                "凭据已保存，provider 服务下次请求时生效。",
              )
            }
            onClear={() =>
              void runProviderCredential(
                "ai",
                (credentials) => credentials.clearAi(ai.provider),
                "凭据已清除。",
              )
            }
          />
        ) : linuxPlatform ? (
          <AiLinuxProviderSection />
        ) : (
          <AiApiTokenSection
            origin={aiOrigin}
            endpoint={ai.endpoint}
            token={aiToken}
            onTokenChange={updateAiToken}
          />
        )
      }
      // Linux 的测试按钮同样放在服务组末尾，与其他平台一致。
      desktopCredentialTest={serviceCredentialTest ?? (linuxPlatform ? linuxCredentialTest : null)}
      modelCatalog={
        client.aiAssistant
          ? {
              busy: aiModelsBusy,
              origin: aiOrigin ?? "",
              models: aiModels ?? undefined,
              status: aiModelsStatus,
              onFetch: () => void fetchAiModels(),
            }
          : null
      }
      candidateLimitShown={candidateLimitShown}
      candidateLimit={ai.candidate_limit}
      onCandidateLimitChange={(candidate_limit) => updateAi({ candidate_limit })}
      promptId={ai.prompt_id}
      promptCustom1={ai.prompt_custom_1 ?? ""}
      promptCustom2={ai.prompt_custom_2 ?? ""}
      promptCustom3={ai.prompt_custom_3 ?? ""}
      onPromptCustom1Change={(prompt_custom_1) => updateAi({ prompt_custom_1 })}
      onPromptCustom2Change={(prompt_custom_2) => updateAi({ prompt_custom_2 })}
      onPromptCustom3Change={(prompt_custom_3) => updateAi({ prompt_custom_3 })}
      // 「AI 润色测试」用的是润色提示词。桌面上这份配置只给拼音联想用，测出来的结果和用户实际得到的没有关系，凭据和接口通不通已经由「测试 AI 辅助配置」回答，所以只在用它润色的 iOS、Android 和 HarmonyOS（手机与 2-in-1 的键盘都用它润色，见 `KeyboardSession.ets`）上保留。
      networkSection={
        showNetwork && onNetworkChange ? (
          <NetworkProxySection value={network} disabled={disabled} onChange={onNetworkChange} />
        ) : null
      }
      testTools={
        client.aiAssistant && !(macosPlatform || windowsPlatform || linuxPlatform) ? (
          <AiTestToolsSection
            input={aiTestInput}
            busy={aiTestBusy}
            status={aiTestStatus}
            output={aiTestOutput}
            onInputChange={setAiTestInput}
            onTest={() => void testAi()}
            onCopyOutput={copyAiTestOutput}
          />
        ) : null
      }
      trailing={trailing}
    />
  );
}
