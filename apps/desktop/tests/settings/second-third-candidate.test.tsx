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

test.each([undefined, false, true])("二三候选开关读取偏好 %s 并只更新该字段", (saved) => {
  const onChange = vi.fn();
  render(
    <InputSharedSettingsSection
      preferences={
        saved === undefined
          ? preferences
          : { ...preferences, second_third_candidate: { enabled: saved, keys: "semicolon_quote" } }
      }
      wordCharacter={defaultWordCharacter}
      navigation={defaultNavigation}
      frequency={defaultFrequency}
      ios={false}
      showInputModeHUD={false}
      showModeScope={false}
      onPreferencesChange={onChange}
    />,
  );
  const toggle = screen.getByLabelText("二三候选") as HTMLInputElement;
  expect(toggle.checked).toBe(saved ?? false);
  fireEvent.click(toggle);
  // 键位随开关一起写，偏好里不会留下缺字段的对象（同 Windows 设置页）。
  expect(onChange).toHaveBeenCalledExactlyOnceWith({
    second_third_candidate: { enabled: !(saved ?? false), keys: "semicolon_quote" },
  });
});
