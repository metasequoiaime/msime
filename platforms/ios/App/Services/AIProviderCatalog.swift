// 由共享 Rust AI 服务目录导出；通过 export_ai_provider_catalog 更新。
enum AIProviderCatalog {
  struct Provider: Sendable {
    let title: String
    let endpoint: String
    let models: [String]
    let documentation: String
  }

  static func provider(id: String) -> Provider? { providers[id] }

  private static let providers: [String: Provider] = [
    "everyapi": Provider(title: "EveryAPI", endpoint: "https://api.everyapi.ai/v1/chat/completions", models: ["deepseek-v4-flash", "deepseek-v4-pro", "claude-sonnet-5", "glm-5.3-flash"], documentation: "https://everyapi.ai/models"),
    "openai": Provider(title: "OpenAI", endpoint: "https://api.openai.com/v1/chat/completions", models: ["gpt-4.1-mini"], documentation: "https://developers.openai.com/api/docs/models/gpt-4.1-mini"),
    "anthropic": Provider(title: "Anthropic · Claude", endpoint: "https://api.anthropic.com/v1/chat/completions", models: ["claude-sonnet-4-6", "claude-opus-5"], documentation: "https://platform.claude.com/docs/en/cli-sdks-libraries/libraries/openai-sdk"),
    "gemini": Provider(title: "Google · Gemini", endpoint: "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions", models: ["gemini-3.8-flash", "gemini-2.5-flash"], documentation: "https://ai.google.dev/gemini-api/docs/openai"),
    "deepseek": Provider(title: "DeepSeek", endpoint: "https://api.deepseek.com/chat/completions", models: ["deepseek-v4-flash", "deepseek-v4-pro"], documentation: "https://api-docs.deepseek.com/"),
    "qwen": Provider(title: "通义千问 · 阿里云百炼", endpoint: "https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions", models: ["qwen-plus"], documentation: "https://help.aliyun.com/zh/model-studio/compatibility-of-openai-with-dashscope"),
    "kimi": Provider(title: "Kimi · 月之暗面", endpoint: "https://api.moonshot.cn/v1/chat/completions", models: ["kimi-k2.6", "kimi-k2.5"], documentation: "https://platform.kimi.com/docs/api/chat"),
    "zhipu": Provider(title: "智谱 · GLM", endpoint: "https://open.bigmodel.cn/api/paas/v4/chat/completions", models: ["glm-4.7", "glm-4.7-flashx"], documentation: "https://docs.bigmodel.cn/cn/guide/models/text/glm-4.7"),
    "minimax": Provider(title: "MiniMax", endpoint: "https://api.minimax.cn/v1/chat/completions", models: ["MiniMax-M3.1-Flash", "MiniMax-M3"], documentation: "https://platform.minimax.cn/docs/api-reference/text-chat-openai"),
    "volcengine": Provider(title: "火山方舟 · 豆包", endpoint: "https://ark.cn-beijing.volces.com/api/v3/chat/completions", models: ["doubao-seed-1.6-flash", "doubao-seed-1.6"], documentation: "https://ark.volcengine.com/region:cn-beijing/docs/chat-api"),
    "siliconflow": Provider(title: "硅基流动", endpoint: "https://api.siliconflow.cn/v1/chat/completions", models: ["Qwen/Qwen3.6-27B"], documentation: "https://docs.siliconflow.cn/docs/userguide/capabilities/text-generation"),
    "groq": Provider(title: "Groq", endpoint: "https://api.groq.com/openai/v1/chat/completions", models: ["llama-3.3-70b-versatile", "llama-3.1-8b-instant"], documentation: "https://console.groq.com/docs/quickstart"),
    "openrouter": Provider(title: "OpenRouter", endpoint: "https://openrouter.ai/api/v1/chat/completions", models: ["openrouter/auto"], documentation: "https://openrouter.ai/docs/quickstart"),
    "ollama": Provider(title: "Ollama", endpoint: "http://localhost:11434/v1/chat/completions", models: [], documentation: "https://docs.ollama.com/api/openai-compatibility"),
    "custom": Provider(title: "自定义", endpoint: "", models: [], documentation: ""),
  ]
}
