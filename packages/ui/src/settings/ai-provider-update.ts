import { AI_PROVIDER_OPTIONS, type AiProviderCatalogEntry } from "./ai-provider-options";
import type { AiAssistantPreferences } from "../index";

export type AiProviderPreset = Pick<AiProviderCatalogEntry, "id" | "endpoint" | "model">;
export type AiProviderPreferences = {
  provider: string;
  endpoint: string;
  model: string;
  endpoints?: Record<string, string>;
  models?: Record<string, string>;
};

/** 槽位只记自定义值：与目录默认相同的值不存，服务商默认值升级后空槽自动跟上（Windows `endpoint_<id>` 槽写空值的对应物）。 */
function storeSlot(
  slots: Record<string, string>,
  provider: string,
  value: string,
  fallback: string | undefined,
): void {
  if (provider && value && value !== fallback) slots[provider] = value;
  else delete slots[provider];
}

export function updateAiProvider<T extends AiProviderPreferences>(
  provider: string,
  current: T,
  options: readonly AiProviderPreset[],
): Partial<T> {
  const next = options.find((option) => option.id === provider) ?? options[options.length - 1];
  const previous = options.find((option) => option.id === current.provider);
  // 先把当前接口地址/模型写回旧提供方的槽位，再按新提供方的槽位回填；没有槽位记录时用目录默认值。
  const endpoints = { ...current.endpoints };
  const models = { ...current.models };
  storeSlot(endpoints, current.provider, current.endpoint.trim(), previous?.endpoint);
  storeSlot(models, current.provider, current.model.trim(), previous?.model);
  return {
    provider: next.id,
    endpoint: endpoints[next.id] || next.endpoint,
    model: models[next.id] || next.model,
    endpoints,
    models,
  } as Partial<T>;
}

/** 接口地址/模型编辑同步进当前提供方的槽位，切换走再切回来时仍是这个值（Windows 写 endpoint 时同时写 endpoint_<provider>）。 */
function slotUpdate<T extends AiProviderPreferences>(
  field: "endpoint" | "model",
  value: string,
  current: T,
  options: readonly AiProviderPreset[],
): Partial<T> {
  const slots = { ...(field === "endpoint" ? current.endpoints : current.models) };
  const fallback = options.find((option) => option.id === current.provider)?.[field];
  storeSlot(slots, current.provider, value.trim(), fallback);
  return { [field]: value, [field === "endpoint" ? "endpoints" : "models"]: slots } as Partial<T>;
}

/** Apply a catalog preset, remembering per-provider endpoint/model slots. */
export function aiProviderUpdate(
  provider: string,
  current: AiAssistantPreferences,
): Partial<AiAssistantPreferences> {
  return updateAiProvider(provider, current, AI_PROVIDER_OPTIONS);
}

export function aiEndpointUpdate(
  endpoint: string,
  current: AiAssistantPreferences,
): Partial<AiAssistantPreferences> {
  return slotUpdate("endpoint", endpoint, current, AI_PROVIDER_OPTIONS);
}

export function aiModelUpdate(
  model: string,
  current: AiAssistantPreferences,
): Partial<AiAssistantPreferences> {
  return slotUpdate("model", model, current, AI_PROVIDER_OPTIONS);
}
