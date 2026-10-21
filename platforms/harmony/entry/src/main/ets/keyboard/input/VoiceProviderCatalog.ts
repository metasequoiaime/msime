// 由共享 Rust 语音服务目录导出；通过 export_voice_provider_catalog 更新。
export class VoiceProviderCatalog {
  static polishEndpoint(id: string): string {
    switch (id) {
      case "siliconflow":
        return "https://api.siliconflow.cn/v1/chat/completions";
      case "openai":
        return "https://api.openai.com/v1/chat/completions";
      case "deepseek":
        return "https://api.deepseek.com/chat/completions";
      case "groq":
        return "https://api.groq.com/openai/v1/chat/completions";
      default:
        return "";
    }
  }

  static polishModel(id: string): string {
    switch (id) {
      case "siliconflow":
        return "Qwen/Qwen3-8B";
      case "openai":
        return "gpt-4o-mini";
      case "deepseek":
        return "deepseek-v4-flash";
      case "groq":
        return "llama-3.3-70b-versatile";
      default:
        return "";
    }
  }

  static httpAsrEndpoint(id: string): string {
    switch (id) {
      case "siliconflow":
        return "https://api.siliconflow.cn/v1/audio/transcriptions";
      case "openai":
        return "https://api.openai.com/v1/audio/transcriptions";
      case "groq":
        return "https://api.groq.com/openai/v1/audio/transcriptions";
      case "everyapi":
        return "https://api.everyapi.ai/v1/audio/transcriptions";
      case "mistral":
        return "https://api.mistral.ai/v1/audio/transcriptions";
      case "bailian":
        return "https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions";
      default:
        return "";
    }
  }

  static httpAsrModel(id: string): string {
    switch (id) {
      case "siliconflow":
        return "FunAudioLLM/SenseVoiceSmall";
      case "openai":
        return "whisper-1";
      case "groq":
        return "whisper-large-v3-turbo";
      case "everyapi":
        return "volc.seedasr.sauc.duration";
      case "mistral":
        return "voxtral-mini-latest";
      case "bailian":
        return "qwen3-asr-flash";
      default:
        return "";
    }
  }

  static httpAsrRequestFormat(id: string): string {
    switch (id) {
      case "siliconflow":
        return "multipart";
      case "openai":
        return "multipart";
      case "groq":
        return "multipart";
      case "everyapi":
        return "multipart";
      case "mistral":
        return "multipart";
      case "bailian":
        return "chat_audio";
      default:
        return "";
    }
  }
}
