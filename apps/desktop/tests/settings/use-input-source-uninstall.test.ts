// @vitest-environment jsdom
import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { useInputSourceUninstall } from "@msime/ui";

afterEach(cleanup);

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((accept) => {
    resolve = accept;
  });
  return { promise, resolve };
}

test("ignores a same-tick duplicate uninstall confirmation", async () => {
  const pending = deferred<void>();
  const uninstallInputSource = vi.fn().mockReturnValue(pending.promise);
  const { result } = renderHook(() => useInputSourceUninstall({ uninstallInputSource }));

  let first!: Promise<void>;
  let second!: Promise<void>;
  act(() => {
    first = result.current.confirmUninstall();
    second = result.current.confirmUninstall();
  });
  expect(uninstallInputSource).toHaveBeenCalledOnce();
  pending.resolve();
  await act(async () => {
    await first;
    await second;
  });
});

test("keeps the confirmation open while System Settings still lists the input method", async () => {
  const uninstallInputSource = vi.fn().mockRejectedValue({ code: "input_source_listed" });
  const { result } = renderHook(() => useInputSourceUninstall({ uninstallInputSource }));

  act(() => result.current.requestUninstall());
  await act(async () => {
    await result.current.confirmUninstall();
  });
  expect(result.current.result).toBe("listed");
  expect(result.current.confirmation).toBe(true);
});
