// @vitest-environment jsdom
import { act, renderHook } from "@testing-library/react";
import { useState } from "react";
import { expect, test, vi } from "vitest";
import { useAiAssistant, type AiAssistantClient, type AiAssistantPreferences } from "@msime/ui";

const ai: AiAssistantPreferences = {
  enabled: true,
  provider: "openai",
  endpoint: "https://ai.example.test/v1",
  model: "fixture-model",
  candidate_limit: 3,
  prompt_custom_1: "",
  prompt_custom_2: "",
  prompt_custom_3: "",
};

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((accept, fail) => {
    resolve = accept;
    reject = fail;
  });
  return { promise, resolve, reject };
}

test.each([
  { endpoint: "https://next.example.test/v1" },
  { provider: "deepseek" },
  { model: "restored-model" },
  { tokens: { "https://ai.example.test": "synthetic-replacement" } },
])("外部替换 AI 配置 %j 后旧模型列表不能改写新配置", async (patch) => {
  const request = deferred<string[]>();
  const client: AiAssistantClient = {
    fetchModels: vi.fn().mockReturnValue(request.promise),
    test: vi.fn(),
  };
  const onChange = vi.fn();
  const { result, rerender } = renderHook(
    ({ preferences }) =>
      useAiAssistant({ client, ai: preferences, providerCredentialAvailable: true, onChange }),
    { initialProps: { preferences: ai } },
  );
  let pending!: Promise<void>;
  act(() => {
    pending = result.current.fetchModels();
  });
  rerender({ preferences: { ...ai, ...patch } });
  expect(result.current.modelsBusy).toBe(false);
  await act(async () => {
    request.resolve(["stale-model"]);
    await pending;
  });
  expect(onChange).not.toHaveBeenCalled();
  expect(result.current.models).toBeNull();
  expect(result.current.modelsStatus).toBe("");
});

test("外部配置 A→B→A 后旧模型请求仍然失效", async () => {
  const request = deferred<string[]>();
  const client: AiAssistantClient = {
    fetchModels: vi.fn().mockReturnValue(request.promise),
    test: vi.fn(),
  };
  const onChange = vi.fn();
  const { result, rerender } = renderHook(
    ({ preferences }) =>
      useAiAssistant({ client, ai: preferences, providerCredentialAvailable: true, onChange }),
    { initialProps: { preferences: ai } },
  );
  let pending!: Promise<void>;
  act(() => {
    pending = result.current.fetchModels();
  });
  rerender({ preferences: { ...ai, model: "temporary-model" } });
  rerender({ preferences: ai });
  await act(async () => {
    request.resolve(["stale-model"]);
    await pending;
  });
  expect(onChange).not.toHaveBeenCalled();
  expect(result.current.models).toBeNull();
});

test.each(["resolve", "reject"] as const)(
  "外部配置变化后旧润色请求的 %s 不覆盖反馈，新请求可立即开始",
  async (outcome) => {
    const old = deferred<string>();
    const next = deferred<string>();
    const client: AiAssistantClient = {
      fetchModels: vi.fn(),
      test: vi.fn().mockReturnValueOnce(old.promise).mockReturnValueOnce(next.promise),
    };
    const { result, rerender } = renderHook(
      ({ preferences }) =>
        useAiAssistant({
          client,
          ai: preferences,
          providerCredentialAvailable: true,
          onChange: vi.fn(),
        }),
      { initialProps: { preferences: ai } },
    );
    act(() => result.current.setTestInput("合成测试文字"));
    let oldPending!: Promise<void>;
    act(() => {
      oldPending = result.current.test();
    });
    rerender({ preferences: { ...ai, model: "next-model" } });
    expect(result.current.testBusy).toBe(false);
    let nextPending!: Promise<void>;
    act(() => {
      nextPending = result.current.test();
    });
    expect(client.test).toHaveBeenCalledTimes(2);
    await act(async () => {
      if (outcome === "resolve") old.resolve("旧配置的合成结果");
      else old.reject(new Error("旧配置的合成错误"));
      await oldPending;
    });
    expect(result.current.testBusy).toBe(true);
    expect(result.current.testOutput).toBe("");
    expect(result.current.testStatus).toBe("");
    await act(async () => {
      next.resolve("当前配置的合成结果");
      await nextPending;
    });
    expect(result.current.testBusy).toBe(false);
    expect(result.current.testOutput).toBe("当前配置的合成结果");
    expect(result.current.testStatus).toBe("已完成");
  },
);

test("宿主凭据能力变化后旧模型错误不写入当前页面", async () => {
  const request = deferred<string[]>();
  const client: AiAssistantClient = {
    fetchModels: vi.fn().mockReturnValue(request.promise),
    test: vi.fn(),
  };
  const { result, rerender } = renderHook(
    ({ available }) =>
      useAiAssistant({
        client,
        ai,
        providerCredentialAvailable: available,
        onChange: vi.fn(),
      }),
    { initialProps: { available: true } },
  );
  let pending!: Promise<void>;
  act(() => {
    pending = result.current.fetchModels();
  });
  rerender({ available: false });
  await act(async () => {
    request.reject(new Error("旧凭据的合成错误"));
    await pending;
  });
  expect(result.current.modelsBusy).toBe(false);
  expect(result.current.modelsStatus).toBe("");
});

test("同值配置副本不会取消当前润色或清空结果", async () => {
  const request = deferred<string>();
  const client: AiAssistantClient = {
    fetchModels: vi.fn(),
    test: vi.fn().mockReturnValue(request.promise),
  };
  const { result, rerender } = renderHook(
    ({ preferences }) =>
      useAiAssistant({
        client,
        ai: preferences,
        providerCredentialAvailable: true,
        onChange: vi.fn(),
      }),
    { initialProps: { preferences: ai } },
  );
  act(() => result.current.setTestInput("合成测试文字"));
  let pending!: Promise<void>;
  act(() => {
    pending = result.current.test();
  });
  rerender({ preferences: { ...ai } });
  expect(result.current.testBusy).toBe(true);
  await act(async () => {
    request.resolve("有效的合成结果");
    await pending;
  });
  expect(result.current.testOutput).toBe("有效的合成结果");
  rerender({ preferences: { ...ai } });
  expect(result.current.testOutput).toBe("有效的合成结果");
});

test("当前模型列表自动选择首个模型后仍保留已获取的列表和反馈", async () => {
  const client: AiAssistantClient = {
    fetchModels: vi.fn().mockResolvedValue(["available-model"]),
    test: vi.fn(),
  };
  const { result } = renderHook(() => {
    const [preferences, setPreferences] = useState(ai);
    return useAiAssistant({
      client,
      ai: preferences,
      providerCredentialAvailable: true,
      onChange: (patch) => setPreferences((current) => ({ ...current, ...patch })),
    });
  });
  await act(async () => {
    await result.current.fetchModels();
  });
  expect(result.current.ai.model).toBe("available-model");
  expect(result.current.models).toEqual(["available-model"]);
  expect(result.current.modelsStatus).toBe("已获取 1 个可用模型。");
  expect(result.current.modelsBusy).toBe(false);
});
