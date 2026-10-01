import { useEffect, useRef, useState, type ReactNode } from "react";
import { runAsyncAction } from "../core/async-action";
import { CredentialTestSection, type CredentialTestState } from "./credential-test-section";
import { providerCredentialErrorMessage } from "./credential-utils";
import type {
  ApiCredentialTestResult,
  ApiCredentialTestService,
  ProviderCredentialClient,
  ProviderCredentialStatus,
  VoiceCredentialKind,
  VoiceCredentialSaveResult,
} from "../index";

export interface ProviderCredentialsHost {
  providerCredentials?: ProviderCredentialClient;
  testApiCredential?: (
    service: ApiCredentialTestService,
    config: Record<string, unknown>,
  ) => Promise<ApiCredentialTestResult>;
}

export interface ProviderCredentialInput {
  token: string;
  appKey: string;
  endpoint?: string;
}

export interface TencentCredentialInput {
  secretId: string;
  secretKey: string;
  region?: string;
}

export type ProviderCredentialBusy = "ai" | "tencent" | VoiceCredentialKind;
export type ProviderCredentialMessage = { ok: boolean; text: string };

export interface UseProviderCredentialsOptions {
  client: ProviderCredentialsHost;
}

export function useProviderCredentials({ client }: UseProviderCredentialsOptions) {
  const [credentialTests, setCredentialTests] = useState<
    Partial<
      Record<
        ApiCredentialTestService,
        { signature: string; busy: boolean; ok?: boolean; message: string }
      >
    >
  >({});
  const credentialTestGeneration = useRef<Partial<Record<ApiCredentialTestService, number>>>({});
  const [providerCredentials, setProviderCredentials] = useState<ProviderCredentialStatus>();
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
  const clientGeneration = useRef(0);

  const updateTencentCredentialInput = (patch: Partial<TencentCredentialInput>) =>
    setTencentCredentialInput((current) => ({ ...current, ...patch }));

  useEffect(() => {
    const generation = ++clientGeneration.current;
    credentialTestGeneration.current = {};
    setCredentialTests((current) => (Object.keys(current).length ? {} : current));
    setProviderCredentialBusy(undefined);
    setProviderCredentialMessages((current) => (Object.keys(current).length ? {} : current));
    const credentials = client.providerCredentials;
    if (!credentials)
      return () => {
        if (clientGeneration.current === generation) clientGeneration.current++;
      };
    let active = true;
    void credentials
      .status()
      .then((status) => {
        if (active) setProviderCredentials(status);
      })
      .catch(() => undefined);
    return () => {
      active = false;
      if (clientGeneration.current === generation) clientGeneration.current++;
    };
  }, [client.providerCredentials, client.testApiCredential]);

  const runCredentialTest = async (
    service: ApiCredentialTestService,
    config: Record<string, unknown>,
  ) => {
    if (!client.testApiCredential) return;
    if (credentialTests[service]?.busy) return;
    const clientVersion = clientGeneration.current;
    const signature = JSON.stringify(config);
    const generation = (credentialTestGeneration.current[service] ?? 0) + 1;
    credentialTestGeneration.current[service] = generation;
    const update = (patch: { busy: boolean; ok?: boolean; message: string }) =>
      setCredentialTests((current) => ({
        ...current,
        [service]: { signature, ...patch },
      }));
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
  };

  async function runCredentialSave<T>(
    kind: ProviderCredentialBusy,
    operation: (credentials: ProviderCredentialClient) => Promise<T>,
    onSuccess: (result: T) => void,
  ) {
    const credentials = client.providerCredentials;
    if (!credentials || providerCredentialBusy === kind) return;
    const clientVersion = clientGeneration.current;
    await runAsyncAction(
      {
        busy: providerCredentialBusy === kind,
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
        if (isCurrent()) onSuccess(result);
      },
      { formatError: providerCredentialErrorMessage },
    );
  }

  const runProviderCredential = async (
    kind: "ai" | "tencent",
    operation: (credentials: ProviderCredentialClient) => Promise<ProviderCredentialStatus>,
    success: string,
  ) => {
    await runCredentialSave(kind, operation, (result) => {
      setProviderCredentials(result);
      if (kind === "ai") setAiCredentialInput("");
      else setTencentCredentialInput({ secretId: "", secretKey: "", region: undefined });
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
    await runCredentialSave(kind, operation, (result) => {
      setProviderCredentials(result.status);
      setVoiceCredentialInput((current) => ({ ...current, [kind]: { token: "", appKey: "" } }));
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
      state={credentialTests[service] as CredentialTestState | undefined}
      disabled={disabled}
      available={Boolean(client.testApiCredential)}
      onTest={() => void runCredentialTest(service, config)}
    />
  );

  return {
    credentialTests,
    providerCredentials,
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
