// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import type { SettingsPageGroup } from "../../../../packages/ui/src/settings/settings-page-projections";
import { SettingsNavigationSearch, type SettingsPageId } from "@msime/ui";

afterEach(cleanup);

const groups: SettingsPageGroup[] = [
  {
    title: "输入",
    pages: [{ id: "input", title: "输入", icon: "input.svg" }],
  },
  {
    title: "外观",
    pages: [{ id: "appearance", title: "外观", icon: "appearance.svg" }],
  },
];

test("设置导航搜索把回车交给筛选结果的第一页", () => {
  const onSelectPage = vi.fn<(page: SettingsPageId) => void>();
  render(
    <SettingsNavigationSearch
      groups={groups}
      value="外"
      onChange={() => {}}
      onSelectPage={onSelectPage}
    />,
  );

  fireEvent.keyDown(screen.getByRole("searchbox", { name: "搜索设置" }), { key: "Enter" });
  expect(onSelectPage).toHaveBeenCalledExactlyOnceWith("input");
});

test("设置导航搜索在组词期间保留输入，并用 Escape 清空普通查询", () => {
  const onChange = vi.fn();
  const onSelectPage = vi.fn();
  render(
    <SettingsNavigationSearch
      groups={groups}
      value="查询"
      onChange={onChange}
      onSelectPage={onSelectPage}
    />,
  );
  const field = screen.getByRole("searchbox", { name: "搜索设置" });

  fireEvent.keyDown(field, { key: "Enter", isComposing: true });
  fireEvent.keyDown(field, { key: "Escape", isComposing: true });
  expect(onSelectPage).not.toHaveBeenCalled();
  expect(onChange).not.toHaveBeenCalled();

  fireEvent.keyDown(field, { key: "Escape" });
  expect(onChange).toHaveBeenCalledExactlyOnceWith("");
});
