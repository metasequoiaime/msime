// @vitest-environment jsdom
import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import {
  type MobileKeyboardFeedback,
  type MobileKeyboardFeedbackClient,
  useMobileKeyboardFeedback,
} from "@msime/ui";

afterEach(cleanup);

const value: MobileKeyboardFeedback = {
  soundEnabled: true,
  hapticsEnabled: true,
  hapticStrength: "medium",
};

function deferred() {
  let resolve!: () => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<void>((accept, fail) => {
    resolve = accept;
    reject = fail;
  });
  return { promise, resolve, reject };
}

function feedbackClient(preview: MobileKeyboardFeedbackClient["preview"]) {
  return {
    load: vi.fn().mockResolvedValue(value),
    save: vi.fn().mockImplementation(async (next: MobileKeyboardFeedback) => next),
    preview,
  };
}

function mountFeedback(client: MobileKeyboardFeedbackClient) {
  const onError = vi.fn();
  const hook = renderHook(
    ({ client, mobile }: { client: MobileKeyboardFeedbackClient; mobile: boolean }) =>
      useMobileKeyboardFeedback({ mobile, client, onError }),
    { initialProps: { client, mobile: true } },
  );
  return {
    ...hook,
    onError,
    startPreview: () => {
      let operation!: Promise<void>;
      act(() => {
        operation = hook.result.current.preview();
      });
      return operation;
    },
    ready: () => waitFor(() => expect(hook.result.current.value).toEqual(value)),
  };
}

test.each(["replace", "reuse", "disable", "reenable"] as const)(
  "ignores a late vibration preview failure after owner transition: %s",
  async (transition) => {
    const request = deferred();
    const clientA = feedbackClient(vi.fn().mockReturnValue(request.promise));
    const clientB = feedbackClient(vi.fn().mockResolvedValue(undefined));
    const hook = mountFeedback(clientA);
    await hook.ready();
    const preview = hook.startPreview();
    expect(clientA.preview).toHaveBeenCalledWith("medium");

    if (transition === "replace" || transition === "reuse") {
      hook.rerender({ client: clientB, mobile: true });
      await hook.ready();
      if (transition === "reuse") {
        hook.rerender({ client: clientA, mobile: true });
        await hook.ready();
      }
    } else {
      hook.rerender({ client: clientA, mobile: false });
      expect(hook.result.current.value).toBeUndefined();
      if (transition === "reenable") {
        hook.rerender({ client: clientA, mobile: true });
        await hook.ready();
      }
    }
    hook.onError.mockClear();
    await act(async () => {
      request.reject(new Error("synthetic preview failure"));
      await preview;
    });

    expect(hook.onError).not.toHaveBeenCalled();
  },
);

test.each(["success", "failure"] as const)(
  "a previous preview failure cannot change feedback after the latest preview %s",
  async (outcome) => {
    const first = deferred();
    const latest = deferred();
    const client = feedbackClient(
      vi.fn().mockReturnValueOnce(first.promise).mockReturnValueOnce(latest.promise),
    );
    const hook = mountFeedback(client);
    await hook.ready();
    const oldPreview = hook.startPreview();
    const newPreview = hook.startPreview();
    expect(client.preview).toHaveBeenCalledTimes(2);
    await act(async () => {
      if (outcome === "success") latest.resolve();
      else latest.reject(new Error("synthetic current preview failure"));
      await newPreview;
    });
    expect(hook.onError).toHaveBeenLastCalledWith(
      outcome === "success" ? "" : "无法预览按键振动，请重试。",
    );
    hook.onError.mockClear();
    await act(async () => {
      first.reject(new Error("synthetic previous preview failure"));
      await oldPreview;
    });

    expect(hook.onError).not.toHaveBeenCalled();
  },
);

test("the current preview failure still reports a retryable error", async () => {
  const client = feedbackClient(vi.fn().mockRejectedValue(new Error("synthetic preview failure")));
  const hook = mountFeedback(client);
  await hook.ready();
  await act(async () => hook.result.current.preview());

  expect(hook.onError).toHaveBeenLastCalledWith("无法预览按键振动，请重试。");
  expect(hook.result.current.busy).toBe(false);
});

test("saving during a preview stays available and invalidates its pending error", async () => {
  const request = deferred();
  const client = feedbackClient(vi.fn().mockReturnValue(request.promise));
  client.save.mockRejectedValue(new Error("synthetic save failure"));
  const hook = mountFeedback(client);
  await hook.ready();
  const preview = hook.startPreview();
  expect(hook.result.current.busy).toBe(false);
  await act(async () => hook.result.current.save({ ...value, hapticsEnabled: false }));
  expect(client.save).toHaveBeenCalledOnce();
  expect(hook.onError).toHaveBeenLastCalledWith("无法保存按键反馈设置，请重试。");
  hook.onError.mockClear();
  await act(async () => {
    request.reject(new Error("synthetic preview failure"));
    await preview;
  });

  expect(hook.onError).not.toHaveBeenCalled();
});
