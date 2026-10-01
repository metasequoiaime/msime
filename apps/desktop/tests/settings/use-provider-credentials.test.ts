// @vitest-environment jsdom
import { act, renderHook, waitFor } from "@testing-library/react";
import { expect, test, vi } from "vitest";
import { useProviderCredentials, type ProviderCredentialStatus } from "@msime/ui";

const savedStatus: ProviderCredentialStatus = {
  ai: [],
  aiInvalid: false,
  tencent: null,
  tencentInvalid: false,
  voiceAsr: [],
  voicePolish: [],
  voiceInvalid: false,
};

test("merges successive Tencent credential input patches against the latest state", () => {
  const { result } = renderHook(() => useProviderCredentials({ client: {} }));

  act(() => {
    result.current.updateTencentCredentialInput({ secretId: "synthetic-id" });
    result.current.updateTencentCredentialInput({ secretKey: "synthetic-key" });
  });

  expect(result.current.tencentCredentialInput).toEqual({
    secretId: "synthetic-id",
    secretKey: "synthetic-key",
    region: undefined,
  });
});

test("a credential test already in progress ignores a second trigger", async () => {
  let resolve!: (value: { ok: boolean; message: string }) => void;
  const pending = new Promise<{ ok: boolean; message: string }>((accept) => {
    resolve = accept;
  });
  const testApiCredential = vi.fn().mockReturnValue(pending);
  const { result } = renderHook(() => useProviderCredentials({ client: { testApiCredential } }));

  let first!: Promise<void>;
  act(() => {
    first = result.current.runCredentialTest("ai.assistant", {
      endpoint: "https://ai.example.test",
    });
  });
  await waitFor(() => expect(result.current.credentialTests["ai.assistant"]?.busy).toBe(true));

  let second!: Promise<void>;
  act(() => {
    second = result.current.runCredentialTest("ai.assistant", {
      endpoint: "https://ai.example.test",
    });
  });
  expect(testApiCredential).toHaveBeenCalledOnce();

  resolve({ ok: true, message: "连接成功" });
  await act(async () => {
    await first;
    await second;
  });
  expect(result.current.credentialTests["ai.assistant"]).toMatchObject({
    busy: false,
    ok: true,
    message: "连接成功",
  });
});

test("a credential save from a replaced client is ignored", async () => {
  let resolve!: (value: never) => void;
  const pending = new Promise<never>((accept) => {
    resolve = accept;
  });
  const oldClient = {
    providerCredentials: { status: vi.fn().mockResolvedValue(undefined) } as never,
  };
  const nextClient = {
    providerCredentials: { status: vi.fn().mockResolvedValue(undefined) } as never,
  };
  const { result, rerender } = renderHook(({ client }) => useProviderCredentials({ client }), {
    initialProps: { client: oldClient },
  });

  let operation!: Promise<void>;
  act(() => {
    operation = result.current.runProviderCredential("ai", async () => pending, "已保存");
  });
  await waitFor(() => expect(result.current.providerCredentialBusy).toBe("ai"));
  rerender({ client: nextClient });
  resolve({} as never);
  await act(async () => operation);

  expect(result.current.providerCredentialBusy).toBeUndefined();
  expect(result.current.providerCredentials).toBeUndefined();
  expect(result.current.providerCredentialMessages.ai).toBeUndefined();
});

test("a provider credential save in progress ignores another save", async () => {
  let resolve!: (value: ProviderCredentialStatus) => void;
  const pending = new Promise<ProviderCredentialStatus>((accept) => {
    resolve = accept;
  });
  const save = vi.fn().mockReturnValue(pending);
  const client = {
    providerCredentials: { status: vi.fn().mockResolvedValue(savedStatus) } as never,
  };
  const { result } = renderHook(() => useProviderCredentials({ client }));

  let first!: Promise<void>;
  act(() => {
    first = result.current.runProviderCredential("ai", save, "已保存");
  });
  await waitFor(() => expect(result.current.providerCredentialBusy).toBe("ai"));

  let second!: Promise<void>;
  act(() => {
    second = result.current.runProviderCredential("ai", save, "已保存");
  });
  expect(save).toHaveBeenCalledOnce();

  resolve(savedStatus);
  await act(async () => {
    await first;
    await second;
  });
  expect(result.current.providerCredentialBusy).toBeUndefined();
  expect(result.current.providerCredentialMessages.ai).toEqual({ ok: true, text: "已保存" });
});

test("a voice credential save in progress ignores another save", async () => {
  let resolve!: (value: { status: ProviderCredentialStatus; serviceUpdated: boolean }) => void;
  const pending = new Promise<{ status: ProviderCredentialStatus; serviceUpdated: boolean }>(
    (accept) => {
      resolve = accept;
    },
  );
  const save = vi.fn().mockReturnValue(pending);
  const client = {
    providerCredentials: { status: vi.fn().mockResolvedValue(savedStatus) } as never,
  };
  const { result } = renderHook(() => useProviderCredentials({ client }));

  let first!: Promise<void>;
  act(() => {
    first = result.current.runVoiceCredential("asr", save, "已保存");
  });
  await waitFor(() => expect(result.current.providerCredentialBusy).toBe("asr"));

  let second!: Promise<void>;
  act(() => {
    second = result.current.runVoiceCredential("asr", save, "已保存");
  });
  expect(save).toHaveBeenCalledOnce();

  resolve({ status: savedStatus, serviceUpdated: false });
  await act(async () => {
    await first;
    await second;
  });
  expect(result.current.providerCredentialBusy).toBeUndefined();
  expect(result.current.providerCredentialMessages.asr).toMatchObject({ ok: false });
});
