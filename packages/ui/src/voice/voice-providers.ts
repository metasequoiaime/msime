/**
 * Per-provider endpoint and model defaults for voice input.
 *
 * These matter for more than convenience. The shipped `asr_endpoint` default is
 * the Doubao websocket URL, and the Windows host treats a `wss://` endpoint as
 * "this is Doubao". So a user who picked OpenAI but kept the stored endpoint
 * had their OpenAI token sent to ByteDance. Rewriting the endpoint when the
 * provider changes is what stops that at the source.
 */
/**
 * `models` is what the service is known to accept, so a user can pick one before
 * holding any credential; `documentation` is where that provider explains the
 * endpoint and how to obtain an API key. Both are presentation only -- the model
 * a request actually sends is still whatever is stored in preferences.
 */
import { fillIfDefault, known, swapTokenSlot, type TokenMap } from "./provider-helpers";

export type ProviderDefaults = {
  endpoint: string;
  model: string;
  models?: readonly string[];
  documentation?: string;
};

export const ASR_PROVIDER_DEFAULTS: Record<string, ProviderDefaults> = {
  system: { endpoint: "", model: "" },
  // On-device recognition. The model is a downloaded model directory or a Whisper file the user points at, not a name a service resolves, so it lives in `asr_model_path` and there is no endpoint, token or model list to offer here.
  local: {
    endpoint: "",
    model: "",
    documentation: "https://huggingface.co/ggerganov/whisper.cpp/tree/main",
  },
  doubao: {
    endpoint: "wss://openspeech.bytedance.com/api/v3/sauc/bigmodel_async",
    model: "",
    documentation: "https://www.volcengine.com/docs/6561/1354869",
  },
  openai: {
    endpoint: "https://api.openai.com/v1/audio/transcriptions",
    model: "whisper-1",
    models: ["gpt-4o-mini-transcribe", "gpt-4o-transcribe", "whisper-1"],
    documentation: "https://developers.openai.com/api/docs/guides/speech-to-text",
  },
  siliconflow: {
    endpoint: "https://api.siliconflow.cn/v1/audio/transcriptions",
    model: "FunAudioLLM/SenseVoiceSmall",
    models: ["FunAudioLLM/SenseVoiceSmall"],
    documentation: "https://siliconflow.readme.io/reference/createaudiotranscriptions",
  },
  groq: {
    endpoint: "https://api.groq.com/openai/v1/audio/transcriptions",
    model: "whisper-large-v3-turbo",
    models: ["whisper-large-v3-turbo", "whisper-large-v3"],
    documentation: "https://console.groq.com/docs/speech-to-text",
  },
  everyapi: {
    endpoint: "https://api.everyapi.ai/v1/audio/transcriptions",
    model: "openai/whisper-large-v3-turbo",
    models: ["openai/whisper-large-v3-turbo", "volc.seedasr.sauc.duration"],
    documentation: "https://everyapi.ai/models",
  },
  mistral: {
    endpoint: "https://api.mistral.ai/v1/audio/transcriptions",
    model: "voxtral-mini-latest",
    models: ["voxtral-mini-latest"],
    documentation: "https://docs.mistral.ai/studio/audio/speech_to_text/offline_transcription",
  },
};

export const POLISH_PROVIDER_DEFAULTS: Record<string, ProviderDefaults> = {
  siliconflow: {
    endpoint: "https://api.siliconflow.cn/v1/chat/completions",
    model: "Qwen/Qwen3-8B",
    models: ["Qwen/Qwen3-8B", "Qwen/Qwen3.6-27B"],
    documentation: "https://docs.siliconflow.cn/docs/userguide/capabilities/text-generation",
  },
  openai: {
    endpoint: "https://api.openai.com/v1/chat/completions",
    model: "gpt-4o-mini",
    models: ["gpt-4o-mini", "gpt-4.1-mini"],
    documentation: "https://developers.openai.com/api/docs/models/gpt-4.1-mini",
  },
  deepseek: {
    endpoint: "https://api.deepseek.com/chat/completions",
    model: "deepseek-v4-flash",
    models: ["deepseek-v4-flash", "deepseek-v4-pro"],
    documentation: "https://api-docs.deepseek.com/",
  },
  groq: {
    endpoint: "https://api.groq.com/openai/v1/chat/completions",
    model: "llama-3.3-70b-versatile",
    models: ["llama-3.3-70b-versatile", "llama-3.1-8b-instant"],
    documentation: "https://console.groq.com/docs/quickstart",
  },
};

/** An older SiliconFlow default that should still be treated as untouched. */
const LEGACY_ASR_MODELS = ["TeleAI/TeleSpeechASR"];

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
  const endpoint = fillIfDefault(
    current.asr_endpoint,
    defaults.endpoint,
    known(ASR_PROVIDER_DEFAULTS, "endpoint"),
  );
  if (endpoint !== undefined) update.asr_endpoint = endpoint;
  const model = fillIfDefault(current.asr_model, defaults.model, [
    ...known(ASR_PROVIDER_DEFAULTS, "model"),
    ...LEGACY_ASR_MODELS,
  ]);
  if (model !== undefined) update.asr_model = model;
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
  const endpoint = fillIfDefault(
    current.polish_endpoint,
    defaults.endpoint,
    known(POLISH_PROVIDER_DEFAULTS, "endpoint"),
  );
  if (endpoint !== undefined) update.polish_endpoint = endpoint;
  const model = fillIfDefault(
    current.polish_model,
    defaults.model,
    known(POLISH_PROVIDER_DEFAULTS, "model"),
  );
  if (model !== undefined) update.polish_model = model;
  return update;
}

/** The two Doubao streaming interfaces, which differ in what they return.
 *
 * Both are recognition endpoints for the same model, so the choice is only
 * ever spelled as a URL in `asr_endpoint`. Nobody types one of these from
 * memory, and the trade-off between them is not visible in the address:
 * whole-sentence streaming uploads while you speak and answers once you stop,
 * which is what the service documents as the more accurate option and
 * recommends for input methods; bidirectional streaming answers incrementally,
 * so an inline preedit updates far more often. */
export type DoubaoStreamEndpoint = { id: string; endpoint: string; title: string };

export const DOUBAO_STREAM_ENDPOINTS: readonly DoubaoStreamEndpoint[] = [
  {
    id: "nostream",
    endpoint: "wss://openspeech.bytedance.com/api/v3/sauc/bigmodel_nostream",
    title: "整句流式（准确率更高）",
  },
  {
    id: "async",
    endpoint: "wss://openspeech.bytedance.com/api/v3/sauc/bigmodel_async",
    title: "双向流式（增量结果）",
  },
];

export function findDoubaoStreamEndpoint(id: string): DoubaoStreamEndpoint | undefined {
  return DOUBAO_STREAM_ENDPOINTS.find((option) => option.id === id);
}

export function doubaoStreamEndpointId(endpoint: string): string {
  return DOUBAO_STREAM_ENDPOINTS.find((option) => option.endpoint === endpoint)?.id ?? "custom";
}
