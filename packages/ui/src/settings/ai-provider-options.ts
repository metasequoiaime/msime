import type { ProviderDefaults } from "../core/provider-preset";
import {
  deepseekChatEndpoint,
  deepseekDocumentationUrl,
  dashScopeChatEndpoint,
  everyApiModelsUrl,
  groqChatEndpoint,
  groqDocumentationUrl,
  openAiChatEndpoint,
  openAiModelsDocumentationUrl,
  siliconFlowChatEndpoint,
  siliconFlowTextDocumentationUrl,
} from "../core/provider-resources";

export type AiProviderOption = { id: string; title: string };

export type AiProviderCatalogEntry = AiProviderOption & ProviderDefaults;

/** Ollama 在本机或局域网运行、不鉴权：选它时页面不显示 API Token，请求也不带（client-providers 的 `ai::provider_uses_token`）。 */
export function aiProviderUsesToken(provider: string): boolean {
  return provider !== "ollama";
}

/** 只在桌面（Windows、macOS、Linux）提供的服务商。手机上的键盘各自判断 Token，还没有接入不鉴权的服务。 */
const DESKTOP_ONLY_AI_PROVIDERS: ReadonlySet<string> = new Set(["ollama"]);

/** 已知模型和接入说明仅用于展示；实际请求仍使用用户保存的偏好。 */
export const AI_PROVIDER_OPTIONS: readonly AiProviderCatalogEntry[] = [
  {
    id: "everyapi",
    title: "EveryAPI",
    endpoint: "https://api.everyapi.ai/v1/chat/completions",
    model: "deepseek-v4-flash",
    models: ["deepseek-v4-flash", "deepseek-v4-pro", "claude-sonnet-5", "glm-5.3-flash"],
    documentation: everyApiModelsUrl,
  },
  {
    id: "openai",
    title: "OpenAI",
    endpoint: openAiChatEndpoint,
    model: "gpt-4.1-mini",
    models: ["gpt-4.1-mini"],
    documentation: openAiModelsDocumentationUrl,
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
    endpoint: deepseekChatEndpoint,
    model: "deepseek-v4-flash",
    models: ["deepseek-v4-flash", "deepseek-v4-pro"],
    documentation: deepseekDocumentationUrl,
  },
  {
    id: "qwen",
    title: "通义千问 · 阿里云百炼",
    endpoint: dashScopeChatEndpoint,
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
    endpoint: siliconFlowChatEndpoint,
    model: "Qwen/Qwen3.6-27B",
    models: ["Qwen/Qwen3.6-27B"],
    documentation: siliconFlowTextDocumentationUrl,
  },
  {
    id: "groq",
    title: "Groq",
    endpoint: groqChatEndpoint,
    model: "llama-3.3-70b-versatile",
    models: ["llama-3.3-70b-versatile", "llama-3.1-8b-instant"],
    documentation: groqDocumentationUrl,
  },
  {
    id: "openrouter",
    title: "OpenRouter",
    endpoint: "https://openrouter.ai/api/v1/chat/completions",
    model: "openrouter/auto",
    models: ["openrouter/auto"],
    documentation: "https://openrouter.ai/docs/quickstart",
  },
  {
    // 模型是用户自己拉到本机的，没有通用的默认值，靠「获取模型」读本机的模型列表。
    id: "ollama",
    title: "Ollama",
    endpoint: "http://localhost:11434/v1/chat/completions",
    model: "",
    documentation: "https://docs.ollama.com/api/openai-compatibility",
  },
  { id: "custom", title: "自定义", endpoint: "", model: "" },
];

export function aiProviderOption(provider: string) {
  return AI_PROVIDER_OPTIONS.find((option) => option.id === provider);
}

/** 下拉里列出的服务商：手机上去掉只在桌面提供的；已经选着的那一项总是留着，免得下拉显示不出当前值。 */
export function aiProviderOptionsFor(
  desktop: boolean,
  selected: string,
): readonly AiProviderCatalogEntry[] {
  if (desktop) return AI_PROVIDER_OPTIONS;
  return AI_PROVIDER_OPTIONS.filter(
    (option) => !DESKTOP_ONLY_AI_PROVIDERS.has(option.id) || option.id === selected,
  );
}
