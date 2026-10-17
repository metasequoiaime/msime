// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { VoiceCredentialSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("uses the bidirectional Doubao endpoint when no endpoint is saved", () => {
  render(
    <VoiceCredentialSection
      kind="asr"
      provider="doubao"
      model=""
      input={{ token: "", appKey: "" }}
      busy={false}
      onChange={vi.fn()}
      onSave={vi.fn()}
      onClear={vi.fn()}
    />,
  );

  expect((screen.getByRole("combobox", { name: "流式接口" }) as HTMLSelectElement).value).toBe(
    "async",
  );
  // 预设地址不再另画一行地址输入框：下拉和输入框写的是同一个接口地址。
  expect(screen.queryByLabelText("识别接口地址")).toBeNull();
  expect(screen.getByLabelText("Doubao API Key")).toBeTruthy();
  expect((screen.getByRole("button", { name: "保存识别凭据" }) as HTMLButtonElement).disabled).toBe(
    true,
  );
});

test("Doubao shows the address field only for a custom endpoint", () => {
  const onChange = vi.fn();
  const view = (endpoint?: string) => (
    <VoiceCredentialSection
      kind="asr"
      provider="doubao"
      model=""
      input={{ token: "", appKey: "", ...(endpoint === undefined ? {} : { endpoint }) }}
      busy={false}
      onChange={onChange}
      onSave={vi.fn()}
      onClear={vi.fn()}
    />
  );
  const { rerender } = render(view());
  const stream = screen.getByRole("combobox", { name: "流式接口" }) as HTMLSelectElement;
  // 选「自定义地址」不写地址，只露出输入框；输入框里还是默认的空地址。
  fireEvent.change(stream, { target: { value: "custom" } });
  expect(onChange).not.toHaveBeenCalled();
  expect(stream.value).toBe("custom");
  const address = screen.getByLabelText("识别接口地址") as HTMLInputElement;
  expect(address.value).toBe("");
  fireEvent.change(address, { target: { value: "wss://asr.example.test/stream" } });
  expect(onChange).toHaveBeenLastCalledWith({ endpoint: "wss://asr.example.test/stream" });
  // 打字打出预设地址时输入框也不收起。
  rerender(view("wss://openspeech.bytedance.com/api/v3/sauc/bigmodel_nostream"));
  expect(screen.getByLabelText("识别接口地址")).toBeTruthy();
  // 改回预设就收起。
  fireEvent.change(stream, { target: { value: "nostream" } });
  expect(screen.queryByLabelText("识别接口地址")).toBeNull();
  // 存的地址不是预设时一打开就显示输入框，下拉停在「自定义地址」。
  cleanup();
  render(view("wss://asr.example.test/stream"));
  expect((screen.getByRole("combobox", { name: "流式接口" }) as HTMLSelectElement).value).toBe(
    "custom",
  );
  expect((screen.getByLabelText("识别接口地址") as HTMLInputElement).value).toBe(
    "wss://asr.example.test/stream",
  );
});

test("submits legacy Doubao credentials and forwards endpoint edits", () => {
  const onChange = vi.fn();
  const onSave = vi.fn();
  render(
    <VoiceCredentialSection
      kind="asr"
      provider="doubao"
      model="volc-model"
      resourceId="volc.resource"
      authMode="legacy"
      input={{ token: "synthetic-token", appKey: "synthetic-app-key" }}
      busy={false}
      onChange={onChange}
      onSave={onSave}
      onClear={vi.fn()}
    />,
  );

  fireEvent.change(screen.getByRole("combobox", { name: "流式接口" }), {
    target: { value: "nostream" },
  });
  expect(onChange).toHaveBeenCalledWith({
    endpoint: "wss://openspeech.bytedance.com/api/v3/sauc/bigmodel_nostream",
  });

  fireEvent.click(screen.getByRole("button", { name: "保存识别凭据" }));
  expect(onSave).toHaveBeenCalledWith({
    kind: "asr",
    provider: "doubao",
    endpoint: "",
    model: "volc-model",
    token: "synthetic-token",
    appKey: "synthetic-app-key",
    resourceId: "volc.resource",
    authMode: "legacy",
  });
});

test("shows stored credential mismatches and provides clearing", () => {
  const onClear = vi.fn();
  render(
    <VoiceCredentialSection
      kind="polish"
      provider="siliconflow"
      model="new-model"
      credentials={{
        voiceAsr: [],
        voicePolish: [
          {
            provider: "siliconflow",
            model: "old-model",
            endpoint: "https://voice.example.test",
            resourceId: null,
            authMode: null,
          },
        ],
        voiceInvalid: false,
      }}
      input={{ token: "", appKey: "" }}
      busy={false}
      onChange={vi.fn()}
      onSave={vi.fn()}
      onClear={onClear}
    />,
  );

  expect(
    screen.getByText("已保存的凭据与上方模型或豆包设置不一致；保存后改为绑定当前设置"),
  ).toBeTruthy();
  expect((screen.getByLabelText("润色接口地址") as HTMLInputElement).value).toBe(
    "https://voice.example.test",
  );
  fireEvent.click(screen.getByRole("button", { name: "清除润色凭据" }));
  expect(onClear).toHaveBeenCalledOnce();
});

test("切换服务后不保留上一服务手动展开的自定义接口", () => {
  const props = {
    kind: "asr" as const,
    model: "",
    input: { token: "", appKey: "" },
    busy: false,
    onChange: vi.fn(),
    onSave: vi.fn(),
    onClear: vi.fn(),
  };
  const view = render(<VoiceCredentialSection {...props} provider="doubao" />);
  fireEvent.change(screen.getByRole("combobox", { name: "流式接口" }), {
    target: { value: "custom" },
  });
  expect(screen.getByLabelText("识别接口地址")).toBeTruthy();
  view.rerender(<VoiceCredentialSection {...props} provider="openai" />);
  view.rerender(<VoiceCredentialSection {...props} provider="doubao" />);
  expect(screen.queryByLabelText("识别接口地址")).toBeNull();
  expect((screen.getByRole("combobox", { name: "流式接口" }) as HTMLSelectElement).value).toBe(
    "async",
  );
});
