// @vitest-environment jsdom
import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, expect, test } from "vitest";
import { useRef, useState } from "react";
import type { SettingsPageId } from "../../../../packages/ui/src/settings/settings-page-registry";
import { pushMobileSettingsState } from "../../../../packages/ui/src/settings/mobile-navigation";
import { useSettingsNavigation } from "../../../../packages/ui/src/settings/use-settings-navigation";

afterEach(() => {
  cleanup();
  window.history.replaceState(null, "");
});

function useNavigationProbe(
  initialPage: SettingsPageId,
  route?: { page: string; nonce: number },
  pageIds: readonly SettingsPageId[] = ["home", "account", "community"],
) {
  const [page, setPage] = useState<SettingsPageId>(initialPage);
  const [accountLoginReturnPage, setAccountLoginReturnPage] = useState<SettingsPageId | null>(null);
  const mobileLastPageByTab = useRef({
    home: "home" as SettingsPageId,
    community: "community" as SettingsPageId,
    "typing-statistics": "typing-statistics" as SettingsPageId,
    account: "account" as SettingsPageId,
  });
  const navigation = useSettingsNavigation({
    mobilePlatform: true,
    mobileHiddenPageIds: [],
    availablePages: pageIds.map((id) => ({ id })),
    page,
    setPage,
    mobileLastPageByTab,
    route,
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

test("mobile root pages keep their own navigation page", () => {
  const { result } = renderHook(() => useNavigationProbe("community"));

  expect(result.current.navigationPage).toBe("community");
});

test("mobile nested pages use their owning parent navigation page", () => {
  const { result } = renderHook(() =>
    useNavigationProbe("community", undefined, ["community", "ai", "chat"]),
  );

  act(() => result.current.selectPage("chat"));

  expect(result.current.navigationPage).toBe("ai");
});

test("desktop merged pages use their carrying page navigation page", () => {
  const { result } = renderHook(() => {
    const [page, setPage] = useState<SettingsPageId>("about");
    const mobileLastPageByTab = useRef({
      home: "home" as SettingsPageId,
      community: "community" as SettingsPageId,
      "typing-statistics": "typing-statistics" as SettingsPageId,
      account: "account" as SettingsPageId,
    });
    const navigation = useSettingsNavigation({
      mobilePlatform: false,
      mobileHiddenPageIds: [],
      availablePages: (["about", "feedback"] as SettingsPageId[]).map((id) => ({ id })),
      page,
      setPage,
      mobileLastPageByTab,
      hasHomePage: false,
      setCommunityDestination: () => {},
      setAccountLoginReturnPage: () => {},
      accountLoginReturnPage: null,
    });
    return { page, ...navigation };
  });

  act(() => result.current.selectPage("feedback"));

  expect(result.current.navigationPage).toBe("about");
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

const tabPages: readonly SettingsPageId[] = [
  "home",
  "skin",
  "appearance",
  "input",
  "community",
  "about",
  "account",
];

// 皮肤页回到「设置」的栈上：从「设置」里的链接打开它，底部仍点亮「设置」，历史记录里写的也是「设置」。
test("opening the skin page keeps it on the 设置 tab", () => {
  const { result } = renderHook(() => useNavigationProbe("home", undefined, tabPages));
  act(() => result.current.selectPage("input"));
  act(() => result.current.selectPage("skin"));
  expect(result.current.page).toBe("skin");
  expect(result.current.mobileActiveTab).toBe("home");
  expect(window.history.state).toMatchObject({ page: "skin", tab: "home" });

  // 切到「社区」再回来，「设置」标签仍是离开时的皮肤页。
  act(() => result.current.selectMobileTab("community"));
  expect(result.current.page).toBe("community");
  expect(result.current.mobileActiveTab).toBe("community");
  act(() => result.current.selectMobileTab("home"));
  expect(result.current.page).toBe("skin");
  expect(result.current.mobileActiveTab).toBe("home");
});

// 宿主路由（例如 HarmonyOS 键盘工具栏的「切换皮肤」打开 `settings:skin`）落在「设置」标签的皮肤页；`settings:community` 落在「社区」标签。
test("host routes to the skin and community pages land on their owning tabs", () => {
  const { result, rerender } = renderHook(
    ({ route }: { route?: { page: string; nonce: number } }) =>
      useNavigationProbe("account", route, tabPages),
    { initialProps: { route: undefined as { page: string; nonce: number } | undefined } },
  );
  rerender({ route: { page: "skin", nonce: 1 } });
  expect(result.current.page).toBe("skin");
  expect(result.current.mobileActiveTab).toBe("home");

  rerender({ route: { page: "community", nonce: 2 } });
  expect(result.current.page).toBe("community");
  expect(result.current.mobileActiveTab).toBe("community");
});

// 返回手势按历史记录里存的 `tab` 恢复。旧版本存的 `more`（全部设置）和 `skin`（皮肤标签）都已不是标签页，按页面归属回到对应标签：皮肤页和其他设置页回「设置」，「关于」回「我的」。
test("history restore drops the old 全部设置 and 皮肤 tabs by page ownership", () => {
  const { result } = renderHook(() => useNavigationProbe("community", undefined, tabPages));
  const restore = (state: Record<string, unknown>) =>
    act(() => {
      window.dispatchEvent(
        new PopStateEvent("popstate", { state: { msimeSettings: true, ...state } }),
      );
    });

  restore({ page: "skin", tab: "skin" });
  expect(result.current.page).toBe("skin");
  expect(result.current.mobileActiveTab).toBe("home");

  restore({ page: "appearance", tab: "skin" });
  expect(result.current.page).toBe("appearance");
  expect(result.current.mobileActiveTab).toBe("home");

  restore({ page: "input", tab: "more" });
  expect(result.current.page).toBe("input");
  expect(result.current.mobileActiveTab).toBe("home");

  restore({ page: "about", tab: "more" });
  expect(result.current.page).toBe("about");
  expect(result.current.mobileActiveTab).toBe("account");

  // 新的「社区」标签照常按存下的 `tab` 恢复。
  restore({ page: "community", tab: "community" });
  expect(result.current.page).toBe("community");
  expect(result.current.mobileActiveTab).toBe("community");
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
        community: "community" as SettingsPageId,
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

test("初始化设置历史时保留宿主字段和面板状态", () => {
  const current = Object.freeze({ host: "synthetic-host", panel: "voice" });
  window.history.replaceState(current, "");
  renderHook(() => useNavigationProbe("home"));
  expect(window.history.state).toEqual({
    host: "synthetic-host",
    panel: "voice",
    msimeSettings: true,
    page: "home",
    tab: "home",
  });
  expect(current).toEqual({ host: "synthetic-host", panel: "voice" });
});

test("替换设置页路由时只清除新状态的旧面板字段", () => {
  const current = Object.freeze({
    host: "synthetic-host",
    panel: "voice",
    msimeSettings: true,
    page: "home",
    tab: "home",
  });
  window.history.replaceState(current, "");
  const length = window.history.length;
  const { result } = renderHook(() => useNavigationProbe("home"));
  act(() => result.current.selectPage("input", true));
  expect(window.history.state).toEqual({
    host: "synthetic-host",
    msimeSettings: true,
    page: "input",
    tab: "home",
  });
  expect(window.history.length).toBe(length);
  expect(current.panel).toBe("voice");
});

test("嵌套设置路由推入新历史并保留宿主页面字段", () => {
  window.history.replaceState({ host: "synthetic-host", page: "input" }, "");
  const length = window.history.length;
  pushMobileSettingsState({ panel: "voice" });
  expect(window.history.state).toEqual({
    host: "synthetic-host",
    page: "input",
    panel: "voice",
    msimeSettings: true,
  });
  expect(window.history.length).toBe(length + 1);
});
