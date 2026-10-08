// @vitest-environment jsdom
import { testHost } from "../support/host";
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { AboutHeroSection, SettingsPage, type Snapshot } from "@msime/ui";
import { HarmonyAboutHero } from "../../../../packages/ui/src/settings/about-hero-section";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

test("renders the product mark, name, and platform description", () => {
  render(<AboutHeroSection logo="data:image/svg+xml,synthetic" description="桌面输入体验" />);

  expect((screen.getByRole("img", { name: "水杉 IME" }) as HTMLImageElement).src).toBe(
    "data:image/svg+xml,synthetic",
  );
  expect(screen.getByText("Metasequoia IME")).toBeTruthy();
  expect(screen.getByText("水杉 IME")).toBeTruthy();
  expect(screen.getByText("桌面输入体验")).toBeTruthy();
});

test("draws no card of its own inside the group that holds it", () => {
  const { container } = render(
    <AboutHeroSection logo="data:image/svg+xml,synthetic" description="桌面输入体验" />,
  );
  expect(container.firstElementChild?.classList.contains("section")).toBe(false);
});

test("the HarmonyOS hero states the product, the version on this platform and the copyright", () => {
  render(<HarmonyAboutHero version="1.4.2" platformLabel="HarmonyOS" update="idle" />);

  expect(screen.getByText("水杉输入法")).toBeTruthy();
  expect(screen.getByText("版本 1.4.2 · HarmonyOS")).toBeTruthy();
  expect(screen.getByText("© 2026 Metasequoia")).toBeTruthy();
  // 设计稿的标语和眉标只属于桌面版主视觉。
  expect(screen.queryByText("Metasequoia IME")).toBeNull();
});

test("the HarmonyOS hero leaves out the update pill when the host has no update check", () => {
  render(<HarmonyAboutHero version="1.4.2" platformLabel="HarmonyOS" update="idle" />);
  expect(screen.queryByRole("button")).toBeNull();
});

test("the update pill walks through 检查更新, 正在检查… and ✓ 已是最新版本", () => {
  const check = vi.fn();
  const { rerender } = render(
    <HarmonyAboutHero
      version="1.4.2"
      platformLabel="HarmonyOS"
      update="idle"
      onCheckForUpdate={check}
    />,
  );
  fireEvent.click(screen.getByRole("button", { name: "检查更新" }));
  expect(check).toHaveBeenCalledTimes(1);

  rerender(
    <HarmonyAboutHero
      version="1.4.2"
      platformLabel="HarmonyOS"
      update="checking"
      onCheckForUpdate={check}
    />,
  );
  const checking = screen.getByRole("button", { name: "正在检查…" }) as HTMLButtonElement;
  // 检查进行中再按一次只会重新开始检查。
  expect(checking.disabled).toBe(true);
  expect(checking.getAttribute("aria-busy")).toBe("true");
  expect(checking.className).not.toContain("bg-transparent");

  rerender(
    <HarmonyAboutHero
      version="1.4.2"
      platformLabel="HarmonyOS"
      update="latest"
      onCheckForUpdate={check}
    />,
  );
  const latest = screen.getByRole("button", { name: "✓ 已是最新版本" });
  // 已是最新版本时为无填充的强调色文字，另外两种状态则是强调色胶囊按钮。
  expect(latest.className).toContain("bg-transparent");
  expect(latest.className).toContain("var(--p-accent-text)");
});

test("an outcome other than current is read out under the copyright", () => {
  render(
    <HarmonyAboutHero
      version="1.4.2"
      platformLabel="HarmonyOS"
      update="idle"
      updateStatus="检查失败，请稍后重试"
      onCheckForUpdate={vi.fn()}
    />,
  );
  expect(screen.getByRole("status").textContent).toBe("检查失败，请稍后重试");
});

const initial: Snapshot = {
  format_version: 1,
  revision: 1,
  preferences: {
    scheme: "quanpin",
    shuangpin_profile: "xiaohe",
    candidate_page_size: 5,
    learning: true,
    chinese_punctuation: true,
  },
};

function renderAbout(host: Record<string, unknown>, openExternalUrl = vi.fn()) {
  render(
    <SettingsPage
      initialPage="about"
      client={{
        load: vi.fn().mockResolvedValue(initial),
        save: vi.fn(),
        readAppVersion: vi.fn().mockResolvedValue("1.4.2"),
        openExternalUrl,
        host: testHost({ platform: "harmony", ...host }),
      }}
    />,
  );
  return openExternalUrl;
}

const groupTitles = (scope: HTMLElement) =>
  [...scope.querySelectorAll("[data-group-title]")].map((node) => node.textContent);

test("HarmonyOS 关于 has the hero, 法律信息 with 查看 buttons and telemetry in its own group", async () => {
  const openExternalUrl = renderAbout({});
  const about = await screen.findByRole("group", { name: "关于" });

  expect(await within(about).findByText("版本 1.4.2 · HarmonyOS")).toBeTruthy();
  expect(within(about).getByRole("button", { name: "检查更新" })).toBeTruthy();
  // 手机没有下载行，所以不会留下空的「版本与更新」分组。
  expect(groupTitles(about)).toEqual(["法律信息", "隐私"]);
  expect(within(about).queryByText("当前版本")).toBeNull();

  fireEvent.click(within(about).getByRole("button", { name: "查看隐私政策" }));
  fireEvent.click(within(about).getByRole("button", { name: "查看开源许可" }));
  await waitFor(() => expect(openExternalUrl).toHaveBeenCalledTimes(2));
  expect(openExternalUrl.mock.calls[0][0]).toBe("https://msime.app/privacy/");
  expect(String(openExternalUrl.mock.calls[1][0])).toMatch(/^https:\/\//);
  expect(within(about).getByRole("switch", { name: "匿名使用统计" })).toBeTruthy();
});

test("the 2in1 names its form factor in the version line and keeps its download rows", async () => {
  renderAbout({ mobile_settings: false, panel_windows: true });
  const about = await screen.findByRole("group", { name: "关于" });

  expect(await within(about).findByText("版本 1.4.2 · HarmonyOS 2in1")).toBeTruthy();
  expect(groupTitles(about)).toEqual(["版本与更新", "法律信息", "隐私"]);
  expect(within(about).getByRole("button", { name: "其他平台下载" })).toBeTruthy();
});

test("the hero pill runs the release check and reports an outcome that is not current", async () => {
  const fetch = vi.fn().mockResolvedValue({ ok: true, status: 200, json: async () => [] });
  vi.stubGlobal("fetch", fetch);
  renderAbout({});
  const about = await screen.findByRole("group", { name: "关于" });

  fireEvent.click(within(about).getByRole("button", { name: "检查更新" }));
  expect(await within(about).findByText("暂无可用发行版")).toBeTruthy();
  expect(fetch).toHaveBeenCalledTimes(1);
  // 并非最新，所以胶囊按钮再次提供检查，而不是声称当前已是最新版本。
  expect(within(about).getByRole("button", { name: "检查更新" })).toBeTruthy();
});
