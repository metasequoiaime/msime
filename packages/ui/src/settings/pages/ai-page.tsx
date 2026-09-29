import { SecretInput } from "../../core/secret-input";
import { defaultAiAssistant } from "../settings-options";
import type { AiAssistantPreferences } from "../../index";
import { useSettingsForm } from "../settings-form-context";
import { AiCredentialSection } from "../ai-credential-section";

/**
 * `models` are the models the provider is known to accept and `documentation`
 * is where it explains the endpoint and how to obtain an API key, both taken
 * from the Apple `AIProviderPreset`. They are presentation only: the request
 * still sends whatever model preferences hold, and 自定义 has neither.
 */
export const AI_PROVIDER_OPTIONS: readonly {
  id: string;
  title: string;
  endpoint: string;
  model: string;
  models?: readonly string[];
  documentation?: string;
}[] = [
  {
    id: "everyapi",
    title: "EveryAPI",
    endpoint: "https://api.everyapi.ai/v1/chat/completions",
    model: "deepseek-v4-flash",
    models: ["deepseek-v4-flash", "deepseek-v4-pro", "claude-sonnet-5", "glm-5.3-flash"],
    documentation: "https://everyapi.ai/models",
  },
  {
    id: "openai",
    title: "OpenAI",
    endpoint: "https://api.openai.com/v1/chat/completions",
    model: "gpt-4.1-mini",
    models: ["gpt-4.1-mini"],
    documentation: "https://developers.openai.com/api/docs/models/gpt-4.1-mini",
  },
  {
    id: "anthropic",
    title: "Anthropic · Claude",
    endpoint: "https://api.anthropic.com/v1/chat/completions",
    model: "claude-sonnet-4-6",
    models: ["claude-sonnet-4-6", "claude-opus-5"],
    documentation: "https://platform.claude.com/docs/en/cli-sdks-libraries/libraries/openai-sdk",
  },
  {
    id: "gemini",
    title: "Google · Gemini",
    endpoint: "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions",
    model: "gemini-3.8-flash",
    models: ["gemini-3.8-flash", "gemini-2.5-flash"],
    documentation: "https://ai.google.dev/gemini-api/docs/openai",
  },
  {
    id: "deepseek",
    title: "DeepSeek",
    endpoint: "https://api.deepseek.com/chat/completions",
    model: "deepseek-v4-flash",
    models: ["deepseek-v4-flash", "deepseek-v4-pro"],
    documentation: "https://api-docs.deepseek.com/",
  },
  {
    id: "qwen",
    title: "通义千问 · 阿里云百炼",
    endpoint: "https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions",
    model: "qwen-plus",
    models: ["qwen-plus"],
    documentation: "https://help.aliyun.com/zh/model-studio/compatibility-of-openai-with-dashscope",
  },
  {
    id: "kimi",
    title: "Kimi · 月之暗面",
    endpoint: "https://api.moonshot.cn/v1/chat/completions",
    model: "kimi-k2.6",
    models: ["kimi-k2.6", "kimi-k2.5"],
    documentation: "https://platform.kimi.com/docs/api/chat",
  },
  {
    id: "zhipu",
    title: "智谱 · GLM",
    endpoint: "https://open.bigmodel.cn/api/paas/v4/chat/completions",
    model: "glm-4.7",
    models: ["glm-4.7", "glm-4.7-flashx"],
    documentation: "https://docs.bigmodel.cn/cn/guide/models/text/glm-4.7",
  },
  {
    id: "siliconflow",
    title: "硅基流动",
    endpoint: "https://api.siliconflow.cn/v1/chat/completions",
    model: "Qwen/Qwen3.6-27B",
    models: ["Qwen/Qwen3.6-27B"],
    documentation: "https://docs.siliconflow.cn/docs/userguide/capabilities/text-generation",
  },
  {
    id: "groq",
    title: "Groq",
    endpoint: "https://api.groq.com/openai/v1/chat/completions",
    model: "llama-3.3-70b-versatile",
    models: ["llama-3.3-70b-versatile", "llama-3.1-8b-instant"],
    documentation: "https://console.groq.com/docs/quickstart",
  },
  {
    id: "openrouter",
    title: "OpenRouter",
    endpoint: "https://openrouter.ai/api/v1/chat/completions",
    model: "openrouter/auto",
    models: ["openrouter/auto"],
    documentation: "https://openrouter.ai/docs/quickstart",
  },
  { id: "custom", title: "自定义", endpoint: "", model: "" },
];

/** Switch providers like the Apple settings page: preset values follow the
 * provider, while a deliberately edited custom endpoint/model are preserved. */
export function aiProviderUpdate(
  provider: string,
  current: AiAssistantPreferences,
): Partial<AiAssistantPreferences> {
  const next =
    AI_PROVIDER_OPTIONS.find((option) => option.id === provider) ??
    AI_PROVIDER_OPTIONS[AI_PROVIDER_OPTIONS.length - 1];
  const previous = AI_PROVIDER_OPTIONS.find((option) => option.id === current.provider);
  return {
    provider: next.id,
    endpoint:
      !current.endpoint.trim() || current.endpoint === previous?.endpoint
        ? next.endpoint
        : current.endpoint,
    model: !current.model.trim() || current.model === previous?.model ? next.model : current.model,
  };
}

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
    <fieldset disabled={busy} hidden={page !== "ai"} aria-label="AI 辅助">
      <div className="section">
        <label className="section-header">
          <span className="section-title">
            启用 AI 辅助
            <small>
              {iosPlatform
                ? "为键盘 AI 联想、回复与润色提供共享配置"
                : androidPlatform
                  ? "为拼音联想和 Android 选中文字润色提供共享配置"
                  : "为拼音联想提供共享配置"}
            </small>
          </span>
          <input
            aria-label="启用 AI 辅助"
            className="toggle"
            type="checkbox"
            checked={ai.enabled}
            onChange={(event) => updateAi({ enabled: event.target.checked })}
          />
        </label>
      </div>
      <div className="section">
        <label className="section-header">
          <span className="section-title">服务提供商</span>
          <select
            aria-label="AI 服务提供商"
            value={ai.provider}
            onChange={(event) => updateAi(aiProviderUpdate(event.target.value, ai))}
          >
            {AI_PROVIDER_OPTIONS.map((option) => (
              <option key={option.id} value={option.id}>
                {option.title}
              </option>
            ))}
          </select>
        </label>
      </div>
      {providerPresetControls(
        "AI ",
        AI_PROVIDER_OPTIONS.find((option) => option.id === ai.provider),
        ai.model,
        (model) => updateAi({ model }),
      )}
      <div className="section">
        <label className="section-header">
          <span className="section-title">模型</span>
          <input
            aria-label="AI 模型"
            value={ai.model}
            onChange={(event) => updateAi({ model: event.target.value })}
          />
        </label>
      </div>
      <div className="section">
        <label className="section-header">
          <span className="section-title">接口地址</span>
          <input
            aria-label="AI 接口地址"
            type="url"
            value={ai.endpoint}
            onChange={(event) => updateAi({ endpoint: event.target.value })}
          />
        </label>
      </div>
      {linuxPlatform && client.providerCredentials ? (
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
        <div className="section">
          <div className="section-title">
            Linux AI provider<small>AI 请求由用户管理的 provider 服务完成</small>
          </div>
          <p className="input-setting-description">
            凭据不保存在共享设置中；请在用户配置目录的 <code>ai-provider.json</code>{" "}
            中配置，并使其中的 provider、接口地址和模型与上方设置一致。
          </p>
          {credentialTestControl(
            "ai.assistant",
            "测试 AI 辅助配置",
            { provider: ai.provider, endpoint: ai.endpoint, model: ai.model },
            !ai.enabled || !aiOrigin || !ai.model.trim(),
          )}
        </div>
      ) : (
        <div className="section">
          <label className="section-header">
            <span className="section-title">
              API Token
              <small>{aiOrigin ? `只用于 ${aiOrigin}` : "请先填写有效的 HTTPS 接口地址"}</small>
            </span>
            <SecretInput
              label="AI API Token"
              disabled={!aiOrigin}
              value={aiToken}
              onChange={updateAiToken}
            />
          </label>
        </div>
      )}
      {(windowsPlatform || macosPlatform || iosPlatform) &&
        credentialTestControl(
          "ai.assistant",
          "测试 AI 辅助配置",
          {
            provider: ai.provider,
            endpoint: ai.endpoint,
            model: ai.model,
            token: aiToken,
          },
          !ai.enabled || !aiOrigin || !ai.model.trim() || !aiToken.trim(),
        )}
      {client.aiAssistant && (
        <div className="section">
          <div className="section-header">
            <span className="section-title">
              服务模型
              <small>从当前服务的模型目录读取；服务不支持时可继续手动填写模型。</small>
            </span>
            <button
              type="button"
              className="secondary"
              disabled={aiModelsBusy || !aiOrigin}
              onClick={() => void fetchAiModels()}
            >
              {aiModelsBusy ? "获取中…" : "获取模型列表"}
            </button>
          </div>
          {aiModels && aiModels.length > 0 && (
            <label className="section-header">
              <span className="section-title">已获取模型</span>
              <select
                aria-label="已获取的 AI 模型"
                value={aiModels.includes(ai.model) ? ai.model : ""}
                onChange={(event) => {
                  if (event.target.value) updateAi({ model: event.target.value });
                }}
              >
                <option value="">选择模型…</option>
                {aiModels.map((model) => (
                  <option key={model} value={model}>
                    {model}
                  </option>
                ))}
              </select>
            </label>
          )}
          {aiModelsStatus && <p role="status">{aiModelsStatus}</p>}
        </div>
      )}
      <div className="section">
        <label className="section-header">
          <span className="section-title">候选数量</span>
          <input
            aria-label="AI 候选数量"
            type="number"
            min="1"
            max="10"
            value={ai.candidate_limit}
            onChange={(event) =>
              updateAi({
                candidate_limit: Math.max(1, Math.min(10, Number(event.target.value) || 3)),
              })
            }
          />
        </label>
      </div>
      <div className="section">
        <label className="section-header">
          <span className="section-title">
            AI 联想提示词方案
            <small>使用选中的独立槽位；槽位留空时使用兼容提示词</small>
          </span>
          <select
            aria-label="AI 联想提示词方案"
            value={ai.prompt_id === "custom" ? "custom_1" : ai.prompt_id || "custom_1"}
            onChange={(event) => updateAi({ prompt_id: event.target.value })}
          >
            <option value="custom_1">自定义一</option>
            <option value="custom_2">自定义二</option>
            <option value="custom_3">自定义三</option>
          </select>
        </label>
      </div>
      <div className="section">
        <label className="section-title">
          兼容提示词<small>旧版提示词，所选自定义槽位留空时使用</small>
        </label>
        <textarea
          aria-label="AI 润色提示词"
          placeholder="留空时使用内置的联想提示词"
          value={ai.prompt ?? defaultAiAssistant.prompt}
          onChange={(event) => updateAi({ prompt: event.target.value })}
        />
      </div>
      <div className="section">
        <label className="section-title">
          自定义提示词一<small>发送给 AI 联想服务的额外提示词</small>
        </label>
        <textarea
          aria-label="自定义提示词一"
          value={ai.prompt_custom_1}
          onChange={(event) => updateAi({ prompt_custom_1: event.target.value })}
        />
      </div>
      <div className="section">
        <label className="section-title">自定义提示词二</label>
        <textarea
          aria-label="自定义提示词二"
          value={ai.prompt_custom_2}
          onChange={(event) => updateAi({ prompt_custom_2: event.target.value })}
        />
      </div>
      <div className="section">
        <label className="section-title">自定义提示词三</label>
        <textarea
          aria-label="自定义提示词三"
          value={ai.prompt_custom_3}
          onChange={(event) => updateAi({ prompt_custom_3: event.target.value })}
        />
      </div>
      {client.aiAssistant && (
        <div className="section ai-test-tools">
          <div className="section-title">
            AI 润色测试
            <small>仅在点击发送时请求当前配置；测试文字不会写入日志。</small>
          </div>
          <textarea
            aria-label="AI 测试输入"
            placeholder="输入一段待润色文字"
            value={aiTestInput}
            onChange={(event) => setAiTestInput(event.target.value)}
          />
          <button
            type="button"
            className="secondary"
            disabled={aiTestBusy || !aiTestInput.trim()}
            onClick={() => void testAi()}
          >
            {aiTestBusy ? "发送中…" : "发送并润色"}
          </button>
          {aiTestStatus && <p role="status">{aiTestStatus}</p>}
          {aiTestOutput && (
            <div className="ai-test-result">
              <div>{aiTestOutput}</div>
              {client.copyText && (
                <button
                  type="button"
                  className="secondary"
                  onClick={() => void client.copyText!(aiTestOutput)}
                >
                  复制结果
                </button>
              )}
            </div>
          )}
        </div>
      )}
    </fieldset>
  );
}
