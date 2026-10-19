// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { ClipboardHistorySection, type ClipboardHistoryEntry } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const paginationProps = {
  historyEnabled: true,
  persistedHistoryEnabled: true,
  ios: false,
  onToggle: vi.fn(),
  onError: vi.fn(),
};

function syntheticHistory(count: number, prefix = "合成分页记录"): ClipboardHistoryEntry[] {
  return Array.from({ length: count }, (_, index) => ({
    text: `${prefix} ${index + 1}`,
    timestampMs: 1_700_000_000_000 + index,
    pinned: index === 0,
  }));
}

test("桌面设置与历史分为顶部标签，往返保留页码且不会重读", async () => {
  const client = { clear: vi.fn(), list: vi.fn(async () => syntheticHistory(45)) };
  render(
    <ClipboardHistorySection {...paginationProps} client={client} tabbed>
      <button type="button">合成中继配置</button>
    </ClipboardHistorySection>,
  );
  expect(screen.getByRole("tab", { name: "设置", selected: true })).toBeTruthy();
  expect(screen.getByRole("switch", { name: "剪贴板历史" })).toBeTruthy();
  expect(screen.queryByRole("button", { name: "清空历史" })).toBeNull();
  expect(screen.queryByRole("tabpanel", { name: "历史记录" })).toBeNull();
  fireEvent.click(screen.getByRole("tab", { name: "历史记录" }));
  await screen.findByText("第 1 / 3 页 · 共 45 条");
  expect(screen.getByRole("button", { name: "清空历史" })).toBeTruthy();
  expect(screen.queryByRole("switch", { name: "剪贴板历史" })).toBeNull();
  expect(screen.queryByRole("button", { name: "合成中继配置" })).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "下一页" }));
  fireEvent.click(screen.getByRole("tab", { name: "设置" }));
  expect(screen.getByRole("button", { name: "合成中继配置" })).toBeTruthy();
  fireEvent.click(screen.getByRole("tab", { name: "历史记录" }));
  expect(screen.getByText("第 2 / 3 页 · 共 45 条")).toBeTruthy();
  expect(client.list).toHaveBeenCalledOnce();
});

test("关闭历史时仍能进入历史标签查看开启说明", () => {
  render(
    <ClipboardHistorySection
      {...paginationProps}
      historyEnabled={false}
      persistedHistoryEnabled={false}
      client={{ clear: vi.fn(), list: vi.fn() }}
      tabbed
    />,
  );
  fireEvent.click(screen.getByRole("tab", { name: "历史记录" }));
  expect(screen.getByText("请先在「剪贴板」中开启剪贴板历史")).toBeTruthy();
  expect(screen.queryByRole("button", { name: "清空历史" })).toBeNull();
});

test("历史每页只渲染 20 条，翻页不重读且操作对应当前页记录", async () => {
  const copy = vi.fn().mockResolvedValue(undefined);
  const list = vi.fn().mockResolvedValue(syntheticHistory(45));
  render(<ClipboardHistorySection {...paginationProps} client={{ clear: vi.fn(), list, copy }} />);
  await screen.findByText("第 1 / 3 页 · 共 45 条");
  expect(screen.getAllByRole("button", { name: "重新复制" })).toHaveLength(20);
  expect(screen.getByText("合成分页记录 20")).toBeTruthy();
  expect(screen.queryByText("合成分页记录 21")).toBeNull();
  expect(screen.getByRole("button", { name: "上一页" }).hasAttribute("disabled")).toBe(true);

  fireEvent.click(screen.getByRole("button", { name: "下一页" }));
  expect(screen.getByText("第 2 / 3 页 · 共 45 条")).toBeTruthy();
  expect(screen.queryByText("合成分页记录 20")).toBeNull();
  const row = screen.getByText("合成分页记录 21").closest("[data-clipboard-entry-row]")!;
  fireEvent.click(within(row as HTMLElement).getByRole("button", { name: "重新复制" }));
  expect(copy).toHaveBeenCalledExactlyOnceWith("合成分页记录 21");

  fireEvent.click(screen.getByRole("button", { name: "下一页" }));
  expect(screen.getByText("第 3 / 3 页 · 共 45 条")).toBeTruthy();
  expect(screen.getAllByRole("button", { name: "重新复制" })).toHaveLength(5);
  expect(screen.getByRole("button", { name: "下一页" }).hasAttribute("disabled")).toBe(true);
  fireEvent.click(screen.getByRole("button", { name: "上一页" }));
  expect(screen.getByText("合成分页记录 21")).toBeTruthy();
  expect(list).toHaveBeenCalledOnce();
});

test("删除末页最后一条回退，清空后也清除分页", async () => {
  let entries = syntheticHistory(21);
  const client = {
    list: vi.fn(async () => entries),
    remove: vi.fn(async (text: string) => {
      entries = entries.filter((entry) => entry.text !== text);
    }),
    clear: vi.fn(async () => {
      entries = [];
    }),
  };
  const view = render(
    <ClipboardHistorySection {...paginationProps} client={client} revision={1} />,
  );
  await screen.findByText("第 1 / 2 页 · 共 21 条");
  fireEvent.click(screen.getByRole("button", { name: "下一页" }));
  fireEvent.click(screen.getByRole("button", { name: "删除剪贴板记录" }));
  await screen.findByText("合成分页记录 1");
  expect(client.remove).toHaveBeenCalledExactlyOnceWith("合成分页记录 21");
  expect(screen.queryByRole("button", { name: "下一页" })).toBeNull();

  entries = syntheticHistory(45);
  view.rerender(<ClipboardHistorySection {...paginationProps} client={client} revision={2} />);
  await screen.findByText("第 1 / 3 页 · 共 45 条");
  fireEvent.click(screen.getByRole("button", { name: "下一页" }));
  fireEvent.click(screen.getByRole("button", { name: "清空历史" }));
  fireEvent.click(screen.getByRole("button", { name: "确认清空" }));
  await screen.findByText("暂无历史记录");
  expect(client.clear).toHaveBeenCalledOnce();
  expect(screen.queryByRole("button", { name: "上一页" })).toBeNull();
});

test("删除后仍有当前页时不跳回第一页，替换客户端则重置页码", async () => {
  let entries = syntheticHistory(45);
  const client = {
    clear: vi.fn(),
    list: vi.fn(async () => entries),
    remove: vi.fn(async (text: string) => {
      entries = entries.filter((entry) => entry.text !== text);
    }),
  };
  const view = render(<ClipboardHistorySection {...paginationProps} client={client} />);
  await screen.findByText("第 1 / 3 页 · 共 45 条");
  fireEvent.click(screen.getByRole("button", { name: "下一页" }));
  fireEvent.click(screen.getAllByRole("button", { name: "删除剪贴板记录" })[0]);
  await screen.findByText("第 2 / 3 页 · 共 44 条");
  expect(screen.getByText("合成分页记录 22")).toBeTruthy();
  expect(screen.queryByText("合成分页记录 1")).toBeNull();

  view.rerender(
    <ClipboardHistorySection
      {...paginationProps}
      client={{ clear: vi.fn(), list: async () => syntheticHistory(45, "合成新客户端") }}
    />,
  );
  await screen.findByText("合成新客户端 1");
  expect(screen.getByText("第 1 / 3 页 · 共 45 条")).toBeTruthy();
  expect(screen.queryByText("合成新客户端 21")).toBeNull();
});

test("clipboard history loads entries and toggles a pin", async () => {
  const setPinned = vi.fn().mockResolvedValue(undefined);
  const clear = vi.fn().mockResolvedValue(undefined);
  const list = vi
    .fn()
    .mockResolvedValueOnce([{ text: "合成测试", timestampMs: 1_700_000_000_000, pinned: false }])
    .mockResolvedValueOnce([{ text: "合成测试", timestampMs: 1_700_000_000_000, pinned: true }]);

  render(
    <ClipboardHistorySection
      client={{ clear, list, setPinned }}
      historyEnabled
      persistedHistoryEnabled
      revision={1}
      ios={false}
      onToggle={vi.fn()}
      onError={vi.fn()}
    />,
  );

  expect(await screen.findByText("合成测试")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "固定剪贴板记录" }));

  await waitFor(() => expect(setPinned).toHaveBeenCalledWith("合成测试", true));
  expect(await screen.findByRole("button", { name: "取消固定剪贴板记录" })).toBeTruthy();
});

test("ignores a second clipboard mutation while the first is pending", async () => {
  let resolvePin!: () => void;
  const setPinned = vi.fn(
    () =>
      new Promise<void>((resolve) => {
        resolvePin = resolve;
      }),
  );
  const list = vi
    .fn()
    .mockResolvedValue([{ text: "合成测试", timestampMs: 1_700_000_000_000, pinned: false }]);
  render(
    <ClipboardHistorySection
      client={{ clear: vi.fn(), list, setPinned }}
      historyEnabled
      persistedHistoryEnabled
      revision={1}
      ios={false}
      onToggle={vi.fn()}
      onError={vi.fn()}
    />,
  );

  const pin = await screen.findByRole("button", { name: "固定剪贴板记录" });
  fireEvent.click(pin);
  fireEvent.click(pin);
  expect(setPinned).toHaveBeenCalledOnce();

  resolvePin();
  await waitFor(() => expect(setPinned).toHaveBeenCalledOnce());
});

test("a clipboard mutation from a replaced client cannot restore stale entries", async () => {
  let resolvePin!: () => void;
  const oldClient = {
    clear: vi.fn().mockResolvedValue(undefined),
    list: vi
      .fn()
      .mockResolvedValue([{ text: "旧记录", timestampMs: 1_700_000_000_000, pinned: false }]),
    setPinned: vi.fn(
      () =>
        new Promise<void>((resolve) => {
          resolvePin = resolve;
        }),
    ),
  };
  const nextClient = {
    clear: vi.fn().mockResolvedValue(undefined),
    list: vi
      .fn()
      .mockResolvedValue([{ text: "新记录", timestampMs: 1_700_000_000_001, pinned: false }]),
    setPinned: vi.fn().mockResolvedValue(undefined),
  };
  const view = render(
    <ClipboardHistorySection
      client={oldClient}
      historyEnabled
      persistedHistoryEnabled
      revision={1}
      ios={false}
      onToggle={vi.fn()}
      onError={vi.fn()}
    />,
  );
  await screen.findByText("旧记录");
  fireEvent.click(screen.getByRole("button", { name: "固定剪贴板记录" }));
  await waitFor(() => expect(oldClient.setPinned).toHaveBeenCalledWith("旧记录", true));

  view.rerender(
    <ClipboardHistorySection
      client={nextClient}
      historyEnabled
      persistedHistoryEnabled
      revision={1}
      ios={false}
      onToggle={vi.fn()}
      onError={vi.fn()}
    />,
  );
  await screen.findByText("新记录");
  await act(async () => {
    resolvePin();
    await Promise.resolve();
    await Promise.resolve();
  });
  expect(screen.queryByText("旧记录")).toBeNull();
  expect(screen.getByText("新记录")).toBeTruthy();
});

test("clears entries when the replacement client has no history capability", async () => {
  const client = {
    clear: vi.fn(),
    list: vi
      .fn()
      .mockResolvedValue([{ text: "旧记录", timestampMs: 1_700_000_000_000, pinned: false }]),
  };
  const props = {
    historyEnabled: true,
    persistedHistoryEnabled: true,
    revision: 1,
    ios: false,
    onToggle: vi.fn(),
    onError: vi.fn(),
  };
  const view = render(<ClipboardHistorySection {...props} client={client} />);
  await screen.findByText("旧记录");

  view.rerender(<ClipboardHistorySection {...props} client={{ clear: vi.fn() }} />);
  await waitFor(() => expect(screen.queryByText("旧记录")).toBeNull());

  let resolveNext!: (value: ClipboardHistoryEntry[]) => void;
  const nextClient = {
    clear: vi.fn(),
    list: vi.fn().mockReturnValue(
      new Promise<ClipboardHistoryEntry[]>((resolve) => {
        resolveNext = resolve;
      }),
    ),
  };
  view.rerender(<ClipboardHistorySection {...props} client={nextClient} />);
  expect(screen.queryByText("旧记录")).toBeNull();
  resolveNext([{ text: "新记录", timestampMs: 1_700_000_000_001, pinned: false }]);
  expect(await screen.findByText("新记录")).toBeTruthy();
});

function renderWithCloud(
  cloudRequest: Parameters<typeof ClipboardHistorySection>[0]["cloudRequest"],
  entries = [{ text: "合成云端记录", timestampMs: 1_700_000_000_000, pinned: false }],
) {
  const list = vi.fn().mockResolvedValue(entries);
  return render(
    <ClipboardHistorySection
      client={{ clear: vi.fn(), list }}
      historyEnabled
      persistedHistoryEnabled
      revision={1}
      ios={false}
      onToggle={vi.fn()}
      onError={vi.fn()}
      cloudRequest={cloudRequest}
    />,
  );
}

test("clipboard history offers no cloud action when the host has no cloud clipboard", async () => {
  renderWithCloud(undefined);

  expect(await screen.findByText("合成云端记录")).toBeTruthy();
  expect(screen.queryByRole("button", { name: "发到云剪贴板" })).toBeNull();
});

test("clipboard history sends only the chosen entry to the cloud clipboard", async () => {
  const cloudRequest = vi
    .fn()
    .mockResolvedValueOnce({ enabled: true, items: [] })
    .mockResolvedValueOnce({ items: [{ id: "c1", text: "合成云端记录" }] });
  renderWithCloud(cloudRequest);

  const send = await screen.findByRole("button", { name: "发到云剪贴板" });
  await waitFor(() => expect(send.hasAttribute("disabled")).toBe(false));
  expect(cloudRequest).toHaveBeenCalledTimes(1);
  expect(cloudRequest).toHaveBeenCalledWith({ operation: "list", search: "" });

  fireEvent.click(send);

  await waitFor(() =>
    expect(cloudRequest).toHaveBeenLastCalledWith({ operation: "add", text: "合成云端记录" }),
  );
  expect(await screen.findByText("已发到云剪贴板")).toBeTruthy();
});

test("clipboard history explains a signed-out account and keeps the cloud action disabled", async () => {
  const cloudRequest = vi.fn().mockRejectedValue({ code: "account_unauthorized" });
  renderWithCloud(cloudRequest);

  expect(await screen.findByText("登录水杉账号后可在设备间同步剪贴板")).toBeTruthy();
  expect(screen.getByRole("button", { name: "发到云剪贴板" }).hasAttribute("disabled")).toBe(true);
  expect(cloudRequest).toHaveBeenCalledTimes(1);
});

test("clipboard history explains a disabled cloud clipboard and does not upload", async () => {
  const cloudRequest = vi.fn().mockResolvedValue({ enabled: false, items: [] });
  renderWithCloud(cloudRequest);

  expect(await screen.findByText("云剪贴板未开启")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "发到云剪贴板" }));
  expect(cloudRequest).toHaveBeenCalledTimes(1);
});

test("clipboard history refuses an entry over the cloud limit instead of truncating it", async () => {
  const cloudRequest = vi.fn().mockResolvedValue({ enabled: true, items: [] });
  renderWithCloud(cloudRequest, [
    { text: "合".repeat(4001), timestampMs: 1_700_000_000_000, pinned: false },
  ]);

  const send = await screen.findByRole("button", { name: "发到云剪贴板" });
  await waitFor(() => expect(send.hasAttribute("disabled")).toBe(false));
  fireEvent.click(send);

  expect(
    await screen.findByText("这条记录超过 4000 个 UTF-16 单元，无法发到云剪贴板"),
  ).toBeTruthy();
  expect(cloudRequest).toHaveBeenCalledTimes(1);
});

test("clipboard history reports a failed upload and a lost session", async () => {
  const cloudRequest = vi
    .fn()
    .mockResolvedValueOnce({ enabled: true, items: [] })
    .mockRejectedValueOnce({ code: "account_unavailable" })
    .mockRejectedValueOnce({ code: "account_unauthorized" });
  renderWithCloud(cloudRequest);

  const send = await screen.findByRole("button", { name: "发到云剪贴板" });
  await waitFor(() => expect(send.hasAttribute("disabled")).toBe(false));
  fireEvent.click(send);
  expect(await screen.findByText("发到云剪贴板失败，请稍后重试")).toBeTruthy();

  await waitFor(() => expect(send.hasAttribute("disabled")).toBe(false));
  fireEvent.click(send);
  expect(await screen.findByText("登录水杉账号后可在设备间同步剪贴板")).toBeTruthy();
  expect(send.hasAttribute("disabled")).toBe(true);
});

test("clipboard history checks the cloud clipboard again when its route changes", async () => {
  const cloudRequest = vi
    .fn()
    .mockRejectedValueOnce({ code: "account_unauthorized" })
    .mockResolvedValue({ enabled: true, items: [] });
  const props = {
    client: {
      clear: vi.fn(),
      list: vi
        .fn()
        .mockResolvedValue([
          { text: "合成云端记录", timestampMs: 1_700_000_000_000, pinned: false },
        ]),
    },
    historyEnabled: true,
    persistedHistoryEnabled: true,
    revision: 1,
    ios: false,
    onToggle: vi.fn(),
    onError: vi.fn(),
    cloudRequest,
  };
  const view = render(<ClipboardHistorySection {...props} cloudRoute="account" />);
  expect(await screen.findByText("登录水杉账号后可在设备间同步剪贴板")).toBeTruthy();

  view.rerender(<ClipboardHistorySection {...props} cloudRoute="relay" />);
  await waitFor(() =>
    expect(screen.getByRole("button", { name: "发到云剪贴板" }).hasAttribute("disabled")).toBe(
      false,
    ),
  );
  expect(cloudRequest).toHaveBeenCalledTimes(2);
  expect(screen.queryByText("登录水杉账号后可在设备间同步剪贴板")).toBeNull();
});

test("同一批点击只发送一条云端记录", async () => {
  let finish!: (value: { items: [] }) => void;
  const pending = new Promise<{ items: [] }>((resolve) => {
    finish = resolve;
  });
  const cloudRequest = vi
    .fn()
    .mockResolvedValueOnce({ enabled: true, items: [] })
    .mockReturnValue(pending);
  renderWithCloud(cloudRequest, [
    { text: "合成记录一", timestampMs: 1_700_000_000_000, pinned: false },
    { text: "合成记录二", timestampMs: 1_700_000_000_001, pinned: false },
  ]);
  const buttons = await screen.findAllByRole("button", { name: "发到云剪贴板" });
  await waitFor(() => expect(buttons[0].hasAttribute("disabled")).toBe(false));
  act(() => {
    fireEvent.click(buttons[0]);
    fireEvent.click(buttons[1]);
  });
  expect(cloudRequest.mock.calls.filter(([action]) => action.operation === "add")).toHaveLength(1);
  await act(async () => finish({ items: [] }));
});

test("替换本地客户端时立即清掉旧条目和清空确认", async () => {
  const old = {
    clear: vi.fn(async () => undefined),
    list: vi.fn(async () => [{ text: "合成旧条目", timestampMs: 1, pinned: false }]),
  };
  let finish!: (value: ClipboardHistoryEntry[]) => void;
  const next = {
    clear: vi.fn(async () => undefined),
    list: vi.fn(
      () =>
        new Promise<ClipboardHistoryEntry[]>((resolve) => {
          finish = resolve;
        }),
    ),
  };
  const props = {
    historyEnabled: true,
    persistedHistoryEnabled: true,
    ios: false,
    onToggle: vi.fn(),
    onError: vi.fn(),
  };
  const view = render(<ClipboardHistorySection {...props} client={old} />);
  await screen.findByText("合成旧条目");
  fireEvent.click(screen.getByRole("button", { name: "清空历史" }));
  expect(screen.getByRole("button", { name: "确认清空" })).toBeTruthy();
  view.rerender(<ClipboardHistorySection {...props} client={next} />);
  expect(screen.queryByText("合成旧条目")).toBeNull();
  expect(screen.queryByRole("button", { name: "确认清空" })).toBeNull();
  await act(async () => finish([{ text: "合成新条目", timestampMs: 2, pinned: false }]));
  expect(screen.getByRole("button", { name: "清空历史" })).toBeTruthy();
  expect(next.clear).not.toHaveBeenCalled();
});

test("重新复制失败由当前客户端的错误入口报告", async () => {
  const onError = vi.fn();
  render(
    <ClipboardHistorySection
      client={{
        clear: vi.fn(),
        list: async () => [{ text: "合成复制条目", timestampMs: 1, pinned: false }],
        copy: vi.fn(async () => {
          throw new Error("synthetic copy failure");
        }),
      }}
      historyEnabled
      persistedHistoryEnabled
      ios={false}
      onToggle={vi.fn()}
      onError={onError}
    />,
  );
  const button = await screen.findByRole("button", { name: "重新复制" });
  await act(async () => fireEvent.click(button));
  expect(onError).toHaveBeenCalledExactlyOnceWith("无法复制剪贴板记录");
});
