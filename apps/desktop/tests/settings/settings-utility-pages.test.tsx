// @vitest-environment jsdom
import { expect, test, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { SettingsUtilityPages } from "@msime/ui";

test("keeps utility page fieldsets composed behind one component", () => {
  render(
    <SettingsUtilityPages
      helpcode={{
        value: { scheme: "quanpin" } as never,
        mobile: false,
        showShiftEntry: false,
        disabled: false,
        hidden: true,
        onChange: vi.fn(),
      }}
      shortcuts={{
        disabled: false,
        hidden: true,
        mobile: false,
        keybindings: {} as never,
        onKeybindingsChange: vi.fn(),
        onInputModeHUDChange: vi.fn(),
        showModeSwitchShortcuts: false,
        macos: false,
        showInputModeHUD: false,
        inputModeHUD: false,
        showFullwidthChord: false,
        fullwidthChord: "",
        windows: false,
        navigation: {} as never,
        numberRowSelection: false,
        showNumberRowSelection: false,
        onNumberRowSelectionChange: vi.fn(),
        showPanelShortcuts: false,
        harmony: false,
        showDesktopMaintenanceShortcuts: false,
        linux: false,
        maintenanceChord: "",
        showRestartInputMethod: false,
      }}
      utilities={{
        disabled: false,
        hidden: true,
        historyEnabled: false,
        persistedHistoryEnabled: false,
        ios: false,
        macos: false,
        onToggleClipboard: vi.fn(),
        onError: vi.fn(),
        onOpenPanel: vi.fn(),
        localModes: {} as never,
        onLocalModesChange: vi.fn(),
      }}
      help={{
        busy: false,
        hidden: true,
        macos: false,
        mobile: false,
        ios: false,
        android: false,
        platformHelpIntro: "",
        platformQuickStart: "",
        platformNetworkDescription: "",
      }}
    />,
  );

  expect(screen.queryByRole("heading", { name: "快捷键" })).toBeNull();
});
