// @vitest-environment jsdom
import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import {
  useVoiceCredentialEditor,
  voiceCredentialEditorModel,
  voiceCredentialSavePayload,
  DoubaoStreamEndpointSelect,
  type UseVoiceCredentialEditorOptions,
  type VoiceCredentialStatus,
} from "@msime/ui";

afterEach(cleanup);

const base = {
  kind: "asr" as const,
  provider: "doubao",
  model: "",
  input: { token: "", appKey: "" },
  busy: false,
};
const stored: VoiceCredentialStatus = {
  voiceAsr: [
    {
      provider: "doubao",
      endpoint: "wss://voice.example.test",
      model: "model-a",
      resourceId: "resource-a",
      authMode: "legacy",
    },
  ],
  voicePolish: [],
  voiceInvalid: false,
};

test("纯模型复用旧凭据、提示差异并支持显式清空接口地址", () => {
  const model = voiceCredentialEditorModel({
    ...base,
    credentials: stored,
    authMode: "legacy",
    model: " model-b ",
    resourceId: "resource-a",
  });
  expect(model.endpoint).toBe("wss://voice.example.test");
  expect(model.status).toContain("不一致");
  expect(model.saveDisabled).toBe(false);
  expect(model.tokenLabel).toBe("识别 API Token");
  expect(
    voiceCredentialEditorModel({
      ...base,
      credentials: stored,
      input: { ...base.input, endpoint: "" },
    }).endpoint,
  ).toBe("");
  expect(
    voiceCredentialEditorModel({ ...base, credentials: { ...stored, voiceInvalid: true } }).status,
  ).toContain("无效");
  expect(typeof DoubaoStreamEndpointSelect).toBe("function");
});

test("保存组装只携带本次非空凭据，非豆包不携带豆包绑定", () => {
  expect(
    voiceCredentialSavePayload({
      ...base,
      credentials: stored,
      authMode: "legacy",
      model: "m",
      resourceId: "r",
      input: { token: " synthetic-token ", appKey: " synthetic-key " },
    }),
  ).toEqual({
    kind: "asr",
    provider: "doubao",
    endpoint: "wss://voice.example.test",
    model: "m",
    resourceId: "r",
    authMode: "legacy",
    token: " synthetic-token ",
    appKey: " synthetic-key ",
  });
  expect(
    voiceCredentialSavePayload({
      ...base,
      kind: "polish",
      provider: "siliconflow",
      authMode: "legacy",
      resourceId: "ignored",
      input: { token: " ", appKey: "ignored" },
    }),
  ).toEqual({
    kind: "polish",
    provider: "siliconflow",
    endpoint: "",
    model: "",
    resourceId: "",
    authMode: "",
  });
});

test("新凭据和旧版豆包鉴权各自守住保存条件，忙碌时保存清除均不写入", () => {
  const onSave = vi.fn();
  const onClear = vi.fn();
  const initialProps: UseVoiceCredentialEditorOptions = {
    ...base,
    onChange: vi.fn(),
    onSave,
    onClear,
  };
  const hook = renderHook(
    (options: UseVoiceCredentialEditorOptions) => useVoiceCredentialEditor(options),
    { initialProps },
  );
  act(() => {
    hook.result.current.save();
    hook.result.current.clear();
  });
  expect(onSave).not.toHaveBeenCalled();
  expect(onClear).not.toHaveBeenCalled();
  hook.rerender({
    ...initialProps,
    authMode: "legacy",
    input: { token: "synthetic-token", appKey: "" },
  });
  act(() => hook.result.current.save());
  expect(onSave).not.toHaveBeenCalled();
  hook.rerender({ ...initialProps, credentials: stored, authMode: "legacy", busy: true });
  act(() => {
    hook.result.current.save();
    hook.result.current.clear();
  });
  expect(onSave).not.toHaveBeenCalled();
  expect(onClear).not.toHaveBeenCalled();
  hook.rerender({ ...initialProps, credentials: stored, authMode: "legacy" });
  act(() => {
    hook.result.current.save();
    hook.result.current.clear();
  });
  expect(onSave).toHaveBeenCalledOnce();
  expect(onClear).toHaveBeenCalledOnce();
});

test("公共状态转发字段编辑，自定义地址输入即使成为预设也保持展开", () => {
  const onChange = vi.fn();
  const initialProps: UseVoiceCredentialEditorOptions = {
    ...base,
    onChange,
    onSave: vi.fn(),
    onClear: vi.fn(),
  };
  const hook = renderHook(
    (options: UseVoiceCredentialEditorOptions) => useVoiceCredentialEditor(options),
    { initialProps },
  );
  expect(hook.result.current.customEndpoint).toBe(false);
  act(() => hook.result.current.setCustomEndpoint(true));
  expect(onChange).not.toHaveBeenCalled();
  act(() =>
    hook.result.current.editEndpoint("wss://openspeech.bytedance.com/api/v3/sauc/bigmodel_async"),
  );
  expect(onChange).toHaveBeenLastCalledWith({
    endpoint: "wss://openspeech.bytedance.com/api/v3/sauc/bigmodel_async",
  });
  hook.rerender({
    ...initialProps,
    input: { ...base.input, endpoint: "wss://openspeech.bytedance.com/api/v3/sauc/bigmodel_async" },
  });
  expect(hook.result.current.customEndpoint).toBe(true);
  act(() => hook.result.current.setCustomEndpoint(false));
  act(() =>
    hook.result.current.selectEndpoint(
      "wss://openspeech.bytedance.com/api/v3/sauc/bigmodel_nostream",
    ),
  );
  expect(hook.result.current.customEndpoint).toBe(false);
  act(() => hook.result.current.editToken("synthetic-token"));
  expect(onChange).toHaveBeenLastCalledWith({ token: "synthetic-token" });
  act(() => hook.result.current.editAppKey("synthetic-key"));
  expect(onChange).toHaveBeenLastCalledWith({ appKey: "synthetic-key" });
});

test("同服务的模型与回调变化保留展开，服务或凭据类别变化重置", () => {
  const initialProps: UseVoiceCredentialEditorOptions = {
    ...base,
    onChange: vi.fn(),
    onSave: vi.fn(),
    onClear: vi.fn(),
  };
  const hook = renderHook(
    (options: UseVoiceCredentialEditorOptions) => useVoiceCredentialEditor(options),
    { initialProps },
  );
  act(() => hook.result.current.setCustomEndpoint(true));
  hook.rerender({ ...initialProps, model: "updated", onChange: vi.fn() });
  expect(hook.result.current.customEndpoint).toBe(true);
  hook.rerender({ ...initialProps, provider: "openai" });
  hook.rerender(initialProps);
  expect(hook.result.current.customEndpoint).toBe(false);
  act(() => hook.result.current.setCustomEndpoint(true));
  hook.rerender({ ...initialProps, kind: "polish" });
  hook.rerender(initialProps);
  expect(hook.result.current.customEndpoint).toBe(false);
});

test("保存使用当前输入和当前回调，留空不把已有凭据重新提交", () => {
  const previousSave = vi.fn();
  const currentSave = vi.fn();
  const initialProps: UseVoiceCredentialEditorOptions = {
    ...base,
    credentials: stored,
    authMode: "legacy",
    onChange: vi.fn(),
    onSave: previousSave,
    onClear: vi.fn(),
  };
  const hook = renderHook(
    (options: UseVoiceCredentialEditorOptions) => useVoiceCredentialEditor(options),
    { initialProps },
  );
  hook.rerender({ ...initialProps, model: "new-model", onSave: currentSave });
  act(() => hook.result.current.save());
  expect(previousSave).not.toHaveBeenCalled();
  expect(currentSave).toHaveBeenCalledExactlyOnceWith({
    kind: "asr",
    provider: "doubao",
    endpoint: "wss://voice.example.test",
    model: "new-model",
    resourceId: "",
    authMode: "legacy",
  });
});
