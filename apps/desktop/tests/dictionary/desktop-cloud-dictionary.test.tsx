// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { StrictMode } from "react";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { DesktopCloudDictionary } from "../../src/dictionary/desktop-cloud-dictionary";
import {
  CloudCandidatesPanel,
  CloudDictionaryApplyPanel,
  CloudDictionaryCatalogPanel,
  CloudDictionaryFilesPanel,
} from "@msime/ui";
import { answerConfirm } from "../support/confirm";

afterEach(cleanup);

test("catalog discards entries and editing state when the account request fails", async () => {
  const request = vi.fn().mockResolvedValue({
    catalog_entries: [{ kind: "pinyin", code: "he", word: "合成", weight: 1 }],
    offset: 0,
    has_more: true,
    revision: 1,
    normalized: "he",
  });
  render(<CloudDictionaryCatalogPanel client={{ close: async () => {}, request }} />);
  fireEvent.change(screen.getByRole("textbox", { name: "完整目录编码" }), {
    target: { value: "he" },
  });
  fireEvent.click(screen.getByRole("button", { name: "查询完整目录" }));
  expect(await screen.findByText("合成")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "编辑" }));
  request.mockRejectedValue(new Error("unavailable"));
  fireEvent.click(screen.getByRole("button", { name: "查询完整目录" }));
  await waitFor(() => expect(screen.queryByText("合成")).toBeNull());
  expect(screen.queryByRole("button", { name: "保存" })).toBeNull();
});

test("candidate lookup failure removes old candidates and fixed positions", async () => {
  const request = vi.fn().mockImplementation(async ({ operation }) =>
    operation === "candidates"
      ? {
          candidates: [{ code: "he", word: "合成", weight: 1 }],
          context: "pinyin:he",
          revision: 1,
        }
      : { positions: [{ context: "pinyin:he", code: "he", word: "合成", position: 1 }] },
  );
  render(<CloudCandidatesPanel client={{ close: async () => {}, request }} />);
  fireEvent.change(screen.getByRole("textbox", { name: "云端候选编码" }), {
    target: { value: "he" },
  });
  fireEvent.click(screen.getByRole("button", { name: "查询云端候选" }));
  expect(await screen.findByRole("button", { name: "取消固定" })).toBeTruthy();
  request.mockRejectedValue(new Error("unavailable"));
  fireEvent.click(screen.getByRole("button", { name: "查询云端候选" }));
  await waitFor(() => expect(screen.queryByRole("button", { name: "调频" })).toBeNull());
  expect(screen.queryByRole("button", { name: "取消固定" })).toBeNull();
});

test("catalog ignores a response from a replaced client", async () => {
  let resolveOld: ((value: object) => void) | undefined;
  const oldRequest = vi.fn().mockImplementation(
    () =>
      new Promise((resolve) => {
        resolveOld = resolve;
      }),
  );
  const nextRequest = vi.fn().mockResolvedValue({
    catalog_entries: [],
    offset: 0,
    has_more: false,
    revision: 2,
    normalized: "he",
  });
  const panel = render(
    <CloudDictionaryCatalogPanel client={{ close: async () => {}, request: oldRequest }} />,
  );
  fireEvent.change(screen.getByRole("textbox", { name: "完整目录编码" }), {
    target: { value: "he" },
  });
  fireEvent.click(screen.getByRole("button", { name: "查询完整目录" }));
  await waitFor(() => expect(oldRequest).toHaveBeenCalled());
  panel.rerender(
    <CloudDictionaryCatalogPanel client={{ close: async () => {}, request: nextRequest }} />,
  );
  await act(async () => {
    resolveOld?.({
      catalog_entries: [{ kind: "pinyin", code: "he", word: "旧客户端", weight: 1 }],
      offset: 0,
      has_more: false,
      revision: 1,
      normalized: "he",
    });
    await Promise.resolve();
  });
  expect(screen.queryByText("旧客户端")).toBeNull();
});

test("candidate lookup ignores a response from a replaced client", async () => {
  let resolveOld: ((value: object) => void) | undefined;
  const oldRequest = vi.fn().mockImplementation(
    () =>
      new Promise((resolve) => {
        resolveOld = resolve;
      }),
  );
  const nextRequest = vi.fn().mockResolvedValue({ candidates: [], context: "", revision: 2 });
  const panel = render(
    <CloudCandidatesPanel client={{ close: async () => {}, request: oldRequest }} />,
  );
  fireEvent.change(screen.getByRole("textbox", { name: "云端候选编码" }), {
    target: { value: "he" },
  });
  fireEvent.click(screen.getByRole("button", { name: "查询云端候选" }));
  await waitFor(() => expect(oldRequest).toHaveBeenCalled());
  panel.rerender(<CloudCandidatesPanel client={{ close: async () => {}, request: nextRequest }} />);
  await act(async () => {
    resolveOld?.({
      candidates: [{ code: "he", word: "旧客户端", weight: 1 }],
      context: "pinyin:he",
      revision: 1,
    });
    await Promise.resolve();
  });
  expect(screen.queryByText("旧客户端")).toBeNull();
});

test("cloud dictionary file snapshot export ignores a response from a replaced client", async () => {
  let resolveOld: ((value: object) => void) | undefined;
  const oldRequest = vi.fn().mockImplementation(({ operation }: { operation: string }) =>
    operation === "snapshot_export"
      ? new Promise((resolve) => {
          resolveOld = resolve;
        })
      : Promise.resolve({}),
  );
  const nextRequest = vi.fn().mockResolvedValue({});
  const panel = render(
    <CloudDictionaryFilesPanel
      client={{ close: async () => {}, request: oldRequest, snapshot: true }}
    />,
  );
  fireEvent.click(screen.getByRole("button", { name: "导出完整快照" }));
  await waitFor(() => expect(oldRequest).toHaveBeenCalledWith({ operation: "snapshot_export" }));
  panel.rerender(
    <CloudDictionaryFilesPanel
      client={{ close: async () => {}, request: nextRequest, snapshot: true }}
    />,
  );
  await act(async () => {
    resolveOld?.({ text: "stale snapshot", filename: "stale.ndjson" });
    await Promise.resolve();
  });
  expect(screen.getByRole("status").textContent).not.toContain("完整云词库快照已导出");
});

test("cloud dictionary file snapshot export works after StrictMode effect replay", async () => {
  const request = vi.fn().mockResolvedValue({
    text: '{"type":"header"}\n',
    filename: "snapshot.ndjson",
  });
  render(
    <StrictMode>
      <CloudDictionaryFilesPanel client={{ close: async () => {}, request, snapshot: true }} />
    </StrictMode>,
  );

  fireEvent.click(screen.getByRole("button", { name: "导出完整快照" }));

  await waitFor(() => expect(request).toHaveBeenCalledWith({ operation: "snapshot_export" }));
});

test("cloud dictionary apply ignores status from a replaced client", async () => {
  let resolveOld: ((value: object) => void) | undefined;
  const oldRequest = vi.fn().mockImplementation(
    () =>
      new Promise((resolve) => {
        resolveOld = resolve;
      }),
  );
  const nextRequest = vi.fn().mockResolvedValue({ localVersion: "new-v2", request: null });
  const panel = render(
    <CloudDictionaryApplyPanel
      client={{ close: async () => {}, request: oldRequest, snapshot: true }}
    />,
  );
  await waitFor(() => expect(oldRequest).toHaveBeenCalledWith({ operation: "snapshot_status" }));
  panel.rerender(
    <CloudDictionaryApplyPanel
      client={{ close: async () => {}, request: nextRequest, snapshot: true }}
    />,
  );
  await screen.findByText("new-v2");
  await act(async () => {
    resolveOld?.({ localVersion: "stale-v1", request: null });
    await Promise.resolve();
  });
  expect(screen.queryByText("stale-v1")).toBeNull();
});

test("desktop dictionary subpages reuse the session client and return without closing its window", async () => {
  const close = vi.fn().mockResolvedValue(undefined);
  const request = vi.fn().mockResolvedValue({ entries: [], has_more: false, offset: 0 });
  render(<DesktopCloudDictionary client={{ close, request }} />);
  await waitFor(() =>
    expect(request).toHaveBeenCalledWith(expect.objectContaining({ operation: "list" })),
  );
  fireEvent.click(screen.getByRole("button", { name: "完整目录" }));
  expect(screen.getByText("完整云词库目录")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "返回云词库" }));
  await waitFor(() =>
    expect(screen.getByRole("button", { name: "云端候选排序" }).hasAttribute("disabled")).toBe(
      false,
    ),
  );
  fireEvent.click(screen.getByRole("button", { name: "云端候选排序" }));
  expect(screen.getByText("仅在点击查询时发送编码；修改只保存到当前账号")).toBeTruthy();
  expect(close).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "关闭" }));
  expect(close).toHaveBeenCalledTimes(1);
});

test("desktop dictionary apply page previews, confirms and cancels through the shared queue", async () => {
  const close = vi.fn().mockResolvedValue(undefined);
  const request = vi.fn().mockImplementation(async (action: { operation: string }) => {
    if (action.operation === "list") return { entries: [], has_more: false, offset: 0 };
    if (action.operation === "snapshot_status") return { localVersion: "local-v1", request: null };
    if (action.operation === "snapshot_preview")
      return {
        previewToken: "preview-token",
        snapshot: {
          cloudRevision: 7,
          sha256: "a".repeat(64),
          bytes: 128,
          records: 4,
          entries: 2,
          overlays: 1,
          positions: 1,
          selections: 0,
        },
      };
    if (action.operation === "snapshot_enqueue")
      return { request: { id: "request", cloudRevision: 7, status: "queued" } };
    if (action.operation === "snapshot_cancel")
      return { request: { id: "request", cloudRevision: 7, status: "cancelled" } };
    return {};
  });
  render(<DesktopCloudDictionary client={{ close, request, snapshot: true }} />);
  await waitFor(() =>
    expect(request).toHaveBeenCalledWith({
      operation: "list",
      kind: "pinyin",
      offset: 0,
      search: "",
    }),
  );
  fireEvent.click(screen.getByRole("button", { name: "应用到本机" }));
  await waitFor(() => expect(screen.getByText("已获取本机词库版本")).toBeTruthy());
  fireEvent.click(screen.getByRole("button", { name: "下载云词库并预览" }));
  expect(await screen.findByText(/云端 revision 7/)).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "替换本机词库" }));
  expect((await screen.findByRole("alertdialog")).textContent).toContain(
    "替换本机个人词库和学习记录",
  );
  await answerConfirm("confirm");
  await waitFor(() =>
    expect(request).toHaveBeenCalledWith({ operation: "snapshot_enqueue", token: "preview-token" }),
  );
  fireEvent.click(screen.getByRole("button", { name: "取消待应用快照" }));
  await answerConfirm("confirm");
  await waitFor(() => expect(request).toHaveBeenCalledWith({ operation: "snapshot_cancel" }));
});

test("apply status polling cannot overwrite a mutation result with a stale response", async () => {
  vi.useFakeTimers();
  let statusCalls = 0;
  let resolveStaleStatus: ((value: object) => void) | undefined;
  const request = vi.fn().mockImplementation(async (action: { operation: string }) => {
    if (action.operation === "snapshot_status") {
      statusCalls += 1;
      if (statusCalls === 1) return { localVersion: "local-v1", request: null };
      return new Promise((resolve) => {
        resolveStaleStatus = resolve;
      });
    }
    if (action.operation === "snapshot_preview")
      return {
        previewToken: "snapshot-token",
        snapshot: {
          cloudRevision: 42,
          sha256: "a".repeat(64),
          bytes: 2048,
          records: 12,
          entries: 4,
          overlays: 4,
          positions: 2,
          selections: 2,
        },
      };
    return {
      request: {
        id: "request",
        cloudRevision: 42,
        fileSha256: "a".repeat(64),
        status: "queued",
      },
    };
  });
  try {
    render(
      <CloudDictionaryApplyPanel client={{ close: async () => {}, request, snapshot: true }} />,
    );
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
      vi.advanceTimersByTime(2000);
      await Promise.resolve();
    });
    expect(screen.getByText("已获取本机词库版本")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "下载云词库并预览" }));
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(screen.getByText(/云端 revision 42/)).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "替换本机词库" }));
    const dialog = screen.getByRole("alertdialog");
    fireEvent.click(dialog.querySelectorAll("button")[1]);
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(screen.getByText("状态：queued · 云端 revision 42")).toBeTruthy();
    resolveStaleStatus?.({ localVersion: "local-v1", request: null });
    await act(async () => {
      await Promise.resolve();
    });
    expect(screen.getByText("状态：queued · 云端 revision 42")).toBeTruthy();
  } finally {
    vi.useRealTimers();
  }
});

test("desktop dictionary file page reuses the authenticated client and returns to entries", async () => {
  const close = vi.fn().mockResolvedValue(undefined);
  const request = vi
    .fn()
    .mockImplementation(async (action: { operation: string }) =>
      action.operation === "export"
        ? { text: "ni\t你\n" }
        : { entries: [], has_more: false, offset: 0 },
    );
  render(<DesktopCloudDictionary client={{ close, request }} />);
  await waitFor(() =>
    expect(request).toHaveBeenCalledWith(expect.objectContaining({ operation: "list" })),
  );
  fireEvent.click(screen.getByRole("button", { name: "导入与导出" }));
  expect(screen.getByText("导入与导出")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "返回云词库" }));
  await waitFor(() => expect(screen.getByRole("button", { name: "导入与导出" })).toBeTruthy());
  expect(close).not.toHaveBeenCalled();
});
