import { utf8Length } from "../Utf8";
import { TextPolicy } from "../TextPolicy";
import { VoiceInputConfiguration, VoiceAsrTokens } from "./VoiceInputConfiguration";

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
   * 整句上传的请求格式，取值与 client-core 的 `asr_request_format` 相同；不是整句上传的 provider 为空串。本宿主的语音设置不经过共享层的 `MobileVoiceProviderConfiguration`，所以在这里留一份同样的对应，识别器和凭据测试只按格式挑请求构造。
   */
  static requestFormat(provider: string): string {
    if (["openai", "siliconflow", "groq", "everyapi", "mistral"].includes(provider)) {
      return HTTP_ASR_MULTIPART;
    }
    return provider === "bailian" ? HTTP_ASR_CHAT_AUDIO : "";
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
    if (provider === "openai") {
      return { endpoint: "https://api.openai.com/v1/audio/transcriptions", model: "whisper-1" };
    }
    if (provider === "siliconflow") {
      return {
        endpoint: "https://api.siliconflow.cn/v1/audio/transcriptions",
        model: "FunAudioLLM/SenseVoiceSmall",
      };
    }
    if (provider === "groq") {
      return {
        endpoint: "https://api.groq.com/openai/v1/audio/transcriptions",
        model: "whisper-large-v3-turbo",
      };
    }
    if (provider === "everyapi") {
      return {
        endpoint: "https://api.everyapi.ai/v1/audio/transcriptions",
        model: "openai/whisper-large-v3-turbo",
      };
    }
    if (provider === "mistral") {
      return {
        endpoint: "https://api.mistral.ai/v1/audio/transcriptions",
        model: "voxtral-mini-latest",
      };
    }
    if (provider === "bailian") {
      return {
        endpoint: "https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions",
        model: "qwen3-asr-flash",
      };
    }
    return null;
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
