import { utf8Length } from "../Utf8";
import { TextPolicy } from "../TextPolicy";
import { VoiceInputConfiguration, VoiceAsrTokens } from "./VoiceInputConfiguration";

export interface HttpAsrDefaults {
  endpoint: string;
  model: string;
}

/** Resolves and validates every OpenAI-compatible batch transcription preset. */
export class HttpAsrConfigurationPolicy {
  static supported(provider: string): boolean {
    return ["openai", "siliconflow", "groq", "everyapi", "mistral"].includes(provider);
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
    return "";
  }

}
