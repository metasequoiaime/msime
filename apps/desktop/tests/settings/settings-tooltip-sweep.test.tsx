// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import {
  InputModeShortcutsSection,
  SettingsTextareaField,
  ShortcutsIntroSection,
  StatisticsSection,
} from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const shortcutScope =
  "输入法快捷键仅在对应输入状态或候选窗口显示时生效。翻页方式和以词定字在「输入 › 选词与翻页」中设置。";

test("快捷键页的总说明收进「输入模式切换」组名旁的「?」，不再常驻页首", () => {
  render(
    <InputModeShortcutsSection
      keybindings={{
        switch_language_shift: true,
        switch_language_ctrl: false,
        switch_language_ctrl_alt_space: true,
        toggle_character_set_ctrl_shift_f: true,
        toggle_fullwidth_option_shift_h: true,
      }}
      onChange={vi.fn()}
      showModeSwitchShortcuts
      macos
      showFullwidthChord={false}
      windows={false}
      intro={<ShortcutsIntroSection mobile={false} />}
    />,
  );

  const help = screen.getByRole("button", { name: /^输入模式切换的说明：/ });
  expect(help.getAttribute("aria-label")).toContain(shortcutScope);
  // 说明只以视觉隐藏的形式留在组名旁，组里不再画一段常驻的灰字。
  const region = screen.getByRole("region", { name: "输入模式切换" });
  expect(region.querySelector("p")).toBeNull();
});

test("文本框的说明进标签旁的「?」，校验问题常驻，文本框经 aria-describedby 读到两者", () => {
  render(
    <SettingsTextareaField
      label="查汉字的五笔编码"
      description="输入或粘贴汉字，列出每个字的 86 和 98 五笔编码。"
      detail="最多 200 个字"
      ariaLabel="要查五笔编码的文字"
      value=""
      onChange={vi.fn()}
    />,
  );

  const field = screen.getByRole("textbox", { name: "要查五笔编码的文字" });
  expect(
    screen.getByRole("button", {
      name: "查汉字的五笔编码的说明：输入或粘贴汉字，列出每个字的 86 和 98 五笔编码。",
    }),
  ).toBeTruthy();
  const described = (field.getAttribute("aria-describedby") ?? "")
    .split(" ")
    .map((id) => document.getElementById(id)?.textContent);
  expect(described).toEqual(["最多 200 个字", "输入或粘贴汉字，列出每个字的 86 和 98 五笔编码。"]);
  // 标签指向文本框，「?」不在标签里，手机上点得开。
  expect(screen.getByText("查汉字的五笔编码").closest("label")?.htmlFor).toBe(field.id);
  expect(screen.getByRole("button", { name: /的说明：/ }).closest("label")).toBeNull();
});

test("统计图表的读法进标题旁的「?」，不在图下常驻", () => {
  render(
    <StatisticsSection id="chart" title="候选命中位置" help="只记位置，不记任何文字。">
      <div>图</div>
    </StatisticsSection>,
  );

  expect(screen.getByRole("heading", { name: "候选命中位置" })).toBeTruthy();
  expect(
    screen.getByRole("button", { name: "候选命中位置的说明：只记位置，不记任何文字。" }),
  ).toBeTruthy();
  expect(screen.getByRole("region", { name: "候选命中位置" }).querySelector("p")).toBeNull();
});
