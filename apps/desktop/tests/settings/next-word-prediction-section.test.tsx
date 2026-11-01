// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import type { Preferences } from "../../../../packages/ui/src/core/preferences-model";
import { InputSharedSettingsSection } from "../../../../packages/ui/src/settings/input-shared-settings-section";
import { NextWordPredictionSection } from "../../../../packages/ui/src/settings/next-word-prediction-section";
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

const title = /^上屏后联想/;

function renderInputSection(saved: boolean | undefined, nextWordPrediction: boolean | undefined) {
  const onChange = vi.fn();
  render(
    <InputSharedSettingsSection
      preferences={{ ...preferences, next_word_prediction: saved }}
      wordCharacter={defaultWordCharacter}
      navigation={defaultNavigation}
      frequency={defaultFrequency}
      ios={false}
      showInputModeHUD={false}
      showModeScope={false}
      mixedInput={defaultMixedInput}
      onMixedInputChange={vi.fn()}
      nextWordPrediction={nextWordPrediction}
      onPreferencesChange={onChange}
    />,
  );
  return onChange;
}

test.each([undefined, false, true])(
  "输入页的上屏后联想读取偏好 %s，缺键按关，只更新这一项",
  (saved) => {
    const onChange = renderInputSection(saved, true);
    const toggle = screen.getByRole("switch", { name: title }) as HTMLInputElement;
    expect(toggle.checked).toBe(saved ?? false);
    fireEvent.click(toggle);
    expect(onChange).toHaveBeenCalledExactlyOnceWith({ next_word_prediction: !(saved ?? false) });
    // 桌面候选窗没有展开、关闭键，不列出按键样式。
    expect(screen.queryByText("联想栏按键")).toBeNull();
  },
);

test.each([undefined, false])("宿主没有声明能力（%s）时输入页不列出上屏后联想", (capability) => {
  renderInputSection(true, capability);
  expect(screen.queryByRole("switch", { name: title })).toBeNull();
});

test("触屏宿主的联想栏按键只在开关打开时可选，选择只更新按键样式", () => {
  const onChange = vi.fn();
  const { rerender } = render(
    <NextWordPredictionSection value={false} showButtons onChange={onChange} />,
  );
  const select = screen.getByRole("combobox") as HTMLSelectElement;
  expect(select.disabled).toBe(true);
  expect(select.value).toBe("expand_only");
  rerender(
    <NextWordPredictionSection value buttons="close_only" showButtons onChange={onChange} />,
  );
  expect(select.disabled).toBe(false);
  expect(select.value).toBe("close_only");
  fireEvent.change(select, { target: { value: "close_and_expand" } });
  expect(onChange).toHaveBeenCalledExactlyOnceWith({
    next_word_prediction_buttons: "close_and_expand",
  });
});
