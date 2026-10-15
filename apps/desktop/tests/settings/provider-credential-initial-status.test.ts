// @vitest-environment jsdom
import { act, renderHook } from "@testing-library/react";
import { expect, test, vi } from "vitest";
import {
  useProviderCredentials,
  type ProviderCredentialClient,
  type ProviderCredentialStatus,
} from "@msime/ui";

const emptyStatus: ProviderCredentialStatus = {
  ai: [],
  aiInvalid: false,
  tencent: null,
  tencentInvalid: false,
  voiceAsr: [],
  voicePolish: [],
  voiceInvalid: false,
};

const populatedStatus: ProviderCredentialStatus = {
  ...emptyStatus,
  ai: [{ provider: "synthetic-ai", endpoint: "https://ai.example.test", model: "model-a" }],
  tencent: { region: "synthetic-region" },
  voiceAsr: [
    {
      provider: "synthetic-voice",
      endpoint: "https://voice.example.test",
      model: "model-a",
      resourceId: null,
      authMode: null,
    },
  ],
  voicePolish: [],
};

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((accept) => {
    resolve = accept;
  });
  return { promise, resolve };
}

function setup() {
  const initialStatus = deferred<ProviderCredentialStatus>();
  const credentials = {
    status: vi.fn(() => initialStatus.promise),
  } as unknown as ProviderCredentialClient;
  const hook = renderHook(() =>
    useProviderCredentials({ client: { providerCredentials: credentials } }),
  );
  return { ...hook, initialStatus };
}

test.each(["ai", "tencent", "asr", "polish"] as const)(
  "%s 保存成功后，迟到的初始状态不能抹掉新状态",
  async (kind) => {
    const { result, initialStatus } = setup();
    await act(async () => {
      if (kind === "ai" || kind === "tencent") {
        await result.current.runProviderCredential(kind, async () => populatedStatus, "已保存");
      } else {
        await result.current.runVoiceCredential(
          kind,
          async () => ({ status: populatedStatus, serviceUpdated: true }),
          "已保存",
        );
      }
    });
    expect(result.current.providerCredentials).toEqual(populatedStatus);

    await act(async () => {
      initialStatus.resolve(emptyStatus);
      await initialStatus.promise;
    });

    expect(result.current.providerCredentials).toEqual(populatedStatus);
    expect(result.current.providerCredentialMessages[kind]).toEqual({ ok: true, text: "已保存" });
  },
);

test.each(["ai", "tencent", "asr", "polish"] as const)(
  "%s 清除成功后，迟到的初始状态不能恢复旧状态",
  async (kind) => {
    const { result, initialStatus } = setup();
    await act(async () => {
      if (kind === "ai" || kind === "tencent") {
        await result.current.runProviderCredential(kind, async () => emptyStatus, "已清除");
      } else {
        await result.current.runVoiceCredential(
          kind,
          async () => ({ status: emptyStatus, serviceUpdated: true }),
          "已清除",
        );
      }
    });

    await act(async () => {
      initialStatus.resolve(populatedStatus);
      await initialStatus.promise;
    });

    expect(result.current.providerCredentials).toEqual(emptyStatus);
    expect(result.current.providerCredentialMessages[kind]).toEqual({ ok: true, text: "已清除" });
  },
);

test("保存失败后，初始状态仍能正常加载", async () => {
  const { result, initialStatus } = setup();
  await act(async () => {
    await result.current.runProviderCredential(
      "ai",
      async () => {
        throw new Error("synthetic failure");
      },
      "已保存",
    );
    initialStatus.resolve(populatedStatus);
    await initialStatus.promise;
  });

  expect(result.current.providerCredentials).toEqual(populatedStatus);
  expect(result.current.providerCredentialMessages.ai?.ok).toBe(false);
});

test("初始状态在保存途中返回时先显示初始值，保存成功后显示新值", async () => {
  const { result, initialStatus } = setup();
  const saved = deferred<ProviderCredentialStatus>();
  let operation!: Promise<void>;
  act(() => {
    operation = result.current.runProviderCredential("ai", () => saved.promise, "已保存");
  });
  await act(async () => {
    initialStatus.resolve(emptyStatus);
    await initialStatus.promise;
  });
  expect(result.current.providerCredentials).toEqual(emptyStatus);

  await act(async () => {
    saved.resolve(populatedStatus);
    await operation;
  });
  expect(result.current.providerCredentials).toEqual(populatedStatus);
});
