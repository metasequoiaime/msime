// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import {
  act,
  cleanup,
  fireEvent,
  render,
  renderHook,
  screen,
  waitFor,
} from "@testing-library/react";
import { SettingsPage, useFeedbackReport, type Snapshot } from "@msime/ui";
import { testHost } from "../support/host";

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

const actions = [
  { label: "复制报告", error: "复制报告失败，请重试。", copied: "已复制报告" },
  { label: "复制群号", error: "复制群号失败，请重试。", copied: "已复制" },
  { label: "在 GitHub 提交", error: "无法打开外部链接，请稍后重试。", copied: null },
] as const;

afterEach(cleanup);

async function renderFeedback(hostAction: (value: string) => Promise<void>) {
  render(
    <SettingsPage
      initialPage="feedback"
      client={{
        load: vi.fn().mockResolvedValue(snapshot),
        save: vi.fn(),
        host: testHost({ platform: "linux" }),
        copyText: hostAction,
        openExternalUrl: hostAction,
      }}
    />,
  );
  await screen.findByRole("textbox", { name: "反馈描述" });
}

test.each(actions)("$label 的异步失败显示可重试的错误", async ({ label, error, copied }) => {
  const hostAction = vi.fn<(value: string) => Promise<void>>().mockRejectedValue({
    code: "unavailable",
  });
  await renderFeedback(hostAction);
  fireEvent.click(screen.getByRole("button", { name: label }));

  await waitFor(() => expect(screen.queryByText(error)).not.toBeNull());
  expect(hostAction).toHaveBeenCalledOnce();
  if (copied) expect(screen.queryByRole("button", { name: copied })).toBeNull();
});

test.each(actions)("$label 的同步失败同样由页面显示", async ({ label, error, copied }) => {
  const hostAction = vi.fn<(value: string) => Promise<void>>().mockImplementation(() => {
    throw new Error("synthetic host failure");
  });
  await renderFeedback(hostAction);
  fireEvent.click(screen.getByRole("button", { name: label }));

  await waitFor(() => expect(screen.queryByText(error)).not.toBeNull());
  expect(hostAction).toHaveBeenCalledOnce();
  if (copied) expect(screen.queryByRole("button", { name: copied })).toBeNull();
});

test.each(actions)(
  "$label 失败后重试会清除旧错误并保留成功反馈",
  async ({ label, error, copied }) => {
    let finish!: () => void;
    const retry = new Promise<void>((resolve) => {
      finish = resolve;
    });
    const hostAction = vi
      .fn<(value: string) => Promise<void>>()
      .mockRejectedValueOnce({ code: "unavailable" })
      .mockReturnValueOnce(retry);
    await renderFeedback(hostAction);
    const button = screen.getByRole("button", { name: label });
    fireEvent.click(button);
    await waitFor(() => expect(screen.queryByText(error)).not.toBeNull());

    fireEvent.click(button);
    expect(screen.queryByText(error)).toBeNull();
    expect(hostAction).toHaveBeenCalledTimes(2);
    if (copied) expect(screen.queryByRole("button", { name: copied })).toBeNull();
    await act(async () => finish());
    if (copied) expect(screen.getByRole("button", { name: copied })).toBeTruthy();
    expect(screen.queryByText(error)).toBeNull();
  },
);

test.each(["copyReport", "copyGroup", "submit"] as const)(
  "%s 卸载后不再报告迟到的宿主失败",
  async (action) => {
    let reject!: (reason: unknown) => void;
    const pending = new Promise<void>((_resolve, fail) => {
      reject = fail;
    });
    const setError = vi.fn();
    const { result, unmount } = renderHook(() =>
      useFeedbackReport({
        supportDiagnostics: "synthetic diagnostics",
        issuesUrl: "https://example.invalid/issues",
        copyText: () => pending,
        openExternalUrl: () => pending,
        setError,
      }),
    );
    act(() => void result.current[action]());
    setError.mockClear();
    unmount();
    await act(async () => reject({ code: "unavailable" }));

    expect(setError).not.toHaveBeenCalled();
  },
);
