// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import {
  CloudDictionaryApplyContent,
  type CloudDictionaryApplyContentProps,
  type CloudDictionarySnapshotRequest,
} from "@msime/ui";

const preview = {
  cloudRevision: 7,
  sha256: "a".repeat(64),
  bytes: 128,
  records: 4,
  entries: 2,
  overlays: 1,
  positions: 1,
  selections: 0,
};
function props(
  overrides: Partial<CloudDictionaryApplyContentProps> = {},
): CloudDictionaryApplyContentProps {
  return {
    localVersion: "synthetic-local-version",
    request: null,
    preview: null,
    canApply: false,
    busy: false,
    appliesInPanel: false,
    onDownload: vi.fn(),
    onRefresh: vi.fn(),
    onEnqueue: vi.fn(),
    onDiscardPreview: vi.fn(),
    onCancel: vi.fn(),
    ...overrides,
  };
}
function request(status: CloudDictionarySnapshotRequest["status"]): CloudDictionarySnapshotRequest {
  return { id: "synthetic-request", cloudRevision: 7, fileSha256: "a".repeat(64), status };
}
afterEach(cleanup);

test("准备区域展示本机版本，尚无预览和请求时只保留准备区域", () => {
  const callbacks = props();
  const { container } = render(<CloudDictionaryApplyContent {...callbacks} />);
  expect(container.children).toHaveLength(1);
  expect(screen.getByRole("region", { name: "准备本机词库" }).parentElement).toBe(container);
  expect(screen.getByText("已获取本机词库版本")).toBeTruthy();
  expect(screen.getByText("synthetic-local-version")).toBeTruthy();
  expect(screen.queryByRole("region", { name: "确认应用" })).toBeNull();
  expect(screen.queryByRole("region", { name: "处理结果" })).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "下载云词库并预览" }));
  fireEvent.click(screen.getByRole("button", { name: "刷新状态" }));
  expect(callbacks.onDownload).toHaveBeenCalledOnce();
  expect(callbacks.onRefresh).toHaveBeenCalledExactlyOnceWith();
  expect(callbacks.onEnqueue).not.toHaveBeenCalled();
  expect(callbacks.onDiscardPreview).not.toHaveBeenCalled();
  expect(callbacks.onCancel).not.toHaveBeenCalled();
});

test.each([null, ""])("本机版本为 %s 时禁止下载但允许刷新", (localVersion) => {
  const callbacks = props({ localVersion });
  render(<CloudDictionaryApplyContent {...callbacks} />);
  expect(screen.getByText("尚未获取本机词库版本")).toBeTruthy();
  expect(screen.getByRole("button", { name: "下载云词库并预览" })).toHaveProperty("disabled", true);
  expect(screen.getByRole("button", { name: "刷新状态" })).toHaveProperty("disabled", false);
  fireEvent.click(screen.getByRole("button", { name: "下载云词库并预览" }));
  expect(callbacks.onDownload).not.toHaveBeenCalled();
});

test("预览展示完整计数，可应用后委托入列和丢弃，保持三段同级顺序", () => {
  const callbacks = props({ preview, canApply: true, request: request("queued") });
  const { container } = render(<CloudDictionaryApplyContent {...callbacks} />);
  expect(Array.from(container.children, (section) => section.getAttribute("aria-label"))).toEqual([
    "准备本机词库",
    "确认应用",
    "处理结果",
  ]);
  expect(screen.getByText("云端 revision 7 · 4 条记录 · 128 字节")).toBeTruthy();
  expect(screen.getByText("词条 2 · 覆盖 1 · 固定 1 · 选择 0")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "替换本机词库" }));
  fireEvent.click(screen.getByRole("button", { name: "丢弃预览" }));
  fireEvent.click(screen.getByRole("button", { name: "取消待应用快照" }));
  expect(callbacks.onEnqueue).toHaveBeenCalledOnce();
  expect(callbacks.onDiscardPreview).toHaveBeenCalledOnce();
  expect(callbacks.onCancel).toHaveBeenCalledOnce();
});

test("有预览但未准备好令牌时只禁用替换，仍可丢弃", () => {
  const callbacks = props({ preview });
  const { rerender } = render(<CloudDictionaryApplyContent {...callbacks} />);
  expect(screen.getByRole("button", { name: "替换本机词库" })).toHaveProperty("disabled", true);
  expect(screen.getByRole("button", { name: "丢弃预览" })).toHaveProperty("disabled", false);
  fireEvent.click(screen.getByRole("button", { name: "替换本机词库" }));
  expect(callbacks.onEnqueue).not.toHaveBeenCalled();
  rerender(<CloudDictionaryApplyContent {...callbacks} canApply preview={null} />);
  expect(screen.queryByRole("region", { name: "确认应用" })).toBeNull();
});

test.each(["queued", "preparing", "applied", "conflict", "failed", "cancelled"] as const)(
  "%s 保留状态信息，并按是否待处理决定下载、取消和宿主提示",
  (status) => {
    const pending = status === "queued" || status === "preparing";
    const callbacks = props({ request: request(status), appliesInPanel: true });
    const { rerender } = render(<CloudDictionaryApplyContent {...callbacks} />);
    expect(screen.getByText(`状态：${status} · 云端 revision 7`)).toBeTruthy();
    expect(screen.getByRole("button", { name: "下载云词库并预览" })).toHaveProperty(
      "disabled",
      pending,
    );
    expect(Boolean(screen.queryByRole("button", { name: "取消待应用快照" }))).toBe(pending);
    expect(Boolean(screen.queryByText(/快照只在这个面板开着时应用/))).toBe(pending);
    rerender(<CloudDictionaryApplyContent {...callbacks} appliesInPanel={false} />);
    expect(screen.queryByText(/快照只在这个面板开着时应用/)).toBeNull();
    expect(Boolean(screen.queryByRole("button", { name: "取消待应用快照" }))).toBe(pending);
  },
);

test("忙碌时禁用五类动作，结束后使用新回调", () => {
  const callbacks = props({ preview, canApply: true, request: request("queued"), busy: true });
  const { rerender } = render(<CloudDictionaryApplyContent {...callbacks} />);
  for (const button of screen.getAllByRole("button")) {
    expect(button).toHaveProperty("disabled", true);
    fireEvent.click(button);
  }
  for (const action of [
    callbacks.onDownload,
    callbacks.onRefresh,
    callbacks.onEnqueue,
    callbacks.onDiscardPreview,
    callbacks.onCancel,
  ]) {
    expect(action).not.toHaveBeenCalled();
  }
  const onDownload = vi.fn();
  rerender(<CloudDictionaryApplyContent {...callbacks} request={null} />);
  expect(screen.getByRole("button", { name: "下载云词库并预览" })).toHaveProperty("disabled", true);
  fireEvent.click(screen.getByRole("button", { name: "下载云词库并预览" }));
  expect(callbacks.onDownload).not.toHaveBeenCalled();
  rerender(
    <CloudDictionaryApplyContent
      {...callbacks}
      busy={false}
      request={null}
      onDownload={onDownload}
    />,
  );
  expect(screen.getByRole("button", { name: "下载云词库并预览" })).toHaveProperty(
    "disabled",
    false,
  );
  fireEvent.click(screen.getByRole("button", { name: "下载云词库并预览" }));
  expect(onDownload).toHaveBeenCalledOnce();
  expect(callbacks.onDownload).not.toHaveBeenCalled();
});
