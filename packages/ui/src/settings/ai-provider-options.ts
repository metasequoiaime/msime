import type { ProviderDefaults } from "../core/provider-preset";
import catalog from "./ai-provider-catalog.json";

export type AiProviderOption = { id: string; title: string };

export type AiProviderCatalogEntry = AiProviderOption & ProviderDefaults;

/** Ollama 在本机或局域网运行、不鉴权：选它时页面不显示 API Token，请求也不带（client-providers 的 `ai::provider_uses_token`）。 */
export function aiProviderUsesToken(provider: string): boolean {
  return provider !== "ollama";
}

/** 只在桌面（Windows、macOS、Linux）提供的服务商。手机上的键盘各自判断 Token，还没有接入不鉴权的服务。 */
const DESKTOP_ONLY_AI_PROVIDERS: ReadonlySet<string> = new Set(["ollama"]);

/** 已知模型和接入说明仅用于展示；实际请求仍使用用户保存的偏好。 */
export const AI_PROVIDER_OPTIONS: readonly AiProviderCatalogEntry[] = catalog;

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
