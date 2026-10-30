// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { CloudDictionaryPanel, type CloudDictionaryPanelClient } from "@msime/ui";

afterEach(cleanup);

test.each(["delete", "download"])("更换宿主后不执行旧确认的 %s 动作", async (action) => {
  const entry = {
    id: "synthetic-old-entry",
    kind: "pinyin" as const,
    code: "synthetic",
    word: "合成旧词条",
    weight: 100,
    revision: 7,
  };
  const request = vi.fn().mockResolvedValue({ entries: [entry], offset: 0, has_more: false });
  const downloadToLocal = vi.fn().mockResolvedValue(undefined);
  const first: CloudDictionaryPanelClient = { close: vi.fn(), request, downloadToLocal };
  const panel = render(<CloudDictionaryPanel client={first} />);
  await screen.findByText(entry.word);
  fireEvent.click(
    screen.getByRole("button", {
      name: action === "delete" ? "删除" : `下载到本机 ${entry.word}`,
    }),
  );
  const nextRequest = vi.fn().mockResolvedValue({
    entries: [{ ...entry, id: "synthetic-new-entry", word: "合成新词条" }],
    offset: 0,
    has_more: false,
  });
  const nextDownload = vi.fn().mockResolvedValue(undefined);
  panel.rerender(
    <CloudDictionaryPanel
      client={{ close: vi.fn(), request: nextRequest, downloadToLocal: nextDownload }}
    />,
  );
  await screen.findByText("合成新词条");
  expect(screen.queryByRole("alertdialog")).toBeNull();
  expect(request).not.toHaveBeenCalledWith(expect.objectContaining({ operation: "delete" }));
  expect(nextRequest).not.toHaveBeenCalledWith(expect.objectContaining({ operation: "delete" }));
  expect(downloadToLocal).not.toHaveBeenCalled();
  expect(nextDownload).not.toHaveBeenCalled();
});

test.each(["confirm", "cancel"] as const)(
  "当前客户端删除确认的 %s 仍按回答执行",
  async (answer) => {
    const entry = {
      id: "synthetic-current-entry",
      kind: "pinyin" as const,
      code: "synthetic",
      word: "合成当前词条",
      weight: 100,
      revision: 7,
    };
    const request = vi.fn().mockResolvedValue({ entries: [entry], offset: 0, has_more: false });
    render(<CloudDictionaryPanel client={{ close: vi.fn(), request }} />);
    await screen.findByText(entry.word);
    fireEvent.click(screen.getByRole("button", { name: "删除" }));
    const dialog = screen.getByRole("alertdialog");
    await act(async () => {
      fireEvent.click(
        within(dialog).getByRole("button", { name: answer === "confirm" ? "删除" : "取消" }),
      );
    });
    const action = { operation: "delete", kind: "pinyin", id: entry.id, revision: 7 };
    if (answer === "confirm") {
      expect(request).toHaveBeenCalledWith(action);
      expect(screen.getByRole("status").textContent).toBe("云词条已删除");
    } else {
      expect(request).not.toHaveBeenCalledWith(action);
    }
  },
);
