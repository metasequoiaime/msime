// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import {
  SettingsPage,
  CommunityGalleryFeedback,
  InputSourceStartupNotice,
  ResourcePackRow,
  type InputSourceStartupStatus,
  type ResourcePacks,
} from "@msime/ui";
import { testHost } from "../support/host";
import { settingsFormReady } from "../support/settings-form";
import { MascotIllustration } from "../../../../packages/ui/src/core/mascot";

afterEach(cleanup);

const mascots = (container: HTMLElement) =>
  [...container.querySelectorAll("img[data-mascot]")].map((img) => img.getAttribute("data-mascot"));

test("插画只做装饰：对辅助技术隐藏，不提供替代文字", () => {
  const { container } = render(<MascotIllustration scene="offline" size={48} />);
  const img = container.querySelector("img")!;
  expect(img.getAttribute("alt")).toBe("");
  expect(img.getAttribute("aria-hidden")).toBe("true");
  expect(img.getAttribute("width")).toBe("48");
  expect(img.getAttribute("src")).toMatch(/offline\.webp|^data:image\/webp/);
});

test("社区读取失败时画断网，列表为空时画空篮子，要求登录和正常列表不配图", () => {
  const failed = render(<CommunityGalleryFeedback error="社区暂时不可用，请稍后重试。" />);
  expect(mascots(failed.container)).toEqual(["offline"]);
  failed.unmount();

  const signIn = render(
    <CommunityGalleryFeedback error="需要登录" signInRequired onLogin={vi.fn()} />,
  );
  expect(mascots(signIn.container)).toEqual([]);
  signIn.unmount();

  const empty = render(<CommunityGalleryFeedback empty={<p>还没有已发布的皮肤。</p>} />);
  expect(mascots(empty.container)).toEqual(["empty"]);
  empty.unmount();

  const listed = render(<CommunityGalleryFeedback notice={<p>已收藏</p>} />);
  expect(mascots(listed.container)).toEqual([]);
});

const startup = (
  action: InputSourceStartupStatus["action"],
  enabled = true,
): InputSourceStartupStatus => ({
  action,
  enabled,
  bundled_version: "1.2.0",
  installed_version: "1.2.0",
});

const renderNotice = (status: InputSourceStartupStatus) =>
  render(
    <InputSourceStartupNotice
      status={status}
      onOpenSettings={vi.fn()}
      onDismiss={vi.fn()}
      onError={vi.fn()}
    />,
  );

test("输入法顺利更新、用户无事可做时才配更新完成的插画", () => {
  const updated = renderNotice(startup("updated"));
  expect(mascots(updated.container)).toEqual(["updated"]);
  updated.unmount();

  for (const status of [startup("failed"), startup("login_required"), startup("updated", false)]) {
    const view = renderNotice(status);
    expect(mascots(view.container)).toEqual([]);
    view.unmount();
  }
});

test("资源包下载中的说明里配下载词库的插画，进度文字不变", () => {
  const packs: ResourcePacks = {
    statuses: [{ id: "language-dictionaries", state: "missing", size: 1024, schemes: [] }],
    progress: {
      "language-dictionaries": {
        id: "language-dictionaries",
        stage: "download",
        downloaded: 1,
        total: 4,
      },
    },
    errors: {},
    ensure: vi.fn(),
    install: vi.fn(),
    cancel: vi.fn(),
  };
  const { container } = render(<ResourcePackRow packs={packs} id="language-dictionaries" />);
  expect(mascots(container)).toEqual(["downloading"]);
  expect(container.textContent).toContain("25%");
});

test.each(["macos", "linux", "ios", "harmony"])(
  "%s 设置页首保留标题与导航，手机不增加桌面插画区",
  async (platform) => {
    const { container } = render(
      <SettingsPage
        initialPage="input"
        client={{
          host: testHost({ platform }),
          load: async () => ({
            format_version: 1,
            revision: 1,
            preferences: {
              scheme: "quanpin",
              shuangpin_profile: "xiaohe",
              candidate_page_size: 5,
              chinese_punctuation: true,
              learning: true,
            },
          }),
          save: vi.fn(),
        }}
      />,
    );
    await settingsFormReady();
    const headerMascots = container.querySelectorAll(
      '[data-settings-mascot-intro] img[data-mascot="input"]',
    );
    expect(headerMascots).toHaveLength(platform === "macos" || platform === "linux" ? 1 : 0);
    expect(container.querySelector("#page-title")?.textContent).toMatch(/输入|设置/);
    for (const mascot of headerMascots) {
      expect(mascot.getAttribute("aria-hidden")).toBe("true");
      expect(mascot.getAttribute("alt")).toBe("");
      expect(mascot.closest("button")).toBeNull();
      expect(mascot.closest("[data-settings-mascot-intro]")?.textContent).toContain("选择输入方案");
    }
    expect(screen.queryByRole("img", { name: "水杉娘" })).toBeNull();
  },
);

test.each([
  ["input", "input", "选择输入方案，调整选词与输出习惯。"],
  ["dictionary", "downloading", "管理词库、词条与常用表达。"],
  ["voice", "notes", "配置语音识别、快捷键与录音行为。"],
  ["tools", "notes", "管理剪贴板与历史记录。"],
  ["skin", "welcome", "挑选主题，调整输入界面的外观。"],
  ["account", "sync", "管理账号与跨设备设置同步。"],
] as const)("%s 页首使用对应场景并保留可用的页面内容", async (initialPage, scene, description) => {
  const { container } = render(
    <SettingsPage
      initialPage={initialPage}
      client={{
        host: testHost({ platform: "macos" }),
        load: async () => ({
          format_version: 1,
          revision: 1,
          preferences: {
            scheme: "quanpin",
            shuangpin_profile: "xiaohe",
            candidate_page_size: 5,
            chinese_punctuation: true,
            learning: true,
          },
        }),
        save: vi.fn(),
        account: {
          status: vi.fn().mockResolvedValue({ user: null }),
          providers: vi.fn().mockResolvedValue({ email: true, phone: false }),
          requestCode: vi.fn(),
          login: vi.fn(),
          profile: vi.fn(),
          rename: vi.fn(),
          logout: vi.fn(),
          deleteAccount: vi.fn(),
          clearExpired: vi.fn(),
        },
      }}
    />,
  );
  if (initialPage === "account") await screen.findByText(description);
  else await settingsFormReady();
  const intro = container.querySelector("[data-settings-mascot-intro]")!;
  expect(intro.querySelector("img")?.getAttribute("data-mascot")).toBe(scene);
  expect(intro.textContent).toContain(description);
  expect(container.querySelectorAll("#page-title")).toHaveLength(1);
  expect(screen.getByRole("main").getAttribute("aria-labelledby")).toBe("page-title");
});
