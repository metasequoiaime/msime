// @vitest-environment jsdom
import { renderHook } from "@testing-library/react";
import { expect, test, vi } from "vitest";
import { useSettingsPageActions, type UseSettingsPageActionsOptions } from "@msime/ui";

function options(): UseSettingsPageActionsOptions {
  return {
    navigation: {
      mobilePlatform: false,
      mobileHiddenPageIds: [],
      availablePages: [{ id: "appearance" }],
      page: "appearance",
      setPage: vi.fn(),
      mobileLastPageByTab: {
        current: {
          home: "home",
          community: "community",
          "typing-statistics": "typing-statistics",
          account: "account",
        },
      },
      hasHomePage: false,
      setCommunityDestination: vi.fn(),
      setAccountLoginReturnPage: vi.fn(),
      accountLoginReturnPage: null,
    },
    destination: {
      setShowTouchSkinEditor: vi.fn(),
      setCommunityDestination: vi.fn(),
    },
    account: {},
    external: {},
    about: {},
    screenKeyboard: {},
    shortcuts: {},
    helpcode: {},
    utilities: {},
    navigationActions: {},
    reload: {},
    status: {},
    selection: {},
    communityDestination: "saved-dictionary",
  } as unknown as UseSettingsPageActionsOptions;
}

test("composes navigation and page action callbacks around shared settings state", () => {
  const input = options();
  const { result } = renderHook(() => useSettingsPageActions(input));

  expect(result.current.mobileActiveTab).toBe("home");
  expect(result.current.communityView).toEqual({
    category: "dictionary",
    scope: "saved",
    initialMine: false,
  });

  result.current.settingsNavigationActions.onOpenAi();
  result.current.aboutSettingsActions.onHelp();
  result.current.settingsPageSelection.onOpenPage("feedback");

  expect(input.navigation.setPage).toHaveBeenNthCalledWith(1, "ai");
  expect(input.navigation.setPage).toHaveBeenNthCalledWith(2, "help");
  expect(input.navigation.setPage).toHaveBeenNthCalledWith(3, "feedback");
});
