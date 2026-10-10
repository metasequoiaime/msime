// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import type { ReactNode } from "react";
import type { NavigationPreferences, Preferences, WordCharacterPreferences } from "@msime/ui";
import { ToastProvider } from "../../../../packages/ui/src/core/toast";
import { CandidateFontSizeSliderRow } from "../../../../packages/ui/src/settings/candidate-sizing-section";
import { NavigationSection } from "../../../../packages/ui/src/settings/navigation-section";
import { AppearanceSettingsPage } from "../../../../packages/ui/src/settings/pages/appearance-page";
import { ShortcutSettingsPage } from "../../../../packages/ui/src/settings/pages/shortcuts-page";
import {
  defaultKeybindings,
  defaultNumberRowSelection,
} from "../../../../packages/ui/src/settings/keybinding-defaults";
import {
  SettingsFormContext,
  type SettingsFormModel,
} from "../../../../packages/ui/src/settings/settings-form-context";
import {
  ShortcutsSettingsSection,
  type ShortcutsSettingsSectionProps,
} from "../../../../packages/ui/src/settings/shortcuts-settings-section";

afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

const navigation: NavigationPreferences = {
  minus_equal: true,
  comma_period: true,
  brackets: false,
  tab: true,
  page_up_down: true,
  arrows: true,
};
const wordCharacter: WordCharacterPreferences = { enabled: true, keys: "brackets" };

function groupTitled(name: string): HTMLElement {
  return screen.getByRole("region", { name });
}

// ---- 候选栏 ----

test("the HarmonyOS phone's paging keys use the design's labels in a two-column grid under a title-only row", () => {
  const onChange = vi.fn();
  render(
    <NavigationSection
      navigation={{ ...navigation, mouse_wheel: true }}
      wordCharacter={wordCharacter}
      linux={false}
      harmonyPhone
      onChange={onChange}
    />,
  );

  const legend = screen.getByText("外接键盘翻页键");
  expect(legend.tagName).toBe("LEGEND");
  expect(legend.hasAttribute("data-row-title")).toBe(true);
  expect(screen.getAllByRole("checkbox").map((box) => box.closest("label")?.textContent)).toEqual([
    "- / =",
    "，/ 。",
    "[ / ]",
    "↑ / ↓",
    "Shift+Tab / Tab",
    "PageUp / PageDown",
  ]);
  expect(screen.queryByText(/鼠标滚轮/)).toBeNull();
  expect(screen.getByRole("checkbox", { name: "↑ / ↓" })).toHaveProperty("checked", true);

  // 打开以词定字正在使用的那对按键时，仍会把它让出去，与「输入」页一致。
  fireEvent.click(screen.getByRole("checkbox", { name: "[ / ]" }));
  expect(onChange).toHaveBeenCalledWith({
    navigation: { ...navigation, mouse_wheel: true, brackets: true },
    wordCharacter: { enabled: false, keys: "brackets" },
  });
});

test("the desktop paging list keeps its labels and the mouse wheel", () => {
  render(
    <NavigationSection
      navigation={navigation}
      wordCharacter={wordCharacter}
      linux={false}
      onChange={vi.fn()}
    />,
  );

  expect(screen.getByText("翻页方式")).toBeTruthy();
  expect(screen.getByRole("checkbox", { name: ", / ." })).toBeTruthy();
  expect(screen.getByRole("checkbox", { name: "鼠标滚轮（候选窗口支持时翻页）" })).toBeTruthy();
});

test("the 候选字号 slider shows a stored size outside 14–24 at the nearest end without writing it", () => {
  const onChange = vi.fn();
  const { rerender } = render(
    <CandidateFontSizeSliderRow preferences={{ candidate_font_size: 30 }} onChange={onChange} />,
  );

  const slider = screen.getByRole("slider", { name: "候选字号" });
  expect(slider.getAttribute("min")).toBe("14");
  expect(slider.getAttribute("max")).toBe("24");
  expect(slider.getAttribute("step")).toBe("1");
  expect(slider).toHaveProperty("value", "24");
  expect(screen.getByText("24px")).toBeTruthy();

  rerender(
    <CandidateFontSizeSliderRow preferences={{ candidate_font_size: 12 }} onChange={onChange} />,
  );
  expect(screen.getByRole("slider", { name: "候选字号" })).toHaveProperty("value", "14");
  expect(onChange).not.toHaveBeenCalled();

  fireEvent.change(screen.getByRole("slider", { name: "候选字号" }), { target: { value: "20" } });
  expect(onChange).toHaveBeenCalledWith({ candidate_font_size: 20 });
});

function appearanceForm(setDraft = vi.fn()): SettingsFormModel {
  const draft = {
    scheme: "quanpin",
    candidate_page_size: 5,
    candidate_font_size: 18,
    navigation,
  } as unknown as Preferences;
  return {
    client: { listFontFamilies: vi.fn(async () => []) },
    host: { platform: "harmony", fixed_candidate_layout: "horizontal" },
    harmonyPlatform: true,
    mobilePlatform: true,
    settingsPlatform: "harmony",
    showCandidateFontControls: true,
    showCandidatePreeditFont: true,
    showCandidateEnglishFont: true,
    showShuangpinPreedit: true,
    showCandidateFollowCursor: false,
    showCandidateWindowScale: false,
    showCandidateWindowOpacity: false,
    showCandidateCornerRadius: false,
    snapshot: undefined,
    draft,
    setDraft,
    busy: false,
    page: "appearance",
    mobileKeyboardFeedback: undefined,
    mobileKeyboardFeedbackBusy: false,
    saveMobileKeyboardFeedback: vi.fn(),
    selectPage: vi.fn(),
    wordCharacter,
  } as unknown as SettingsFormModel;
}

test("the HarmonyOS phone 候选栏 page has only the 候选栏 group, with the rest under 更多选项", () => {
  const setDraft = vi.fn();
  render(
    <SettingsFormContext.Provider value={appearanceForm(setDraft)}>
      <div data-platform="harmony">
        <AppearanceSettingsPage />
      </div>
    </SettingsFormContext.Provider>,
  );

  expect(
    Array.from(document.querySelectorAll("[data-group-title]")).map((node) => node.textContent),
  ).toEqual(["候选栏"]);
  // 外接键盘的翻页键在「外接键盘快捷键」页，不在这里。
  expect(screen.queryByText("外接键盘翻页键")).toBeNull();
  // 没有预览、没有主题链接、没有每页数量行（手机候选条从不读取它），也没有「候选栏高度」和「翻页按钮」。
  expect(screen.queryByText("皮肤、颜色与明暗")).toBeNull();
  expect(screen.queryByText("每页候选项数量")).toBeNull();
  expect(screen.queryByText("候选栏高度")).toBeNull();
  expect(screen.queryByText("翻页按钮")).toBeNull();

  const candidate = groupTitled("候选栏");
  expect(within(candidate).getByRole("slider", { name: "候选字号" })).toHaveProperty("value", "18");
  const more = candidate.querySelector("details");
  expect(more).not.toBeNull();
  for (const title of ["候选字体", "预编辑字号", "候选栏预编辑"]) {
    expect(screen.getByText(title).closest("details")).toBe(more);
  }
});

// ---- 外接键盘快捷键 ----

function shortcuts(
  overrides: Partial<ShortcutsSettingsSectionProps> = {},
  wrap: (node: ReactNode) => ReactNode = (node) => node,
) {
  const props: ShortcutsSettingsSectionProps = {
    disabled: false,
    hidden: false,
    mobile: true,
    keybindings: { ...defaultKeybindings },
    onKeybindingsChange: vi.fn(),
    showModeSwitchShortcuts: true,
    macos: false,
    linux: false,
    showFullwidthChord: false,
    fullwidthChord: "Alt+Shift+H",
    windows: false,
    navigation,
    wordCharacter,
    numberRowSelection: true,
    showNumberRowSelection: true,
    onNumberRowSelectionChange: vi.fn(),
    showPanelShortcuts: false,
    harmony: true,
    ...overrides,
  };
  render(wrap(<ShortcutsSettingsSection {...props} />));
  return props;
}

test("the HarmonyOS phone shortcuts page has no intro or notes and offers only the chords the keyboard routes", () => {
  shortcuts();

  expect(
    Array.from(document.querySelectorAll("[data-group-title]")).map((node) => node.textContent),
  ).toEqual(["通用", "候选", "按键速查"]);
  expect(screen.queryByText(/输入法快捷键仅在/)).toBeNull();
  expect(screen.queryByText("输入和选取候选词时使用")).toBeNull();
  expect(screen.queryByText(/在当前输入上下文中切换中英文模式/)).toBeNull();

  const language = screen.getByRole("combobox", { name: "中/英文切换" });
  expect(Array.from((language as HTMLSelectElement).options).map((option) => option.text)).toEqual([
    "Shift",
    "Ctrl",
    "Ctrl + Alt + Space",
    "无",
  ]);
  const characterSet = screen.getByRole("combobox", { name: "简繁切换" });
  expect(
    Array.from((characterSet as HTMLSelectElement).options).map((option) => option.text),
  ).toEqual(["Ctrl + Shift + F", "无"]);
  for (const unrouted of [
    /CapsLock/,
    /全角/,
    /标点切换/,
    /打开设置/,
    /第二、三候选/,
    /Tab 键在候选间移动/,
  ]) {
    expect(screen.queryByText(unrouted)).toBeNull();
  }

  // 速查行仍在，折叠在「按键速查」下。
  const reference = groupTitled("按键速查");
  expect(within(reference).getByText("编辑输入串").closest("details")).not.toBeNull();
});

test("the HarmonyOS phone shortcut selects write the routed bindings", () => {
  const props = shortcuts();

  fireEvent.change(screen.getByRole("combobox", { name: "中/英文切换" }), {
    target: { value: "none" },
  });
  expect(props.onKeybindingsChange).toHaveBeenLastCalledWith({
    switch_language_shift: false,
    switch_language_ctrl: false,
    switch_language_ctrl_alt_space: false,
  });

  fireEvent.change(screen.getByRole("combobox", { name: "中/英文切换" }), {
    target: { value: "ctrl" },
  });
  expect(props.onKeybindingsChange).toHaveBeenLastCalledWith({
    switch_language_shift: false,
    switch_language_ctrl: true,
    switch_language_ctrl_alt_space: false,
  });

  fireEvent.change(screen.getByRole("combobox", { name: "简繁切换" }), {
    target: { value: "none" },
  });
  expect(props.onKeybindingsChange).toHaveBeenLastCalledWith({
    toggle_character_set_ctrl_shift_f: false,
  });
});

test("the HarmonyOS phone shortcut selects show the stored choice", () => {
  shortcuts({
    keybindings: {
      ...defaultKeybindings,
      switch_language_shift: false,
      switch_language_ctrl: false,
      switch_language_ctrl_alt_space: false,
      toggle_character_set_ctrl_shift_f: false,
    },
  });

  expect(screen.getByRole("combobox", { name: "中/英文切换" })).toHaveProperty("value", "none");
  expect(screen.getByRole("combobox", { name: "简繁切换" })).toHaveProperty("value", "none");
});

test("恢复默认 writes the default bindings and number-row selection and confirms with a toast", () => {
  vi.useFakeTimers();
  const props = shortcuts({ numberRowSelection: false }, (node) => (
    <ToastProvider>{node}</ToastProvider>
  ));

  const candidate = groupTitled("候选");
  expect(within(candidate).getByRole("switch", { name: "数字键选词" })).toHaveProperty(
    "checked",
    false,
  );
  fireEvent.click(within(candidate).getByRole("button", { name: "恢复默认" }));

  expect(props.onKeybindingsChange).toHaveBeenCalledWith(defaultKeybindings);
  expect(props.onNumberRowSelectionChange).toHaveBeenCalledWith(defaultNumberRowSelection);
  expect(defaultNumberRowSelection).toBe(true);
  expect(screen.getByRole("status").textContent).toBe("已恢复默认快捷键");
  act(() => {
    vi.advanceTimersByTime(1600);
  });
  expect(screen.getByRole("status").textContent).toBe("");
});

test("HarmonyOS 2in1 keeps the desktop shortcuts page", () => {
  shortcuts({ mobile: false });

  expect(screen.getByText(/输入法快捷键仅在/)).toBeTruthy();
  expect(groupTitled("输入模式切换")).toBeTruthy();
  expect(groupTitled("候选操作")).toBeTruthy();
  expect(screen.queryByRole("button", { name: "恢复默认" })).toBeNull();
  expect(screen.getByRole("switch", { name: /Ctrl\+Shift\+F 切换繁体输出/ })).toBeTruthy();
});

test("the HarmonyOS phone shortcuts page carries the external keyboard paging keys between 候选 and 按键速查", () => {
  shortcuts({
    paging: (
      <NavigationSection
        navigation={navigation}
        wordCharacter={wordCharacter}
        linux={false}
        harmonyPhone
        onChange={vi.fn()}
      />
    ),
  });

  expect(
    Array.from(document.querySelectorAll("[data-group-title]")).map((node) => node.textContent),
  ).toEqual(["通用", "候选", "翻页", "按键速查"]);
  expect(within(groupTitled("翻页")).getByText("外接键盘翻页键")).toBeTruthy();
});

test("only the HarmonyOS phone shows a paging group on the shortcuts page", () => {
  shortcuts({ harmony: false, paging: <span>翻页键</span> });
  expect(screen.queryByText("翻页键")).toBeNull();
});

test("the HarmonyOS phone 外接键盘快捷键 page writes the paging keys it shows", () => {
  const setDraft = vi.fn();
  render(
    <SettingsFormContext.Provider
      value={
        {
          ...appearanceForm(setDraft),
          page: "shortcuts",
          keybindings: { ...defaultKeybindings },
          showModeSwitchShortcuts: true,
          showNumberRowSelection: true,
          showPanelShortcuts: false,
          fullwidthChord: "Alt+Shift+H",
        } as unknown as SettingsFormModel
      }
    >
      <ToastProvider>
        <ShortcutSettingsPage />
      </ToastProvider>
    </SettingsFormContext.Provider>,
  );

  const paging = groupTitled("翻页");
  fireEvent.click(within(paging).getByRole("checkbox", { name: "PageUp / PageDown" }));
  const update = setDraft.mock.calls.at(-1)?.[0] as (current: Preferences) => Preferences;
  expect(update({ navigation } as Preferences).navigation).toEqual({
    ...navigation,
    page_up_down: false,
  });
});
