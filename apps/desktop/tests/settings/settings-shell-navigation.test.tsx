// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, renderHook, screen } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { useSettingsShellNavigation } from "../../../../packages/ui/src/settings/use-settings-shell-navigation";
import type {
  SettingsPageGroup,
  SettingsPageItem,
} from "../../../../packages/ui/src/settings/settings-page-projections";
import type { SettingsPageId } from "../../../../packages/ui/src/settings/settings-page-registry";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const page = (id: SettingsPageId, title?: string): SettingsPageItem => ({
  id,
  title: title ?? id,
  icon: `${id}.svg`,
});

const groups: SettingsPageGroup[] = [
  { pages: [page("input", "输入")] },
  { title: "支持", pages: [page("feedback", "帮助与反馈"), page("about", "关于")] },
];

function renderNavigation(
  overrides: Partial<Parameters<typeof useSettingsShellNavigation>[0]> = {},
) {
  const launchPage = overrides.launchPage ?? { current: "input" as SettingsPageId };
  const selectPage = vi.fn();
  const setNavQuery = vi.fn();
  const result = renderHook(() =>
    useSettingsShellNavigation({
      page: "input",
      navigationPage: "input",
      launchPage,
      availablePages: groups.flatMap((group) => group.pages),
      sidebarGroups: groups,
      navQuery: "",
      setNavQuery,
      selectPage,
      ...overrides,
    }),
  );
  return { ...result, launchPage, selectPage, setNavQuery };
}

test("推入页返回沿历史记录，启动页返回替换到父页", () => {
  const back = vi.spyOn(window.history, "back").mockImplementation(() => undefined);
  const nested = renderNavigation({ page: "feedback", navigationPage: "feedback" });
  expect(nested.result.current.parentPage).toBeUndefined();
  act(() => nested.result.current.goBack());
  expect(back).toHaveBeenCalledTimes(1);
  expect(nested.selectPage).not.toHaveBeenCalled();

  const child = renderNavigation({
    page: "help",
    navigationPage: "feedback",
    launchPage: { current: "help" },
  });
  expect(child.result.current.parentPage?.id).toBe("feedback");
  act(() => child.result.current.goBack());
  expect(child.selectPage).toHaveBeenCalledWith("feedback", true);
  expect(child.launchPage.current).toBe("feedback");
});

test("侧栏搜索只保留标题命中的分组", () => {
  const { result } = renderNavigation({ navQuery: "  帮助  " });
  expect(result.current.shownSidebarGroups).toEqual([
    {
      title: "支持",
      pages: [groups[1].pages[0]],
    },
  ]);
});

test("搜索框回调使用外壳的查询 setter和页面选择器", () => {
  const { result, setNavQuery, selectPage } = renderNavigation({ navQuery: "输入" });
  render(result.current.navSearchField);
  const input = screen.getByRole("searchbox");
  fireEvent.change(input, { target: { value: "帮助" } });
  expect(setNavQuery).toHaveBeenCalledWith("帮助");
  fireEvent.keyDown(input, { key: "Enter" });
  expect(selectPage).toHaveBeenCalledWith("input");
});
