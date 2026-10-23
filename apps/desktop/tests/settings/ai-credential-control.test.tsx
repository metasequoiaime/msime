// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import {
  AiCredentialControl,
  type AiCredentialControlProps,
} from "../../../../packages/ui/src/settings/ai-credential-control";
import { useProviderCredentials } from "../../../../packages/ui/src/settings/use-provider-credentials";
import type {
  ProviderCredentialClient,
  ProviderCredentialStatus,
} from "../../../../packages/ui/src/core/provider-credential-contracts";

const ai = {
  provider: "openai",
  endpoint: "https://ai.example.test/v1",
  model: "synthetic-model",
};
const status: ProviderCredentialStatus = {
  ai: [],
  aiInvalid: false,
  tencent: null,
  tencentInvalid: false,
  voiceAsr: [],
  voicePolish: [],
  voiceInvalid: false,
};

function props(overrides: Partial<AiCredentialControlProps> = {}): AiCredentialControlProps {
  return {
    ai,
    aiOrigin: "https://ai.example.test",
    aiToken: "synthetic-direct-token",
    updateAiToken: vi.fn(),
    linuxPlatform: true,
    providerCredentialAvailable: true,
    aiCredentialInput: "",
    setAiCredentialInput: vi.fn(),
    providerCredentialBusy: undefined,
    providerCredentialMessages: {},
    runProviderCredential: vi.fn(),
    ...overrides,
  };
}

afterEach(cleanup);

test.each([
  { linuxPlatform: true, providerCredentialAvailable: true },
  { linuxPlatform: true, providerCredentialAvailable: false },
  { linuxPlatform: false, providerCredentialAvailable: true },
  { linuxPlatform: false, providerCredentialAvailable: false },
])("不鉴权服务不显示凭据编辑或回退说明：%j", (host) => {
  const { container } = render(
    <AiCredentialControl {...props({ ...host, ai: { ...ai, provider: "ollama" } })} />,
  );
  expect(container.textContent).toBe("");
  expect(screen.queryByLabelText("AI API Token")).toBeNull();
});

test("Linux 缺少凭据能力时只显示现有服务配置说明", () => {
  render(<AiCredentialControl {...props({ providerCredentialAvailable: false })} />);
  expect(screen.getByText("ai-provider.json")).toBeTruthy();
  expect(screen.queryByLabelText("AI API Token")).toBeNull();
});

test.each([true, false])("其他宿主直接编辑 Token，不调用 provider：能力 %s", (available) => {
  const options = props({ linuxPlatform: false, providerCredentialAvailable: available });
  const view = render(<AiCredentialControl {...options} />);
  expect((screen.getByLabelText("AI API Token") as HTMLInputElement).value).toBe(
    "synthetic-direct-token",
  );
  fireEvent.change(screen.getByLabelText("AI API Token"), {
    target: { value: "synthetic-edited-token" },
  });
  expect(options.updateAiToken).toHaveBeenCalledExactlyOnceWith("synthetic-edited-token");
  expect(options.setAiCredentialInput).not.toHaveBeenCalled();
  expect(options.runProviderCredential).not.toHaveBeenCalled();
  view.rerender(<AiCredentialControl {...options} aiOrigin={null} />);
  expect((screen.getByLabelText("AI API Token") as HTMLInputElement).disabled).toBe(true);
});

function ProviderEditor({
  credentials,
  preferences = ai,
}: {
  credentials: ProviderCredentialClient;
  preferences?: typeof ai;
}) {
  const state = useProviderCredentials({ client: { providerCredentials: credentials } });
  return (
    <AiCredentialControl
      {...props()}
      {...state}
      ai={preferences}
      providerCredentials={state.providerCredentials}
      storedAiCredential={state.providerCredentials?.ai.find(
        (stored) => stored.provider === preferences.provider,
      )}
    />
  );
}

test("Linux 独立编辑器保存原文、保留新草稿，留空重新绑定并清除当前服务", async () => {
  let finish!: (value: ProviderCredentialStatus) => void;
  const saved = { ...status, ai: [ai, { ...ai, provider: "deepseek" }] };
  const credentials: ProviderCredentialClient = {
    status: vi.fn().mockResolvedValue(status),
    saveAi: vi
      .fn()
      .mockImplementationOnce(
        () =>
          new Promise<ProviderCredentialStatus>((resolve) => {
            finish = resolve;
          }),
      )
      .mockResolvedValue(saved),
    clearAi: vi.fn().mockResolvedValue(status),
    saveTencent: vi.fn(),
    clearTencent: vi.fn(),
    saveVoice: vi.fn(),
    clearVoice: vi.fn(),
  };
  const view = render(<ProviderEditor credentials={credentials} />);
  await screen.findByText(/尚未保存/);
  fireEvent.change(screen.getByLabelText("AI API Token"), {
    target: { value: " synthetic-provider-token " },
  });
  fireEvent.click(screen.getByRole("button", { name: "保存凭据" }));
  expect(credentials.saveAi).toHaveBeenCalledExactlyOnceWith({
    ...ai,
    token: " synthetic-provider-token ",
  });
  expect((screen.getByRole("button", { name: "保存凭据" }) as HTMLButtonElement).disabled).toBe(
    true,
  );
  fireEvent.change(screen.getByLabelText("AI API Token"), {
    target: { value: "synthetic-new-draft" },
  });
  await act(async () => finish(saved));
  await screen.findByText("凭据已保存，provider 服务下次请求时生效。");
  expect((screen.getByLabelText("AI API Token") as HTMLInputElement).value).toBe(
    "synthetic-new-draft",
  );

  fireEvent.change(screen.getByLabelText("AI API Token"), { target: { value: "  " } });
  const next = { ...ai, model: "synthetic-next-model" };
  view.rerender(<ProviderEditor credentials={credentials} preferences={next} />);
  fireEvent.click(screen.getByRole("button", { name: "保存凭据" }));
  await waitFor(() => expect(credentials.saveAi).toHaveBeenLastCalledWith(next));
  await waitFor(() =>
    expect((screen.getByLabelText("AI API Token") as HTMLInputElement).value).toBe(""),
  );

  view.rerender(
    <ProviderEditor credentials={credentials} preferences={{ ...next, provider: "deepseek" }} />,
  );
  fireEvent.click(screen.getByRole("button", { name: "清除凭据" }));
  await screen.findByText("凭据已清除。");
  expect(credentials.clearAi).toHaveBeenCalledExactlyOnceWith("deepseek");
});
