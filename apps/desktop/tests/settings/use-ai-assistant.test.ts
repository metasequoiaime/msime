// @vitest-environment jsdom
import { act, renderHook } from "@testing-library/react";
import { expect, test, vi } from "vitest";
import {
  useAiAssistant,
  type UseAiAssistantOptions,
} from "../../../../packages/ui/src/settings/use-ai-assistant";

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
