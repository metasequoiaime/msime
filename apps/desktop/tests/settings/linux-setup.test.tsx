// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { StrictMode } from "react";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import {
  LinuxSetupPage,
  type LinuxSetupClient,
  type LinuxSetupLine,
  type LinuxSetupStatus,
} from "@msime/ui";

afterEach(cleanup);

const missing: LinuxSetupStatus = {
  prepared: false,
  stateDirectory: "/home/user/.config/msime-client",
  directoryOccupied: false,
  setupAvailable: true,
};

test("Linux first-run page runs setup with the download choice and streams its output", async () => {
  const client: LinuxSetupClient = {
    run: vi.fn(async (_choices, onLine) => {
      onLine({ text: "词库已校验：/usr/share/msime-client/resources", error: false });
      onLine({ text: "启用用户服务失败", error: true });
      return { ...missing, prepared: true };
    }),
  };
  const onComplete = vi.fn();
  render(<LinuxSetupPage status={missing} client={client} onComplete={onComplete} />);
  expect(screen.getByText(/\/home\/user\/\.config\/msime-client/)).toBeTruthy();
  fireEvent.click(screen.getByRole("checkbox", { name: /从固定地址下载/ }));
  fireEvent.click(screen.getByRole("button", { name: "开始配置" }));
  await screen.findByRole("heading", { name: "配置完成" });
  expect(client.run).toHaveBeenCalledWith(
    { download: true, cloudCandidates: true },
    expect.any(Function),
  );
  const log = screen.getByRole("log", { name: "配置输出" });
  expect(log.textContent).toContain("词库已校验");
  expect(log.textContent).toContain("启用用户服务失败");
  fireEvent.click(screen.getByRole("button", { name: "进入设置" }));
  expect(onComplete).toHaveBeenCalled();
});

test("Linux setup remains available after StrictMode effect replay", async () => {
  const run = vi.fn().mockResolvedValue({ ...missing, prepared: true });
  render(
    <StrictMode>
      <LinuxSetupPage status={missing} client={{ run }} onComplete={vi.fn()} />
    </StrictMode>,
  );

  fireEvent.click(screen.getByRole("button", { name: "开始配置" }));
  await waitFor(() => expect(run).toHaveBeenCalledOnce());
});

test("ignores a same-tick duplicate setup action", async () => {
  let resolveRun!: (result: LinuxSetupStatus) => void;
  const run = vi.fn(
    (_choices: unknown, _onLine: (line: LinuxSetupLine) => void) =>
      new Promise<LinuxSetupStatus>((resolve) => {
        resolveRun = resolve;
      }),
  );
  render(<LinuxSetupPage status={missing} client={{ run }} onComplete={vi.fn()} />);

  await act(async () => {
    fireEvent.click(screen.getByRole("button", { name: "开始配置" }));
    fireEvent.click(screen.getByRole("button", { name: "开始配置" }));
  });
  expect(run).toHaveBeenCalledOnce();
  resolveRun({ ...missing, prepared: true });
  await screen.findByRole("heading", { name: "配置完成" });
});

test("Linux first-run setup remains available after StrictMode effect replay", async () => {
  const run = vi.fn(async () => ({ ...missing, prepared: true }));
  render(
    <StrictMode>
      <LinuxSetupPage status={missing} client={{ run }} onComplete={vi.fn()} />
    </StrictMode>,
  );

  fireEvent.click(screen.getByRole("button", { name: "开始配置" }));
  await screen.findByRole("heading", { name: "配置完成" });
  expect(run).toHaveBeenCalledOnce();
});

test("Linux first-run page discloses cloud candidates and passes a declined choice", async () => {
  const client: LinuxSetupClient = {
    run: vi.fn(async () => ({ ...missing, prepared: true })),
  };
  render(<LinuxSetupPage status={missing} client={client} onComplete={vi.fn()} />);
  const cloud = screen.getByRole("checkbox", { name: /启用云候选/ }) as HTMLInputElement;
  expect(cloud.checked).toBe(true);
  expect(cloud.closest("label")?.textContent).toContain("inputtools.google.com");
  fireEvent.click(cloud);
  fireEvent.click(screen.getByRole("button", { name: "开始配置" }));
  await screen.findByRole("heading", { name: "配置完成" });
  expect(client.run).toHaveBeenCalledWith(
    { download: false, cloudCandidates: false },
    expect.any(Function),
  );
});

test("Linux first-run page keeps the output and offers a retry when setup fails", async () => {
  const client: LinuxSetupClient = {
    run: vi.fn(async (_choices, onLine) => {
      onLine({ text: "词库目录不可用", error: true });
      throw { code: "setup_failed" };
    }),
  };
  render(<LinuxSetupPage status={missing} client={client} onComplete={vi.fn()} />);
  fireEvent.click(screen.getByRole("button", { name: "开始配置" }));
  expect((await screen.findByRole("alert")).textContent).toContain("查看上面的输出");
  expect(client.run).toHaveBeenCalledWith(
    { download: false, cloudCandidates: true },
    expect.any(Function),
  );
  expect(screen.getByRole("log").textContent).toContain("词库目录不可用");
  await waitFor(() =>
    expect((screen.getByRole("button", { name: "重新配置" }) as HTMLButtonElement).disabled).toBe(
      false,
    ),
  );
});

test("late setup output and completion are ignored after the page unmounts", async () => {
  let emit!: (line: LinuxSetupLine) => void;
  let release!: (result: LinuxSetupStatus) => void;
  const run = vi.fn((_choices: unknown, onLine: (line: LinuxSetupLine) => void) => {
    emit = onLine;
    return new Promise<LinuxSetupStatus>((resolve) => {
      release = resolve;
    });
  });
  const view = render(<LinuxSetupPage status={missing} client={{ run }} onComplete={vi.fn()} />);
  fireEvent.click(screen.getByRole("button", { name: "开始配置" }));
  view.unmount();
  emit({ text: "晚到的输出", error: false });
  release({ ...missing, prepared: true });
  await Promise.resolve();
});

test("Linux first-run page explains why it cannot start instead of offering a failing button", () => {
  const run = vi.fn();
  const { rerender } = render(
    <LinuxSetupPage
      status={{ ...missing, directoryOccupied: true }}
      client={{ run }}
      onComplete={vi.fn()}
    />,
  );
  expect(screen.getByRole("alert").textContent).toContain("缺少 runtime-options.json");
  expect((screen.getByRole("button", { name: "开始配置" }) as HTMLButtonElement).disabled).toBe(
    true,
  );
  rerender(
    <LinuxSetupPage
      status={{ ...missing, setupAvailable: false }}
      client={{ run }}
      onComplete={vi.fn()}
    />,
  );
  expect(screen.getByRole("alert").textContent).toContain("msime-linux-setup");
  expect(run).not.toHaveBeenCalled();
});
