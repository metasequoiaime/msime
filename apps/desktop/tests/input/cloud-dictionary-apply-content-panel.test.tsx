// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { CloudDictionaryApplyPanel, type CloudDictionaryPanelClient } from "@msime/ui";

const snapshot = {
  cloudRevision: 7,
  sha256: "a".repeat(64),
  bytes: 128,
  records: 4,
  entries: 2,
  overlays: 1,
  positions: 1,
  selections: 0,
};
function host(previewToken: string): CloudDictionaryPanelClient {
  return {
    close: vi.fn(),
    request: vi
      .fn()
      .mockImplementation(async ({ operation }) =>
        operation === "snapshot_status"
          ? { localVersion: "synthetic-local-version" }
          : { snapshot, previewToken },
      ),
  };
}
async function download() {
  await waitFor(() =>
    expect(screen.getByRole("button", { name: "下载云词库并预览" })).toHaveProperty(
      "disabled",
      false,
    ),
  );
  fireEvent.click(screen.getByRole("button", { name: "下载云词库并预览" }));
  await screen.findByRole("region", { name: "确认应用" });
  await waitFor(() =>
    expect(screen.getByRole("button", { name: "丢弃预览" })).toHaveProperty("disabled", false),
  );
}
afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

test("面板丢弃预览后清理展示且可重新下载，不请求入列", async () => {
  const client = host("synthetic-preview-token");
  render(<CloudDictionaryApplyPanel client={client} />);
  await download();
  expect(screen.getByRole("button", { name: "替换本机词库" })).toHaveProperty("disabled", false);
  fireEvent.click(screen.getByRole("button", { name: "丢弃预览" }));
  expect(screen.queryByRole("region", { name: "确认应用" })).toBeNull();
  expect(screen.queryByRole("alertdialog")).toBeNull();
  expect(client.request).not.toHaveBeenCalledWith(
    expect.objectContaining({ operation: "snapshot_enqueue" }),
  );
  await download();
  expect(
    vi
      .mocked(client.request)
      .mock.calls.filter(([action]) => action.operation === "snapshot_preview"),
  ).toHaveLength(2);
});

test("面板收到空预览令牌时显示元数据但禁止替换", async () => {
  const client = host("");
  render(<CloudDictionaryApplyPanel client={client} />);
  await download();
  expect(screen.getByText("云端 revision 7 · 4 条记录 · 128 字节")).toBeTruthy();
  expect(screen.getByRole("button", { name: "替换本机词库" })).toHaveProperty("disabled", true);
  fireEvent.click(screen.getByRole("button", { name: "替换本机词库" }));
  expect(screen.queryByRole("alertdialog")).toBeNull();
  expect(client.request).not.toHaveBeenCalledWith(
    expect.objectContaining({ operation: "snapshot_enqueue" }),
  );
});

test("状态读取慢于两秒时仍展示本机版本并允许下载", async () => {
  vi.useFakeTimers();
  const client: CloudDictionaryPanelClient = {
    close: vi.fn(),
    request: vi.fn(
      () =>
        new Promise<{ localVersion: string }>((resolve) => {
          setTimeout(() => resolve({ localVersion: "synthetic-slow-version" }), 2500);
        }),
    ),
  };
  render(<CloudDictionaryApplyPanel client={client} />);
  const downloadButton = screen.getByRole("button", { name: "下载云词库并预览" });
  expect(downloadButton).toHaveProperty("disabled", true);
  await act(async () => {
    await vi.advanceTimersByTimeAsync(2500);
  });
  expect(screen.getByText("synthetic-slow-version")).toBeTruthy();
  expect(downloadButton).toHaveProperty("disabled", false);
  expect(client.request).toHaveBeenCalledTimes(1);
});
