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

function renderSection(preferences: Preferences, onChange = vi.fn()) {
  render(
    <InputSharedSettingsSection
      preferences={preferences}
      wordCharacter={defaultWordCharacter}
      navigation={defaultNavigation}
      frequency={defaultFrequency}
      ios={false}
      showInputModeHUD={false}
      showModeScope={false}
      onPreferencesChange={onChange}
    />,
  );
  return onChange;
}

test("非五笔方案不渲染造词落库位置", () => {
  for (const scheme of ["quanpin", "shuangpin", "cantonese"] as const) {
    renderSection({ ...preferences, scheme });
    expect(screen.queryByLabelText("五笔自造词位置")).toBeNull();
    cleanup();
  }
});

test("学习关着时不渲染造词落库位置", () => {
  renderSection({ ...preferences, scheme: "wubi", learning: false });
  expect(screen.queryByLabelText("五笔自造词位置")).toBeNull();
});

test("五笔方案学习开着时渲染落库位置，缺字段按默认显示", () => {
  renderSection({ ...preferences, scheme: "wubi" });
  expect((screen.getByLabelText("五笔自造词位置") as HTMLSelectElement).value).toBe("bottom");
  expect(screen.queryByLabelText("五笔造词时机")).toBeNull();
});

test("读出保存的非默认落库位置", () => {
  renderSection({
    ...preferences,
    scheme: "wubi",
    wubi_phrase_placement: "top",
  });
  expect((screen.getByLabelText("五笔自造词位置") as HTMLSelectElement).value).toBe("top");
});

test("切换位置只更新该字段", () => {
  const onChange = renderSection({ ...preferences, scheme: "wubi" });
  fireEvent.change(screen.getByLabelText("五笔自造词位置"), { target: { value: "top" } });
  expect(onChange).toHaveBeenCalledWith({ wubi_phrase_placement: "top" });
});
