// @vitest-environment jsdom
import { cleanup, renderHook } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { useAccountSessionActions } from "../../../../packages/ui/src/account/use-account-session-actions";

afterEach(cleanup);

test("session actions pass logout scope and deletion to the current screen", async () => {
  const client = {
    logout: vi.fn().mockResolvedValue(undefined),
    deleteAccount: vi.fn().mockResolvedValue(undefined),
  };
  const onSignedOut = vi.fn();
  const onDeleted = vi.fn();
  const { result } = renderHook(() =>
    useAccountSessionActions({
      client,
      mounted: { current: true },
      clientGeneration: { current: 1 },
      perform: (operation) => operation(),
      onSignedOut,
      onDeleted,
    }),
  );

  await result.current.signOut(true);
  await result.current.deleteAccount();
  expect(client.logout).toHaveBeenCalledWith(true);
  expect(onSignedOut).toHaveBeenCalledWith(true);
  expect(client.deleteAccount).toHaveBeenCalledOnce();
  expect(onDeleted).toHaveBeenCalledOnce();
  expect(onDeleted).toHaveBeenCalledWith(false);
});

test("deletion reports anonymous voice samples that were not removed", async () => {
  for (const [result, kept] of [
    [{ voice_samples: { ok: false, error: "account_network" } }, true],
    [{ voice_samples: { ok: true } }, false],
    [{ voice_samples: null }, false],
    [undefined, false],
  ] as const) {
    const client = {
      logout: vi.fn().mockResolvedValue(undefined),
      deleteAccount: vi.fn().mockResolvedValue(result),
    };
    const onDeleted = vi.fn();
    const { result: hook } = renderHook(() =>
      useAccountSessionActions({
        client,
        mounted: { current: true },
        clientGeneration: { current: 1 },
        perform: (operation) => operation(),
        onSignedOut: vi.fn(),
        onDeleted,
      }),
    );
    await hook.current.deleteAccount();
    expect(onDeleted).toHaveBeenCalledOnce();
    expect(onDeleted).toHaveBeenCalledWith(kept);
    cleanup();
  }
});

test("session actions ignore results from an old client generation", async () => {
  let resolveLogout!: () => void;
  let resolveDelete!: () => void;
  const client = {
    logout: vi.fn(() => new Promise<void>((done) => (resolveLogout = done))),
    deleteAccount: vi.fn(() => new Promise<void>((done) => (resolveDelete = done))),
  };
  const clientGeneration = { current: 1 };
  const onSignedOut = vi.fn();
  const onDeleted = vi.fn();
  const { result } = renderHook(() =>
    useAccountSessionActions({
      client,
      mounted: { current: true },
      clientGeneration,
      perform: (operation) => operation(),
      onSignedOut,
      onDeleted,
    }),
  );

  const logout = result.current.signOut(false);
  clientGeneration.current = 2;
  resolveLogout();
  await logout;
  const deletion = result.current.deleteAccount();
  clientGeneration.current = 3;
  resolveDelete();
  await deletion;
  expect(onSignedOut).not.toHaveBeenCalled();
  expect(onDeleted).not.toHaveBeenCalled();
});
