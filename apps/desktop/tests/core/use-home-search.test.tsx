// @vitest-environment jsdom
import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, expect, test } from "vitest";
import { useHomeSearch } from "../../../../packages/ui/src/keyboard/use-home-search";
import type { Preferences } from "../../../../packages/ui/src/core/preferences-model";

afterEach(cleanup);

const preferences = {
  scheme: "quanpin",
  shuangpin_profile: "xiaohe",
  candidate_page_size: 5,
  learning: true,
  chinese_punctuation: true,
} as Preferences;

test("首页搜索跟随宿主页面与偏好刷新，并保留查询", () => {
  const { result, rerender } = renderHook(
    ({ pages, learning }) => useHomeSearch({ ...preferences, learning }, pages),
    { initialProps: { pages: [{ id: "dictionary", title: "词库" }], learning: true } },
  );
  act(() => result.current.setQuery("记忆新词已开"));
  expect(result.current.matches.map((row) => row.id)).toEqual(["dictionary"]);

  rerender({ pages: [{ id: "dictionary", title: "词库" }], learning: false });
  expect(result.current.query).toBe("记忆新词已开");
  expect(result.current.matches).toEqual([]);

  act(() => result.current.setQuery("AI"));
  rerender({ pages: [{ id: "ai", title: "AI 辅助" }], learning: false });
  expect(result.current.matches.map((row) => row.id)).toEqual(["ai"]);
  expect(result.current.groups.flat().map((row) => row.id)).toEqual(["ai"]);
});

test("首页搜索的各次调用独立持有查询，空白查询显示分组", () => {
  const pages = [{ id: "ai", title: "AI 辅助" }];
  const first = renderHook(() => useHomeSearch(preferences, pages));
  const second = renderHook(() => useHomeSearch(preferences, pages));
  act(() => first.result.current.setQuery("  ai  "));
  expect(first.result.current.needle).toBe("ai");
  expect(first.result.current.matches).toHaveLength(1);
  expect(second.result.current.query).toBe("");
  expect(second.result.current.matches).toEqual([]);

  act(() => first.result.current.setQuery("   "));
  expect(first.result.current.needle).toBe("");
  expect(first.result.current.matches).toEqual([]);
  expect(first.result.current.groups.flat()).toHaveLength(1);
});
