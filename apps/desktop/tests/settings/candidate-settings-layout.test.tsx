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

test("候选窗口页是一列：主流程四行，其余收在同一个「更多选项」里", async () => {
  await renderAppearance();
  expect(screen.queryByRole("tablist")).toBeNull();
  expect(screen.queryByRole("tabpanel")).toBeNull();
  // 颜色和明暗在侧栏的「主题」页，本页不再放跳转行。
  expect(screen.queryByRole("button", { name: "皮肤、颜色与明暗" })).toBeNull();
  const group = screen.getByRole("region", { name: "布局与字体" });
  const more = group.querySelector("details")!;
  expect(more.open).toBe(false);
  expect(within(more).getByText("更多选项")).toBeTruthy();
  const main = rowTitles(group).filter((title) => !rowTitles(more).includes(title));
  expect(main).toEqual(["候选项排列方式", "每页候选项数量", "候选字体", "字号"]);
  expect(rowTitles(more)).toEqual([
    "展开候选",
    "显示页码和翻页按钮",
    "显示水杉 logo",
    "跟随光标",
    "英文字体",
    "预编辑字号",
    "整体大小",
    "不透明度",
    "圆角大小",
    "内边距",
    "候选窗口预编辑",
    // 双拼预编辑在没有触屏方案的宿主上放在「输入 › 方案」的双拼详情里，这里不画。
    "行内预编辑",
  ]);
  // 只有一个折叠区，也没有「窗口样式」那样的第二个设置组。
  expect(group.querySelectorAll("details")).toHaveLength(1);
  expect(screen.queryByRole("region", { name: "窗口样式" })).toBeNull();
});

test("折叠区里的修改和主流程一样写进草稿，预览共用一个", async () => {
  const save = await renderAppearance();
  const preview = screen.getByRole("region", { name: "候选窗口预览" });
  expect(preview.querySelector(".candidate-brand")).toBeNull();
  fireEvent.click(screen.getByRole("switch", { name: "显示水杉 logo" }));
  expect(preview.querySelector(".candidate-brand")).not.toBeNull();
  fireEvent.click(screen.getByRole("radio", { name: "横向" }));
  expect(preview.querySelector('[data-preview-layout="horizontal"]')).not.toBeNull();
  expect(preview.querySelector("[data-preview-reserve]")).toBeNull();
  fireEvent.change(screen.getByRole("combobox", { name: "字号" }), { target: { value: "24" } });
  expect(preview.querySelector('[data-font-size="24"]')).not.toBeNull();
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
  const presets = screen.getByRole("radiogroup", { name: "候选字体" });
  expect(within(presets).getByRole("radio", { name: "默认" })).toHaveProperty("checked", true);
  expect(screen.queryByLabelText("主字体")).toBeNull();
  fireEvent.click(within(presets).getByRole("radio", { name: "自定义…" }));
  expect(within(presets).getByRole("radio", { name: "自定义…" })).toHaveProperty("checked", true);
  // 只亮出字体框，字体本身不变。
  const field = screen.getByLabelText("主字体") as HTMLInputElement;
  expect(field.value).toBe("Noto Sans SC");
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
