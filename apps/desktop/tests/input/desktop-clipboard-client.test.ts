import { beforeEach, expect, test, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { createDesktopClipboardClient } from "../../src/input/desktop-clipboard-client";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
beforeEach(() => vi.resetAllMocks());

test("创建客户端不触发原生操作", () => {
  createDesktopClipboardClient();
  expect(invoke).not.toHaveBeenCalled();
});

test.each(["list", "sync"] as const)("%s 保留记录元数据、顺序与重复文字", async (method) => {
  const entries = [
    { text: "合成重复记录", timestampMs: 9, pinned: true },
    { text: "合成重复记录", timestampMs: 4, pinned: false },
  ];
  vi.mocked(invoke).mockResolvedValueOnce(entries).mockResolvedValueOnce([]);
  const clipboard = createDesktopClipboardClient();
  await expect(clipboard[method]()).resolves.toBe(entries);
  await expect(clipboard[method]()).resolves.toEqual([]);
  expect(vi.mocked(invoke).mock.calls).toEqual([
    [`${method}_clipboard_history`],
    [`${method}_clipboard_history`],
  ]);
});

test("复制、删除、清空和固定保留各自命令和参数", async () => {
  vi.mocked(invoke).mockResolvedValue(undefined);
  const clipboard = createDesktopClipboardClient();
  await clipboard.copy("合成复制记录");
  await clipboard.remove("合成删除记录");
  await clipboard.clear();
  await clipboard.setPinned("合成固定记录", true);
  await clipboard.setPinned("合成固定记录", false);
  expect(vi.mocked(invoke).mock.calls).toEqual([
    ["copy_text", { text: "合成复制记录" }],
    ["remove_clipboard_history", { text: "合成删除记录" }],
    ["clear_clipboard_history"],
    ["set_clipboard_history_pinned", { text: "合成固定记录", pinned: true }],
    ["set_clipboard_history_pinned", { text: "合成固定记录", pinned: false }],
  ]);
});

test.each(["list", "sync", "copy", "remove", "clear", "setPinned"] as const)(
  "%s 失败时原样返回错误且不重试",
  async (method) => {
    const failure = new Error("合成宿主拒绝");
    vi.mocked(invoke).mockRejectedValueOnce(failure);
    const clipboard = createDesktopClipboardClient();
    await expect(clipboard[method]("合成记录", true)).rejects.toBe(failure);
    expect(invoke).toHaveBeenCalledOnce();
  },
);
