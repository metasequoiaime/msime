// @vitest-environment jsdom
import {
  act,
  cleanup,
  fireEvent,
  render,
  renderHook,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { SettingsPage, useUpdateCheck, type Snapshot, type UpdateCheckResult } from "@msime/ui";
import { testHost } from "../support/host";

afterEach(cleanup);

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((accept, refuse) => {
    resolve = accept;
    reject = refuse;
  });
  return { promise, resolve, reject };
}

const initial: Snapshot = {
  format_version: 1,
  revision: 1,
  preferences: {
    scheme: "quanpin",
    shuangpin_profile: "xiaohe",
    candidate_page_size: 5,
    learning: true,
    theme: "system",
    chinese_punctuation: true,
  },
};

const cached: UpdateCheckResult = {
  status: "available",
  update: {
    version: { display: "1.5.0", parts: [1, 5, 0] },
    release_url: "https://github.com/metasequoiaime/msime/releases/tag/linux-v1.5.0",
    installer_name: "msime-linux_1.5.0_amd64.deb",
    installer_sha256: null,
    signed: false,
  },
};

function renderAbout() {
  const pending = deferred<UpdateCheckResult | null>();
  const manual = deferred<UpdateCheckResult>();
  const checkUpdate = vi.fn().mockReturnValue(manual.promise);
  render(
    <SettingsPage
      initialPage="about"
      client={{
        load: vi.fn().mockResolvedValue(initial),
        save: vi.fn(),
        readAppVersion: vi.fn().mockResolvedValue("1.0.0"),
        host: testHost({ platform: "linux" }),
        pendingUpdateCheck: () => pending.promise,
        checkUpdate,
      }}
    />,
  );
  return { pending, manual, checkUpdate };
}

test.each(["checking", "current", "none", "failed"] as const)(
  "手动检查为 %s 时迟到的后台记录不能恢复旧下载入口",
  async (outcome) => {
    const { pending, manual, checkUpdate } = renderAbout();
    const about = await screen.findByRole("group", { name: "关于" });
    await within(about).findAllByText("v1.0.0");
    fireEvent.click(within(about).getByRole("button", { name: "检查更新" }));
    expect(checkUpdate).toHaveBeenCalledOnce();
    expect(within(about).getByRole("button", { name: "正在检查…" })).toBeTruthy();

    if (outcome !== "checking") {
      await act(async () => {
        if (outcome === "failed") manual.reject(new Error("update_check_unavailable"));
        else if (outcome === "none") manual.resolve({ status: "none" });
        else
          manual.resolve({
            status: "current",
            update: {
              ...cached.update,
              version: { display: "1.0.0", parts: [1, 0, 0] },
              release_url: "https://github.com/metasequoiaime/msime/releases/tag/linux-v1.0.0",
            },
          });
      });
    }
    await act(async () => pending.resolve(cached));

    expect(within(about).queryByText("水杉 IME v1.5.0 已发布。")).toBeNull();
    expect(within(about).queryByRole("button", { name: "国内镜像下载" })).toBeNull();
    expect(within(about).queryByText("发现新版本 v1.5.0")).toBeNull();
    if (outcome === "checking") {
      expect(within(about).getByRole("button", { name: "正在检查…" })).toBeTruthy();
      await act(async () => manual.resolve({ status: "none" }));
    } else {
      const status = {
        current: "已是最新版本",
        none: "暂无可用发行版",
        failed: "检查失败，请稍后重试",
      }[outcome];
      expect(within(about).getByText(status)).toBeTruthy();
    }
  },
);

test("手动检查开始前仍显示缓存的新版本，开始后清掉旧下载入口", async () => {
  const { pending, manual } = renderAbout();
  const about = await screen.findByRole("group", { name: "关于" });
  await within(about).findAllByText("v1.0.0");
  await act(async () => pending.resolve(cached));
  await waitFor(() => expect(within(about).getByText("水杉 IME v1.5.0 已发布。")).toBeTruthy());

  fireEvent.click(within(about).getByRole("button", { name: "检查更新" }));
  expect(within(about).queryByText("水杉 IME v1.5.0 已发布。")).toBeNull();
  await act(async () => manual.resolve({ status: "none" }));
  expect(within(about).getByText("暂无可用发行版")).toBeTruthy();
});

test("替换缓存读取函数仍尊重手动检查，版本归属变化后允许重新预填", async () => {
  const pendingUpdateCheck = vi.fn().mockResolvedValue(cached);
  const checkUpdate = vi.fn().mockResolvedValue({ status: "none" });
  const { result, rerender } = renderHook(
    ({ readPending, version }) =>
      useUpdateCheck({
        checkUpdate,
        pendingUpdateCheck: readPending,
        platform: "linux",
        currentAppVersion: version,
      }),
    { initialProps: { readPending: pendingUpdateCheck, version: "1.0.0" } },
  );
  await waitFor(() => expect(result.current.available).not.toBeNull());
  await act(async () => result.current.checkForUpdate());
  expect(result.current.available).toBeNull();

  const readAgain = vi.fn().mockResolvedValue(cached);
  rerender({ readPending: readAgain, version: "1.0.0" });
  await act(async () => {});
  expect(readAgain).toHaveBeenCalledOnce();
  expect(result.current.available).toBeNull();
  expect(result.current.status).toBe("暂无可用发行版");

  rerender({ readPending: readAgain, version: "1.1.0" });
  await waitFor(() => expect(result.current.available?.version.display).toBe("1.5.0"));
  expect(result.current.status).toBe("发现新版本 v1.5.0");
});
