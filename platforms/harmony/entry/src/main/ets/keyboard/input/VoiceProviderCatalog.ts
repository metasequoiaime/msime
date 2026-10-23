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
      case "minimax":
        return "https://api.minimax.cn/v1/chat/completions";
      case "kimi":
        return "https://api.moonshot.cn/v1/chat/completions";
      case "zhipu":
        return "https://open.bigmodel.cn/api/paas/v4/chat/completions";
      case "qwen":
        return "https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions";
      case "volcengine":
        return "https://ark.cn-beijing.volces.com/api/v3/chat/completions";
      case "openrouter":
        return "https://openrouter.ai/api/v1/chat/completions";
      case "custom":
        return "";
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
      case "minimax":
        return "MiniMax-M3.1-Flash";
      case "kimi":
        return "kimi-k2-0905-preview";
      case "zhipu":
        return "glm-4-flash";
      case "qwen":
        return "qwen-plus";
      case "volcengine":
        return "doubao-seed-1.6-flash";
      case "openrouter":
        return "openrouter/auto";
      case "custom":
        return "";
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
      case "zhipu":
        return "https://open.bigmodel.cn/api/paas/v4/audio/transcriptions";
      case "minimax":
        return "https://api.minimax.cn/v1/speech_to_text";
      case "custom":
        return "";
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
      case "zhipu":
        return "glm-asr";
      case "minimax":
        return "asr-1.0";
      case "custom":
        return "";
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
      case "zhipu":
        return "multipart";
      case "minimax":
        return "multipart";
      case "custom":
        return "multipart";
      default:
        return "";
    }
  }
}
