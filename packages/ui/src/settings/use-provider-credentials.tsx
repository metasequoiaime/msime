import type {
  ProviderCredentialClient,
  ProviderCredentialStatus,
  VoiceCredentialKind,
  VoiceCredentialSaveResult,
} from "../core/provider-credential-contracts";
import { useEffect, useRef, useState, type ReactNode } from "react";
import { runAsyncAction } from "../core/async-action";
import { CredentialTestSection } from "./credential-test-section";
import type { CredentialTestState } from "./credential-test-state";
import type { CredentialStatusMessageValue } from "./credential-status-message-model";
import type { ProviderCredentialInput } from "./provider-credential-input";
import type { TencentCredentialInput } from "./tencent-credential-model";
import { providerCredentialErrorMessage } from "./credential-utils";
import { useAsyncGeneration } from "./use-async-generation";
import type {
  ApiCredentialTestResult,
  ApiCredentialTestService,
} from "../core/api-credential-test";

export type { ProviderCredentialInput } from "./provider-credential-input";
export type { TencentCredentialInput } from "./tencent-credential-model";

export interface ProviderCredentialsHost {
  providerCredentials?: ProviderCredentialClient;
  testApiCredential?: (
    service: ApiCredentialTestService,
    config: Record<string, unknown>,
  ) => Promise<ApiCredentialTestResult>;
}

export type ProviderCredentialBusy = "ai" | "tencent" | VoiceCredentialKind;
export type ProviderCredentialMessage = CredentialStatusMessageValue;

export interface UseProviderCredentialsOptions {
  client: ProviderCredentialsHost;
}

export function useProviderCredentials({ client }: UseProviderCredentialsOptions) {
  const [credentialTests, setCredentialTests] = useState<
    Partial<Record<ApiCredentialTestService, CredentialTestState>>
  >({});
  const credentialTestGeneration = useRef<Partial<Record<ApiCredentialTestService, number>>>({});
  const [providerCredentials, setProviderCredentials] = useState<ProviderCredentialStatus>();
  const credentialStatusGeneration = useRef(0);
  const [aiCredentialGeneration, setAiCredentialGeneration] = useState(0);
  const aiCredentialGenerationRef = useRef(0);
  const advanceAiCredentialGeneration = () => {
    aiCredentialGenerationRef.current += 1;
    setAiCredentialGeneration(aiCredentialGenerationRef.current);
  };
  const [aiCredentialInput, setAiCredentialInput] = useState("");
  const [tencentCredentialInput, setTencentCredentialInput] = useState<TencentCredentialInput>({
    secretId: "",
    secretKey: "",
    region: undefined as string | undefined,
  });
  const [voiceCredentialInput, setVoiceCredentialInput] = useState<
    Record<VoiceCredentialKind, ProviderCredentialInput>
  >({ asr: { token: "", appKey: "" }, polish: { token: "", appKey: "" } });
  const [providerCredentialBusy, setProviderCredentialBusy] = useState<ProviderCredentialBusy>();
  const [providerCredentialMessages, setProviderCredentialMessages] = useState<
    Partial<Record<ProviderCredentialBusy, ProviderCredentialMessage>>
  >({});
  const credentialSaveRunning = useRef(false);
  const credentialTestRunning = useRef<Partial<Record<ApiCredentialTestService, number>>>({});
  const credentialTestOwner = useAsyncGeneration();
  const clientGeneration = useAsyncGeneration(client.providerCredentials, client.testApiCredential);

  const updateTencentCredentialInput = (patch: Partial<TencentCredentialInput>) =>
    setTencentCredentialInput((current) => ({ ...current, ...patch }));

  useEffect(() => {
    const generation = clientGeneration.current;
    const statusGeneration = credentialStatusGeneration.current;
    credentialTestGeneration.current = {};
    credentialTestRunning.current = {};
    credentialTestOwner.current++;
    credentialSaveRunning.current = false;
    setProviderCredentials(undefined);
    advanceAiCredentialGeneration();
    setCredentialTests((current) => (Object.keys(current).length ? {} : current));
    setProviderCredentialBusy(undefined);
    setProviderCredentialMessages((current) => (Object.keys(current).length ? {} : current));
    const credentials = client.providerCredentials;
    if (!credentials) return;
    void credentials
      .status()
      .then((status) => {
        if (
          clientGeneration.current === generation &&
          credentialStatusGeneration.current === statusGeneration
        )
          setProviderCredentials(status);
      })
      .catch(() => undefined);
  }, [client.providerCredentials, client.testApiCredential]);

  const runCredentialTest = async (
    service: ApiCredentialTestService,
    config: Record<string, unknown>,
  ) => {
    if (!client.testApiCredential) return;
    if (credentialTests[service]?.busy || credentialTestRunning.current[service] !== undefined)
      return;
    const clientVersion = clientGeneration.current;
    const owner = ++credentialTestOwner.current;
    credentialTestRunning.current[service] = owner;
    const signature = JSON.stringify(config);
    const generation = (credentialTestGeneration.current[service] ?? 0) + 1;
    credentialTestGeneration.current[service] = generation;
    const update = (patch: { busy: boolean; ok?: boolean; message: string }) =>
      setCredentialTests((current) => ({
        ...current,
        [service]: { signature, ...patch },
      }));
    try {
      await runAsyncAction(
        {
          busy: false,
          isCurrent: () =>
            clientGeneration.current === clientVersion &&
            credentialTestGeneration.current[service] === generation,
          setBusy: (busy) =>
            setCredentialTests((current) => ({
              ...current,
              [service]: {
                ...(current[service]?.signature === signature ? current[service] : {}),
                signature,
                busy,
              },
            })),
          setError: (message) =>
            update(message ? { busy: true, ok: false, message } : { busy: true, message }),
        },
        async (isCurrent) => {
          const result = await client.testApiCredential!(service, config);
          if (!isCurrent()) return;
          update({ busy: true, ...result });
        },
        { formatError: () => "无法连接 provider，请确认服务已启动。" },
      );
    } finally {
      if (credentialTestRunning.current[service] === owner)
        delete credentialTestRunning.current[service];
    }
  };

  async function runCredentialSave<T>(
    kind: ProviderCredentialBusy,
    operation: (credentials: ProviderCredentialClient) => Promise<T>,
    onSuccess: (result: T) => void,
  ) {
    const credentials = client.providerCredentials;
    if (!credentials || credentialSaveRunning.current) return;
    const clientVersion = clientGeneration.current;
    credentialSaveRunning.current = true;
    try {
      await runAsyncAction(
        {
          busy: false,
          isCurrent: () => clientGeneration.current === clientVersion,
          setBusy: (busy) => setProviderCredentialBusy(busy ? kind : undefined),
          setError: (message) =>
            setProviderCredentialMessages((current) => ({
              ...current,
              [kind]: message ? { ok: false, text: message } : undefined,
            })),
        },
        async (isCurrent) => {
          const result = await operation(credentials);
          if (isCurrent()) {
            credentialStatusGeneration.current += 1;
            onSuccess(result);
          }
        },
        { formatError: providerCredentialErrorMessage },
      );
    } finally {
      if (clientVersion === clientGeneration.current) credentialSaveRunning.current = false;
    }
  }

  const runProviderCredential = async (
    kind: "ai" | "tencent",
    operation: (credentials: ProviderCredentialClient) => Promise<ProviderCredentialStatus>,
    success: string,
  ) => {
    const aiInput = aiCredentialInput;
    const tencentInput = tencentCredentialInput;
    await runCredentialSave(kind, operation, (result) => {
      setProviderCredentials(result);
      // 只清空操作开始时的字段，保留等待回执期间继续输入的内容。
      if (kind === "ai") {
        setAiCredentialInput((current) => (current === aiInput ? "" : current));
        advanceAiCredentialGeneration();
      } else
        setTencentCredentialInput((current) => ({
          secretId: current.secretId === tencentInput.secretId ? "" : current.secretId,
          secretKey: current.secretKey === tencentInput.secretKey ? "" : current.secretKey,
          region: current.region === tencentInput.region ? undefined : current.region,
        }));
      setProviderCredentialMessages((current) => ({
        ...current,
        [kind]: { ok: true, text: success },
      }));
    });
  };

  const runVoiceCredential = async (
    kind: VoiceCredentialKind,
    operation: (credentials: ProviderCredentialClient) => Promise<VoiceCredentialSaveResult>,
    success: string,
  ) => {
    const input = voiceCredentialInput[kind];
    await runCredentialSave(kind, operation, (result) => {
      setProviderCredentials(result.status);
      setVoiceCredentialInput((current) => ({
        ...current,
        [kind]: {
          token: current[kind].token === input.token ? "" : current[kind].token,
          appKey: current[kind].appKey === input.appKey ? "" : current[kind].appKey,
          ...(current[kind].endpoint === input.endpoint
            ? {}
            : { endpoint: current[kind].endpoint }),
        },
      }));
      setProviderCredentialMessages((current) => ({
        ...current,
        [kind]: result.serviceUpdated
          ? { ok: true, text: success }
          : {
              ok: false,
              text: `${success}但未能更新语音服务，请运行 systemctl --user enable --now msime-linux-voice.socket。`,
            },
      }));
    });
  };

  const credentialTestControl = (
    service: ApiCredentialTestService,
    label: string,
    config: Record<string, unknown>,
    disabled = false,
  ): ReactNode => (
    <CredentialTestSection
      label={label}
      config={config}
      state={credentialTests[service]}
      disabled={disabled}
      available={Boolean(client.testApiCredential)}
      onTest={() => void runCredentialTest(service, config)}
    />
  );

  return {
    credentialTests,
    providerCredentials,
    aiCredentialGeneration,
    aiCredentialGenerationRef,
    aiCredentialInput,
    setAiCredentialInput,
    tencentCredentialInput,
    setTencentCredentialInput,
    updateTencentCredentialInput,
    voiceCredentialInput,
    setVoiceCredentialInput,
    providerCredentialBusy,
    providerCredentialMessages,
    runCredentialTest,
    runProviderCredential,
    runVoiceCredential,
    credentialTestControl,
  };
}

export type VoiceCredentialBindings = Pick<
  ReturnType<typeof useProviderCredentials>,
  | "voiceCredentialInput"
  | "setVoiceCredentialInput"
  | "providerCredentialBusy"
  | "providerCredentialMessages"
  | "runVoiceCredential"
>;
