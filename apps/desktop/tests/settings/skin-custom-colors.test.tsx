// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, render, screen, within } from "@testing-library/react";
import { SettingsPage, type Snapshot } from "@msime/ui";
import { testHost } from "../support/host";
import { settingsFormReady } from "../support/settings-form";

afterEach(cleanup);

const initial: Snapshot = {
  format_version: 1,
  revision: 1,
  preferences: {
    scheme: "quanpin",
    shuangpin_profile: "xiaohe",
    candidate_page_size: 6,
    learning: true,
    chinese_punctuation: true,
  },
};

async function openSkinPage(platform: string, host: Record<string, unknown> = {}) {
  render(
    <SettingsPage
      initialPage="skin"
      client={{
        load: async () => initial,
        save: vi.fn(),
        scanSkinCatalog: vi.fn().mockResolvedValue({ directory: "", packages: [], issues: [] }),
        host: testHost({
          platform,
          candidate_row_colors: true,
          candidate_selection_appearance: true,
          ...host,
        }),
      }}
    />,
  );
  await settingsFormReady();
  return screen.getByRole("region", { name: "自定义主题" });
}

/** 「自定义配色」折叠区里的取色器标题，按先后。 */
function pickerTitles(fold: HTMLElement) {
  return Array.from(fold.querySelectorAll("[data-row-title]"), (title) => title.textContent);
}

// 取色器收在「自定义配色」里、默认收起；悬停色也在其中，系统底色上推不出悬停色，深色自定义表面只能靠它。读编号色和边框色的宿主（Android、iOS）七个都给。
test("the colour pickers sit in a collapsed 自定义配色 fold, hover included", async () => {
  const group = await openSkinPage("windows", {
    candidate_border_color: true,
    candidate_number_color: true,
  });
  const fold = group.querySelector("details")!;
  expect(fold.open).toBe(false);
  expect(within(fold).getByText("自定义配色")).toBeTruthy();
  expect(pickerTitles(fold)).toEqual([
    "候选表面色",
    "候选文字颜色",
    "候选编号颜色",
    "候选强调色",
    "候选选中色",
    "候选悬停色",
    "候选边框色",
  ]);
});

// 桌面宿主的编号色和边框色固定跟随主题（宿主不读这两个槽位），不给取色器；其余五个照旧。
test.each(["macos", "windows", "linux"])(
  "%s offers no number or border colour picker",
  async (platform) => {
    const group = await openSkinPage(platform);
    const fold = group.querySelector("details")!;
    expect(pickerTitles(fold)).toEqual([
      "候选表面色",
      "候选文字颜色",
      "候选强调色",
      "候选选中色",
      "候选悬停色",
    ]);
    expect(screen.queryByLabelText("候选编号颜色")).toBeNull();
    expect(screen.queryByLabelText("候选边框色")).toBeNull();
  },
);

// 文档里存着旧的编号色和边框色时，桌面宿主不画它们（编号色由文字颜色推出，边框跟随主题），设置页的三处预览也不能画；读这两个槽位的宿主照旧画。Linux 没有工具栏预览，另外两处预览走同一个 `previewCustomTheme`。
test.each([
  ["macos", false],
  ["windows", false],
  ["windows", true],
] as const)(
  "the %s previews draw the number and border colours only where the host does (reads them: %s)",
  async (platform, drawn) => {
    render(
      <SettingsPage
        initialPage="appearance"
        client={{
          load: async () => ({
            ...initial,
            preferences: {
              ...initial.preferences,
              global_theme: "custom",
              custom_theme: {
                base: "system",
                candidate_colors: { text: "#102030", number: "#123456", border: "#654321" },
              },
            },
          }),
          save: vi.fn(),
          scanSkinCatalog: vi.fn().mockResolvedValue({ directory: "", packages: [], issues: [] }),
          host: testHost({
            platform,
            candidate_row_colors: true,
            candidate_selection_appearance: true,
            floating_toolbar: true,
            floating_toolbar_appearance: true,
            candidate_number_color: drawn,
            candidate_border_color: drawn,
          }),
        }}
      />,
    );
    await settingsFormReady();
    const previews = [
      document.querySelector<HTMLElement>(".appearance-candidate-preview"),
      document.querySelector<HTMLElement>(
        '[data-global-theme="custom"]:not(.appearance-candidate-preview):not([data-toolbar-preview])',
      ),
      document.querySelector<HTMLElement>("[data-toolbar-preview]"),
    ];
    for (const preview of previews) {
      expect(preview).not.toBeNull();
      const number = preview!.style.getPropertyValue("--cand-num");
      const border = preview!.style.getPropertyValue("--cand-border");
      if (drawn) {
        expect(number).toBe("#123456");
        expect(border).toBe("#654321");
      } else {
        // 选了文字颜色，编号色由它按约 62% 不透明推出，与原生宿主一致。
        expect(number).toBe("#1020309D");
        expect(border).not.toBe("#654321");
      }
    }
  },
);
