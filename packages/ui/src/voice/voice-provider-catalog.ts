import catalog from "./voice-provider-catalog.json";
import type { ProviderDefaults } from "../core/provider-preset";

type CatalogProvider = (typeof catalog.asr)[number];

/** 空模型列表保持旧默认表省略 `models` 的形状，实际请求仍使用已保存的配置。 */
export function voiceProviderDefaults(
  providers: readonly CatalogProvider[],
): Record<string, ProviderDefaults> {
  return Object.fromEntries(
    providers.map(({ id, endpoint, model, models, documentation }) => [
      id,
      { endpoint, model, documentation, ...(models.length ? { models } : {}) },
    ]),
  );
}

export const voiceProviderCatalog = catalog;
