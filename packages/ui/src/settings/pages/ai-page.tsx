import { defaultAiAssistant } from "../ai-assistant-defaults";
import { AiApiTokenSection } from "../ai-api-token-section";
import {
  aiCredentialTestDisabled,
  aiProviderCredentialTestConfig,
  aiServiceCredentialTestConfig,
  aiServiceCredentialTestDisabled,
} from "../ai-credential-test-config";
import { aiCredentialSaveConfig } from "../ai-credential-save-config";
import { AiCredentialSection } from "../ai-credential-section";
import { AiLinuxProviderSection } from "../ai-linux-provider-section";
import { AiSettingsPageSection } from "../ai-settings-page-section";
import { AiTestToolsSection } from "../ai-test-tools-section";
import { AI_PROVIDER_OPTIONS, aiProviderOption } from "../ai-provider-options";
import { aiProviderUpdate } from "../ai-provider-update";
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
    <AiSettingsPageSection
      disabled={busy}
      hidden={page !== "ai"}
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
        aiProviderOption(ai.provider),
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
                  credentials.saveAi(aiCredentialSaveConfig(ai, aiCredentialInput)),
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
              aiProviderCredentialTestConfig(ai),
              aiCredentialTestDisabled(ai, aiOrigin),
            )}
          </AiCredentialSection>
        ) : linuxPlatform ? (
          <AiLinuxProviderSection>
            {credentialTestControl(
              "ai.assistant",
              "测试 AI 辅助配置",
              aiProviderCredentialTestConfig(ai),
              aiCredentialTestDisabled(ai, aiOrigin),
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
              aiServiceCredentialTestConfig(ai, aiToken),
              aiServiceCredentialTestDisabled(ai, aiOrigin, aiToken),
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
      mcpConnect={null}
    />
  );
}
