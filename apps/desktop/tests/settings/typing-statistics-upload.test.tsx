// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import {
  TypingStatisticsPage,
  type TypingStatistics,
  type TypingStatisticsStatus,
  type TypingStatisticsUploadClient,
  type TypingStatisticsUploadStatus,
} from "@msime/ui";
import { TypingStatisticsUpload } from "../../../../packages/ui/src/settings/typing-statistics-upload";

afterEach(cleanup);

const value: TypingStatistics = { enabled: true, total: 10, days: { "2026-10-17": 10 } };
const status: TypingStatisticsStatus = { statistics: value, availability: "ready" };

function page(cloudUpload: TypingStatisticsUploadClient) {
  render(
    <TypingStatisticsPage
      client={{
        load: async () => status,
        setEnabled: async () => status,
        reset: async () => status,
        cloudUpload,
      }}
    />,
  );
}

function uploadClient(initial: TypingStatisticsUploadStatus) {
  let current = initial;
  return {
    status: vi.fn(async () => current),
    sync: vi.fn(async () => current),
    setEnabled: vi.fn(async (enabled: boolean) => {
      current = { ...current, enabled, lastUploadMs: enabled ? 1 : null };
      return current;
    }),
  };
}

test("opening the page asks the host to catch up and the switch uploads and deletes", async () => {
  const client = uploadClient({ enabled: false, signedIn: true });
  page(client);
  const toggle = (await screen.findByLabelText("上传到云端")) as HTMLInputElement;
  await waitFor(() => expect(toggle.disabled).toBe(false));
  expect(client.sync).toHaveBeenCalledTimes(1);
  expect(toggle.checked).toBe(false);
  expect(screen.getByText(/仅在本机保存日期、分类和数量/)).toBeTruthy();

  fireEvent.click(toggle);
  await waitFor(() => expect(client.setEnabled).toHaveBeenCalledWith(true));
  await screen.findByText(/已打开上传/);
  expect(toggle.checked).toBe(true);
  // 打开后页面不再说统计只保存在本机。
  expect(screen.queryByText(/仅在本机保存日期、分类和数量/)).toBeNull();
  expect(screen.getByText(/已打开「上传到云端」/)).toBeTruthy();

  fireEvent.click(toggle);
  await waitFor(() => expect(client.setEnabled).toHaveBeenCalledWith(false));
  await screen.findByText("已关闭上传，云端的打字统计已删除。");
});

test("the switch stays off while signed out", async () => {
  const client = uploadClient({ enabled: false, signedIn: false });
  page(client);
  const toggle = (await screen.findByLabelText("上传到云端")) as HTMLInputElement;
  await waitFor(() => expect(client.sync).toHaveBeenCalled());
  await screen.findByText(/登录水杉账号后可以打开/);
  expect(toggle.disabled).toBe(true);
});

test("a failed delete says the cloud copy is still there and rereads the switch", async () => {
  const client = uploadClient({ enabled: true, signedIn: true, lastUploadMs: 1 });
  client.setEnabled.mockImplementationOnce(async () => {
    throw { code: "account_unavailable" };
  });
  client.status.mockImplementation(async () => ({ enabled: false, signedIn: true }));
  page(client);
  const toggle = (await screen.findByLabelText("上传到云端")) as HTMLInputElement;
  await waitFor(() => expect(toggle.checked).toBe(true));
  fireEvent.click(toggle);
  await screen.findByText(/云端的数据没有删掉/);
  await waitFor(() => expect(toggle.checked).toBe(false));
});

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => (resolve = done));
  return { promise, resolve };
}

test("客户端替换后，旧的失败补读不能覆盖新开关或通知新页面", async () => {
  const recovery = deferred<TypingStatisticsUploadStatus>();
  const previous = uploadClient({ enabled: true, signedIn: true });
  previous.setEnabled.mockRejectedValueOnce({ code: "account_unavailable" });
  previous.status.mockReturnValueOnce(recovery.promise);
  const oldChange = vi.fn();
  const nextChange = vi.fn();
  const view = render(<TypingStatisticsUpload client={previous} onEnabledChange={oldChange} />);
  const toggle = screen.getByLabelText("上传到云端");
  await waitFor(() => expect(toggle).toHaveProperty("checked", true));
  fireEvent.click(toggle);
  await waitFor(() => expect(previous.status).toHaveBeenCalledOnce());
  oldChange.mockClear();

  const next = uploadClient({ enabled: false, signedIn: true });
  view.rerender(<TypingStatisticsUpload client={next} onEnabledChange={nextChange} />);
  await waitFor(() => expect(nextChange).toHaveBeenCalledExactlyOnceWith(false));
  await act(async () => recovery.resolve({ enabled: true, signedIn: true }));
  expect(toggle).toHaveProperty("checked", false);
  expect(nextChange).toHaveBeenCalledExactlyOnceWith(false);
  expect(oldChange).not.toHaveBeenCalled();
});

test("客户端替换后，新开关不必等待旧客户端的变更请求", async () => {
  const pending = deferred<TypingStatisticsUploadStatus>();
  const previous = uploadClient({ enabled: false, signedIn: true });
  previous.setEnabled.mockReturnValueOnce(pending.promise);
  const view = render(<TypingStatisticsUpload client={previous} />);
  const toggle = screen.getByLabelText("上传到云端");
  await waitFor(() => expect(toggle).toHaveProperty("disabled", false));
  fireEvent.click(toggle);
  expect(toggle).toHaveProperty("disabled", true);

  const next = uploadClient({ enabled: false, signedIn: true });
  view.rerender(<TypingStatisticsUpload client={next} />);
  await waitFor(() => expect(next.sync).toHaveBeenCalledOnce());
  expect(toggle).toHaveProperty("disabled", false);
  fireEvent.click(toggle);
  await screen.findByText("已打开上传，最近 7 天的每日计数已传到你的水杉账号。");
  await act(async () => pending.resolve({ enabled: false, signedIn: false }));
  expect(toggle).toHaveProperty("checked", true);
});

test("卸载后失败补读的迟到结果不再通知页面", async () => {
  const recovery = deferred<TypingStatisticsUploadStatus>();
  const client = uploadClient({ enabled: true, signedIn: true });
  client.setEnabled.mockRejectedValueOnce({ code: "account_unavailable" });
  client.status.mockReturnValueOnce(recovery.promise);
  const onEnabledChange = vi.fn();
  const view = render(<TypingStatisticsUpload client={client} onEnabledChange={onEnabledChange} />);
  const toggle = screen.getByLabelText("上传到云端");
  await waitFor(() => expect(toggle).toHaveProperty("checked", true));
  fireEvent.click(toggle);
  await waitFor(() => expect(client.status).toHaveBeenCalledOnce());
  onEnabledChange.mockClear();
  view.unmount();
  await act(async () => recovery.resolve({ enabled: false, signedIn: true }));
  expect(onEnabledChange).not.toHaveBeenCalled();
});
