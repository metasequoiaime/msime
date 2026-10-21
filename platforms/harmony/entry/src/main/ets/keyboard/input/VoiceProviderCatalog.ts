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
}
