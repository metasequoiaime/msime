import { useEffect, useRef, useState } from "react";
import { runAsyncAction } from "../core/async-action";
import { aiCredentialOrigin } from "./credential-utils";
import { aiPolishTestPrompt } from "./ai-assistant-defaults";
import { useAsyncGeneration } from "./use-async-generation";
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
  const requestGeneration = useAsyncGeneration(client);
  const modelsActionBusy = useRef(false);
  const modelsActionOwner = useAsyncGeneration();
  const testActionBusy = useRef(false);
  const testActionOwner = useRef(0);
  const origin = aiCredentialOrigin(ai.endpoint);
  const token = origin ? (ai.tokens?.[origin] ?? "") : "";

  const runAction = async (
    busyRef: { current: boolean },
    ownerRef: { current: number },
    busy: boolean,
    setBusy: (value: boolean) => void,
    setError: (message: string) => void,
    generation: number,
    operation: (isCurrent: () => boolean) => Promise<void>,
    formatError: (error: unknown) => string,
  ) => {
    if (busyRef.current || busy) return;
    const owner = ++ownerRef.current;
    busyRef.current = true;
    await runAsyncAction(
      {
        busy: false,
        isCurrent: () => generation === requestGeneration.current && owner === ownerRef.current,
        setBusy: (value) => {
          if (owner !== ownerRef.current) return;
          busyRef.current = value;
          setBusy(value);
        },
        setError,
      },
      operation,
      { formatError },
    );
    if (owner === ownerRef.current) busyRef.current = false;
  };

  useEffect(() => {
    modelsActionOwner.current += 1;
    modelsActionBusy.current = false;
    testActionOwner.current += 1;
    testActionBusy.current = false;
    setModels(null);
    setModelsStatus("");
    setModelsBusy(false);
    setTestOutput("");
    setTestStatus("");
    setTestBusy(false);
  }, [client]);

  const updateAi = (patch: Partial<AiAssistantPreferences>) => {
    requestGeneration.current += 1;
    modelsActionOwner.current += 1;
    modelsActionBusy.current = false;
    testActionOwner.current += 1;
    testActionBusy.current = false;
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
    modelsActionOwner.current += 1;
    modelsActionBusy.current = false;
    testActionOwner.current += 1;
    testActionBusy.current = false;
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
    await runAction(
      modelsActionBusy,
      modelsActionOwner,
      modelsBusy,
      setModelsBusy,
      setModelsStatus,
      generation,
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
      (cause) =>
        cause instanceof Error ? cause.message : "获取模型失败，请检查地址、密钥和网络。",
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
    if (testActionBusy.current || testBusy) return;
    const generation = ++requestGeneration.current;
    setTestOutput("");
    await runAction(
      testActionBusy,
      testActionOwner,
      testBusy,
      setTestBusy,
      setTestStatus,
      generation,
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
      (cause) =>
        cause instanceof Error ? cause.message : "AI 请求失败，请检查地址、模型、密钥和网络。",
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
