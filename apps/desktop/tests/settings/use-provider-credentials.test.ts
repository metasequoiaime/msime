// @vitest-environment jsdom
import { act, renderHook, waitFor } from "@testing-library/react";
import { expect, test, vi } from "vitest";
import { useProviderCredentials } from "@msime/ui";

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
