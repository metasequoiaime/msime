import { expect, test, vi } from "vitest";
import { runAccountOperation } from "@msime/ui";

test("runs an account operation with shared busy and status handling", async () => {
  const setBusy = vi.fn();
  const setError = vi.fn();
  const setNotice = vi.fn();
  const operation = vi.fn().mockResolvedValue(undefined);

  await runAccountOperation(
    {
      busy: false,
      isCurrent: () => true,
      setBusy,
      setError,
      setNotice,
    },
    operation,
  );

  expect(operation).toHaveBeenCalledOnce();
  expect(setBusy.mock.calls).toEqual([[true], [false]]);
  expect(setError).toHaveBeenCalledWith("");
  expect(setNotice).toHaveBeenCalledWith("");
});

test("reports ordinary failures, ignores cancellations, and skips stale operations", async () => {
  const make = () => ({
    setBusy: vi.fn(),
    setError: vi.fn(),
    setNotice: vi.fn(),
  });
  const ordinary = make();
  await runAccountOperation({ busy: false, isCurrent: () => true, ...ordinary }, async () => {
    throw new Error("synthetic failure");
  });
  expect(ordinary.setError).toHaveBeenCalledWith("账号服务暂不可用，请稍后再试。");

  const cancelled = make();
  await runAccountOperation({ busy: false, isCurrent: () => true, ...cancelled }, async () => {
    throw { code: "account_cancelled" };
  });
  expect(cancelled.setError).not.toHaveBeenCalledWith("账号服务暂不可用，请稍后再试。");

  const stale = make();
  const operation = vi.fn();
  await runAccountOperation({ busy: true, isCurrent: () => true, ...stale }, operation);
  await runAccountOperation({ busy: false, isCurrent: () => false, ...stale }, operation);
  expect(operation).not.toHaveBeenCalled();
  expect(stale.setBusy).not.toHaveBeenCalled();
});

test("ignores a failure and busy cleanup after the client changes", async () => {
  let reject!: (reason: unknown) => void;
  let current = true;
  const setBusy = vi.fn();
  const setError = vi.fn();
  const setNotice = vi.fn();
  const pending = runAccountOperation(
    { busy: false, isCurrent: () => current, setBusy, setError, setNotice },
    () => new Promise<void>((_resolve, fail) => (reject = fail)),
  );

  current = false;
  reject(new Error("synthetic late failure"));
  await pending;
  expect(setBusy.mock.calls).toEqual([[true]]);
  expect(setError).toHaveBeenCalledExactlyOnceWith("");
});

// 传输层的失败（断网、连接被重置）报的是网络，不说账号服务坏了。
test("a network failure reads as the network, not as the account service", async () => {
  const setError = vi.fn();
  await runAccountOperation(
    { busy: false, isCurrent: () => true, setBusy: vi.fn(), setError, setNotice: vi.fn() },
    async () => {
      throw { code: "account_network" };
    },
  );
  expect(setError).toHaveBeenLastCalledWith("网络连接失败，请检查网络后重试。");
});
