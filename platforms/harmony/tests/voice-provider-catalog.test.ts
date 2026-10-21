import { VoiceProviderCatalog } from "../entry/src/main/ets/keyboard/input/VoiceProviderCatalog";
import { DEFAULT_VOICE_INPUT_CONFIGURATION } from "../entry/src/main/ets/keyboard/input/VoiceInputConfiguration";

declare const process: { exitCode?: number };

// 固定迁移前的服务默认值，避免生成器与消费者一起漂移。
const cases: { id: string; endpoint: string; model: string }[] = [
  {
    id: "siliconflow",
    endpoint: "https://api.siliconflow.cn/v1/chat/completions",
    model: "Qwen/Qwen3-8B",
  },
  {
    id: "openai",
    endpoint: "https://api.openai.com/v1/chat/completions",
    model: "gpt-4o-mini",
  },
  {
    id: "deepseek",
    endpoint: "https://api.deepseek.com/chat/completions",
    model: "deepseek-v4-flash",
  },
  {
    id: "groq",
    endpoint: "https://api.groq.com/openai/v1/chat/completions",
    model: "llama-3.3-70b-versatile",
  },
  {
    id: "",
    endpoint: "",
    model: "",
  },
  {
    id: "synthetic-unknown",
    endpoint: "",
    model: "",
  },
  {
    id: " openai ",
    endpoint: "",
    model: "",
  },
  {
    id: "OPENAI",
    endpoint: "",
    model: "",
  },
];
let failures = 0;
for (const item of cases) {
  if (
    VoiceProviderCatalog.polishEndpoint(item.id) !== item.endpoint ||
    VoiceProviderCatalog.polishModel(item.id) !== item.model
  ) {
    console.error(`FAIL 润色服务预设 ${item.id}`);
    failures++;
  }
}
const expectedDefaults = {
  polish_provider: "siliconflow",
  polish_endpoint: "https://api.siliconflow.cn/v1/chat/completions",
  polish_model: "Qwen/Qwen3-8B",
};
if (
  DEFAULT_VOICE_INPUT_CONFIGURATION.polish_provider !== expectedDefaults.polish_provider ||
  DEFAULT_VOICE_INPUT_CONFIGURATION.polish_endpoint !== expectedDefaults.polish_endpoint ||
  DEFAULT_VOICE_INPUT_CONFIGURATION.polish_model !== expectedDefaults.polish_model
) {
  console.error("FAIL 初始润色配置");
  failures++;
}
if (failures > 0) process.exitCode = 1;
else console.log("  ok  润色服务目录、未知服务回退与初始配置");
