import { useRef, useState } from "react";
import { runAsyncAction } from "../core/async-action";
import { aiCredentialOrigin } from "./credential-utils";
import { aiPolishTestPrompt } from "./ai-assistant-defaults";
import type { AiAssistantClient, AiAssistantPreferences } from "../index";

export interface UseAiAssistantOptions {
  client?: AiAssistantClient;
  ai: AiAssistantPreferences;
  providerCredentialAvailable: boolean;
  onChange: (patch: Partial<AiAssistantPreferences>) => void;
}

/** Owns AI model discovery and prompt test requests, including stale-response cancellation. */
export function useAiAssistant({
  client,
  ai,
  providerCredentialAvailable,
  onChange,
}: UseAiAssistantOptions) {
  const [models, setModels] = useState<string[] | null>(null);
  const [modelsStatus, setModelsStatus] = useState("");
  const [modelsBusy, setModelsBusy] = useState(false);
  const [testInput, setTestInput] = useState("");
  const [testOutput, setTestOutput] = useState("");
  const [testStatus, setTestStatus] = useState("");
  const [testBusy, setTestBusy] = useState(false);
  const requestGeneration = useRef(0);
  const origin = aiCredentialOrigin(ai.endpoint);
  const token = origin ? (ai.tokens?.[origin] ?? "") : "";

  const updateAi = (patch: Partial<AiAssistantPreferences>) => {
    requestGeneration.current += 1;
    setModelsBusy(false);
    setTestBusy(false);
    setTestOutput("");
    setTestStatus("");
    if (patch.provider !== undefined || patch.endpoint !== undefined) {
      setModels(null);
      setModelsStatus("");
    }
    onChange(patch);
  };

  const updateToken = (value: string) => {
    requestGeneration.current += 1;
    setModelsBusy(false);
    setTestBusy(false);
    setTestOutput("");
    setTestStatus("");
    if (origin) updateAi({ token: "", tokens: { ...ai.tokens, [origin]: value } });
  };

  const fetchModels = async () => {
    if (!client) return;
    if (!origin) {
      setModelsStatus("请先填写完整的 HTTPS 接口地址。");
      return;
    }
    if (!providerCredentialAvailable && !token.trim()) {
      setModelsStatus("请先填写 API Token，或使用已保存的密钥。");
      return;
    }
    const generation = requestGeneration.current;
    await runAsyncAction(
      {
        busy: modelsBusy,
        isCurrent: () => generation === requestGeneration.current,
        setBusy: setModelsBusy,
        setError: setModelsStatus,
      },
      async (isCurrent) => {
        const available = await client.fetchModels({
          endpoint: ai.endpoint,
          token,
          provider: ai.provider,
        });
        if (!isCurrent()) return;
        setModels(available);
        setModelsStatus(`已获取 ${available.length} 个可用模型。`);
        if (available.length && !available.includes(ai.model)) updateAi({ model: available[0] });
      },
      {
        formatError: (cause) =>
          cause instanceof Error ? cause.message : "获取模型失败，请检查地址、密钥和网络。",
      },
    );
  };

  const test = async () => {
    if (!client) return;
    if (!testInput.trim()) {
      setTestStatus("请先输入待润色文字。");
      return;
    }
    if (!origin || (!providerCredentialAvailable && !token.trim())) {
      setTestStatus(
        providerCredentialAvailable
          ? "请先填写有效的 HTTPS 接口地址。"
          : "请先填写有效的 HTTPS 接口地址和 API Token。",
      );
      return;
    }
    const generation = ++requestGeneration.current;
    setTestOutput("");
    await runAsyncAction(
      {
        busy: testBusy,
        isCurrent: () => generation === requestGeneration.current,
        setBusy: setTestBusy,
        setError: setTestStatus,
      },
      async (isCurrent) => {
        const result = await client.test({
          endpoint: ai.endpoint,
          model: ai.model,
          provider: ai.provider,
          prompt: aiPolishTestPrompt,
          token,
          text: testInput,
        });
        if (!isCurrent()) return;
        setTestOutput(result);
        setTestStatus("已完成");
      },
      {
        formatError: (cause) =>
          cause instanceof Error ? cause.message : "AI 请求失败，请检查地址、模型、密钥和网络。",
      },
    );
  };

  return {
    ai,
    origin,
    token,
    models,
    modelsStatus,
    modelsBusy,
    testInput,
    setTestInput,
    testOutput,
    testStatus,
    testBusy,
    updateAi,
    updateToken,
    fetchModels,
    test,
  };
}
