// @vitest-environment jsdom
import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import {
  useAiAssistant,
  type UseAiAssistantOptions,
} from "../../../../packages/ui/src/settings/use-ai-assistant";

afterEach(cleanup);

const preferences: UseAiAssistantOptions["ai"] = {
  enabled: true,
  provider: "synthetic",
  model: "synthetic-model",
  endpoint: "https://synthetic.example/v1",
  candidate_limit: 3,
  prompt_custom_1: "",
  prompt_custom_2: "",
  prompt_custom_3: "",
};

test("ignores a second model fetch while the first is pending", async () => {
  let resolve!: (models: string[]) => void;
  const pending = new Promise<string[]>((accept) => {
    resolve = accept;
  });
  const fetchModels = vi.fn().mockReturnValue(pending);
  const client = {
    fetchModels,
    test: vi.fn(),
  };
  const { result } = renderHook(() =>
    useAiAssistant({
      client,
      ai: preferences,
      providerCredentialAvailable: true,
      onChange: vi.fn(),
    }),
  );

  let first!: Promise<void>;
  let second!: Promise<void>;
  act(() => {
    first = result.current.fetchModels();
    second = result.current.fetchModels();
  });
  expect(fetchModels).toHaveBeenCalledOnce();

  resolve(["synthetic-model"]);
  await act(async () => {
    await first;
    await second;
  });
  expect(fetchModels).toHaveBeenCalledOnce();
});

test.each(
  (["models", "test"] as const).flatMap((action) =>
    [
      { name: "异常消息", cause: new Error("合成服务错误"), message: "合成服务错误" },
      { name: "空异常消息", cause: new Error(""), message: "" },
      { name: "非异常字符串", cause: "synthetic failure", message: undefined },
      { name: "带消息的普通对象", cause: { message: "synthetic failure" }, message: undefined },
    ].map((failure) => ({ action, ...failure })),
  ),
)("$action 保留$name 的既有状态文案并结束忙碌", async ({ action, cause, message }) => {
  const client = {
    fetchModels: vi.fn().mockRejectedValue(cause),
    test: vi.fn().mockRejectedValue(cause),
  };
  const { result } = renderHook(() =>
    useAiAssistant({
      client,
      ai: preferences,
      providerCredentialAvailable: true,
      onChange: vi.fn(),
    }),
  );
  act(() => result.current.setTestInput("合成测试文字"));
  await act(async () => {
    await (action === "models" ? result.current.fetchModels() : result.current.test());
  });

  const fallback =
    action === "models"
      ? "获取模型失败，请检查地址、密钥和网络。"
      : "AI 请求失败，请检查地址、模型、密钥和网络。";
  expect(action === "models" ? result.current.modelsStatus : result.current.testStatus).toBe(
    message ?? fallback,
  );
  expect(action === "models" ? result.current.modelsBusy : result.current.testBusy).toBe(false);
});

test("Ollama fetches models and tests without reading a stored token", async () => {
  const fetchModels = vi.fn().mockResolvedValue(["qwen3:8b"]);
  const testRequest = vi.fn().mockResolvedValue("ok");
  const ollama: UseAiAssistantOptions["ai"] = {
    ...preferences,
    provider: "ollama",
    endpoint: "http://localhost:11434/v1/chat/completions",
    tokens: { "http://localhost:11434": "stale-token" },
  };
  const { result } = renderHook(() =>
    useAiAssistant({
      client: { fetchModels, test: testRequest },
      ai: ollama,
      providerCredentialAvailable: false,
      onChange: vi.fn(),
    }),
  );
  expect(result.current.token).toBe("");
  await act(async () => {
    await result.current.fetchModels();
  });
  expect(fetchModels).toHaveBeenCalledWith({
    endpoint: ollama.endpoint,
    token: "",
    provider: "ollama",
  });
  act(() => result.current.setTestInput("synthetic text"));
  await act(async () => {
    await result.current.test();
  });
  expect(testRequest).toHaveBeenCalledWith(
    expect.objectContaining({ provider: "ollama", token: "" }),
  );
});
