// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import type { Preferences } from "../../../../packages/ui/src/core/preferences-model";
import { InputSharedSettingsSection } from "../../../../packages/ui/src/settings/input-shared-settings-section";
import {
  defaultNavigation,
  defaultWordCharacter,
} from "../../../../packages/ui/src/settings/candidate-paging-preferences";
import { defaultFrequency } from "../../../../packages/ui/src/settings/frequency-preferences";
import { defaultMixedInput } from "../../../../packages/ui/src/settings/mixed-input-preferences";

afterEach(cleanup);

const preferences: Preferences = {
  scheme: "quanpin",
  shuangpin_profile: "xiaohe",
  candidate_page_size: 5,
  learning: true,
  chinese_punctuation: true,
};

const title = "移动光标后编辑光标后的部分";

function renderSection(
  saved: boolean | undefined,
  caretEditSuffix: boolean | undefined,
  onChange = vi.fn(),
) {
  render(
    <InputSharedSettingsSection
      preferences={{ ...preferences, caret_edit_suffix: saved }}
      wordCharacter={defaultWordCharacter}
      navigation={defaultNavigation}
      frequency={defaultFrequency}
      ios={false}
      showInputModeHUD={false}
      showModeScope={false}
      mixedInput={defaultMixedInput}
      onMixedInputChange={vi.fn()}
      quickPhraseCandidates
      caretEditSuffix={caretEditSuffix}
      onPreferencesChange={onChange}
    />,
  );
  return onChange;
}

test.each([undefined, false, true])(
  "光标后编辑开关读取偏好 %s，缺键按关，只更新这一项",
  (saved) => {
    const onChange = renderSection(saved, true);
    const toggle = screen.getByRole("switch", {
      name: new RegExp(`^${title}`),
    }) as HTMLInputElement;
    expect(toggle.checked).toBe(saved ?? false);
    fireEvent.click(toggle);
    expect(onChange).toHaveBeenCalledExactlyOnceWith({ caret_edit_suffix: !(saved ?? false) });
  },
);

test("光标后编辑在「候选与联想」的「更多选项」里，紧挨着快捷短语混输", () => {
  renderSection(undefined, true);
  const toggle = screen.getByRole("switch", { name: new RegExp(`^${title}`) });
  const fold = toggle.closest("details") as HTMLDetailsElement;
  expect(fold).toBeTruthy();
  expect(fold.open).toBe(false);
  const switches = within(fold).getAllByRole("switch");
  const index = switches.indexOf(toggle);
  expect(switches[index + 1]).toBe(screen.getByRole("switch", { name: /^快捷短语混输/ }));
});

test.each([undefined, false])("宿主没有声明能力（%s）时不列出光标后编辑", (caretEditSuffix) => {
  renderSection(true, caretEditSuffix);
  expect(screen.queryByRole("switch", { name: new RegExp(`^${title}`) })).toBeNull();
});
