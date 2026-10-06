// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { ClipboardHistorySection, type ClipboardHistoryEntry } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
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
