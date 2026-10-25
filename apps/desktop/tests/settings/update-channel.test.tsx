// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import {
  SettingsPage,
  type HostReleaseUpdate,
  type Preferences,
  type Snapshot,
  type UpdateCheckResult,
} from "@msime/ui";
import { testHost } from "../support/host";

afterEach(cleanup);

// 关于页「版本与更新」里的更新通道：只有用这份页面当设置界面的桌面宿主（macOS、Linux）给出选择，检查和后台记录都按选的通道；鸿蒙的发布不分通道，不给。

function snapshot(preferences: Partial<Preferences> = {}): Snapshot {
  return {
    format_version: 1,
    revision: 1,
    preferences: {
      scheme: "quanpin",
      shuangpin_profile: "xiaohe",
      candidate_page_size: 5,
      learning: true,
      theme: "system",
      chinese_punctuation: true,
      ...preferences,
    },
  };
}

const releases = "https://github.com/metasequoiaime/msime/releases";

function release(fields: Partial<HostReleaseUpdate> = {}): HostReleaseUpdate {
  return {
    version: { display: "1.4.0", parts: [1, 4, 0] },
    release_url: `${releases}/tag/linux-v1.4.0`,
    installer_name: null,
    installer_sha256: null,
    signed: false,
    ...fields,
  };
}

async function openAbout() {
  const about = await screen.findByRole("group", { name: "关于" });
  await within(about).findAllByText("v1.0.0");
  return about;
}

test.each(["linux", "macos"] as const)(
  "%s 关于页给出更新通道，检查更新带上选的通道并写进偏好",
  async (platform) => {
    const checkUpdate = vi.fn().mockResolvedValue({ status: "none" } satisfies UpdateCheckResult);
    const save = vi.fn(async (revision: number, preferences: Preferences) => ({
      format_version: 1,
      revision: revision + 1,
      preferences,
    }));
    render(
      <SettingsPage
        initialPage="about"
        client={{
          load: vi.fn().mockResolvedValue(snapshot()),
          save,
          readAppVersion: vi.fn().mockResolvedValue("1.0.0"),
          host: testHost({ platform }),
          checkUpdate,
        }}
      />,
    );
    const about = await openAbout();
    const channel = within(about).getByRole("combobox", { name: "更新通道" }) as HTMLSelectElement;
    expect(channel.value).toBe("stable");
    expect(Array.from(channel.options, (option) => option.textContent)).toEqual([
      "稳定版",
      "测试版",
      "每日构建",
    ]);

    fireEvent.click(within(about).getByRole("button", { name: "检查更新" }));
    await waitFor(() =>
      expect(checkUpdate).toHaveBeenLastCalledWith(
        expect.objectContaining({ platform, channel: "stable" }),
      ),
    );

    fireEvent.change(channel, { target: { value: "beta" } });
    expect(channel.value).toBe("beta");
    // 不等自动保存：检查直接带上页面上选的通道。
    fireEvent.click(within(about).getByRole("button", { name: "检查更新" }));
    await waitFor(() =>
      expect(checkUpdate).toHaveBeenLastCalledWith(
        expect.objectContaining({ platform, channel: "beta" }),
      ),
    );
    await waitFor(() =>
      expect(save).toHaveBeenCalledWith(
        expect.any(Number),
        expect.objectContaining({ update_channel: "beta" }),
      ),
    );
  },
);

test("偏好里存着的通道读进选择框，不认识的值按稳定版", async () => {
  for (const [stored, shown] of [
    ["nightly", "nightly"],
    ["canary", "stable"],
  ] as const) {
    const checkUpdate = vi.fn().mockResolvedValue({ status: "none" } satisfies UpdateCheckResult);
    render(
      <SettingsPage
        initialPage="about"
        client={{
          load: vi
            .fn()
            .mockResolvedValue(
              snapshot({ update_channel: stored as Preferences["update_channel"] }),
            ),
          save: vi.fn(),
          readAppVersion: vi.fn().mockResolvedValue("1.0.0"),
          host: testHost({ platform: "linux" }),
          checkUpdate,
        }}
      />,
    );
    const about = await openAbout();
    expect(
      (within(about).getByRole("combobox", { name: "更新通道" }) as HTMLSelectElement).value,
    ).toBe(shown);
    fireEvent.click(within(about).getByRole("button", { name: "检查更新" }));
    await waitFor(() =>
      expect(checkUpdate).toHaveBeenCalledWith(expect.objectContaining({ channel: shown })),
    );
    cleanup();
  }
});

test.each(["harmony", "windows"] as const)(
  "%s 的关于页不给更新通道，检查更新也不带通道",
  async (platform) => {
    const checkUpdate = vi.fn().mockResolvedValue({ status: "none" } satisfies UpdateCheckResult);
    render(
      <SettingsPage
        initialPage="about"
        client={{
          load: vi.fn().mockResolvedValue(snapshot({ update_channel: "beta" })),
          save: vi.fn(),
          readAppVersion: vi.fn().mockResolvedValue("1.0.0"),
          host: testHost({ platform }),
          checkUpdate,
        }}
      />,
    );
    const about = await screen.findByRole("group", { name: "关于" });
    const check = await within(about).findByRole("button", { name: "检查更新" });
    expect(within(about).queryByRole("combobox", { name: "更新通道" })).toBeNull();
    expect(within(about).queryByText("更新通道")).toBeNull();
    fireEvent.click(check);
    await waitFor(() => expect(checkUpdate).toHaveBeenCalledOnce());
    expect(checkUpdate.mock.calls[0]?.[0]).not.toHaveProperty("channel");
  },
);

test("每日构建的结果写明通道和构建号，下载指向它自己的发布页", async () => {
  const update = release({
    release_url: `${releases}/tag/nightly`,
    installer_name: "msime-linux_1.4.0_amd64-nightly.123.1.deb",
    installer_sha256: "a".repeat(64),
    channel: "nightly",
    build_run: 123,
  });
  const openExternalUrl = vi.fn().mockResolvedValue(undefined);
  render(
    <SettingsPage
      initialPage="about"
      client={{
        load: vi.fn().mockResolvedValue(snapshot({ update_channel: "nightly" })),
        save: vi.fn(),
        readAppVersion: vi.fn().mockResolvedValue("1.0.0"),
        host: testHost({ platform: "linux", arch: "x86_64" }),
        openExternalUrl,
        checkUpdate: vi.fn().mockResolvedValue({ status: "available", update }),
      }}
    />,
  );
  const about = await openAbout();
  fireEvent.click(within(about).getByRole("button", { name: "检查更新" }));
  expect(await within(about).findByText("发现新版本 每日构建 v1.4.0（构建 123）")).toBeTruthy();
  expect(within(about).getByText("水杉 IME 每日构建 v1.4.0（构建 123） 已发布。")).toBeTruthy();
  expect(
    within(about).getByText("sha256sum msime-linux_1.4.0_amd64-nightly.123.1.deb"),
  ).toBeTruthy();
  expect(within(about).queryByRole("button", { name: "前往下载" })).toBeNull();
  fireEvent.click(within(about).getByRole("button", { name: "前往发布页" }));
  await waitFor(() => expect(openExternalUrl).toHaveBeenCalledWith(`${releases}/tag/nightly`));
});

test("测试版的结果写明通道，没有通道字段的旧结果照旧按正式版显示", async () => {
  for (const [update, status] of [
    [
      release({ release_url: `${releases}/tag/linux-v1.4.0-beta`, channel: "beta", build_run: 9 }),
      "发现新版本 测试版 v1.4.0",
    ],
    [release(), "发现新版本 v1.4.0"],
  ] as const) {
    render(
      <SettingsPage
        initialPage="about"
        client={{
          load: vi.fn().mockResolvedValue(snapshot()),
          save: vi.fn(),
          readAppVersion: vi.fn().mockResolvedValue("1.0.0"),
          host: testHost({ platform: "linux" }),
          checkUpdate: vi.fn().mockResolvedValue({ status: "available", update }),
        }}
      />,
    );
    const about = await openAbout();
    fireEvent.click(within(about).getByRole("button", { name: "检查更新" }));
    expect(await within(about).findByText(status)).toBeTruthy();
    cleanup();
  }
});

test("后台记录按页面选的通道读，换了通道清掉旧通道的结果再读一次", async () => {
  const pendingUpdateCheck = vi.fn(async (channel?: string) =>
    channel === "stable"
      ? ({ status: "available", update: release() } satisfies UpdateCheckResult)
      : null,
  );
  render(
    <SettingsPage
      initialPage="about"
      client={{
        load: vi.fn().mockResolvedValue(snapshot()),
        save: vi.fn(async (revision: number, preferences: Preferences) => ({
          format_version: 1,
          revision: revision + 1,
          preferences,
        })),
        readAppVersion: vi.fn().mockResolvedValue("1.0.0"),
        host: testHost({ platform: "linux" }),
        pendingUpdateCheck,
        checkUpdate: vi.fn(),
      }}
    />,
  );
  const about = await openAbout();
  expect(await within(about).findByText("水杉 IME v1.4.0 已发布。")).toBeTruthy();
  expect(pendingUpdateCheck).toHaveBeenLastCalledWith("stable");

  await act(async () => {
    fireEvent.change(within(about).getByRole("combobox", { name: "更新通道" }), {
      target: { value: "beta" },
    });
  });
  await waitFor(() => expect(pendingUpdateCheck).toHaveBeenLastCalledWith("beta"));
  expect(within(about).queryByText("水杉 IME v1.4.0 已发布。")).toBeNull();
  expect(within(about).queryByText("发现新版本 v1.4.0")).toBeNull();
});

test("macOS 的原生应用更新按页面选的通道读更新 feed", async () => {
  const checkForApplicationUpdate = vi.fn().mockResolvedValue(undefined);
  const checkUpdate = vi.fn();
  render(
    <SettingsPage
      initialPage="about"
      client={{
        load: vi.fn().mockResolvedValue(snapshot()),
        save: vi.fn(async (revision: number, preferences: Preferences) => ({
          format_version: 1,
          revision: revision + 1,
          preferences,
        })),
        readAppVersion: vi.fn().mockResolvedValue("1.0.0"),
        host: testHost({ platform: "macos" }),
        checkForApplicationUpdate,
        checkUpdate,
      }}
    />,
  );
  const about = await openAbout();
  fireEvent.click(within(about).getByRole("button", { name: "检查更新" }));
  await waitFor(() => expect(checkForApplicationUpdate).toHaveBeenLastCalledWith("stable"));

  for (const channel of ["beta", "nightly"] as const) {
    await act(async () => {
      fireEvent.change(within(about).getByRole("combobox", { name: "更新通道" }), {
        target: { value: channel },
      });
    });
    // 不等自动保存：原生更新直接按页面上选的通道读 feed。
    fireEvent.click(within(about).getByRole("button", { name: "检查更新" }));
    await waitFor(() => expect(checkForApplicationUpdate).toHaveBeenLastCalledWith(channel));
  }
  expect(checkUpdate).not.toHaveBeenCalled();
});

test("后台记录里的每日构建不给镜像链接和校验值，重新检查后才给", async () => {
  const recorded = release({
    release_url: `${releases}/tag/nightly`,
    installer_name: "msime-linux_1.4.0_amd64-nightly.123.1.deb",
    installer_sha256: "a".repeat(64),
    channel: "nightly",
    build_run: 123,
  });
  const fresh = release({
    release_url: `${releases}/tag/nightly`,
    installer_name: "msime-linux_1.4.0_amd64-nightly.124.1.deb",
    installer_sha256: "b".repeat(64),
    channel: "nightly",
    build_run: 124,
  });
  const openExternalUrl = vi.fn().mockResolvedValue(undefined);
  render(
    <SettingsPage
      initialPage="about"
      client={{
        load: vi.fn().mockResolvedValue(snapshot({ update_channel: "nightly" })),
        save: vi.fn(),
        readAppVersion: vi.fn().mockResolvedValue("1.0.0"),
        host: testHost({ platform: "linux", arch: "x86_64" }),
        openExternalUrl,
        pendingUpdateCheck: vi.fn().mockResolvedValue({ status: "available", update: recorded }),
        checkUpdate: vi.fn().mockResolvedValue({ status: "available", update: fresh }),
      }}
    />,
  );
  const about = await openAbout();
  expect(
    await within(about).findByText("水杉 IME 每日构建 v1.4.0（构建 123） 已发布。"),
  ).toBeTruthy();
  expect(within(about).getByText(/发布页上的安装包可能已经更新/)).toBeTruthy();
  expect(within(about).queryByText(/sha256sum/)).toBeNull();
  expect(within(about).queryByText("a".repeat(64))).toBeNull();
  expect(within(about).queryByRole("button", { name: "国内镜像下载" })).toBeNull();
  fireEvent.click(within(about).getByRole("button", { name: "前往发布页" }));
  await waitFor(() => expect(openExternalUrl).toHaveBeenCalledWith(`${releases}/tag/nightly`));

  fireEvent.click(within(about).getByRole("button", { name: "检查更新" }));
  expect(
    await within(about).findByText("水杉 IME 每日构建 v1.4.0（构建 124） 已发布。"),
  ).toBeTruthy();
  expect(within(about).queryByText(/发布页上的安装包可能已经更新/)).toBeNull();
  expect(
    within(about).getByText("sha256sum msime-linux_1.4.0_amd64-nightly.124.1.deb"),
  ).toBeTruthy();
  expect(within(about).getByRole("button", { name: "国内镜像下载" })).toBeTruthy();
});

test("后台记录里的正式版照常给镜像链接和校验值", async () => {
  const recorded = release({
    installer_name: "msime-linux_1.4.0_amd64.deb",
    installer_sha256: "c".repeat(64),
    channel: "stable",
  });
  render(
    <SettingsPage
      initialPage="about"
      client={{
        load: vi.fn().mockResolvedValue(snapshot()),
        save: vi.fn(),
        readAppVersion: vi.fn().mockResolvedValue("1.0.0"),
        host: testHost({ platform: "linux", arch: "x86_64" }),
        pendingUpdateCheck: vi.fn().mockResolvedValue({ status: "available", update: recorded }),
        checkUpdate: vi.fn(),
      }}
    />,
  );
  const about = await openAbout();
  expect(await within(about).findByText("水杉 IME v1.4.0 已发布。")).toBeTruthy();
  expect(within(about).queryByText(/发布页上的安装包可能已经更新/)).toBeNull();
  expect(within(about).getByText("sha256sum msime-linux_1.4.0_amd64.deb")).toBeTruthy();
  expect(within(about).getByRole("button", { name: "国内镜像下载" })).toBeTruthy();
});
