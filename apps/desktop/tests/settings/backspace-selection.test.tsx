// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import type { Preferences } from "../../../../packages/ui/src/core/preferences-model";
import { InputSharedSettingsSection } from "../../../../packages/ui/src/settings/input-shared-settings-section";
import {
  defaultNavigation,
  defaultWordCharacter,
} from "../../../../packages/ui/src/settings/candidate-paging-preferences";
import { defaultFrequency } from "../../../../packages/ui/src/settings/frequency-preferences";

afterEach(cleanup);

const preferences: Preferences = {
  scheme: "quanpin",
  shuangpin_profile: "xiaohe",
  candidate_page_size: 5,
  learning: true,
  chinese_punctuation: true,
};

test.each([undefined, false, true])("退格回退开关读取偏好 %s 并只更新该字段", (saved) => {
  const onChange = vi.fn();
  render(
    <InputSharedSettingsSection
      preferences={{ ...preferences, backspace_reverts_selection: saved }}
      wordCharacter={defaultWordCharacter}
      navigation={defaultNavigation}
      frequency={defaultFrequency}
      ios={false}
      showInputModeHUD={false}
      showModeScope={false}
      onPreferencesChange={onChange}
    />,
  );
  const toggle = screen.getByLabelText("退格键回退已选候选词") as HTMLInputElement;
  expect(toggle.checked).toBe(saved ?? true);
  fireEvent.click(toggle);
  expect(onChange).toHaveBeenCalledExactlyOnceWith({
    backspace_reverts_selection: !(saved ?? true),
  });
});
