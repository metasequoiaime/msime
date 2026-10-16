// @vitest-environment jsdom
import { act, renderHook } from "@testing-library/react";
import { expect, test, vi } from "vitest";
import { useProviderCredentials, type ProviderCredentialStatus } from "@msime/ui";

const status: ProviderCredentialStatus = {
  ai: [],
  aiInvalid: false,
  tencent: null,
  tencentInvalid: false,
  voiceAsr: [],
  voicePolish: [],
  voiceInvalid: false,
};

test.each(["ai", "tencent", "asr", "polish"] as const)(
  "%s 操作成功时保留请求期间输入的新草稿",
  async (kind) => {
    let finish!: () => void;
    const pending = new Promise<void>((resolve) => {
      finish = resolve;
    });
    const client = { providerCredentials: { status: vi.fn().mockResolvedValue(status) } as never };
    const { result } = renderHook(() => useProviderCredentials({ client }));
    act(() => {
      result.current.setAiCredentialInput("synthetic-old-token");
      result.current.setTencentCredentialInput({
        secretId: "synthetic-id",
        secretKey: "synthetic-key",
        region: "old-region",
      });
      result.current.setVoiceCredentialInput({
        asr: {
          token: "synthetic-token",
          appKey: "synthetic-app",
          endpoint: "https://old.example.test",
        },
        polish: { token: "synthetic-token", appKey: "synthetic-app" },
      });
    });
    let operation!: Promise<void>;
    act(() => {
      operation =
        kind === "ai" || kind === "tencent"
          ? result.current.runProviderCredential(
              kind,
              async () => {
                await pending;
                return status;
              },
              "已保存",
            )
          : result.current.runVoiceCredential(
              kind,
              async () => {
                await pending;
                return { status, serviceUpdated: true };
              },
              "已保存",
            );
    });
    await act(async () => {
      // 新编辑与旧回执放进同一批更新，避免依赖渲染或 effect 的时序。
      if (kind === "ai") result.current.setAiCredentialInput("synthetic-new-token");
      else if (kind === "tencent")
        result.current.updateTencentCredentialInput({ region: "new-region" });
      else
        result.current.setVoiceCredentialInput((current) => ({
          ...current,
          [kind]: { ...current[kind], token: "synthetic-new-token" },
        }));
      finish();
      await operation;
    });
    if (kind === "ai") expect(result.current.aiCredentialInput).toBe("synthetic-new-token");
    else if (kind === "tencent")
      expect(result.current.tencentCredentialInput).toEqual({
        secretId: "",
        secretKey: "",
        region: "new-region",
      });
    else {
      expect(result.current.voiceCredentialInput[kind]).toEqual({
        token: "synthetic-new-token",
        appKey: "",
      });
      const other = kind === "asr" ? "polish" : "asr";
      expect(result.current.voiceCredentialInput[other].token).toBe("synthetic-token");
    }
    expect(result.current.providerCredentials).toEqual(status);
    expect(result.current.providerCredentialBusy).toBeUndefined();
  },
);

test.each(["ai", "tencent", "asr", "polish"] as const)(
  "%s 成功后清空未继续编辑的草稿，失败后保留草稿",
  async (kind) => {
    const client = { providerCredentials: { status: vi.fn().mockResolvedValue(status) } as never };
    const { result } = renderHook(() => useProviderCredentials({ client }));
    act(() => {
      result.current.setAiCredentialInput("synthetic-token");
      result.current.setTencentCredentialInput({
        secretId: "synthetic-id",
        secretKey: "synthetic-key",
        region: "region",
      });
      result.current.setVoiceCredentialInput({
        asr: { token: "synthetic-token", appKey: "synthetic-app" },
        polish: { token: "synthetic-token", appKey: "synthetic-app" },
      });
    });
    const run = (fail: boolean) =>
      kind === "ai" || kind === "tencent"
        ? result.current.runProviderCredential(
            kind,
            async () => {
              if (fail) throw new Error("synthetic");
              return status;
            },
            "已保存",
          )
        : result.current.runVoiceCredential(
            kind,
            async () => {
              if (fail) throw new Error("synthetic");
              return { status, serviceUpdated: true };
            },
            "已保存",
          );
    await act(async () => {
      await run(true);
    });
    expect(result.current.aiCredentialInput).toBe("synthetic-token");
    expect(result.current.tencentCredentialInput.secretId).toBe("synthetic-id");
    expect(result.current.voiceCredentialInput.asr.token).toBe("synthetic-token");
    await act(async () => {
      await run(false);
    });
    if (kind === "ai") expect(result.current.aiCredentialInput).toBe("");
    else if (kind === "tencent")
      expect(result.current.tencentCredentialInput).toEqual({
        secretId: "",
        secretKey: "",
        region: undefined,
      });
    else expect(result.current.voiceCredentialInput[kind]).toEqual({ token: "", appKey: "" });
  },
);

test.each(["secretId", "secretKey"] as const)("腾讯保留新编辑的 %s", async (field) => {
  let finish!: () => void;
  const pending = new Promise<void>((resolve) => {
    finish = resolve;
  });
  const client = { providerCredentials: { status: vi.fn().mockResolvedValue(status) } as never };
  const { result } = renderHook(() => useProviderCredentials({ client }));
  act(() =>
    result.current.setTencentCredentialInput({
      secretId: "synthetic-id",
      secretKey: "synthetic-key",
      region: "region",
    }),
  );
  let operation!: Promise<void>;
  act(() => {
    operation = result.current.runProviderCredential(
      "tencent",
      async () => {
        await pending;
        return status;
      },
      "已清除",
    );
  });
  await act(async () => {
    result.current.updateTencentCredentialInput({ [field]: "synthetic-new" });
    finish();
    await operation;
  });
  expect(result.current.tencentCredentialInput).toEqual({
    secretId: "",
    secretKey: "",
    region: undefined,
    [field]: "synthetic-new",
  });
});

test.each(["asr", "polish"] as const)("%s 保留新的 App Key 与接口地址", async (kind) => {
  let finish!: () => void;
  const pending = new Promise<void>((resolve) => {
    finish = resolve;
  });
  const client = { providerCredentials: { status: vi.fn().mockResolvedValue(status) } as never };
  const { result } = renderHook(() => useProviderCredentials({ client }));
  act(() =>
    result.current.setVoiceCredentialInput((current) => ({
      ...current,
      [kind]: {
        token: "synthetic-token",
        appKey: "synthetic-app",
        endpoint: "https://old.example.test",
      },
    })),
  );
  let operation!: Promise<void>;
  act(() => {
    operation = result.current.runVoiceCredential(
      kind,
      async () => {
        await pending;
        return { status, serviceUpdated: false };
      },
      "已清除",
    );
  });
  await act(async () => {
    result.current.setVoiceCredentialInput((current) => ({
      ...current,
      [kind]: {
        ...current[kind],
        appKey: "synthetic-new-app",
        endpoint: "https://new.example.test",
      },
    }));
    finish();
    await operation;
  });
  expect(result.current.voiceCredentialInput[kind]).toEqual({
    token: "",
    appKey: "synthetic-new-app",
    endpoint: "https://new.example.test",
  });
});
