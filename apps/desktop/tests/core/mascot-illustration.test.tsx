// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, render } from "@testing-library/react";
import {
  CommunityGalleryFeedback,
  InputSourceStartupNotice,
  ResourcePackRow,
  type InputSourceStartupStatus,
  type ResourcePacks,
} from "@msime/ui";
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
