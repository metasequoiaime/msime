// @vitest-environment jsdom
import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { usePanelAction } from "../../../../packages/ui/src/keyboard/use-panel-action";

afterEach(() => {
  vi.restoreAllMocks();
});

test("serializes actions, reports failures, and ignores stale completion", async () => {
  const notices: string[] = [];
  let resolve!: () => void;
  const pending = new Promise<void>((accept) => {
    resolve = accept;
  });
  const { result } = renderHook(() => usePanelAction((message) => notices.push(message)));

  let first: Promise<void> | undefined;
  act(() => {
    first = result.current.run(async () => pending, "第一次失败");
  });
  expect(result.current.busyRef.current).toBe(true);
  expect(result.current.run(async () => undefined, "不应执行")).toBeUndefined();

  act(() => result.current.invalidate());
  resolve();
  await act(async () => {
    await first;
  });
  expect(result.current.busyRef.current).toBe(false);
  expect(notices).toEqual([]);
});

test("maps an action failure while the revision is current", async () => {
  const notices: string[] = [];
  const { result } = renderHook(() => usePanelAction((message) => notices.push(message)));

  await act(async () => {
    await result.current.run(async () => {
      throw new Error("synthetic failure");
    }, "操作失败");
  });

  await waitFor(() => expect(notices).toEqual(["操作失败"]));
  expect(result.current.busy).toBe(false);
});
