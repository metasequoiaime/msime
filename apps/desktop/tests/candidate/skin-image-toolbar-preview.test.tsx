// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, waitFor } from "@testing-library/react";
import cases from "../../../../shared/skins/toolbar-image-layout-cases.json";
import {
  toolbarImageLayout,
  toolbarPreviewRequests,
  type SkinToolbarImage,
  type ToolbarImageKey,
} from "../../../../packages/ui/src/skin/toolbar-image-layout";
import { SkinImageToolbarPreview } from "../../../../packages/ui/src/skin/skin-image-toolbar-preview";
import { defaultFloatingToolbar } from "../../../../packages/ui/src/settings/floating-toolbar-defaults";

afterEach(cleanup);

test("图片工具栏预览与 Rust 解析、布局使用同一组契约样例", () => {
  for (const fixture of cases) {
    expect(
      toolbarImageLayout(
        fixture.skin as SkinToolbarImage,
        fixture.shown as Array<[ToolbarImageKey, string]>,
      ),
    ).toEqual(fixture.expected);
  }
});

test("垂直自动排列忽略隐藏按钮，固定按钮不消耗位置，未列出的按钮不出现", () => {
  const skin: SkinToolbarImage = {
    ...(cases[0].skin as SkinToolbarImage),
    buttonFlow: { xDip: 0, yDip: 0, direction: "vertical", gapDip: 4 },
  };
  const layout = toolbarImageLayout(skin, [
    ["status", "ja"],
    ["fullwidth", "half"],
    ["settings", ""],
    ["emoji", ""],
  ]);
  expect(layout.buttons.map((slot) => [slot.key, slot.yDip, slot.image])).toEqual([
    ["status", 1, "status.png"],
    ["settings", 0, null],
    ["emoji", 24, null],
  ]);
  expect(layout.buttons[2].iconSizeDip).toBe(18);
  expect(
    toolbarImageLayout({ ...skin, buttonFlow: null }, [
      ["status", "en"],
      ["punctuation", "cn"],
      ["settings", ""],
    ]).buttons.map((slot) => slot.key),
  ).toEqual(["status"]);
  expect(
    toolbarPreviewRequests({
      ...defaultFloatingToolbar,
      english_mode: false,
      settings: false,
      emoji: true,
    }).map(([key]) => key),
  ).not.toContain("status");
});

test("背景外按钮保留完整尺寸，图片读取失败回退图标且刷新会重新读资源", async () => {
  const readImage = vi.fn(async () => ({ contentType: "image/png", bytes: [137, 80, 78, 71] }));
  const skin = cases[0].skin as SkinToolbarImage;
  const view = render(
    <SkinImageToolbarPreview skin={skin} id="synthetic" readImage={readImage} revision={0} />,
  );
  const preview = view.container.querySelector<HTMLElement>("[data-toolbar-image-preview]")!;
  expect(preview.style.width).toBe("210px");
  expect(preview.querySelector(".status-bar,.divider,.drag-handle")).toBeNull();
  expect(view.container.querySelector("[data-toolbar-image-button=fullwidth]")).toBeNull();
  await waitFor(() => expect(view.container.querySelectorAll("img").length).toBeGreaterThan(1));
  expect(readImage).toHaveBeenCalledWith("synthetic", "cn.png");
  const status = view.container.querySelector("[data-toolbar-image-button=status]")!;
  fireEvent.error(status.querySelectorAll("img")[1]);
  expect(status.textContent).toBe("中");
  readImage.mockClear();
  view.rerender(
    <SkinImageToolbarPreview skin={skin} id="synthetic" readImage={readImage} revision={1} />,
  );
  await waitFor(() => expect(readImage).toHaveBeenCalledWith("synthetic", "cn.png"));
});
