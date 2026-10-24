import { expect, test, vi } from "vitest";

vi.mock("../../../../packages/ui/src/settings/ai-provider-catalog.json", () => ({
  default: [
    {
      id: "synthetic-ai",
      title: "合成服务𠮷",
      endpoint: "https://ai.example.invalid/chat/completions",
      model: "默认𠮷",
      models: ["建议二", "默认𠮷"],
      documentation: "https://ai.example.invalid/docs",
    },
    { id: "ollama", title: "合成本地", endpoint: "http://localhost:11434/v1/test", model: "" },
    { id: "custom", title: "合成自定义", endpoint: "", model: "" },
  ],
}));

import {
  AI_PROVIDER_OPTIONS,
  aiProviderOption,
  aiProviderOptionsFor,
  aiProviderUsesToken,
} from "../../../../packages/ui/src/settings/ai-provider-options";
import { updateAiProvider } from "../../../../packages/ui/src/settings/ai-provider-update";

test("AI 选择器实际读取共享目录的集合、顺序与全部元数据", () => {
  expect(AI_PROVIDER_OPTIONS.map((p) => p.id)).toEqual(["synthetic-ai", "ollama", "custom"]);
  expect(aiProviderOption("synthetic-ai")).toEqual({
    id: "synthetic-ai",
    title: "合成服务𠮷",
    endpoint: "https://ai.example.invalid/chat/completions",
    model: "默认𠮷",
    models: ["建议二", "默认𠮷"],
    documentation: "https://ai.example.invalid/docs",
  });
  expect(aiProviderOption("openai")).toBeUndefined();
  expect(aiProviderOption("SYNTHETIC-AI")).toBeUndefined();
  expect(aiProviderOption(" synthetic-ai ")).toBeUndefined();
});

test("投影保留缺省字段形状与现有桌面、鉴权策略", () => {
  expect(aiProviderOption("ollama")).not.toHaveProperty("models");
  expect(aiProviderOption("custom")).not.toHaveProperty("documentation");
  expect(aiProviderOptionsFor(true, "custom")).toBe(AI_PROVIDER_OPTIONS);
  expect(aiProviderOptionsFor(false, "custom").map((p) => p.id)).toEqual([
    "synthetic-ai",
    "custom",
  ]);
  expect(aiProviderOptionsFor(false, "ollama")).toEqual(AI_PROVIDER_OPTIONS);
  expect(aiProviderUsesToken("ollama")).toBe(false);
  expect(aiProviderUsesToken("synthetic-ai")).toBe(true);
});

test("换服务读取独立默认模型并保留自定义值与未知服务回退", () => {
  expect(
    updateAiProvider(
      "synthetic-ai",
      { provider: "custom", endpoint: "", model: "" },
      AI_PROVIDER_OPTIONS,
    ),
  ).toEqual({
    provider: "synthetic-ai",
    endpoint: "https://ai.example.invalid/chat/completions",
    model: "默认𠮷",
    endpoints: {},
    models: {},
  });
  const custom = {
    provider: "custom",
    endpoint: "https://custom.example.invalid/v1",
    model: "synthetic-custom-model",
  };
  // 自定义值写回 custom 的槽位，新提供方用自己的默认值；切回来原样返回。
  const patched = updateAiProvider("synthetic-ai", custom, AI_PROVIDER_OPTIONS);
  expect(patched).toEqual({
    provider: "synthetic-ai",
    endpoint: "https://ai.example.invalid/chat/completions",
    model: "默认𠮷",
    endpoints: { custom: "https://custom.example.invalid/v1" },
    models: { custom: "synthetic-custom-model" },
  });
  expect(updateAiProvider("custom", { ...custom, ...patched }, AI_PROVIDER_OPTIONS)).toMatchObject({
    endpoint: "https://custom.example.invalid/v1",
    model: "synthetic-custom-model",
  });
  expect(
    updateAiProvider(
      "unknown",
      { provider: "synthetic-ai", endpoint: "", model: "" },
      AI_PROVIDER_OPTIONS,
    ),
  ).toEqual({ provider: "custom", endpoint: "", model: "", endpoints: {}, models: {} });
});
