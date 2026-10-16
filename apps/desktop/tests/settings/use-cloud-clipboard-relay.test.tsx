// @vitest-environment jsdom
import { StrictMode, useState } from "react";
import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import {
  CloudClipboardRelaySection,
  cloudClipboardRelayConfigured,
  cloudClipboardRelayTokenIssue,
  cloudClipboardRelayUrlIssue,
  useCloudClipboardRelay,
  withCloudClipboardRelay,
  type CloudClipboardRelayPreferences,
  type UseCloudClipboardRelayOptions,
} from "@msime/ui";

afterEach(cleanup);

const saved = { url: "https://relay.example.test/old", token: "synthetic-relay-token-0001" };
const replacementToken = "synthetic-relay-token-0002";

test("包入口公开中继控件、公共状态和既有纯规则", () => {
  expect(typeof CloudClipboardRelaySection).toBe("function");
  expect(typeof useCloudClipboardRelay).toBe("function");
  expect(cloudClipboardRelayUrlIssue(saved.url)).toBe("");
  expect(cloudClipboardRelayTokenIssue(saved.token)).toBe("");
  expect(cloudClipboardRelayConfigured(undefined)).toBe(false);
  expect(typeof withCloudClipboardRelay).toBe("function");
});

test("初次配置先保留草稿，校验通过后显式保存规范化的地址和令牌", () => {
  const onChange = vi.fn();
  const hook = renderHook(() => useCloudClipboardRelay({ relay: undefined, onChange }));
  expect(hook.result.current.mode).toBe("account");
  expect(hook.result.current.configured).toBe(false);
  expect(hook.result.current.canSave).toBe(false);
  act(() => hook.result.current.setMode("relay"));
  act(() => hook.result.current.save());
  expect(onChange).not.toHaveBeenCalled();
  act(() => hook.result.current.editUrl("http://relay.example.test"));
  act(() => hook.result.current.editToken("short"));
  expect(hook.result.current.urlIssue).toContain("HTTPS");
  expect(hook.result.current.tokenIssue).toContain("16");
  act(() => hook.result.current.save());
  expect(onChange).not.toHaveBeenCalled();
  act(() => hook.result.current.editUrl(" https://relay.example.test/msime "));
  act(() => hook.result.current.editToken(` ${saved.token} `));
  expect(hook.result.current.canSave).toBe(true);
  expect(onChange).not.toHaveBeenCalled();
  act(() => hook.result.current.save());
  expect(onChange).toHaveBeenCalledExactlyOnceWith({
    url: "https://relay.example.test/msime",
    token: saved.token,
  });
});

test("已保存令牌不回填，空输入保留它，只改地址也能保存", () => {
  const onChange = vi.fn();
  const hook = renderHook(() => useCloudClipboardRelay({ relay: saved, onChange }));
  expect(hook.result.current.mode).toBe("relay");
  expect(hook.result.current.savedUrl).toBe(saved.url);
  expect(hook.result.current.hasSavedToken).toBe(true);
  expect(hook.result.current.token).toBe("");
  expect(hook.result.current.tokenIssue).toBe("");
  expect(hook.result.current.canSave).toBe(false);
  act(() => hook.result.current.save());
  expect(onChange).not.toHaveBeenCalled();
  act(() => hook.result.current.editUrl("https://relay.example.test/new"));
  act(() => hook.result.current.editToken("   "));
  expect(hook.result.current.canSave).toBe(true);
  act(() => hook.result.current.save());
  expect(onChange).toHaveBeenCalledExactlyOnceWith({
    url: "https://relay.example.test/new",
    token: saved.token,
  });
});

test("轮换令牌后等待受控保存回填，回填时清空令牌草稿", () => {
  const hook = renderHook(() => {
    const [relay, onChange] = useState<CloudClipboardRelayPreferences | undefined>(saved);
    return useCloudClipboardRelay({ relay, onChange });
  });
  act(() => hook.result.current.editToken(replacementToken));
  expect(hook.result.current.canSave).toBe(true);
  act(() => hook.result.current.save());
  expect(hook.result.current.token).toBe("");
  expect(hook.result.current.hasSavedToken).toBe(true);
  expect(hook.result.current.canSave).toBe(false);
});

test("非法的新令牌不能因为已有令牌而绕过校验", () => {
  const onChange = vi.fn();
  const hook = renderHook(() => useCloudClipboardRelay({ relay: saved, onChange }));
  act(() => hook.result.current.editUrl("https://relay.example.test/new"));
  act(() => hook.result.current.editToken("invalid token with spaces"));
  expect(hook.result.current.tokenIssue).toContain("不能有空格");
  expect(hook.result.current.canSave).toBe(false);
  act(() => hook.result.current.save());
  expect(onChange).not.toHaveBeenCalled();
});

test("切换模式不写入，显式改回账号才清除配置", () => {
  const onChange = vi.fn();
  const hook = renderHook(() => useCloudClipboardRelay({ relay: saved, onChange }));
  act(() => hook.result.current.switchToAccount());
  expect(onChange).not.toHaveBeenCalled();
  act(() => hook.result.current.editUrl("https://relay.example.test/draft"));
  act(() => hook.result.current.setMode("account"));
  expect(hook.result.current.canSave).toBe(false);
  expect(hook.result.current.canSwitchToAccount).toBe(true);
  act(() => hook.result.current.save());
  expect(onChange).not.toHaveBeenCalled();
  act(() => hook.result.current.setMode("relay"));
  expect(hook.result.current.url).toBe("https://relay.example.test/draft");
  act(() => hook.result.current.setMode("account"));
  act(() => hook.result.current.switchToAccount());
  expect(onChange).toHaveBeenCalledExactlyOnceWith(undefined);
});

test("禁用时独立调用保存和改回账号也不会写入，恢复后保留草稿", () => {
  const onChange = vi.fn();
  const hook = renderHook(
    ({ disabled }) => useCloudClipboardRelay({ relay: saved, disabled, onChange }),
    { initialProps: { disabled: false } },
  );
  act(() => hook.result.current.editUrl("https://relay.example.test/new"));
  hook.rerender({ disabled: true });
  expect(hook.result.current.canSave).toBe(false);
  act(() => hook.result.current.save());
  expect(onChange).not.toHaveBeenCalled();
  hook.rerender({ disabled: false });
  expect(hook.result.current.canSave).toBe(true);
  act(() => hook.result.current.setMode("account"));
  hook.rerender({ disabled: true });
  expect(hook.result.current.canSwitchToAccount).toBe(false);
  act(() => hook.result.current.switchToAccount());
  expect(onChange).not.toHaveBeenCalled();
  hook.rerender({ disabled: false });
  act(() => hook.result.current.switchToAccount());
  expect(onChange).toHaveBeenCalledExactlyOnceWith(undefined);
});

test.each(["地址", "令牌", "清除"])("外部%s变更清空旧草稿并重新同步模式", (change) => {
  const onChange = vi.fn();
  const initialProps: UseCloudClipboardRelayOptions = { relay: saved, onChange };
  const hook = renderHook(
    (options: UseCloudClipboardRelayOptions) => useCloudClipboardRelay(options),
    {
      initialProps,
    },
  );
  act(() => hook.result.current.editUrl("https://relay.example.test/draft"));
  act(() => hook.result.current.editToken("synthetic-unsaved-token"));
  act(() => hook.result.current.setMode("account"));
  const relay =
    change === "清除"
      ? undefined
      : {
          ...saved,
          ...(change === "地址"
            ? { url: "https://relay.example.test/external" }
            : { token: replacementToken }),
        };
  hook.rerender({ relay, onChange });
  expect(hook.result.current.url).toBe(relay?.url ?? "");
  expect(hook.result.current.token).toBe("");
  expect(hook.result.current.mode).toBe(relay ? "relay" : "account");
  expect(hook.result.current.hasSavedToken).toBe(Boolean(relay));
  expect(hook.result.current.canSave).toBe(false);
  expect(onChange).not.toHaveBeenCalled();
});

test("同值偏好和回调重建保留本地草稿，保存使用当前回调", () => {
  const previous = vi.fn();
  const current = vi.fn();
  const hook = renderHook(
    (options: UseCloudClipboardRelayOptions) => useCloudClipboardRelay(options),
    { initialProps: { relay: saved, onChange: previous }, wrapper: StrictMode },
  );
  act(() => hook.result.current.editUrl("https://relay.example.test/draft"));
  act(() => hook.result.current.editToken(replacementToken));
  hook.rerender({ relay: { ...saved }, onChange: current });
  expect(hook.result.current.url).toBe("https://relay.example.test/draft");
  expect(hook.result.current.token).toBe(replacementToken);
  act(() => hook.result.current.save());
  expect(previous).not.toHaveBeenCalled();
  expect(current).toHaveBeenCalledExactlyOnceWith({
    url: "https://relay.example.test/draft",
    token: replacementToken,
  });
});
