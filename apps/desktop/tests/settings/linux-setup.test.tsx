// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { StrictMode } from "react";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import {
  LinuxSetupPage,
  nextSetupProgress,
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
    { download: true, cloudCandidates: false },
    expect.any(Function),
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

test("Linux first-run page discloses cloud candidates, leaves them off, and passes an accepted choice", async () => {
  const client: LinuxSetupClient = {
    run: vi.fn(async () => ({ ...missing, prepared: true })),
  };
  render(<LinuxSetupPage status={missing} client={client} onComplete={vi.fn()} />);
  const cloud = screen.getByRole("checkbox", { name: /启用云候选/ }) as HTMLInputElement;
  expect(cloud.checked).toBe(false);
  expect(cloud.closest("label")?.textContent).toContain("inputtools.google.com");
  fireEvent.click(cloud);
  fireEvent.click(screen.getByRole("button", { name: "开始配置" }));
  await screen.findByRole("heading", { name: "配置完成" });
  expect(client.run).toHaveBeenCalledWith(
    { download: false, cloudCandidates: true },
    expect.any(Function),
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
  expect((await screen.findByRole("alert")).textContent).toContain("显示详细输出");
  expect(client.run).toHaveBeenCalledWith(
    { download: false, cloudCandidates: false },
    expect.any(Function),
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

test("配置按脚本报告的阶段推进进度条，结束后以结构化卡片说明输入法列表，原始输出收起", async () => {
  let report!: (stage: "verify" | "prepare" | "register") => void;
  let finish!: (result: LinuxSetupStatus) => void;
  const run = vi.fn(
    (
      _choices: unknown,
      onLine: (line: LinuxSetupLine) => void,
      onStage?: (stage: never) => void,
    ) => {
      onLine({ text: "词库已校验：/usr/share/msime-client/resources", error: false });
      report = onStage as typeof report;
      return new Promise<LinuxSetupStatus>((resolve) => {
        finish = resolve;
      });
    },
  );
  render(<LinuxSetupPage status={missing} client={{ run }} onComplete={vi.fn()} />);
  fireEvent.click(screen.getByRole("button", { name: "开始配置" }));
  const bar = await screen.findByRole("progressbar", { name: "正在配置" });
  await act(async () => report("prepare"));
  expect(screen.getByText("正在准备输入法配置…")).toBeTruthy();
  await act(async () =>
    finish({ ...missing, prepared: true, registration: { host: "fcitx5", registered: true } }),
  );
  expect(bar.isConnected).toBe(false);
  expect(screen.getByRole("heading", { name: "配置完成" })).toBeTruthy();
  const card = screen.getByRole("status", { name: "输入法列表" });
  expect(card.textContent).toContain("已加入 Fcitx5 输入法组");
  expect(card.querySelector("[data-mascot]")).not.toBeNull();
  // 原始输出不再是要用户自己读的结论，收在「显示详细输出」里。
  const details = screen.getByRole("log", { name: "配置输出" }).closest("details");
  expect(details?.open).toBe(false);
});

test("进度条先追上阶段起点，在阶段内缓慢前进且不越过上限", () => {
  expect(nextSetupProgress(0, "prepare")).toBe(52);
  const next = nextSetupProgress(52, "prepare");
  expect(next).toBeGreaterThan(52);
  expect(next).toBeLessThan(72);
  expect(nextSetupProgress(72, "prepare")).toBe(72);
  // 已经走过的进度不会因为后到的阶段倒退。
  expect(nextSetupProgress(60, "download")).toBe(60);
});

test.each([
  [{ host: "fcitx5", registered: false }, "把水杉输入法加入 Fcitx5 输入法组", "打开 Fcitx5 配置"],
  [{ host: "ibus", registered: false }, "把水杉输入法加入输入源", "复制 ibus restart"],
  [{ host: null, registered: false }, "启动 Fcitx5 或 IBus 后加入输入法列表", null],
] as const)("没能自动加入时给出分步说明和对应的按钮：%o", async (registration, title, button) => {
  const openFcitx5Config = vi.fn(async () => {});
  const copyText = vi.fn(async (_text: string) => {});
  const run = vi.fn(async () => ({ ...missing, prepared: true, registration }));
  render(
    <LinuxSetupPage
      status={missing}
      client={{ run, openFcitx5Config, copyText }}
      onComplete={vi.fn()}
    />,
  );
  fireEvent.click(screen.getByRole("button", { name: "开始配置" }));
  const card = await screen.findByRole("status", { name: "输入法列表" });
  expect(card.textContent).toContain(title);
  expect(card.querySelector("[data-mascot]")).toBeNull();
  expect(card.querySelector("ol")).not.toBeNull();
  if (button === "打开 Fcitx5 配置") {
    fireEvent.click(screen.getByRole("button", { name: button }));
    expect(openFcitx5Config).toHaveBeenCalledOnce();
  } else if (button) {
    fireEvent.click(screen.getByRole("button", { name: button }));
    await waitFor(() => expect(copyText).toHaveBeenCalledWith("ibus restart"));
    expect(await screen.findByText("已复制 ibus restart")).toBeTruthy();
  } else {
    expect(card.querySelector("button")).toBeNull();
  }
});

test("窗口没有系统装饰，页面自带可拖动的标题栏和关闭按钮", () => {
  const close = vi.fn();
  render(<LinuxSetupPage status={missing} client={{ run: vi.fn(), close }} onComplete={vi.fn()} />);
  const header = screen.getByRole("main", { name: "首次配置" }).querySelector("header");
  expect(header?.hasAttribute("data-tauri-drag-region")).toBe(true);
  fireEvent.click(screen.getByRole("button", { name: "关闭" }));
  expect(close).toHaveBeenCalledOnce();
});

test("宿主报告词库过期时以「词库需要更新」打开，立即更新走 update，也可以稍后再说", async () => {
  const prepared: LinuxSetupStatus = { ...missing, prepared: true, dictionaryOutdated: true };
  const run = vi.fn();
  const update = vi.fn(
    async (onLine: (line: LinuxSetupLine) => void, onStage?: (stage: "update") => void) => {
      onLine({ text: "已切换到新版本的词库", error: false });
      onStage?.("update");
      return { ...prepared, dictionaryOutdated: false };
    },
  );
  const onComplete = vi.fn();
  const view = render(
    <LinuxSetupPage status={prepared} client={{ run, update }} onComplete={onComplete} />,
  );
  expect(screen.getByRole("main", { name: "更新词库" })).toBeTruthy();
  expect(screen.getByRole("heading", { name: "词库需要更新" })).toBeTruthy();
  expect(screen.queryByRole("checkbox")).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "稍后" }));
  expect(onComplete).toHaveBeenCalledOnce();
  fireEvent.click(screen.getByRole("button", { name: "立即更新" }));
  await screen.findByRole("heading", { name: "词库已更新" });
  expect(update).toHaveBeenCalledOnce();
  expect(run).not.toHaveBeenCalled();
  expect(screen.queryByRole("status", { name: "输入法列表" })).toBeNull();
  view.unmount();

  // 宿主没有提供更新时说明原因，不给一个按了会失败的按钮。
  render(<LinuxSetupPage status={prepared} client={{ run }} onComplete={vi.fn()} />);
  expect(screen.getByRole("alert").textContent).toContain("msime-linux-setup");
  expect((screen.getByRole("button", { name: "立即更新" }) as HTMLButtonElement).disabled).toBe(
    true,
  );
});

test("配置成功时脚本报告的提示列在结果卡片上，不必展开详细输出", async () => {
  const note = "提示：当前是 GNOME 的 Wayland 会话，且设置了 GTK_IM_MODULE=fcitx。";
  const run = vi.fn(async (_choices: unknown, onLine: (line: LinuxSetupLine) => void) => {
    onLine({ text: note, error: false });
    return {
      ...missing,
      prepared: true,
      registration: { host: "fcitx5" as const, registered: true },
      notes: [note],
    };
  });
  render(<LinuxSetupPage status={missing} client={{ run }} onComplete={vi.fn()} />);
  fireEvent.click(screen.getByRole("button", { name: "开始配置" }));
  const card = await screen.findByRole("status", { name: "输入法列表" });
  expect(card.querySelector('[aria-label="配置提示"]')?.textContent).toContain(note);
  expect(screen.getByRole("log", { name: "配置输出" }).closest("details")?.open).toBe(false);
});

test("首次配置页的产品名、结果卡片和终端命令随安装的版本", async () => {
  const wubi = {
    id: "wubi",
    display_name: "水杉五笔",
    input_schemes: ["wubi" as const],
    default_scheme: "wubi" as const,
    temporary_japanese: false,
    neural_keyboard: false,
    offline_glosses: false,
    handwriting: false,
    wubi_mixed_pinyin_default: false,
  };
  const run = vi.fn(async () => ({
    ...missing,
    prepared: true,
    registration: { host: null, registered: false },
  }));
  render(<LinuxSetupPage status={missing} client={{ run }} edition={wubi} onComplete={vi.fn()} />);
  expect(screen.getByRole("heading", { level: 1 }).textContent).toBe("水杉五笔");
  expect(screen.getByText(/msime-linux-wubi-setup 完成同样的配置/)).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "开始配置" }));
  const card = await screen.findByRole("status", { name: "输入法列表" });
  expect(card.textContent).toContain("msime-linux-wubi-setup --register");
  expect(card.textContent).toContain("「水杉五笔」");
  expect(card.textContent).toContain("「Metasequoia 水杉五笔」");
  expect(card.textContent).not.toContain("水杉输入法");
});
