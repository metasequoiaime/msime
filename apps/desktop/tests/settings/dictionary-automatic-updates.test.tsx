// @vitest-environment jsdom
import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import type { SettingsClient, Snapshot } from "@msime/ui";
import type {
  DictionaryClient,
  DictionaryUpdateStatus,
} from "../../../../packages/ui/src/dictionary/dictionary-contracts";
import { useSettingsPageModel } from "../../../../packages/ui/src/settings/use-settings-page-model";
import { testHost } from "../support/host";

const hour = 60 * 60 * 1000;
const snapshot: Snapshot = {
  format_version: 1,
  revision: 1,
  preferences: {
    scheme: "quanpin",
    shuangpin_profile: "xiaohe",
    candidate_page_size: 5,
    learning: true,
    chinese_punctuation: true,
  },
};
const status: DictionaryUpdateStatus = {
  enabled: true,
  version: "synthetic-v1",
  phase: "idle",
  checked_at: 1,
  downloaded_bytes: 0,
  total_bytes: 0,
  error: null,
  can_rollback: false,
  pending_restart: false,
};

function clientWith(updates?: DictionaryClient["updates"]): SettingsClient {
  return {
    host: testHost({ platform: "linux" }),
    load: async () => snapshot,
    save: async (_revision, preferences) => ({ ...snapshot, preferences }),
    dictionary: {
      list: async () => ({ entries: [], has_more: false }),
      edit: async () => {},
      updates,
    },
  };
}

afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

test("首次、焦点和整小时触发自动检查，焦点和普通重渲染不重置小时边界", async () => {
  vi.useFakeTimers();
  const calls: { owner: DictionaryClient; command: string }[] = [];
  const client = clientWith(async function (this: DictionaryClient, command) {
    calls.push({ owner: this, command });
    return status;
  });
  const view = renderHook(({ client }) => useSettingsPageModel({ client }), {
    initialProps: { client },
  });
  await act(async () => {});
  expect(calls).toEqual([{ owner: client.dictionary, command: "automatic" }]);

  act(() => vi.advanceTimersByTime(hour / 2));
  act(() => window.dispatchEvent(new Event("focus")));
  view.rerender({ client: { ...client } });
  act(() => vi.advanceTimersByTime(hour / 2 - 1));
  expect(calls).toHaveLength(2);
  act(() => vi.advanceTimersByTime(1));
  expect(calls).toHaveLength(3);
  expect(
    calls.every((call) => call.owner === client.dictionary && call.command === "automatic"),
  ).toBe(true);

  view.unmount();
  act(() => {
    window.dispatchEvent(new Event("focus"));
    vi.advanceTimersByTime(hour * 2);
  });
  expect(calls).toHaveLength(3);
});

test("更新入口替换后立即检查，并从替换时刻重新计算小时边界", async () => {
  vi.useFakeTimers();
  const oldUpdates = vi.fn(async () => status);
  const nextUpdates = vi.fn(async () => status);
  const view = renderHook(({ client }) => useSettingsPageModel({ client }), {
    initialProps: { client: clientWith(oldUpdates) },
  });
  await act(async () => {});
  act(() => vi.advanceTimersByTime(hour / 2));
  view.rerender({ client: clientWith(nextUpdates) });
  await act(async () => {});
  expect(oldUpdates).toHaveBeenCalledOnce();
  expect(nextUpdates).toHaveBeenCalledOnce();
  act(() => vi.advanceTimersByTime(hour / 2));
  expect(nextUpdates).toHaveBeenCalledOnce();
  act(() => vi.advanceTimersByTime(hour / 2));
  expect(nextUpdates).toHaveBeenCalledTimes(2);
  act(() => window.dispatchEvent(new Event("focus")));
  expect(oldUpdates).toHaveBeenCalledOnce();
  expect(nextUpdates).toHaveBeenCalledTimes(3);
});

test("更新能力晚到时开始检查，失败不会阻断后续焦点和小时检查", async () => {
  vi.useFakeTimers();
  const view = renderHook(({ client }) => useSettingsPageModel({ client }), {
    initialProps: { client: { ...clientWith(), dictionary: undefined } as SettingsClient },
  });
  await act(async () => {});
  const updates = vi
    .fn()
    .mockRejectedValueOnce(new Error("synthetic offline"))
    .mockResolvedValue(status);
  view.rerender({ client: clientWith(updates) });
  await act(async () => {});
  expect(updates).toHaveBeenCalledOnce();
  await act(async () => {
    window.dispatchEvent(new Event("focus"));
  });
  await act(async () => {
    vi.advanceTimersByTime(hour);
  });
  expect(updates.mock.calls).toEqual([["automatic"], ["automatic"], ["automatic"]]);
});
