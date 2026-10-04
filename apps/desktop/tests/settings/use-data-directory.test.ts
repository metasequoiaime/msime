// @vitest-environment jsdom
import { act, renderHook, waitFor } from "@testing-library/react";
import { expect, test, vi } from "vitest";
import { useDataDirectory, type DataDirectoryClient } from "@msime/ui";

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((accept) => {
    resolve = accept;
  });
  return { promise, resolve };
}

test("a move response from a replaced data directory client is ignored", async () => {
  const pendingMove = deferred<Awaited<ReturnType<DataDirectoryClient["move"]>>>();
  const oldClient: DataDirectoryClient = {
    status: vi.fn().mockResolvedValue({ path: "/old", isDefault: true }),
    pick: vi.fn().mockResolvedValue("/target"),
    move: vi.fn().mockReturnValue(pendingMove.promise),
  };
  const nextClient: DataDirectoryClient = {
    status: vi.fn().mockResolvedValue({ path: "/new", isDefault: false }),
    pick: vi.fn(),
    move: vi.fn(),
  };
  const confirm = vi.fn().mockResolvedValue(true);
  const { result, rerender } = renderHook(
    ({ client }) => useDataDirectory({ client, enabled: true, confirm }),
    { initialProps: { client: oldClient } },
  );
  await waitFor(() => expect(result.current.dataDirectory?.path).toBe("/old"));

  let pending!: Promise<void>;
  act(() => {
    pending = result.current.choose();
  });
  await waitFor(() => expect(oldClient.move).toHaveBeenCalledOnce());
  rerender({ client: nextClient });
  await waitFor(() => expect(result.current.dataDirectory?.path).toBe("/new"));

  pendingMove.resolve({
    path: "/old-result",
    isDefault: false,
    retainedOldData: false,
  });
  await act(async () => pending);
  expect(result.current.dataDirectory?.path).toBe("/new");
});

test("clears the directory when the capability is disabled", async () => {
  const client: DataDirectoryClient = {
    status: vi.fn().mockResolvedValue({ path: "/current", isDefault: true }),
    pick: vi.fn(),
    move: vi.fn(),
  };
  const { result, rerender } = renderHook(
    ({ enabled }) => useDataDirectory({ client, enabled, confirm: vi.fn() }),
    { initialProps: { enabled: true } },
  );
  await waitFor(() => expect(result.current.dataDirectory?.path).toBe("/current"));

  rerender({ enabled: false });
  await waitFor(() => expect(result.current.dataDirectory).toBeUndefined());
});

test("ignores a same-tick duplicate directory choice", async () => {
  const pendingPick = deferred<string | null>();
  const client: DataDirectoryClient = {
    status: vi.fn().mockResolvedValue({ path: "/current", isDefault: true }),
    pick: vi.fn().mockReturnValue(pendingPick.promise),
    move: vi.fn(),
  };
  const confirm = vi.fn();
  const { result } = renderHook(() => useDataDirectory({ client, enabled: true, confirm }));
  await waitFor(() => expect(result.current.dataDirectory?.path).toBe("/current"));

  let first!: Promise<void>;
  let second!: Promise<void>;
  act(() => {
    first = result.current.choose();
    second = result.current.choose();
  });
  expect(client.pick).toHaveBeenCalledOnce();
  pendingPick.resolve(null);
  await act(async () => {
    await first;
    await second;
  });
});
