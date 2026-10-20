import { useEffect, useRef, useState } from "react";
import { exceptionMessage } from "../core/exception-message";
import { runAsyncAction } from "../core/async-action";
import { deepEqual } from "../core/deep-equal";
import { aiCredentialOrigin } from "./credential-utils";
import { aiEndpointHint } from "./ai-endpoint-policy";
import { aiPolishTestPrompt } from "./ai-assistant-defaults";
import { aiProviderUsesToken } from "./ai-provider-options";
import { useAsyncGeneration } from "./use-async-generation";
import type { AiAssistantPreferences } from "./ai-assistant-preferences";
import type { AiAssistantClient } from "../core/ai-assistant-client";

export interface UseAiAssistantOptions {
  client?: AiAssistantClient;
  ai: AiAssistantPreferences;
  providerCredentialAvailable: boolean;
  credentialGeneration?: number;
  /** 成功回执先更新此代次，避免 React 提交前接受旧请求。 */
  credentialGenerationRef?: Readonly<{ current: number }>;
  onChange: (patch: Partial<AiAssistantPreferences>) => void;
}

/** 管理 AI 模型获取和润色测试，丢弃旧客户端或旧配置的异步反馈。 */
export function useAiAssistant({
  client,
  ai,
  providerCredentialAvailable,
  credentialGeneration = 0,
  credentialGenerationRef,
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
  const previousOwner = useRef<
    | {
        client: AiAssistantClient | undefined;
        ai: AiAssistantPreferences;
        providerCredentialAvailable: boolean;
        credentialGeneration: number;
      }
    | undefined
  >(undefined);
  const modelsActionBusy = useRef(false);
  const modelsActionOwner = useAsyncGeneration();
  const testActionBusy = useRef(false);
  const testActionOwner = useAsyncGeneration();
  const origin = aiCredentialOrigin(ai.endpoint);
  // 不鉴权的服务（Ollama）不读这个地址下存过的 Token，请求带空 Token，宿主据此不发 Authorization。
  const usesToken = aiProviderUsesToken(ai.provider);
  const token = origin && usesToken ? (ai.tokens?.[origin] ?? "") : "";

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
    const credentialVersion = credentialGenerationRef?.current;
    const owner = ++ownerRef.current;
    busyRef.current = true;
    await runAsyncAction(
      {
        busy: false,
        isCurrent: () =>
          generation === requestGeneration.current &&
          owner === ownerRef.current &&
          credentialVersion === credentialGenerationRef?.current,
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
    const previous = previousOwner.current;
    previousOwner.current = { client, ai, providerCredentialAvailable, credentialGeneration };
    // 保存回执会返回同值副本；只有语义配置或凭据代次变化才使请求失效。
    if (
      previous &&
      previous.client === client &&
      previous.providerCredentialAvailable === providerCredentialAvailable &&
      previous.credentialGeneration === credentialGeneration &&
      deepEqual(previous.ai, ai)
    )
      return;
    requestGeneration.current += 1;
    modelsActionOwner.current += 1;
    modelsActionBusy.current = false;
    testActionOwner.current += 1;
    testActionBusy.current = false;
    setModelsBusy(false);
    setTestOutput("");
    setTestStatus("");
    setTestBusy(false);
  }, [client, ai, providerCredentialAvailable, credentialGeneration]);

  // 自动选模也会替换配置，但模型目录来源不变时保留刚获取的列表。
  useEffect(() => {
    setModels(null);
    setModelsStatus("");
  }, [client, ai.endpoint, ai.provider, token, providerCredentialAvailable, credentialGeneration]);

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
      setModelsStatus(aiEndpointHint(ai.endpoint));
      return;
    }
    if (usesToken && !providerCredentialAvailable && !token.trim()) {
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
      (cause) => exceptionMessage(cause, "获取模型失败，请检查地址、密钥和网络。"),
    );
  };

  const test = async () => {
    if (!client) return;
    if (!testInput.trim()) {
      setTestStatus("请先输入待润色文字。");
      return;
    }
    if (!origin) {
      setTestStatus(aiEndpointHint(ai.endpoint));
      return;
    }
    if (usesToken && !providerCredentialAvailable && !token.trim()) {
      setTestStatus("请先填写 API Token。");
      return;
    }
    if (testActionBusy.current || testBusy) return;
    const generation = requestGeneration.current;
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
      (cause) => exceptionMessage(cause, "AI 请求失败，请检查地址、模型、密钥和网络。"),
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
