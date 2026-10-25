// @vitest-environment jsdom
import { StrictMode } from "react";
import { afterEach, expect, test, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { TypingStatisticsPage, type TypingStatisticsClient } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

function setup() {
  let lock = () => {};
  let resume = () => {};
  const requests: { resolve(): void; reject(error: Error): void }[] = [];
  const status = {
    statistics: { enabled: false, total: 0, days: {} },
    availability: "ready" as const,
  };
  const load = vi.fn(async () => status);
  const upload = vi.fn(async () => ({ enabled: false, signedIn: false }));
  const cancel = vi.fn();
  const authentication = {
    authenticate: vi.fn(
      () => new Promise<void>((resolve, reject) => requests.push({ resolve, reject })),
    ),
    cancel,
    onResume: (listener: () => void) => {
      resume = listener;
      return () => {
        resume = () => {};
      };
    },
    onLock: (listener: () => void) => {
      lock = listener;
      return () => {
        lock = () => {};
      };
    },
  };
  const client: TypingStatisticsClient = {
    load,
    setEnabled: load,
    reset: load,
    authentication,
    cloudUpload: { status: upload, setEnabled: upload, sync: upload },
  };
  return {
    client,
    load,
    upload,
    authentication,
    requests,
    lock: () => lock(),
    resume: () => resume(),
  };
}

test("验证完成前不读取统计和上传状态，系统确认无凭据时正常加载", async () => {
  const host = setup();
  render(<TypingStatisticsPage client={host.client} />);
  await waitFor(() => expect(host.requests).toHaveLength(1));
  expect(host.load).not.toHaveBeenCalled();
  expect(host.upload).not.toHaveBeenCalled();
  await act(async () => host.requests[0].resolve());
  await waitFor(() => expect(host.load).toHaveBeenCalledOnce());
});

test("取消或认证故障后保留重试按钮，不循环弹窗和读取数据", async () => {
  const host = setup();
  render(<TypingStatisticsPage client={host.client} />);
  await waitFor(() => expect(host.requests).toHaveLength(1));
  await act(async () => host.requests[0].reject(new Error("synthetic cancellation")));
  fireEvent(document, new Event("visibilitychange"));
  expect(host.authentication.authenticate).toHaveBeenCalledOnce();
  expect(host.load).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "验证身份" }));
  await waitFor(() => expect(host.requests).toHaveLength(2));
  await act(async () => host.requests[1].resolve());
  await waitFor(() => expect(host.load).toHaveBeenCalledOnce());
});

test("退后台撤销授权，旧认证回调不能解锁返回后的会话", async () => {
  const host = setup();
  render(<TypingStatisticsPage client={host.client} />);
  await waitFor(() => expect(host.requests).toHaveLength(1));
  act(host.lock);
  fireEvent(document, new Event("visibilitychange"));
  await waitFor(() => expect(host.requests).toHaveLength(2));
  await act(async () => host.requests[0].resolve());
  expect(host.load).not.toHaveBeenCalled();
  await act(async () => host.requests[1].resolve());
  await waitFor(() => expect(host.load).toHaveBeenCalledOnce());
  act(host.lock);
  expect(screen.queryByText("验证身份后查看打字统计")).not.toBeNull();
  fireEvent(document, new Event("visibilitychange"));
  await waitFor(() => expect(host.requests).toHaveLength(3));
});

test("认证窗口临时遮住网页不重开认证，页面卸载会取消请求", async () => {
  const host = setup();
  const view = render(<TypingStatisticsPage client={host.client} />);
  await waitFor(() => expect(host.requests).toHaveLength(1));
  vi.spyOn(document, "visibilityState", "get").mockReturnValue("hidden");
  fireEvent(document, new Event("visibilitychange"));
  vi.spyOn(document, "visibilityState", "get").mockReturnValue("visible");
  fireEvent(document, new Event("visibilitychange"));
  expect(host.authentication.authenticate).toHaveBeenCalledOnce();
  view.unmount();
  expect(host.authentication.cancel).toHaveBeenCalledOnce();
  await act(async () => host.requests[0].resolve());
  expect(host.load).not.toHaveBeenCalled();
});

test("宿主前台通知在网页没有可见性事件时也重新验证", async () => {
  const host = setup();
  render(<TypingStatisticsPage client={host.client} />);
  await waitFor(() => expect(host.requests).toHaveLength(1));
  await act(async () => host.requests[0].resolve());
  await waitFor(() => expect(host.load).toHaveBeenCalledOnce());
  act(host.lock);
  act(host.resume);
  await waitFor(() => expect(host.requests).toHaveLength(2));
  expect(host.load).toHaveBeenCalledOnce();
});

test("开发模式重复挂载不会启动已经取消的认证请求", async () => {
  const host = setup();
  render(
    <StrictMode>
      <TypingStatisticsPage client={host.client} />
    </StrictMode>,
  );
  await waitFor(() => expect(host.requests).toHaveLength(1));
  expect(host.authentication.authenticate).toHaveBeenCalledOnce();
  await act(async () => host.requests[0].resolve());
  await waitFor(() => expect(host.load).toHaveBeenCalled());
});

test("更换宿主后旧授权不能触发新宿主的数据读取", async () => {
  const first = setup();
  const second = setup();
  const view = render(<TypingStatisticsPage client={first.client} />);
  await waitFor(() => expect(first.requests).toHaveLength(1));
  await act(async () => first.requests[0].resolve());
  await waitFor(() => expect(first.load).toHaveBeenCalledOnce());
  view.rerender(<TypingStatisticsPage client={second.client} />);
  await waitFor(() => expect(second.requests).toHaveLength(1));
  expect(second.load).not.toHaveBeenCalled();
  await act(async () => second.requests[0].resolve());
  await waitFor(() => expect(second.load).toHaveBeenCalledOnce());
});
