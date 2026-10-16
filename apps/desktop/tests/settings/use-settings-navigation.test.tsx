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
    more: "more" as SettingsPageId,
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

// 「手写输入」并进屏幕键盘页的桌面宿主上，handwriting 这个旧 id 从路由、导航调用和启动页都打开屏幕键盘页，而不是落到默认页。
test("an aliased page id opens the page that now carries it", () => {
  const aliases: Partial<Record<SettingsPageId, SettingsPageId>> = {
    handwriting: "screen-keyboard",
  };
  const { result, rerender } = renderHook(
    ({
      initialPage,
      route,
    }: {
      initialPage: SettingsPageId;
      route?: { page: string; nonce: number };
    }) => {
      const [page, setPage] = useState<SettingsPageId>(initialPage);
      const mobileLastPageByTab = useRef({
        home: "home" as SettingsPageId,
        more: "more" as SettingsPageId,
        "typing-statistics": "typing-statistics" as SettingsPageId,
        account: "account" as SettingsPageId,
      });
      const navigation = useSettingsNavigation({
        mobilePlatform: false,
        mobileHiddenPageIds: [],
        availablePages: (["input", "screen-keyboard", "voice"] as SettingsPageId[]).map((id) => ({
          id,
        })),
        pageAliases: aliases,
        page,
        setPage,
        mobileLastPageByTab,
        route,
        hasHomePage: false,
        setCommunityDestination: () => {},
        setAccountLoginReturnPage: () => {},
        accountLoginReturnPage: null,
      });
      return { page, ...navigation };
    },
    {
      initialProps: {
        initialPage: "handwriting" as SettingsPageId,
        route: undefined as { page: string; nonce: number } | undefined,
      },
    },
  );
  expect(result.current.page).toBe("screen-keyboard");

  act(() => result.current.selectPage("voice"));
  act(() => result.current.selectPage("handwriting"));
  expect(result.current.page).toBe("screen-keyboard");

  act(() => result.current.selectPage("voice"));
  rerender({ initialPage: "handwriting", route: { page: "handwriting", nonce: 1 } });
  expect(result.current.page).toBe("screen-keyboard");
});
