// @vitest-environment jsdom
import { expect, test, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { SettingsInteractionPages } from "@msime/ui";

vi.mock("../../../../packages/ui/src/settings/screen-keyboard-settings-section", () => ({
  ScreenKeyboardSettingsSection: () => null,
}));

test("preserves the handwriting page boundary", () => {
  render(
    <SettingsInteractionPages
      screenKeyboard={{} as never}
      handwriting={{
        ios: false,
        android: false,
        harmony: false,
        macos: false,
        mobile: false,
        onOpenHandwriting: vi.fn(),
      }}
      handwritingDisabled={false}
      handwritingHidden={false}
    />,
  );

  expect(screen.getByRole("group", { name: "手写识别板" })).toBeTruthy();
});
