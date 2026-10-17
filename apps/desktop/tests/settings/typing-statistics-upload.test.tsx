// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import {
  TypingStatisticsPage,
  type TypingStatistics,
  type TypingStatisticsStatus,
  type TypingStatisticsUploadClient,
  type TypingStatisticsUploadStatus,
} from "@msime/ui";

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
