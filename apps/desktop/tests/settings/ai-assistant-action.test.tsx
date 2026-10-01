// @vitest-environment jsdom
import { act, renderHook } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { useAiAssistant, type AiAssistantClient, type AiAssistantPreferences } from "@msime/ui";

afterEach(() => vi.restoreAllMocks());

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

test("a stale model request cannot overwrite the status after the AI settings change", async () => {
  const request = deferred<string[]>();
  const client: AiAssistantClient = {
    fetchModels: vi.fn().mockReturnValue(request.promise),
    test: vi.fn(),
  };
  const onChange = vi.fn();
  const { result } = renderHook(() =>
    useAiAssistant({
      client,
      ai,
      providerCredentialAvailable: true,
      onChange,
    }),
  );

  let pending!: Promise<void>;
  act(() => {
    pending = result.current.fetchModels();
  });
  act(() => {
    result.current.updateAi({ provider: "deepseek" });
  });
  await act(async () => {
    request.reject(new Error("stale failure"));
    await pending;
  });

  expect(result.current.modelsStatus).toBe("");
  expect(result.current.modelsBusy).toBe(false);
});
