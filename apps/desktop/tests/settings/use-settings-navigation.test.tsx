// @vitest-environment jsdom
import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, expect, test } from "vitest";
import { useRef, useState } from "react";
import type { SettingsPageId } from "../../../../packages/ui/src/settings/settings-page-registry";
import { useSettingsNavigation } from "../../../../packages/ui/src/settings/use-settings-navigation";

afterEach(() => {
  cleanup();
  window.history.replaceState(null, "");
});

function useNavigationProbe(initialPage: SettingsPageId) {
  const [page, setPage] = useState<SettingsPageId>(initialPage);
  const [accountLoginReturnPage, setAccountLoginReturnPage] = useState<SettingsPageId | null>(null);
  const mobileLastPageByTab = useRef({
    home: "home" as SettingsPageId,
    chat: "chat" as SettingsPageId,
    "typing-statistics": "typing-statistics" as SettingsPageId,
    account: "account" as SettingsPageId,
  });
  const navigation = useSettingsNavigation({
    mobilePlatform: true,
    mobileHiddenPageIds: [],
    availablePages: (["home", "account", "community"] as SettingsPageId[]).map((id) => ({ id })),
    page,
    setPage,
    mobileLastPageByTab,
    hasHomePage: true,
    setCommunityDestination: () => {},
    setAccountLoginReturnPage,
    accountLoginReturnPage,
  });
  return { page, accountLoginReturnPage, ...navigation };
}

test("leaving the account tab clears the pending login return page", () => {
  const { result } = renderHook(() => useNavigationProbe("community"));

  act(() => result.current.openAccountLogin());
  expect(result.current.page).toBe("account");
  expect(result.current.accountLoginReturnPage).toBe("community");

  act(() => result.current.selectMobileTab("home"));
  expect(result.current.accountLoginReturnPage).toBeNull();
  act(() => result.current.selectMobileTab("account"));
  expect(result.current.page).toBe("account");

  act(() => result.current.finishAccountLogin());

  expect(result.current.page).toBe("account");
});

test("system back also clears a pending login return page", () => {
  const { result } = renderHook(() => useNavigationProbe("community"));

  act(() => result.current.openAccountLogin());
  act(() => {
    window.dispatchEvent(
      new PopStateEvent("popstate", { state: { msimeSettings: true, page: "community" } }),
    );
  });

  expect(result.current.page).toBe("community");
  expect(result.current.accountLoginReturnPage).toBeNull();
});
