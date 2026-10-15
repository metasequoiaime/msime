// @vitest-environment jsdom
import { act, renderHook } from "@testing-library/react";
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

function setup() {
  const models = deferred<string[]>();
  const polish = deferred<string>();
  const client: AiAssistantClient = {
    fetchModels: vi.fn().mockReturnValue(models.promise),
    test: vi.fn().mockReturnValue(polish.promise),
  };
  const onChange = vi.fn();
  const hook = renderHook(() =>
    useAiAssistant({ client, ai, providerCredentialAvailable: true, onChange }),
  );
  act(() => hook.result.current.setTestInput("合成测试文字"));
  return { ...hook, models, polish, onChange };
}

test.each(["resolve", "reject"] as const)(
  "获取模型途中发送润色后，模型请求的 %s 仍能完成并释放忙碌状态",
  async (outcome) => {
    const { result, models, polish } = setup();
    let listing!: Promise<void>;
    let testing!: Promise<void>;
    act(() => {
      listing = result.current.fetchModels();
    });
    act(() => {
      testing = result.current.test();
    });
    await act(async () => {
      if (outcome === "resolve") models.resolve([ai.model]);
      else models.reject(new Error("合成模型目录错误"));
      await listing;
    });
    expect(result.current.modelsBusy).toBe(false);
    expect(result.current.testBusy).toBe(true);
    expect(result.current.modelsStatus).toBe(
      outcome === "resolve" ? "已获取 1 个可用模型。" : "合成模型目录错误",
    );
    await act(async () => {
      polish.resolve("合成润色结果");
      await testing;
    });
    expect(result.current.testOutput).toBe("合成润色结果");
    expect(result.current.testBusy).toBe(false);
  },
);

test("润色途中获取当前模型目录，两种请求都能保留各自结果", async () => {
  const { result, models, polish } = setup();
  let testing!: Promise<void>;
  let listing!: Promise<void>;
  act(() => {
    testing = result.current.test();
  });
  act(() => {
    listing = result.current.fetchModels();
  });
  await act(async () => {
    polish.resolve("合成润色结果");
    await testing;
  });
  expect(result.current.modelsBusy).toBe(true);
  await act(async () => {
    models.resolve([ai.model]);
    await listing;
  });
  expect(result.current.models).toEqual([ai.model]);
  expect(result.current.testOutput).toBe("合成润色结果");
  expect(result.current.modelsBusy).toBe(false);
  expect(result.current.testBusy).toBe(false);
});

test("模型目录自动选择新模型后，仍丢弃旧模型的在途润色结果", async () => {
  const { result, models, polish, onChange } = setup();
  let listing!: Promise<void>;
  let testing!: Promise<void>;
  act(() => {
    listing = result.current.fetchModels();
  });
  act(() => {
    testing = result.current.test();
  });
  await act(async () => {
    models.resolve(["replacement-model"]);
    await listing;
  });
  expect(onChange).toHaveBeenCalledWith({ model: "replacement-model" });
  expect(result.current.testBusy).toBe(false);
  await act(async () => {
    polish.resolve("旧模型的合成结果");
    await testing;
  });
  expect(result.current.testOutput).toBe("");
  expect(result.current.testStatus).toBe("");
});
