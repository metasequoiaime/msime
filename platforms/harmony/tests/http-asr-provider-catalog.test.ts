import { HttpAsrConfigurationPolicy } from "../entry/src/main/ets/keyboard/input/HttpAsrConfigurationPolicy";
import { DEFAULT_VOICE_INPUT_CONFIGURATION } from "../entry/src/main/ets/keyboard/input/VoiceInputConfiguration";
import { VoiceProviderCatalog } from "../entry/src/main/ets/keyboard/input/VoiceProviderCatalog";

declare const process: { exitCode?: number };

// 固定迁移前的 HTTP 识别预设，避免生成器和消费者一起漂移。
const presets: { id: string; endpoint: string; model: string; format: string }[] = [
  {
    id: "openai",
    endpoint: "https://api.openai.com/v1/audio/transcriptions",
    model: "whisper-1",
    format: "multipart",
  },
  {
    id: "siliconflow",
    endpoint: "https://api.siliconflow.cn/v1/audio/transcriptions",
    model: "FunAudioLLM/SenseVoiceSmall",
    format: "multipart",
  },
  {
    id: "groq",
    endpoint: "https://api.groq.com/openai/v1/audio/transcriptions",
    model: "whisper-large-v3-turbo",
    format: "multipart",
  },
  {
    id: "everyapi",
    endpoint: "https://api.everyapi.ai/v1/audio/transcriptions",
    model: "volc.seedasr.sauc.duration",
    format: "multipart",
  },
  {
    id: "mistral",
    endpoint: "https://api.mistral.ai/v1/audio/transcriptions",
    model: "voxtral-mini-latest",
    format: "multipart",
  },
  {
    id: "bailian",
    endpoint: "https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions",
    model: "qwen3-asr-flash",
    format: "chat_audio",
  },
];
let failures = 0;
function check(matched: boolean, message: string): void {
  if (matched) return;
  console.error(`FAIL ${message}`);
  failures++;
}
for (const preset of presets) {
  const defaults = HttpAsrConfigurationPolicy.defaults(preset.id);
  check(
    defaults?.endpoint === preset.endpoint && defaults.model === preset.model,
    `HTTP 识别默认值 ${preset.id}`,
  );
  check(
    HttpAsrConfigurationPolicy.supported(preset.id) &&
      HttpAsrConfigurationPolicy.requestFormat(preset.id) === preset.format,
    `HTTP 识别请求格式 ${preset.id}`,
  );
  const config = {
    ...DEFAULT_VOICE_INPUT_CONFIGURATION,
    asr_provider: preset.id,
    asr_endpoint: " ",
    asr_model: " ",
    asr_token: "synthetic-token",
  };
  check(
    HttpAsrConfigurationPolicy.endpoint(config) === preset.endpoint &&
      HttpAsrConfigurationPolicy.model(config) === preset.model &&
      HttpAsrConfigurationPolicy.valid(config),
    `HTTP 识别空白字段回退 ${preset.id}`,
  );
  const custom = {
    ...config,
    asr_endpoint: " https://synthetic.example.test/asr ",
    asr_model: " 合成识别模型😀 ",
  };
  check(
    HttpAsrConfigurationPolicy.endpoint(custom) === "https://synthetic.example.test/asr" &&
      HttpAsrConfigurationPolicy.model(custom) === "合成识别模型😀" &&
      HttpAsrConfigurationPolicy.valid(custom),
    `HTTP 识别自定义值优先 ${preset.id}`,
  );
  if (defaults !== null) defaults.endpoint = "https://synthetic.example.test/mutated";
  check(
    HttpAsrConfigurationPolicy.defaults(preset.id)?.endpoint === preset.endpoint,
    `HTTP 识别默认值不会被调用方污染 ${preset.id}`,
  );
}
// 流式、本地、系统与不规范 id 不得进入 HTTP 上传路径。
const unsupported: string[] = [
  "doubao",
  "local",
  "system",
  "",
  "synthetic-unknown",
  " openai ",
  "OPENAI",
];
for (const provider of unsupported) {
  check(
    VoiceProviderCatalog.httpAsrEndpoint(provider) === "" &&
      VoiceProviderCatalog.httpAsrModel(provider) === "" &&
      VoiceProviderCatalog.httpAsrRequestFormat(provider) === "",
    `生成目录排除非 HTTP 服务 ${provider}`,
  );
  check(
    HttpAsrConfigurationPolicy.defaults(provider) === null &&
      HttpAsrConfigurationPolicy.requestFormat(provider) === "" &&
      !HttpAsrConfigurationPolicy.supported(provider),
    `非 HTTP 服务回退 ${provider}`,
  );
}
if (failures > 0) process.exitCode = 1;
else console.log("  ok  HTTP 识别服务目录、请求格式、回退与自定义优先");
