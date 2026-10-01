// @vitest-environment jsdom
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { copyAccountId } from "@msime/ui";

beforeEach(() => {
  vi.useFakeTimers();
  Object.defineProperty(navigator, "clipboard", {
    configurable: true,
    value: { writeText: vi.fn().mockResolvedValue(undefined) },
  });
});

afterEach(() => {
  vi.useRealTimers();
});

test("copies the account id, reports the copied state, and resets it", async () => {
  const setCopied = vi.fn();
  const setError = vi.fn();
  copyAccountId({ id: "synthetic-account", isMounted: () => true, setCopied, setError });

  await vi.waitFor(() =>
    expect(navigator.clipboard.writeText).toHaveBeenCalledWith("synthetic-account"),
  );
  await vi.waitFor(() => expect(setCopied).toHaveBeenCalledWith(true));
  vi.advanceTimersByTime(1800);
  expect(setCopied).toHaveBeenLastCalledWith(false);
  expect(setError).not.toHaveBeenCalled();
});

test("does not update state after unmount and reports copy failures while mounted", async () => {
  const setCopied = vi.fn();
  const setError = vi.fn();
  let mounted = true;
  const writeText = vi.fn().mockRejectedValue(new Error("synthetic clipboard failure"));
  Object.defineProperty(navigator, "clipboard", {
    configurable: true,
    value: { writeText },
  });

  copyAccountId({ id: "synthetic-account", isMounted: () => mounted, setCopied, setError });
  await vi.waitFor(() =>
    expect(setError).toHaveBeenCalledWith("账号 ID 暂时无法复制，请稍后重试。"),
  );
  mounted = false;
  copyAccountId({ id: "synthetic-account", isMounted: () => mounted, setCopied, setError });
  expect(setCopied).not.toHaveBeenCalled();
});
