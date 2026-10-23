import { utf8Length } from "../Utf8";
import { TextPolicy } from "../TextPolicy";
import { VoiceInputConfiguration, VoiceAsrTokens } from "./VoiceInputConfiguration";
import { VoiceProviderCatalog } from "./VoiceProviderCatalog";

export interface HttpAsrDefaults {
  endpoint: string;
  model: string;
}

/** OpenAI 兼容的 `/audio/transcriptions` multipart 上传。 */
export const HTTP_ASR_MULTIPART: string = "multipart";
/** Chat Completions 带 `input_audio` 的 JSON 请求（阿里云百炼），回答在 `choices[0].message.content`。 */
export const HTTP_ASR_CHAT_AUDIO: string = "chat_audio";

/** 解析并校验所有整句识别预设。 */
export class HttpAsrConfigurationPolicy {
  static supported(provider: string): boolean {
    return HttpAsrConfigurationPolicy.requestFormat(provider).length > 0;
  }

  /**
   * 整句上传格式由共享 Rust 目录导出；流式、本地与未知服务为空串。识别器和凭据测试只按格式挑请求构造。
   */
  static requestFormat(provider: string): string {
    return VoiceProviderCatalog.httpAsrRequestFormat(provider);
  }

  /** chat_audio 的请求体：唯一一条 user 消息，内容是录音的 Base64 数据 URL。不带语种，交给模型自动识别。 */
  static chatAudioBody(model: string, wavBase64: string): string {
    return JSON.stringify({
      model: model,
      stream: false,
      messages: [
        {
          role: "user",
          content: [
            { type: "input_audio", input_audio: { data: `data:audio/wav;base64,${wavBase64}` } },
          ],
        },
      ],
    });
  }

  static defaults(provider: string): HttpAsrDefaults | null {
    if (VoiceProviderCatalog.httpAsrRequestFormat(provider).length === 0) return null;
    return {
      endpoint: VoiceProviderCatalog.httpAsrEndpoint(provider),
      model: VoiceProviderCatalog.httpAsrModel(provider),
    };
  }

  static endpoint(config: VoiceInputConfiguration): string {
    return config.asr_endpoint.trim() || (this.defaults(config.asr_provider)?.endpoint ?? "");
  }

  static model(config: VoiceInputConfiguration): string {
    return config.asr_model.trim() || (this.defaults(config.asr_provider)?.model ?? "");
  }

  static token(config: VoiceInputConfiguration): string {
    const flat: string = config.asr_token.trim();
    return flat.length > 0
      ? flat
      : this.providerToken(config.asr_tokens ?? {}, config.asr_provider);
  }

  static transcriptionLanguage(provider: string, value: string): string {
    if (provider === "siliconflow") return "";
    const language: string = value.trim().toLowerCase();
    if (language.length === 0 || language === "auto") return "";
    const hyphen: number = language.indexOf("-");
    const underscore: number = language.indexOf("_");
    let separator: number = hyphen;
    if (separator < 0 || (underscore >= 0 && underscore < separator)) separator = underscore;
    return separator < 0 ? language : language.slice(0, separator);
  }

  static valid(config: VoiceInputConfiguration): boolean {
    const endpoint: string = this.endpoint(config);
    const model: string = this.model(config);
    const token: string = this.token(config);
    return (
      this.supported(config.asr_provider) &&
      TextPolicy.validAuthority(endpoint, ["https://"]) &&
      model.length > 0 &&
      utf8Length(model) <= 512 &&
      !TextPolicy.hasControl(model) &&
      token.length > 0 &&
      utf8Length(token) <= 16384 &&
      !TextPolicy.hasControl(token)
    );
  }

  private static providerToken(tokens: VoiceAsrTokens, provider: string): string {
    if (provider === "openai") return (tokens.openai ?? "").trim();
    if (provider === "siliconflow") return (tokens.siliconflow ?? "").trim();
    if (provider === "groq") return (tokens.groq ?? "").trim();
    if (provider === "everyapi") return (tokens.everyapi ?? "").trim();
    if (provider === "mistral") return (tokens.mistral ?? "").trim();
    if (provider === "bailian") return (tokens.bailian ?? "").trim();
    return "";
  }
}
