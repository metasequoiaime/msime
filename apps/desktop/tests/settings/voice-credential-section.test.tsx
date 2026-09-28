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
  expect(screen.getByLabelText("Doubao API Key")).toBeTruthy();
  expect((screen.getByRole("button", { name: "保存识别凭据" }) as HTMLButtonElement).disabled).toBe(
    true,
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
