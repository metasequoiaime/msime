// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { SettingsPage, type Snapshot } from "@msime/ui";
import { testHost } from "../support/host";
import { settingsFormReady, saveSettingsNow } from "../support/settings-form";

afterEach(cleanup);

const initial: Snapshot = {
  format_version: 1,
  revision: 7,
  preferences: {
    scheme: "quanpin",
    shuangpin_profile: "xiaohe",
    candidate_page_size: 5,
    candidate_font_size: 18,
    learning: true,
    chinese_punctuation: true,
  },
};

async function renderAppearance(preferences: Snapshot["preferences"] = initial.preferences) {
  const loaded = { ...initial, preferences };
  const save = vi.fn().mockImplementation(async (revision, saved) => ({
    ...loaded,
    revision: revision + 1,
    preferences: saved,
  }));
  render(
    <SettingsPage
      initialPage="appearance"
      client={{
        load: async () => loaded,
        save,
        host: testHost({
          platform: "macos",
          candidate_font_controls: true,
          candidate_page_number: true,
          candidate_follow_cursor: true,
          candidate_window_scale: true,
          candidate_window_opacity: true,
          candidate_corner_radius: true,
          candidate_padding: true,
        }),
      }}
    />,
  );
  await settingsFormReady();
  return save;
}

/** 「候选窗口」页里各行的标题，按页面上的先后。 */
function rowTitles(container: HTMLElement) {
  return Array.from(container.querySelectorAll("[data-row-title]"), (title) => title.textContent);
}

test("顶部四标签按功能分组，只显示当前分区，共用预览位于标签之后", async () => {
  const save = await renderAppearance();
  const tablist = screen.getByRole("tablist", { name: "候选窗口设置" });
  expect(
    within(tablist)
      .getAllByRole("tab")
      .map((tab) => tab.textContent),
  ).toEqual(["布局", "字体与大小", "窗口样式", "预编辑"]);
  const preview = screen.getByRole("region", { name: "候选窗口预览" });
  expect(tablist.compareDocumentPosition(preview) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  const rows = {
    布局: [
      "候选项排列方式",
      "每页候选项数量",
      "展开候选",
      "显示页码和翻页按钮",
      "显示水杉 logo",
      "跟随光标",
    ],
    字体与大小: ["候选字体", "字号", "英文字体", "整体大小"],
    窗口样式: [
      "收紧窗口宽度",
      "不透明度",
      "圆角大小",
      "内边距",
      "固定候选徽标",
      "固定候选徽标样式",
      "AI 联想候选徽标样式",
    ],
    预编辑: ["预编辑字号", "候选窗口预编辑", "行内预编辑"],
  };
  const content = document.getElementById("settings-content")!;
  for (const [name, titles] of Object.entries(rows)) {
    content.scrollTop = name === "布局" ? 0 : 300;
    fireEvent.click(within(tablist).getByRole("tab", { name }));
    expect(content.scrollTop).toBe(0);
    expect(screen.getAllByRole("tabpanel")).toHaveLength(1);
    const panel = screen.getByRole("tabpanel", { name });
    expect(rowTitles(panel)).toEqual(titles);
    expect(panel.querySelector("details")).toBeNull();
    expect(screen.getByRole("region", { name: "候选窗口预览" })).toBe(preview);
  }
  expect(save).not.toHaveBeenCalled();
  expect(screen.queryByRole("button", { name: "皮肤、颜色与明暗" })).toBeNull();
});

test("候选徽标只随 macOS 出现，改动写进同一草稿并保存", async () => {
  const save = await renderAppearance();
  fireEvent.click(screen.getByRole("tab", { name: "窗口样式" }));
  // 出厂默认：固定排位徽标开、样式回形针、AI 徽标机器人。
  const fixedBadge = screen.getByRole("switch", { name: "固定候选徽标" }) as HTMLInputElement;
  expect(fixedBadge.checked).toBe(true);
  const fixedStyle = screen.getByRole("combobox", {
    name: "固定候选徽标样式",
  }) as HTMLSelectElement;
  expect(fixedStyle.value).toBe("paperclip");
  const aiStyle = screen.getByRole("combobox", {
    name: "AI 联想候选徽标样式",
  }) as HTMLSelectElement;
  expect(aiStyle.value).toBe("robot");

  fireEvent.click(fixedBadge);
  fireEvent.change(fixedStyle, { target: { value: "dot" } });
  fireEvent.change(aiStyle, { target: { value: "whale" } });
  saveSettingsNow();
  await screen.findByText("已保存");
  expect(save).toHaveBeenLastCalledWith(
    expect.any(Number),
    expect.objectContaining({
      candidate_fixed_badge: false,
      candidate_fixed_badge_style: "dot",
      candidate_ai_badge_style: "whale",
    }),
  );
});

test("顶部标签支持方向键、Home 和 End，并同步焦点及选中状态", async () => {
  await renderAppearance();
  const layout = screen.getByRole("tab", { name: "布局" });
  const font = screen.getByRole("tab", { name: "字体与大小" });
  const preedit = screen.getByRole("tab", { name: "预编辑" });
  layout.focus();
  for (const [from, key, target] of [
    [layout, "ArrowRight", font],
    [font, "End", preedit],
    [preedit, "ArrowRight", layout],
    [layout, "ArrowLeft", preedit],
    [preedit, "Home", layout],
  ] as const) {
    fireEvent.keyDown(from, { key });
    expect(document.activeElement).toBe(target);
    expect(target.getAttribute("aria-selected")).toBe("true");
    expect(screen.getByRole("tabpanel").id).toBe(target.getAttribute("aria-controls"));
  }
});

test("跨标签修改写进同一草稿并保存，预览共用一个", async () => {
  const save = await renderAppearance();
  const preview = screen.getByRole("region", { name: "候选窗口预览" });
  expect(preview.querySelector(".candidate-brand")).toBeNull();
  fireEvent.click(screen.getByRole("switch", { name: "显示水杉 logo" }));
  expect(preview.querySelector(".candidate-brand")).not.toBeNull();
  fireEvent.click(screen.getByRole("radio", { name: "横向" }));
  expect(preview.querySelector('[data-preview-layout="horizontal"]')).not.toBeNull();
  expect(preview.querySelector("[data-preview-reserve]")).toBeNull();
  fireEvent.click(screen.getByRole("tab", { name: "字体与大小" }));
  fireEvent.change(screen.getByRole("combobox", { name: "字号" }), { target: { value: "24" } });
  expect(preview.querySelector('[data-font-size="24"]')).not.toBeNull();
  fireEvent.click(screen.getByRole("tab", { name: "预编辑" }));
  fireEvent.change(screen.getByRole("combobox", { name: "候选窗口预编辑" }), {
    target: { value: "empty" },
  });
  expect(screen.getByRole("region", { name: "候选窗口预览" })).toBe(preview);
  saveSettingsNow();
  await screen.findByText("已保存");
  expect(save).toHaveBeenLastCalledWith(
    initial.revision + save.mock.calls.length - 1,
    expect.objectContaining({
      candidate_layout: "horizontal",
      candidate_font_size: 24,
      candidate_preedit_style: "empty",
      show_app_logo: true,
    }),
  );
});

test("选「自定义…」才出现字体框，选回预设就收起；非预设字体直接显示字体框", async () => {
  const save = await renderAppearance();
  fireEvent.click(screen.getByRole("tab", { name: "字体与大小" }));
  const presets = screen.getByRole("radiogroup", { name: "候选字体" });
  expect(within(presets).getByRole("radio", { name: "默认" })).toHaveProperty("checked", true);
  expect(screen.queryByLabelText("主字体")).toBeNull();
  fireEvent.click(within(presets).getByRole("radio", { name: "自定义…" }));
  expect(within(presets).getByRole("radio", { name: "自定义…" })).toHaveProperty("checked", true);
  // 只亮出字体框，字体本身不变。
  const field = screen.getByLabelText("主字体") as HTMLInputElement;
  expect(field.value).toBe("Noto Sans SC");
  fireEvent.click(screen.getByRole("tab", { name: "布局" }));
  fireEvent.click(screen.getByRole("button", { name: "主题" }));
  fireEvent.click(screen.getByRole("button", { name: "候选窗口" }));
  fireEvent.click(screen.getByRole("tab", { name: "字体与大小" }));
  expect(screen.getByRole("combobox", { name: "主字体" })).toBe(field);
  expect(within(presets).getByRole("radio", { name: "自定义…" })).toHaveProperty("checked", true);
  // 没有预设的字体（例如霞鹜文楷）仍然可以填。
  fireEvent.change(field, { target: { value: "LXGW WenKai" } });
  expect(within(presets).getByRole("radio", { name: "自定义…" })).toHaveProperty("checked", true);
  saveSettingsNow();
  await screen.findByText("已保存");
  expect(save).toHaveBeenLastCalledWith(
    expect.any(Number),
    expect.objectContaining({ candidate_font_family: "LXGW WenKai" }),
  );
  fireEvent.click(within(presets).getByRole("radio", { name: "黑体" }));
  expect(screen.queryByLabelText("主字体")).toBeNull();
  expect(within(presets).getByRole("radio", { name: "黑体" })).toHaveProperty("checked", true);
});

test("存着非预设字体时一打开就是「自定义…」和字体框", async () => {
  await renderAppearance({ ...initial.preferences, candidate_font_family: "LXGW WenKai" });
  fireEvent.click(screen.getByRole("tab", { name: "字体与大小" }));
  const presets = screen.getByRole("radiogroup", { name: "候选字体" });
  expect(within(presets).getByRole("radio", { name: "自定义…" })).toHaveProperty("checked", true);
  expect((screen.getByLabelText("主字体") as HTMLInputElement).value).toBe("LXGW WenKai");
});

test("手机候选栏也是一列，字号在主流程里", async () => {
  render(
    <SettingsPage
      initialPage="appearance"
      client={{
        load: async () => initial,
        save: vi.fn(),
        host: testHost({ platform: "ios", candidate_font_controls: true }),
      }}
    />,
  );
  await settingsFormReady();
  expect(screen.queryByRole("tablist")).toBeNull();
  expect(screen.getByRole("combobox", { name: "字号" })).toBeTruthy();
  // 触屏键盘把拼写写进输入框、候选栏不画它，没有「候选栏预编辑」。
  expect(screen.queryByRole("combobox", { name: "候选栏预编辑" })).toBeNull();
  expect(screen.queryByRole("combobox", { name: "候选窗口预编辑" })).toBeNull();
});
