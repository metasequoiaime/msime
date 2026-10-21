/** 语音预设读取共享 Rust 目录的静态导出，换服务时继续清除旧服务默认地址与凭据。 */
import { fillProviderDefaults, known, swapTokenSlot, type TokenMap } from "./provider-helpers";
import { voiceProviderCatalog, voiceProviderDefaults } from "./voice-provider-catalog";
import type { ProviderDefaults } from "../core/provider-preset";

export type { ProviderDefaults } from "../core/provider-preset";

// 公开数组保留历史顺序；新增服务随后追加，显示选择器仍遵循 Rust 目录顺序。
const serviceProviderOrder = [
  "openai",
  "siliconflow",
  "groq",
  "everyapi",
  "mistral",
  "bailian",
  "doubao",
];
export const ASR_SERVICE_PROVIDER_IDS: readonly string[] = voiceProviderCatalog.asr
  .map(({ id }) => id)
  .sort((left, right) => {
    const rank = (id: string) => {
      const index = serviceProviderOrder.indexOf(id);
      return index < 0 ? serviceProviderOrder.length : index;
    };
    return rank(left) - rank(right);
  });

/** Returns whether an ASR provider uses the shared service credential flow. */
export function isAsrServiceProvider(provider: string): boolean {
  return ASR_SERVICE_PROVIDER_IDS.includes(provider);
}

/** Resolve an editable provider setting, falling back to its shipped default. */
export function providerSettingValue(
  value: string | undefined,
  provider: string | undefined,
  defaults: Record<string, ProviderDefaults>,
  field: "endpoint" | "model",
): string {
  return value?.trim() || defaults[provider ?? ""]?.[field] || "";
}

export const ASR_PROVIDER_DEFAULTS: Record<string, ProviderDefaults> = {
  system: { endpoint: "", model: "" },
  // 本地模型路径由 `asr_model_path` 保存，不提供服务地址、凭据或模型名。
  local: { endpoint: "", model: "" },
  ...voiceProviderDefaults(voiceProviderCatalog.asr),
};

export const POLISH_PROVIDER_DEFAULTS = voiceProviderDefaults(voiceProviderCatalog.polish);

/** The voice fields to update when the recognition provider changes. */
export function asrProviderUpdate(
  provider: string,
  current: {
    asr_provider?: string;
    asr_endpoint?: string;
    asr_model?: string;
    asr_token?: string;
    asr_tokens?: TokenMap;
  },
) {
  const defaults = ASR_PROVIDER_DEFAULTS[provider];
  const swapped = swapTokenSlot(
    current.asr_provider ?? "",
    provider,
    current.asr_token ?? "",
    current.asr_tokens,
  );
  const update: {
    asr_provider: string;
    asr_endpoint?: string;
    asr_model?: string;
    asr_token: string;
    asr_tokens: TokenMap;
  } = {
    asr_provider: provider,
    asr_token: swapped.token,
    asr_tokens: swapped.tokens,
  };
  // Neither of these sends anything to a service, so neither keeps a credential slot.
  if (provider === "system" || provider === "local") {
    update.asr_token = "";
    delete update.asr_tokens[provider];
  }
  if (!defaults) return update;
  const fields = fillProviderDefaults(
    current.asr_endpoint,
    current.asr_model,
    defaults.endpoint,
    defaults.model,
    known(ASR_PROVIDER_DEFAULTS, "endpoint"),
    known(ASR_PROVIDER_DEFAULTS, "model"),
  );
  if (fields.endpoint !== undefined) update.asr_endpoint = fields.endpoint;
  if (fields.model !== undefined) update.asr_model = fields.model;
  return update;
}

/** The voice fields to update when the polish provider changes. */
export function polishProviderUpdate(
  provider: string,
  current: {
    polish_provider?: string;
    polish_endpoint?: string;
    polish_model?: string;
    polish_token?: string;
    polish_tokens?: TokenMap;
  },
) {
  const defaults = POLISH_PROVIDER_DEFAULTS[provider];
  const swapped = swapTokenSlot(
    current.polish_provider ?? "",
    provider,
    current.polish_token ?? "",
    current.polish_tokens,
  );
  const update: {
    polish_provider: string;
    polish_endpoint?: string;
    polish_model?: string;
    polish_token: string;
    polish_tokens: TokenMap;
  } = {
    polish_provider: provider,
    polish_token: swapped.token,
    polish_tokens: swapped.tokens,
  };
  if (!defaults) return update;
  const fields = fillProviderDefaults(
    current.polish_endpoint,
    current.polish_model,
    defaults.endpoint,
    defaults.model,
    known(POLISH_PROVIDER_DEFAULTS, "endpoint"),
    known(POLISH_PROVIDER_DEFAULTS, "model"),
  );
  if (fields.endpoint !== undefined) update.polish_endpoint = fields.endpoint;
  if (fields.model !== undefined) update.polish_model = fields.model;
  return update;
}

/** 同一识别模型的整句流式与双向增量接口；选择值仍保存到 `asr_endpoint`。 */
export type DoubaoStreamEndpoint = { id: string; endpoint: string; title: string };

export const DOUBAO_STREAM_ENDPOINTS: readonly DoubaoStreamEndpoint[] =
  voiceProviderCatalog.doubao_stream_endpoints.map(({ id, endpoint, title }) => ({
    id,
    endpoint,
    title,
  }));

export function findDoubaoStreamEndpoint(id: string): DoubaoStreamEndpoint | undefined {
  return DOUBAO_STREAM_ENDPOINTS.find((option) => option.id === id);
}

export function doubaoStreamEndpointId(endpoint: string): string {
  return DOUBAO_STREAM_ENDPOINTS.find((option) => option.endpoint === endpoint)?.id ?? "custom";
}
