import { AiGenerationGate } from "../entry/src/main/ets/keyboard/AiGenerationGate";
import {
  FunctionPanelItem,
  FunctionPanelPolicy,
} from "../entry/src/main/ets/keyboard/FunctionPanelPolicy";
import {
  TranslationPolicy,
  TranslationQuery,
} from "../entry/src/main/ets/keyboard/candidate/TranslationPolicy";

declare const process: { exitCode?: number };

// 鸿蒙版拿到 AI 生成类资质之前，Rust 的 `HostCapabilities::for_platform(Harmony).ai_generation` 为 false；键盘和设置桥据此不发 AI 请求、不画 AI 入口。

let failures = 0;
function check(condition: boolean, message: string): void {
  if (condition) {
    console.log(`  ok  ${message}`);
  } else {
    console.error(`FAIL  ${message}`);
    failures++;
  }
}

const capabilities = (aiGeneration: boolean | undefined): string =>
  JSON.stringify({
    ok: true,
    value:
      aiGeneration === undefined
        ? { platform: "harmony" }
        : { platform: "harmony", ai_generation: aiGeneration },
  });

// ---- 读宿主能力 ----
check(AiGenerationGate.enabled() === false, "adopt 之前按关闭处理");
check(
  AiGenerationGate.fromCapabilities(capabilities(false)) === false,
  "ai_generation 为 false 时关闭",
);
check(
  AiGenerationGate.fromCapabilities(capabilities(true)) === true,
  "ai_generation 为 true 时打开",
);
check(
  AiGenerationGate.fromCapabilities(capabilities(undefined)) === false,
  "回答里没有这一项时按关闭处理",
);
check(
  AiGenerationGate.fromCapabilities(JSON.stringify({ ok: false, error: "invalid platform" })) ===
    false,
  "宿主拒绝时按关闭处理",
);
check(AiGenerationGate.fromCapabilities("not json") === false, "读不懂的回答按关闭处理");
check(
  AiGenerationGate.fromCapabilities(
    JSON.stringify({ ok: true, value: { ai_generation: "true" } }),
  ) === false,
  "只有布尔 true 才算打开",
);

// ---- 总闸关着：功能面板、翻译服务和设置桥 ----
AiGenerationGate.adopt(capabilities(false));
check(AiGenerationGate.enabled() === false, "adopt 关闭的能力后保持关闭");

const ids = (items: FunctionPanelItem[]): string[] =>
  items.map((item: FunctionPanelItem) => item.id);
const closedTiles: string[] = ids(FunctionPanelPolicy.items(AiGenerationGate.enabled()));
check(
  !closedTiles.includes(FunctionPanelPolicy.SMART_REPLY) &&
    !closedTiles.includes(FunctionPanelPolicy.AI_POLISH),
  "总闸关着时功能面板没有高情商回复和 AI 润色",
);
check(
  closedTiles.length === FunctionPanelPolicy.items().length - 2 &&
    closedTiles.includes(FunctionPanelPolicy.VOICE) &&
    closedTiles.includes(FunctionPanelPolicy.TRANSLATION),
  "其余格子（语音输入、候选翻译等）照旧，顺序不变",
);
check(
  FunctionPanelPolicy.items(false) === FunctionPanelPolicy.items(false),
  "去掉 AI 格子的列表每次是同一个数组，分页网格不会重建",
);

const aiOnly: TranslationQuery = {
  generation: 3,
  target_language: "en",
  candidates: [{ text: "你好", source: 0 }],
  custom_translation: null,
  tencent_tmt: null,
  niutrans: null,
  ai_assistant: {
    enabled: true,
    provider: "deepseek",
    model: "deepseek-chat",
    token: "synthetic-token",
    endpoint: "https://api.deepseek.com/v1",
  },
  english_gloss: true,
};
check(TranslationPolicy.provider(aiOnly) === "", "总闸关着时 AI 辅助翻译当作没有选在线服务");
const withTencent: TranslationQuery = {
  ...aiOnly,
  tencent_tmt: {
    enabled: true,
    secret_id: "AKIDsynthetic",
    secret_key: "synthetic",
    region: "ap-guangzhou",
  },
};
check(TranslationPolicy.provider(withTencent) === "tencent", "机器翻译服务不受总闸影响");

for (const kind of ["chat", "ai_skin", "ai_test", "ai_models"]) {
  check(!AiGenerationGate.allowsSettingsRequest(kind, "{}"), `设置桥拒绝 ${kind}`);
}
for (const operation of ["chat", "chat_models"]) {
  check(
    !AiGenerationGate.allowsSettingsRequest("account", JSON.stringify({ operation })),
    `设置桥拒绝经账号通道发的 ${operation}`,
  );
}
for (const service of ["ai.assistant", "voice.polish", "translation.ai"]) {
  check(
    !AiGenerationGate.allowsSettingsRequest(
      "api_credential",
      JSON.stringify({ service, config: {} }),
    ),
    `设置桥拒绝探 AI 服务的配置测试 ${service}`,
  );
}
for (const [kind, payload] of [
  ["account", JSON.stringify({ operation: "login" })],
  ["account", JSON.stringify({ operation: "feedback" })],
  ["api_credential", JSON.stringify({ service: "voice.asr", config: {} })],
  ["api_credential", JSON.stringify({ service: "translation.tencent", config: {} })],
  ["community_resource", "{}"],
  ["cloud_dictionary", "{}"],
  ["update_check", "{}"],
]) {
  check(AiGenerationGate.allowsSettingsRequest(kind, payload), `设置桥照常放行 ${kind} ${payload}`);
}
check(
  JSON.parse(AiGenerationGate.refusal()).error === "ai_generation_unavailable",
  "拒绝时给页面一个错误码，而不是让请求一直挂着",
);

// ---- 资质到位、总闸打开 ----
AiGenerationGate.adopt(capabilities(true));
const openTiles: string[] = ids(FunctionPanelPolicy.items(AiGenerationGate.enabled()));
check(
  openTiles.includes(FunctionPanelPolicy.SMART_REPLY) &&
    openTiles.includes(FunctionPanelPolicy.AI_POLISH),
  "总闸打开时两个 AI 格子回来",
);
check(TranslationPolicy.provider(aiOnly) === "ai", "总闸打开时 AI 辅助翻译照旧");
check(AiGenerationGate.allowsSettingsRequest("chat", "{}"), "总闸打开时设置桥放行 AI 对话");
AiGenerationGate.adopt(capabilities(false));

if (failures > 0) process.exitCode = 1;
