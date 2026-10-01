// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { usePreferencesSnapshot, type Snapshot } from "@msime/ui";

afterEach(cleanup);

const snapshot = (revision: number, theme: "light" | "dark"): Snapshot => ({
  format_version: 1,
  revision,
  preferences: {
    scheme: "quanpin",
    shuangpin_profile: "xiaohe",
    candidate_page_size: 6,
    learning: true,
    chinese_punctuation: true,
    theme,
  },
});

test("subscribes before loading and keeps only newer preference snapshots", async () => {
  let emit!: (value: Snapshot) => void;
  let resolve!: (value: Snapshot) => void;
  const order: string[] = [];
  const stop = vi.fn();
  const preferences = {
    onPreferencesChanged: async (listener: (value: Snapshot) => void) => {
      order.push("subscribe");
      emit = listener;
      return stop;
    },
    load: () => {
      order.push("load");
      return new Promise<Snapshot>((done) => {
        resolve = done;
      });
    },
  };
  const { result, unmount } = renderHook(() => usePreferencesSnapshot(preferences));

  await waitFor(() => expect(order).toEqual(["subscribe", "load"]));
  act(() => emit(snapshot(3, "dark")));
  expect(result.current?.revision).toBe(3);
  await act(async () => resolve(snapshot(2, "light")));
  expect(result.current?.revision).toBe(3);
  act(() => emit(snapshot(4, "light")));
  expect(result.current?.revision).toBe(4);
  unmount();
  expect(stop).toHaveBeenCalledOnce();
});

test("clears the previous snapshot when requested client changes", async () => {
  const first = { load: async () => snapshot(1, "dark") };
  const second = { load: async () => snapshot(2, "light") };
  const { result, rerender } = renderHook(
    ({ preferences }) => usePreferencesSnapshot(preferences, true),
    { initialProps: { preferences: first } },
  );

  await waitFor(() => expect(result.current?.revision).toBe(1));
  rerender({ preferences: second });
  expect(result.current).toBeNull();
  await waitFor(() => expect(result.current?.revision).toBe(2));
});
