import type { AiAssistantPreferences, ProviderCredentialStatus, SettingsClient } from "../index";
import type { useProviderCredentials } from "./use-provider-credentials";
import { AI_PROVIDER_OPTIONS } from "./ai-provider-options";
import { aiProviderUpdate } from "./ai-provider-update";
import { defaultAiAssistant } from "./ai-assistant-defaults";
import { AiSettingsPageSection } from "./ai-settings-page-section";
import { AiCredentialSection } from "./ai-credential-section";
import { AiLinuxProviderSection } from "./ai-linux-provider-section";
import { AiApiTokenSection } from "./ai-api-token-section";
import { AiTestToolsSection } from "./ai-test-tools-section";
import { McpConnectSection } from "./mcp-connect";
import type { ProviderPresetControlFactory } from "./provider-preset-control";

export interface AiSettingsPanelProps {
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
  providerPresetControls: ProviderPresetControlFactory;
  linuxPlatform: boolean;
  windowsPlatform: boolean;
  macosPlatform: boolean;
  iosPlatform: boolean;
  androidPlatform: boolean;
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
}

/** Complete AI settings composition; host state and provider operations stay with SettingsPage. */
export function AiSettingsPanel({
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
  providerPresetControls,
  linuxPlatform,
  windowsPlatform,
  macosPlatform,
  iosPlatform,
  androidPlatform,
  providerCredentials,
  storedAiCredential,
  aiCredentialInput,
  setAiCredentialInput,
  providerCredentialBusy,
  providerCredentialMessages,
  runProviderCredential,
  credentialTestControl,
}: AiSettingsPanelProps) {
  return (
    <AiSettingsPageSection
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
      providerOptions={AI_PROVIDER_OPTIONS}
      model={ai.model}
      endpoint={ai.endpoint}
      providerPreset={providerPresetControls(
        "AI ",
        AI_PROVIDER_OPTIONS.find((option) => option.id === ai.provider),
        ai.model,
        (model) => updateAi({ model }),
      )}
      onEnabledChange={(enabled) => updateAi({ enabled })}
      onProviderChange={(provider) => updateAi(aiProviderUpdate(provider, ai))}
      onModelChange={(model) => updateAi({ model })}
      onEndpointChange={(endpoint) => updateAi({ endpoint })}
      credentialSection={
        linuxPlatform && client.providerCredentials ? (
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
                (credentials) =>
                  credentials.saveAi({
                    provider: ai.provider,
                    endpoint: ai.endpoint,
                    model: ai.model,
                    ...(aiCredentialInput.trim() ? { token: aiCredentialInput } : {}),
                  }),
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
          >
            {credentialTestControl(
              "ai.assistant",
              "测试 AI 辅助配置",
              { provider: ai.provider, endpoint: ai.endpoint, model: ai.model },
              !ai.enabled || !aiOrigin || !ai.model.trim(),
            )}
          </AiCredentialSection>
        ) : linuxPlatform ? (
          <AiLinuxProviderSection>
            {credentialTestControl(
              "ai.assistant",
              "测试 AI 辅助配置",
              { provider: ai.provider, endpoint: ai.endpoint, model: ai.model },
              !ai.enabled || !aiOrigin || !ai.model.trim(),
            )}
          </AiLinuxProviderSection>
        ) : (
          <AiApiTokenSection origin={aiOrigin} token={aiToken} onTokenChange={updateAiToken} />
        )
      }
      desktopCredentialTest={
        windowsPlatform || macosPlatform || iosPlatform
          ? credentialTestControl(
              "ai.assistant",
              "测试 AI 辅助配置",
              {
                provider: ai.provider,
                endpoint: ai.endpoint,
                model: ai.model,
                token: aiToken,
              },
              !ai.enabled || !aiOrigin || !ai.model.trim() || !aiToken.trim(),
            )
          : null
      }
      modelCatalog={
        client.aiAssistant
          ? {
              busy: aiModelsBusy,
              origin: aiOrigin ?? "",
              models: aiModels ?? undefined,
              status: aiModelsStatus,
              onFetch: () => void fetchAiModels(),
              onSelect: (model) => updateAi({ model }),
            }
          : null
      }
      candidateLimit={ai.candidate_limit}
      onCandidateLimitChange={(candidate_limit) => updateAi({ candidate_limit })}
      promptId={ai.prompt_id}
      prompt={ai.prompt}
      promptCustom1={ai.prompt_custom_1 ?? ""}
      promptCustom2={ai.prompt_custom_2 ?? ""}
      promptCustom3={ai.prompt_custom_3 ?? ""}
      fallbackPrompt={defaultAiAssistant.prompt ?? ""}
      onPromptIdChange={(prompt_id) => updateAi({ prompt_id })}
      onPromptChange={(prompt) => updateAi({ prompt })}
      onPromptCustom1Change={(prompt_custom_1) => updateAi({ prompt_custom_1 })}
      onPromptCustom2Change={(prompt_custom_2) => updateAi({ prompt_custom_2 })}
      onPromptCustom3Change={(prompt_custom_3) => updateAi({ prompt_custom_3 })}
      testTools={
        client.aiAssistant ? (
          <AiTestToolsSection
            input={aiTestInput}
            busy={aiTestBusy}
            status={aiTestStatus}
            output={aiTestOutput}
            onInputChange={setAiTestInput}
            onTest={() => void testAi()}
            onCopyOutput={client.copyText ? () => void client.copyText!(aiTestOutput) : undefined}
          />
        ) : null
      }
      mcpConnect={
        client.mcpServerStatus ? (
          <McpConnectSection
            status={client.mcpServerStatus}
            install={client.installMcpClient}
            copyText={client.copyText}
          />
        ) : null
      }
    />
  );
}
