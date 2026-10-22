// @vitest-environment jsdom
import {
  act,
  cleanup,
  fireEvent,
  render,
  renderHook,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import {
  SettingsPage,
  useAiAssistant,
  useProviderCredentials,
  type ProviderCredentialClient,
  type ProviderCredentialStatus,
  type Snapshot,
} from "@msime/ui";
import { testHost } from "../support/host";
import { saveSettingsNow, settingsFormReady } from "../support/settings-form";

afterEach(cleanup);

const initial: Snapshot = {
  format_version: 1,
  revision: 1,
  preferences: {
    scheme: "quanpin",
    shuangpin_profile: "xiaohe",
    candidate_page_size: 5,
    learning: true,
    chinese_punctuation: true,
    ai_assistant: {
      enabled: true,
      provider: "openai",
      endpoint: "https://ai.example.test/v1",
      model: "fixture-model",
      candidate_limit: 3,
      prompt_custom_1: "",
      prompt_custom_2: "",
      prompt_custom_3: "",
    },
  },
};
const status: ProviderCredentialStatus = {
  ai: [{ provider: "openai", endpoint: "https://ai.example.test/v1", model: "fixture-model" }],
  aiInvalid: false,
  tencent: null,
  tencentInvalid: false,
  voiceAsr: [],
  voicePolish: [],
  voiceInvalid: false,
};

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((accept, fail) => {
    resolve = accept;
    reject = fail;
  });
  return { promise, resolve, reject };
}

test("真实保存回执返回同值 AI 副本时，当前润色请求和结果保持有效", async () => {
  const saving = deferred<Snapshot>();
  const polish = deferred<string>();
  let receipt!: Snapshot;
  const save = vi.fn((_revision, preferences) => {
    receipt = { ...initial, revision: 2, preferences: structuredClone(preferences) };
    return saving.promise;
  });
  render(
    <SettingsPage
      initialPage="ai"
      client={{
        load: async () => initial,
        save,
        // 「AI 润色测试」只在用这份配置润色的触屏宿主上保留（桌面上已经撤掉），所以在 Android 上测。
        host: testHost({ platform: "android" }),
        aiAssistant: { fetchModels: vi.fn(), test: () => polish.promise },
      }}
    />,
  );
  await settingsFormReady();
  fireEvent.change(screen.getByLabelText("AI API Token"), { target: { value: "synthetic-token" } });
  fireEvent.click(screen.getByRole("switch", { name: "启用 AI 辅助" }));
  saveSettingsNow();
  await waitFor(() => expect(save).toHaveBeenCalledOnce());
  fireEvent.change(screen.getByLabelText("AI 测试输入"), { target: { value: "合成测试文字" } });
  fireEvent.click(screen.getByRole("button", { name: "发送并润色" }));
  await act(async () => {
    saving.resolve(receipt);
    await saving.promise;
  });
  expect(screen.getByRole("button", { name: "发送中…" }).hasAttribute("disabled")).toBe(true);
  await act(async () => {
    polish.resolve("有效的合成润色结果");
    await polish.promise;
  });
  expect(await screen.findByText("有效的合成润色结果")).toBeTruthy();
});

test("AI 润色测试结果复制失败时显示可重试的错误", async () => {
  const copyText = vi.fn().mockRejectedValue(new Error("synthetic clipboard failure"));
  render(
    <SettingsPage
      initialPage="ai"
      client={{
        load: async () => initial,
        save: vi.fn(),
        copyText,
        host: testHost({ platform: "android" }),
        aiAssistant: {
          fetchModels: vi.fn(),
          test: vi.fn().mockResolvedValue("可复制的合成结果"),
        },
      }}
    />,
  );
  await settingsFormReady();
  fireEvent.change(screen.getByLabelText("AI API Token"), {
    target: { value: "synthetic-token" },
  });
  fireEvent.click(screen.getByRole("switch", { name: "启用 AI 辅助" }));
  fireEvent.change(screen.getByLabelText("AI 测试输入"), { target: { value: "合成测试文字" } });
  fireEvent.click(screen.getByRole("button", { name: "发送并润色" }));
  await screen.findByText("可复制的合成结果");

  fireEvent.click(screen.getByRole("button", { name: "复制结果" }));

  expect(await screen.findByText("复制 AI 测试结果失败，请重试。")).toBeTruthy();
  expect(copyText).toHaveBeenCalledWith("可复制的合成结果");
});

test.each(["save", "clear"] as const)(
  "Linux AI 凭据 %s 完成后，旧模型列表不能覆盖配置",
  async (operation) => {
    const models = deferred<string[]>();
    const credentials = {
      status: async () => structuredClone(status),
      saveAi: vi.fn().mockResolvedValue(structuredClone(status)),
      clearAi: vi.fn().mockResolvedValue({ ...status, ai: [] }),
    } as unknown as ProviderCredentialClient;
    render(
      <SettingsPage
        initialPage="ai"
        client={{
          load: async () => initial,
          save: vi.fn(),
          providerCredentials: credentials,
          host: testHost({ platform: "linux", ai_provider_credentials: true }),
          aiAssistant: { fetchModels: () => models.promise, test: vi.fn() },
        }}
      />,
    );
    await settingsFormReady();
    const group = await screen.findByRole("group", { name: "AI 凭据" });
    await within(group).findByText("已保存，留空则保留原凭据");
    fireEvent.click(screen.getByRole("button", { name: "获取模型列表" }));
    if (operation === "save") {
      fireEvent.change(within(group).getByLabelText("AI API Token"), {
        target: { value: "synthetic-rotated-key" },
      });
      fireEvent.click(within(group).getByRole("button", { name: "保存凭据" }));
      await within(group).findByText("凭据已保存，provider 服务下次请求时生效。");
    } else {
      fireEvent.click(within(group).getByRole("button", { name: "清除凭据" }));
      await within(group).findByText("凭据已清除。");
    }
    await act(async () => {
      models.resolve(["stale-model"]);
      await models.promise;
    });
    expect((screen.queryByLabelText("AI 模型（自定义）") as HTMLInputElement | null)?.value).toBe(
      "fixture-model",
    );
    expect(screen.queryByText("已获取 1 个可用模型。")).toBeNull();
    expect(screen.getByRole("button", { name: "获取模型列表" }).hasAttribute("disabled")).toBe(
      false,
    );
  },
);

test.each(
  (["save", "clear"] as const).flatMap((operation) =>
    (["models", "polish"] as const).flatMap((action) =>
      (["resolve", "reject"] as const).map((outcome) => ({ operation, action, outcome })),
    ),
  ),
)(
  "AI 凭据 $operation 成功后、React 提交前旧 $action 的 $outcome 必须失效",
  async ({ operation, action, outcome }) => {
    const models = deferred<string[]>();
    const polish = deferred<string>();
    const credentials = {
      providerCredentials: {
        status: async () => structuredClone(status),
      } as ProviderCredentialClient,
    };
    const aiClient = { fetchModels: () => models.promise, test: () => polish.promise };
    const onChange = vi.fn();
    const { result } = renderHook(() => {
      const provider = useProviderCredentials({ client: credentials });
      const assistant = useAiAssistant({
        client: aiClient,
        ai: initial.preferences.ai_assistant!,
        providerCredentialAvailable: true,
        credentialGeneration: provider.aiCredentialGeneration,
        credentialGenerationRef: provider.aiCredentialGenerationRef,
        onChange,
      });
      return { provider, assistant };
    });
    act(() => result.current.assistant.setTestInput("合成测试文字"));
    let pending!: Promise<void>;
    act(() => {
      pending =
        action === "models"
          ? result.current.assistant.fetchModels()
          : result.current.assistant.test();
    });
    await act(async () => {
      await result.current.provider.runProviderCredential(
        "ai",
        async () => (operation === "save" ? structuredClone(status) : { ...status, ai: [] }),
        "合成凭据操作成功",
      );
      // 两个回执在同一批更新中完成，不能依赖下一轮 effect 才使旧请求失效。
      if (outcome === "reject") {
        (action === "models" ? models : polish).reject(new Error("旧凭据的合成错误"));
      } else if (action === "models") models.resolve(["stale-model"]);
      else polish.resolve("旧凭据的合成润色结果");
      await pending;
    });
    expect(onChange).not.toHaveBeenCalled();
    expect(result.current.assistant.models).toBeNull();
    expect(result.current.assistant.modelsStatus).toBe("");
    expect(result.current.assistant.modelsBusy).toBe(false);
    expect(result.current.assistant.testOutput).toBe("");
    expect(result.current.assistant.testStatus).toBe("");
    expect(result.current.assistant.testBusy).toBe(false);
  },
);

test.each(["failed-ai", "tencent"] as const)(
  "%s 凭据操作不会使当前 AI 模型请求失效",
  async (operation) => {
    const models = deferred<string[]>();
    const credentials = {
      providerCredentials: {
        status: async () => structuredClone(status),
      } as ProviderCredentialClient,
    };
    const aiClient = { fetchModels: () => models.promise, test: vi.fn() };
    const { result } = renderHook(() => {
      const provider = useProviderCredentials({ client: credentials });
      const assistant = useAiAssistant({
        client: aiClient,
        ai: initial.preferences.ai_assistant!,
        providerCredentialAvailable: true,
        credentialGeneration: provider.aiCredentialGeneration,
        credentialGenerationRef: provider.aiCredentialGenerationRef,
        onChange: vi.fn(),
      });
      return { provider, assistant };
    });
    let pending!: Promise<void>;
    act(() => {
      pending = result.current.assistant.fetchModels();
    });
    await act(async () => {
      await result.current.provider.runProviderCredential(
        operation === "failed-ai" ? "ai" : "tencent",
        async () => {
          if (operation === "failed-ai") throw new Error("合成保存失败");
          return structuredClone(status);
        },
        "合成凭据操作成功",
      );
      models.resolve(["fixture-model"]);
      await pending;
    });
    expect(result.current.assistant.models).toEqual(["fixture-model"]);
    expect(result.current.assistant.modelsStatus).toBe("已获取 1 个可用模型。");
    expect(result.current.assistant.modelsBusy).toBe(false);
  },
);
