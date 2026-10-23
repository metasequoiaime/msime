import { expect, test } from "vitest";
import { AI_PROVIDER_OPTIONS, aiEndpointUpdate, aiModelUpdate, aiProviderUpdate } from "@msime/ui";
import { defaultAiAssistant } from "../../../../packages/ui/src/settings/ai-assistant-defaults";
import type { AiAssistantPreferences } from "@msime/ui";

const base = (): AiAssistantPreferences => ({ ...defaultAiAssistant });

// A8：切走再切回来，自定义的接口地址/模型从每提供方槽位回来，默认值不落进槽位。
test("切提供方再切回来，自定义的 endpoint/model 还在", () => {
  const custom = {
    ...base(),
    endpoint: "https://llm.internal.example/v1/chat/completions",
    model: "my-model",
  };
  const toOpenai = { ...custom, ...aiProviderUpdate("openai", custom) };
  const openai = AI_PROVIDER_OPTIONS.find((option) => option.id === "openai")!;
  // 切走时现值进 deepseek 的槽位，新提供方用自己的默认值。
  expect(toOpenai.provider).toBe("openai");
  expect(toOpenai.endpoint).toBe(openai.endpoint);
  expect(toOpenai.model).toBe(openai.model);
  expect(toOpenai.endpoints).toEqual({
    deepseek: "https://llm.internal.example/v1/chat/completions",
  });
  expect(toOpenai.models).toEqual({ deepseek: "my-model" });
  const back = { ...toOpenai, ...aiProviderUpdate("deepseek", toOpenai) };
  expect(back.provider).toBe("deepseek");
  expect(back.endpoint).toBe("https://llm.internal.example/v1/chat/completions");
  expect(back.model).toBe("my-model");
});

test("默认值不写入槽位，多个提供方各自记忆", () => {
  const start = base();
  const toKimi = { ...start, ...aiProviderUpdate("kimi", start) };
  // deepseek 用的是目录默认值，没有自定义内容可存。
  expect(toKimi.endpoints).toEqual({});
  expect(toKimi.models).toEqual({});
  // 在 kimi 上改成自定义值再切走，deepseek 切回来仍是默认值。
  const kimiCustom = { ...toKimi, ...aiModelUpdate("kimi-custom", toKimi) };
  expect(kimiCustom.models).toEqual({ kimi: "kimi-custom" });
  const back = { ...kimiCustom, ...aiProviderUpdate("deepseek", kimiCustom) };
  expect(back.endpoint).toBe("https://api.deepseek.com/chat/completions");
  expect(back.model).toBe("deepseek-v4-flash");
});

test("空槽位按目录默认值回填，地址与模型独立处理", () => {
  const openai = AI_PROVIDER_OPTIONS.find((option) => option.id === "openai")!;
  for (const [endpoint, model, expectedEndpoint, expectedModel] of [
    ["", "", openai.endpoint, openai.model],
    ["", "synthetic-model", openai.endpoint, "synthetic-model"],
    [
      "https://fixture.example/v1/chat/completions",
      "",
      "https://fixture.example/v1/chat/completions",
      openai.model,
    ],
  ] as const) {
    const current = {
      ...base(),
      endpoints: { openai: endpoint },
      models: { openai: model },
    };
    const patch = aiProviderUpdate("openai", current);
    expect(patch.endpoint).toBe(expectedEndpoint);
    expect(patch.model).toBe(expectedModel);
    expect(current.endpoints).toEqual({ openai: endpoint });
    expect(current.models).toEqual({ openai: model });
  }
});

test("编辑接口地址/模型时同步当前提供方的槽位", () => {
  const start = base();
  const edited = { ...start, ...aiEndpointUpdate("https://gateway.internal/v1", start) };
  expect(edited.endpoint).toBe("https://gateway.internal/v1");
  expect(edited.endpoints).toEqual({ deepseek: "https://gateway.internal/v1" });
  // 改回目录默认值时删键，默认值升级后空槽自动跟上。
  const reverted = {
    ...edited,
    ...aiEndpointUpdate("https://api.deepseek.com/chat/completions", edited),
  };
  expect(reverted.endpoints).toEqual({});
});
