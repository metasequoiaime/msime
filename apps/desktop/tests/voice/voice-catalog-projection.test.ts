import { expect, test, vi } from "vitest";

vi.mock("../../../../packages/ui/src/voice/voice-provider-catalog.json", () => ({
  default: {
    asr: [
      {
        id: "synthetic-asr",
        title: "合成识别",
        endpoint: "https://voice.example.invalid/asr",
        model: "synthetic-default",
        models: ["synthetic-second", "synthetic-default"],
        documentation: "https://voice.example.invalid/docs",
      },
      {
        id: "doubao",
        title: "合成流式",
        endpoint: "wss://voice.example.invalid/default",
        model: "",
        models: [],
        documentation: "https://voice.example.invalid/stream-docs",
      },
    ],
    polish: [
      {
        id: "synthetic-polish",
        title: "合成润色",
        endpoint: "https://voice.example.invalid/polish",
        model: "synthetic-polish-default",
        models: ["synthetic-polish-default"],
        documentation: "https://voice.example.invalid/polish-docs",
      },
    ],
    doubao_stream_endpoints: [
      {
        id: "third",
        title: "第三项",
        label: "短标题",
        endpoint: "wss://voice.example.invalid/third",
      },
      {
        id: "first",
        title: "第一项",
        label: "短标题",
        endpoint: "wss://voice.example.invalid/first",
      },
      {
        id: "second",
        title: "第二项",
        label: "短标题",
        endpoint: "wss://voice.example.invalid/second",
      },
    ],
  },
}));

import {
  ASR_PROVIDER_DEFAULTS,
  ASR_SERVICE_PROVIDER_IDS,
  POLISH_PROVIDER_DEFAULTS,
  DOUBAO_STREAM_ENDPOINTS,
  asrProviderUpdate,
  doubaoStreamEndpointId,
  findDoubaoStreamEndpoint,
} from "../../../../packages/ui/src/voice/voice-providers";
import {
  ASR_PROVIDER_OPTIONS,
  POLISH_PROVIDER_OPTIONS,
} from "../../../../packages/ui/src/voice/voice-provider-options";

test("默认表读取目录字段并保留无模型条目与宿主选项", () => {
  expect(ASR_PROVIDER_DEFAULTS).toEqual({
    system: { endpoint: "", model: "" },
    local: { endpoint: "", model: "" },
    "synthetic-asr": {
      endpoint: "https://voice.example.invalid/asr",
      model: "synthetic-default",
      models: ["synthetic-second", "synthetic-default"],
      documentation: "https://voice.example.invalid/docs",
    },
    doubao: {
      endpoint: "wss://voice.example.invalid/default",
      model: "",
      documentation: "https://voice.example.invalid/stream-docs",
    },
  });
  expect(POLISH_PROVIDER_DEFAULTS).toEqual({
    "synthetic-polish": {
      endpoint: "https://voice.example.invalid/polish",
      model: "synthetic-polish-default",
      models: ["synthetic-polish-default"],
      documentation: "https://voice.example.invalid/polish-docs",
    },
  });
  expect(ASR_SERVICE_PROVIDER_IDS).toEqual(["doubao", "synthetic-asr"]);
});

test("选择器保留目录数量、顺序和标题", () => {
  expect(ASR_PROVIDER_OPTIONS).toEqual([
    { value: "synthetic-asr", label: "合成识别" },
    { value: "doubao", label: "合成流式" },
  ]);
  expect(POLISH_PROVIDER_OPTIONS).toEqual([{ value: "synthetic-polish", label: "合成润色" }]);
  expect(DOUBAO_STREAM_ENDPOINTS).toEqual([
    { id: "third", title: "第三项", endpoint: "wss://voice.example.invalid/third" },
    { id: "first", title: "第一项", endpoint: "wss://voice.example.invalid/first" },
    { id: "second", title: "第二项", endpoint: "wss://voice.example.invalid/second" },
  ]);
  expect(findDoubaoStreamEndpoint("second")?.endpoint).toBe("wss://voice.example.invalid/second");
  expect(doubaoStreamEndpointId("wss://voice.example.invalid/third")).toBe("third");
  expect(doubaoStreamEndpointId("wss://voice.example.invalid/custom")).toBe("custom");
});

test("换服务使用目录默认模型并保留自定义配置", () => {
  expect(asrProviderUpdate("synthetic-asr", {})).toMatchObject({
    asr_endpoint: "https://voice.example.invalid/asr",
    asr_model: "synthetic-default",
  });
  const custom = asrProviderUpdate("synthetic-asr", {
    asr_endpoint: "https://custom.example.invalid/asr",
    asr_model: "synthetic-custom",
  });
  expect(custom).not.toHaveProperty("asr_endpoint");
  expect(custom).not.toHaveProperty("asr_model");
});
